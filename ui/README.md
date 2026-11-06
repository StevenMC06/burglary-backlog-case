# UI preview branch

The console combines the glass case-review layout and COBE globe transition with a compact desktop inbox, score meters, four-cell case summary, and source-report/property overview. The illustrations are not case evidence. Below 1180px, Cases opens the inbox as a disclosure; selecting a case closes it. Desktop selection keeps the inbox visible.

## Preview

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
