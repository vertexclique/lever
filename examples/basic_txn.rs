use lever::txn::prelude::*;

fn main() {
    let manager = lever::lever().manager();

    let txn = manager.txn_build(
        // Select concurrency scheme
        TransactionConcurrency::Optimistic,
        // Select isolation scheme
        TransactionIsolation::RepeatableRead,
        // Give timeout for transaction conflict resolution in milliseconds
        100_usize,
        // Work element size inside the given transaction
        1_usize,
        // Name of the transaction
        "basic_txn".into(),
    );

    let mut customers = TVar::new(123_456);

    println!("I have {} customers right now.", customers.get_data());

    txn.begin(|t| {
        let mut churned = t.read(&customers);
        churned += 1;
        t.write(&mut customers, churned);
    })
    .unwrap();

    println!(
        "I have {} customers right now. I gained 1.",
        customers.get_data()
    );

    // You don't necessarily need to use convenience methods.

    txn.begin(|t| {
        let mut churned = *customers;
        churned -= 123_000;
        t.write(&mut customers, churned);
    })
    .unwrap();

    println!(
        "I have {} customers right now. I think I lost a lot.",
        customers.get_data()
    );
}
