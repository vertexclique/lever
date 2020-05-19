use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};

use rand::prelude::*;
use rand_distr::Pareto;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn pure_read(lotable: Arc<RwLock<HashMap<String, u64>>>, key: String, thread_count: u64) {
    let mut threads = vec![];

    for thread_no in 0..thread_count {
        let lotable = lotable.clone();
        let key = key.clone();

        let t = std::thread::Builder::new()
            .name(format!("t_{}", thread_no))
            .spawn(move || {
                let loguard = lotable.read().unwrap();
                loguard.get(&key);

                // if let Ok(loguard) = lotable.read() {
                //     loguard.get(&key);
                // }
            })
            .unwrap();

        threads.push(t);
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }
}

fn bench_arc_rwlock_pure_reads(c: &mut Criterion) {
    let lotable = {
        let mut table: HashMap<String, u64> = HashMap::new();
        table.insert("data".into(), 1_u64);
        Arc::new(RwLock::new(table))
    };
    let key: String = "CORE".into();

    let threads = 8;

    let mut group = c.benchmark_group("arc_rwlock_read_throughput");
    group.throughput(Throughput::Elements(threads as u64));
    group.bench_function("pure reads", move |b| {
        b.iter_batched(
            || (lotable.clone(), key.clone()),
            |vars| pure_read(vars.0, vars.1, threads),
            BatchSize::SmallInput,
        )
    });
}

////////////////////////////////

fn rw_pareto(
    lotable: Arc<RwLock<HashMap<String, u64>>>,
    key: String,
    dist: f64,
    thread_count: u64,
) {
    let mut threads = vec![];

    for thread_no in 0..thread_count {
        let lotable = lotable.clone();
        let key = key.clone();

        let t = std::thread::Builder::new()
            .name(format!("t_{}", thread_no))
            .spawn(move || {
                if dist < 0.8_f64 {
                    let loguard = lotable.read().unwrap();
                    loguard.get(&key);

                // if let Ok(loguard) = lotable.read() {
                //     loguard.get(&key);
                // }
                } else {
                    let loguard = lotable.read().unwrap();
                    let data: u64 = *loguard.get(&key).unwrap();

                    // if let Ok(loguard) = lotable.read() {
                    //     if let Some(datac) = loguard.get(&key) {
                    //         data = *datac;
                    //     }
                    // }

                    let mut loguard = lotable.write().unwrap();
                    loguard.insert(key, data + 1);
                    // if let Ok(mut loguard) = lotable.write() {
                    //     loguard.insert(key, data + 1);
                    // }
                }
            })
            .unwrap();

        threads.push(t);
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }
}

fn bench_arc_rwlock_rw_pareto(c: &mut Criterion) {
    let lotable = {
        let mut table: HashMap<String, u64> = HashMap::new();
        table.insert("data".into(), 1_u64);
        Arc::new(RwLock::new(table))
    };
    let key: String = "CORE".into();

    let threads = 8;

    let mut group = c.benchmark_group("arc_rwlock_rw_pareto_throughput");
    group.throughput(Throughput::Elements(threads as u64));
    group.bench_function("rw_pareto", move |b| {
        b.iter_batched(
            || {
                let dist: f64 =
                    1. / thread_rng().sample(Pareto::new(1., 5.0_f64.log(4.0_f64)).unwrap());
                (lotable.clone(), key.clone(), dist)
            },
            |vars| rw_pareto(vars.0, vars.1, vars.2, threads),
            BatchSize::SmallInput,
        )
    });
}

////////////////////////////////

fn pure_writes(lotable: Arc<RwLock<HashMap<String, u64>>>, key: String, thread_count: u64) {
    let mut threads = vec![];

    for thread_no in 0..thread_count {
        let lotable = lotable.clone();
        let key = key.clone();

        let t = std::thread::Builder::new()
            .name(format!("t_{}", thread_no))
            .spawn(move || {
                if let Ok(mut loguard) = lotable.write() {
                    loguard.insert(key, thread_no);
                }
            })
            .unwrap();

        threads.push(t);
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }
}

fn bench_arc_rwlock_pure_writes(c: &mut Criterion) {
    let lotable = {
        let mut table: HashMap<String, u64> = HashMap::new();
        table.insert("data".into(), 1_u64);
        Arc::new(RwLock::new(table))
    };
    let key: String = "CORE".into();

    let threads = 8;

    let mut group = c.benchmark_group("arc_rwlock_write_throughput");
    group.throughput(Throughput::Elements(threads as u64));
    group.bench_function("pure writes", move |b| {
        b.iter_batched(
            || (lotable.clone(), key.clone()),
            |vars| pure_writes(vars.0, vars.1, threads),
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = arc_rwlock_benches;
    config = Criterion::default();
    targets = bench_arc_rwlock_pure_reads, bench_arc_rwlock_rw_pareto, bench_arc_rwlock_pure_writes
}
criterion_main!(arc_rwlock_benches);
