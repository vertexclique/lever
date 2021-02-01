use lever::sync::atomics::AtomicBox;

use crossbeam_utils::thread;
use std::cell::Cell;

#[derive(Debug, Clone, Copy)]
enum RefOrInt<'a> {
    Ref(&'a u64),
    Int(u64),
}
static SOME_INT: u64 = 123;

// https://github.com/vertexclique/lever/issues/15#issue-740069651
// Run this one in release mode for it to fail
fn main() {
    let cell = Cell::new(RefOrInt::Ref(&SOME_INT));
    let atomic_box = AtomicBox::new(&cell);

    thread::scope(|s| {
        s.spawn(move |_| {
            let smuggled_cell = atomic_box.get();

            loop {
                // Repeatedly write Ref(&addr) and Int(0xdeadbeef) into the cell.
                smuggled_cell.set(RefOrInt::Ref(&SOME_INT));
                smuggled_cell.set(RefOrInt::Int(0xdeadbeef));
            }
        });

        loop {
            if let RefOrInt::Ref(addr) = cell.get() {
                // Hope that between the time we pattern match the object as a
                // `Ref`, it gets written to by the other thread.
                if addr as *const u64 == &SOME_INT as *const u64 {
                    continue;
                }

                // Due to the data race, obtaining Ref(0xdeadbeef) is possible
                println!("Pointer is now: {:p}", addr);
                println!("Dereferencing addr will now segfault: {}", *addr);
            }
        }
    });
}
