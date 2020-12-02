use crate::stats::bitonics::CountingBitonic;
use crate::table::lotable::LOTable;
use std::borrow::Cow;
use anyhow::*;
use std::sync::Arc;

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

///
/// Represents a zone map for a column
pub struct ZoneMap {
    /// Zone map built in
    zones: LOTable<usize, Zone>,
}

unsafe impl Send for ZoneMap {}
unsafe impl Sync for ZoneMap {}

impl ZoneMap {
    ///
    /// Create new zone map
    pub fn new() -> ZoneMap {
        Self {
            zones: LOTable::new(),
        }
    }

    /// Insert given zone data with given zone id into the zone map
    /// Returns old zone data if zone data exists
    pub fn insert(&self, zone_id: usize, zone_data: Zone) -> Result<Arc<Option<Zone>>> {
        self.zones.insert(zone_id, zone_data)
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

    /// Get zone selectivity hits for the given zone id
    pub fn zone_hits(&self, zone_id: usize) -> usize {
        self.zones.get(&zone_id).map_or(0, |z| z.hits())
    }
}
