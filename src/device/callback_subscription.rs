use std::sync::{Arc, Mutex};

use super::{ContactSink, Device, DeviceInner};
use crate::{Contact, GestureEvent, GestureRecognizer};

/// Borrowed frames are valid only during the callback. `Stopped` is delivered
/// once when the subscription closes, including device removal or monitor stop.
pub enum ContactEvent<'a> {
    Frame(&'a [Contact]),
    Stopped,
    /// Explicit reset or wake recovery; subsequent frames can begin a new gesture.
    Reset,
}

type Handler = Box<dyn for<'a> FnMut(ContactEvent<'a>) + Send>;

pub(crate) struct CallbackSink(Mutex<Option<Handler>>);
impl CallbackSink {
    pub(crate) fn frame(&self, contacts: &[Contact]) {
        if let Some(handler) = self.0.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            handler(ContactEvent::Frame(contacts));
        }
    }
}
impl ContactSink for CallbackSink {
    fn deliver(&self, _: &Arc<DeviceInner>, contacts: &[Contact]) {
        self.frame(contacts);
    }

    fn reset(&self) {
        if let Some(handler) = self.0.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            handler(ContactEvent::Reset);
        }
    }

    fn close(&self) {
        if let Some(mut handler) = self.0.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handler(ContactEvent::Stopped);
        }
    }
}

/// An inline subscription: no frame copies, queues, or pump thread. Dropping it
/// waits for an outstanding delivery and closes the handler. Callbacks for one
/// subscription are serialized; different devices may call concurrently.
///
/// Keep handlers short. Do not start/stop the device or monitor, or drop this
/// subscription, from its handler: those operations synchronize with delivery.
pub struct ContactSubscription {
    pub(crate) sink: Arc<CallbackSink>,
}
impl ContactSubscription {
    /// Cancels current recognition without closing the subscription.
    /// Like drop, this must be called outside its handler.
    pub fn reset(&self) {
        self.sink.reset();
    }

    pub(crate) fn new(handler: impl FnMut(ContactEvent<'_>) + Send + 'static) -> Self {
        Self {
            sink: Arc::new(CallbackSink(Mutex::new(Some(Box::new(handler))))),
        }
    }
}
impl Drop for ContactSubscription {
    fn drop(&mut self) {
        self.sink.close();
    }
}

pub(crate) fn gesture_handler(
    mut recognizer: GestureRecognizer,
    mut handler: impl FnMut(GestureEvent) + Send,
) -> impl FnMut(ContactEvent<'_>) + Send {
    move |event| {
        let event = match event {
            ContactEvent::Frame(contacts) => recognizer.process(contacts),
            ContactEvent::Stopped | ContactEvent::Reset => recognizer.reset(),
        };
        if let Some(event) = event {
            handler(event);
        }
    }
}

impl Device {
    /// Recognizes borrowed frames inline on the framework callback thread.
    /// Each subscription owns its recognizer. Full lift ends normally; stop or
    /// removal resets it. Unlike queue-backed gesture streams, this subscription has no autonomous
    /// inactivity timer: only lift, reset, and stop end recognition.
    pub fn subscribe_gestures(
        &self,
        recognizer: GestureRecognizer,
        handler: impl FnMut(GestureEvent) + Send + 'static,
    ) -> ContactSubscription {
        self.subscribe_contacts(gesture_handler(recognizer, handler))
    }

    /// Subscribes to borrowed frames and stop notifications synchronously.
    /// See `ContactSubscription` for callback lifetime and synchronization rules.
    pub fn subscribe_contacts(
        &self,
        handler: impl FnMut(ContactEvent<'_>) + Send + 'static,
    ) -> ContactSubscription {
        let subscription = ContactSubscription::new(handler);
        let sink: Arc<dyn ContactSink> = subscription.sink.clone();
        self.add_contact_sink(Arc::downgrade(&sink));
        subscription
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContactState, Finger, GestureEndReason, GesturePhase, GestureTypes, Hand, Point, Vector,
    };

    fn frame(x: f32) -> [Contact; 2] {
        std::array::from_fn(|i| {
            Contact::new(
                0,
                0.0,
                i as i32,
                ContactState::Touching,
                Some(Finger::Index),
                Some(Hand::Right),
                Vector::new(Point::new(x + i as f32 * 0.1, 0.5), Point::ZERO),
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                Vector::new(Point::ZERO, Point::ZERO),
                0.0,
            )
        })
    }

    #[test]
    fn inline_recognition_is_device_local_and_reset_and_close_end_once() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let subscriptions: [_; 2] = std::array::from_fn(|device| {
            let events = events.clone();
            ContactSubscription::new(gesture_handler(
                GestureRecognizer::new(2).with_gesture_types(GestureTypes::SWIPE),
                move |event| {
                    events
                        .lock()
                        .unwrap()
                        .push((device, event, std::thread::current().id()))
                },
            ))
        });
        subscriptions[0].sink.frame(&frame(0.1));
        subscriptions[1].sink.frame(&frame(0.7));
        subscriptions[0].sink.frame(&frame(0.3));
        subscriptions[1].sink.frame(&frame(0.5));
        subscriptions[0].reset();
        subscriptions[0].sink.frame(&frame(0.4));
        subscriptions[0].sink.frame(&frame(0.6));
        subscriptions[0].sink.frame(&[]);
        let retired = subscriptions[1].sink.clone();
        drop(subscriptions);
        retired.frame(&frame(0.3));
        retired.close();
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 9);
        assert!(events
            .iter()
            .all(|(_, _, thread)| *thread == std::thread::current().id()));
        let GestureEvent::Swipe(a) = events[2].1 else {
            panic!("first swipe")
        };
        let GestureEvent::Swipe(b) = events[3].1 else {
            panic!("second swipe")
        };
        assert!((a.translation.x - 0.2).abs() < 0.001);
        assert!((b.translation.x + 0.2).abs() < 0.001);
        assert_eq!(
            events[4].1.phase(),
            GesturePhase::Ended(GestureEndReason::Cancelled)
        );
        assert_eq!(events[5].1.phase(), GesturePhase::Determining);
        assert_eq!(
            events[7].1.phase(),
            GesturePhase::Ended(GestureEndReason::Lifted)
        );
        assert_eq!(
            events[8].1.phase(),
            GesturePhase::Ended(GestureEndReason::Cancelled)
        );
    }

    #[test]
    fn dropping_subscription_waits_for_delivery_and_prevents_late_calls() {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (closed_tx, closed_rx) = std::sync::mpsc::channel();
        let subscription = ContactSubscription::new(move |event| match event {
            ContactEvent::Frame(_) => {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }
            ContactEvent::Stopped => closed_tx.send(()).unwrap(),
            ContactEvent::Reset => unreachable!(),
        });
        let sink = subscription.sink.clone();
        let producer = std::thread::spawn(move || {
            sink.frame(&[]);
            sink
        });
        entered_rx.recv().unwrap();
        let closer = std::thread::spawn(move || drop(subscription));
        assert!(closed_rx
            .recv_timeout(std::time::Duration::from_millis(20))
            .is_err());
        release_tx.send(()).unwrap();
        let sink = producer.join().unwrap();
        closer.join().unwrap();
        closed_rx.recv().unwrap();
        sink.frame(&[]);
        sink.reset();
        sink.close();
        assert!(closed_rx.try_recv().is_err());
    }
    // Allocation accounting is local to each benchmark thread; unrelated tests
    // cannot inflate its counts. This allocator is absent from production builds.
    struct Allocator;
    thread_local! { static ALLOCATIONS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
    #[global_allocator]
    static ALLOCATOR: Allocator = Allocator;
    unsafe impl std::alloc::GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let _ = ALLOCATIONS.try_with(|n| {
                if let Some(count) = n.get() {
                    n.set(Some(count + 1));
                }
            });
            unsafe { std::alloc::System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
            unsafe { std::alloc::System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
            let _ = ALLOCATIONS.try_with(|n| {
                if let Some(count) = n.get() {
                    n.set(Some(count + 1));
                }
            });
            unsafe { std::alloc::System.realloc(ptr, layout, size) }
        }
    }

    /// Run with --release --ignored --nocapture. Queue capacity matches native
    /// streams: newest wins. Report actual deliveries alongside time so dropped
    /// frames are not mistaken for faster recognition. Times exclude setup.
    #[test]
    #[ignore = "manual callback/stream allocation and latency benchmark"]
    fn benchmark_delivery() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;
        const FRAMES: usize = 100_000;
        let mut contacts = frame(0.1).to_vec();
        contacts.push(contacts[0]);
        let recognizer = || GestureRecognizer::new(3).with_gesture_types(GestureTypes::SWIPE);
        let mut direct = recognizer();
        let events = Arc::new(AtomicUsize::new(0));
        let counter = events.clone();
        let inline = ContactSubscription::new(gesture_handler(recognizer(), move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
        }));
        // Warm lazy runtime/mutex initialization before counting active frames.
        direct.process(&contacts);
        inline.sink.frame(&contacts);
        events.store(0, Ordering::Relaxed);
        for mode in ["recognizer", "inline", "queue/thread"] {
            let mut latencies = Vec::with_capacity(FRAMES);
            let queue = crate::queue::Queue::<Vec<Contact>>::new(1);
            let (tx, rx) = std::sync::mpsc::channel();
            let worker = if mode == "queue/thread" {
                let queue = queue.clone();
                Some(std::thread::spawn(move || {
                    let mut recognizer = recognizer();
                    let mut delivered = 0;
                    ALLOCATIONS.with(|n| n.set(Some(0)));
                    while let Some(contacts) = queue.recv() {
                        delivered += 1;
                        if let Some(event) = recognizer.process(&contacts) {
                            tx.send(event).unwrap();
                        }
                    }
                    let allocations = ALLOCATIONS.with(|n| n.replace(None).unwrap());
                    (delivered, allocations)
                }))
            } else {
                None
            };
            let mut delivered = 0;
            let start = Instant::now();
            ALLOCATIONS.with(|n| n.set(Some(0)));
            for index in 0..FRAMES {
                for contact in &mut contacts {
                    contact.normalized.position.x = 0.1 + (index % 100) as f32 * 0.002;
                }
                let before = Instant::now();
                match mode {
                    "recognizer" => {
                        delivered += usize::from(direct.process(&contacts).is_some());
                    }
                    "inline" => inline.sink.frame(&contacts),
                    _ => queue.push(contacts.clone()),
                }
                latencies.push(before.elapsed().as_nanos());
            }
            let allocations = ALLOCATIONS.with(|n| n.replace(None).unwrap());
            queue.close();
            let (frames, worker_allocations) =
                worker.map_or((FRAMES, 0), |worker| worker.join().unwrap());
            let elapsed = start.elapsed();
            if mode == "queue/thread" {
                delivered = rx.try_iter().count();
            }
            if mode == "inline" {
                delivered = events.load(Ordering::Relaxed);
            }
            latencies.sort_unstable();
            println!("{mode}: offered={FRAMES} processed={frames} events={delivered} elapsed={elapsed:?} allocations={} producer_p50={}ns producer_p99={}ns", allocations + worker_allocations, latencies[FRAMES/2], latencies[FRAMES*99/100]);
            assert!(delivered > 0);
            if mode != "queue/thread" {
                assert_eq!(allocations, 0);
            }
        }
    }
}
