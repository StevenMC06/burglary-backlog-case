//! `casefile bench [--out evals/bench.md] [--scales 1,4,10] [--reps 3]`: wall-clock of the
//! brute-force reference vs the optimised engine paths on the seed dataset, tiled into
//! multi-year archives (`k` copies, each shifted a year with fresh ids, so no tile links to
//! another and the planted structure repeats). Every timed pair is also checked for identical
//! output. Timings are machine-dependent; the equality checks and the ratios are what matter.

use std::fmt::Write as _;
use std::time::Instant;

use anyhow::bail;
use casefile_core::{Dataset, linkage, property};

use crate::mcp::DEFAULT_CLUSTER_THRESHOLD;
use crate::store::Store;

const YEAR_SECS: i64 = 365 * 86_400;

fn tile(ds: &Dataset, k: usize) -> Dataset {
    let mut out = Dataset { cases: vec![], pawn: vec![] };
    for t in 0..k {
        let shift = t as i64 * YEAR_SECS;
        for c in &ds.cases {
            let mut c = c.clone();
            c.id = format!("{}~{t}", c.id);
            c.window_start += shift;
            c.window_end += shift;
            c.reported_at += shift;
            for it in &mut c.items {
                it.id = format!("{}~{t}", it.id);
            }
            out.cases.push(c);
        }
        for p in &ds.pawn {
            let mut p = p.clone();
            p.id = format!("{}~{t}", p.id);
            p.received_at += shift;
            out.pawn.push(p);
        }
    }
    out
}

/// Median wall-clock milliseconds of `reps` runs, plus the last result.
fn time<T>(reps: usize, mut f: impl FnMut() -> T) -> (f64, T) {
    let mut ms = vec![];
    let mut last = None;
    for _ in 0..reps.max(1) {
        let t = Instant::now();
        last = Some(f());
        ms.push(t.elapsed().as_secs_f64() * 1e3);
    }
    ms.sort_by(f64::total_cmp);
    (ms[ms.len() / 2], last.unwrap_or_else(|| unreachable!("reps >= 1")))
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let root = crate::root();
    let out_path = crate::flag(args, "--out").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("evals/bench.md"));
    let reps: usize = crate::flag(args, "--reps").and_then(|r| r.parse().ok()).unwrap_or(3);
    let scales: Vec<usize> = crate::flag(args, "--scales")
        .unwrap_or_else(|| "1,4,10".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .filter(|&k| k >= 1)
        .collect();
    let base = Store::load(&root)?.ds;
    let t = DEFAULT_CLUSTER_THRESHOLD;

    let mut md = String::new();
    writeln!(md, "# Engine benchmark (`casefile bench`)\n")?;
    writeln!(
        md,
        "Seed dataset tiled into k one-year copies. Median of {reps} runs, release build, this machine; \
         \"same\" = optimised output equals the brute-force reference exactly. Threshold {t}.\n"
    )?;
    writeln!(md, "| k | cases | pawn | step | reference ms | optimised ms | speed-up | same |")?;
    writeln!(md, "|---|---|---|---|---|---|---|---|")?;
    let mut all_same = true;
    for &k in &scales {
        let ds = tile(&base, k);
        let (n, p) = (ds.cases.len(), ds.pawn.len());
        let probes: Vec<&str> = ds.cases.iter().step_by((n / 50).max(1)).map(|c| c.id.as_str()).collect();
        let rows: Vec<(&str, (f64, bool), f64)> = {
            let (r_ms, r) = time(reps, || linkage::propose_clusters_reference(&ds, t));
            let (f_ms, f) = time(reps, || linkage::propose_clusters(&ds, t));
            let clusters = ((r_ms, r == f), f_ms);
            let (r_ms, r) = time(reps, || property::all_hits_reference(&ds));
            let (f_ms, f) = time(reps, || property::all_hits(&ds));
            let hits = ((r_ms, r == f), f_ms);
            let (r_ms, r) = time(reps, || probes.iter().map(|id| linkage::similar_cases_reference(&ds, id, 5)).collect::<Vec<_>>());
            let (f_ms, f) = time(reps, || probes.iter().map(|id| linkage::similar_cases(&ds, id, 5)).collect::<Vec<_>>());
            let similar = ((r_ms, r == f), f_ms);
            vec![
                ("propose_clusters", clusters.0, clusters.1),
                ("all_hits (pawn matching)", hits.0, hits.1),
                ("similar_cases k=5 (50 probes)", similar.0, similar.1),
            ]
        };
        for (name, (r_ms, same), f_ms) in rows {
            all_same &= same;
            let ratio = if f_ms > 0.0 { r_ms / f_ms } else { f64::INFINITY };
            writeln!(md, "| {k} | {n} | {p} | {name} | {r_ms:.1} | {f_ms:.1} | {ratio:.1}x | {} |", if same { "yes" } else { "NO" })?;
        }
        eprintln!("bench: k={k} done");
    }
    writeln!(
        md,
        "\nWhy it is faster, with no change to results:\n\
         - propose_clusters: time-sorted sweep (only pairs inside the 180-day window are visited), an exact \
         score upper bound (full MO agreement) rejects pairs before MO work, MO features precomputed as a \
         bitmask and interned signature ids (no per-pair allocation), work split across threads.\n\
         - all_hits: pawn records normalised once and indexed by category, serial and marking; an item is \
         compared only with records that can match it under the rules.\n\
         - similar_cases: bounded min-heap top-k (O(n log k)) with the same upper bound used as a \
         branch-and-bound cut against the current k-th best.\n"
    )?;
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&out_path, &md)?;
    print!("{md}");
    if !all_same {
        bail!("optimised output differs from the reference; see {}", out_path.display());
    }
    Ok(())
}
