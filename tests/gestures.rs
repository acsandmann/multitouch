use multitouch::{
    Contact, Finger, FingerCountChange, GestureEndReason, GestureEvent, GesturePhase,
    GestureRecognizer, GestureTypes,
};

fn two(a: (f32, f32), b: (f32, f32)) -> [Contact; 2] {
    [
        Contact::test_contact(a.0, a.1, 1, Some(Finger::Index)),
        Contact::test_contact(b.0, b.1, 2, Some(Finger::Middle)),
    ]
}

#[test]
fn swipe_begins_and_changes() {
    let mut r = GestureRecognizer::new(2);
    assert_eq!(r.process(&two((0.3, 0.5), (0.4, 0.5))).unwrap().phase(), GesturePhase::Determining);

    let event = r.process(&two((0.4, 0.5), (0.5, 0.5))).unwrap();
    let GestureEvent::Swipe(event) = event else { panic!("expected swipe") };
    assert_eq!(event.phase, GesturePhase::Began);
    assert!(event.translation.x > 0.0);

    let event = r.process(&two((0.5, 0.5), (0.6, 0.5))).unwrap();
    let GestureEvent::Swipe(event) = event else { panic!("expected changed swipe") };
    assert_eq!(event.phase, GesturePhase::Changed);
}

#[test]
fn magnify_has_priority_over_swipe() {
    let mut r = GestureRecognizer::new(2);
    let _ = r.process(&two((0.45, 0.5), (0.55, 0.5)));
    let event = r.process(&two((0.35, 0.5), (0.75, 0.5))).unwrap();
    assert!(matches!(event, GestureEvent::Magnify(_)));
}

#[test]
fn rotation_is_detected_and_can_be_disabled() {
    let mut r = GestureRecognizer::new(2);
    let _ = r.process(&two((0.3, 0.5), (0.7, 0.5)));
    let event = r.process(&two((0.3268, 0.4), (0.6732, 0.6))).unwrap();
    assert!(matches!(event, GestureEvent::Rotation(_)));

    let mut r = GestureRecognizer::new(2).with_gesture_types(GestureTypes::SWIPE | GestureTypes::MAGNIFY);
    let _ = r.process(&two((0.3, 0.5), (0.7, 0.5)));
    let event = r.process(&two((0.3268, 0.4), (0.6732, 0.6))).unwrap();
    assert_eq!(event.phase(), GesturePhase::Determining);
}

#[test]
fn unresolved_lift_and_timeout_are_reported() {
    let mut r = GestureRecognizer::new(2);
    let _ = r.process(&two((0.5, 0.5), (0.6, 0.5)));
    assert!(matches!(r.process(&[]), Some(GestureEvent::UnresolvedEnded(GestureEndReason::Lifted))));

    let mut r = GestureRecognizer::new(2);
    let _ = r.process(&two((0.5, 0.5), (0.6, 0.5)));
    assert_eq!(r.timeout(), Some(GestureEvent::UnresolvedEnded(GestureEndReason::TimedOut)));
}

#[test]
fn resolved_gesture_is_sticky_until_below_two_fingers() {
    let mut r = GestureRecognizer::new(2);
    let _ = r.process(&two((0.3, 0.5), (0.4, 0.5)));
    let _ = r.process(&two((0.4, 0.5), (0.5, 0.5)));

    let three = [
        Contact::test_contact(0.4, 0.5, 1, Some(Finger::Index)),
        Contact::test_contact(0.5, 0.5, 2, Some(Finger::Middle)),
        Contact::test_contact(0.6, 0.5, 3, Some(Finger::Ring)),
    ];
    let event = r.process(&three).unwrap();
    let GestureEvent::Swipe(event) = event else { panic!("expected swipe") };
    assert_eq!(event.phase, GesturePhase::Changed);
    assert_eq!(event.finger_count, 3);

    let one = [Contact::test_contact(0.5, 0.5, 1, Some(Finger::Index))];
    let GestureEvent::Swipe(event) = r.process(&one).unwrap() else { panic!("expected ended swipe") };
    assert_eq!(event.phase, GesturePhase::Ended(GestureEndReason::Lifted));
}

#[test]
fn exact_count_mode_reports_count_change() {
    let mut r = GestureRecognizer::new(3).with_exact_finger_count(true);
    let origin = [
        Contact::test_contact(0.3, 0.5, 1, Some(Finger::Index)),
        Contact::test_contact(0.4, 0.5, 2, Some(Finger::Middle)),
        Contact::test_contact(0.5, 0.5, 3, Some(Finger::Ring)),
    ];
    let moved = [
        Contact::test_contact(0.4, 0.5, 1, Some(Finger::Index)),
        Contact::test_contact(0.5, 0.5, 2, Some(Finger::Middle)),
        Contact::test_contact(0.6, 0.5, 3, Some(Finger::Ring)),
    ];
    let _ = r.process(&origin);
    assert_eq!(r.process(&moved).unwrap().phase(), GesturePhase::Began);

    let event = r.process(&two((0.4, 0.5), (0.5, 0.5))).unwrap();
    assert_eq!(
        event.phase(),
        GesturePhase::Ended(GestureEndReason::FingerCountChanged(FingerCountChange::Decreased))
    );
}

#[test]
fn activation_after_finger_count_decrease_is_blocked() {
    let mut r = GestureRecognizer::new(2);
    let three = [
        Contact::test_contact(0.3, 0.5, 1, Some(Finger::Index)),
        Contact::test_contact(0.4, 0.5, 2, Some(Finger::Middle)),
        Contact::test_contact(0.5, 0.5, 3, Some(Finger::Ring)),
    ];
    let _ = r.process(&three);
    assert!(r.process(&two((0.4, 0.5), (0.5, 0.5))).is_none());
    assert!(r.process(&two((0.5, 0.5), (0.6, 0.5))).is_none());
}
