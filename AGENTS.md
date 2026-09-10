# Overframe agent coordination

This is a stable policy and navigation layer for Codex working alongside Claude.
Preserve `CLAUDE.md` and the existing `.claude/` agents, guides, hooks, and commands;
do not replace, rename, delete, or simplify that infrastructure as a migration.
Keep changing backlog, release, and session state in the existing shared documents.

## Authority and project references

- [CLAUDE.md](CLAUDE.md): architecture, commands, conventions, and operational knowledge.
- [WORKFLOW.md](WORKFLOW.md): human/agent process and approval boundaries.
- [TASKS.md](TASKS.md): priorities and backlog; verify mutable state against Git/code.
- [DEVLOG.md](.claude/DEVLOG.md): historical decisions and agent handoffs.
- [WORKSPACE.md](WORKSPACE.md): workspace and monorepo structure.

Explicit human task instructions take precedence over repository defaults.
Product-owner decisions outrank agent assumptions. Git and current code outrank
stale documentation for factual repository state; implementation is not permission
to weaken policy. Investigate and report documentation/implementation conflicts
rather than silently choosing one. Never rewrite historical DEVLOG entries to
pretend they describe current behavior.

Read the relevant guide before touching its domain:

| Domain | Guide |
| --- | --- |
| IPC, navigation, persistence, dependencies | [Security](.claude/guides/SECURITY.md) |
| UI, keyboard interaction, focus | [Accessibility](.claude/guides/ACCESSIBILITY.md) |
| Visual components, tokens, layout | [Design](.claude/guides/DESIGN.md) |
| Tabs, polling, rendering, memory | [Performance](.claude/guides/PERFORMANCE.md) |
| Logic, regression tests, QA | [Testing](.claude/guides/TESTING.md) |

## Team model

- Human: Product Owner and final approval authority.
- Codex main agent: Technical Lead/orchestrator when working in Codex.
- Codex subagents: specialized workers/reviewers for independent or parallel work.
- Claude: engineering agent using the existing `.claude/` infrastructure.

Git, TASKS.md, DEVLOG.md, and shared documentation coordinate the team. Codex and
Claude must never silently overwrite each other's active work.

## Mandatory session bootstrap

Before modifying anything in every Codex engineering session:

1. Run `git status --short`, `git branch --show-current`, and
   `git log --oneline --decorate -10`.
2. Read TASKS.md, the latest relevant DEVLOG entries, and WORKFLOW.md. Check entry
   dates and relevance; do not assume the first log entry is the newest.
3. Read CLAUDE.md when architectural or operational context is needed.
4. Read the relevant domain guides above.
5. Reconcile documentation with actual Git/code state. Distinguish cached remote
   references and historical check results from freshly verified evidence.
6. Inspect branches/worktrees for overlapping work, using `git branch -a` and
   `git worktree list` as needed. Resolve unclear ownership before overlapping edits.

Never use destructive Git commands to resolve uncertainty.

## Permanent constraints and approvals

- Windows-only product support until explicitly changed by the human.
- Development normally branches from `dev`; use PRs for integration. Never push
  directly to `main`. Preserve human merge/release authority.
- Explicit human approval is required for new npm dependencies, changes to
  `.github/workflows/`, and global shortcut behavior changes.
- Never publish releases automatically, including pushing release-triggering tags.
- Persisted schema changes require migration/backward-compatibility consideration;
  preserve WORKFLOW.md's human authority over major architectural/schema decisions.
- Renderer code has zero direct Node.js access. OS operations cross
  `window.aether` → preload → IPC → main. Declare new channels in
  `src/shared/ipc.ts` and validate main-side IPC inputs at runtime.
- Web content must remain isolated from Electron privileges. No analytics or
  telemetry; existing network/egress exceptions must not silently expand.
- Avoid scope creep. Never automatically perform destructive Git cleanup/reset,
  bypass safeguards, or rewrite another agent's uncommitted work.

Preserve the stronger approval boundaries in WORKFLOW.md, including human
validation before committing. When approval requirements are ambiguous, stop
and ask before taking an irreversible action. Do not infer push/PR authorization
from conflicting workflow summaries or slash commands.

## Ownership and delegation

For substantial work, record or communicate the owner, branch/worktree, scope,
acceptance criteria, baseline commit, and handoff status. Prefer separate
branches/worktrees for genuinely parallel implementation. Serialize edits to
shared coordination documents and integration where possible. Never switch
branches underneath another agent's uncommitted work.

Use subagents whenever independent work can improve speed or quality. Typical
roles: architecture/research, independent component implementation, QA/testing,
security, performance, accessibility, and regression review. Give each a bounded
scope and explicit write/review ownership.

The main Codex agent checks findings, resolves conflicts, integrates code, inspects
the final diff, runs final verification, and reports evidence to the human.
Subagent output is never automatically correct.

## Codex safeguards and verification

Claude hooks and slash commands do not automatically run in Codex. Explicitly
reproduce their intent:

- Before edits: inspect Git state, read relevant guides, confirm scope and approvals.
- During edits: keep changes scoped, use targeted lint/type/test feedback where
  useful, and never bypass Git or project safeguards.
- Before completion: inspect `git diff` and untracked additions, run appropriate
  targeted tests and required project gates, obtain applicable specialist reviews,
  verify runtime/visual behavior, and report remaining human testing.

For non-trivial changes follow:
understand → plan → implement → inspect diff → targeted tests → project-required
checks → runtime/visual verification when applicable → specialist/regression review
→ final evidence report → human validation where required.

Use `pnpm typecheck`, `pnpm lint`, `pnpm test`, `pnpm test:coverage`,
`pnpm build`, and `pnpm smoke` according to scope and WORKFLOW.md's required gates.
Rebuild relevant native C++/addon changes with `pnpm build:addon` before runtime QA.
Preserve the full pre-commit gate in WORKFLOW.md unless explicitly overridden by
the human. Report checks skipped and why; never label unrun checks as passing.

A compile, unit-test pass, or smoke pass alone does not prove product correctness.
Smoke is not visual correctness; focused coverage is not whole-product coverage.
Native WebView2 content may need additional observation, and gaming behavior still
requires human validation. Verify what metrics actually measure before making
performance claims. Confirm installed Git hooks in each checkout; do not assume
Claude hooks or worktree setup provide them.

## Runtime ownership

Single-instance behavior, the Observer at `127.0.0.1:9119`, browser/user data, and
Windows native state are shared resources. Only one agent/worktree owns active
runtime testing at a time; separate worktrees do not isolate those resources.
Never kill or replace another agent's running instance without coordination.
Verify the checkout/build being tested before trusting screenshots, metrics, or
logs. Observer evaluation and mutation endpoints are not read-only diagnostics.

## Completion and handoff

Report what changed, files changed, important design decisions, tests/checks run
and their results, runtime/visual evidence when relevant, specialist findings,
known limitations, remaining human validation, and the recommended next action.

Use TASKS.md and DEVLOG.md for meaningful cross-agent continuity when appropriate
and within authorized scope; do not mutate them merely because a session occurred.
Recheck Git state before handoff. Keep historical evidence intact and distinguish
completed implementation, verified behavior, and pending human approval.
