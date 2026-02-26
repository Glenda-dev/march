use alloc::collections::BinaryHeap;
use glenda::cap::CapPtr;

#[derive(Eq, PartialEq)]
pub struct TimerEvent {
    pub deadline_ns: u64,
    pub reply_cap: CapPtr,
}

impl Ord for TimerEvent {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // Min-heap: reverse the ordering
        other.deadline_ns.cmp(&self.deadline_ns)
    }
}

impl PartialOrd for TimerEvent {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct TimerHeap {
    heap: BinaryHeap<TimerEvent>,
}

impl TimerHeap {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, deadline_ns: u64, reply_cap: CapPtr) {
        self.heap.push(TimerEvent {
            deadline_ns,
            reply_cap,
        });
    }

    pub fn peek_deadline(&self) -> Option<u64> {
        self.heap.peek().map(|e| e.deadline_ns)
    }

    pub fn pop_expired(&mut self, now_ns: u64) -> Option<CapPtr> {
        if let Some(event) = self.heap.peek() {
            if event.deadline_ns <= now_ns {
                return self.heap.pop().map(|e| e.reply_cap);
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}
