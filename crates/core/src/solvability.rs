//! Explainable solvability score (0-100) from SolvabilityFacts, MO and status.
//!
//! SPEC: logistic model with NAMED coefficients per factor (witness, suspect_description,
//! vehicle_description, video_available, physical_evidence, traceable_property, window_hours
//! (negative, log-scaled), plus `linked_to_series: bool` and `property_hit: bool` passed in as
//! context). score = 100 * sigmoid(intercept + sum). contributions = each factor's contribution in
//! score points relative to the intercept-only score (so they explain the number), sorted by
//! |value| desc. Bands: < 25 Weak, < 55 Moderate, else Strong. Deterministic. Provide `fit()` that
//! trains coefficients by plain gradient descent on (facts, label) pairs (no ML crates) so evals can
//! report a fitted model; default coefficients are hand-set constants documented with the
//! literature (Eck 1983; Greater Manchester replication).
use crate::types::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    pub linked_to_series: bool,
    pub property_hit: bool,
}

pub fn score(case: &Case, ctx: Context) -> SolvabilityScore {
    let _ = (case, ctx);
    todo!("solvability::score")
}
