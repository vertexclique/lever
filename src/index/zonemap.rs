use crate::table::lotable::LOTable;
use crate::stats::bitonics::CountingBitonic;
use std::borrow::Cow;

///
/// Represents single zone definition for the selectivity
#[derive(Default, Clone, Debug)]
pub struct Zone {
    pub min: usize,
    pub max: usize,
    pub selectivity: usize,
    pub stats: CountingBitonic
}

unsafe impl Send for Zone {}
unsafe impl Sync for Zone {}

impl Zone {
    /// Get selectivity hit count for the zone
    pub fn hits(&self) -> usize {
        self.stats.get()
    }
}

///
/// Represents a zone map for a table
pub struct ZoneMap {
    zones: LOTable<usize, Zone>
}

unsafe impl Send for ZoneMap {}
unsafe impl Sync for ZoneMap {}

impl ZoneMap {
    ///
    /// Create new zone map
    pub fn new() -> ZoneMap {
        Self {
            zones: LOTable::new()
        }
    }

    ///
    /// Update given zone id with given zone data
    pub fn update(&self, zone_id: usize, zone_data: Zone)
    {
        let _ = self.zones.insert(zone_id, zone_data);
    }

    ///
    /// Get selectivity for the given zone id
    pub fn selectivity(&self, zone_id: usize) -> usize {
        self.zones.replace_with(&zone_id, |z| {
            z.map_or(Some(Zone::default()), |z| {
                z.stats.traverse(zone_id);
                Some(z.to_owned())
            })
        }).map_or(0, |z| z.selectivity)
    }
}