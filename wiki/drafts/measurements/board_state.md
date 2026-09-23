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

## Design-blocked, and where the decision lives

Every one of the five names its decision, so none is silently stuck:

| Row | Named decision in OWNER_QUESTIONS.md |
| --- | --- |
| MODULE-NAMESPACE-RESOLUTION | `unmanaged-root-package-identity` (question 10) |
| NOMINAL-FIELD-FLOW | `mutable-self-receiver-declared-field-rows` (question 1) |
| NATIVE-WRAPPER-ENCODING-AARCH64 | `aarch64-semantic-wrapper-arrival-shape` (question 5) |
| OWNED-SELF-RECEIVER-AFFINE-DISCARD | `owned-self-receiver-implicit-retirement` (question 12, filed 2026-09-23) |
| ARITHMETIC-POLICY-REALIZATION | `terminal-operation-level-trap-crash-site` -- one lane only; the row says the rest "is implementation work, not a reason to mark this whole row owner-blocked" |

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
both. It is one recognizer, not many gaps:
`typed-trees-to-checked-trees` `execution/terminal_unit/control/statement_sequence.rs`
omits a machine's Unit plan when a structural local's initializer is not a
`Call`. The corpus census in
[compiler progress](compiler_progress.md) breaks the family down by omission
site; the largest is 21 fixtures at `local data: structural call binding`.
