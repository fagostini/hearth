//! Startup config (TOML): the children, grace period, relay. Each child's rules live in
//! SQLite once imported (see `ChildRule::to_rules`) and are edited from the parent app.
use chrono::NaiveTime;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Config {
    /// Seconds between lock and logout once the limit is reached.
    #[serde(default = "default_grace")]
    pub grace_seconds: u64,
    /// ntfy server for the parent's phone (e.g. "https://ntfy.sh"); absent = no phone.
    pub relay: Option<String>,
    #[serde(default)]
    pub child: Vec<ChildRule>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ChildRule {
    pub user: String,
    pub daily_minutes: u64,
    /// "HH:MM-HH:MM", same day (no overnight windows). None = any time.
    pub allowed: Option<String>,
    /// Overrides per day: keys mon..sun.
    #[serde(default)]
    pub weekday: HashMap<String, DayRule>,
    /// Daily minutes per desktop app ID (e.g. "org.mozilla.firefox"); 0 = blocked.
    #[serde(default)]
    pub apps: HashMap<String, u64>,
    /// DNS filtering; absent = no filtering for this child.
    pub web: Option<Web>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Web {
    /// Domains blocked with all their subdomains.
    #[serde(default)]
    pub block: Vec<String>,
    #[serde(default = "yes")]
    pub safesearch: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DayRule {
    pub daily_minutes: Option<u64>,
    pub allowed: Option<String>,
}

pub type Window = (NaiveTime, NaiveTime);

/// The day's rule: (minutes, allowed window).
#[derive(Default)]
pub struct Day {
    pub minutes: u64,
    pub window: Option<Window>,
}

const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

pub fn parse_window(s: &str) -> anyhow::Result<Window> {
    let (a, b) = s.split_once('-').ok_or_else(|| anyhow::anyhow!("allowed: expected HH:MM-HH:MM, got {s:?}"))?;
    let (a, b) = (NaiveTime::parse_from_str(a.trim(), "%H:%M")?, NaiveTime::parse_from_str(b.trim(), "%H:%M")?);
    anyhow::ensure!(a < b, "allowed: {s:?} must start before it ends (no overnight windows)");
    Ok((a, b))
}

impl ChildRule {
    /// Resolve all seven days up front, so a bad rule fails at startup, not at midnight.
    pub fn days(&self) -> anyhow::Result<[Day; 7]> {
        for k in self.weekday.keys() {
            anyhow::ensure!(DAYS.contains(&k.as_str()), "weekday: unknown day {k:?} (use mon..sun)");
        }
        let base = self.allowed.as_deref().map(parse_window).transpose()?;
        let day = |name: &str| -> anyhow::Result<Day> {
            let o = self.weekday.get(name);
            Ok(Day {
                minutes: o.and_then(|d| d.daily_minutes).unwrap_or(self.daily_minutes),
                window: match o.and_then(|d| d.allowed.as_deref()) {
                    Some(w) => Some(parse_window(w)?),
                    None => base,
                },
            })
        };
        Ok([day("mon")?, day("tue")?, day("wed")?, day("thu")?, day("fri")?, day("sat")?, day("sun")?])
    }
}

impl ChildRule {
    /// First-run import into the app's model: one limit and window for school days
    /// (Mon-Fri, taken from Monday) and one for weekends (from Saturday).
    /// ponytail: other per-day TOML overrides are dropped; add per-day rules to the app
    /// model if parents ask for them.
    pub fn to_rules(&self) -> anyhow::Result<hearth_core::Rules> {
        let d = self.days()?;
        let win = |w: Option<Window>| w.map(|(a, b)| format!("{}-{}", a.format("%H:%M"), b.format("%H:%M")));
        let mut apps: Vec<_> = self.apps.iter().map(|(a, m)| hearth_core::AppRule { app: a.clone(), minutes: *m as u32 }).collect();
        apps.sort_by(|a, b| a.app.cmp(&b.app));
        Ok(hearth_core::Rules {
            user: self.user.clone(),
            school_minutes: d[0].minutes as u32,
            weekend_minutes: d[5].minutes as u32,
            school_window: win(d[0].window),
            weekend_window: win(d[5].window),
            apps,
            web: self.web.as_ref().map(|w| hearth_core::WebRule { safesearch: w.safesearch, block: w.block.clone() }),
        })
    }
}

/// The seven days (Mon..Sun) of app-model rules. Also the validation for rules that
/// arrive from a phone: anything a parent can send is checked here before it's used.
pub fn days_from(r: &hearth_core::Rules) -> anyhow::Result<[Day; 7]> {
    use anyhow::ensure;
    ensure!(r.school_minutes <= 1440 && r.weekend_minutes <= 1440, "a day has only 24 hours");
    let school = r.school_window.as_deref().map(parse_window).transpose()?;
    let weekend = r.weekend_window.as_deref().map(parse_window).transpose()?;
    ensure!(r.apps.len() <= 200, "too many apps");
    for a in &r.apps {
        ensure!(!a.app.trim().is_empty() && a.app.len() <= 255 && !a.app.contains(['/', ' ']), "not an app ID: {:?}", a.app);
        ensure!(a.minutes <= 1440, "a day has only 24 hours");
    }
    if let Some(w) = &r.web {
        ensure!(w.block.len() <= 2000, "too many blocked sites");
        for d in &w.block {
            let ok = !d.is_empty() && d.len() <= 253 && d.contains('.') && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
            ensure!(ok, "not a website address: {d:?}");
        }
    }
    let s = Day { minutes: r.school_minutes as u64, window: school };
    let w = || Day { minutes: r.weekend_minutes as u64, window: weekend };
    let sd = || Day { minutes: s.minutes, window: s.window };
    Ok([sd(), sd(), sd(), sd(), sd(), w(), w()])
}

fn default_grace() -> u64 {
    60
}

/// UID of `user` from passwd-format text.
pub fn uid_of(passwd: &str, user: &str) -> Option<u32> {
    passwd.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() > 2 && f[0] == user).then(|| f[2].parse().ok()).flatten()
    })
}

/// Is `user` listed in an admin group (sudo, wheel, admin) in group-format text?
/// Rules never apply to admins, so a bad rule can't lock the parent out.
pub fn is_admin(group: &str, user: &str) -> bool {
    group.lines().any(|l| {
        let f: Vec<&str> = l.split(':').collect();
        f.len() > 3 && ["sudo", "wheel", "admin"].contains(&f[0]) && f[3].split(',').any(|m| m == user)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let passwd = "root:x:0:0::/root:/bin/bash\nalice:x:1001:1001::/home/alice:/bin/bash\n";
        assert_eq!(uid_of(passwd, "alice"), Some(1001));
        assert_eq!(uid_of(passwd, "bob"), None);
        let group = "sudo:x:27:mum,dad\nwheel:x:10:\nalicegrp:x:1001:alice\n";
        assert!(is_admin(group, "dad"));
        assert!(!is_admin(group, "alice"));
        let c: Config = toml::from_str("[[child]]\nuser = \"alice\"\ndaily_minutes = 120\n").unwrap();
        assert_eq!((c.grace_seconds, c.child[0].daily_minutes), (60, 120));

        let c: Config = toml::from_str(
            "[[child]]\nuser = \"a\"\ndaily_minutes = 60\nallowed = \"07:00-20:00\"\n\
             [child.weekday.sat]\ndaily_minutes = 180\nallowed = \"08:00-21:30\"\n",
        )
        .unwrap();
        let d = c.child[0].days().unwrap();
        let t = |s| NaiveTime::parse_from_str(s, "%H:%M").unwrap();
        assert_eq!((d[0].minutes, d[0].window), (60, Some((t("07:00"), t("20:00")))));
        assert_eq!((d[5].minutes, d[5].window), (180, Some((t("08:00"), t("21:30")))));
        assert!(parse_window("20:00-07:00").is_err());
        assert!(parse_window("7am-8pm").is_err());
        let bad: Config = toml::from_str("[[child]]\nuser = \"a\"\ndaily_minutes = 1\n[child.weekday.funday]\n").unwrap();
        assert!(bad.child[0].days().is_err());

        let r = c.child[0].to_rules().unwrap();
        assert_eq!((r.school_minutes, r.weekend_minutes), (60, 180));
        assert_eq!((r.school_window.as_deref(), r.weekend_window.as_deref()), (Some("07:00-20:00"), Some("08:00-21:30")));
        let d = days_from(&r).unwrap();
        assert_eq!((d[4].minutes, d[6].minutes), (60, 180));
    }

    #[test]
    fn phone_rules_are_checked() {
        let ok = hearth_core::Rules { user: "a".into(), school_minutes: 60, weekend_minutes: 90, ..Default::default() };
        assert!(days_from(&ok).is_ok());
        let bad = |f: fn(&mut hearth_core::Rules)| {
            let mut r = ok.clone();
            f(&mut r);
            days_from(&r).is_err()
        };
        assert!(bad(|r| r.school_minutes = 5000));
        assert!(bad(|r| r.school_window = Some("21:00-07:00".into())));
        assert!(bad(|r| r.apps = vec![hearth_core::AppRule { app: "../etc".into(), minutes: 0 }]));
        assert!(bad(|r| r.web = Some(hearth_core::WebRule { safesearch: true, block: vec!["bad domain".into()] })));
        assert!(bad(|r| r.web = Some(hearth_core::WebRule { safesearch: true, block: vec!["*.com".into()] })));
    }
}
