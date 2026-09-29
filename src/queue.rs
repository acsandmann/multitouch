use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub struct Queue<T> {
    capacity: usize,
    state: Mutex<State<T>>,
    wake: Condvar,
}

struct State<T> {
    values: VecDeque<T>,
    closed: bool,
}

impl<T> Queue<T> {
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            capacity: capacity.max(1),
            state: Mutex::new(State { values: VecDeque::new(), closed: false }),
            wake: Condvar::new(),
        })
    }

    pub fn push(&self, value: T) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return;
        }
        while state.values.len() >= self.capacity {
            state.values.pop_front();
        }
        state.values.push_back(value);
        self.wake.notify_one();
    }

    pub fn recv(&self) -> Option<T> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(value) = state.values.pop_front() {
                return Some(value);
            }
            if state.closed {
                return None;
            }
            state = self.wake.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn try_recv(&self) -> Option<T> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).values.pop_front()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<T> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(value) = state.values.pop_front() {
                return Some(value);
            }
            if state.closed {
                return None;
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            let wait = deadline.saturating_duration_since(now);
            let (next, result) = self.wake.wait_timeout(state, wait).unwrap_or_else(|e| e.into_inner());
            state = next;
            if result.timed_out() && state.values.is_empty() {
                return None;
            }
        }
    }

    pub fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        self.wake.notify_all();
    }
}
