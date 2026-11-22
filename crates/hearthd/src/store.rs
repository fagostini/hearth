//! Daily usage per child in SQLite (root-owned, 0600).
use rusqlite::{params, Connection, OptionalExtension};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

pub struct Store(Connection);

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let db = Connection::open(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS usage (
                day TEXT NOT NULL, uid INTEGER NOT NULL, seconds INTEGER NOT NULL,
                PRIMARY KEY (day, uid));
             CREATE TABLE IF NOT EXISTS app_usage (
                day TEXT NOT NULL, uid INTEGER NOT NULL, app TEXT NOT NULL, seconds INTEGER NOT NULL,
                PRIMARY KEY (day, uid, app));
             CREATE TABLE IF NOT EXISTS bonus (
                day TEXT NOT NULL, uid INTEGER NOT NULL, seconds INTEGER NOT NULL,
                PRIMARY KEY (day, uid));
             CREATE TABLE IF NOT EXISTS rules (user TEXT PRIMARY KEY, json TEXT NOT NULL)",
        )?;
        Ok(Self(db))
    }

    pub fn load(&self, day: &str, uid: u32) -> anyhow::Result<u64> {
        let s = self
            .0
            .query_row("SELECT seconds FROM usage WHERE day = ?1 AND uid = ?2", params![day, uid], |r| r.get::<_, i64>(0))
            .optional()?;
        Ok(s.unwrap_or(0) as u64)
    }

    pub fn save(&self, day: &str, uid: u32, seconds: u64) -> anyhow::Result<()> {
        self.0.execute(
            "INSERT INTO usage (day, uid, seconds) VALUES (?1, ?2, ?3)
             ON CONFLICT (day, uid) DO UPDATE SET seconds = excluded.seconds",
            params![day, uid, seconds as i64],
        )?;
        Ok(())
    }
}

impl Store {
    pub fn load_apps(&self, day: &str, uid: u32) -> anyhow::Result<std::collections::HashMap<String, u64>> {
        let mut q = self.0.prepare("SELECT app, seconds FROM app_usage WHERE day = ?1 AND uid = ?2")?;
        let rows = q.query_map(params![day, uid], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn load_rules(&self, user: &str) -> anyhow::Result<Option<hearth_core::Rules>> {
        let j: Option<String> = self.0.query_row("SELECT json FROM rules WHERE user = ?1", params![user], |r| r.get(0)).optional()?;
        Ok(j.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    pub fn save_rules(&self, r: &hearth_core::Rules) -> anyhow::Result<()> {
        self.0.execute(
            "INSERT INTO rules (user, json) VALUES (?1, ?2) ON CONFLICT (user) DO UPDATE SET json = excluded.json",
            params![r.user, serde_json::to_string(r)?],
        )?;
        Ok(())
    }

    /// Seconds used and approved extra per day since `from` ("YYYY-MM-DD", inclusive).
    pub fn days_since(&self, uid: u32, from: &str) -> anyhow::Result<Vec<(String, i64, i64)>> {
        let mut q = self.0.prepare(
            "SELECT u.day, u.seconds, COALESCE(b.seconds, 0) FROM usage u
             LEFT JOIN bonus b ON b.day = u.day AND b.uid = u.uid
             WHERE u.uid = ?1 AND u.day >= ?2 ORDER BY u.day",
        )?;
        let rows = q.query_map(params![uid, from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Seconds per app since `from`, most used first.
    pub fn apps_since(&self, uid: u32, from: &str) -> anyhow::Result<Vec<(String, i64)>> {
        let mut q = self.0.prepare(
            "SELECT app, SUM(seconds) s FROM app_usage WHERE uid = ?1 AND day >= ?2 GROUP BY app ORDER BY s DESC",
        )?;
        let rows = q.query_map(params![uid, from], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Extra time approved by a parent for that day.
    pub fn load_bonus(&self, day: &str, uid: u32) -> anyhow::Result<u64> {
        let s = self
            .0
            .query_row("SELECT seconds FROM bonus WHERE day = ?1 AND uid = ?2", params![day, uid], |r| r.get::<_, i64>(0))
            .optional()?;
        Ok(s.unwrap_or(0) as u64)
    }

    pub fn save_bonus(&self, day: &str, uid: u32, seconds: u64) -> anyhow::Result<()> {
        self.0.execute(
            "INSERT INTO bonus (day, uid, seconds) VALUES (?1, ?2, ?3)
             ON CONFLICT (day, uid) DO UPDATE SET seconds = excluded.seconds",
            params![day, uid, seconds as i64],
        )?;
        Ok(())
    }

    pub fn save_app(&self, day: &str, uid: u32, app: &str, seconds: u64) -> anyhow::Result<()> {
        self.0.execute(
            "INSERT INTO app_usage (day, uid, app, seconds) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (day, uid, app) DO UPDATE SET seconds = excluded.seconds",
            params![day, uid, app, seconds as i64],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("hearth-test-{}", std::process::id()));
        let s = super::Store::open(&dir.join("h.db")).unwrap();
        assert_eq!(s.load("2026-11-21", 1001).unwrap(), 0);
        s.save("2026-11-21", 1001, 60).unwrap();
        s.save("2026-11-21", 1001, 90).unwrap();
        assert_eq!(s.load("2026-11-21", 1001).unwrap(), 90);
        assert_eq!(s.load("2026-11-22", 1001).unwrap(), 0);
        s.save_app("2026-11-21", 1001, "org.mozilla.firefox", 30).unwrap();
        s.save_app("2026-11-21", 1001, "org.mozilla.firefox", 45).unwrap();
        assert_eq!(s.load_apps("2026-11-21", 1001).unwrap()["org.mozilla.firefox"], 45);
        s.save_bonus("2026-11-21", 1001, 600).unwrap();
        s.save("2026-11-20", 1001, 30).unwrap();
        assert_eq!(s.days_since(1001, "2026-11-20").unwrap(), vec![("2026-11-20".into(), 30, 0), ("2026-11-21".into(), 90, 600)]);
        assert_eq!(s.apps_since(1001, "2026-11-15").unwrap(), vec![("org.mozilla.firefox".into(), 45)]);
        let r = hearth_core::Rules { user: "mia".into(), school_minutes: 60, ..Default::default() };
        assert!(s.load_rules("mia").unwrap().is_none());
        s.save_rules(&r).unwrap();
        assert_eq!(s.load_rules("mia").unwrap(), Some(r));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
