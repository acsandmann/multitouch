use super::event::{
    GestureEndReason, GestureEvent, GesturePhase, MagnifyEvent, RotationEvent, SwipeEvent,
};
use super::recognizer::{GestureKind, GestureRecognizer};
use crate::Point;
use std::f32::consts::PI;
use std::time::Instant;

impl GestureRecognizer {
    pub(super) fn make_event(
        &self,
        phase: GesturePhase,
        centroid: Point,
        distance: f32,
        angle: f32,
        now: Instant,
        finger_count: usize,
    ) -> Option<GestureEvent> {
        let origin_centroid = self.origin_centroid?;
        let origin_distance = self.origin_distance?;
        let origin_angle = self.origin_angle?;
        let dt = self
            .last_event_time
            .map(|t| now.saturating_duration_since(t).as_secs_f32())
            .unwrap_or(0.0);

        match self.gesture_kind? {
            GestureKind::Swipe => {
                let translation = Point::new(
                    centroid.x - origin_centroid.x,
                    centroid.y - origin_centroid.y,
                );
                let velocity = if dt > 0.0 {
                    self.last_centroid
                        .map(|last| {
                            Point::new((centroid.x - last.x) / dt, (centroid.y - last.y) / dt)
                        })
                        .unwrap_or(Point::ZERO)
                } else {
                    Point::ZERO
                };
                Some(GestureEvent::Swipe(SwipeEvent {
                    phase,
                    translation,
                    velocity,
                    centroid,
                    angle: translation.y.atan2(translation.x),
                    distance: translation.x.hypot(translation.y),
                    finger_count,
                }))
            }
            GestureKind::Magnify => {
                let velocity = if dt > 0.0 {
                    self.last_distance
                        .map(|last| (distance - last) / dt)
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                Some(GestureEvent::Magnify(MagnifyEvent {
                    phase,
                    distance,
                    origin_distance,
                    velocity,
                    centroid,
                    finger_count,
                }))
            }
            GestureKind::Rotation => {
                let rotation = angle_difference(origin_angle, angle);
                let velocity = if dt > 0.0 {
                    self.last_angle
                        .map(|last| (rotation - angle_difference(origin_angle, last)) / dt)
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                Some(GestureEvent::Rotation(RotationEvent {
                    phase,
                    rotation,
                    velocity,
                    centroid,
                    finger_count,
                }))
            }
        }
    }

    pub(super) fn make_end_event(
        &self,
        reason: GestureEndReason,
        active_finger_count: usize,
        now: Instant,
    ) -> Option<GestureEvent> {
        self.make_event(
            GesturePhase::Ended(reason),
            self.last_centroid?,
            self.last_distance?,
            self.last_angle?,
            now,
            active_finger_count,
        )
    }
}

pub(super) fn angle_difference(a: f32, b: f32) -> f32 {
    let mut diff = b - a;
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }
    diff
}
