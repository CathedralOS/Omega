# Local testing

Choose focused checks that show whether the changed behavior works, then land
through [admission and a squash merge](landing.md). Main verification follows
the merge; admission does not run the compiler tests for you.

[AGENTS.md](../AGENTS.md) owns the common workflow. This guide supplies test
selection, execution evidence and platform details. The
[CI rebuild plan](../wiki/drafts/designs/ci_rebuild_plan.md) records intended CI
work; [.github/workflows/](../.github/workflows/) defines what is implemented.
Older qualification procedures below do not reinstate retired checks.

## Validation scope

Prove the thing you built with tests on the changed code, then a very small
area around it. If that is fine, land. If it is not, investigate that failure.
There is no fixture corpus to run and no ledger: everything that ran or
recorded the corpus under `tests/omega/` (`corpus_runner`, `cargo xtask corpus`,
`cargo xtask corpus-gate`, the post-merge row measurement and the published
ledger) was removed on the owner's ruling of 2026-11-13, and the fixtures
themselves were deleted with it.
[tests/README.md](../tests/README.md) says how to read a deleted fixture from
history. A program a test reads is kept beside the tests, under
`tests/fixtures/`.

Iteration uses focused behavioral feedback; landing establishes the scoped
change and attributes affected failures; release runs the full release matrix.
A known-red unrelated baseline need not block a checkpoint, but is not release
acceptance. Add tests for missing behavioral distinctions, not mechanically
expanded combinations or test-count targets.

An agent may take **VALIDATION-ENVIRONMENT** or **ITERATION-PERFORMANCE** after
a round of compiler work (see [working the board](../TASKS.md#working-the-board)).
Name the obstructed workflow and adequate target first. Repair the bottleneck
or validation path, demonstrate the result and stop. Do not silently turn a
feature assignment into profiling or repeated broad test runs.

Routine changes, including `advance`, use focused regression coverage, affected
crate checks, and relevant integration tests. Choose checks from the changed
behavior and its dependencies/source readers, not from the mere existence of a
new worktree. Use crate-scoped check/Clippy for Rust changes.
For prose-only instructions, review consistency, links, and skill metadata;
run source audits only when their actual input rules are affected. No Rust build
is required merely to edit workflow prose.

A bug fix needs a witnessed regression and checks for affected behavior. Reuse
successful results when their inputs are unchanged. Full workspace runs
are user-triggered baseline, health or release work. Keep ordinary repairs on
the affected tests and crates.
An unverified base alone does not force a routine task to establish a full baseline.

Attribute unexpected failures from the actual diagnostic, dependencies and
source readers. Do not add before/after runs
against moving main. Confirmed unrelated failures do not block landing a
scoped change: retain the command, revision, and evidence, and report them without
repairing them in the same task. New or unexplained failures in affected behavior
must be resolved before landing. Do not repeat broad suites to attribute one test.

### Merge validation budget

Run the affected crates' unit tests and Clippy locally, with the tests that
read the same mechanism and the known affected samples. Reuse a verified warm producer
when its inputs still apply. A filter matching no executed tests supplies no
evidence. Compiler-program acceptance still needs its actual engine observations.
If resources obstruct the necessary run, identify that concrete blocker rather
than dispatching the retired `compiler.yml` or `compiler-regression.yml` workflows.

For each proposed build or test, name the decision it answers, the affected
behavior, and why applicable existing evidence is insufficient. Estimate time
and resources, including compilation. Reuse evidence only when its revision,
toolchain, configuration, relevant dependencies and consumers still apply;
distinguish cold and warm observations. Retain command, inputs and outcome.

If a run exceeds its budget, inspect progress, CPU activity, logs and blockers
before deciding whether to continue, narrow or revise it. Record the decision
and reason; do not blindly wait, restart or expand the suite. A justified slow
run may continue. A budget is not an automatic timeout or a passing result.
Reviewers rerun checks to resolve material evidence gaps, not every expensive
build by default. Required CI and release gates remain mandatory.

Before starting a build, record a short plan in the PR or claim: the behavior
changed, the smallest checks that establish it, reusable observations, and the
expected build plus execution time. Choose by affected behavior, not by a
growing list of every test added during the session. Capability-wide acceptance
does not automatically become the suite for every complete progress PR.

Routine merge validation should take minutes. If a planned run exceeds about
15 minutes, or another repair expands the plan, reassess its scope before
launching more work. Include compilation cost: seven seconds of selected tests
can still require a 25-minute build. Reuse a verified warm producer, select the
affected crate and tests, or use an existing hosted check where it establishes
the required behavior. A genuinely necessary slow check can run; state which
changed obligation requires it. This checkpoint introduces no permission gate
and never turns an unexecuted check into a pass.

Full Product compilation, self-hosting, bootstrap reconstruction, broad runtime
cohorts and release qualification are not routine progress-PR prerequisites.
Run them for an explicitly assigned acceptance that needs them. Unfiltered
workspace and full-baseline runs remain user-triggered only. Discovering a compiler performance problem
during an additional run does not automatically attach that repair and another
complete validation chain to an independently complete PR. Record its actual
failure and remaining work separately; preserve affected-behavior failures as
blockers and continue the parent capability after landing the completed step.

Once the scoped checks and any required review pass, push, update the PR, and
follow [admission and squash-merge landing](landing.md). Do not add more suites,
rebuild an unchanged producer, or create nested reviews of scripts/manifests merely to
increase confidence. Required semantic review examines the changed meaning and
relevant controls; it does not restart every earlier review. Keep the actual
revision, command and outcome in the existing PR/claim rather than building an
ad hoc qualification framework. Report pending or failed extended validation
honestly without describing the whole compiler or release as qualified.

The [PR #730 retrospective](../wiki/drafts/measurements/merge_validation_2026_10_31/README.md)
records the cost and failure pattern behind this rule.

### Historical pilot records

The Python pilot and its workflow were removed in #2080. Use the current Rust
qualification route below for new program observations. Existing pilot records
retain their original source, producer, profile and executor identities; assess
reuse through the [execution evidence rule](#execution-evidence-and-replay-inputs).
The [historical pilot protocol](https://github.com/CathedralOS/Omega/blob/868574432568c7bb2586952397d3e8a217ad79a0/tools/testing.md#exact-fixtures-on-a-hosted-pilot)
explains those original records, including replay plans and SDK-specific controls.

### Current Rust qualification

`mbx xtask qualify` runs a bounded, explicit selection of sample programs
through the ordinary CLI sample gate and retains the evidence. Name the
programs with `--samples samples/project,...` and a new output directory with
`--output`; an existing directory refuses rather than mixing runs. The
selection is limited to 200 programs. These are exact paths, not search
fragments, and each program's authored oracle is checked before compiler
preparation.

```sh
mbx xtask qualify \
  --samples samples/cli/facts/length/local \
  --output build/qualification-local
```

Use the command on the matching authorized host and retain its output
directory even on failure. Acceptance requires actual native and Psi exits at
the authored values, agreement, and any output oracle. A rejection counts only
with the sample's intended diagnostic.

The directory contains the exact request, source revision/tree and a standard Git
ZIP source archive, per-command stdout/stderr and exit receipts, parsed outcomes,
producer Cargo artifacts and fingerprints, binary digests, relevant build inputs,
and before/after source identities, with the accepted review decisions and
consumed project/dependency locks. Never commit the raw directory. A failed
summary is unfinished qualification.

### Optimized validation compilers

Current gates prepare executables through `mbx` for their source checkout. They
bind each actual image digest to Cargo's optimized artifact profile and its
matching fingerprint, including effective Rust flags. Effective optimization
level 0, hidden response-file flags, missing fingerprints, changed image bytes,
or changed source during preparation refuse.
Relevant toolchain, host, jobs, flags and profile inputs are retained; credentials
and the full process environment are never captured.

Rust compiler preparation has no wall-clock time cap: waiting for shared mbx
capacity is not compiler failure. Receipts retain elapsed time and use a null
`timeout_ms` for unlimited preparation. Other commands
retain their explicit/default limits; native execution and Psi work
budgets are unchanged.

Every shared command reuses `bounded-process`'s native owner through its trusted
tooling API, with a Unix process group or suspended-start Windows Job Object.
Windows assignment/resume failure uses the same stop-and-reap path as bounded
runtime launches; runtime resource-capped constructors remain unchanged.
On timeout, wait failure, normal completion or unwinding,
the owner terminates remaining descendants and reaps the command, preserving
peer jobs. Receipts retain the actual command status, original stdout/stderr,
container identity and cleanup errors; a cleanup failure cannot become a pass.
`timeout_cleanup.reaped` records reaping the owned command, not a claim that
every orphaned Unix descendant was adopted by this process. The same cleanup
is recorded as `container_cleanup` for non-timeout completion.

### Execution evidence and replay inputs

Record what a command actually did: source checking, target publication, native
execution or Terminal execution. A successful compiler exit or target flag does
not establish that a program ran. Runtime acceptance records each required
engine's actual exit and authored payload/output/writeback oracle. Where useful,
corrupt the expected result and verify the failure path in both engines. A reject
twin must reach its intended semantic obligation; missing imports, stale setup
or an unrelated refusal do not establish that the fact is enforced. Keep source
rejection, unsupported publication and runtime failure distinct, and name
unavailable hosts. Consult CLI help before combining unfamiliar modes or flags.

Prepare ordinary package acceptance before a frozen execution capture. Publishing
an ignored `omega.lock` can change its source directory's timestamps even when
tracked sources are unchanged, correctly invalidating that capture's stability
check. For the sample gate, run `--accepted-inputs-only` after normal per-project
review. It disables package updates and stale-state cleanup/retries; the normal
CLI still enforces exact accepted inputs and every authored execution oracle.
Retain the consumed locks as replay inputs. Do not relax source-mutation checks
or copy an accepted lock into another checkout as authority.

Fresh replay projects contain authored sources, declared resources and oracle
files. Exclude generated build directories, caches, binaries and package-review
state. Carry a lock only when that exact approved lock is an explicit check
input; otherwise generate it normally in the fresh project. Preserve historical
producers, products and locks unchanged. Perform necessary generated-file updates
outside active qualification captures. Retain setup failures and their corrected
attempts instead of disguising them as compiler or semantic outcomes.

Use existing qualification/claim records for a compact evidence ledger: asserted
behavior, observed source and producer identities, library/toolchain/environment,
target/engine, command, exit/diagnostic, source/oracle/artifact hashes, and the
reason to reuse or rerun. Keep raw outcomes with their actual observed revision.

### Main-change validation

Record scoped evidence on a fixed revision, with source/oracle hashes,
compiler binary identity and producer revision, toolchain, target and relevant
environment/resources. Keep frozen expected results and refusal controls intact.
Keep the initial base fixed through publication. A changed main SHA
alone does not trigger pull, rebase, rebuild, requalification or review. Only a
demonstrated actual merge conflict, concrete prerequisite or changed affected
source justifies targeted local work and reassessment against recorded inputs.

When pinned qualification is explicitly requested, or the assigned release or
trust-boundary outcome requires it, use the executable
[pinned qualification runner](qualification.md) to capture dependency identities
and measured checks. An ordinary progress PR needs its scoped evidence and
passing admission; it does not need a new qualification plan or packet. For a
qualification assignment, keep the checkpoint fixed through publication;
`assess`/`run-pending` uses that recorded base. After justified source integration,
assess only the changed closure. Preserve original observation identities when
reusing evidence. Immutable producers and complete reviewed closures remain
required within a qualification capture; file replacement during capture
invalidates it even when the validator exits successfully.

- Resolve ordinary Git text conflicts while preserving both outcomes' intent.
  Unrelated changes do not require repeating unaffected checks.
- Changes to exercised compiler semantics, libraries, runtime, source readers,
  fixtures or execution environment require the affected checks again. A compiler
  repair must be validated with a compiler built from the repaired candidate;
  an older binary cannot establish the repair's behavior.
- An unchanged sample may reuse a verified binary and its recorded checks when
  the governing inputs remain unchanged. This establishes the recorded producer's
  behavior, not the behavior of an untested newer compiler.

Inspect the complete relevant old/new delta and its consumer closure before
judging applicability. Follow true and false edges, negation and short-circuit
paths, preserving selected operator meaning. For builtin integer equality, a
false `x == k` edge excludes `k` even without a `!=` token. A spelling search
alone cannot prove that a new branch is irrelevant. Schema/version/default-field,
codec, lowering and admission changes may execute even when the advertised new
feature is absent. Compiler checking, formatting and lint inspect source
independently of runtime branch reachability.

For each check, connect the changes to its asserted behavior and state the
evidence supporting reuse or the focused rerun needed. If that argument is
uncertain or costs more than the check, rerun the affected check. Register
independent source-scope reuse through [qualification](qualification.md), retaining
the original run, producer and artifact identities. New corroborating executions
keep their own identities; do not relabel an old observation as a new execution.

Share a completed replay's artifacts and receipts across checks/reviewers when
the protocol supports that reuse; do not repeat the entire preparation/execution
pass merely to fill another manifest. Keep current producer/review bindings
separate from historical frozen products. This does not waive affected-consumer
reruns, artifact integrity or review required for the submitted change.

Record evidence against the source actually tested. A new main SHA alone
does not require author rebase, tests or review. The [admission check](landing.md)
checks rules and formatting; the author runs affected tests and Clippy before
squash-merging, and main is verified afterward. Review and rerun affected checks
when the author edits source, resolves a conflict or discovers a changed semantic premise.
Independent-workspace checks remain part of focused author validation, not a
per-main-movement qualification loop. New affected failures still require repair;
authority, custody, digest and golden rules remain. Post-merge runtime checks and
release qualification are separate from the deliberately small admission gate.
Postlanding verification observes the merged source head, published commit and
CI results; it does not rebuild the compiler or rerun local qualification.

### Independent semantic review

When the assignment requires independent review, provide the exact submitted
candidate and base, relevant accepted canonical passages and source identities,
original baseline outcomes, preserved positives/controls, the evidence ledger,
current versus historical observations, and known limitations. Review the
[requirements and evidence](../AGENTS.md#requirements-and-evidence) established
by those consumers, including the semantic premise behind a rewrite, exact
subjects/actuals, effects/lifetime, formation and authority. For every fact
witness under `samples/cli/facts/`, check the runtime counterfactual alongside
the static reject twin: if `Psi 06–08` or `Omega 00–09` inverted the branch,
took the wildcard arm, omitted the cleanup action, or miscalculated the slot
offset, would `main()` trap? A witness whose branches yield the same result on
the only tested input or whose cleanup mutates only a dying temporary fails
semantic review just as a missing `invalid/main.omg` twin does. Inspect raw evidence;
check counts, file presence and a reviewer's identity field alone prove none of
those obligations. Choose necessary independent replays from the affected scope.

Return an exact-candidate acceptance decision, concrete blocking findings with
file/line or evidence references, any needed focused checks, and material
limitations. Distinguish demonstrated defects from unresolved concerns and
documented baseline failures. Review authority does not transfer implementation
ownership: the worker resolves findings and finishes authorized publication.
An approval names the source reviewed. Do not relabel it as a review of the
published squash commit, and do not require another review solely for that
SHA. Author changes, conflict resolutions or changed semantic assumptions need
affected review; routine base movement alone does not require another author review.

## Running one test

A crate's own tests run by package and name filter:

```bash
mbx nextest run -p isa-x86_64 encode
```

Compiler behavior is tested by a Rust test on the changed code. When the test
needs an Omega program, keep that program beside the tests under
`tests/fixtures/<tier>/<group>/<name>/` and name it from
the test; see `omega-rust/omega/compilation/compiler/tests/README.md`. Keep
the few programs a test reads, never a matrix.

## Sample check gate

`tools/sample_gate.py` is the samples' cheap check.
`tools/sample_native_gate.py` builds and runs applicable samples against their
native oracles, which is the release gate; this one runs `omega --check` over
each positive `samples/**` project and validates intended-invalid source controls
against their authored diagnostic identity. The gates share the role-aware
[project discovery](../samples/README.md#project-roles-and-discovery), including
nested build roots and non-`main.omg` entries. Libraries qualify through consumers;
external submodules use their own gates. Checks run into a scratch build
directory, and groups the first diagnostic per sample by cause.

```bash
python3 tools/sample_gate.py                        # the whole tree
python3 tools/sample_gate.py --filter text,gui/     # matching paths
python3 tools/sample_gate.py --cli target/debug/omega --write-results out.json
```

It writes nothing into the working tree and needs no native toolchain, so it
is the sweep to run before reporting sample coverage. About twelve minutes
for a historical 148-project tree; current selection comes from discovery, not
that historical count.

`mbx xtask sample-gate` runs each sample's `omega update`, then builds
and runs it. Without `--cli` it first builds the opt-level 1 CLI with `mbx`
(direct Cargo fallback is forbidden) in the checkout's own Cargo target directory
(`target/`, or `CARGO_TARGET_DIR`) and measures that. Package review of a
std-using sample takes minutes with an unoptimized binary: 31 projects took
66 s with the optimized CLI against 436 s unoptimized, with identical results.
The dev profile is opt-level 1, so the gate's build and a plain
`mbx build -p omega` share every artifact, and a compiler the session already
built costs the gate under a second. A separate gate directory used to compile
the whole compiler a second time, 12 minutes cold on a 4-core cloud session.
A run never executes the shared image: it copies the built binary aside under
`build/gate-cli.lock` and runs the copy, so two gates can reuse and rebuild the
target directory at once without one holding an open image the other's build
cannot replace (witnessed 2026-10-17: the second of two concurrent gates failed
with `failed to remove file build/gate-cli/debug/omega.exe: Access is denied`).
An explicit `--cli` older
than the checkout's last compiler change draws a warning, and the run record
marks it stale. Each run is recorded under the main checkout's
`build/sample-gate-runs/`, shared by every worktree, with per-sample and
per-phase times. Programs run from a scratch directory under
their own ignored `build/`, so files they write stay out of the checkout. A
failure caused by this checkout's stale package state (an accepted lock or
review left by an interrupted run or another compiler build) is measured once
more from a clean review and reported as `retried`; a compiler defect
reproduces on that retry. A sample whose `build.omg` binds no `ProgramEntry` is a check-only
probe; the gate checks it and reports it apart from the native score.
Its failure still makes the gate fail. Intended-invalid projects never enter
package acceptance or native execution: they must exit 1 from checking and match
all authored diagnostic fragments. Missing expectations, wrong reasons, crashes
and timeouts fail; no negative coverage disappears into an unscored row. A
program with `expected_native_error.txt` must pass `--check` and then fail its
native build with every non-empty line of that file in the output, reported as
`native-rejected`; an empty file, a check failure, a native build that succeeds
or a missing line fails it. Existing
no-oracle/freestanding reporting and positive package-review behavior are unchanged.

```bash
python3 tools/sample_native_gate.py --filter cli/ --jobs 8 --timeout 900 < /dev/null
python3 tools/sample_native_gate.py --cli <existing-omega> --filter cli/games
```

For work selection, `python3 tools/sample_native_gate.py --programs-only`
omits the same probe/control trees the work queue excludes. It retains
host exclusions and missing-oracle reports. This is application discovery,
not release acceptance: the unfiltered default gate still checks every project.
Compiler advancement adds `--facts` (`--programs-only --find-work --facts`,
then `python3 tools/sample_queue.py --facts`). That record is filtered to
`samples/cli/facts` and does not replace the unfiltered application queue.
Ordinary program runs omit `--facts`.
Unexpected failures print immediately as they finish, before the aggregate
report. Only a completed run writes a queue record; an interrupted run is not
complete coverage.

`python3 tools/sample_queue.py` reads the latest unfiltered application or full run record and lists
the samples that do not yet run, smallest first with each refusal: the work
queue of [the sample loop](../AGENTS.md#the-sample-loop). After the work it
lists, apart, samples whose README cites `OWNER_QUESTIONS.md` (waiting on that
decision) and programs with no oracle (nothing defines them running). `--all`
adds probes and invalid controls and therefore requires a full gate record.

The sample gate executes a project carrying `input.txt` beside its entry with
that file as stdin (absent means an empty stream) and then scores the authored
`expected_native_exit[.<target>].txt`, `expected_stdout.txt` and
`expected_stderr.txt` files exactly where present, falling back to the
header oracle for channels without a file. Samples without `input.txt` run
with empty stdin under the header oracle alone. Every non-execution child in
both gates still gets an empty stdin. Close stdin
(`< /dev/null`, or `$null |` in PowerShell) on a direct `omega` command run in
the background: `omega run` forwards stdin and otherwise waits at zero CPU.

A sample is the product's own customer, so a cluster names a capability the
language owes rather than a fixture to repair. Read the
`provider absent on this host` cluster before treating a refusal as
breakage: a sample declaring a provider this host does not supply is
reporting the host.

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

## Stage line coverage

`cargo xtask stage-coverage` answers one question: how many lines of the
checking stages does a program make the compiler run? It measures, it does not
gate; nothing in CI calls it. Use it to compare a program against another, or
one commit against another, when deciding what a change to the checker removes.

```bash
mbx xtask stage-coverage samples/cli/facts/length/local < /dev/null
mbx xtask stage-coverage --list programs.txt --json report.json < /dev/null
```

A program is a `main.omg` or a fixture directory holding one; `--list` reads
one path per line and ignores blank lines and lines starting with `#`. The
report has one row per program and, for more than one program, a row for the
lines run by any of them:

```
| Lines | Exit | Psi 05 | validation | Psi 06 | All three |
| --- | ---: | ---: | ---: | ---: | ---: |
| Instrumented | | ... | ... | ... | ... |
| Run by `samples/cli/facts/length/local/main.omg` | 1 | ... | ... | ... | ... |
```

How to read it:

- **A line is counted by source path.** Psi 05 is
  `pipeline/05_typed-trees-to-checked-trees/`, validation is `validation/src/`
  (not its `representation-planning` subcrate) and
  Psi 06 is `pipeline/06_checked-trees-to-lowered-psi/`, all under
  `omega-rust/psi/compilation/`.
- **Instrumented** lines are the lines that carry a coverage counter in this
  build, which is fewer than the lines in the files. A line is **run** when its
  counter is not zero.
- **Exit** is the compiler's own exit code. A program that fails still gets a
  row, because its count up to the failure is real; read the exit code before
  comparing rows. A program rejected in checking never reaches Psi 06.
- **Counts are comparable only within one build.** A different commit
  instruments a different set of lines, so compare two commits by their run
  counts and quote each commit's instrumented total beside them.

The command builds `omega` with `-C instrument-coverage` at
`CARGO_PROFILE_DEV_OPT_LEVEL=1` into its own `target/stage-coverage`, so it
does not disturb the ordinary build cache. The first build is a full build of
the compiler; `--build-only` does just that step, and `--target-dir` moves it.
By default each program runs as `omega run --engine terminal <main.omg>`;
arguments after `--` replace that, for example `-- --check`. Programs run one
at a time, and each program's compiler output is kept under
`target/stage-coverage/stage-coverage-profiles/<index>/`.

It needs the `llvm-profdata` and `llvm-cov` that ship with the pinned
toolchain, because the profile format changes between LLVM releases and a
system `llvm-cov` usually cannot read it:

```bash
rustup component add llvm-tools
```


## Full baseline

Use these gates when establishing or refreshing a full checkout baseline. The
full baseline is a manual trigger: only the user starts it. Agents run the
focused checks under [validation scope](#validation-scope) instead.

```bash
python tools/check_native_boundary.py
python tools/fmt.py --check
mbx clippy --workspace --all-targets -- -D warnings
mbx check --workspace --all-targets
mbx nextest run --workspace --lib --no-fail-fast
```

The native-boundary gate walks Cargo's declared production path dependencies,
including optional, build and target-specific edges. It rejects source/compiler,
package and interpreter owners anywhere behind `native-compiler`. Its seeded
negative controls run with `python tools/tests/test_native_boundary.py -v`.
This is a check of the native facade, not a claim that all workspace topology
has converged to the two-driver contract. The same script's
`FORBIDDEN_DEPENDENCIES` include the Omega firewall: Omega compilation crates
do not depend on source-side Psi, and no Omega stage depends on another. Edges
that predate those rules sit in `FIREWALL_ALLOWANCES` with a reason each; a
removed edge's row must be deleted, so the list only shrinks. The post-merge
check runs the boundary audit on main; admission does not run it for a PR.

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

`python3 tools/doctor.py` checks all of the below in one pass: it compiles,
links, and runs a probe binary (which is what an Xcode linker older than the
active SDK breaks), verifies the pinned toolchain resolves, reports mbx and
cargo-nextest versions, confirms the claims coordination ref answers, and
checks `pull.rebase`. Exit 1 means a check failed; warnings are degradations
with the remedy printed beside them. On Linux, `bash tools/agent_setup.sh`
installs everything doctor checks for; it is also the setup script for Claude
Code cloud environments (`if [ -d Omega ]; then bash Omega/tools/agent_setup.sh; fi`).

The selector requires Git, the pinned Rust toolchain, `mbx` 1.29.0 or newer, and
`cargo-nextest` 0.9.140 or newer (`mbx` is mandatory; direct Cargo fallback is forbidden).
`mbx` is [Mr Boxington](https://mr-boxington.jdx.dev/), a shared build cache
that wraps Cargo ([source](https://github.com/jdx/mr-boxington)); installation
instructions are at <https://mr-boxington.jdx.dev/installation>.
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
| Root `AGENTS.md`, `CLAUDE.md`, `README.md`, `OWNER_QUESTIONS.md`, `TASKS.md`, `TASKS_BOOTSTRAP.md`, `TASKS_OPTIMIZER.md`; Markdown under `wiki/` | No library commands — reported under `documentation_paths` in the plan; a `jev` augment may still add checks. |
| Manifests, build scripts, lockfiles, toolchain/configuration, shared fixtures, Omega library sources, tools, and unknown inputs | All libraries. Markdown outside the allowlist, including this file, also takes this fallback. |
| No changed input | No commands; library filter `none()`. |

A change under a `PATH_CANDIDATES` prefix in `test_affected.py` also runs that
catalog candidate, reported under `path_candidates`: `tools/` runs the tools
unit tests, `tools/release/` the release-record plan, `tools/bootstrap/` and
`bootstrap/` the chain-hygiene check, and `omega-rust/` the fmt check. Changes under
`source/library/` run `std_package_check` (`omega --check
source/library/std/main.omg`, about 2.5 minutes on a debug build): a
program compiles only the std bodies it reaches, so only a whole-package
check observes a std body that fails checking.

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
`build/typesafe.env.txt` in this or — from a linked worktree — the main
checkout, or `~/.config/typesafe/typesafe.env.txt`), the plan
additionally asks a System One model which entries of
[test_select_candidates.json](test_select_candidates.json) the diff could break
*beyond what the deterministic baseline already covers*, and appends commands
for flagged candidates the baseline lacks. The augment is union-only: it can
add checks (e.g., a standard-library compile for a name-resolution change)
but never removes baseline
coverage, so every failure mode degrades to deterministic selection. An entry
the augment adds repeatedly for the same path family is promoted into
`PATH_CANDIDATES`: the augment is the discovery layer, the table is the
owner. The
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

[release_record.py](release/release_record.py) runs the
[release matrix](../wiki/drafts/reference/rust_compiler_completion.md#release-matrix):
the six `RC-*` gates with their exact contract invocations on one host, and
writes that lane's release record (`commit`, toolchain, host, commands,
results, elapsed time, expected skips, direct-execution observation). `close`
joins the four lanes' same-commit records into the release closure. It does
not substitute for the selector — `plan` reviews what would run on this host,
`run` executes. See
[recording a runner row](release/README.md#recording-a-runner-row) and
[joining the four lanes](release/README.md#joining-the-four-lanes).

## Slow builds

Use [test-cycle measurements](../wiki/drafts/measurements/test_cycle_measurements.md) to distinguish
compilation, test execution, and repeated landing validation. The Windows
measurements did not establish a stable universal test-thread cap or a full-suite
speedup from nextest alone; avoid unrelated test execution using the selector above.

If small crates each pause for seconds before parsing, inspect `target/` before
changing test or compiler architecture: a long-lived `target/debug/deps` with
hundreds of thousands of stale hashed artifacts makes rustc rescan it per
crate. `cargo clean` fixes it.

### Warm build for cloud sessions

A Claude Code cloud session starts in a fresh container with nothing built, and
`mbx build -p omega` there takes about 11 minutes (688 s measured on a 4-CPU
container, 2026-11-11). The `Warm build` workflow builds main's compiler in a
container set up like a session and publishes the build directory, about
270 MB compressed, to the GitHub Container Registry as
`ghcr.io/cathedralos/omega-warm-build:<slot>` (`primary` and `worker`), not as a
release: releases are the product that installers resolve. A session's
environment must allow `ghcr.io` and its blob host
`pkg-containers.githubusercontent.com`. The SessionStart hook in `.claude/settings.json` runs
`bash tools/warm_build.sh session-start`, which unpacks it into the primary
checkout's `target/` and into `.agents/worktree/worker-1/target` before the
session's first command. Managed Codex environments can run the same command
from their setup script; `CODEX_ENVIRONMENT_ID` enables restoration and preserves
the restored worker's base during acquisition, just as `CLAUDE_CODE_REMOTE=true`
does for Claude Code. Local Codex sessions do not opt in merely by running Codex.
The archive's Cargo paths and toolchain must still match the environment for
its compiled artifacts to be reused. Measured on the same container with the
archives CI published for commit `fd5aef2f7`: 14 s to restore both, then
`mbx build -p omega` took 30 s in the primary checkout, where it reran the
`compiler` build script and recompiled 4 crates, and 5 s in the worker slot.

That holds only while the checkout's compiler sources match the published
commit. A change to a foundation crate recompiles every workspace crate above
it: the same restored build, three commits away with `language-core` changed,
took 673 s. So the hook creates `.agents/worktree/worker-1` at the published
commit, `cargo xtask worktree acquire` hands it out there while it is untouched
(it prints how many commits behind `origin/main` that is), and the work keeps
that commit as its base. Integrate newer source only for an actual conflict or
needed prerequisite, then follow [squash-merge landing](landing.md). A slot that
has been released or moved resets to `origin/main` as before.

`mbx` does not provide this reuse in this workspace. `.cargo/config.toml` turns
on Cargo's checksum freshness, Cargo then passes
`-Zchecksum-hash-algorithm=blake3` to rustc, and mbx 1.29.2 bypasses its cache
for a flag it does not model: a full build reports `0 hits, 0 misses, 213
bypassed` and stores nothing (`mbx explain build -p <crate>` names the flag).
`mbx` still schedules builds and manages target directories.

Managed setup (`tools/agent_setup.sh`, including `--mbx-only`) keeps the shared
`.agents/worktree` pool in mbx's global `target.keep` list through
`mbx xtask managed-cache-retention`. The managed path bootstraps this Rust helper
with the repository toolchain; a command-local `MBX_GC_AUTO=false` protects that
bootstrap without changing global GC policy. It resolves the primary Git root
even when setup runs from a detached worker, appends only that pool's absolute
path and preserves existing entries and GC settings for other targets and
shared cache state. Local sessions do not change settings. An explicit
`MBX_TARGET_KEEP` remains the operator's override.

mbx 1.29.2 rejects collection settings in workspace `.mbx.toml`; its
`settings get/ls` commands read environment/global policy, not that file.
`target.keep` protects live pool targets and their unused units from size/age
collection, including low-disk sweeps. Deleted checkouts are still collectible,
and a matching `target.evict_first` entry that reaches deeper wins (ties keep).
Kept data can exceed the target budget and free-space floor, so disk pressure
needs deliberate review rather than removing a warm worker cache mid-task.
Do not move caches, change running jobs' settings, or collect peers' outputs
during validation. Use `mbx gc --dry-run --json` to inspect the selected policy;
the preview does not delete data or prove that a live compiler's locks are free.

The hook also sets `gc.min_free_size` to 5 GiB when it is unset. The
container's disk reports 252 GB while a session may use about 30 GB of it, so
the default floor described below is 25 GiB and one compiler build crosses it.

### Build cache placement

`mbx` computes its garbage-collection floor per disk, not per cache:
`gc.min_free_size` defaults to 10% of each disk clamped to [5 GiB, 50 GiB]
(`mbx settings ls` prints the default; `mbx gc --help` describes the
over-budget collection). With `cache_dir` left unset — the platform cache
directory, `~/Library/Caches/mbx` on macOS — a 460 GiB system volume therefore
holds a ~46 GiB floor, and the cache evicts whenever that volume is near it.
Measured here on 2026-10-20: the GC line read `43.7 GiB free on the disk holding
/Users/jarod/Library/Caches/mbx, under the 45.0 GiB minimum`, and it `removed 2
target directories (1.9 GiB logical, 0 abandoned and 2 live)` — two live
worktrees — after which `target/debug/deps` was empty and the `corpus_runner` a
gate had just built was gone.

The symptom is not a slow build, it is a false success: `target/debug/omega`
disappears, a missing binary prints nothing, and a shell reading a pipeline's
exit status reports success — indistinguishable from a fixture that accepted.
Never accept an empty diagnostic without `test -x target/debug/omega` first, and
`tools/swarm/binary_fresh.sh` immediately before a measurement run; report
`exit 127` as a missing instrument, never as a result. A long measurement can
also copy the binary out of the worktree, or chain the runs that must share one
runner.

The fix is two fixes, which is why it belongs in a setting rather than a habit.
`mbx settings set cache_dir <volume holding the checkout>` (measured here:
`/Volumes/Crucial/mbx-cache`). First, the floor then applies to that volume —
10% of 3.6 TB exceeds the 50 GiB ceiling against 3.2 TB free — so eviction
pressure disappears. Second, cache and target finally share a filesystem, so
restore clones or links instead of copying: `mbx doctor` reports
`<cache> -> <worktree>/target: cloning is supported`, and the action-store
budget scales with the volume (`gc.max_size` defaults to 5% of the cache disk
clamped to [5 GiB, 500 GiB] — `185.0 GiB budget` here against roughly 23 GiB
when the cache sits on the system volume). Change the setting, then verify with
`mbx doctor`.

Changing `cache_dir` strands the old target directories, because a worktree's
`target` is a symlink into `<cache_dir>/targets/v1/<hash>`. Three states then
produce the same failure — a build or gate refusing with `Caused by: Not a
directory (os error 20)` before compiling anything — and one remedy covers all
three: remove `target` (it is ignored scratch; no source is touched), let the
next `mbx` invocation recreate it under the new cache, and rebuild, because the
collected target directories took every binary with them.

- **A symlink into the old cache**, whose target area the GC then collected —
  measured here, on a worktree whose link named one of the `2 live` target
  directories the GC had just removed; `rm target` and the next `mbx` run
  recreated the link under the new cache.
- **Absent** — already removed, or never created; `mbx` creates it.
- **A plain empty directory**, which `mbx` never replaces with its link:
  measured by the discovery lane, where `cargo` failed with the same `Not a
  directory (os error 20)` until `rmdir target` and a rebuild.

Inspect each worktree's `target` with `ls -ld target`, not `readlink`: a plain
empty directory is the third state above, and `readlink` reports it as "no
symlink", which reads as fine.

Reclaiming the old cache before the links are fixed is what turned this into an
outage, because every worktree fails at once and the failure does not read like
one: a gate reports it as `corpus_gate: runner build failed (101)` and a
dependency as the same `os error 20`, both of which look like a compiler or
runner defect. Do it in order: change `cache_dir`, fix `target` in every
worktree, verify one `mbx build` in one of them, and only then reclaim the old
cache. A measurement run that started before the move measured a runner that no
longer exists, so re-run it rather than reusing its result.

Moving a cache between volumes has its own trap. macOS ships `openrsync` as
`/usr/bin/rsync`: it rejects GNU-only flags such as `--info=...` by printing
usage and copying nothing, and `rsync ... | tail -1` reports exit 0 because the
pipeline's status is the last command's. Use `ditto` or `rsync -aH`, and never
read a pipeline's exit code as the tool's.

### Fast sample work discovery

Use `python3 tools/sample_native_gate.py --programs-only --find-work` to find the
next application failure. It remeasures previous failures first, excludes live
claimed outcomes and owner prerequisites, then tries smaller applications. It
stops scheduling when a confirmed failure supplies work. `sample_queue.py` reads
that partial discovery record and labels unmeasured applications unknown.
`--facts` on that command and on `sample_queue.py` limits the same search to
`samples/cli/facts` for compiler advancement. Without `--facts`, ordinary
program discovery is unchanged.
Historical verdicts guide order only: they never become fresh acceptance.

Discovery does not replace complete application coverage or the full release
gate. Run without `--find-work` for those gates; `sample_queue.py --all` accepts
only complete full-gate records. This avoids waiting for every unrelated program
before starting one repair, while keeping release coverage explicit.
