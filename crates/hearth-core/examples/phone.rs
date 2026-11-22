//! Stand-in for the parent's phone app, for testing the relay before the Flutter app
//! exists. State (the phone's key and the paired PC) lives in $HEARTH_PHONE
//! (default ./phone.json).
//!
//!   phone pair '<QR payload>' [name]      pair with a PC (payload from `hearthd pair --text`)
//!   phone status                          ask the PC for a status
//!   phone listen [--approve N|--decline]  print messages; answer time requests
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use hearth_core::{Body, Channel, Identity, PairOffer, PairRequest, Peer};
use serde::{Deserialize, Serialize};
use std::io::BufRead;

#[derive(Serialize, Deserialize)]
struct State {
    key: String,
    relay: String,
    topic: String,
    pc: Peer,
}

fn path() -> String {
    std::env::var("HEARTH_PHONE").unwrap_or("phone.json".into())
}
fn load() -> anyhow::Result<(State, Identity)> {
    let st: State = serde_json::from_str(&std::fs::read_to_string(path())?)?;
    let key: [u8; 32] = B64.decode(&st.key)?.try_into().map_err(|_| anyhow::anyhow!("bad key"))?;
    Ok((st, Identity::from_bytes(key)))
}
fn save(st: &State) -> anyhow::Result<()> {
    Ok(std::fs::write(path(), serde_json::to_string_pretty(st)?)?)
}
fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}
fn send(st: &mut State, me: &Identity, body: Body) -> anyhow::Result<()> {
    let env = Channel::new(me, &mut st.pc)?.seal(body, now())?;
    save(st)?;
    ureq::post(&format!("{}/{}", st.relay, st.topic)).send(env)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("pair") => {
            let offer = PairOffer::from_qr(args.get(1).ok_or_else(|| anyhow::anyhow!("missing QR payload"))?)?;
            let me = Identity::generate();
            let (req, pc) = PairRequest::new(&me, &offer, args.get(2).map_or("Test phone", |s| s))?;
            save(&State { key: B64.encode(me.to_bytes()), relay: offer.relay.clone(), topic: offer.topic.clone(), pc })?;
            ureq::post(&format!("{}/{}", offer.relay, offer.topic)).send(serde_json::to_string(&req)?)?;
            println!("pairing request sent");
        }
        Some("status") => {
            let (mut st, me) = load()?;
            send(&mut st, &me, Body::GetStatus)?;
            println!("status requested");
        }
        Some("listen") => {
            let approve = args.iter().position(|a| a == "--approve").and_then(|i| args.get(i + 1)?.parse::<u32>().ok());
            let decline = args.iter().any(|a| a == "--decline");
            let (st, _) = load()?;
            let resp = ureq::get(&format!("{}/{}/json", st.relay, st.topic)).call()?;
            for line in std::io::BufReader::new(resp.into_body().into_reader()).lines() {
                let v: serde_json::Value = serde_json::from_str(&line?)?;
                if v["event"] != "message" {
                    continue;
                }
                let text = v["message"].as_str().unwrap_or_default();
                let (mut st, me) = load()?;
                let Ok(env) = serde_json::from_str::<hearth_core::Envelope>(text) else { continue };
                if env.from != st.pc.public {
                    continue; // our own message, or another phone's
                }
                let body = Channel::new(&me, &mut st.pc).and_then(|mut c| c.open(text));
                save(&st)?;
                match body {
                    Ok(b) => {
                        println!("{}", serde_json::to_string(&b)?);
                        if let Body::ExtensionRequest { id, .. } = b {
                            if let Some(m) = approve {
                                send(&mut st, &me, Body::ExtensionReply { id, approve: true, minutes: m })?;
                                println!("-> approved {m} min");
                            } else if decline {
                                send(&mut st, &me, Body::ExtensionReply { id, approve: false, minutes: 0 })?;
                                println!("-> declined");
                            }
                        }
                    }
                    Err(e) => println!("rejected: {e}"),
                }
            }
        }
        _ => eprintln!("usage: phone pair <qr> [name] | status | listen [--approve N|--decline]"),
    }
    Ok(())
}
