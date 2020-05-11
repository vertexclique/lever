/// Ifaces for lock-free concurrency primitives
pub mod ifaces;

pub(crate) mod arcunique;
pub(crate) mod atomics;

/// Tas based reentrant RW lock implementation
pub mod rerwlock;
/// Basic treiber stack
pub mod treiber;
/// TTas based spin lock implementation
pub(crate) mod ttas;

///
/// Prelude for the synchronization primitives
pub mod prelude {
    pub use super::rerwlock::*;
    pub use super::treiber::*;
    pub use super::ttas::*;
}
