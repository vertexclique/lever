use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use lever::index::zonemap::ZoneMap;

fn bench_zonemap_selected(c: &mut Criterion) {
    c.bench_function("bench_unoptimized", move |b| {
        b.iter_batched(
            || {
                let customers: Vec<i32> =
                    vec![vec![1, 0, -1, -2].repeat(500), vec![1, 2, 3, 4].repeat(250)].concat();

                let ingestion_data = vec![("customers", customers.as_slice())];
                (ZoneMap::from(ingestion_data), customers)
            },
            |(zm, customers)| {
                let (l, r) = zm.scan_range("customers", 4, 4, &*customers);
                customers[l..=r].iter().filter(|x| **x >= 4).sum::<i32>()
            },
            BatchSize::LargeInput,
        )
    });
}

fn bench_unoptimized(c: &mut Criterion) {
    c.bench_function("bench_zonemap_selected", move |b| {
        b.iter_batched(
            || {
                let customers: Vec<i32> =
                    vec![vec![1, 0, -1, -2].repeat(500), vec![1, 2, 3, 4].repeat(250)].concat();

                let _ingestion_data = vec![("customers", customers.as_slice())];

                customers
            },
            |data| {
                data.as_slice()
                    .into_iter()
                    .filter(|x| **x >= 4)
                    .sum::<i32>()
            },
            BatchSize::LargeInput,
        )
    });
}

criterion_group! {
    name = zonemap_benches;
    config = Criterion::default();
    targets = bench_zonemap_selected, bench_unoptimized
}
criterion_main!(zonemap_benches);
