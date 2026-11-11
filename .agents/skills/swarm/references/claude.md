# Claude Swarm (`claude-code-remote` Adapter)

This reference defines the `claude-code-remote` container transport for running a wave of Claude Code cloud sessions.
- **Worker workflow:** Every session follows [advance](../../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../../AGENTS.md#law-of-completion) for capability selection, the 9-place sample loop, validation, and board updates. Point sessions to `.agents/skills/advance/SKILL.md` rather than duplicating `/advance` instructions.
- **Coordinator review & landing:** Sessions push a **draft** PR (`gh pr create --draft`) and stop (`candidate_ready`). The coordinator audits every diff using [coordinator-review](../../coordinator-review/SKILL.md) before running `cargo xtask land land <pr> --expected-head <sha>`.

## 1. Claude Cloud Session Tools

Load the deferred `claude-code-remote` tools with `ToolSearch` first:

| Operation | Tool | Notes |
| --- | --- | --- |
| Pick the environment | `list_environments` | Use the `Omega` environment's returned `env_…` id; never guess it. |
| Start a session | `create_session` | Pass `model`, `environment_id`, `source_url`, `source_revision`, `outcome_branch`, `tags`, `permission_mode: "auto"`, `prompt`. |
| Read state and cost | `get_session` | `status_bucket`, `session_context.model`, `external_metadata.last_served_model`, `usage.cost_usd`, `post_turn_summary`. |
| Read the transcript | `list_events` | Pass `kinds` (e.g. `["assistant","result"]`). Slice saved page files instead of rereading. |
| Continue a session | `send_message` | Arrives as a peer message to continue on the warm container. |
| Stop a turn | `interrupt_session` | Ends the current turn only. |
| Release the container | `archive_session` | Only after all work is landed or abandoned and confirmed pushed. |

Never schedule periodic timers, `schedule` calls, `send_later` check-ins, or sleep loops to poll sessions; rely on reactive notifications so coordinator tokens are not wasted.

## 2. Warm Containers & Dispatch

1. **Start sessions warm:**
   - Set the environment's setup script to `if [ -d Omega ]; then bash Omega/tools/agent_setup.sh; fi`; [agent_setup.sh](../../../../tools/agent_setup.sh) installs the pinned Rust toolchain, `zstd`, `mbx`, prebuilt `cargo-nextest` and the crates.
   - Instruct each session to acquire a pool slot via `cargo xtask worktree acquire <lane>-<topic>` and build with `mbx build -p omega` so sample and corpus gates reuse `target/` (direct `cargo build` / `cargo test` fallback is forbidden).
   - A session's slot starts at the commit the restored build was published for, not at the newest `origin/main`: `acquire` prints both commits. Building there takes under a minute; a few commits later, once a foundation crate has changed, it takes the full 11 minutes of a cold build. Instruct sessions to keep that base and rebase onto `origin/main` only when the task needs a commit that is newer or the PR conflicts; the merge queue integrates the rest. A slot with no restored build, or whose published commit `origin/main` does not contain, starts at `origin/main` as before.
   - Instruct sessions to run long builds/gates in the background and wait on completion notifications rather than `sleep` or `pgrep` loops, and pass `--base <commit>` to `cargo xtask check-rules`.
2. **Record & dispatch:** Fetch `origin/main`, reserve outcomes via [dispatch.md](../../../../tools/swarm/dispatch.md), record the wave ledger under `build/swarm/<wave>/`, and pass each session its `TASKS.md` capability, witness path, and instruction to follow [advance](../../advance/SKILL.md) (`/advance`), stop at a draft PR (`gh pr create --draft`), and print a final JSON verdict block (`candidate_ready`).
3. **Review, land & reuse:** When a session finishes, run the 5-point diff audit in [coordinator-review](../../coordinator-review/SKILL.md), land via `cargo xtask land land`, and `send_message` the next requirement of the parent capability to the same warm session.
