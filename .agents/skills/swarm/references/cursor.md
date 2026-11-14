# Cursor Swarm (Cursor Cloud `Task` Adapter)

This reference defines the Cursor cloud `Task` transport for running each subagent on its own cloud VM and branch.

Session ownership, execution and publication follow the [swarm skill](../SKILL.md), [advance](../../advance/SKILL.md), and [landing](../../../../tools/docs/landing.md).

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

1. **Dispatch:** Complete `/advance` preflight and [claim registration](../../../../tools/docs/claims.md), then launch concurrent background `Task` calls. Give each subagent its capability, witness, fixed base, and instruction to follow `/advance` through verified landing.
2. **Reactive wakeup only:** Background cloud subagents notify the coordinator when they finish. Never poll, sleep, or schedule timers.
3. **Reuse pod:** After a subagent verifies its landing and reports the PR URL, submitted head, published commit and observed limits, resume the **same** agent ID on its warm VM for the next requirement of its parent capability; compact only when switching to a different capability outcome.
