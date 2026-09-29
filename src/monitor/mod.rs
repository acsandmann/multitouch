mod callbacks;
mod hub;
mod inner;
mod stream;

pub use stream::MonitorStream;

use crate::device::Device;
use crate::ffi::*;
use crate::queue::Queue;
use callbacks::{devices_added_callback, devices_removed_callback};
use hub::MonitorHub;
use inner::{MonitorInner, MonitorState, monitor_registry};
use std::ptr;
use std::sync::{Arc, Mutex};

pub struct Monitor {
    inner: Arc<MonitorInner>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MonitorInner {
                state: Mutex::new(MonitorState::default()),
                lifecycle_gate: Mutex::new(()),
                callback_gate: Mutex::new(()),
                hub: Arc::new(MonitorHub {
                    subscribers: Mutex::new(Vec::new()),
                }),
            }),
        }
    }

    pub fn start(&self) -> bool {
        let _lifecycle = self
            .inner
            .lifecycle_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if self
            .inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .running
        {
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
        if add_matching.is_null()
            || unsafe {
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
        if remove_matching.is_null()
            || unsafe {
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
                if added_iterator != 0 {
                    let _ = IOObjectRelease(added_iterator);
                }
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
