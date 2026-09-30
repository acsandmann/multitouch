//! Raw macOS multitouch, gesture recognition, device monitoring, and haptics.

mod types;
pub use types::*;

pub mod gesture;
pub use gesture::{
    ContactFilter, FingerCountChange, GestureEndReason, GestureEvent, GesturePhase,
    GestureRecognizer, GestureStream, GestureTypes, MagnifyEvent, RotationEvent, SwipeEvent,
};

mod cf;
mod device;
mod ffi;
mod haptics;
mod monitor;
mod power;
mod queue;

pub use device::{ContactEvent, ContactStream, ContactSubscription, Device, PathStream};
pub use haptics::{Actuator, HapticPattern};
pub use monitor::{Monitor, MonitorEvent, MonitorStream};
