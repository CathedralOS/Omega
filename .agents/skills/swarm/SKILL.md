---
name: swarm
description: Coordinate a requested local or cloud swarm of concurrent Omega agents using the exposed platform tools. Covers Local, Antigravity, Claude Code, Codex, Cursor, Devin, and Dot session setup; execution and landing follow /advance and AGENTS.md.
---

# Swarm (Platform Setup & Coordination Adapter)

This skill defines environment setup and session transport. Execution ownership
follows [AGENTS.md](../../../AGENTS.md#sessions-and-subagents), the worker workflow
is [advance](../advance/SKILL.md), and publication follows
[landing](../../../tools/landing.md). Point workers to those sources instead of
copying their rules into prompts or platform references.

- **Dispatch:** The coordinator runs `/advance` Step 1 and starts workers immediately. Each worker owns Steps 2–5 in its isolated `.agents/worktree/worker-N` pool slot and context window, including self-review with [coordinator-review](../coordinator-review/SKILL.md) and verified landing. A coordinator may review a diff; workers do not wait for routine coordinator approval. Explicit source-upload batch integration is a separate mode described in [Codex](references/codex.md).
- **Keep the warm base:** Record the source commit that matches the slot's retained build. Locally that is the slot's warm `target/`; in a cloud session it is the restored build's published commit, reported by `cargo xtask worktree acquire`. Pass it as the fixed base and to `cargo xtask check-rules --base`. Rebase when a task needs a newer prerequisite or has an actual conflict, following the canonical landing rules.
- **Reactive wakeup only:** Never schedule periodic timers, `schedule`, cron, `send_later`, or status-polling loops. End the turn after spawning or messaging workers and let completion notifications wake the coordinator.

## Select Your Environment Reference

Read **only** the reference matching the runtime tools exposed in your current environment:

| Environment | Exposed Runtime Tools / Signals | Setup & Transport Reference |
| --- | --- | --- |
| **1. Local Host** | `invoke_subagent`, `send_message`, `manage_subagents` on a host with local `cargo`/`mbx` | [references/local.md](references/local.md) & [references/host-setup.md](references/host-setup.md) |
| **2. Antigravity Cloud** | `invoke_subagent`, `send_message`, `manage_subagents` in an `exebox` Linux VM; verify toolchain and execution access | [references/antigravity.md](references/antigravity.md) |
| **3. Claude Code Remote** | `claude-code-remote` (`list_environments`, `create_session`, `get_session`, `send_message`) | [references/claude.md](references/claude.md) & [tools/agent_setup.sh](../../../tools/agent_setup.sh) |
| **4. Codex Cloud** | `list_threads`, `create_thread`, `send_message_to_thread`, `wait_threads` | [references/codex.md](references/codex.md) |
| **5. Cursor Cloud** | `cursor` `Task` tool with `environment: "cloud"` and `run_in_background: true` | [references/cursor.md](references/cursor.md) |
| **6. Devin Cloud** | Devin cloud session tools + `cargo xtask claims` | [references/devin.md](references/devin.md) |
| **7. Dot Collaboration** | `collaboration.spawn_agent`, `followup_task`, `send_message`, `wait_agent` | [references/dot.md](references/dot.md), [references/cloud-launch.md](references/cloud-launch.md), [references/cloud-coordination.md](references/cloud-coordination.md) & [references/recovery.md](references/recovery.md) |
