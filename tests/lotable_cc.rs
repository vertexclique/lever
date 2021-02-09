use lever::table::prelude::*;

use std::sync::Arc;

#[test]
fn lotable_concurrent() {
    let lotable = {
        let table: LOTable<String, u64> = LOTable::new();
        table.insert("data".into(), 1_u64).unwrap();
        Arc::new(table)
    };

    let mut threads = vec![];

    for thread_no in 0..100 {
        let lotable = lotable.clone();

        let t = std::thread::Builder::new()
            .name(format!("t_{}", thread_no))
            .spawn(move || {
                if thread_no % 2 == 0 {
                    // Reader threads
                    let _data = lotable.get(&"data".to_string());
                } else {
                    // Writer threads
                    let data = lotable.get(&"data".to_string()).unwrap();
                    lotable.insert("data".into(), data + 1).unwrap();
                }
            })
            .unwrap();

        threads.push(t);
    }

    for t in threads.into_iter() {
        t.join().unwrap();
    }
}
