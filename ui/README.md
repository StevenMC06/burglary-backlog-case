# UI preview branch

This branch contains the glass case-review layout, compact case switcher, evidence tabs, readable activity, and COBE globe transition. The illustrations are not case evidence.

## Preview

```sh
git fetch origin
git switch --track origin/steven/calm-casefile-ui
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
