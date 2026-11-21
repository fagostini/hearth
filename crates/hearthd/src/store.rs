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
                PRIMARY KEY (day, uid))",
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
        std::fs::remove_dir_all(dir).unwrap();
    }
}
