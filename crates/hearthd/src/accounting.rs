//! Pure accounting and enforcement rules (no I/O), so they can be unit-tested.
use std::time::Duration;

/// No input for this long = idle. ponytail: fixed for the MVP; per-child setting if parents ask.
pub const IDLE_AFTER: Duration = Duration::from_secs(5 * 60);

/// Does this session's time count? `input_age` is `None` when no input device
/// could be read: unknown counts as active.
pub fn counts(active: bool, locked: bool, input_age: Option<Duration>) -> bool {
    active && !locked && input_age.is_none_or(|a| a < IDLE_AFTER)
}

/// Cap a tick delta so a stalled daemon or a suspend/resume never adds a big chunk.
pub fn capped(delta: Duration, tick: Duration) -> Duration {
    delta.min(tick * 2)
}

/// Outside the allowed window (start inclusive, end exclusive)?
pub fn outside(window: Option<(chrono::NaiveTime, chrono::NaiveTime)>, now: chrono::NaiveTime) -> bool {
    window.is_some_and(|(a, b)| now < a || now >= b)
}

/// Time left today: the smaller of what's left of the daily limit and what's left
/// of the allowed window. Zero outside the window.
pub fn remaining(
    limit: Duration,
    used: Duration,
    window: Option<(chrono::NaiveTime, chrono::NaiveTime)>,
    now: chrono::NaiveTime,
) -> Duration {
    let by_limit = limit.saturating_sub(used);
    match window {
        Some(w) if outside(Some(w), now) => Duration::ZERO,
        Some((_, end)) => by_limit.min((end - now).to_std().unwrap_or_default()),
        None => by_limit,
    }
}

#[derive(Debug, PartialEq)]
pub enum Action {
    Nothing,
    Lock,
    Terminate,
}

/// What to do with one of the child's sessions. `over_for` is how long the child
/// has been over the limit today (`None` = not over).
pub fn action(over_for: Option<Duration>, grace: Duration, locked: bool) -> Action {
    match over_for {
        None => Action::Nothing,
        Some(t) if t >= grace => Action::Terminate,
        Some(_) if locked => Action::Nothing,
        Some(_) => Action::Lock, // also re-locks if the child unlocks with their own password
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const S: fn(u64) -> Duration = Duration::from_secs;

    #[test]
    fn counting() {
        assert!(counts(true, false, Some(S(10))));
        assert!(!counts(true, false, Some(S(5 * 60)))); // idle
        assert!(counts(true, false, None)); // unknown = active
        assert!(!counts(false, false, Some(S(1)))); // background session
        assert!(!counts(true, true, Some(S(1)))); // locked
    }

    #[test]
    fn cap() {
        assert_eq!(capped(S(15), S(15)), S(15));
        assert_eq!(capped(S(3600), S(15)), S(30)); // resume after suspend
    }

    #[test]
    fn window() {
        let t = |s| chrono::NaiveTime::parse_from_str(s, "%H:%M").unwrap();
        let w = Some((t("07:00"), t("20:00")));
        assert!(outside(w, t("06:59")));
        assert!(!outside(w, t("07:00")));
        assert!(outside(w, t("20:00")));
        assert!(!outside(None, t("03:00")));
        let r = |used, w, now| remaining(S(3600), S(used), w, t(now)).as_secs();
        assert_eq!(r(600, None, "12:00"), 3000);
        assert_eq!(r(4000, None, "12:00"), 0);
        assert_eq!(r(0, w, "19:50"), 600); // window ends first
        assert_eq!(r(0, w, "21:00"), 0); // outside
    }

    #[test]
    fn enforcement() {
        assert_eq!(action(None, S(60), false), Action::Nothing);
        assert_eq!(action(Some(S(0)), S(60), false), Action::Lock);
        assert_eq!(action(Some(S(30)), S(60), true), Action::Nothing);
        assert_eq!(action(Some(S(60)), S(60), true), Action::Terminate);
    }
}
