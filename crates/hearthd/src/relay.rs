//! Talking to the parent's phone(s) through an ntfy relay. Everything except pairing
//! requests is end-to-end encrypted with hearth-core; the relay sees envelopes only.
use anyhow::{Context, Result};
use hearth_core::{Body, Channel, Identity, PairOffer, PairRequest, Peer};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Kept in /var/lib/hearth/relay.json (0600, root). Holds the PC's private key.
#[derive(Serialize, Deserialize)]
struct State {
    identity: String,
    topic: String,
    peers: Vec<Peer>,
    /// Phone public key -> UnifiedPush endpoint (wake-ups while the app is closed).
    #[serde(default)]
    push: std::collections::HashMap<String, String>,
}

pub struct Relay {
    url: String,
    path: PathBuf,
    identity: Identity,
    topic: String,
    peers: Vec<Peer>,
    push: std::collections::HashMap<String, String>,
    offer: Option<(PairOffer, Instant)>,
}

/// How long a QR code stays valid.
const OFFER_TTL: Duration = Duration::from_secs(10 * 60);

fn b64(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

impl Relay {
    pub fn open(url: &str, path: &Path) -> Result<Self> {
        let (identity, topic, peers, push) = match fs::read_to_string(path) {
            Ok(s) => {
                let st: State = serde_json::from_str(&s).context("relay.json")?;
                let key = {
                    use base64::Engine;
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&st.identity)?
                };
                (Identity::from_bytes(key.try_into().map_err(|_| anyhow::anyhow!("bad key"))?), st.topic, st.peers, st.push)
            }
            Err(_) => {
                let id = Identity::generate();
                let topic = PairOffer::new(&id, url, None).topic;
                (id, topic, Vec::new(), Default::default())
            }
        };
        let r = Self { url: url.trim_end_matches('/').into(), path: path.into(), identity, topic, peers, push, offer: None };
        r.save()?;
        Ok(r)
    }

    fn save(&self) -> Result<()> {
        let st = State { identity: b64(&self.identity.to_bytes()), topic: self.topic.clone(), peers: self.peers.clone(), push: self.push.clone() };
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&st)?)?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        fs::rename(tmp, &self.path)?; // atomic: a crash never leaves half a key file
        Ok(())
    }

    pub fn paired(&self) -> usize {
        self.peers.len()
    }

    /// New one-time pairing code (replaces any earlier one). Returns the QR payload.
    pub fn start_pairing(&mut self) -> String {
        let offer = PairOffer::new(&self.identity, &self.url, Some(self.topic.clone()));
        let qr = offer.to_qr();
        self.offer = Some((offer, Instant::now()));
        qr
    }

    /// Send a message to every paired phone.
    pub fn broadcast(&mut self, body: Body) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        for i in 0..self.peers.len() {
            let sealed = Channel::new(&self.identity, &mut self.peers[i]).and_then(|mut c| c.seal(body.clone(), now));
            match sealed {
                Ok(env) => {
                    // Requests also wake a closed app: the same sealed envelope goes to
                    // the phone's push endpoint. Nothing readable leaves the PC.
                    if matches!(body, Body::ExtensionRequest { .. }) {
                        if let Some(ep) = self.push.get(&self.peers[i].public) {
                            post(ep.clone(), env.clone());
                        }
                    }
                    publish(&self.url, &self.topic, env)
                }
                Err(e) => warn!(%e, "seal failed"),
            }
        }
        if let Err(e) = self.save() {
            warn!(%e, "relay state not saved");
        }
    }

    /// Handle one message from the relay. Returns decrypted bodies from paired phones.
    pub fn receive(&mut self, text: &str) -> Option<Body> {
        if let Ok(req) = serde_json::from_str::<PairRequest>(text) {
            let Some((offer, at)) = &self.offer else { return None };
            if at.elapsed() > OFFER_TTL {
                self.offer = None;
                return None;
            }
            match offer.verify(&req) {
                Ok(peer) => {
                    info!(phone = peer.name, "phone paired");
                    self.peers.retain(|p| p.public != peer.public);
                    self.peers.push(peer);
                    self.offer = None; // one-time
                    let _ = self.save();
                    return Some(Body::GetStatus); // greet the new phone with a status
                }
                Err(e) => warn!(%e, "pairing request rejected"),
            }
            return None;
        }
        let from = serde_json::from_str::<hearth_core::Envelope>(text).ok()?.from;
        let me = b64(&self.identity.public());
        if from == me {
            return None; // our own message echoed by the topic
        }
        let peer = self.peers.iter_mut().find(|p| p.public == from)?;
        let res = Channel::new(&self.identity, peer).and_then(|mut c| c.open(text));
        let _ = self.save();
        match res {
            Ok(Body::SetPush { endpoint }) => {
                match endpoint.filter(|e| push_endpoint_ok(e)) {
                    Some(e) => {
                        info!(phone = from.get(..8).unwrap_or(""), "push endpoint set");
                        self.push.insert(from, e);
                    }
                    None => {
                        self.push.remove(&from);
                    }
                }
                let _ = self.save();
                None
            }
            Ok(b) => Some(b),
            Err(e) => {
                warn!(%e, "message from phone rejected");
                None
            }
        }
    }

    /// Subscribe to the topic on a thread (ureq is blocking) and forward messages.
    /// Reconnects with backoff; resumes from the last seen message id so nothing sent
    /// while disconnected is lost (duplicates are rejected by sequence numbers).
    pub fn subscribe(&self, tx: mpsc::UnboundedSender<String>) {
        let url = format!("{}/{}/json", self.url, self.topic);
        std::thread::spawn(move || {
            #[derive(Deserialize)]
            struct Event {
                id: String,
                event: String,
                #[serde(default)]
                message: String,
            }
            let mut since: Option<String> = None;
            let mut backoff = 1;
            loop {
                let u = match &since { Some(id) => format!("{url}?since={id}"), None => url.clone() };
                match ureq::get(&u).call() {
                    Ok(resp) => {
                        backoff = 1;
                        for line in BufReader::new(resp.into_body().into_reader()).lines() {
                            let Ok(line) = line else { break };
                            if let Ok(ev) = serde_json::from_str::<Event>(&line) {
                                if ev.event == "message" {
                                    since = Some(ev.id);
                                    if tx.send(ev.message).is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => warn!(%e, "relay connection failed"),
                }
                std::thread::sleep(Duration::from_secs(backoff));
                backoff = (backoff * 2).min(60);
            }
        });
    }
}

/// A UnifiedPush endpoint is a plain http(s) URL chosen by the phone's distributor.
/// Only a paired (authenticated) phone can set one.
/// ponytail: no private-address filtering; a paired parent is trusted not to aim the
/// PC at its own LAN. Add it if phones ever come from less trusted places.
fn push_endpoint_ok(e: &str) -> bool {
    (e.starts_with("https://") || e.starts_with("http://")) && e.len() <= 2048 && !e.chars().any(char::is_whitespace)
}

fn post(url: String, body: String) {
    std::thread::spawn(move || {
        if let Err(e) = ureq::post(&url).send(body) {
            warn!(%e, "push wake-up failed");
        }
    });
}

fn publish(url: &str, topic: &str, body: String) {
    let u = format!("{url}/{topic}");
    std::thread::spawn(move || {
        if let Err(e) = ureq::post(&u).send(body) {
            warn!(%e, "relay publish failed");
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn push_endpoints() {
        assert!(super::push_endpoint_ok("https://ntfy.sh/upAbC123?up=1"));
        assert!(!super::push_endpoint_ok("file:///etc/shadow"));
        assert!(!super::push_endpoint_ok("https://x y"));
    }
}
