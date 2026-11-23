# Record: unit_063 (areas 03-source-stage-05 and 04-source-stages-06-07)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/`, detached at `4f3c255d0559`. Paths are relative to it.
`P5` = `source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program`.
`P6` = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program`.
The unit has 14 files from stage 05 (all tests) and 28 files from stage 06 (6 examples, `lib.rs`, 17 `src` files and sub-modules, 4 test files). I read every line of every range. "read" below means I read the cited lines; "grepped" means a Grep count or listing; "inferred" means I did not open the other site.

Stage 05 in this unit is test code only. It exercises `lower_bodies`, `lower::machine`, `outcome::outcomes`, `counted_outcomes` and `prepare_compilation`. Stage 06 in this unit is the part of the checker that is not the prover or the places analysis: call contracts, ceilings, crash coverage, formation, entry-input inference, join predicates, dominance and equality transport.

---

## Stage 05 tests (14 files)

### P5/src/tests.rs (4,625 lines, 64 `#[test]`, grepped count)

1. **Purpose.** In-crate unit tests that build closed `Body` / `Signature` / `Outcome` values by hand (no source text) and lower them, then assert the exact blocks, operations, edges, obligations and place/write rows that come out. They are the stage's regression record for lowering shape. Stated in the header: "Lowering tests build closed bodies directly" (`tests.rs:1`).
2. **Shape.**
   - Helpers: `lower` (`:294`), `lower_with` (`:300`), `try_lower` (`:310`), `lower_records` (`:327`; calls `lower::places::declare` `:342` then `lower::machine` `:351`), `assert_module_and_tables_agree` (`:369`), `lower_places` (`:2568`), `lower_sums`/`try_lower_sums` (`:3640`, `:3653`), `settle_both` (`:2177`).
   - Fixtures: `records()` (Point/Line, `:2492`), `sums()` (Point/Shape with cases Dot/Pair/Boxed/Empty, `:3570`), `constrained()` (data `requires`, `:4118`), `erased_module()` (`:4288`), `Step` enum for generated outcome programs (`:2155`).
   - Nested module `provider_source_identities` (`:27`) with 2 tests on `Program::with_captures` / `exact_toolchain_sources`.
   - Submodules mounted by path: `mod loops;` (`:4437`) and `array_views` (`:4522`).
3. **Inputs and outputs.** Input: hand-built bodies, signatures, outcome rows, record tables. Output: asserts only. Reads `crate::lowering::{body, outcome, lower, boundaries, callables, qualifications, canonical, components}` and `terminal_check_tables` rows. Also calls `terminal_codec::encode_module`/`decode_module` (`:3045`, `:4237`, `:4398`) and `terminal_semantics::plain_affine::verify` / `verify_claims_in` (`:3137`, `:3170`, `:4590`, `:4622`).
4. **How it decides (what the tests pin).**
   - Left-to-right evaluation by operand order, not by id (`:433`, `:2615`, `:3681`).
   - `a > b` lowers as `b < a` by operand identity (`:497`).
   - A call that never returns is a block terminator `InvokeNoNormal`; later statements are not lowered and the first one is recorded once with its call in `lowered.sources.unreachable` (`:1412`, `:1447`, `:1745`). The warning keys on source spans built by `authored(n)` (`:1722`).
   - Callee crash routes are substituted with actuals (`:1056`, `:1289`). A private body route does not replace a published ceiling (`:1171`, `:1205`, `:1241`).
   - Outcome settling callee-first is compared to whole-program rounds on 4000 xorshift-generated programs of up to 5 machines (`:2254`); the lowest refusing machine is named (`:2412`); 512-link chain walks each body once (`:2436`).
   - A cast owes `OperationSafety` only when it is partial (widen, no obligation) (`:843`).
   - Record/sum places: one place per projection, interned and reused (`:2694`); a whole copy logs one `Overwritten` row on its fresh root and leaves leaf contents to the checker (`:2769`); a record argument is copied into a root and its home retired on the continuation edge (`:2904`).
   - Dispatch lists every declared case in declaration order, undeclared ones share the wildcard block (`:3779`); a missing case with no wildcard is `Unestablished` (`:3854`); a wildcard after all cases is dropped and recorded in `sources.dead_wildcards` (`:3876`).
   - Data `requires` mint one `DataDomain` obligation per clause per forming operation, created before it (`:4141`).
   - Erased formals have identity but no runtime lane; an erased binding read at run time is refused (`:4332`, `:4405`).
5. **Cases by shape.** Refusals pinned by exact detail strings, each quoted from the test:
   - "value machine can fall off its end" (`:907`).
   - "designated operands on a call that is no boundary call" (`:1888`).
   - "transition crash in a case arm" (Unsupported, `:4061`).
   - "spelling scalar home without a type identity" (Unsupported, `:3547`).
   - "a dispatch selects no arm for a declared case" (`:3857`).
   - "completion an erased binding is read as a runtime value" (`:4417`) and "expression erased term other than a literal or a binding" (`:4434`).
   - Slot past the end gives `ForeignIdentity` with "slot is past the end" (`:1655`).
6. **Same job elsewhere.** The outcome-settling test (`:2254`) is the same comparison done by `outcome::outcomes` (callee-first) and `counted_outcomes(.., by_rounds)`; the by-rounds path is kept only as an oracle (inferred from `settle_both` passing `by_rounds`, `:2226`). `terminal_semantics::plain_affine::verify` is a second checker of custody that these tests use as an independent oracle (`:3137`).
7. **Unused or one-caller.** Tests only. `Step::Refuse` is used by `settle_both` only. `fn lower_places` has 8 call sites in this file (grepped by eye from read; not counted with a command).
8. **Stated limits.** Comment at `:3551` ("Sums by structure") frames sums as a slice; test `each_sum_construct_outside_the_slice_is_refused_by_name_in_lowering` (`:4040`) names only one refusal ("transition crash in a case arm") although the name says "each". A comment at `:2861-2865` says lowering "does not say what a copy's leaves hold"; the checker resolves it (`terminal-psi-to-terminal-psi`, `places` named in the comment, an older crate name).
9. **Fail-open risks.** None seen (test file). One coverage gap relevant to fail-open: `a_refused_operand_is_not_a_never_returning_evaluation` (`:1634`) pins that a refused operand is not treated as a never-returning call.

### P5/src/tests/array_views.rs (259 lines)

1. **Purpose.** Tests that `CollectionView` and `CollectionSubslice` expressions lower to `EstablishElementView` / `EstablishMutableElementView` operations that keep the backing array and take a separate loan.
2. **Shape.** `array_types` (`:6`), tests `explicit_array_views_retain_backing_and_a_separate_loan` (`:50`, loops over Shared and Mutable) and `direct_array_windows_check_endpoints_against_the_whole_loan` (`:163`).
3. **I/O.** Builds `[u8;3]` and a borrowed byte-view record; reads the lowered operations and `ObligationKind::Loan` / `OperationSafety` rows.
4. **Decides.** Keyed on identity: the view's `source.place` must equal the whole-array loan place, and `ByteSequenceLength` and `ByteSequenceSubslice` must measure that same place (`:249-250`). A window owes its own endpoint obligation of kind `OperationSafety` (`:251`).
5. **Cases by shape.** Fixed `[u8; 3]` only (`ArrayExtent::Literal(3)`, `:36`); element type `u8` only.
6. **Same job elsewhere.** Integration test `prepared_array_extents.rs` also covers `[u8; N]` length at stage 05.
7. **Unused.** Tests only.
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/src/tests/loops.rs (656 lines, 10 `#[test]` per crate-map)

1. **Purpose.** Call cycles lowered as loops: head blocks, jump-backs, `scalar_block_invariants`, rank witnesses.
2. **Shape.** `elaborate` (`:47`), `countdown` (`:62`), `mutual` (`:238`), `ranked_countdown` (`:415`); tests at `:88`, `:173`, `:266`, `:304`, `:335`, `:426`, `:544`, `:586`, `:597`, `:613`.
3. **I/O.** `lower_bodies` over several machines; reads `module.proofs.scalar_block_invariants`, `ranked_scc`, `ObligationKind::{ArrivalInvariant, WellFoundedness, RankComparison}`.
4. **Decides.** A machine's `requires` is the head's invariant and each edge into the head owes an `ArrivalInvariant` (`:173`). Rows are in `(machine, head)` order as the codec requires (`:294`). `Strict` vs `Preserving` edge comparison depends on whether the jump passes the call argument (`:487`).
5. **Cases by shape.** Refusals by exact string (`:335`): "call recursion outside a terminal position" (twice: result used in `1 + f(n)`, and match scrutinee), "contract termination promise over a call cycle" (a `Promised` termination on a cycle without ranking), "contract guarded crash route on a call cycle". The comment at `:359` says a completed match does not make its scrutinee a terminal call. A promise on a caller of an unranked loop is refused (`:394`, `:573`).
6. **Same job elsewhere.** Stage 06 `arrivals.rs` admits the same rows (`P6/src/arrivals.rs:51`).
7. **Unused.** Tests only.
8. **Stated limits.** Test name `what_the_lane_does_not_serve_yet_on_a_cycle_is_refused_by_name` (`:335`) records that three cycle shapes are not served.
9. **Fail-open.** None seen. `a_promise_on_a_machine_that_reaches_no_cycle_is_served` (`:586`) pins that a promise is accepted when no cycle is reachable.

### P5/tests/block_values.rs (167 lines, 4 tests)

1. **Purpose.** Handoff test: lowering reads a block-valued `match` arm's own statements, then value, then ends the block locals' storage. Stage 05 side of the typing/checker triple named at `:7-10`.
2. **Shape.** Uses `support/dispatch_arms.rs` (`#[path]`, `:18`). Helper `returned_constants` (`:30`). Tests `:61`, `:73`, `:98`, `:120`.
3. **I/O.** Takes `PreparedTypedTrees` back with `into_inner`, edits one fact, re-runs stage 04 (`prepared_with`), lowers.
4. **Decides.** Block value is whatever the block's own statement bound (41 vs changed 43, `:92`). Removing the binding statement is refused with detail containing "local of another scope" (`:114`). A block's record local ends on the arm's edge into the join, as `DiscardRoot`; the root is identified by the operation result's structural type whose identity text contains `Counter` (`:163`, a source-spelling compare on identity text, test-side).
5. **Cases by shape.** One program, `MAIN` (`pick`, `scoped`).
6. **Same job elsewhere.** `subjectless_match.rs` uses the same support file; stage 06 has `tests/block_values.rs` (named in comment, not read).
7. **Unused.** Registered in `source-to-psi/Cargo.toml:157` (grepped).
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/tests/positional_field_reads.rs (233 lines, 2 tests)

1. **Purpose.** Lowering must read the field typing selected for each place of a positional pattern. Changing only `MemberSelection::Field` changes only that read.
2. **Shape.** `prepare` (`:66`), `lower` (`:89`), `returned_fields` (`:106`), `prepared_with` (`:154`), `select` (`:166`). Tests `:175`, `:189`.
3. **I/O.** Uses support from stage 03 tests by relative `#[path]` (`:17`). Reads `OperationKind::PrimitiveScalarRead` path field identity text.
4. **Decides.** The expected names come from `identities.text(field)` (`:146`): the test compares the field's declared name string (`zeta`, `alpha`), so it is a source-spelling compare in the test.
5. **Cases by shape.** Four fixed machines (`by_place`, `with_rest`, `in_arm`, `by_name`) over one `Pair`.
6. **Same job elsewhere.** `selected_operations.rs` `a_field_read_names_the_receiver_and_the_field_typing_selected` (`:126`) does the same for a non-pattern member read.
7. **Unused.** Registered at `Cargo.toml:147`.
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/tests/prepared_array_extents.rs (281 lines, 5 tests)

1. **Purpose.** Stage 05 must give each array the length stage 04 prepared for its extent and refuse an array passed where another length is required.
2. **Shape.** `lowered_extent` (`:50`), `parameter_extent` (`:67`), `assert_wrong_length` (`:101`), `set_evaluated_value` (`:146`), `set_extent` (`:154`). Tests `:175-281`.
3. **I/O.** Edits `ConstDeclaration.evaluated_value`, `materialized_initializer` and `FixedArrayLength` in the typed trees.
4. **Decides.** Keyed on declaration identity: an extent reads the evaluated value of the constant it *names* (`FixedArrayLength::ConstParameter { symbol }`), not the same-named constant in another module (`:227`). A named constant with `evaluated_value = None` gives `Unsupported` "array extent that names a constant with no evaluated array length" (`:262`), not a guessed length. A mismatch gives `Violation` with the exact sentence at `:109-110` and a span equal to `passed_argument(name)`.
5. **Cases by shape.** `east\N` = 2, `west\N` = 3 only; arrays of bytes.
6. **Same job elsewhere.** `P5/src/tests/array_views.rs` fixes `[u8;3]`.
7. **Unused.** Registered `Cargo.toml:152`.
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/tests/prepared_boolean_patterns.rs (224 lines, 3 tests)

1. **Purpose.** A `match` on a qualified Boolean constant branches on the constant value stage 04 prepared.
2. **Shape.** `lower` (`:32`), `tested_values` (`:49`), `prepared_with` (`:128`), `declaration_mut` (`:138`). Tests `:153`, `:181`, `:205`.
3. **I/O.** Edits `ConstDeclaration.evaluated_value` / `materialized_initializer`.
4. **Decides.** `tested_values` checks that each machine starts with one `BooleanConstant` plus `BooleanEqual` branch and returns 17 on equal, 23 otherwise (`:49-123`). A declaration whose `evaluated_value` contradicts its initializer is refused with detail containing "constant encoding and materialized carrier differ" (`:198`).
5. **Cases by shape.** Booleans only; shape fixed by comment `:45-48` ("Every machine must have the same shape").
6. **Same job elsewhere.** `prepared_scalar_values.rs` covers integer constants and patterns.
7. **Unused.** Registered `Cargo.toml:142`.
8. **Stated limits.** None.
9. **Fail-open.** None seen. The refusal test pins that a disagreement between two prepared facts is refused rather than one of them chosen.

### P5/tests/prepared_scalar_values.rs (386 lines, 4 tests)

1. **Purpose.** Stage 05 lowers each value stage 04 settled to that exact value: constant read, value pattern, folded negative operand, record constant in declaration order.
2. **Shape.** Uses `06/tests/support/source_handoff.rs` via `#[path]` (`:14`); helpers `returned_constant` (`:33`), `tested_pattern` (`:104`), `record_fields` (`:271`), `established` (`:302`). Tests `:45`, `:156`, `:203`, `:331`.
3. **I/O.** Small source programs compiled through stage 05 with `Program::of`; some twins edit expression nodes (`*trees.expression_table.expression_mut(left) = read`).
4. **Decides.** Constant `LEFT` read returns the declaration's value regardless of declaration order (`:46-66`). Grouped patterns `(0u8)`, `((0u8))` lower to identical operations (`:167`). `x > -1`, `x > (-(1))`, `x > 1 - 2` all land as one `Signed(-1)` constant of the formal's type, and `x > -y` computes `0 - y` at run time (`:203-268`). Record fields are established in declaration order regardless of the order written (`:331`); the record's identity is found by `identity.contains(r"path\(Pair\)")` (`:273`, `:283`) - a text compare in test code.
5. **Cases by shape.** `u8` / `i8` scalars and `data Pair [copy] { first: u8; second: u8; }` only.
6. **Same job elsewhere.** `selected_calls.rs`, `selected_operations.rs` share `source_handoff.rs`.
7. **Unused.** Registered `Cargo.toml:177`.
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/tests/selected_calls.rs (464 lines, 6 tests)

1. **Purpose.** Each call lowers to the callee typing selected, with arguments as written; each callee `requires` clause and crash route is owed at that call only.
2. **Shape.** `call` (`:44`), `outer_callee` (`:69`), `picks` (`:180`), `owed_by_each_call` (`:237`), `guarded_value` (`:356`). Tests `:121`, `:217`, `:283`, `:308`, `:382`, `:429`.
3. **I/O.** Source twins and an edit that copies a `CallExpression` from one call to another (`:143-154`).
4. **Decides.** Callee identity is keyed on typing's selection records: if `target_symbol` alone disagrees with the other selection records the call is refused, detail "call static application registers another declaration while reading the body of `use_it`" (`:173`). Qualified vs imported same-spelled `pick` stay distinct by module (`:217`). A call owes one `CallRequirement { invocation, clause }` per callee clause; the debt belongs to the outer call, and moving the clause moves the debt (`:283`). Operators dispatch to the machine whose domain holds the operands (`:429`).
5. **Cases by shape.** `u8` only; two-clause callee; one-route Trap guard `b == 0` (`guarded_value` accepts exactly one trap route with one predicate alternative, `:356-379`).
6. **Same job elsewhere.** Stage 06 `call_contracts.rs` reads the same invocation rows (see below).
7. **Unused.** Registered `Cargo.toml:172`.
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/tests/selected_operations.rs (461 lines, 6 tests)

1. **Purpose.** Reads, element indexing and arithmetic lower to the place, selector and policy typing chose; a machine's crash routes keep the formals the author named.
2. **Shape.** `field_read` (`:54`), `member_read` (`:102`), `element_read` (`:190`), `guard` (`:349`). Tests `:126`, `:223`, `:272`, `:376`, `:396`, `:442`.
3. **I/O.** Edits `MemberSelection`, `Indexed.index` and `Binary.policy`.
4. **Decides.** A field read names the receiver place and field typing selected (`:126`). An element read selects with its own index and owes its own bounds `OperationSafety` (`:223`). Policy `Exact` gives `ExactIntegerAdd` owing safety; `Wrapping` and `Saturating` owe nothing (`:272-345`). `a /! b` lowers to `TrappingInteger::Divide` and owes `CrashCoverage` for `Trap` at that operation (`:396`). A contract index `a[4]` of `[u8;4]` is refused at stage "lowering" with message "Violation: contract array index 4 is outside its fixed extent 4 while reading the signature of `bad`" (`:453`); `a[3]` lowers.
5. **Cases by shape.** `Cell`/`Pair` copy records, `[u32;4]` and `@[u32]` views, `u8` arithmetic.
6. **Same job elsewhere.** `P5/src/tests.rs` `a_cast_is_one_exact_conversion...` and `every_exact_operation_is_partial_by_the_policy_catalog` pin the same exact-vs-total split from the body side.
7. **Unused.** Registered `Cargo.toml:182`.
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/tests/subjectless_match.rs (139 lines, 3 tests)

1. **Purpose.** Each condition arm of a `match` without a subject lowers to its own branch condition in order, and no subject is read.
2. **Shape.** `branch` (`:35`), `with_operator` (`:77`). Tests `:94`, `:107`, `:119`. Uses `support/dispatch_arms.rs` (`GUARDED`).
3. **I/O.** Edits `BinaryOperator` of the first arm's condition; drops the `_` arm by shrinking `HandleSpan` (`:127`).
4. **Decides.** `a > b` becomes `IntegerLessThan(b, a)` and the subject `true` the reader substitutes is never made as a `BooleanConstant` (`:100-103`). A match with no subject and no `_` arm is refused with a detail containing "this match has no `_` arm: when every condition is false no arm applies" (`:136`).
5. **Cases by shape.** `larger` and `gap` over `u32`.
6. **Same job elsewhere.** none beyond the stage 03/06 twins named in comments.
7. **Unused.** Registered `Cargo.toml:162`.
8. **Stated limits.** None.
9. **Fail-open.** None seen.

### P5/tests/support/dispatch_arms.rs (239 lines)

1. **Purpose.** Shared programs and builders for block-valued arm tests and subjectless match tests: `MAIN`, `ESCAPE`, `GUARDED` (`:38`, `:50`, `:61`); `typed_source` (`:76`), `Prepared` (`:138`), `prepare_source` (`:195`), `machine` (`:227`), `block_binding` (`:99`), `subjectless_matches` (`:123`).
2. **I/O.** Runs lexer, parser, resolver, typer, stage 04 through test-support APIs; `#![allow(dead_code)]` (`:15`). Imports the frontend helper by an eight-level relative path (`:17`).
3. **Decides.** `machine` finds a lowered machine by `symbols.name(...) == name` (`:237`): a name compare, in test code. `block_binding` finds a block by local name (`:113`).
4. **Same job elsewhere.** `Prepared` is repeated almost field for field in `support/sum_dispatch.rs:71` and `positional_field_reads.rs:35` (read; three copies of the same "typed to prepared to lower" scaffold).
5. **Unused.** `ESCAPE` is defined here; I did not find a use in this unit (grepped no further; `ESCAPE` is described as "refused", so likely used by stage 06 tests, inferred).
6. **Fail-open.** None seen.

### P5/tests/support/sum_dispatch.rs (271 lines)

1. **Purpose.** One program of two unrelated sums `Left` and `Right` whose cases spell alike, plus lookup helpers over the lowered module.
2. **Shape.** `MAIN` (`:36`), `ENTRIES` (`:69`), `prepare` (`:119`), `sum` (`:178`), `dispatch` (`:203`), `dispatch_edges_mut` (`:227`), `payload_loan` / `payload_loan_mut` (`:247`, `:262`). The `_mut` helpers exist so stage 06 tests can forge a changed edge (inferred; comment at `:226`).
3. **Decides.** `sum` finds a sum by `identity.contains(r"path\(Left\)")` (`:179-187`): a text compare on identity.
4. **Unused.** `dispatch_edges_mut` and `payload_loan_mut` are not used by `variant_parent_identity.rs` (read). Count outside the unit not grepped; suspected used by stage 06's `tests/variant_parent_identity.rs`.
5. **Fail-open.** None seen.

### P5/tests/variant_parent_identity.rs (194 lines, 2 tests)

1. **Purpose.** Stage 05 must say which sum each match arm selects a case of. A `StructuralCaseId` is unique per module and an edge or `Case` path step names the sum through it.
2. **Shape.** `both_sums` (`:40`), tests `a_dispatch_lists_the_cases_of_the_sum_its_subject_holds` (`:85`) and `a_payload_loan_names_the_case_of_the_sum_it_borrows_from` (`:173`).
3. **Decides.** Keyed on identity: dispatch edges list exactly the declared case ids of the subject's own sum; payload fields of an edge are the ids of its own case (`:99-114`); a loan path begins `Case(small.id)` then `Field(identity)`; since `Left::Small.value` and `Right::Small.value` spell alike, only the `Case` id separates them (`:193`).
4. **Cases by shape.** `Left`/`Right` with `Small(value)` and `Nothing`.
5. **Stated limits.** None.
6. **Fail-open.** None seen. Comment `:22` points to the stage 06 test for what happens when the id belongs to the other sum (not read).

---

## Stage 06 (28 files)

### Examples (6 files, 2/5/16/15/6/3 lines)

All six are one-line wrappers that `include!` a harness from `tools/benchmark/...` (outside this unit).
- `P6/examples/machine_checks.rs:2` includes `tools/benchmark/machine-checks/harness.rs`; `machine_checks_mimalloc.rs:2-5` sets `mimalloc::MiMalloc` as global allocator then includes the same harness.
- `proof_preparation.rs:1-16` and `proof_preparation_timing.rs:1-15` mount `../src/known.rs` and `../src/prove.rs` with `#[path]` and `#[allow(dead_code)]`, define a local `mod checking { pub(crate) use super::known; }`, set `COUNT_ALLOCATIONS` true / false, and include `tools/benchmark/proof-preparation/harness.rs`.
- `service_correspondence.rs:1-6` (timing; includes the harness and, under `cfg(test)`, `tests.rs`) and `service_correspondence_allocations.rs:1-3` (allocation count).
- All six are registered in `source-to-psi/Cargo.toml:247-303` (grepped, 6 matches).
- Stated limit: `proof_preparation.rs:9` says "The measured implementation now belongs to source settlement" (a stale name: no crate by that name in my reading). The two proof-preparation examples compile `known.rs` and `prove.rs` a second time as separate modules; `known.rs` refers to `crate::checking`-style paths through the local `checking` shim.
- Fail-open: none (benchmarks).

### P6/lib.rs (32 lines)

Crate root: `#![forbid(unsafe_code)]` (`:1`), `#[cfg(test)]` re-aliases of upstream crates (`:3-20`) as `lexing`, `parsing`, `source_inputs`, `source_symbols`, `typing`, `resolution`, `source_state`, `lowering`, `preparation`; mounts the real code `#[path = "src/lib.rs"] pub mod checking` (`:21-22`); test-only `prepare_fixture` (`:30`) calls `preparation::prepare_typed_trees(typed, &[], &[])`. Everything inside the crate refers to itself as `crate::checking::...`. Used by every file below.

### P6/src/arrivals.rs (115 lines)

1. **Purpose.** Admits one authored/inferred block invariant row (`ScalarBlockInvariant`) for a call-cycle loop head before its predicate is used as knowledge.
2. **Shape.** `admit(machine, lowered, tables, invariant) -> Checked<()>` (`:51`), private `block` (`:35`).
3. **I/O.** Reads `TerminalMachine` blocks, `MachineTables.values`, `.edges`, `.obligations`.
4. **Decides.** Four refusals (all `RefusalReason::ForeignIdentity`): header not a block or is the entry (`:66`); a predicate value not a `MachineParameter` or a `BlockParameter` of the same header (`:71-79`); an edge into the head with other than exactly one arrival entry (`:80-94`); an arrival naming another edge or a non-`ArrivalInvariant` obligation (`:95-113`). Keyed on identity (`ValueOrigin`, `EdgeId`, `ObligationId`).
5. **Cases by shape.** Predicate scope limited to machine formals and the head's own parameters (comment `:44-46`).
6. **Same job elsewhere.** `joins::in_scope` (`joins.rs:178`) states the scope check for an *inferred* predicate; `arrivals::admit` does it for an authored one (comment `joins.rs:177`).
7. **Unused.** `admit`: 1 call site, `joins.rs:89` (grepped).
8. **Stated limits.** Header comment `:19-24`: the predicate is "the producer's, never inferred here".
9. **Fail-open.** None seen: every unmatched case returns `Err`.

### P6/src/branch_warnings.rs (293 lines)

1. **Purpose.** Advisory "impossible arm" report: for each authored branch, asks whether `Falsehood` is derivable from the knowledge at that arm's entry and, if so and the preceding decision point was not already contradictory, reports the arm with its premises. It never discharges an obligation (`:1-3`).
2. **Shape.** `Deferred` (`:24`, mutex of `Prepared` plus `OnceLock<Vec<ImpossibleArm>>`), `Analysis` (`:90`), `Analysis::run` (`:170`), free `refutation` (`:239`).
3. **I/O.** Inputs: `Graph`, `PropositionContext`, `MachineTables`, `TerminalMachine`, `&[Branch]`. Output: `Vec<ImpossibleArm { block, premises }>`. Runs `Prover::prove(&Falsehood)` then `verify_obligation` with the kernel.
4. **Decides.** Keyed on program point (`ProgramPoint::BlockEntry` or `AfterOperation`, `:121-132`) and kernel acceptance. Premises are the knowledge rows whose index appears in the certificate's accepted assumption list, and a premise is kept only if its `proposition` equals the supplied one at that index (`:278-284`); a `BranchCondition` whose edge enters this very block is excluded (`:212-217`).
5. **Cases by shape.** Probe obligation id is hard-coded `ObligationId::new(1)` (`:247`); evidence identity reuses `probe.get()` (`:267`).
6. **Same job elsewhere.** `Prover::new` / `verify_obligation` is the same pair `formation.rs::Scope::prove` uses via `search_certificate` / `verify_certificate` (`formation.rs:440-461`) and `joins::argument_bounds`.
7. **Unused.** `Deferred::new`, `materialize` (called in `lib.rs:727`, grepped); `query_count`/`searches` are `cfg(test)`.
8. **Stated limits.** `:167-169`: "Missing/refused evidence stays quiet: failed search does not prove that an arrival is impossible."
9. **Fail-open.** Classified "stated": `refutation` returns `None` on any failure (`:247-272`), meaning no warning; the effect is quiet, never acceptance of a program. A poisoned mutex is recovered with `into_inner` (`:61-64`), stated at `:59-60`.

### P6/src/call_contracts.rs (603 lines)

1. **Purpose.** "One public contract source for every lowered invocation" (`:1`): given an invocation row it finds the actual lowered call and builds the public contract the caller is judged against, for a declared machine, a selected callable value, or a boundary declaration.
2. **Shape.**
   - `Inputs` (`:17`) and `Inputs::declared` (`:32`): callee, contract, required contract, actuals (scalar, structural, erased), requirement obligations, claim transfers, completion receipts, crash continuations.
   - `PublicContract` (`:127`), `Guarantees` (`:81`), `ScalarParameters` (`:98`), `ResultFormal` (`:118`).
   - `Invocation` (`:171`) with `structural_inputs` (`:177`).
   - `TailCall` (`:198`), `TailCallSites` (`:207`), `tail_call` (`:233`).
   - `operation_inputs` (`:333`), `inputs_at` (`:356`), `invocation` (`:416`), `terminator_site` (`:60`).
3. **I/O.** Reads `TerminalModule` (machines, boundary declarations, callable contracts, envelopes), `MachineTables`, `InvocationRow`. Produces the `Invocation` view; mutates nothing.
4. **Decides.**
   - Match of the table row to the lowered call is by `InvocationSite` identity, and duplicates are refused: "one invocation site has multiple lowered call targets" (`:406`).
   - `row.callee != inputs.callee || row.required_contract != inputs.required_contract` refuses (`:429`).
   - For `Callee::Selected` the contract is the one in the operand value's `ScalarType::Callable(contract)`; a `rigid` value or a retained-contract mismatch is refused (`:462`).
   - For a boundary: completion kind must match the site kind (`NormalReturn` with `Operation`, `External` with `ExternalCompletion`, `:496-519`), published may-suspend and may-block must equal the envelope (`:527-534`).
   - A tail call must find exactly one jump with that edge and exactly one loop head and the head's scalar and structural formals must equal the callee's on type, position, multiplicity, access and qualification sets (`:287-319`).
5. **Cases by shape.**
   - Boundary calls with any `additional_input_qualifications`, structural result qualifications, `normal_qualifications`, `content_guarantees`, `result_reference_regions` or `program_local_root_introductions` are refused `Unsupported` ("boundary call needs its structural qualification proof", "boundary contract needs structural domain, reference or content publication", `:540-569`).
   - External completion with a crash route is refused "external completion that declares a crash route" (`:505-511`).
   - A tail call with any erased argument or erased formal is refused (`:289-291`).
6. **Same job elsewhere.** The row-to-site lookup `inputs_at` repeats a block/operation scan; `crashes::call_site` (`crashes.rs:179`) and `ceilings::judge_axes` (`ceilings.rs:1170`) each call `invocation`. Users of `call_contracts::`: 36 occurrences across 10 stage 06 files plus 1 unrelated hit in `psi-to-native/.../target-vocabulary/src/lib.rs` (command: Grep count of `call_contracts::` over `source-rust`). So the "one source" claim holds for the stage 06 call-judging files I could see.
7. **Unused.** `Guarantees::clause` is `cfg(test/test-support)` only (`:86`). `terminator_site` I did not grep (suspected used by `minted.rs`/`lib.rs`; open).
8. **Stated limits.** `:520-522`: "suspension needs a separate continuation/carry producer, checked with all call forms below". `:535-537`: "Additional provider obligations still need their independent proof and publication."
9. **Fail-open.** None seen. The only `Option` that could weaken is `crash_continuations` (`:49-53`): `Some(..)` only for a declared machine callee, `None` for a boundary; `crashes::call_site` then falls back to the contract's own routes (`crashes.rs:205-219`). Traced: both branches are exact.

### P6/src/callable_arrivals.rs (119 lines)

1. **Purpose.** Checks ordinary calls to installed continuations from a canonical `CallableArrivalWitness`, using the same discharge and kernel as other calls (`:1-4`).
2. **Shape.** `check(witnesses, activity)` (`:10`), `check_witness` (`:32`).
3. **I/O.** In: witnesses (each `Checked<CallableArrivalWitness>`). Calls `crate::checking::discharge` (`:54`). Out: `Vec<UnprovedObligation>` or the first refusal.
4. **Decides.** A witness is refused if a nominal premise's path root differs from its occurrence symbol, the member span is absent, or the required-input row's occurrence differs (`:37-52`). For each verdict `Err` with `RefusalReason::Unproved(cause)` it records an unproved obligation; any other refusal reason propagates (`:70-72`). Any loan `use`/`window` failure propagates; any `conflicts` refuses with "logical call witness needs its complete loan judgment" (`:109-116`).
5. **Cases by shape.** Diagnostic machine identity is the constant `MachineId::new(1)` (`:36`).
6. **Same job elsewhere.** `ceilings.rs` and `verdict.rs` also build `UnprovedObligation` (`verdict.rs:1233`, `:1294` per grep).
7. **Unused.** `check`: 1 call site, `verdict.rs:1564` (grepped).
8. **Stated limits.** `:24-25`: "A missing part of any binding's premise theory cannot turn failed search under the other premises into a source rejection."
9. **Fail-open.** None seen. `elaboration.requirements.contains(&machine)` skips bodyless requirement machines (`:57`); traced: they have no obligations to judge.

### P6/src/ceilings.rs (1,480 lines)

1. **Purpose.** Checks that every call fits the permissions its caller publishes, on five axes: service reach, synchronous entries, suspension, blocking, termination (`Axis::ALL`, `:33`). Also validates the shape of every published service row and callable envelope.
2. **Shape.**
   - `Axis` (`:25`), `Permission` (`:43`), `Excess` (`:146`).
   - `Envelope` enum (`:51`): `Runtime`, `Generic`, `Abstract` (source-only requirement ceilings, "not encoded, sealed or executable" `:54`), with `reach`, `entries`, `suspension`, `blocking`, `termination`, `row(axis)`.
   - `machine_envelope` (`:215`), `envelope_for` (`:258`), `Contributions` (`:314`), `Calls` (`:390`).
   - `judge` (`:931`), `judge_declared_rows` (`:941`, test-support), `judge_axes` (`:947`), `contain` (`:1290`), `tail_contribution` (`:1325`), `fixed_refinement_reach` (`:1396`), `well_formed_reach` (`:165`), `validate_check_form_envelopes` (`:759`), `validate_requirement_envelopes` (`:827`).
3. **I/O.** Reads `Elaboration` (module, tables, `check_form_envelopes`, `generic_callable_declarations`, `requirement_ceilings`, `installation_bound_refinement`) and `terminal_semantics::callable_values`. Output: `Vec<Excess>` or a `ForeignIdentity` refusal.
4. **How it decides.** Keyed on service identity (`ServiceId`), parameter coordinate and `CallableContractId`, never on names. Containment is `outside = contributes minus ceiling` per axis by binary search (`:1304-1308`). A callable value invoked by a holder is resolved through `resolve_callable_value` to descriptors and symbolic formals; a descriptor must carry the same contract and an existing machine (`:493-507`), a formal input must carry the same contract (`:508-513`), and a formal maps to `Entry::Parameter(authored coordinate)` (`:475`). Machine envelope selection order: check-form envelope, runtime envelope, generic declaration, then abstract requirement ceiling; overlap of check-form and runtime is refused (`:224`).
5. **Cases by shape.** Listed refusals with their text:
   - Nested structural parameter path non-empty: "invokes needs the exact nested structural parameter path" (`:457-458`).
   - Structural callable actual not a primitive callable carrier: "invoked structural actual needs an exact nested callable parameter path" (`:580`, `:680`).
   - Opaque callable with a ceiling but no origin: "invoked opaque callable has a contract ceiling but no retained entry origin" (`:518`).
   - A nested callable whose envelope entry is a `Parameter`: "nested callable invocation needs its exact invocation actual mapping" (`:749`).
   - "open abstract service reach cannot establish a finite caller ceiling" (`:1190`, `:1197`, `:1344`), same for open invocation ceiling (`:1213`, `:1356`).
   - "callee termination premises need their exact invocation proof" when the callee's termination is conditional and the caller ceiling is `Terminates` (`:1236`, `:1368`).
   - "installation refinement reach through a selector value" (`:1448`).
6. **Same job elsewhere.** `well_formed_reach` is also called by `static_contracts.rs:53` (grepped). The reach-row well-formedness check at `:173-202` duplicates nothing in this unit. The tail-call reach and entries computation in `tail_contribution` (`:1325`) repeats most of the per-invocation block in `judge_axes` (`:1182-1253`): same four steps (reach, entries, `operational`, conditional-progress refusal, installation-bound override, `contain`). Both are in this file.
7. **Unused.** `judge`: callers `verdict.rs:1363` and 11 test sites (grepped listing). `well_formed_reach`: 8 call sites (6 here, 1 `static_contracts.rs:53`, 1 definition). `judge_declared_rows`: 1 non-test caller, `refinement.rs:81`. `Excess`, `Axis`, `Permission` are consumed by `verdict.rs` (`:639-739`).
8. **Stated limits.** Comment `:54-55` "These source-only signatures are not encoded, sealed or executable. An unknown bound stays unknown when its other contract axes are known." Header `:3-4`: invoking a callable holder "also enters that exact formal". Doc on `judge_declared_rows` `:937-939`: "claiming executable acceptance" is not given; the caller still owes operational refinement.
9. **Fail-open risks.**
   - `contain` at `:1300-1302`: `let Some(ceiling) = ceiling.row(axis) else { continue; }` skips an axis whose caller ceiling is open (reach `Inferred`/`Open`, entries `Inferred`, suspension/blocking `None`). Classified **traced**: for reach and entries an open ceiling means the caller publishes no bound (an inferred caller cannot exceed what it infers); where the callee is open and the caller finite the code refuses first (`:1189-1200`, `:1212-1216`); for suspension and blocking `operational_from` refuses when the callee value is unknown and the caller's is known (`:329-335`), reached only if the axes include them (`:400-404`). So `judge_declared_rows` (axes Reach and Entries only) skips the operational check by design (doc `:937-939`).
   - `Contributions::operational` ignores an unknown (`None`) suspension or blocking bound of the callee when the caller's bound is also unknown (`:343-348`; comment `:341-342`). Stated.
   - `fixed_refinement_reach` may replace the contribution of a caller in installation-bound refinement (`:1238-1253`, `:1371-1380`): when the reached reach exceeds the ceiling, the reach is recomputed by walking declared callees and keeping only fixed reaches (`:1413`, `:1420`). Classified **open** (no comment says why a recomputed row replaces an exceeding row; the function doc `:1393-1395` says "Only the proof of an abstract installation requirement may leave its child rows conditional").
   - `needs_values` (`:1106-1151`) gate: callable value resolution is prepared only when some invocation, envelope or requirement row names a parameter entry or selected callee. If none do, `values` is `None` and a later `context.resolve` refuses (`:429-431`), so a missed condition fails closed.

### P6/src/crashes.rs (313 lines)

1. **Purpose.** Builds the proposition that a `CrashCoverage` obligation must prove: the site's guard implies the machine's published same-cause routes.
2. **Shape.** `coverage_goal` (`:57`), `Guard` enum (`:103`: `Path`, `Route`, `Unless`), `operation_site` (`:125`), `call_site` (`:179`), `block_site` (`:225`), `coverage` (`:287`).
3. **I/O.** Reads module, machine, tables, scalar types; `terminal_semantics::trapping_integer_semantics`. Returns a `Proposition`.
4. **Decides.** Guard shapes: `Crash` terminator uses its `site_guard` clauses as path facts; a call uses the callee's route for the cause with the call's actuals substituted (`crate::checking::substitute`, `:209-217`); a trapping primitive uses its negated definedness (`Unless`). The goal combines with the machine's coverage for the cause: published `Truth` -> `Truth`; no route -> `Falsehood` (so `Guard::Path` must be shown unreachable; `Unless(c)` gives `c`); else `Implication` or `Disjunction` (`:75-99`, `:287-312`). Keyed on `CrashCause` and site identity.
5. **Cases by shape.** A trapping primitive has only `Trap`; any other cause is `ForeignIdentity` (`:151-157`). A site must have exactly one invocation row (`:196-202`).
6. **Same job elsewhere.** The route-with-actuals substitution is the one used by `ceilings` for crash-free call checks? Not seen. `crashes` and `call_contracts` both use `crash_routes`.
7. **Unused.** `coverage_goal`: 1 call site, `lib.rs:2277` (grepped).
8. **Stated limits.** Header `:11-15`: a machine that publishes no route for the cause "owes `Falsehood`" and "this prover does not do" showing unreachability, "so such a site is unproved".
9. **Fail-open.** `Guard::Route` with `coverage == Truth` returns `Truth` (`:92`): **stated** (header: an unconditional published route covers every site). `Unless` with `coverage == Truth` returns `Truth` (`:77`): **stated** (header "an unconditional route owes nothing"). `Guard::Path` with an empty `site_guard` and coverage `Truth` returns `Truth` (`:82-87`): traced to the same rule. Nothing else.

### P6/src/describe.rs (609 lines)

1. **Purpose.** Turns an unproved obligation into a message: what was not proved, with authored names. Never states that something is false (`:17-20`).
2. **Shape.** `unproved` (`:40`), `established_predicate_arrival` (`:218`), `head_arrival` (`:270`), `crash_coverage` (`:333`), `indexed_extent` (`:389`), `operation_safety` (`:443`), `premise` (`:517`), `owner` (`:569`), `primitive` (`:588`).
3. **I/O.** Reads `Elaboration.names` (`MachineNames`), tables, module; returns `String`.
4. **Decides.** Keyed on `ObligationKind` and identities; names come from the names table and `SourceOrigin` spans (`names.binder(span)`, `names.arguments(span)`). Falls back to identity-only text when the names table is missing (`:102-105`, `:126`). `head_arrival` takes the first `names` entry whose `arguments(span)` matches the call span (`:291-294`): a span match across all machines.
5. **Cases by shape.** `operation_safety` handles `ExactIntegerAdd/Subtract/Multiply/ShiftLeft/ShiftRight/Divide/Remainder` and `IntegerExactCast`; any other kind gets "cannot prove operation safety" (`:510`). `primitive` handles `bool`, float (always printed `float`), callable, integer.
6. **Same job elsewhere.** none.
7. **Unused.** `unproved`: callers `verdict.rs:1233,1294` and `callable_arrivals.rs:92`; `owner`: `verdict.rs:162,205,232`; `premise`: `verdict.rs:209` (grepped).
8. **Stated limits.** The exact-arithmetic message cites "decision 17" (`:487`), a number that AGENTS.md says is a mutable queue position (grepped, 1 hit in `describe.rs`, 1 in `tests.rs`).
9. **Fail-open.** None seen (text only). One caveat: `ObligationKind::ArrivalInvariant` for a cast uses `established_predicate_arrival` keyed on `predicate_establishments` by destination block (`:232-237`).

### P6/src/entry_inputs.rs (1,693 lines) plus 3 submodules

1. **Purpose.** For a private helper with no written `requires`, infers an entry requirement from what every caller establishes at its call, installs it as an ordinary entry requirement, and mints one `CallRequirement` obligation at every real call. Describing an arrival is inference, "never evidence": the final discharge checks all calls (`:7-11`). A written contract "remains the whole input promise" (`:4-5`).
2. **Shape.** `infer` (`:48`), `describe_calls` (`:110`), `CallSites` (`:409`, built by `CallSites::new` `:419`), `Arrival` (`:391`), `LoopHead` (`:399`), `derived` (`:686`), `known_at` (`:752`), `describe` (`:793`), `InputBindings` (`:949`), `RenamedTerm` (`:874`), `canonical` (`:1064`), `canonical_parts` (`:1088`), `formed` (`:1224`), `require` (`:1291`). Submodules below.
3. **I/O.** In: `ReplayScope` (mutable elaboration), `Unreached`, `Inhabitation`, `ProofSearchActivity`. Out: rewrites `module.machines[callee].contract.requires`, adds obligations, invocation `requirements`, operation and terminator `requirement_obligations`, `names.requires`, and `proofs.scalar_block_invariants` rows for every loop head of a recursive component (`:1612-1692`).
4. **How it decides.**
   - Eligibility is `elaboration.private_inputs` (`:421`); a machine that a generated owned-cleanup action invokes is made non-private so disposal never demands anything of its caller (`:424-440`).
   - Processing order: ordinary callers before helpers, a ready queue by remaining incoming call count (`:61-106`).
   - What each arrival says: `describe` (`:793`) states every available fact, expanded by `expand_value` and bound to callee formals through `InputBindings::value`, which accepts a value if it equals a call actual or, failing that, if some value equal to it (through caller `Equal(Value, Value)` facts, `:1001-1031`) is an actual; otherwise `Err(Unbound::Unproved)`. A fact whose atom cannot be bound becomes `Truth` via `joins::project` (monotone weakening, `joins.rs:349-364`).
   - Arrivals join as a disjunction (`joins::connective(arrivals, false)` `:82`) then `canonical`ised; the clause is added only if `formed` says its arithmetic forms (`:1224`).
   - `integer_cases::exhaustive` removes disjunctions that cover all orders of two integer terms (`:1234-1246`).
   - `derived` adds literal bounds from `prover.literal_bounds(here)` for integer actuals (`:736-746`).
5. **Cases by shape.**
   - `integer_cases.rs:16` handles only comparisons `Equal/LessThan/LessOrEqual` whose operand terms are `Value`, `Integer`, `IntegerField` or `Application` of one fixed-width non-address integer type (`:116-134`; comment `:122` "A comparison of computed terms could owe independent formation").
   - `InputBindings::field` and `::case` always refuse ("a caller storage path is not a callee input value", `:1048`; case `:1058`): storage facts cross only through `fields.formal_observation` / `field_crossings`.
   - `CallSites::new` accepts only call sites of kind `Operation` or `InvokeNoNormal` (`:606-641`); any other site kind that targets a private callee is refused "a private invocation has no call position".
   - A loop head may carry only one member correspondence (`:510-516`).
6. **Same job elsewhere.**
   - `joins::connective` is the shared AND/OR builder (`joins.rs:329`); `entry_inputs::canonical_parts` (`:1088`) duplicates the same flatten / unit / zero logic with extra canonical ordering and dedup (`:1088-1205`). Both exist; `canonical_parts` calls `joins::connective`'s logic by hand. Suspected tie to `result_summaries::infer` (`result_summaries.rs:505-583`, not read) which calls `known_at`, `describe` and `canonical` from here (grepped).
   - `entry_inputs::known_at` (`:752`) and `entry_inputs::derived` (`:686`) both walk `tables.knowledge` with `graph.dominates(row.available_from, point)` (also at `joins::describe` `:652-654`).
7. **Unused.** `infer`: caller `lib.rs`-level `ReplayScope::infer_inputs` (`formation.rs:153-159`). `known_at`: 2 local calls plus `result_summaries.rs:505`. `describe`: 1 local (`:272`, `:333`) plus `result_summaries.rs:507`. `canonical`: 5 local plus `result_summaries.rs:576,583`. All from the grep listing.
8. **Stated limits.** `:13-15`: "Only facts about exact actuals cross here." `:1217-1223`: "Nobody wrote the condition, so a conjunct that cannot form ... is not the program's error: leave it out." Comment `:1230-1232`: universal disjunctions omitted.
9. **Fail-open risks.**
   - `formed` drops an inferred conjunct that cannot form (`:1267-1288`, and returns `None` for `Truth`). **Stated** (`:1219-1222`, "Keeping fewer conjuncts only gives the body fewer premises, and every arrival still proves what is kept"). Effect: the helper loses a premise, which can only make a later body proof fail, never pass.
   - `forms` swallows every error except `ForeignIdentity` as "does not form" (`:1260-1261`). **Traced**: a non-`ForeignIdentity` refusal during formation becomes "omit the conjunct", not accept.
   - `integer_cases::exhaustive` returning true removes a disjunction from the inferred requirement (`:1234`). **Traced**: it returns true only when the arms cover every order (`LESS|EQUAL|GREATER`) of every compared pair, so the dropped condition is a tautology; the test at `integer_cases.rs:148-174` pins the conjunction handling.
   - `describe` skips callable-typed formals (`:806-808`, comment "Callable identity and selection have no mathematical equality").
   - `unreached.mark_formal(callee, formal)` records formals the integer rules do not reach (`:364-366`); effect on later proof not read here (open).

#### P6/src/entry_inputs/inferred_calls.rs (155 lines)
1. **Purpose.** Remembers which clauses were added by inference and which call obligations were minted, so canonical reordering does not lose them; `checked` turns the record into `InvocationPrivateInputs`.
2. **Shape.** `InferredCalls` (`:14`), `add` (`:30`), `call` (`:36`), `checked(program, discharge)` (`:47`).
3. **Decides.** Re-finds each clause's canonical ordinal by equality of proposition in `machine.requires` (`:54-71`); re-verifies each retained call's row, site, callee and requirement obligation (`:99-135`). A call is kept in the result only if its verdict is `Discharged::Certificate` (`:136-144`): "Only final, discharged calls appear" (`:2`).
4. **Fail-open.** None seen: any mismatch returns a `ForeignIdentity` refusal (`mismatch`, `:76`). A call whose obligation is not discharged by certificate is silently left out of the retained record (`:136-144`); this narrows what is published, not what is accepted. Classification: traced.

#### P6/src/entry_inputs/integer_cases.rs (175 lines)
1. **Purpose.** Decide whether a disjunction of integer comparisons is exhaustive (covers all orders), so an inferred requirement can omit it.
2. **Shape.** `exhaustive` (`:16`), `Orders` (`:30`), `require_false` (`:37`), `truth` (`:75`), `comparison` (`:109`); one unit test (`:148`).
3. **Decides.** Keys on operand pair `(left, right)` ordered, with a bitmask of `LESS=1, EQUAL=2, GREATER=4`. A conjunction with exactly one unknown part treats that part as required false (`:41-52`). Bounded by "each new fact removes a possible order" (`:12-15`).
4. **Cases by shape.** Only integer fixed-width types; addresses excluded (`:119`); only `Value`, `Integer`, `IntegerField`, `Application` terms (`:123-131`).
5. **Fail-open.** None seen: `comparison` returns `None` for anything else, making `require_false` return `false` (not exhaustive).

#### P6/src/entry_inputs/recursive.rs (196 lines)
1. **Purpose.** Transports inferred inputs along the actual edges of a recursive component (frame-reusing jumps).
2. **Shape.** `Transfer` (`:19`), `Transfers` (`:24`), `infer` (`:46`), `enqueue_formed` (`:153`), `Missing` (`:173`).
3. **Decides.** For each incoming predicate conjunct from every arrival, the clause is pushed through each transfer edge by substitution; an image is offered only if it holds on *every* arrival of the callee or that arrival is `Falsehood` (`:119-130`); `Missing` refuses every unnamed value, field or case (`:173-196`: "input is not preserved across this transfer").
4. **Stated limit.** `:105-107`: termination argument that "the set of distinct images of the finite initial predicates is finite".
5. **Fail-open.** None seen. `.expect("member indexed")` appears 9 times (panic, not accept).

### P6/src/external_providers.rs (251 lines)

1. **Purpose.** A native provider candidate becomes a checked conformance only after signature, operational envelope and kernel refinement judgments (`:1-2`).
2. **Shape.** `view` (`:13`), `header_matches_boundary` (`:25`), `publish(&mut Elaboration)` (`:53`).
3. **I/O.** Reads `elaboration.external_provider_candidates`, module boundaries; writes the sorted, de-duplicated `module.interfaces.external_providers` (`:249`).
4. **Decides.** Per candidate: both crash policies must be `Closed` (`:66-74`); the required boundary header must equal the candidate's (`header_matches_boundary`) with same identity, completion, parameter and result types and envelope projections (`:78-97`); `callable_envelope_refines(required, supplied)` yields `Conflict` on failure (`:124-128`); structural qualifications of required and supplied must be equal (`:140-167`); each refinement goal from `contract_refinement_goals` must derive through `derive_contract_obligation` with ids `1..n` (`:180-202`); a native boundary header must match the candidate exactly or be absent in both (`:212-236`).
5. **Cases by shape.** Refuses `Unsupported` when the required boundary has any of `normal_qualifications`, `scalar_domain_requirements`, `reference_outputs`, `content_guarantees`, `result_reference_regions`, `program_local_root_introductions` (`:98-110`).
6. **Same job elsewhere.** Header-vs-boundary equality repeats in two places in this file (`:89` for the required header, `:222` for the supplied).
7. **Unused.** `publish`: 1 call site, `verdict.rs:1400` (grepped).
8. **Stated limits.** `:138-139`: "The header reader currently publishes these as exact source shapes."
9. **Fail-open risks.** `.zip` at `:140-144` pairs required and supplied structural parameters and stops at the shorter list; equal length is not asserted here. Classified **open** for this file (the equal-length check is probably inside `contract_refinement_goals`, which I did not read). The zero-candidate and zero-published case returns `Ok(())` (`:54-58`): stated by structure (nothing to publish).

### P6/src/formation.rs (1,385 lines)

1. **Purpose.** "A condition cannot justify arithmetic needed to state that condition" (`:1`): before any contract clause enters proof knowledge, every partial computation in it must be proved total from the clauses already formed. Also owns the `ReplayScope` through which later inference edits the elaboration.
2. **Shape.**
   - `ReplayScope` (`:27`), `prepare_replay` (`:33`), `FormedModule` (`:127`), `ReplayScope::{infer_inputs, infer_results, require, summarize, publish_rankings, finalize_callable_refinements, checking}` (`:153-281`).
   - `Scope` (`:352`) with `ClauseBindings` impl (`:560`): `value`, `field`, `structural_read`, `extent`, `byte_predicate`, `case`, `computed`; `Scope::clauses` (`:510`), `prove` (`:431`).
   - Free: `owes` (`:852`), `contract` (`:890`), `module` (`:975`), `data_domains` (`:1302`), `add_projected_defaults` (`:1223`), `DataSubject` (`:1142`).
3. **I/O.** In: `TerminalModule`, contracts, `predicate_sources`. Out: `Checked<()>`; clauses become `Facts.premises` in a throwaway `Scope`, nothing persists.
4. **How it decides.**
   - `owes(clauses)` is true if any term is a `ByteSequencePredicate`, a `StructuralRead`, or a term for which `CanonicalScalarGoal::for_term` is not `Ok(None)` (`:852-865`). Only clauses that "owe" fail the whole entrance when they do not bind (`:533-537`).
   - A clause joins premises only after all its terms form; pending clauses are retried so independent bounds can be in either order; a mutually pending cycle makes no progress and refuses (`:507-546`).
   - Each partial computation is proved by `search_certificate` plus `verify_certificate` over the facts so far (`:440-461`) - the same prover and kernel as ordinary goals.
   - `Version::Entry` vs `Current` of a storage observation is chosen by `root.has_entry`, `completion`, and `live = root.borrowed || path contains Referent` (`:628-634`, `:724-729`, `:811-816`).
   - Root lookup by `PlaceId`, ambiguous root refused (`:465-477`).
5. **Cases by shape.**
   - `descriptor_extent`: only `ElementView` or `ByteSequence::BorrowedView` carriers; completion extent of a borrowed or referent descriptor refused "completion extent needs its exact storage version" (`:389-402`).
   - `field`: a `FixedIndex` step is allowed only on a `FixedArray` prefix ("contract view index needs an established extent observation", `:601-618`); `case` always refuses "partial contract under a case requires its exact case observation" (`:829`).
   - `byte_predicate`: refuses a `Case` segment, and a `FixedIndex` unless the array is literal-length and the index is in bounds (`:754-777`).
   - `module()` (`:975`): refuses qualified mathematical predicate inputs "qualified mathematical predicate input is not supported" (`:1025`).
   - `data_domains`: a cyclic inline carrier refused (`:1234-1238`).
6. **Same job elsewhere.** `Scope::prove` and `joins::argument_bounds` (`joins.rs:581`) and `branch_warnings::refutation` (`branch_warnings.rs:239`) each build a `Prover` and call the kernel; three separate call sites of the same pair (inferred, not diffed). `formation::contract` is called from `entry_inputs::formed` (`entry_inputs.rs:1257`) as the single "does this clause form" oracle.
7. **Unused.** `prepare_replay`: pub, entry per crate-map (`P06/src/formation.rs:33`); not grepped here. `FormedModule::nominal_calls`/`source` visible only through `ReplayScope::formed`.
8. **Stated limits.** `:22-26`: "An explicit formation entrance may search; replay itself must not." `:946-947`: source-checked contracts "are independently retained interfaces, not an additional premise".
9. **Fail-open risks.**
   - `Scope::clauses`: a clause that does not bind and does not "owe" is silently skipped and contributes no premise (`:525-538`). **Stated** (`:529-533`). Effect: a clause with no arithmetic, byte or storage-read terms that fails to bind simply gives the checker fewer premises. Wrong program that gets through: none directly, as fewer premises never prove more; but the *same* clause as an entry contract is still demanded of callers elsewhere (traced by reading, not by running).
   - `contract`: if `owes(...)` is false for the whole contract the function returns `Ok(())` without forming anything (`:904-919`); equal to "nothing to form". **Traced**.
   - `validate_float_meaning_source` returns `Ok(())` (`:565-569`). **Stated**: "Formation checks a symbolic declaration equation. It mints no concrete projection or runtime owner."
   - `.expect("nonzero")` on `ValueId::new(index + 1)` (`:421`, `:434`): a panic path, not acceptance.
   - Fallback diagnostic machine `MachineId::new(1)` when the module has no machines (`:52-53`, `:75-80`, `:100-103`, `:980-984`): refusal text may name machine 1 even when it is not the culprit.

### P6/src/generic_storage.rs (74 lines)

1. **Purpose.** A generic type's storage keeps the binder that declared it; its bounds and unknown contents cannot be replaced with copy or cleanup permission (`:1-3`).
2. **Shape.** `declared(&Elaboration) -> Checked<()>` (`:9`).
3. **Decides.** The structural type roster must be strictly ordered by id; `generic_storage` sources must have unique `storage` ids, each with a valid `parameter` equal to `generic_binders[ordinal]`; the type row must be `TypeParameter` shape and equal in identity text, multiplicity, and have no `owns_linear_claim`, `no_code_affine_cleanup`, `descriptor_cleanup`, `nominal_cleanup`, `quotient_carrier`, `scalar_restriction` or `requires` (`:47-59`); every `TypeParameter` row must be declared (`:65-72`). Identity compare is by text (`identity != Some(source.identity.as_str())`, `:48-49`).
4. **Same job elsewhere.** none.
5. **Unused.** 2 call sites: `reuse.rs:1552` and `verdict.rs:994` (grepped).
6. **Fail-open.** None seen. Diagnostic machine is `MachineId::new(1)` (`:10`).

### P6/src/graph.rs (384 lines) and graph/roots.rs (70 lines)

1. **Purpose.** Dominance of program points: "Premises must dominate their uses on every path" (`:2`). Builds a dominance forest once per machine from the table edges (including edges no terminator owns, which "conservatively remove availability", `:4`) and answers `dominates(available, used)`.
2. **Shape.** `Graph` (`:22`), `Graph::new` (`:42`), `entry`, `block_count`, `in_cycle` (`:172`), `operation_count`, `predecessors`, `reverse_postorder` (`:191`), `locate` (`:197`), `locate_with_dominance` (`:204`), `dominates` (`:253`), `ordered_blocks` (`:322`), `cyclic_blocks` (`:350`). `roots::virtual_entries` (`roots.rs:4`).
3. **I/O.** Reads `TerminalMachine.blocks`, `MachineTables.edges`; uses `terminal_semantics::control_graph::dominance_forest`. Produces a `Graph`.
4. **How it decides.**
   - Refuses unless blocks are dense in id order, entry exists, each operation appears once, each edge's source/target exists, each cleanup roster edge belongs to its block (`:54-104`).
   - `dominates` by program point kind: same block compares positions; `Edge(e)` available gives a premise only if the destination has exactly one predecessor entry and its dominance depth is not 1 (virtual-entry) (`:305-313`); `BeforeCleanup/AfterCleanup` order only on their own edge by `(ordinal, after)` (`:270-275`); unknown points are an `Err` even compared with themselves (`:253-257`, test `:332`).
   - Virtual entries: if all blocks are entry-reachable only the entry is a root; otherwise each member of a disconnected *source* strongly connected component is a root (`roots.rs:20-69`); a downstream block keeps its dominators.
5. **Cases by shape.** Block ids must equal position + 1 (dense) (`:55`).
6. **Same job elsewhere.** The test oracle in `graph/tests.rs:135` recomputes the same rooted semantics independently.
7. **Unused.** Callers per grep: `reverse_postorder` 1 in `joins.rs:436` (+3 tests); `in_cycle` 2 in `joins.rs` + `places.rs:3496`, `backing.rs:683,775`; `block_count` 6 sites in `places.rs`/`byte_views.rs`; `operation_count` 3 sites in `places.rs`/`log.rs`. (command: Grep `(block_count|operation_count|reverse_postorder|in_cycle)\(`.)
8. **Stated limits.** `graph.rs:117-118`: "The legacy traversal may start inside a disconnected downstream component ... Keep that consumer order".
9. **Fail-open.** None seen: every unresolved point or malformed edge is an `Err`; `dominates` returns `Ok(false)` in the conservative direction (cleanup into a different block `:281-290`, edge with several predecessors `:308`).

### P6/src/graph/tests.rs (539 lines)

1. **Purpose.** Tests for `Graph`: an ignored scaling driver (`:96`), an exhaustive check of all 65,536 four-block edge sets against an independent oracle (`:182`, mask over `count*count` bits), 256 random larger graphs (`:211`), rootless cycles (`:250`), disconnected source order (`:269`), entry backedge (`:295`), duplicate edges (`:309`), foreign points refuse (`:332`), malformed topology (`:351`), 20,000-block iterative traversal for 5 shapes with `retained_bytes < count * 256` (`:413`), extra table edges (`:444`), cleanup point ordering (`:489`).
2. **Shape.** `fixture` (`:13`), `shaped_edges` (`:51`), `oracle` (`:135`, transitive closure plus removal search).
3. **Decides.** The oracle defines roots as: node 0, or a node not reachable from 0 whose SCC has no incoming edge from another SCC (`:151-161`).
4. **Stated limits.** The ignored test uses env vars `OMEGA_DOMINANCE_SHAPE` and `OMEGA_DOMINANCE_BLOCKS` (`:96-102`).
5. **Fail-open.** None.

### P6/src/joins.rs (1,285 lines)

1. **Purpose.** What a block parameter carries across a join: infers a predicate for each join from the facts on each arrival, records it as `BlockInvariant` knowledge, and states each arrival's goal. "Nothing here reads a name or text: values are compared by identity" (`:53`).
2. **Shape.** `prepare` (`:76`), `infer` (`:371`), `argument_bounds` (`:581`), `describe` (`:634`), `arrival_goal` (`:875`), `check` (`:1035`), `connective` (`:329`), `project` (`:353`), `rename` (`:1000`), `in_scope` (`:178`), `available_place` (`:230`), `available_value` (`:285`), `Machine` (`:1012`), plus a 2-test module (`:1137`).
3. **I/O.** Reads `Graph`, `MachineTables` (knowledge, edges, values, obligations), `Storage` (places analysis), declared `ScalarBlockInvariant`s. Mutates `tables` (adds knowledge rows or edits an existing proposition) and `decided` verdict slots.
4. **How it decides.**
   - `infer` visits blocks in reverse postorder; a join with no parameters, no snapshots and no byte predicates is skipped (`:440-442`); a block is skipped (`skip = true`) if any incoming edge is dominated by the block's own entry (a cycle) or an incoming edge passes no arguments (a dispatch arm) (`:455-470`); an inferred predicate is a disjunction over incoming edges (`connective(disjuncts, false)`) conjoined with byte predicates (`:532-537`).
   - Per arrival `describe` conjoins: parameter equals argument (expanded through checked operation equations when the expansion differs), integer literal bounds from the prover, facts available on the edge but not at the join (renamed), and a preceding join's predicate read with its result fixed (`:634-869`). A part that names a value out of scope, or a contradiction, is dropped; "that weakens the disjunct, never strengthens it" (`:864-866`).
   - Scope: `available_value` admits machine parameters, storage content of a formal at entry, block parameters of this or a dominating block, and an *integer* operation result that dominates the join (`:285-327`; comment `:273-284` says Booleans are excluded because a dominating Boolean "left a predicate whose case split exhausted the prover's bounded disjunction search").
   - `check` decides every `ArrivalInvariant`; on any failure the destination's inferred predicate is replaced by its declared predicate (or `Truth`) and the loop repeats until no change; the weakened blocks are returned (`:1035-1135`).
5. **Cases by shape.** In `in_scope`, a proposition containing `Atom`, `IeeeFloatComparison`, `ByteSequenceEqual`, `StructuralIdentity`, `StructuralCaseMembership` or `ContentConservation` is "not candidate for this scalar arrival protocol" and treated out of scope (`:199-223`). `available_place` accepts only a formal `Parameter` or a `BlockParameter` of this or a dominating block with a matching `formal.place`/`position` (`:236-270`).
6. **Same job elsewhere.** `connective` is the one AND/OR constructor used across stage 06 (grepped: `entry_inputs.rs`, `joins.rs`; not counted); `entry_inputs::canonical_parts` is a second, ordering one (see above). `arrivals::admit` admits the declared invariant; `prepare` calls it (`:89`).
7. **Unused.** `prepare`, `infer`, `check` used from `entry_inputs.rs:162,178` and `lib.rs` (not grepped for `lib.rs`; crate-map names `lib.rs` as host of discharge). `arrival_goal` is called from `lib.rs`' goal builder (inferred).
8. **Stated limits.** `:50-51`: "A dispatch arm's payload parameters are bound by `DispatchBinding` rows, and lowering mints no arrival for them." `:47-49`: weakened joins are reported in `MachineDischarge::weakened_joins`.
9. **Fail-open risks.**
   - `arrival_goal` returns `Truth` when the destination has no predicate row (`:887-889`). **Traced**: with no knowledge row for the join, no later obligation can rely on it; it is also the declared-invariant-free case lowering mints arrivals for (the test `P5/src/tests/loops.rs:149-152` says "with no predicate on the head its goal is `Truth`").
   - `infer`'s `skip` leaves a join without any predicate (`:455-470`): fail-closed for uses after it.
   - `check` replaces a failed inferred predicate by the declared one; "a declared predicate is never weakened" (`arrivals.rs:23-24`): `*predicate != required` branch at `:1125`.
   - `describe` swallows unbound snapshot terms: `Err(Unbound::Unproved | Unsupported) => {}` (`:504-507`); only `Unbound::Refused` propagates. **Stated** in the module header (`:43-47`) as weakening.
   - `argument_bounds` returns an empty set on any failure of `hypotheses` (`:602-604`). Weaker.
   - `available_place` / `available_value` use `.is_ok_and(|d| d)` on `dominates` (`:257`, `:296`): an `Err` becomes `false`, never `true`.

### P6/src/known.rs (925 lines) and known/normalization.rs (88 lines)

1. **Purpose.** Equality transport at one program point: turns the assumptions and axioms available into a flat fact list, picks one defining equation per value, normalizes every premise by unfolding those definitions with the kernel's own `check_value_equality_denotation`, and records the derivation so a certificate can be rebuilt. Shared by proof search and by entry-input description (`:1-3`). "This computes no new domain, authority or assumption; final obligation discharge still checks the evidence with the kernel." (`:10-12`)
2. **Shape.** `ProofValues` trait (`:35`); `Fact` (`:94`) with `Origin` (`:72`: `Assumption`, `SemanticAxiom`, `PendingConjunct`, `Conjunction`, `IntegerEquality`, `EqualitySymmetry`, `Transport`); `Known` (`:227`); `Known::{new, prepare, arrival_definitions, integer_equalities, add_literal, add_integer_alias, expand_value, same_denotation, transport_inputs, normalize}`; `TransportInputs` (`:248`); `Fact::disjunction_origin` (`:129`), `Fact::proof` (`:173`); free `flatten`, `integer_equality`, `is_branch_fact`. In `known/normalization.rs`: `NormalizedPremises`, `BranchPreparation` (`:18`) with `begin`, `inherited`, `record`.
3. **I/O.** In: `PropositionContext`, assumptions, axioms, a `ProofValues` owner (here `IdTable<ValueId, ValueRow>`). Out: `Known` whose `facts` each can be rendered to a `ProofNode`.
4. **How it decides.**
   - A value's definition (an "equation") is an `Equal(Value, term)` fact that is: a Boolean literal only if it is an axiom; an integer or float literal; a `Value` copy (a storage content defined by a non-storage value is deferred so exact stored contents stay the root, `:424-437`); other terms only if the subject is Boolean, or an axiom over an integer (`:442-445`); or an `Application` equation (`:449-451`).
   - First definition wins; a later axiom with a different proposition replaces an assumption's definition (`:481-488`).
   - Branch literal `b == literal` fills an undefined Boolean value (`add_literal`, `:615-634`); integer alias of two undefined values orders by "non-storage first, then id" (`:664-681`).
   - `transport_inputs` selects only the definitions transitively used by a proposition, indexed by value or by `(application, carrier)` (`:753-822`).
   - `normalize` re-runs a premise through `check_value_equality_denotation`; on `Err` the premise is simply not transformed (`:865-873`, `continue`).
   - `BranchPreparation::inherited` reuses the parent's simplified premise only if the selected equation set is pairwise the same (`normalization.rs:48-68`).
5. **Cases by shape.** `integer_equality` lifts `Equal` only between *fixed-carrier non-address* integer terms (`:324-329`).
6. **Same job elsewhere.** `expand_value` (`:685`) and `same_denotation` (`:718`) both call `transport_inputs` then a kernel helper; `entry_inputs::known_at` builds a `Known` with the same inputs as the prover (`entry_inputs.rs:752-789`). The `proof_preparation` examples compile this file a second time (see Examples).
7. **Unused.** `Known::new`: 4 test constructions here plus `entry_inputs.rs:783`; `FactProofWork::nodes` is `cfg(test)`. Not individually grepped.
8. **Stated limits.** `:683-684`: "If it cannot be expanded, keep the original checked value."
9. **Fail-open risks.**
   - `expand_value`: on kernel error returns the unexpanded actual (`:712-714`). Effect: fewer rewrites.
   - `same_denotation`: any kernel error returns `false` (`:744-747`); `Ok(Truth)` returns `true` (`:745`). **Traced**: `Truth` only if the kernel's denotation of the equality is trivially true.
   - `.expect(...)` on proof-value identity (`:467-472`): panics when the owner's identities are not contiguous. Not acceptance.
   - A `NormalizedPremises` row is reused only when equation identity matches; otherwise recomputed (`normalization.rs:59-67`): fail-closed.

### P6/src/known/tests.rs (441 lines)

1. **Purpose.** Unit tests for `Known`: conjunction origins and proofs checked with the kernel (`check_all`, `:18`), sharing of the parent conjunction proof (`:122`), empty conjunction still refused by the kernel (`:165`), first-definition retention and axiom identity (`:192`), integer equality lifts in both orientations (`:228`), branch literal transport captures only prior equations (`:284`), offset value identities (`:348`), application transport prefix and carrier (`:388`).
2. **Decides.** Every derived fact's proof is re-admitted with `proof_admission::accept_certificate` (`:26`), so the tests treat the kernel as the oracle.
3. **Fail-open.** None.

---

## Coverage

Read in full (every line): all 42 ranges of `units/unit_063.tsv`.
- Stage 05 (14 files): `src/tests.rs` 1-4625, `src/tests/array_views.rs` 1-259, `src/tests/loops.rs` 1-656, `tests/block_values.rs` 1-167, `tests/positional_field_reads.rs` 1-233, `tests/prepared_array_extents.rs` 1-281, `tests/prepared_boolean_patterns.rs` 1-224, `tests/prepared_scalar_values.rs` 1-386, `tests/selected_calls.rs` 1-464, `tests/selected_operations.rs` 1-461, `tests/subjectless_match.rs` 1-139, `tests/support/dispatch_arms.rs` 1-239, `tests/support/sum_dispatch.rs` 1-271, `tests/variant_parent_identity.rs` 1-194. Total 8,599 lines.
- Stage 06 (28 files): `examples/*` (6 files, 47 lines), `lib.rs` 32, `src/arrivals.rs` 115, `branch_warnings.rs` 293, `call_contracts.rs` 603, `callable_arrivals.rs` 119, `ceilings.rs` 1480, `crashes.rs` 313, `describe.rs` 609, `entry_inputs.rs` 1693, `entry_inputs/inferred_calls.rs` 155, `integer_cases.rs` 175, `recursive.rs` 196, `external_providers.rs` 251, `formation.rs` 1385, `generic_storage.rs` 74, `graph.rs` 384, `graph/roots.rs` 70, `graph/tests.rs` 539, `joins.rs` 1285, `known.rs` 925, `known/normalization.rs` 88, `known/tests.rs` 441. Total 11,272 lines.
- Grand total read in full: 19,871 lines (sum of line counts of the 42 rows).
- Skimmed: none. Not read: none.

Commands run for counts: Grep count of `#[test]` in `P5/src/tests.rs` (64); Grep count of `call_contracts::` over `source-rust` (36 occurrences, 11 files); Grep listing of `(block_count|operation_count|reverse_postorder|in_cycle)\(` and `ceilings::|well_formed_reach|judge_declared_rows` and `coverage_goal|crashes::` over stage 06 / `source-rust`; Grep of `external_providers::publish|generic_storage::declared|arrivals::admit|callable_arrivals::check|branch_warnings::|entry_inputs::...|describe::...|known_at\(` over `source-rust` (head limit 60: the listing showed 24 lines, below the cap); Grep count of `decision 17` in stage 06 (2); Grep of `Cargo.toml` of `source-to-psi` for the registered test and example paths (15 lines, 9 + 6).

## Five things a maintainer most needs to know about this unit

1. **Stage 05's tests in this unit are of two kinds: closed-body lowering tests, and handoff twin tests.** `src/tests.rs` bypasses the reader entirely (`lower_bodies`, `lower::machine`), while `tests/*.rs` run real source through stages 00 to 04, then edit one fact in the stage 04 product (`into_inner`, edit, `prepare_typed_trees`) and require the lowered output to follow it (e.g. `prepared_array_extents.rs:202`, `selected_calls.rs:159` shows two selection records that disagree are refused). Integration tests are not in the stage 05 `Cargo.toml`; they are `[[test]]` entries in `source-to-psi/Cargo.toml:142-182` (grepped).
2. **Stage 06's rule for inference is "describe, form, then re-prove".** `entry_inputs.rs` guesses a helper's input requirement from every caller's facts, drops any conjunct that does not "form" (`:1224-1288`), and then every call and every loop-head arrival still owes the clause to the same prover and kernel. The same shape appears in `joins.rs` (an inferred join predicate is weakened to the declared one if any arrival fails, `:1035-1135`) and `result_summaries` (not in this unit). So an inference bug here shows up as a failed proof or weakened predicate, not as acceptance.
3. **Where a `None`/open row skips a check is on the ceiling side.** `ceilings.rs:1300` skips an axis with an open caller ceiling; the compensating refusals are at `:1189-1200`, `:1212-1216` (open callee under finite caller) and `:329-335` (unknown operational bound under a known one). `judge_declared_rows` runs only Reach and Entries (`:944`), so refinement of declared rows does not check suspension, blocking or termination (stated in the doc at `:937-939`).
4. **Dominance (`graph.rs`) is the single availability test, and it is deliberately conservative.** A fact on an edge counts only if the edge is the sole way into its target and the target is not a virtual root (`:305-313`); disconnected source cycles get virtual roots (`roots.rs`); unknown points are `Err`, not `false`. Callers that turn the error into `false` (`joins.rs:257`, `:296`) keep it conservative.
5. **Formation is the gate on every contract clause, and it keys on structure, not names.** `formation.rs` proves each partial arithmetic in a clause total before the clause becomes a premise, with the ordinary prover and kernel (`:431-463`). It decides whether a failed-to-bind clause is an error by `owes` (`:852`): only clauses with arithmetic, byte predicates or structural reads fail the entrance. Test-side name compares exist (`sum`'s `identity.contains("path(Left)")` in `sum_dispatch.rs:179`, `machine`'s `symbols.name(..) == name` in `dispatch_arms.rs:237`) but none in the production files read here, except `generic_storage.rs:48-49` comparing identity text and `describe.rs` (display only).

## What you are unsure about

- Whether `ESCAPE` (`dispatch_arms.rs:50`) and `dispatch_edges_mut` / `payload_loan_mut` (`sum_dispatch.rs:227`, `:262`) are used by stage 06 tests. I did not grep their names outside this unit.
- The length-equality of `candidate.required.structural_parameters` and `candidate.supplied.structural_parameters` before the `.zip` in `external_providers.rs:140-144`; the check may be inside `contract_refinement_goals` (`terminal_semantics`), which I did not read.
- Whether `unreached.mark_formal(callee, formal)` (`entry_inputs.rs:364-366`) can ever make a value look *less* constrained to a later proof; the consumer (`Unreached::values`, in `lib.rs`) is outside this unit.
- Whether `fixed_refinement_reach` (`ceilings.rs:1396`) is sound for every callee kind; I recorded it as an "open" fail-open item because no comment gives the reason for replacing an exceeding reach.
- Callers of `call_contracts::terminator_site` and `TailCallSites` outside `ceilings.rs`: I counted `call_contracts::` occurrences by file, not per function.
- Whether the `proof_preparation*.rs` examples still compile: they mount `../src/known.rs` and `../src/prove.rs` with a local `mod checking { pub(crate) use super::known; }`, but `known.rs` uses `crate::checking`-style paths only through `proof_admission`/`terminal_check_tables`, not `crate::checking`. I did not build anything (read-only rule).
