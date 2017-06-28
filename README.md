# hurdles

[![Crates.io](https://img.shields.io/crates/v/hurdles.svg)](https://crates.io/crates/hurdles)
[![Documentation](https://docs.rs/hurdles/badge.svg)](https://docs.rs/hurdles/)
[![Build Status](https://travis-ci.org/jonhoo/arccstr.svg?branch=master)](https://travis-ci.org/jonhoo/arccstr)

A thread barrier like [`std::sync::Barrier`] with a more scalable implementation than the one
provided by the standard library (which uses a `Mutex`). A barrier enables multiple threads to
synchronize the beginning of some computation.

## Examples

```rust
use hurdles::Barrier;
use std::thread;

let mut handles = Vec::with_capacity(10);
let mut barrier = Barrier::new(10);
for _ in 0..10 {
    let mut c = barrier.clone();
    // The same messages will be printed together.
    // You will NOT see any interleaving.
    handles.push(thread::spawn(move|| {
        println!("before wait");
        c.wait();
        println!("after wait");
    }));
}
// Wait for other threads to finish.
for handle in handles {
    handle.join().unwrap();
}
```

This crate currently implements a counter-based linear barrier as described in "3.1 Centralized
Barriers" in Mellor-Crummey and Scott’s paper [Algorithms for scalable synchronization on
shared-memory multiprocessors][1] from 1991. For a higher-level explanation, see Lars-Dominik
Braun's [Introduction to barrier algorithms][2].

[1]: https://dl.acm.org/citation.cfm?doid=103727.103729
[2]: https://6xq.net/barrier-intro/
[`std::sync::Barrier`]: https://doc.rust-lang.org/std/sync/struct.Barrier.html
