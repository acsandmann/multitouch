use std::fmt;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector {
    pub position: Point,
    pub velocity: Point,
}

impl Vector {
    pub const fn new(position: Point, velocity: Point) -> Self {
        Self { position, velocity }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactState {
    NotTracking = 0,
    Starting = 1,
    Hovering = 2,
    Making = 3,
    Touching = 4,
    Breaking = 5,
    Lingering = 6,
    OutOfRange = 7,
}

impl ContactState {
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::NotTracking,
            1 => Self::Starting,
            2 => Self::Hovering,
            3 => Self::Making,
            4 => Self::Touching,
            5 => Self::Breaking,
            6 => Self::Lingering,
            _ => Self::OutOfRange,
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Making | Self::Touching)
    }
}

impl fmt::Display for ContactState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotTracking => "not tracking",
            Self::Starting => "starting",
            Self::Hovering => "hovering",
            Self::Making => "making",
            Self::Touching => "touching",
            Self::Breaking => "breaking",
            Self::Lingering => "lingering",
            Self::OutOfRange => "out of range",
        })
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finger {
    Thumb = 1,
    Index = 2,
    Middle = 3,
    Ring = 4,
    Pinky = 5,
}

impl Finger {
    pub const fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            1 => Some(Self::Thumb),
            2 => Some(Self::Index),
            3 => Some(Self::Middle),
            4 => Some(Self::Ring),
            5 => Some(Self::Pinky),
            _ => None,
        }
    }
}

impl fmt::Display for Finger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Thumb => "thumb",
            Self::Index => "index",
            Self::Middle => "middle",
            Self::Ring => "ring",
            Self::Pinky => "pinky",
        })
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Left = -1,
    Right = 1,
}

impl Hand {
    pub const fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            -1 => Some(Self::Left),
            1 => Some(Self::Right),
            _ => None,
        }
    }
}

impl fmt::Display for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

/// ABI-compatible contact structure emitted by MultitouchSupport.framework.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub frame: i32,
    pub timestamp: f64,
    pub id: i32,
    state_raw: i32,
    finger_raw: i32,
    hand_raw: i32,
    pub normalized: Vector,
    pub total_capacitance: f32,
    pub pressure: f32,
    pub angle: f32,
    pub major_axis: f32,
    pub minor_axis: f32,
    pub absolute: Vector,
    _reserved_0: f32,
    _reserved_1: f32,
    pub density: f32,
}

impl Contact {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        frame: i32,
        timestamp: f64,
        id: i32,
        state: ContactState,
        finger: Option<Finger>,
        hand: Option<Hand>,
        normalized: Vector,
        total_capacitance: f32,
        pressure: f32,
        angle: f32,
        major_axis: f32,
        minor_axis: f32,
        absolute: Vector,
        density: f32,
    ) -> Self {
        Self {
            frame,
            timestamp,
            id,
            state_raw: state as i32,
            finger_raw: match finger { Some(v) => v as i32, None => 0 },
            hand_raw: match hand { Some(v) => v as i32, None => 0 },
            normalized,
            total_capacitance,
            pressure,
            angle,
            major_axis,
            minor_axis,
            absolute,
            _reserved_0: 0.0,
            _reserved_1: 0.0,
            density,
        }
    }

    pub const fn state(&self) -> ContactState {
        ContactState::from_raw(self.state_raw)
    }

    pub const fn finger(&self) -> Option<Finger> {
        Finger::from_raw(self.finger_raw)
    }

    pub const fn hand(&self) -> Option<Hand> {
        Hand::from_raw(self.hand_raw)
    }

    pub const fn is_palm(&self) -> bool {
        self.finger().is_none() || self.hand().is_none()
    }

    #[doc(hidden)]
    pub const fn test_contact(x: f32, y: f32, id: i32, finger: Option<Finger>) -> Self {
        Self::new(
            0,
            0.0,
            id,
            ContactState::Touching,
            finger,
            Some(Hand::Right),
            Vector::new(Point::new(x, y), Point::ZERO),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            Vector::new(Point::ZERO, Point::ZERO),
            0.0,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathEvent {
    pub path_id: usize,
    pub stage: ContactState,
    pub contact: Contact,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunMode {
    #[default]
    Verbose = 0,
    LessVerbose = 0x1000_0000,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackPattern {
    Firm = 1,
    FirmStrong = 2,
    Medium = 3,
    MediumStrong = 4,
    Light = 5,
    LightStrong = 6,
    Click = 15,
    SecondaryClick = 16,
}

impl FeedbackPattern {
    pub const ALL: [Self; 8] = [
        Self::Firm,
        Self::FirmStrong,
        Self::Medium,
        Self::MediumStrong,
        Self::Light,
        Self::LightStrong,
        Self::Click,
        Self::SecondaryClick,
    ];
}

impl fmt::Display for FeedbackPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Firm => "firm",
            Self::FirmStrong => "firm, strong",
            Self::Medium => "medium",
            Self::MediumStrong => "medium, strong",
            Self::Light => "light",
            Self::LightStrong => "light, strong",
            Self::Click => "click",
            Self::SecondaryClick => "secondary click",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseWaveformType {
    None,
    Gaussian,
}

impl BaseWaveformType {
    pub(crate) const fn as_str(self) -> &'static str {
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
        Self { kind, duration_ms, amplitude }
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
    pub(crate) const fn as_str(self) -> &'static str {
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
        Self { kind, delay_ms, duration_ms, amplitude, frequency_khz }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntensityMultipliers {
    pub light: f32,
    pub medium: f32,
    pub firm: f32,
}

impl Default for IntensityMultipliers {
    fn default() -> Self {
        Self { light: 1.0, medium: 1.0, firm: 1.0 }
    }
}
