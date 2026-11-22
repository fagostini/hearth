//! hearth-agent: runs in the child's desktop session and shows warnings as desktop
//! notifications. Holds no rules and enforces nothing: killing it changes nothing.
use std::{collections::HashMap, time::Duration};
use zbus::{proxy, Connection, Variant as Value};

#[proxy(
    interface = "org.hearth.Daemon1",
    default_service = "org.hearth.Daemon1",
    default_path = "/org/hearth/Daemon1"
)]
trait Daemon {
    fn status(&self) -> zbus::Result<(i64, String)>;
}

#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, &Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

/// Warn when time left first drops to or below these (seconds).
const THRESHOLDS: [i64; 3] = [600, 300, 60];

/// Which warning to show now, if any: the smallest threshold crossed since last check.
fn due(prev: i64, left: i64) -> Option<i64> {
    THRESHOLDS.into_iter().filter(|&t| left <= t && prev > t).min()
}

fn mins(s: i64) -> String {
    match (s + 59) / 60 {
        1 => "1 minute".into(),
        m => format!("{m} minutes"),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> zbus::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--status") {
        let (left, state) = DaemonProxy::new(&Connection::system().await?).await?.status().await?;
        println!("{state} {}", if left < 0 { "-".into() } else { mins(left) });
        return Ok(());
    }
    let notes = NotificationsProxy::new(&Connection::session().await?).await?;
    let notify = |summary: String, body: &'static str, critical: bool| {
        let notes = notes.clone();
        async move {
            let urgency = Value::U8(if critical { 2 } else { 1 });
            let hints = HashMap::from([("urgency", &urgency)]);
            if let Err(e) = notes.notify("Hearth", 0, "appointment-soon", &summary, body, &[], hints, -1).await {
                eprintln!("notify: {e}");
            }
        }
    };

    let (mut prev, mut greeted) = (i64::MAX, false);
    loop {
        // Reconnect each round: hearthd may restart. ponytail: poll every 15 s (the
        // daemon's own tick); a PropertiesChanged signal if this ever matters.
        let status = async { DaemonProxy::new(&Connection::system().await?).await?.status().await };
        match status.await {
            Ok((left, state)) if left >= 0 => {
                if !greeted {
                    greeted = true;
                    if left > THRESHOLDS[0] {
                        notify(format!("{} of screen time left today", mins(left)), "", false).await;
                    }
                }
                // No "time's up" message: the screen locks at that moment, so nobody would
                // see it. The 1-minute warning is the last one.
                if state != "over" {
                    if let Some(t) = due(prev, left) {
                        let body = if t <= 60 { "Save your work now. The screen locks in a minute." } else { "Save your work soon." };
                        notify(format!("{} left", mins(left)), body, t <= 60).await;
                    }
                    prev = left;
                }
            }
            Ok(_) => {} // no rule for this user
            Err(e) => eprintln!("hearthd: {e}"),
        }
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings() {
        assert_eq!(due(i64::MAX, 3600), None);
        assert_eq!(due(615, 600), Some(600));
        assert_eq!(due(600, 585), None); // already warned
        assert_eq!(due(i64::MAX, 250), Some(300)); // login with 4 min left: one warning, not three
        assert_eq!(due(400, 30), Some(60)); // skipped past 5 min (window end): smallest
        assert_eq!(mins(59), "1 minute");
        assert_eq!(mins(61), "2 minutes");
    }
}
