---
name: swarm
description: Coordinate a local or cloud swarm of concurrent agents across 7 supported environments (Local, Antigravity Cloud, Claude Code Remote, Codex Cloud, Cursor Cloud, Devin Cloud, Dot Collaboration). Every worker follows /advance, and the coordinator reviews every candidate diff with coordinator-review.
---

# Swarm (Platform Setup & Coordination Adapter)

This skill defines the environment setup and session transport for running concurrent worker agents under a coordinator session.
- **Division of labor:** Each worker follows [advance](../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../AGENTS.md#law-of-completion) in its own isolated `.agents/worktree/worker-N` pool slot (`cargo xtask worktree acquire <lane>-<topic>`) and context window. The coordinator runs Step 1 (preflight, board/corpus triage from `TASKS.md` and `tests/omega/corpus_outcomes.txt` without reading `omega-rust/` source files to pre-diagnose the bug, worktree slot acquisition via `cargo xtask worktree acquire <lane>-<topic>`, and claims via `cargo xtask claims`) and launches workers immediately so workers own Steps 2–5. Point workers to `.agents/skills/advance/SKILL.md` rather than duplicating `/advance` instructions.
- **Base work on the built commit; pull `main` only when needed:** A worker starts where its slot's build already is and rebases onto `origin/main` only when its task needs a newer commit or its PR conflicts; the merge queue integrates everything else. Locally that is the slot's warm `target/`, kept across tasks. In a cloud session it is the commit the restored build was published for: `cargo xtask worktree acquire` leaves the slot there and prints both commits, because a few commits further on the first build is as slow as a cold one. Pass that commit as the worker's fixed base and to `cargo xtask check-rules --base`.
- **Coordinator review & landing:** Workers push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every candidate diff using [coordinator-review](../coordinator-review/SKILL.md) before running `cargo xtask land land <pr> --expected-head <sha>`.
- **Reactive wakeup only:** Never schedule periodic timers, `schedule`, cron, `send_later`, or status-polling loops. End the turn after spawning or messaging workers and let completion notifications wake the coordinator.

## Select Your Environment Reference

Read **only** the reference matching the runtime tools exposed in your current environment:

| Environment | Exposed Runtime Tools / Signals | Setup & Transport Reference |
| --- | --- | --- |
| **1. Local Host** | `invoke_subagent`, `send_message`, `manage_subagents` on a host with local `cargo`/`mbx` | [references/local.md](references/local.md) & [references/host-setup.md](references/host-setup.md) |
| **2. Antigravity Cloud** | `invoke_subagent`, `send_message`, `manage_subagents` in an `exebox` Linux VM without local `rustc` (validated via `compiler-pilot.yml`) | [references/antigravity.md](references/antigravity.md) |
| **3. Claude Code Remote** | `claude-code-remote` (`list_environments`, `create_session`, `get_session`, `send_message`) | [references/claude.md](references/claude.md) & [tools/agent_setup.sh](../../../tools/agent_setup.sh) |
| **4. Codex Cloud** | `list_threads`, `create_thread`, `send_message_to_thread`, `wait_threads` | [references/codex.md](references/codex.md) |
| **5. Cursor Cloud** | `cursor` `Task` tool with `environment: "cloud"` and `run_in_background: true` | [references/cursor.md](references/cursor.md) |
| **6. Devin Cloud** | Devin cloud session tools + `cargo xtask claims` | [references/devin.md](references/devin.md) |
| **7. Dot Collaboration** | `collaboration.spawn_agent`, `followup_task`, `send_message`, `wait_agent` | [references/dot.md](references/dot.md), [references/cloud-launch.md](references/cloud-launch.md), [references/cloud-coordination.md](references/cloud-coordination.md) & [references/recovery.md](references/recovery.md) |
