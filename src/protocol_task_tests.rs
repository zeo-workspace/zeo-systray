// Story 013 / task 1.1 — the task-outcome datagram (R5.1, R5.5, R5.8, Q12).
//
// Included at the end of `mod tests` in `src/protocol.rs`:
//
//     include!("protocol_task_tests.rs");
//
// so it shares that module's `use super::*`. Every name runs under the
// `cargo test protocol` filter through the module path.

/// The shared contract fixture: Zeo's patch 0034 carries a byte-identical copy
/// and serialises its datagram against it.
const TASK_OUTCOME_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/task-outcome.json"
));

/// The fixture as a JSON object, so a test can add or remove one field.
fn task_outcome_value() -> serde_json::Map<String, serde_json::Value> {
    match serde_json::from_str::<serde_json::Value>(TASK_OUTCOME_FIXTURE) {
        Ok(serde_json::Value::Object(map)) => map,
        other => panic!("the fixture must be a JSON object, got {other:?}"),
    }
}

fn parse_datagram(value: serde_json::Map<String, serde_json::Value>) -> serde_json::Result<Datagram> {
    serde_json::from_value::<Datagram>(serde_json::Value::Object(value))
}

/// The wire word for an outcome: exhaustive, so a fifth variant fails to compile here.
fn outcome_word(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Completed => "completed",
        Outcome::Failed => "failed",
        Outcome::Stopped => "stopped",
        Outcome::Interrupted => "interrupted",
    }
}

// ---- R5.8 — hostile halves first ------------------------------------------------

/// Wrongly collapse: a task outcome that also carries an event's `kind` is still a
/// task outcome — here even with every field an `Event` requires, so only the
/// variant order (`Task` first) decides it. `kind` alone must not pull it onto the `Event` variant, where it
/// would change a session's status instead of raising a task notification.
#[test]
fn a_task_outcome_carrying_a_stray_kind_is_still_a_task() {
    let mut value = task_outcome_value();
    value.insert("kind".into(), serde_json::json!("Stop"));
    value.insert("background".into(), serde_json::json!([]));
    assert!(
        matches!(parse_datagram(value), Ok(Datagram::Task(_))),
        "a payload with `task_outcome` and a stray `kind` must land on Datagram::Task"
    );
}

/// Wrongly split: the legacy `Event` datagram, byte for byte what a `notify` built
/// before this story sends, still parses as an `Event` — adding a third shape must
/// not turn an old sender's datagram into a failed parse or a task.
#[test]
fn a_legacy_event_still_parses_as_an_event_beside_the_task_shape() {
    let event = r#"{"kind":"Stop","session_id":"s","cwd":"/p","project":"p",
                    "title":null,"agent_type":null,"message":null,"background":[]}"#;
    assert!(matches!(
        serde_json::from_str::<Datagram>(event),
        Ok(Datagram::Event(_))
    ));
    // An event with every optional field set is no closer to a task.
    let full = r#"{"kind":"Notification","session_id":"s","cwd":"/p","project":"p",
                   "title":"t","agent_type":"Explore","message":"hello",
                   "background":[{"kind":"shell","status":"running"}]}"#;
    assert!(matches!(
        serde_json::from_str::<Datagram>(full),
        Ok(Datagram::Event(_))
    ));
    // And the usage shape is not claimed by the Task variant tried before it.
    let usage = r#"{"usage":[{"key":"five_hour","label":"5-hour",
                    "percent":12.0,"resets_at":1800000000}]}"#;
    assert!(matches!(
        serde_json::from_str::<Datagram>(usage),
        Ok(Datagram::Usage(_))
    ));
}

/// Near-identical to a task outcome but without its discriminating field: it is
/// neither shape, and must be rejected rather than guessed into one.
#[test]
fn a_task_shape_without_task_outcome_is_rejected() {
    let mut value = task_outcome_value();
    value.remove("task_outcome");
    assert!(parse_datagram(value).is_err());
}

/// An outcome word nobody defined is rejected, not mapped onto a known one.
#[test]
fn an_unknown_outcome_word_is_rejected() {
    let mut value = task_outcome_value();
    value.insert("task_outcome".into(), serde_json::json!("exploded"));
    assert!(parse_datagram(value).is_err());
}

/// All three shapes land on their own variant (benign half).
#[test]
fn datagrams_of_all_three_shapes_are_told_apart() {
    let event = r#"{"kind":"Stop","session_id":"s","cwd":"/p","project":"p",
                    "title":null,"agent_type":null,"message":null,"background":[]}"#;
    let usage = r#"{"usage":[{"key":"five_hour","label":"5-hour",
                    "percent":12.0,"resets_at":1800000000}]}"#;
    assert!(matches!(
        serde_json::from_str::<Datagram>(event),
        Ok(Datagram::Event(_))
    ));
    assert!(matches!(
        serde_json::from_str::<Datagram>(usage),
        Ok(Datagram::Usage(_))
    ));
    assert!(matches!(
        serde_json::from_str::<Datagram>(TASK_OUTCOME_FIXTURE),
        Ok(Datagram::Task(_))
    ));
}

// ---- R5.1 / Q12 — the shared fixture --------------------------------------------

#[test]
fn the_shared_fixture_parses_into_every_task_outcome_field() {
    let Ok(Datagram::Task(task)) = serde_json::from_str::<Datagram>(TASK_OUTCOME_FIXTURE) else {
        panic!("the fixture must parse as Datagram::Task");
    };
    assert_eq!(outcome_word(&task.task_outcome), "failed");
    assert_eq!(task.session_id, "sess-013");
    assert_eq!(task.task_id, "task-7");
    assert_eq!(task.cwd, "/home/user/zed-patches");
    assert_eq!(task.project, "zed-patches");
    assert_eq!(task.thread_title.as_deref(), Some("Bump the series"));
    assert_eq!(task.task_type, "shell");
    assert_eq!(task.duration_ms, 20_500);
}

#[test]
fn every_outcome_word_parses_to_its_own_outcome() {
    for word in ["completed", "failed", "stopped", "interrupted"] {
        let mut value = task_outcome_value();
        value.insert("task_outcome".into(), serde_json::json!(word));
        let Ok(Datagram::Task(task)) = parse_datagram(value) else {
            panic!("`{word}` must parse as a task outcome");
        };
        assert_eq!(outcome_word(&task.task_outcome), word);
    }
}

/// `thread_title` is `string | null`: a thread with no title yet still notifies.
#[test]
fn a_null_thread_title_is_accepted() {
    let mut value = task_outcome_value();
    value.insert("thread_title".into(), serde_json::Value::Null);
    let Ok(Datagram::Task(task)) = parse_datagram(value) else {
        panic!("a null thread_title must still parse");
    };
    assert_eq!(task.thread_title, None);
}

// ---- R5.5 — the privacy boundary ----------------------------------------------

/// A sender that oversteps and adds free text must not have it kept: the parsed
/// task outcome has no home for a description, summary or command line, so none
/// of it survives into anything the daemon can show or store.
#[test]
fn free_text_on_a_task_outcome_is_not_kept() {
    let mut value = task_outcome_value();
    value.insert("description".into(), serde_json::json!("DESC-do-not-leak"));
    value.insert("summary".into(), serde_json::json!("SUMMARY-do-not-leak"));
    value.insert(
        "command".into(),
        serde_json::json!("curl -H 'Authorization: Bearer LEAKME' https://x"),
    );
    let Ok(Datagram::Task(task)) = parse_datagram(value) else {
        panic!("extra fields must not break the parse");
    };
    let kept = format!("{task:?}");
    assert!(!kept.contains("DESC-do-not-leak"));
    assert!(!kept.contains("SUMMARY-do-not-leak"));
    assert!(!kept.contains("LEAKME"));
}
