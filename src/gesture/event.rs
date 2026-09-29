use crate::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FingerCountChange {
    Increased,
    Decreased,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureEndReason {
    Lifted,
    FingerCountChanged(FingerCountChange),
    TimedOut,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    Possible,
    Determining,
    Began,
    Changed,
    Ended(GestureEndReason),
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwipeEvent {
    pub phase: GesturePhase,
    pub translation: Point,
    pub velocity: Point,
    pub centroid: Point,
    pub angle: f32,
    pub distance: f32,
    pub finger_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MagnifyEvent {
    pub phase: GesturePhase,
    pub distance: f32,
    pub origin_distance: f32,
    pub velocity: f32,
    pub centroid: Point,
    pub finger_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationEvent {
    pub phase: GesturePhase,
    pub rotation: f32,
    pub velocity: f32,
    pub centroid: Point,
    pub finger_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureEvent {
    Determining {
        centroid: Point,
        finger_count: usize,
    },
    UnresolvedEnded(GestureEndReason),
    Swipe(SwipeEvent),
    Magnify(MagnifyEvent),
    Rotation(RotationEvent),
}

impl GestureEvent {
    pub const fn phase(self) -> GesturePhase {
        match self {
            Self::Determining { .. } => GesturePhase::Determining,
            Self::UnresolvedEnded(GestureEndReason::Cancelled) => GesturePhase::Cancelled,
            Self::UnresolvedEnded(reason) => GesturePhase::Ended(reason),
            Self::Swipe(event) => event.phase,
            Self::Magnify(event) => event.phase,
            Self::Rotation(event) => event.phase,
        }
    }
}
