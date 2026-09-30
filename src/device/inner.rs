use super::callbacks::{contact_frame_callback, path_callback};
use crate::ffi::*;
use crate::queue::Queue;
use crate::{Contact, PathEvent, RunMode};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::time::Duration;

/// Shares Rust state when the same native device is wrapped more than once.
static DEVICES_BY_REF: OnceLock<RwLock<HashMap<usize, Weak<DeviceInner>>>> = OnceLock::new();

pub(super) fn registry() -> &'static RwLock<HashMap<usize, Weak<DeviceInner>>> {
    DEVICES_BY_REF.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Give each native registration a strong reference that keeps its refcon
/// valid until the matching unregister call completes.
pub(super) fn registration_refcon(inner: &Arc<DeviceInner>) -> *mut std::ffi::c_void {
    Arc::into_raw(Arc::clone(inner)) as *mut std::ffi::c_void
}

/// Recover an owned Arc for the duration of a callback. The registration's
/// strong reference keeps the allocation alive while this increment occurs.
#[inline]
pub(super) unsafe fn callback_inner(refcon: *mut std::ffi::c_void) -> Option<Arc<DeviceInner>> {
    let ptr = refcon.cast::<DeviceInner>();
    if ptr.is_null() {
        return None;
    }
    unsafe { Arc::increment_strong_count(ptr) };
    Some(unsafe { Arc::from_raw(ptr) })
}

/// Release the strong reference transferred to a native registration.
pub(super) unsafe fn release_registration_refcon(inner: &DeviceInner) {
    unsafe { drop(Arc::from_raw(inner as *const DeviceInner)) };
}

/// Receiver of raw contact frames, invoked directly on the framework's
/// callback thread. Implementations must be cheap and must not block.
pub(crate) trait ContactSink: Send + Sync {
    fn deliver(&self, device: &Arc<DeviceInner>, contacts: &[Contact]);

    /// Called when the device stops delivering frames to this sink.
    fn close(&self) {}
}

impl ContactSink for Queue<Vec<Contact>> {
    #[inline]
    fn deliver(&self, _device: &Arc<DeviceInner>, contacts: &[Contact]) {
        self.push(contacts.to_vec());
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
                release_registration_refcon(self);
            }
            if self.path_registered.swap(false, Ordering::AcqRel) {
                MTUnregisterPathCallbackWithRefcon(self.raw, Some(path_callback));
                release_registration_refcon(self);
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
