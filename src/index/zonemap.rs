use crate::stats::bitonics::CountingBitonic;
use crate::table::lotable::LOTable;
use anyhow::*;
use slice_group_by::GroupBy;
use std::borrow::{Borrow, Cow};
use std::ops::{Deref, Range, RangeBounds};
use std::sync::Arc;
use std::any::Any;
use itertools::Itertools;

///
/// Represents single zone definition for the selectivity
#[derive(Default, Clone, Debug)]
pub struct Zone {
    pub min: usize,
    pub max: usize,
    pub selectivity: usize,
    pub stats: CountingBitonic,
}

unsafe impl Send for Zone {}
unsafe impl Sync for Zone {}

impl Zone {
    /// Get selectivity hit count for the zone
    pub fn hits(&self) -> usize {
        self.stats.get()
    }

    ///
    /// Return zone triple of (min, max, selectivity)
    pub fn zone_triple(&self) -> (usize, usize, usize) {
        (self.min, self.max, self.selectivity)
    }
}

impl From<(usize, usize)> for Zone {
    fn from(r: (usize, usize)) -> Self {
        Zone {
            min: r.0,
            max: r.1,
            ..Self::default()
        }
    }
}

impl From<(usize, usize, usize)> for Zone {
    fn from(r: (usize, usize, usize)) -> Self {
        Zone {
            min: r.0,
            max: r.1,
            selectivity: r.2,
            ..Self::default()
        }
    }
}

///
/// Represents a zone data for a column
#[derive(Debug, Clone)]
pub struct ColumnZoneData {
    /// Zone map built in
    zones: LOTable<usize, Zone>,
}

unsafe impl Send for ColumnZoneData {}
unsafe impl Sync for ColumnZoneData {}

impl ColumnZoneData {
    ///
    /// Create new column zone data
    pub fn new() -> ColumnZoneData {
        Self {
            zones: LOTable::new(),
        }
    }

    ///
    /// Insert given zone data with given zone id into the column zone data
    /// Returns old zone data if zone data exists
    pub fn insert(&self, zone_id: usize, zone_data: Zone) -> Result<Arc<Option<Zone>>> {
        self.zones.insert(zone_id, zone_data)
    }

    ///
    /// Inserts given zone dataset into this column zone data
    pub fn batch_insert(&self, zones: Vec<(usize, Zone)>) {
        zones.iter().for_each(|(zid, zdata)| {
            let _ = self.zones.insert(*zid, zdata.clone());
        })
    }

    ///
    /// Update given zone id with given selectivity data
    pub fn update(&self, zone_id: usize, min: usize, max: usize, selectivity: usize) {
        let zone = Zone {
            min,
            max,
            selectivity,
            ..Default::default()
        };
        let _ = self.zones.insert(zone_id, zone);
    }

    ///
    /// Update given zone id with given zone data
    pub fn update_zone(&self, zone_id: usize, zone_data: Zone) {
        let _ = self.zones.insert(zone_id, zone_data);
    }

    ///
    /// Get selectivity for the given zone id
    pub fn selectivity(&self, zone_id: usize) -> usize {
        self.zones
            .replace_with(&zone_id, |z| {
                z.map_or(Some(Zone::default()), |z| {
                    z.stats.traverse(zone_id);
                    Some(z.to_owned())
                })
            })
            .map_or(0, |z| z.selectivity)
    }

    ///
    /// Returns selectivity in question, queried by the range
    pub fn selectivity_range<R>(&self, range_min: R, range_max: R, data: &[R]) -> usize
    where
        R: PartialOrd + std::fmt::Debug
    {
        self
            .zones
            .values()
            .into_iter()
            .filter(|z| {
                let (zl, zr, _) = z.zone_triple();
                (&data[zl]..=&data[zr]).contains(&&range_min) ||
                (&data[zl]..=&data[zr]).contains(&&range_max)
            })
            .map(|z| z.selectivity)
            .sum()
    }

    ///
    /// Returns scan range in question, queried by the constraint range
    pub fn scan_range<R>(&self, range_min: R, range_max: R, data: &[R]) -> (usize, usize)
    where
        R: PartialOrd
    {
        self
            .zones
            .values()
            .into_iter()
            .filter(|z| {
                let (zl, zr, _) = z.zone_triple();
                (&data[zl]..=&data[zr]).contains(&&range_min) ||
                    (&data[zl]..=&data[zr]).contains(&&range_max)
            })
            .fold((usize::MAX, 0_usize), |mut acc, e| {
                acc.0 = acc.0.min(e.min);
                acc.1 = acc.1.max(e.max);
                acc
            })
    }

    /// Get zone selectivity hits for the given zone id
    pub fn zone_hits(&self, zone_id: usize) -> usize {
        self.zones.get(&zone_id).map_or(0, |z| z.hits())
    }
}

///
/// Represents a zone map for a table
#[derive(Debug, Clone)]
pub struct ZoneMap {
    col_zones: LOTable<String, ColumnZoneData>,
}

impl ZoneMap {
    ///
    /// Create new zone map
    pub fn new() -> ZoneMap {
        Self {
            col_zones: LOTable::new(),
        }
    }

    ///
    /// Insert given column zone data with given zone id into the column zone map
    /// Returns old column zone data if column zone data exists
    pub fn insert<T>(
        &self,
        column: T,
        zone_data: ColumnZoneData,
    ) -> Result<Arc<Option<ColumnZoneData>>>
    where
        T: Into<String>,
    {
        self.col_zones.insert(column.into(), zone_data)
    }

    ///
    /// Returns selectivity in question, queried by the range
    pub fn selectivity_range<C, R>(&self, column: C, range_min: R, range_max: R, data: &[R]) -> usize
    where
        C: Into<String>,
        R: PartialOrd + std::fmt::Debug
    {
        self.col_zones.get(&column.into()).map_or(0_usize, |c| c.selectivity_range(range_min, range_max, data))
    }

    ///
    /// Returns scan range in question, queried by the constraint range
    pub fn scan_range<C, R>(&self, column: C, range_min: R, range_max: R, data: &[R]) -> (usize, usize)
        where
            C: Into<String>,
            R: PartialOrd + std::fmt::Debug
    {
        self.col_zones.get(&column.into()).map_or((0, 0), |c| c.scan_range(range_min, range_max, data))
    }
}

impl<'a, T, R> From<Vec<(T, &'a [R])>> for ZoneMap
where
    T: Into<String>,
    R: PartialOrd,
{
    fn from(data: Vec<(T, &'a [R])>) -> Self {
        let zm = ZoneMap::new();
        data.into_iter().for_each(|(col, d)| {
            let mut row_id = 0_usize;
            let czm = ColumnZoneData::new();
            d.linear_group_by(|l, r| l < r).for_each(|d| {
                let r = d.len();
                let offset = row_id;
                let z = Zone::from((row_id, row_id + r - 1, r));
                row_id += r;
                let _ = czm.insert(offset, z);
            });

            let _ = zm.insert(col.into(), czm);
        });
        zm
    }
}


#[cfg(test)]
mod tests_zone_map {
    use super::*;

    #[test]
    fn test_zone_selectivity() {
        let customers: Vec<i32> = vec![
            vec![1, 0, -1, -2].repeat(2),
            vec![1, 2, 3, 4].repeat(3)
        ].concat();
        let products = vec![4, 3, 2, 1].repeat(100);
        let payouts = vec![4, 2, 6, 7].repeat(100);

        let ingestion_data = vec![
            ("customers", customers.as_slice()),
            ("products", products.as_slice()),
            ("payouts", payouts.as_slice()),
        ];

        let zone_map = ZoneMap::from(ingestion_data);

        // Selectivity range is: [-2, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]
        assert_eq!(zone_map.selectivity_range("customers", 4, 4, &*customers), 13);
    }

    #[test]
    fn test_zone_scan_range() {
        let customers: Vec<i32> = vec![
            vec![1, 0, -1, -2].repeat(2),
            vec![1, 2, 3, 4].repeat(3)
        ].concat();
        let products = vec![4, 3, 2, 1].repeat(100);
        let payouts = vec![4, 2, 6, 7].repeat(100);

        let ingestion_data = vec![
            ("customers", customers.as_slice()),
            ("products", products.as_slice()),
            ("payouts", payouts.as_slice()),
        ];

        let zone_map = ZoneMap::from(ingestion_data);

        // Selectivity range is: [-2, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]
        // Scan range is: [7, 19]
        assert_eq!(zone_map.scan_range("customers", 4, 4, &*customers), (7, 19));
    }
}