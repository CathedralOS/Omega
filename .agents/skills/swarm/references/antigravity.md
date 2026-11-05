# Antigravity Cloud Swarm (`exebox` VM + `compiler-pilot.yml`)

This guide covers coordinating concurrent in-session subagents in an Antigravity Cloud Linux VM (`exebox` sandbox, `/home/antigravity/CathedralOS/Omega`). Every worker follows [advance](../../advance/SKILL.md) (`/advance`) and [AGENTS.md](../../../../AGENTS.md#law-of-completion), and the coordinator audits every diff with [coordinator-review](../../coordinator-review/SKILL.md).

## 1. Subagent Invocation & Worktree Setup

1. **One isolated detached worktree per worker:** Never edit in `/home/antigravity/CathedralOS/Omega`. Create a detached worktree per worker before calling `invoke_subagent`:
   ```bash
   git fetch origin main
   git worktree add --detach .agents/worktree/<short-name> origin/main
   ```
   Local named branches (`git checkout -b` / `git switch -c`) are banned.
2. **Invoke subagents with `Model: "inherit"` and `Workspace: "inherit"`:**
   - Use `invoke_subagent` with `TypeName: "self"`, `Model: "inherit"`, and `Workspace: "inherit"`.
   - Do **not** pass tiered model aliases (`"flash"`, `"pro"`, `"flash_lite"`) when `PlatformClient` is unconfigured in the remote agent container (`PlatformClient is nil`); `"inherit"` uses the active session's model directly.
   - Branched or shared workspace modes are unsupported in the container; isolation comes from each worker operating exclusively inside its assigned `.agents/worktree/<short-name>` directory.
3. **Symptom-level dispatch:** Give each worker its worktree path, remote ref (`work/<short-name>`), and assigned `TASKS.md` outcome or `tests/omega/corpus_outcomes.txt` failure cluster immediately after preflight so workers perform all `omega-rust/` reading and root-cause tracing in parallel.

## 2. Hosted Compiler Validation via `compiler-pilot.yml`

The Antigravity `exebox` container has no local `rustc`/`cargo` toolchain and maps `/home` and `/tmp` to UID `65534` (`nobody:nogroup`) under its user namespace, which causes `omega`'s cache-ancestry ownership verification (`verify_cache_ancestry`) to refuse local execution. Workers validate compiler and fixture changes on GitHub Actions using [`.github/workflows/compiler-pilot.yml`](../../../../.github/workflows/compiler-pilot.yml):

1. **Commit on detached `HEAD` and push to `origin/work/<short-name>`:**
   ```bash
   git push origin HEAD:refs/heads/work/<short-name> --force-with-lease
   ```
2. **Trigger `compiler-pilot.yml` for the target fixtures or samples:**
   ```bash
   gh workflow run compiler-pilot.yml --ref work/<short-name> \
     -f fixtures='<comma-separated pass/...,fail/...,run/... fixtures>' \
     -f samples='<optional comma-separated samples/cli/... paths>' \
     -f native_host=linux_x86_64
   ```
   - `compiler-pilot.yml` runs `cargo check --locked -p omega` and executes `tools/ci/compiler_pilot.py` on a full Ubuntu runner (`linux_x86_64`, `linux_arm64`, `macos_arm64`, or `windows_x86_64`), uploading `compiler-pilot-records-<host>-<head_sha>-1` (`checking.json`, `both.json`, `samples.json`, `cli.json`).
   - Note: `compiler_pilot.py` validates `fixtures` under `tests/omega/{pass,fail,run}/` (check-mode for `pass/` and `fail/`, both-mode for `run/` and `pass/*_exit`).
3. **Download and splice golden records cleanly:**
   ```bash
   gh run download <run-id> -n compiler-pilot-records-linux_x86_64-<head_sha>-1 -D /tmp/pilot-<short-name>
   ```
   Splice `checking.json` into `tests/omega/corpus_outcomes.txt` and `both.json` into `tests/omega/corpus_both_linux_x86_64.txt` using `tools/corpus_records.py` (`corpus_records.splice`), commit the updated golden files and `TASKS.md`, push `HEAD:refs/heads/work/<short-name> --force-with-lease`, verify `python3 tools/check_rules.py --base origin/main --title "<title>"`, and open a draft PR (`gh pr create --draft --head work/<short-name>`).
