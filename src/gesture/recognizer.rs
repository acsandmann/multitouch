use super::emit::angle_difference;
use super::event::{FingerCountChange, GestureEndReason, GestureEvent, GesturePhase};
use super::frame::Frame;
use super::kinds::GestureTypes;
use crate::{Contact, Point};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GestureKind {
    Swipe,
    Magnify,
    Rotation,
}

#[derive(Debug)]
pub struct GestureRecognizer {
    pub required_finger_count: usize,
    pub requires_exact_finger_count_to_continue: bool,
    pub disallow_activation_from_finger_count_decrease: bool,
    pub recognized_gesture_types: GestureTypes,
    pub minimum_swipe_translation: f32,
    pub minimum_magnification_distance: f32,
    pub minimum_rotation: f32,
    pub inactivity_timeout: Duration,

    pub(super) phase: GesturePhase,
    pub(super) gesture_kind: Option<GestureKind>,
    pub(super) origin_centroid: Option<Point>,
    pub(super) origin_distance: Option<f32>,
    pub(super) origin_angle: Option<f32>,
    pub(super) last_centroid: Option<Point>,
    pub(super) last_distance: Option<f32>,
    pub(super) last_angle: Option<f32>,
    pub(super) last_event_time: Option<Instant>,
    pub(super) previous_active_finger_count: usize,
    pub(super) finger_count_decreased_in_current_touch_sequence: bool,
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new(2)
    }
}

impl GestureRecognizer {
    pub fn new(finger_count: usize) -> Self {
        Self {
            required_finger_count: finger_count,
            requires_exact_finger_count_to_continue: false,
            disallow_activation_from_finger_count_decrease: true,
            recognized_gesture_types: GestureTypes::ALL,
            minimum_swipe_translation: 0.08,
            minimum_magnification_distance: 0.1,
            minimum_rotation: 0.15,
            inactivity_timeout: Duration::from_millis(250),
            phase: GesturePhase::Possible,
            gesture_kind: None,
            origin_centroid: None,
            origin_distance: None,
            origin_angle: None,
            last_centroid: None,
            last_distance: None,
            last_angle: None,
            last_event_time: None,
            previous_active_finger_count: 0,
            finger_count_decreased_in_current_touch_sequence: false,
        }
    }

    pub fn with_gesture_types(mut self, types: GestureTypes) -> Self {
        self.recognized_gesture_types = types;
        self
    }

    pub fn with_exact_finger_count(mut self, exact: bool) -> Self {
        self.requires_exact_finger_count_to_continue = exact;
        self
    }

    pub fn process(&mut self, contacts: &[Contact]) -> Option<GestureEvent> {
        self.process_at(contacts, Instant::now())
    }

    fn process_at(&mut self, contacts: &[Contact], now: Instant) -> Option<GestureEvent> {
        let mut frame = Frame::measure(contacts);
        let count = frame.count;

        if count == 0 {
            self.previous_active_finger_count = 0;
            self.finger_count_decreased_in_current_touch_sequence = false;
        } else {
            if count < self.previous_active_finger_count {
                self.finger_count_decreased_in_current_touch_sequence = true;
            }
            self.previous_active_finger_count = count;
        }

        if self.gesture_kind.is_some() {
            if !self.should_continue_resolved_gesture(count) {
                let reason = if count < 2 {
                    GestureEndReason::Lifted
                } else {
                    GestureEndReason::FingerCountChanged(if count > self.required_finger_count {
                        FingerCountChange::Increased
                    } else {
                        FingerCountChange::Decreased
                    })
                };
                let event = self.make_end_event(reason, count, now);
                self.reset_state();
                return event;
            }
            if self.gesture_kind != Some(GestureKind::Swipe) {
                frame.measure_geometry(contacts);
            }
            return self.emit_tracked(&frame, GesturePhase::Changed, now);
        }

        if count != self.required_finger_count {
            if self.phase == GesturePhase::Determining {
                let event = if count < self.required_finger_count {
                    GestureEvent::UnresolvedEnded(GestureEndReason::Lifted)
                } else {
                    GestureEvent::UnresolvedEnded(GestureEndReason::Cancelled)
                };
                self.reset_state();
                return Some(event);
            }
            return None;
        }

        if self.disallow_activation_from_finger_count_decrease
            && self.finger_count_decreased_in_current_touch_sequence
        {
            return None;
        }

        frame.measure_geometry(contacts);
        let (centroid, distance, angle) = (frame.centroid, frame.distance, frame.angle);

        let (origin_centroid, origin_distance, origin_angle) = match (
            self.origin_centroid,
            self.origin_distance,
            self.origin_angle,
        ) {
            (Some(c), Some(d), Some(a)) => (c, d, a),
            _ => {
                self.origin_centroid = Some(centroid);
                self.origin_distance = Some(distance);
                self.origin_angle = Some(angle);
                self.last_centroid = Some(centroid);
                self.last_distance = Some(distance);
                self.last_angle = Some(angle);
                self.last_event_time = Some(now);
                self.phase = GesturePhase::Determining;
                return Some(GestureEvent::Determining {
                    centroid,
                    finger_count: count,
                });
            }
        };

        self.gesture_kind = if self
            .recognized_gesture_types
            .contains(GestureTypes::MAGNIFY)
            && (distance - origin_distance).abs() > self.minimum_magnification_distance
        {
            Some(GestureKind::Magnify)
        } else if self
            .recognized_gesture_types
            .contains(GestureTypes::ROTATION)
            && angle_difference(origin_angle, angle).abs() > self.minimum_rotation
        {
            Some(GestureKind::Rotation)
        } else if self.recognized_gesture_types.contains(GestureTypes::SWIPE)
            && (centroid.x - origin_centroid.x).hypot(centroid.y - origin_centroid.y)
                > self.minimum_swipe_translation
        {
            Some(GestureKind::Swipe)
        } else {
            None
        };

        if self.gesture_kind.is_none() {
            self.last_centroid = Some(centroid);
            self.last_distance = Some(distance);
            self.last_angle = Some(angle);
            self.last_event_time = Some(now);
            return Some(GestureEvent::Determining {
                centroid,
                finger_count: count,
            });
        }

        self.emit_tracked(&frame, GesturePhase::Began, now)
    }

    pub fn reset(&mut self) -> Option<GestureEvent> {
        let event = if matches!(self.phase, GesturePhase::Began | GesturePhase::Changed) {
            self.make_end_event(
                GestureEndReason::Cancelled,
                self.previous_active_finger_count,
                Instant::now(),
            )
        } else if self.phase == GesturePhase::Determining {
            Some(GestureEvent::UnresolvedEnded(GestureEndReason::Cancelled))
        } else {
            None
        };
        self.reset_state();
        event
    }

    pub fn timeout(&mut self) -> Option<GestureEvent> {
        let event = if matches!(self.phase, GesturePhase::Began | GesturePhase::Changed) {
            self.make_end_event(
                GestureEndReason::TimedOut,
                self.previous_active_finger_count,
                Instant::now(),
            )
        } else if self.phase == GesturePhase::Determining {
            Some(GestureEvent::UnresolvedEnded(GestureEndReason::TimedOut))
        } else {
            None
        };
        if event.is_some() {
            self.reset_state();
        }
        event
    }

    fn should_continue_resolved_gesture(&self, count: usize) -> bool {
        if self.requires_exact_finger_count_to_continue {
            count == self.required_finger_count
        } else {
            count >= 2
        }
    }

    fn emit_tracked(
        &mut self,
        frame: &Frame,
        phase: GesturePhase,
        now: Instant,
    ) -> Option<GestureEvent> {
        let (centroid, distance, angle) = (frame.centroid, frame.distance, frame.angle);
        self.phase = phase;
        let event = self.make_event(phase, centroid, distance, angle, now, frame.count);
        self.last_centroid = Some(centroid);
        self.last_distance = Some(distance);
        self.last_angle = Some(angle);
        self.last_event_time = Some(now);
        event
    }

    fn reset_state(&mut self) {
        self.phase = GesturePhase::Possible;
        self.gesture_kind = None;
        self.origin_centroid = None;
        self.origin_distance = None;
        self.origin_angle = None;
        self.last_centroid = None;
        self.last_distance = None;
        self.last_angle = None;
        self.last_event_time = None;
    }
}
