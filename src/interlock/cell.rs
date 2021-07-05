use std::sync::atomic::{AtomicUsize, Ordering};
use std::cell::UnsafeCell;
use std::ops::{DerefMut, Deref};

/**
This is a very specialized cell that has following semantics:
- it can be locked by calling lock()
- it can be unlocked by calling unlock()
- if the counter is fully unlocked (has lock count of 0), the reference to the data can be taken by calling take()
- when the reference goes out of scope, the counter goes into the 'completed' state
- when in 'completed' state, the counter can be reset

This type behavior can be described as the following state machine:
**/
pub struct CountCell<T: ?Sized> {
    borrow: AtomicUsize,
    value: UnsafeCell<T>
}

pub struct CountRef<'a, T: ?Sized> {
    value: &'a mut T,
    borrow: &'a AtomicUsize
}

const LOCK_BIT: usize = !(::core::usize::MAX >> 1); //currently locked
const COMP_BIT: usize = !(::core::usize::MAX >> 2) & !LOCK_BIT; //completed
const CNT_MASK: usize = !(LOCK_BIT | COMP_BIT);
impl<T: ?Sized> CountCell<T> {

    pub fn reset(&self, value: usize) {
        match self.borrow.compare_exchange(COMP_BIT, value, Ordering::Release, Ordering::Relaxed) {
            Err(v) => panic!("attempt to reset non completed counter: {}", v),
            _ => {}
        }
    }

    pub fn lock(&self) { //locks the counter so the task cannot be started w/o unlocking it first
        let new = self.borrow.fetch_add(1, Ordering::Acquire) + 1;

        if new == COMP_BIT {
            self.borrow.fetch_sub(1, Ordering::AcqRel);
            panic!("failed to acquire lock: too many locks");
        }
    }

    pub fn unlock(&self) -> bool { //unlocks the counter and returns true if task is fully unlocked
        let old = self.borrow.fetch_sub(1, Ordering::Release);

        if old & CNT_MASK == 0 {
            self.borrow.fetch_add(1, Ordering::AcqRel);
            panic!("failed to release the lock: lock underflow")
        }

        old == 1
    }

    // locks task forever so that it cannot be unlocked (only works if we have no locks atm)
    pub fn take(&self) -> Option<CountRef<T>> {
        match self.borrow.compare_exchange(
            0,
            LOCK_BIT,
            Ordering::AcqRel,
            Ordering::Relaxed) {
                Ok(_) => Some(CountRef {
                    borrow: &self.borrow,
                    value: unsafe { &mut *self.value.get() }
                }),
                Err(_) => None
        }
    }
}

impl<T> CountCell<T> {

    pub fn new(value: T) -> Self {
        Self { value: UnsafeCell::new(value), borrow: AtomicUsize::new(COMP_BIT) }
    }
}

impl<'a, T: ?Sized> Deref for CountRef<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T: ?Sized> DerefMut for CountRef<'a, T> {

    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<'a, T: ?Sized> Drop for CountRef<'a, T> {

    #[inline]
    fn drop(&mut self) {
        self.borrow.fetch_xor(LOCK_BIT | COMP_BIT, Ordering::AcqRel);
    }
}

//SAFETY: this cell is mutable borrow only
unsafe impl<T: ?Sized> Sync for CountCell<T> {}

mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "attempt to reset non completed counter: 2")]
    fn panic_double_reset() {
        let cell = CountCell::new(());

        cell.reset(2);
        cell.reset(2);
    }

    #[test]
    #[should_panic(expected = "failed to release the lock: lock underflow")]
    fn panic_underflow() {
        let cell = CountCell::new(());

        cell.reset(0);
        cell.unlock();
    }

    #[test]
    fn take() {
        let cell = CountCell::new(());

        cell.reset(1);
        assert!(cell.take().is_none(), "cell gave up value while locked");

        assert!(cell.unlock(), "cell failed to unlock");
        assert!(cell.take().is_some(), "cell does not want to give up the lock >/<");
    }

    #[test]
    fn unlock() {
        let cell = CountCell::new(());

        cell.reset(1);
        assert!(cell.unlock(), "cell failed to unlock");
    }
}