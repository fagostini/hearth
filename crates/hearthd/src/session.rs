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
    #[zbus(property)]
    fn leader(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn scope(&self) -> zbus::Result<String>;
}

pub struct Info {
    pub id: String,
    pub uid: u32,
    pub active: bool,
    pub locked: bool,
    pub leader: u32,
    pub scope: String,
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
            zbus::Result::Ok(Some((s.active().await?, s.locked_hint().await?, s.leader().await?, s.scope().await?)))
        };
        if let Ok(Some((active, locked, leader, scope))) = read.await {
            out.push(Info { id, uid, active, locked, leader, scope });
        }
    }
    Ok(out)
}

/// Ask the session to end the way a normal logout would: SIGTERM every process in
/// its scope except the leader (the display manager's PAM helper). Killing the
/// leader too (what TerminateSession does) makes SDDM 0.20 treat the session as
/// crashed and leave a black screen instead of the login screen.
/// Returns false if the scope's process list can't be read (cgroup v1, gone).
pub fn end_gracefully(s: &Info) -> bool {
    let path = format!("/sys/fs/cgroup/user.slice/user-{}.slice/{}/cgroup.procs", s.uid, s.scope);
    let Ok(procs) = std::fs::read_to_string(path) else { return false };
    for pid in procs.lines().filter_map(|l| l.parse::<i32>().ok()) {
        if pid != s.leader as i32 {
            // SAFETY: plain syscall; a PID that already exited just returns ESRCH.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
    true
}
