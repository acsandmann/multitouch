use super::inner::monitor_registry;
use crate::ffi::*;
use std::sync::Weak;

pub(super) unsafe extern "C" fn devices_added_callback(
    _refcon: *mut std::ffi::c_void,
    iterator: io_iterator_t,
) {
    let monitor = monitor_registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&iterator)
        .and_then(Weak::upgrade);
    if let Some(monitor) = monitor {
        monitor.handle_added(iterator);
    }
}

pub(super) unsafe extern "C" fn devices_removed_callback(
    _refcon: *mut std::ffi::c_void,
    iterator: io_iterator_t,
) {
    let monitor = monitor_registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&iterator)
        .and_then(Weak::upgrade);
    if let Some(monitor) = monitor {
        monitor.handle_removed(iterator);
    }
}
