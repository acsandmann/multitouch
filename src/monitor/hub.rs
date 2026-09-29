use crate::Contact;
use crate::device::{ContactSink, Device, DeviceInner};
use crate::queue::Queue;
use std::sync::{Arc, Mutex, Weak};

pub(super) type Frame = (Device, Vec<Contact>);

pub(super) struct MonitorHub {
    pub(super) subscribers: Mutex<Vec<Weak<Queue<Frame>>>>,
}

impl MonitorHub {
    /// Fans a frame out to every subscriber. All but the last receive a clone;
    /// the last takes ownership, so the single-subscriber case never copies.
    fn broadcast(&self, device: &Arc<DeviceInner>, contacts: Vec<Contact>) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        if subscribers.is_empty() {
            return;
        }
        let mut pending: Option<Arc<Queue<Frame>>> = None;
        subscribers.retain(|weak| match weak.upgrade() {
            Some(queue) => {
                if let Some(previous) = pending.replace(queue) {
                    previous.push((
                        Device {
                            inner: Arc::clone(device),
                        },
                        contacts.clone(),
                    ));
                }
                true
            }
            None => false,
        });
        drop(subscribers);
        if let Some(last) = pending {
            last.push((
                Device {
                    inner: Arc::clone(device),
                },
                contacts,
            ));
        }
    }

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
    fn deliver(&self, device: &Arc<DeviceInner>, contacts: Vec<Contact>) {
        self.broadcast(device, contacts);
    }
}
