mod accounting;
mod api;
mod apps;
mod activity;
mod config;
mod dns;
mod relay;
mod session;
mod setup;
mod store;

use accounting::{action, capped, counts, outside, remaining, Action};
use chrono::Datelike;
use anyhow::Context;
use std::{collections::HashMap, env, fs, path::PathBuf, time::{Duration, Instant}};
use tracing::{info, warn};

const TICK: Duration = Duration::from_secs(15);
const CHECKPOINT: Duration = Duration::from_secs(30);
/// After a graceful logout request, how long before TerminateSession.
const FORCE_AFTER: Duration = Duration::from_secs(10);

struct Child {
    user: String,
    uid: u32,
    rules: hearth_core::Rules,
    days: [config::Day; 7],
    used: Duration,
    /// Daily app limits (app ID -> limit) and today's use per app.
    app_limits: HashMap<String, Duration>,
    app_used: HashMap<String, Duration>,
    over_since: Option<Instant>,
    /// Extra time a parent approved today, and the id of an unanswered request.
    bonus: Duration,
    /// Unanswered request: (id, minutes, reason). Resent when a phone asks for a
    /// status, so a phone that restarted (or paired later) still sees it.
    pending: Option<(String, u32, String)>,
    /// Latest answer shown to the child: (counter, text).
    answer: (u32, String),
}

impl Child {
    /// Switch to new rules (from the database or a phone). Checks them first; on error
    /// nothing changes.
    fn set_rules(&mut self, rules: hearth_core::Rules) -> anyhow::Result<()> {
        self.days = config::days_from(&rules)?;
        self.app_limits = rules.apps.iter().map(|a| (a.app.clone(), Duration::from_secs(a.minutes as u64 * 60))).collect();
        self.rules = rules;
        Ok(())
    }
}

/// Hand the web rules to the resolver (hearth-dns runs unprivileged and can't read the
/// database) and point the firewall at the children who have them.
async fn apply_web(children: &[Child], path: &std::path::Path) {
    let web: Vec<dns::WebPolicy> = children
        .iter()
        .filter_map(|c| {
            let w = c.rules.web.as_ref()?;
            Some(dns::WebPolicy { user: c.user.clone(), uid: c.uid, safesearch: w.safesearch, block: w.block.clone() })
        })
        .collect();
    if let Err(e) = dns::write_policies(path, &web) {
        warn!(%e, "web rules not written: filtering keeps its old rules");
    }
    dns::apply_nft(&web.iter().map(|w| w.uid).collect::<Vec<_>>()).await;
}

/// The last 7 days per child, for the phone's week view.
fn history_body(children: &[Child], store: &store::Store) -> anyhow::Result<hearth_core::Body> {
    let today = chrono::Local::now().date_naive();
    let from = (today - chrono::Days::new(6)).to_string();
    let mut out = Vec::new();
    for c in children {
        let rows = store.days_since(c.uid, &from)?;
        let mut days = Vec::new();
        for i in (0..7).rev() {
            let date = today - chrono::Days::new(i);
            let key = date.to_string();
            let (used, extra) = rows.iter().find(|r| r.0 == key).map_or((0, 0), |r| (r.1, r.2));
            // ponytail: limit from today's rules; store past limits if rules change often.
            let limit = c.days[date.weekday().num_days_from_monday() as usize].minutes as i64 * 60;
            days.push(hearth_core::DayUse { date: key, used_s: used, extra_s: extra, limit_s: limit });
        }
        let apps = store.apps_since(c.uid, &from)?.into_iter().map(|(app, seconds)| hearth_core::AppUse { app, seconds }).collect();
        out.push(hearth_core::ChildHistory { user: c.user.clone(), days, apps });
    }
    Ok(hearth_core::Body::History { children: out })
}

/// Today's status of every child, for the parent's phone.
fn status_body(children: &[Child], shared: &api::Shared) -> hearth_core::Body {
    let map = shared.lock().unwrap();
    hearth_core::Body::Status {
        children: children
            .iter()
            .map(|c| {
                let (left, state, _) = map.get(&c.uid).cloned().unwrap_or((-1, "ok".into(), String::new()));
                hearth_core::ChildStatus { user: c.user.clone(), used_s: c.used.as_secs() as i64, left_s: left, state }
            })
            .collect(),
    }
}

fn today() -> String {
    chrono::Local::now().date_naive().to_string()
}

/// Any sudo rights at all, including grants in /etc/sudoers.d that no group shows
/// (cloud-init, hand-edited sudoers). Needs root; sudo missing = no rights.
fn has_sudo(user: &str) -> bool {
    std::process::Command::new("sudo")
        .args(["-n", "-l", "-U", user])
        .env("LC_ALL", "C")
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("may run the following"))
}

/// One-time pairing code from the running daemon, shown as a QR code (and as text).
async fn show_pairing_code(text: bool) -> anyhow::Result<()> {
    let conn = zbus::Connection::system().await?;
    let reply = conn
        .call_method(Some("org.hearth.Daemon1"), "/org/hearth/Daemon1", Some("org.hearth.Daemon1"), "StartPairing", &())
        .await?;
    let payload: String = reply.body().deserialize()?;
    let qr = qrcode::QrCode::new(payload.as_bytes())?;
    println!("{}", qr.render::<qrcode::render::unicode::Dense1x2>().quiet_zone(true).build());
    println!("Scan with the Hearth app on your phone (Pair, on the first screen). The code works once, for 10 minutes.");
    println!("No camera? Run: sudo hearthd pair --text, and type the code in the app.");
    if text {
        println!("{payload}");
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().without_time().init(); // journald adds timestamps
    match env::args().nth(1).as_deref() {
        // `sudo hearthd pair`: ask the running daemon for a one-time code, show it as QR.
        Some("pair") => return show_pairing_code(env::args().any(|a| a == "--text")).await,
        Some("setup") => return setup::run().await,
        _ => {}
    }
    let config_path = env::var("HEARTH_CONFIG").unwrap_or("/etc/hearth/hearthd.toml".into());
    let db_path = PathBuf::from(env::var("HEARTH_DB").unwrap_or("/var/lib/hearth/hearth.db".into()));

    let cfg: config::Config = toml::from_str(&fs::read_to_string(&config_path).with_context(|| config_path.clone())?)?;
    let passwd = fs::read_to_string("/etc/passwd")?;
    if env::args().nth(1).as_deref() == Some("dns") {
        // Unprivileged resolver service: reads only the web rules hearthd hands over.
        return dns::run(PathBuf::from(env::var("HEARTH_WEB").unwrap_or("/run/hearth/web.json".into()))).await;
    }
    let grace = Duration::from_secs(cfg.grace_seconds);
    let group = fs::read_to_string("/etc/group")?;

    let store = store::Store::open(&db_path)?;
    let mut day = today();

    let cfg_relay = cfg.relay.clone();
    let mut children = Vec::new();
    for r in cfg.child {
        let Some(uid) = config::uid_of(&passwd, &r.user) else {
            warn!(user = r.user, "no such user, rule ignored");
            continue;
        };
        if uid == 0 || config::is_admin(&group, &r.user) || has_sudo(&r.user) {
            warn!(user = r.user, "admin account, rule ignored");
            continue;
        }
        // Rules: from the database once imported; the TOML only seeds the first run.
        let rules = match store.load_rules(&r.user)? {
            Some(saved) => saved,
            None => {
                let imported = r.to_rules().with_context(|| format!("rule for {}", r.user))?;
                store.save_rules(&imported)?;
                info!(user = r.user, "rules imported from the config file");
                imported
            }
        };
        let used = Duration::from_secs(store.load(&day, uid)?);
        info!(user = r.user, uid, school_min = rules.school_minutes, weekend_min = rules.weekend_minutes, used_s = used.as_secs(), "child");
        let app_used = store.load_apps(&day, uid)?.into_iter().map(|(a, s)| (a, Duration::from_secs(s))).collect();
        let bonus = Duration::from_secs(store.load_bonus(&day, uid)?);
        let mut c = Child {
            user: r.user,
            uid,
            days: Default::default(),
            rules: Default::default(),
            used,
            app_limits: HashMap::new(),
            app_used,
            over_since: None,
            bonus,
            pending: None,
            answer: (0, String::new()),
        };
        c.set_rules(rules).with_context(|| format!("saved rules for {}", c.user))?;
        children.push(c);
    }
    let uids: Vec<u32> = children.iter().map(|c| c.uid).collect();
    let web_path = PathBuf::from(env::var("HEARTH_WEB").unwrap_or("/run/hearth/web.json".into()));
    apply_web(&children, &web_path).await;

    let activity = activity::Activity::default();
    tokio::spawn(activity.clone().watch());

    let state_dir = db_path.parent().map(PathBuf::from).unwrap_or_default();
    let mut relay = match &cfg_relay {
        Some(url) => match relay::Relay::open(url, &state_dir.join("relay.json")) {
            Ok(r) => {
                info!(url, phones = r.paired(), "relay on");
                Some(r)
            }
            Err(e) => {
                warn!(%e, "relay off");
                None
            }
        },
        None => None,
    };
    let (relay_tx, mut relay_rx) = tokio::sync::mpsc::unbounded_channel();
    if let Some(r) = &relay {
        r.subscribe(relay_tx);
    }

    let status = api::Shared::default();
    let (api_tx, mut api_rx) = tokio::sync::mpsc::unbounded_channel();
    let conn = zbus::connection::Builder::system()
        .serve_at("/org/hearth/Daemon1", api::Api { shared: status.clone(), tx: api_tx })
        .name("org.hearth.Daemon1")
        .build()
        .await?;
    let mgr = session::ManagerProxy::new(&conn).await?;

    let mut ending: HashMap<String, Instant> = HashMap::new(); // session id -> SIGTERM sent
    let mut closing: HashMap<std::path::PathBuf, Instant> = HashMap::new(); // app cgroup -> SIGTERM sent
    let mut ticker = tokio::time::interval(TICK);
    let (mut last, mut last_wall, mut last_save) = (Instant::now(), chrono::Local::now(), Instant::now());
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            Some(ev) = api_rx.recv() => {
                match ev {
                    api::Event::Pair(reply) => {
                        let _ = reply.send(relay.as_mut().map(|r| r.start_pairing()).ok_or_else(|| "no relay configured (set relay = \"https://…\" in hearthd.toml)".to_string()));
                    }
                    api::Event::Ask { uid, minutes, reason, reply } => {
                        let msg = match (children.iter_mut().find(|c| c.uid == uid), relay.as_mut()) {
                            (None, _) => "No time limits apply to you.".to_string(),
                            (_, None) => "Your parent hasn't set up their phone yet.".into(),
                            (Some(_), Some(r)) if r.paired() == 0 => "Your parent hasn't set up their phone yet.".into(),
                            (Some(c), _) if c.pending.is_some() => "You already asked. Waiting for an answer.".into(),
                            (Some(c), Some(r)) => {
                                let id = format!("{}-{}", c.uid, chrono::Utc::now().timestamp());
                                info!(user = c.user, minutes, "asking parent for more time");
                                c.pending = Some((id.clone(), minutes, reason.clone()));
                                r.broadcast(hearth_core::Body::ExtensionRequest { id, user: c.user.clone(), minutes, reason });
                                "Asked. You'll get a notification when your parent answers.".into()
                            }
                        };
                        let _ = reply.send(msg);
                    }
                }
                continue;
            }
            Some(text) = relay_rx.recv() => {
                let Some(r) = relay.as_mut() else { continue };
                let Some((from, body)) = r.receive(&text) else { continue };
                match body {
                    hearth_core::Body::GetRules => {
                    r.send(&from, hearth_core::Body::Rules { children: children.iter().map(|c| c.rules.clone()).collect() });
                    }
                    hearth_core::Body::GetHistory => match history_body(&children, &store) {
                    Ok(h) => r.send(&from, h),
                        Err(e) => warn!(%e, "history query failed"),
                    },
                    hearth_core::Body::SetRules { rules } => {
                        let res = match children.iter_mut().find(|c| c.user == rules.user) {
                            None => Err(anyhow::anyhow!("no child called {}", rules.user)),
                            Some(c) => c.set_rules(rules.clone()).and_then(|()| store.save_rules(&rules)),
                        };
                        match res {
                            Ok(()) => {
                                info!(user = rules.user, "rules changed from the phone");
                                apply_web(&children, &web_path).await;
                            }
                            Err(e) => {
                                warn!(user = rules.user, %e, "rules from the phone refused");
                                r.send(&from, hearth_core::Body::Notice { text: format!("Rules not changed: {e}") });
                            }
                        }
                        // Always answer with what is in force now, so the app shows the truth.
                        r.broadcast(hearth_core::Body::Rules { children: children.iter().map(|c| c.rules.clone()).collect() });
                    }
                    hearth_core::Body::GetStatus => {
                    r.send(&from, status_body(&children, &status));
                        for c in &children {
                            if let Some((id, minutes, reason)) = &c.pending {
                                r.send(&from, hearth_core::Body::ExtensionRequest { id: id.clone(), user: c.user.clone(), minutes: *minutes, reason: reason.clone() });
                            }
                        }
                    }
                    hearth_core::Body::ExtensionReply { id, approve, minutes } => {
                        if let Some(c) = children.iter_mut().find(|c| c.pending.as_ref().is_some_and(|p| p.0 == id)) {
                            c.pending = None;
                            let text = if approve {
                                let m = minutes.min(240);
                                c.bonus += Duration::from_secs(m as u64 * 60);
                                store.save_bonus(&day, c.uid, c.bonus.as_secs())?;
                                info!(user = c.user, minutes = m, "parent approved more time");
                                format!("Your parent gave you {m} more minutes.")
                            } else {
                                info!(user = c.user, "parent declined more time");
                                "Your parent said no this time.".to_string()
                            };
                            c.answer = (c.answer.0 + 1, text);
                        }
                    }
                    _ => {}
                }
                continue;
            }
        }
        let now = Instant::now();
        let delta = capped(now - last, TICK);
        last = now;

        // Wall clock only decides the day; log jumps (time zone or clock changes).
        let wall = chrono::Local::now();
        let wall_delta = (wall - last_wall).num_seconds() - delta.as_secs() as i64;
        if wall_delta.abs() > 5 * 60 {
            warn!(jump_s = wall_delta, "wall clock jumped");
        }
        last_wall = wall;

        let new_day = today();
        if new_day != day {
            for c in &mut children {
                store.save(&day, c.uid, c.used.as_secs())?;
                for (a, u) in &c.app_used {
                    store.save_app(&day, c.uid, a, u.as_secs())?;
                }
                c.used = Duration::ZERO;
                c.app_used.clear();
                c.bonus = Duration::ZERO;
                c.pending = None;
            }
            day = new_day;
        }

        let sessions = match session::list(&conn, &mgr, &uids).await {
            Ok(s) => s,
            Err(e) => {
                warn!(%e, "logind query failed");
                continue;
            }
        };
        let input_age = activity.input_age(now);
        ending.retain(|id, _| sessions.iter().any(|s| &s.id == id));

        let mut reached = Vec::new();
        for c in &mut children {
            let mine: Vec<_> = sessions.iter().filter(|s| s.uid == c.uid).collect();
            let counting = mine.iter().any(|s| counts(s.active, s.locked, input_age));
            if counting {
                c.used += delta;
            }

            // Per-app "time open": only limited apps are tracked. An app with two
            // units (e.g. Flatpak + launcher scope) counts once.
            if !c.app_limits.is_empty() {
                for (app, cgroups) in apps::running(c.uid) {
                    let Some(limit) = c.app_limits.get(&app) else { continue };
                    let used = c.app_used.entry(app.clone()).or_default();
                    if counting {
                        *used += delta;
                    }
                    if *used < *limit {
                        continue;
                    }
                    for cg in cgroups {
                        match closing.get(&cg) {
                            None => {
                                info!(user = c.user, app, "app limit reached, closing");
                                apps::term(&cg);
                                closing.insert(cg, now);
                            }
                            Some(t) if now - *t >= FORCE_AFTER => {
                                warn!(user = c.user, app, "app still open, killing");
                                apps::kill(&cg);
                            }
                            Some(_) => {}
                        }
                    }
                }
            }
            let today = &c.days[wall.weekday().num_days_from_monday() as usize];
            let limit = Duration::from_secs(today.minutes * 60) + c.bonus;
            let over_limit = c.used >= limit;
            let over = over_limit || outside(today.window, wall.time());
            if !over {
                c.over_since = None; // back inside the window, or a new day reset `used`
            } else if c.over_since.is_none() && !mine.is_empty() {
                // Kept across logouts, so logging in again doesn't restart the grace period.
                info!(user = c.user, reason = if over_limit { "daily limit" } else { "outside allowed hours" }, "enforcing");
                c.over_since = Some(now);
                reached.push(c.user.clone());
            }
            let left = remaining(limit, c.used, today.window, wall.time());
            let state = if over { "over" } else { "ok" };
            let answer = if c.answer.0 == 0 { String::new() } else { format!("{}|{}", c.answer.0, c.answer.1) };
            status.lock().unwrap().insert(c.uid, (left.as_secs() as i64, state.into(), answer));
            for s in mine {
                let res = match action(c.over_since.map(|t| now - t), grace, s.locked) {
                    Action::Nothing => continue,
                    Action::Lock => mgr.lock_session(&s.id).await,
                    Action::Terminate => match ending.get(&s.id) {
                        None if session::end_gracefully(s) => {
                            info!(user = c.user, session = s.id, "logging out");
                            ending.insert(s.id.clone(), now);
                            Ok(())
                        }
                        Some(t) if now - *t < FORCE_AFTER => Ok(()),
                        _ => {
                            info!(user = c.user, session = s.id, "terminating");
                            mgr.terminate_session(&s.id).await
                        }
                    },
                };
                if let Err(e) = res {
                    warn!(user = c.user, session = s.id, %e, "enforcement failed");
                }
            }
        }

        if !reached.is_empty() {
            if let Some(r) = relay.as_mut().filter(|r| r.paired() > 0) {
                r.broadcast(status_body(&children, &status)); // a child just hit a limit
            }
        }

        // ponytail: no shutdown hook; a crash or stop loses at most one checkpoint (30 s).
        if now - last_save >= CHECKPOINT {
            for c in &children {
                store.save(&day, c.uid, c.used.as_secs())?;
                for (a, u) in &c.app_used {
                    store.save_app(&day, c.uid, a, u.as_secs())?;
                }
            }
            closing.retain(|cg, _| cg.exists());
            last_save = now;
        }
    }
}
