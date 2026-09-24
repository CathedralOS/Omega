# Local test selection

[test_affected.py](test_affected.py) selects tests for changes since a previously
verified commit. Validation policy, baseline gates, compiler corpus filters,
and platform requirements belong in [AGENTS.md](../AGENTS.md#commands).

## Requirements and commands

The selector requires Python 3.9+, Git, the pinned Rust toolchain, and
`cargo-nextest` 0.9.140 or newer. It uses `mbx` when available, otherwise Cargo.
Check `mbx nextest --version` (or `cargo nextest --version`); nextest installation
instructions are at <https://nexte.st/docs/installation/pre-built-binaries/>.
Run these commands from the repository checkout.

PowerShell:

```powershell
python tools/test_affected.py --base VERIFIED_COMMIT --plan
# Inspect the plan before running:
python tools/test_affected.py --base VERIFIED_COMMIT
```

macOS shell:

```sh
python3 tools/test_affected.py --base VERIFIED_COMMIT --plan
# Inspect the plan before running:
python3 tools/test_affected.py --base VERIFIED_COMMIT
```

Use the exact commit whose baseline is verified, not an unverified `origin/main`.
`--full` replaces `--base VERIFIED_COMMIT` to select the complete portable test
phases; it does not run formatting, Clippy, workspace checking, doctests, or all
integration/bootstrap gates. See the [full baseline](../AGENTS.md#full-baseline).
The same Python invocation with `--full --plan` previews those commands.

The process returns nonzero when any selected phase fails and still attempts
the remaining phases. In a PowerShell wrapper, propagate `$LASTEXITCODE`; in a
shell wrapper, propagate `$?`. Do not edit the worktree during a run. Retain the
printed plan with results: the selector neither caches evidence nor certifies
its base or grants a landing reservation.

## Coverage and limits

The JSON plan reports the resolved base, changed paths, affected packages,
documentation paths, fallback reasons, filter, and exact command arguments.
Git compares the base tree to current working files, including staged,
unstaged, deleted, moved, and untracked nonignored files. It includes incoming
main changes rather than using a merge-base. Invalid references fail.

| Changed input | Selection |
| --- | --- |
| Rust `.rs` under a known workspace crate's `src/` | That crate and declared reverse dependencies, including dev/build/optional/platform dependencies, plus known source-reader edges. |
| Root `AGENTS.md`, `CLAUDE.md`, `README.md`, `OWNER_QUESTIONS.md`, `TASKS.md`, `TASKS_BOOTSTRAP.md`, `TASKS_OPTIMIZER.md`; Markdown under `wiki/` | Architecture and the exact compiler corpus audit named in `DOCUMENTATION_TEST`; no libraries unless other inputs require them. |
| Manifests, build scripts, lockfiles, toolchain/configuration, shared fixtures, Omega library sources, tools, and unknown inputs | All libraries. Markdown outside the allowlist, including this file, also takes this fallback. |
| No changed input | Architecture only; library filter `none()`. |

Architecture always runs because it reads repository layout and sources.
Mixed documentation/Rust changes retain the Rust dependency closure. Library
commands keep `--workspace` feature unification even for a narrow filter;
splitting compilation into separate package builds need not be equivalent.
A narrow selection containing only binary crates may report zero library tests
and succeed. Full library runs reject an unexpectedly empty suite.

`SOURCE_READERS` records dependencies invisible to Cargo, including
`terminal-codec`'s embedded verifier/proof sources. Changes to its source-reader
implementation force the full fallback. New cross-crate source or fixture
readers must be represented in this map or force full selection. New readers of
allowlisted documentation must be covered by its audit route or removed from
that allowlist. Arbitrary runtime filesystem reads cannot be inferred from
Cargo metadata.

Selection is a conservative dependency calculation, not an independence proof.
A verified-base recheck assumes unchanged host, toolchain, feature flags,
environment, and external inputs. Follow [validation scope](../AGENTS.md#validation-scope)
when that evidence is unavailable; do not describe scoped checks as a verified
full baseline.

## Jev semantic augment

When a TypeSafe API key is configured (`TYPESAFE_API_KEY`, or
`build/typesafe.env.txt`, or `~/.config/typesafe/typesafe.env.txt`), the plan
additionally asks a System One model which entries of
[test_select_candidates.json](test_select_candidates.json) the diff could break
*beyond what the deterministic baseline already covers*, and appends commands
for flagged candidates the baseline lacks. The augment is union-only: it can
add checks (e.g., a standard-library compile for a name-resolution change, or
a path-catalog architecture test for a file move) but never removes baseline
coverage, so every failure mode degrades to deterministic selection. The
`jev` plan field reports `augmented` with `flagged`/`added`/`covered`/
`suggested` lists. With no key configured the plan reports `unconfigured` and
behaves exactly as before; a configured-but-unreachable API prints one stderr
warning and reports `unavailable`. Suppress it with `--no-jev` or
`OMEGA_JEV_OFFLINE=1`. Responses are cached per request payload under
`build/test-select-cache/`. The candidate catalog is the maintenance surface:
descriptions state what each test observes and what breaks it — keep them
current or recall silently degrades.

`--base X --attribute LOG` is the sibling advisory for the *is this red
mine* question: it parses FAIL/FAILED/error lines from a captured test log
and classifies each against the candidate diff — `YOURS` (investigate the
commit), `baseline` (check whether it fails at base), or `environmental`
(flake/host/timing). Solo calls per failure — batch labels contaminate.

[triage_advisor.py](triage_advisor.py) is the companion advisory for the
other half of the debug ledger: pipe it captured failure output plus the
command and it names the most likely suspect commit inside a ranked window
(or `none_in_window` for infrastructure and pre-window history), the owning
layer, and a recent-change confidence. Advisory only — it orders
investigation, discharges nothing, and exits 0 on its own failures.

For an explicitly scoped manual library check, a nextest filter can select a
crate and its reverse dependencies while retaining workspace features:

```sh
mbx nextest run --workspace --lib --no-fail-fast -E 'rdeps(=x86-encoding)'
```

Unlike the selector, this command does not add source-reader edges or audits.
[Nextest configuration](../.config/nextest.toml) disables retries, reserves the
whole test pool for process resource-limit cases, and schedules long package
fixtures early without a universal thread cap. Doctests remain a separate Cargo
test mode. [Temporary Windows measurements](../wiki/drafts/measurements/test_cycle_measurements.md)
explain why neither a thread cap nor nextest alone has an established full-suite
speedup.

## Release matrix

[release_matrix.py](release_matrix.py) runs the
[release matrix](../wiki/drafts/reference/rust_compiler_completion.md#release-matrix):
the eight `RC-*` gates with their exact contract invocations plus the hosted
platform rows, and writes the release record (`commit`, toolchain, host,
commands, results, elapsed time, expected skips). It does not substitute for
the selector — `--plan` reviews what would run, `--run` executes. See
[release_matrix.md](release_matrix.md).
