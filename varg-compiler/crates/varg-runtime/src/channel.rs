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

    // ── Adversarial / edge-case tests ────────────────────────────────────────

    #[test]
    fn test_channel_zero_capacity_coerced_to_one() {
        // capacity 0 must become 1 (not a zero-size dead channel)
        let ch = __varg_channel_new(0);
        assert!(__varg_channel_send(&ch, "x"), "capacity-0 channel must accept at least 1 message");
        assert_eq!(__varg_channel_try_recv(&ch), "x");
    }

    #[test]
    fn test_channel_negative_capacity_coerced_to_one() {
        let ch = __varg_channel_new(-100);
        assert!(__varg_channel_send(&ch, "x"));
    }

    #[test]
    fn test_channel_send_full_then_recv_frees_slot() {
        // Fill to capacity, verify send fails, receive frees a slot, then send succeeds
        let ch = __varg_channel_new(2);
        assert!(__varg_channel_send(&ch, "a"));
        assert!(__varg_channel_send(&ch, "b"));
        assert!(!__varg_channel_send(&ch, "c"), "must reject when full");
        __varg_channel_try_recv(&ch); // drain one
        assert!(__varg_channel_send(&ch, "c"), "slot freed by recv must allow new send");
    }

    #[test]
    fn test_channel_recv_timeout_closed_empty_returns_immediately() {
        // Closed + empty channel must not block — should return "" instantly
        let ch = __varg_channel_new(5);
        __varg_channel_close(&ch);
        let start = Instant::now();
        let result = __varg_channel_recv_timeout(&ch, 5_000);
        let elapsed = start.elapsed();
        assert!(result.is_empty());
        assert!(elapsed < Duration::from_millis(100), "closed empty channel must not block, elapsed={elapsed:?}");
    }

    #[test]
    fn test_channel_send_to_closed_fails() {
        let ch = __varg_channel_new(10);
        __varg_channel_close(&ch);
        assert!(!__varg_channel_send(&ch, "msg"), "send to closed channel must return false");
    }

    #[test]
    fn test_channel_recv_timeout_gets_pre_queued_message_even_when_closed() {
        // If messages are already queued before close, recv must drain them
        let ch = __varg_channel_new(5);
        __varg_channel_send(&ch, "pre");
        __varg_channel_close(&ch);
        assert_eq!(__varg_channel_recv_timeout(&ch, 100), "pre", "queued msg must survive close");
    }

    #[test]
    fn test_channel_len_invariant_after_mixed_ops() {
        let ch = __varg_channel_new(10);
        for i in 0..5 { __varg_channel_send(&ch, &i.to_string()); }
        assert_eq!(__varg_channel_len(&ch), 5);
        __varg_channel_try_recv(&ch);
        __varg_channel_try_recv(&ch);
        assert_eq!(__varg_channel_len(&ch), 3);
        __varg_channel_send(&ch, "extra");
        assert_eq!(__varg_channel_len(&ch), 4);
    }

    #[test]
    fn test_channel_empty_string_is_valid_message() {
        // Empty string is a legitimate value, not a sentinel for "empty channel"
        let ch = __varg_channel_new(5);
        __varg_channel_send(&ch, "");
        assert_eq!(__varg_channel_len(&ch), 1, "empty string must be counted as a message");
        // try_recv returns "" for both "got empty string" and "queue empty" — verify via len
        __varg_channel_try_recv(&ch);
        assert_eq!(__varg_channel_len(&ch), 0);
    }

    #[test]
    fn test_channel_very_large_message() {
        let ch = __varg_channel_new(1);
        let big = "x".repeat(1_000_000);
        assert!(__varg_channel_send(&ch, &big));
        let got = __varg_channel_try_recv(&ch);
        assert_eq!(got.len(), 1_000_000);
    }

    #[test]
    fn test_channel_concurrent_producer_consumer() {
        use std::thread;
        let ch = __varg_channel_new(100);
        let ch2 = ch.clone();
        let producer = thread::spawn(move || {
            for i in 0..50i32 {
                __varg_channel_send(&ch2, &i.to_string());
            }
        });
        producer.join().unwrap();
        let mut count = 0;
        while !__varg_channel_try_recv(&ch).is_empty() || __varg_channel_len(&ch) > 0 {
            count += 1;
            if count > 100 { break; }
        }
        // All 50 messages should have been produced; len should now be 0
        assert_eq!(__varg_channel_len(&ch), 0);
    }

    #[test]
    fn test_channel_close_is_idempotent() {
        let ch = __varg_channel_new(5);
        __varg_channel_close(&ch);
        __varg_channel_close(&ch); // must not panic
        assert!(__varg_channel_is_closed(&ch));
    }
}
