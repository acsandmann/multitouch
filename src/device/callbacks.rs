use super::inner::{callback_inner, ContactSink};
use crate::ffi::*;
use crate::{Contact, ContactState, PathEvent};
use std::sync::Arc;

pub(super) unsafe extern "C" fn contact_frame_callback(
    _device: MTDeviceRef,
    data: *mut Contact,
    count: i32,
    _timestamp: f64,
    _frame: i32,
    refcon: *mut std::ffi::c_void,
) {
    if data.is_null() || count < 0 {
        return;
    }
    let Some(inner) = (unsafe { callback_inner(refcon) }) else {
        return;
    };

    let mut subscribers = inner
        .contact_subscribers
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if subscribers.is_empty() {
        return;
    }

    // The framework reuses its buffer, so exactly one copy is made. Every
    // subscriber but the last gets a clone; the last takes ownership, so the
    // common single-subscriber case allocates once per frame.
    let contacts = unsafe { std::slice::from_raw_parts(data, count as usize) }.to_vec();
    let mut pending: Option<Arc<dyn ContactSink>> = None;
    subscribers.retain(|weak| match weak.upgrade() {
        Some(sink) => {
            if let Some(previous) = pending.replace(sink) {
                previous.deliver(&inner, contacts.clone());
            }
            true
        }
        None => false,
    });
    drop(subscribers);
    if let Some(last) = pending {
        last.deliver(&inner, contacts);
    }
}

pub(super) unsafe extern "C" fn path_callback(
    _device: MTDeviceRef,
    path_id: isize,
    stage: isize,
    contact: *mut Contact,
    refcon: *mut std::ffi::c_void,
) {
    if contact.is_null() {
        return;
    }
    let Some(inner) = (unsafe { callback_inner(refcon) }) else {
        return;
    };

    let event = PathEvent {
        path_id: path_id.max(0) as usize,
        stage: ContactState::from_raw(stage as i32),
        contact: unsafe { *contact },
    };
    let mut subscribers = inner
        .path_subscribers
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    subscribers.retain(|weak| {
        if let Some(queue) = weak.upgrade() {
            queue.push(event);
            true
        } else {
            false
        }
    });
}
