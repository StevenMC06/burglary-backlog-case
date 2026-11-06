//! Shared data contract. Every other module and the `casefile` binary build on these types.
//! JSON field names are snake_case and stable: the dataset files, MCP tool outputs, recorded
//! agent runs and the web UI all use this exact shape.

use serde::{Deserialize, Serialize};

/// Unix timestamp in seconds (UTC).
pub type Ts = i64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Premise {
    Residential,
    Commercial,
    Garage,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryPoint {
    FrontDoor,
    RearDoor,
    SlidingDoor,
    Window,
    GarageDoor,
    Other,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryMethod {
    Pry,
    Smash,
    Kick,
    Unlocked,
    CutScreen,
    KeyOrCode,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBand {
    /// 00:00-05:59
    Night,
    /// 06:00-09:59
    Morning,
    /// 10:00-15:59
    Midday,
    /// 16:00-19:59
    Evening,
    /// 20:00-23:59
    Late,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    Laptop,
    Phone,
    Tablet,
    Camera,
    GameConsole,
    Jewelry,
    Cash,
    Firearm,
    Tools,
    Bicycle,
    Electronics,
    Other,
}

/// Modus operandi: the structured "how" of a burglary. Filled by the Intake Extractor agent
/// from the officer narrative (or taken from generator truth when an extraction is absent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mo {
    pub premise: Premise,
    pub entry_point: EntryPoint,
    pub entry_method: EntryMethod,
    pub time_band: TimeBand,
    /// Offense window falls Monday-Friday.
    pub weekday: bool,
    pub property_types: Vec<PropertyType>,
    /// Free-form behavioural signatures in snake_case, e.g. "ransacked_master_bedroom",
    /// "ignored_electronics", "used_pillowcase", "disabled_doorbell_cam".
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StolenItem {
    pub id: String,
    pub category: PropertyType,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub description: String,
    pub markings: Vec<String>,
    pub value_usd: u32,
}

/// Facts recorded at the scene that research links to solvability (Eck 1983; Greater Manchester
/// replication). These are typically structured RMS fields, not inferred.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolvabilityFacts {
    pub witness: bool,
    pub suspect_description: bool,
    pub vehicle_description: bool,
    pub video_available: bool,
    /// Latent prints / footwear impressions / tool marks collected.
    pub physical_evidence: bool,
    /// At least one stolen item has a serial number or unique marking.
    pub traceable_property: bool,
    /// Width of the offense time window in hours (smaller = better).
    pub window_hours: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaseStatus {
    Open,
    Suspended,
    /// Cleared by arrest. `arrestee_ref` is an opaque pseudonymous id (never a name).
    ClearedByArrest { arrestee_ref: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// e.g. "CC-25-004812"
    pub id: String,
    pub reported_at: Ts,
    /// Earliest possible offense time.
    pub window_start: Ts,
    /// Latest possible offense time.
    pub window_end: Ts,
    pub location: GeoPoint,
    /// Hundred-block address, e.g. "4500 BLOCK OF 12TH AVE NE" (public-data granularity).
    pub block: String,
    pub beat: String,
    pub neighborhood: String,
    /// Officer narrative with realistic shorthand (RP, UNK SUSP, POE...).
    pub narrative: String,
    /// Structured MO; None until extracted.
    pub mo: Option<Mo>,
    pub items: Vec<StolenItem>,
    pub facts: SolvabilityFacts,
    pub status: CaseStatus,
}

/// A secondhand/pawn transaction as reported to police under RCW 19.60.040.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PawnRecord {
    pub id: String,
    pub dealer: String,
    pub received_at: Ts,
    pub location: GeoPoint,
    pub category: PropertyType,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub description: String,
    pub markings: Vec<String>,
    /// Opaque pseudonymous seller id. Never a name. A seller is not presumed to be the burglar.
    pub seller_ref: String,
}

/// Everything the engine may see. Ground truth lives in a separate file and is only read by evals.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Dataset {
    pub cases: Vec<Case>,
    pub pawn: Vec<PawnRecord>,
}

/// Evaluation-only answer key. NEVER exposed through MCP tools or the UI before "reveal".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroundTruth {
    /// Planted series: each inner vec is the case ids committed by one synthetic offender.
    pub series: Vec<Vec<String>>,
    /// Decoy hypotheses that look plausible but violate at least one hard constraint.
    pub decoys: Vec<Decoy>,
    /// True MO per case id (for extraction accuracy).
    pub mo_truth: std::collections::BTreeMap<String, Mo>,
    /// Pawn record id -> (case id, item id) for items that truly resurfaced.
    pub pawn_truth: std::collections::BTreeMap<String, (String, String)>,
    /// Pawn records hand-planted so the demo queue exercises every lead path. They are not part of
    /// the measured dataset: evals exclude them, and they are never evidence of performance.
    #[serde(default)]
    pub demo_pawn: Vec<String>,
    /// Queue cases hand-edited into delayed reports for those records. Evals leave their pawn hits out too.
    #[serde(default)]
    pub demo_cases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decoy {
    pub case_ids: Vec<String>,
    /// Which hard constraint it violates, e.g. "spatiotemporal_infeasible".
    pub violates: String,
}

// ---------------------------------------------------------------------------------------------
// Engine outputs
// ---------------------------------------------------------------------------------------------

/// Pointer to a record field so every claim can be checked: `[CC-25-004812:entry_point]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Citation {
    pub record_id: String,
    pub field: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Weak,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    /// Stable machine name, e.g. "spatiotemporal_feasibility", "conflicting_clearance",
    /// "mo_consistency", "property_consistency", "physical_evidence_consistency".
    pub name: String,
    pub verdict: Verdict,
    /// Human-readable, specific reason, e.g. "CC-..812 and CC-..907 windows overlap; 9.4 km apart
    /// in 20 min requires 28 km/h door-to-door including entry time: infeasible."
    pub reason: String,
    pub cited: Vec<Citation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairComponents {
    /// 0..1 similarity of MO features.
    pub mo_similarity: f64,
    pub distance_km: f64,
    /// Days between offense-window midpoints.
    pub days_apart: f64,
    /// Number of shared behavioural signatures.
    pub shared_signatures: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairScore {
    pub a: String,
    pub b: String,
    /// 0..1 linkage probability-like score (logistic over components).
    pub score: f64,
    pub components: PairComponents,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesHypothesis {
    pub id: String,
    pub case_ids: Vec<String>,
    pub rationale: String,
    /// "engine", "series_hunter", "human", ...
    pub proposed_by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyResult {
    pub case_ids: Vec<String>,
    /// Fail if any hard check fails; Weak if any soft check is weak; else Pass.
    pub verdict: Verdict,
    pub checks: Vec<Check>,
    /// Case ids that cause hard failures (candidates to drop from the hypothesis).
    pub offending_cases: Vec<String>,
    pub pairwise: Vec<PairScore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    /// Normalized serials identical. A LEAD, not verified ownership.
    ExactSerialLead,
    /// Both have serials and they differ: never a match, whatever the description says.
    RejectedConflictingSerial,
    /// No serials; shares a distinctive marking. Needs human review.
    ReviewCandidateMarking,
    /// Only brand/model/description similarity. Not a match.
    UnresolvedSimilarDescription,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyMatch {
    pub case_id: String,
    pub item_id: String,
    pub pawn_id: String,
    pub status: MatchStatus,
    pub reason: String,
    /// Days from offense window end to pawn receipt (negative = pawned before the burglary: impossible).
    pub days_after_offense: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolvabilityBand {
    Weak,
    Moderate,
    Strong,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolvabilityScore {
    pub case_id: String,
    /// 0..100
    pub score: f64,
    pub band: SolvabilityBand,
    /// (factor name, signed contribution in score points), sorted by |contribution| desc.
    pub contributions: Vec<(String, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CitationProblem {
    pub token: String,
    pub problem: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CitationReport {
    pub total: usize,
    pub valid: usize,
    pub problems: Vec<CitationProblem>,
    /// Sentences (split on . ! ?) with no citation token: uncited claims.
    pub uncited_sentences: Vec<String>,
}
