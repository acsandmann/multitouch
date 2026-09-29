use crate::{Contact, Point};

#[inline]
fn is_active_finger(contact: &Contact) -> bool {
    !contact.is_palm() && contact.state().is_active()
}

/// Measurements of one frame's active, non-palm fingers.
///
/// Equivalent to running `ContactFilter::active_fingers` followed by
/// `centroid`, `max_inter_finger_distance` and `inter_finger_angle`, but done
/// in place: the recognizer runs once per sensor frame, so it must not
/// allocate or copy contacts.
pub(super) struct Frame {
    pub(super) count: usize,
    pub(super) centroid: Point,
    pub(super) distance: f32,
    pub(super) angle: f32,
}

impl Frame {
    pub(super) fn measure(contacts: &[Contact]) -> Self {
        let mut count = 0_usize;
        let (mut sum_x, mut sum_y) = (0.0_f32, 0.0_f32);
        for contact in contacts {
            if is_active_finger(contact) {
                count += 1;
                sum_x += contact.normalized.position.x;
                sum_y += contact.normalized.position.y;
            }
        }
        if count == 0 {
            return Self {
                count,
                centroid: Point::ZERO,
                distance: 0.0,
                angle: 0.0,
            };
        }
        let n = count as f32;
        let centroid = Point::new(sum_x / n, sum_y / n);

        // Track the farthest pair by squared distance (first pair wins ties,
        // strictly-greater from zero, as in Subsurface) and take one square
        // root at the end.
        let mut best_sq = 0.0_f32;
        let mut best: Option<(Point, Point)> = None;
        if count >= 2 {
            for (i, a) in contacts.iter().enumerate() {
                if !is_active_finger(a) {
                    continue;
                }
                let ap = a.normalized.position;
                for b in &contacts[i + 1..] {
                    if !is_active_finger(b) {
                        continue;
                    }
                    let bp = b.normalized.position;
                    let (dx, dy) = (bp.x - ap.x, bp.y - ap.y);
                    let sq = dx * dx + dy * dy;
                    if sq > best_sq {
                        best_sq = sq;
                        best = Some((ap, bp));
                    }
                }
            }
        }
        let angle = best.map_or(0.0, |(a, b)| (b.y - a.y).atan2(b.x - a.x));
        Self {
            count,
            centroid,
            distance: best_sq.sqrt(),
            angle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContactFilter;
    use crate::{ContactState, Finger, Hand, Vector};

    fn contact(x: f32, y: f32, state: ContactState, finger: Option<Finger>) -> Contact {
        Contact::new(
            0,
            0.0,
            0,
            state,
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

    /// The allocation-free per-frame path must agree with the public filter API.
    #[test]
    fn frame_measure_matches_contact_filter() {
        let frames: Vec<Vec<Contact>> = vec![
            vec![],
            vec![contact(
                0.5,
                0.5,
                ContactState::Touching,
                Some(Finger::Index),
            )],
            vec![
                contact(0.1, 0.2, ContactState::Touching, Some(Finger::Index)),
                contact(0.9, 0.7, ContactState::Making, Some(Finger::Middle)),
                contact(0.5, 0.5, ContactState::Hovering, Some(Finger::Ring)),
                contact(0.4, 0.4, ContactState::Touching, None),
                contact(0.3, 0.8, ContactState::Touching, Some(Finger::Pinky)),
            ],
            vec![
                contact(0.5, 0.5, ContactState::Touching, Some(Finger::Index)),
                contact(0.5, 0.5, ContactState::Touching, Some(Finger::Middle)),
            ],
        ];

        for frame in frames {
            let expected = ContactFilter::active_fingers(&frame);
            let m = Frame::measure(&frame);
            assert_eq!(m.count, expected.len());
            let c = ContactFilter::centroid(&expected);
            assert_eq!((m.centroid.x, m.centroid.y), (c.x, c.y));
            let d = ContactFilter::max_inter_finger_distance(&expected);
            assert!((m.distance - d).abs() < 1e-6, "{} vs {d}", m.distance);
            let a = ContactFilter::inter_finger_angle(&expected).unwrap_or(0.0);
            assert_eq!(m.angle, a);
        }
    }
}
