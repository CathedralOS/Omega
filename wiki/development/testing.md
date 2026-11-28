# Local testing

Run focused checks that show the changed behavior works, then land through
[admission and a squash merge](landing.md). Admission stays light and
compiles and tests no Rust code; heavy checks run on the LAN VMs, never on a
GitHub runner ([AGENTS.md](../../AGENTS.md#heavy-checks-run-on-the-lan-vms)).

[AGENTS.md](../../AGENTS.md) holds the common workflow; this guide covers test
choice, run records and platforms.
[.github/workflows/](../../.github/workflows/) is what actually runs.
The older qualification procedures below do not bring back retired checks.

`xtask` commands that need a repository use the nearest Omega workspace that
contains the directory you run them from, even if the binary was built in
another checkout. Run them inside the checkout to inspect; outside one they
refuse instead of using the build checkout.
Explicit paths such as `fmt --repository` and `rust-producer-omission --root`
still choose their inputs. `fmt` already defaulted to the current directory.

Keep a Rust test’s inputs under its crate’s `tests/fixtures/<feature>/<tier>/<case>/`. Multiple owners can share the same input under the repository’s `tests/fixtures/`. Shared Rust harness code lives in `tests/support/`.

Bootstrap tests live in `tests/bootstrap/`, grouped by candidate. The `interpreted/` suite contains its rung and cross-rung gates. The portable [bootstrap checks](../../tests/bootstrap/conformance/README.md) and their expected outcomes can be copied alongside the selected chain. Retired gates and fixtures are available in [the archive](../../_recycling/tests/).

## Validation scope

Prove the thing you built with tests on the changed code, then a very small
area around it. If that is fine, land. If it is not, investigate that failure.
There is no fixture corpus to run and no ledger: everything that ran or
recorded the corpus under `tests/omega/` (`corpus_runner`, `cargo xtask corpus`,
`cargo xtask corpus-gate`, the corpus row measurement and the published
ledger) was removed on the owner's ruling of 2026-11-13, and the fixtures
themselves were deleted with it.
[tests/README.md](../../_recycling/tests/README.md) says how to read a deleted fixture from
history. A program a test reads is kept beside the tests, under
`tests/fixtures/`.

Iterate on focused tests. Landing shows the change works and explains
failures in what it affects; a release runs the full release matrix. An
unrelated failing base need not block a checkpoint but does not pass a release.
Add tests for behavior no test yet distinguishes, not combinations or counts.

After a round of compiler work, an agent may take a
[development-health outcome](../../TASKS.md#development-health-outcomes) to address
a slow build or missing test coverage.
Name the blocked workflow and what would be good enough, fix the bottleneck
or test path, show the result and stop. Do not quietly turn a feature task into
profiling or repeated broad runs.

Routine changes, including `advance`, run the regression tests for the change,
the affected crates' checks and the relevant integration tests. Pick checks
from the changed behavior and the code that depends on or reads it, not
because the worktree is new. Run check and Clippy per affected crate. For prose-only instructions, check consistency, links and
skill metadata; run source audits only when their input rules change. Editing
workflow prose needs no Rust build.

A bug fix needs a test that shows the bug and checks of the affected behavior.
Reuse passing results whose inputs are unchanged. Full workspace runs are
baseline, health or release work only the user starts; an unverified base is no
reason for a routine task to run one.

Explain an unexpected failure from its diagnostic and the change's dependencies
and readers, without before/after runs against a moving `main`. A failure confirmed unrelated does not block landing:
keep its command, revision and evidence, and report it without fixing it in
the same task. New or unexplained failures in affected behavior must be fixed
before landing. Do not rerun broad suites to find which change broke one test.

### Merge validation budget

Run the affected crates' unit tests and Clippy locally, plus the tests of the
same mechanism and the samples known to be affected. Reuse a verified warm
compiler build when its inputs still apply. A filter that runs no tests proves
nothing. Accepting a compiled program still needs its real Psi and native runs.
If resources block a needed run, name the blocker; do not dispatch the
retired `compiler.yml` or `compiler-regression.yml` workflows.

For each planned build or test, say what it decides, what behavior it covers
and why existing results are not enough, and estimate its time and resources,
compilation included. Reuse a result only when its revision,
toolchain, configuration, dependencies and dependents still apply; say whether
it was a cold or warm build. Keep the command, inputs and outcome.

If a run goes over budget, check progress, CPU use, logs and blockers, then
continue, narrow or change it and record why; do not blindly wait, restart or
widen the suite.
A budget is neither a timeout nor a pass. Reviewers rerun checks to fill real
gaps in the evidence, not every expensive build. Required CI and release gates
still apply.

Before a build, write a short plan in the PR or claim: the changed behavior,
the smallest checks that show it, reusable results and expected build and run
time. Pick by affected behavior, not every test added this session. The acceptance for a whole capability is not automatically the
suite for every PR that advances it.

Routine merge testing should take minutes. Rethink the scope if a planned run
exceeds about 15 minutes or another repair grows the plan. Count compilation:
seven seconds of tests can need a 25-minute build. Reuse a verified warm build,
select the affected crate and tests, or use a hosted check that covers the
behavior. A truly needed slow check can run; say which change requires it. This is not a permission step,
and a check that did not run never counts as a pass.

A routine progress PR does not need full Product compilation, self-hosting,
bootstrap reconstruction, broad runtime groups or release qualification unless
its assigned acceptance does. A compiler performance problem
found during an extra run does not attach its fix and another full test chain
to an otherwise complete PR. Record it separately; affected-behavior failures
still block, and the larger capability continues after the finished step lands.

Once the scoped checks and any required review pass, push, update the PR and
follow [admission and squash-merge landing](landing.md). Do not add suites,
rebuild an unchanged compiler or re-review scripts and manifests for comfort. A required semantic review looks at the changed meaning and the
relevant controls; it does not redo earlier reviews. Keep the revision, command
and outcome in the existing PR or claim, not a new qualification framework. Report pending or failed extended tests honestly, without calling the
whole compiler or release qualified.

The [PR #730 retrospective](../drafts/measurements/merge_validation_2026_10_31/README.md)
records the cost and failure pattern behind this rule.

### Historical pilot records

The earlier pilot workflow was removed in #2080; use the Rust qualification
below for new program runs. Old pilot records keep their original source,
compiler, profile and executor identities; judge reuse by the
[execution evidence rule](#execution-evidence-and-replay-inputs).
The [historical pilot protocol](https://github.com/CathedralOS/Omega/blob/868574432568c7bb2586952397d3e8a217ad79a0/tools/testing.md#exact-fixtures-on-a-hosted-pilot)
explains those original records, including replay plans and SDK-specific controls.

### Current Rust qualification

`mbx xtask qualify` runs a named list of at most 200 sample programs through
the ordinary CLI sample gate and keeps the evidence. Name the programs with
`--samples samples/project,...` (exact paths, not search fragments) and a new
output directory with `--output`; an existing directory is refused so runs do
not mix. Each program's expected results are checked before the compiler is
built. `--target <host>` defaults to this host and must match it; each
sample must bind that target. The command refuses a dirty source tree: it
qualifies only committed sources.

```sh
mbx xtask qualify \
  --samples samples/cli/facts/length/local \
  --output build/qualification-local
```

Run it on the matching authorized host and keep its output directory, even
on failure. Acceptance requires actual native and Psi exits at
the authored values, agreement, and any output oracle. A rejection counts only
with the sample's intended diagnostic.

The directory contains the exact request, source revision/tree and a standard Git
ZIP source archive, per-command stdout/stderr and exit receipts, parsed outcomes,
producer Cargo artifacts and fingerprints, binary digests, relevant build inputs,
and before/after source identities, with the accepted review decisions and
consumed project/dependency locks. Never commit the raw directory. A failed
summary is unfinished qualification.

Qualification records what happened. It does not approve a design, work out
every dependency, or replace review and [landing](landing.md).

### Optimized validation compilers

Gates build their executables with `mbx` from their own checkout and tie each
binary's digest to Cargo's optimized profile and matching fingerprint,
including the Rust flags in effect. They refuse optimization level 0, hidden
response-file flags, a missing fingerprint, or binary or source bytes that
change during the build. They keep the toolchain, host, jobs, flags and
profile; they never record credentials or the full environment.

Building the Rust compiler has no time limit: waiting for shared mbx capacity
is not a compiler failure. Receipts record elapsed time with a null
`timeout_ms`. Other commands keep their explicit or default limits, as do
native execution and Psi work budgets.

Every shared command runs under `bounded-process` through its trusted tooling
API, in a Unix process group or a Windows Job Object started suspended. If
Windows cannot assign or resume the job, it stops and reaps the process the
same way bounded runtime launches do; the resource-capped runtime
constructors are unchanged. On timeout, wait failure, normal exit or panic,
it kills the remaining child processes and reaps the command, leaving other
jobs alone. Receipts retain the actual command status, original stdout/stderr,
container identity and cleanup errors; a cleanup failure cannot become a pass.
`timeout_cleanup.reaped` records reaping the owned command, not a claim that
every orphaned Unix descendant was adopted by this process. The same cleanup
is recorded as `container_cleanup` for non-timeout completion.

### Execution evidence and replay inputs

Record what a command actually did: checked source, published a target, ran
natively or ran in the Psi interpreter. A compiler exit of 0 or a target flag
does not show that a program ran. A runtime acceptance records each required
engine's real exit and the expected payload, output or writeback. Where useful,
break the expected result and confirm both engines fail. A negative twin must
be refused for its intended reason; a missing import, stale setup or unrelated
refusal does not show the fact is enforced. Keep source refusal, unsupported
publication and runtime failure apart, and name hosts you could not use. Read
CLI help before combining unfamiliar modes or flags.

Accept packages before starting a frozen execution capture. Writing an
ignored `omega.lock` can change its directory's timestamps even when tracked
sources are unchanged, which rightly fails the capture's stability check. For
the sample gate, run `--accepted-inputs-only` after the normal per-project
review. It turns off package updates and stale-state cleanup and retries; the
CLI still requires the exact accepted inputs and checks every expected result.
Keep the locks it read as replay inputs. Do not loosen source-change checks or
copy an accepted lock into another checkout as if it were approval there.

A fresh replay project holds the written sources, declared resources and
expected-result files, without build directories, caches, binaries or
package-review state. Bring a lock only when that exact approved lock is an
input to the check; otherwise let the fresh project generate it. Leave old
compilers, products and locks unchanged. Update generated files outside a
running qualification capture. Keep setup failures and their fixed retries
instead of passing them off as compiler or semantic results.

Record evidence in the existing qualification or claim records, briefly: the
behavior claimed, the source and compiler identities, library, toolchain and
environment, target and engine, command, exit or diagnostic, source, oracle
and artifact hashes, and why you reused or reran it. Keep raw results with the
revision they were observed on.

### Main-change validation

Record evidence on one fixed revision, with source and oracle hashes, the
compiler binary and the revision it was built from, toolchain, target and
relevant environment and resources. Do not change frozen expected results or
refusal controls. Keep the starting base until the PR is published. A new
`main` SHA alone is no reason to pull, rebase, rebuild, requalify or review.
Only a real merge conflict, a concrete prerequisite or a change to affected
source justifies targeted work and rechecking against the recorded inputs.

When a release or trust-boundary task asks for sample qualification, run
[`mbx xtask qualify`](#current-rust-qualification). An ordinary progress PR
needs its scoped evidence and a passing admission. After a justified source
change, rerun the affected checks. Reused evidence keeps its original identities.

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

For each check, say how the changes relate to what it tests and why its
result can be reused or which focused rerun is needed. If that is uncertain or
costs more than the check, rerun it. Reused evidence keeps its original run,
compiler and artifact identities; a new confirming run keeps its own. Never
relabel an old result as a new run.

Share a finished replay's artifacts and receipts across checks and reviewers
when the protocol allows; do not redo the whole preparation and run just to
fill another manifest. Keep current compiler and review bindings apart from
old frozen products. This does not waive reruns of affected code, artifact
integrity or review the submitted change requires.

For a layer-local change, run the owning layer's fixture. If the behavior spans
a boundary, exercise its consumer or public entry point. Use an end-to-end run
when the assigned result depends on the whole program.

Record evidence against the source you tested. The [admission check](landing.md)
compiles nothing, so the author runs affected tests and Clippy before
squash-merging. Review and rerun affected checks
when the author edits source, resolves a conflict or finds a changed semantic
assumption. These checks are part of the author's focused testing, not
rerun each time `main` moves. New failures in affected code
still need fixing, and the authority, custody, digest and golden-file rules
still apply. Release qualification is separate from the deliberately small
admission check. After landing, verification looks at the merged source,
published commit and admission result; it does not rebuild the compiler or
rerun local qualification.

### Independent semantic review

When the task requires independent review, give the reviewer the exact
submitted change and base, the relevant accepted spec passages and source
identities, the original baseline results, the positives and controls kept,
the evidence record, which results are current and which are old, and known
limits. Review the
[requirements and evidence](../../AGENTS.md#requirements-and-evidence) established
by those consumers, including the semantic premise behind a rewrite, exact
subjects/actuals, effects/lifetime, formation and authority. For every fact
witness under `samples/cli/facts/`, check the runtime counterfactual alongside
the static reject twin: if `Psi 06–08` or `Omega 00–09` inverted the branch,
took the wildcard arm, omitted the cleanup action, or miscalculated the slot
offset, would `main()` trap? A witness whose branches yield the same result on
the only tested input or whose cleanup mutates only a dying temporary fails
semantic review just as a missing `invalid/main.omg` twin does. Inspect the raw evidence;
counts, file presence or a reviewer's name prove none of this. Choose needed
independent replays from the affected scope.

The reviewer returns an accept or reject decision for that exact change,
blocking findings with file/line or evidence references, any focused checks
needed, and important limits, keeping shown defects apart from open concerns
and documented baseline failures. The reviewer does not take over the work:
the worker fixes findings and publishes. An approval names the source
reviewed; do not relabel it as a review of the squash commit or demand another
review just for that SHA. Author changes, conflict resolutions or changed
semantic assumptions need review of what they affect; routine base movement
does not.

## Running one test

A crate's own tests run by package and name filter:

```bash
mbx nextest run -p isa-x86_64 encode
```

Compiler behavior is tested by a Rust test on the changed code. When the test
needs an Omega program, keep that program beside the tests under
`tests/fixtures/<feature>/<tier>/<name>/` and name it from
the test; see `source-rust/subsystems/compiler/tests/README.md`. Keep
the few programs a test reads, never a matrix.

## Sample check gate

`mbx xtask sample-gate` builds and runs applicable samples against their
native oracles. It uses the role-aware
[project discovery](../../samples/README.md#project-roles-and-discovery), including
nested build roots and non-`main.omg` entries. Libraries qualify through consumers;
external submodules use their own gates.

`mbx xtask sample-gate` first builds the opt-level 1 CLI from the invoking
checkout with `mbx` and authenticates its Cargo artifacts, fingerprints and
effective Rust flags. It rejects `--cli`: an external executable cannot supply
the required current-source build evidence. Builds use the checkout's own Cargo
target directory (`target/`, or `CARGO_TARGET_DIR`). The gate then runs each
sample's `omega packages update`, builds it and runs it. By default a sample's
inputs are its files, assets and module subdirectories, stopping at nested
projects with their own `build.omg` or `sample.json`, which are found
separately. A `source_inputs` list in the sample replaces that default. The
gate passes the same input list to preparation, checking and execution.
Package review of a sample using std takes minutes with an unoptimized binary:
31 projects took 66 s with the optimized CLI and 436 s unoptimized, with the
same results. The dev profile is opt-level 1, so compatible gate builds and a
plain `mbx build -p omega` reuse existing build outputs. The gate runs the
authenticated Cargo executable directly. Source identity must stay unchanged
during producer preparation, and the gate rechecks recorded binary digests
after the samples finish.

Each run is recorded under the invoking checkout's `build/sample-gate-runs/`,
with producer evidence under `producer/` and per-sample command receipts and
times under the sample's relative path. Worktrees keep separate run directories.
For a runnable application the gate launches the compiled executable and runs
the source through the interpreter in separate empty working directories under
its per-sample evidence directory. Each receives the same captured stdin bytes.
The gate compares their termination, stdout and stderr, and checks each
against the sample's expected result. `native-outputs/` and `psi-outputs/`
retain raw guest stdout and stderr separately from the CLI diagnostics.
Their `termination.tsv` records the observed native wait status, trap
presentation and whether the retained program permits process exit, or the Psi return, exact exit status, crash or incomplete result.
A Unix wait status exposes only the low byte. A matching byte leaves an explicit Psi process exit unverified, including exits in the range 0 through 255. For a unit return, the gate requires native wait status zero and a retained program whose instructions exclude process exit. A
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

`mbx xtask sample-gate --programs-only` omits probe and control trees.
`mbx xtask sample-gate --facts` limits the run to `samples/cli/facts`.
`--filter` selects path fragments.
The unfiltered gate still checks every discovered project.
`--jobs <n>` verifies that many samples at once (each in its own directory and
evidence folder; the default is one), and `--out <file.json>` writes one row per
sample with the gate's verdict (`pass`, `checked`, `rejected`, `no-oracle`,
`not-host-runnable` or `fail`) and, for a failure, its reason.

The sample gate executes a project carrying `input.txt` beside its entry with
that file as stdin (absent means an empty stream) and then scores the authored
`expected_native_exit[.<target>].txt`, `expected_stdout.txt` and
`expected_stderr.txt` files exactly where present, falling back to the
header oracle for channels without a file. Samples without `input.txt` run
with empty stdin under the header oracle alone. Every non-execution child in
both gates still gets an empty stdin. Close stdin
(`< /dev/null`, or `$null |` in PowerShell) on a direct `omega` command run in
the background: `omega run` forwards stdin and otherwise waits at zero CPU.

A group of failing samples names a capability the language still lacks, not
fixtures to repair. Read the
`provider absent on this host` cluster before treating a refusal as
breakage: a sample declaring a provider this host does not supply is
reporting the host.

## Bootstrap gates

Run the topology check and the Alpha-to-Beta reconstruction from the repository root.
The topology check uses the developer verifier built by `mbx build -p xtask`.

```sh
sh tools/bootstrap/check-chain-hygiene.sh
sh tests/bootstrap/interpreted/alpha-beta-edge.sh --edge
```

The [Linux cold-build harness](../../tests/bootstrap/linux_x86_64/README.md)
invokes the seed and compilers. Its sources and executable live with the tests.
The [route reports](bootstrap/README.md) record experiments across languages.
The [proof tests](../../tests/bootstrap/proofs/README.md) have their own inputs
and checks.

## Full baseline

Use these gates when establishing or refreshing a full checkout baseline. The
full baseline is a manual trigger: only the user starts it. Agents run the
focused checks under [validation scope](#validation-scope) instead.

```bash
mbx xtask check-boundary
mbx xtask fmt --check
mbx clippy --workspace --all-targets -- -D warnings
mbx check --workspace --all-targets
mbx nextest run --workspace --lib --no-fail-fast
```

`mbx xtask check-boundary` walks Cargo's production path dependencies,
including optional, build and target-specific edges. It checks the direction
of numbered-stage dependencies through shared crates, the lexer and parser
boundaries, and the source, package and interpreter dependencies reachable
from `native-compiler`. A numbered source stage may link the representations
it reads but no other source stage's crate. Stages 04, 05 and 06 each build
their product through an issuer crate, which only that stage and the
product's representation may link, as a test dependency too. Package dependency editing uses a scoped allowance
for the Build text reader. The same command checks restricted program access.
Run `mbx xtask check-boundary --ownership` to isolate package and Build
invocation routing from the full representation audit. A crate reaches another
crate only through its `Cargo.toml`, so these dependency edges are the firewall;
no per-item signature review is required. [Dependency rules](../../tools/xtask/src/boundary.rs)
· [Transitive graph checks](../../tools/xtask/src/boundary/linear_pipeline.rs)

`mbx nextest run --workspace --lib --no-fail-fast` is the platform-portable
subset: all library tests, no target-specific executable/runtime legs.
`--no-fail-fast` is mandatory; retries are disabled. Platform integration tests
are separate and must report an explicit skip when the host cannot run them.

Otherwise run the checks [validation scope](#validation-scope) calls for,
including relevant integration and bootstrap checks, and report them as
scoped, never as a verified full baseline.

## Library cross-target check

`mbx xtask check-portable-services` checks that `std` gives each portable call
the same refusals and limits on every hosted target, as
[three layers](../spec/build/platform_services.md#three-layers)
requires. It reads `std`'s source, not a build, and takes well under a second.
For each service that `std/build.omg` supplies on all five targets, the check
follows each member through the raw providers selected for that target,
including members mapped to a machine in a named file. It reports refusals,
guard limits, constant arguments and clamps that only some targets reach.

Most of those rows are not divergences: layout offsets, OS flag words and OS
limits differ by nature. Each reviewed row sits in
`tools/ratchets/portable_services.txt` with a reason (`noise`, `allowed`, or
`known` plus the `OWNER_CODE_CONCERNS.md` heading that queues it). The check
fails on a row the baseline lacks and on a baseline row that no longer
appears. A review may list exact members together, as in
`Filesystem.{read,write}`. Each member still has its own comparison row.
When the check reports a new row, read the call in both adapter families,
then fix the divergence or record the row with its reason. `--list` prints
every current row.

`--observe` reports what it would block and exits 0. Admission does not run
it.

## Requirements and commands

`mbx xtask doctor` checks all of the below in one pass: it compiles,
links, and runs a probe binary (which is what an Xcode linker older than the
active SDK breaks), verifies the pinned toolchain resolves, reports mbx and
cargo-nextest versions, confirms the claims coordination ref answers, and
checks `pull.rebase`. Exit 1 means a check failed; warnings are degradations
with the remedy printed beside them. On Linux, `bash tools/agent_setup.sh`
installs the tools for the checked-out revision. Use the [cloud baseline
command](#run-a-linux-cloud-baseline-on-current-main) when a measurement needs current main.

Builds need Git, the pinned Rust toolchain, `mbx` at the version in
`tools/mbx-version`, and
`cargo-nextest` 0.9.140 or newer (`mbx` is mandatory; direct Cargo fallback is forbidden).
`mbx` is [Mr Boxington](https://mr-boxington.jdx.dev/), a shared build cache
that wraps Cargo ([source](https://github.com/jdx/mr-boxington)); installation
instructions are at <https://mr-boxington.jdx.dev/installation>.
Check `mbx nextest --version` (or `cargo nextest --version`); nextest installation
instructions are at <https://nexte.st/docs/installation/pre-built-binaries/>.
Run commands from the repository checkout.

For an explicitly scoped manual library check, a nextest filter can select a
crate and its reverse dependencies while retaining workspace features:

```sh
mbx nextest run --workspace --lib --no-fail-fast -E 'rdeps(=x86_64-encoding)'
```

This command does not add source-reader edges or audits.
[Nextest configuration](../../.config/nextest.toml) disables retries, reserves the
whole test pool for process resource-limit cases, and schedules long package
fixtures early without a universal thread cap. Doctests remain a separate Cargo
test mode. [Temporary Windows measurements](../drafts/measurements/test_cycle_measurements.md)
explain why neither a thread cap nor nextest alone has an established full-suite
speedup.

## Slow builds

Use [test-cycle measurements](../drafts/measurements/test_cycle_measurements.md) to tell
compilation, test execution and repeated landing checks apart. They found no
reliable thread cap or nextest speedup, so avoid running unrelated tests.

By default, mbx keeps an edited crate and its dependents in the editing
worktree's private cache. A second worktree can spend hours rebuilding the
same compiler. When several worktrees need the same revision, run
`MBX_LEARNED_INCREMENTAL=0 mbx build --locked -p omega` in the first one.
This lets mbx share newly compiled crates with the other worktrees when their
sources and build options match. The setting applies to one command.

If small crates each pause for seconds before parsing, inspect `target/` before
changing test or compiler architecture: a long-lived `target/debug/deps` with
hundreds of thousands of stale hashed artifacts makes rustc rescan it per
crate. `cargo clean` fixes it.

### Run a Linux cloud baseline on current main

From the Omega checkout, prefix the measurement command with the launcher.

```sh
bash tools/cloud_baseline.sh mbx build --locked -p omega
```

The launcher fetches main from `origin` (or the public Omega repository when
the snapshot has no origin), then runs the command in a separate checkout of
that exact commit. It prints the full commit, effective mbx version, host and
checkout path to stderr before the command starts. With no arguments, it prints
a shell command for entering the prepared checkout with the qualified mbx on PATH.

For a saved cloud image that predates the launcher, add the following to the
environment setup script, running from the Omega checkout. It installs the
launcher from main even when the image has an old checkout and an old mbx.

```bash
set -e
remote=origin
git remote get-url origin >/dev/null 2>&1 || remote=https://github.com/CathedralOS/Omega.git
git fetch --no-tags --no-write-fetch-head "$remote" +refs/heads/main:refs/omega/cloud-baseline/main
launcher=$(mktemp)
git show refs/omega/cloud-baseline/main:tools/cloud_baseline.sh > "$launcher"
install -Dm755 "$launcher" "$HOME/.local/bin/omega-baseline"
rm -f "$launcher"
```

Then run `~/.local/bin/omega-baseline mbx build --locked -p omega` from the
snapshot. The installed launcher fetches main on every invocation.

Repeated runs at the same commit reuse the checkout. The installer reuses
its verified binary when the build inputs match. Persist
`${XDG_CACHE_HOME:-$HOME/.cache}/omega-mbx` (or the directory named by
`OMEGA_MBX_INSTALL_CACHE`) across cloud sessions to reuse it on a fresh image.
A cold cache builds mbx once. A newer main gets a new sibling checkout, so an
ongoing measurement keeps its source and tool. When finished with a revision,
use `git worktree remove <printed-checkout-path>` to reclaim its space.

### Warm build for cloud sessions

A Claude Code cloud session starts in a fresh container with nothing built, and
`mbx build -p omega` there takes about 11 minutes (688 s measured on a 4-CPU
container, 2026-11-11). The omega-linux VM builds main's compiler in a
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
The archive includes the generated files that mbx stores outside `target/`.
Cargo records their absolute paths, so reuse requires the same checkout path,
mbx cache path and Rust toolchain. Restore keeps any existing generated files
in the cache. Measured on the same container with the
archives published for commit `fd5aef2f7`: 14 s to restore both, then
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

Nothing publishes the build automatically. The coordinator, or an agent the
coordinator asks, publishes it on `omega-linux` when cloud sessions start too
many commits behind `main`, for example after a wave of merges. From any
checkout on that VM:

```sh
GH_TOKEN="$(gh auth token)" ~/omega-agent/bin/vm-heavy warm-build \
  bash tools/warm_build.sh publish-from-vm [commit]
```

It builds main's tip, or the named commit, for both slots, each in a fresh
`ubuntu:24.04` container as root at `/workspace/Omega`, then packs and pushes
each tag. The VM's `gh` login can write packages, and the token reaches the
container only through the command's environment. On 2026-11-23 it published
commit `4854044f9603` in about 10 minutes, each slot's compiler build taking
about 2 minutes. A fresh session container then restored both slots in 11 s
and 19 s, and its first `mbx build -p omega` in each compiled nothing.

### Checksum-capable mbx setup

Cargo content fingerprints remain enabled in `.cargo/config.toml`. Cargo passes
`-Zchecksum-hash-algorithm=blake3` to rustc so it can detect changed bytes even
when a file's size and modification time were restored. Official mbx 1.29.2
and the inspected 1.30.0 adapter do not model that flag; they schedule builds
but bypass the shared Rust cache. A newer version number alone does not prove
support for this workflow.

`tools/mbx-version` pins the currently qualified adapter build. Both
`tools/agent_setup.sh` and the warm-build container use `tools/xtask/bootstrap-mbx.sh`: it verifies the
official bootstrap release's SHA-256, checks out upstream commit
`7fa83b28d239bd39d1079127ffd2f8b0f92df00a`, and applies the immutable repository
patch `tools/xtask/mbx-checksum.patch`. That patch retains the flag in command
replay, action keys and predictions. Cached dep-info keeps its raw checksum
comments. Before constructing a checksum action, the driver checks the entire
build-script watch signature and requires a reliable change token for every
file input. Filesystems with weak identities, including the modeled Linux NFS
case, run rustc without shared caching rather than treating an inode as proof
that its contents are unchanged. Other compiler flags retain existing policy.
A second patch, `tools/xtask/mbx-host-ceiling.patch`, applied after it, holds
every process an mbx command starts under the host CPU ceiling that the mbx
scheduler budget sets ([host build budget](build_slot.md#the-host-cpu-ceiling)).

The first install builds the tool with the selected Rust toolchain, optimized
at level 3, without LTO, with 16 codegen units, unwind and stripped symbols.
The installer caches a checksummed binary under
`${XDG_CACHE_HOME:-$HOME/.cache}/omega-mbx/artifacts/`. Its key covers upstream
source, the captured patch, installer, profile, compiler identity and Rust flags.
Subsequent checkouts reuse that artifact. The generated source/build workspace
is separate from Omega targets and keeps installers from mutating it
concurrently; only a live installer uses its lock. On failure the installer removes its own lock and temporary
files. A stale reservation after an interrupted process must be investigated
before removing it. Source changes during installation abort publication or
installation instead of recording mismatched inputs.

The warm-build publication keeps the small completed artifacts between runs,
not the whole tool build workspace, under the installer's recipe identity.
The unmodified official tool bootstraps the patched tool with content
fingerprints enabled and collection disabled for that command. The known
`build_script_execution = false` policy also applies to the tool bootstrap;
no global settings change. Setup uses the normal host Rust/C linker tools;
Windows uses Git for Windows' Bash and Rust's bundled LLD. No cloud environment
configuration or publication step is part of repository setup.

To install only mbx, use `bash tools/agent_setup.sh --mbx-only`. The common
installer also accepts `--install-dir DIRECTORY` for an isolated candidate.
`mbx xtask doctor` recognizes the qualified patch identity and warns when an
otherwise sufficiently recent binary has not been qualified for checksum
caching; it does not infer support or lack of support from a newer number.

The focused host check is:

```sh
bash tools/xtask/check-mbx-checksum.sh /absolute/path/to/mbx NEW_OUTPUT_DIRECTORY
```

It primes an isolated store, removes only its probe's products to require a
real mbx cache hit, then changes an included constant from 1 to 2 while retaining
the restored products and dep-info and restoring the exact source size and
nanosecond mtime. Cargo must invoke rustc using checksum freshness, and the
resulting program must print 2. Its logs preserve every stage. This is a host
check, not qualification of untested operating systems or filesystems. Do not
disable content fingerprints or strip checksum flags to obtain cache hits.

Windows MSVC host builds select Rust's bundled LLD linker through
`.cargo/config.toml`. Microsoft's `link.exe` refused an existing 266-character
managed dependency path with LNK1104 while opening the same file through its
209-character checkout alias; LLD linked and ran a focused probe using that
exact long input path. This avoids shortening crate names, relocating caches or
installing a separate linker: the configuration names `rust-lld`, the copy every
rustup toolchain ships, because the flavor alone makes rustc look for a program
named `lld` on `PATH`. The MSVC ABI and SDK/runtime libraries are still
used. The setting changes Rust host builds, not Omega's native code emission.

When build outputs exceed the disk budget, mbx collects idle worker targets
before assigned ones. The repository's `.mbx.toml` names the Git worktree's
`topic` file. Acquiring a worker writes its topic and releasing the worker
removes it. Each collector reads the current assignment, including a collector
started from another checkout after a build has finished.

An assigned target can still be collected when idle targets do not free enough
space. Explicit `target.keep` and `target.evict_first` settings take precedence.
The existing age limit, running-command leases and Cargo locks still apply.
Inspect a collection with `mbx gc --dry-run --json`.

Older managed setups added the shared worker pool to the global `target.keep`
list. The installer preserves those entries because the same list holds
operator choices. To enable assignment-based ordering for a previously pinned
pool, remove its path from `target.keep` while preserving the other entries.

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
`tools/benchmark/binary_fresh.sh` immediately before a measurement run; report
`exit 127` as a missing instrument, never as a result. A long measurement can
also copy the binary out of the worktree, or chain the runs that must share one
runner.

One setting fixes two problems:
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

A local worktree assignment reserves the checkout, while mbx manages its
build storage under the workstation's disk budget. When mbx collects a target
between commands, the worktree and its source are still usable. From that
worktree root, run the build with mbx's default target placement, for example
`CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega < /dev/null`. mbx 1.29.2
recreates its collected view under the current target root. Setting
`CARGO_TARGET_DIR=$PWD/target` or passing `--target-dir target` selects an
explicit path even when it names the default location. mbx then leaves the
dangling link in place, and Cargo reports `Not a directory (os error 20)`.
If the shell sets an explicit target for other work, omit it just for this
default-placement build. [Target placement in mbx 1.29.2](https://github.com/jdx/mr-boxington/blob/v1.29.2/crates/mbx/src/target.rs#L813-L838).

Changing `cache_dir` also changes the default managed target root. If the old
view and its ownership record have already been collected, mbx cannot identify
a dangling link into the old root as its own. Inspect `ls -ld target` first.
On Unix, `rm target` unlinks a verified dangling link to your old mbx cache.
Do this from the owning worktree between commands, then rerun the
default-placement build. A real, empty `target/` directory can be removed
with `rmdir target` to opt into managed placement. Keep live links, populated
directories and operator-selected targets in place.
[Managed link recognition in mbx 1.29.2](https://github.com/jdx/mr-boxington/blob/v1.29.2/crates/mbx/src/target.rs#L1298-L1318).

When planning a cache move, verify a build using the new root before
reclaiming the old cache. Repeat any measurement whose executable disappeared
before it ran.

Moving a cache between volumes has its own trap. macOS ships `openrsync` as
`/usr/bin/rsync`: it rejects GNU-only flags such as `--info=...` by printing
usage and copying nothing, and `rsync ... | tail -1` reports exit 0 because the
pipeline's status is the last command's. Use `ditto` or `rsync -aH`, and never
read a pipeline's exit code as the tool's.
