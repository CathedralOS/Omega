# Local testing

The canonical contract for validation policy, baseline gates, corpus filters,
test selection, and platform requirements. [AGENTS.md](../AGENTS.md) keeps only
the everyday command surface and links here; the sections below were promoted
from it. [baseline_gate.py](baseline_gate.py) runs the full-baseline gates as
one command.

## Validation scope

Routine changes, including `advance`, use focused regression coverage, affected
crate checks, and relevant integration tests. Choose checks from the changed
behavior and its dependencies/source readers, not from the mere existence of a
new worktree. Use crate-scoped check/Clippy for Rust changes.
For prose-only instructions, review consistency, links, and skill metadata;
run source audits only when their actual input rules are affected. No Rust build
is required merely to edit workflow prose.

A bug fix needs a witnessed regression and checks for affected behavior. Reuse
successful results when their inputs are unchanged. Full workspace/corpus runs
are for explicit baseline, health, or release work, or changes whose impact
cannot reasonably be bounded. Explain that scope before starting a full run.
An unverified base alone does not force a routine task to establish a full baseline.

Attribute unexpected failures with a focused baseline comparison or dependency
and source-reader evidence. Confirmed unrelated failures do not block landing a
scoped change: retain the command, revision, and evidence, and report them without
repairing them in the same task. New or unexplained failures in affected behavior
must be resolved before landing. Do not repeat broad suites to attribute one test.


## Running one test

A crate's own tests run by package and name filter:

```bash
mbx nextest run -p terminal-verifier boundary_requires
```

The `compiler` crate has no Rust test targets besides `corpus_runner`. Its
behavior is tested end to end: a fixture under `tests/omega/{pass,fail,run}` is
the test, and the corpus gate below runs it. Add a fixture, not a Rust test
file; see `omega-rust/omega/compiler/tests/README.md`.

## Corpus outcome gate

`tools/corpus_gate.py` is the compiler's end-to-end test. It runs the
`corpus_runner` binary (a `harness = false` test target that
compiles every `tests/omega/{pass,fail,run}` fixture through the compile and
checked-compile routes) and diffs per-fixture outcome records —
checked/rejected, diagnostic messages, expected-fragment satisfaction, and
per-fixture compile time — against `tests/omega/corpus_outcomes.txt`. A diff
means observable behavior moved; `--record` re-pins the golden when the
movement is intended.

```bash
python3 tools/corpus_gate.py --filter termination   # one domain in the loop
python3 tools/corpus_gate.py --filter wire/,fail/proofs
python3 tools/corpus_gate.py --record             # re-pin the golden
```

`--filter` matches comma-separated `tier/group/name` fragments (also
`OMEGA_CORPUS_FIXTURE_FILTER`); `--shard k/N` selects a
deterministic hash slice for splitting the run across sessions.

To measure what your own change moved, record a baseline once per base commit
and diff against it, rather than running the corpus twice per measurement:

```bash
git stash && python3 tools/corpus_gate.py --baseline --record && git stash pop
python3 tools/corpus_gate.py --baseline   # after every later edit
```

`--baseline` keys its file on `git rev-parse HEAD` under ignored
`build/corpus_baselines/`. Use it when the checked-in golden disagrees with
your checkout over another lane's in-flight movement, which is the usual case:
a run against the shared golden cannot separate that drift from yours, and
re-recording the shared golden would pin their regression as expected. A subset diff
compares only the fixtures that ran against the same golden; a subset
`--record` merges into it. The unfiltered corpus is scheduled-workload cost,
not loop cost — keep it out of routine iteration.

The default run checks fixtures only. `--native` builds every pass and run
fixture for the host target, executes the run tier and `*_exit` fixtures in a
temporary directory (stdin from `input.txt`, stdout compared with
`expected_stdout.txt`), and diffs against that host's golden
`tests/omega/corpus_native_<target>.txt`; records gain `exit:<code>` and
`stdout:match|differs`. Use it with `--filter` for backend-visible changes.

```bash
python3 tools/corpus_gate.py --native --filter providers/
```

Large structural moves (deleting a mechanism, rewriting a seam) may land while
some fixtures stop compiling, under one rule. A regressed pass or run fixture
must reject with a diagnostic whose message starts `unimplemented:` and names
the missing piece; the same commit re-records the affected goldens and states
how many fixtures moved. `grep -c "unimplemented:"` over the goldens is the
debt to burn down. Never allowed: a fail fixture that stops rejecting
(unsound acceptance), a crash, or a native program that builds and exits
differently from its golden. Those block landing.

## Bootstrap gates

Bootstrap gates are `sh` scripts, not Cargo tests, and self-skip when `python3`
is absent:

```bash
sh tools/bootstrap/check-chain-hygiene.sh
```

```bash
sh tests/bootstrap/alpha-beta-edge.sh --edge
```

The first is the single repository-topology gate. The second is the currently
closed bootstrap floor. There is deliberately no wrapper that pretends to run
the whole chain.


## Full baseline

Use these gates when establishing or refreshing a full checkout baseline:

```bash
python tools/fmt.py --check
mbx clippy --workspace --all-targets -- -D warnings
python tools/corpus_gate.py
mbx check --workspace --all-targets
mbx nextest run --workspace --lib --no-fail-fast
```

`mbx nextest run --workspace --lib --no-fail-fast` is the platform-portable
subset: all library tests, no target-specific executable/runtime legs.
`--no-fail-fast` is mandatory; retries are disabled. Platform integration tests
are separate and must report an explicit skip when the host cannot run them.

For a scoped recheck with a previously verified commit, use the
[test_affected.py](test_affected.py) selector documented below:
`--base VERIFIED_COMMIT` replaces the library gate commands above,
and a configured TypeSafe key adds a `jev` augment (see Jev semantic augment).
Keep checks applicable under Validation scope, including relevant
integration/bootstrap checks. If a full-baseline claim is needed and its prior
evidence, environment, or input dependencies are uncertain, use `--full`. For
routine work without that evidence, select and report scoped checks explicitly;
do not describe them as a verified full baseline.

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
integration/bootstrap gates. See the [full baseline](#full-baseline) gates above.
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
| Root `AGENTS.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `README.md`, `OWNER_QUESTIONS.md`, `TASKS.md`, `TASKS_BOOTSTRAP.md`, `TASKS_OPTIMIZER.md`; Markdown under `wiki/` | No library commands — reported under `documentation_paths` in the plan; a `jev` augment may still add checks. |
| Manifests, build scripts, lockfiles, toolchain/configuration, shared fixtures, Omega library sources, tools, and unknown inputs | All libraries. Markdown outside the allowlist, including this file, also takes this fallback. |
| No changed input | No commands; library filter `none()`. |

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
environment, and external inputs. Follow [validation scope](#validation-scope)
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
a std-compile for a name-resolution change) but never removes baseline
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

## Slow builds

Use [test-cycle measurements](../wiki/drafts/measurements/test_cycle_measurements.md) to distinguish
compilation, test execution, and repeated landing validation. The Windows
measurements did not establish a stable universal test-thread cap or a full-suite
speedup from nextest alone; avoid unrelated test execution using the selector above.

If small crates each pause for seconds before parsing, inspect `target/` before
changing test or compiler architecture: a long-lived `target/debug/deps` with
hundreds of thousands of stale hashed artifacts makes rustc rescan it per
crate. `cargo clean` fixes it.

