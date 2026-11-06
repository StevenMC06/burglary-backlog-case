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
use std::collections::{BTreeSet, HashMap};

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

/// An item with its serial, markings and description normalised once.
struct ItemView<'a> {
    item: &'a StolenItem,
    serial: Option<String>,
    marks: BTreeSet<String>,
    tokens: BTreeSet<String>,
}

impl<'a> ItemView<'a> {
    fn new(item: &'a StolenItem) -> Self {
        ItemView {
            item,
            serial: serial_of(&item.serial),
            marks: item.markings.iter().map(|m| m.trim().to_lowercase()).filter(|m| !m.is_empty()).collect(),
            tokens: tokens(&item.description),
        }
    }
}

/// A pawn record with its serial, markings (in record order) and description normalised once.
struct PawnView<'a> {
    pawn: &'a PawnRecord,
    serial: Option<String>,
    marks: Vec<String>,
    tokens: BTreeSet<String>,
}

impl<'a> PawnView<'a> {
    fn new(pawn: &'a PawnRecord) -> Self {
        PawnView {
            pawn,
            serial: serial_of(&pawn.serial),
            marks: pawn.markings.iter().map(|m| m.trim().to_lowercase()).collect(),
            tokens: tokens(&pawn.description),
        }
    }
}

fn jaccard_sets(x: &BTreeSet<String>, y: &BTreeSet<String>) -> f64 {
    let inter = x.intersection(y).count();
    let union = x.len() + y.len() - inter;
    if union == 0 { 0.0 } else { inter as f64 / union as f64 }
}

pub fn compare(case: &Case, item: &StolenItem, pawn: &PawnRecord) -> Option<PropertyMatch> {
    compare_views(case, &ItemView::new(item), &PawnView::new(pawn))
}

/// The matching rules (module docs), on pre-normalised views. Single source of truth for both
/// [`compare`] and the indexed batch path.
fn compare_views(case: &Case, iv: &ItemView, pv: &PawnView) -> Option<PropertyMatch> {
    let (item, pawn) = (iv.item, pv.pawn);
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
    if let (Some(a), Some(b)) = (&iv.serial, &pv.serial) {
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
    if let Some(m) = pv.marks.iter().find(|m| iv.marks.contains(*m)) {
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
    let j = jaccard_sets(&iv.tokens, &pv.tokens);
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

/// Pawn records normalised once and bucketed so each stolen item is compared only with records
/// that can possibly match it. A record can match an item only if it (a) shares the item's
/// category (needed for a conflicting-serial or similar-description result), (b) carries the same
/// normalised serial (an exact-serial lead, any category), or (c) carries one of the item's
/// markings (a marking candidate, any category). Every other record returns no match under the
/// rules, so skipping it loses nothing. Candidates are visited in record order, so the output is
/// identical to the brute-force scan (pinned by tests).
pub struct PropertyIndex<'a> {
    views: Vec<PawnView<'a>>,
    by_category: HashMap<PropertyType, Vec<usize>>,
    by_serial: HashMap<String, Vec<usize>>,
    by_marking: HashMap<String, Vec<usize>>,
}

impl<'a> PropertyIndex<'a> {
    pub fn new(ds: &'a Dataset) -> Self {
        let views: Vec<PawnView> = ds.pawn.iter().map(PawnView::new).collect();
        let (mut by_category, mut by_serial, mut by_marking) =
            (HashMap::<_, Vec<usize>>::new(), HashMap::<_, Vec<usize>>::new(), HashMap::<_, Vec<usize>>::new());
        for (k, v) in views.iter().enumerate() {
            by_category.entry(v.pawn.category).or_default().push(k);
            if let Some(s) = &v.serial {
                by_serial.entry(s.clone()).or_default().push(k);
            }
            for m in v.marks.iter().filter(|m| !m.is_empty()) {
                by_marking.entry(m.clone()).or_default().push(k);
            }
        }
        PropertyIndex { views, by_category, by_serial, by_marking }
    }

    /// Same result as [`match_case`] for this case.
    pub fn match_case(&self, case: &Case) -> Vec<PropertyMatch> {
        let mut out = Vec::new();
        let mut cand: Vec<usize> = Vec::new();
        for item in &case.items {
            let iv = ItemView::new(item);
            cand.clear();
            cand.extend(self.by_category.get(&item.category).into_iter().flatten());
            if let Some(s) = &iv.serial {
                cand.extend(self.by_serial.get(s).into_iter().flatten());
            }
            for m in &iv.marks {
                cand.extend(self.by_marking.get(m).into_iter().flatten());
            }
            cand.sort_unstable();
            cand.dedup();
            out.extend(cand.iter().filter_map(|&k| compare_views(case, &iv, &self.views[k])));
        }
        sort_matches(&mut out);
        out
    }
}

fn sort_matches(out: &mut [PropertyMatch]) {
    out.sort_by(|a, b| {
        strength(a.status)
            .cmp(&strength(b.status))
            .then(a.days_after_offense.total_cmp(&b.days_after_offense))
            .then_with(|| (&a.item_id, &a.pawn_id).cmp(&(&b.item_id, &b.pawn_id)))
    });
}

/// Every match for every case, via [`PropertyIndex`] (built once).
pub fn all_hits(ds: &Dataset) -> Vec<PropertyMatch> {
    let idx = PropertyIndex::new(ds);
    // match_case looks a case up by id (first wins), so duplicate ids resolve the same way here.
    let mut first: HashMap<&str, &Case> = HashMap::new();
    for c in &ds.cases {
        first.entry(c.id.as_str()).or_insert(c);
    }
    ds.cases.iter().flat_map(|c| idx.match_case(first.get(c.id.as_str()).copied().unwrap_or(c))).collect()
}

/// Brute-force reference for [`all_hits`]: every item against every record. Oracle for tests and
/// `casefile bench`.
#[doc(hidden)]
pub fn all_hits_reference(ds: &Dataset) -> Vec<PropertyMatch> {
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
    /// Pseudo-random cases and pawn records exercising every rule branch: shared and conflicting
    /// serials (with formatting noise), cross-category serials, markings with case/space noise,
    /// empty markings, brand/model look-alikes, description overlap and pre-offense records.
    fn fuzz(n_cases: usize, n_pawn: usize, seed: u64) -> Dataset {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut rnd = move |m: u64| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s % m
        };
        let cats = [PropertyType::Laptop, PropertyType::Phone, PropertyType::Jewelry];
        let serials = ["AB-123", "ab123", "SN0123", "SNO123", "ZZ9", ""];
        let marks = ["Sticker Fox", " sticker fox", "engraved J", "", "blue tape"];
        let words = ["silver", "thin", "laptop", "gold", "ring", "black", "phone", "case"];
        let desc = |r: &mut dyn FnMut(u64) -> u64| (0..1 + r(4)).map(|_| words[r(8) as usize]).collect::<Vec<_>>().join(" ");
        let mut cases = vec![];
        for i in 0..n_cases {
            let mut c = case(&format!("C{i:04}"), 47.6, -122.3, rnd(60) as i64 * DAY, 4);
            c.items = (0..rnd(4))
                .map(|k| StolenItem {
                    id: format!("C{i:04}-I{k}"),
                    category: cats[rnd(3) as usize],
                    brand: [None, Some("Acme".to_string()), Some(" acme".to_string())][rnd(3) as usize].clone(),
                    model: [None, Some("X1".to_string())][rnd(2) as usize].clone(),
                    serial: (rnd(2) == 0).then(|| serials[rnd(6) as usize].to_string()),
                    description: desc(&mut rnd),
                    markings: (0..rnd(3)).map(|_| marks[rnd(5) as usize].to_string()).collect(),
                    value_usd: 100,
                })
                .collect();
            cases.push(c);
        }
        let pawn = (0..n_pawn)
            .map(|k| PawnRecord {
                id: format!("P{k:04}"),
                dealer: "D1".into(),
                received_at: T0 + (rnd(80) as i64 - 10) * DAY,
                location: GeoPoint { lat: 47.6, lon: -122.3 },
                category: cats[rnd(3) as usize],
                brand: [None, Some("ACME".to_string())][rnd(2) as usize].clone(),
                model: [None, Some("x1 ".to_string()), Some("X2".to_string())][rnd(3) as usize].clone(),
                serial: (rnd(2) == 0).then(|| serials[rnd(6) as usize].to_string()),
                description: desc(&mut rnd),
                markings: (0..rnd(3)).map(|_| marks[rnd(5) as usize].to_string()).collect(),
                seller_ref: "S1".into(),
            })
            .collect();
        Dataset { cases, pawn }
    }

    #[test]
    fn indexed_all_hits_equal_brute_force() {
        for seed in 1..6 {
            let d = fuzz(120, 150, seed);
            let fast = all_hits(&d);
            assert_eq!(fast, all_hits_reference(&d), "seed {seed}");
            // The fuzz must reach every outcome, or the equality above proves little.
            for st in [
                MatchStatus::ExactSerialLead,
                MatchStatus::RejectedConflictingSerial,
                MatchStatus::ReviewCandidateMarking,
                MatchStatus::UnresolvedSimilarDescription,
            ] {
                assert!(fast.iter().any(|m| m.status == st), "seed {seed} never produced {st:?}");
            }
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
