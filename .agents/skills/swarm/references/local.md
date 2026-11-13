# Local Swarm (Shared Host Setup)

This reference defines the local shared-machine transport for running concurrent agents on one host with a local Rust toolchain.

Session ownership, execution and publication follow the [swarm skill](../SKILL.md), [advance](../../advance/SKILL.md), and [landing](../../../../tools/landing.md).

## 1. Local Host Setup & Worktrees

Local subagents share one physical machine (CPU, RAM, disk, and process table):
1. **Preflight:** Complete `/advance` Step 1, including live claims and open PRs, and follow [claim registration](../../../../tools/claims.md) before dispatching.
2. **One isolated `worker-N` pool slot per worker:** Never edit in the primary checkout and never run two editing agents in the same worktree:
   ```bash
   cargo xtask worktree acquire <lane>-<topic>
   ```
   Local named branches are banned. Reuse a worker's fixed `.agents/worktree/worker-N` slot across slices (`cargo xtask worktree release`) so its `target/` cache stays warm.
3. **Host build concurrency ([host-setup.md](host-setup.md)):**
   - Agent slots and expensive-build slots are separate. Keep each worktree's `CARGO_TARGET_DIR` independent (`CARGO_PROFILE_DEV_OPT_LEVEL=1`).
   - Before dispatching, fix the [host build budget](../../../../tools/build_slot.md#on-a-workstation-someone-is-using) and write it into every worker prompt: cap the total build load of all workers so the machine never reaches 100% (one expensive build at a time unless a shared limit caps the total), and on a workstation its owner is using run at the lowest OS priority. Worker subagents do not inherit the coordinator's environment.
   - Where supported, share verification builds as per-wave generations (`--cli` / `--runner`) built once at the wave base.

## 2. Launching & Communicating with Local Subagents

1. **Use native in-session subagent tools** (`invoke_subagent` / `send_message` / `manage_subagents`), never detached shell loops.
2. **Self-contained dispatch prompt:** Give each worker:
   - Its isolated worktree path (`.agents/worktree/<short-name>`) and fixed base SHA.
   - Its assigned `TASKS.md` capability outcome, witness path, and requirement to discharge.
   - Instruction to follow `/advance` through verified landing and report the PR URL, submitted head, published commit and observed limits.
3. **Reactive wakeup only (no timers or polling):** After spawning or messaging subagents or starting a background task, end the turn and wait for subagent messages or task-completion notifications to wake the coordinator. Never call `schedule`, cron, or status-polling loops.
4. **Warm workbench continuation:** After a worker verifies its landing, message the same worker (`send_message`) on its warm worktree with the next requirement of its parent outcome. At the user's stopping condition, stop refilling, drain active workers, release owned claims, and report observed outcomes per [Explain the change](../../../../AGENTS.md#explain-the-change).
