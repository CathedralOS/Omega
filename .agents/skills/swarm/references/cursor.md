# Cursor Swarm (Cursor Cloud `Task` Adapter)

This reference defines the Cursor cloud `Task` transport for running each subagent on its own cloud VM and branch.
- **Worker workflow:** Every subagent follows [advance](../../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../../AGENTS.md#law-of-completion) for capability selection, validation, and board updates. Point subagents to `.agents/skills/advance/SKILL.md` rather than duplicating `/advance` instructions.
- **Coordinator review & landing:** Subagents push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every diff using [coordinator-review](../../coordinator-review/SKILL.md) before running `cargo xtask land land <pr> --expected-head <sha>`.

## 1. Cursor Cloud `Task` Tool & Environment

Inspect the `Task` schema from the `cursor` namespace before dispatch:

| Action | `Task` field |
| --- | --- |
| Separate cloud VM and branch | `environment: "cloud"` |
| Implementation owner | `subagent_type: "generalPurpose"` |
| Non-blocking background execution | `run_in_background: true` |
| Start from remote `main` | `cloud_base_branch: "main"` |
| Specific environment build (e.g. Windows) | `cloud_requested_environment_build_id` (when supplied) |

- Pass `grok-4.7-medium` unless the user specifies another exposed model slug (never `-fast`).
- Record slot assignments in `build/swarm/<wave>/inflight.json` before invoking `Task`, and attach each returned agent ID.
- Each cloud VM runs `bash tools/agent_setup.sh` to install/update `mbx` and builds in its checkout's `target/` with `mbx build -p omega` (direct `cargo build` / `cargo test` fallback is forbidden).

## 2. Dispatch, Reactive Drain & Reuse

1. **Dispatch:** Fetch `origin/main`, reserve outcomes via [dispatch.md](../../../../tools/swarm/dispatch.md), and launch concurrent background `Task` calls. Give each subagent its `TASKS.md` capability, witness path, fetched `main` SHA, and instruction to follow [advance](../../advance/SKILL.md) (`/advance`) and stop at a draft PR (`candidate_ready`).
2. **Reactive wakeup only:** Background cloud subagents notify the coordinator when they finish. Never poll, sleep, or schedule timers.
3. **Review, land & reuse pod:** When a subagent reports `candidate_ready`, run the 5-point audit in [coordinator-review](../../coordinator-review/SKILL.md) and land via `cargo xtask land land`. Resume the **same** agent ID on its warm VM for the next requirement of its parent capability; compact only when switching to a different capability outcome.
