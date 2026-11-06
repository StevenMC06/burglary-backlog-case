//! casefile: one binary, several subcommands. No CLI-parsing dependency on purpose.
//!
//!   casefile datagen [--out data/seed] [--seed 7]      generate synthetic dataset + ground truth
//!   casefile serve   [--port 8080]                      web console (sergeant/detective), replay mode
//!   casefile mcp                                        MCP server over stdio (Claude Code / Desktop)
//!   casefile record  <agent> <input-id> [--model m]     run an agent once via `claude -p`, save run
//!   casefile eval    [--out evals/report.md]            ground-truth evaluation
//!   casefile cost    [--out docs/deck/cost.json]        cost & clearance scenarios (official baseline)
//!   casefile bench   [--out evals/bench.md]             engine speed: brute-force reference vs optimised
//!   casefile serve   --public                           bind 0.0.0.0 (or $PORT), sandboxed capped audit log

mod audit;
mod bench;
mod cost_cli;
mod datagen;
mod eval;
mod mcp;
mod record;
mod server;
mod store;

use std::path::PathBuf;

/// Read `--name value` from args.
pub fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

pub fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Project root: $CASEFILE_ROOT or current directory.
pub fn root() -> PathBuf {
    std::env::var_os("CASEFILE_ROOT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = match args.split_first() {
        Some((c, r)) => (c.as_str(), r.to_vec()),
        None => ("help", vec![]),
    };
    match cmd {
        "datagen" => datagen::run(&rest),
        "serve" => tokio::runtime::Runtime::new()?.block_on(server::run(&rest)),
        "mcp" => tokio::runtime::Runtime::new()?.block_on(mcp::run(&rest)),
        "record" => record::run(&rest),
        "eval" => eval::run(&rest),
        "cost" => cost_cli::run(&rest),
        "bench" => bench::run(&rest),
        _ => {
            eprintln!("usage: casefile <datagen|serve|mcp|record|eval|cost|bench> [options]  (see src/main.rs)");
            Ok(())
        }
    }
}
