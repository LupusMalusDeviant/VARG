// Wave 33: Typed Channels for inter-agent communication
// channel_new(cap) -> Channel
// channel_send(ch, value) -> bool
// channel_recv(ch) -> string          (blocks up to 30s)
// channel_recv_timeout(ch, ms) -> string
// channel_try_recv(ch) -> string      (non-blocking, "" if empty)
// channel_len(ch) -> int
// channel_close(ch)
// channel_is_closed(ch) -> bool

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct VargChannel {
    queue: VecDeque<String>,
    closed: bool,
    capacity: usize,
}

impl VargChannel {
    fn new(capacity: usize) -> Self {
        VargChannel { queue: VecDeque::new(), closed: false, capacity: capacity.max(1) }
    }

    fn send(&mut self, value: String) -> bool {
        if self.closed || self.queue.len() >= self.capacity {
            return false;
        }
        self.queue.push_back(value);
        true
    }

    fn try_recv(&mut self) -> Option<String> {
        self.queue.pop_front()
    }
}

pub type ChannelHandle = Arc<Mutex<VargChannel>>;

pub fn __varg_channel_new(capacity: i64) -> ChannelHandle {
    Arc::new(Mutex::new(VargChannel::new(capacity.max(1) as usize)))
}

pub fn __varg_channel_send(h: &ChannelHandle, value: &str) -> bool {
    h.lock().unwrap().send(value.to_string())
}

pub fn __varg_channel_try_recv(h: &ChannelHandle) -> String {
    h.lock().unwrap().try_recv().unwrap_or_default()
}

pub fn __varg_channel_recv_timeout(h: &ChannelHandle, timeout_ms: i64) -> String {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(0) as u64);
    loop {
        {
            let mut ch = h.lock().unwrap();
            if let Some(v) = ch.try_recv() { return v; }
            if ch.closed { return String::new(); }
        }
        if Instant::now() >= deadline { return String::new(); }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn __varg_channel_recv(h: &ChannelHandle) -> String {
    __varg_channel_recv_timeout(h, 30_000)
}

pub fn __varg_channel_len(h: &ChannelHandle) -> i64 {
    h.lock().unwrap().queue.len() as i64
}

pub fn __varg_channel_close(h: &ChannelHandle) {
    h.lock().unwrap().closed = true;
}

pub fn __varg_channel_is_closed(h: &ChannelHandle) -> bool {
    h.lock().unwrap().closed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_send_try_recv() {
        let ch = __varg_channel_new(10);
        assert!(__varg_channel_send(&ch, "hello"));
        assert_eq!(__varg_channel_try_recv(&ch), "hello");
    }

    #[test]
    fn test_channel_fifo_order() {
        let ch = __varg_channel_new(10);
        for msg in ["first", "second", "third"] {
            __varg_channel_send(&ch, msg);
        }
        assert_eq!(__varg_channel_try_recv(&ch), "first");
        assert_eq!(__varg_channel_try_recv(&ch), "second");
        assert_eq!(__varg_channel_try_recv(&ch), "third");
    }

    #[test]
    fn test_channel_capacity_enforced() {
        let ch = __varg_channel_new(2);
        assert!(__varg_channel_send(&ch, "1"));
        assert!(__varg_channel_send(&ch, "2"));
        assert!(!__varg_channel_send(&ch, "3")); // full
    }

    #[test]
    fn test_channel_len() {
        let ch = __varg_channel_new(10);
        assert_eq!(__varg_channel_len(&ch), 0);
        __varg_channel_send(&ch, "x");
        assert_eq!(__varg_channel_len(&ch), 1);
        __varg_channel_try_recv(&ch);
        assert_eq!(__varg_channel_len(&ch), 0);
    }

    #[test]
    fn test_channel_close_prevents_send() {
        let ch = __varg_channel_new(10);
        assert!(!__varg_channel_is_closed(&ch));
        __varg_channel_close(&ch);
        assert!(__varg_channel_is_closed(&ch));
        assert!(!__varg_channel_send(&ch, "x"));
    }

    #[test]
    fn test_channel_recv_timeout_returns_empty_on_timeout() {
        let ch = __varg_channel_new(10);
        let start = Instant::now();
        let result = __varg_channel_recv_timeout(&ch, 50);
        assert!(start.elapsed() >= Duration::from_millis(49));
        assert!(result.is_empty());
    }

    #[test]
    fn test_channel_try_recv_empty_returns_empty_string() {
        let ch = __varg_channel_new(10);
        assert_eq!(__varg_channel_try_recv(&ch), "");
    }
}
