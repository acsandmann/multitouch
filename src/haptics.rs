use crate::cf::{self, Cf, array, dictionary, number, string};
use crate::ffi::*;
use crate::{BaseWaveform, FeedbackPattern, IntensityMultipliers, ToneWaveform};
use objc2_core_foundation::{CFDictionary, CFRetained};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct HapticPattern {
    pub base_waveform: BaseWaveform,
    pub tones: Vec<ToneWaveform>,
    pub base_multipliers: Option<IntensityMultipliers>,
    pub tone_multipliers: Option<IntensityMultipliers>,
}

impl HapticPattern {
    pub fn new(base_waveform: BaseWaveform) -> Self {
        Self {
            base_waveform,
            tones: Vec::new(),
            base_multipliers: None,
            tone_multipliers: None,
        }
    }

    pub fn with_tones(mut self, tones: impl IntoIterator<Item = ToneWaveform>) -> Self {
        self.tones = tones.into_iter().collect();
        self
    }

    pub fn with_base_multipliers(mut self, multipliers: IntensityMultipliers) -> Self {
        self.base_multipliers = Some(multipliers);
        self
    }

    pub fn with_tone_multipliers(mut self, multipliers: IntensityMultipliers) -> Self {
        self.tone_multipliers = Some(multipliers);
        self
    }

    fn to_cf_dictionary(&self) -> CFRetained<CFDictionary> {
        let mut entries = vec![("BaseWaveform", waveform_dictionary(self.base_waveform))];

        if !self.tones.is_empty() {
            let tones: Vec<Cf> = self.tones.iter().copied().map(tone_dictionary).collect();
            entries.push(("Tones", array(&tones).into()));
        }

        if let Some(multipliers) = self.base_multipliers {
            entries.push(("BaseMultipliers", multiplier_dictionary(multipliers)));
        }
        if let Some(multipliers) = self.tone_multipliers {
            entries.push(("ToneMultipliers", multiplier_dictionary(multipliers)));
        }

        dictionary(&entries)
    }
}

fn waveform_dictionary(waveform: BaseWaveform) -> Cf {
    dictionary(&[
        ("Type", string(waveform.kind.as_str())),
        ("DurationMS", number(waveform.duration_ms)),
        ("Amplitude", number(waveform.amplitude)),
    ])
    .into()
}

fn tone_dictionary(tone: ToneWaveform) -> Cf {
    dictionary(&[
        ("Type", string(tone.kind.as_str())),
        ("DelayMS", number(tone.delay_ms)),
        ("DurationMS", number(tone.duration_ms)),
        ("Amplitude", number(tone.amplitude)),
        ("FrequencykHz", number(tone.frequency_khz)),
    ])
    .into()
}

fn multiplier_dictionary(m: IntensityMultipliers) -> Cf {
    dictionary(&[
        ("Light", number(m.light as f64)),
        ("Medium", number(m.medium as f64)),
        ("Firm", number(m.firm as f64)),
    ])
    .into()
}

struct ActuatorInner {
    raw: MTActuatorRef,
    owns_ref: bool,
}

unsafe impl Send for ActuatorInner {}
unsafe impl Sync for ActuatorInner {}

impl Drop for ActuatorInner {
    fn drop(&mut self) {
        unsafe {
            if MTActuatorIsOpen(self.raw) {
                let _ = MTActuatorClose(self.raw);
            }
            if self.owns_ref {
                cf::release(self.raw.cast_const());
            }
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
            inner: Arc::new(ActuatorInner {
                raw,
                owns_ref: true,
            }),
        })
    }

    pub fn from_device_id(device_id: u64) -> Option<Self> {
        let raw = unsafe { MTActuatorCreateFromDeviceID(device_id) };
        (!raw.is_null()).then(|| Self {
            inner: Arc::new(ActuatorInner {
                raw,
                owns_ref: true,
            }),
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
