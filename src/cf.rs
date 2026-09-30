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

/// Builds a dictionary with stack-backed temporary storage, omitting absent values.
pub fn dictionary<const N: usize>(entries: [(&str, Option<Cf>); N]) -> CFRetained<CFDictionary> {
    let keys: [_; N] = std::array::from_fn(|i| {
        entries[i]
            .1
            .as_ref()
            .map(|_| CFString::from_str(entries[i].0))
    });
    let mut pairs = keys
        .iter()
        .zip(&entries)
        .filter_map(|(key, (_, value))| Some((key.as_deref()?, value.as_deref()?)));
    let dict = if let Some((key, value)) = pairs.next() {
        let mut key_refs = [key; N];
        let mut value_refs = [value; N];
        let mut len = 1;
        for (key, value) in pairs {
            key_refs[len] = key;
            value_refs[len] = value;
            len += 1;
        }
        CFDictionary::<CFString, CFType>::from_slices(&key_refs[..len], &value_refs[..len])
    } else {
        CFDictionary::<CFString, CFType>::from_slices(&[], &[])
    };
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
