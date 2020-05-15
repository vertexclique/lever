use lever::prelude::*;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

fn main() {
    let lotable: Arc<RwLock<HashMap<String, u64>>> = Arc::new(RwLock::new(HashMap::default()));

    // RW from 1_000 threads concurrently.
    let thread_count = 8;
    let mut threads = vec![];

    for thread_no in 0..thread_count {
        let lotable = lotable.clone();

        let t = std::thread::Builder::new()
            .name(format!("t_{}", thread_no))
            .spawn(move || {
                let key = format!("{}", thread_no);
                let mut loguard = lotable.write().unwrap();
                loguard.insert(key.clone(), thread_no);
                drop(loguard);

                let loguard = lotable.read().unwrap();
                let _ = loguard.get(&key).unwrap();
            })
            .unwrap();

        threads.push(t);
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }
}
