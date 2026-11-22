//! Running desktop apps per user, from the systemd cgroup tree.
//! Units follow systemd's Desktop Environment Integration naming:
//! app[-<launcher>]-<AppID>[-<RANDOM>|@<RANDOM>].scope/.service, plus D-Bus
//! activated apps as dbus-:<bus>-<AppID>@<n>.service. "-" inside IDs is escaped as \x2d.
use std::{collections::HashMap, fs, path::PathBuf};

/// Desktop-file app ID (without ".desktop") from a unit name, or None if it isn't an app unit.
pub fn app_id(unit: &str) -> Option<String> {
    let base = unit.strip_suffix(".scope").or_else(|| unit.strip_suffix(".service"))?;
    let base = base.split('@').next()?; // drop @RANDOM / @n
    let id = if let Some(rest) = base.strip_prefix("snap.") {
        // snap.<snap>.<app>-<UUID>: desktop ID is <snap>_<app>
        let name = rest.get(..rest.len().checked_sub(37)?)?;
        return name.split_once('.').map(|(snap, app)| format!("{snap}_{app}"));
    } else if let Some(rest) = base.strip_prefix("dbus-") {
        rest.rsplit('-').next()? // dbus-:1.2-<AppID>
    } else {
        let mut parts: Vec<&str> = base.strip_prefix("app-")?.split('-').collect();
        if parts.len() > 1 && parts.last()?.chars().all(|c| c.is_ascii_hexdigit()) {
            parts.pop(); // -<RANDOM>: a PID or hex string
        }
        if parts.len() > 1 && !parts[0].contains('.') {
            parts.remove(0); // launcher: gnome, KDE, flatpak, cinnamon...
        }
        if parts.len() != 1 {
            return None;
        }
        parts[0]
    };
    let id = id.replace("\\x2d", "-");
    (!id.is_empty() && !id.starts_with(':')).then_some(id)
}

/// Running apps of `uid`: app ID -> cgroup dirs (an app can have more than one unit).
/// Empty cgroups (unit exiting) are skipped.
/// ponytail: rescanned every tick; inotify on the tree if 15 s ever feels slow.
pub fn running(uid: u32) -> HashMap<String, Vec<PathBuf>> {
    let root = PathBuf::from(format!(
        "/sys/fs/cgroup/user.slice/user-{uid}.slice/user@{uid}.service/app.slice"
    ));
    let mut out: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if !path.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".slice") {
                stack.push(path); // e.g. app-dbus\x2d...slice around D-Bus activated apps
            } else if let Some(id) = app_id(&name) {
                if fs::read_to_string(path.join("cgroup.procs")).is_ok_and(|p| !p.trim().is_empty()) {
                    out.entry(id).or_default().push(path);
                }
            }
        }
    }
    out
}

/// SIGTERM every process in the cgroup (polite close).
pub fn term(cg: &PathBuf) {
    if let Ok(procs) = fs::read_to_string(cg.join("cgroup.procs")) {
        for pid in procs.lines().filter_map(|l| l.parse::<i32>().ok()) {
            // SAFETY: plain syscall; an exited PID just returns ESRCH.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
}

/// SIGKILL the whole cgroup, including children forked meanwhile (cgroup v2, kernel 5.14+).
pub fn kill(cg: &PathBuf) {
    let _ = fs::write(cg.join("cgroup.kill"), "1");
}

#[cfg(test)]
mod tests {
    use super::app_id;

    #[test]
    fn names() {
        let a = |u| app_id(u).as_deref().map(str::to_owned);
        assert_eq!(a("app-gnome-org.gnome.Evince-12345.scope").as_deref(), Some("org.gnome.Evince"));
        assert_eq!(a("app-flatpak-org.telegram.desktop-4321.scope").as_deref(), Some("org.telegram.desktop"));
        assert_eq!(a("app-KDE-org.kde.okular@0f3a9c.service").as_deref(), Some("org.kde.okular"));
        assert_eq!(a("app-org.kde.konsole-1234.scope").as_deref(), Some("org.kde.konsole"));
        assert_eq!(a("app-gnome-firefox-999.scope").as_deref(), Some("firefox"));
        assert_eq!(a("app-cinnamon-google\\x2dchrome-77.scope").as_deref(), Some("google-chrome"));
        assert_eq!(a("dbus-:1.2-org.gnome.Nautilus@0.service").as_deref(), Some("org.gnome.Nautilus"));
        assert_eq!(a("app-gnome-firefox_firefox-5.scope").as_deref(), Some("firefox_firefox"));
        assert_eq!(a("snap.firefox.firefox-392a5b7d-2e45-43b6-82d8-cacddce9bb18.scope").as_deref(), Some("firefox_firefox"));
        assert_eq!(a("snap.code-insiders.code-insiders-392a5b7d-2e45-43b6-82d8-cacddce9bb18.scope").as_deref(), Some("code-insiders_code-insiders"));
        assert_eq!(a("snap.x.scope"), None);
        assert_eq!(a("pipewire.service"), None);
        assert_eq!(a("app.slice"), None);
        assert_eq!(a("dbus-:1.2-org.freedesktop.portal.Desktop@0.service").as_deref(), Some("org.freedesktop.portal.Desktop"));
    }
}
