use super::pattern::HapticPattern;
use crate::FeedbackPattern;
use crate::cf;
use crate::ffi::*;
use std::fmt;
use std::sync::Arc;

struct ActuatorInner {
    raw: MTActuatorRef,
}

unsafe impl Send for ActuatorInner {}
unsafe impl Sync for ActuatorInner {}

impl Drop for ActuatorInner {
    fn drop(&mut self) {
        unsafe {
            if MTActuatorIsOpen(self.raw) {
                let _ = MTActuatorClose(self.raw);
            }
            cf::release(self.raw.cast_const());
        }
    }
}

#[derive(Clone)]
pub struct Actuator {
    inner: Arc<ActuatorInner>,
}

impl Actuator {
    pub(crate) fn from_borrowed(raw: MTActuatorRef) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        // MTDeviceGetMTActuator follows the Get rule. Retain it here so an
        // Actuator remains valid even if the Device wrapper is dropped first.
        unsafe { cf::retain(raw.cast_const()) };
        Some(Self {
            inner: Arc::new(ActuatorInner { raw }),
        })
    }

    pub fn from_device_id(device_id: u64) -> Option<Self> {
        let raw = unsafe { MTActuatorCreateFromDeviceID(device_id) };
        (!raw.is_null()).then(|| Self {
            inner: Arc::new(ActuatorInner { raw }),
        })
    }

    pub fn is_open(&self) -> bool {
        unsafe { MTActuatorIsOpen(self.inner.raw) }
    }

    pub fn open(&self) -> bool {
        unsafe { MTActuatorOpen(self.inner.raw) == K_IO_RETURN_SUCCESS }
    }

    pub fn close(&self) -> bool {
        unsafe { MTActuatorClose(self.inner.raw) == K_IO_RETURN_SUCCESS }
    }

    pub fn system_actuations_enabled(&self) -> bool {
        unsafe { MTActuatorGetSystemActuationsEnabled(self.inner.raw) }
    }

    pub fn set_system_actuations_enabled(&self, enabled: bool) -> bool {
        unsafe {
            MTActuatorSetSystemActuationsEnabled(self.inner.raw, enabled) == K_IO_RETURN_SUCCESS
        }
    }

    pub fn actuate(&self, pattern: FeedbackPattern, intensity: f32) -> bool {
        if !self.is_open() {
            return false;
        }
        unsafe {
            MTActuatorActuate(
                self.inner.raw,
                pattern as i32,
                0,
                intensity.clamp(0.0, 1.0),
                1.0,
            ) == K_IO_RETURN_SUCCESS
        }
    }

    pub fn actuate_custom(&self, pattern: &HapticPattern) -> bool {
        if !self.is_open() {
            return false;
        }
        let dictionary = pattern.to_cf_dictionary();
        let actuation =
            unsafe { MTActuationCreateFromDictionary(dictionary.as_ref(), self.inner.raw) };
        if actuation.is_null() {
            return false;
        }
        let ok = unsafe { MTActuationActuate(actuation, self.inner.raw, 0) == K_IO_RETURN_SUCCESS };
        unsafe { cf::release(actuation) };
        ok
    }
}

impl fmt::Debug for Actuator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Actuator")
            .field("is_open", &self.is_open())
            .finish()
    }
}
