//! Storing where each host is served from, and aggregating it for the map.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};

use crate::error::Result;
use crate::geo::HostLocation;
use crate::monitor::host_key;

use super::{now, Vault};

impl Vault {
    /// Record (or refresh) where a host is served from.
    pub fn set_host_location(&self, location: &HostLocation) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO host_locations
                (host, ip, country, city, latitude, longitude, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(host) DO UPDATE SET
                ip = excluded.ip,
                country = excluded.country,
                city = excluded.city,
                latitude = excluded.latitude,
                longitude = excluded.longitude,
                resolved_at = excluded.resolved_at",
            params![
                location.host,
                location.ip,
                location.country,
                location.city,
                location.latitude,
                location.longitude,
                location.resolved_at,
            ],
        )?;
        Ok(())
    }

    /// When a host was last located, if ever. Drives the staleness check so
    /// a sweep can skip a DNS lookup it already did this month.
    pub fn host_location_age(&self, host: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT resolved_at FROM host_locations WHERE host = ?1",
                params![host],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Every located host that can be drawn, with how many saves it holds,
    /// busiest first.
    ///
    /// The save counting happens in Rust rather than as a SQL join, so that
    /// [`host_key`] stays the single definition of what a host *is*. An
    /// earlier version reimplemented the rule in SQL and quietly mangled any
    /// domain beginning with `w` — `LTRIM(host, 'wW.')` turns `wikipedia.org`
    /// into `ikipedia.org`, because LTRIM strips a character *set*, not a
    /// prefix. Grouping a few thousand URLs in a HashMap costs nothing and
    /// cannot drift.
    pub fn map_points(&self) -> Result<Vec<HostLocation>> {
        let mut counts: HashMap<String, i64> = HashMap::new();
        {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT url FROM saves")?;
            let urls = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for url in urls {
                *counts.entry(host_key(&url?)).or_insert(0) += 1;
            }
        }

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT host, ip, country, city, latitude, longitude, resolved_at
             FROM host_locations
             WHERE latitude IS NOT NULL AND longitude IS NOT NULL",
        )?;
        let mut points = stmt
            .query_map([], |r| {
                let host: String = r.get(0)?;
                Ok(HostLocation {
                    save_count: 0,
                    host,
                    ip: r.get(1)?,
                    country: r.get(2)?,
                    city: r.get(3)?,
                    latitude: r.get(4)?,
                    longitude: r.get(5)?,
                    resolved_at: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for point in &mut points {
            point.save_count = counts.get(&point.host).copied().unwrap_or(0);
        }
        points.sort_by(|a, b| b.save_count.cmp(&a.save_count).then(a.host.cmp(&b.host)));
        Ok(points)
    }

    /// Hosts per country, most first. Works with a country-level database
    /// too, where there are no coordinates to plot.
    pub fn hosts_by_country(&self) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT country, COUNT(*) FROM host_locations
             WHERE country != '' GROUP BY country ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// How many hosts have been located at all. Lets the UI tell "no database
    /// yet" apart from "nothing resolved yet".
    pub fn located_host_count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM host_locations", [], |r| r.get(0))?)
    }

    /// Forget every stored location — for when the user swaps a country
    /// database for a city one and wants coordinates filled in.
    pub fn clear_host_locations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM host_locations", [])?;
        Ok(())
    }

    /// Convenience for hosts resolved outside a sweep.
    pub fn touch_host_location(&self, location: &HostLocation) -> Result<()> {
        let mut fresh = location.clone();
        fresh.resolved_at = now();
        self.set_host_location(&fresh)
    }
}
