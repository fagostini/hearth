//! What the Flutter app can call. Thin wrappers over hearth_core::client; Dart passes
//! the path of the pairing file (inside the app's private storage).
//! ponytail: the phone's private key sits in that app-private file; move it to the
//! Android Keystore / iOS Keychain before release.
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

pub fn request_history(state_path: String) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Body::GetHistory)
}

pub fn request_rules(state_path: String) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Body::GetRules)
}

pub fn set_rules(state_path: String, rules: Rules) -> anyhow::Result<()> {
    client::send(Path::new(&state_path), Body::SetRules { rules: rules.into() })
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
