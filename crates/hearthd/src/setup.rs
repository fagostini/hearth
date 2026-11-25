//! `sudo hearthd setup`: first-run wizard. Picks the child accounts (creating them if
//! needed), checks they aren't administrators, warns about Malcontent's own limits,
//! fixes the nss-resolve DNS bypass, writes /etc/hearth/hearthd.toml, starts the
//! services and offers to pair the parent's phone.
use crate::config;
use anyhow::{Context, bail};
use std::{
    fs,
    io::{self, BufRead, Write},
    path::Path,
    process::Command,
};

const CONFIG: &str = "/etc/hearth/hearthd.toml";
const NSSWITCH: &str = "/etc/nsswitch.conf";

struct Child {
    user: String,
    school: u32,
    weekend: u32,
    allowed: Option<String>,
    safesearch: bool,
}

pub async fn run() -> anyhow::Result<()> {
    anyhow::ensure!(nix_is_root(), "run it as root: sudo hearthd setup");
    println!("Hearth setup\n============\n");
    if Path::new(CONFIG).exists() && !yes(&format!("{CONFIG} exists. Replace it?"), false)? {
        println!("Nothing changed. Edit {CONFIG} by hand, or change rules from the parent app.");
        return Ok(());
    }

    println!("Each child needs their own Linux account, without administrator rights.");
    println!("Hearth tells children apart by account: siblings sharing one account share one set of rules.\n");
    let mut users = pick_children()?;
    while users.is_empty() || yes("Create another account for a child?", users.is_empty())? {
        match create_account()? {
            Some(u) => users.push(u),
            None if users.is_empty() => bail!("no child accounts, nothing to set up"),
            None => break,
        }
    }

    let mut children = Vec::new();
    for user in users {
        println!("\nRules for {user} (the parent app can change them later):");
        let school = number("  Minutes per day, Monday to Friday", 120)?;
        let weekend = number("  Minutes per day, Saturday and Sunday", 180)?;
        let allowed = loop {
            let w = ask("  Allowed hours (HH:MM-HH:MM, or \"any\")", "07:00-20:00")?;
            if w == "any" {
                break None;
            }
            match config::parse_window(&w) {
                Ok(_) => break Some(w),
                Err(e) => println!("  {e}"),
            }
        };
        let safesearch = yes("  Turn on web filtering (SafeSearch on Google, Bing, DuckDuckGo, YouTube)?", true)?;
        if malcontent_limits(config::uid_of(&fs::read_to_string("/etc/passwd")?, &user).unwrap_or(0)).await {
            println!(
                "  Warning: GNOME Parental Controls (Malcontent) also limits {user}'s screen time. Two tools\n  \
                 would lock the same session. Turn its screen time limits off in Settings > Parental Controls."
            );
        }
        children.push(Child { user, school, weekend, allowed, safesearch });
    }

    println!("\nThe parent app talks to this PC through an ntfy relay. Messages are end to end");
    println!("encrypted: the relay can't read them. Use the public one, or your own ntfy server.");
    let relay = ask("Relay", "https://ntfy.sh")?;

    if children.iter().any(|c| c.safesearch) {
        fix_nss_resolve()?;
    }

    fs::create_dir_all("/etc/hearth")?;
    // Readable by hearth-dns (a DynamicUser); it holds no secrets.
    fs::write(CONFIG, config_toml(&children, &relay)).with_context(|| CONFIG)?;
    println!("\nWrote {CONFIG}.");
    if Path::new("/var/lib/hearth/hearth.db").exists() {
        println!("Children Hearth already knew keep the rules they had; the ones above apply to new children.");
    }

    for args in [&["enable", "hearth-dns.service", "hearthd.service"][..], &["restart", "hearth-dns.service", "hearthd.service"]] {
        let ok = Command::new("systemctl").args(args).status().is_ok_and(|s| s.success());
        anyhow::ensure!(ok, "systemctl {} failed; see: journalctl -u hearthd", args[0]);
    }
    println!("Hearth is running. Children see it the next time they log in.\n");

    if yes("Pair the parent's phone now? (install the Hearth app first)", true)? {
        // The daemon needs a moment to start and reach the relay.
        for i in 0.. {
            match crate::show_pairing_code(false).await {
                Ok(()) => break,
                Err(_) if i < 10 => tokio::time::sleep(std::time::Duration::from_secs(1)).await,
                Err(e) => {
                    println!("Couldn't get a pairing code ({e}). Try later: sudo hearthd pair");
                    break;
                }
            }
        }
    } else {
        println!("Pair later with: sudo hearthd pair");
    }
    Ok(())
}

/// Lists the people accounts and asks which belong to children. Admins are refused:
/// they could undo Hearth.
fn pick_children() -> anyhow::Result<Vec<String>> {
    let passwd = fs::read_to_string("/etc/passwd")?;
    let group = fs::read_to_string("/etc/group")?;
    let people = people(&passwd);
    if people.is_empty() {
        return Ok(vec![]);
    }
    println!("Accounts on this PC:");
    for u in &people {
        let admin = config::is_admin(&group, u) || crate::has_sudo(u);
        println!("  {u}{}", if admin { "  (administrator)" } else { "" });
    }
    loop {
        let line = ask("Which are children's accounts? (names separated by spaces, or empty for none)", "")?;
        let picked: Vec<String> = line.split_whitespace().map(String::from).collect();
        let mut ok = true;
        for u in &picked {
            if !people.contains(u) {
                println!("  There's no account called {u}.");
                ok = false;
            } else if config::is_admin(&group, u) || crate::has_sudo(u) {
                println!("  {u} is an administrator. A child's account must be a standard one:");
                println!("  remove it from the admin group first (Ubuntu: sudo deluser {u} sudo; Fedora: sudo gpasswd -d {u} wheel).");
                ok = false;
            }
        }
        if ok {
            return Ok(picked);
        }
    }
}

/// Accounts of people: normal UIDs with a login shell.
fn people(passwd: &str) -> Vec<String> {
    passwd
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            let uid: u32 = f.get(2)?.parse().ok()?;
            let shell = f.get(6).copied().unwrap_or("");
            ((1000..60000).contains(&uid) && !shell.ends_with("nologin") && !shell.ends_with("false")).then(|| f[0].to_string())
        })
        .collect()
}

fn create_account() -> anyhow::Result<Option<String>> {
    let name = ask("New account name (lower case, e.g. mia; empty to skip)", "")?;
    if name.is_empty() {
        return Ok(None);
    }
    if !valid_name(&name) {
        println!("  Use lower-case letters, digits, - or _, starting with a letter.");
        return Ok(None);
    }
    let ok = Command::new("useradd").args(["-m", "-s", "/bin/bash", &name]).status().is_ok_and(|s| s.success())
        && Command::new("passwd").arg(&name).status().is_ok_and(|s| s.success());
    if !ok {
        println!("  Couldn't create {name}.");
        return Ok(None);
    }
    println!("  Created {name}. It appears on the login screen.");
    Ok(Some(name))
}

fn valid_name(n: &str) -> bool {
    n.len() <= 32
        && n.starts_with(|c: char| c.is_ascii_lowercase())
        && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// GNOME's own screen time limits for this account, through Malcontent's AccountsService
/// extension. LimitType 0 = no limit. Absent (not GNOME, or no Malcontent) = false.
async fn malcontent_limits(uid: u32) -> bool {
    let Ok(conn) = zbus::Connection::system().await else { return false };
    let path = format!("/org/freedesktop/Accounts/User{uid}");
    let Ok(p) = zbus::Proxy::new(&conn, "org.freedesktop.Accounts", path, "com.endlessm.ParentalControls.SessionLimits").await
    else {
        return false;
    };
    p.get_property::<u32>("LimitType").await.is_ok_and(|t| t != 0)
}

/// With `resolve` in nsswitch's hosts line, programs ask systemd-resolved over D-Bus
/// instead of DNS, and the query leaves as systemd-resolve, not as the child, so the
/// per-child redirect never sees it. Without it they use DNS through resolv.conf.
/// ponytail: on Fedora authselect may write the line back; hearthd could check at start.
fn fix_nss_resolve() -> anyhow::Result<()> {
    let Ok(text) = fs::read_to_string(NSSWITCH) else { return Ok(()) };
    let Some(fixed) = without_nss_resolve(&text) else { return Ok(()) };
    println!("\nThis PC looks up names through systemd-resolved directly ({NSSWITCH}), which");
    println!("would let children get around web filtering. Hearth can switch it to plain DNS");
    println!("(still through systemd-resolved, so nothing else changes).");
    if yes("Change it? (a backup is kept)", true)? {
        fs::copy(NSSWITCH, format!("{NSSWITCH}.hearth-backup"))?;
        fs::write(NSSWITCH, fixed)?;
        println!("Changed. Backup: {NSSWITCH}.hearth-backup");
    } else {
        println!("Not changed: web filtering can be bypassed on this PC.");
    }
    Ok(())
}

/// The file without `resolve` (and its `[...]` action) on the hosts line, with `dns`
/// present. None if there's nothing to change.
fn without_nss_resolve(text: &str) -> Option<String> {
    let mut changed = false;
    let out: Vec<String> = text
        .lines()
        .map(|l| {
            let Some(rest) = l.strip_prefix("hosts:") else { return l.to_string() };
            let words: Vec<&str> = rest.split_whitespace().collect();
            if !words.contains(&"resolve") {
                return l.to_string();
            }
            changed = true;
            let mut kept = Vec::new();
            let mut skip_action = false;
            for w in words {
                if w == "resolve" {
                    skip_action = true;
                    continue;
                }
                if skip_action && w.starts_with('[') {
                    continue;
                }
                skip_action = false;
                kept.push(w);
            }
            if !kept.contains(&"dns") {
                kept.push("dns");
            }
            format!("hosts:      {}", kept.join(" "))
        })
        .collect();
    changed.then(|| out.join("\n") + "\n")
}

fn config_toml(children: &[Child], relay: &str) -> String {
    let mut s = String::from(
        "# Written by `hearthd setup`. Each child's rules below are imported into the database\n\
         # on first start; after that the parent app edits them.\n",
    );
    s += &format!("relay = {relay:?}\ngrace_seconds = 60\n");
    for c in children {
        s += &format!("\n[[child]]\nuser = {:?}\ndaily_minutes = {}\n", c.user, c.school);
        if let Some(w) = &c.allowed {
            s += &format!("allowed = {w:?}\n");
        }
        for d in ["sat", "sun"] {
            s += &format!("[child.weekday.{d}]\ndaily_minutes = {}\n", c.weekend);
        }
        if c.safesearch {
            s += "[child.web]\nsafesearch = true\n";
        }
    }
    s
}

fn nix_is_root() -> bool {
    // ponytail: /proc/self/status instead of a libc dependency for one geteuid().
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Uid:")).map(|l| l.split_whitespace().nth(2) == Some("0")))
        .unwrap_or(false)
}

fn ask(prompt: &str, default: &str) -> anyhow::Result<String> {
    if default.is_empty() {
        print!("{prompt}: ");
    } else {
        print!("{prompt} [{default}]: ");
    }
    io::stdout().flush()?;
    let mut line = String::new();
    anyhow::ensure!(io::stdin().lock().read_line(&mut line)? > 0, "input ended");
    let line = line.trim();
    Ok(if line.is_empty() { default.to_string() } else { line.to_string() })
}

fn yes(prompt: &str, default: bool) -> anyhow::Result<bool> {
    loop {
        match ask(prompt, if default { "Y/n" } else { "y/N" })?.to_lowercase().as_str() {
            "y/n" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("  Please answer y or n."),
        }
    }
}

fn number(prompt: &str, default: u32) -> anyhow::Result<u32> {
    loop {
        match ask(prompt, &default.to_string())?.parse::<u32>() {
            Ok(n) if n <= 1440 => return Ok(n),
            _ => println!("  A number of minutes from 0 to 1440."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nss_resolve_removed() {
        let fedora = "passwd: files\nhosts:      files myhostname mdns4_minimal [NOTFOUND=return] resolve [!UNAVAIL=return] dns\n";
        let fixed = without_nss_resolve(fedora).unwrap();
        assert!(fixed.contains("hosts:      files myhostname mdns4_minimal [NOTFOUND=return] dns\n"));
        assert!(fixed.starts_with("passwd: files\n"));
        assert_eq!(without_nss_resolve("hosts: files resolve\n").unwrap(), "hosts:      files dns\n");
        assert!(without_nss_resolve("hosts: files mdns4_minimal [NOTFOUND=return] dns\n").is_none()); // Ubuntu
    }

    #[test]
    fn written_config_loads() {
        let kids = [
            Child { user: "mia".into(), school: 90, weekend: 150, allowed: Some("07:00-20:00".into()), safesearch: true },
            Child { user: "leo".into(), school: 0, weekend: 0, allowed: None, safesearch: false },
        ];
        let cfg: config::Config = toml::from_str(&config_toml(&kids, "https://ntfy.sh")).unwrap();
        assert_eq!(cfg.relay.as_deref(), Some("https://ntfy.sh"));
        let r = cfg.child[0].to_rules().unwrap();
        assert_eq!((r.school_minutes, r.weekend_minutes, r.school_window.as_deref()), (90, 150, Some("07:00-20:00")));
        assert!(cfg.child[0].web.is_some() && cfg.child[1].web.is_none());
    }

    #[test]
    fn people_and_names() {
        let p = "root:x:0:0::/root:/bin/bash\nkid:x:1001:1001::/home/kid:/bin/bash\nsvc:x:1002:1002::/:/usr/sbin/nologin\n";
        assert_eq!(people(p), ["kid"]);
        assert!(valid_name("mia-2") && !valid_name("Mia") && !valid_name("2mia") && !valid_name(""));
    }
}
