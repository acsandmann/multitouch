use crate::device::Device;
use crate::ffi::*;
use crate::queue::Queue;
use crate::Contact;
use std::collections::HashMap;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

struct MonitorHub {
    subscribers: Mutex<Vec<Weak<Queue<(Device, Vec<Contact>)>>>>,
}

impl MonitorHub {
    fn broadcast(&self, value: (Device, Vec<Contact>)) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subscribers.retain(|weak| {
            if let Some(queue) = weak.upgrade() {
                queue.push(value.clone());
                true
            } else {
                false
            }
        });
    }

    fn close(&self) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        for weak in subscribers.drain(..) {
            if let Some(queue) = weak.upgrade() {
                queue.close();
            }
        }
    }
}

struct MonitorState {
    running: bool,
    port: IONotificationPortRef,
    added_iterator: io_iterator_t,
    removed_iterator: io_iterator_t,
    devices: HashMap<u64, Device>,
    services: HashMap<io_service_t, u64>,
    threads: HashMap<u64, JoinHandle<()>>,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            running: false,
            port: ptr::null_mut(),
            added_iterator: 0,
            removed_iterator: 0,
            devices: HashMap::new(),
            services: HashMap::new(),
            threads: HashMap::new(),
        }
    }
}

struct MonitorInner {
    state: Mutex<MonitorState>,
    lifecycle_gate: Mutex<()>,
    callback_gate: Mutex<()>,
    hub: Arc<MonitorHub>,
}

unsafe impl Send for MonitorInner {}
unsafe impl Sync for MonitorInner {}

static MONITORS_BY_ITERATOR: OnceLock<Mutex<HashMap<io_iterator_t, Weak<MonitorInner>>>> = OnceLock::new();

fn monitor_registry() -> &'static Mutex<HashMap<io_iterator_t, Weak<MonitorInner>>> {
    MONITORS_BY_ITERATOR.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct Monitor {
    inner: Arc<MonitorInner>,
}

impl Default for Monitor {
    fn default() -> Self { Self::new() }
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MonitorInner {
                state: Mutex::new(MonitorState::default()),
                lifecycle_gate: Mutex::new(()),
                callback_gate: Mutex::new(()),
                hub: Arc::new(MonitorHub { subscribers: Mutex::new(Vec::new()) }),
            }),
        }
    }

    pub fn start(&self) -> bool {
        let _lifecycle = self.inner.lifecycle_gate.lock().unwrap_or_else(|e| e.into_inner());
        if self.inner.state.lock().unwrap_or_else(|e| e.into_inner()).running {
            return true;
        }

        let port = unsafe { IONotificationPortCreate(K_IO_MAIN_PORT_DEFAULT) };
        if port.is_null() {
            return false;
        }
        let queue = unsafe { dispatch_get_global_queue(0x21, 0) }; // QOS_CLASS_USER_INTERACTIVE
        unsafe { IONotificationPortSetDispatchQueue(port, queue) };

        let mut added_iterator = 0;
        let mut removed_iterator = 0;

        let add_matching = unsafe { IOServiceMatching(c"AppleMultitouchDevice".as_ptr()) };
        if add_matching.is_null() || unsafe {
            IOServiceAddMatchingNotification(
                port,
                c"IOServiceFirstMatch".as_ptr(),
                add_matching.cast_const(),
                Some(devices_added_callback),
                ptr::null_mut(),
                &mut added_iterator,
            )
        } != KERN_SUCCESS
        {
            unsafe { IONotificationPortDestroy(port) };
            return false;
        }

        let remove_matching = unsafe { IOServiceMatching(c"AppleMultitouchDevice".as_ptr()) };
        if remove_matching.is_null() || unsafe {
            IOServiceAddMatchingNotification(
                port,
                c"IOServiceTerminate".as_ptr(),
                remove_matching.cast_const(),
                Some(devices_removed_callback),
                ptr::null_mut(),
                &mut removed_iterator,
            )
        } != KERN_SUCCESS
        {
            unsafe {
                if added_iterator != 0 { let _ = IOObjectRelease(added_iterator); }
                IONotificationPortDestroy(port);
            }
            return false;
        }

        {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            state.running = true;
            state.port = port;
            state.added_iterator = added_iterator;
            state.removed_iterator = removed_iterator;
        }
        {
            let mut registry = monitor_registry().lock().unwrap_or_else(|e| e.into_inner());
            let weak = Arc::downgrade(&self.inner);
            registry.insert(added_iterator, weak.clone());
            registry.insert(removed_iterator, weak);
        }

        self.inner.handle_added(added_iterator);
        // Draining this once arms termination notifications.
        self.inner.handle_removed(removed_iterator);
        true
    }

    pub fn stop(&self) {
        self.inner.stop();
    }

    pub fn contacts(&self) -> MonitorStream {
        let queue = Queue::new(1);
        self.inner
            .hub
            .subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&queue));
        MonitorStream { queue }
    }

    pub fn active_devices(&self) -> Vec<Device> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .devices
            .values()
            .cloned()
            .collect()
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

impl MonitorInner {
    fn is_likely_trackpad(device: &Device) -> bool {
        if device.family_id() == Some(105) {
            return false;
        }
        !device
            .sensor_surface_dimensions()
            .is_some_and(|(width, height)| width > 1000 && height < 100)
    }

    fn handle_added(&self, iterator: io_iterator_t) {
        let _gate = self.callback_gate.lock().unwrap_or_else(|e| e.into_inner());
        if !self.state.lock().unwrap_or_else(|e| e.into_inner()).running {
            return;
        }
        loop {
            let service = unsafe { IOIteratorNext(iterator) };
            if service == 0 {
                break;
            }

            let maybe_device = Device::from_service(service);
            let accepted = maybe_device.and_then(|device| {
                let device_id = device.device_id()?;
                if !Self::is_likely_trackpad(&device) {
                    return None;
                }
                let already_tracked = self
                    .state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .devices
                    .contains_key(&device_id);
                if already_tracked || !device.start() {
                    return None;
                }
                Some((device_id, device))
            });

            if let Some((device_id, device)) = accepted {
                unsafe { let _ = IOObjectRetain(service); }
                let hub = Arc::clone(&self.hub);
                let thread_device = device.clone();
                let mut stream = device.contact_frames();
                let handle = std::thread::spawn(move || {
                    for contacts in &mut stream {
                        hub.broadcast((thread_device.clone(), contacts));
                    }
                });

                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                state.devices.insert(device_id, device);
                state.services.insert(service, device_id);
                state.threads.insert(device_id, handle);
            }

            // Release the reference returned by IOIteratorNext. Accepted services
            // hold one additional retain in `state.services` until removal/stop.
            unsafe { let _ = IOObjectRelease(service); }
        }
    }

    fn handle_removed(&self, iterator: io_iterator_t) {
        let _gate = self.callback_gate.lock().unwrap_or_else(|e| e.into_inner());
        if !self.state.lock().unwrap_or_else(|e| e.into_inner()).running {
            return;
        }
        loop {
            let service = unsafe { IOIteratorNext(iterator) };
            if service == 0 {
                break;
            }

            let removed = {
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                state.services.remove(&service).and_then(|device_id| {
                    let device = state.devices.remove(&device_id)?;
                    let handle = state.threads.remove(&device_id);
                    Some((device, handle))
                })
            };

            if let Some((device, handle)) = removed {
                device.stop();
                // Lifetime retain taken when the device was accepted.
                unsafe { let _ = IOObjectRelease(service); }
                if let Some(handle) = handle {
                    let _ = handle.join();
                }
            }

            // Iterator-owned reference.
            unsafe { let _ = IOObjectRelease(service); }
        }
    }

    fn stop(&self) {
        let _lifecycle = self.lifecycle_gate.lock().unwrap_or_else(|e| e.into_inner());
        let _gate = self.callback_gate.lock().unwrap_or_else(|e| e.into_inner());
        let (was_running, port, added, removed, devices, services, threads) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let snapshot = (
                state.running,
                state.port,
                state.added_iterator,
                state.removed_iterator,
                std::mem::take(&mut state.devices),
                std::mem::take(&mut state.services),
                std::mem::take(&mut state.threads),
            );
            state.running = false;
            state.port = ptr::null_mut();
            state.added_iterator = 0;
            state.removed_iterator = 0;
            snapshot
        };

        {
            let mut registry = monitor_registry().lock().unwrap_or_else(|e| e.into_inner());
            if added != 0 { registry.remove(&added); }
            if removed != 0 { registry.remove(&removed); }
        }

        self.hub.close();
        if !was_running {
            return;
        }

        for device in devices.values() {
            device.stop();
        }
        for service in services.keys().copied() {
            unsafe { let _ = IOObjectRelease(service); }
        }
        unsafe {
            if added != 0 { let _ = IOObjectRelease(added); }
            if removed != 0 { let _ = IOObjectRelease(removed); }
            if !port.is_null() { IONotificationPortDestroy(port); }
        }
        for (_, thread) in threads {
            let _ = thread.join();
        }
    }
}

unsafe extern "C" fn devices_added_callback(_refcon: *mut std::ffi::c_void, iterator: io_iterator_t) {
    let monitor = monitor_registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&iterator)
        .and_then(Weak::upgrade);
    if let Some(monitor) = monitor {
        monitor.handle_added(iterator);
    }
}

unsafe extern "C" fn devices_removed_callback(_refcon: *mut std::ffi::c_void, iterator: io_iterator_t) {
    let monitor = monitor_registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&iterator)
        .and_then(Weak::upgrade);
    if let Some(monitor) = monitor {
        monitor.handle_removed(iterator);
    }
}

pub struct MonitorStream {
    queue: Arc<Queue<(Device, Vec<Contact>)>>,
}

impl MonitorStream {
    pub fn recv(&self) -> Option<(Device, Vec<Contact>)> { self.queue.recv() }
    pub fn try_recv(&self) -> Option<(Device, Vec<Contact>)> { self.queue.try_recv() }
    pub fn recv_timeout(&self, timeout: Duration) -> Option<(Device, Vec<Contact>)> {
        self.queue.recv_timeout(timeout)
    }
}

impl Iterator for MonitorStream {
    type Item = (Device, Vec<Contact>);
    fn next(&mut self) -> Option<Self::Item> { self.queue.recv() }
}
