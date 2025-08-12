// SPDX-License-Identifier: MIT OR Apache-2.0
/*!
On the wasm main thread, we can't necessarily lock.

Instead we use a spinlock.  It is important to ensure that the spinlock
must be held for as short a time as possible.
*/

use std::cell::UnsafeCell;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

const UNLOCKED: u8 = 0;
//we allow for 254 readers or 1 writer (val = 255)
const LOCKED_WRITE: u8 = u8::MAX;

pub struct Spinlock<T> {
    data: UnsafeCell<T>,
    locked: std::sync::atomic::AtomicU8,
}

unsafe impl<T: Send> Send for Spinlock<T> {}
unsafe impl<T: Send> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    pub fn new(data: T) -> Self {
        Spinlock {
            data: UnsafeCell::new(data),
            locked: AtomicU8::new(UNLOCKED),
        }
    }

    fn spin_lock_write(&self) {
        // Spin until we can acquire the lock
        while self
            .locked
            .compare_exchange_weak(UNLOCKED, LOCKED_WRITE, Acquire, Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
    }

    fn spin_unlock_write(&self) {
        // Release the lock
        self.locked.store(UNLOCKED, Release);
    }

    fn spin_lock_read(&self) {
        while self
            .locked
            .fetch_update(Acquire, Relaxed, |v| {
                if v < (LOCKED_WRITE - 1) {
                    Some(v + 1)
                } else {
                    None
                }
            })
            .is_err()
        {
            std::hint::spin_loop();
        }
    }
    fn spin_unlock_read(&self) {
        // Release the lock
        self.locked.fetch_sub(1, Release);
    }

    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        // Spin until we can acquire the lock
        self.spin_lock_write();

        // SAFETY: We have exclusive access to the data now
        let result = unsafe { f(&mut *self.data.get()) };

        // Release the lock
        self.spin_unlock_write();
        result
    }

    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        // Spin until we can acquire the lock
        self.spin_lock_read();

        // SAFETY: We have shared access to the data now
        let result = unsafe { f(&*self.data.get()) };

        // Release the lock
        self.spin_unlock_read();
        result
    }
}
