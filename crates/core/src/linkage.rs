//! Pairwise case linkage (MO + space + time) and cluster proposals.
//!
//! SPEC (implementer fills bodies, keep signatures):
//! - `mo_similarity`: weighted agreement over premise, entry_point, entry_method, time_band, weekday
//!   (exact match each), Jaccard over property_types, Jaccard over signatures. Weights must be named
//!   consts. Unknown values never count as agreement. Missing MO on either side => 0.0.
//! - `pair_score`: logistic over [mo_similarity, distance_km, days_apart, shared_signatures] with
//!   named, documented coefficients (literature: burglary linkage logistic models reach AUC > .9 on
//!   MO + inter-crime distance + temporal proximity; Tonkin/Woodhams; Leicester comparison).
//!   Must be symmetric: pair_score(a,b).score == pair_score(b,a).score.
//! - `similar_cases`: top-k other cases by score, descending, ties broken by case id.
//! - `propose_clusters`: build graph with edges score >= threshold, return connected components of
//!   size >= 2 as SeriesHypothesis (proposed_by = "engine", id = "S-<n>" stable ordering by
//!   smallest case id). Deterministic.
//!
//! # How the model works (and why it is explainable)
//!
//! The pair score is a hand-set logistic model, `score = sigmoid(z)` with
//!
//! ```text
//! z = INTERCEPT
//!   + W_MO   * mo_similarity                      (0..1)
//!   + W_DIST * ln(1 + distance_km)                (log: distance-decay is steepest near zero)
//!   + W_TIME * ln(1 + days_apart)                 (log: same reason for temporal proximity)
//!   + W_SIG  * min(shared_signatures, SIG_CAP)    (rare behavioural signatures are strong evidence)
//! ```
//!
//! The functional form follows the burglary-linkage literature: logistic regression over
//! behavioural (MO) similarity, inter-crime distance and temporal proximity reaches AUC > .9 on
//! solved series (Tonkin, Woodhams, Bull, Bond and Palmer; the Leicester comparison of linkage
//! methods found inter-crime distance and temporal proximity at least as discriminating as MO
//! alone). We do not fit these weights on real data (we have none); they are set so that the
//! operating points below hold, and they are pinned by unit tests:
//!
//! | pair                                                         | score  |
//! |--------------------------------------------------------------|--------|
//! | same offender: MO sim >= .7, < 2 km, < 21 days, 1 signature  | >= .8  |
//! | unrelated: MO sim ~ .3, ~8 km, ~90 days, no shared signature | <= .01 |
//!
//! A pair where either case has no extracted MO is scored on space and time only and then
//! multiplied by [`NO_MO_DISCOUNT`]; such a pair can never reach the default 0.5 threshold.
//!
//! Cases are linked, never people: nothing here names or scores an offender or a neighbourhood.

use crate::geo::haversine_km;
use crate::types::*;
use serde::Serialize;

// --- MO similarity weights (sum to 1.0, pinned by a unit test) -----------------------------

/// Weight of premise-type agreement. Premise (house/garage/commercial) is the strongest
/// single behavioural constraint: offenders specialise by target type.
pub const W_PREMISE: f64 = 0.20;
/// Weight of entry-point agreement (front/rear/sliding door, window...).
pub const W_ENTRY_POINT: f64 = 0.15;
/// Weight of entry-method agreement (pry, smash, cut screen...). Method is among the most
/// stable MO features across an offender's series.
pub const W_ENTRY_METHOD: f64 = 0.15;
/// Weight of time-of-day band agreement. Lower than the entry features because windows are wide
/// and bands are coarse.
pub const W_TIME_BAND: f64 = 0.10;
/// Weight of weekday/weekend agreement. Lowest: it is a coin-flip feature (a binary field agrees
/// by chance half the time), so it carries little evidence.
pub const W_WEEKDAY: f64 = 0.05;
/// Weight of the Jaccard overlap of property types taken (offenders prefer particular goods).
pub const W_PROPERTY: f64 = 0.15;
/// Weight of the Jaccard overlap of behavioural signatures (e.g. ransacked master bedroom, used
/// pillowcase). Highest with premise because signatures are the rarest, most discriminating cues.
pub const W_SIGNATURES: f64 = 0.20;

// --- Pair-score logistic coefficients -------------------------------------------------------

/// Logistic intercept: a pair with no MO agreement, zero distance and zero days apart sits at
/// sigmoid(-0.4) = .40 before the MO term is added, i.e. proximity alone is never decisive.
pub const INTERCEPT: f64 = -0.4;
/// Coefficient on `mo_similarity` (0..1). Full MO agreement is worth +6 logits.
pub const W_MO: f64 = 6.0;
/// Coefficient on `ln(1 + distance_km)`. Negative: farther apart is less likely the same
/// offender (distance-decay is the most robust finding in geographic offender linkage).
pub const W_DIST: f64 = -1.3;
/// Coefficient on `ln(1 + days_apart)`. Negative: temporal proximity raises the odds of a link.
pub const W_TIME: f64 = -0.9;
/// Coefficient per shared behavioural signature (up to [`SIG_CAP`]).
pub const W_SIG: f64 = 0.6;
/// Shared signatures beyond this count add nothing (stops one verbose narrative dominating).
pub const SIG_CAP: usize = 3;
/// Multiplier applied to the probability when either case has no extracted MO. Without MO we
/// have only space and time, which the literature finds much weaker than MO plus space-time, so
/// the score is "heavily discounted": the best possible value is sigmoid(-0.4) * .5 = .2007.
pub const NO_MO_DISCOUNT: f64 = 0.5;

// --- Candidate window and clustering --------------------------------------------------------

/// Pairs further apart than this are not clustering candidates (cheap prefilter in
/// [`propose_clusters`]). Generous: the longest planted active period is ten weeks.
pub const MAX_LINK_DAYS: f64 = 180.0;
/// Pairs further apart than this are not clustering candidates. Above the 12 km `verify` spread
/// limit, so geographically implausible "decoy" groups still reach `verify_series`.
pub const MAX_LINK_KM: f64 = 30.0;
/// A connected component bigger than this is treated as chaining (A~B, B~C, C~D...) and is
/// re-split at a higher threshold. Real burglary series are small; the heaviest planted one is 12.
pub const MAX_CLUSTER_SIZE: usize = 15;
/// Threshold increase per re-split round.
pub const RESPLIT_STEP: f64 = 0.05;
/// Re-splitting stops when the next threshold would exceed this (nothing is left to cut).
pub const MAX_RESPLIT_THRESHOLD: f64 = 0.995;

const SECS_PER_DAY: f64 = 86_400.0;

#[cfg(test)]
pub(crate) mod test_support {
    //! Small fixtures shared by the unit tests of this crate.
    use crate::types::*;

    pub const DAY: i64 = 86_400;
    pub const HOUR: i64 = 3_600;
    /// 2026-01-01 00:00:00 UTC.
    pub const T0: i64 = 1_767_225_600;

    pub fn facts() -> SolvabilityFacts {
        SolvabilityFacts {
            witness: false,
            suspect_description: false,
            vehicle_description: false,
            video_available: false,
            physical_evidence: false,
            traceable_property: false,
            window_hours: 8.0,
        }
    }

    pub fn mo() -> Mo {
        Mo {
            premise: Premise::Residential,
            entry_point: EntryPoint::RearDoor,
            entry_method: EntryMethod::Pry,
            time_band: TimeBand::Midday,
            weekday: true,
            property_types: vec![PropertyType::Laptop, PropertyType::Jewelry],
            signatures: vec!["ransacked_master_bedroom".into()],
        }
    }

    /// A case at (`lat`, `lon`) whose window starts `start` seconds after [`T0`] and lasts
    /// `hours` hours, with the default MO.
    pub fn case(id: &str, lat: f64, lon: f64, start: i64, hours: i64) -> Case {
        Case {
            id: id.to_string(),
            reported_at: T0 + start + hours * HOUR,
            window_start: T0 + start,
            window_end: T0 + start + hours * HOUR,
            location: GeoPoint { lat, lon },
            block: "100 BLOCK OF TEST ST".into(),
            beat: "B1".into(),
            neighborhood: "Testville".into(),
            narrative: String::new(),
            mo: Some(mo()),
            items: vec![],
            facts: facts(),
            status: CaseStatus::Open,
        }
    }
}

/// Midpoint of the offense window, in seconds.
fn window_mid(c: &Case) -> f64 {
    (c.window_start as f64 + c.window_end as f64) / 2.0
}

/// Count of distinct elements of `v` under `eq`.
fn unique_count<T>(v: &[T], eq: impl Fn(&T, &T) -> bool) -> usize {
    (0..v.len())
        .filter(|&i| !v[..i].iter().any(|p| eq(p, &v[i])))
        .count()
}

/// Count of distinct elements of `a` that also occur in `b`.
fn intersection_count<T>(a: &[T], b: &[T], eq: impl Fn(&T, &T) -> bool) -> usize {
    (0..a.len())
        .filter(|&i| !a[..i].iter().any(|p| eq(p, &a[i])) && b.iter().any(|q| eq(q, &a[i])))
        .count()
}

/// Jaccard index of two lists treated as sets. Two empty lists give 0.0: absence of data is
/// not evidence of agreement.
fn jaccard<T>(a: &[T], b: &[T], eq: impl Fn(&T, &T) -> bool + Copy) -> f64 {
    let inter = intersection_count(a, b, eq);
    let union = unique_count(a, eq) + unique_count(b, eq) - inter;
    if union == 0 {
        0.0
    } else {
        inter as f64 / union as f64
    }
}

#[allow(clippy::ptr_arg)] // must match the `Fn(&T, &T)` bound with T = String
fn sig_eq(a: &String, b: &String) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Number of distinct behavioural signatures present in both MOs (case-insensitive).
fn shared_signatures(a: &Mo, b: &Mo) -> usize {
    intersection_count(&a.signatures, &b.signatures, sig_eq)
}

fn mo_sim(a: &Mo, b: &Mo) -> f64 {
    let agree = |same: bool, unknown: bool| if same && !unknown { 1.0 } else { 0.0 };
    W_PREMISE * agree(a.premise == b.premise, false)
        + W_ENTRY_POINT
            * agree(
                a.entry_point == b.entry_point,
                a.entry_point == EntryPoint::Unknown,
            )
        + W_ENTRY_METHOD
            * agree(
                a.entry_method == b.entry_method,
                a.entry_method == EntryMethod::Unknown,
            )
        + W_TIME_BAND
            * agree(
                a.time_band == b.time_band,
                a.time_band == TimeBand::Unknown,
            )
        + W_WEEKDAY * agree(a.weekday == b.weekday, false)
        + W_PROPERTY * jaccard(&a.property_types, &b.property_types, |x, y| x == y)
        + W_SIGNATURES * jaccard(&a.signatures, &b.signatures, sig_eq)
}

/// Weighted MO agreement in 0..1. Exact match on premise, entry point, entry method, time band
/// and weekday; Jaccard on property types and on signatures. `Unknown` never counts as
/// agreement, and a missing MO on either side gives 0.0. Symmetric.
pub fn mo_similarity(a: &Case, b: &Case) -> f64 {
    match (&a.mo, &b.mo) {
        (Some(x), Some(y)) => mo_sim(x, y).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z.clamp(-40.0, 40.0)).exp())
}

/// The pair-score model applied to already-computed components. `has_mo` is false when either
/// case lacks an extracted MO (then the [`NO_MO_DISCOUNT`] applies).
pub fn score_from_components(c: &PairComponents, has_mo: bool) -> f64 {
    let z = INTERCEPT
        + W_MO * c.mo_similarity
        + W_DIST * (1.0 + c.distance_km.max(0.0)).ln()
        + W_TIME * (1.0 + c.days_apart.max(0.0)).ln()
        + W_SIG * c.shared_signatures.min(SIG_CAP) as f64;
    let p = sigmoid(z);
    if has_mo { p } else { p * NO_MO_DISCOUNT }
}

/// Components for a pair, computed in canonical (id-sorted) order so the result is bit-for-bit
/// symmetric.
fn components(a: &Case, b: &Case) -> PairComponents {
    let (x, y) = if a.id <= b.id { (a, b) } else { (b, a) };
    PairComponents {
        mo_similarity: mo_similarity(x, y),
        distance_km: haversine_km(x.location, y.location),
        days_apart: (window_mid(x) - window_mid(y)).abs() / SECS_PER_DAY,
        shared_signatures: match (&x.mo, &y.mo) {
            (Some(p), Some(q)) => shared_signatures(p, q),
            _ => 0,
        },
    }
}

/// Linkage score for a pair of cases: a logistic over MO similarity, inter-crime distance,
/// temporal proximity and shared signatures (see the module docs for coefficients and
/// calibration). Symmetric: `pair_score(a, b).score == pair_score(b, a).score`.
pub fn pair_score(a: &Case, b: &Case) -> PairScore {
    let c = components(a, b);
    let has_mo = a.mo.is_some() && b.mo.is_some();
    PairScore {
        a: a.id.clone(),
        b: b.id.clone(),
        score: score_from_components(&c, has_mo),
        components: c,
    }
}

/// Candidate in the top-k heap: "greater" means a better rank (higher score, then smaller id).
struct Ranked<'a> {
    score: f64,
    case: &'a Case,
}
impl PartialEq for Ranked<'_> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o).is_eq()
    }
}
impl Eq for Ranked<'_> {}
impl PartialOrd for Ranked<'_> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ranked<'_> {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.score.total_cmp(&o.score).then_with(|| o.case.id.cmp(&self.case.id))
    }
}

/// Upper bound on the pair score: full MO agreement and as many shared signatures as both cases
/// hold. Never below the exact score, so a pair under a cutoff by this bound is under it exactly.
fn score_upper_bound(x: &Case, y: &Case) -> f64 {
    let shared = match (&x.mo, &y.mo) {
        (Some(p), Some(q)) => p.signatures.len().min(q.signatures.len()),
        _ => 0,
    };
    score_from_components(
        &PairComponents {
            mo_similarity: 1.0,
            distance_km: haversine_km(x.location, y.location),
            days_apart: (window_mid(x) - window_mid(y)).abs() / SECS_PER_DAY,
            shared_signatures: shared,
        },
        true,
    )
}

/// Top `k` other cases by pair score against `case_id`, descending, ties broken by case id.
/// Unknown id => empty. Exact, not a prefilter: a bounded min-heap keeps the best `k` seen so far
/// (O(n log k)), and a case whose score upper bound is already below the current k-th best is
/// skipped without any MO work. Only the `k` survivors allocate a [`PairScore`].
pub fn similar_cases(ds: &Dataset, case_id: &str, k: usize) -> Vec<PairScore> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let Some(target) = ds.cases.iter().find(|c| c.id == case_id) else {
        return Vec::new();
    };
    if k == 0 {
        return Vec::new();
    }
    let mut heap: BinaryHeap<Reverse<Ranked>> = BinaryHeap::with_capacity(k.min(ds.cases.len()) + 1);
    for c in ds.cases.iter().filter(|c| c.id != case_id) {
        if heap.len() == k
            && let Some(Reverse(worst)) = heap.peek()
        {
            let (x, y) = if target.id <= c.id { (target, c) } else { (c, target) };
            if score_upper_bound(x, y) < worst.score {
                continue;
            }
        }
        let score = score_from_components(&components(target, c), target.mo.is_some() && c.mo.is_some());
        let cand = Ranked { score, case: c };
        if heap.len() < k {
            heap.push(Reverse(cand));
        } else if heap.peek().is_some_and(|Reverse(w)| cand > *w) {
            heap.pop();
            heap.push(Reverse(cand));
        }
    }
    let mut best: Vec<Ranked> = heap.into_iter().map(|Reverse(r)| r).collect();
    best.sort_by(|a, b| b.cmp(a));
    best.into_iter().map(|r| pair_score(target, r.case)).collect()
}

/// Brute-force reference for [`similar_cases`]: score everything, full sort, truncate. Oracle for
/// tests and `casefile bench`.
#[doc(hidden)]
pub fn similar_cases_reference(ds: &Dataset, case_id: &str, k: usize) -> Vec<PairScore> {
    let Some(target) = ds.cases.iter().find(|c| c.id == case_id) else {
        return Vec::new();
    };
    let mut out: Vec<PairScore> =
        ds.cases.iter().filter(|c| c.id != case_id).map(|c| pair_score(target, c)).collect();
    out.sort_by(|x, y| y.score.total_cmp(&x.score).then_with(|| x.b.cmp(&y.b)));
    out.truncate(k);
    out
}

/// Disjoint-set forest over `n` positions.
struct Dsu {
    parent: Vec<usize>,
}

impl Dsu {
    fn new(n: usize) -> Self {
        Dsu {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            // Keep the smaller index as the root so results do not depend on edge order.
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.parent[hi] = lo;
        }
    }
}

type Edge = (usize, usize, f64);

/// Connected components (size >= 2) of the graph restricted to `edges` with score >= `t`.
/// Positions refer to the id-sorted case list; each component is sorted ascending.
fn components_at(n: usize, edges: &[Edge], t: f64) -> Vec<Vec<usize>> {
    let mut dsu = Dsu::new(n);
    for &(i, j, s) in edges {
        if s >= t {
            dsu.union(i, j);
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    let mut touched = vec![false; n];
    for &(i, j, s) in edges {
        if s >= t {
            touched[i] = true;
            touched[j] = true;
        }
    }
    for (i, &t_) in touched.iter().enumerate() {
        if t_ {
            let r = dsu.find(i);
            groups.entry(r).or_default().push(i);
        }
    }
    groups.into_values().filter(|g| g.len() >= 2).collect()
}

fn snake<T: Serialize>(x: &T) -> String {
    serde_json::to_value(x)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// One sentence describing why these cases were grouped: strength, shared MO, span, spread.
fn rationale(members: &[&Case], edges: &[Edge], pos: &[usize], t: f64) -> String {
    let in_group = |i: usize| pos.contains(&i);
    let used: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.2 >= t && in_group(e.0) && in_group(e.1))
        .collect();
    let mean = if used.is_empty() {
        0.0
    } else {
        used.iter().map(|e| e.2).sum::<f64>() / used.len() as f64
    };
    let best = used
        .iter()
        .max_by(|x, y| x.2.total_cmp(&y.2).then_with(|| y.0.cmp(&x.0)).then_with(|| y.1.cmp(&x.1)));
    let mut max_km = 0.0_f64;
    for (i, a) in members.iter().enumerate() {
        for b in &members[i + 1..] {
            max_km = max_km.max(haversine_km(a.location, b.location));
        }
    }
    let mids: Vec<f64> = members.iter().map(|c| window_mid(c)).collect();
    let span_days = (mids.iter().cloned().fold(f64::MIN, f64::max)
        - mids.iter().cloned().fold(f64::MAX, f64::min))
        / SECS_PER_DAY;

    // Features on which every case with an MO agrees (and is not Unknown).
    let mos: Vec<&Mo> = members.iter().filter_map(|c| c.mo.as_ref()).collect();
    let mut shared: Vec<String> = Vec::new();
    if let Some(first) = mos.first() {
        if mos.iter().all(|m| m.premise == first.premise) {
            shared.push(format!("premise {}", snake(&first.premise)));
        }
        if first.entry_point != EntryPoint::Unknown
            && mos.iter().all(|m| m.entry_point == first.entry_point)
        {
            shared.push(format!("entry point {}", snake(&first.entry_point)));
        }
        if first.entry_method != EntryMethod::Unknown
            && mos.iter().all(|m| m.entry_method == first.entry_method)
        {
            shared.push(format!("entry method {}", snake(&first.entry_method)));
        }
        if first.time_band != TimeBand::Unknown
            && mos.iter().all(|m| m.time_band == first.time_band)
        {
            shared.push(format!("time band {}", snake(&first.time_band)));
        }
        for s in &first.signatures {
            if mos
                .iter()
                .all(|m| m.signatures.iter().any(|x| sig_eq(x, s)))
            {
                shared.push(format!("signature {s}"));
            }
        }
    }
    let shared_txt = if shared.is_empty() {
        "no single MO feature common to all".to_string()
    } else {
        format!("common to all: {}", shared.join(", "))
    };
    let best_txt = match best {
        Some(&&(i, j, s)) => {
            // `i`/`j` index the sorted list, which `members` mirrors via `pos`.
            let name = |p: usize| {
                pos.iter()
                    .position(|&q| q == p)
                    .map(|k| members[k].id.as_str())
                    .unwrap_or("?")
            };
            format!("strongest pair {} and {} at {:.2}", name(i), name(j), s)
        }
        None => "no linked pair".to_string(),
    };
    format!(
        "{} cases linked at pair score >= {:.2} ({}; mean linked-pair score {:.2}); offense windows span {:.0} days, widest pair {:.1} km apart; {}.",
        members.len(),
        t,
        best_txt,
        mean,
        span_days,
        max_km,
        shared_txt
    )
}

/// Propose candidate series: connected components (size >= 2) of the graph whose edges are pairs
/// scoring >= `threshold`. Components larger than [`MAX_CLUSTER_SIZE`] are re-split with
/// `threshold + 0.05`, repeatedly, to stop chaining; a component that is still larger than that
/// at [`MAX_RESPLIT_THRESHOLD`] is dropped rather than proposed. Pairs more than [`MAX_LINK_DAYS`] or
/// [`MAX_LINK_KM`] apart are never candidates. Output is deterministic: ids are `S-1`, `S-2`...
/// in ascending order of each cluster's smallest case id, and `case_ids` are sorted.
/// These are hypotheses for `verify::verify_series`, never conclusions.
///
/// Implementation (same output as [`propose_clusters_reference`], pinned by tests): cases are
/// prepared once (property types as a bitmask, signatures interned to sorted integer ids), then a
/// time-sorted sweep visits only pairs inside [`MAX_LINK_DAYS`]. For each, an upper bound on the
/// score (full MO agreement, as many shared signatures as both cases hold) rejects the pair before
/// any MO work when even that cannot reach `threshold`. Survivors get the exact same formula,
/// allocation-free, split across threads and re-ordered by `(i, j)`.
pub fn propose_clusters(ds: &Dataset, threshold: f64) -> Vec<SeriesHypothesis> {
    let mut sorted: Vec<&Case> = ds.cases.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let n = sorted.len();
    let prep = Prepared::new(&sorted);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| prep.mid[a].total_cmp(&prep.mid[b]).then(a.cmp(&b)));

    let threads = if n < 512 { 1 } else { std::thread::available_parallelism().map_or(1, |p| p.get().min(8)) };
    let chunk = n.div_ceil(threads).max(1);
    let mut edges: Vec<Edge> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..n)
            .step_by(chunk)
            .map(|lo| {
                let (prep, order) = (&prep, &order);
                sc.spawn(move || prep.edges_from(order, lo..(lo + chunk).min(n), threshold))
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    edges.sort_by_key(|e| (e.0, e.1));
    clusters_from_edges(&sorted, edges, threshold)
}

/// Per-case numbers computed once so the pair loop does no allocation and no string work.
struct Prepared<'a> {
    cases: &'a [&'a Case],
    mid: Vec<f64>,
    mo: Vec<Option<MoBits<'a>>>,
}

struct MoBits<'a> {
    mo: &'a Mo,
    props: u16,
    /// Interned (trimmed, ASCII-lowercased) signatures, sorted and deduplicated.
    sigs: Vec<u32>,
}

fn jaccard_mask(a: u16, b: u16) -> f64 {
    let inter = (a & b).count_ones();
    let union = a.count_ones() + b.count_ones() - inter;
    if union == 0 { 0.0 } else { inter as f64 / union as f64 }
}

/// Size of the intersection of two sorted, deduplicated id lists (merge walk).
fn sorted_inter(a: &[u32], b: &[u32]) -> usize {
    let (mut i, mut j, mut n) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                n += 1;
                i += 1;
                j += 1;
            }
        }
    }
    n
}

fn jaccard_sorted(a: &[u32], b: &[u32]) -> f64 {
    let union = a.len() + b.len() - sorted_inter(a, b);
    if union == 0 { 0.0 } else { sorted_inter(a, b) as f64 / union as f64 }
}

impl<'a> Prepared<'a> {
    fn new(cases: &'a [&'a Case]) -> Self {
        let mut ids: std::collections::HashMap<String, u32> = Default::default();
        let mo = cases
            .iter()
            .map(|c| {
                c.mo.as_ref().map(|m| {
                    let mut sigs: Vec<u32> = m
                        .signatures
                        .iter()
                        .map(|s| {
                            let next = ids.len() as u32;
                            *ids.entry(s.trim().to_ascii_lowercase()).or_insert(next)
                        })
                        .collect();
                    sigs.sort_unstable();
                    sigs.dedup();
                    let props = m.property_types.iter().fold(0u16, |acc, p| acc | (1 << (*p as u16)));
                    MoBits { mo: m, props, sigs }
                })
            })
            .collect();
        Prepared { cases, mid: cases.iter().map(|c| window_mid(c)).collect(), mo }
    }

    /// Same value, bit for bit, as `mo_sim` on the two MOs, plus the shared-signature count.
    fn mo_sim(&self, i: usize, j: usize) -> Option<(f64, usize)> {
        let (a, b) = (self.mo[i].as_ref()?, self.mo[j].as_ref()?);
        let (x, y) = (a.mo, b.mo);
        let agree = |same: bool, unknown: bool| if same && !unknown { 1.0 } else { 0.0 };
        let sim = W_PREMISE * agree(x.premise == y.premise, false)
            + W_ENTRY_POINT * agree(x.entry_point == y.entry_point, x.entry_point == EntryPoint::Unknown)
            + W_ENTRY_METHOD * agree(x.entry_method == y.entry_method, x.entry_method == EntryMethod::Unknown)
            + W_TIME_BAND * agree(x.time_band == y.time_band, x.time_band == TimeBand::Unknown)
            + W_WEEKDAY * agree(x.weekday == y.weekday, false)
            + W_PROPERTY * jaccard_mask(a.props, b.props)
            + W_SIGNATURES * jaccard_sorted(&a.sigs, &b.sigs);
        Some((sim.clamp(0.0, 1.0), sorted_inter(&a.sigs, &b.sigs)))
    }

    /// Edges `(i, j, score)`, `i < j` in id-sorted positions, reaching `threshold`, for the cases
    /// at `order[range]` paired with every later-in-time case inside the link window.
    fn edges_from(&self, order: &[usize], range: std::ops::Range<usize>, threshold: f64) -> Vec<Edge> {
        let km_per_deg = 111.2_f64;
        let mut out = Vec::new();
        for p in range {
            let a = order[p];
            for &b in &order[p + 1..] {
                if (self.mid[b] - self.mid[a]) / SECS_PER_DAY > MAX_LINK_DAYS {
                    break; // sorted by time: every later case is further away still
                }
                let (i, j) = if a < b { (a, b) } else { (b, a) };
                let (pa, pb) = (self.cases[i].location, self.cases[j].location);
                let dlat = (pa.lat - pb.lat).abs() * km_per_deg;
                let cos = (((pa.lat + pb.lat) / 2.0).to_radians()).cos().abs();
                let dlon = (pa.lon - pb.lon).abs() * km_per_deg * cos;
                if dlat > MAX_LINK_KM || dlon > MAX_LINK_KM {
                    continue;
                }
                let distance_km = haversine_km(pa, pb);
                let days_apart = (self.mid[i] - self.mid[j]).abs() / SECS_PER_DAY;
                let (sa, sb) = (&self.mo[i], &self.mo[j]);
                let max_shared = match (sa, sb) {
                    (Some(x), Some(y)) => x.sigs.len().min(y.sigs.len()),
                    _ => 0,
                };
                let bound = score_from_components(
                    &PairComponents { mo_similarity: 1.0, distance_km, days_apart, shared_signatures: max_shared },
                    true,
                );
                if bound < threshold {
                    continue;
                }
                let (mo_similarity, shared_signatures) = self.mo_sim(i, j).unwrap_or((0.0, 0));
                let c = PairComponents { mo_similarity, distance_km, days_apart, shared_signatures };
                let score = score_from_components(&c, sa.is_some() && sb.is_some());
                if score >= threshold {
                    out.push((i, j, score));
                }
            }
        }
        out
    }
}

/// How many pairs survive each stage of [`propose_clusters`] at `threshold`:
/// `[all pairs, inside the time window, inside the box, pass the upper bound, edges]`.
/// For explaining and benchmarking only; single-threaded.
#[doc(hidden)]
pub fn pair_funnel(ds: &Dataset, threshold: f64) -> [usize; 5] {
    let mut sorted: Vec<&Case> = ds.cases.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let n = sorted.len();
    let prep = Prepared::new(&sorted);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| prep.mid[a].total_cmp(&prep.mid[b]).then(a.cmp(&b)));
    let mut f = [n * n.saturating_sub(1) / 2, 0, 0, 0, 0];
    for p in 0..n {
        let a = order[p];
        for &b in &order[p + 1..] {
            if (prep.mid[b] - prep.mid[a]) / SECS_PER_DAY > MAX_LINK_DAYS {
                break;
            }
            f[1] += 1;
            let (i, j) = if a < b { (a, b) } else { (b, a) };
            let (pa, pb) = (sorted[i].location, sorted[j].location);
            let dlat = (pa.lat - pb.lat).abs() * 111.2;
            let dlon = (pa.lon - pb.lon).abs() * 111.2 * (((pa.lat + pb.lat) / 2.0).to_radians()).cos().abs();
            if dlat > MAX_LINK_KM || dlon > MAX_LINK_KM {
                continue;
            }
            f[2] += 1;
            let max_shared = match (&prep.mo[i], &prep.mo[j]) {
                (Some(x), Some(y)) => x.sigs.len().min(y.sigs.len()),
                _ => 0,
            };
            let (distance_km, days_apart) = (haversine_km(pa, pb), (prep.mid[i] - prep.mid[j]).abs() / SECS_PER_DAY);
            let bound = score_from_components(
                &PairComponents { mo_similarity: 1.0, distance_km, days_apart, shared_signatures: max_shared },
                true,
            );
            if bound < threshold {
                continue;
            }
            f[3] += 1;
            let (mo_similarity, shared_signatures) = prep.mo_sim(i, j).unwrap_or((0.0, 0));
            let c = PairComponents { mo_similarity, distance_km, days_apart, shared_signatures };
            if score_from_components(&c, prep.mo[i].is_some() && prep.mo[j].is_some()) >= threshold {
                f[4] += 1;
            }
        }
    }
    f
}

/// Brute-force reference for [`propose_clusters`]: every pair inside the time/box prefilter goes
/// through [`pair_score`]. Oracle for tests and `casefile bench`.
#[doc(hidden)]
pub fn propose_clusters_reference(ds: &Dataset, threshold: f64) -> Vec<SeriesHypothesis> {
    let mut sorted: Vec<&Case> = ds.cases.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let n = sorted.len();

    // Cheap per-case numbers for the prefilter.
    let mids: Vec<f64> = sorted.iter().map(|c| window_mid(c)).collect();
    let km_per_deg = 111.2_f64;
    let mut edges: Vec<Edge> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            if (mids[i] - mids[j]).abs() / SECS_PER_DAY > MAX_LINK_DAYS {
                continue;
            }
            let (pa, pb) = (sorted[i].location, sorted[j].location);
            let dlat = (pa.lat - pb.lat).abs() * km_per_deg;
            let cos = (((pa.lat + pb.lat) / 2.0).to_radians()).cos().abs();
            let dlon = (pa.lon - pb.lon).abs() * km_per_deg * cos;
            if dlat > MAX_LINK_KM || dlon > MAX_LINK_KM {
                continue;
            }
            let s = pair_score(sorted[i], sorted[j]);
            if s.score >= threshold {
                edges.push((i, j, s.score));
            }
        }
    }

    clusters_from_edges(&sorted, edges, threshold)
}

/// Shared tail: components, re-split while too large, then hypotheses. `sorted` is id-sorted and
/// `edges` hold positions in it, ordered by `(i, j)`.
fn clusters_from_edges(sorted: &[&Case], edges: Vec<Edge>, threshold: f64) -> Vec<SeriesHypothesis> {
    let n = sorted.len();
    // Components, re-split while too large.
    let mut done: Vec<(Vec<usize>, f64)> = Vec::new();
    let mut work: Vec<(Vec<usize>, f64)> = components_at(n, &edges, threshold)
        .into_iter()
        .map(|g| (g, threshold))
        .collect();
    while let Some((group, t)) = work.pop() {
        let next = t + RESPLIT_STEP;
        if group.len() <= MAX_CLUSTER_SIZE {
            done.push((group, t));
            continue;
        }
        if next > MAX_RESPLIT_THRESHOLD {
            // Still a giant blob at the highest usable threshold: it is a dense-area MO artefact,
            // not an investigable series. Dropping it is safer than proposing 30+ cases.
            continue;
        }
        let sub: Vec<Edge> = edges
            .iter()
            .copied()
            .filter(|e| group.binary_search(&e.0).is_ok() && group.binary_search(&e.1).is_ok())
            .collect();
        for g in components_at(n, &sub, next) {
            work.push((g, next));
        }
    }
    done.sort_by_key(|(g, _)| g[0]);

    done.into_iter()
        .enumerate()
        .map(|(k, (group, t))| {
            let members: Vec<&Case> = group.iter().map(|&p| sorted[p]).collect();
            SeriesHypothesis {
                id: format!("S-{}", k + 1),
                case_ids: members.iter().map(|c| c.id.clone()).collect(),
                rationale: rationale(&members, &edges, &group, t),
                proposed_by: "engine".to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    // Seattle-ish coordinates; 0.009 deg of latitude is about 1 km.
    const LAT: f64 = 47.6;
    const LON: f64 = -122.3;
    const KM: f64 = 0.009;

    #[test]
    fn weights_sum_to_one() {
        let s = W_PREMISE + W_ENTRY_POINT + W_ENTRY_METHOD + W_TIME_BAND + W_WEEKDAY + W_PROPERTY
            + W_SIGNATURES;
        assert!((s - 1.0).abs() < 1e-12);
    }

    #[test]
    fn identical_mo_is_one_and_symmetric() {
        let a = case("A", LAT, LON, 0, 8);
        let b = case("B", LAT, LON, DAY, 8);
        assert!((mo_similarity(&a, &b) - 1.0).abs() < 1e-12);
        assert_eq!(mo_similarity(&a, &b), mo_similarity(&b, &a));
    }

    #[test]
    fn missing_mo_gives_zero() {
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT, LON, 0, 8);
        b.mo = None;
        assert_eq!(mo_similarity(&a, &b), 0.0);
        assert_eq!(mo_similarity(&b, &a), 0.0);
        assert_eq!(mo_similarity(&b, &b), 0.0);
    }

    #[test]
    fn unknown_values_never_agree() {
        let mut a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT, LON, 0, 8);
        for c in [&mut a, &mut b] {
            let m = c.mo.as_mut().unwrap();
            m.entry_point = EntryPoint::Unknown;
            m.entry_method = EntryMethod::Unknown;
            m.time_band = TimeBand::Unknown;
            m.property_types.clear();
            m.signatures.clear();
        }
        // Only premise and weekday can agree.
        let expect = W_PREMISE + W_WEEKDAY;
        assert!((mo_similarity(&a, &b) - expect).abs() < 1e-12);
    }

    #[test]
    fn jaccard_parts_are_partial() {
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT, LON, 0, 8);
        // property types {laptop, jewelry} vs {laptop, cash}: 1/3.
        b.mo.as_mut().unwrap().property_types = vec![PropertyType::Laptop, PropertyType::Cash];
        let expect = 1.0 - W_PROPERTY + W_PROPERTY / 3.0;
        assert!((mo_similarity(&a, &b) - expect).abs() < 1e-12);
    }

    #[test]
    fn duplicate_entries_do_not_inflate_jaccard() {
        let mut a = case("A", LAT, LON, 0, 8);
        let b = case("B", LAT, LON, 0, 8);
        a.mo.as_mut().unwrap().property_types = vec![
            PropertyType::Laptop,
            PropertyType::Laptop,
            PropertyType::Jewelry,
        ];
        assert!((mo_similarity(&a, &b) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn signatures_compare_case_insensitively() {
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT, LON, 0, 8);
        b.mo.as_mut().unwrap().signatures = vec![" Ransacked_Master_Bedroom ".into()];
        assert_eq!(pair_score(&a, &b).components.shared_signatures, 1);
    }

    #[test]
    fn same_offender_pair_scores_high() {
        // MO sim .8 (one property type differs), ~1 km, 14 days, 1 shared signature.
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT + KM, LON, 14 * DAY, 8);
        b.mo.as_mut().unwrap().time_band = TimeBand::Evening;
        b.mo.as_mut().unwrap().weekday = false;
        let p = pair_score(&a, &b);
        assert!(p.components.mo_similarity >= 0.7, "{p:?}");
        assert!(p.components.distance_km < 2.0);
        assert!(p.components.days_apart < 21.0);
        assert!(p.score >= 0.8, "same-offender pair scored {}", p.score);
    }

    #[test]
    fn unrelated_pair_scores_low() {
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT + 8.0 * KM, LON, 90 * DAY, 8);
        {
            let m = b.mo.as_mut().unwrap();
            m.premise = Premise::Commercial;
            m.entry_point = EntryPoint::Window;
            m.entry_method = EntryMethod::Smash;
            m.time_band = TimeBand::Night;
            m.property_types = vec![PropertyType::Cash];
            m.signatures = vec![];
        }
        let p = pair_score(&a, &b);
        assert!(p.score <= 0.1, "unrelated pair scored {}", p.score);
    }

    #[test]
    fn typical_unrelated_pair_with_some_mo_overlap_stays_below_default_threshold() {
        // MO sim about .3, 8 km, 90 days.
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT + 8.0 * KM, LON, 90 * DAY, 8);
        {
            let m = b.mo.as_mut().unwrap();
            m.entry_point = EntryPoint::Window;
            m.entry_method = EntryMethod::Smash;
            m.time_band = TimeBand::Night;
            m.property_types = vec![PropertyType::Cash];
            m.signatures = vec![];
        }
        let p = pair_score(&a, &b);
        assert!(p.components.mo_similarity < 0.5);
        assert!(p.score < 0.01, "{}", p.score);
    }

    #[test]
    fn pair_score_is_exactly_symmetric() {
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT + 3.0 * KM, LON + 2.0 * KM, 5 * DAY + 123, 5);
        b.mo.as_mut().unwrap().entry_method = EntryMethod::Smash;
        let ab = pair_score(&a, &b);
        let ba = pair_score(&b, &a);
        assert_eq!(ab.score, ba.score);
        assert_eq!(ab.components, ba.components);
    }

    #[test]
    fn closer_and_sooner_scores_higher() {
        let a = case("A", LAT, LON, 0, 8);
        let near = pair_score(&a, &case("B", LAT + KM, LON, 2 * DAY, 8)).score;
        let far = pair_score(&a, &case("C", LAT + 10.0 * KM, LON, 2 * DAY, 8)).score;
        let late = pair_score(&a, &case("D", LAT + KM, LON, 60 * DAY, 8)).score;
        assert!(near > far);
        assert!(near > late);
    }

    #[test]
    fn more_shared_signatures_scores_higher_and_is_capped() {
        let sigs = |n: usize| -> Vec<String> { (0..n).map(|i| format!("sig_{i}")).collect() };
        let score_with = |n: usize| {
            let mut a = case("A", LAT, LON, 0, 8);
            let mut b = case("B", LAT, LON, DAY, 8);
            a.mo.as_mut().unwrap().signatures = sigs(n);
            b.mo.as_mut().unwrap().signatures = sigs(n);
            pair_score(&a, &b)
        };
        assert!(score_with(2).components.shared_signatures == 2);
        // The logit term grows to the cap and stops; the Jaccard term is 1.0 for all n >= 1.
        let s3 = score_with(3).score;
        let s6 = score_with(6).score;
        assert_eq!(s3, s6);
        assert!(score_with(1).score <= s3);
    }

    #[test]
    fn missing_mo_is_heavily_discounted() {
        let a = case("A", LAT, LON, 0, 8);
        let mut b = case("B", LAT, LON, 0, 8);
        b.mo = None;
        let p = pair_score(&a, &b);
        assert_eq!(p.components.mo_similarity, 0.0);
        assert_eq!(p.components.shared_signatures, 0);
        // Same place, same time, still far below the default 0.5 threshold.
        assert!(p.score <= 0.21, "{}", p.score);
        assert!(p.score > 0.0);
        assert_eq!(p.score, pair_score(&b, &a).score);
    }

    #[test]
    fn similar_cases_sorted_truncated_and_tie_broken_by_id() {
        let ds = Dataset {
            cases: vec![
                case("T", LAT, LON, 0, 8),
                case("Z", LAT, LON, DAY, 8),
                case("M", LAT, LON, DAY, 8), // identical to Z => tie, M before Z
                case("F", LAT + 20.0 * KM, LON, 100 * DAY, 8),
            ],
            pawn: vec![],
        };
        let r = similar_cases(&ds, "T", 3);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].b, "M");
        assert_eq!(r[1].b, "Z");
        assert_eq!(r[2].b, "F");
        assert!(r[0].score >= r[1].score && r[1].score >= r[2].score);
        assert!(r.iter().all(|p| p.a == "T"));
        assert_eq!(similar_cases(&ds, "T", 1).len(), 1);
        assert_eq!(similar_cases(&ds, "T", 0).len(), 0);
        assert!(similar_cases(&ds, "NOPE", 3).is_empty());
    }

    fn series(prefix: &str, n: usize, lat: f64, lon: f64, mo: Mo) -> Vec<Case> {
        (0..n)
            .map(|i| {
                let mut c = case(
                    &format!("{prefix}{i:02}"),
                    lat + (i as f64) * 0.2 * KM,
                    lon,
                    (i as i64) * 3 * DAY,
                    8,
                );
                c.mo = Some(mo.clone());
                c
            })
            .collect()
    }

    fn other_mo() -> Mo {
        Mo {
            premise: Premise::Commercial,
            entry_point: EntryPoint::Window,
            entry_method: EntryMethod::Smash,
            time_band: TimeBand::Night,
            weekday: false,
            property_types: vec![PropertyType::Cash, PropertyType::Tools],
            signatures: vec!["cut_phone_line".into()],
        }
    }

    #[test]
    fn clusters_find_planted_series_and_ignore_noise() {
        let mut cases = series("A", 4, LAT, LON, mo());
        cases.extend(series("B", 3, LAT + 0.2, LON + 0.2, other_mo()));
        // Isolated singleton far away and long ago.
        cases.push(case("Z99", LAT - 0.3, LON, 200 * DAY, 8));
        let ds = Dataset { cases, pawn: vec![] };
        let cl = propose_clusters(&ds, 0.5);
        assert_eq!(cl.len(), 2, "{cl:?}");
        assert_eq!(cl[0].id, "S-1");
        assert_eq!(cl[0].case_ids, vec!["A00", "A01", "A02", "A03"]);
        assert_eq!(cl[1].id, "S-2");
        assert_eq!(cl[1].case_ids, vec!["B00", "B01", "B02"]);
        assert!(cl.iter().all(|h| h.proposed_by == "engine"));
        assert!(cl[0].rationale.contains("4 cases"), "{}", cl[0].rationale);
        assert!(cl[0].rationale.contains("premise residential"), "{}", cl[0].rationale);
    }

    #[test]
    fn clusters_are_deterministic_and_input_order_independent() {
        let mut cases = series("A", 4, LAT, LON, mo());
        cases.extend(series("B", 3, LAT + 0.2, LON + 0.2, other_mo()));
        let ds = Dataset { cases: cases.clone(), pawn: vec![] };
        cases.reverse();
        let rev = Dataset { cases, pawn: vec![] };
        let a = propose_clusters(&ds, 0.5);
        assert_eq!(a, propose_clusters(&ds, 0.5));
        assert_eq!(a, propose_clusters(&rev, 0.5));
    }

    #[test]
    fn clusters_ids_follow_smallest_case_id() {
        let mut cases = series("M", 2, LAT, LON, mo());
        cases.extend(series("C", 2, LAT + 0.2, LON, other_mo()));
        let ds = Dataset { cases, pawn: vec![] };
        let cl = propose_clusters(&ds, 0.5);
        assert_eq!(cl[0].case_ids[0], "C00");
        assert_eq!(cl[0].id, "S-1");
        assert_eq!(cl[1].case_ids[0], "M00");
    }

    /// 30 near-identical cases in blocks of five: inside a block the next case is 1 day and
    /// 0.2 km later (score ~.99), between blocks 20 days and 3 km (score ~.84). At threshold
    /// 0.5 the whole chain is one component; it must be cut back into the five-case blocks.
    fn chain(n: usize) -> Vec<Case> {
        let (mut lat, mut day) = (LAT, 0_i64);
        (0..n)
            .map(|i| {
                if i % 5 == 0 && i > 0 {
                    lat += 3.0 * KM;
                    day += 20;
                } else if i > 0 {
                    lat += 0.2 * KM;
                    day += 1;
                }
                case(&format!("C{i:02}"), lat, LON, day * DAY, 8)
            })
            .collect()
    }

    #[test]
    fn giant_component_is_resplit() {
        let ds = Dataset { cases: chain(30), pawn: vec![] };
        let cl = propose_clusters(&ds, 0.5);
        let sizes: Vec<usize> = cl.iter().map(|h| h.case_ids.len()).collect();
        assert_eq!(sizes, vec![5; 6], "{cl:?}");
        // The re-split raised the threshold above the weak inter-block links.
        assert!(cl.iter().all(|h| !h.rationale.contains("pair score >= 0.50")));
    }

    #[test]
    fn small_chain_is_not_resplit() {
        let ds = Dataset { cases: chain(10), pawn: vec![] };
        let cl = propose_clusters(&ds, 0.5);
        assert_eq!(cl.len(), 1);
        assert_eq!(cl[0].case_ids.len(), 10);
        assert!(cl[0].rationale.contains("pair score >= 0.50"));
    }

    #[test]
    fn unsplittable_giant_blob_is_dropped() {
        // 20 identical cases on the same day and place: every pair scores > .99, no cut exists.
        let cases: Vec<Case> = (0..20)
            .map(|i| case(&format!("C{i:02}"), LAT, LON, 0, 8))
            .collect();
        let ds = Dataset { cases, pawn: vec![] };
        assert!(propose_clusters(&ds, 0.5).is_empty());
    }

    #[test]
    fn higher_threshold_gives_finer_or_equal_groups() {
        let mut cases = series("A", 4, LAT, LON, mo());
        cases.extend(series("B", 3, LAT + 0.2, LON + 0.2, other_mo()));
        let ds = Dataset { cases, pawn: vec![] };
        assert!(propose_clusters(&ds, 0.999_999).is_empty());
        assert!(propose_clusters(&Dataset::default(), 0.5).is_empty());
    }

    #[test]
    fn no_mo_cases_do_not_cluster_at_default_threshold() {
        let mut cases = series("A", 3, LAT, LON, mo());
        for c in &mut cases {
            c.mo = None;
        }
        let ds = Dataset { cases, pawn: vec![] };
        assert!(propose_clusters(&ds, 0.5).is_empty());
    }

    #[test]
    fn prefilter_drops_far_and_old_pairs() {
        let mut cases = series("A", 2, LAT, LON, mo());
        let mut far = case("A99", LAT, LON, 0, 8);
        far.location = GeoPoint { lat: LAT + 0.6, lon: LON }; // about 67 km
        cases.push(far);
        let mut old = case("A98", LAT, LON, 400 * DAY, 8);
        old.mo = Some(mo());
        cases.push(old);
        let ds = Dataset { cases, pawn: vec![] };
        let cl = propose_clusters(&ds, 0.5);
        assert_eq!(cl.len(), 1);
        assert_eq!(cl[0].case_ids, vec!["A00", "A01"]);
    }

    /// Deterministic pseudo-random dataset with the awkward cases: missing MOs, Unknown values,
    /// duplicate and differently-cased signatures, empty lists, shared coordinates and times.
    pub(crate) fn fuzz_dataset(n: usize, seed: u64) -> Dataset {
        let mut s = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut rnd = move |m: u64| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) % m
        };
        let sigs = ["ransacked", "Ransacked ", "pillowcase", "cut_phone_line", "muddy_prints", "PILLOWCASE"];
        let cases = (0..n)
            .map(|i| {
                let mut c = case(
                    &format!("F{:05}", (i * 7919) % 100_003),
                    LAT + rnd(40) as f64 * 0.004,
                    LON + rnd(40) as f64 * 0.004,
                    rnd(300) as i64 * DAY + rnd(3) as i64 * HOUR,
                    1 + rnd(12) as i64,
                );
                c.mo = if rnd(10) == 0 {
                    None
                } else {
                    Some(Mo {
                        premise: [Premise::Residential, Premise::Commercial, Premise::Garage][rnd(3) as usize],
                        entry_point: [EntryPoint::RearDoor, EntryPoint::Window, EntryPoint::Unknown][rnd(3) as usize],
                        entry_method: [EntryMethod::Pry, EntryMethod::Smash, EntryMethod::Unknown][rnd(3) as usize],
                        time_band: [TimeBand::Night, TimeBand::Midday, TimeBand::Unknown][rnd(3) as usize],
                        weekday: rnd(2) == 0,
                        property_types: (0..rnd(4))
                            .map(|_| [PropertyType::Laptop, PropertyType::Cash, PropertyType::Tools, PropertyType::Other][rnd(4) as usize])
                            .collect(),
                        signatures: (0..rnd(5)).map(|_| sigs[rnd(6) as usize].to_string()).collect(),
                    })
                };
                c
            })
            .collect();
        Dataset { cases, pawn: vec![] }
    }

    #[test]
    fn fast_clusters_equal_reference_on_fuzzed_data() {
        for (n, seed) in [(60, 1), (400, 2), (700, 3)] {
            let ds = fuzz_dataset(n, seed);
            for t in [0.2, 0.5, 0.8, 0.9, 0.99] {
                assert_eq!(propose_clusters(&ds, t), propose_clusters_reference(&ds, t), "n={n} seed={seed} t={t}");
            }
        }
    }

    #[test]
    fn pair_funnel_narrows_and_counts_edges() {
        let ds = fuzz_dataset(400, 2);
        let f = pair_funnel(&ds, 0.5);
        assert_eq!(f[0], 400 * 399 / 2);
        assert!(f.windows(2).all(|w| w[0] >= w[1]), "{f:?}");
        assert!(f[4] > 0, "fuzz data should produce some edges: {f:?}");
    }

    #[test]
    fn fast_similar_cases_equal_reference_on_fuzzed_data() {
        let ds = fuzz_dataset(300, 9);
        for c in ds.cases.iter().step_by(23) {
            for k in [0, 1, 5, 50, 400] {
                assert_eq!(similar_cases(&ds, &c.id, k), similar_cases_reference(&ds, &c.id, k), "{} k={k}", c.id);
            }
        }
    }

    #[test]
    fn two_thousand_cases_run_fast_enough() {
        // Smoke test for the O(n^2) loop: spread cases so most pairs are prefiltered or low.
        let cases: Vec<Case> = (0..2000)
            .map(|i| {
                let mut c = case(
                    &format!("CC-{i:05}"),
                    LAT + ((i * 7919) % 400) as f64 * 0.001,
                    LON + ((i * 104_729) % 400) as f64 * 0.001,
                    ((i * 31) % 365) as i64 * DAY,
                    8,
                );
                if let Some(m) = c.mo.as_mut() {
                    m.entry_method = if i % 3 == 0 { EntryMethod::Pry } else { EntryMethod::Smash };
                    m.time_band = if i % 5 == 0 { TimeBand::Night } else { TimeBand::Midday };
                }
                c
            })
            .collect();
        let ds = Dataset { cases, pawn: vec![] };
        let cl = propose_clusters(&ds, 0.8);
        assert!(cl.iter().all(|h| h.case_ids.len() <= MAX_CLUSTER_SIZE));
    }
}
