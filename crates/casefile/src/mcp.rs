//! `casefile mcp`: the connector layer. An MCP (Model Context Protocol) server over stdio that gives
//! agents read access to the synthetic case dataset and the deterministic engine, plus two
//! write-paths that are deliberately human-gated: `request_approval` and `draft_victim_update`.
//!
//! Design rules (docs/BUILD_PLAN.md, docs/GUY_NOTES.md):
//! - Ground truth is never exposed: it is dropped right after the store loads.
//! - The engine, not the model, does every comparison/score/check. The tools are thin wrappers.
//! - `request_approval` never blocks: it returns a ticket at once (the "approval MCP" idea); a human
//!   decides later and the ticket escalates after `escalate_after_minutes` (120).
//! - There is no tool that sends anything to a victim. Drafts wait for a human.
//! - Engine functions run under `catch_unwind`, so a bug or an unfinished engine function becomes an
//!   MCP tool error, never a dead server. Logging goes to stderr only (stdout is the protocol).
//! - `CASEFILE_TOOLS=a,b,c` restricts which tools are listed (per-agent least privilege, set by
//!   `casefile record`); `CASEFILE_AGENT=<name>` labels audit entries.

use std::collections::{BTreeSet, HashMap};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use casefile_core::{
    Case, CaseStatus, MatchStatus, SeriesHypothesis, citations, cost, find_case, linkage, property,
    solvability, verify,
};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::audit::AuditLog;
use crate::store::Store;

/// Minutes before an unanswered approval ticket escalates to the next approver.
pub const ESCALATE_AFTER_MINUTES: u32 = 120;
/// Cluster threshold used when the caller gives none (also for the solvability context).
pub const DEFAULT_CLUSTER_THRESHOLD: f64 = 0.7;
const MAX_SEARCH_LIMIT: usize = 50;
const MAX_SIMILAR_K: usize = 10;
const MAX_CLUSTERS_RETURNED: usize = 60;
const MAX_PAYLOAD_BYTES: usize = 20_000;
const MAX_MESSAGE_CHARS: usize = 2_000;

// ---------------------------------------------------------------------------------------------
// Time helpers (UTC, no chrono). Shared with record.rs.
// ---------------------------------------------------------------------------------------------

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Unix seconds -> `2026-11-05T08:00:00Z`.
pub fn iso_utc(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", secs / 3600, (secs % 3600) / 60, secs % 60)
}

/// Accepts unix seconds, `YYYY-MM-DD` (start of day, or end of day when `end_of_day`), or
/// `YYYY-MM-DDTHH:MM[:SS][Z]`.
pub fn parse_time(s: &str, end_of_day: bool) -> Option<i64> {
    let s = s.trim();
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && s.len() > 8 {
        return s.parse().ok();
    }
    let s = s.trim_end_matches('Z');
    let (date, time) = match s.split_once(['T', ' ']) {
        Some((d, t)) => (d, Some(t)),
        None => (s, None),
    };
    let mut dp = date.split('-');
    let y: i64 = dp.next()?.parse().ok()?;
    let m: i64 = dp.next()?.parse().ok()?;
    let d: i64 = dp.next()?.parse().ok()?;
    if dp.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let mut secs = days_from_civil(y, m, d) * 86_400;
    match time {
        None => {
            if end_of_day {
                secs += 86_399;
            }
        }
        Some(t) => {
            let mut tp = t.split(':');
            let h: i64 = tp.next()?.parse().ok()?;
            let mi: i64 = tp.next().unwrap_or("0").parse().ok()?;
            let se: i64 = tp.next().unwrap_or("0").parse().ok()?;
            if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..61).contains(&se) {
                return None;
            }
            secs += h * 3600 + mi * 60 + se;
        }
    }
    Some(secs)
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Parameter types (JSON-schema'd; the doc comments become the schema descriptions the LLM reads)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaseIdParams {
    /// Case id exactly as it appears in the data, e.g. "CC-25-004812".
    pub case_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchCasesParams {
    /// Neighborhood name, case-insensitive substring match (e.g. "Ballard").
    pub neighborhood: Option<String>,
    /// Premise type: one of residential, commercial, garage, storage. Only cases whose MO has been
    /// extracted can match.
    pub premise: Option<String>,
    /// Only cases reported at or after this time. ISO date or datetime (2026-11-05, 2026-11-05T08:00Z).
    pub since: Option<String>,
    /// Only cases reported at or before this time. A bare date includes the whole day.
    pub until: Option<String>,
    /// open, suspended or cleared (cleared by arrest).
    pub status: Option<String>,
    /// Maximum rows to return (default 20, maximum 50). Newest first.
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SimilarCasesParams {
    /// The case to compare against all other cases.
    pub case_id: String,
    /// How many neighbours to return (default 5, maximum 10).
    pub k: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProposeClustersParams {
    /// Edge threshold on the 0..1 pair score (default 0.7). Higher is stricter and returns fewer,
    /// tighter groups.
    pub threshold: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VerifySeriesParams {
    /// Two or more case ids that you hypothesise were committed by the same offender.
    pub case_ids: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckCitationsParams {
    /// The full text to check. Citation tokens look like [CC-25-004812:entry_point] or
    /// [CC-25-004812:items[I-2].serial=ABC123].
    pub text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RequestApprovalParams {
    /// What kind of decision this is: assignment, series_alert, victim_message, packet_release or other.
    pub kind: String,
    /// The case id or series id the decision is about (e.g. "CC-25-004812" or "S-3").
    pub subject: String,
    /// One or two plain sentences for the human approver: what you propose and why.
    pub summary: String,
    /// Structured details to attach (case ids, citations, draft id...). Any JSON object.
    pub payload: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovalStatusParams {
    /// The ticket id returned by request_approval, e.g. "AP-0003".
    pub ticket_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DraftVictimUpdateParams {
    /// The case this update is about.
    pub case_id: String,
    /// Language of the message: an ISO code or name ("es", "Spanish", "en").
    pub language: String,
    /// The message text. Calm, plain language. No suspect information, no promises.
    pub message: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CostModelParams {
    /// Scenario name: Conservative, Base or Optimistic. Omit to get all three.
    pub scenario: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Persisted records
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub ticket_id: String,
    pub kind: String,
    pub subject: String,
    pub summary: String,
    pub payload: Value,
    /// pending | approved | rejected
    pub status: String,
    pub requested_by: String,
    pub created_at: String,
    pub created_ts: i64,
    pub escalate_after_minutes: u32,
    pub escalate_at: String,
    #[serde(default)]
    pub decided_by: Option<String>,
    #[serde(default)]
    pub decided_at: Option<String>,
    #[serde(default)]
    pub decision_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub draft_id: String,
    pub case_id: String,
    pub language: String,
    pub message: String,
    /// Always pending_approval until a human acts. Nothing is ever sent from here.
    pub status: String,
    pub ticket_id: String,
    pub created_at: String,
    pub created_by: String,
}

fn read_json_list<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, String> {
    match std::fs::read(path) {
        Ok(b) if b.iter().all(|c| c.is_ascii_whitespace()) => Ok(vec![]),
        Ok(b) => serde_json::from_slice(&b)
            .map_err(|e| format!("{} is not valid JSON ({e}); refusing to overwrite it", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

fn write_json_atomic<T: Serialize>(path: &Path, rows: &[T]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(rows).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, body).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------------------------

struct Inner {
    store: Store,
    root: PathBuf,
    actor: String,
    /// Serialises read-modify-write of approvals.json / drafts.json inside this process.
    files: Mutex<()>,
    clusters: Mutex<HashMap<u64, Arc<Vec<SeriesHypothesis>>>>,
}

#[derive(Clone)]
pub struct CasefileServer {
    inner: Arc<Inner>,
    tool_router: ToolRouter<Self>,
}

/// Run an engine call; turn a panic (bug, or an engine function still `todo!()`) into an error.
fn guard<T>(what: &str, f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        let msg = p
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        format!("engine failure in {what}: {msg}")
    })
}

fn to_text(v: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(v).map_err(|e| format!("cannot serialise result: {e}"))
}

fn status_kind(c: &Case) -> &'static str {
    match c.status {
        CaseStatus::Open => "open",
        CaseStatus::Suspended => "suspended",
        CaseStatus::ClearedByArrest { .. } => "cleared_by_arrest",
    }
}

fn snake<T: Serialize>(v: &T) -> Option<String> {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string))
}

fn excerpt(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        out.push_str("...");
    }
    out
}

fn case_summary(c: &Case) -> Value {
    json!({
        "case_id": c.id,
        "reported_at": iso_utc(c.reported_at),
        "block": c.block,
        "neighborhood": c.neighborhood,
        "status": status_kind(c),
        "premise": c.mo.as_ref().and_then(|m| snake(&m.premise)),
        "entry_point": c.mo.as_ref().and_then(|m| snake(&m.entry_point)),
        "item_count": c.items.len(),
        "narrative_excerpt": excerpt(&c.narrative, 160),
    })
}

fn case_full(c: &Case) -> Result<Value, String> {
    let mut v = serde_json::to_value(c).map_err(|e| e.to_string())?;
    if let Some(o) = v.as_object_mut() {
        o.insert("reported_at_iso".into(), iso_utc(c.reported_at).into());
        o.insert("window_start_iso".into(), iso_utc(c.window_start).into());
        o.insert("window_end_iso".into(), iso_utc(c.window_end).into());
        o.insert("mo_extracted".into(), c.mo.is_some().into());
        o.insert(
            "note".into(),
            "Times are UTC. Cite fields as [case_id:field], e.g. [CC-25-004812:mo.entry_point] or [CC-25-004812:items[I-1].serial]."
                .into(),
        );
    }
    Ok(v)
}

impl CasefileServer {
    pub fn new(store: Store, root: PathBuf) -> Self {
        let agent = std::env::var("CASEFILE_AGENT").unwrap_or_default();
        let tools = std::env::var("CASEFILE_TOOLS").unwrap_or_default();
        Self::with_options(store, root, &agent, &tools)
    }

    /// `agent`: label for audit entries ("" = generic). `tools`: comma list of tool names to expose
    /// ("" = all).
    pub fn with_options(store: Store, root: PathBuf, agent: &str, tools: &str) -> Self {
        let actor = match agent.trim() {
            "" => "agent:mcp".to_string(),
            a => format!("agent:{a}"),
        };
        let mut router = Self::tool_router();
        let allowed: BTreeSet<String> =
            tools.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if !allowed.is_empty() {
            for t in router.list_all() {
                if !allowed.contains(t.name.as_ref()) {
                    router.disable_route(t.name.to_string());
                }
            }
        }
        Self {
            inner: Arc::new(Inner {
                store,
                root,
                actor,
                files: Mutex::new(()),
                clusters: Mutex::new(HashMap::new()),
            }),
            tool_router: router,
        }
    }

    fn ds(&self) -> &casefile_core::Dataset {
        &self.inner.store.ds
    }

    fn need_case(&self, id: &str) -> Result<&Case, String> {
        find_case(self.ds(), id.trim()).ok_or_else(|| {
            format!("unknown case_id '{id}'. Case ids look like CC-25-004812; use search_cases to find one.")
        })
    }

    fn clusters(&self, threshold: f64) -> Result<Arc<Vec<SeriesHypothesis>>, String> {
        let key = threshold.to_bits();
        {
            let cache = self.inner.clusters.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(c) = cache.get(&key) {
                return Ok(c.clone());
            }
        }
        let computed = Arc::new(guard("propose_clusters", || linkage::propose_clusters(self.ds(), threshold))?);
        let mut cache = self.inner.clusters.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(key, computed.clone());
        Ok(computed)
    }

    fn lock_files(&self) -> std::sync::MutexGuard<'_, ()> {
        self.inner.files.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn approvals_path(&self) -> PathBuf {
        self.inner.root.join("data/approvals.json")
    }

    fn drafts_path(&self) -> PathBuf {
        self.inner.root.join("data/drafts.json")
    }

    /// Create a pending ticket, persist it and append to the audit log. Never waits for a human.
    fn create_ticket(
        &self,
        kind: &str,
        subject: &str,
        summary: &str,
        payload: Value,
    ) -> Result<Approval, String> {
        let _g = self.lock_files();
        let path = self.approvals_path();
        let mut rows: Vec<Approval> = read_json_list(&path)?;
        let next = rows
            .iter()
            .filter_map(|a| a.ticket_id.strip_prefix("AP-").and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0)
            + 1;
        let now = now_ts();
        let ticket = Approval {
            ticket_id: format!("AP-{next:04}"),
            kind: kind.to_string(),
            subject: subject.to_string(),
            summary: summary.to_string(),
            payload,
            status: "pending".into(),
            requested_by: self.inner.actor.clone(),
            created_at: iso_utc(now),
            created_ts: now,
            escalate_after_minutes: ESCALATE_AFTER_MINUTES,
            escalate_at: iso_utc(now + i64::from(ESCALATE_AFTER_MINUTES) * 60),
            decided_by: None,
            decided_at: None,
            decision_note: None,
        };
        rows.push(ticket.clone());
        write_json_atomic(&path, &rows)?;
        AuditLog::open(&self.inner.root)
            .append(
                &self.inner.actor,
                "approval_requested",
                &ticket.ticket_id,
                json!({"kind": kind, "subject": subject, "summary": summary,
                       "escalate_after_minutes": ESCALATE_AFTER_MINUTES}),
            )
            .map_err(|e| format!("audit log write failed: {e}"))?;
        Ok(ticket)
    }
}

#[tool_router]
impl CasefileServer {
    #[tool(
        description = "Fetch one burglary case: ids, UTC times, block address, neighborhood, officer narrative, extracted MO (null if not yet extracted), stolen items with serials, scene facts (witness, video, physical evidence) and status. Use this first to ground any statement about a case; cite fields as [case_id:field].",
        annotations(read_only_hint = true)
    )]
    async fn get_case(&self, Parameters(p): Parameters<CaseIdParams>) -> Result<String, String> {
        let c = self.need_case(&p.case_id)?;
        to_text(&case_full(c)?)
    }

    #[tool(
        description = "List case summaries filtered by neighborhood, premise, date range and status, newest first (max 50). Returns case_id, time, block, status, premise, entry point and a narrative excerpt. Use get_case for the full record.",
        annotations(read_only_hint = true)
    )]
    async fn search_cases(&self, Parameters(p): Parameters<SearchCasesParams>) -> Result<String, String> {
        let since = match p.since.as_deref() {
            Some(s) => Some(parse_time(s, false).ok_or_else(|| format!("cannot parse since '{s}'; use YYYY-MM-DD"))?),
            None => None,
        };
        let until = match p.until.as_deref() {
            Some(s) => Some(parse_time(s, true).ok_or_else(|| format!("cannot parse until '{s}'; use YYYY-MM-DD"))?),
            None => None,
        };
        let status = p.status.as_deref().map(|s| s.trim().to_lowercase());
        if let Some(s) = &status
            && !["open", "suspended", "cleared", "cleared_by_arrest"].contains(&s.as_str())
        {
            return Err(format!("status must be open, suspended or cleared, got '{s}'"));
        }
        let premise = p.premise.as_deref().map(|s| s.trim().to_lowercase());
        let hood = p.neighborhood.as_deref().map(|s| s.trim().to_lowercase());
        let limit = p.limit.unwrap_or(20).clamp(1, MAX_SEARCH_LIMIT);

        let mut hits: Vec<&Case> = self
            .ds()
            .cases
            .iter()
            .filter(|c| since.is_none_or(|t| c.reported_at >= t))
            .filter(|c| until.is_none_or(|t| c.reported_at <= t))
            .filter(|c| hood.as_ref().is_none_or(|h| c.neighborhood.to_lowercase().contains(h)))
            .filter(|c| {
                premise
                    .as_ref()
                    .is_none_or(|p| c.mo.as_ref().and_then(|m| snake(&m.premise)).as_deref() == Some(p.as_str()))
            })
            .filter(|c| match status.as_deref() {
                None => true,
                Some("cleared" | "cleared_by_arrest") => status_kind(c) == "cleared_by_arrest",
                Some(s) => status_kind(c) == s,
            })
            .collect();
        hits.sort_by(|a, b| b.reported_at.cmp(&a.reported_at).then_with(|| a.id.cmp(&b.id)));
        let total = hits.len();
        let rows: Vec<Value> = hits.into_iter().take(limit).map(case_summary).collect();
        to_text(&json!({"total_matches": total, "returned": rows.len(), "cases": rows}))
    }

    #[tool(
        description = "Top-k most similar other cases to a given case, ranked by the engine's pair score (MO agreement + distance + time gap + shared signatures). Each row has the score and its components. Similar is not the same offender: run verify_series before claiming a link.",
        annotations(read_only_hint = true)
    )]
    async fn similar_cases(&self, Parameters(p): Parameters<SimilarCasesParams>) -> Result<String, String> {
        let c = self.need_case(&p.case_id)?;
        let k = p.k.unwrap_or(5).clamp(1, MAX_SIMILAR_K);
        let pairs = guard("similar_cases", || linkage::similar_cases(self.ds(), &c.id, k))?;
        let rows: Vec<Value> = pairs
            .iter()
            .map(|ps| {
                let other_id = if ps.a == c.id { &ps.b } else { &ps.a };
                let summary = find_case(self.ds(), other_id).map(case_summary);
                json!({"other_case_id": other_id, "score": ps.score, "components": ps.components, "other": summary})
            })
            .collect();
        to_text(&json!({"case_id": c.id, "k": k, "similar": rows}))
    }

    #[tool(
        description = "Engine-proposed candidate series: groups of cases linked by pair score at or above the threshold (connected components, size >= 2). These are hypotheses only. Pass promising groups to verify_series before relying on them. Output is capped to the 60 largest groups.",
        annotations(read_only_hint = true)
    )]
    async fn propose_clusters(&self, Parameters(p): Parameters<ProposeClustersParams>) -> Result<String, String> {
        let t = p.threshold.unwrap_or(DEFAULT_CLUSTER_THRESHOLD);
        if !(0.0..=1.0).contains(&t) {
            return Err(format!("threshold must be between 0 and 1, got {t}"));
        }
        let all = self.clusters(t)?;
        let mut sorted: Vec<&SeriesHypothesis> = all.iter().collect();
        sorted.sort_by(|a, b| b.case_ids.len().cmp(&a.case_ids.len()).then_with(|| a.id.cmp(&b.id)));
        let shown: Vec<&&SeriesHypothesis> = sorted.iter().take(MAX_CLUSTERS_RETURNED).collect();
        to_text(&json!({
            "threshold": t,
            "total_clusters": all.len(),
            "returned": shown.len(),
            "clusters": shown,
        }))
    }

    #[tool(
        description = "The verifier (the game engine): check a hypothesised series against hard facts. Runs spatiotemporal feasibility (can one offender physically do all offenses), conflicting-clearance (cases cleared to different arrestees), MO and property consistency. Returns an overall verdict (pass/weak/fail), each check with a reason and cited fields, offending_cases to drop, and pairwise scores. Always call this before stating that cases are linked.",
        annotations(read_only_hint = true)
    )]
    async fn verify_series(&self, Parameters(p): Parameters<VerifySeriesParams>) -> Result<String, String> {
        let ids: Vec<String> = p.case_ids.iter().map(|s| s.trim().to_string()).collect();
        if ids.len() < 2 {
            return Err("verify_series needs at least two case_ids".into());
        }
        let unique: BTreeSet<&String> = ids.iter().collect();
        if unique.len() != ids.len() {
            return Err("case_ids contains duplicates".into());
        }
        let unknown: Vec<&String> = ids.iter().filter(|i| find_case(self.ds(), i).is_none()).collect();
        if !unknown.is_empty() {
            return Err(format!("unknown case_ids: {}", unknown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
        }
        let r = guard("verify_series", || verify::verify_series(self.ds(), &ids))?;
        to_text(&r)
    }

    #[tool(
        description = "Explainable solvability score (0-100, band weak/moderate/strong) for a case from a logistic model over scene facts, with each factor's signed contribution in points. Context (series link, property hit) is computed by the engine from clusters and pawn matching and is echoed back. Use it to support a triage disposition; do not compute scores yourself.",
        annotations(read_only_hint = true)
    )]
    async fn score_solvability(&self, Parameters(p): Parameters<CaseIdParams>) -> Result<String, String> {
        let c = self.need_case(&p.case_id)?;
        let clusters = self.clusters(DEFAULT_CLUSTER_THRESHOLD)?;
        let linked = clusters.iter().any(|h| h.case_ids.contains(&c.id));
        let matches = guard("pawn matching", || property::match_case(self.ds(), &c.id))?;
        let hit = matches.iter().any(|m| m.status == MatchStatus::ExactSerialLead);
        let ctx = solvability::Context { linked_to_series: linked, property_hit: hit };
        let s = guard("score_solvability", || solvability::score(c, ctx))?;
        to_text(&json!({
            "solvability": s,
            "context": {
                "linked_to_series": linked,
                "series_threshold": DEFAULT_CLUSTER_THRESHOLD,
                "property_hit": hit,
                "property_hit_means": "at least one exact serial lead against the pawn ledger",
            }
        }))
    }

    #[tool(
        description = "Match a case's stolen items against pawn/secondhand records (RCW 19.60.040 reports). Each match has a status: exact_serial_lead (identical normalised serial: a LEAD, not proof of ownership), rejected_conflicting_serial (never a match), review_candidate_marking (shared marking, needs a human), unresolved_similar_description (not a match). Includes pawn record details (dealer, date, seller_ref is opaque and a seller is not presumed to be the burglar).",
        annotations(read_only_hint = true)
    )]
    async fn pawn_lookup(&self, Parameters(p): Parameters<CaseIdParams>) -> Result<String, String> {
        let c = self.need_case(&p.case_id)?;
        let matches = guard("pawn_lookup", || property::match_case(self.ds(), &c.id))?;
        let rows: Vec<Value> = matches
            .iter()
            .map(|m| {
                let pawn = self.ds().pawn.iter().find(|r| r.id == m.pawn_id).map(|r| {
                    json!({
                        "pawn_id": r.id, "dealer": r.dealer, "received_at": iso_utc(r.received_at),
                        "category": r.category, "brand": r.brand, "model": r.model, "serial": r.serial,
                        "description": r.description, "markings": r.markings, "seller_ref": r.seller_ref,
                    })
                });
                json!({"match": m, "pawn_record": pawn})
            })
            .collect();
        to_text(&json!({"case_id": c.id, "match_count": rows.len(), "matches": rows}))
    }

    #[tool(
        description = "Check every citation token [RECORD_ID:field] (optionally [RECORD_ID:field=value]) in a text against the dataset. Reports invalid tokens with the precise problem and lists uncited factual sentences. Call it on your draft and fix every problem before you finish.",
        annotations(read_only_hint = true)
    )]
    async fn check_citations(&self, Parameters(p): Parameters<CheckCitationsParams>) -> Result<String, String> {
        if p.text.trim().is_empty() {
            return Err("text is empty".into());
        }
        let r = guard("check_citations", || citations::check(self.ds(), &p.text))?;
        to_text(&json!({"ok": r.problems.is_empty() && r.uncited_sentences.is_empty(), "report": r}))
    }

    #[tool(
        description = "Ask a human to approve something (an assignment, a series alert, a victim message). NON-BLOCKING: returns a ticket immediately with status pending; the human decides later. The ticket escalates to the next approver after escalate_after_minutes (120). Do not wait: finish your own work and report the ticket id. Recorded in the tamper-evident audit log."
    )]
    async fn request_approval(&self, Parameters(p): Parameters<RequestApprovalParams>) -> Result<String, String> {
        let kind = p.kind.trim();
        let subject = p.subject.trim();
        let summary = p.summary.trim();
        if kind.is_empty() || kind.len() > 64 {
            return Err("kind must be 1-64 characters (assignment, series_alert, victim_message, packet_release, other)".into());
        }
        if subject.is_empty() || subject.len() > 128 {
            return Err("subject must be 1-128 characters".into());
        }
        if summary.is_empty() || summary.len() > 1000 {
            return Err("summary must be 1-1000 characters".into());
        }
        let payload = p.payload.unwrap_or(Value::Null);
        if payload.to_string().len() > MAX_PAYLOAD_BYTES {
            return Err(format!("payload is larger than {MAX_PAYLOAD_BYTES} bytes; send ids and citations, not full records"));
        }
        let t = self.create_ticket(kind, subject, summary, payload)?;
        to_text(&json!({
            "ticket_id": t.ticket_id,
            "status": "pending",
            "escalate_after_minutes": t.escalate_after_minutes,
            "escalate_at": t.escalate_at,
            "message": "Queued for a human. This call did not wait. Continue your work; poll approval_status later if you need the outcome.",
        }))
    }

    #[tool(
        description = "Look up an approval ticket created by request_approval: pending, approved or rejected, with who decided and when. A pending ticket older than its escalation time reports escalated=true (it has gone to the next approver). Non-blocking; poll it, do not loop on it.",
        annotations(read_only_hint = true)
    )]
    async fn approval_status(&self, Parameters(p): Parameters<ApprovalStatusParams>) -> Result<String, String> {
        let id = p.ticket_id.trim();
        let rows: Vec<Approval> = {
            let _g = self.lock_files();
            read_json_list(&self.approvals_path())?
        };
        let mut t = rows
            .into_iter()
            .find(|a| a.ticket_id.eq_ignore_ascii_case(id))
            .ok_or_else(|| format!("unknown ticket_id '{id}'"))?;
        // A decision may have been recorded through the audit log (web console) rather than the file.
        if t.status == "pending" {
            let decision = AuditLog::open(&self.inner.root)
                .entries()
                .into_iter()
                .filter(|e| e.subject == t.ticket_id)
                .rfind(|e| {
                    let a = e.action.to_lowercase();
                    a.starts_with("approve") || a.starts_with("reject") || a == "deny" || a == "denied"
                });
            if let Some(e) = decision {
                let a = e.action.to_lowercase();
                t.status = if a.starts_with("approve") { "approved" } else { "rejected" }.into();
                t.decided_by = Some(e.actor.clone());
                t.decided_at = Some(e.ts.clone());
                t.decision_note = e.detail.get("reason").and_then(Value::as_str).map(str::to_string);
            }
        }
        let now = now_ts();
        let overdue = t.status == "pending" && now >= t.created_ts + i64::from(t.escalate_after_minutes) * 60;
        to_text(&json!({
            "ticket_id": t.ticket_id,
            "status": t.status,
            "kind": t.kind,
            "subject": t.subject,
            "created_at": t.created_at,
            "escalate_at": t.escalate_at,
            "escalated": overdue,
            "minutes_pending": if t.status == "pending" { Some((now - t.created_ts).max(0) / 60) } else { None },
            "decided_by": t.decided_by,
            "decided_at": t.decided_at,
            "decision_note": t.decision_note,
        }))
    }

    #[tool(
        description = "Store a draft status update for a victim, in their language, pending human approval. There is NO send tool: nothing leaves the system. Creates an approval ticket so a person reviews it. Keep it calm and plain; no suspect information, no promises of arrest or recovery, no internal details."
    )]
    async fn draft_victim_update(&self, Parameters(p): Parameters<DraftVictimUpdateParams>) -> Result<String, String> {
        let c = self.need_case(&p.case_id)?;
        let language = p.language.trim();
        let message = p.message.trim();
        if language.is_empty() || language.len() > 40 {
            return Err("language must be 1-40 characters, e.g. 'es' or 'Spanish'".into());
        }
        if message.is_empty() {
            return Err("message is empty".into());
        }
        if message.chars().count() > MAX_MESSAGE_CHARS {
            return Err(format!("message is longer than {MAX_MESSAGE_CHARS} characters; shorten it"));
        }
        let ticket = self.create_ticket(
            "victim_message",
            &c.id,
            &format!("Victim update draft ({language}) for {}", c.id),
            json!({"case_id": c.id, "language": language, "message": message}),
        )?;
        let _g = self.lock_files();
        let path = self.drafts_path();
        let mut rows: Vec<Draft> = read_json_list(&path)?;
        let next = rows
            .iter()
            .filter_map(|d| d.draft_id.strip_prefix("DR-").and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0)
            + 1;
        let draft = Draft {
            draft_id: format!("DR-{next:04}"),
            case_id: c.id.clone(),
            language: language.to_string(),
            message: message.to_string(),
            status: "pending_approval".into(),
            ticket_id: ticket.ticket_id.clone(),
            created_at: iso_utc(now_ts()),
            created_by: self.inner.actor.clone(),
        };
        rows.push(draft.clone());
        write_json_atomic(&path, &rows)?;
        AuditLog::open(&self.inner.root)
            .append(
                &self.inner.actor,
                "victim_draft_created",
                &draft.draft_id,
                json!({"case_id": c.id, "language": language, "ticket_id": ticket.ticket_id}),
            )
            .map_err(|e| format!("audit log write failed: {e}"))?;
        to_text(&json!({
            "draft_id": draft.draft_id,
            "ticket_id": ticket.ticket_id,
            "status": "pending_approval",
            "escalate_after_minutes": ESCALATE_AFTER_MINUTES,
            "message": "Draft stored. It has NOT been sent and cannot be sent by an agent; a person must approve it.",
        }))
    }

    #[tool(
        description = "Run the deterministic cost and clearance model against the official baseline (10,000 burglary cases/year, 13% clearance, $3.05M annual labor). Returns steady-state net savings, year-one ROI, payback, additional clearances and whether the 15% cost target and +325 clearance target are met, per scenario. Quote these numbers; never estimate them yourself.",
        annotations(read_only_hint = true)
    )]
    async fn cost_model(&self, Parameters(p): Parameters<CostModelParams>) -> Result<String, String> {
        let all = cost::scenarios();
        let chosen: Vec<_> = match p.scenario.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => all,
            Some(name) => {
                let found: Vec<_> = all.iter().filter(|s| s.name.eq_ignore_ascii_case(name)).cloned().collect();
                if found.is_empty() {
                    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
                    return Err(format!("unknown scenario '{name}'; choose one of: {}", names.join(", ")));
                }
                found
            }
        };
        let results: Vec<_> = guard("cost_model", || chosen.iter().map(cost::compute).collect::<Vec<_>>())?;
        to_text(&json!({
            "baseline": {
                "cases_per_year": cost::baseline::CASES_PER_YEAR,
                "clearance_rate": cost::baseline::CLEARANCE_RATE,
                "annual_labor": cost::baseline::ANNUAL_LABOR,
                "cost_target": cost::baseline::COST_TARGET,
                "clearance_target_additional": cost::baseline::CLEARANCE_TARGET_ADDITIONAL,
            },
            "results": results,
        }))
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "casefile",
    version = "0.1.0",
    instructions = "Casefile connector for the fictional Cascade City burglary backlog (synthetic data). Read tools expose cases, linkage, the verifier, solvability, pawn matching and the cost model. Cite every claim as [CASE_ID:field]. request_approval is non-blocking and draft_victim_update only stores a draft: humans make every decision and nothing is sent."
)]
impl ServerHandler for CasefileServer {}

/// Entry point for `casefile mcp`.
pub async fn run(_args: &[String]) -> anyhow::Result<()> {
    let root = crate::root();
    let mut store = Store::load(&root).map_err(|e| {
        anyhow::anyhow!("casefile mcp: cannot load dataset from {} (set CASEFILE_ROOT): {e:#}", root.display())
    })?;
    store.truth = None; // ground truth must never be reachable from a tool
    eprintln!(
        "casefile mcp: {} cases, {} pawn records, root {}",
        store.ds.cases.len(),
        store.ds.pawn.len(),
        root.display()
    );
    let server = CasefileServer::new(store, root).serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests;
