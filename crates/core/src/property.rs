//! Stolen-property matching against pawn/secondhand records (RCW 19.60.040 reports).
//!
//! SPEC (rules carried over from the team's RecoverLink prototype, keep them):
//! - normalize serial: uppercase, strip non-alphanumerics. Do NOT conflate O/0 or I/1.
//! - both serials present and different => RejectedConflictingSerial (even if description identical)
//! - both present and equal => ExactSerialLead (a lead, never verified ownership)
//! - no serials, share a distinctive marking (case-insensitive, trimmed) => ReviewCandidateMarking
//! - otherwise, same category + brand/model or description token-Jaccard >= 0.5 =>
//!   UnresolvedSimilarDescription; else no match (not returned).
//! - pawn received_at before the case window_start => never a match (impossible timeline).
//! `match_case` returns matches for every item of the case, sorted by status strength then
//! days_after_offense. `all_hits` scans all cases (used by nightly batch / evals).
use crate::types::*;

pub fn normalize_serial(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

pub fn compare(case: &Case, item: &StolenItem, pawn: &PawnRecord) -> Option<PropertyMatch> {
    let _ = (case, item, pawn);
    todo!("property::compare")
}

pub fn match_case(ds: &Dataset, case_id: &str) -> Vec<PropertyMatch> {
    let _ = (ds, case_id);
    todo!("property::match_case")
}

pub fn all_hits(ds: &Dataset) -> Vec<PropertyMatch> {
    let _ = ds;
    todo!("property::all_hits")
}
