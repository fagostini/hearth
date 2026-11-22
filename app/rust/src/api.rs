//! What the Flutter app can call. Thin wrappers over hearth_core::client; Dart passes
//! the path of the pairing file (inside the app's private storage).
//! ponytail: the phone's private key sits in that app-private file; move it to the
//! Android Keystore / iOS Keychain before release.
use crate::frb_generated::StreamSink;
use hearth_core::{client, Body};
use std::path::Path;

/// One child's status as the PC reported it.
pub struct Kid {
    pub user: String,
    pub used_s: i64,
    /// -1 = no limit.
    pub left_s: i64,
    pub over: bool,
}

pub enum Msg {
    Status { children: Vec<Kid> },
    Request { id: String, user: String, minutes: u32, reason: String },
    /// Connected to the relay again.
    Online,
    /// Can't reach the relay; retrying.
    Problem { text: String },
}

#[flutter_rust_bridge::frb(sync)]
pub fn is_paired(state_path: String) -> bool {
    client::is_paired(Path::new(&state_path))
}

pub fn pair(state_path: String, qr: String, phone_name: String) -> anyhow::Result<()> {
    client::pair(Path::new(&state_path), &qr, &phone_name)
}

pub fn request_status(state_path: String) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Body::GetStatus)
}

pub fn answer(state_path: String, id: String, approve: bool, minutes: u32) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Body::ExtensionReply { id, approve, minutes })
}

/// Stream of messages from the PC. Runs until the Dart side stops listening.
pub fn listen(state_path: String, sink: StreamSink<Msg>) -> anyhow::Result<()> {
    let sink = &sink;
    client::listen(
        Path::new(&state_path),
        |body| {
            let msg = match body {
                Body::Status { children } => Msg::Status {
                    children: children
                        .into_iter()
                        .map(|c| Kid { over: c.state == "over", user: c.user, used_s: c.used_s, left_s: c.left_s })
                        .collect(),
                },
                Body::ExtensionRequest { id, user, minutes, reason } => Msg::Request { id, user, minutes, reason },
                _ => return true, // phone-to-PC messages: ignore
            };
            sink.add(msg).is_ok() // false once Dart has cancelled the stream
        },
        |link| {
            let _ = sink.add(match link {
                None => Msg::Online,
                Some(text) => Msg::Problem { text },
            });
        },
    )
}
