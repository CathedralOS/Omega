---
name: swarm
disable-model-invocation: true
description: Use only when the user explicitly requests swarm. Coordinate concurrent Omega agents using local or cloud session tools.
---

# Swarm (Platform Setup & Coordination Adapter)

This skill covers platform setup and how to start and message worker
sessions. Ownership follows [AGENTS.md](../../../AGENTS.md#sessions-and-subagents),
and merging follows
[landing](../../../wiki/development/landing.md). Point workers to those files; do not
copy their rules into prompts.

- **Dispatch:** The coordinator chooses the outcome, reserves a worktree, registers the claim and starts the worker. Each worker reproduces the failure, implements and tests the repair, reviews the diff and lands it in its own `.agents/worktree/worker-N` pool slot and context window. A coordinator may review a diff; workers do not wait for its approval. Batch integration of uploaded source, used only on request, is in [Codex](references/codex.md).
- **Keep the warm base:** Record the commit the slot's build was made from: locally, the commit behind its `target/`; in a cloud session, the restored build's commit, printed by `cargo xtask worktree acquire`. Pass it as the fixed base and to `cargo xtask check-rules --base`. Rebase only for a needed prerequisite or a real conflict.
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
| **7. Native in-session collaboration** | `collaboration.spawn_agent`, `followup_task`, `send_message`, `wait_agent` | [references/dot.md](references/dot.md); these tools share the current computer and do not by themselves imply a cloud executor |
