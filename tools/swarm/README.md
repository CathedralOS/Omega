# Swarm launcher

`launch.py` fans "advance the compiler" out to Devin Cloud sessions: the
coordinator writes a wave manifest that pre-assigns one board item per session,
the launcher creates one session per entry, and each session runs the ordinary
`advance` skill restricted to that item, landing through `tools/landing.py`.

The launcher writes no board text, holds no landing claim, creates no Git refs,
and keeps receipts only under the ignored `build/swarm/<wave>/` directory.
Partitioning lives in the manifest, not in any ownership system.

## Prerequisites

- Python 3.9+ and Git, run from the repository root (or pass `--repository`).
- `DEVIN_API_KEY` (a `cog_...` service-user key) and `DEVIN_ORG_ID` in the
  environment. They are read from the environment only and never printed.

Windows PowerShell:

```powershell
$env:DEVIN_API_KEY = "cog_..."
$env:DEVIN_ORG_ID = "org-..."
python tools/swarm/launch.py plan --manifest tools/swarm/waves/wave-1.json
```

macOS shell (use `python3`):

```sh
export DEVIN_API_KEY=cog_...
export DEVIN_ORG_ID=org-...
python3 tools/swarm/launch.py plan --manifest tools/swarm/waves/wave-1.json
```

`plan` needs no credentials and no network. `launch --dry-run` needs no
credentials either; real `launch`, `status`, and `report` call the Devin API
(`--base-url`, default `https://api.devin.ai/v3`).

## Workflow

1. Write a manifest (see `manifest.schema.json`; `waves/wave-1.json` is the
   worked example).
2. `plan --manifest <file>` — validates the manifest, runs `host_gates` and
   the omega-route check, probes owning-path freshness (including each path's
   crate), renders one prompt per session to `build/swarm/<wave>/prompts/`, and
   prints the exact request bodies. Review the prompts. Use
   `--skip-host-gates` or `--skip-route-check` when the coordinator deliberately
   does not run those checks.
3. `launch --manifest <file>` — creates one session per entry and records
   `receipts.json`. A name with an existing receipt is skipped; pass
   `--relaunch <name>` to recreate just that session. `--dry-run` prints the
   bodies without network.
4. `status --wave <id>` — polls each session's status and ACU consumption.
5. `report --wave <id>` — merges receipts and structured output into
   `build/swarm/<wave>/report.md`, the input for a `retrospect` pass.

## Coordinator selection rules

Prioritize immediate customer outcomes within the requested scope, not the ease
of producing a helper commit. Give one session the bounded end-to-end slice and
its application command; several pipeline crates can belong to that one slice.
Check live assignments with the coordinator before partitioning shared paths.
An exclusion applies to its wave, not all future work: reassess it when preparing
a new wave and retain it only for a current conflict or concrete blocker. Do not
edit a launched wave to reassign its running sessions.

Pick items that are:

- runnable on Linux x86-64 (no Windows/macOS/QEMU acceptance required),
- non-overlapping: review parent/child paths and shared dependencies as well as
  identical `owning_paths` entries; the launcher's exact-path check is not enough,
- not in this wave's `exclusions` or another session's active edit assignment,
- named in a board the launcher knows (`TASKS.md`, `TASKS_BOOTSTRAP.md`,
  `TASKS_OPTIMIZER.md`), with the item present as `**<item>.**`.
- ready to attempt on the assigned host: `host_gates` are environment/access and
  prerequisite checks that must pass before assignment. Keep the expected-red
  customer reproduction in the board acceptance or `suggested_first_slice`, not
  in `host_gates`. Acceptance must pass after implementation, not before selection,
- an optimizer item's owning path must be the `X-to-X` stage crate at the
  item's representation level, and that compiler-tree crate under
  `omega-rust/` must exist and be on the `omega` route; the route check does
  not reject test-harness crates outside `omega-rust/`. If the stage does not
  exist the slot is `probe_only` or dropped (wave-3 miss:
  `EXACT-MACHINE-SIMPLIFICATIONS`),
- `probe_only` is for an explicitly chosen feasibility investigation, not merely
  a cross-stage repair or a failing customer. Ordinary implementation slots must
  attempt the design-backed repair; helper tests alone do not establish closure.
- `budget_exhausted` requires active implementation of a slice the child still
  believes is landable. Diagnose scope honestly, but crossing a crate or stage
  boundary is not by itself a reason to stop at `verification_only`.

## Lead-only launch checklist

Before the first wave, a human with org access must:

1. Create a service user with `UseDevinSessions` under Settings > Service
   users, generate its `cog_` key, and note the org id.
2. Confirm Devin's GitHub integration covers both `CathedralOS/Omega` and
   `CathedralOS/Squalr-Omega` (the latter is needed for the blueprint's
   best-effort `git submodule update --init`) and can push `main` and
   `refs/coordination/*`.
3. Choose the agent mode in the manifest's `devin_mode` (wave 1 uses `fusion`).
4. Sync `.devin/blueprint.yaml` and wait for the snapshot build.

## Stated limits

- Cloud sessions expose no model choice. The only per-session knob is the
  agent mode (`normal`, `fast`, `lite`, `ultra`, `fusion`), passed as
  `devin_mode`; `null` means the org default mode. Which model backs each
  mode is not documented and cannot be pinned from the launcher.
- Snapshot warmth is relative to the snapshot's build commit. It buys
  crates.io downloads, the registry index, build scripts, and unchanged leaf
  crates; at this repository's commit velocity the first `cargo check` still
  rebuilds most of the workspace. Blueprints rebuild roughly every 24 hours.
- The launcher holds no ownership. The coordinator must reconcile assignments
  with other active waves and local work before launch and on overlap reports.
  A manifest partitions only its own sessions, and exact path checks cannot catch
  every shared dependency. The landing protocol serializes publication; it does
  not prevent competing implementations. Unknown ownership needs coordination,
  not an inferred claim from an old manifest or worktree.
