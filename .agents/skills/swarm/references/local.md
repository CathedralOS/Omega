# Local Swarm (Shared Host Setup)

This reference defines the local shared-machine transport for running concurrent agents on one host with a local Rust toolchain.
- **Worker workflow:** Every worker follows [advance](../../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../../AGENTS.md#law-of-completion) for capability selection, validation, and board updates. Do not duplicate `/advance` rules in worker prompts; point the worker to `.agents/skills/advance/SKILL.md`.
- **Coordinator review & landing:** Workers push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every diff using [coordinator-review](../../coordinator-review/SKILL.md) before running `cargo xtask land land <pr> --expected-head <sha>`.

## 1. Local Host Setup & Worktrees

Local subagents share one physical machine (CPU, RAM, disk, and process table):
1. **Preflight:** Fetch `origin/main` (`git fetch origin main`) and check live claims (`cargo xtask claims status`) and open PRs before dispatching. Reserve outcomes with [dispatch.md](../../../../tools/swarm/dispatch.md).
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
   - Instruction to follow [advance](../../advance/SKILL.md) (`/advance`), stop at a **draft PR** (`gh pr create --draft`), and message the coordinator with `candidate_ready` for [coordinator-review](../../coordinator-review/SKILL.md).
3. **Reactive wakeup only (no timers or polling):** After spawning or messaging subagents or starting a background task, end the turn and wait for subagent messages or task-completion notifications to wake the coordinator. Never call `schedule`, cron, or status-polling loops.
4. **Warm workbench continuation:** When a worker's PR passes [coordinator-review](../../coordinator-review/SKILL.md) and lands, message the same worker (`send_message`) on its warm worktree with the next requirement of its parent outcome. At the user's stopping condition, stop refilling, drain active workers, release owned claims, and report observed outcomes per [Explain the change](../../../../AGENTS.md#explain-the-change).
