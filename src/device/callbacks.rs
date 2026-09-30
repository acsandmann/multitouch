use super::inner::callback_inner;
use crate::ffi::*;
use crate::{Contact, ContactState, PathEvent};

pub(super) unsafe extern "C" fn contact_frame_callback(
    _device: MTDeviceRef,
    data: *mut Contact,
    count: i32,
    _timestamp: f64,
    _frame: i32,
    refcon: *mut std::ffi::c_void,
) {
    if count < 0 || (data.is_null() && count != 0) {
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

    // Borrow the framework buffer; only queue-backed consumers copy it.
    let contacts = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, count as usize) }
    };
    subscribers.retain(|weak| {
        if let Some(sink) = weak.upgrade() {
            sink.deliver(&inner, contacts);
            true
        } else {
            false
        }
    });
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
