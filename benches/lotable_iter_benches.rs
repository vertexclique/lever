use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use lever::table::prelude::*;

use rand::prelude::*;
use rand_distr::Pareto;
use rayon::prelude::*;
use std::sync::Arc;

const BATCH_SIZE: usize = 50;

fn pure_read(lotable: Arc<LOTable<String, u64>>, key: String) {
    (0..BATCH_SIZE).into_par_iter().for_each(|_| {
        lotable.get(&key.clone());
    });
}

fn bench_lotable_pure_reads(c: &mut Criterion) {
    let lotable = {
        let table: LOTable<String, u64> = LOTable::new();
        table.insert("data".into(), 1_u64);
        Arc::new(table)
    };
    let key: String = "CORE".into();
    lotable.insert(key.clone(), 123_456);

    let mut group = c.benchmark_group("lotable_iter_read_throughput");
    group.throughput(Throughput::Elements(BATCH_SIZE as u64));
    group.bench_function("pure reads", move |b| {
        b.iter_batched(
            || (lotable.clone(), key.clone()),
            |vars| pure_read(vars.0, vars.1),
            BatchSize::SmallInput,
        )
    });
}

fn rw_pareto(lotable: Arc<LOTable<String, u64>>, key: String, dist: f64) {
    (0..BATCH_SIZE).into_par_iter().for_each(|_| {
        if dist < 0.8_f64 {
            lotable.get(&key.clone());
        } else {
            let data = lotable.get(&key).unwrap();
            lotable.insert(key.clone(), data + 1);
        }
    });
}

fn bench_lotable_rw_pareto(c: &mut Criterion) {
    let lotable = {
        let table: LOTable<String, u64> = LOTable::new();
        table.insert("data".into(), 1_u64);
        Arc::new(table)
    };
    let key: String = "CORE".into();
    lotable.insert(key.clone(), 123_456);

    let mut group = c.benchmark_group("lotable_iter_rw_pareto_throughput");
    group.throughput(Throughput::Elements(BATCH_SIZE as u64));
    group.bench_function("rw_pareto", move |b| {
        b.iter_batched(
            || {
                let dist: f64 =
                    1. / thread_rng().sample(Pareto::new(1., 5.0_f64.log(4.0_f64)).unwrap());
                (lotable.clone(), key.clone(), dist)
            },
            |vars| rw_pareto(vars.0, vars.1, vars.2),
            BatchSize::SmallInput,
        )
    });
}

////////////////////////////////

fn pure_writes(lotable: Arc<LOTable<String, u64>>, key: String) {
    (0..BATCH_SIZE).into_par_iter().for_each(|i| {
        lotable.insert(key.clone(), i as u64);
    });
}

fn bench_lotable_pure_writes(c: &mut Criterion) {
    let lotable = {
        let table: LOTable<String, u64> = LOTable::new();
        table.insert("data".into(), 1_u64);
        Arc::new(table)
    };
    let key: String = "CORE".into();
    lotable.insert(key.clone(), 123_456);

    let mut group = c.benchmark_group("lotable_iter_write_throughput");
    group.throughput(Throughput::Elements(BATCH_SIZE as u64));
    group.bench_function("pure writes", move |b| {
        b.iter_batched(
            || (lotable.clone(), key.clone()),
            |vars| pure_writes(vars.0, vars.1),
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = lotable_iter_benches;
    config = Criterion::default();
    targets = bench_lotable_pure_reads, bench_lotable_rw_pareto, bench_lotable_pure_writes
}
criterion_main!(lotable_iter_benches);
