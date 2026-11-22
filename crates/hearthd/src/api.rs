//! org.hearth.Daemon1 on the system bus. Read-only for now: each caller only sees
//! its own status, identified by the bus (not by anything the caller sends).
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use zbus::{fdo, interface, message::Header, Connection};

/// uid -> (seconds left today, state). State: "ok" or "over" (lock/logout running).
pub type Shared = Arc<Mutex<HashMap<u32, (i64, String)>>>;

pub struct Api(pub Shared);

#[interface(name = "org.hearth.Daemon1")]
impl Api {
    /// The caller's own (seconds left today, state). (-1, "unlimited") if no rule applies.
    async fn status(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<(i64, String)> {
        let sender = hdr.sender().ok_or_else(|| fdo::Error::Failed("no sender".into()))?;
        let uid = fdo::DBusProxy::new(conn).await?.get_connection_unix_user(sender.clone().into()).await?;
        Ok(self.0.lock().unwrap().get(&uid).cloned().unwrap_or((-1, "unlimited".into())))
    }
}
