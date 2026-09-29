#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseWaveformType {
    None,
    Gaussian,
}

impl BaseWaveformType {
    /// The string the framework expects for the waveform `Type` key.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Gaussian => "Gaussian",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BaseWaveform {
    pub kind: BaseWaveformType,
    pub duration_ms: f64,
    pub amplitude: f64,
}

impl BaseWaveform {
    pub const fn new(kind: BaseWaveformType, duration_ms: f64, amplitude: f64) -> Self {
        Self {
            kind,
            duration_ms,
            amplitude,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToneWaveformType {
    None,
    Sine,
    Square,
    Sawtooth,
}

impl ToneWaveformType {
    /// The string the framework expects for the tone `Type` key.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Sine => "Sine",
            Self::Square => "Square",
            Self::Sawtooth => "Sawtooth",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneWaveform {
    pub kind: ToneWaveformType,
    pub delay_ms: f64,
    pub duration_ms: f64,
    pub amplitude: f64,
    pub frequency_khz: f64,
}

impl ToneWaveform {
    pub const fn new(
        kind: ToneWaveformType,
        delay_ms: f64,
        duration_ms: f64,
        amplitude: f64,
        frequency_khz: f64,
    ) -> Self {
        Self {
            kind,
            delay_ms,
            duration_ms,
            amplitude,
            frequency_khz,
        }
    }

    /// A tone with no delay (Subsurface's default `delayMS = 0`).
    pub const fn immediate(
        kind: ToneWaveformType,
        duration_ms: f64,
        amplitude: f64,
        frequency_khz: f64,
    ) -> Self {
        Self::new(kind, 0.0, duration_ms, amplitude, frequency_khz)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntensityMultipliers {
    pub light: f32,
    pub medium: f32,
    pub firm: f32,
}

impl IntensityMultipliers {
    pub const fn new(light: f32, medium: f32, firm: f32) -> Self {
        Self {
            light,
            medium,
            firm,
        }
    }
}

impl Default for IntensityMultipliers {
    fn default() -> Self {
        Self::new(1.0, 1.0, 1.0)
    }
}
