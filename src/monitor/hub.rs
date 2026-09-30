use crate::Contact;
use crate::device::{ContactSink, Device, DeviceInner};
use crate::queue::Queue;
use std::sync::{Arc, Mutex, Weak};

pub(super) type Frame = (Device, Vec<Contact>);

pub(super) struct MonitorHub {
    pub(super) subscribers: Mutex<Vec<Weak<Queue<Frame>>>>,
}

impl MonitorHub {
    pub(super) fn close(&self) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        for weak in subscribers.drain(..) {
            if let Some(queue) = weak.upgrade() {
                queue.close();
            }
        }
    }
}

impl ContactSink for MonitorHub {
    #[inline]
    fn deliver(&self, device: &Arc<DeviceInner>, contacts: &[Contact]) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subscribers.retain(|weak| {
            if let Some(queue) = weak.upgrade() {
                queue.push((
                    Device {
                        inner: Arc::clone(device),
                    },
                    contacts.to_vec(),
                ));
                true
            } else {
                false
            }
        });
    }
}
