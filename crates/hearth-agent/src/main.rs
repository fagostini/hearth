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
    fn status(&self) -> zbus::Result<(i64, String, String)>;
    fn request_extension(&self, minutes: u32, reason: &str) -> zbus::Result<String>;
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

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;
}

/// Minutes asked for by the notification button.
const ASK_MINUTES: u32 = 15;

/// Warn when time left first drops to or below these (seconds).
const THRESHOLDS: [i64; 3] = [600, 300, 60];

/// Which warning to show now, if any: the smallest threshold crossed since last check.
fn due(prev: i64, left: i64) -> Option<i64> {
    if left <= 0 {
        return None; // at the limit: the daemon is already locking
    }
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
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--status") => {
            let (left, state, answer) = DaemonProxy::new(&Connection::system().await?).await?.status().await?;
            println!("{state} {} {answer}", if left < 0 { "-".into() } else { mins(left) });
            return Ok(());
        }
        Some("--ask") => {
            // hearth-agent --ask [minutes] [reason...]
            let minutes = args.get(2).and_then(|m| m.parse().ok()).unwrap_or(ASK_MINUTES);
            let reason = args.get(3..).map(|r| r.join(" ")).unwrap_or_default();
            println!("{}", DaemonProxy::new(&Connection::system().await?).await?.request_extension(minutes, &reason).await?);
            return Ok(());
        }
        _ => {}
    }
    let notes = NotificationsProxy::new(&Connection::session().await?).await?;
    let ask_label = format!("Ask for {ASK_MINUTES} more minutes");
    // Returns the notification id (0 on failure). `ask` adds the "more time" button.
    let notify = |summary: String, body: String, critical: bool, ask: bool| {
        let (notes, label) = (notes.clone(), ask_label.clone());
        async move {
            let urgency = Value::U8(if critical { 2 } else { 1 });
            let hints = HashMap::from([("urgency", &urgency)]);
            let actions: Vec<&str> = if ask { vec!["ask", &label] } else { vec![] };
            notes.notify("Hearth", 0, "appointment-soon", &summary, &body, &actions, hints, -1).await.unwrap_or_else(|e| {
                eprintln!("notify: {e}");
                0
            })
        }
    };

    // Button presses: ask the daemon, show its reply.
    let mut clicks = notes.receive_action_invoked().await?;
    let mut ours: Vec<u32> = Vec::new();

    let (mut prev, mut greeted, mut answered) = (i64::MAX, false, String::new());
    loop {
        // Reconnect each round: hearthd may restart. ponytail: poll every 15 s (the
        // daemon's own tick); a PropertiesChanged signal if this ever matters.
        let status = async { DaemonProxy::new(&Connection::system().await?).await?.status().await };
        match status.await {
            Ok((left, state, answer)) if left >= 0 => {
                if !greeted {
                    greeted = true;
                    answered = answer.clone(); // don't repeat an old answer after login
                    if left > THRESHOLDS[0] {
                        notify(format!("{} of screen time left today", mins(left)), String::new(), false, false).await;
                    }
                }
                if answer != answered {
                    answered = answer.clone();
                    let text = answer.split_once('|').map_or(answer.as_str(), |(_, t)| t).to_string();
                    notify(text, String::new(), false, false).await;
                    prev = left; // a new budget: warnings start over from here
                }
                // No "time's up" message: the screen locks at that moment, so nobody would
                // see it. The 1-minute warning is the last one.
                if state != "over" {
                    if let Some(t) = due(prev, left) {
                        let body = if t <= 60 { "Save your work now. The screen locks in a minute." } else { "Save your work soon." };
                        let id = notify(format!("{} left", mins(left)), body.into(), t <= 60, t <= 300).await;
                        ours.push(id);
                    }
                    prev = left;
                }
            }
            Ok(_) => {} // no rule for this user
            Err(e) => eprintln!("hearthd: {e}"),
        }
        // Wait 15 s, or less if a notification button is pressed.
        let tick = tokio::time::sleep(Duration::from_secs(15));
        tokio::pin!(tick);
        loop {
            tokio::select! {
                _ = &mut tick => break,
                Some(sig) = futures_util::StreamExt::next(&mut clicks) => {
                    let Ok(a) = sig.args() else { continue };
                    if a.action_key == "ask" && ours.contains(&a.id) {
                        let reply = async {
                            DaemonProxy::new(&Connection::system().await?).await?.request_extension(ASK_MINUTES, "").await
                        };
                        let text = reply.await.unwrap_or_else(|e| format!("Couldn't ask: {e}"));
                        notify(text, String::new(), false, false).await;
                    }
                }
            }
        }
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
        assert_eq!(due(i64::MAX, 0), None);
        assert_eq!(mins(59), "1 minute");
        assert_eq!(mins(61), "2 minutes");
    }
}
