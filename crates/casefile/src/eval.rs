//! `casefile eval`: score the system against the synthetic ground truth.
//!
//!   casefile eval                      full report -> evals/report.md + evals/results.json
//!   casefile eval --make-subsets       write data/seed/eval_subsets.json (inputs for the
//!                                      LLM-only baseline recordings) and exit
//!
//! Measures:
//! 1. Series recovery (engine clusters, then verify_series pruning) vs planted series:
//!    pairwise precision/recall/F1 and "series recovered" (best Jaccard >= 0.5).
//! 2. Decoys: share rejected by verify_series (hard-constraint Fail).
//! 3. Property matching vs pawn_truth: exact-serial lead precision/recall; traps not promoted.
//! 4. Extraction accuracy of recorded extractor runs vs mo_truth (per field).
//! 5. "The wall": LLM-only series finding (recorded runs, agent `series_llm_only`) vs Casefile on
//!    the same subsets of 20 / 60 / 150 cases.
//!
//! Ground truth is read here and nowhere else.

use crate::store::Store;
use casefile_core::{linkage, property, verify, Dataset, GroundTruth, MatchStatus, Verdict};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Operating point picked from the sweep in the report (same synthetic data: optimistic).
const THRESHOLD: f64 = 0.9;
const SUBSET_SIZES: [usize; 3] = [20, 60, 150];

type Pair = (String, String);

fn pairs_of(groups: &[Vec<String>]) -> BTreeSet<Pair> {
    let mut out = BTreeSet::new();
    for g in groups {
        let mut g = g.clone();
        g.sort();
        for i in 0..g.len() {
            for j in i + 1..g.len() {
                out.insert((g[i].clone(), g[j].clone()));
            }
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Prf {
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub predicted_pairs: usize,
    pub true_pairs: usize,
    pub series_recovered: usize,
    pub series_total: usize,
}

fn jaccard(a: &[String], b: &[String]) -> f64 {
    let a: BTreeSet<_> = a.iter().collect();
    let b: BTreeSet<_> = b.iter().collect();
    let inter = a.intersection(&b).count() as f64;
    let uni = a.union(&b).count() as f64;
    if uni == 0.0 { 0.0 } else { inter / uni }
}

fn prf(pred: &[Vec<String>], truth: &[Vec<String>]) -> Prf {
    let p = pairs_of(pred);
    let t = pairs_of(truth);
    let tp = p.intersection(&t).count() as f64;
    let precision = if p.is_empty() { 0.0 } else { tp / p.len() as f64 };
    let recall = if t.is_empty() { 0.0 } else { tp / t.len() as f64 };
    let f1 = if precision + recall == 0.0 { 0.0 } else { 2.0 * precision * recall / (precision + recall) };
    let series_recovered = truth
        .iter()
        .filter(|s| pred.iter().any(|g| jaccard(g, s) >= 0.5))
        .count();
    Prf {
        precision,
        recall,
        f1,
        predicted_pairs: p.len(),
        true_pairs: t.len(),
        series_recovered,
        series_total: truth.len(),
    }
}

/// Engine clusters, then drop hard-failing cases flagged by verify_series (one pruning pass).
fn casefile_groups(ds: &Dataset) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
    casefile_groups_at(ds, THRESHOLD)
}

fn casefile_groups_at(ds: &Dataset, threshold: f64) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
    let raw: Vec<Vec<String>> =
        linkage::propose_clusters(ds, threshold).into_iter().map(|h| h.case_ids).collect();
    let mut pruned = vec![];
    for g in &raw {
        let v = verify::verify_series(ds, g);
        let keep: Vec<String> = if v.verdict == Verdict::Fail {
            g.iter().filter(|id| !v.offending_cases.contains(id)).cloned().collect()
        } else {
            g.clone()
        };
        if keep.len() >= 2 {
            pruned.push(keep);
        }
    }
    (raw, pruned)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subset {
    pub name: String,
    pub case_ids: Vec<String>,
}

/// Deterministic subsets: whole planted series + decoy members + singletons, by size.
pub(crate) fn make_subsets(ds: &Dataset, truth: &GroundTruth) -> Vec<Subset> {
    let in_series: BTreeSet<&String> = truth.series.iter().flatten().collect();
    let singletons: Vec<&String> =
        ds.cases.iter().map(|c| &c.id).filter(|id| !in_series.contains(id)).collect();
    let mut series = truth.series.clone();
    series.sort_by_key(|s| (s.len(), s[0].clone()));
    let mut out = vec![];
    for &n in &SUBSET_SIZES {
        let mut ids: Vec<String> = vec![];
        // ~45% of the subset from planted series (whole series only), rest singletons + decoys.
        for s in series.iter().filter(|s| s.len() <= 7) {
            if ids.len() + s.len() > n * 45 / 100 {
                continue;
            }
            ids.extend(s.iter().cloned());
        }
        for d in &truth.decoys {
            if ids.len() + d.case_ids.len() > n * 60 / 100 {
                break;
            }
            for id in &d.case_ids {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
        }
        let mut k = 0usize;
        while ids.len() < n && k < singletons.len() {
            // stride through singletons for geographic/temporal spread
            let id = singletons[(k * 37) % singletons.len()].clone();
            if !ids.contains(&id) {
                ids.push(id);
            }
            k += 1;
        }
        ids.sort();
        out.push(Subset { name: format!("n{n}"), case_ids: ids });
    }
    out
}

fn sub_dataset(ds: &Dataset, ids: &[String]) -> Dataset {
    let set: BTreeSet<&String> = ids.iter().collect();
    Dataset { cases: ds.cases.iter().filter(|c| set.contains(&c.id)).cloned().collect(), pawn: vec![] }
}

fn sub_truth(truth: &GroundTruth, ids: &[String]) -> Vec<Vec<String>> {
    let set: BTreeSet<&String> = ids.iter().collect();
    truth
        .series
        .iter()
        .map(|s| s.iter().filter(|id| set.contains(id)).cloned().collect::<Vec<_>>())
        .filter(|s| s.len() >= 2)
        .collect()
}

fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let root = crate::root();
    let store = Store::load(&root)?;
    let truth = store.truth.clone().ok_or_else(|| anyhow::anyhow!("data/seed/ground_truth.json missing"))?;
    let ds = &store.ds;

    if crate::has(args, "--make-subsets") {
        let subsets = make_subsets(ds, &truth);
        let p = root.join("data/seed/eval_subsets.json");
        std::fs::write(&p, serde_json::to_string_pretty(&subsets)?)?;
        for s in &subsets {
            eprintln!("{}: {} cases", s.name, s.case_ids.len());
        }
        eprintln!("wrote {}", p.display());
        return Ok(());
    }

    let mut md = String::from("# Casefile evaluation (synthetic ground truth)\n\n");
    md.push_str(&format!(
        "Dataset: {} cases, {} pawn records, {} planted series, {} decoys. Engine: {}.\n\n",
        ds.cases.len(),
        ds.pawn.len() - truth.demo_pawn.len().min(ds.pawn.len()),
        truth.series.len(),
        truth.decoys.len(),
        casefile_core::ENGINE_VERSION
    ));

    // 1. Series recovery, full dataset.
    let t0 = std::time::Instant::now();
    let (raw, pruned) = casefile_groups(ds);
    let secs = t0.elapsed().as_secs_f64();
    let raw_m = prf(&raw, &truth.series);
    let pr_m = prf(&pruned, &truth.series);
    md.push_str("## 1. Series recovery (full dataset)\n\n| stage | precision | recall | F1 | series recovered |\n|---|---|---|---|---|\n");
    md.push_str(&format!(
        "| engine clusters (threshold {THRESHOLD}) | {} | {} | {} | {}/{} |\n",
        pct(raw_m.precision), pct(raw_m.recall), pct(raw_m.f1), raw_m.series_recovered, raw_m.series_total
    ));
    md.push_str(&format!(
        "| + verify_series pruning | {} | {} | {} | {}/{} |\n\nRuntime: {:.2} s for {} cases.\n\n",
        pct(pr_m.precision), pct(pr_m.recall), pct(pr_m.f1), pr_m.series_recovered, pr_m.series_total, secs, ds.cases.len()
    ));
    // Threshold sweep: the operating point is a precision/recall choice, so show the curve.
    md.push_str("Operating points (engine clusters + verify pruning):\n\n| threshold | precision | recall | F1 | series recovered |\n|---|---|---|---|---|\n");
    for th in [0.5, 0.6, 0.7, 0.8, 0.9, 0.95] {
        let (_, p) = casefile_groups_at(ds, th);
        let m = prf(&p, &truth.series);
        md.push_str(&format!(
            "| {th} | {} | {} | {} | {}/{} |\n",
            pct(m.precision), pct(m.recall), pct(m.f1), m.series_recovered, m.series_total
        ));
    }
    md.push('\n');

    // 2. Decoys.
    let mut decoy_rows = vec![];
    let mut rejected = 0usize;
    for d in &truth.decoys {
        let v = verify::verify_series(ds, &d.case_ids);
        let failed_names: Vec<String> = v
            .checks
            .iter()
            .filter(|c| c.verdict == Verdict::Fail)
            .map(|c| c.name.clone())
            .collect();
        if v.verdict == Verdict::Fail {
            rejected += 1;
        }
        decoy_rows.push(serde_json::json!({
            "case_ids": d.case_ids, "violates": d.violates,
            "verdict": v.verdict, "failed_checks": failed_names,
        }));
    }
    md.push_str(&format!(
        "## 2. Decoy hypotheses\n\n{rejected}/{} decoys rejected by `verify_series` (each violates a hard constraint).\n\n",
        truth.decoys.len()
    ));

    // 3. Property matching.
    // Demo-queue pawn records are planted to exercise the UI; they are not measured data.
    let demo_pawn: BTreeSet<&String> = truth.demo_pawn.iter().collect();
    let demo_cases: BTreeSet<&String> = truth.demo_cases.iter().collect();
    let hits: Vec<_> = property::all_hits(ds)
        .into_iter()
        .filter(|h| !demo_pawn.contains(&h.pawn_id) && !demo_cases.contains(&h.case_id))
        .collect();
    let truth_pairs: BTreeSet<(String, String, String)> = truth
        .pawn_truth
        .iter()
        .map(|(pawn, (case, item))| (pawn.clone(), case.clone(), item.clone()))
        .collect();
    let exact: Vec<_> = hits.iter().filter(|h| h.status == MatchStatus::ExactSerialLead).collect();
    let exact_tp = exact
        .iter()
        .filter(|h| truth_pairs.contains(&(h.pawn_id.clone(), h.case_id.clone(), h.item_id.clone())))
        .count();
    let rejected_conflicts = hits.iter().filter(|h| h.status == MatchStatus::RejectedConflictingSerial).count();
    let marking = hits.iter().filter(|h| h.status == MatchStatus::ReviewCandidateMarking).count();
    let found_truth = truth_pairs
        .iter()
        .filter(|(p, c, i)| {
            hits.iter().any(|h| &h.pawn_id == p && &h.case_id == c && &h.item_id == i
                && matches!(h.status, MatchStatus::ExactSerialLead | MatchStatus::ReviewCandidateMarking))
        })
        .count();
    md.push_str(&format!(
        "## 3. Stolen-property matching\n\nExact-serial leads: {} ({} correct, precision {}). Resurfaced items found as lead or review candidate: {}/{} (recall {}). Conflicting-serial look-alikes rejected: {}. Marking candidates for human review: {}.\n\n",
        exact.len(), exact_tp,
        pct(if exact.is_empty() { 0.0 } else { exact_tp as f64 / exact.len() as f64 }),
        found_truth, truth_pairs.len(),
        pct(if truth_pairs.is_empty() { 0.0 } else { found_truth as f64 / truth_pairs.len() as f64 }),
        rejected_conflicts, marking
    ));
    if !demo_pawn.is_empty() {
        md.push_str(&format!(
            "{} further pawn records on {} queue cases were hand-planted for the demo (a curated mix of lead types, not a measured rate). Those records and cases are excluded from every number above.\n\n",
            demo_pawn.len(),
            demo_cases.len()
        ));
    }

    // 4. Extraction accuracy.
    let runs = Store::runs_in(&root, "extractor");
    let fields = ["premise", "entry_point", "entry_method", "time_band", "weekday"];
    let mut correct: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut prop_j = vec![];
    for r in &runs {
        let (Some(t), Some(got)) = (truth.mo_truth.get(&r.input_id), r.output.get("mo")) else { continue };
        let tv = serde_json::to_value(t)?;
        for f in fields {
            let e = correct.entry(f).or_default();
            e.1 += 1;
            if tv.get(f) == got.get(f) {
                e.0 += 1;
            }
        }
        let set = |v: Option<&serde_json::Value>| -> BTreeSet<String> {
            v.and_then(|x| x.as_array()).map(|a| a.iter().map(|s| s.to_string()).collect()).unwrap_or_default()
        };
        let (a, b) = (set(tv.get("property_types")), set(got.get("property_types")));
        let u = a.union(&b).count();
        prop_j.push(if u == 0 { 1.0 } else { a.intersection(&b).count() as f64 / u as f64 });
    }
    md.push_str(&format!("## 4. Extraction (Extractor agent, recorded runs: {})\n\n", runs.len()));
    if !runs.is_empty() {
        md.push_str("| field | accuracy |\n|---|---|\n");
        for (f, (ok, n)) in &correct {
            md.push_str(&format!("| {f} | {} ({ok}/{n}) |\n", pct(*ok as f64 / (*n).max(1) as f64)));
        }
        let mean = prop_j.iter().sum::<f64>() / prop_j.len().max(1) as f64;
        md.push_str(&format!("| property_types (mean Jaccard) | {} |\n\n", pct(mean)));
    }

    // 5. The wall: LLM-only vs Casefile on subsets.
    let subsets: Vec<Subset> = std::fs::read(root.join("data/seed/eval_subsets.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut wall = vec![];
    md.push_str("## 5. Single LLM vs Casefile (same cases)\n\n| cases | single LLM (Opus, all reports in context) F1 | Casefile (engine + verify) F1 | decoy pairs accepted by LLM |\n|---|---|---|---|\n");
    for s in &subsets {
        let sds = sub_dataset(ds, &s.case_ids);
        let st = sub_truth(&truth, &s.case_ids);
        let (_, ours) = casefile_groups(&sds);
        let ours_m = prf(&ours, &st);
        let llm = store.run("series_llm_only", &s.name);
        let llm_groups: Option<Vec<Vec<String>>> = llm
            .as_ref()
            .and_then(|r| r.output.get("series").cloned())
            .and_then(|v| serde_json::from_value::<Vec<Vec<String>>>(v).ok());
        let llm_m = llm_groups.as_ref().map(|g| prf(g, &st));
        let decoy_pairs: BTreeSet<Pair> = pairs_of(
            &truth.decoys.iter().map(|d| d.case_ids.clone()).collect::<Vec<_>>(),
        );
        let truth_pairs = pairs_of(&truth.series);
        let llm_bad = llm_groups
            .as_ref()
            .map(|g| pairs_of(g).iter().filter(|p| decoy_pairs.contains(*p) && !truth_pairs.contains(*p)).count());
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            s.case_ids.len(),
            llm_m.as_ref().map(|m| pct(m.f1)).unwrap_or_else(|| "not recorded".into()),
            pct(ours_m.f1),
            llm_bad.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
        ));
        wall.push(serde_json::json!({
            "subset": s.name, "cases": s.case_ids.len(),
            "llm_only": llm_m, "casefile": ours_m, "llm_decoy_pairs_accepted": llm_bad,
        }));
    }
    md.push_str("\nAll data synthetic; planted ground truth. These numbers show the method works on data built to resemble the problem, not production accuracy.\n");

    let out_dir = root.join("evals");
    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(out_dir.join("report.md"), &md)?;
    std::fs::write(
        out_dir.join("results.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "series_raw": raw_m, "series_pruned": pr_m, "decoys": decoy_rows,
            "decoys_rejected": rejected, "property": {
                "exact_leads": exact.len(), "exact_correct": exact_tp,
                "truth_resurfaced": truth_pairs.len(), "found": found_truth,
                "conflicts_rejected": rejected_conflicts, "marking_candidates": marking },
            "extraction_runs": runs.len(), "wall": wall,
        }))?,
    )?;
    println!("{md}");
    Ok(())
}
