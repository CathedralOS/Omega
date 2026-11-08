# Codex Swarm (Codex Cloud Thread Adapter)

This reference defines the Codex cloud-thread transport for coordinating independent Codex executors.
- **Worker workflow:** Every session follows [advance](../../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../../AGENTS.md#law-of-completion) for capability selection, validation, and board updates. Point sessions to `.agents/skills/advance/SKILL.md` rather than duplicating `/advance` instructions.
- **Coordinator review & landing:** Workers push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every diff using [coordinator-review](../../coordinator-review/SKILL.md) before running `cargo xtask land land <pr> --expected-head <sha>`.

## 1. Codex Session Tools & Controller Adapter

Discover the host's exposed Codex session schemas before dispatch:

| Capability | Codex tool |
| --- | --- |
| Inspect existing sessions | `list_threads`, `read_thread` |
| Select an accessible project | `list_projects` |
| Create a requested session | `create_thread` |
| Resume the same session | `send_message_to_thread` |
| Observe completion or required input | `wait_threads` |

- Follow [dispatch.md](../../../../tools/swarm/dispatch.md) and [cloud.md](../../../../tools/swarm/cloud.md) (`spec-context`, `prepare-step`, `dispatch-step`, `save-step-result`, `complete-step`) to bind threads, track `build/swarm/<wave>/pool.json`, and preserve receipts.
- Reuse settled sessions in the same saved Omega environment so their checkout, `target/`, and MBX caches stay warm. Each independent executor runs [cloud launch preflight](cloud-launch.md).
- Never schedule periodic timers, cron jobs, or polling loops; use bounded `wait_threads` or reactive completion events.

## 2. Dispatch, Batch Intake (Optional) & Coordinator Review

1. **Standard PR mode:** Dispatch each session with its `TASKS.md` capability, witness path, and instruction to follow [advance](../../advance/SKILL.md) (`/advance`) and stop at a draft PR (`candidate_ready`). The coordinator audits the diff via [coordinator-review](../../coordinator-review/SKILL.md), lands via `cargo xtask land land`, and sends the next requirement of the parent capability via `send_message_to_thread`.
2. **Compact source batch-intake mode (when explicitly requested):**
   - Workers package `committed.patch`, `staged.patch`, `unstaged.patch`, and a provenance manifest into a small source ZIP (`< 50 MiB`, no build caches or `.git`) and report `source_ready`.
   - The integration session materializes and verifies hashes locally, integrates compatible candidates sequentially onto a clean worktree from fetched `origin/main`, builds and validates the combined closure ([qualification.md](../../../../tools/qualification.md)), runs [coordinator-review](../../coordinator-review/SKILL.md) on the combined diff, and lands through `cargo xtask land land`.
