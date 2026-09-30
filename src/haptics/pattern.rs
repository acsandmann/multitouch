use crate::cf::{Cf, array, dictionary, number, number_f32, string};
use crate::{BaseWaveform, IntensityMultipliers, ToneWaveform};
use objc2_core_foundation::{CFDictionary, CFRetained};

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

    pub(super) fn to_cf_dictionary(&self) -> CFRetained<CFDictionary> {
        dictionary([
            (
                "BaseWaveform",
                Some(waveform_dictionary(self.base_waveform)),
            ),
            (
                "Tones",
                (!self.tones.is_empty()).then(|| {
                    let tones: Vec<_> = self.tones.iter().copied().map(tone_dictionary).collect();
                    array(&tones).into()
                }),
            ),
            (
                "BaseMultipliers",
                self.base_multipliers.map(multiplier_dictionary),
            ),
            (
                "ToneMultipliers",
                self.tone_multipliers.map(multiplier_dictionary),
            ),
        ])
    }
}

fn waveform_dictionary(waveform: BaseWaveform) -> Cf {
    dictionary([
        ("Type", Some(string(waveform.kind.as_str()))),
        ("DurationMS", Some(number(waveform.duration_ms))),
        ("Amplitude", Some(number(waveform.amplitude))),
    ])
    .into()
}

fn tone_dictionary(tone: ToneWaveform) -> Cf {
    dictionary([
        ("Type", Some(string(tone.kind.as_str()))),
        ("DelayMS", Some(number(tone.delay_ms))),
        ("DurationMS", Some(number(tone.duration_ms))),
        ("Amplitude", Some(number(tone.amplitude))),
        ("FrequencykHz", Some(number(tone.frequency_khz))),
    ])
    .into()
}

fn multiplier_dictionary(m: IntensityMultipliers) -> Cf {
    dictionary([
        ("Light", Some(number_f32(m.light))),
        ("Medium", Some(number_f32(m.medium))),
        ("Firm", Some(number_f32(m.firm))),
    ])
    .into()
}
