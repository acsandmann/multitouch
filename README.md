# multitouch

raw macOS multitouch, gesture recognition, device monitoring, and force touch haptics in rust

## documentation

documentation is available on [docs.rs](https://docs.rs/multitouch).

## synchronous gestures

For latency-sensitive clients, recognize borrowed contact frames on the native
callback thread without frame copies, queues, or a pump thread:

```rust
use multitouch::{GestureRecognizer, GestureTypes, Monitor};

let monitor = Monitor::with_gesture_handler(
    |_| GestureRecognizer::new(3).with_gesture_types(GestureTypes::SWIPE),
    |device, event| {
        // Each device owns an independent recognizer. Keep this handler short.
        println!("{device:?}: {event:?}");
    },
);
assert!(monitor.start());
// Keep `monitor` alive while your application runs. Drop/stop synchronizes with
// outstanding callbacks and cancels any active recognition.
```

`Device::subscribe_gestures` provides the same path for a specific device; keep
its `ContactSubscription` alive. `subscribe_contacts` and
`Monitor::with_device_handler` expose borrowed frames plus stop/reset events for
clients that need their own admission or release policy. Reset cancels recognition
without retiring the subscription. Wake recovery resets these handlers before
restarting native delivery.

Callbacks for one subscription are serialized; different devices may deliver
concurrently. Do not start/stop a device or monitor, reset a subscription, or drop
it from its own callback. Inline subscriptions end on lift, reset, or stop;
they do not schedule inactivity timers. Existing queue-backed gesture streams
retain their inactivity timeout behavior.

The delivery benchmark can be run with:

```sh
cargo test --release benchmark_delivery -- --ignored --nocapture
```

## license

licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
