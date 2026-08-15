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
//`with` hands out &T to up to 254 concurrent readers, so T must be Sync, not just Send.
unsafe impl<T: Send + Sync> Sync for Spinlock<T> {}

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
        //hand-rolled rather than `fetch_update`, which nightly has deprecated in
        //favour of a `try_update` that our MSRV does not have
        let mut current = self.locked.load(Relaxed);
        loop {
            if current >= LOCKED_WRITE - 1 {
                //a writer holds the lock, or we are at the reader ceiling
                std::hint::spin_loop();
                current = self.locked.load(Relaxed);
                continue;
            }
            match self
                .locked
                .compare_exchange_weak(current, current + 1, Acquire, Relaxed)
            {
                Ok(_) => return,
                Err(actual) => {
                    std::hint::spin_loop();
                    current = actual;
                }
            }
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

        //unlock on drop so a panic in f can't leave the lock held forever
        struct WriteUnlock<'a, T>(&'a Spinlock<T>);
        impl<T> Drop for WriteUnlock<'_, T> {
            fn drop(&mut self) {
                self.0.spin_unlock_write();
            }
        }
        let _guard = WriteUnlock(self);

        // SAFETY: We have exclusive access to the data now
        unsafe { f(&mut *self.data.get()) }
    }

    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        // Spin until we can acquire the lock
        self.spin_lock_read();

        //unlock on drop so a panic in f can't leave the lock held forever
        struct ReadUnlock<'a, T>(&'a Spinlock<T>);
        impl<T> Drop for ReadUnlock<'_, T> {
            fn drop(&mut self) {
                self.0.spin_unlock_read();
            }
        }
        let _guard = ReadUnlock(self);

        // SAFETY: We have shared access to the data now
        unsafe { f(&*self.data.get()) }
    }
}
