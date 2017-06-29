//! A scalable barrier (like [`std::sync::Barrier`]) that enables multiple threads to synchronize
//! the beginning of some computation.
//!
//! This crate provides a similar interface as [`std::sync::Barrier`], but behaves much better in
//! the face of many concurrently waiting threads, and incurs a lower per-thread latency penalty
//! (see benchmarks below). The interface does differ from the standard library barrier however:
//!
//!  - `Barrier` in this crate is `Clone`, and should *not* be wrapped in a `sync::Arc`.
//!  - `Barrier::wait` in this crate takes a `&mut self` receiver as each thread must keep some
//!    local state.
//!
//! Furthermore, when a thread blocks on `Barrier::wait`, the thread will (currently) *never* be
//! suspended, and instead spin on the barrier. For the first few spins, it will also not call
//! `sched_yield` to avoid the cost of thread sleep/wakeup. If threads are expected to reach the
//! barrier at nearly the same time, or barrier latency is critical, this is probably what you
//! want. However, if barriers are staggered and far between, then you may want to use
//! [`std::sync::Barrier`] instead, as it is better about handling long waits.
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
//! # Implementation
//!
//! At the time of writing, the implementation of `std::sync::Barrier` internally uses a `Mutex`,
//! which causes contention with many waiting threads, and incurs an undue performance overhead for
//! each call to `wait`.
//!
//! This crate instead implements a counter-based linear barrier as described in "3.1 Centralized
//! Barriers" in Mellor-Crummey and Scott’s paper [Algorithms for scalable synchronization on
//! shared-memory multiprocessors][1] from 1991. For a higher-level explanation, see Lars-Dominik
//! Braun's [Introduction to barrier algorithms][2].
//!
//! # Numbers
//!
//! Modern laptop with 2-core (4HT) Intel Core i7-5600U @ 2.60GHz:
//!
//! ```text
//! test tests::ours_2 ... bench:         190 ns/iter (+/- 24)
//! test tests::std_2  ... bench:       2,054 ns/iter (+/- 822)
//! test tests::ours_4 ... bench:         236 ns/iter (+/- 2)
//! test tests::std_4  ... bench:      11,913 ns/iter (+/- 60)
//! ```
//!
//! Dell server with 2x 10-core (20HT) Intel Xeon E5-2660 v3 @ 2.60GHz across two NUMA nodes:
//!
//! ```text
//! test tests::ours_4  ... bench:         689 ns/iter (+/- 9)
//! test tests::std_4   ... bench:       4,762 ns/iter (+/- 151)
//! test tests::ours_8  ... bench:       1,380 ns/iter (+/- 13)
//! test tests::std_8   ... bench:      17,545 ns/iter (+/- 288)
//! test tests::ours_16 ... bench:       2,970 ns/iter (+/- 33)
//! test tests::std_16  ... bench:      38,215 ns/iter (+/- 469)
//! test tests::ours_32 ... bench:       3,838 ns/iter (+/- 129)
//! test tests::std_32  ... bench:      94,266 ns/iter (+/- 12,243)
//! ```
//!
//! [1]: https://dl.acm.org/citation.cfm?doid=103727.103729
//! [2]: https://6xq.net/barrier-intro/
//! [`std::sync::Barrier`]: https://doc.rust-lang.org/std/sync/struct.Barrier.html
#![deny(missing_docs)]
#![cfg_attr(feature = "nightly", feature(test))]

#[cfg(feature = "nightly")]
extern crate test;

extern crate parking_lot_core;

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
            let mut wait = parking_lot_core::SpinWait::new();
            while self.inner.gsense.load(atomic::Ordering::SeqCst) != self.lsense {
                // XXX: in theory we could go even further and park the thread eventually
                wait.spin();
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

    #[cfg(feature = "nightly")]
    use test::Bencher;

    #[cfg(feature = "nightly")]
    const BENCH_THREADS: usize = 4;

    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", bench)]
    fn ours(b: &mut Bencher) {
        let mut barrier = Barrier::new(BENCH_THREADS);
        for _ in 0..(BENCH_THREADS - 1) {
            let mut barrier = barrier.clone();
            thread::spawn(move || loop {
                barrier.wait();
            });
        }
        b.iter(move || { barrier.wait(); })
    }

    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", bench)]
    fn std(b: &mut Bencher) {
        use std::sync::{self, Arc};
        let barrier = Arc::new(sync::Barrier::new(BENCH_THREADS));
        for _ in 0..(BENCH_THREADS - 1) {
            let barrier = barrier.clone();
            thread::spawn(move || loop {
                barrier.wait();
            });
        }
        b.iter(move || { barrier.wait(); })
    }

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
