use super::*;
use casefile_core::{Dataset, GeoPoint, Mo, SolvabilityFacts, StolenItem};
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

fn temp_root() -> PathBuf {
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("casefile-mcp-test-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn mk_case(id: &str, hood: &str, reported_at: i64, premise: Option<&str>, status: CaseStatus) -> Case {
    let mo = premise.map(|p| {
        serde_json::from_value::<Mo>(json!({
            "premise": p, "entry_point": "rear_door", "entry_method": "pry", "time_band": "midday",
            "weekday": true, "property_types": ["laptop"], "signatures": ["ransacked_master_bedroom"]
        }))
        .unwrap()
    });
    Case {
        id: id.into(),
        reported_at,
        window_start: reported_at - 36_000,
        window_end: reported_at - 7_200,
        location: GeoPoint { lat: 47.6, lon: -122.3 },
        block: "4500 BLOCK OF 12TH AVE NE".into(),
        beat: "U1".into(),
        neighborhood: hood.into(),
        narrative: format!("RP states POE via rear slider. UNK SUSP. Case {id}."),
        mo,
        items: vec![StolenItem {
            id: "I-1".into(),
            category: casefile_core::PropertyType::Laptop,
            brand: Some("Zentro".into()),
            model: Some("Z14".into()),
            serial: Some("C02XK1".into()),
            description: "silver laptop".into(),
            markings: vec![],
            value_usd: 900,
        }],
        facts: SolvabilityFacts {
            witness: false,
            suspect_description: false,
            vehicle_description: false,
            video_available: false,
            physical_evidence: false,
            traceable_property: true,
            window_hours: 8.0,
        },
        status,
    }
}

fn server_in(root: &Path) -> CasefileServer {
    let day = 1_793_000_000; // 2026-10
    let ds = Dataset {
        cases: vec![
            mk_case("CC-25-000001", "Ballard", day, Some("residential"), CaseStatus::Open),
            mk_case("CC-25-000002", "Fremont", day + 86_400, Some("commercial"), CaseStatus::Suspended),
            mk_case(
                "CC-25-000003",
                "Ballard",
                day + 2 * 86_400,
                None,
                CaseStatus::ClearedByArrest { arrestee_ref: "A-017".into() },
            ),
        ],
        pawn: vec![],
    };
    let truth = casefile_core::GroundTruth {
        series: vec![vec!["CC-25-000001".into(), "CC-25-000002".into()]],
        ..Default::default()
    };
    let mut store = Store { root: root.to_path_buf(), ds, truth: Some(truth) };
    store.truth = None;
    CasefileServer::with_options(store, root.to_path_buf(), "triage", "")
}

fn parse(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn iso_roundtrip() {
    assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso_utc(1_793_000_000), "2026-10-26T07:33:20Z");
    for s in ["2026-11-05T08:00:00Z", "2024-02-29T23:59:59Z", "2000-01-01T00:00:00Z"] {
        assert_eq!(iso_utc(parse_time(s, false).unwrap()), s);
    }
    assert_eq!(parse_time("2026-11-05", false), parse_time("2026-11-05T00:00", false));
    assert_eq!(parse_time("2026-11-05", true).unwrap() - parse_time("2026-11-05", false).unwrap(), 86_399);
    assert_eq!(parse_time("1793000000", false), Some(1_793_000_000));
    assert_eq!(parse_time("not a date", false), None);
    assert_eq!(parse_time("2026-13-01", false), None);
}

#[test]
fn lists_every_planned_tool_with_schemas_and_no_send_tool() {
    let root = temp_root();
    let s = server_in(&root);
    let tools = s.tool_router.list_all();
    let names: BTreeSet<String> = tools.iter().map(|t| t.name.to_string()).collect();
    let want = [
        "get_case",
        "search_cases",
        "similar_cases",
        "propose_clusters",
        "verify_series",
        "score_solvability",
        "pawn_lookup",
        "check_citations",
        "request_approval",
        "approval_status",
        "draft_victim_update",
        "cost_model",
    ];
    assert_eq!(names, want.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>());
    assert!(names.iter().all(|n| !n.contains("send")), "no send tool may exist");
    for t in &tools {
        let d = t.description.as_deref().unwrap_or("");
        assert!(d.len() > 60, "{} needs an LLM-friendly description", t.name);
        assert_eq!(t.input_schema.get("type").and_then(Value::as_str), Some("object"), "{}", t.name);
    }
    let get_case = tools.iter().find(|t| t.name == "get_case").unwrap();
    assert!(get_case.input_schema.get("properties").unwrap().get("case_id").is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tool_allowlist_hides_other_tools() {
    let root = temp_root();
    let ds = Dataset::default();
    let store = Store { root: root.clone(), ds, truth: None };
    let s = CasefileServer::with_options(store, root.clone(), "skeptic", "verify_series, get_case");
    let names: BTreeSet<String> = s.tool_router.list_all().iter().map(|t| t.name.to_string()).collect();
    assert_eq!(names, ["get_case", "verify_series"].iter().map(|s| s.to_string()).collect());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn get_case_and_errors() {
    let root = temp_root();
    let s = server_in(&root);
    let v = parse(&s.get_case(Parameters(CaseIdParams { case_id: "CC-25-000001".into() })).await.unwrap());
    assert_eq!(v["id"], "CC-25-000001");
    assert_eq!(v["items"][0]["serial"], "C02XK1");
    assert!(v["window_start_iso"].as_str().unwrap().ends_with('Z'));
    assert_eq!(v["mo"]["premise"], "residential");
    let text = v.to_string();
    assert!(!text.contains("ground_truth") && !text.contains("mo_truth"));

    let e = s.get_case(Parameters(CaseIdParams { case_id: "CC-00-nope".into() })).await.unwrap_err();
    assert!(e.contains("unknown case_id") && e.contains("search_cases"), "{e}");
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn search_cases_filters_and_limits() {
    let root = temp_root();
    let s = server_in(&root);
    let run = |p: SearchCasesParams| {
        let s = s.clone();
        async move { s.search_cases(Parameters(p)).await }
    };
    let none = || SearchCasesParams {
        neighborhood: None,
        premise: None,
        since: None,
        until: None,
        status: None,
        limit: None,
    };
    let all = parse(&run(none()).await.unwrap());
    assert_eq!(all["total_matches"], 3);
    assert_eq!(all["cases"][0]["case_id"], "CC-25-000003", "newest first");

    let ballard = parse(&run(SearchCasesParams { neighborhood: Some("ballard".into()), ..none() }).await.unwrap());
    assert_eq!(ballard["total_matches"], 2);
    let res = parse(&run(SearchCasesParams { premise: Some("Residential".into()), ..none() }).await.unwrap());
    assert_eq!(res["total_matches"], 1);
    let cleared = parse(&run(SearchCasesParams { status: Some("cleared".into()), ..none() }).await.unwrap());
    assert_eq!(cleared["cases"][0]["status"], "cleared_by_arrest");
    let limited = parse(&run(SearchCasesParams { limit: Some(1), ..none() }).await.unwrap());
    assert_eq!(limited["returned"], 1);
    assert_eq!(limited["total_matches"], 3);
    let since = parse(&run(SearchCasesParams { since: Some("2026-10-27".into()), ..none() }).await.unwrap());
    assert_eq!(since["total_matches"], 2);

    let bad = run(SearchCasesParams { since: Some("yesterday-ish".into()), ..none() }).await.unwrap_err();
    assert!(bad.contains("cannot parse since"), "{bad}");
    let bad = run(SearchCasesParams { status: Some("pending".into()), ..none() }).await.unwrap_err();
    assert!(bad.contains("status must be"), "{bad}");
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn verify_series_rejects_bad_input_without_panicking() {
    let root = temp_root();
    let s = server_in(&root);
    let one = s.verify_series(Parameters(VerifySeriesParams { case_ids: vec!["CC-25-000001".into()] })).await;
    assert!(one.unwrap_err().contains("at least two"));
    let dup = s
        .verify_series(Parameters(VerifySeriesParams {
            case_ids: vec!["CC-25-000001".into(), "CC-25-000001".into()],
        }))
        .await;
    assert!(dup.unwrap_err().contains("duplicates"));
    let unk = s
        .verify_series(Parameters(VerifySeriesParams { case_ids: vec!["CC-25-000001".into(), "NOPE".into()] }))
        .await;
    assert!(unk.unwrap_err().contains("NOPE"));
    // Valid ids reach the engine. Whether it is implemented or still todo!(), the server must not die.
    let ok = s
        .verify_series(Parameters(VerifySeriesParams {
            case_ids: vec!["CC-25-000001".into(), "CC-25-000002".into()],
        }))
        .await;
    match ok {
        Ok(text) => assert!(parse(&text).get("verdict").is_some()),
        Err(e) => assert!(e.contains("engine failure"), "{e}"),
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn guard_converts_panics_to_errors() {
    let r: Result<u8, String> = guard("demo", || panic!("boom {}", 7));
    let e = r.unwrap_err();
    assert!(e.contains("demo") && e.contains("boom 7"), "{e}");
    let r: Result<u8, String> = guard("demo", || todo!("not yet"));
    assert!(r.unwrap_err().contains("not yet"));
    assert_eq!(guard("ok", || 5).unwrap(), 5);
}

#[tokio::test]
async fn request_approval_is_non_blocking_persisted_and_audited() {
    let root = temp_root();
    let s = server_in(&root);
    let started = std::time::Instant::now();
    let out = s
        .request_approval(Parameters(RequestApprovalParams {
            kind: "series_alert".into(),
            subject: "S-3".into(),
            summary: "Verified 3-case series; request detective assignment.".into(),
            payload: Some(json!({"case_ids": ["CC-25-000001", "CC-25-000002"]})),
        }))
        .await
        .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    let v = parse(&out);
    assert_eq!(v["ticket_id"], "AP-0001");
    assert_eq!(v["status"], "pending");
    assert_eq!(v["escalate_after_minutes"], 120);

    let saved: Vec<Approval> = serde_json::from_slice(&std::fs::read(root.join("data/approvals.json")).unwrap()).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].subject, "S-3");
    assert_eq!(saved[0].requested_by, "agent:triage");
    assert_eq!(saved[0].payload["case_ids"][0], "CC-25-000001");

    let log = AuditLog::open(&root);
    let entries = log.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action, "approval_requested");
    assert_eq!(entries[0].subject, "AP-0001");
    assert_eq!(log.verify(), Ok(1));

    // second ticket increments
    let v2 = parse(
        &s.request_approval(Parameters(RequestApprovalParams {
            kind: "assignment".into(),
            subject: "CC-25-000002".into(),
            summary: "assign".into(),
            payload: None,
        }))
        .await
        .unwrap(),
    );
    assert_eq!(v2["ticket_id"], "AP-0002");

    // status is pending, not escalated
    let st = parse(&s.approval_status(Parameters(ApprovalStatusParams { ticket_id: "AP-0001".into() })).await.unwrap());
    assert_eq!(st["status"], "pending");
    assert_eq!(st["escalated"], false);

    // a human decision recorded in the audit log shows up
    log.append("sgt.demo", "approve", "AP-0001", json!({"reason": "looks right"})).unwrap();
    let st = parse(&s.approval_status(Parameters(ApprovalStatusParams { ticket_id: "AP-0001".into() })).await.unwrap());
    assert_eq!(st["status"], "approved");
    assert_eq!(st["decided_by"], "sgt.demo");
    assert_eq!(st["decision_note"], "looks right");
    assert_eq!(AuditLog::open(&root).verify(), Ok(3));

    let unk = s.approval_status(Parameters(ApprovalStatusParams { ticket_id: "AP-9999".into() })).await;
    assert!(unk.unwrap_err().contains("unknown ticket_id"));
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn overdue_pending_ticket_reports_escalation() {
    let root = temp_root();
    let s = server_in(&root);
    s.request_approval(Parameters(RequestApprovalParams {
        kind: "assignment".into(),
        subject: "CC-25-000001".into(),
        summary: "x".into(),
        payload: None,
    }))
    .await
    .unwrap();
    // age the ticket by 3 hours
    let path = root.join("data/approvals.json");
    let mut rows: Vec<Approval> = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    rows[0].created_ts -= 3 * 3600;
    std::fs::write(&path, serde_json::to_vec(&rows).unwrap()).unwrap();
    let st = parse(&s.approval_status(Parameters(ApprovalStatusParams { ticket_id: "AP-0001".into() })).await.unwrap());
    assert_eq!(st["status"], "pending");
    assert_eq!(st["escalated"], true);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn request_approval_validates_input() {
    let root = temp_root();
    let s = server_in(&root);
    let bad = s
        .request_approval(Parameters(RequestApprovalParams {
            kind: "".into(),
            subject: "x".into(),
            summary: "y".into(),
            payload: None,
        }))
        .await;
    assert!(bad.unwrap_err().contains("kind"));
    assert!(!root.join("data/approvals.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn corrupt_approvals_file_is_not_overwritten() {
    let root = temp_root();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::write(root.join("data/approvals.json"), "{ not json").unwrap();
    let s = server_in(&root);
    let e = s
        .request_approval(Parameters(RequestApprovalParams {
            kind: "other".into(),
            subject: "x".into(),
            summary: "y".into(),
            payload: None,
        }))
        .await
        .unwrap_err();
    assert!(e.contains("refusing to overwrite"), "{e}");
    assert_eq!(std::fs::read_to_string(root.join("data/approvals.json")).unwrap(), "{ not json");
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn draft_victim_update_stores_draft_and_ticket_but_sends_nothing() {
    let root = temp_root();
    let s = server_in(&root);
    let out = s
        .draft_victim_update(Parameters(DraftVictimUpdateParams {
            case_id: "CC-25-000001".into(),
            language: "es".into(),
            message: "Su caso sigue abierto. Le avisaremos de cualquier novedad.".into(),
        }))
        .await
        .unwrap();
    let v = parse(&out);
    assert_eq!(v["status"], "pending_approval");
    assert_eq!(v["draft_id"], "DR-0001");
    assert_eq!(v["ticket_id"], "AP-0001");
    let drafts: Vec<Draft> = serde_json::from_slice(&std::fs::read(root.join("data/drafts.json")).unwrap()).unwrap();
    assert_eq!(drafts[0].case_id, "CC-25-000001");
    assert_eq!(drafts[0].status, "pending_approval");
    let approvals: Vec<Approval> = serde_json::from_slice(&std::fs::read(root.join("data/approvals.json")).unwrap()).unwrap();
    assert_eq!(approvals[0].kind, "victim_message");
    assert_eq!(AuditLog::open(&root).verify(), Ok(2));

    let e = s
        .draft_victim_update(Parameters(DraftVictimUpdateParams {
            case_id: "NOPE".into(),
            language: "es".into(),
            message: "hola".into(),
        }))
        .await
        .unwrap_err();
    assert!(e.contains("unknown case_id"));
    let e = s
        .draft_victim_update(Parameters(DraftVictimUpdateParams {
            case_id: "CC-25-000001".into(),
            language: "es".into(),
            message: "x".repeat(MAX_MESSAGE_CHARS + 1),
        }))
        .await
        .unwrap_err();
    assert!(e.contains("longer than"));
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn cost_model_filters_scenarios() {
    let root = temp_root();
    let s = server_in(&root);
    let all = parse(&s.cost_model(Parameters(CostModelParams { scenario: None })).await.unwrap());
    assert_eq!(all["results"].as_array().unwrap().len(), 3);
    assert_eq!(all["baseline"]["cases_per_year"], 10_000.0);
    let one = parse(&s.cost_model(Parameters(CostModelParams { scenario: Some("base".into()) })).await.unwrap());
    assert_eq!(one["results"].as_array().unwrap().len(), 1);
    let e = s.cost_model(Parameters(CostModelParams { scenario: Some("wild".into()) })).await.unwrap_err();
    assert!(e.contains("Conservative"));
    let _ = std::fs::remove_dir_all(root);
}

/// Full protocol round trip over an in-memory pipe: initialize, tools/list, tools/call (a tool error).
#[tokio::test]
async fn json_rpc_round_trip_over_pipe() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let root = temp_root();
    let server = server_in(&root);
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    let (sr, sw) = tokio::io::split(server_io);
    let task = tokio::spawn(async move {
        let running = server.serve((sr, sw)).await.expect("serve");
        let _ = running.waiting().await;
    });

    let (cr, mut cw) = tokio::io::split(client_io);
    let mut lines = BufReader::new(cr).lines();
    async fn send(w: &mut (impl tokio::io::AsyncWrite + Unpin), v: Value) {
        w.write_all(format!("{v}\n").as_bytes()).await.unwrap();
        w.flush().await.unwrap();
    }
    async fn recv_id(
        lines: &mut tokio::io::Lines<BufReader<impl tokio::io::AsyncRead + Unpin>>,
        id: u64,
    ) -> Value {
        loop {
            let line = tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
                .await
                .expect("timeout")
                .unwrap()
                .expect("eof");
            let v: Value = serde_json::from_str(&line).unwrap();
            if v.get("id") == Some(&json!(id)) {
                return v;
            }
        }
    }

    send(
        &mut cw,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}),
    )
    .await;
    let init = recv_id(&mut lines, 1).await;
    assert_eq!(init["result"]["serverInfo"]["name"], "casefile", "{init}");
    send(&mut cw, json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;

    send(&mut cw, json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).await;
    let list = recv_id(&mut lines, 2).await;
    let n = list["result"]["tools"].as_array().unwrap().len();
    assert_eq!(n, 12, "{list}");

    send(
        &mut cw,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_case","arguments":{"case_id":"NOPE"}}}),
    )
    .await;
    let call = recv_id(&mut lines, 3).await;
    assert_eq!(call["result"]["isError"], true, "{call}");
    assert!(call["result"]["content"][0]["text"].as_str().unwrap().contains("unknown case_id"));

    send(
        &mut cw,
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_case","arguments":{"case_id":"CC-25-000001"}}}),
    )
    .await;
    let call = recv_id(&mut lines, 4).await;
    assert_ne!(call["result"]["isError"], true, "{call}");

    drop(cw);
    drop(lines);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), task).await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn corrupt_audit_cannot_authorize_or_create_ticket() {
    let root = temp_root();
    let s = server_in(&root);
    s.request_approval(Parameters(RequestApprovalParams {
        kind: "assignment".into(), subject: "CC-25-000002".into(),
        summary: "assign".into(), payload: None,
    })).await.unwrap();
    AuditLog::open(&root).append("sgt.demo", "approve", "AP-0001", json!({})).unwrap();
    let audit = root.join("data/audit.jsonl");
    let changed = std::fs::read_to_string(&audit).unwrap().replace("sgt.demo", "forged.actor");
    std::fs::write(&audit, changed).unwrap();
    assert!(s.approval_status(Parameters(ApprovalStatusParams {ticket_id: "AP-0001".into()})).await.is_err());
    let path = root.join("data/approvals.json");
    let before = std::fs::read(&path).unwrap();
    assert!(s.request_approval(Parameters(RequestApprovalParams {
        kind: "assignment".into(), subject: "CC-25-000002".into(),
        summary: "assign again".into(), payload: None,
    })).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
    let _ = std::fs::remove_dir_all(root);
}
