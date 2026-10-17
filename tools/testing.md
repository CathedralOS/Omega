# Local testing

The canonical contract for validation policy, baseline gates, corpus filters,
test selection, and platform requirements. [AGENTS.md](../AGENTS.md) keeps only
the everyday command surface and links here; the sections below were promoted
from it. [baseline_gate.py](baseline_gate.py) runs the full-baseline gates as
one command.

## Validation scope

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
successful results when their inputs are unchanged. Full workspace/corpus runs
are for explicit baseline, health, or release work, or changes whose impact
cannot reasonably be bounded. Explain that scope before starting a full run.
An unverified base alone does not force a routine task to establish a full baseline.

Attribute unexpected failures with a focused baseline comparison or dependency
and source-reader evidence. For corpus movement the comparison is
`python3 tools/corpus_gate.py --compare <parent> --filter <groups the change can
move>`; a diff against the checked-in golden mixes this change with other
lanes' drift and cannot attribute either. Confirmed unrelated failures do not block landing a
scoped change: retain the command, revision, and evidence, and report them without
repairing them in the same task. New or unexplained failures in affected behavior
must be resolved before landing. Do not repeat broad suites to attribute one test.

### Main-change validation

Qualify the outcome on a fixed recorded revision, with source/oracle hashes,
compiler binary identity and producer revision, toolchain, target and relevant
environment/resources. Keep frozen expected results and refusal controls intact.
After main advances, compare the incoming changes against those recorded inputs:

Use the executable [pinned qualification runner](qualification.md) to capture
reviewed dependency identities and measured checks on that revision. Keep the
checkpoint fixed during qualification; integrate main near publication, outside
the queue, then `assess`/`run-pending` the exact incoming changes. Its worktree
registration binds the final assessment to ordinary landing verification, while
the original observation SHA remains visible when evidence is reused. Immutable
producer artifacts and complete reviewed closures are required; file replacement
during capture invalidates evidence even when the validator exits successfully.

- Resolve ordinary Git text conflicts while preserving both outcomes' intent.
  Unrelated changes do not require repeating unaffected checks.
- Changes to exercised compiler semantics, libraries, runtime, source readers,
  fixtures or execution environment require the affected checks again. A compiler
  repair must be validated with a compiler built from the repaired candidate;
  an older binary cannot establish the repair's behavior.
- An unchanged sample may reuse a verified binary and its recorded checks when
  the governing inputs remain unchanged. This establishes the recorded producer's
  behavior, not the behavior of an untested newer compiler.

Record why evidence remains applicable to the integrated candidate; a new SHA
alone is not a full-baseline requirement. Independent review required by the
assignment covers the final integrated tree. After every rebase, obtain the
normal exact-SHA, workspace-aware [landing receipt](landing.md#verify-the-build-before-publishing),
even when behavioral evidence is reusable. New or unexplained affected failures
still block landing; required authority, custody, digest and golden gates remain.


## Running one test

A crate's own tests run by package and name filter:

```bash
mbx nextest run -p isa-x86_64 encode
```

Language behavior is tested end to end: a fixture under
`tests/omega/{pass,fail,run}` is the test, and the corpus gate below runs it.
Add a fixture, not a Rust test. A Rust test belongs only to a unit with no
Omega source input — an encoder, a table, a graph algorithm, the proof
kernel — and never pins a compiler-internal custody, replay or forgery
check. The front-end, checking, lowering, Terminal, optimizer, realization
and package crates carry no behavior tests; do not add them back. A test that
scans a crate's own sources as an architecture guard is the one exception. The `compiler`
crate's only test target is `corpus_runner`; see
`omega-rust/omega/compilation/driver/tests/README.md`.

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

`--runner PATH` may reuse an immutable compiler built in another checkout.
The gate supplies `OMEGA_CORPUS_ROOT` so fixture sources, oracles and discovery
belong to the measured checkout; the runner's compiled-in path is only the
default for direct invocation. The SDK follows `OMEGA_LIBRARY_ROOT`, preserving
an explicit library override. Prebuilt runners must report their effective
fixture root through `fixture-root-v1`; missing or mismatched reports refuse
recording. Historical `--compare` runners freshly built in their measured
checkout remain supported. Qualification still binds the compiler artifact and
immutable source/oracle bytes.

`--filter` matches comma-separated `tier/group/name` fragments (also
`OMEGA_CORPUS_FIXTURE_FILTER`); `--shard k/N` selects a
deterministic hash slice for splitting the run across sessions.
`OMEGA_CORPUS_JOBS` overrides the worker count, which defaults to the CPU
count -- cap it when several sessions measure at once, since N concurrent runs
otherwise request N times the machine.

Golden history distance is optional reporting metadata. Each ancestry lookup
has a two-second budget; unavailable or slow history is reported as an
unavailable distance. This does not change fixture results, expected refusals,
oracle checks, corpus comparisons or their verdicts.

For retained direct-native artifacts, the corpus explicitly audits the
target-operation input before publication. Instruction construction consumes
those rows directly;
its independent source/target and reference-custody checks run here, not as a
second compiler phase in the default compile path.

A run outlasts a shell, so wait on **what the gate prints**, not on whether a
process exists:

```bash
until grep -qE "corpus_gate: (clean|[0-9]+ structural)" "$LOG"; do sleep 30; done
```

Do not wait with `until ! pgrep -f "corpus_gate.py"`. `pgrep -f` matches full
command lines, and the waiting shell's own command line contains the pattern as
the `pgrep` argument, so the condition is never false and the loop never exits.
`pgrep` excludes itself but not its parent shell. Sessions have lost hours to
this, and the symptom is several identical waiters stacked on one target because
each earlier one never returned. If you must match a process, bracket the
pattern so it cannot match itself (`pgrep -f "[c]orpus_gate.py"`), or capture
the PID and poll `kill -0`.

Pipe a long run through `tee`, never `tail`: `tail` buffers until EOF, so a
30-minute run shows nothing until it ends.

Exit codes are the gate's verdict: 0 clean, 1 moved (structural or perf
moves, no regression), 2 usage error, 3 regressed (a fixture verdict of
`regressed`, or a failed authored exit, `expected_stdout.txt` or
`expected_stderr.txt` oracle — including the
`--record` refusal), 4 infrastructure failure (runner build failed, no
binary, runner exited nonzero, unreadable records). `--bisect` remaps for
`git bisect run`: regressed -> 1 (bad), infrastructure -> 125 (skip),
anything else -> 0 (good), usage -> 255 (abort). A diagnostics-only move
on a fixture whose status did not change is `moved`, not `regressed`, so
`--bisect` reports it good; bisect that class with a probe that greps the
runner's record for the new diagnostic text. Before bisecting, `python
tools/jev_attribute.py --signature "<failing test or diagnostic>" --window
BASE..HEAD --with-diff 60 --paths <implicated files>` ranks the window's
commits by whose added lines literally show the failure's evidence (SUSPECT
rows first; exit 2 when no key is configured, silent under
`OMEGA_JEV_OFFLINE=1`). `--paths` restricts each excerpt to the implicated
files so a large commit's deciding hunk does not fall outside the
`--with-diff` cap; a window of thousands of commits must first be narrowed
with a path filter — `--window "$(git log --first-parent --format=%h
BASE..HEAD -- <owning directory> | paste -sd,)"` — and the tool batches what
remains under the API's payload limit. The question is literal: a commit
that enables a diagnostic without adding its text ranks `absent`, as the
2026-10-08 dogfood in the typesafe-experiments record shows, so an
all-`absent` ranking means bisect, not "outside the window". Describe the
observation only in the signature; naming a suspected file or commit steers
the answer toward it. `--json PATH` writes the
full per-fixture verdict document on every outcome, including
infrastructure failure and record refusal. Diagnostics compare against the
golden after prefix normalization (`corpus_records.DIAGNOSTIC_PREFIX_ALIASES`),
so renaming a diagnostic's wording is not a move; golden rows stamped
`@<revision>` report how many commits behind HEAD each measurement is.

### One golden, every host

`tests/omega/corpus_outcomes.txt` is a single checked-in file shared by
Linux, macOS and Windows developers, so a row whose recorded verdict depends
on the recording host can never be clean everywhere -- and, worse, is clean
on the host that recorded it, which makes it invisible. Reading a
Linux-recorded row beside a Darwin-recorded one manufactures contradictions
reproducible on neither host.

The check leg's realization of entry-binding fixtures is where a
target-scoped verdict enters an otherwise target-neutral row, so the runner
takes its realization target from the fixture's own `build.omg` -- the first
of `macos_arm64`, `linux_x86_64`, `windows_x86_64` that the fixture binds,
else its first declared target -- and never from the recording host. The row
names the result as a `check-target:<target>` fact, so a realized row says
which target it is a verdict about and a reintroduced host dependence shows
up as a per-row diff rather than a silent false green. A row with no
`check-target:` fact realized nothing and is target-neutral.

Three rules follow. A target-scoped verdict belongs in that target's own
golden, `tests/omega/corpus_native_<target>.txt`, not here: when the
`linux_x86_64` filesystem provider table makes `pass/filesystem/native_close`
build on Linux and refuse on macOS, the check row records the refusal for the
target it names while `corpus_native_linux_x86_64.txt` records `built` and
`corpus_native_macos_arm64.txt` records `rejected`. Do not pin a fixture
whose diagnostic names the realized target, and watch for the indirect form:
a fixture that reaches a provider only some targets supply. And do not teach
the check leg to prefer the recording host the way the native leg does --
`pass_realization_target` prefers the host because its records go to that
host's own golden; `check_leg_realization_target` must not, and the two
choosers stay separate for that reason.

### Attributing a move

The printed summary cuts each field at 300 columns, which is usually exactly
where the diagnostic that moved is. `--write-moves` gives you every moved
fixture's before and after records as JSON, untruncated, in the same pass:

```bash
python3 tools/corpus_gate.py --write-moves build/moves.json
python3 -c "import json; [print(r['fixture'], r['after']['diagnostics']) \
    for r in json.load(open('build/moves.json'))]"
```

Then rerun ONLY the moved names against the base you suspect
(`--filter <comma list>` from a worktree detached at that commit). A row that
moves there too is not yours. Attribute every row before re-pinning any of
them: a bulk `--record` pins whatever another lane broke as expected. Adding
`--jev` to the same diffing invocation asks the advisory `record_safe`
verdict, which names suspect rows through the causal "could this commit
produce this diff" question — advisory only; it does not replace
attributing each row. `python tools/corpus_pin_audit.py` lists fixtures
whose expected fragment matches many other fixtures' diagnostics (a pin
that cannot tell its rule from any rejection) and fixtures whose extra
diagnostics trip a sibling's pinned rule; tighten those pins before
relying on `expected` at --record time. Every gate run also prints how
many golden rows are pinned in a status their tier contradicts (a pass
fixture recorded `rejected`). `python3 tools/corpus_gate.py
--list-pinned-regressions` reads the selected golden and names those rows without
building a runner, executing fixtures or rewriting results. It accepts `--golden`
and the native/interpreter target-selection flags; measurement options such as
`--record`, `--filter`, `--shard`, `--compare` and `--json` are rejected. Exit 0
means the stored roster was read, even when it contains regressions; missing or
malformed goldens exit 4. This query makes no claim about the current compiler.
That count is the honest size of the pinned backlog whether or not the
rows carry `regressed-from:`.

Two measurement hazards, both of which have produced a wrong mechanism here.
The compiler caches resolved package snapshots under its user cache
directory, so a probe COUNTING how many times a front-end pass runs answers
about the cache, not the program — probe what a later stage observes, and
alternate the two versions rather than running each once. And `--record`
restamps `millis` on every row it touches, so after a filtered re-record,
restore the previous timing on any row whose status, expected-fragment
verdict, facts and diagnostics are unchanged; otherwise a four-row movement
lands as a forty-row diff in a file every lane edits.

**A full run can exceed the harness background cap, and the way through is
slices, not detaching.** At six concurrent sessions on a 10-cpu host a full
`--record` took 19 minutes of runner build plus over 100 minutes of
fixtures and did not finish inside a 2-hour cap; `nohup`/`disown` is not a way
around it, and killing a gate's parent leaves its `corpus_runner` executing
against a dead pipe. Compare in several `--filter <group>/` slices instead:
`--compare` caches the base commit's rows per fixture, so each slice adds to the
same cache and compares only the fixtures that ran. Use the trailing slash on each group -- a bare substring
like `time` matches 510 fixtures, some already red, and their unmet exit
promises abort the run before the golden diff.

To measure what your own change or series moved, compare against the commit it
builds on. Every agent and host uses the same command:

```bash
python3 tools/corpus_gate.py --compare origin/main --filter calls/,host/
python3 tools/corpus_gate.py --compare origin/main --native --filter calls/
```

Name the fixtures the change can move: `--compare` refuses to run without
`--filter` or `--shard`, because both sides of an unfiltered comparison run the
whole corpus, about two hours on a Windows host.

The base is built from its commit in a separate worktree (`build/cmp`) with its
own build directory, so neither uncommitted edits nor artifacts from this tree
can leak into it. A run exits 1 when any fixture loses its outcome, its
expected fragments or an execution fact such as `exit:70`, and prints every
loss and gain. Name the base explicitly: comparing against a newer `main` than
your series sits on attributes other lanes' movement to you. Rebase first, then
compare against the commit you rebased onto. Edits made to this tree while its
own runner links still change what the head side measured, so finish edits
before starting a run.

**Build the runner from the tree you are measuring.** `--runner <binary>`
reuses a prebuilt runner, which is a real saving and a real hazard: a stale
binary records the *previous* compiler's verdicts against the current tree, and
the `@<sha>` stamp names the tree rather than the binary, so the rows look
freshly measured and attribute verdicts to a commit that cannot produce them.
One re-record pinned fifteen rows to a diagnostic removed eleven commits
earlier, and it was caught only because the text contradicted a landed change.
Reuse a runner only within one tree state.

`--compare` builds its base with its own `CARGO_TARGET_DIR`; keep any other
baseline and candidate builds in separate target directories the same way. A successful Cargo invocation against a shared target
directory can retain another worktree's artifacts when timestamp freshness sees
the candidate sources as older than those artifacts. Rebuilding the runner alone
does not establish that its linked compiler dependencies came from the candidate.
An isolated target directory avoids that ambiguity without deleting peer caches.
On the pinned nightly, content freshness needs both
`CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true` (or `cargo -Z checksum-freshness`)
and `CARGO_BUILD_FINGERPRINT=content`; enabling the unstable feature alone keeps
timestamp freshness. See [Cargo's checksum freshness contract](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#checksum-freshness).

The gate defaults `OMEGA_LIBRARY_ROOT` to the measured checkout's
`source/library`, preserving an explicit caller override. Shared Cargo caches
can retain a different worktree's development library path; a fresh runner can
otherwise read that old library and falsely reject exact bundled entry contracts.
Direct runner or CLI invocations should select the matching library explicitly
when sharing build artifacts across checkouts. Release library validation still
checks the selected content; this does not waive compiler or package identity.

`--compare` caches the base under ignored
`build/corpus_baselines/<sha>[-leg].txt`. Use it when the checked-in golden
disagrees with your checkout over another lane's in-flight movement, which is
the usual case: a run against the shared golden cannot separate that drift from
yours, and re-recording the shared golden would pin their regression as
expected. A subset diff compares only the fixtures that ran; a subset
`--record` splices the rows it measured into the golden's existing text and
leaves every other row, comment and context line byte-for-byte, so sessions
recording different fixtures rebase without golden conflicts. The unfiltered
corpus is scheduled-workload cost,
not loop cost — keep it out of routine iteration.

The default run checks fixtures only. `--native` realizes each pass and run
fixture that binds a program-entry root in its own `build.omg`, executes the
run tier, `*_exit` fixtures and fixtures with `expected_native_exit.txt` in a
temporary directory (stdin from `input.txt`, stdout/stderr compared independently
with `expected_stdout.txt`/`expected_stderr.txt`), and diffs against
that host's golden `tests/omega/corpus_native_<target>.txt`; records gain
`exit:<code>` and the authored `stdout:match|differs|unavailable` or
`stderr:match|differs|unavailable` facts. Use it with `--filter` for
backend-visible changes.

A build-only pass fixture can separately require native publication to refuse
with `expected_native_rejection.txt`: each nonempty line is a required diagnostic
fragment, and the observed status must be `rejected`. This oracle is independent
of Source acceptance. It cannot coexist with native exit or stream oracles,
the run tier or the `*_exit` convention. A matching refusal records
`native-rejection:match`; it is an accepted publication test, never a built or
executed program. Wrong diagnostics, another status or execution facts make
`--record` refuse the row. Use this only for an authored publication refusal,
not for missing implementation in a program intended to run.

`expected_native_exit.txt` holds one decimal process exit code or `trap`.
The latter requires SIGTRAP for ARM64, SIGILL for Unix x86-64, or
STATUS_ILLEGAL_INSTRUCTION for Windows x86-64. Missing execution, timeout,
segmentation faults and ordinary exits cannot satisfy a trap expectation.
An `expected_stdout.txt` or `expected_stderr.txt` independently requires completed
native execution with the corresponding `stdout:match` or `stderr:match`, even
when no exit oracle is authored. Both pipes drain concurrently and comparisons
preserve arbitrary bytes, normalizing only CRLF to LF. Wrong or missing
output and incomplete execution cannot be recorded as the expected result.
Duplicate execution-fact keys are malformed runner evidence and fail as
infrastructure errors rather than overwriting an earlier observation.
These native oracles are enforced before golden comparison or `--record`, so
re-recording cannot bless a wrong runtime result. Bind the host in `build.omg`
when providing an oracle. Without an oracle, the existing observation-only
golden behavior is unchanged. `--both` also attempts the admitted Terminal
program/instance core, sharing the native report's exact artifact/profile for
identical targets without replaying Build. `interpreter:trapped` requires the
core's typed Trap outcome; decline, exhaustion and host signals cannot satisfy
the interpreted trap oracle. Missing entry/import provisions remain explicit
runtime-driver dependencies, not synthetic exits.
When `--interpret` or `--both` is requested, each authored stream oracle
also requires completed interpreter execution and its own
`interpreter-stdout:match` or `interpreter-stderr:match`.
Matching exit codes cannot excuse missing or different output, and `--record`
refuses that result before changing the golden.

The interpreted and combined defaults name `corpus_interpreted.txt` and
`corpus_both_<target>.txt`; an absent golden is a usage error. The repository's
bounded migration replay explicitly compares with `corpus_outcomes.txt`, leaves
new runtime facts visible as moves, and uses the exact reviewed
`tests/omega/interpreter_migration.json` designation. Unimplemented interpreter
debt stays nonzero and never exempts a wrong native result. The bounded replay
command is in [the corpus tests guide](../omega-rust/omega/compilation/driver/tests/README.md#reviewed-interpreter-migration-debt).

`--checked-interpreter` explicitly selects the independent checked-tree source
oracle instead of Terminal. It compiles the same reviewed fixture inputs and
executes the exact selected entry with its provisioned receiver; it never serves
as a fallback for a Terminal refusal. Records include
`interpreter-engine:checked-tree`, and the separate default goldens are
`corpus_checked_interpreted.txt` and `corpus_checked_both_<target>.txt` when
combined with `--both`. An absent golden still requires an explicit `--golden`.
The [corpus tests guide](../omega-rust/omega/compilation/driver/tests/README.md)
documents the bounded source/Trap replay. Checked-engine declines cannot satisfy
the same unchanged authored exit or stdout promises.

It realizes for the target the fixture declares, preferring the host when the
fixture binds it so the product can also run; a fixture binding only foreign
targets is still realized, because target lowering and emission are most of
what the leg measures. A fixture that binds no root is not realized at all
and stays on the check route, recording `checked`. Both rules matter for
reading a result: before them the leg built everything for the host, so a
fixture with no entry answered "native-artifact production requires one exact
selected program entry" and one binding only `windows_x86_64` answered
"selected target `<host>` has no bound required root slot" -- 13 of 28
rejections in one sample were the harness choosing for the fixture rather
than the fixture's own behavior.

```bash
python3 tools/corpus_gate.py --native --filter providers/
```

### Recording a regression without pinning it

A status column says what the compiler did today; it cannot also say what the
fixture used to do. So a native row that stops building reads exactly like one
that never built on this host, and until this was separated `--record` could
not write the loss down at all: an unmet authored oracle refused the whole
write, which is right about blessing and wrong about silence. Eight macOS rows
went on claiming `built`, six of them with an authored exit, for fixtures whose
check leg had started refusing.

Two different facts about the program now get two different answers.

- The fixture produced no result at all -- it rejects, is incomplete, is
  sanctioned `unimplemented:` debt or timed out. Nothing contradicts the
  oracle, and today's row cannot be read as the promise being met, so
  `--record` writes the observation plus a
  `regressed-from:<status>[+fact]*` token naming the last success the row
  recorded and that success's execution facts. The run still prints the unmet
  promise and still exits `regressed` (3), with the verdict document reading
  `recorded_regression` and counting `lost_successes`.
- The fixture ran and contradicted the author -- a different exit, different
  output, a trap that did not fire. That row is success-shaped: every reader
  that counts successes counts it, so `--record` refuses it outright, names
  the fixture, and leaves the golden alone.

The token is provenance the gate derives when it replaces a row, never a
runner observation. It never joins `facts`, so nothing counting executions
sees it and the row diff ignores it. It maintains itself in both directions: a
row that keeps failing carries the *original* success forward rather than
re-pointing at yesterday's refusal, and a fixture that builds again drops the
token, which is what makes the repair a visible move. A row that merely drops
to a weaker success (`built` to `checked`) carries no token, because its own
status column already says that.

Read a golden with `corpus_scoreboard.py`, whose lead line now carries a
`regressed` count and which names each row and the outcome it lost. That
count, unlike `rejected`, distinguishes a standing loss this host remembers
from a fixture that has never run here, from one file and without a `--base`
diff.

Comparing the per-host native golden against `corpus_outcomes.txt` is the
cheap detector for the rows this is about, and costs one file read: a fixture
the native golden calls `built` while the check golden refuses is a native
verdict outliving a check-leg regression. Compare against every non-success
check status, not `rejected` alone -- one of the eight was `incomplete`. A
detector reading diagnostics cannot find these at all, because a false green
carries no diagnostic.

```bash
python3 tools/corpus_scoreboard.py --cross-golden   # exits 1 on a stale success
```

`python3 tools/corpus_scoreboard.py` reports the golden as a scoreboard —
`macos_arm64 native: 64 authored / 172 executed / 746 fixtures` — where
"authored" means executed with a matching `expected_native_exit.txt`
oracle. `--base <rev>` diffs two goldens and lists `lost` authored runs as
regressions; wave reports lead with this number, not commit counts. A
`regressed` count in the lead line is the standing loss the golden itself
remembers -- see
[recording a regression](#recording-a-regression-without-pinning-it).

The scoreboard reads the golden, which moves only for fixtures a lander ran;
routine validation filters, so rows go stale — 71 were re-recorded on
2026-09-29 — and a fixture that goes red outside every lander's filter
stays green on paper. `python3 tools/native_watch.py` is the scheduled
full native run that catches it: it snapshots `origin/main` in a scratch
checkout, runs every fixture natively against an empty golden, and diffs
the result against its previous snapshot under ignored
`build/native_watch/`. It exits 1 on a regression the rule below does not
sanction, bisecting each to its first bad commit (`--no-bisect` to skip),
and writes `build/native_watch/report-<sha>.md` listing regressions,
sanctioned `unimplemented:` debt, fixtures that arrived red, and fixtures
that turned green. It edits no golden or board; the report is for whoever
files the item. Run it every few hours on each host that has a native
golden — the scratch checkout and its mbx cache are removed after each run:

```bash
# macOS: every 4 hours via launchd (~/Library/LaunchAgents/dev.omega.native-watch.plist)
#   ProgramArguments: /usr/bin/python3 <repo>/tools/native_watch.py
#   WorkingDirectory: <repo>   StartInterval: 14400
launchctl load ~/Library/LaunchAgents/dev.omega.native-watch.plist
```

```powershell
# Windows: every 4 hours via Task Scheduler
schtasks /Create /SC HOURLY /MO 4 /TN OmegaNativeWatch /TR "python <repo>\tools\native_watch.py"
```

Large structural moves (deleting a mechanism, rewriting a seam) may land while
some fixtures stop compiling, under one rule. A regressed pass or run fixture
must reject with a diagnostic whose message starts `unimplemented:` and names
the missing piece; the same commit re-records the affected goldens and states
how many fixtures moved. `grep -c "unimplemented:"` over the goldens is the
debt to burn down. Never allowed: a fail fixture that stops rejecting
(unsound acceptance), a crash, or a native program that builds and exits
differently from its golden. Those block landing.

## Sample check gate

`tools/sample_gate.py` is the samples' cheap counterpart to the corpus gate.
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

`tools/sample_native_gate.py` runs each sample's `omega update`, then builds
and runs it. Without `--cli` it first builds an opt-level 1 CLI into
`build/gate-cli/` (with `mbx`, or Cargo when `mbx` is absent) and measures
that: package review of a std-using sample takes minutes with the
unoptimized dev binary, and 31 projects took 66 s with the optimized CLI
against 436 s with the dev build, with identical results. The build is
incremental and leaves `target/debug/omega` alone. The shared
`build/gate-cli/` caches that build across every gate in one checkout, but a
run never executes the shared image: it copies the built binary aside under a
`build/gate-cli.lock` and runs the copy, so two gates can reuse and rebuild the
shared directory at once without one holding an open image the other's build
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
and timeouts fail; no negative coverage disappears into an unscored row. Existing
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

The corpus gate executes fixtures with `input.txt` as stdin (absent means an
empty stream); the sample gate does the same for a project carrying
`input.txt` beside its entry and then scores the authored
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


## Full baseline

Use these gates when establishing or refreshing a full checkout baseline:

```bash
python tools/check_native_boundary.py
python tools/fmt.py --check
mbx clippy --workspace --all-targets -- -D warnings
python tools/corpus_gate.py
mbx check --workspace --all-targets
mbx nextest run --workspace --lib --no-fail-fast
```

The native-boundary gate walks Cargo's declared production path dependencies,
including optional, build and target-specific edges. It rejects source/compiler,
package and interpreter owners anywhere behind `native-production`. Its seeded
negative controls run with `python tools/tests/test_native_boundary.py -v`.
This is a check of the native facade, not a claim that all workspace topology
has converged to the two-driver contract.

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
with the remedy printed beside them.

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
| Root `AGENTS.md`, `CLAUDE.md`, `README.md`, `OWNER_QUESTIONS.md`, `TASKS.md`, `TASKS_BOOTSTRAP.md`, `TASKS_OPTIMIZER.md`; Markdown under `wiki/` | No library commands — reported under `documentation_paths` in the plan; a `jev` augment may still add checks. |
| Manifests, build scripts, lockfiles, toolchain/configuration, shared fixtures, Omega library sources, tools, and unknown inputs | All libraries. Markdown outside the allowlist, including this file, also takes this fallback. |
| No changed input | No commands; library filter `none()`. |

A change under a `PATH_CANDIDATES` prefix in `test_affected.py` also runs that
catalog candidate, reported under `path_candidates`: `tools/` runs the tools
unit tests, `tools/release/` the release-record plan, `tools/bootstrap/` and
`bootstrap/` the chain-hygiene check, `omega-rust/` the fmt check, and
`tests/omega/<tier>/<group>/` the corpus gate filtered to the touched groups
(a golden-only edit selects no gate). Changes under
`source/library/` run `std_package_check` (`omega --check
source/library/std/main.omg`, about 2.5 minutes on a debug build): a corpus
fixture compiles only the std bodies its program reaches, so only a whole-package
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
the eight `RC-*` gates with their exact contract invocations on one host, and
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
