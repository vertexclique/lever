use std::sync::{
    atomic::{self, AtomicBool, AtomicU8, AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Debug)]
struct Balancer {
    toggle: Arc<AtomicBool>,
}

impl Balancer {
    pub fn new() -> Balancer {
        Balancer {
            toggle: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Returns output wire based on the switch
    pub fn traverse(&self) -> usize {
        // TODO: refactor
        let res = self.toggle.load(Ordering::SeqCst);
        self.toggle.store(!res, Ordering::SeqCst);
        if res {
            0_usize
        } else {
            1_usize
        }
    }
}

unsafe impl Send for Balancer {}
unsafe impl Sync for Balancer {}

#[derive(Clone, Debug)]
struct BalancingMerger {
    halves: Vec<BalancingMerger>,
    layer: Vec<Balancer>,
    width: usize,
}

impl BalancingMerger {
    pub fn new(width: usize) -> BalancingMerger {
        let layer = (0..width / 2)
            .into_iter()
            .map(|_| Balancer::new())
            .collect::<Vec<Balancer>>();

        let halves = if width > 2 {
            vec![
                BalancingMerger::new(width / 2),
                BalancingMerger::new(width / 2),
            ]
        } else {
            vec![]
        };

        BalancingMerger {
            halves,
            layer,
            width,
        }
    }

    /// Traverses edges for the mergers
    pub fn traverse(&self, input: usize) -> usize {
        let output = if self.width > 2 {
            self.halves[input % 2].traverse(input / 2)
        } else {
            0
        };

        output + self.layer[output].traverse()
    }
}

unsafe impl Send for BalancingMerger {}
unsafe impl Sync for BalancingMerger {}

/// Balancing bitonic network
#[derive(Clone, Debug)]
pub struct BalancingBitonic {
    halves: Vec<BalancingBitonic>,
    merger: BalancingMerger,
    width: usize,
}

impl BalancingBitonic {
    pub fn new(width: usize) -> BalancingBitonic {
        assert_eq!(width % 2, 0, "Wires should be multiple of two.");
        let halves = if width > 2 {
            vec![
                BalancingBitonic::new(width / 2),
                BalancingBitonic::new(width / 2),
            ]
        } else {
            vec![]
        };

        BalancingBitonic {
            halves,
            merger: BalancingMerger::new(width),
            width,
        }
    }

    pub fn traverse(&self, input: usize) -> usize {
        let output = if self.width > 2 {
            self.halves[input % 2].traverse(input / 2)
        } else {
            0
        };

        output + self.merger.traverse(output)
    }
}

unsafe impl Send for BalancingBitonic {}
unsafe impl Sync for BalancingBitonic {}

/// Counting bitonic network
#[derive(Clone, Debug)]
pub struct CountingBitonic {
    /// Underlying balancing bitonic implementation
    balancing: BalancingBitonic,
    /// Represents current wire traversal counter value
    state: Arc<AtomicUsize>,
    /// Represents full wire trips
    trips: Arc<AtomicUsize>,
    /// Width of the bitonic network
    width: usize,
}

impl CountingBitonic {
    ///
    /// Create new counting bitonic network.
    pub fn new(width: usize) -> CountingBitonic {
        CountingBitonic {
            balancing: BalancingBitonic::new(width),
            state: Arc::new(AtomicUsize::default()),
            trips: Arc::new(AtomicUsize::default()),
            width,
        }
    }

    /// Traverse data through the counting bitonic.
    pub fn traverse(&self, input: usize) -> usize {
        let wire = self.balancing.traverse(input);
        let trips = self.trips.fetch_add(1, Ordering::AcqRel) + 1;
        let (q, r) = (trips / self.width, trips % self.width);
        if r > 0 {
            self.state.fetch_add(wire, Ordering::AcqRel)
        } else {
            wire.checked_sub(q).map_or_else(
                || self.state.fetch_add(wire, Ordering::AcqRel),
                |e| self.state.fetch_add(e, Ordering::AcqRel),
            )
        }
    }

    /// Get inner state for the counting bitonic
    pub fn get(&self) -> usize {
        self.state.load(Ordering::Acquire)
    }

    // TODO: min max here?
}

unsafe impl Send for CountingBitonic {}
unsafe impl Sync for CountingBitonic {}

impl Default for CountingBitonic {
    fn default() -> Self {
        CountingBitonic::new(8)
    }
}

#[cfg(test)]
mod test_bitonics {
    use super::*;

    #[test]
    fn test_balancing_bitonic_traversal() {
        let data: Vec<Vec<usize>> = vec![
            vec![9, 3, 1],
            vec![5, 4],
            vec![11, 23, 4, 10],
            vec![30, 40, 2],
        ];

        let bitonic = BalancingBitonic::new(4);
        let wires = data
            .iter()
            .flatten()
            .map(|d| bitonic.traverse(*d))
            .collect::<Vec<usize>>();

        assert_eq!(&*wires, [0, 2, 1, 3, 0, 1, 2, 3, 0, 2, 1, 3])

        // 0: 9, 4, 10,
        // 1: 1, 11, 40,
        // 2: 3, 23, 30,
        // 3: 5, 4, 2
    }

    #[test]
    fn test_counting_bitonic_traversal() {
        let data: Vec<Vec<usize>> = vec![
            vec![9, 3, 1],
            vec![5, 4],
            vec![11, 23, 4, 10],
            vec![30, 40, 2],
        ];

        let bitonic = CountingBitonic::new(4);
        let wires = data
            .iter()
            .flatten()
            .map(|d| bitonic.traverse(*d))
            .collect::<Vec<usize>>();

        assert_eq!(&*wires, [0, 0, 2, 3, 5, 5, 6, 8, 9, 9, 11, 12])
    }

    #[test]
    fn test_counting_bitonic_traversal_and_get() {
        let data: Vec<Vec<usize>> = vec![
            vec![9, 3, 1],
            vec![5, 4],
            vec![11, 23, 4, 10],
            vec![30, 40, 2],
        ];

        let bitonic = CountingBitonic::new(4);
        let wires = data
            .iter()
            .flatten()
            .map(|d| bitonic.traverse(*d))
            .collect::<Vec<usize>>();

        assert_eq!(&*wires, [0, 0, 2, 3, 5, 5, 6, 8, 9, 9, 11, 12]);
        assert_eq!(bitonic.get(), 12);
    }

    #[test]
    fn test_balancing_bitonic_mt_traversal() {
        (0..10_000).into_iter().for_each(|_| {
            let bitonic = Arc::new(BalancingBitonic::new(4));

            let data1: Vec<usize> = vec![9, 3, 1];
            let bitonic1 = bitonic.clone();
            let bdata1 = std::thread::spawn(move || {
                data1
                    .iter()
                    .map(|d| bitonic1.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let data2: Vec<usize> = vec![5, 4];
            let bitonic2 = bitonic.clone();
            let bdata2 = std::thread::spawn(move || {
                data2
                    .iter()
                    .map(|d| bitonic2.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let data3: Vec<usize> = vec![11, 23, 4, 10];
            let bitonic3 = bitonic.clone();
            let bdata3 = std::thread::spawn(move || {
                data3
                    .iter()
                    .map(|d| bitonic3.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let data4: Vec<usize> = vec![30, 40, 2];
            let bitonic4 = bitonic.clone();
            let bdata4 = std::thread::spawn(move || {
                data4
                    .iter()
                    .map(|d| bitonic4.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let (bdata1, bdata2, bdata3, bdata4) = (
                bdata1.join().unwrap(),
                bdata2.join().unwrap(),
                bdata3.join().unwrap(),
                bdata4.join().unwrap(),
            );
            let res: Vec<usize> = [bdata1, bdata2, bdata3, bdata4].concat();

            assert!(res.iter().count() == 12);
        });
    }

    #[test]
    fn test_counting_bitonic_mt_traversal() {
        (0..10_000).into_iter().for_each(|_| {
            let bitonic = Arc::new(CountingBitonic::new(4));

            let data1: Vec<usize> = vec![9, 3, 1];
            let bitonic1 = bitonic.clone();
            let bdata1 = std::thread::spawn(move || {
                data1
                    .iter()
                    .map(|d| bitonic1.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let data2: Vec<usize> = vec![5, 4];
            let bitonic2 = bitonic.clone();
            let bdata2 = std::thread::spawn(move || {
                data2
                    .iter()
                    .map(|d| bitonic2.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let data3: Vec<usize> = vec![11, 23, 4, 10];
            let bitonic3 = bitonic.clone();
            let bdata3 = std::thread::spawn(move || {
                data3
                    .iter()
                    .map(|d| bitonic3.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let data4: Vec<usize> = vec![30, 40, 2];
            let bitonic4 = bitonic.clone();
            let bdata4 = std::thread::spawn(move || {
                data4
                    .iter()
                    .map(|d| bitonic4.traverse(*d))
                    .collect::<Vec<usize>>()
            });

            let (bdata1, bdata2, bdata3, bdata4) = (
                bdata1.join().unwrap(),
                bdata2.join().unwrap(),
                bdata3.join().unwrap(),
                bdata4.join().unwrap(),
            );
            let res: Vec<usize> = [bdata1, bdata2, bdata3, bdata4].concat();

            assert!(res.iter().count() == 12);
            assert!(res.iter().find(|&e| *e >= 12 / 2).is_some())
        });
    }
}
