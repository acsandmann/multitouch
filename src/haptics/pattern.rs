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
        ("Light", number_f32(m.light)),
        ("Medium", number_f32(m.medium)),
        ("Firm", number_f32(m.firm)),
    ])
    .into()
}
