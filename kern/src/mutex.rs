use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut, Drop};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use aarch64;
use crate::percore;

#[repr(align(32))]
pub struct Mutex<T> {
    data: UnsafeCell<T>,
    lock: AtomicBool,
    owner: AtomicUsize,
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T: 'a> {
    lock: &'a Mutex<T>,
}

impl<'a, T> !Send for MutexGuard<'a, T> {}
unsafe impl<'a, T: Sync> Sync for MutexGuard<'a, T> {}

impl<T> Mutex<T> {
    pub const fn new(val: T) -> Mutex<T> {
        Mutex {
            lock: AtomicBool::new(false),
            owner: AtomicUsize::new(usize::max_value()),
            data: UnsafeCell::new(val),
        }
    }
}

impl<T> Mutex<T> {
    // Once MMU/cache is enabled, do the right thing here. For now, we don't
    // need any real synchronization.
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        match percore::is_mmu_ready() {
            false => {
                // let this = percore::getcpu();
                let this = aarch64::affinity();
                assert_eq!(this, 0);
                if !self.lock.load(Ordering::Relaxed) || self.owner.load(Ordering::Relaxed) == this {
                    self.lock.store(true, Ordering::Relaxed);
                    self.owner.store(this, Ordering::Relaxed);
                    Some(MutexGuard { lock: &self })
                } else {
                    None
                }
            }
            true => {
                if !self.lock.compare_and_swap(false, true, Ordering::AcqRel) {
                    self.owner.store(percore::getcpu(), Ordering::Release);
                    Some(MutexGuard { lock: &self })
                } else {
                    None
                }
            }
        }

    }

    // Once MMU/cache is enabled, do the right thing here. For now, we don't
    // need any real synchronization.
    #[inline(never)]
    pub fn lock(&self) -> MutexGuard<T> {
        // Wait until we can "aquire" the lock, then "acquire" it.
        loop {
            match self.try_lock() {
                Some(guard) => return guard,
                None => continue,
            }
        }
    }

    fn unlock(&self) {
        let this = aarch64::affinity();
        match percore::is_mmu_ready() {
            false => {
                assert_eq!(this, 0);
                self.lock.store(false, Ordering::Relaxed);
            },
            true => {
                if self.owner.load(Ordering::Acquire) == this {
                    self.lock.store(false, Ordering::Release);
                    if this != 0 || percore::get_preemptive_counter() !=0 {
                        percore::putcpu(this);
                    }
                }

            }
        }
    }
}

impl<'a, T: 'a> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: 'a> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: 'a> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock()
    }
}

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
            None => f.debug_struct("Mutex").field("data", &"<locked>").finish(),
        }
    }
}
