mod accounting;
mod api;
mod activity;
mod config;
mod session;
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
    days: [config::Day; 7],
    used: Duration,
    over_since: Option<Instant>,
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().without_time().init(); // journald adds timestamps
    let config_path = env::var("HEARTH_CONFIG").unwrap_or("/etc/hearth/hearthd.toml".into());
    let db_path = PathBuf::from(env::var("HEARTH_DB").unwrap_or("/var/lib/hearth/hearth.db".into()));

    let cfg: config::Config = toml::from_str(&fs::read_to_string(&config_path).with_context(|| config_path.clone())?)?;
    let grace = Duration::from_secs(cfg.grace_seconds);
    let (passwd, group) = (fs::read_to_string("/etc/passwd")?, fs::read_to_string("/etc/group")?);
    let store = store::Store::open(&db_path)?;
    let mut day = today();

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
        let days = r.days().with_context(|| format!("rule for {}", r.user))?;
        let used = Duration::from_secs(store.load(&day, uid)?);
        info!(user = r.user, uid, limit_min = r.daily_minutes, allowed = r.allowed, used_s = used.as_secs(), "child");
        children.push(Child { user: r.user, uid, days, used, over_since: None });
    }
    let uids: Vec<u32> = children.iter().map(|c| c.uid).collect();

    let activity = activity::Activity::default();
    tokio::spawn(activity.clone().watch());

    let status = api::Shared::default();
    let conn = zbus::connection::Builder::system()
        .serve_at("/org/hearth/Daemon1", api::Api(status.clone()))
        .name("org.hearth.Daemon1")
        .build()
        .await?;
    let mgr = session::ManagerProxy::new(&conn).await?;

    let mut ending: HashMap<String, Instant> = HashMap::new(); // session id -> SIGTERM sent
    let mut ticker = tokio::time::interval(TICK);
    let (mut last, mut last_wall, mut last_save) = (Instant::now(), chrono::Local::now(), Instant::now());
    loop {
        ticker.tick().await;
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
                c.used = Duration::ZERO;
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

        for c in &mut children {
            let mine: Vec<_> = sessions.iter().filter(|s| s.uid == c.uid).collect();
            if mine.iter().any(|s| counts(s.active, s.locked, input_age)) {
                c.used += delta;
            }
            let today = &c.days[wall.weekday().num_days_from_monday() as usize];
            let over_limit = c.used >= Duration::from_secs(today.minutes * 60);
            let over = over_limit || outside(today.window, wall.time());
            if !over {
                c.over_since = None; // back inside the window, or a new day reset `used`
            } else if c.over_since.is_none() && !mine.is_empty() {
                // Kept across logouts, so logging in again doesn't restart the grace period.
                info!(user = c.user, reason = if over_limit { "daily limit" } else { "outside allowed hours" }, "enforcing");
                c.over_since = Some(now);
            }
            let left = remaining(Duration::from_secs(today.minutes * 60), c.used, today.window, wall.time());
            let state = if c.over_since.is_some() { "over" } else { "ok" };
            status.lock().unwrap().insert(c.uid, (left.as_secs() as i64, state.into()));
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

        // ponytail: no shutdown hook; a crash or stop loses at most one checkpoint (30 s).
        if now - last_save >= CHECKPOINT {
            for c in &children {
                store.save(&day, c.uid, c.used.as_secs())?;
            }
            last_save = now;
        }
    }
}
