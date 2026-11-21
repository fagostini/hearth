mod accounting;
mod activity;
mod config;
mod session;
mod store;

use accounting::{action, capped, counts, Action};
use anyhow::Context;
use std::{env, fs, path::PathBuf, time::{Duration, Instant}};
use tracing::{info, warn};

const TICK: Duration = Duration::from_secs(15);
const CHECKPOINT: Duration = Duration::from_secs(30);

struct Child {
    user: String,
    uid: u32,
    limit: Duration,
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
        let used = Duration::from_secs(store.load(&day, uid)?);
        info!(user = r.user, uid, limit_min = r.daily_minutes, used_s = used.as_secs(), "child");
        children.push(Child { user: r.user, uid, limit: Duration::from_secs(r.daily_minutes * 60), used, over_since: None });
    }
    let uids: Vec<u32> = children.iter().map(|c| c.uid).collect();

    let activity = activity::Activity::default();
    tokio::spawn(activity.clone().watch());

    let conn = zbus::Connection::system().await?;
    let mgr = session::ManagerProxy::new(&conn).await?;

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
                c.over_since = None;
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

        for c in &mut children {
            let mine: Vec<_> = sessions.iter().filter(|s| s.uid == c.uid).collect();
            if mine.iter().any(|s| counts(s.active, s.locked, input_age)) {
                c.used += delta;
            }
            if c.used >= c.limit && c.over_since.is_none() && !mine.is_empty() {
                info!(user = c.user, "daily limit reached");
                c.over_since = Some(now);
            }
            for s in mine {
                let res = match action(c.over_since.map(|t| now - t), grace, s.locked) {
                    Action::Nothing => continue,
                    Action::Lock => mgr.lock_session(&s.id).await,
                    Action::Terminate => mgr.terminate_session(&s.id).await,
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
