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
2. `plan --manifest <file>` — validates the manifest, runs `host_gates`,
   probes owning-path freshness (including each path's crate), renders one
   prompt per session to `build/swarm/<wave>/prompts/`, and prints the exact
   request bodies. Review the prompts. Use `--skip-host-gates` when the
   coordinator deliberately does not run them.
3. `launch --manifest <file>` — creates one session per entry and records
   `receipts.json`. A name with an existing receipt is skipped; pass
   `--relaunch <name>` to recreate just that session. `--dry-run` prints the
   bodies without network.
4. `status --wave <id>` — polls each session's status and ACU consumption.
5. `report --wave <id>` — merges receipts and structured output into
   `build/swarm/<wave>/report.md`, the input for a `retrospect` pass.

## Coordinator selection rules

Pick items that are:

- runnable on Linux x86-64 (no Windows/macOS/QEMU acceptance required),
- non-overlapping: no two sessions share an `owning_paths` entry,
- not in `exclusions` and not an item a human is actively landing,
- named in a board the launcher knows (`TASKS.md`, `TASKS_BOOTSTRAP.md`,
  `TASKS_OPTIMIZER.md`), with the item present as `**<item>.**`.
- each item's acceptance gates exit 0 on Linux; declare them in `host_gates`
  because `plan` runs them and refuses the wave otherwise,
- a slot whose slice may not be one crate is declared `probe_only`, so a
  `verification_only` result is planned rather than counted as a miss.

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
- The launcher holds no ownership. Session collisions with each other are
  prevented by the manifest; collisions with human work are absorbed by the
  landing protocol, exactly as for local `advance` invocations.
