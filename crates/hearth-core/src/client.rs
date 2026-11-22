//! The phone side of a pairing: one PC, reached through its ntfy topic.
//! Used by the parent app (through flutter_rust_bridge) and by `examples/phone.rs`.
//! State (the phone's private key, relay, topic, counters) lives in one JSON file the
//! caller chooses; every read-modify-write holds a process-wide lock so a listener and
//! a sender in the same process never lose each other's counter updates.
use crate::{Body, Channel, Envelope, Identity, PairOffer, PairRequest, Peer};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::{io::BufRead, path::Path, sync::Mutex, time::Duration};

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct State {
    key: String,
    relay: String,
    topic: String,
    pc: Peer,
}

fn load(path: &Path) -> Result<(State, Identity)> {
    let st: State = serde_json::from_str(&std::fs::read_to_string(path).context("not paired")?)?;
    let key: [u8; 32] = B64.decode(&st.key)?.try_into().map_err(|_| anyhow::anyhow!("bad key"))?;
    Ok((st, Identity::from_bytes(key)))
}

fn save(path: &Path, st: &State) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(st)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn is_paired(path: &Path) -> bool {
    load(path).is_ok()
}

/// Pair with the PC whose QR payload was scanned. Overwrites any earlier pairing.
pub fn pair(path: &Path, qr: &str, phone_name: &str) -> Result<()> {
    let offer = PairOffer::from_qr(qr.trim())?;
    let me = Identity::generate();
    let (req, pc) = PairRequest::new(&me, &offer, phone_name)?;
    let _g = LOCK.lock().unwrap();
    ureq::post(&format!("{}/{}", offer.relay, offer.topic))
        .send(serde_json::to_string(&req)?)
        .context("can't reach the relay")?;
    save(path, &State { key: B64.encode(me.to_bytes()), relay: offer.relay, topic: offer.topic, pc })
}

/// Encrypt and send one message to the PC.
pub fn send(path: &Path, body: Body) -> Result<()> {
    let _g = LOCK.lock().unwrap();
    let (mut st, me) = load(path)?;
    let env = Channel::new(&me, &mut st.pc)?.seal(body, now())?;
    save(path, &st)?; // counter first: a lost send is fine, a reused counter is not
    let url = format!("{}/{}", st.relay, st.topic);
    drop(_g);
    ureq::post(&url).send(env).context("can't reach the relay")?;
    Ok(())
}

/// Receive messages from the PC until `on_message` returns false. Reconnects with
/// backoff and resumes from the last message id. `on_error` hears about connection
/// problems only; messages that fail to open (tampered, replayed) are dropped.
pub fn listen(path: &Path, mut on_message: impl FnMut(Body) -> bool, mut on_error: impl FnMut(String)) -> Result<()> {
    let (relay, topic) = {
        let (st, _) = load(path)?;
        (st.relay, st.topic)
    };
    let mut since: Option<String> = None;
    let mut backoff = 1;
    loop {
        let url = match &since {
            Some(id) => format!("{relay}/{topic}/json?since={id}"),
            None => format!("{relay}/{topic}/json"),
        };
        match ureq::get(&url).call() {
            Ok(resp) => {
                backoff = 1;
                for line in std::io::BufReader::new(resp.into_body().into_reader()).lines() {
                    let Ok(line) = line else { break };
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
                    if v["event"] != "message" {
                        continue;
                    }
                    since = v["id"].as_str().map(String::from);
                    let text = v["message"].as_str().unwrap_or_default();
                    let Ok(env) = serde_json::from_str::<Envelope>(text) else { continue };
                    let opened = {
                        let _g = LOCK.lock().unwrap();
                        let (mut st, me) = load(path)?;
                        // The topic is shared by every phone paired with this PC: skip
                        // what isn't from the PC to us (our own sends, other phones' mail).
                        if env.from != st.pc.public || env.to != B64.encode(me.public()) {
                            continue;
                        }
                        let r = Channel::new(&me, &mut st.pc).and_then(|mut c| c.open(text));
                        save(path, &st)?;
                        r
                    };
                    match opened {
                        Ok(body) => {
                            if !on_message(body) {
                                return Ok(());
                            }
                        }
                        Err(_) => {} // ponytail: dropped silently; count them if it ever matters
                    }
                }
            }
            Err(e) => on_error(format!("relay: {e}")),
        }
        std::thread::sleep(Duration::from_secs(backoff));
        backoff = (backoff * 2).min(60);
    }
}
