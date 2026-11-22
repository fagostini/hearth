//! The phone side of a pairing: one PC, reached through its ntfy topic.
//! Used by the parent app (through flutter_rust_bridge) and by `examples/phone.rs`.
//! State (the phone's private key, relay, topic, counters) lives in one file the caller
//! chooses, encrypted with a 32-byte key the caller keeps somewhere safer (the app keeps
//! it in Android Keystore / iOS Keychain); `None` = plain JSON (command-line tools).
//! Every read-modify-write holds a process-wide lock so a listener and a sender in the
//! same process never lose each other's counter updates.
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

/// Key that encrypts the state file.
pub type StateKey = [u8; 32];

fn decode(bytes: &[u8], key: Option<&StateKey>) -> Result<State> {
    use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
    if bytes.first() == Some(&b'{') {
        // Plain JSON: written by a command-line tool, or by an app version before
        // encryption (the next save encrypts it).
        return Ok(serde_json::from_slice(bytes)?);
    }
    let key = key.context("pairing file is encrypted and no key was given")?;
    anyhow::ensure!(bytes.len() > 24, "pairing file is damaged");
    let (nonce, data) = bytes.split_at(24);
    let plain = XChaCha20Poly1305::new(key.into())
        .decrypt(&XNonce::from(<[u8; 24]>::try_from(nonce)?), data)
        .map_err(|_| anyhow::anyhow!("pairing file can't be decrypted (wrong key)"))?;
    Ok(serde_json::from_slice(&plain)?)
}

fn encode(st: &State, key: Option<&StateKey>) -> Result<Vec<u8>> {
    use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
    let json = serde_json::to_vec_pretty(st)?;
    let Some(key) = key else { return Ok(json) };
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).expect("OS random number generator");
    let data = XChaCha20Poly1305::new(key.into())
        .encrypt(&XNonce::from(nonce), json.as_slice())
        .map_err(|_| anyhow::anyhow!("encrypt"))?;
    Ok([nonce.as_slice(), &data].concat())
}

fn load(path: &Path, key: Option<&StateKey>) -> Result<(State, Identity)> {
    let st = decode(&std::fs::read(path).context("not paired")?, key)?;
    let key: [u8; 32] = B64.decode(&st.key)?.try_into().map_err(|_| anyhow::anyhow!("bad key"))?;
    Ok((st, Identity::from_bytes(key)))
}

fn save(path: &Path, st: &State, key: Option<&StateKey>) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, encode(st, key)?)?;
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

pub fn is_paired(path: &Path, key: Option<&StateKey>) -> bool {
    load(path, key).is_ok()
}

/// Pair with the PC whose QR payload was scanned. Overwrites any earlier pairing.
pub fn pair(path: &Path, key: Option<&StateKey>, qr: &str, phone_name: &str) -> Result<()> {
    let offer = PairOffer::from_qr(qr.trim())?;
    let me = Identity::generate();
    let (req, pc) = PairRequest::new(&me, &offer, phone_name)?;
    let _g = LOCK.lock().unwrap();
    ureq::post(&format!("{}/{}", offer.relay, offer.topic))
        .send(serde_json::to_string(&req)?)
        .context("can't reach the relay")?;
    save(path, &State { key: B64.encode(me.to_bytes()), relay: offer.relay, topic: offer.topic, pc }, key)
}

/// Encrypt and send one message to the PC.
pub fn send(path: &Path, key: Option<&StateKey>, body: Body) -> Result<()> {
    let _g = LOCK.lock().unwrap();
    let (mut st, me) = load(path, key)?;
    let env = Channel::new(&me, &mut st.pc)?.seal(body, now())?;
    save(path, &st, key)?; // counter first: a lost send is fine, a reused counter is not
    let url = format!("{}/{}", st.relay, st.topic);
    drop(_g);
    ureq::post(&url).send(env).context("can't reach the relay")?;
    Ok(())
}

/// Receive messages from the PC until `on_message` returns false. Reconnects with
/// backoff and resumes from the last message id. `on_link` hears the connection
/// state: `None` when connected, `Some(reason)` when not. Messages that fail to open
/// (tampered, replayed) are dropped.
pub fn listen(path: &Path, key: Option<&StateKey>, mut on_message: impl FnMut(Body) -> bool, mut on_link: impl FnMut(Option<String>)) -> Result<()> {
    let (relay, topic) = {
        let (st, _) = load(path, key)?;
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
                on_link(None);
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
                        let (mut st, me) = load(path, key)?;
                        // The topic is shared by every phone paired with this PC: skip
                        // what isn't from the PC to us (our own sends, other phones' mail).
                        if env.from != st.pc.public || env.to != B64.encode(me.public()) {
                            continue;
                        }
                        let r = Channel::new(&me, &mut st.pc).and_then(|mut c| c.open(text));
                        save(path, &st, key)?;
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
            Err(e) => on_link(Some(format!("relay: {e}"))),
        }
        std::thread::sleep(Duration::from_secs(backoff));
        backoff = (backoff * 2).min(60);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_file_encryption() {
        let st = State { key: "k".into(), relay: "https://r".into(), topic: "t".into(), pc: Peer { public: "p".into(), name: "PC".into(), sent: 3, received: 4 } };
        let key = [7u8; 32];
        let sealed = encode(&st, Some(&key)).unwrap();
        assert!(!String::from_utf8_lossy(&sealed).contains("https://r"));
        assert_eq!(decode(&sealed, Some(&key)).unwrap().pc.sent, 3);
        assert!(decode(&sealed, Some(&[8u8; 32])).is_err());
        assert!(decode(&sealed, None).is_err());
        // An old plain file still opens, with or without a key.
        let plain = encode(&st, None).unwrap();
        assert_eq!(decode(&plain, Some(&key)).unwrap().topic, "t");
    }
}
