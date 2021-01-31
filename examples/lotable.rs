use lever::prelude::*;
use std::sync::Arc;

fn main() {
    let lotable: Arc<LOTable<String, u64>> = Arc::new(LOTable::new());

    // RW from 1_000 threads concurrently.
    let thread_count = 8;
    let mut threads = vec![];

    for thread_no in 0..thread_count {
        let lotable = lotable.clone();

        let t = std::thread::Builder::new()
            .name(format!("t_{}", thread_no))
            .spawn(move || {
                let key = format!("{}", thread_no);
                lotable.insert(key.clone(), thread_no).unwrap();
                let _ = lotable.get(&key).unwrap();
            })
            .unwrap();

        threads.push(t);
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }
}
