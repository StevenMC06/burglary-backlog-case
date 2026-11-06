//! `verify_series`: the deterministic yes/no "grid" for a series hypothesis.
//!
//! SPEC (implementer fills bodies, keep signatures). Checks, in this order:
//! 1. HARD `spatiotemporal_feasibility`: for every pair, one offender must be able to travel
//!    between scenes. Best case = the largest separation the two windows allow (earliest start of
//!    one to latest end of the other); it must cover distance_km / MAX_URBAN_SPEED_KMH plus
//!    MIN_ON_SCENE_MIN. Only FAIL when even the best-case timing is infeasible.
//! 2. HARD `conflicting_clearance`: two cases cleared by arrest with different arrestee_ref => FAIL.
//! 3. SOFT `mo_consistency`: mean pairwise mo_similarity; < 0.35 Fail(soft->Weak), < 0.55 Weak.
//! 4. SOFT `property_consistency`: some property type must recur across cases; else Weak.
//! 5. SOFT `geographic_spread`: max pairwise distance > 12 km => Weak (burglars commute short).
//!
//! Verdict: Fail if any HARD check fails; Weak if any soft check is Weak/Fail; else Pass.
//! `offending_cases`: for each hard pair conflict, the case with the lower mean pair_score to the
//! rest (ties: later id). Unknown case ids => single HARD Fail check "unknown_case".
use crate::find_case;
use crate::geo::haversine_km;
use crate::linkage::{mo_similarity, pair_score};
use crate::types::*;

/// Assumed best-case urban door-to-door speed between scenes.
pub const MAX_URBAN_SPEED_KMH: f64 = 40.0;
/// Minimum minutes an offender needs at a scene (entry, search, exit).
pub const MIN_ON_SCENE_MIN: f64 = 10.0;
/// Mean MO similarity below this is a failed soft check (verdict Weak overall).
pub const MO_FAIL_BELOW: f64 = 0.35;
/// Mean MO similarity below this is a weak soft check.
pub const MO_WEAK_BELOW: f64 = 0.55;
/// Burglars commute short; a series spread wider than this is weak.
pub const MAX_SPREAD_KM: f64 = 12.0;

fn cite(case: &Case, field: &str) -> Citation {
    Citation { record_id: case.id.clone(), field: field.to_string() }
}

fn check(name: &str, verdict: Verdict, reason: String, cited: Vec<Citation>) -> Check {
    Check { name: name.to_string(), verdict, reason, cited }
}

/// Best-case seconds available for one offender to get from one offense to the other: the largest
/// separation the two windows allow, whichever order is more generous. Overlapping windows still
/// give a positive value because the offenses need not be simultaneous.
fn best_case_gap_secs(a: &Case, b: &Case) -> f64 {
    let g1 = b.window_end - a.window_start;
    let g2 = a.window_end - b.window_start;
    g1.max(g2).max(0) as f64
}

/// Mean pair score of `id` against every other case in the series.
fn mean_score_to_rest(cases: &[&Case], id: &str) -> f64 {
    let Some(me) = cases.iter().find(|c| c.id == id) else {
        return 0.0;
    };
    let others: Vec<f64> =
        cases.iter().filter(|c| c.id != id).map(|c| pair_score(me, c).score).collect();
    if others.is_empty() { 0.0 } else { others.iter().sum::<f64>() / others.len() as f64 }
}

/// The weaker-linked case of a conflicting pair (ties go to the later id).
fn weaker<'a>(cases: &[&Case], a: &'a str, b: &'a str) -> &'a str {
    let (sa, sb) = (mean_score_to_rest(cases, a), mean_score_to_rest(cases, b));
    if sa < sb || (sa == sb && a > b) { a } else { b }
}

pub fn verify_series(ds: &Dataset, case_ids: &[String]) -> VerifyResult {
    let mut cases: Vec<&Case> = Vec::new();
    let mut unknown: Vec<&String> = Vec::new();
    for id in case_ids {
        match find_case(ds, id) {
            Some(c) => cases.push(c),
            None => unknown.push(id),
        }
    }
    if !unknown.is_empty() {
        let names: Vec<&str> = unknown.iter().map(|s| s.as_str()).collect();
        return VerifyResult {
            case_ids: case_ids.to_vec(),
            verdict: Verdict::Fail,
            checks: vec![check(
                "unknown_case",
                Verdict::Fail,
                format!("unknown case id(s) not in the dataset: {}", names.join(", ")),
                vec![],
            )],
            offending_cases: unknown.iter().map(|s| s.to_string()).collect(),
            pairwise: vec![],
        };
    }

    let mut checks = Vec::new();
    let mut offending: Vec<String> = Vec::new();
    let mut pairwise = Vec::new();
    let mut pair_list: Vec<(&Case, &Case)> = Vec::new();
    for (i, a) in cases.iter().enumerate() {
        for b in &cases[i + 1..] {
            pair_list.push((a, b));
            pairwise.push(pair_score(a, b));
        }
    }

    // 1. HARD spatiotemporal feasibility.
    let mut infeasible = Vec::new();
    for (a, b) in &pair_list {
        let km = haversine_km(a.location, b.location);
        let need_min = km / MAX_URBAN_SPEED_KMH * 60.0 + MIN_ON_SCENE_MIN;
        let have_min = best_case_gap_secs(a, b) / 60.0;
        if have_min < need_min {
            infeasible.push(format!(
                "{} and {} are {:.1} km apart; best-case time between offenses is {:.0} min but travel at {:.0} km/h plus {:.0} min on scene needs {:.0} min: infeasible",
                a.id, b.id, km, have_min, MAX_URBAN_SPEED_KMH, MIN_ON_SCENE_MIN, need_min
            ));
            let w = weaker(&cases, &a.id, &b.id).to_string();
            if !offending.contains(&w) {
                offending.push(w);
            }
        }
    }
    if infeasible.is_empty() {
        checks.push(check(
            "spatiotemporal_feasibility",
            Verdict::Pass,
            format!("all {} pair(s) are reachable by one offender in the best case", pair_list.len()),
            vec![],
        ));
    } else {
        let cited = cases
            .iter()
            .flat_map(|c| [cite(c, "window_start"), cite(c, "window_end"), cite(c, "location")])
            .collect();
        checks.push(check("spatiotemporal_feasibility", Verdict::Fail, infeasible.join("; "), cited));
    }

    // 2. HARD conflicting clearance.
    let mut conflicts = Vec::new();
    for (a, b) in &pair_list {
        if let (
            CaseStatus::ClearedByArrest { arrestee_ref: x },
            CaseStatus::ClearedByArrest { arrestee_ref: y },
        ) = (&a.status, &b.status)
            && x != y
        {
            conflicts.push(format!(
                "{} was cleared by arrest of {} but {} by arrest of {}: one offender cannot be two arrestees",
                a.id, x, b.id, y
            ));
            let w = weaker(&cases, &a.id, &b.id).to_string();
            if !offending.contains(&w) {
                offending.push(w);
            }
        }
    }
    if conflicts.is_empty() {
        checks.push(check("conflicting_clearance", Verdict::Pass, "no conflicting arrestees".into(), vec![]));
    } else {
        let cited = cases.iter().map(|c| cite(c, "status")).collect();
        checks.push(check("conflicting_clearance", Verdict::Fail, conflicts.join("; "), cited));
    }

    // 3. SOFT MO consistency.
    if pair_list.is_empty() {
        checks.push(check("mo_consistency", Verdict::Pass, "fewer than two cases: nothing to compare".into(), vec![]));
    } else {
        let mean = pair_list.iter().map(|(a, b)| mo_similarity(a, b)).sum::<f64>() / pair_list.len() as f64;
        let v = if mean < MO_FAIL_BELOW {
            Verdict::Fail
        } else if mean < MO_WEAK_BELOW {
            Verdict::Weak
        } else {
            Verdict::Pass
        };
        checks.push(check(
            "mo_consistency",
            v,
            format!(
                "mean pairwise MO similarity {:.2} (weak below {:.2}, failing below {:.2})",
                mean, MO_WEAK_BELOW, MO_FAIL_BELOW
            ),
            cases.iter().map(|c| cite(c, "mo")).collect(),
        ));
    }

    // 4. SOFT property consistency: some property type must recur across cases.
    let with_mo: Vec<&Case> = cases.iter().copied().filter(|c| c.mo.is_some()).collect();
    if with_mo.len() < 2 {
        checks.push(check(
            "property_consistency",
            Verdict::Pass,
            "fewer than two cases with an MO: nothing to compare".into(),
            vec![],
        ));
    } else {
        let shared = with_mo.iter().filter_map(|c| c.mo.as_ref()).flat_map(|m| m.property_types.iter()).any(|t| {
            with_mo.iter().filter(|c| c.mo.as_ref().is_some_and(|m| m.property_types.contains(t))).count() >= 2
        });
        let (v, reason) = if shared {
            (Verdict::Pass, "at least one stolen property type recurs across cases".to_string())
        } else {
            (Verdict::Weak, "no stolen property type is shared by any two cases".to_string())
        };
        checks.push(check(
            "property_consistency",
            v,
            reason,
            with_mo.iter().map(|c| cite(c, "mo.property_types")).collect(),
        ));
    }

    // 5. SOFT geographic spread.
    let max_km = pair_list.iter().map(|(a, b)| haversine_km(a.location, b.location)).fold(0.0_f64, f64::max);
    let (v, reason) = if max_km > MAX_SPREAD_KM {
        (Verdict::Weak, format!("widest pair is {:.1} km apart (> {:.0} km): burglars commute short", max_km, MAX_SPREAD_KM))
    } else {
        (Verdict::Pass, format!("widest pair is {:.1} km apart (<= {:.0} km)", max_km, MAX_SPREAD_KM))
    };
    checks.push(check("geographic_spread", v, reason, cases.iter().map(|c| cite(c, "location")).collect()));

    let hard = ["spatiotemporal_feasibility", "conflicting_clearance"];
    let verdict = if checks.iter().any(|c| hard.contains(&c.name.as_str()) && c.verdict == Verdict::Fail) {
        Verdict::Fail
    } else if checks.iter().any(|c| c.verdict != Verdict::Pass) {
        Verdict::Weak
    } else {
        Verdict::Pass
    };
    offending.sort();
    VerifyResult { case_ids: case_ids.to_vec(), verdict, checks, offending_cases: offending, pairwise }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linkage::test_support::{case, HOUR};

    fn ds(cases: Vec<Case>) -> Dataset {
        Dataset { cases, pawn: vec![] }
    }
    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    fn arrested(mut c: Case, who: &str) -> Case {
        c.status = CaseStatus::ClearedByArrest { arrestee_ref: who.into() };
        c
    }

    #[test]
    fn infeasible_pair_fails() {
        // ~55 km apart, identical 1h windows: needs ~93 min, best case is 60.
        let d = ds(vec![case("A", 47.10, -122.30, 0, 1), case("B", 47.60, -122.30, 0, 1)]);
        let r = verify_series(&d, &ids(&["A", "B"]));
        assert_eq!(r.verdict, Verdict::Fail);
        assert_eq!(r.offending_cases.len(), 1);
        assert!(r.checks.iter().any(|c| c.name == "spatiotemporal_feasibility" && c.verdict == Verdict::Fail));
    }

    #[test]
    fn conflicting_arrestees_fail() {
        let d = ds(vec![
            arrested(case("A", 47.60, -122.30, 0, 1), "X1"),
            arrested(case("B", 47.601, -122.30, 48 * HOUR, 1), "X2"),
        ]);
        let r = verify_series(&d, &ids(&["A", "B"]));
        assert_eq!(r.verdict, Verdict::Fail);
        assert!(r.checks.iter().any(|c| c.name == "conflicting_clearance" && c.verdict == Verdict::Fail));
        let d = ds(vec![
            arrested(case("A", 47.60, -122.30, 0, 1), "X1"),
            arrested(case("B", 47.601, -122.30, 48 * HOUR, 1), "X1"),
        ]);
        assert_eq!(verify_series(&d, &ids(&["A", "B"])).verdict, Verdict::Pass);
    }

    #[test]
    fn clean_series_passes() {
        let d = ds(vec![
            case("A", 47.60, -122.30, 0, 2),
            case("B", 47.61, -122.30, 48 * HOUR, 2),
            case("C", 47.62, -122.30, 96 * HOUR, 2),
        ]);
        let r = verify_series(&d, &ids(&["A", "B", "C"]));
        assert_eq!(r.verdict, Verdict::Pass, "{:?}", r.checks);
        assert!(r.offending_cases.is_empty());
        assert_eq!(r.pairwise.len(), 3);
    }

    #[test]
    fn wide_spread_is_weak_and_unknown_case_fails() {
        let d = ds(vec![case("A", 47.40, -122.30, 0, 2), case("B", 47.70, -122.30, 48 * HOUR, 2)]);
        assert_eq!(verify_series(&d, &ids(&["A", "B"])).verdict, Verdict::Weak);
        let r = verify_series(&d, &ids(&["A", "ZZ"]));
        assert_eq!(r.verdict, Verdict::Fail);
        assert_eq!(r.checks.len(), 1);
        assert_eq!(r.checks[0].name, "unknown_case");
    }
}
