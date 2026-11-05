---
name: local-swarm
description: Coordinate a requested local swarm using one isolated worktree per agent, shared-host build slot discipline, and coordinator-reviewed landing. Use for local concurrent agents sharing one machine. Every worker follows /advance, and the coordinator reviews every candidate diff with coordinator-review.
---

# Local Swarm (Platform Adapter)

This skill defines **only** the local shared-machine transport for running concurrent agents on one host.
- **Division of labor (Coordinator vs. Worker):** Each worker gets its own isolated worktree and clean context window specifically to own compiler source reading, root-cause tracing, and implementation ([advance](../advance/SKILL.md) Steps 2–5). Keep the coordinator's shared context light for triage and review: select distinct board items or diagnostic clusters from `TASKS.md` and `tests/omega/corpus_outcomes.txt` without reading `omega-rust/` source files in the coordinator, create the worktrees and claims, and launch the workers immediately. Do not duplicate `/advance` rules in worker prompts; point the worker to `.agents/skills/advance/SKILL.md`.
- **Coordinator review & landing:** Workers push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every diff using [coordinator-review](../coordinator-review/SKILL.md) before running `python3 tools/landing.py land <pr> --expected-head <sha>`.

## 1. Local Host Setup & Worktrees

Local subagents share one physical machine (CPU, RAM, disk, and process table):
1. **Preflight:** Fetch `origin/main` (`git fetch origin main`) and check live claims (`python3 tools/claims.py status`) and open PRs before dispatching. Reserve outcomes with [dispatch.md](../../../tools/swarm/dispatch.md).
2. **One isolated detached worktree per worker:** Never edit in the primary checkout and never run two editing agents in the same worktree:
   ```bash
   git worktree add --detach .agents/worktree/<short-name> origin/main
   ```
   Local named branches are banned. Reuse a worker's existing worktree across slices so its `target/` cache stays warm.
3. **Host build concurrency ([host-setup.md](host-setup.md)):**
   - Agent slots and expensive-build slots are separate. Keep each worktree's `CARGO_TARGET_DIR` independent (`CARGO_PROFILE_DEV_OPT_LEVEL=1`).
   - Take the local build pool once for the build/test chain via `tools/build_slot.py` and release it before waiting on GitHub CI.
   - Where supported, share verification builds as per-wave generations (`--cli` / `--runner`) built once at the wave base.

## 2. Launching & Communicating with Local Subagents

1. **Use native in-session subagent tools** (`invoke_subagent` / `send_message` / `manage_subagents`), never detached shell loops.
2. **Symptom-level dispatch prompt:** Give each worker:
   - Its isolated worktree path (`.agents/worktree/<short-name>`) and fixed base SHA.
   - Its assigned `TASKS.md` capability outcome, failing witness or `corpus_outcomes.txt` diagnostic cluster, and requirement to discharge (leaving `omega-rust/` code reading and root-cause tracing to the worker).
   - Instruction to follow [advance](../advance/SKILL.md) (`/advance`), stop at a **draft PR** (`gh pr create --draft`), and message the coordinator with `candidate_ready` for [coordinator-review](../coordinator-review/SKILL.md).
3. **Reactive wakeup only (no timers or polling):** After spawning or messaging subagents or starting a background task, end the turn and wait for subagent messages or task-completion notifications to wake the coordinator. Never call `schedule`, cron, or status-polling loops.
4. **Warm workbench continuation:** When a worker's PR passes [coordinator-review](../coordinator-review/SKILL.md) and lands, message the same worker (`send_message`) on its warm worktree with the next requirement of its parent outcome. At the user's stopping condition, stop refilling, drain active workers, release owned claims, and report observed outcomes per [Explain the change](../../../AGENTS.md#explain-the-change).
