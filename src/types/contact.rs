use super::geometry::Vector;
use std::fmt;

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
            Self::NotTracking => "Not tracking",
            Self::Starting => "Starting",
            Self::Hovering => "Hovering",
            Self::Making => "Making",
            Self::Touching => "Touching",
            Self::Breaking => "Breaking",
            Self::Lingering => "Lingering",
            Self::OutOfRange => "Out of range",
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
            Self::Thumb => "Thumb",
            Self::Index => "Index",
            Self::Middle => "Middle",
            Self::Ring => "Ring",
            Self::Pinky => "Pinky",
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
            Self::Left => "Left",
            Self::Right => "Right",
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
            finger_raw: match finger {
                Some(v) => v as i32,
                None => 0,
            },
            hand_raw: match hand {
                Some(v) => v as i32,
                None => 0,
            },
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathEvent {
    pub path_id: usize,
    pub stage: ContactState,
    pub contact: Contact,
}
