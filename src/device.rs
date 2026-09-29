use crate::cf;
use crate::ffi::*;
use crate::haptics::Actuator;
use crate::power;
use crate::queue::Queue;
use crate::{Contact, ContactState, PathEvent, RunMode};
use objc2_core_foundation::{CFRetained, CFString};
use std::collections::HashMap;
use std::fmt;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak, mpsc};
use std::time::Duration;

static DEVICES_BY_REF: OnceLock<Mutex<HashMap<usize, Weak<DeviceInner>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<usize, Weak<DeviceInner>>> {
    DEVICES_BY_REF.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) struct DeviceInner {
    pub(crate) raw: MTDeviceRef,
    contact_registered: AtomicBool,
    path_registered: AtomicBool,
    contact_subscribers: Mutex<Vec<Weak<Queue<Vec<Contact>>>>>,
    path_subscribers: Mutex<Vec<Weak<Queue<PathEvent>>>>,
    run_gate: Mutex<()>,
    auto_restart_on_wake: AtomicBool,
    wanted_running: AtomicBool,
}

unsafe impl Send for DeviceInner {}
unsafe impl Sync for DeviceInner {}

impl DeviceInner {
    pub(crate) fn raw_restart_after_wake(&self) {
        let _run = self.run_gate.lock().unwrap_or_else(|e| e.into_inner());
        if !self.auto_restart_on_wake.load(Ordering::Acquire)
            || !self.wanted_running.load(Ordering::Acquire)
        {
            return;
        }
        unsafe {
            let _ = MTDeviceStop(self.raw);
        }
        std::thread::sleep(Duration::from_secs(1));
        if self.wanted_running.load(Ordering::Acquire) {
            unsafe {
                let _ = MTDeviceStart(self.raw, RunMode::Verbose as i32);
            }
        }
    }

    fn close_streams(&self) {
        let mut contacts = self
            .contact_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for weak in contacts.drain(..) {
            if let Some(queue) = weak.upgrade() {
                queue.close();
            }
        }
        let mut paths = self
            .path_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for weak in paths.drain(..) {
            if let Some(queue) = weak.upgrade() {
                queue.close();
            }
        }
    }
}

impl Drop for DeviceInner {
    fn drop(&mut self) {
        self.wanted_running.store(false, Ordering::Release);
        unsafe {
            if self.contact_registered.swap(false, Ordering::AcqRel) {
                MTUnregisterContactFrameCallback(self.raw, Some(contact_frame_callback));
            }
            if self.path_registered.swap(false, Ordering::AcqRel) {
                MTUnregisterPathCallback(self.raw, Some(path_callback));
            }
            let _ = MTDeviceStop(self.raw);
        }
        self.close_streams();
        let mut registry = registry().lock().unwrap_or_else(|e| e.into_inner());
        let key = self.raw as usize;
        if registry
            .get(&key)
            .is_some_and(|weak| std::ptr::eq(weak.as_ptr(), self as *const DeviceInner))
        {
            registry.remove(&key);
        }
        drop(registry);
        unsafe { MTDeviceRelease(self.raw) };
    }
}

#[derive(Clone)]
pub struct Device {
    pub(crate) inner: Arc<DeviceInner>,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    fn from_owned_raw(raw: MTDeviceRef) -> Option<Self> {
        if raw.is_null() {
            return None;
        }

        // Create/Get functions hand us ownership. If this exact native object is
        // already wrapped, share the Rust state and balance the newly acquired
        // native reference immediately. This also guarantees one callback
        // registration per MTDeviceRef.
        let mut registry = registry().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = registry.get(&(raw as usize)).and_then(Weak::upgrade) {
            drop(registry);
            unsafe { MTDeviceRelease(raw) };
            return Some(Self { inner: existing });
        }

        let inner = Arc::new(DeviceInner {
            raw,
            contact_registered: AtomicBool::new(false),
            path_registered: AtomicBool::new(false),
            contact_subscribers: Mutex::new(Vec::new()),
            path_subscribers: Mutex::new(Vec::new()),
            run_gate: Mutex::new(()),
            auto_restart_on_wake: AtomicBool::new(true),
            wanted_running: AtomicBool::new(false),
        });
        registry.insert(raw as usize, Arc::downgrade(&inner));
        drop(registry);
        Some(Self { inner })
    }

    pub fn is_available() -> bool {
        unsafe { MTDeviceIsAvailable() }
    }

    pub fn default() -> Option<Self> {
        Self::from_owned_raw(unsafe { MTDeviceCreateDefault() })
    }

    pub fn all() -> Vec<Self> {
        // MTDeviceCreateList follows the Create rule; the array is released on drop.
        let Some(list) = (unsafe { MTDeviceCreateList() }) else {
            return Vec::new();
        };
        let list = unsafe { CFRetained::from_raw(list) };
        let count = list.count();
        let mut devices = Vec::with_capacity(count.max(0) as usize);
        for index in 0..count {
            let raw = unsafe { list.value_at_index(index) } as MTDeviceRef;
            if raw.is_null() {
                continue;
            }
            unsafe { cf::retain(raw.cast_const()) };
            if let Some(device) = Self::from_owned_raw(raw) {
                devices.push(device);
            }
        }
        devices
    }

    pub fn current_absolute_time() -> f64 {
        unsafe { MTAbsoluteTimeGetCurrent() }
    }

    pub fn from_device_id(device_id: u64) -> Option<Self> {
        Self::from_owned_raw(unsafe { MTDeviceCreateFromDeviceID(device_id) })
    }

    pub fn from_service(service: u32) -> Option<Self> {
        Self::from_owned_raw(unsafe { MTDeviceCreateFromService(service) })
    }

    pub fn start(&self) -> bool {
        let _run = self
            .inner
            .run_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let ok = unsafe { MTDeviceStart(self.inner.raw, RunMode::Verbose as i32) == 0 };
        if ok {
            self.inner.wanted_running.store(true, Ordering::Release);
            power::watch(&self.inner);
        }
        ok
    }

    pub fn stop(&self) -> bool {
        let _run = self
            .inner
            .run_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.inner.wanted_running.store(false, Ordering::Release);
        power::unwatch(&self.inner);
        self.remove_contact_frame_callback();
        self.remove_path_callback();
        unsafe { MTDeviceStop(self.inner.raw) == 0 }
    }

    pub fn is_running(&self) -> bool {
        unsafe { MTDeviceIsRunning(self.inner.raw) }
    }
    pub fn is_built_in(&self) -> bool {
        unsafe { MTDeviceIsBuiltIn(self.inner.raw) }
    }
    pub fn is_opaque_surface(&self) -> bool {
        unsafe { MTDeviceIsOpaqueSurface(self.inner.raw) }
    }
    pub fn is_alive(&self) -> bool {
        unsafe { MTDeviceIsAlive(self.inner.raw) }
    }
    pub fn is_hid_device(&self) -> bool {
        unsafe { MTDeviceIsMTHIDDevice(self.inner.raw) }
    }
    pub fn supports_force(&self) -> bool {
        unsafe { MTDeviceSupportsForce(self.inner.raw) }
    }
    pub fn supports_actuation(&self) -> bool {
        unsafe { MTDeviceSupportsActuation(self.inner.raw) }
    }
    pub fn is_driver_ready(&self) -> bool {
        unsafe { MTDeviceDriverIsReady(self.inner.raw) }
    }
    pub fn supports_power_control(&self) -> bool {
        unsafe { MTDevicePowerControlSupported(self.inner.raw) }
    }

    pub fn service(&self) -> u32 {
        unsafe { MTDeviceGetService(self.inner.raw) }
    }

    pub fn sensor_surface_dimensions(&self) -> Option<(i32, i32)> {
        let (mut width, mut height) = (0, 0);
        (unsafe { MTDeviceGetSensorSurfaceDimensions(self.inner.raw, &mut width, &mut height) }
            == 0)
            .then_some((width, height))
    }

    pub fn sensor_dimensions(&self) -> Option<(i32, i32)> {
        let (mut rows, mut columns) = (0, 0);
        (unsafe { MTDeviceGetSensorDimensions(self.inner.raw, &mut rows, &mut columns) } == 0)
            .then_some((rows, columns))
    }

    fn get_i32(&self, f: unsafe extern "C" fn(MTDeviceRef, *mut i32) -> OSStatus) -> Option<i32> {
        let mut value = 0;
        (unsafe { f(self.inner.raw, &mut value) } == 0).then_some(value)
    }

    pub fn family_id(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetFamilyID)
    }
    pub fn version(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetVersion)
    }
    pub fn driver_type(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetDriverType)
    }
    pub fn transport_method(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetTransportMethod)
    }

    pub fn device_id(&self) -> Option<u64> {
        let mut value = 0;
        (unsafe { MTDeviceGetDeviceID(self.inner.raw, &mut value) } == 0).then_some(value)
    }

    pub fn serial_number(&self) -> Option<String> {
        let mut value: *const CFString = ptr::null();
        if unsafe { MTDeviceGetSerialNumber(self.inner.raw, &mut value) } != 0 {
            return None;
        }
        // Get rule: the string is borrowed from the device.
        unsafe { value.as_ref() }.map(ToString::to_string)
    }

    pub fn name(&self) -> String {
        match self.family_id() {
            Some(98 | 99 | 100 | 101 | 102 | 103 | 104 | 108 | 109) => "MacBook Trackpad".into(),
            Some(105) => "Touch Bar".into(),
            Some(112 | 113) => "Magic Mouse".into(),
            Some(128 | 129 | 130) => "Magic Trackpad".into(),
            family => {
                let id = family
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".into());
                if self
                    .sensor_surface_dimensions()
                    .is_some_and(|(w, h)| w > 1000 && h < 100)
                {
                    format!("Unknown Touch Bar (family ID: {id})")
                } else {
                    format!("Unknown Device (family ID: {id})")
                }
            }
        }
    }

    pub fn system_force_response_enabled(&self) -> bool {
        unsafe { MTDeviceGetSystemForceResponseEnabled(self.inner.raw) }
    }

    pub fn set_system_force_response_enabled(&self, enabled: bool) {
        unsafe { MTDeviceSetSystemForceResponseEnabled(self.inner.raw, enabled) };
    }

    pub fn supports_silent_click(&self) -> bool {
        let mut supported = false;
        unsafe { MTDeviceSupportsSilentClick(self.inner.raw, &mut supported) == 0 && supported }
    }

    pub fn power_enabled(&self) -> bool {
        let mut enabled = false;
        unsafe { MTDevicePowerGetEnabled(self.inner.raw, &mut enabled) };
        enabled
    }

    pub fn set_power_enabled(&self, enabled: bool) -> bool {
        unsafe { MTDevicePowerSetEnabled(self.inner.raw, enabled) == 0 }
    }

    pub fn auto_restart_on_wake(&self) -> bool {
        self.inner.auto_restart_on_wake.load(Ordering::Acquire)
    }

    pub fn set_auto_restart_on_wake(&self, enabled: bool) {
        self.inner
            .auto_restart_on_wake
            .store(enabled, Ordering::Release);
    }

    pub fn system_actuations_enabled(&self) -> Option<bool> {
        let actuator = unsafe { MTDeviceGetMTActuator(self.inner.raw) };
        (!actuator.is_null()).then(|| unsafe { MTActuatorGetSystemActuationsEnabled(actuator) })
    }

    pub fn set_system_actuations_enabled(&self, enabled: bool) -> bool {
        let actuator = unsafe { MTDeviceGetMTActuator(self.inner.raw) };
        !actuator.is_null()
            && unsafe { MTActuatorSetSystemActuationsEnabled(actuator, enabled) == 0 }
    }

    pub fn contact_frames(&self) -> ContactStream {
        let queue = Queue::new(1);
        self.inner
            .contact_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&queue));

        if !self.inner.contact_registered.swap(true, Ordering::AcqRel) {
            unsafe { MTRegisterContactFrameCallback(self.inner.raw, Some(contact_frame_callback)) };
        }
        ContactStream { queue }
    }

    pub fn paths(&self) -> PathStream {
        let queue = Queue::new(32);
        self.inner
            .path_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&queue));
        if !self.inner.path_registered.swap(true, Ordering::AcqRel) {
            unsafe { MTRegisterPathCallback(self.inner.raw, Some(path_callback)) };
        }
        PathStream { queue }
    }

    pub fn remove_contact_frame_callback(&self) {
        if self.inner.contact_registered.swap(false, Ordering::AcqRel) {
            unsafe {
                MTUnregisterContactFrameCallback(self.inner.raw, Some(contact_frame_callback))
            };
        }
        let mut subscribers = self
            .inner
            .contact_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for weak in subscribers.drain(..) {
            if let Some(queue) = weak.upgrade() {
                queue.close();
            }
        }
    }

    pub fn remove_path_callback(&self) {
        if self.inner.path_registered.swap(false, Ordering::AcqRel) {
            unsafe { MTUnregisterPathCallback(self.inner.raw, Some(path_callback)) };
        }
        let mut subscribers = self
            .inner
            .path_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for weak in subscribers.drain(..) {
            if let Some(queue) = weak.upgrade() {
                queue.close();
            }
        }
    }

    pub fn actuator(&self) -> Option<Actuator> {
        let raw = unsafe { MTDeviceGetMTActuator(self.inner.raw) };
        Actuator::from_borrowed(raw)
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Device")
            .field("name", &self.name())
            .field("device_id", &self.device_id())
            .field("family_id", &self.family_id())
            .field("is_running", &self.is_running())
            .finish()
    }
}

unsafe extern "C" fn contact_frame_callback(
    device: MTDeviceRef,
    data: *mut Contact,
    count: i32,
    _timestamp: f64,
    _frame: i32,
) -> i32 {
    if data.is_null() || count < 0 {
        return 0;
    }
    let inner = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(device as usize))
        .and_then(Weak::upgrade);
    let Some(inner) = inner else { return 0 };

    let contacts = unsafe { std::slice::from_raw_parts(data, count as usize) }.to_vec();
    let mut subscribers = inner
        .contact_subscribers
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    subscribers.retain(|weak| {
        if let Some(queue) = weak.upgrade() {
            queue.push(contacts.clone());
            true
        } else {
            false
        }
    });
    0
}

unsafe extern "C" fn path_callback(
    device: MTDeviceRef,
    path_id: isize,
    stage: isize,
    contact: *mut Contact,
) {
    if contact.is_null() {
        return;
    }
    let inner = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(device as usize))
        .and_then(Weak::upgrade);
    let Some(inner) = inner else { return };

    let event = PathEvent {
        path_id: path_id.max(0) as usize,
        stage: ContactState::from_raw(stage as i32),
        contact: unsafe { *contact },
    };
    let mut subscribers = inner
        .path_subscribers
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    subscribers.retain(|weak| {
        if let Some(queue) = weak.upgrade() {
            queue.push(event);
            true
        } else {
            false
        }
    });
}

pub struct ContactStream {
    queue: Arc<Queue<Vec<Contact>>>,
}

impl ContactStream {
    pub fn recv(&self) -> Option<Vec<Contact>> {
        self.queue.recv()
    }
    pub fn try_recv(&self) -> Option<Vec<Contact>> {
        self.queue.try_recv()
    }
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Vec<Contact>> {
        self.queue.recv_timeout(timeout)
    }

    pub fn into_receiver(self) -> mpsc::Receiver<Vec<Contact>> {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut stream = self;
            for frame in &mut stream {
                if tx.send(frame).is_err() {
                    break;
                }
            }
        });
        rx
    }
}

impl Iterator for ContactStream {
    type Item = Vec<Contact>;
    fn next(&mut self) -> Option<Self::Item> {
        self.queue.recv()
    }
}

pub struct PathStream {
    queue: Arc<Queue<PathEvent>>,
}

impl PathStream {
    pub fn recv(&self) -> Option<PathEvent> {
        self.queue.recv()
    }
    pub fn try_recv(&self) -> Option<PathEvent> {
        self.queue.try_recv()
    }
}

impl Iterator for PathStream {
    type Item = PathEvent;
    fn next(&mut self) -> Option<Self::Item> {
        self.queue.recv()
    }
}
