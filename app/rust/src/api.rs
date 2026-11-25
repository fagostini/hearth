//! What the Flutter app can call. Thin wrappers over hearth_core::client. Dart passes
//! the pairing file's path (app-private storage) and the 32-byte key that encrypts it,
//! which Dart keeps in the platform keystore (Android Keystore, iOS Keychain, libsecret).
use crate::frb_generated::StreamSink;
use hearth_core::{client, Body};

// Plain copies of hearth_core's types for Dart (flutter_rust_bridge generates classes
// for types defined here).
use std::path::Path;

/// One child's status as the PC reported it.
pub struct Kid {
    pub user: String,
    pub used_s: i64,
    /// -1 = no limit.
    pub left_s: i64,
    pub over: bool,
}

pub struct Rules {
    pub user: String,
    pub school_minutes: u32,
    pub weekend_minutes: u32,
    pub school_window: Option<String>,
    pub weekend_window: Option<String>,
    pub apps: Vec<AppRule>,
    pub web: Option<WebRule>,
}

pub struct AppRule {
    pub app: String,
    /// 0 = blocked.
    pub minutes: u32,
}

pub struct WebRule {
    pub safesearch: bool,
    pub block: Vec<String>,
}

pub struct DayUse {
    pub date: String,
    pub used_s: i64,
    pub extra_s: i64,
    pub limit_s: i64,
}

pub struct AppUse {
    pub app: String,
    pub seconds: i64,
}

pub struct ChildHistory {
    pub user: String,
    pub days: Vec<DayUse>,
    pub apps: Vec<AppUse>,
}

impl From<hearth_core::Rules> for Rules {
    fn from(r: hearth_core::Rules) -> Self {
        Self {
            user: r.user,
            school_minutes: r.school_minutes,
            weekend_minutes: r.weekend_minutes,
            school_window: r.school_window,
            weekend_window: r.weekend_window,
            apps: r.apps.into_iter().map(|a| AppRule { app: a.app, minutes: a.minutes }).collect(),
            web: r.web.map(|w| WebRule { safesearch: w.safesearch, block: w.block }),
        }
    }
}

impl From<Rules> for hearth_core::Rules {
    fn from(r: Rules) -> Self {
        Self {
            user: r.user,
            school_minutes: r.school_minutes,
            weekend_minutes: r.weekend_minutes,
            school_window: r.school_window,
            weekend_window: r.weekend_window,
            apps: r.apps.into_iter().map(|a| hearth_core::AppRule { app: a.app, minutes: a.minutes }).collect(),
            web: r.web.map(|w| hearth_core::WebRule { safesearch: w.safesearch, block: w.block }),
        }
    }
}

pub enum Msg {
    Status { children: Vec<Kid> },
    Request { id: String, user: String, minutes: u32, reason: String },
    History { children: Vec<ChildHistory> },
    Rules { children: Vec<Rules> },
    /// Something the PC wants the parent to read (e.g. why a change was refused).
    Notice { text: String },
    /// Connected to the relay again.
    Online,
    /// Can't reach the relay; retrying.
    Problem { text: String },
}

fn key(k: &[u8]) -> anyhow::Result<client::StateKey> {
    k.try_into().map_err(|_| anyhow::anyhow!("state key must be 32 bytes"))
}

#[flutter_rust_bridge::frb(sync)]
pub fn is_paired(state_path: String, state_key: Vec<u8>) -> bool {
    key(&state_key).is_ok_and(|k| client::is_paired(Path::new(&state_path), Some(&k)))
}

pub fn pair(state_path: String, state_key: Vec<u8>, qr: String, phone_name: String) -> anyhow::Result<()> {
    client::pair(Path::new(&state_path), Some(&key(&state_key)?), &qr, &phone_name)
}

pub fn request_status(state_path: String, state_key: Vec<u8>) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Some(&key(&state_key)?), Body::GetStatus)
}

pub fn request_history(state_path: String, state_key: Vec<u8>) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Some(&key(&state_key)?), Body::GetHistory)
}

pub fn request_rules(state_path: String, state_key: Vec<u8>) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Some(&key(&state_key)?), Body::GetRules)
}

pub fn set_rules(state_path: String, state_key: Vec<u8>, rules: Rules) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Some(&key(&state_key)?), Body::SetRules { rules: rules.into() })
}

pub fn answer(state_path: String, state_key: Vec<u8>, id: String, approve: bool, minutes: u32) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Some(&key(&state_key)?), Body::ExtensionReply { id, approve, minutes })
}

/// Stream of messages from the PC. Runs until the Dart side stops listening.
pub fn listen(state_path: String, state_key: Vec<u8>, sink: StreamSink<Msg>) -> anyhow::Result<()> {
    let sink = &sink;
    let k = key(&state_key)?;
    client::listen(
        Path::new(&state_path),
        Some(&k),
        |body| match to_msg(body) {
            Some(msg) => sink.add(msg).is_ok(), // false once Dart has cancelled the stream
            None => true,
        },
        |link| {
            let _ = sink.add(match link {
                None => Msg::Online,
                Some(text) => Msg::Problem { text },
            });
        },
    )
}

/// PC-to-phone messages the app shows; phone-to-PC ones (our own) give None.
fn to_msg(body: Body) -> Option<Msg> {
    Some(match body {
                Body::Status { children } => Msg::Status {
                    children: children
                        .into_iter()
                        .map(|c| Kid { over: c.state == "over", user: c.user, used_s: c.used_s, left_s: c.left_s })
                        .collect(),
                },
                Body::ExtensionRequest { id, user, minutes, reason } => Msg::Request { id, user, minutes, reason },
                Body::History { children } => Msg::History {
                    children: children
                        .into_iter()
                        .map(|c| ChildHistory {
                            user: c.user,
                            days: c.days.into_iter().map(|d| DayUse { date: d.date, used_s: d.used_s, extra_s: d.extra_s, limit_s: d.limit_s }).collect(),
                            apps: c.apps.into_iter().map(|a| AppUse { app: a.app, seconds: a.seconds }).collect(),
                        })
                        .collect(),
                },
                Body::Rules { children } => Msg::Rules { children: children.into_iter().map(Rules::from).collect() },
                Body::Notice { text } => Msg::Notice { text },
        _ => return None,
    })
}

pub fn set_push(state_path: String, state_key: Vec<u8>, endpoint: Option<String>) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Some(&key(&state_key)?), Body::SetPush { endpoint })
}

/// Decrypt a message that arrived by push while the app was closed.
pub fn open_push(state_path: String, state_key: Vec<u8>, payload: Vec<u8>) -> anyhow::Result<Option<Msg>> {
    let body = client::open_push(Path::new(&state_path), Some(&key(&state_key)?), &payload)?;
    Ok(to_msg(body))
}

/// What the PC sent since the app last heard from the relay (hourly background check).
pub fn poll(state_path: String, state_key: Vec<u8>) -> anyhow::Result<Vec<Msg>> {
    let bodies = client::poll(Path::new(&state_path), Some(&key(&state_key)?))?;
    Ok(bodies.into_iter().filter_map(to_msg).collect())
}
