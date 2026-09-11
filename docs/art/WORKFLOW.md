# Fighting-game collaboration workflow

This adapts the useful coordination practices of the browser-overlay project to the fighting game. It preserves the product owner's current instructions, [ROLES.md](ROLES.md), the [procedural format](procedural/FORMAT.md), the [animation contract](procedural/anim/FORMAT.md) and [EXCHANGE.md](EXCHANGE.md). It does not import the overlay's code, branching policy or command hooks.

## Start with repository identity

Before changing files, record:

```text
git rev-parse --show-toplevel
git remote get-url origin
git branch --show-current
git rev-parse HEAD
git status --short
git log --oneline --decorate -5
```

For this game, `origin` must identify `Gisleno-bit/OverFrame`, and the root contains the Rust game `Cargo.toml`. A root package describing an Electron browser overlay or a remote ending in `Overframe-1` identifies the other product. Resolve any mismatch before editing. Read [REPOSITORY_MAP.md](REPOSITORY_MAP.md) before working near the nested checkout.

Inspect existing changes before pulling. Use `git pull --ff-only` before edits when the current worktree permits it. If work overlaps or a pull cannot fast-forward, preserve it and resolve ownership; do not reset, clean, stash, switch branches under active work or overwrite files to make the checkpoint match old prose.

Read the latest relevant dated review, [PROJECT_STATUS.md](../../PROJECT_STATUS.md), [ROLES.md](ROLES.md), the affected spec and its contract. Git and actual runtime evidence establish current state; old test totals and the order of log entries do not. Inspect other branches/worktrees when work overlaps.

## Ownership and a concrete handoff

The product owner decides scope, priorities and release approval. ChatGPT authors visual direction in `docs/art/` and `assets/brand/`; Claude authors game implementation, tests, capture/export tooling and CI. ChatGPT may transfer Claude's exact delivered implementation bytes and execute checks under the existing task authorization. That transfer is not permission to invent implementation outside the agreed role.

Each handoff records the following together:

| Field | Required content |
| --- | --- |
| Identity | Canonical repository, actual checkout path, branch and full base SHA |
| Owner and scope | Agent, allowed paths, current changes and overlap with another worker |
| Contract | Exact committed spec/review, stable piece/action IDs and constraints |
| Acceptance | Observable runtime behavior, required cases, images and regression boundaries |
| Delivery | Full commit SHA, or fresh source archive with SHA-256 of the archive and each delivered file |
| Verification | Commands actually run, results, environment and limitations |
| Evidence | Source SHA, evidence commit, run/attempt, filenames and renderer |
| Next action | Unresolved observations, current blocker and the precise resumption step |

Use the existing Claude task for ongoing implementation. Name the canonical game repository and full spec/review commit in the message, not just “Overframe.” Read the response and inspect the actual delivered diff before integration. A claimed timestamp or edited-file activity is not proof of transferred content.

If Claude's VM cannot reliably write the checkout, request a fresh archive containing only its changed implementation files, generated from the current VM work. Require its base/commit identity, complete archive and per-file SHA-256 hashes, and explicit declarations of incomplete checks. Inspect archive paths and the diff against the verified local base, preserve current art changes, and transfer only intended files. Do not reuse an old archive under a new claim. Do not commit partial code as verified merely to obtain a SHA.

## Checks and evidence

Scale checks to the change. Documentation-only work needs link/path checks, a scoped diff and `git diff --check`; existing source CI still reports its own result after a push. Implementation changes need the affected behavior checks and the project's required build gates. Current Windows validation commands are:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
python -m unittest discover -s tools -p test_evidence.py
cargo build --release
```

Read the actual workflows for additional CI feature/platform coverage. Record exact results rather than a copied historical total. Regression assertions must be capable of detecting the original failure; do not weaken tests, skip an active interval or relabel an exception to create a pass.

Commit and push scoped art deliveries as `art:` on game `main`, as requested by the product owner. Claude implementation deliveries use the appropriate conventional prefix after checks. Stage explicit paths; never add the nested overlay or transfer archives. A release tag or publication is a separate acceptance gate, not a way to test this workflow.

Build the capture executable from a clean, identified implementation commit. Record source SHA, executable hash, platform and GPU/driver. Capture using the existing suite and that full SHA:

```text
target\release\overframe.exe --capture <output-directory> --sha <40-character-source-sha>
```

One worker owns the live game/runtime capture at a time. Verify the actual executable and checkout being observed. Do not close an unrelated user's app or trust images from an unidentified running instance.

Fetch matching CI evidence, verify the manifest and hashes, and review the actual `game3d` images. Keep source SHA and evidence commit distinct. A software-rendered CI capture can verify geometry and appearance, but it cannot establish real Windows controller input or stable hardware frame rate. A passing compile or smoke run also cannot establish those claims.

The Kestrel gate includes all 22 action IDs and 23 current variants, both facings, exact designated pieces, all real active/late ticks, charge, jab chaining and hitlag. Neutral special needs both its real projectile emission and its later real fighter hitbox, with the supplemental view required by the animation contract. Ground support requires both foot bounds and their support-plane measurements. Preserve existing simulation, damage, frame data, hitboxes, capsules, model scale and bone lengths; report unreachable geometry.

Write review observations by piece/pose against the actual source SHA. `not_run` stays unverified; numerical contact alone does not constitute visual approval. Preserve fixed cameras and runtime event provenance. An art pass requires the specified same-source images, not a stale image, concept, renamed character or test summary.

## Continue without losing state

Append meaningful dated checkpoints to the current review rather than replacing history. Keep implemented, transferred, tested, visually reviewed and approved states separate. When an external limit interrupts work, record the exact blocked step, last integrated source, partial work location and next action. Resume that work when access returns instead of repeating research or assuming partial edits passed.

The user has authorized needed model switches. That does not imply purchasing credits. Scheduled unattended follow-ups must have the required specific authorization and a successfully created schedule; a proposed or rejected follow-up is not running.

These Markdown instructions do not install or execute hooks. The overlay's TypeScript lint hooks, pnpm commands and Observer endpoints would not validate this game. Automatic game checks remain the existing GitHub workflows; any additional executable integration must be authored and tested by Claude within the agreed role.
