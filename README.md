# Casefile: Team 6, ESEC case comp (Cascade City burglary backlog)

AI that helps property-crimes detectives connect burglaries: it links cases, matches stolen items against pawn records, and hands over a verified, cited lead packet. **AI suggests, a rules engine verifies, people decide.**

All data is synthetic (fictional Cascade City) on public Seattle geography. There is no facial recognition and no names.

```sh
cargo run -- cost      # cost & clearance scenarios on the official case baseline
cargo run -- datagen   # regenerate the synthetic dataset (deterministic)
cargo run -- serve     # web console at http://localhost:8080 (offline replay of recorded AI runs)
cargo run -- mcp       # MCP server (stdio) for Claude Code / Claude Desktop
cargo test             # engine + app tests
```

Docs:
- [docs/CASE_OFFICIAL.md](docs/CASE_OFFICIAL.md): the case, rubric and judges
- [docs/BUILD_PLAN.md](docs/BUILD_PLAN.md): architecture and contracts
- [docs/PROPOSAL.md](docs/PROPOSAL.md): original research proposal (numbers superseded by the official baseline)
- [docs/GUY_NOTES.md](docs/GUY_NOTES.md): technical judge notes
