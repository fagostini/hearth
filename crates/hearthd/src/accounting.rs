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
    fn enforcement() {
        assert_eq!(action(None, S(60), false), Action::Nothing);
        assert_eq!(action(Some(S(0)), S(60), false), Action::Lock);
        assert_eq!(action(Some(S(30)), S(60), true), Action::Nothing);
        assert_eq!(action(Some(S(60)), S(60), true), Action::Terminate);
    }
}
