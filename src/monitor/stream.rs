use std::{
  sync::{Arc, mpsc::RecvTimeoutError},
  time::Duration,
};

use super::hub::Frame;
use crate::{Contact, Device, queue::Queue};

pub struct MonitorStream {
  pub(super) queue: Arc<Queue<Frame>>,
}

impl MonitorStream {
  pub fn recv(&self) -> Option<(Device, Vec<Contact>)> { self.queue.recv() }

  pub fn try_recv(&self) -> Option<(Device, Vec<Contact>)> { self.queue.try_recv() }

  pub fn recv_timeout(&self, timeout: Duration) -> Option<(Device, Vec<Contact>)> { self.queue.recv_timeout(timeout) }

  pub(crate) fn recv_deadline(&self, timeout: Duration) -> Result<Frame, RecvTimeoutError> { self.queue.recv_deadline(timeout) }
}

impl Iterator for MonitorStream {
  type Item = (Device, Vec<Contact>);

  fn next(&mut self) -> Option<Self::Item> { self.queue.recv() }
}
