use crate::queue::Queue;
use crate::{Contact, PathEvent};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

pub struct ContactStream {
    pub(super) queue: Arc<Queue<Vec<Contact>>>,
}

impl ContactStream {
    pub fn recv(&self) -> Option<Vec<Contact>> {
        self.queue.recv()
    }
    pub fn try_recv(&self) -> Option<Vec<Contact>> {
        self.queue.try_recv()
    }
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Vec<Contact>> {
        self.queue.recv_timeout(timeout)
    }

    pub(crate) fn recv_deadline(
        &self,
        timeout: Duration,
    ) -> Result<Vec<Contact>, RecvTimeoutError> {
        self.queue.recv_deadline(timeout)
    }

    pub fn into_receiver(self) -> mpsc::Receiver<Vec<Contact>> {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut stream = self;
            for frame in &mut stream {
                if tx.send(frame).is_err() {
                    break;
                }
            }
        });
        rx
    }
}

impl Iterator for ContactStream {
    type Item = Vec<Contact>;
    fn next(&mut self) -> Option<Self::Item> {
        self.queue.recv()
    }
}

pub struct PathStream {
    pub(super) queue: Arc<Queue<PathEvent>>,
}

impl PathStream {
    pub fn recv(&self) -> Option<PathEvent> {
        self.queue.recv()
    }
    pub fn try_recv(&self) -> Option<PathEvent> {
        self.queue.try_recv()
    }
}

impl Iterator for PathStream {
    type Item = PathEvent;
    fn next(&mut self) -> Option<Self::Item> {
        self.queue.recv()
    }
}
