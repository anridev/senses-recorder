//! Single producer/single consumer storage. The capture callback is the sole
//! producer; the writer is the sole consumer. Neither side retains a slot.
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const MAX_FRAMES: usize = 8192;
const CAPACITY: usize = 32;

pub struct Block {
    pub frames: usize,
    pub samples: Box<[f32]>,
}

pub struct Queue {
    slots: Box<[UnsafeCell<Block>]>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

// Each slot has exactly one owner, transferred with release/acquire indices.
unsafe impl Sync for Queue {}

impl Queue {
    pub fn new() -> Self {
        Self {
            slots: (0..CAPACITY)
                .map(|_| {
                    UnsafeCell::new(Block {
                        frames: 0,
                        samples: vec![0.0; MAX_FRAMES * 2].into_boxed_slice(),
                    })
                })
                .collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Safety: only the single producer may call this; it may not retain slot references.
    pub unsafe fn push(&self, frames: usize, fill: impl FnOnce(&mut [f32])) -> bool {
        if frames > MAX_FRAMES {
            return false;
        }
        let head = self.head.load(Ordering::Relaxed);
        if head.wrapping_sub(self.tail.load(Ordering::Acquire)) >= CAPACITY {
            return false;
        }
        let block = unsafe { &mut *self.slots[head % CAPACITY].get() };
        block.frames = frames;
        fill(&mut block.samples[..frames * 2]);
        self.head.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    /// Safety: only the single consumer may call this; it may not retain slot references.
    pub unsafe fn pop(&self, consume: impl FnOnce(&Block)) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return false;
        }
        consume(unsafe { &*self.slots[tail % CAPACITY].get() });
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn overflow_never_overwrites_unread_audio() {
        let q = Queue::new();
        unsafe {
            for i in 0..CAPACITY {
                assert!(q.push(1, |s| s.fill(i as f32)));
            }
            assert!(!q.push(1, |_| panic!("must not overwrite")));
            for i in 0..CAPACITY {
                assert!(q.pop(|b| assert_eq!(b.samples[0], i as f32)));
            }
            assert!(!q.pop(|_| panic!("empty")));
            assert!(!q.push(MAX_FRAMES + 1, |_| {}));
        }
    }
    #[test]
    fn concurrent_wraparound_preserves_frames() {
        let q = Arc::new(Queue::new());
        let p = q.clone();
        let producer = std::thread::spawn(move || unsafe {
            for i in 0..20_000 {
                while !p.push(1, |s| s.fill(i as f32)) {
                    std::thread::yield_now();
                }
            }
        });
        unsafe {
            for i in 0..20_000 {
                while !q.pop(|b| assert_eq!(b.samples[0], i as f32)) {
                    std::thread::yield_now();
                }
            }
        }
        producer.join().unwrap();
    }
}
