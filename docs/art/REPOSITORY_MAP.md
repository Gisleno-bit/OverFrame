# Repository identity and preserved work

On 2026-09-11 the product owner confirmed that this task is the fighting game with Kestrel, Boulder, Viper and Trama. Its canonical repository is [Gisleno-bit/OverFrame](https://github.com/Gisleno-bit/OverFrame), on `main`.

## Two products with the same name

| Repository | Product and stack | Development work to preserve |
| --- | --- | --- |
| `Gisleno-bit/OverFrame` | Platform fighting game; Rust, macroquad, deterministic simulation and rollback networking | Fighter/stage specs, game source, frame-data exports, fixed-camera evidence and art reviews |
| `Gisleno-bit/Overframe-1` | Windows browser overlay; Electron, React, TypeScript and native WebView2 | Browser tabs, profiles, sessions, collections, overlay behavior, native shutdown fixes and its existing agent infrastructure |

The local game checkout currently contains an independent overlay checkout in its `Overframe/` directory. That directory has its own Git history and remote. It is not a game module, dependency or submodule to add to the game. `Claude outputs/` holds transfer archives, not game source. Verify both before any operation that recurses beyond tracked files. Prefer `git ls-files` for a repository inventory; never use a blanket add of these directories.

The game remote also retains `chore/codex-agent-coordination` at `f5fd00f3cc92a144465571b15dec1d442f9278f0`, an overlay-history branch. It is not a game implementation branch. No pull request existed for it at this audit. Preserve it as historical work; do not merge it into game `main` as an agent-setup shortcut.

## Audit snapshot, not a moving status assertion

The following facts were checked against Git and GitHub on 2026-09-11:

- Game `main` was `596d2b28945f25a634adf8eb4a31025f4024f9f5`, with 127 tracked files. Only the independent overlay directory and old Claude delivery directory were untracked.
- Overlay `dev` and `origin/dev` were `1a8589e1ba24d23b47b6510f4483912cbcd1c39d`, with 280 tracked files and a clean worktree. [PR #3](https://github.com/Gisleno-bit/Overframe-1/pull/3) had merged the session-preservation and WebView2 window-class fix into `dev`; its [CI run](https://github.com/Gisleno-bit/Overframe-1/actions/runs/34524206585) passed. This is historical verification of that commit, not a new runtime test.
- The selected game `main` and overlay `dev` histories have no common commit. An all-ref comparison does contain overlay history because of the separate coordination branch described above.
- The overlay's current tracked text contained none of the fighter/stage identifiers above. All 20 unique fetched `origin` branch tips were inspected for Rust/Cargo files and fighter-named paths; none were found. Binary image contents were not audited for this conclusion.

No product source, assets, dependencies, CI workflows, branches or Git histories were merged as part of this reconciliation. No checkout was moved or deleted and no remote was repointed. The overlay's own development rules remain in its repository.

## What was brought into the game workflow

The game now uses adapted coordination practices from the overlay's [AGENTS.md](https://github.com/Gisleno-bit/Overframe-1/blob/1a8589e1ba24d23b47b6510f4483912cbcd1c39d/AGENTS.md), [WORKFLOW.md](https://github.com/Gisleno-bit/Overframe-1/blob/1a8589e1ba24d23b47b6510f4483912cbcd1c39d/WORKFLOW.md) and dated development log:

- Identify the exact repository, branch, commit, owner and allowed paths before editing.
- Preserve existing agent work and hand off measurable acceptance criteria.
- Keep historical evidence distinct from current checks and pending approval.
- Assign one owner to live runtime testing and identify the binary behind evidence.
- Use regression checks that can detect the original failure, and state their actual coverage.

These practices are implemented in [WORKFLOW.md](WORKFLOW.md) and the scoped [AGENTS.md](AGENTS.md), retaining the game's [roles](ROLES.md) and [evidence contract](EXCHANGE.md). Electron commands, WebView2 fixes, overlay branding, browser security rules, Store packaging and its `dev`/PR policy were not copied into the fighter.

## Resume the game

Read the latest dated checkpoint in [the Kestrel implementation review](reviews/91f9c5a2834353bbb7207dcc4d866a4e513fd48f.md), then recheck Git and the live evidence pointer described in [PROJECT_STATUS.md](../../PROJECT_STATUS.md).

The last integrated Kestrel implementation in that checkpoint is `91f9c5a2834353bbb7207dcc4d866a4e513fd48f`. Opus authored additional animation work in its existing Claude task, but no fresh complete source archive or final verification reached the Windows checkout before the session-limit interruption. Retrieve and verify that work; do not treat it as lost or already integrated. The archive handoff and remaining acceptance criteria are in [WORKFLOW.md](WORKFLOW.md).

The character order remains Kestrel → Boulder → Viper → Trama, followed by Lattice, HUD/identity, Windows/controller/performance validation and the pilot release materials. Repository reconciliation grants no animation, hardware or release pass.
