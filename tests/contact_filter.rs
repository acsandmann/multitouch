use multitouch::{Contact, ContactFilter, ContactState, Finger, Hand, Point, Vector};

fn contact(x: f32, y: f32, id: i32, finger: Option<Finger>) -> Contact {
    Contact::new(
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

#[test]
fn filters_palms() {
    let contacts = [
        contact(0.3, 0.3, 1, Some(Finger::Index)),
        contact(0.5, 0.5, 2, None),
        contact(0.7, 0.7, 3, Some(Finger::Middle)),
    ];
    assert_eq!(ContactFilter::remove_palms(&contacts).len(), 2);
}

#[test]
fn computes_centroid_and_distance() {
    let contacts = [
        contact(0.2, 0.4, 1, Some(Finger::Index)),
        contact(0.8, 0.6, 2, Some(Finger::Middle)),
    ];
    let c = ContactFilter::centroid(&contacts);
    assert!((c.x - 0.5).abs() < 0.001);
    assert!((c.y - 0.5).abs() < 0.001);

    let contacts = [
        contact(0.0, 0.0, 1, Some(Finger::Index)),
        contact(0.3, 0.4, 2, Some(Finger::Middle)),
    ];
    assert!((ContactFilter::inter_finger_distance(&contacts).unwrap() - 0.5).abs() < 0.001);
}

#[test]
fn farthest_pair_and_angle_work() {
    let contacts = [
        contact(0.0, 0.0, 1, Some(Finger::Index)),
        contact(0.1, 0.0, 2, Some(Finger::Middle)),
        contact(0.5, 0.0, 3, Some(Finger::Ring)),
    ];
    assert!((ContactFilter::max_inter_finger_distance(&contacts) - 0.5).abs() < 0.001);

    let vertical = [
        contact(0.5, 0.2, 1, Some(Finger::Index)),
        contact(0.5, 0.8, 2, Some(Finger::Middle)),
    ];
    assert!(
        (ContactFilter::inter_finger_angle(&vertical).unwrap() - std::f32::consts::FRAC_PI_2).abs()
            < 0.01
    );
}
