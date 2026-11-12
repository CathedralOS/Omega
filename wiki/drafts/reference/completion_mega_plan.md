# Completion mega plan

The Rust reference compiler cannot ship yet. None of the six
[release gates](rust_compiler_completion.md#release-matrix) has passed on any of
the four hosted targets. Main also currently fails `RC-REPOSITORY` on formatting
and warnings. Work volume is not the problem: main took about 9,800 commits in
the 30 days ending 2026-11-05. The problem is convergence. The corpus loses
passing rows as fast as other rows start passing, and the corpus has been
reshaped often enough that week-to-week percentages are not comparable.

This plan does four things, in order:

1. Stop regressions and set one fixed baseline.
2. Finish the ratified surface migrations, which unblock most of the remaining
   failures.
3. Close the native matrix on each host.
4. Produce the four-host release record.

Every number below is measured from main at `5ce48a36e` (2026-11-05). The
measurement commands are in [Measurement protocol](#measurement-protocol).
[rust_compiler_completion.md](rust_compiler_completion.md) remains the
acceptance contract. This document only sequences the work toward it.
Percentages here are internal progress proxies. They are not gate results,
because a gate has no partial credit.

## Where we are

### Release gates × hosts

| Gate | linux_x86_64 | linux_arm64 | macos_arm64 | windows_x86_64 |
| --- | --- | --- | --- | --- |
| `RC-REPOSITORY` | **red on main**: `fmt.py --check` reports 8 unformatted files; an `omega` build reports unused imports, which fail `clippy -D warnings` | same commit | same commit | same commit |
| `RC-SOURCE-SEMANTICS` | open: 620 of 16,650 golden rows fail; 506 fixtures on disk have no golden row | — | — | — |
| `RC-BUILD-AND-PACKAGES` | not run since the 2026-09-20 record | not run | not run | draft record in [CathedralOS/Omega#1882](https://github.com/CathedralOS/Omega/pull/1882) |
| `RC-NATIVE-MATRIX` | open: 69% authored, 274 regressed rows | open: 71% authored (`--both` golden), 10 regressed | open: 55% authored, 434 unstamped legacy rows | open: 65% authored, 130 regressed |
| `RC-DIAGNOSTICS` | open: 180 `fail/` rows reject for the wrong reason, 2 are accepted, 1 is incomplete | — | — | — |
| `RC-REPRESENTATIVE-PROGRAMS` | see [Fresh sample measurement](#fresh-sample-measurement) | no record | no record | no record |

A dash means the gate is host-independent at this stage (checking is the same
on every host) or that the cell is not recorded separately.

### Check leg (`tests/omega/corpus_outcomes.txt`)

| Tier | Correct | Total | Rate |
| --- | ---: | ---: | ---: |
| `pass/` (checks) | 8,027 | 8,442 | 95.1% |
| `run/` (checks) | 215 | 237 | 90.7% |
| `fail/` (rejects for the expected reason) | 7,788 | 7,971 | 97.7% |
| **All** | **16,030** | **16,650** | **96.3%** |

- 459 rows carry `regressed-from:`. Each is a fixture that passed at some point
  and no longer does.
- The golden is a historical three-way integration, not a fresh full run (see
  its `# context:` line). Its rows carry stamps from 464 different
  revisions, 116 of them dirty `+wt` trees, and 1,311 rows carry a `+wt` stamp.
- 506 fixtures on disk have no golden row, so the denominator is incomplete.

Domains with the most failing check rows: `pass/facts` 78, `pass/proofs` 29,
`pass/providers` 27, `generics` 42 (pass and fail), `domains` 34, `build` 32,
`pass/borrows` 19, `pass/float` 19, `pass/layouts` 17, `pass/drops` 16.

### Native leg (authored exit oracle met / 7,449 fixtures that carry one)

| Host golden | Authored | Executed | Rejected | Regressed | Notes |
| --- | ---: | ---: | ---: | ---: | --- |
| `corpus_native_linux_x86_64.txt` | 5,167 (69%) | 5,602 | 271 | 274 | |
| `corpus_both_linux_arm64.txt` | 5,273 (71%) | 6,605 | 973 | 10 | includes `--both` interpreter agreement |
| `corpus_native_windows_x86_64.txt` | 4,870 (65%) | 5,302 | 334 | 130 | |
| `corpus_native_macos_arm64.txt` | 4,088 (55%) | 4,519 | 16 | 2 | only 5,208 rows; 434 have no revision stamp |

`corpus_scoreboard.py` divides by the rows present in the golden. The table
above divides by the authored oracles on disk, so missing rows count as
failures, as the closure rule requires.

### Biggest regression clusters

These are the first diagnostics of rows that previously passed. Repairing a
cluster's root cause is how the rows return.

| Rows | Leg | First diagnostic (normalized) |
| ---: | --- | --- |
| 24 | linux native | `IntegerAt call in NativePlacement::initialize requires every performed value to fit its physical…` |
| 23 | linux native | `machine call cycle … rank range membership, pinned endpoints, or nonincrease is unproven` |
| 21 | linux native | `cleanup effect dependency … has no exact structural residual disposition` |
| 21 | linux native | `… has no executable body: this compiler does not lower a state of it yet (normal-exit cleanup)` |
| 21 + 22 | native + check | `terminal-artifact production failed: Lowering(…)` |
| 17 | check | `exact arithmetic … may overflow …: the operands are not provably in range` |
| 11 | linux native | `placed view …: demanded premise closure checking failed` |
| 7 each | check | write-only input authority on opaque implementations; `cannot form specification operator in closed const argument`; filesystem `MalformedStructuralOperation`; retained loan backing |

### Migration debt (ratified surface changes still unfinished)

These are counted from tracked source with `git grep`. Each count is a coarse
regex over authored source, so it can include prose. Treat it as a trend line,
not an exact site list.

| Board item | Old surface still present | Sites |
| --- | --- | --- |
| NESTED-MACHINE-UNIFICATION | `state` declarations in `.omg` | ≈0 in sources; `KeywordKind::State` still parsed |
| CANONICALIZE-SCALAR-RANGE-CONTRACTS | `T [lo..hi]` suffixes / `TypeConstraintNode::Range` | 15 `.omg` sites / 85 Rust files |
| REQUIRES-UNIFICATION | `where` | 728 hits in 620 files |
| BUILD-OWNED-NATIVE-BINDINGS | `via` | 708 hits in 319 files |
| TOP-LEVEL-BOUNDARY-REQUIREMENTS | `boundary requirement` | 61 hits in 59 files |
| OPERATOR-MACHINE-SUPPLY | `operator` introducer | 52 hits in 18 files |
| STAGED-BUILD-CAPABILITIES | `BuildInputs`/`BuildSource`/`BuildOutput`; `depend*(` | 721 hits in 98 files; 3,087 hits in 2,996 files |

### Spec coverage

`python3 tools/progress.py` maps 104 of 148 language-spec sections to at least
one pass group. 44 have no pass group at all. The largest gaps are reflection
(13 sections), assembly (5), generics (6, including transparent aliases and
generic machine requirements), `Nested machines and identity`,
`Data and case decomposition`, `Requirement clauses` and
`Borrowed-storage invariant windows`. Each of these is also a board item below.
A gate cannot pass on a feature nothing exercises.

### Trend and why it is not converging

| Main as of | Check rows | Check correct | Linux native authored / rows |
| --- | ---: | ---: | ---: |
| 2026-10-06 | 2,267 | 97.9% | 542 / 1,022 |
| 2026-10-22 | 11,502 | 99.9% | 2,475 / 3,537 |
| 2026-10-29 | 20,210 | 97.9% (287 regressed) | 7,245 / 8,750 |
| 2026-11-05 | 16,650 | 96.3% (459 regressed) | 5,167 / 7,013 |

The corpus grew ninefold in three weeks, then the deduplication of facts
([CathedralOS/Omega#1841](https://github.com/CathedralOS/Omega/pull/1841),
[CathedralOS/Omega#1844](https://github.com/CathedralOS/Omega/pull/1844)) removed
roughly 4,300 fixtures. Because the denominator moved, raw rates cannot show
progress. The regressed count can, and it is rising. In the last 30 days, 45%
of commits used the `psi` lane while 6% used `backend`. Checking work is
outpacing native realization, which is why native rates trail check rates by
about 25 points.

## What's left

The open board ([TASKS.md](../../../TASKS.md)) has 41 outcomes, including two development-health
items. Grouped by what blocks the release:

| Workstream | Board items | Gate(s) it closes |
| --- | --- | --- |
| A. Main health | RC-REPOSITORY red; 459 + 274 regressed rows; 8 open draft PRs; ITERATION-PERFORMANCE, VALIDATION-ENVIRONMENT | all; nothing else counts while main regresses |
| B. Ratified surface migrations (human priority) | NESTED-MACHINE-UNIFICATION, CANONICALIZE-SCALAR-RANGE-CONTRACTS, STATIC-MACHINE-MIGRATION, RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY, BUILD-OWNED-TARGET-SELECTION, BUILD-OWNED-NATIVE-BINDINGS, TOP-LEVEL-BOUNDARY-REQUIREMENTS, OPERATOR-MACHINE-SUPPLY, REQUIRES-UNIFICATION | SOURCE-SEMANTICS, DIAGNOSTICS, NATIVE-MATRIX |
| C. One pipeline, one artifact | PIPELINE-ARCHITECTURE-ENFORCEMENT, PROVIDER-SELECTION-AFTER-TERMINAL, LOC-VALUE-CUSTODY, EARLY-TERMINAL-JOBS, UNIVERSAL-INTERPRETER-CLOSURE | NATIVE-MATRIX (`--both`), BUILD-AND-PACKAGES |
| D. Semantic frontier | STATE-LOCAL-VALUE-FRONTIER, MODULE-NAMESPACE-RESOLUTION, DOMAIN-ACCESS-AUTHORITY, TERMINATION-RANKING-CHECKS, RECORD-OR-SUM-DATA, LIBRARY-REPRESENTATION-FAMILIES, STRUCTURAL-BORROW-IDENTITY, PROOF-KERNEL-CORE, ADDRESS-EXPOSURE, ATOMIC-MEMORY-MODEL | SOURCE-SEMANTICS, DIAGNOSTICS |
| E. Build, entry and ABI | STAGED-BUILD-CAPABILITIES, ENTRY-CONTENT-ROOTS, NORMALIZED-ABI-LOWERING, BUILD-SEMANTIC-EXCLUSIONS, TEST-GROUP-FILESYSTEM-TARGETS, CHECKED-ASM-LABEL-CONTRACTS | BUILD-AND-PACKAGES, NATIVE-MATRIX |
| F. Programs and libraries | SAMPLE-CORPUS (Squalr, macOS `.app`), WIRE-CODEC-LIBRARY, COMPONENT-SUBSTRATE (filesystem services, TR3–TR8) | REPRESENTATIVE-PROGRAMS |
| G. Release | RC-RELEASE-RECORD-AND-CLOSURE | closure |
| H. Not in the hosted release | TOPOLOGY-PLAN-VERIFICATION, EMBEDDING-SOURCE-TO-HOST, EXTENT-LIBRARY-TRANSFORMS (UEFI), TASKS_OPTIMIZER.md, TASKS_BOOTSTRAP.md, OMEGA-PRODUCT-COMPILER-SOURCE | none directly; see the scope decision below |

### Scope decision for the owner

The contract defines the release surface as the accepted specification, not as
the board. Workstream H items whose fixtures already sit in `tests/omega/pass`
(embedding, topology, scientific, extent and content custody) count toward
`RC-SOURCE-SEMANTICS`. For each one, the owner must choose one of two paths
before Wave 3:

- **Finish it** within this release.
- **Fence it.** Make the compiler reject the feature explicitly, which the
  contract permits. Then move its rows to a failing-by-design record and revise
  the matrix in the same change.

Without this choice, the release depends on the largest and least-started
items. Record the answer in [OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md).

## Priorities

The order follows [AGENTS.md](../../../AGENTS.md#priority-and-continuous-execution).
It adds one rule the data supports: **a regression outranks any new capability.**
A row that worked before is the cheapest one to get back, and every unrepaired
regression hides newer breakage in the same area.

1. **Main health** (A). Fix the red `RC-REPOSITORY` and the regression clusters,
   and bring the 8 draft PRs to a merge or a recorded withdrawal.
2. **Ratified migrations** (B), in dependency order:
   TOP-LEVEL-BOUNDARY-REQUIREMENTS → BUILD-OWNED-NATIVE-BINDINGS →
   BUILD-OWNED-TARGET-SELECTION → STATIC-MACHINE-MIGRATION →
   RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY; in parallel,
   NESTED-MACHINE-UNIFICATION, CANONICALIZE-SCALAR-RANGE-CONTRACTS,
   REQUIRES-UNIFICATION and OPERATOR-MACHINE-SUPPLY.
   Each one deletes an old parse path. Until that path is gone, every new
   fixture risks being written against the dying surface.
3. **One pipeline, one artifact** (C). `--both` and four-host runs need one
   Terminal artifact per build, settled once per target.
4. **Semantic frontier and build/ABI** (D, E), worked as diagnostic clusters
   from the golden rather than one fixture at a time.
5. **Per-host native closure and programs** (F).
6. **Release record** (G).

## The plan

Each wave has exit criteria that someone can measure. Waves overlap where the
workstreams are independent. A later wave never starts by weakening an earlier
wave's exit criteria.

### Wave 0 — stabilize and baseline (first; short)

- Fix the 8 rustfmt diffs and the unused imports so `RC-REPOSITORY` is green.
  Add `fmt.py --check` and a warnings-as-errors build to the landing gate, so it
  cannot go red again unnoticed.
- Triage the 8 open draft PRs
  ([#1919](https://github.com/CathedralOS/Omega/pull/1919),
  [#1882](https://github.com/CathedralOS/Omega/pull/1882),
  [#1880](https://github.com/CathedralOS/Omega/pull/1880),
  [#1874](https://github.com/CathedralOS/Omega/pull/1874),
  [#1853](https://github.com/CathedralOS/Omega/pull/1853),
  [#1831](https://github.com/CathedralOS/Omega/pull/1831),
  [#1823](https://github.com/CathedralOS/Omega/pull/1823),
  [#1818](https://github.com/CathedralOS/Omega/pull/1818)) through
  coordinator-review, then land each or record its withdrawal.
- **Frozen baseline.** Take one clean commit and record the check leg in shards
  (`--shard`, under the 200-fixture local budget) plus `--native` on all four
  hosts. That fixes the 506 missing rows and the 1,311 dirty-stamped rows, and gives each
  later wave the same denominator. Tag it `baseline-2026-11`.
- **Regression burn-down** by cluster, using the table above. Each worker takes
  one diagnostic family and returns all of its rows in one PR, as
  [sweep whole root-cause clusters](../../../AGENTS.md#decision-framing-for-compiler-completion)
  requires.
- **Measurement gaps.** Extend `tools/compiler_dashboard.py` (do not add a new
  tool) with this document's three missing views: the gate × host matrix,
  the migration-debt counters, and native rates measured against on-disk
  oracles.

**Exit:** `RC-REPOSITORY` green on main. Zero `regressed-from:` rows in the
check golden and the Linux and Windows native goldens. Every golden recorded
from one clean revision. No PR older than 72 hours without a recorded next
action.

### Wave 1 — ratified surface migrations

Run them as parallel lanes. Each lane ends when its old syntax is gone from
the parser, which is what the board's acceptance lines require:

| Lane | Delete at the end | Acceptance (from the board) |
| --- | --- | --- |
| Boundary requirements | standalone `boundary requirement` | POSIX and Windows GUI customers `--both` |
| Native bindings | `via` parsing (708 sites) | PlaySoundW / ALSA / AudioQueue `--both` on matching hosts |
| Target selection | target-prefixed machines, target modifiers | one portable call → per-target impls from one artifact |
| Static machines | `machine Data::operation`, attached helpers | 9-place `dropped by`, `generic_counters`, `traits/,layouts/,progress/` `--both` |
| Runtime callable/package dispatch | `dyn` selectors and capture rosters | [Machine values and some](../designs/machine_values_some.md), migrated `traits/` and package controls, `--both` on all hosts |
| Nested machines | `KeywordKind::State`, `state` syntax | 3-level nesting, cycles, 9-place rows `--both` |
| Scalar ranges | `T [lo..hi]` parsing, `TypeConstraintNode::Range` | all sources `--both` |
| Requires | `ClauseKind::Where`, `where` (728 sites) | data/generic/callable/asm `requires` accepted; `where` rejects |
| Operator supply | `operator` introducer | `declared_operator_match_result` etc. `--both` |

Large mechanical rewrites (`where`, `via`, `depend*`) should land as one script
plus one generated diff per lane. Hand edits across 600 files cause conflicts
with every other open lane.

**Exit:** every migration-debt counter is 0 and each old parse path is deleted
in both compilers. The check leg is at or above the Wave 0 baseline, with zero
regressed rows.

### Wave 2 — one pipeline, one artifact

PIPELINE-ARCHITECTURE-ENFORCEMENT, then PROVIDER-SELECTION-AFTER-TERMINAL and
LOC-VALUE-CUSTODY (which retires the Psi 06 shape gates). After those,
EARLY-TERMINAL-JOBS and UNIVERSAL-INTERPRETER-CLOSURE, which delete the
checked interpreter.

**Exit:** two-target builds run Psi 00–07 and Build once. `FIREWALL_ALLOWANCES`
is empty. `tools/stage06_shape_gates.txt` and
`tools/stage06_source_context.txt` are empty. `interpreter_migration.json` is
empty. `--both` passes wherever native passes.

### Wave 3 — semantic frontier as clusters

Work items D and E as diagnostic clusters, highest row count first. Use
`python3 tools/progress.py` (failure families) and the golden's first
diagnostics. Each item's board acceptance list defines done. Close each of the
44 unexercised spec sections with a twin pair, or fence it explicitly under the
owner's scope decision.

**Exit:** `RC-SOURCE-SEMANTICS` and `RC-DIAGNOSTICS` pass on a full sharded run:
every `pass/`/`run/` row checks, and every `fail/` row is `expected`.
`RC-BUILD-AND-PACKAGES` passes.

### Wave 4 — per-host native and programs

One owner per host burns that host's native golden down to 100% authored.
Order: linux_x86_64 (the release-record host), then windows_x86_64, then
linux_arm64 (move from the `--both` emulated record to a native runner), then
macos_arm64 (first restamp the 434 legacy rows). Run
`sample_native_gate.py` for every sample, then Squalr
(`squalr/tools/verify.py native`) and the `window_demo` `.app`.

**Exit:** `RC-NATIVE-MATRIX` and `RC-REPRESENTATIVE-PROGRAMS` pass on each
matching host.

### Wave 5 — release record

Use a clean checkout of one commit and run `tools/release/release_record.py`
on all four hosts. Record all six gates with toolchain, host, commands,
elapsed time and the exact expected skips.

**Exit:** the [closure rule](rust_compiler_completion.md#closure-rule). After
that, optimizer, bootstrap and Omega-written-compiler work resumes against a
frozen, maintained Rust reference.

### Staffing

The bottleneck is landing, not authoring. Run `/swarm` with one coordinator and
about 8–10 workers. In Wave 0, every worker takes a regression cluster. In
Wave 1, use one worker per migration lane. From Wave 2 on, assign one host
owner per platform, because a matching-host run cannot be done remotely. The
coordinator does triage and review only, as AGENTS.md requires. Regressions
come off the top of every queue.

## Measurement protocol

Run these weekly from the frozen-baseline commit forward. Report the deltas,
not commit counts.

```bash
python3 tools/corpus_gate.py --list-pinned-regressions < /dev/null
```

```bash
python3 tools/corpus_scoreboard.py --golden tests/omega/corpus_native_linux_x86_64.txt
```

```bash
python3 tools/progress.py < /dev/null
```

```bash
python3 tools/compiler_dashboard.py --output build/dashboard.html
```

```bash
python3 tools/fmt.py --check
```

Report these numbers each week:

- regressed rows by golden (target 0);
- check-leg correct rows over the frozen denominator;
- native authored rows per host over on-disk oracles;
- migration-debt counters;
- unexercised spec sections;
- `RC-REPOSITORY` state;
- open PR age.

## Fresh sample measurement

Recorded 2026-11-05 on `linux_x86_64` at `5ce48a36e` (4 cores; compiler built
with `CARGO_PROFILE_DEV_OPT_LEVEL=1`). The run covered every non-facts,
non-probe sample with an authored exit or output oracle (326 projects; filter
fragments also selected nested subprojects):

Results pending: the run starts after the compiler build at this commit finishes.
