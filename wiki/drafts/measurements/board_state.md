# Board state: every open row's blocking condition

A per-row index of `TASKS.md` at `2b106d32b4`, so a worker picking up the
board can see what is startable without reading 4,400 lines, and so a reader
can tell "nobody has done this" from "nobody CAN do this yet".

This is an index, not a verdict. Its buckets were assigned by reading each
row's own text. **Four were spot-checked against the tree and three were
wrong**, always in the same direction -- optimistic:

- `TRANSPARENT-TRAIT-REFINEMENTS` read as startable; it was already complete
  and has since been removed from the board.
- `PRIVILEGED-PORT-EFFECT-SETTLEMENTS` reads as one named file; nothing in
  the selection to emission chain produces port effects at all, so it is the
  whole chain.
- `OPTIONAL-STDLIB-SEMANTIC-BINDINGS` reads as one fallback to delete; the
  deletion is gated on corpus-wide consumer migration.
- `BUILD-EXCLUSION-REALIZATION` cited a test to upgrade that no longer
  exists, because the upgrade landed and the test was renamed.

So treat ACTIONABLE as an upper bound and confirm a row against the tree
before committing to it. "Names concrete files" does not mean "small".

## Counts

| Bucket | Rows | What it means |
| --- | --- | --- |
| ACTIONABLE | 35 | names files/symbols/tests and a specific change; startable today |
| LARGE-FEATURE | 30 | direction exists, multi-crate, no single entry point |
| BLOCKED-DEPENDENCY | 26 | the row names another row or gate that must land first |
| MEASUREMENT | 9 | remaining work is running gates on a named host and recording |
| BLOCKED-OWNER | 5 | needs a language or semantics ruling |
| **Total** | **105** | plus one row closed on 2026-09-23 |

## Design-blocked at the recorded revision

These are the snapshot's classifications, not current blockers. Mutable receiver
contracts are now explicit in the specification; NOMINAL-FIELD-FLOW is
implementation cleanup. Trap-site encoding is also delegated implementation work;
human Psi design review is deferred. Other decisions use stable names rather than
queue numbers.

| Row | Decision reference |
| --- | --- |
| MODULE-NAMESPACE-RESOLUTION | Now implementation under [default builds and anonymous owner identity](../../spec/build/declarations.md#default-builds-and-anonymous-owner-identity), not owner-blocked |
| NOMINAL-FIELD-FLOW | [mutable-self-receiver-declared-field-rows](../../spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains), settled |
| NATIVE-WRAPPER-ENCODING-AARCH64 | Now implementation reconciliation against [target-authored entry contracts](../../spec/build/entry_roots.md), not owner-blocked |
| OWNED-SELF-RECEIVER-AFFINE-DISCARD | Now implementation repair under [ordinary parameter ownership](../../spec/language/ownership.md#consumers-and-cleanup), not owner-blocked |
| ARITHMETIC-POLICY-REALIZATION | `terminal-operation-level-trap-crash-site`, now [delegated implementation](../../../omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit), not owner-blocked |

`SCALAR-ROUTE-REQUIREMENT-OBLIGATION-COUNT` reads as owner-blocked and is
not: its "owner" is the scalar-graph code owner, not the language owner. The
row now says so, because a classification pass got this wrong.

## Blocked on a dependency

Each waits on a row, not on a decision. The most-depended-on rows are worth
knowing, because unblocking one frees several:

| Depended-on row | Rows waiting on it |
| --- | --- |
| STATE-LOCAL-VALUE-FRONTIER | MACOS-APPLICATION-PUBLICATION, BUILD-PRODUCT-REFERENCES, SAMPLE-CORPUS, FILESYSTEM-RELEASE-CONTRACT, TERMINATION-RANKING-CHECKS (native leg), PROOF-CERTIFICATION-BRIDGE |
| COMPONENT-SUBSTRATE | PSI-COMPONENT-REPLACEMENT, TOPOLOGY-PRIVATE-PIPE-INSTALLATION, BOUNDED-INSTALLATION-REACH-ROWS, BUILD-EXCLUSION-REALIZATION |
| REMOVE-BRACKETED-RANGE-ANNOTATIONS | WRITE-ONLY-BORROW, PLACED-ACCESS-NATIVE-OPS, ADDRESS-TRANSLATION-CANARY (fixture migration) |
| BUMP-ALLOCATOR-CANARY + PLAN-LAID-VIEWS | VEC-NATIVE-GROWTH, SQUALR-HEADLESS, ADDRESS-TRANSLATION-CANARY |
| UEFI-PHYSICAL-SEMANTIC-ENTRY | UEFI-OS-HANDOFF, EXCEPTION-ROOTS-AND-TIMER, CANARY-CORPUS (entry-free fixtures) |
| EMBEDDING-SOURCE-TO-HOST | EMBEDDING-RESOURCE-LIFETIMES, INTERPRETED-CATHEDRAL |
| BUILD-TEST-GROUPS | BUILD-TEST-AUTHORITY |
| all eight RC gates | OMEGA-PRODUCT-COMPILER-SOURCE |
| C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT | RC-PCC-REPLAY |

## Blocked on a host this machine is not

Nine rows need a runner rather than a change. Recorded so nobody re-derives
which:

| Row | Host required |
| --- | --- |
| RC-NATIVE-MATRIX-LINUX-X86-64 | Linux x86-64 (it also carries one concrete code fix -- `ElfDynamicTag` skips tag 8 to 10, so `DT_RELAENT` is never emitted beside `DT_RELA`/`DT_RELASZ`) |
| RC-NATIVE-MATRIX-LINUX-ARM64 | Linux AArch64, or a named and versioned emulator |
| RC-NATIVE-MATRIX-WINDOWS-X64 | Windows x86-64 |
| MACOS-X64-HOST-PROFILE | Intel macOS |
| SYMBOLIC-MATERIALIZATION | Linux AArch64 |
| RC-NATIVE-MATRIX-MACOS-ARM64 | macOS arm64 -- **partially discharged**, see below |
| RC-REPOSITORY-CLOSURE, RC-BUILD-AND-PACKAGES, RC-RELEASE-RECORD-AND-CLOSURE | any, but they are evidence assembly rather than repair |

The macOS arm64 lane is the one this machine could answer, and both of its
measurable commands now are: the RC-NATIVE-MATRIX gate (1158 run, 1140
passed, 18 failed, 1 named skip) and the direct-execution observation (925
run, 175 passed, 750 failed, **zero execution mismatches**). Its remaining
two gates, RC-SOURCE-SEMANTICS and RC-REPRESENTATIVE-PROGRAMS, are
unmeasured.

## The largest single lever

`STATE-LOCAL-VALUE-FRONTIER` is worth naming separately. The
`ProgramEntry establishment rejoins 0 Terminal attachment identities` family
is 533 of 750 failures in the macOS direct-execution observation and ~549 of
770 in the Linux record -- host-independent, and the dominant blocker on
both. It was recorded here as "one recognizer, not many gaps" --
`typed-trees-to-checked-trees`
`execution/terminal_unit/control/statement_sequence.rs` omitting a Unit plan
when a structural local's initializer is not a `Call`.

**That framing is wrong, measured 2026-09-27 against the macOS arm64 native
roster at `a9478e720fc`** (`tests/omega/corpus_native_macos_arm64.txt`):
the 144 omissions in it spread across **48 distinct phases**, the largest
holding 26 rows, and only **41 of 144** sit under `statement sequence:` at
all. The other 103 decline in `state_graph/`, `structural_scalar_store/`,
`calls/` and `types/` -- different files, different capabilities.

It matters because "one recognizer" reads as one fix clearing 533 failures,
and a worker who takes the row on that basis will find a dozen unrelated
capabilities behind it. Reduced to one-token controls so far, each with its
own pending fixture: reference bindings into locals (which alone wears SEVEN
phase labels), record literals supplying array or reference fields, composite
and value-call transition arguments, value calls as non-root subexpressions,
domained byte-carrier stores, arithmetic-policy and record array elements,
predicate-domained case payloads, float computation, ranged const parameters
and float-range attached fields. Three are located to an exact line; see
[macos_arm64_native_rejections](macos_arm64_native_rejections.md).

The shared `rejoins 0 Terminal attachment identities` line is the SYMPTOM
every one of them prints. Counting it as a population is what makes the row
look like a single lever. The corpus census in
[compiler progress](compiler_progress.md) breaks the family down by omission
site; the largest is 21 fixtures at `local data: structural call binding`.

## Spot-checks that ran the OTHER way: rows citing red tests that are green

Measured 2026-09-27. The spot-checks above found rows optimistic. These two
are the opposite — rows that name a specific failing test and hold it as
remaining work, when the test passes. Both were found by grepping `TASKS.md`
for "is red", "red since", "still fails", which is a cheap sweep worth
repeating.

- **MATCH-SELECTIVE-LOWERING** says
  `value_dispatch::owned_results::folded_index_projection::folded_index_faces_the_literal_lowering_boundary`
  "is red since `c37c3dbd91`". It passes in 0.008s
  (`cargo nextest run -p checked-trees-to-lowered-psi --test suite`), and the
  test file's own module doc already records the resolution. The bullet's
  stated goal — the folded index getting the same checked plan so the pair
  meets one boundary — is MET.
  What that bullet is ACTUALLY waiting on is worth carrying forward: both
  its sources build `Pack { items: [Cell { tag: 41 }, Cell { tag: 42 }] }`
  and stop at `Unsupported("fixed array literal has no Terminal
  establishment")` — a record literal supplying an array field, which is
  **RECORD-LITERAL-ARRAY-FIELD**. Clearing that moves both, and its element
  there is a RECORD rather than a scalar.

- **CHECK-CLOSURE-ONCE**'s block says
  `omega_cases_declare_a_standard_library_edge_exactly_when_they_use_it`
  "still fails" because
  `tests/omega/pass/providers/boundary_scalar_field_store` declares a
  standard-library edge it never uses. It passes in 0.276s
  (`cargo nextest run -p package-manager --test suite`), and that fixture
  does use the edge — its `main.omg` opens with
  `use omega_language_std::console;`.

Neither row was edited, because `TASKS.md` was held by MAIN-BOARD-AUDIT when
this was measured; both corrections are also in claim notes against their
items. A row asserting a red test is cheap to verify and expensive to
believe: it tells a worker the item is blocked on someone else's repair when
it is not.

## Board rows citing test files that no longer exist

Measured 2026-09-27 by extracting every `` `...rs` `` path `TASKS.md` cites
and resolving it against the tree.

**208 cited source paths; 14 are absent from the live tree entirely** --
not moved under a different prefix, absent. Several rows assert them as
present evidence: BOUNDARY-ISSUANCE says
"`compiler/tests/secondary_processor_startup.rs` integration already exist",
and INTERPRETED-ASSEMBLY says "the existing `canary_suite/inline_asm.rs` x86
fence, interrupt, flags, MSR and control-register byte-emission customers".
`omega-rust/omega/compilation/driver/tests/` now holds five entries.

These files were not lost to restructuring: `c60792d7bdd`, "repo: the
compiler is tested through the corpus gate alone", deleted 185,937 lines of
Rust tests across 114 targets on the owner's direction, because they pinned
internals as often as behavior and every compiler change paid their build.
The deletion was policy. What the rows get wrong is only their tense --
asserting as present evidence something deliberately removed.

**13 of the 14 are recoverable** two ways: from `c60792d7bdd^` in history,
and from abandoned `.codex/worktrees/` copies predating it. Prefer history.
Either is a lead to RE-EXPRESS the behavior as an ordinary corpus entry --
which is what the owner's direction asks for, and what
TERMINATION-RANKING-CHECKS already spells out for two of them -- not a
reason to restore a Rust test target.

| cited path | recoverable from a worktree |
| --- | --- |
| `06_checked-trees-to-lowered-psi/tests/non_byte_borrowed_view_admission.rs` | NO — nowhere |
| `calling_policy_plans/opaque_boundaries.rs` | yes |
| `canary_suite/inline_asm.rs` | yes |
| `compiler/tests/access_plans/source_access_policies.rs` | yes |
| `compiler/tests/build_target_activation/qualified_provider_selection.rs` | yes |
| `compiler/tests/canary_suite/task_runtime.rs` | yes |
| `compiler/tests/kernel_equality_transport.rs` | yes |
| `compiler/tests/layout_plans/interrupt_descriptor_tables.rs` | yes |
| `compiler/tests/layout_plans/writer_lowering.rs` | yes |
| `compiler/tests/pcc_publication.rs` | yes |
| `compiler/tests/secondary_processor_startup.rs` | yes |
| `compiler/tests/terminal_authority/filesystem_cohort_witness.rs` | yes |
| `native_realization/optimized_fragment_projection.rs` | yes |
| `scalar_graph_input/exact_cast_carrier_tests.rs` | yes |

A row that cites a deleted test as existing evidence overstates its own
progress twice: the behavior is unverified, and the next reader spends time
looking for a file to extend. Re-run this sweep after any crate
restructuring; it is a dozen lines of Python and finished in seconds.

## SAMPLE-CORPUS rows that name a LATER blocker than the current one

Measured 2026-09-27 at `695b5e05d07` against a full `tools/sample_gate.py`
run. Three rows attribute a sample to an item that is not what refuses it
today. The attributions are not wrong about the obligation; they are wrong
about the order, and a reader who takes the row will start in the wrong
place.

| sample | row says | what actually refuses it now |
| --- | --- | --- |
| `systems/elapsed_timer` | **REFERENCE-LOCAL-BINDING-ROUTES** / **RECAST-SOURCE-POSITIONS** for the borrowed local recast | provider selection: `Main::time::host` has no selected provider for `TimeHost` |
| `gui/window_app`, `gui/window_demo` | **BOUNDARY-PRIMITIVE-ARRAY-PARAMETER** for by-value primitive-array boundary arguments | `validated named operator application has no checked selected use` — see [named_operator_selected_use.md](named_operator_selected_use.md) |
| `gui/windowed_calculator` | not listed in that row at all | the same named-operator failure as the other two |

`systems/descriptor_walk`, the other sample in `elapsed_timer`'s row, DOES
fail where the row says (`statement sequence: local data: structural call
binding`), so the row is right for one of its two customers.

### `TimeHost` has no implementation on any target

`elapsed_timer` cannot be wired the way the nine Clock-bearing samples were.
`source/library/std/time_host.omg` fully specifies the seam — including the
darwin mapping (`clock_gettime_nsec_np`, units `1_000_000_000`, epoch offset
`0`) and the per-target constants — but `grep` over
`source/library/std/targets/` finds **no `TimeHost` or `monotonic_ticks`
implementation for any target**, including `windows_x86_64`, which that file
says rung 5 binds first. The `TASKS_TIME.md` it cites no longer exists.

So this is unstarted realization work with **no board item naming it**, not a
design question: the design is settled in `time_host.omg` down to the
constants. Someone should either restore an item for it or fold it into an
existing one. It also blocks `tick_marquee`'s portable route — that sample
now checks only because its `Clock2` is package-local and was wired to the
darwin leaf directly.
