//! Child sessions from logind (org.freedesktop.login1).
use zbus::{proxy, Connection, OwnedObjectPath};

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait Manager {
    fn list_sessions(&self) -> zbus::Result<Vec<(String, u32, String, String, OwnedObjectPath)>>;
    fn lock_session(&self, session_id: &str) -> zbus::Result<()>;
    fn terminate_session(&self, session_id: &str) -> zbus::Result<()>;
}

#[proxy(interface = "org.freedesktop.login1.Session", default_service = "org.freedesktop.login1")]
pub trait Session {
    #[zbus(property)]
    fn active(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn locked_hint(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn class(&self) -> zbus::Result<String>;
}

pub struct Info {
    pub id: String,
    pub uid: u32,
    pub active: bool,
    pub locked: bool,
}

/// User-class sessions of the given UIDs. Sessions that vanish mid-query are skipped.
pub async fn list(conn: &Connection, mgr: &ManagerProxy<'_>, uids: &[u32]) -> zbus::Result<Vec<Info>> {
    let mut out = Vec::new();
    for (id, uid, _user, _seat, path) in mgr.list_sessions().await? {
        if !uids.contains(&uid) {
            continue;
        }
        let read = async {
            let s = SessionProxy::builder(conn)
                .path(path)
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await?;
            if s.class().await? != "user" {
                return Ok(None);
            }
            zbus::Result::Ok(Some((s.active().await?, s.locked_hint().await?)))
        };
        if let Ok(Some((active, locked))) = read.await {
            out.push(Info { id, uid, active, locked });
        }
    }
    Ok(out)
}
