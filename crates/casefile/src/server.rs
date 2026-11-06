//! `casefile serve`: the web console. Static `ui/` plus a small JSON API over the real seed data.
//!
//! Every number comes from the deterministic engine in `casefile-core`; recorded agent runs
//! (data/recordings) are layered on top when present, and the UI shows "AI run not recorded"
//! when they are not. Ground truth is never served.
//!
//! Replay note: `/api/replay/{agent}/{id}` returns a paced timeline (`at_ms` per event, scaled by
//! `?speed=`) that the UI plays back; a true SSE body would need a `Stream` implementation and
//! the workspace has no futures crate, so pacing is client-side over a server-built timeline.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path as AxPath, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use casefile_core::{
    Case, MatchStatus, PropertyMatch, SeriesHypothesis, cost, find_case, linkage, property, solvability,
    verify,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::services::ServeDir;

use crate::audit::AuditLog;
use crate::mcp::DEFAULT_CLUSTER_THRESHOLD;
use crate::store::{RecordedRun, Store};

type ApiError = (StatusCode, Json<Value>);
type ApiResult = Result<Json<Value>, ApiError>;

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    (code, Json(json!({ "error": msg.into() })))
}

/// Agents whose recordings the case card shows.
const CASE_AGENTS: [&str; 4] = ["extractor", "triage", "packet_writer", "victim_liaison"];
const MAX_FIELD: usize = 4_000;

pub struct App {
    root: PathBuf,
    store: Store,
    audit: AuditLog,
    audit_lock: Mutex<()>,
    clusters: Vec<SeriesHypothesis>,
    hits: Vec<PropertyMatch>,
    series_of: HashMap<String, String>,
    queue: Vec<String>,
    demo: Value,
}

impl App {
    pub fn build(root: &Path, store: Store, audit: AuditLog) -> Self {
        let clusters = linkage::propose_clusters(&store.ds, DEFAULT_CLUSTER_THRESHOLD);
        let hits = property::all_hits(&store.ds);
        let mut series_of = HashMap::new();
        for h in &clusters {
            for id in &h.case_ids {
                series_of.entry(id.clone()).or_insert_with(|| h.id.clone());
            }
        }
        let demo: Value = std::fs::read(root.join("data/seed/demo.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        let queue: Vec<String> = demo
            .get("queue")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        Self {
            root: root.to_path_buf(),
            store,
            audit,
            audit_lock: Mutex::new(()),
            clusters,
            hits,
            series_of,
            queue,
            demo,
        }
    }

    fn ctx(&self, id: &str) -> solvability::Context {
        solvability::Context {
            linked_to_series: self.series_of.contains_key(id),
            property_hit: self
                .hits
                .iter()
                .any(|m| m.case_id == id && m.status == MatchStatus::ExactSerialLead),
        }
    }

    fn strongest_pawn(&self, id: &str) -> Option<MatchStatus> {
        let rank = |s: MatchStatus| match s {
            MatchStatus::ExactSerialLead => 0,
            MatchStatus::ReviewCandidateMarking => 1,
            MatchStatus::RejectedConflictingSerial => 2,
            MatchStatus::UnresolvedSimilarDescription => 3,
        };
        self.hits.iter().filter(|m| m.case_id == id).map(|m| m.status).min_by_key(|s| rank(*s))
    }
}

fn brief(c: &Case) -> Value {
    json!({
        "id": c.id, "block": c.block, "neighborhood": c.neighborhood, "beat": c.beat,
        "reported_at": c.reported_at, "window_start": c.window_start, "window_end": c.window_end,
        "status": c.status, "location": c.location,
    })
}

// ---------------------------------------------------------------------------------------------
// Pure JSON builders (unit-tested)
// ---------------------------------------------------------------------------------------------

pub fn build_queue(app: &App) -> Value {
    let mut rows: Vec<(f64, Value)> = app
        .queue
        .iter()
        .filter_map(|id| find_case(&app.store.ds, id))
        .map(|c| {
            let s = solvability::score(c, app.ctx(&c.id));
            let narrative: String = c.narrative.chars().take(110).collect();
            (
                s.score,
                json!({
                    "id": c.id, "block": c.block, "neighborhood": c.neighborhood,
                    "reported_at": c.reported_at, "snippet": narrative,
                    "solvability": {"score": s.score, "band": s.band},
                    "flags": {
                        "series_id": app.series_of.get(&c.id),
                        "pawn_hit": app.strongest_pawn(&c.id),
                    },
                }),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1["id"].as_str().cmp(&b.1["id"].as_str())));
    let ai_runs = recorded_agents(&app.root);
    json!({
        "cases": rows.into_iter().map(|r| r.1).collect::<Vec<_>>(),
        "ai_recorded": !ai_runs.is_empty(),
        "recorded_agents": ai_runs,
    })
}

fn recorded_agents(root: &Path) -> Vec<String> {
    let dir = root.join("data/recordings");
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    std::fs::read_dir(e.path())
                        .map(|mut d| d.any(|f| f.is_ok_and(|f| f.path().extension().is_some_and(|x| x == "json"))))
                        .unwrap_or(false)
                })
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn run_json(r: &RecordedRun) -> Value {
    json!({
        "agent": r.agent, "model": r.model, "recorded_at": r.recorded_at, "output": r.output,
        "duration_ms": r.duration_ms, "cost_usd": r.cost_usd,
        "input_tokens": r.input_tokens, "output_tokens": r.output_tokens,
        "events": r.events.len(),
    })
}

const VICTIM_TEMPLATE_ES: &str = "Estimado/a residente: su denuncia {ID} fue recibida y revisada por nuestro equipo. \
Un detective tiene el caso y le avisaremos cuando haya novedades que podamos compartir. \
No podemos dar detalles de la investigacion en curso ni prometer resultados. \
Si recuerda algo nuevo, responda a este mensaje. Gracias por su paciencia.";

/// Deterministic checklist used when no triage run is recorded. Every step is a suggestion.
fn engine_steps(c: &Case, series: Option<&String>, matches: &[PropertyMatch]) -> Vec<Value> {
    let mut v = vec![];
    let mut add = |step: String, why: String, basis: &str, min: u32| {
        v.push(json!({"step": step, "why": why, "lawful_basis_note": basis, "est_minutes": min, "source": "engine"}));
    };
    if let Some(m) = matches.iter().find(|m| m.status == MatchStatus::ExactSerialLead) {
        add(
            format!("Review secondhand record {} against item {}", m.pawn_id, m.item_id),
            format!("Serial numbers match [{}:serial]. A lead, not proof of ownership.", m.case_id),
            "Secondhand dealer reports are filed under RCW 19.60.040; verify with the owner first.",
            30,
        );
    }
    if let Some(s) = series {
        add(
            format!("Open linked-series view {s} and read the verify checks"),
            "Engine links this case to others by MO, place and time.".into(),
            "Internal records review.",
            20,
        );
    }
    if c.facts.video_available {
        add("Collect and review available video".into(), "Video is recorded as available [facts:video_available].".into(), "Obtain with consent or lawful request.", 45);
    }
    if c.facts.physical_evidence {
        add("Send collected physical evidence to the lab queue".into(), "Physical evidence is recorded [facts:physical_evidence].".into(), "Standard chain of custody.", 15);
    }
    if c.facts.witness || c.facts.suspect_description {
        add("Re-contact the witness for a follow-up interview".into(), "A witness or description is on file.".into(), "Voluntary interview.", 40);
    }
    add("Approve the victim status update draft".into(), "Victims should hear back within the week.".into(), "Draft only; a person approves before anything is sent.", 5);
    v
}

pub fn build_case(app: &App, id: &str) -> Option<Value> {
    let c = find_case(&app.store.ds, id)?;
    let sol = solvability::score(c, app.ctx(id));
    let matches = property::match_case(&app.store.ds, id);
    let pawn: Vec<Value> = matches
        .iter()
        .map(|m| {
            let rec = app.store.ds.pawn.iter().find(|p| p.id == m.pawn_id);
            let item = c.items.iter().find(|i| i.id == m.item_id);
            json!({"match": m, "pawn": rec, "item": item})
        })
        .collect();
    let similar: Vec<Value> = linkage::similar_cases(&app.store.ds, id, 5)
        .into_iter()
        .map(|p| {
            let other = if p.a == id { &p.b } else { &p.a };
            json!({"other": find_case(&app.store.ds, other).map(brief), "pair": p})
        })
        .collect();
    let mut recorded = BTreeMap::new();
    for a in CASE_AGENTS {
        recorded.insert(a, app.store.run(a, id).map(|r| run_json(&r)));
    }
    let series = app.series_of.get(id);
    let steps = match recorded.get("triage").and_then(|r| r.as_ref()).and_then(|r| r["output"]["next_steps"].as_array()) {
        Some(a) if !a.is_empty() => a
            .iter()
            .map(|s| {
                let mut s = s.clone();
                s["source"] = json!("triage");
                s
            })
            .collect(),
        _ => engine_steps(c, series, &matches),
    };
    Some(json!({
        "case": c,
        "solvability": sol,
        "pawn": pawn,
        "similar": similar,
        "series_id": series,
        "recorded": recorded,
        "next_steps": steps,
        "victim_template": {"language": "es", "message": VICTIM_TEMPLATE_ES.replace("{ID}", id), "source": "engine_template"},
    }))
}

pub fn build_series(app: &App) -> Value {
    let q: HashSet<&String> = app.queue.iter().collect();
    let mut out: Vec<Value> = vec![];
    let hero_cleared = app.demo["hero_series"]["arrestee_ref"].as_str().zip(app.demo["hero_series"]["cleared_case"].as_str());
    for h in app.clusters.iter().filter(|h| h.case_ids.iter().any(|i| q.contains(i))) {
        out.push(series_json(app, &h.id, &h.case_ids, &h.rationale, &h.proposed_by, hero_cleared));
    }
    if let Some(ids) = app.demo["hero_series"]["case_ids"].as_array() {
        let ids: Vec<String> = ids.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        let covered = out.iter().any(|s| ids.iter().all(|i| s["case_ids"].as_array().is_some_and(|a| a.iter().any(|x| x == i))));
        if !covered && !ids.is_empty() {
            out.insert(
                0,
                series_json(
                    app,
                    "H-HERO",
                    &ids,
                    "Demo hypothesis: the queue case plus its earlier cases, submitted to the verifier. Engine clusters need extracted MOs for queue cases.",
                    "demo hypothesis",
                    hero_cleared,
                ),
            );
        }
    }
    if let Some(ids) = app.demo["hero_decoy"]["case_ids"].as_array() {
        let ids: Vec<String> = ids.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        if !ids.is_empty() {
            out.push(series_json(
                app,
                "H-DECOY",
                &ids,
                "Plausible-looking grouping submitted for checking. The verifier tests it.",
                "demo hypothesis",
                None,
            ));
        }
    }
    json!({"series": out, "ai_recorded": !Store::runs_in(&app.root, "series_hunter").is_empty()})
}

fn series_json(
    app: &App,
    id: &str,
    ids: &[String],
    rationale: &str,
    by: &str,
    cleared: Option<(&str, &str)>,
) -> Value {
    let v = verify::verify_series(&app.store.ds, ids);
    let cases: Vec<Value> = ids.iter().filter_map(|i| find_case(&app.store.ds, i)).map(brief).collect();
    let find = |agent: &str| ids.iter().find_map(|i| app.store.run(agent, i)).map(|r| run_json(&r));
    let cleared_case = cleared
        .filter(|(_, c)| ids.iter().any(|i| i == c))
        .map(|(a, c)| json!({"case_id": c, "arrestee_ref": a}));
    json!({
        "id": id, "case_ids": ids, "rationale": rationale, "proposed_by": by,
        "in_queue": ids.iter().filter(|i| app.queue.contains(i)).collect::<Vec<_>>(),
        "verify": v, "cases": cases, "hunter": find("series_hunter"), "skeptic": find("skeptic"),
        "cleared": cleared_case,
    })
}

pub fn build_kpi() -> Value {
    let scenarios = cost::scenarios();
    let results: Vec<_> = scenarios.iter().map(cost::compute).collect();
    json!({
        "baseline": {
            "cases": cost::baseline::CASES_PER_YEAR, "annual_labor": cost::baseline::ANNUAL_LABOR,
            "cost_target": cost::baseline::COST_TARGET,
            "clearance_target_additional": cost::baseline::CLEARANCE_TARGET_ADDITIONAL,
        },
        "scenarios": scenarios,
        "results": results,
        "note": "Modelled estimates from stated assumptions, not measured outcomes. Targets are shown as met or missed as computed.",
    })
}

pub fn build_points(ds: &casefile_core::Dataset, ids: &str) -> Value {
    let pts: Vec<Value> = ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(200)
        .filter_map(|i| find_case(ds, i))
        .map(|c| json!({"id": c.id, "lat": c.location.lat, "lon": c.location.lon, "block": c.block,
                        "window_start": c.window_start, "window_end": c.window_end}))
        .collect();
    json!({"points": pts})
}

/// Only ids made of these characters may reach the filesystem.
pub fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn replay_timeline(run: &RecordedRun, speed: f64) -> Value {
    let speed = if speed.is_finite() { speed.clamp(0.1, 20.0) } else { 1.0 };
    let mut items: Vec<Value> = vec![];
    let clip = |s: &str| -> String { s.chars().take(400).collect() };
    for e in &run.events {
        let content = e.pointer("/message/content").and_then(Value::as_array);
        match (e.get("type").and_then(Value::as_str), content) {
            (Some("assistant"), Some(parts)) => {
                for p in parts {
                    match p.get("type").and_then(Value::as_str) {
                        Some("text") => items.push(json!({"kind": "text", "text": clip(p["text"].as_str().unwrap_or(""))})),
                        Some("tool_use") => {
                            let name = p["name"].as_str().unwrap_or("tool");
                            let name = name.rsplit("__").next().unwrap_or(name);
                            items.push(json!({"kind": "tool_call", "name": name, "input": clip(&p["input"].to_string())}));
                        }
                        _ => {}
                    }
                }
            }
            (Some("user"), Some(parts)) => {
                for p in parts {
                    if p.get("type").and_then(Value::as_str) == Some("tool_result") {
                        let t = match &p["content"] {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        items.push(json!({"kind": "tool_result", "text": clip(&t)}));
                    }
                }
            }
            _ => {}
        }
    }
    items.push(json!({"kind": "final", "text": "Structured output validated against the agent schema."}));
    for (n, it) in items.iter_mut().enumerate() {
        it["at_ms"] = json!((n as f64 * 700.0 / speed).round() as u64);
    }
    json!({"agent": run.agent, "input_id": run.input_id, "model": run.model, "speed": speed,
           "events": items, "output": run.output})
}

/// 7 agents and the tool channels they use, read from agents/*/agent.json.
pub fn build_team(root: &Path) -> Value {
    let mut agents: Vec<Value> = vec![];
    if let Ok(rd) = std::fs::read_dir(root.join("agents")) {
        let mut dirs: Vec<_> = rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());
        for d in dirs {
            let name = d.file_name().to_string_lossy().to_string();
            let Some(def) = std::fs::read(d.path().join("agent.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            else {
                continue;
            };
            agents.push(json!({
                "name": name, "model": def["model"], "tools": def["allowed_tools"], "input": def["input"],
            }));
        }
    }
    let mut tools: Vec<String> = agents
        .iter()
        .flat_map(|a| a["tools"].as_array().cloned().unwrap_or_default())
        .filter_map(|t| t.as_str().map(String::from))
        .collect();
    tools.sort();
    tools.dedup();
    let tool_rows: Vec<Value> = tools
        .iter()
        .map(|t| {
            let by: Vec<&str> = agents
                .iter()
                .filter(|a| a["tools"].as_array().is_some_and(|v| v.iter().any(|x| x == t)))
                .filter_map(|a| a["name"].as_str())
                .collect();
            json!({"tool": t, "agents": by})
        })
        .collect();
    json!({
        "agents": agents,
        "tools": tool_rows,
        "channels": [
            {"id": "victim", "label": "Victim channel", "via": "draft_victim_update", "drafted_by": ["victim_liaison"],
             "human_approval": true, "note": "Drafts only. Nothing is sent without a person approving."},
            {"id": "patrol", "label": "Patrol channel", "via": "request_approval", "drafted_by": ["series_hunter", "skeptic"],
             "human_approval": true, "note": "Series alerts are drafts until a sergeant approves."},
        ],
    })
}

pub fn build_audit(log: &AuditLog) -> Value {
    let entries = match log.try_entries() {
        Ok(entries) => entries,
        Err(e) => return json!({"entries": [], "chain": {"intact": false, "error": e.to_string()}}),
    };
    let chain = match log.verify() {
        Ok(n) => json!({"intact": true, "entries": n}),
        Err(seq) => json!({"intact": false, "entries": entries.len(), "first_bad_seq": seq}),
    };
    json!({"entries": entries, "chain": chain})
}

#[derive(Deserialize)]
pub struct DecisionIn {
    actor: String,
    action: String,
    subject: String,
    #[serde(default)]
    detail: Value,
}

/// Match the exact recommendation shown by the console; ambiguous duplicates fail closed.
fn canonical_step(steps: &[Value], detail: &Value) -> Result<Value, String> {
    let text = detail["step"].as_str().filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "proposed step required".to_string())?;
    let source = detail["source"].as_str().filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "step source required".to_string())?;
    let mut matches = steps.iter().filter(|s| s["step"] == text && s["source"] == source);
    let step = matches.next().ok_or_else(|| "step/source does not match a current recommendation".to_string())?;
    if matches.next().is_some() { return Err("ambiguous recommendation".into()); }
    Ok(step.clone())
}

/// Validate against server-owned records, never the caller's supplied verdict.
fn validate_decision(app: &App, d: &mut DecisionIn) -> Result<(), String> {
    match d.action.as_str() {
        "approve_series_alert" => {
            let all = build_series(app);
            let series = all["series"].as_array().and_then(|rows| rows.iter().find(|s| s["id"] == d.subject))
                .ok_or_else(|| "unknown series subject".to_string())?;
            if series["verify"]["verdict"] == "fail" {
                return Err("cannot approve a failed series".into());
            }
            // Audit the facts actually approved, not caller-supplied membership/verdict.
            if !d.detail.is_object() { d.detail = json!({}); }
            d.detail["case_ids"] = series["case_ids"].clone();
            d.detail["verdict"] = series["verify"]["verdict"].clone();
        }
        "accept_step" | "reject_step" | "approve_victim_update" => {
            if find_case(&app.store.ds, &d.subject).is_none() {
                return Err("unknown case subject".into());
            }
            if d.action == "accept_step" || d.action == "reject_step" {
                let case = build_case(app, &d.subject).ok_or_else(|| "unknown case subject".to_string())?;
                let steps = case["next_steps"].as_array().ok_or_else(|| "recommendations unavailable".to_string())?;
                let matched = canonical_step(steps, &d.detail)?;
                d.detail["recommendation"] = matched;
            }
            if d.action == "reject_step" {
                let reason = d.detail["reason"].as_str().unwrap_or("");
                if !["Not lawful", "Not available", "Too resource-intensive", "Other"].contains(&reason) {
                    return Err("recognized rejection reason required".into());
                }
                if reason == "Other" && d.detail["note"].as_str().unwrap_or("").trim().is_empty() {
                    return Err("explanation required for Other".into());
                }
            }
        }
        _ => return Err("unknown decision action".into()),
    }
    Ok(())
}

pub fn record_decision(log: &AuditLog, d: DecisionIn) -> Result<Value, String> {
    for (n, v) in [("actor", &d.actor), ("action", &d.action), ("subject", &d.subject)] {
        if v.trim().is_empty() {
            return Err(format!("{n} must not be empty"));
        }
        if v.len() > 200 {
            return Err(format!("{n} is too long"));
        }
    }
    if d.detail.to_string().len() > MAX_FIELD {
        return Err("detail is too long".into());
    }
    let e = log.append(d.actor.trim(), d.action.trim(), d.subject.trim(), d.detail).map_err(|e| format!("{e:#}"))?;
    Ok(json!({"entry": e}))
}

// ---------------------------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------------------------

type Shared = State<Arc<App>>;

async fn h_queue(State(a): Shared) -> ApiResult {
    Ok(Json(build_queue(&a)))
}

async fn h_case(State(a): Shared, AxPath(id): AxPath<String>) -> ApiResult {
    build_case(&a, &id).map(Json).ok_or_else(|| err(StatusCode::NOT_FOUND, format!("unknown case {id}")))
}

async fn h_series(State(a): Shared) -> ApiResult {
    Ok(Json(build_series(&a)))
}

#[derive(Deserialize)]
struct ReplayQ {
    speed: Option<f64>,
}

async fn h_replay(
    State(a): Shared,
    AxPath((agent, id)): AxPath<(String, String)>,
    Query(q): Query<ReplayQ>,
) -> ApiResult {
    if !safe_id(&agent) || !safe_id(&id) {
        return Err(err(StatusCode::BAD_REQUEST, "invalid agent or id"));
    }
    let run = a.store.run(&agent, &id).ok_or_else(|| err(StatusCode::NOT_FOUND, "AI run not recorded"))?;
    Ok(Json(replay_timeline(&run, q.speed.unwrap_or(1.0))))
}

async fn h_decision(State(a): Shared, Json(mut d): Json<DecisionIn>) -> ApiResult {
    let _g = a.audit_lock.lock().unwrap_or_else(|e| e.into_inner());
    validate_decision(&a, &mut d).map_err(|m| err(StatusCode::BAD_REQUEST, m))?;
    record_decision(&a.audit, d).map(Json).map_err(|m| err(StatusCode::BAD_REQUEST, m))
}

async fn h_audit(State(a): Shared) -> ApiResult {
    Ok(Json(build_audit(&a.audit)))
}

async fn h_kpi() -> ApiResult {
    Ok(Json(build_kpi()))
}

#[derive(Deserialize)]
struct PointsQ {
    ids: Option<String>,
}

async fn h_points(State(a): Shared, Query(q): Query<PointsQ>) -> ApiResult {
    Ok(Json(build_points(&a.store.ds, q.ids.as_deref().unwrap_or(""))))
}

async fn h_team(State(a): Shared) -> ApiResult {
    Ok(Json(build_team(&a.root)))
}

pub fn router(app: Arc<App>) -> Router {
    let ui = app.root.join("ui");
    Router::new()
        .route("/api/queue", get(h_queue))
        .route("/api/case/{id}", get(h_case))
        .route("/api/series", get(h_series))
        .route("/api/replay/{agent}/{id}", get(h_replay))
        .route("/api/decision", post(h_decision))
        .route("/api/audit", get(h_audit))
        .route("/api/kpi", get(h_kpi))
        .route("/api/points", get(h_points))
        .route("/api/team", get(h_team))
        .fallback_service(ServeDir::new(ui))
        .with_state(app)
}

/// Entries a hosted (`--public`) demo keeps before refusing more.
const PUBLIC_AUDIT_CAP: usize = 200;

/// Listen address: `--host`, else `0.0.0.0` when `--public`/`CASEFILE_PUBLIC=1`, else loopback.
/// Port: `--port`, else `$PORT` (set by Render, Fly, Railway, Heroku), else 8080.
pub fn listen_addr(args: &[String]) -> anyhow::Result<(String, u16, bool)> {
    let public = crate::has(args, "--public") || std::env::var("CASEFILE_PUBLIC").is_ok_and(|v| v == "1");
    let port_s = crate::flag(args, "--port").or_else(|| std::env::var("PORT").ok());
    let port: u16 = match port_s {
        Some(p) => p.parse().map_err(|_| anyhow::anyhow!("port must be a number, got '{p}'"))?,
        None => 8080,
    };
    let host = crate::flag(args, "--host").unwrap_or_else(|| if public { "0.0.0.0".into() } else { "127.0.0.1".into() });
    Ok((host, port, public))
}

pub async fn run(args: &[String]) -> anyhow::Result<()> {
    let (host, port, public) = listen_addr(args)?;
    let root = crate::root();
    let store = Store::load(&root)?;
    let audit = if public {
        // Public demo: audit entries go to a throwaway directory, capped, and reset on every start,
        // so visitors never touch the repo's data/audit.jsonl and cannot grow it without bound.
        let dir = std::env::temp_dir().join("casefile-public-demo");
        let _ = std::fs::remove_dir_all(&dir);
        AuditLog::open(&dir).with_cap(PUBLIC_AUDIT_CAP)
    } else {
        AuditLog::open(&root)
    };
    let app = Arc::new(App::build(&root, store, audit));
    let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await?;
    eprintln!("casefile console: http://{host}:{port}/  (synthetic data{})", if public { ", public mode" } else { "" });
    axum::serve(listener, router(app)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn app() -> &'static App {
        static APP: OnceLock<App> = OnceLock::new();
        APP.get_or_init(|| {
            let root = repo_root();
            let store = Store::load(&root).expect("seed data");
            let tmp = std::env::temp_dir().join(format!("casefile-server-{}", std::process::id()));
            App::build(&root, store, AuditLog::open(&tmp))
        })
    }

    #[test]
    fn step_match_rejects_missing_or_duplicate_recommendations() {
        let step = json!({"step":"Review source", "source":"engine", "why":"evidence"});
        let detail = json!({"step":"Review source", "source":"engine"});
        assert_eq!(canonical_step(std::slice::from_ref(&step), &detail).unwrap(), step);
        assert!(canonical_step(&[step.clone(), step], &detail).is_err());
        assert!(canonical_step(&[], &detail).is_err());
        assert!(canonical_step(&[], &json!({"step":"Review source"})).is_err());
    }

    #[tokio::test]
    async fn backend_decisions_reject_bypass_without_append() {
        let root = repo_root();
        let dir = std::env::temp_dir().join(format!("casefile-gate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let a = Arc::new(App::build(&root, Store::load(&root).unwrap(), AuditLog::open(&dir)));
        let case_id = a.queue[0].clone();
        let decision = |action: &str, subject: &str, detail: Value| DecisionIn {
            actor: "det.demo".into(), action: action.into(), subject: subject.into(), detail,
        };
        let case = build_case(&a, &case_id).unwrap();
        let step = case["next_steps"][0].clone();
        let detail = |extra: Value| {
            let mut result = extra;
            result["step"] = step["step"].clone();
            result["source"] = step["source"].clone();
            result
        };
        for d in [
            decision("accept_step", &case_id, json!({})),
            decision("accept_step", &case_id, json!({"step":"invented", "source":"engine"})),
            decision("accept_step", &case_id, json!({"step":step["step"], "source":"wrong"})),
            decision("approve_series_alert", "H-DECOY", json!({"verdict":"pass", "case_ids":[]})),
            decision("approve_series_alert", "NO-SERIES", json!({})),
            decision("accept_step", "NO-CASE", json!({})),
            decision("made_up", &case_id, json!({})),
            decision("reject_step", &case_id, json!({})),
            decision("reject_step", &case_id, detail(json!({"reason":"invalid"}))),
            decision("reject_step", &case_id, detail(json!({"reason":"Other", "note":"  "}))),
        ] {
            let bytes = std::fs::read(dir.join("data/audit.jsonl")).ok();
            let failure = h_decision(State(a.clone()), Json(d)).await.unwrap_err();
            assert_eq!(failure.0, StatusCode::BAD_REQUEST);
            assert_eq!(a.audit.verify(), Ok(0));
            assert_eq!(std::fs::read(dir.join("data/audit.jsonl")).ok(), bytes);
        }
        for (index, d) in [
            decision("accept_step", &case_id, detail(json!({"recommendation":"forged"}))),
            decision("reject_step", &case_id, detail(json!({"reason":"Not lawful"}))),
            decision("reject_step", &case_id, detail(json!({"reason":"Other", "note":"source unavailable"}))),
            decision("approve_victim_update", &case_id, json!({})),
        ].into_iter().enumerate() {
            assert!(h_decision(State(a.clone()), Json(d)).await.is_ok());
            assert_eq!(a.audit.verify(), Ok(index + 1));
            if index < 3 {
                assert_eq!(a.audit.try_entries().unwrap().last().unwrap().detail["recommendation"], step);
            }
        }
        let series = build_series(&a);
        let valid = series["series"].as_array().unwrap().iter().find(|row| row["verify"]["verdict"] != "fail").unwrap();
        let before = a.audit.verify().unwrap();
        assert!(h_decision(State(a.clone()), Json(decision("approve_series_alert", valid["id"].as_str().unwrap(), json!({"verdict":"fail"})))).await.is_ok());
        assert_eq!(a.audit.verify(), Ok(before + 1));
        let entries = a.audit.try_entries().unwrap();
        let last = entries.last().unwrap();
        assert_eq!(last.detail["case_ids"], valid["case_ids"]);
        assert_eq!(last.detail["verdict"], valid["verify"]["verdict"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn queue_is_sorted_and_flagged() {
        let q = build_queue(app());
        let cases = q["cases"].as_array().unwrap();
        assert_eq!(cases.len(), app().queue.len());
        assert!(cases.len() > 10);
        let s: Vec<f64> = cases.iter().map(|c| c["solvability"]["score"].as_f64().unwrap()).collect();
        assert!(s.windows(2).all(|w| w[0] >= w[1]));
        let hero = app().demo["hero_pawn"]["case_id"].as_str().unwrap();
        let row = cases.iter().find(|c| c["id"] == hero).unwrap();
        assert_eq!(row["flags"]["pawn_hit"], "exact_serial_lead");
    }

    #[test]
    fn case_detail_has_exact_lead_and_trap() {
        let a = app();
        let hero = a.demo["hero_pawn"]["case_id"].as_str().unwrap();
        let v = build_case(a, hero).unwrap();
        let pawn = v["pawn"].as_array().unwrap();
        let st = |id: &str| pawn.iter().find(|p| p["match"]["pawn_id"] == id).map(|p| p["match"]["status"].clone());
        assert_eq!(st(a.demo["hero_pawn"]["pawn_id"].as_str().unwrap()), Some(json!("exact_serial_lead")));
        assert_eq!(
            st(a.demo["hero_pawn"]["trap_pawn_id"].as_str().unwrap()),
            Some(json!("rejected_conflicting_serial"))
        );
        assert!(v["recorded"].as_object().unwrap().contains_key("triage"));
        assert!(!v["next_steps"].as_array().unwrap().is_empty());
        assert_eq!(v["victim_template"]["language"], "es");
        assert!(v["similar"].as_array().unwrap().len() <= 5);
        assert!(build_case(a, "CC-NOPE").is_none());
    }

    #[test]
    fn series_includes_hero_and_decoy_failure() {
        let a = app();
        let s = build_series(a);
        let list = s["series"].as_array().unwrap();
        let hero_q = a.demo["hero_series"]["queue_case"].as_str().unwrap();
        let hero = list.iter().find(|x| x["case_ids"].as_array().unwrap().iter().any(|i| i == hero_q)).unwrap();
        assert!(!hero["verify"]["checks"].as_array().unwrap().is_empty());
        let decoy = list.iter().find(|x| x["id"] == "H-DECOY").unwrap();
        assert_ne!(decoy["verify"]["verdict"], "pass");
        assert!(!decoy["verify"]["checks"].as_array().unwrap().is_empty());
    }

    #[test]
    fn kpi_reports_all_scenarios_and_target_flags() {
        let k = build_kpi();
        let r = k["results"].as_array().unwrap();
        assert_eq!(r.len(), k["scenarios"].as_array().unwrap().len());
        assert!(r.iter().all(|x| x["meets_cost_target"].is_boolean() && x["steady_net_pct"].is_number()));
    }

    #[test]
    fn points_and_team() {
        let p = build_points(&app().store.ds, &format!("{},bogus", app().queue[0]));
        assert_eq!(p["points"].as_array().unwrap().len(), 1);
        let t = build_team(&repo_root());
        assert_eq!(t["agents"].as_array().unwrap().len(), 7);
        let gc = t["tools"].as_array().unwrap().iter().find(|x| x["tool"] == "get_case").unwrap();
        assert!(gc["agents"].as_array().unwrap().len() >= 3);
        assert_eq!(t["channels"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn decision_is_audited_and_validated() {
        let dir = std::env::temp_dir().join(format!("casefile-dec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let log = AuditLog::open(&dir);
        let ok = DecisionIn {
            actor: "det.demo".into(),
            action: "reject_step".into(),
            subject: "CC-1".into(),
            detail: json!({"reason": "Not lawful"}),
        };
        assert!(record_decision(&log, ok).is_ok());
        let bad = DecisionIn { actor: " ".into(), action: "x".into(), subject: "y".into(), detail: Value::Null };
        assert!(record_decision(&log, bad).is_err());
        let a = build_audit(&log);
        assert_eq!(a["chain"]["intact"], true);
        assert_eq!(a["chain"]["entries"], 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replay_timeline_paces_and_summarizes() {
        let run = RecordedRun {
            agent: "triage".into(),
            input_id: "CC-1".into(),
            model: "sonnet".into(),
            recorded_at: "t".into(),
            events: vec![
                json!({"type": "system"}),
                json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "mcp__casefile__get_case", "input": {"case_id": "CC-1"}}]}}),
                json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": "ok"}]}}),
            ],
            output: json!({}),
            duration_ms: 1,
            cost_usd: 0.0,
            input_tokens: 0,
            output_tokens: 0,
        };
        let t = replay_timeline(&run, 2.0);
        let ev = t["events"].as_array().unwrap();
        assert_eq!(ev[0]["name"], "get_case");
        assert_eq!(ev[1]["kind"], "tool_result");
        assert_eq!(ev[1]["at_ms"], 350);
        assert!(safe_id("CC-26-001") && !safe_id("../x") && !safe_id(""));
    }
}
