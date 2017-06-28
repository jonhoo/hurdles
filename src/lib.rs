//! A thread barrier like [`std::sync::Barrier`] with a more scalable implementation than the one
//! provided by the standard library (which uses a `Mutex`). A barrier enables multiple threads to
//! synchronize the beginning of some computation.
//!
//! # Examples
//!
//! ```
//! use hurdles::Barrier;
//! use std::thread;
//!
//! let mut handles = Vec::with_capacity(10);
//! let mut barrier = Barrier::new(10);
//! for _ in 0..10 {
//!     let mut c = barrier.clone();
//!     // The same messages will be printed together.
//!     // You will NOT see any interleaving.
//!     handles.push(thread::spawn(move|| {
//!         println!("before wait");
//!         c.wait();
//!         println!("after wait");
//!     }));
//! }
//! // Wait for other threads to finish.
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//! ```
//!
//! This crate currently implements a counter-based linear barrier as described in "3.1 Centralized
//! Barriers" in Mellor-Crummey and Scott’s paper [Algorithms for scalable synchronization on
//! shared-memory multiprocessors][1] from 1991. For a higher-level explanation, see Lars-Dominik
//! Braun's [Introduction to barrier algorithms][2].
//!
//! [1]: https://dl.acm.org/citation.cfm?doid=103727.103729
//! [2]: https://6xq.net/barrier-intro/
//! [`std::sync::Barrier`]: https://doc.rust-lang.org/std/sync/struct.Barrier.html
#![deny(missing_docs)]

use std::sync::{atomic, Arc};

struct BarrierInner {
    gsense: atomic::AtomicBool,
    count: atomic::AtomicUsize,
    max: usize,
}

/// A barrier which enables multiple threads to synchronize the beginning of some computation.
pub struct Barrier {
    inner: Arc<BarrierInner>,
    lsense: bool,
    used: bool,
}

/// A `BarrierWaitResult` is returned by [`wait`] when all threads in the [`Barrier`]
/// have rendezvoused.
///
/// # Examples
///
/// ```
/// use hurdles::Barrier;
///
/// let mut barrier = Barrier::new(1);
/// let barrier_wait_result = barrier.wait();
/// ```
///
/// [`wait`]: struct.Barrier.html#method.wait
/// [`Barrier`]: struct.Barrier.html
pub struct BarrierWaitResult(bool);

impl Barrier {
    /// Creates a new barrier that can block a given number of threads.
    ///
    /// A barrier will block `n-1` threads which call [`wait`] and then wake up all threads at once
    /// when the `n`th thread calls [`wait`].
    ///
    /// # Examples
    ///
    /// ```
    /// use hurdles::Barrier;
    /// let mut barrier = Barrier::new(10);
    /// ```
    ///
    /// [`wait`]: struct.Barrier.html#method.wait
    pub fn new(n: usize) -> Self {
        Barrier {
            used: false,
            lsense: true,
            inner: Arc::new(BarrierInner {
                gsense: atomic::AtomicBool::new(true),
                count: atomic::AtomicUsize::new(n),
                max: n,
            }),
        }
    }

    /// Blocks the current thread until all threads have rendezvoused here.
    ///
    /// Barriers are re-usable after all threads have rendezvoused once, and can be used
    /// continuously.
    ///
    /// A single (arbitrary) thread will receive a [`BarrierWaitResult`] that returns `true` from
    /// [`is_leader`] when returning from this function, and all other threads will receive a
    /// result that will return `false` from [`is_leader`].
    ///
    /// # Examples
    ///
    /// ```
    /// use hurdles::Barrier;
    /// use std::thread;
    ///
    /// let mut handles = Vec::with_capacity(10);
    /// let mut barrier = Barrier::new(10);
    /// for _ in 0..10 {
    ///     let mut c = barrier.clone();
    ///     // The same messages will be printed together.
    ///     // You will NOT see any interleaving.
    ///     handles.push(thread::spawn(move|| {
    ///         println!("before wait");
    ///         c.wait();
    ///         println!("after wait");
    ///     }));
    /// }
    /// // Wait for other threads to finish.
    /// for handle in handles {
    ///     handle.join().unwrap();
    /// }
    /// ```
    ///
    /// [`BarrierWaitResult`]: struct.BarrierWaitResult.html
    /// [`is_leader`]: struct.BarrierWaitResult.html#method.is_leader
    pub fn wait(&mut self) -> BarrierWaitResult {
        self.used = true;
        self.lsense = !self.lsense;
        if self.inner.count.fetch_sub(1, atomic::Ordering::SeqCst) == 1 {
            // we're the last to reach the barrier -- release all
            self.inner
                .count
                .store(self.inner.max, atomic::Ordering::SeqCst);
            self.inner
                .gsense
                .store(self.lsense, atomic::Ordering::SeqCst);
            BarrierWaitResult(true)
        } else {
            // wait for everyone to reach the barrier
            while self.inner.gsense.load(atomic::Ordering::SeqCst) != self.lsense {
                // spin
            }
            BarrierWaitResult(false)
        }
    }
}

impl Clone for Barrier {
    fn clone(&self) -> Self {
        assert!(!self.used);
        Barrier {
            used: false,
            lsense: self.lsense,
            inner: self.inner.clone(),
        }
    }
}

impl BarrierWaitResult {
    /// Returns whether this thread from [`wait`] is the "leader thread".
    ///
    /// Only one thread will have `true` returned from their result, all other
    /// threads will have `false` returned.
    ///
    /// [`wait`]: struct.Barrier.html#method.wait
    ///
    /// # Examples
    ///
    /// ```
    /// use hurdles::Barrier;
    ///
    /// let mut barrier = Barrier::new(1);
    /// let barrier_wait_result = barrier.wait();
    /// assert_eq!(barrier_wait_result.is_leader(), true);
    /// ```
    pub fn is_leader(&self) -> bool {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Barrier;
    use std::sync::mpsc::{channel, TryRecvError};
    use std::thread;

    #[test]
    fn test_barrier() {
        const N: usize = 10;

        let mut barrier = Barrier::new(N);
        let (tx, rx) = channel();

        for _ in 0..N - 1 {
            let mut c = barrier.clone();
            let tx = tx.clone();
            thread::spawn(move || { tx.send(c.wait().is_leader()).unwrap(); });
        }

        // At this point, all spawned threads should be blocked,
        // so we shouldn't get anything from the port
        assert!(match rx.try_recv() {
            Err(TryRecvError::Empty) => true,
            _ => false,
        });

        let mut leader_found = barrier.wait().is_leader();

        // Now, the barrier is cleared and we should get data.
        for _ in 0..N - 1 {
            if rx.recv().unwrap() {
                assert!(!leader_found);
                leader_found = true;
            }
        }
        assert!(leader_found);
    }
}
