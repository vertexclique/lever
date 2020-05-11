use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use lever::table::prelude::*;
use lever::txn::prelude::*;
use rand::prelude::*;
use rand_distr::Pareto;

fn pure_read(txn: Txn, tvar: TVar<LTable<String, String>>) -> LTable<String, String> {
    let res = txn.begin(|t: &mut Txn| t.read(&tvar));

    res
}

fn bench_pure_reads(c: &mut Criterion) {
    let mut ltable = LTable::<String, String>::create("pure_reads".to_owned());
    ltable.insert("Interstellar".into(), "Gravity".into());

    let txn = TxnManager::manager().txn_build(
        TransactionConcurrency::Optimistic,
        TransactionIsolation::Serializable,
        100_usize,
        1_usize,
        "pure_reads".into(),
    );

    c.bench_function("pure_reads", move |b| {
        b.iter_batched(
            || {
                let tvar = TVar::new(ltable.clone());
                tvar
            },
            |tvar| pure_read(txn.clone(), tvar),
            BatchSize::SmallInput,
        )
    });
}

fn rw_pareto(txn: Txn, mut vars: (f64, TVar<LTable<String, String>>)) {
    if vars.0 < 0.8_f64 {
        txn.begin(|t: &mut Txn| {
            t.read(&vars.1);
        });
    } else {
        txn.begin(|t: &mut Txn| {
            let mut x = t.read(&vars.1);
            x.insert("RoboCop".into(), "Annihilation".into());
            t.write(&mut vars.1, x.clone());
        });
    }
}

fn bench_rw_pareto(c: &mut Criterion) {
    let mut ltable = LTable::<String, String>::create("rw_pareto".to_owned());
    ltable.insert("Interstellar".into(), "Gravity".into());

    let txn = TxnManager::manager().txn_build(
        TransactionConcurrency::Optimistic,
        TransactionIsolation::Serializable,
        100_usize,
        1_usize,
        "rw_pareto".into(),
    );

    c.bench_function("rw_pareto", move |b| {
        b.iter_batched(
            || {
                let tvar = TVar::new(ltable.clone());
                let dist: f64 =
                    1. / thread_rng().sample(Pareto::new(1., 5.0_f64.log(4.0_f64)).unwrap());

                (dist, tvar)
            },
            |vars| rw_pareto(txn.clone(), vars),
            BatchSize::SmallInput,
        )
    });
}

fn pure_write(txn: Txn, mut vars: TVar<LTable<String, String>>) {
    txn.begin(|t: &mut Txn| {
        let mut x = t.read(&vars);
        x.insert("RoboCop".into(), "Annihilation".into());
        t.write(&mut vars, x.clone());
    });
}

fn bench_pure_write(c: &mut Criterion) {
    let mut ltable = LTable::<String, String>::create("pure_writes".to_owned());
    ltable.insert("Interstellar".into(), "Gravity".into());

    let txn = TxnManager::manager().txn_build(
        TransactionConcurrency::Optimistic,
        TransactionIsolation::Serializable,
        100_usize,
        1_usize,
        "pure_write".into(),
    );

    c.bench_function("pure_writes", move |b| {
        b.iter_batched(
            || {
                let tvar = TVar::new(ltable.clone());
                tvar
            },
            |tvar| pure_write(txn.clone(), tvar),
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = op_ser_benches;
    config = Criterion::default();
    targets = bench_pure_reads, bench_rw_pareto, bench_pure_write
}
criterion_main!(op_ser_benches);
