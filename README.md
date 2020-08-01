<h1 align="center">
    <img src="https://github.com/vertexclique/lever/raw/master/img/lever-logo.png"/>
</h1>
<div align="center">
 <strong>
   Pillars for Transactional Systems and Data Grids
 </strong>
<hr>

[![Build Status](https://github.com/vertexclique/lever/workflows/CI/badge.svg)](https://github.com/vertexclique/lever/actions)
[![Latest Version](https://img.shields.io/crates/v/lever.svg)](https://crates.io/crates/lever)
[![Rust Documentation](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/lever/)
</div>

Lever is a library for writing transactional systems (esp. for in-memory data). It consists of various parts:
* `sync`: Synchronization primitives for transactional systems
* `table`: Various KV table kinds backed by transactional algorithms
* `txn`: Transactional primitives and management

Lever is using MVCC model to manage concurrency. It supplies building blocks for in-memory data stores for
transactional endpoints, databases and systems. Unblocked execution path is main aim for lever while 
not sacrificing failover mechanisms.

Lever provides STM, lock-free, wait-free synchronization primitives and various other tools to facilitate writing
transactional in-memory systems.

# Sync
Synchronization primitives which can allow users to write concurrent task structures. Lever don't have runtime or async code.
Whole library is based on top of POSIX threads and agnostic IO. That said, these are the few structures which can be used in sync package:

* Lock-free ReentrantRwLock
* Spinlocks
* Fair locks

# Tables

Lever's table system can be can be used like this:
```rust
use lever::prelude::*;
use std::sync::Arc;

fn main() {
    let lotable: Arc<LOTable<String, u64>> = Arc::new(LOTable::new());

    // RW from 1_000 threads concurrently.
    let thread_count = 1_000;
    let mut threads = vec![];

    for thread_no in 0..thread_count {
        let lotable = lotable.clone();

        let t = std::thread::Builder::new()
            .name(format!("t_{}", thread_no))
            .spawn(move || {
                let key = format!("{}", thread_no);
                lotable.insert(key.clone(), thread_no);
                let _ = lotable.get(&key).unwrap();
            })
            .unwrap();

        threads.push(t);
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }
}
```

Mind that Lever comes with MVCC. MVCC is fully implemented with BOCC style for optimistic locking.
Lever is under heavy work, other txn resolution algos and concurrency control mechanisms are under active development.

# Transaction System

Transaction system has couple of primitives. Which are `begin`, `commit` and various methods for management of the
transaction over the course of execution.

Example transaction would be like:
```rust
use lever::prelude::*;

let mut customers = TVar::new(123_456);

txn.begin(|t| {
    let mut churned = t.read(&customers);
    churned += 1;
    t.write(&mut customers, churned);
});

println!("I have {} customers right now. I gained 1.", customers.get_data());
```

For more examples please visit [examples](https://github.com/vertexclique/lever) directory.

## Performance

Initial benchmarks show very high throughput for varying workloads.

Workloads are separated in benchmarks like:
* Pure reads from concurrent 8 threads
* 80-20 R/RW mixed from concurrent 8 threads
* Pure writes from concurrent 8 threads

Lever is performant. E.g. Lever's table implementations are doing 25+ million operations under 1,9 seconds.
Whole thing is used in production and continuously improved. This crate consolidates plenty of primitives, tools, structures and such.
You can try benchmarking yourself. Benchmarking code is included.

## Notes for the user

Note that transactions never and ever inherit heavy work in their code path since they are mostly intended for accessing
to shared memory and concurrency enabled by their code paths success.

Isolation separated to threads not onto a global memory. That's why it is extremely fast.

Rollbacks are automatic, it won't interfere with your program, or bail. There will be system which incorporates fatal aborts. That work is ongoing.

## TODO

- [ ] Fatal aborts
- [ ] Other concurrency schemes
- [ ] Various conflict resolution strategies.
- [ ] Grid communication
- [ ] ...

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
