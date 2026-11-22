//! Stand-in for the parent's phone app (same client code the app uses).
//! State lives in $HEARTH_PHONE (default ./phone.json).
//!
//!   phone pair '<QR payload>' [name]      pair with a PC (payload from `hearthd pair --text`)
//!   phone status                          ask the PC for a status
//!   phone listen [--approve N|--decline]  print messages; answer time requests
use hearth_core::{client, Body};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let path = PathBuf::from(std::env::var("HEARTH_PHONE").unwrap_or("phone.json".into()));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("pair") => {
            client::pair(&path, args.get(1).ok_or_else(|| anyhow::anyhow!("missing QR payload"))?, args.get(2).map_or("Test phone", |s| s))?;
            println!("pairing request sent");
        }
        Some("status") => {
            client::send(&path, Body::GetStatus)?;
            println!("status requested");
        }
        Some("listen") => {
            let approve = args.iter().position(|a| a == "--approve").and_then(|i| args.get(i + 1)?.parse::<u32>().ok());
            let decline = args.iter().any(|a| a == "--decline");
            client::listen(
                &path,
                |b| {
                    println!("{}", serde_json::to_string(&b).unwrap_or_default());
                    if let Body::ExtensionRequest { id, .. } = b {
                        let reply = match (approve, decline) {
                            (Some(m), _) => Some(Body::ExtensionReply { id, approve: true, minutes: m }),
                            (None, true) => Some(Body::ExtensionReply { id, approve: false, minutes: 0 }),
                            _ => None,
                        };
                        if let Some(r) = reply {
                            match client::send(&path, r.clone()) {
                                Ok(()) => println!("-> {}", serde_json::to_string(&r).unwrap_or_default()),
                                Err(e) => println!("reply failed: {e}"),
                            }
                        }
                    }
                    true
                },
                |e| println!("rejected: {e}"),
            )?;
        }
        _ => eprintln!("usage: phone pair <qr> [name] | status | listen [--approve N|--decline]"),
    }
    Ok(())
}
