//! Cost and clearance model on the OFFICIAL case baseline. Slide numbers come from here, never
//! from an LLM. Every input is a named, stated assumption; scenarios are planning brackets, not
//! confidence intervals.
//!
//! Accounting rules (adopted from the team's independent review):
//! - Freed hours are monetized as *capacity value*; cash savings require avoidable spend (overtime,
//!   unfilled detective positions). The deck must say which.
//! - Hours needed to work the new leads are a COST (funded follow-up), never also a saving.
//! - Year one uses a ramp on benefits but full recurring and one-time costs.

use serde::{Deserialize, Serialize};

/// Official case baseline (fictional Cascade City). Do not edit: these are given by the case.
pub mod baseline {
    pub const CASES_PER_YEAR: f64 = 10_000.0;
    pub const CLEARANCE_RATE: f64 = 0.13;
    pub const OFFICER_RATE: f64 = 50.0;
    pub const DETECTIVE_RATE: f64 = 70.0;
    pub const INTAKE_MIN: f64 = 15.0;
    pub const REPORT_MIN: f64 = 60.0;
    pub const VIDEO_MIN: f64 = 120.0;
    pub const EVIDENCE_MIN: f64 = 45.0;
    pub const DETECTIVE_MIN: f64 = 90.0;
    /// $305/case x 10,000 = $3.05M.
    pub const ANNUAL_LABOR: f64 = 3_050_000.0;
    /// 15% net target.
    pub const COST_TARGET: f64 = 457_500.0;
    /// 25% relative -> 16.25% -> +325.
    pub const CLEARANCE_TARGET_ADDITIONAL: f64 = 325.0;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostItem {
    pub name: String,
    pub usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    /// Share of cases where detectives actually use the packet.
    pub adoption: f64,
    /// Share of adopted cases the AI packet shows as no-lead (no solvability factors, no link,
    /// no property hit); a detective signs off the screening packet instead of a full review.
    pub screened_share: f64,
    /// Detective minutes on a screened case (sign-off, victim letter) instead of 90.
    pub screened_detective_min: f64,
    /// Detective minutes saved on the remaining adopted cases (packet pre-assembles
    /// cross-references, linked cases, property hits, next-steps checklist).
    pub packet_min_saved: f64,
    /// Officer minutes saved in evidence processing (structured stolen-item cataloging).
    pub evidence_min_saved: f64,
    // --- clearance pathway (new, verified, case-level leads) ---
    /// Share of baseline cleared cases where the engine surfaces >=1 verified linked open case.
    pub link_hit_rate: f64,
    /// Average verified linked open cases per hit.
    pub linked_open_per_hit: f64,
    /// Share of cases with traceable property (serial / unique marking).
    pub traceable_share: f64,
    /// Share of traceable cases whose items resurface in the pawn feed within the window.
    pub resurface_rate: f64,
    /// Moderate-solvability cases that newly get worked because packets free detective time.
    pub triage_leads: f64,
    /// Share of new leads whose follow-up is completed.
    pub completion: f64,
    /// Share of completed follow-ups that produce an incremental clearance.
    pub conversion: f64,
    /// Detective hours per completed follow-up, through disposition.
    pub followup_hours: f64,
    /// Clearances lost because screened cases get less detective time (honesty term).
    pub screening_clearance_loss: f64,
    // --- costs ---
    pub recurring: Vec<CostItem>,
    pub one_time: Vec<CostItem>,
    /// Benefit realised in year one relative to steady state.
    pub year_one_ramp: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostResult {
    pub scenario: String,
    pub adopted_cases: f64,
    pub detective_hours_saved: f64,
    pub officer_hours_saved: f64,
    pub gross_capacity_value: f64,
    pub recurring_cost: f64,
    pub one_time_cost: f64,
    pub new_leads: f64,
    pub leads_from_linkage: f64,
    pub leads_from_property: f64,
    pub leads_from_triage: f64,
    pub followup_hours: f64,
    pub followup_cost: f64,
    pub steady_net: f64,
    pub steady_net_pct: f64,
    pub meets_cost_target: bool,
    pub year_one_net: f64,
    pub year_one_roi: f64,
    /// Years from launch until cumulative net turns positive (None = never under fixed inputs).
    pub payback_years: Option<f64>,
    pub additional_clearances: f64,
    pub clearance_rate: f64,
    pub meets_clearance_target: bool,
}

fn sum(items: &[CostItem]) -> f64 {
    items.iter().map(|i| i.usd).sum()
}

pub fn compute(s: &Scenario) -> CostResult {
    use baseline::*;
    let adopted = CASES_PER_YEAR * s.adoption;
    let screened = adopted * s.screened_share;
    let remaining = adopted - screened;
    let det_min_saved = screened * (DETECTIVE_MIN - s.screened_detective_min).max(0.0)
        + remaining * s.packet_min_saved;
    let detective_hours_saved = det_min_saved / 60.0;
    let officer_hours_saved = adopted * s.evidence_min_saved / 60.0;
    let gross = detective_hours_saved * DETECTIVE_RATE + officer_hours_saved * OFFICER_RATE;

    let baseline_cleared = CASES_PER_YEAR * CLEARANCE_RATE;
    let leads_from_linkage = baseline_cleared * s.link_hit_rate * s.linked_open_per_hit;
    let leads_from_property = CASES_PER_YEAR * s.traceable_share * s.resurface_rate * s.adoption;
    let leads_from_triage = s.triage_leads;
    let new_leads = leads_from_linkage + leads_from_property + leads_from_triage;
    let completed = new_leads * s.completion;
    let followup_hours = completed * s.followup_hours;
    let followup_cost = followup_hours * DETECTIVE_RATE;

    let recurring = sum(&s.recurring);
    let one_time = sum(&s.one_time);
    let steady_net = gross - recurring - followup_cost;
    let year_one_net = gross * s.year_one_ramp - recurring - followup_cost - one_time;
    let year_one_cost = recurring + followup_cost + one_time;
    let payback_years = if year_one_net >= 0.0 {
        // Recovers within year one; approximate linear month.
        let monthly = (gross * s.year_one_ramp - recurring - followup_cost) / 12.0;
        if monthly > 0.0 { Some((one_time / monthly) / 12.0) } else { None }
    } else if steady_net > 0.0 {
        Some(1.0 + (-year_one_net) / steady_net)
    } else {
        None
    };
    let additional = completed * s.conversion - s.screening_clearance_loss;
    let clearance_rate = (baseline_cleared + additional) / CASES_PER_YEAR;

    CostResult {
        scenario: s.name.clone(),
        adopted_cases: adopted,
        detective_hours_saved,
        officer_hours_saved,
        gross_capacity_value: gross,
        recurring_cost: recurring,
        one_time_cost: one_time,
        new_leads,
        leads_from_linkage,
        leads_from_property,
        leads_from_triage,
        followup_hours,
        followup_cost,
        steady_net,
        steady_net_pct: steady_net / ANNUAL_LABOR,
        meets_cost_target: steady_net >= COST_TARGET,
        year_one_net,
        year_one_roi: year_one_net / year_one_cost,
        payback_years,
        additional_clearances: additional,
        clearance_rate,
        meets_clearance_target: additional >= CLEARANCE_TARGET_ADDITIONAL,
    }
}

fn items(v: &[(&str, f64)]) -> Vec<CostItem> {
    v.iter().map(|(n, u)| CostItem { name: (*n).into(), usd: *u }).collect()
}

/// Conservative / Base / Optimistic planning brackets. Edit here, re-run `casefile cost`.
pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "Conservative".into(),
            adoption: 0.70,
            screened_share: 0.30,
            screened_detective_min: 45.0,
            packet_min_saved: 10.0,
            evidence_min_saved: 5.0,
            link_hit_rate: 0.20,
            linked_open_per_hit: 1.2,
            traceable_share: 0.30,
            resurface_rate: 0.06,
            triage_leads: 60.0,
            completion: 0.60,
            conversion: 0.35,
            followup_hours: 3.0,
            screening_clearance_loss: 30.0,
            recurring: items(&[
                ("Model inference + hosting (CJIS-aligned cloud)", 30_000.0),
                ("Integration & support", 30_000.0),
                ("Security / audit", 15_000.0),
                ("Ongoing evaluation & training", 10_000.0),
            ]),
            one_time: items(&[
                ("RMS + pawn-feed integration", 80_000.0),
                ("Security & CJIS review", 30_000.0),
                ("Ground-truth evaluation set", 30_000.0),
                ("Training & change management", 20_000.0),
            ]),
            year_one_ramp: 0.50,
        },
        Scenario {
            name: "Base".into(),
            adoption: 0.85,
            screened_share: 0.40,
            screened_detective_min: 30.0,
            packet_min_saved: 20.0,
            evidence_min_saved: 10.0,
            link_hit_rate: 0.30,
            linked_open_per_hit: 1.5,
            traceable_share: 0.35,
            resurface_rate: 0.10,
            triage_leads: 120.0,
            completion: 0.65,
            conversion: 0.50,
            followup_hours: 2.0,
            screening_clearance_loss: 10.0,
            recurring: items(&[
                ("Model inference + hosting (CJIS-aligned cloud)", 12_000.0),
                ("Integration & support", 25_000.0),
                ("Security / audit", 15_000.0),
                ("Ongoing evaluation & training", 10_000.0),
            ]),
            one_time: items(&[
                ("RMS + pawn-feed integration", 60_000.0),
                ("Security & CJIS review", 25_000.0),
                ("Ground-truth evaluation set", 25_000.0),
                ("Training & change management", 20_000.0),
            ]),
            year_one_ramp: 0.65,
        },
        Scenario {
            name: "Optimistic".into(),
            adoption: 0.95,
            screened_share: 0.45,
            screened_detective_min: 20.0,
            packet_min_saved: 30.0,
            evidence_min_saved: 15.0,
            link_hit_rate: 0.35,
            linked_open_per_hit: 1.8,
            traceable_share: 0.40,
            resurface_rate: 0.12,
            triage_leads: 150.0,
            completion: 0.70,
            conversion: 0.50,
            followup_hours: 2.0,
            screening_clearance_loss: 0.0,
            recurring: items(&[
                ("Model inference + hosting (CJIS-aligned cloud)", 10_000.0),
                ("Integration & support", 20_000.0),
                ("Security / audit", 12_000.0),
                ("Ongoing evaluation & training", 8_000.0),
            ]),
            one_time: items(&[
                ("RMS + pawn-feed integration", 50_000.0),
                ("Security & CJIS review", 20_000.0),
                ("Ground-truth evaluation set", 20_000.0),
                ("Training & change management", 20_000.0),
            ]),
            year_one_ramp: 0.75,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_name(n: &str) -> CostResult {
        compute(scenarios().iter().find(|s| s.name == n).unwrap())
    }

    #[test]
    fn baseline_matches_case() {
        use baseline::*;
        let per_case = (INTAKE_MIN + REPORT_MIN + VIDEO_MIN + EVIDENCE_MIN) / 60.0 * OFFICER_RATE
            + DETECTIVE_MIN / 60.0 * DETECTIVE_RATE;
        assert!((per_case - 305.0).abs() < 1e-9);
        assert!((per_case * CASES_PER_YEAR - ANNUAL_LABOR).abs() < 1e-6);
        assert!((ANNUAL_LABOR * 0.15 - COST_TARGET).abs() < 1e-6);
    }

    #[test]
    fn base_scenario_hand_check() {
        let r = by_name("Base");
        // 8,500 adopted; 3,400 screened x 60 min + 5,100 x 20 min = 5,100 detective hours.
        assert!((r.detective_hours_saved - 5_100.0).abs() < 1e-6);
        // 8,500 x 10 min = 1,416.67 officer hours.
        assert!((r.officer_hours_saved - 8_500.0 * 10.0 / 60.0).abs() < 1e-6);
        // Leads: 1,300 x .3 x 1.5 = 585; 10,000 x .35 x .10 x .85 = 297.5; + 120.
        assert!((r.new_leads - 1_002.5).abs() < 1e-6);
        assert!(r.steady_net > 0.0 && !r.meets_cost_target);
    }

    #[test]
    fn optimistic_meets_both_targets_conservative_meets_neither() {
        let o = by_name("Optimistic");
        assert!(o.meets_cost_target && o.meets_clearance_target);
        let c = by_name("Conservative");
        assert!(!c.meets_cost_target && !c.meets_clearance_target);
    }

    #[test]
    fn followup_is_cost_not_saving() {
        let mut s = scenarios().into_iter().find(|s| s.name == "Base").unwrap();
        let before = compute(&s).steady_net;
        s.followup_hours *= 2.0;
        let after = compute(&s).steady_net;
        assert!(after < before);
    }
}
