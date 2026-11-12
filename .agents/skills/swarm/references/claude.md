# Claude Swarm (`claude-code-remote` Adapter)

This reference defines the `claude-code-remote` container transport for running a wave of Claude Code cloud sessions.

Session ownership, execution and publication follow the [swarm skill](../SKILL.md), [advance](../../advance/SKILL.md), and [landing](../../../../tools/landing.md).

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
   - A session's slot starts at the commit the restored build was published for; `acquire` prints that commit and `origin/main`. Keep that fixed base when its producer inputs remain applicable, and update it for a needed prerequisite or actual conflict under the landing rules. A slot with no restored build, or whose published commit `origin/main` does not contain, starts at `origin/main` as before.
   - Instruct sessions to run long builds/gates in the background and wait on completion notifications rather than `sleep` or `pgrep` loops, and pass `--base <commit>` to `cargo xtask check-rules`.
2. **Record & dispatch:** Complete `/advance` preflight and [claim registration](../../../../tools/claims.md), record the wave ledger under `build/swarm/<wave>/`, and pass each session its capability, witness, fixed base, and instruction to follow `/advance` through verified landing. Retain the PR URL, submitted head, published commit and observed limits in the completion report.
3. **Reuse:** After the session verifies its landing, `send_message` the next requirement of the parent capability to the same warm session.
