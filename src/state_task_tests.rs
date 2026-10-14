// Story 013 / task 1.2 — `TrayState::apply_task` (R5.6, R5.7, R5.5).
//
// Included at the end of `src/state.rs` as its own module:
//
//     #[cfg(test)]
//     mod task_tests {
//         include!("state_task_tests.rs");
//     }
//
// Every name runs under the `cargo test state` filter through the module path.

use super::*;
use crate::protocol::TaskOutcome;

/// A task outcome built through the wire format, so the test pins the JSON the
/// sender writes rather than the struct's Rust field types.
fn task_outcome(outcome: &str, session: &str, task: &str, title: Option<&str>) -> TaskOutcome {
    serde_json::from_value(serde_json::json!({
        "task_outcome": outcome,
        "session_id": session,
        "task_id": task,
        "cwd": "/home/user/zed-patches",
        "project": "zed-patches",
        "thread_title": title,
        "task_type": "shell",
        "duration_ms": 20_500u64,
    }))
    .expect("a task outcome parses")
}

// ---- R5.6 -------------------------------------------------------------------------

#[test]
fn a_task_outcome_notifies_titled_with_outcome_and_project() {
    for (word, title) in [
        ("failed", "Task failed — zed-patches"),
        ("completed", "Task completed — zed-patches"),
        ("stopped", "Task stopped — zed-patches"),
        ("interrupted", "Task interrupted — zed-patches"),
    ] {
        let mut state = TrayState::default();
        let notification = state
            .apply_task(task_outcome(word, "s1", "t1", Some("Bump the series")))
            .unwrap_or_else(|| panic!("a `{word}` task outcome must notify"));
        assert_eq!(notification.summary, title);
    }
}

#[test]
fn the_body_carries_the_thread_title_and_a_readable_duration() {
    let mut state = TrayState::default();
    let notification = state
        .apply_task(task_outcome("completed", "s1", "t1", Some("Bump the series")))
        .expect("a completed task outcome notifies");
    assert!(
        notification.body.starts_with("Bump the series · "),
        "body was {:?}",
        notification.body
    );
    let duration = notification.body.trim_start_matches("Bump the series · ");
    assert!(!duration.is_empty(), "the body must name the duration");
    // A human duration, not the raw wire milliseconds.
    assert!(!notification.body.contains("20500"), "body was {:?}", notification.body);
}

/// A thread that has no title yet is named by its task type, never by a
/// placeholder from Rust or JSON.
#[test]
fn a_missing_thread_title_leaves_no_placeholder_in_the_body() {
    let mut state = TrayState::default();
    let notification = state
        .apply_task(task_outcome("failed", "s1", "t1", None))
        .expect("a failed task outcome notifies");
    assert!(!notification.body.contains("None"), "body was {:?}", notification.body);
    assert!(!notification.body.contains("null"), "body was {:?}", notification.body);
    assert!(
        notification.body.starts_with("shell · "),
        "body was {:?}",
        notification.body
    );
}

// ---- R5.7 — the click path is fed by the datagram's own session and cwd -----------

#[test]
fn the_notification_points_at_the_tasks_session_and_cwd() {
    let mut state = TrayState::default();
    let notification = state
        .apply_task(task_outcome("failed", "sess-013", "t1", Some("Bump the series")))
        .expect("a failed task outcome notifies");
    assert_eq!(notification.session_id, "sess-013");
    assert_eq!(notification.cwd, "/home/user/zed-patches");
}

// ---- mute and history ---------------------------------------------------------------

#[test]
fn muting_suppresses_a_task_popup_but_still_records_it() {
    let mut state = TrayState {
        muted: true,
        ..Default::default()
    };
    assert!(
        state
            .apply_task(task_outcome("failed", "s1", "t1", Some("Bump the series")))
            .is_none()
    );
    assert_eq!(state.history().len(), 1);
    assert_eq!(state.history()[0].session_id, "s1");
    assert_eq!(state.history()[0].cwd, "/home/user/zed-patches");
}

/// Hostile half: two different tasks of the same session ending must not collapse
/// into one history entry.
#[test]
fn two_task_outcomes_of_one_session_are_two_history_entries() {
    let mut state = TrayState::default();
    state.apply_task(task_outcome("completed", "s1", "t1", Some("Same title")));
    state.apply_task(task_outcome("failed", "s1", "t2", Some("Same title")));
    assert_eq!(state.history().len(), 2);
    // Most recent first, and the two are distinguishable.
    assert_ne!(state.history()[0].summary, state.history()[1].summary);
}

/// A task outcome is a notification, not a session transition: it does not
/// invent a session row in the tray's menu.
#[test]
fn a_task_outcome_does_not_create_a_session() {
    let mut state = TrayState::default();
    state.apply_task(task_outcome("failed", "s1", "t1", Some("Bump the series")));
    assert_eq!(state.sessions().count(), 0);
}

// ---- R5.5 -------------------------------------------------------------------------

#[test]
fn free_text_sent_beside_a_task_outcome_never_reaches_the_popup_or_history() {
    let task: TaskOutcome = serde_json::from_value(serde_json::json!({
        "task_outcome": "failed",
        "session_id": "s1",
        "task_id": "t1",
        "cwd": "/home/user/zed-patches",
        "project": "zed-patches",
        "thread_title": "Bump the series",
        "task_type": "shell",
        "duration_ms": 1_000u64,
        "description": "DESC-do-not-leak",
        "summary": "SUMMARY-do-not-leak",
        "command": "curl -H 'Authorization: Bearer LEAKME' https://x",
    }))
    .expect("extra fields must not break the parse");

    let mut state = TrayState::default();
    let notification = state.apply_task(task).expect("a failed task outcome notifies");
    let shown = format!(
        "{} {} {} {}",
        notification.summary,
        notification.body,
        state.history()[0].summary,
        state.history()[0].label
    );
    for needle in ["DESC-do-not-leak", "SUMMARY-do-not-leak", "LEAKME", "curl"] {
        assert!(!shown.contains(needle), "{needle} leaked into {shown:?}");
    }
}

// ---- Story 050: Open only for sessions Zeo owns -----------------------------------

fn claimed(kind: EventKind, session: &str, client: Option<&str>) -> crate::protocol::Event {
    let mut event = super::tests_support::event(kind, session, vec![]);
    event.client = client.map(str::to_owned);
    event
}

#[test]
fn a_zeo_session_is_openable_everywhere() {
    let mut state = TrayState::default();
    let notification = state
        .apply(claimed(EventKind::Stop, "z", Some("zeo")))
        .expect("Stop notifies");
    assert!(notification.openable, "the popup offers Open");
    assert!(state.history()[0].openable, "the history row opens it");
    assert!(
        state.sessions().all(|session| session.openable),
        "the session row opens it"
    );
}

#[test]
fn a_session_nobody_claimed_still_notifies_but_cannot_be_opened() {
    let mut state = TrayState::default();
    let notification = state
        .apply(claimed(EventKind::Stop, "cli", None))
        .expect("an unclaimed session still notifies");
    assert!(!notification.openable, "no Open on the popup");
    assert!(!state.history()[0].openable, "the history row is disabled");
    assert!(
        state.sessions().all(|session| !session.openable),
        "the session row is disabled"
    );
}

#[test]
fn an_unknown_client_is_not_zeo() {
    let mut state = TrayState::default();
    let notification = state
        .apply(claimed(EventKind::Stop, "vscode", Some("vscode")))
        .expect("Stop notifies");
    assert!(!notification.openable);
}

#[test]
fn ownership_survives_a_later_unmarked_event() {
    let mut state = TrayState::default();
    state.apply(claimed(EventKind::SessionStart, "z", Some("zeo")));
    let notification = state
        .apply(claimed(EventKind::Stop, "z", None))
        .expect("Stop notifies");
    assert!(notification.openable, "a claimed session stays claimed");
}

#[test]
fn a_task_outcome_is_zeos_by_construction() {
    let mut state = TrayState::default();
    let notification = state
        .apply_task(task_outcome("failed", "s1", "t1", None))
        .expect("a task outcome notifies");
    assert!(notification.openable);
    assert!(state.history()[0].openable);
}

#[test]
fn zeo_only_silences_unclaimed_sessions_but_keeps_the_record() {
    let mut state = TrayState {
        zeo_only: true,
        ..Default::default()
    };
    assert!(
        state.apply(claimed(EventKind::Stop, "cli", None)).is_none(),
        "no popup for a session Zeo did not start"
    );
    assert_eq!(state.history().len(), 1, "the history still records it");
    assert!(
        state
            .apply(claimed(EventKind::Stop, "z", Some("zeo")))
            .is_some(),
        "Zeo's own sessions still notify"
    );
}

#[test]
fn zeo_only_is_read_from_its_environment_variable() {
    assert!(zeo_only_from_env(Some("1")));
    assert!(!zeo_only_from_env(None));
    assert!(!zeo_only_from_env(Some("0")));
}

// A custom opener (`ZEO_SYSTRAY_OPEN_CMD`, e.g. `xdg-open {cwd}`) does not need a
// Zeo thread, so it keeps every session clickable -- but `zeo_only` still asks
// who started the session, not what a click would run.
#[test]
fn a_custom_opener_keeps_every_session_openable() {
    let mut state = TrayState {
        open_any: true,
        ..Default::default()
    };
    let notification = state
        .apply(claimed(EventKind::Stop, "cli", None))
        .expect("Stop notifies");
    assert!(notification.openable);
    assert!(state.sessions().all(|session| session.openable));

    let mut silenced = TrayState {
        open_any: true,
        zeo_only: true,
        ..Default::default()
    };
    assert!(
        silenced
            .apply(claimed(EventKind::Stop, "cli", None))
            .is_none(),
        "zeo_only still silences a session Zeo did not start"
    );
}

// ---- Story 050, round 3: say when a session was started outside Zeo -------------

#[test]
fn an_unclaimed_session_says_it_is_outside_zeo() {
    let mut state = TrayState::default();
    let notification = state
        .apply(claimed(EventKind::Stop, "cli", None))
        .expect("Stop notifies");
    assert!(
        notification.body.ends_with("Outside Zeo — no thread to open."),
        "body was {:?}",
        notification.body
    );
    assert!(state.history()[0].label.ends_with("(outside Zeo)"));
    assert!(
        state
            .sessions()
            .all(|session| session.menu_label().ends_with("(outside Zeo)"))
    );
}

#[test]
fn a_zeo_session_is_not_labelled_outside() {
    let mut state = TrayState::default();
    let notification = state
        .apply(claimed(EventKind::Stop, "z", Some("zeo")))
        .expect("Stop notifies");
    assert!(!notification.body.contains("Outside Zeo"));
    assert!(!state.history()[0].label.contains("outside Zeo"));
    assert!(
        state
            .sessions()
            .all(|session| !session.menu_label().contains("outside Zeo"))
    );
}

#[test]
fn a_custom_opener_still_says_outside_zeo_but_not_that_nothing_opens() {
    let mut state = TrayState {
        open_any: true,
        ..Default::default()
    };
    let notification = state
        .apply(claimed(EventKind::Stop, "cli", None))
        .expect("Stop notifies");
    assert!(
        notification.body.ends_with("Outside Zeo."),
        "body was {:?}",
        notification.body
    );
}
