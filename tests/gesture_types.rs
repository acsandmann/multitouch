use multitouch::GestureTypes;

#[test]
fn behaves_like_an_option_set() {
    let all = GestureTypes::ALL;
    assert!(all.contains(GestureTypes::SWIPE | GestureTypes::ROTATION));
    assert_eq!(all.bits(), 0b111);
    assert_eq!(
        GestureTypes::from_bits(0b101),
        GestureTypes::SWIPE | GestureTypes::ROTATION
    );

    assert!(GestureTypes::default().is_empty());
    assert_eq!(GestureTypes::default(), GestureTypes::NONE);

    let swipe_magnify = GestureTypes::SWIPE | GestureTypes::MAGNIFY;
    assert_eq!(all - GestureTypes::ROTATION, swipe_magnify);
    assert_eq!(all.difference(swipe_magnify), GestureTypes::ROTATION);
    assert_eq!(swipe_magnify & GestureTypes::MAGNIFY, GestureTypes::MAGNIFY);
    assert_eq!(swipe_magnify ^ GestureTypes::ALL, GestureTypes::ROTATION);
    assert!(swipe_magnify.intersects(GestureTypes::MAGNIFY));
    assert!(!swipe_magnify.intersects(GestureTypes::ROTATION));
    assert!(!swipe_magnify.contains(GestureTypes::ALL));

    let mut set = GestureTypes::NONE;
    set.insert(GestureTypes::SWIPE);
    set |= GestureTypes::ROTATION;
    assert_eq!(set.bits(), 0b101);
    set.remove(GestureTypes::SWIPE);
    set -= GestureTypes::MAGNIFY;
    assert_eq!(set, GestureTypes::ROTATION);
    set &= GestureTypes::SWIPE;
    assert!(set.is_empty());
}
