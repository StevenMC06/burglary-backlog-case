//! Stolen-property matching against pawn/secondhand records (RCW 19.60.040 reports).
//!
//! SPEC (rules carried over from the team's RecoverLink prototype, keep them):
//! - normalize serial: uppercase, strip non-alphanumerics. Do NOT conflate O/0 or I/1.
//! - both serials present and different => RejectedConflictingSerial (even if description identical;
//!   only reported when the categories agree, to avoid noise from unrelated items)
//! - both present and equal => ExactSerialLead (a lead, never verified ownership)
//! - otherwise, shared distinctive marking (case-insensitive, trimmed) => ReviewCandidateMarking
//! - otherwise, same category + brand/model or description token-Jaccard >= 0.5 =>
//!   UnresolvedSimilarDescription; else no match (not returned).
//! - pawn received_at before the case window_start => never a match (impossible timeline).
//!
//! `match_case` returns matches for every item of the case, sorted by status strength then
//! days_after_offense. `all_hits` scans all cases (used by nightly batch / evals).
use crate::types::*;
use std::collections::BTreeSet;

const SECS_PER_DAY: f64 = 86_400.0;
/// Minimum description token Jaccard for an unresolved similar-description match.
pub const DESC_JACCARD_MIN: f64 = 0.5;

pub fn normalize_serial(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

fn serial_of(s: &Option<String>) -> Option<String> {
    s.as_deref().map(normalize_serial).filter(|n| !n.is_empty())
}

fn tokens(s: &str) -> BTreeSet<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).map(|t| t.to_lowercase()).collect()
}

fn jaccard(a: &str, b: &str) -> f64 {
    let (x, y) = (tokens(a), tokens(b));
    let union = x.union(&y).count();
    if union == 0 { 0.0 } else { x.intersection(&y).count() as f64 / union as f64 }
}

fn same_opt(a: &Option<String>, b: &Option<String>) -> bool {
    matches!((a, b), (Some(x), Some(y)) if x.trim().eq_ignore_ascii_case(y.trim()))
}

fn strength(s: MatchStatus) -> u8 {
    match s {
        MatchStatus::ExactSerialLead => 0,
        MatchStatus::ReviewCandidateMarking => 1,
        MatchStatus::UnresolvedSimilarDescription => 2,
        MatchStatus::RejectedConflictingSerial => 3,
    }
}

pub fn compare(case: &Case, item: &StolenItem, pawn: &PawnRecord) -> Option<PropertyMatch> {
    if pawn.received_at < case.window_start {
        return None;
    }
    let days = (pawn.received_at - case.window_end) as f64 / SECS_PER_DAY;
    let mk = |status, reason: String| {
        Some(PropertyMatch {
            case_id: case.id.clone(),
            item_id: item.id.clone(),
            pawn_id: pawn.id.clone(),
            status,
            reason,
            days_after_offense: days,
        })
    };
    let same_cat = item.category == pawn.category;
    let (si, sp) = (serial_of(&item.serial), serial_of(&pawn.serial));
    if let (Some(a), Some(b)) = (&si, &sp) {
        if a == b {
            return mk(
                MatchStatus::ExactSerialLead,
                format!("serial {} matches pawn record {}: a lead, ownership not verified", a, pawn.id),
            );
        }
        return if same_cat {
            mk(
                MatchStatus::RejectedConflictingSerial,
                format!("serials differ ({} vs {}): rejected regardless of description", a, b),
            )
        } else {
            None
        };
    }
    let item_marks: BTreeSet<String> =
        item.markings.iter().map(|m| m.trim().to_lowercase()).filter(|m| !m.is_empty()).collect();
    if let Some(m) = pawn.markings.iter().map(|m| m.trim().to_lowercase()).find(|m| item_marks.contains(m)) {
        return mk(
            MatchStatus::ReviewCandidateMarking,
            format!("shared marking \"{}\"; needs human review", m),
        );
    }
    if !same_cat {
        return None;
    }
    let brand = same_opt(&item.brand, &pawn.brand);
    let model_ok = same_opt(&item.model, &pawn.model) || item.model.is_none() || pawn.model.is_none();
    let j = jaccard(&item.description, &pawn.description);
    if (brand && model_ok) || j >= DESC_JACCARD_MIN {
        return mk(
            MatchStatus::UnresolvedSimilarDescription,
            format!("same category with similar brand/model/description (token overlap {:.2}); not a match", j),
        );
    }
    None
}

pub fn match_case(ds: &Dataset, case_id: &str) -> Vec<PropertyMatch> {
    let Some(case) = crate::find_case(ds, case_id) else {
        return vec![];
    };
    let mut out: Vec<PropertyMatch> = case
        .items
        .iter()
        .flat_map(|item| ds.pawn.iter().filter_map(move |p| compare(case, item, p)))
        .collect();
    out.sort_by(|a, b| {
        strength(a.status)
            .cmp(&strength(b.status))
            .then(a.days_after_offense.total_cmp(&b.days_after_offense))
            .then_with(|| (&a.item_id, &a.pawn_id).cmp(&(&b.item_id, &b.pawn_id)))
    });
    out
}

pub fn all_hits(ds: &Dataset) -> Vec<PropertyMatch> {
    ds.cases.iter().flat_map(|c| match_case(ds, &c.id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linkage::test_support::{case, DAY, T0};

    fn item(id: &str, serial: Option<&str>, marks: &[&str], desc: &str) -> StolenItem {
        StolenItem {
            id: id.into(),
            category: PropertyType::Laptop,
            brand: None,
            model: None,
            serial: serial.map(String::from),
            description: desc.into(),
            markings: marks.iter().map(|s| s.to_string()).collect(),
            value_usd: 500,
        }
    }
    fn pawn(id: &str, at: i64, serial: Option<&str>, marks: &[&str], desc: &str) -> PawnRecord {
        PawnRecord {
            id: id.into(),
            dealer: "D1".into(),
            received_at: at,
            location: GeoPoint { lat: 47.6, lon: -122.3 },
            category: PropertyType::Laptop,
            brand: None,
            model: None,
            serial: serial.map(String::from),
            description: desc.into(),
            markings: marks.iter().map(|s| s.to_string()).collect(),
            seller_ref: "S1".into(),
        }
    }
    fn ds(items: Vec<StolenItem>, pawns: Vec<PawnRecord>) -> Dataset {
        let mut c = case("C1", 47.6, -122.3, 0, 2);
        c.items = items;
        Dataset { cases: vec![c], pawn: pawns }
    }

    #[test]
    fn exact_serial_is_lead_with_normalization() {
        let d = ds(vec![item("I1", Some("ab-123"), &[], "x")], vec![pawn("P1", T0 + 5 * DAY, Some("AB 123"), &[], "y")]);
        let m = match_case(&d, "C1");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].status, MatchStatus::ExactSerialLead);
        assert!(m[0].days_after_offense > 4.0);
    }

    #[test]
    fn o_zero_is_not_conflated_and_conflict_rejected() {
        let d = ds(
            vec![item("I1", Some("SN0123"), &[], "silver laptop")],
            vec![pawn("P1", T0 + 5 * DAY, Some("SNO123"), &[], "silver laptop")],
        );
        let m = match_case(&d, "C1");
        assert_eq!(m[0].status, MatchStatus::RejectedConflictingSerial);
    }

    #[test]
    fn marking_candidate_without_serials() {
        let d = ds(vec![item("I1", None, &[" Sticker Fox "], "a")], vec![pawn("P1", T0 + 5 * DAY, None, &["sticker fox"], "b")]);
        assert_eq!(match_case(&d, "C1")[0].status, MatchStatus::ReviewCandidateMarking);
    }

    #[test]
    fn similar_description_and_no_match() {
        let d = ds(
            vec![item("I1", None, &[], "silver thin laptop")],
            vec![pawn("P1", T0 + 5 * DAY, None, &[], "silver thin laptop"), pawn("P2", T0 + 5 * DAY, None, &[], "red kayak")],
        );
        let m = match_case(&d, "C1");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].status, MatchStatus::UnresolvedSimilarDescription);
    }

    #[test]
    fn pre_offense_pawn_never_matches() {
        let d = ds(vec![item("I1", Some("AB1"), &[], "x")], vec![pawn("P1", T0 - DAY, Some("AB1"), &[], "x")]);
        assert!(all_hits(&d).is_empty());
    }

    #[test]
    fn sorted_by_strength_then_days() {
        let d = ds(
            vec![item("I1", Some("AB1"), &[], "silver thin laptop")],
            vec![
                pawn("P1", T0 + 9 * DAY, Some("ZZ9"), &[], "silver thin laptop"),
                pawn("P2", T0 + 9 * DAY, Some("AB1"), &[], "q"),
                pawn("P3", T0 + 4 * DAY, Some("AB1"), &[], "q"),
            ],
        );
        let ids: Vec<String> = match_case(&d, "C1").into_iter().map(|m| m.pawn_id).collect();
        assert_eq!(ids, ["P3", "P2", "P1"]);
        assert!(match_case(&d, "nope").is_empty());
    }
}
