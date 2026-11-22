//! hearth-core: what the PC (hearthd) and the parent's phone app share.
//! - Message types (`Body`).
//! - End-to-end encryption between two paired devices (`Channel`): X25519 static keys,
//!   HKDF-SHA256, XChaCha20-Poly1305 with random nonces, per-sender sequence numbers
//!   against replay. The relay (ntfy) only ever sees `Envelope`s.
//! - Pairing (`PairOffer` / `PairRequest`): the PC shows a QR code with its public key
//!   and a one-time secret; the phone proves it saw the QR code with an HMAC over both
//!   public keys. Someone who only watches or writes to the relay can't pair.
#[cfg(feature = "client")]
pub mod client;

use anyhow::{bail, ensure, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

pub type Key = [u8; 32];

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("OS random number generator");
    b
}

fn b64(b: &[u8]) -> String {
    B64.encode(b)
}

fn unb64<const N: usize>(s: &str) -> Result<[u8; N]> {
    B64.decode(s)?.try_into().map_err(|_| anyhow::anyhow!("expected {N} bytes"))
}

/// A device's long-term key pair. Store `to_bytes()` privately (root-only file on the
/// PC, the platform keystore on the phone).
pub struct Identity(StaticSecret);

impl Identity {
    pub fn generate() -> Self {
        Self(StaticSecret::from(random::<32>()))
    }
    pub fn from_bytes(b: Key) -> Self {
        Self(StaticSecret::from(b))
    }
    pub fn to_bytes(&self) -> Key {
        self.0.to_bytes()
    }
    pub fn public(&self) -> Key {
        PublicKey::from(&self.0).to_bytes()
    }
}

// ---------- messages ----------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChildStatus {
    pub user: String,
    /// Seconds used today and left today (-1 = no limit).
    pub used_s: i64,
    pub left_s: i64,
    /// "ok" or "over".
    pub state: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    /// PC -> phone: current state of every child.
    Status { children: Vec<ChildStatus> },
    /// PC -> phone: a child asks for more time.
    ExtensionRequest { id: String, user: String, minutes: u32, reason: String },
    /// Phone -> PC: the parent's answer. `minutes` may differ from what was asked.
    ExtensionReply { id: String, approve: bool, minutes: u32 },
    /// Phone -> PC: send a fresh Status.
    GetStatus,
}

/// What travels over the relay. Only `from` and `to` are readable (public keys).
#[derive(Serialize, Deserialize, Debug)]
pub struct Envelope {
    pub v: u8,
    pub from: String,
    pub to: String,
    pub nonce: String,
    pub data: String,
}

#[derive(Serialize, Deserialize)]
struct Sealed {
    seq: u64,
    /// Sender's clock, seconds since the Unix epoch (informational; ordering uses `seq`).
    ts: u64,
    body: Body,
}

// ---------- channel ----------

/// Persistent state of one pairing. Save it after every `seal`/`open` (the counters
/// must survive restarts, or replay protection resets).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Peer {
    pub public: String,
    pub name: String,
    pub sent: u64,
    pub received: u64,
}

pub struct Channel<'a> {
    me: &'a Identity,
    peer: &'a mut Peer,
    cipher: XChaCha20Poly1305,
}

impl<'a> Channel<'a> {
    pub fn new(me: &'a Identity, peer: &'a mut Peer) -> Result<Self> {
        let their: Key = unb64(&peer.public)?;
        let shared = me.0.diffie_hellman(&PublicKey::from(their));
        ensure!(shared.was_contributory(), "invalid peer key");
        // Both sides derive the same key: sort the public keys for the HKDF info.
        let (a, b) = { let m = me.public(); if m < their { (m, their) } else { (their, m) } };
        let mut key = [0u8; 32];
        hkdf::Hkdf::<Sha256>::new(Some(b"hearth v1"), shared.as_bytes())
            .expand(&[a.as_slice(), b.as_slice()].concat(), &mut key)
            .map_err(|_| anyhow::anyhow!("hkdf"))?;
        Ok(Self { me, peer, cipher: XChaCha20Poly1305::new(&key.into()) })
    }

    /// AAD binds sender and recipient, so a message can't be reflected back.
    fn aad(from: &str, to: &str) -> Vec<u8> {
        format!("hearth v1|{from}|{to}").into_bytes()
    }

    pub fn seal(&mut self, body: Body, now: u64) -> Result<String> {
        self.peer.sent += 1;
        let plain = serde_json::to_vec(&Sealed { seq: self.peer.sent, ts: now, body })?;
        let (from, to) = (b64(&self.me.public()), self.peer.public.clone());
        let nonce = random::<24>();
        let data = self
            .cipher
            .encrypt(&XNonce::from(nonce), Payload { msg: &plain, aad: &Self::aad(&from, &to) })
            .map_err(|_| anyhow::anyhow!("encrypt"))?;
        Ok(serde_json::to_string(&Envelope { v: 1, from, to, nonce: b64(&nonce), data: b64(&data) })?)
    }

    /// Decrypt an envelope from this peer. Rejects anything not from the peer, not to
    /// us, tampered with, or not newer than the last accepted message.
    pub fn open(&mut self, envelope: &str) -> Result<Body> {
        let e: Envelope = serde_json::from_str(envelope).context("not an envelope")?;
        ensure!(e.v == 1, "unknown version {}", e.v);
        ensure!(e.from == self.peer.public && e.to == b64(&self.me.public()), "not for this pairing");
        let nonce: [u8; 24] = unb64(&e.nonce)?;
        let plain = self
            .cipher
            .decrypt(&XNonce::from(nonce), Payload { msg: &B64.decode(&e.data)?, aad: &Self::aad(&e.from, &e.to) })
            .map_err(|_| anyhow::anyhow!("authentication failed"))?;
        let s: Sealed = serde_json::from_slice(&plain)?;
        if s.seq <= self.peer.received {
            bail!("replayed or old message (seq {} <= {})", s.seq, self.peer.received);
        }
        self.peer.received = s.seq;
        Ok(s.body)
    }
}

// ---------- pairing ----------

/// Shown by the PC as a QR code (`to_qr`). Valid until the PC discards it (it should
/// expire it after a few minutes and after the first successful pairing).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PairOffer {
    pub v: u8,
    /// Relay base URL, e.g. "https://ntfy.sh".
    pub relay: String,
    /// Random, unguessable topic shared by this PC and its paired phones.
    pub topic: String,
    pub pc: String,
    pub secret: String,
}

/// Sent by the phone over the relay, in the clear (it contains only public keys).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PairRequest {
    pub kind: String, // "pair"
    pub phone: String,
    pub name: String,
    pub mac: String,
}

fn pair_mac(secret: &Key, phone: &str, pc: &str, name: &str) -> Hmac<Sha256> {
    let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(secret).expect("any key length");
    m.update(format!("hearth pair v1|{phone}|{pc}|{name}").as_bytes());
    m
}

impl PairOffer {
    pub fn new(pc: &Identity, relay: &str, topic: Option<String>) -> Self {
        Self {
            v: 1,
            relay: relay.trim_end_matches('/').into(),
            topic: topic.unwrap_or_else(|| format!("hearth-{}", b64(&random::<18>()))),
            pc: b64(&pc.public()),
            secret: b64(&random::<32>()),
        }
    }

    pub fn to_qr(&self) -> String {
        serde_json::to_string(self).expect("serializable")
    }

    pub fn from_qr(s: &str) -> Result<Self> {
        let o: Self = serde_json::from_str(s).context("not a Hearth pairing code")?;
        ensure!(o.v == 1, "unsupported pairing code version");
        unb64::<32>(&o.pc)?;
        unb64::<32>(&o.secret)?;
        Ok(o)
    }

    /// PC side: check a phone's request. On success returns the new peer to store.
    pub fn verify(&self, req: &PairRequest) -> Result<Peer> {
        ensure!(req.kind == "pair", "not a pairing request");
        unb64::<32>(&req.phone)?;
        let mac = pair_mac(&unb64(&self.secret)?, &req.phone, &self.pc, &req.name);
        mac.verify_slice(&B64.decode(&req.mac)?).map_err(|_| anyhow::anyhow!("wrong pairing code"))?;
        Ok(Peer { public: req.phone.clone(), name: req.name.clone(), sent: 0, received: 0 })
    }
}

impl PairRequest {
    /// Phone side: answer a scanned offer. The phone stores the returned peer (the PC).
    pub fn new(phone: &Identity, offer: &PairOffer, name: &str) -> Result<(Self, Peer)> {
        let pk = b64(&phone.public());
        let mac = pair_mac(&unb64(&offer.secret)?, &pk, &offer.pc, name).finalize().into_bytes();
        let req = Self { kind: "pair".into(), phone: pk, name: name.into(), mac: b64(&mac) };
        Ok((req, Peer { public: offer.pc.clone(), name: "PC".into(), sent: 0, received: 0 }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paired() -> (Identity, Identity, Peer, Peer) {
        let (pc, phone) = (Identity::generate(), Identity::generate());
        let offer = PairOffer::from_qr(&PairOffer::new(&pc, "https://ntfy.sh/", None).to_qr()).unwrap();
        assert_eq!(offer.relay, "https://ntfy.sh");
        let (req, phone_side) = PairRequest::new(&phone, &offer, "Mum's phone").unwrap();
        let pc_side = offer.verify(&req).unwrap();
        (pc, phone, pc_side, phone_side)
    }

    #[test]
    fn roundtrip_both_ways() {
        let (pc, phone, mut pc_peer, mut phone_peer) = paired();
        let req = Body::ExtensionRequest { id: "r1".into(), user: "alice".into(), minutes: 15, reason: "homework".into() };
        let wire = Channel::new(&pc, &mut pc_peer).unwrap().seal(req.clone(), 1).unwrap();
        assert!(!wire.contains("homework")); // relay can't read it
        assert_eq!(Channel::new(&phone, &mut phone_peer).unwrap().open(&wire).unwrap(), req);

        let reply = Body::ExtensionReply { id: "r1".into(), approve: true, minutes: 10 };
        let wire = Channel::new(&phone, &mut phone_peer).unwrap().seal(reply.clone(), 2).unwrap();
        assert_eq!(Channel::new(&pc, &mut pc_peer).unwrap().open(&wire).unwrap(), reply);
    }

    #[test]
    fn rejects_replay_tamper_reflection_strangers() {
        let (pc, phone, mut pc_peer, mut phone_peer) = paired();
        let wire = Channel::new(&pc, &mut pc_peer).unwrap().seal(Body::GetStatus, 1).unwrap();
        let mut rx = Channel::new(&phone, &mut phone_peer).unwrap();
        rx.open(&wire).unwrap();
        assert!(rx.open(&wire).is_err(), "replay accepted");

        let wire2 = Channel::new(&pc, &mut pc_peer).unwrap().seal(Body::GetStatus, 2).unwrap();
        let mut e: Envelope = serde_json::from_str(&wire2).unwrap();
        let mut d = B64.decode(&e.data).unwrap();
        d[0] ^= 1;
        e.data = b64(&d);
        assert!(Channel::new(&phone, &mut phone_peer).unwrap().open(&serde_json::to_string(&e).unwrap()).is_err());

        // The PC's own message bounced back to it.
        let wire3 = Channel::new(&pc, &mut pc_peer).unwrap().seal(Body::GetStatus, 3).unwrap();
        assert!(Channel::new(&pc, &mut pc_peer).unwrap().open(&wire3).is_err());

        // A stranger who knows the PC's public key (it's on the relay) can't write.
        let mut fake_peer = Peer { public: pc_peer.public.clone(), ..phone_peer.clone() };
        let stranger = Identity::generate();
        let forged = Channel::new(&stranger, &mut fake_peer).unwrap().seal(Body::GetStatus, 9).unwrap();
        assert!(Channel::new(&pc, &mut pc_peer).unwrap().open(&forged).is_err());
    }

    #[test]
    fn pairing_needs_the_qr_secret() {
        let (pc, phone) = (Identity::generate(), Identity::generate());
        let offer = PairOffer::new(&pc, "https://ntfy.sh", None);
        let mut guess = offer.clone();
        guess.secret = b64(&random::<32>()); // saw the topic and the PC key, not the QR code
        let (req, _) = PairRequest::new(&phone, &guess, "evil").unwrap();
        assert!(offer.verify(&req).is_err());
        let (mut req, _) = PairRequest::new(&phone, &offer, "Dad").unwrap();
        req.name = "Mum".into(); // name is covered by the MAC
        assert!(offer.verify(&req).is_err());
        assert!(PairOffer::from_qr("https://example.com").is_err());
    }

    #[test]
    fn identity_persists() {
        let a = Identity::generate();
        assert_eq!(Identity::from_bytes(a.to_bytes()).public(), a.public());
    }
}
