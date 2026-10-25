# Local swarm: one session, one measured host

This guide prepares **in-session subagents**, not separate Devin cloud VMs.
Read [the local-swarm skill](SKILL.md), [AGENTS.md](../../../AGENTS.md) and the
repository's testing/landing rules. Setup is not authority to launch a swarm.
Every editing agent owns an isolated worktree and an outcome through acceptance.

## Discover before asking

Recover outcome, stopping point, model/effort and authorized agent width from
the request, standing preferences and actual runtime controls. Ask only for a
missing real decision. A prompt cannot select a model; inherited or unavailable
effort controls must be reported honestly. Inspect live claims and retained
agent handles before resuming or dispatching. Do not hijack another outcome.

Use the user's requested worker width as the launch target. Before treating a
thread-limit rejection as unavailable capacity, inspect the runtime's agent
configuration and apply the authorized width through its supported settings.
For Codex, `agents.max_concurrent_threads_per_session` in `~/.codex/config.toml`
sets spawned-agent width, excluding the coordinator (`agents.max_threads` is its
legacy alias). Verify effective admission after changing it; a loaded session
may retain its earlier setting. Keep configured agent width, observed host
pressure and expensive-build concurrency separate in prompts and reports.
See the [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

Inventory the actual host before invoking tools that may auto-install runtimes:

| Record | Why it matters |
| --- | --- |
| OS/version and architecture | Local agents cannot provide other hosts' native coverage |
| CPU affinity/quota and visible CPUs | Cargo's per-process jobs setting is not a host-wide resource budget |
| Available RAM and enforced memory limit | Other agents, linkers and tests share it |
| Free disk on checkout/cache/target volumes | A managed-cache limit does not bound the entire disk |
| Pinned Rust, native linker/SDK, MBX, nextest, Python | Successful setup must match the executing shell/interpreter |
| Existing producers and cache roots | Reuse valid source/profile/toolchain receipts instead of cold rebuilding |
| Running builds, ownership and actual agent capacity | Agent slots are different from expensive-build slots |

Discover paths; do not assume Ubuntu directories everywhere. Check source
revision, dirty state, target, profile/flags, toolchain, binary hash and bundled
library root before reusing a compiler. Warmth does not qualify newer source.

## Native host readiness

| Host | Verify | Respect |
| --- | --- | --- |
| Linux | Native linker/development libraries, pinned tools, cgroup/affinity CPU and memory limits | XDG cache/config, mounted volumes, actual interpreter |
| Windows | Rust target, MSVC `link.exe` and SDK/developer environment, supported Python, Git long-path readiness | Git Bash versus PowerShell, native AppData paths, drive volumes, native child stdin |
| macOS | Architecture, Xcode/CLT, SDK/clang target, Rustup shims and repo pin, supported Python | Homebrew/native Library paths; ARM64 and Intel are different host claims |

A worker's implicit Rustup download is not a base-image prerequisite. If setup
must be persisted, read the effective blueprint and test missing-tool bootstrap
in an isolated tool home. Never uninstall active global tools as a probe.
Use documented installation routes and request approval for persistent changes.
Bound installer diagnostics; a witnessed external feed/SDK failure is not a
compiler regression or permission to bypass access/security controls.

Windows builders need a native EOF-producing entrypoint. Use a verified launcher
or Python `subprocess.run(..., stdin=subprocess.DEVNULL)`; the POSIX
`os.dup2`/`os.execvp` route was not suitable in the measured Windows environment.
On Linux/macOS use the configured guard or `< /dev/null`. Select the same Python
interpreter for package installation and project scripts. Use shell-native
syntax; a `.ps1` is not itself macOS support. Do not globally change Git config.

## Admit builds, not just agents

1. Reuse a valid baseline producer, or warm it once for the required source and
   profile. Keep one shared **MBX action cache**, with independent managed
   targets/worktrees. Do not share `CARGO_TARGET_DIR` or another agent's checkout.
2. Separate authorized agent width from expensive-build width. With no shared
   contention evidence, start one expensive build at a time; this is a conservative
   baseline, not a measured optimum. Other slots continue useful work on their
   assigned outcomes. Do not invent filler audits to keep slots occupied.
3. Include coordinator, Rust compilation, native linking, corpus execution and
   other resident processes in the CPU/memory envelope. Build admission applies
   across agents, not just within one Cargo invocation. Use an existing scheduler
   only after verifying what it coordinates across processes.
4. If overlap is justified, compare one versus a small higher build width on the
   same host/source/profile and comparable cache state. Record wall/CPU, peak RAM,
   queue delay, disk/cache growth and required behavior. Adopt only a real gain
   in useful outcome throughput without unacceptable E2E latency or pressure.
   Stop the experiment once the adequate envelope is established.
5. Keep worktree paths stable during an ongoing outcome. Reuse a completed slot
   only after preserving its branch/artifacts and verifying completion or that
   its session stopped. Never reset, stash or clean another session's state.

Use optimized dev (`CARGO_PROFILE_DEV_OPT_LEVEL=1`) for active Rust edit/rebuild
work. Release is useful for repeated execution and final qualification; do not
change repository profile defaults. Run focused checks before broad suites.
Build unique compiler edits against their actual source, not a peer's binary.

### Evidence, not universal presets

Devin host measurements at compiler revision
`f2acd8ecd41a20318368d18028be7e01b2739e9c`:

| Observed host | Same-checkout no-op | New optimized-dev worktree | Interpretation |
| --- | ---: | ---: | --- |
| Ubuntu x86_64, 8 vCPU / 31 GiB | 0.58 s release | 278 s, 120 hits / 82 misses | Path/workspace reuse remains partial |
| Windows x64, 8 CPU / ~32 GiB | 0.95 s release | 251 s, 148 hits / 56 misses | One jobs=4 comparison took 442 s; no case for forcing a lower cap on this host |
| macOS ARM64, 12 visible CPU / 16 GiB | 0.90 s release | 107 s, 147 hits / 55 misses | Jobs comparisons were warmth-confounded; no universal cap established |

These were single-build host measurements, **not concurrent local-swarm
throughput tests**. CPU count alone does not establish the best agent/build
width. Reuse relevant receipts; do not rerun the entire qualification campaign
for every wave or present these machines as required hardware.

Evidence: [Ubuntu worker](https://app.devin.ai/sessions/3aed8451c84649e19fce5ebbf456702b),
[Windows worker](https://app.devin.ai/sessions/4bb030fb07c74a2494c396b1a3940259),
[macOS worker](https://app.devin.ai/sessions/6907dd86375e4a2a883615e50958a54e).

## Share verification builds as generations

Per-slot verification builds were the dominant build-pool demand in the first
wide local wave: every worktree built its own test runner (and sometimes `omega`)
for its first gate run, producing median 17-minute and worst 75-minute pool
waits with 25-45 waiters. Batch them instead:

1. Pick the sub-batch base revision G and create the sub-batch worktrees **at G**.
2. Build ONE pair at G — the `omega` binary plus the corpus test runner — through
   the existing pool as a single hold (`tools/build_slot.py --slots 2 --
   <build both>`), then publish both outside any worktree (for example
   `build/swarm/<wave>/producers/gen-<rev11>/`) with a `GENERATION` file (exact
   revision + sha256 of each artifact) and a `READY` marker. With a warm MBX
   action cache the pair compiles in ~1-2 minutes; the pool queue dominates the
   wall time.
3. Every slot uses the published pair instead of building: place/twin runs via
   `<gen>/omega run --both ...`, gates via `tools/corpus_gate.py --cli
   <gen>/omega --runner <gen>/corpus_runner`.
4. Validity: a generation is valid for a worktree whose base B is psi-identical —
   `git diff G..B -- omega-rust` empty. Basing the sub-batch at G makes this
   true by construction. After psi landings, re-roll the generation for the next
   sub-batch: one build per batch, not per slot. Re-point the generation
   worktree (`git reset --hard G`) rather than creating a new one.
5. Slots never build for verification. A demonstrated red place needing a source
   repair may build at most once for that repair. Golden record legs still need
   a runner built at the revision the record must stamp.

Observed on a 16 GiB, 10-CPU macOS ARM64 host (2026-10-23): per-slot runner
builds queued 20-60 minutes each; after generations, an 11-slot sub-batch landed
three rows inside 90 minutes with zero verification builds, generation re-rolls
compiled in 42-81 seconds warm, and a 32-wide dispatch sat at load ~45-50 with
~1.5 GiB free (earlier 22-wide record phases peaked at load 99, ~90 MB free).
The next ceiling is not builds: it is golden-record memory floors (~1.2 GB free)
and landing rebase+verify traffic.

## Take the pool once for the whole chain

The pool is a race among pollers, not a queue, and its wait outlasts a
per-step pattern: on 2026-10-23, amid a fifty-lane LOC wave, a lane's single
chain acquisition waited 3,039 to 5,282 seconds (up to 88 minutes) for one of
two slots, and the wave's live logs recorded 1,594 `waiting for one of 2
slot(s)` announcements. A chain that re-queued for each check, test run and
landing re-verify would pay that wait several times and would not fit its own
`--timeout`.

Wrap the chain instead — one acquisition, strictly sequential steps:

    python3 tools/build_slot.py --slots 2 --timeout 5400 -- bash chain.sh

`hold()` exports `OMEGA_BUILD_SLOT_HELD=1` into everything it wraps, and
nested pool users — a nested `build_slot.py` and `corpus_gate.py`'s runner build —
inherit it and short-circuit. Local check → crate tests → scoped corpus gate
costs one acquisition. Release the hold before submitting or waiting for
GitHub's merge queue; integration builds run on GitHub
([build slots](../../../tools/build_slot.md#one-hold-for-a-whole-chain)).

Two habits keep a killed lane cheap to resume, learned across two host
restarts in one day:

- Commit as soon as the change is coherent, and amend or squash before
  publishing; a committed mechanism survives a session that does not.
- `claims.py note` at every milestone, not only before publish; the note is
  what a successor agent reads to continue without re-deriving the work.

This play is per machine. Worktrees of one checkout share its pool and its MBX
action cache; separate computers — cloud executors, another host — have their
own pools and cannot lend, batch or upload builds to each other, so each
session takes its own machine's pool once for its own chain. Cross-machine
coordination is the claims registry and the serialized landing queue; the
build pool is not a cross-machine resource.

The fix loop is not the chain. Repairing a compile error or a red test is a
minutes-scale check-edit-check loop; running each iteration as its own chain
attempt pays the multi-hour wait for a one-line fix. Do the loop inside the
ONE chain hold when you have it (acquire once, iterate until green, then
continue through the local gates, then release before PR submission),
and run quick single-crate checks in the separate diag pool
([build slots](../../../tools/build_slot.md#diagnostics-dont-queue-with-chains))
when you need one before a chain, so diag work never contends with the two
chain slots.

## Submit once to the merge queue

After focused validation and required review, push the source branch, open a PR,
and follow [landing](../../../tools/landing.md). GitHub checks the integrated
candidate and serializes publication. Main advancing does not send the author
back through a rebase/build/review loop. Resolve actual conflicts or failed
checks, and verify the merged PR's source head and published squash commit.
Do not keep local build slots occupied while waiting. The old `lane_land`
lease driver is retired; do not recreate it in per-lane scripts.

On this host `gh` is installed and authenticated (keychain) at
`/opt/homebrew/bin/gh`, but it is not on the PATH of non-login shells:
`command -v gh` failing in a plain `bash -c` context is not proof gh is
absent. Use `zsh -lic '...'`, or prepend `export PATH="/opt/homebrew/bin:$PATH"`,
for submission and queue commands. Run the `landing.py` helper from a tree that
carries the cutover (a fetched main checkout or any post-cutover worktree); a
checkout that predates it still has the retired lease CLI.

## Bound the shared disk safely

Read the effective MBX config and distinguish the action-cache cap from the
combined managed-storage cap. `max_total_size` is the combined cap; `max_size`
alone is not. Active/recent state and unmanaged outputs still need free space.
The qualified workers use a **30 GiB starting managed budget**, not a universal
optimum; discover each machine's disk and working set before applying it.

Use the configured pressure guard when present. The tested Linux/macOS starting
thresholds warn below 20 GiB free and refuse builds below 2 GiB after supported
collection; adapt policy only with host evidence. Coordinate pressure handling
across slots, preview with `mbx gc --dry-run`, and use supported GC. Help/dry-run
commands must not trigger deletion. A GC failure is not permission for manual
cache-directory deletion. Preserve active worktrees, unpublished source and
package-review receipts; remove only outcome-owned, verified disposable state.

## Dispatch, validate and finish

Use `tools/swarm/launch.py local --manifest <file> --create-worktrees` only when
authorized. It renders prompts/worktrees; it does not spawn agents. Use actual
in-session agent tools and returned handles if exposed. If unavailable, report
that limitation; do not substitute cloud sessions or detached shell agents.

Dispatch outcome, acceptance, exact worktree/base, model controls, retained
receipts, next action, dependencies, build-admission plan and publication
authority. Keep real agent states through `tools/swarm/fill.py` and the local
skill. Coalesce redundant baseline qualification, not checks for unique edits.

Required behavior is native execution agreeing with Terminal Psi plus relevant
invalid controls. Test-injected admission does not qualify the public CLI.
Never rewrite fixtures, copy stale locks or blanket-accept package authority.
Affected target/ABI/runtime changes need affected-host validation; local agents
on one VM cannot establish other OSes' native parity. Batch frozen-revision
cross-OS checks through authorized hosts. Release requires the repo's complete
host/architecture matrix, including Linux ARM64 where required.

At the stopping point, stop refilling, drain evidence, preserve unresolved work
and follow normal landing rules. Report one short readiness table: host/tools,
authorized agent width, measured build width, cache/disk policy, accepted outcomes
and remaining gaps. Never claim fleet or local concurrency qualification from
no-op timings alone.
