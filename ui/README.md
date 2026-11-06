# UI preview branch

The console combines glass case-review surfaces and the COBE globe transition with a desktop inbox, compact summary strip, and report/property overview. Plain-English wording appears directly below the source report; review factors stay visible below the action tabs. All pending actions show their wording and Accept/Reject controls, without expandable explanations or packet/rationale sections. Text and spacing share a responsive scale (14px at 1280px wide, growing on ultrawide screens), and the workspace fills the browser width. The illustrations are not case evidence. Below 1180px, Cases opens the inbox as a disclosure; selecting a case closes it. Desktop selection keeps the inbox visible.

## Preview

The Series tab uses a compact schematic map and directly visible case rows with time bars. Case rows open the record while keeping the series selected. Arrow buttons browse other groups; checkmarks replace verdict badges, and conflicting or uncertain checks show their reasons inline. Failed checks block approval. Cases with no linked series show an empty state and let you explicitly browse the other groups.

```sh
cargo run -- serve --port 8081
```

Open http://localhost:8081/?case=CC-26-001996. Use Cases to switch records. Replay zoom restarts the globe transition. Drag the globe or use arrow keys to rotate it. WebGL is required for the globe; an illustration is shown when unavailable. Reduced-motion preferences skip the automatic zoom.

## Checks

```sh
node ui/checks.cjs
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

The checked-in vendor assets work offline; no frontend build is needed to preview. To regenerate them, run `node tools/build-motion.cjs` (Node/npm and network access required). It pins Motion 14.2.0, COBE 0.6.4, and esbuild 0.25.12. Licenses are in `ui/vendor/LICENSES.txt`.
