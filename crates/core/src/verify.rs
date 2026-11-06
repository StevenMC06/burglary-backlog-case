//! `verify_series`: the deterministic yes/no "grid" for a series hypothesis.
//!
//! SPEC (implementer fills bodies, keep signatures). Checks, in this order:
//! 1. HARD `spatiotemporal_feasibility`: for every pair whose offense windows make them as close in
//!    time as possible, require that one offender could travel between scenes: minimum time gap
//!    between windows (0 if overlapping, then use the overlap-aware best case) must allow
//!    distance_km / MAX_URBAN_SPEED_KMH + MIN_ON_SCENE_MIN. Only FAIL when even the best-case
//!    timing is infeasible. Constants named and documented (e.g. 40 km/h, 10 min on scene).
//! 2. HARD `conflicting_clearance`: two cases cleared by arrest with different arrestee_ref => FAIL.
//! 3. SOFT `mo_consistency`: mean pairwise mo_similarity; < 0.35 Fail(soft->Weak), < 0.55 Weak.
//! 4. SOFT `property_consistency`: overlap of property_types across cases; disjoint => Weak.
//! 5. SOFT `geographic_spread`: max pairwise distance > 12 km => Weak (burglars commute short).
//! Verdict: Fail if any HARD check fails; Weak if any soft check is Weak/Fail; else Pass.
//! `offending_cases`: the minimal set of case ids implicated in hard failures (for a pair conflict
//! choose the case with the lower mean pair_score to the rest). Reasons must be specific and cite
//! fields, e.g. Citation{record_id: case id, field: "window_start"}.
//! Unknown case ids => single HARD Fail check "unknown_case".
use crate::types::*;

pub const MAX_URBAN_SPEED_KMH: f64 = 40.0;
pub const MIN_ON_SCENE_MIN: f64 = 10.0;

pub fn verify_series(ds: &Dataset, case_ids: &[String]) -> VerifyResult {
    let _ = (ds, case_ids);
    todo!("verify::verify_series")
}
