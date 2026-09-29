use crate::{Contact, Point};

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
        // Strictly-greater comparison from 0 mirrors Subsurface: fully
        // coincident contacts have no farthest pair.
        let mut best = None;
        let mut max_distance = 0.0_f32;
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
