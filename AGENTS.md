# AGENTS.md: rules for every agent working in this repo

- **Checks** (all must pass before you commit): `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings` (if clippy is installed).
- **Dependencies:** only those already in the Cargo.toml files. Adding any crate means stop and report.
- **Errors:** `anyhow` in the binary; no `unwrap`/`expect` outside tests; never panic on user or LLM input.
- **Data:** synthetic or public only (`data/public` is Seattle open data). No real names. Offenders and sellers are opaque refs.
- **Windows shell:** write files with the Write/Edit tools. Bash heredocs containing quotes have failed in this environment.
- **Cost discipline:** read only the files your brief names. Don't re-read files you just wrote. Keep test fixtures small. At most 2 screenshot iterations for UI.
- **Commits:** Conventional Commits, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`, using `git -c user.name="Kevin" -c user.email="khanhpham01012003@gmail.com"`.
- Specs: `docs/BUILD_PLAN.md`. Case facts: `docs/CASE_OFFICIAL.md`.
