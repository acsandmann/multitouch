use crate::{Contact, Device, Monitor, Point};
use std::f32::consts::PI;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureTypes(u8);

impl GestureTypes {
    pub const NONE: Self = Self(0);
    pub const SWIPE: Self = Self(1 << 0);
    pub const MAGNIFY: Self = Self(1 << 1);
    pub const ROTATION: Self = Self(1 << 2);
    pub const ALL: Self = Self(Self::SWIPE.0 | Self::MAGNIFY.0 | Self::ROTATION.0);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOr for GestureTypes {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

pub struct ContactFilter;

impl ContactFilter {
    pub fn remove_palms(contacts: &[Contact]) -> Vec<Contact> {
        contacts.iter().copied().filter(|c| !c.is_palm()).collect()
    }

    pub fn active_touches(contacts: &[Contact]) -> Vec<Contact> {
        contacts
            .iter()
            .copied()
            .filter(|c| c.state().is_active())
            .collect()
    }

    pub fn active_fingers(contacts: &[Contact]) -> Vec<Contact> {
        contacts
            .iter()
            .copied()
            .filter(|c| !c.is_palm() && c.state().is_active())
            .collect()
    }

    pub fn centroid(contacts: &[Contact]) -> Point {
        if contacts.is_empty() {
            return Point::ZERO;
        }
        let (x, y) = contacts.iter().fold((0.0_f32, 0.0_f32), |(x, y), contact| {
            (
                x + contact.normalized.position.x,
                y + contact.normalized.position.y,
            )
        });
        let n = contacts.len() as f32;
        Point::new(x / n, y / n)
    }

    pub fn inter_finger_distance(contacts: &[Contact]) -> Option<f32> {
        if contacts.len() != 2 {
            return None;
        }
        let a = contacts[0].normalized.position;
        let b = contacts[1].normalized.position;
        Some((b.x - a.x).hypot(b.y - a.y))
    }

    pub fn max_inter_finger_distance(contacts: &[Contact]) -> f32 {
        let mut max_distance = 0.0_f32;
        for (i, a) in contacts.iter().enumerate() {
            for b in &contacts[i + 1..] {
                let a = a.normalized.position;
                let b = b.normalized.position;
                max_distance = max_distance.max((b.x - a.x).hypot(b.y - a.y));
            }
        }
        max_distance
    }

    pub fn farthest_pair(contacts: &[Contact]) -> Option<(Contact, Contact)> {
        if contacts.len() < 2 {
            return None;
        }
        let mut best = None;
        let mut max_distance = -1.0_f32;
        for (i, a) in contacts.iter().copied().enumerate() {
            for b in contacts[i + 1..].iter().copied() {
                let ap = a.normalized.position;
                let bp = b.normalized.position;
                let distance = (bp.x - ap.x).hypot(bp.y - ap.y);
                if distance > max_distance {
                    max_distance = distance;
                    best = Some((a, b));
                }
            }
        }
        best
    }

    pub fn inter_finger_angle(contacts: &[Contact]) -> Option<f32> {
        let (a, b) = Self::farthest_pair(contacts)?;
        let a = a.normalized.position;
        let b = b.normalized.position;
        Some((b.y - a.y).atan2(b.x - a.x))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GestureKind {
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

    phase: GesturePhase,
    gesture_kind: Option<GestureKind>,
    origin_centroid: Option<Point>,
    origin_distance: Option<f32>,
    origin_angle: Option<f32>,
    last_centroid: Option<Point>,
    last_distance: Option<f32>,
    last_angle: Option<f32>,
    last_event_time: Option<Instant>,
    previous_active_finger_count: usize,
    finger_count_decreased_in_current_touch_sequence: bool,
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
        let filtered = ContactFilter::active_fingers(contacts);
        let count = filtered.len();

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
            return self.emit_tracked(&filtered, GesturePhase::Changed, count, now);
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

        let centroid = ContactFilter::centroid(&filtered);
        let distance = ContactFilter::max_inter_finger_distance(&filtered);
        let angle = ContactFilter::inter_finger_angle(&filtered).unwrap_or(0.0);

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

        let distance_delta = distance - origin_distance;
        let angle_delta = angle_difference(origin_angle, angle);
        let translation = (centroid.x - origin_centroid.x).hypot(centroid.y - origin_centroid.y);

        self.gesture_kind = if self
            .recognized_gesture_types
            .contains(GestureTypes::MAGNIFY)
            && distance_delta.abs() > self.minimum_magnification_distance
        {
            Some(GestureKind::Magnify)
        } else if self
            .recognized_gesture_types
            .contains(GestureTypes::ROTATION)
            && angle_delta.abs() > self.minimum_rotation
        {
            Some(GestureKind::Rotation)
        } else if self.recognized_gesture_types.contains(GestureTypes::SWIPE)
            && translation > self.minimum_swipe_translation
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

        self.phase = GesturePhase::Began;
        let event = self.make_event(GesturePhase::Began, centroid, distance, angle, now, count);
        self.last_centroid = Some(centroid);
        self.last_distance = Some(distance);
        self.last_angle = Some(angle);
        self.last_event_time = Some(now);
        event
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

    /// Spawn a gesture stream from one multitouch device.
    pub fn events_from_device(self, device: &Device) -> GestureStream {
        self.events_from_receiver(device.contact_frames().into_receiver())
    }

    /// Spawn a gesture stream from a monitor. Frames from every monitored
    /// multitouch device are fed into this recognizer.
    pub fn events_from_monitor(self, monitor: &Monitor) -> GestureStream {
        let mut stream = monitor.contacts();
        let (tx, rx) = mpsc::sync_channel(1);
        thread::spawn(move || {
            for (_, contacts) in &mut stream {
                if tx.send(contacts).is_err() {
                    break;
                }
            }
        });
        self.events_from_receiver(rx)
    }

    pub fn events_from_receiver(self, receiver: Receiver<Vec<Contact>>) -> GestureStream {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut recognizer = self;
            loop {
                match receiver.recv_timeout(recognizer.inactivity_timeout) {
                    Ok(frame) => {
                        if let Some(event) = recognizer.process(&frame) {
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if let Some(event) = recognizer.timeout() {
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        if let Some(event) = recognizer.reset() {
                            let _ = tx.send(event);
                        }
                        break;
                    }
                }
            }
        });
        GestureStream { receiver: rx }
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
        filtered: &[Contact],
        phase: GesturePhase,
        count: usize,
        now: Instant,
    ) -> Option<GestureEvent> {
        let centroid = ContactFilter::centroid(filtered);
        let distance = ContactFilter::max_inter_finger_distance(filtered);
        let angle = ContactFilter::inter_finger_angle(filtered).unwrap_or(0.0);
        self.phase = phase;
        let event = self.make_event(phase, centroid, distance, angle, now, count);
        self.last_centroid = Some(centroid);
        self.last_distance = Some(distance);
        self.last_angle = Some(angle);
        self.last_event_time = Some(now);
        event
    }

    fn make_event(
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

    fn make_end_event(
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

fn angle_difference(a: f32, b: f32) -> f32 {
    let mut diff = b - a;
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }
    diff
}

pub struct GestureStream {
    receiver: Receiver<GestureEvent>,
}

impl GestureStream {
    pub fn recv(&self) -> Result<GestureEvent, mpsc::RecvError> {
        self.receiver.recv()
    }

    pub fn try_recv(&self) -> Result<GestureEvent, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
}

impl Iterator for GestureStream {
    type Item = GestureEvent;
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}
