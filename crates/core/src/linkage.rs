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
use crate::types::*;

pub fn mo_similarity(a: &Case, b: &Case) -> f64 { let _ = (a, b); todo!("linkage::mo_similarity") }

pub fn pair_score(a: &Case, b: &Case) -> PairScore { let _ = (a, b); todo!("linkage::pair_score") }

pub fn similar_cases(ds: &Dataset, case_id: &str, k: usize) -> Vec<PairScore> {
    let _ = (ds, case_id, k);
    todo!("linkage::similar_cases")
}

pub fn propose_clusters(ds: &Dataset, threshold: f64) -> Vec<SeriesHypothesis> {
    let _ = (ds, threshold);
    todo!("linkage::propose_clusters")
}
