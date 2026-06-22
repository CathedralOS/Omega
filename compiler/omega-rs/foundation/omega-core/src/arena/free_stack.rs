use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const EMPTY: u64 = 0;

pub struct FreeStack {
    head: AtomicU64,
    next: Box<[AtomicU32]>,
}

impl FreeStack {
    pub fn new(capacity: usize) -> Self {
        let next = (0..capacity).map(|_| AtomicU32::new(0)).collect();

        Self {
            head: AtomicU64::new(EMPTY),
            next,
        }
    }

    pub fn push(&self, index: u32) {
        debug_assert_ne!(index, 0);
        let slot_index = usize::try_from(index).expect("free stack index overflow");

        loop {
            let head = self.head.load(Ordering::Relaxed);
            let (old_index, old_tag) = unpack(head);
            self.next[slot_index].store(old_index, Ordering::Relaxed);
            let new_head = pack(index, old_tag.wrapping_add(1));

            if self
                .head
                .compare_exchange_weak(head, new_head, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }
    }

    pub fn pop(&self) -> Option<u32> {
        loop {
            let head = self.head.load(Ordering::Acquire);

            if head == EMPTY {
                return None;
            }

            let (index, tag) = unpack(head);
            let slot_index = usize::try_from(index).expect("free stack index overflow");
            let next_index = self.next[slot_index].load(Ordering::Relaxed);
            let new_head = if next_index == 0 {
                EMPTY
            } else {
                pack(next_index, tag.wrapping_add(1))
            };

            if self
                .head
                .compare_exchange_weak(head, new_head, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return Some(index);
            }
        }
    }
}

#[inline]
fn pack(index: u32, tag: u32) -> u64 {
    u64::from(index) | (u64::from(tag) << 32)
}

#[inline]
fn unpack(value: u64) -> (u32, u32) {
    (value as u32, (value >> 32) as u32)
}
