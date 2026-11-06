//! Explainable solvability score (0-100) from SolvabilityFacts, MO and status.
//!
//! SPEC: logistic model with NAMED, hand-set coefficients per factor (witness,
//! suspect_description, vehicle_description, video_available, physical_evidence,
//! traceable_property, window_hours (negative, log-scaled), plus `linked_to_series` and
//! `property_hit` passed in as context). score = 100 * sigmoid(intercept + sum).
//!
//! Contributions are computed by adding the factors one at a time in a fixed order to the
//! intercept-only score, so they sum EXACTLY to `score - baseline` (the intercept-only score) and
//! explain the number. They are sorted by |value| desc. Bands: < 25 Weak, < 55 Moderate, else
//! Strong. Deterministic.
//!
//! Coefficients are hand-set to follow the direction and rough ordering reported in the
//! solvability literature (Eck 1983; the Greater Manchester replication): a witness, a suspect or
//! vehicle description, video and physical evidence each raise solvability; a long unknown offense
//! window lowers it. No fitting is performed (scope trim: no `fit()`).
use crate::types::*;

/// Intercept-only logit (about a 27% baseline for a burglary with no leads).
pub const INTERCEPT: f64 = -1.0;
pub const C_WITNESS: f64 = 0.9;
pub const C_SUSPECT_DESCRIPTION: f64 = 0.8;
pub const C_VEHICLE_DESCRIPTION: f64 = 0.6;
pub const C_VIDEO: f64 = 0.7;
pub const C_PHYSICAL_EVIDENCE: f64 = 0.5;
pub const C_TRACEABLE_PROPERTY: f64 = 0.4;
/// Multiplies ln(1 + window_hours); negative: wider window, harder case.
pub const C_WINDOW_LN_HOURS: f64 = -0.3;
pub const C_LINKED_TO_SERIES: f64 = 0.5;
pub const C_PROPERTY_HIT: f64 = 1.2;
/// Score below this is Weak.
pub const BAND_WEAK_BELOW: f64 = 25.0;
/// Score below this is Moderate; at or above is Strong.
pub const BAND_MODERATE_BELOW: f64 = 55.0;

#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    pub linked_to_series: bool,
    pub property_hit: bool,
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z.clamp(-40.0, 40.0)).exp())
}

pub fn score(case: &Case, ctx: Context) -> SolvabilityScore {
    let f = &case.facts;
    let flag = |b: bool, c: f64| if b { c } else { 0.0 };
    let hours = if f.window_hours.is_finite() { f.window_hours.max(0.0) } else { 0.0 };
    let terms: [(&str, f64); 9] = [
        ("witness", flag(f.witness, C_WITNESS)),
        ("suspect_description", flag(f.suspect_description, C_SUSPECT_DESCRIPTION)),
        ("vehicle_description", flag(f.vehicle_description, C_VEHICLE_DESCRIPTION)),
        ("video_available", flag(f.video_available, C_VIDEO)),
        ("physical_evidence", flag(f.physical_evidence, C_PHYSICAL_EVIDENCE)),
        ("traceable_property", flag(f.traceable_property, C_TRACEABLE_PROPERTY)),
        ("window_hours", C_WINDOW_LN_HOURS * (1.0 + hours).ln()),
        ("linked_to_series", flag(ctx.linked_to_series, C_LINKED_TO_SERIES)),
        ("property_hit", flag(ctx.property_hit, C_PROPERTY_HIT)),
    ];
    let mut z = INTERCEPT;
    let mut prev = 100.0 * sigmoid(z);
    let mut contributions: Vec<(String, f64)> = Vec::with_capacity(terms.len());
    for (name, t) in terms {
        z += t;
        let now = 100.0 * sigmoid(z);
        contributions.push((name.to_string(), now - prev));
        prev = now;
    }
    contributions.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
    let score = 100.0 * sigmoid(z);
    let band = if score < BAND_WEAK_BELOW {
        SolvabilityBand::Weak
    } else if score < BAND_MODERATE_BELOW {
        SolvabilityBand::Moderate
    } else {
        SolvabilityBand::Strong
    };
    SolvabilityScore { case_id: case.id.clone(), score, band, contributions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linkage::test_support::case;

    #[test]
    fn deterministic_and_baseline_weak() {
        let c = case("A", 47.6, -122.3, 0, 8);
        let a = score(&c, Context::default());
        assert_eq!(a, score(&c, Context::default()));
        assert_eq!(a.band, SolvabilityBand::Weak);
        assert!(a.score > 0.0 && a.score < 100.0);
    }

    #[test]
    fn bands_follow_evidence() {
        let mut c = case("A", 47.6, -122.3, 0, 8);
        c.facts.witness = true;
        assert_eq!(score(&c, Context::default()).band, SolvabilityBand::Moderate);
        c.facts.video_available = true;
        c.facts.physical_evidence = true;
        c.facts.suspect_description = true;
        assert_eq!(score(&c, Context { linked_to_series: true, property_hit: true }).band, SolvabilityBand::Strong);
    }

    #[test]
    fn contributions_explain_score_and_sort() {
        let mut c = case("A", 47.6, -122.3, 0, 24);
        c.facts.witness = true;
        c.facts.video_available = true;
        let s = score(&c, Context { linked_to_series: false, property_hit: true });
        let baseline = 100.0 * sigmoid(INTERCEPT);
        let sum: f64 = s.contributions.iter().map(|(_, v)| v).sum();
        assert!((baseline + sum - s.score).abs() < 1e-9);
        assert!(s.contributions.windows(2).all(|w| w[0].1.abs() >= w[1].1.abs()));
        assert!(s.contributions.iter().any(|(n, v)| n == "window_hours" && *v < 0.0));
        assert!(s.contributions.iter().any(|(n, v)| n == "property_hit" && *v > 0.0));
    }
}
