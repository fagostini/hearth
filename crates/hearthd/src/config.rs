//! Rules. ponytail: a TOML file until the parent app's D-Bus API (M5) moves rules into SQLite.
use chrono::NaiveTime;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Config {
    /// Seconds between lock and logout once the limit is reached.
    #[serde(default = "default_grace")]
    pub grace_seconds: u64,
    #[serde(default)]
    pub child: Vec<ChildRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildRule {
    pub user: String,
    pub daily_minutes: u64,
    /// "HH:MM-HH:MM", same day (no overnight windows). None = any time.
    pub allowed: Option<String>,
    /// Overrides per day: keys mon..sun.
    #[serde(default)]
    pub weekday: HashMap<String, DayRule>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DayRule {
    pub daily_minutes: Option<u64>,
    pub allowed: Option<String>,
}

pub type Window = (NaiveTime, NaiveTime);

/// The day's rule: (minutes, allowed window).
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
    }
}
