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

For an explicitly scoped manual library check, a nextest filter can select a
crate and its reverse dependencies while retaining workspace features:

```sh
mbx nextest run --workspace --lib --no-fail-fast -E 'rdeps(=x86-encoding)'
```

Unlike the selector, this command does not add source-reader edges or audits.
[Nextest configuration](../.config/nextest.toml) disables retries, reserves the
whole test pool for process resource-limit cases, and schedules long package
fixtures early without a universal thread cap. Doctests remain a separate Cargo
test mode. [Temporary Windows measurements](../wiki/drafts/test_cycle_measurements.md)
explain why neither a thread cap nor nextest alone has an established full-suite
speedup.
