use super::Device;
use super::callbacks::{contact_frame_callback, path_callback};
use super::inner::{ContactSink, registration_refcon, release_registration_refcon};
use super::stream::{ContactStream, PathStream};
use crate::Contact;
use crate::ffi::*;
use crate::queue::Queue;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

impl Device {
    pub fn contact_frames(&self) -> ContactStream {
        let queue: Arc<Queue<Vec<Contact>>> = Queue::new(1);
        let weak: Weak<Queue<Vec<Contact>>> = Arc::downgrade(&queue);
        let sink: Weak<dyn ContactSink> = weak;
        self.add_contact_sink(sink);
        ContactStream { queue }
    }

    /// Subscribes `sink` to raw frames and installs the native callback if this
    /// is the first subscriber.
    pub(crate) fn add_contact_sink(&self, sink: Weak<dyn ContactSink>) {
        self.inner
            .contact_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(sink);

        if !self.inner.contact_registered.swap(true, Ordering::AcqRel) {
            let refcon = registration_refcon(&self.inner);
            unsafe {
                MTRegisterContactFrameCallbackWithRefcon(
                    self.inner.raw,
                    Some(contact_frame_callback),
                    refcon,
                )
            };
        }
    }

    pub fn paths(&self) -> PathStream {
        let queue = Queue::new(32);
        self.inner
            .path_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&queue));
        if !self.inner.path_registered.swap(true, Ordering::AcqRel) {
            let refcon = registration_refcon(&self.inner);
            let registered = unsafe {
                MTRegisterPathCallbackWithRefcon(self.inner.raw, Some(path_callback), refcon)
            };
            if !registered {
                self.inner.path_registered.store(false, Ordering::Release);
                unsafe { release_registration_refcon(&self.inner) };
            }
        }
        PathStream { queue }
    }

    pub fn remove_contact_frame_callback(&self) {
        if self.inner.contact_registered.swap(false, Ordering::AcqRel) {
            unsafe {
                MTUnregisterContactFrameCallback(self.inner.raw, Some(contact_frame_callback))
            };
            unsafe { release_registration_refcon(&self.inner) };
        }
        let mut subscribers = self
            .inner
            .contact_subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for weak in subscribers.drain(..) {
            if let Some(sink) = weak.upgrade() {
                sink.close();
            }
        }
    }

    pub fn remove_path_callback(&self) {
        if self.inner.path_registered.swap(false, Ordering::AcqRel) {
            unsafe { MTUnregisterPathCallbackWithRefcon(self.inner.raw, Some(path_callback)) };
            unsafe { release_registration_refcon(&self.inner) };
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
}
