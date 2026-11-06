//! `casefile cost`: compute scenarios on the official baseline; print a table and write JSON for
//! the deck charts.
use casefile_core::cost::{self, baseline};

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let out = crate::flag(args, "--out").unwrap_or_else(|| "docs/deck/cost.json".into());
    let scenarios = cost::scenarios();
    let results: Vec<_> = scenarios.iter().map(cost::compute).collect();
    println!(
        "{:<13}{:>12}{:>12}{:>10}{:>13}{:>9}{:>9}{:>10}",
        "scenario", "gross", "steady_net", "net_%", "year1_net", "payback", "+clear", "rate"
    );
    for r in &results {
        println!(
            "{:<13}{:>12.0}{:>12.0}{:>9.2}%{:>13.0}{:>9}{:>9.1}{:>9.2}%",
            r.scenario,
            r.gross_capacity_value,
            r.steady_net,
            r.steady_net_pct * 100.0,
            r.year_one_net,
            r.payback_years.map(|y| format!("{y:.2}y")).unwrap_or_else(|| "never".into()),
            r.additional_clearances,
            r.clearance_rate * 100.0
        );
    }
    println!(
        "targets: steady net >= ${:.0} (15% of ${:.0}); +{:.0} clearances",
        baseline::COST_TARGET,
        baseline::ANNUAL_LABOR,
        baseline::CLEARANCE_TARGET_ADDITIONAL
    );
    let path = std::path::Path::new(&out);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let doc = serde_json::json!({
        "baseline": {
            "cases": baseline::CASES_PER_YEAR,
            "annual_labor": baseline::ANNUAL_LABOR,
            "officer_rate": baseline::OFFICER_RATE,
            "detective_rate": baseline::DETECTIVE_RATE,
            "cost_target": baseline::COST_TARGET,
            "clearance_target_additional": baseline::CLEARANCE_TARGET_ADDITIONAL
        },
        "scenarios": scenarios,
        "results": results,
    });
    std::fs::write(path, serde_json::to_string_pretty(&doc)?)?;
    eprintln!("wrote {}", path.display());
    Ok(())
}
