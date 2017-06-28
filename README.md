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

## Implementation

This crate currently implements a counter-based linear barrier as described in "3.1 Centralized
Barriers" in Mellor-Crummey and Scott’s paper [Algorithms for scalable synchronization on
shared-memory multiprocessors][1] from 1991. For a higher-level explanation, see Lars-Dominik
Braun's [Introduction to barrier algorithms][2].

## Numbers

Modern laptop with 2-core (4HT) Intel Core i7-5600U @ 2.60GHz:

```text
test tests::ours_2 ... bench:         135 ns/iter (+/- 1)
test tests::std_2  ... bench:         276 ns/iter (+/- 181)
test tests::ours_4 ... bench:         235 ns/iter (+/- 14)
test tests::std_4  ... bench:      11,882 ns/iter (+/- 111)
```

Dell server with 2x 10-core (20HT) Intel Xeon E5-2660 v3 @ 2.60GHz across two NUMA nodes:

```text
test tests::ours_4  ... bench:         568 ns/iter (+/- 4)
test tests::std_4   ... bench:       4,568 ns/iter (+/- 88)
test tests::ours_8  ... bench:       1,454 ns/iter (+/- 14)
test tests::std_8   ... bench:      17,668 ns/iter (+/- 322)
test tests::ours_16 ... bench:       2,856 ns/iter (+/- 32)
test tests::std_16  ... bench:      38,254 ns/iter (+/- 597)
test tests::ours_32 ... bench:       3,848 ns/iter (+/- 36)
test tests::std_32  ... bench:      86,194 ns/iter (+/- 15,506)
```

[1]: https://dl.acm.org/citation.cfm?doid=103727.103729
[2]: https://6xq.net/barrier-intro/
[`std::sync::Barrier`]: https://doc.rust-lang.org/std/sync/struct.Barrier.html
