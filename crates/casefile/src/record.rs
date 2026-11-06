//! `casefile record <agent> <input-id> [--model m] [--force]` and
//! `casefile record <agent> --ids file [--model m] [--force]`.
//!
//! Runs one agent through `claude -p` (stream-json), parses the stream into a `RecordedRun` and
//! saves it at data/recordings/<agent>/<input-id>.json. Runs are sequential; each is retried once.
//! Tests never invoke claude: they cover the argv/prompt builder and the stream parser.
//!
//! agents/<agent>/agent.json:
//!   { "model": "sonnet", "allowed_tools": ["get_case"], "input": "<kind>" }
//! `input` kinds (what gets inlined into the user prompt):
//!   case_narrative          narrative + items of case <input-id>
//!   case_id                 just the id (the agent fetches data with tools)
//!   subset_narratives       all narratives of subset <input-id> (data/seed/eval_subsets.json)
//!   recording:<agent>       the saved output of another agent's run for <input-id>

use crate::{flag, has, root, store::RecordedRun, store::Store};
use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize)]
pub struct AgentDef {
    pub model: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    pub input: String,
}

/// Parsed result of one `claude -p` stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub events: Vec<Value>,
    pub output: Value,
    pub duration_ms: u64,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let model_override = flag(args, "--model");
    let ids_file = flag(args, "--ids");
    let force = has(args, "--force");
    if has(args, "--concurrency") {
        eprintln!("record: --concurrency ignored; runs are sequential");
    }
    let positional = positionals(args);
    let agent = positional.first().cloned().ok_or_else(|| {
        anyhow!("usage: casefile record <agent> <input-id> [--model m] [--force] | --ids file")
    })?;
    let ids: Vec<String> = match (&ids_file, positional.get(1)) {
        (Some(f), _) => parse_ids(&std::fs::read_to_string(f).with_context(|| format!("reading {f}"))?),
        (None, Some(id)) => vec![id.clone()],
        (None, None) => bail!("record: need <input-id> or --ids file"),
    };
    let root = root();
    let mut failed = vec![];
    for id in &ids {
        let out = recording_path(&root, &agent, id);
        if out.exists() && !force {
            eprintln!("skip {agent}/{id}: recording exists (use --force)");
            continue;
        }
        match record_with_retry(&root, &agent, id, model_override.as_deref()) {
            Ok(run) => {
                save(&out, &run)?;
                eprintln!(
                    "recorded {agent}/{id}: {} in / {} out tokens, ${:.4}, {} ms",
                    run.input_tokens, run.output_tokens, run.cost_usd, run.duration_ms
                );
            }
            Err(e) => {
                eprintln!("FAILED {agent}/{id}: {e:#}");
                failed.push(id.clone());
            }
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        bail!("{} run(s) failed: {}", failed.len(), failed.join(", "))
    }
}

/// Args that are neither flags nor flag values.
fn positionals(args: &[String]) -> Vec<String> {
    let mut out = vec![];
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--model" || a == "--ids" || a == "--concurrency" {
            i += 2;
        } else if a.starts_with("--") {
            i += 1;
        } else {
            out.push(a.clone());
            i += 1;
        }
    }
    out
}

pub fn parse_ids(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn recording_path(root: &Path, agent: &str, id: &str) -> PathBuf {
    root.join("data/recordings").join(agent).join(format!("{id}.json"))
}

fn save(path: &Path, run: &RecordedRun) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(run)?)?;
    Ok(())
}

fn safe_name(s: &str) -> anyhow::Result<()> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        bail!("invalid name {s:?}: use letters, digits, '_' or '-'");
    }
    Ok(())
}

fn record_with_retry(root: &Path, agent: &str, id: &str, model: Option<&str>) -> anyhow::Result<RecordedRun> {
    match record_once(root, agent, id, model) {
        Ok(r) => Ok(r),
        Err(first) => {
            eprintln!("retrying {agent}/{id} after: {first:#}");
            record_once(root, agent, id, model)
        }
    }
}

fn record_once(root: &Path, agent: &str, id: &str, model: Option<&str>) -> anyhow::Result<RecordedRun> {
    safe_name(agent)?;
    safe_name(id)?;
    let dir = root.join("agents").join(agent);
    let def: AgentDef = serde_json::from_slice(
        &std::fs::read(dir.join("agent.json")).with_context(|| format!("reading agents/{agent}/agent.json"))?,
    )?;
    let schema: Value = serde_json::from_slice(&std::fs::read(dir.join("schema.json")).context("reading schema.json")?)?;
    let model = model.unwrap_or(&def.model).to_string();
    let prompt = build_prompt(root, &def, id)?;

    let exe = std::env::current_exe().context("locating own executable")?;
    let abs_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mcp_path = if def.allowed_tools.is_empty() {
        None
    } else {
        let p = std::env::temp_dir().join(format!("casefile-mcp-{}-{agent}-{id}.json", std::process::id()));
        std::fs::write(&p, serde_json::to_vec(&mcp_config(&exe, &abs_root))?)?;
        Some(p)
    };
    let argv = build_argv(&def, &model, &dir.join("system.md"), &schema, mcp_path.as_deref());

    let bin = std::env::var_os("CLAUDE_BIN").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("claude"));
    let started = std::time::Instant::now();
    let result = exec(&bin, &argv, &prompt, root);
    if let Some(p) = &mcp_path {
        let _ = std::fs::remove_file(p);
    }
    let stdout = result?;
    let wall_ms = started.elapsed().as_millis() as u64;
    let mut parsed = parse_stream(&stdout)?;
    if parsed.duration_ms == 0 {
        parsed.duration_ms = wall_ms;
    }
    Ok(RecordedRun {
        agent: agent.to_string(),
        input_id: id.to_string(),
        model,
        recorded_at: now_stamp(),
        events: parsed.events,
        output: parsed.output,
        duration_ms: parsed.duration_ms,
        cost_usd: parsed.cost_usd,
        input_tokens: parsed.input_tokens,
        output_tokens: parsed.output_tokens,
    })
}

fn exec(bin: &Path, argv: &[String], prompt: &str, cwd: &Path) -> anyhow::Result<String> {
    let mut child = Command::new(bin)
        .args(argv)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {} (set CLAUDE_BIN)", bin.display()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // A result line may still explain the failure; parse_stream reports it if present.
        match parse_stream(&stdout) {
            Ok(_) => {}
            Err(e) => bail!("claude exited with {}: {} ({e})", out.status, err.trim()),
        }
        bail!("claude exited with {}: {}", out.status, err.trim());
    }
    Ok(stdout)
}

fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

pub fn mcp_config(exe: &Path, root: &Path) -> Value {
    json!({"mcpServers": {"casefile": {
        "command": exe.to_string_lossy(),
        "args": ["mcp"],
        "env": {"CASEFILE_ROOT": root.to_string_lossy()}
    }}})
}

/// Argument vector for `claude`. The user prompt is sent on stdin.
pub fn build_argv(def: &AgentDef, model: &str, system_file: &Path, schema: &Value, mcp: Option<&Path>) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-p".into(),
        "--model".into(),
        model.into(),
        "--system-prompt-file".into(),
        system_file.to_string_lossy().into_owned(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--json-schema".into(),
        schema.to_string(),
        "--no-session-persistence".into(),
        // Built-in tools off; only the listed MCP tools are reachable.
        "--tools".into(),
        String::new(),
    ];
    if let Some(p) = mcp {
        a.push("--mcp-config".into());
        a.push(p.to_string_lossy().into_owned());
        a.push("--strict-mcp-config".into());
        let tools: Vec<String> = def.allowed_tools.iter().map(|t| format!("mcp__casefile__{t}")).collect();
        a.push("--allowedTools".into());
        a.push(tools.join(","));
    }
    a
}

/// User prompt with the data the agent definition says to inline.
pub fn build_prompt(root: &Path, def: &AgentDef, id: &str) -> anyhow::Result<String> {
    match def.input.as_str() {
        "case_id" => Ok(format!("Input id: {id}\nUse your tools to fetch what you need.")),
        "case_narrative" => {
            let store = Store::load(root)?;
            let c = store.ds.cases.iter().find(|c| c.id == id).ok_or_else(|| anyhow!("unknown case {id}"))?;
            Ok(case_block(c))
        }
        "subset_narratives" => {
            let subsets: Vec<Value> = serde_json::from_slice(
                &std::fs::read(root.join("data/seed/eval_subsets.json")).context("reading data/seed/eval_subsets.json")?,
            )?;
            let sub = subsets
                .iter()
                .find(|s| s.get("name").and_then(Value::as_str) == Some(id))
                .ok_or_else(|| anyhow!("unknown subset {id}"))?;
            let ids: Vec<String> = serde_json::from_value(sub.get("case_ids").cloned().unwrap_or_default())?;
            let store = Store::load(root)?;
            let mut out = format!("Subset {id}: {} cases.\n\n", ids.len());
            for cid in &ids {
                if let Some(c) = store.ds.cases.iter().find(|c| &c.id == cid) {
                    out.push_str(&case_block(c));
                    out.push_str("\n\n");
                }
            }
            Ok(out)
        }
        k if k.starts_with("recording:") => {
            let src = &k["recording:".len()..];
            safe_name(src)?;
            let run: RecordedRun = serde_json::from_slice(
                &std::fs::read(recording_path(root, src, id)).with_context(|| format!("no {src} recording for {id}"))?,
            )?;
            Ok(format!("Input id: {id}\nOutput of the {src} agent to review:\n{}", serde_json::to_string_pretty(&run.output)?))
        }
        other => bail!("unknown input kind {other:?}"),
    }
}

fn case_block(c: &casefile_core::Case) -> String {
    let items: Vec<String> = c
        .items
        .iter()
        .map(|i| format!("- {} {} {}: {}", i.id, i.brand.as_deref().unwrap_or(""), i.serial.as_deref().unwrap_or("no serial"), i.description))
        .collect();
    format!("CASE {} ({})\nNarrative: {}\nItems:\n{}", c.id, c.neighborhood, c.narrative, items.join("\n"))
}

/// Parse `--output-format stream-json` lines. Fails if there is no successful result line.
pub fn parse_stream(text: &str) -> anyhow::Result<Parsed> {
    let events: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .filter(Value::is_object)
        .collect();
    let result = events
        .iter()
        .rev()
        .find(|e| e.get("type").and_then(Value::as_str) == Some("result"))
        .ok_or_else(|| anyhow!("no result event in stream"))?;
    if result.get("is_error").and_then(Value::as_bool).unwrap_or(false)
        || result.get("subtype").and_then(Value::as_str).is_some_and(|s| s != "success")
    {
        bail!("agent run failed: {}", result.get("result").and_then(Value::as_str).or_else(|| result.get("subtype").and_then(Value::as_str)).unwrap_or("unknown error"));
    }
    let output = match result.get("structured_output") {
        Some(v) if !v.is_null() => v.clone(),
        _ => result
            .get("result")
            .and_then(Value::as_str)
            .and_then(|s| serde_json::from_str::<Value>(strip_fence(s)).ok())
            .ok_or_else(|| anyhow!("result has no structured output"))?,
    };
    let usage = result.get("usage");
    let u = |k: &str| usage.and_then(|u| u.get(k)).and_then(Value::as_u64).unwrap_or(0);
    Ok(Parsed {
        output,
        duration_ms: result.get("duration_ms").and_then(Value::as_u64).unwrap_or(0),
        cost_usd: result.get("total_cost_usd").and_then(Value::as_f64).unwrap_or(0.0),
        input_tokens: u("input_tokens") + u("cache_creation_input_tokens") + u("cache_read_input_tokens"),
        output_tokens: u("output_tokens"),
        events,
    })
}

fn strip_fence(s: &str) -> &str {
    let t = s.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    t.strip_suffix("```").unwrap_or(t).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: &str = concat!(
        "{\"type\":\"system\",\"subtype\":\"init\"}\n",
        "not json\n",
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"mcp__casefile__get_case\"}]}}\n",
        "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"duration_ms\":1234,\"total_cost_usd\":0.0123,",
        "\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":5,\"cache_read_input_tokens\":100,\"output_tokens\":7},",
        "\"result\":\"done\",\"structured_output\":{\"a\":1}}\n"
    );

    fn def(tools: &[&str]) -> AgentDef {
        AgentDef { model: "sonnet".into(), allowed_tools: tools.iter().map(|s| s.to_string()).collect(), input: "case_id".into() }
    }

    #[test]
    fn parses_stream() {
        let p = parse_stream(STREAM).unwrap();
        assert_eq!(p.events.len(), 3);
        assert_eq!(p.output, json!({"a": 1}));
        assert_eq!(p.duration_ms, 1234);
        assert_eq!(p.input_tokens, 115);
        assert_eq!(p.output_tokens, 7);
        assert!((p.cost_usd - 0.0123).abs() < 1e-9);
    }

    #[test]
    fn falls_back_to_result_text_json() {
        let s = "{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"```json\\n{\\\"b\\\":2}\\n```\"}\n";
        assert_eq!(parse_stream(s).unwrap().output, json!({"b": 2}));
    }

    #[test]
    fn rejects_error_and_missing_result() {
        let e = "{\"type\":\"result\",\"subtype\":\"error_max_turns\",\"is_error\":true}\n";
        assert!(parse_stream(e).is_err());
        assert!(parse_stream("{\"type\":\"system\"}\n").is_err());
        let no_out = "{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"plain text\"}\n";
        assert!(parse_stream(no_out).is_err());
    }

    #[test]
    fn argv_without_tools_disables_everything() {
        let a = build_argv(&def(&[]), "haiku", Path::new("agents/x/system.md"), &json!({"type": "object"}), None);
        let i = a.iter().position(|x| x == "--tools").unwrap();
        assert_eq!(a[i + 1], "");
        assert!(!a.iter().any(|x| x == "--mcp-config"));
        assert!(a.contains(&"--no-session-persistence".to_string()));
        assert!(a.contains(&"stream-json".to_string()));
        assert!(a.contains(&"--verbose".to_string()));
        let m = a.iter().position(|x| x == "--model").unwrap();
        assert_eq!(a[m + 1], "haiku");
        let s = a.iter().position(|x| x == "--json-schema").unwrap();
        assert_eq!(a[s + 1], "{\"type\":\"object\"}");
    }

    #[test]
    fn argv_with_tools_is_strict_and_prefixed() {
        let a = build_argv(&def(&["get_case", "verify_series"]), "opus", Path::new("s.md"), &json!({}), Some(Path::new("m.json")));
        assert!(a.contains(&"--strict-mcp-config".to_string()));
        let i = a.iter().position(|x| x == "--allowedTools").unwrap();
        assert_eq!(a[i + 1], "mcp__casefile__get_case,mcp__casefile__verify_series");
        let c = a.iter().position(|x| x == "--mcp-config").unwrap();
        assert_eq!(a[c + 1], "m.json");
    }

    #[test]
    fn mcp_config_points_at_exe_with_root() {
        let v = mcp_config(Path::new("casefile"), Path::new("/r"));
        assert_eq!(v["mcpServers"]["casefile"]["args"], json!(["mcp"]));
        assert_eq!(v["mcpServers"]["casefile"]["env"]["CASEFILE_ROOT"], "/r");
    }

    #[test]
    fn prompt_case_id_and_unknown_kind() {
        let p = build_prompt(Path::new("."), &def(&[]), "CC-1").unwrap();
        assert!(p.contains("CC-1"));
        let mut d = def(&[]);
        d.input = "bogus".into();
        assert!(build_prompt(Path::new("."), &d, "CC-1").is_err());
    }

    #[test]
    fn ids_file_and_positionals() {
        assert_eq!(parse_ids("a\n# c\n\n  b  \n"), vec!["a", "b"]);
        let args: Vec<String> = ["triage", "CC-1", "--model", "opus", "--force"].iter().map(|s| s.to_string()).collect();
        assert_eq!(positionals(&args), vec!["triage", "CC-1"]);
    }

    #[test]
    fn rejects_path_like_names() {
        assert!(safe_name("../x").is_err());
        assert!(safe_name("CC-25-004812").is_ok());
    }
}
