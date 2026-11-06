//! Data access layer shared by the MCP server, web console and evals.
//! Files (all made-up data):
//!   data/seed/dataset.json        Dataset (cases + pawn records)
//!   data/seed/ground_truth.json   GroundTruth (evals only; never served before "reveal")
//!   data/recordings/<agent>/<input-id>.json   RecordedRun (see record.rs)
//! Extracted MOs from recordings/extractor override case.mo; otherwise case.mo stays as generated
//! (the generator writes RMS-structured MOs for non-demo cases).

use anyhow::Context;
use casefile_core::{Dataset, GroundTruth, Mo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedRun {
    pub agent: String,
    pub input_id: String,
    pub model: String,
    pub recorded_at: String,
    /// Raw `claude -p --output-format stream-json` events (system/assistant/user/result lines).
    pub events: Vec<serde_json::Value>,
    /// Final structured output (validated against the agent's JSON schema).
    pub output: serde_json::Value,
    pub duration_ms: u64,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub struct Store {
    pub root: PathBuf,
    pub ds: Dataset,
    pub truth: Option<GroundTruth>,
}

impl Store {
    pub fn load(root: &Path) -> anyhow::Result<Self> {
        let seed = root.join("data/seed");
        let ds_path = seed.join("dataset.json");
        let mut ds: Dataset = serde_json::from_slice(
            &std::fs::read(&ds_path).with_context(|| format!("reading {}", ds_path.display()))?,
        )?;
        let truth = std::fs::read(seed.join("ground_truth.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        // Overlay extracted MOs from recorded extractor runs.
        for run in Self::runs_in(root, "extractor") {
            let mo = run.output.get("mo").cloned().unwrap_or_default();
            if let Ok(mo) = serde_json::from_value::<Mo>(mo)
                && let Some(c) = ds.cases.iter_mut().find(|c| c.id == run.input_id)
            {
                c.mo = Some(mo);
            }
        }
        Ok(Self { root: root.to_path_buf(), ds, truth })
    }

    pub fn runs_in(root: &Path, agent: &str) -> Vec<RecordedRun> {
        let dir = root.join("data/recordings").join(agent);
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(dir) {
            let mut paths: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            paths.sort();
            for p in paths {
                if p.extension().is_some_and(|e| e == "json")
                    && let Some(r) = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok())
                {
                    out.push(r);
                }
            }
        }
        out
    }

    pub fn run(&self, agent: &str, input_id: &str) -> Option<RecordedRun> {
        let p = self.root.join("data/recordings").join(agent).join(format!("{input_id}.json"));
        serde_json::from_slice(&std::fs::read(p).ok()?).ok()
    }
}
