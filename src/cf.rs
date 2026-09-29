use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use std::ffi::c_void;
use std::ptr::NonNull;

pub type Cf = CFRetained<CFType>;

pub fn string(value: &str) -> Cf {
    CFString::from_str(value).into()
}

pub fn number(value: f64) -> Cf {
    CFNumber::new_f64(value).into()
}

pub fn number_f32(value: f32) -> Cf {
    CFNumber::new_f32(value).into()
}

/// Builds a dictionary keyed by CFStrings created from `entries`' names.
pub fn dictionary(entries: &[(&str, Cf)]) -> CFRetained<CFDictionary> {
    let keys: Vec<CFRetained<CFString>> =
        entries.iter().map(|(k, _)| CFString::from_str(k)).collect();
    let key_refs: Vec<&CFString> = keys.iter().map(|k| &**k).collect();
    let value_refs: Vec<&CFType> = entries.iter().map(|(_, v)| &**v).collect();
    let dict = CFDictionary::<CFString, CFType>::from_slices(&key_refs, &value_refs);
    // SAFETY: the typed and untyped CFDictionary are the same object.
    unsafe { CFRetained::cast_unchecked::<CFDictionary>(dict) }
}

pub fn array(values: &[Cf]) -> CFRetained<CFArray> {
    let array = CFArray::<CFType>::from_retained_objects(values);
    // SAFETY: the typed and untyped CFArray are the same object.
    unsafe { CFRetained::cast_unchecked::<CFArray>(array) }
}

/// Increments the retain count of a raw CF object.
///
/// # Safety
/// `raw` must be null or a valid CoreFoundation object.
pub unsafe fn retain(raw: *const c_void) {
    if let Some(ptr) = NonNull::new(raw.cast_mut().cast::<CFType>()) {
        // Leak the extra reference; the caller now owns it.
        let _ = CFRetained::into_raw(unsafe { CFRetained::retain(ptr) });
    }
}

/// Decrements the retain count of a raw CF object.
///
/// # Safety
/// `raw` must be null or a valid CoreFoundation object the caller owns a reference to.
pub unsafe fn release(raw: *const c_void) {
    if let Some(ptr) = NonNull::new(raw.cast_mut().cast::<CFType>()) {
        drop(unsafe { CFRetained::from_raw(ptr) });
    }
}
