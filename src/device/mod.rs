mod callback_subscription;
mod callbacks;
mod inner;
mod properties;
mod stream;
mod subscribe;

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

pub(crate) use callback_subscription::gesture_handler;
pub use callback_subscription::{ContactEvent, ContactSubscription};
use inner::registry;
pub(crate) use inner::{ContactSink, DeviceInner};
use objc2_core_foundation::CFRetained;
pub use stream::{ContactStream, PathStream};

use crate::ffi::*;
use crate::haptics::Actuator;
use crate::{RunMode, cf, power};

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
        let mut registry = registry().write().unwrap_or_else(|e| e.into_inner());
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

    #[allow(clippy::should_implement_trait)] // Native default device can be unavailable.
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
