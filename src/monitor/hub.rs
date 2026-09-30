use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use super::{EventHandler, MonitorEvent};
use crate::Contact;
use crate::device::{ContactSink, ContactSubscription, Device, DeviceInner};
use crate::queue::Queue;
pub(super) type DeviceFactory = Box<dyn Fn(&Device) -> ContactSubscription + Send + Sync>;

pub(super) type Frame = (Device, Vec<Contact>);

pub(super) struct MonitorHub {
    pub(super) handler: Option<EventHandler>,
    pub(super) factory: Option<DeviceFactory>,
    pub(super) has_subscribers: AtomicBool,
    pub(super) subscribers: Mutex<Vec<Weak<Queue<Frame>>>>,
}

struct DeviceSink {
    subscription: ContactSubscription,
    hub: Arc<MonitorHub>,
}
impl ContactSink for DeviceSink {
    fn deliver(&self, device: &Arc<DeviceInner>, contacts: &[Contact]) {
        self.subscription.sink.deliver(device, contacts);
        self.hub.deliver(device, contacts);
    }

    fn close(&self) {
        self.subscription.sink.close();
    }

    fn reset(&self) {
        self.subscription.sink.reset();
    }
}

impl MonitorHub {
    pub(super) fn sink(self: &Arc<Self>, device: &Device) -> Arc<dyn ContactSink> {
        match &self.factory {
            Some(factory) => Arc::new(DeviceSink {
                subscription: factory(device),
                hub: self.clone(),
            }),
            None => self.clone(),
        }
    }

    pub(super) fn removed(&self, id: u64) {
        if let Some(handler) = &self.handler {
            handler(MonitorEvent::DeviceRemoved(id));
        }
    }

    pub(super) fn close(&self) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        self.has_subscribers.store(false, Ordering::Release);
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
        if let Some(handler) = &self.handler {
            handler(MonitorEvent::Contacts {
                device: Device {
                    inner: Arc::clone(device),
                },
                contacts,
            });
        }
        if !self.has_subscribers.load(Ordering::Acquire) {
            return;
        }
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
        if subscribers.is_empty() {
            self.has_subscribers.store(false, Ordering::Release);
        }
    }
}
