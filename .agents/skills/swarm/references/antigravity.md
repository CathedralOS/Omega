# Antigravity Cloud Swarm (`exebox` VM)

This guide covers coordinating concurrent in-session subagents in an Antigravity Cloud Linux VM (`exebox` sandbox, `/home/antigravity/CathedralOS/Omega`). Session ownership, execution and publication follow the [swarm skill](../SKILL.md), [advance](../../advance/SKILL.md), and [landing](../../../../tools/docs/landing.md).

## 1. Subagent Invocation & Worktree Setup

1. **One isolated `worker-N` pool slot per worker:** Never edit in `/home/antigravity/CathedralOS/Omega`. Acquire a pool slot per worker before calling `invoke_subagent`:
   ```bash
   cargo xtask worktree acquire <lane>-<topic>
   ```
   Local named branches (`git checkout -b` / `git switch -c`) are banned.
2. **Invoke subagents with `Model: "inherit"` and `Workspace: "inherit"`:**
   - Use `invoke_subagent` with `TypeName: "self"`, `Model: "inherit"`, and `Workspace: "inherit"`.
   - Do **not** pass tiered model aliases (`"flash"`, `"pro"`, `"flash_lite"`) when `PlatformClient` is unconfigured in the remote agent container (`PlatformClient is nil`); `"inherit"` uses the active session's model directly.
   - Branched or shared workspace modes are unsupported in the container; isolation comes from each worker operating exclusively inside its assigned `.agents/worktree/<short-name>` directory.
3. **Symptom-level dispatch:** Give each worker its worktree path, remote ref (`work/<short-name>`), and assigned `TASKS.md` outcome immediately after preflight so workers perform all `omega-rust/` reading and root-cause tracing in parallel.

## 2. Verify Compiler Execution Access

The observed `exebox` container lacked a local `rustc`/`cargo` toolchain and mapped `/home` and `/tmp` to UID `65534` (`nobody:nogroup`) under its user namespace. Omega's cache-ancestry ownership verification (`verify_cache_ancestry`) refused execution there. Verify the current executor through [cloud launch preflight](cloud-launch.md); do not assume those historical limits still apply.

Use an authorized executor with the pinned toolchain and valid custody for the focused checks required by [testing](../../../../tools/docs/testing.md). If this executor cannot provide them, report the exact limitation and preserve the source and evidence while continuing feasible work. Admission does not replace behavioral validation. Follow the current landing route once the assigned validation is complete; the retired hosted compiler-pilot workflow supplies no acceptance evidence.
