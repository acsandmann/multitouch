use super::event::GestureEvent;
use super::recognizer::GestureRecognizer;
use crate::{Contact, Device, Monitor};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

impl GestureRecognizer {
    /// Spawn a gesture stream from one multitouch device.
    pub fn events_from_device(self, device: &Device) -> GestureStream {
        let stream = device.contact_frames();
        self.spawn_events(move |timeout| stream.recv_deadline(timeout))
    }

    /// Spawn a gesture stream from a monitor. Frames from every monitored
    /// multitouch device are fed into this recognizer.
    pub fn events_from_monitor(self, monitor: &Monitor) -> GestureStream {
        let stream = monitor.contacts();
        self.spawn_events(move |timeout| {
            stream.recv_deadline(timeout).map(|(_, contacts)| contacts)
        })
    }

    pub fn events_from_receiver(self, receiver: Receiver<Vec<Contact>>) -> GestureStream {
        self.spawn_events(move |timeout| receiver.recv_timeout(timeout))
    }

    /// Runs the recognizer on its own thread, pulling frames from `next`.
    /// `next` is given the current inactivity timeout and must report a
    /// timeout or a closed source distinctly.
    fn spawn_events<F>(self, mut next: F) -> GestureStream
    where
        F: FnMut(Duration) -> Result<Vec<Contact>, RecvTimeoutError> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut recognizer = self;
            loop {
                match next(recognizer.inactivity_timeout) {
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
        GestureStream::new(rx)
    }
}

pub struct GestureStream {
    receiver: Receiver<GestureEvent>,
}

impl GestureStream {
    pub(super) fn new(receiver: Receiver<GestureEvent>) -> Self {
        Self { receiver }
    }

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
