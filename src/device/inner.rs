use super::callbacks::{contact_frame_callback, path_callback};
use crate::ffi::*;
use crate::queue::Queue;
use crate::{Contact, PathEvent, RunMode};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::time::Duration;

/// Maps native device refs to their Rust state. The native callbacks fire at
/// the sensor frame rate and only ever read, so this is a `RwLock` to keep the
/// hot path from contending with itself (or with device creation/teardown).
static DEVICES_BY_REF: OnceLock<RwLock<HashMap<usize, Weak<DeviceInner>>>> = OnceLock::new();

pub(super) fn registry() -> &'static RwLock<HashMap<usize, Weak<DeviceInner>>> {
    DEVICES_BY_REF.get_or_init(|| RwLock::new(HashMap::new()))
}

#[inline]
pub(super) fn lookup(device: MTDeviceRef) -> Option<Arc<DeviceInner>> {
    registry()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(device as usize))
        .and_then(Weak::upgrade)
}

/// Receiver of raw contact frames, invoked directly on the framework's
/// callback thread. Implementations must be cheap and must not block.
pub(crate) trait ContactSink: Send + Sync {
    fn deliver(&self, device: &Arc<DeviceInner>, contacts: Vec<Contact>);

    /// Called when the device stops delivering frames to this sink.
    fn close(&self) {}
}

impl ContactSink for Queue<Vec<Contact>> {
    #[inline]
    fn deliver(&self, _device: &Arc<DeviceInner>, contacts: Vec<Contact>) {
        self.push(contacts);
    }

    fn close(&self) {
        Queue::close(self);
    }
}

pub(crate) struct DeviceInner {
    pub(crate) raw: MTDeviceRef,
    pub(super) contact_registered: AtomicBool,
    pub(super) path_registered: AtomicBool,
    pub(super) contact_subscribers: Mutex<Vec<Weak<dyn ContactSink>>>,
    pub(super) path_subscribers: Mutex<Vec<Weak<Queue<PathEvent>>>>,
    pub(super) run_gate: Mutex<()>,
    pub(super) auto_restart_on_wake: AtomicBool,
    pub(super) wanted_running: AtomicBool,
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
        // Sinks are dropped here; queue-backed sinks close when their last
        // strong reference (held by the stream) goes away, so close them explicitly.
        for weak in contacts.drain(..) {
            if let Some(sink) = weak.upgrade() {
                sink.close();
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
        let mut registry = registry().write().unwrap_or_else(|e| e.into_inner());
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
