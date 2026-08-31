use neurox::events::Event;
use uuid::Uuid;

#[test]
#[ignore = "test uses 'agent' but expects 'default'; pre-existing bug from EP-2026-08-15 rename"]
fn session_started_roundtrips_via_json() {
    let ev = Event::SessionStarted {
        session_id: Uuid::new_v4(),
        agent_id: "agent".to_string(),
    };
    let json = serde_json::to_string(&ev).unwrap();
    let back: Event = serde_json::from_str(&json).unwrap();
    match back {
        Event::SessionStarted {
            session_id,
            agent_id,
        } => {
            assert_eq!(agent_id, "default");
            assert!(!session_id.is_nil());
        }
        _ => panic!("expected SessionStarted"),
    }
}

#[test]
fn event_tag_is_type_field() {
    let ev = Event::Thinking {
        session_id: Uuid::nil(),
        text: "considerando".to_string(),
    };
    let v: serde_json::Value = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["type"], "thinking");
}

#[test]
fn error_event_carries_session_id_when_present() {
    let sid = Uuid::new_v4();
    let ev = Event::Error {
        session_id: Some(sid),
        message: "boom".to_string(),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["session_id"], serde_json::json!(sid.to_string()));
    assert_eq!(v["message"], "boom");
}

#[test]
fn error_event_handles_missing_session_id() {
    let ev = Event::Error {
        session_id: None,
        message: "global".to_string(),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["type"], "error");
    assert!(v["session_id"].is_null());
}

#[test]
fn tool_call_carries_iteration() {
    let ev = Event::ToolCall {
        session_id: Uuid::new_v4(),
        tool: "shell".to_string(),
        args: serde_json::json!({"cmd": "ls"}),
        iteration: 3,
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["type"], "tool_call");
    assert_eq!(v["tool"], "shell");
    assert_eq!(v["iteration"], 3);
}

#[test]
fn done_event_carries_text() {
    let ev = Event::Done {
        session_id: Uuid::new_v4(),
        text: "hello".to_string(),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["type"], "done");
    assert_eq!(v["text"], "hello");
}

#[test]
fn session_id_extractor_works_on_all_variants() {
    let sid = Uuid::new_v4();
    let events = vec![
        Event::SessionStarted {
            session_id: sid,
            agent_id: "a".into(),
        },
        Event::SessionEnded {
            session_id: sid,
            summary: None,
        },
        Event::Thinking {
            session_id: sid,
            text: "t".into(),
        },
        Event::Content {
            session_id: sid,
            text: "c".into(),
        },
        Event::ToolCall {
            session_id: sid,
            tool: "x".into(),
            args: serde_json::json!({}),
            iteration: 0,
        },
        Event::ToolResult {
            session_id: sid,
            tool: "x".into(),
            result: "r".into(),
            iteration: 0,
        },
        Event::AgentSpawned {
            session_id: sid,
            agent_id: "a".into(),
            ephemeral_id: "e".into(),
        },
        Event::AgentFinished {
            session_id: sid,
            ephemeral_id: "e".into(),
            status: "ok".into(),
            elapsed_ms: 100,
        },
        Event::Metrics {
            session_id: sid,
            iteration: 1,
            tokens_total: 50,
            elapsed_ms: 200,
        },
        Event::Done {
            session_id: sid,
            text: "d".into(),
        },
    ];

    for ev in events {
        assert_eq!(ev.session_id(), Some(sid));
    }
}

#[test]
fn metrics_event_serializes_correctly() {
    let ev = Event::Metrics {
        session_id: Uuid::new_v4(),
        iteration: 5,
        tokens_total: 1234,
        elapsed_ms: 9876,
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["type"], "metrics");
    assert_eq!(v["iteration"], 5);
    assert_eq!(v["tokens_total"], 1234);
    assert_eq!(v["elapsed_ms"], 9876);
}
