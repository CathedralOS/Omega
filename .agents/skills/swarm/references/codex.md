# Codex Swarm (Codex Cloud Thread Adapter)

This reference defines the Codex cloud-thread transport for coordinating independent Codex executors.

Session ownership, execution and publication follow the [swarm skill](../SKILL.md), [advance](../../advance/SKILL.md), and [landing](../../../../tools/landing.md).

## 1. Codex Session Tools & Controller Adapter

Discover the host's exposed Codex session schemas before dispatch:

| Capability | Codex tool |
| --- | --- |
| Inspect existing sessions | `list_threads`, `read_thread` |
| Select an accessible project | `list_projects` |
| Create a requested session | `create_thread` |
| Resume the same session | `send_message_to_thread` |
| Observe completion or required input | `wait_threads` |

- Follow `/advance` preflight and [claim registration](../../../../tools/claims.md). Record each assignment's returned thread and environment IDs, fixed base, phase and evidence under `build/swarm/<wave>/`; dispatch through the currently exposed session tools.
- Reuse settled sessions in the same saved Omega environment so their checkout, `target/`, and MBX caches stay warm. Each independent executor runs [cloud launch preflight](cloud-launch.md).
- Never schedule periodic timers, cron jobs, or polling loops; use bounded `wait_threads` or reactive completion events.

## 2. Dispatch & Optional Batch Intake

1. **Standard PR mode:** Dispatch each session with its capability, witness, fixed base, and instruction to follow `/advance` through verified landing. After the session reports the PR URL, submitted head, published commit and observed limits, send the next requirement of the parent capability via `send_message_to_thread`.
2. **Compact source batch-intake mode (when explicitly requested):**
   - Assign one integration session ownership of combined validation, review and publication. Source workers do not open or merge individual PRs in this mode; they package `committed.patch`, `staged.patch`, `unstaged.patch`, and a provenance manifest into a small source ZIP (`< 50 MiB`, no build caches or `.git`) and report `source_ready` with their actual checks and remaining limits.
   - The integration session materializes and verifies hashes locally, integrates compatible candidates sequentially onto a clean isolated worktree from fetched `origin/main`, and validates the combined closure under [testing](../../../../tools/testing.md). It reviews the combined diff with [coordinator-review](../../coordinator-review/SKILL.md), publishes and lands under [landing](../../../../tools/landing.md), and verifies remote main. Source intake alone does not complete the assigned outcome.
