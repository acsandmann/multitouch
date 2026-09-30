mod callbacks;
mod hub;
mod inner;
mod stream;

pub use stream::MonitorStream;

/// Events delivered directly on the native callback thread.
/// Contacts are borrowed and remain valid only for the duration of the handler.
pub enum MonitorEvent<'a> {
    Contacts {
        device: Device,
        contacts: &'a [crate::Contact],
    },
    DeviceRemoved(u64),
}

type EventHandler = Box<dyn for<'a> Fn(MonitorEvent<'a>) + Send + Sync>;

use std::ptr;
use std::sync::{Arc, Mutex};

use callbacks::{devices_added_callback, devices_removed_callback};
use hub::MonitorHub;
use inner::{MonitorInner, MonitorState, monitor_registry};

use crate::device::{ContactEvent, ContactSubscription, Device, gesture_handler};
use crate::ffi::*;
use crate::queue::Queue;
use crate::{GestureEvent, GestureRecognizer};

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
        Self::with_event_handler(None, None)
    }

    /// Receives contact frames and native device-removal notifications without
    /// a stream or pump thread. Keep the handler short and nonblocking; it must
    /// not start or stop the monitor or its devices from inside the callback.
    pub fn with_handler(handler: impl Fn(MonitorEvent<'_>) + Send + Sync + 'static) -> Self {
        Self::with_event_handler(Some(Box::new(handler)), None)
    }

    /// Creates a separate synchronous handler for each accepted physical device.
    /// Factories run at attachment; borrowed frames never pass through a queue.
    /// See `ContactSubscription` for callback synchronization rules. Stop/removal
    /// delivers `ContactEvent::Stopped`; restarting creates fresh handlers.
    pub fn with_device_handler<H>(factory: impl Fn(&Device) -> H + Send + Sync + 'static) -> Self
    where
        H: FnMut(ContactEvent<'_>) + Send + 'static,
    {
        Self::with_event_handler(
            None,
            Some(Box::new(move |device| {
                ContactSubscription::new(factory(device))
            })),
        )
    }

    /// Recognizes gestures inline, with independent state for each device.
    /// The handler can run concurrently for different devices. It must not
    /// start/stop the monitor or devices from inside a delivery.
    pub fn with_gesture_handler(
        recognizer: impl Fn(&Device) -> GestureRecognizer + Send + Sync + 'static,
        handler: impl Fn(&Device, GestureEvent) + Send + Sync + 'static,
    ) -> Self {
        let handler = Arc::new(handler);
        Self::with_device_handler(move |device| {
            let recognizer = recognizer(device);
            let device = device.clone();
            let handler = handler.clone();
            gesture_handler(recognizer, move |event| handler(&device, event))
        })
    }

    fn with_event_handler(
        handler: Option<EventHandler>,
        factory: Option<hub::DeviceFactory>,
    ) -> Self {
        Self {
            inner: Arc::new(MonitorInner {
                state: Mutex::new(MonitorState::default()),
                lifecycle_gate: Mutex::new(()),
                callback_gate: Mutex::new(()),
                hub: Arc::new(MonitorHub {
                    handler,
                    factory,
                    has_subscribers: std::sync::atomic::AtomicBool::new(false),
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
        self.inner
            .hub
            .has_subscribers
            .store(true, std::sync::atomic::Ordering::Release);
        MonitorStream { queue }
    }

    pub fn active_devices(&self) -> Vec<Device> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .devices
            .values()
            .map(|(device, _)| device.clone())
            .collect()
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.inner.stop();
    }
}
