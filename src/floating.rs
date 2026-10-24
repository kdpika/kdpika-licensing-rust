use std::sync::{Condvar, Mutex};
pub(crate) struct Heartbeat {
    pub state: Mutex<(u64, bool)>,
    pub wake: Condvar,
}
impl Heartbeat {
    pub fn new() -> Self {
        Self {
            state: Mutex::new((0, false)),
            wake: Condvar::new(),
        }
    }
    pub fn stop(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.0 += 1;
            s.1 = false;
            self.wake.notify_all();
        }
    }
}
