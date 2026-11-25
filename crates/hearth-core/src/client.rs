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
    /// Last relay message seen (listen or poll), so the hourly poll only fetches news.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    since: Option<String>,
    relay: String,
    topic: String,
    pc: Peer,
}

/// Key that encrypts the state file.
pub type StateKey = [u8; 32];

fn decode(bytes: &[u8], key: Option<&StateKey>) -> Result<State> {
    use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
    // Plain JSON: written by a command-line tool, or by an app version before
    // encryption (the next save encrypts it). Tried only after decryption fails: a
    // random nonce starts with '{' once in 256 saves.
    let plain_json = || -> Result<State> { Ok(serde_json::from_slice(bytes)?) };
    let Some(key) = key else { return plain_json() };
    let opened = (bytes.len() > 24).then(|| bytes.split_at(24)).and_then(|(nonce, data)| {
        XChaCha20Poly1305::new(key.into()).decrypt(&XNonce::from(<[u8; 24]>::try_from(nonce).ok()?), data).ok()
    });
    match opened {
        Some(plain) => Ok(serde_json::from_slice(&plain)?),
        None if bytes.first() == Some(&b'{') => plain_json(),
        None => anyhow::bail!("pairing file can't be decrypted (wrong key or damaged)"),
    }
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
    post(&format!("{}/{}", offer.relay, offer.topic), serde_json::to_string(&req)?)
        .context("can't reach the relay")?;
    save(path, &State { since: None, key: B64.encode(me.to_bytes()), relay: offer.relay, topic: offer.topic, pc }, key)
}

/// Post to the relay with a deadline: an unreachable relay must fail, not hang while
/// holding LOCK (which would block every later call in the app).
fn post(url: &str, body: String) -> Result<(), ureq::Error> {
    ureq::post(url).config().timeout_global(Some(Duration::from_secs(15))).build().send(body).map(|_| ())
}

/// Encrypt and send one message to the PC.
pub fn send(path: &Path, key: Option<&StateKey>, body: Body) -> Result<()> {
    let _g = LOCK.lock().unwrap();
    let (mut st, me) = load(path, key)?;
    let env = Channel::new(&me, &mut st.pc)?.seal(body, now())?;
    save(path, &st, key)?; // counter first: a lost send is fine, a reused counter is not
    let url = format!("{}/{}", st.relay, st.topic);
    drop(_g);
    post(&url, env).context("can't reach the relay")?;
    Ok(())
}

/// Open one envelope that arrived by push (the app was woken while closed). Same checks
/// as the live listener: only from the paired PC, addressed to us, newer than the last.
pub fn open_push(path: &Path, key: Option<&StateKey>, payload: &[u8]) -> Result<Body> {
    let text = std::str::from_utf8(payload).context("push payload isn't text")?;
    let env: Envelope = serde_json::from_str(text).context("push payload isn't a Hearth message")?;
    let _g = LOCK.lock().unwrap();
    let (mut st, me) = load(path, key)?;
    anyhow::ensure!(env.from == st.pc.public && env.to == B64.encode(me.public()), "not for this pairing");
    let body = Channel::new(&me, &mut st.pc)?.open(text)?;
    save(path, &st, key)?;
    Ok(body)
}

/// One line of the relay's JSON stream. Returns the message id and, if the message is
/// from the PC to us and opens (not tampered or replayed), its body. Records the id.
fn take(path: &Path, key: Option<&StateKey>, line: &str) -> Result<Option<(String, Option<Body>)>> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return Ok(None) };
    let (Some("message"), Some(id)) = (v["event"].as_str(), v["id"].as_str()) else { return Ok(None) };
    let text = v["message"].as_str().unwrap_or_default();
    let _g = LOCK.lock().unwrap();
    let (mut st, me) = load(path, key)?;
    st.since = Some(id.to_string());
    // The topic is shared by every phone paired with this PC: skip what isn't from the
    // PC to us (our own sends, other phones' mail).
    // ponytail: messages that fail to open are dropped silently; count them if it matters.
    let body = serde_json::from_str::<Envelope>(text)
        .ok()
        .filter(|env| env.from == st.pc.public && env.to == B64.encode(me.public()))
        .and_then(|_| Channel::new(&me, &mut st.pc).and_then(|mut c| c.open(text)).ok());
    save(path, &st, key)?;
    Ok(Some((id.to_string(), body)))
}

/// Fetch what the PC sent since the last listen or poll, without staying connected
/// (the hourly background check). The relay keeps messages for 12 hours.
pub fn poll(path: &Path, key: Option<&StateKey>) -> Result<Vec<Body>> {
    let (st, _) = load(path, key)?;
    let since = st.since.as_deref().unwrap_or("12h");
    let url = format!("{}/{}/json?poll=1&since={since}", st.relay, st.topic);
    let resp = ureq::get(&url).config().timeout_global(Some(Duration::from_secs(30))).build().call().context("can't reach the relay")?;
    let mut out = Vec::new();
    for line in std::io::BufReader::new(resp.into_body().into_reader()).lines() {
        if let Some((_, Some(body))) = take(path, key, &line?)? {
            out.push(body);
        }
    }
    Ok(out)
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
    let mut since = load(path, key)?.0.since;
    let mut backoff = 1;
    loop {
        let url = match &since {
            Some(id) => format!("{relay}/{topic}/json?since={id}"),
            None => format!("{relay}/{topic}/json"),
        };
        match ureq::get(&url).config().timeout_connect(Some(Duration::from_secs(10))).build().call() {
            Ok(resp) => {
                backoff = 1;
                on_link(None);
                for line in std::io::BufReader::new(resp.into_body().into_reader()).lines() {
                    let Ok(line) = line else { break };
                    let Some((id, body)) = take(path, key, &line)? else { continue };
                    since = Some(id);
                    if let Some(body) = body {
                        if !on_message(body) {
                            return Ok(());
                        }
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
        let st = State { since: None, key: "k".into(), relay: "https://r".into(), topic: "t".into(), pc: Peer { public: "p".into(), name: "PC".into(), sent: 3, received: 4, seen: 0 } };
        let key = [7u8; 32];
        let sealed = encode(&st, Some(&key)).unwrap();
        assert!(!String::from_utf8_lossy(&sealed).contains("https://r"));
        assert_eq!(decode(&sealed, Some(&key)).unwrap().pc.sent, 3);
        assert!(decode(&sealed, Some(&[8u8; 32])).is_err());
        assert!(decode(&sealed, None).is_err());
        // An old plain file still opens, with or without a key.
        let plain = encode(&st, None).unwrap();
        assert_eq!(decode(&plain, Some(&key)).unwrap().topic, "t");
        // The random nonce sometimes starts with '{'; that must not look like plain JSON.
        let brace = std::iter::repeat_with(|| encode(&st, Some(&key)).unwrap()).find(|b| b[0] == b'{').unwrap();
        assert_eq!(decode(&brace, Some(&key)).unwrap().pc.received, 4);
    }
}
