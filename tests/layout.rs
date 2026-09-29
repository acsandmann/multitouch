use multitouch::Contact;
use std::mem::{offset_of, size_of};

#[test]
fn contact_matches_multitouchsupport_layout() {
    assert_eq!(size_of::<Contact>(), 96);
    assert_eq!(offset_of!(Contact, frame), 0);
    assert_eq!(offset_of!(Contact, timestamp), 8);
    assert_eq!(offset_of!(Contact, id), 16);
    assert_eq!(offset_of!(Contact, normalized), 32);
    assert_eq!(offset_of!(Contact, total_capacitance), 48);
    assert_eq!(offset_of!(Contact, pressure), 52);
    assert_eq!(offset_of!(Contact, absolute), 68);
    assert_eq!(offset_of!(Contact, density), 92);
}
