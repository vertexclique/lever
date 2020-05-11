use super::transact::TransactionIsolation;
use crate::txn::readset::ReadSet;
use crate::txn::writeset::WriteSet;
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::txn) enum CompareSet {
    ReadLocal,
    WriteLocal,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord)]
pub(in crate::txn) struct Compare {
    rev: u64,
    current: bool,
    set: CompareSet,
}

impl Compare {
    pub(in crate::txn) fn new(rev: u64, current: bool, set: CompareSet) -> Self {
        Self { rev, current, set }
    }

    pub(in crate::txn) fn check(&self, other: &Compare, ordering: Ordering) -> bool {
        self.cmp(&other) == ordering
    }
}

impl PartialOrd for Compare {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.rev.cmp(&other.rev))
    }
}

pub(crate) struct ConflictManager;

impl ConflictManager {
    pub(crate) fn check<T: 'static + Clone + Sync + Send>(iso: &TransactionIsolation) -> bool {
        match iso {
            // Serializable is also a checking for write conflicts, seprated from serializable reads only mode.
            // Serializable will do check for write conflicts too. Even that would never happen.
            TransactionIsolation::Serializable => {
                let rs = ReadSet::local();
                let ws = WriteSet::local();

                let mut linear = rs.cmps::<T>();
                let _writes_before_rev: Vec<Compare>;

                let pinned_rev = rs.first::<T>().checked_add(1).unwrap_or(u64::MAX);
                let writes_before_rev = ws.writes_before::<T>(pinned_rev);
                linear.extend(writes_before_rev);

                // dbg!(&linear);

                linear.iter().all(|x| x.current)
            }
            TransactionIsolation::RepeatableRead => {
                let rs = ReadSet::local();
                let cmps = rs.cmps::<T>();
                cmps.iter().all(|x| x.current)
            }
            TransactionIsolation::ReadCommitted => true,
        }
    }
}
