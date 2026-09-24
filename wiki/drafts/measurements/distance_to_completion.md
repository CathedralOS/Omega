# Distance to completion: Rust reference compiler

A retrospective measuring how far the repository is from the
[completion contract](../reference/rust_compiler_completion.md): eight gates
plus four hosted platform runs, all green at one clean commit. Assembled
2026-09-24 at HEAD `eda5356432` by seven read-only review passes over the
boards, committed release records, measurement docs, corpus golden, and git
history. No test suites were run for this document; every status below cites
recorded evidence with its revision and date, and the newest evidence is
~1 day old. Refresh by re-deriving each table from current records, not by
appending dated copies. This is a measurement, not a board item; it creates
no obligations.

## The bar

Closure is binary: `RC-REPOSITORY`, `RC-SOURCE-SEMANTICS`, `RC-PCC-REPLAY`,
`RC-PORTABLE-PSI`, `RC-BUILD-AND-PACKAGES`, `RC-NATIVE-MATRIX`,
`RC-DIAGNOSTICS`, `RC-REPRESENTATIVE-PROGRAMS`, plus hosted runs on
`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` — all at one
clean commit. The contract excludes freestanding EFI, optimization quality,
the bootstrap chain, and the Omega-written product compiler as separate
programs of work.

## Headline: 0 of 12 closure rows are closed

| Row | Status | Strongest dated evidence |
| --- | --- | --- |
| RC-REPOSITORY | open | `workspace --lib` 13 of 16,169 failed at `5958706064` (09-20, linux); arch test 595/596 at `301582616c` (09-21). The full 5-command block has never been run once. |
| RC-SOURCE-SEMANTICS | open | `-p compiler --all-targets`: 3281 run / 2014 pass / **1267 fail** at `04d2099ae1` (09-23, macOS). Linux leg: pass-canary 2146 run / 58 fail at `210ffe3c93`. |
| RC-PCC-REPLAY | open | Fast sweep at `a2924de738` (09-23, macOS): terminal-verifier 3 red, terminal-interpreter 2 red, optimization-unit-semantics 3 red; codec clean. No doc leg, no full 5-crate block, no `terminal-psi-to-abstract-operations` reading. |
| RC-PORTABLE-PSI | **gate reading passes** | Two committed, revalidated records (`30ed858582`, `ea025447fe`, 09-21, linux). Never co-committed with the other seven, so it cannot count toward closure. |
| RC-BUILD-AND-PACKAGES | open | `package-evidence` 11 red at `a2924de738`; `build_target_activation` 112/118 and `package_compilation_inputs` 194/196 at `7b224763615`. 5 of 7 package crates and 4 of 6 integration targets unmeasured. |
| RC-NATIVE-MATRIX | open | linux_x86_64: 116 fail of 1122 differential at `a0b906db93` (09-20). macos_arm64: 18 fail of 1158 at `04d2099ae1` (09-23). Also requires RC-SOURCE-SEMANTICS per host, which is red everywhere. |
| RC-DIAGNOSTICS | open — nearest green | All named fail-canary members pass at `7b224763615` (09-21, linux); 1 fixture unregistered; shares RC-SOURCE-SEMANTICS negatives. |
| RC-REPRESENTATIVE-PROGRAMS | open | `samples_compile` 11/33 groups pass at `04d2099ae1` (macOS). No linux/windows reading; harness itself flagged immature. |
| linux_x86_64 | open | Recorded run: differential 116 fail; `_runs` direct execution **143 pass / 770 fail** of 913. |
| linux_arm64 | open — no evidence | Nothing exists: no host, no named-emulator record (recorder supports `--emulator`; never used). |
| macos_arm64 | open | Differential 18 fail; `_runs` 175 pass / 750 fail of 925 — **zero execution mismatches**: every binary that emits runs correctly. Recorder refuses the lane record while native execution is red. |
| windows_x86_64 | open — no evidence | No record, no host measurement; only `--no-run` cross-compiles. This workstation is a matching host. |

## What is actually done

- **Structure: complete.** All 20 planned pipeline stages exist as substantive
  crates — zero stubs, zero `todo!`/`unimplemented!` in `pipeline/`. Both ISAs
  (x86-64, AArch64), all three object formats (ELF, Mach-O, PE), six admitted
  target profiles, and the full semantics/verifier/codec/proof layers are
  populated. The distance is behavioral, not architectural.
- **The compiler understands far more than it realizes.** Per
  [compiler_progress.md](compiler_progress.md) (rev `117f2abc9c`, 09-22,
  windows_x86_64): of 2059 pass fixtures, 690 check clean and 90 compile —
  but only **5 reach native run** on that host. Spec coverage: 4/66 core
  sections have a fixture that runs.
- **The negative corpus is effectively green:** 1156/1156 fail-tier fixtures
  reject with expected diagnostics; 21 acceptance holes and 35 diagnostic
  mismatches remain pinned.
- **The recorder substrate is done:** `tools/release/release_record.py` plus
  its coverage tests exist; two records validate. What is missing is passing
  evidence to record.
- **Corpus golden** (3,327 pinned): pass 1863 checked / ~196 red incl. 5
  crashes; fail 1236 rejected / 21 checked; run 4/11 checked; 35
  diagnostic-expectation mismatches. The corpus gate is targetless, so the
  dominant run-tier families never fire here — corpus red understates
  gate red by design.

## The red is concentrated: five mechanisms own most of it

Each family has a named board owner and an unblocked repair path.

1. **Unit-plan omission → `ProgramEntry establishment rejoins 0 Terminal
   attachment identities`.** ~533–549 of the ~750–770 `_runs` failures on
   both recorded hosts, 72/173 rostered pass failures, ~75–79 stops in the
   samples run. **One mechanism**: `typed-trees-to-checked-trees/src/
   execution/terminal_unit/` omits a machine's Unit plan whenever a
   statement's shape falls outside a per-site recognizer set — the
   compositional-lowering defect AGENTS.md warns about, quantified. Largest
   sites: `local data: structural call binding` (21), `structural field
   store: pure source` (12), `call: call operation` (12). Sample sites
   rank differently than fixture sites — fixtures pin what prior fixtures
   needed; samples predict what programs write. Owner:
   **STATE-LOCAL-VALUE-FRONTIER** (blocks 6 other board rows); sub-fences
   CANARY-CORPUS, GENERAL-CYCLIC-EXECUTION, UEFI-OS-HANDOFF,
   ENTRY-CONTENT-ROOTS. Repair: replace recognizers with compositional
   evaluation/control operations.
2. **`Service<R>` → `Binding<R>` carrier-spelling fixture drift.** ~65–116
   differential failures; mechanical migration owned by
   **ENTRY-CONTENT-ROOTS**. Validation correctly rejects the old spellings;
   fixtures must catch up without weakening rejection.
3. **Windows hosted-exit settlement.** 181 of 896 compile-stage failures on
   this host: `Console::exit_process has no admitted native settlement`.
   One missing realization — bind kernel32 `ExitProcess` via `DllImport` in
   `providers/settlements/source_imports.rs` — owned by the Process-exit
   contract board section.
4. **x86 FMA provider transport.** Encoder exists
   (`machine-emission/src/x86_fma.rs`) but is not wired through the
   instruction pipeline (`object_emission.rs:42` refuses). Small blast
   radius, on the release path via RC-BUILD-AND-PACKAGES. Owner:
   **X86-FMA-PROVIDER-TRANSPORT**.
5. **Checked-trapping conversion lowering** (`Lowering(Unsupported)`):
   `WrappingIntegerDivide`/`SaturatingIntegerMultiply` legalization and
   trapping casts refuse runtime preparation. Named a dominant class inside
   RC-SOURCE-SEMANTICS' 1267. Owner: **ARITHMETIC-POLICY-REALIZATION** —
   needs the delegated `terminal-operation-level-trap-crash-site`
   representation; explicitly implementation work, not owner-blocked.

Adjacent but distinct: **exit-71-vs-70 emitted-binary miscompiles** —
`runtime_shift_signedness`, `runtime_shift_right_atwidth`,
`const_fold_unsigned_shift_right_arg`, `runtime_bitwise_high_ops` — four
genuine x86-64 codegen defects needing a Linux host to bisect; unattributed
to a dedicated item. And the **Fused-provider family** (29 fixtures) is a
*library absence* — FilesystemHost/TimeHost/Gui/Input/ObjectiveC have no
provider — a correct refusal, not a compiler defect.

## Board composition: the work is real and gate-bound

All 129 open items (105 TASKS.md + 17 optimizer + 7 bootstrap) classified by
reading each body against the contract:

- **A — directly blocks a named gate: 81 of 105.** Per-gate counts:
  RC-SOURCE-SEMANTICS **60**, RC-NATIVE-MATRIX **46**, RC-PCC-REPLAY 18,
  RC-BUILD-AND-PACKAGES 16, RC-REPRESENTATIVE-PROGRAMS 10, RC-DIAGNOSTICS 2,
  RC-PORTABLE-PSI 1, RC-REPOSITORY 1 (items name several gates; counts
  overlap). RC-SOURCE-SEMANTICS is the critical path: its
  `-p compiler --all-targets` sweep also covers ~110 ungated compiler test
  targets (178 recorded failing tests across 23).
- **B — indirect/feeder machinery: 11.** Evidence producers without a named
  gate fixture.
- **C — contract-excluded programs: 13.** Cathedral/QEMU (3), freestanding
  EFI/UEFI-OS (2), embedding/interpreted (4), installation runtime (2),
  extra platform (1), Omega-written compiler (1).
- **D — speculative/unratified expansion: 0.** Every item traces to a
  ratified `wiki/spec/` contract. The board is not bloated with invention;
  it is large because the contract is large.

Cross-cutting index [board_state.md](board_state.md) (rev `2b106d32b4`)
buckets the same rows: 35 actionable, 30 large-feature, 26
dependency-blocked, 9 measurement-only, 5 owner-blocked — with a measured
caveat that spot-checked buckets were optimistic. Dependency levers:
STATE-LOCAL-VALUE-FRONTIER unblocks 6 rows, COMPONENT-SUBSTRATE 4,
REMOVE-BRACKETED-RANGE-ANNOTATIONS 3.

Owner-decision blockers: Q1 `compile-time-proof-work-ceiling` gates
RC-PCC-REPLAY (via C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT); Q2
`aarch64-semantic-wrapper-arrival-shape` gates
NATIVE-WRAPPER-ENCODING-AARCH64; Q3 `interpreted-inline-assembly` is a
separate program. Both Q1 and Q2 carry proposed solutions.

## Trajectory: divergent at current allocation

Measured from git history (23,790 commits; September ~10,092, ~439/day,
swarm-driven since 09-12):

- **Release-closure work is flat at ~8% of commits** across August and
  September; zero `RC-`-named commits in the last 500.
- **~28% of the last 500 commits are coordination/bookkeeping** (board,
  merge, swarm-merge, swarm, measurements lanes); ~24% touched TASKS.md.
- **Open items grew 30 → 105 since Aug 1.** Measured organic September
  rate: ~77 adds/week vs ~70 removes/week — **net +8–10/week; the board
  does not converge at this rate.** The Sep 19–21 doc-mining spike
  (+3,264/−3,505 item lines) was a one-off consolidation event, still
  +33–37 residual. Best sustained removal observed: −14/week for 2 days
  post-consolidation; at that rate ~7.5 weeks to clear 105 items.
- Discovery is outpacing closure: more commits now record what is red than
  land gate-advancing work.

## Evidence that does not exist

linux_arm64 and windows_x86_64 have zero platform evidence; RC-REPOSITORY's
5-command block has never run as a block; no `mbx test --doc` legs; no
readings for 5 of 7 package crates, 4 of 6 compiler integration targets,
`terminal-psi-to-abstract-operations`, or `samples_compile` outside macOS;
no expected-skip declarations; nothing recorded at HEAD.

## Distance estimate (inference, labeled)

- **Measured today:** 0/12 rows; the passing legs (Portable-Psi record,
  near-green diagnostics, 1156/1156 negative corpus, zero macOS execution
  mismatches) show the end state is reachable — emitted binaries are
  correct; the compiler fails to *produce* them, it does not produce wrong
  ones.
- **The capability distance is bounded and named:** five mechanisms above,
  plus a thin tail of genuine miscompiles and unmeasured gates. Roughly
  81 board items stand between here and a recordable attempt, but the
  dominant share of *red* concentrates in ~5 repairs and 1 fixture
  migration.
- **At the measured organic net rate (+8–10 items/week) the board never
  empties; at the best sustained removal rate it clears in ~7.5 weeks.**
  Neither is a promise: the binding constraint is allocation — closure
  work holds ~8% of effort while discovery scales with swarm volume.
- The honest single-line answer: **structurally complete, behaviorally a
  concentrated distance away, currently diverging.** The work is "repair
  ~5 mechanisms, migrate ~100 fixtures, record 12 rows," executed under a
  policy that today spends most effort elsewhere.

## Recommended moves (smallest first)

1. **Rebalance the next wave to the five mechanisms** — STATE-LOCAL-VALUE-FRONTIER
   first (unblocks 6 rows, ~70% of run-tier red), then ENTRY-CONTENT-ROOTS
   migration, Process-exit settlement, FMA transport,
   ARITHMETIC-POLICY-REALIZATION. Acceptance: next recorded `_runs`
   observation drops materially; samples_compile pass count rises.
2. **Record windows_x86_64 from this host** — the only lane a matching
   machine can answer without new hardware. linux_arm64 needs the unused
   `--emulator` path or an owner-named runner.
3. **Run RC-REPOSITORY's 5-command block once at a clean commit** — cheapest
   gate, never attempted as a block; converts "unmeasured" to a named red
   or green.
4. **Answer Q1 and Q2** — proposed solutions exist; both sit on gate
   critical paths.
5. **Stop board mining until open items < ~80** — consolidation cost is
   measured (~40 commits for one spike); steady-state discovery already
   exceeds closure.

## Live verification at HEAD (2026-09-24, windows_x86_64)

After the review passes above, the load-bearing claims were checked against
the live tree and the compiler was actually run — surfacing a regression no
record had captured.

Verified as written: item anchors at TASKS.md 880/1450/2992/3786; the FMA
refusal at `native-realization/object_emission.rs:42` with the unused
`x86_fma.rs` encoder present; the recognizer mechanism in
`terminal_unit/control/statement_sequence.rs::first_unsupported_statement`
(`LocalData` admitted only through a set of shape predicates). `python
tools/fmt.py --check` exits 0 — the formatting leg of RC-REPOSITORY is green
at HEAD, the first gate leg ever measured at this revision.

Witnessed fixture behavior: the pinned-red
`pass/text/domain_forget_validate_transitions` reproduces its recorded
diagnostic verbatim. **The pinned-green `case_literal_texteq_terminal_exit`
rejects at HEAD with 272 diagnostics** — and `source/library/std/main.omg`
fails standalone with the same 272, so `omega_language_std` does not compile
at HEAD: every std-dependent fixture, sample, and product is currently red.
A no-std fixture (`pass/arithmetic/anonymous_rational_arguments`) compiles
clean, confining the breakage to the std closure.

Cause, byte-offset verified: commit `7b9dce26d7` (psi: case values require
carrier-qualified names, landed ~23h before this measurement) made
`references.rs:66` reject single-segment authored names matching case
declarations. It also fires on the *head segment of already-qualified paths*:
each of the 272 error offsets lands exactly on the `Float` token of
`Float::meaning32(...)`/`Float::meaning64(...)` calls in `ensures` clauses of
`source/library/core/float_operations.omg`, where `Float` is a machine
namespace, not a case value. The check skips names inside a multi-member
path node but not a single-segment name that is the base of a longer
qualified path; `Float` matches `case Float` declarations
(`ValueClass`/`AbiValueClass` in `std/calling.omg`) inside the package
closure. The same commit migrated test fixtures but zero `source/library/`
files, and its recorded validation ran no std-consuming compile.

Two consequences for the distance estimate above: every gate recorded
before `7b9dce26d7` is now optimistic — the live tree is strictly redder
than any evidence on file; and the `Type::{}` in the diagnostic is a literal
placeholder, so the message's suggested carrier path is wrong (a second,
smaller defect in the same check).

## Examined evidence and caveats

Batch: contract; `tools/release/records/` (2 JSON); TASKS.md / OPTIMIZER /
BOOTSTRAP fully enumerated; OWNER_QUESTIONS.md open section; corpus_outcomes
.json; `compiler_progress.md`, `board_state.md`, `known_baseline_failures.md`,
`macos_arm64_rc_gates.md`; git history through HEAD. Weak findings kept
separate: 21 fail-tier acceptance holes and 35 diagnostic mismatches may be
intentional pins; aarch64 emission parity unverified (code exists, output
unproven); bucket-A classification is an upper bound — spot checks found
labels optimistic, and several A rows may move to C if the owner deems
task-runtime/component/quotient surface post-completion.
