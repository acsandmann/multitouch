use std::collections::VecDeque;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Bounded, newest-wins queue used to hand frames from framework callback
/// threads to consumers without ever blocking the producer on a slow reader.
pub struct Queue<T> {
    capacity: usize,
    state: Mutex<State<T>>,
    wake: Condvar,
}

struct State<T> {
    values: VecDeque<T>,
    closed: bool,
    /// Number of consumers currently parked on the condvar. Lets producers skip
    /// the `notify` call entirely when nobody is waiting.
    waiters: usize,
}

impl<T> Queue<T> {
    pub fn new(capacity: usize) -> Arc<Self> {
        let capacity = capacity.max(1);
        Arc::new(Self {
            capacity,
            state: Mutex::new(State {
                values: VecDeque::with_capacity(capacity),
                closed: false,
                waiters: 0,
            }),
            wake: Condvar::new(),
        })
    }

    #[inline]
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn push(&self, value: T) {
        let mut state = self.lock();
        if state.closed {
            return;
        }
        // At most one value is ever evicted per push. It is dropped after the
        // lock is released so that deallocation never extends the critical
        // section held by the framework callback thread.
        let evicted = if state.values.len() >= self.capacity {
            state.values.pop_front()
        } else {
            None
        };
        state.values.push_back(value);
        let notify = state.waiters > 0;
        drop(state);
        if notify {
            self.wake.notify_one();
        }
        drop(evicted);
    }

    pub fn recv(&self) -> Option<T> {
        let mut state = self.lock();
        loop {
            if let Some(value) = state.values.pop_front() {
                return Some(value);
            }
            if state.closed {
                return None;
            }
            state.waiters += 1;
            state = self.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            state.waiters -= 1;
        }
    }

    pub fn try_recv(&self) -> Option<T> {
        self.lock().values.pop_front()
    }

    /// Waits up to `timeout` for a value, distinguishing a timeout from a
    /// closed queue.
    pub fn recv_deadline(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        let deadline = Instant::now().checked_add(timeout);
        let mut state = self.lock();
        loop {
            if let Some(value) = state.values.pop_front() {
                return Ok(value);
            }
            if state.closed {
                return Err(RecvTimeoutError::Disconnected);
            }
            state.waiters += 1;
            state = match deadline {
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        state.waiters -= 1;
                        return Err(RecvTimeoutError::Timeout);
                    }
                    self.wake
                        .wait_timeout(state, deadline - now)
                        .unwrap_or_else(|e| e.into_inner())
                        .0
                }
                // Timeout too large to represent: wait indefinitely.
                None => self.wake.wait(state).unwrap_or_else(|e| e.into_inner()),
            };
            state.waiters -= 1;
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<T> {
        self.recv_deadline(timeout).ok()
    }

    pub fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        let has_waiters = state.waiters > 0;
        drop(state);
        if has_waiters {
            self.wake.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn newest_value_wins() {
        let q = Queue::new(1);
        q.push(1);
        q.push(2);
        q.push(3);
        assert_eq!(q.try_recv(), Some(3));
        assert_eq!(q.try_recv(), None);
    }

    #[test]
    fn timeout_and_close_are_distinguished() {
        let q: Arc<Queue<u8>> = Queue::new(1);
        assert_eq!(
            q.recv_deadline(Duration::from_millis(5)),
            Err(RecvTimeoutError::Timeout)
        );
        q.push(7);
        q.close();
        // Buffered values drain before the closed state is reported.
        assert_eq!(q.recv_deadline(Duration::from_millis(5)), Ok(7));
        assert_eq!(
            q.recv_deadline(Duration::from_millis(5)),
            Err(RecvTimeoutError::Disconnected)
        );
        q.push(9);
        assert_eq!(q.try_recv(), None, "push after close is ignored");
    }

    #[test]
    fn blocked_receiver_is_woken_by_push_and_close() {
        let q: Arc<Queue<u8>> = Queue::new(1);
        let rx = Arc::clone(&q);
        let handle = thread::spawn(move || (rx.recv(), rx.recv()));
        thread::sleep(Duration::from_millis(30));
        q.push(1);
        thread::sleep(Duration::from_millis(30));
        q.close();
        assert_eq!(handle.join().unwrap(), (Some(1), None));
    }

    #[test]
    fn huge_timeout_does_not_panic() {
        let q: Arc<Queue<u8>> = Queue::new(1);
        q.push(1);
        assert_eq!(q.recv_deadline(Duration::MAX), Ok(1));
    }
}
