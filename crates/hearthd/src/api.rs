//! org.hearth.Daemon1 on the system bus. The caller is always identified by the bus
//! (its Unix UID), never by anything it sends.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};
use zbus::{fdo, interface, message::Header, Connection};

/// Per child: seconds left today, state ("ok"/"over"), and the latest answer to a
/// time request as "<n>|<text>" (n increases with each answer; "" = none yet).
pub type Shared = Arc<Mutex<HashMap<u32, (i64, String, String)>>>;

/// Requests the main loop answers (it owns the children and the relay).
pub enum Event {
    Pair(oneshot::Sender<Result<String, String>>),
    Ask { uid: u32, minutes: u32, reason: String, reply: oneshot::Sender<String> },
}

pub struct Api {
    pub shared: Shared,
    pub tx: mpsc::UnboundedSender<Event>,
}

async fn caller_uid(hdr: &Header<'_>, conn: &Connection) -> fdo::Result<u32> {
    let sender = hdr.sender().ok_or_else(|| fdo::Error::Failed("no sender".into()))?;
    fdo::DBusProxy::new(conn).await?.get_connection_unix_user(sender.clone().into()).await
}

#[interface(name = "org.hearth.Daemon1")]
impl Api {
    /// The caller's own (seconds left today, state, last answer).
    /// (-1, "unlimited", "") if no rule applies.
    async fn status(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<(i64, String, String)> {
        let uid = caller_uid(&hdr, conn).await?;
        Ok(self.shared.lock().unwrap().get(&uid).cloned().unwrap_or((-1, "unlimited".into(), String::new())))
    }

    /// Child asks their parent for more time. Returns a sentence to show the child.
    async fn request_extension(
        &self,
        minutes: u32,
        reason: String,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        let uid = caller_uid(&hdr, conn).await?;
        let reason: String = reason.chars().take(200).collect();
        let (reply, rx) = oneshot::channel();
        self.tx.send(Event::Ask { uid, minutes: minutes.clamp(5, 120), reason, reply }).map_err(|_| fdo::Error::Failed("daemon busy".into()))?;
        rx.await.map_err(|_| fdo::Error::Failed("no answer".into()))
    }

    /// New one-time pairing code for a parent's phone (QR payload). Root only.
    /// ponytail: root check; polkit (admin password from the desktop app) comes with M6.
    async fn start_pairing(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &Connection,
    ) -> fdo::Result<String> {
        if caller_uid(&hdr, conn).await? != 0 {
            return Err(fdo::Error::AccessDenied("only root can pair a phone".into()));
        }
        let (reply, rx) = oneshot::channel();
        self.tx.send(Event::Pair(reply)).map_err(|_| fdo::Error::Failed("daemon busy".into()))?;
        rx.await.map_err(|_| fdo::Error::Failed("no answer".into()))?.map_err(fdo::Error::Failed)
    }
}
