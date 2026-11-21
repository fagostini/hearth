//! Rules. ponytail: a TOML file until the parent app's D-Bus API (M5) moves rules into SQLite.
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    /// Seconds between lock and logout once the limit is reached.
    #[serde(default = "default_grace")]
    pub grace_seconds: u64,
    #[serde(default)]
    pub child: Vec<ChildRule>,
}

#[derive(Deserialize)]
pub struct ChildRule {
    pub user: String,
    pub daily_minutes: u64,
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
    }
}
