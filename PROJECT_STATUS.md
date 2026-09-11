# Project status — where the truth lives

This file explains **how** the current state of OVERFRAME is published; it
never states a build result itself (a file on `main` cannot know the result of
the CI run that builds it).

## Machine-readable state

| What | Where | Written by |
|---|---|---|
| Latest CI attempt + latest fully-passing build | `https://raw.githubusercontent.com/Gisleno-bit/OverFrame/visual-evidence/PROJECT_STATUS.json` | CI (`.github/workflows/visual-evidence.yml`) |
| Evidence of one run (images, runtime exports, manifest) | `visual-evidence` branch: `builds/<source_sha>/attempt-<run_id>-<run_attempt>/` | CI |
| Art specifications (JSON is the numeric source) | `docs/art/procedural/` on `main` | ChatGPT (spec) / Claude (implementation notes) |
| Capture cameras, poses and fixtures | `docs/art/capture-suite.json` on `main` | Claude |
| Review notes against one run | `docs/art/reviews/<source_sha>.md` on `main` | ChatGPT |
| The exchange contract itself | `docs/art/EXCHANGE.md` | agreed |
| Who does what, and the animation/move references | `docs/art/ROLES.md` | agreed |

The repository must be **public** for the raw URLs above to answer; a
private repository returns 404 to anyone without a token.

Read order for a reviewer: `PROJECT_STATUS.json` → compare `source_sha` with
the current `main` HEAD → open `manifest.json` of that attempt → read images
and specs **by full SHA**, never from a moving `main` URL.

## `PROJECT_STATUS.json` schema (`schema_version: 1`)

```json
{
  "schema_version": 1,
  "status": "pass | fail | pending",
  "source_sha": "<40 hex>",
  "version": "<Cargo.toml version>",
  "latest_attempt": { "run_id": "…", "run_attempt": "…", "status": "…", "path": "builds/<sha>/attempt-<id>-<n>/", "utc": "…" },
  "latest_pass":    { "source_sha": "…", "run_id": "…", "run_attempt": "…", "path": "…", "utc": "…" } | null,
  "evidence_commit": "<40 hex of the visual-evidence commit that carries latest_attempt>",
  "checks": { "fmt": "pass|fail|not_run", "clippy": "…", "tests": "…", "tests_passed": <int|null> },
  "capabilities": { "<name>": "verified | implemented_unverified | specified_not_implemented | not_verified" }
}
```

`latest_pass` may lag behind `latest_attempt` on purpose: a failed run never
borrows images from an earlier one. `not_run` is never green.

## Manifest of one attempt

`builds/<sha>/attempt-<run_id>-<run_attempt>/manifest.json` lists: full source
SHA, version, SHA-256 of every spec file, capture-suite id, run id/attempt,
UTC date, platform, the renderer actually used (`game3d` = the production
macroquad renderer under Mesa/llvmpipe, `sim2d` = the headless diagnostic
rasteriser), checks with command/result/test totals, and every file with its
SHA-256, dimensions, tick and camera. Capabilities are `verified` only when
the manifest names the test or image that proves them.

## How to run the same captures locally

```text
cargo build --release
overframe --capture out/                 # Windows / desktop: real GPU
xvfb-run -a overframe --capture out/     # Linux without a display (Mesa)
# The capture command above also writes out/runtime/ (CSV and JSON).
```

Everything under `out/` is what CI publishes, byte-for-byte in layout.
The separate `overframe-replay` tool renders a scripted 2D demo; it does not
accept `--export-runtime`. Use the production capture command for these exports.
