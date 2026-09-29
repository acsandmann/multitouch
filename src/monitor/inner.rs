use super::hub::MonitorHub;
use crate::device::Device;
use crate::ffi::*;
use std::collections::HashMap;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub(super) struct MonitorState {
    pub(super) running: bool,
    pub(super) port: IONotificationPortRef,
    pub(super) added_iterator: io_iterator_t,
    pub(super) removed_iterator: io_iterator_t,
    pub(super) devices: HashMap<u64, Device>,
    pub(super) services: HashMap<io_service_t, u64>,
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
        }
    }
}

pub(super) struct MonitorInner {
    pub(super) state: Mutex<MonitorState>,
    pub(super) lifecycle_gate: Mutex<()>,
    pub(super) callback_gate: Mutex<()>,
    pub(super) hub: Arc<MonitorHub>,
}

unsafe impl Send for MonitorInner {}
unsafe impl Sync for MonitorInner {}

static MONITORS_BY_ITERATOR: OnceLock<Mutex<HashMap<io_iterator_t, Weak<MonitorInner>>>> =
    OnceLock::new();

pub(super) fn monitor_registry() -> &'static Mutex<HashMap<io_iterator_t, Weak<MonitorInner>>> {
    MONITORS_BY_ITERATOR.get_or_init(|| Mutex::new(HashMap::new()))
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

    pub(super) fn handle_added(&self, iterator: io_iterator_t) {
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
                unsafe {
                    let _ = IOObjectRetain(service);
                }
                // Frames go straight from the framework callback into the hub;
                // no per-device pump thread or extra queue hop is needed.
                let hub: Weak<MonitorHub> = Arc::downgrade(&self.hub);
                device.add_contact_sink(hub);

                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                state.devices.insert(device_id, device);
                state.services.insert(service, device_id);
            }

            // Release the reference returned by IOIteratorNext. Accepted services
            // hold one additional retain in `state.services` until removal/stop.
            unsafe {
                let _ = IOObjectRelease(service);
            }
        }
    }

    pub(super) fn handle_removed(&self, iterator: io_iterator_t) {
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
                state
                    .services
                    .remove(&service)
                    .and_then(|device_id| state.devices.remove(&device_id))
            };

            if let Some(device) = removed {
                device.stop();
                // Lifetime retain taken when the device was accepted.
                unsafe {
                    let _ = IOObjectRelease(service);
                }
            }

            // Iterator-owned reference.
            unsafe {
                let _ = IOObjectRelease(service);
            }
        }
    }

    pub(super) fn stop(&self) {
        let _lifecycle = self
            .lifecycle_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _gate = self.callback_gate.lock().unwrap_or_else(|e| e.into_inner());
        let (was_running, port, added, removed, devices, services) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let snapshot = (
                state.running,
                state.port,
                state.added_iterator,
                state.removed_iterator,
                std::mem::take(&mut state.devices),
                std::mem::take(&mut state.services),
            );
            state.running = false;
            state.port = ptr::null_mut();
            state.added_iterator = 0;
            state.removed_iterator = 0;
            snapshot
        };

        {
            let mut registry = monitor_registry().lock().unwrap_or_else(|e| e.into_inner());
            if added != 0 {
                registry.remove(&added);
            }
            if removed != 0 {
                registry.remove(&removed);
            }
        }

        self.hub.close();
        if !was_running {
            return;
        }

        for device in devices.values() {
            device.stop();
        }
        for service in services.keys().copied() {
            unsafe {
                let _ = IOObjectRelease(service);
            }
        }
        unsafe {
            if added != 0 {
                let _ = IOObjectRelease(added);
            }
            if removed != 0 {
                let _ = IOObjectRelease(removed);
            }
            if !port.is_null() {
                IONotificationPortDestroy(port);
            }
        }
    }
}
