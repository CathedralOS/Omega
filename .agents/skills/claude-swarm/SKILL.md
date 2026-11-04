---
name: claude-swarm
description: Coordinate a requested wave of Claude Code cloud sessions, one session per capability outcome, each on its own container and branch, launched and followed through the claude-code-remote session tools. Use when the user asks for Claude sessions, a Claude swarm, or a model trial across Claude cloud sessions. Use dot-swarm for dot native workers, cursor-swarm for Cursor, devin-swarm for Devin, codex-swarm for Codex, and local-swarm for agents sharing one machine.
---

# Claude Code cloud session swarm

A Claude swarm runs one Claude Code cloud session per outcome. Each session
gets its own container, checkout and branch. It carries its outcome through
complete witnesses and lands them on main itself. The coordinator chooses the
outcomes, starts the sessions, reads their results and verifies each landing.
Writing or editing this skill does not launch a session or advance the
compiler.

Apply the [law of completion](../../../AGENTS.md#law-of-completion): each
session owns its outcome through acceptance, review and verified landing. Read
[AGENTS.md](../../../AGENTS.md), [advance](../advance/SKILL.md),
[claims](../../../tools/claims.md), [testing](../../../tools/testing.md) and
[landing](../../../tools/landing.md). Before automatic assignment, follow the
[reserve-before-dispatch procedure](../../../tools/swarm/dispatch.md).

## The session tools

The `claude-code-remote` tools are deferred; load them with ToolSearch first.

| Operation | Tool | Notes |
| --- | --- | --- |
| Pick the environment | `list_environments` | Use the `Omega` environment's returned `env_…` id; never guess it. |
| Start a session | `create_session` | Pass `model`, `environment_id`, `source_url`, `source_revision`, `outcome_branch`, `tags`, `permission_mode: "auto"`, `prompt`. |
| Read state and cost | `get_session` | `status_bucket`, `session_context.model`, `external_metadata.last_served_model`, `usage.cost_usd`, `post_turn_summary`. |
| Read the transcript | `list_events` | Pass `kinds` (for example `["assistant","result"]`). Pages over the size limit are saved to a file; slice that file instead of rereading. |
| Continue a session | `send_message` | Arrives as a peer message, not a user instruction. |
| Stop a turn | `interrupt_session` | Ends the current turn only. |
| Release the container | `archive_session` | Only after the work is landed or abandoned and the user is done with it. |

Never schedule periodic timers, `schedule` calls, `send_later` check-ins, or
sleep loops to poll sessions; rely on reactive messages from subagents or
other sessions so coordinator tokens are not wasted.

Measured on 2026-11-03 (twelve trial sessions):

- `create_session` sets the model but has no effort field. Every session
  gets the platform default. Record what `get_session` reports and never
  claim an effort level you did not set.
- A session that fails reports back as a `<child-session-event>`. One that
  finishes cleanly does not, so read `get_session` at each check-in.
- Each session prints its result in its own transcript. Ask for a final JSON
  verdict block so that `list_events` with `kinds: ["result"]` finds it.
- Six Opus 5.5 sessions cost $1.26–1.87 each in their first seven minutes.
  Their first long wait was the cold compiler build. Earlier whole advance
  sessions cost $12–18. Count cost before launching a wave.

## Start sessions warm

A new session is a fresh 4-CPU container. Without preparation it spent 3.5–5
minutes starting up and then 12–13 minutes on a cold `cargo build -p omega`
before it could run a program. Two pieces remove that cost:

- **The environment's setup script**, [references/environment-setup.sh](references/environment-setup.sh),
  pasted into the Omega environment (session title bar → environment → Edit
  → Setup script). It installs the pinned toolchain, crates, `zstd` and a
  prebuilt cargo-nextest, and finishes inside the roughly five minutes the
  environment needs to cache its snapshot. Building nextest from source pushed
  an earlier script past that limit, so it ran again at every session start.
  Keep the file and the saved script identical. When you change one, change
  the other in the same step.
- **The warm build** ([tools/warm_build.py](../../../tools/warm_build.py)). CI
  publishes main's build after every push. The repository's SessionStart hook
  restores it into `.agents/worktree/warm`, at the commit it was built from.

Measured on 2026-11-03 in a fresh session with both in place: the first
command ran 56 s after creation, and `warm_build.py wait` reported `ready`. The
first `cargo build -p omega` compiled 4 crates in 8.4 s, and the smoke fixture
exited 0 under `omega run --both`. The prompt keeps the session on that build:

- Run `python3 tools/warm_build.py wait` before the first build, then work in
  `.agents/worktree/warm` and record its commit as the base. If the status
  says the restore failed, the session builds cold there; report the reason.
- Don't move the warm worktree to a newer main before starting. Main's
  frequent changes to the lowest crates make that rebuild as slow as a cold
  one. The merge queue integrates with current main at landing.
- Build with a plain `cargo build -p omega`. The sample gate and the corpus
  gate reuse that `target/`. Never build a second copy into another target
  directory.
- Pin `source_revision` to the coordinator's fetched base when sessions must
  start from the same commit, as in a model comparison. Such a session works
  away from the warm build's commit, and its first build is cold.

The trial sessions also lost time waiting badly. Seven had `sleep` loops
blocked by the harness. In one, `while pgrep -f "cargo build …"` matched its
own shell and kept waiting 16.5 minutes after the build had finished. Tell
sessions to start a long build or gate with `run_in_background` and act on its
completion notification, and never to poll with `sleep` or `pgrep -f`. A
worktree made from a fetched commit has no `origin/main`, so they pass
`--base <commit>` to `tools/check_rules.py`.

## Select outcomes, then record before starting

1. Fetch `origin/main` and record its exact commit. A failed fetch stops the
   launch.
2. Choose distinct outcomes with the [sample loop](../../../AGENTS.md#the-sample-loop)
   selection rules, one per session, by root cause, not by file. Check
   `python3 tools/claims.py status` and open pull requests for live work on
   the same outcome.
3. Before calling `create_session`, write the wave ledger under
   `build/swarm/<wave>/` (or the session scratchpad): outcome, base, branch,
   model, prompt file and stopping condition. After each call, add the
   returned session id. An uncertain create is reconciled with
   `list_sessions` before retrying.

## The assignment prompt

Each prompt is self-contained: the session knows only what it says.

- The board item and its acceptance, the next requirement, the witness path
  and its expected result, and the other sessions' outcomes.
- Where to work: the warm worktree, at the commit it arrives on, which the
  session records as its base. Name a different base only when the work needs
  it: a comparison pinned to one commit, or an existing PR to rebase. Create
  another worktree only when the session needs two at once.
- Publication: push `HEAD` to `work/<name>`, open a draft PR, finish the
  validation, then `python3 tools/landing.py land <n> --expected-head <sha>`.
  In these sessions `landing.py` uses REST because the GitHub proxy refuses
  GraphQL. Requesting auto-merge queues a green PR.
- The [worker goal from dot-swarm](../dot-swarm/SKILL.md#the-workers-goal),
  copied verbatim.
- The verdict block it must end with: `item`, `requirement`, `result`
  (`landed` / `candidate_ready` / `blocked`), `base`, `branch`, `pr_url`,
  `commits`, `baseline_failure`, `witnesses`, `checks`, `remaining`,
  `blockers`.

For a model comparison, give both arms the identical prompt with only an
opaque label and branch changed. Pin the same `source_revision`. Forbid
reading any other trial branch or PR. Assign labels to models at random.
Stop each arm at a draft PR, so that two arms never land the same fact.

## Drain and continue

At each check-in, read every session's `get_session`. For a settled session,
read its verdict, fetch `origin/main`, and confirm that a `landed` result's PR
is MERGED with its submitted head. A landed witness is not the parent outcome:
while acceptance remains, `send_message` the same session its next
requirement, because its warm build and investigation carry over. Start a new
session only for a new independent outcome.

Archive a session only when its outcome is landed or abandoned and the user is
done with it. Archiving discards its container, including any unpushed
commits, so confirm that nothing unpushed remains first.

Report to the user in the order of [Explain the change](../../../AGENTS.md#explain-the-change):
what now runs, what remains of each outcome, then sessions, cost and base.
