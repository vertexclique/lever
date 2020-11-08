use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct Balancer {
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

#[derive(Debug)]
pub struct Merger {
    halves: Vec<Merger>,
    layer: Vec<Balancer>,
    width: usize,
}

impl Merger {
    pub fn new(width: usize) -> Merger {
        let layer = (0..width / 2)
            .into_iter()
            .map(|_| Balancer::new())
            .collect::<Vec<Balancer>>();

        let halves = if width > 2 {
            vec![Merger::new(width / 2), Merger::new(width / 2)]
        } else {
            vec![]
        };

        Merger {
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

#[derive(Debug)]
pub struct Bitonic {
    halves: Vec<Bitonic>,
    merger: Merger,
    width: usize,
}

impl Bitonic {
    pub fn new(width: usize) -> Bitonic {
        assert_eq!(width % 2, 0, "Wires should be multiple of two.");
        let halves = if width > 2 {
            vec![Bitonic::new(width / 2), Bitonic::new(width / 2)]
        } else {
            vec![]
        };

        Bitonic {
            halves,
            merger: Merger::new(width),
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

#[cfg(test)]
mod test_bitonics {
    use super::*;

    #[test]
    fn test_bitonic_traversal() {
        let data: Vec<Vec<usize>> = vec![
            vec![9, 3, 1],
            vec![5, 4],
            vec![11, 23, 4, 10],
            vec![30, 40, 2],
        ];

        let bitonic = Bitonic::new(4);
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
}
