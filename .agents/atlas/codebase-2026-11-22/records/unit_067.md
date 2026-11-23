# Record: unit_067 (area 04-source-stages-06-07)

`T` = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/tests`.
Every file in this unit is a `#[cfg(test)]` module of the stage-06 crate (the lowered-form checker); each is declared by
`mod <name>;` in `T.rs` (`src/tests.rs:28-102`, read; `mod loops;` is declared late at `tests.rs:7575`, `mod table_faults;` at
`:5114`; `rankings` is `pub(crate) mod` at `:81`). The unit is 46 files, 21,801 lines, 487 `#[test]` functions
(sum of per-file `grep -c '#\[test\]'`). None of it ships; it is the evidence behind what the checker accepts and refuses.
Nothing here is production code, so items 4, 7 and 9 of the brief are answered as "what the test pins" and "what the test
tolerates".

## Shared shape (read once, applies to every file below)

- Two test styles. (a) **Source style**: a Omega program as a string, parsed and typed by `boundary_calls::typed` /
  `try_typed` (`boundary_calls` is outside this unit; 119 call sites in `T`, grepped), lowered with
  `crate::lowering::elaborate_for` / `prepare_compilation` / `elaborate_closure` / `elaborate_generic`, then judged with
  `crate::checking::judge` / `judge_compilation` (or `build_purpose::judge_build`, 52 uses, grepped). (b) **Lowered-body
  style**: hand-built `Machine`/`Body`/`Signature` values from the `super::*` helpers in `tests.rs` (`machine`, `returns`,
  `call`, `strict`, `slot`, `at_most`, ...), lowered with `lower(..)` / `crate::lowering::lower_bodies`, discharged, and the
  proof kernel re-decides each certificate (`assert_kernel_accepted`, `verdicts`, `certificate` - defined in `tests.rs`, not
  in this unit).
- Positive/negative twins are the norm: one `.replace(..)` edit of the source string or one changed number
  (e.g. `case_payloads.rs:94-99`, `contract_formation.rs:301`, `indexed_contracts.rs:52`).
- A third style forges the checker's input: take an accepted `Elaboration`, edit one table row or module field, call
  `judge`, and expect `RefusalReason::ForeignIdentity` with an exact detail string. Files that do this: `ceilings.rs`
  (641-674, 790-815), `lifetime_generics.rs` (395-411, 476-501), `operational_effects.rs` (218-281, 354-380, 740-778,
  845-863), `owned_cleanup.rs` (194-233, 357-385), `owned_dispatch.rs` (139-168, 258-304), `qualified_terms.rs` (38-140,
  288-352, 550-562), `rankings.rs` (194-258, 399-436), `loops.rs` (194-200), `parallel_checks.rs` (71-97), `proof_scope.rs`
  (76-121), `references.rs` (601-639, 759-855), `returned_views.rs` (79-121, 426-546, 1018-1118), `hosted_primitives.rs`
  (111-249), `representation_views.rs` (302-423), `indexed_contracts.rs` (364-428), `result_fields.rs` (338-376),
  `machine_inputs.rs` (18-82).
- Several tests run the published artifact in the Psi interpreter (`terminal_interpreter::TerminalProgram::borrow`,
  `WorkMeter::with_budget`) to confirm a result value, e.g. `case_payloads.rs:555-575`, `collection_members.rs:321-361`,
  `loan_regions.rs:60-89`. They do not run natively.
- Many tests judge by comparing the verdict to an exact message string (a `verdict()` helper returning `String`, 15 copies
  across `T`: `grep -n "fn verdict(" T/*.rs`; in this unit: `case_payloads.rs:19`, `empty_records.rs:60`, `floats.rs:15`,
  `loan_regions.rs:143`, `machine_lifetimes.rs:17`, `owned_dispatch.rs:34`, `representation_views.rs:18`,
  `returned_views.rs:36`). So many assertions are keyed on **source spelling of a diagnostic** (the error text), not on an
  error code. A reword of a diagnostic breaks them. Notably `lifetime_elision.rs:82,115,151,287` embed a hash-shaped
  package identity string `package-owner(32:0707...07)` inside the expected message.
- Same job elsewhere: the "build a `Compilation { calling: None, providers: None, ... }` literal and run
  `prepare_compilation`/`elaborate_for` then `judge_compilation`" block is repeated 26 times in 23 files of this unit
  (`grep -c 'crate::lowering::Compilation {'`) and 68 times in all of `T`; `build_purpose::roster` (`build_purpose.rs:170`)
  is the shared builder some files use instead (e.g. `case_payloads.rs:106`, `entry_inputs.rs:136`).
- Helpers exported to other test files (`pub(super)` / `pub(crate)`), with user counts from `grep -rE` over `T`:
  `entryless::judged_typed_result` (`entryless.rs:18`; 15 uses in 7 files), `entryless::judged` (`:6`; 21 uses), `entryless::
  judged_typed` (`:11`; 1 use, `termination.rs`), `entryless::whole_subject_result` (`:28`; 8 uses incl. its own file),
  `rankings::typed_with_ranking_catalog` (`rankings.rs:263`; 17 uses in `collection_members.rs`, `rankings.rs`,
  `successor_loans.rs`, `termination.rs`), `references::{Lending, unit, pair, named, field, lend, invoke, store, pair_of,
  accepted, lower_lending}` (`references.rs:21-231`; used by `mutable_formals.rs`, `arithmetic_policies.rs`,
  `successor_loans.rs`; `lower_lending` 16 uses), `mutable_formals::unsafe_operations` (`mutable_formals.rs:39`; 8 uses in
  `arithmetic_policies.rs` and itself).
- Outside this unit and not read: `stored_references::run` (used by `loan_windows.rs:8`, `returned_views.rs:1336,1377`),
  `calling_policies::prepared` (used by `result_fields.rs:233,324`, `returned_views.rs:195`), `build_purpose.rs`,
  `boundary_calls.rs`, `tests.rs` helpers. "suspected" wherever I rely on what they do.

## Per-file entries

### case_payloads.rs (771 lines, 18 tests)
- **Purpose.** Pins when a loan of a sum's payload field is accepted: only where the sum is known to hold that case
  (header comment `:1-11`, citing patterns.md "A sum held in a field").
- **Shape.** `verdict(build)` `:19` (judge a Build program; returns detail text or `"accepted"`), consts `NO_CASE` `:42`,
  `VIEWS` `:44`, `SCALARS` `:53`; `OWNED_STORE` `:544` is `include_str!` of
  `P06/tests/fixtures/ownership/run/known_case_payload_store/main.omg` through a `CARGO_MANIFEST_DIR/../../../../../../source-rust/...`
  path; `execute_owned_store` `:555`; `arm_read` `:579`; `shelf`/`element_store` `:732-761`.
- **I/O.** Reads source strings, `judge_build`, `boundary_calls`, `build_purpose::roster`; no output beyond asserts.
- **How it decides.** Keyed on exact diagnostic text (`NO_CASE`, `:391`, `:433`, `:455`) and, for the recursive-arrival tests, on
  a **source position**: it finds the byte offset of `"block directory("` (`:117`, `:277`) and selects lowered edges whose
  origin span starts there (`:122-125`, `:282-285`), then checks `ArrivalInvariant` rows by `(machine, obligation, edge, origin)`
  (`:178-190`).
- **Cases by shape.** `an_indexed_case_payload_proves_a_recursive_arrival` `:66` is one `[Frame; 2]` array with `Empty`/`Directory`
  cases; `shelf` tests use `[Choice; 2]` with fixed index `1` versus runtime `slot` (`:763-771`).
- **Stated limits.** `:610` "Lowering used to return the dispatch-time 1 here" (past defect, now pinned).
- **Fail-open.** none seen (tests only). Note `:196-200` panics if refused for the `directory` row, else other outcomes also panic.

### ceilings.rs (979 lines, 17 tests)
- **Purpose.** Service reach (`reaches`), synchronous entry (`invokes`), suspension and blocking envelopes, and requirement
  envelopes for abstract callbacks (`:1-4`).
- **Shape.** `judged` `:12` (builds a `Compilation` literal), `program` `:44`, `exceeded` `:74`, `rows` `:94`, `entries` `:203`,
  `callable_witness` `:490`.
- **Calls into checker internals.** `crate::checking::prove_callable_refinement` (`:532`), `static_contracts::admit` (entryless.rs
  neighbour, here `:98` in entryless; in this file `static_contracts::requirement_declarations` `:639,650,669`), `ceilings::judge`
  (`:709,788,815,883`), `ceilings::Axis` / `Permission` (`:885-889`), `discharge` (`:693`).
- **How it decides.** Keyed on identity (service ids found by `declaration.id == *service`, `:97-111`), on message prefixes/suffixes
  such as `" (service reach)"` `:174` and `" (synchronous invocation)"` `:284`, and on exact detail strings for forged rows
  (`:654`, `:671`, `:817`).
- **Cases by shape.** Forged tests select "the first" row: `form.requirements[0]` `:777`, `delivery .next()` (`:608-615`).
- **Stated limits.** `:358-359` "execution through that entry identity remains explicitly unsupported".
- **Fail-open.** none seen. `:797-801` asserts that clearing one `suspension` field still passes because "The generic declaration
  still establishes the relay's suspension bound": a test that a missing row is accepted, with the reason stated at `:796`.

### collection_members.rs (933 lines, 26 tests)
- **Purpose.** Member selection on collections (`items.length` is a measure, a field called `length` stays a field, an unknown
  receiver gains no builtin, `:1-2`) and the checker facts about view/element length across calls and stores.
- **Shape.** `members` `:7` (reads `ExpressionNode::Member` selections from the typed trees), `judged` `:73`, `judged_invocation`
  `:77`, `elaborated` `:104`, `ranked_sequence_loop` `:847`, `judged_ranked` `:867`.
- **How it decides.** Keyed on identity of the **descriptor/place** (`source.place == machine.structural_parameters[0].place`
  `:123,142,198`) and on `ObligationKind` (`OperationSafety`, `CallRequirement`, `ArrivalInvariant`). The `a_loop_length_premise...`
  pair differ only in which tuple argument the back edge passes (`:890`, `:921`), and the negative checks the row **text**
  `"position + 1 <= other.length"` `:929`.
- **Hand-built loop.** `an_overlapping_mutable_call_on_a_backedge_invalidates_the_next_read` `:656-840` rewrites a lowered body into
  a loop (moves staged-call operations into a new `latch` block, adds `EdgeRow`s with empty dispositions `:753-757`, builds
  `Graph::new` `:774`, `places::Storage::new` `:791`, calls `storage.read_equation` `:828`). Comment `:651-654`: "Source storage
  cycles are currently unserved ... This asserts no source-loop acceptance."
- **Same job elsewhere.** The `judged`/`elaborated` pair duplicates `entryless.rs:6-39` in shape.
- **Fail-open.** none seen in the checker; test-side: `a_mutable_boundary_keeps_the_extent_used_to_restore_a_saved_byte`
  `:508-605` has an `if let Rejected {...}` branch that checks only unproved failures but then falls to a final
  `assert!(matches!(verdict, Accepted))` at `:601`, so the branch is a diagnostic aid, not a second accepted outcome.

### compiler_primitive_headers.rs (715 lines, 13 tests)
- **Purpose.** A catalog (compiler-supplied) primitive is recognised only from real bundled toolchain source with a complete,
  exact header (`:1-2`). Files read: `source/library/core/indexing/indexing.omg` (`:7`) and
  `.../byte_observation/byte_observation.omg` (`:116,219`), plus `samples/cli/probes/bounded_concat/main.omg` (`:606`).
- **Shape.** `typed`/`typed_at` `:12-48` run the frontend by hand (own `SourceMap`, `Lexer`, `parse_syntax_trees_with_id`,
  `resolution::resolve`, `typing::lower_symbol_resolved_trees`), `observed_typed` `:235`, `judge_observed` `:251`, `execute_observed`
  `:277`.
- **How it decides.** Keyed on **`SourceOrigin`** and on exact source text: the same signature as `User` origin, or `Toolchain`
  origin with one appended comment line (`:158`), gets no catalog authority (`:157-173`; also `:218-233`). `authenticated_byte_calls_
  still_require_their_complete_actual_header` `:378-440` mutates 5 header facts one at a time (`is_public`,
  `admitted_same_stack_bytes`, an extra lifetime, a foreign operand lifetime owner, a renamed machine) and expects
  `compiler_primitive_schema` to err.
- **Cases by shape.** Tests list four byte predicates (`valid_utf8`, `no_nul`, `ascii_only`, `non_empty`) `:128-133` and three
  collection primitives (`read`, `range`, `view`) `:58-62`.
- **Stated limits.** `:482-492` prints both verdicts "so an unserved cast or observation is visible"; `:1-2` "These controls do not
  claim application checking".
- **Fail-open.** none seen. `an_indexed_store_proves_both...` `:699-714` accepts a negative if **any** verdict is a `Rejected`
  with a `DataDomain` row (`verdicts.iter().any(..)`), so it would also pass if other verdicts were accepted (test-side looseness).

### connected_facts.rs (64 lines, 2 tests)
- **Purpose.** Pins `crate::checking::connected` (narrowing the facts handed to the prover/kernel before `decide`): keep facts
  that share a value with the goal directly or through a chain, and facts naming no value; drop others (`:1-4`).
- **Shape.** `hypotheses(assumptions, axioms)` `:20` builds `Hypotheses` with dummy `KnowledgeId` rows; tests `:31`, `:57`.
- **How it decides.** Keyed on **value identity** (`ValueId`) of terms. `connected(..)` returns `None` when every fact is connected
  (`:63`), meaning "use the full set".
- **Fail-open.** none seen. Note facts naming no value are always kept (`:34`, `:45-47`), pinned deliberately.

### contract_formation.rs (949 lines, 38 tests)
- **Purpose.** "A condition cannot make its own arithmetic safe" (`:1`): contract clauses (`requires`, `ensures`, domain predicates,
  data `requires`, predicate definitions) are formed in order and must not use their own consequence to justify partial
  arithmetic (subtraction, division, casts).
- **Shape.** Lowered-body half `:6-247` (`form` `:22` calls `crate::checking::formation::module`; `prepare_replay` `:104,245`);
  source half `:249-949` (`source_result` `:249`, `accepted_source` `:253`, `executed_source` `:261`, `rejected_source` `:278`).
- **How it decides.** Pins refusal text `"contract arithmetic is not proved total before its condition"` (`:308,357,400,...`) and
  `RefusalReason::Violation`. Domain variants prefix the text: `"data domain: ..."` `:426`, `"scalar domain: ..."` `:693,833`.
  `:458-464` forged `predicate.subject = id(8)` expects `RefusalReason::Unsupported` and `"contract field has no declared root"`.
- **Cases by shape.** `byte_predicate_contracts_reject_nonbyte_inputs_under_a_false_precondition` `:147-217` loops two
  completions (`[false, true]`) over a single helper whose actual input is a `u32`. `unused_helper_arguments...` `:805-838` lists six
  literal predicate spellings (`always(1u64 / self)`, `zero(...)`, `identity(...)` ...).
- **Same job elsewhere.** `rejected_source` in the same file; `ranges.rs` and `reach.rs` test arithmetic safety from the lowered
  side instead of the contract side.
- **Fail-open.** none seen. `:130` and `:844-866` pin that a false helper result does not hide an unformed argument.

### empty_records.rs (187 lines, 3 tests)
- **Purpose.** `data Token {}` (no fields, no cases) is an ordinary record with an empty roster: established by a literal or a
  declared local, copied if `[copy]`, moved once if not (`:1-7`).
- **Shape.** `tokens(copyable)` `:17` builds two `RecordType` rows (`Token`, `Holder`) directly; `verdict`/`accepted`/`unestablished`
  `:60-85` use `elaborate_over` (helper in `tests.rs`, not read).
- **How it decides.** An affine empty record read twice is refused by "custody replay", asserted only as non-empty `unestablished`
  (`:133`, comment `:131-132`: "There is no leaf to find unestablished, so the custody replay is what rejects it").
- **Fail-open.** none seen.

### entry_inputs.rs (1044 lines, 19 tests)
- **Purpose.** Inferred input contracts for private helpers: a private machine with no `requires` gets one inferred from all its
  callers (join), and each caller must prove its own arrival; `private_inputs` / `local_results` flags mark which machines
  participate (`:36-40`).
- **Shape.** `predecessor` `:3`, `guarded_calls` `:14`, `with_private_inputs` `:36`, `judge_source_arrivals(source, expected_private)`
  `:123-152` (asserts the **set of private helper names** equals a list sorted by name, `:144-150`).
- **How it decides.** Keyed on machine **name** for "which helpers are private" (`:147`) and on machine id for row checks
  (`row.machine == id(3)`, `:328,340,414`). It also inspects the published identity table:
  `module.identities.proved_private_result` `:167,181,188`, `inferred_input_clause` `:216`, `matches_inferred_input_clause`
  `:222,876`, `proved_inferred_input` `:252,889`.
- **Cases by shape.** `a_guarded_field_survives_a_wrapping_call_cycle_and_rejects_unguarded_twins` `:709-837` is a table of 7 named
  variants; `a_guard_on_a_field_written_before_the_call_reaches_the_helper` `:975-1044` a table of 5; one fixture read through
  `include_str!` `:266`.
- **Stated limits.** `:454-456` comment: "A fixed-input boundary does not receive its caller's stronger fact" (test name `:419`).
- **Fail-open.** none seen. `an_unstated_disjunct_is_proved_false_not_assumed` `:949` pins that an unnamed disjunct is not taken as
  true.

### entryless.rs (500 lines, 18 tests)
- **Purpose.** Source validation of a whole subject with no program entry (library/declaration-only); a subject passes only if
  its closure, generic forms and witnesses all pass (`:25-27`).
- **Shape.** `judged` `:6`, `judged_typed` `:11`, `judged_typed_result` `:18`, `whole_subject_result` `:28`, `judged_parts` `:41`
  (builds `Compilation { entries: &[], subjects: &[None], selected: ... }` `:54-65`).
- **How it decides.** `whole_subject_result` `:28-39` removes and returns the **first** non-`Accepted` verdict in judgment order, else
  index 0 (the closure); asserts verdict list non-empty (`:31`, pinned by `#[should_panic]` test `:399-402`).
- **Cases by shape.** `specialized_callback_reach_keeps_omission_empty_and_named_services_distinct` `:184` lists 3 clauses; alias tests `:107-181`.
- **Same job elsewhere.** The first-failure rule duplicates what `build_purpose::judge_build` does for `RouteVerdict` lists (suspected,
  `build_purpose.rs` not read; `whole_subject_result` is also used there, grepped 8 total uses across 4 files).
- **Fail-open.** none seen. `:343-362` asserts that every generic form must be accepted for whole-subject acceptance.

### every_refusal.rs (223 lines, 5 tests)
- **Purpose.** A compilation reports every machine the checker cannot read yet, one verdict each, and each refusal is the one
  that machine meets alone (`:1-5`; mentions `crate::lowering::read::Program::keep`).
- **Shape.** `THREE_HELPERS` `:15`, `verdicts` `:28`, `not_supported` `:53` (collects `FallBack` details).
- **How it decides.** Keyed on diagnostic text: `ADDRESS_CAST = "expression cast to or from an address"` `:9`, suffix
  `"while reading the body of `first`"` `:72`, `"a call of a machine whose signature is refused above (`scaled`)"` `:127`,
  `"whole-referent restoration requires unqualified nonlinear owned storage without claims"` `:218-220`.
- **Stated limits.** `:12-13` casts to `addr` are refused (counts_and_addresses.md); `:171-172` the storage plan "refuses each as
  not supported".
- **Fail-open.** none seen.

### floats.rs (188 lines, 4 tests)
- **Purpose.** `f32`/`f64` as held and passed values; operators, casts and clause comparisons without a selected declaration are
  refused by name (`:1-9`).
- **Shape.** `verdict` `:15`; tests `:32`, `:75`, `:104`, `:144` (literal landing checks `IeeeFloatValue::Binary32/64` bits of 6 literals).
- **Cases by shape.** Refusals are an explicit list of six programs `:105-138` with exact texts like
  `"typing operand types while reading the body of `build_entry`"` and `"contract IEEE comparison without its selected declaration ..."`.
- **Stated limits.** `:9` "Operators, casts and clause comparisons outside the canonical primitive path still refuse."
- **Fail-open.** none seen.

### guard_contradiction.rs (248 lines, 4 tests)
- **Purpose.** Proves that `low <= high` and a taken `high < low` guard never hold together, using the kernel's value-equality
  denotation, over **every pair of 8-bit values**, both signs (`:17-111`: 256x256 per sign, `assert_eq!(pairs, 256 * 256)` `:109`).
  Also crash-guard disproof by a literal argument `:116-190` and a closed-term contradiction `:200-248`.
- **Calls.** `proof_admission::{check_value_equality_denotation, decide_primitive, check_certificate}`; `crate::checking::prove::Prover::new(..).prove`
  `:230-237`.
- **Cases by shape.** 8-bit integer only; sample `samples/cli/facts/wrapping_integer_arithmetic/local/main.omg` via `include_str!`
  `:201` (the comment `:192-198` records an earlier `RuleConclusionMismatch("predicate denotation")` failure).
- **Fail-open.** none seen.

### handoffs.rs (45 lines, 1 test)
- Purpose: a prepared Terminal input keeps contracts after the typed owner drops (`:1,6-45`); source `ensures result == 7`/`8`.
  Accepted at 7, `Rejected` with non-empty `unproved` at 8. No fail-open.

### hosted_primitives.rs (279 lines, 2 tests)
- **Purpose.** Compiler primitive custody (`ProcessExitPrimitive`) differs from an ordinary provider body; the published artifact
  must reject relabelled catalog declarations (`:1`).
- **Shape.** `program` `:5` takes sources from `bundled_declarations::sealed_intrinsics::hosted_process_exit_sources()` (outside this
  unit); `judge_program` `:31`; two tests `:51`, `:252`.
- **How it decides.** Keyed on **identity text and bytes**: it substitutes equal-length strings in the semantic bytes (4 pairs
  `:111-119`), requires every substitution to match (`:129`) and the decoder to fail with the text `"hosted primitive differs from its
  exact Core declaration and requirement"` `:135-137`; 7 field mutations of the boundary row `:206-249` must all fail decode
  (`is_err()`). It also pins format marker 270 as unsupported (`:141-146`).
- **Cases by shape.** `mutation` loop `0..7` with `match` listing 7 arms (`:222-241`).
- **Fail-open.** none seen. `:148-149` states the relocation controls "do not claim that this partial relocation is an executable
  module rewrite".

### indexed_contracts.rs (428 lines, 11 tests)
- **Purpose.** Indexed contract clauses (`self[4095] == 0`, `buffer.words[4095]`) name the actual array leaf, also through record
  fields, domains and recursive edges; mutable handback keeps the leaf.
- **Shape.** `invocation(text, root)` `:5` (entry chosen **by machine name**, `:8-11`); tests `:31-428`; `NATIVE` `:246` is
  `include_str!("../../tests/fixtures/calls/run/indexed_array_contract/main.omg")`.
- **How it decides.** Reparse test `:32-106` requires every unproved row to be `OperationSafety` and `statement.ends_with("cannot
  prove index is within length 8184")` `:96-99` (text key). `each_contract_index_is_checked_against_its_own_array` `:183` pins a
  `Violation` with text `"contract array index 2 is outside its fixed extent 2 while reading the signature of `entry`"`. Forged
  snapshot test `:364-428` expects detail text `"a returned storage snapshot is outside the call's established storage in machine
  `main`"` or `"a storage snapshot is not after a declared call in machine `main`"`.
- **Cases by shape.** Fixed `[u16; 4096]`, `[u16; 8184]`, `[Item; 2]`; the reparse test reproduces a Windows reparse record (`:35-44`).
- **Fail-open.** none seen. `:66-70` prints verdicts before asserting (debug aid).

### inferred_bounds.rs (197 lines, 3 tests)
- Purpose: what inference records about a value crossing a contract boundary: the interval derived for a computed actual,
  a bound that holds only by cases (a conjunction guard), and helper summaries recomputed once an omitted input is inferred.
  `with_private` `:8` sets `private_inputs`/`local_results`; `accepted`/`unproved` `:19-39`; tests `:53`, `:99`, `:150`.
  Keyed on machine id and `safety(&row.kind)` (helper from `tests.rs`). No fail-open.

### inferred_locals.rs (61 lines, 1 test)
- Purpose: an inferred `let` keeps the initializer's actual generic type and linear ownership. Positive: fixture
  `P06/tests/fixtures/ownership/run/inferred_generic_record/main.omg` (`:4-5`); negative `.../fail/inferred_generic_record_reuse/main.omg`,
  keyed on the text `"decomposition source is not a complete live value"` `:56`. `judged` `:9` uses `subjects: &[None]`, no entries.
  Prints verdicts with `println!` `:37,49`. No fail-open.

### layout_queries.rs (353 lines, 7 tests)
- **Purpose.** `address_of`, `size_of`, `align_of` use the same physical type in proof and execution; target-dependent layout is kept
  as a **pending prerequisite** rather than decided portably (`:135-137`: "Portable acceptance keeps this target prerequisite pending.
  The CLI twin must refuse it after selecting the actual layout; no early certificate may replace that check.").
- **Shape.** `assert_deferred_wrong_size` `:138-226` evaluates pending `ScalarCondition` predicates with assumed sizes 4/8; uses
  `terminal_semantics::layout_call_conditions`, `Discharged::TargetCondition`, `numerics::integer_bounds::evaluate` `:338`.
- **How it decides.** Expects `pending == 2` and `false_at_four == 1` (`:221-225`): counts, from running the retained conditions
  against hard-coded sizes (u32 size taken from the test, f64/addr 8, record 4, `:160-172`). `SELECT` guard table `:243-247`.
- **Cases by shape.** Hard-coded "64-bit test target" (`:157-158`).
- **Fail-open (test-side).** `a_live_field_only_proves_the_alignment...` `:37-60` uses `match` with arms only for the expected result and
  `other => panic!`. Elsewhere none seen.

### lifetime_elision.rs (348 lines, 10 tests)
- **Purpose.** Data that declares lifetime parameters may be named in a signature without them (elision); a bare occurrence stands for
  the declaration's one record; undeclared tags and elided views in fields are refused by name (`:1-4`).
- **Shape.** `violation` `:10` (single violation statement), `refusal` `:21`, `record_rows` `:223-260` (reads
  `prepared.source.forms[0].records` and their `application.is_some()`); tests `:29-348`.
- **How it decides.** Exact-message keyed: e.g. `"this type names the lifetime `'missing`, which its declaration does not declare ..."`
  `:53-54`, `"typing data field that elides a lifetime argument ..."` `:114`, `"typing result without a lifetime of a signature with
  several input lifetimes ..."` `:186`; the messages embed `package-owner(32:0707...07)` `:82,115,151,287`.
- **Stated limits.** `:324` `"type generic data with lifetimes, bounds, cleanup, a quotient or retired members"` (data with both a
  type and a lifetime parameter is not read); `:297-299` comment on which declarations give no record row; `:196-198` "what stops it
  then is the checker's own question about a reference kept in storage the caller lent, which no slice answers yet" (in loan_regions;
  see there).
- **Fail-open.** none seen.

### lifetime_generics.rs (519 lines, 10 tests)
- **Purpose.** Machines generic over a lifetime: one machine, judged once; a call instantiates each lifetime with the caller's
  region; one lifetime given two regions is refused (`:1-12`).
- **Shape.** `accepted` `:17`, `refusal` `:26`, `HOLDER` `:34`, `lifetime_requirement(contract)` `:201-233` (judges a
  `boundary trait` requirement with its own lifetime; asserts **2 verdicts**: the closure and the requirement form, `:239,262`),
  `selected_generic_helpers_keep_their_ordinary_lifetime_obligations` `:280-519` (the largest; builds `relay::<u32, make>` and
  forges the requirement `envelope` 3 ways).
- **How it decides.** Text-keyed: `"typing call that gives one lifetime two regions while reading the body of `build_entry`"` `:75`,
  `"typing assigned value type while reading the body of `named`"` `:114`. Forged envelopes must produce `ForeignIdentity` with
  prefix `"abstract envelope differs from its complete source declaration"` `:407,499`.
- **Cases by shape.** Calls into `typed.expression_table.static_machine_application(..)` and `machine_instantiation` of the typed
  trees (`:298-329`), so the test reads typed-tree internals.
- **Same job elsewhere.** The "two regions for one lifetime" check is also tested at `machine_lifetimes.rs:279-337` and
  `loan_regions.rs`.
- **Fail-open.** none seen.

### loan_regions.rs (342 lines, 7 tests)
- **Purpose.** One lifetime given the loans of two places: a loan's region lies inside the lent place's region; no named lifetime is
  ordered against another (`:1-14`).
- **Shape.** `CELLS` `:19`, `SELECTED` `:26`, `ENTRY` `:40`, `run` `:60` (judge + publish + interpret in 10,000-unit meter),
  `completes` `:131`, `verdict` `:143`.
- **How it decides.** `a_reference_backed_by_either_of_two_inputs_is_refused_by_name_when_returned` `:271-301` expects the exact
  text `"call reference result has alternatives from different input roots"` `:282`. `:199-203` expects
  `"a reference retained in caller storage needs its checked output-loan correspondence in machine `build_entry`"`.
- **Stated limits.** `:196-198` "what stops it then is the checker's own question about a reference kept in storage the caller
  lent, which no slice answers yet".
- **Fail-open.** none seen; interpreter-level `ChangeWhileBorrowed` text checked at `:128,341`.

### loan_windows.rs (53 lines, 3 tests)
- Purpose: a call writing a field through `@mutable` opens an invariant window on the enclosing record (`Span requires self.low <=
  self.high`). Uses `super::stored_references::run` (`:8`, outside unit). Positive: restore `s.low = 1;` `:38`; refused text
  contains `DataDomain` and `clause: 0` `:45`; lending a field the domain does not read (`spare`) is accepted `:52`. Header `:4-6`
  records the old defect ("The checker used to owe nothing for such a call"). No fail-open.

### loops.rs (248 lines, 7 tests)
- **Purpose.** Call cycles lowered as loops: a head's invariant is proved on every arrival and is all that is known of the head's
  parameters (`:1-2`).
- **Shape.** `countdown(requires, next)` `:28`, `mutual` `:137`; checks one `BlockInvariant` knowledge row with
  `available_from == ProgramPoint::BlockEntry(id(2))` `:79-91`.
- **How it decides.** `an_invariant_row_that_misses_an_arrival_refuses_the_machine` `:194-200` pops one `arrivals` entry and expects
  `ForeignIdentity`. `a_cycle_checks_the_entered_guarantee_at_each_members_return` `:203` expects `Discharged::EveryExit` with 2 exits.
- **Fail-open.** none seen.

### machine_inputs.rs (102 lines, 1 test)
- **Purpose.** `PreparedMachineInputs` (crate::checking::machine_inputs) lets a machine keep authored branch diagnostics except for
  declared **specializations** (`:5`). Test forges `contract.declaration` with a `specialization: Some(..)` on machine 2, adds a
  duplicate and removes machine 3, then checks `terminal_check_tables::program::authored_machine_sources` returns `[id(1), id(3)]`
  `:75-82` and that `inputs.for_machine(id(2)).branches.is_empty()` `:90`. Keyed on the presence of
  `declaration.specialization` (`:68-71`); comment `:51-52` "Preserve the old existential rule even for duplicate metadata and an
  unmatched source owner". No fail-open.

### machine_lifetimes.rs (785 lines, 18 tests)
- **Purpose.** Machines that declare lifetime parameters and calls passing byte views; elision rules; caller-chosen result lifetimes;
  the published declaration header keeps the result lifetime relation across encode/decode (`:1-11`, `:99-144`).
- **Shape.** `verdict` `:17` (returns `"accepted"`, `"rejected"`, or the refusal text), `published_label_signature` `:33-95`
  (publishes under `TerminalPublication::Portable`, encodes with `terminal_codec::encode_module`, decodes, compares
  `(parameter_types, result_type, contract)` of the declaration whose callable identity **contains the substring "label"** `:66-68`);
  `LITERAL_MATCH` `:417` (fixture `calls/run/caller_chosen_literal_match/main.omg`); `DATA_QUALIFIED_MACHINE` `:626`.
- **How it decides.** The `label` lookup is **source spelling** (`identity.contains("label")` `:67`). `a_literal_arrival_source_
  retains_its_exact_establishment` `:455-530` checks `terminal_semantics::structural_argument_source` returns `None` for a missing,
  duplicate, `Result` kind or `TrivialAffineLocal` kind producer row (`:499,512,528`).
- **Cases by shape.** `a_result_lifetime_no_input_carries_is_chosen_by_the_caller` `:386` lists 4 programs; verdict strings for
  elision `"typing result without a lifetime of a signature with several input lifetimes while reading the signature of `pick`"` `:362`.
- **Fail-open.** none seen.

### mathematical_clauses.rs (462 lines, 11 tests)
- **Purpose.** Contract clauses over mathematical integers (`embed(x) + embed(y)*10 <= 99`): a caller owes them over its own actuals;
  the kernel evaluates closed comparisons; the prover combines operand bounds (`:1-16`).
- **Shape.** `embedded` `:24`, `calling(requires, clause, rest, count)` `:56`, `requirement_of` `:79`, `cause_of` `:85`.
- **How it decides.** Checks `UnprovedCause::NoDerivation` versus `NoRuleForGoalForm` and `RouteVerdict::Incomplete` (`:130,162,202`).
  `rule_coverage_of_a_mathematical_goal_is_read_from_its_form` `:305-358` calls `crate::checking::has_rules_for(..)` on 14 goal
  shapes and pins which have rules: `packed_clause()` (sum with product) has **none** (`:328`); `embedded(1) == 7` has none (`:340`);
  mixed-width product has none (`:338`).
- **Cases by shape.** Rules are shape-selected: closed, open-equal, sum of two same-carrier values, `x + 1`; the packed product-in-sum
  goal is checked by bounds but reported as incomplete when bounds fail (`:198-208`).
- **Fail-open (stated by tests).** `:198-199` "Failure to bound this compound expression is reported as incomplete" - an `Incomplete`
  verdict, not an `Accepted`.

### mutable_formals.rs (457 lines, 11 tests)
- **Purpose.** A `mutable x: u32` formal is the callee's own storage; entry `requires` is a fact about the entry value only, and a
  write to it (scalar rebinding or record field store) invalidates that fact; the caller's argument is untouched (`:1-16`).
- **Shape.** `headroom` `:26`, `route_accepts` `:30`, `unsafe_operations(verdict)` `:39-57` (pub(super); 8 uses), `field_at_most_ten`
  `:197`, `stored_then_read` `:213`; fixture `tests/fixtures/ownership/run/borrowed_root_replacement/main.omg` `:315`.
- **How it decides.** Keyed on field index (`field(named(0, pair()), read)` read vs `bounded` field; sibling field keeps its fact
  `:243`) and exact message `"this place requires mutable source access, and the reference it is reached through grants shared access
  only ..."` `:365`, `"whole-value replacement cannot implicitly discard a linear value"` `:396`, FallBack text
  `"statement whole assignment through a reference while reading the body of `build_entry`"` `:455`.
- **Stated limits.** `:10-14` names `prepared-typed-trees-to-lowered-program/src/read/machine.rs` as the reader deciding which formals
  may be written (path from an older crate name; not checked).
- **Fail-open.** none seen.

### operational_effects.rs (1047 lines, 25 tests)
- **Purpose.** Blocking is an interface promise (`blocks`) plus an independent per-call acknowledgement (`block`/`suspend`); suspension
  crossings with live frontiers; tail calls keep the callee's interface (`:1`).
- **Shape.** `BLOCKING` `:5`, `SUSPENDING` `:150`, `LOOP` `:328`; `invocation` `:19`, `elaborate` `:27` (entry **by name "main"**
  `:35`), `accepted` `:62`, `violation(text, expected)` `:69` (substring of `violation.message()`).
- **How it decides.** Keyed on message substrings `"blocks ceiling is false"` `:88`, `"requires a block acknowledgement"` `:92`,
  `"requires a suspend acknowledgement"` `:210`. `a_missing_or_altered_crossing_is_not_accepted` `:218-281` runs
  `crate::checking::operational::judge` on four forged states and pins four exact `ForeignIdentity` messages (`:246,256,263,279`).
  Frontier/loan tests inspect `TerminalSuspensionCallPlan` (`live_values`, `loans.occurrences`, `backing`) `:781-1047`.
- **Cases by shape.** Marker table `:83-105`; local-borrow shapes (struct local, scalar through loan, fixed array element with
  `FixedIndex(0)`) `:866-916`.
- **Stated limits.** `:283-285` "a possibly suspending terminal call is still refused by name"; `:706-723`
  `"selected public contract on a call cycle"` is a `FallBack`/`Unsupported`; `:1046-1047` literal established after a wait has no
  home in the frontier.
- **Fail-open (test-side).** `a_terminal_call_that_may_suspend_is_refused_by_name` `:287-309` accepts **either** `FallBack` with a
  given message or `Accepted` with a crossing (the comment `:297-298` says "The route lowered the final call as an operation").
  So that test passes in two states; if the refusal were removed, it would still pass when a crossing exists.

### optimization.rs (88 lines, 2 tests)
- Purpose: source validity is decided before the optimizer rewrite; the artifact binds the selected pass and evidence to the
  rechecked program (`:1-2`). `publish(source, selections)` `:6-31` calls `lower_for_publication` then
  `crate::checking::judge_publication` (the function production uses, per crate map). Positive: selecting
  `PsiOptimization::SparseConditionalConstantPropagation` removes the `BooleanNot` (count 1 -> 0 `:54-55`); record's `input_semantic`/
  `output_semantic` differ and `selected.seals.module == record.output_semantic()` `:56-64`. Negative: `StateSpecialization` yields
  exactly one message `"not supported yet: source Psi optimization `StateSpecialization` has no implemented pass"` `:85` (keyed on the
  enum name). Invalid source's refusal messages are identical with or without a selection `:81`. No fail-open.

### owned_cleanup.rs (401 lines, 6 tests)
- **Purpose.** A by-value consumer (`dropped by dispose`) owes its authored `requires` at disposal, proved at the exact cleanup action
  (`:1`).
- **Shape.** `program(valid, receiver, body, effects)` `:6`, `DECOMPOSE` `:26`, `judged` `:34`, `accepted` `:38`.
- **How it decides.** Cleanup call rows are found by `InvocationSite::Cleanup { edge, action_ordinal }` (`:76-79`); requires obligations must
  have `created_at == ProgramPoint::BeforeCleanup { .. }` `:87`. Receiving test `:144-234` pops/swaps/relocates one row (3 mutations)
  and expects `ForeignIdentity`. Refusals keyed on text: `"recursive generated cleanup"`, `"incompatible value parameter"`,
  `"abnormal completion"` `:239-246`; `"decomposition source is not a complete live value"`, `"record pattern that omits a field"`,
  `"normal edge cannot implicitly dispose storage without an authorized disposition"` `:285-308`.
- **Cases by shape.** `linear_cleanup_transfers_the_receipt_claim_exactly_once` `:323-401` expects exactly one claim transfer, then
  duplicates/clears it and expects the text `"cleanup transfers differ"`.
- **Same job elsewhere.** `terminal_semantics::plain_affine` is also exercised in `owned_dispatch.rs` and `references.rs`.
- **Fail-open.** none seen.

### owned_dispatch.rs (305 lines, 4 tests)
- **Purpose.** Dispatch over a non-`[copy]` owned sum: each arm owes only what its selected case holds; the subject is disposed on
  an edge that never names it (`:1-12`).
- **Shape.** `DATA` `:19`, `verdict` `:34` (first `unestablished.detail`), `selecting` `:50`; edge tests edit
  `Terminator::StructuralCase` cases `:109-305` (`plain_affine::verify` `:126,145`).
- **How it decides.** Exact text `"owned storage is used after its value was transferred or consumed in machine `route`"` `:102`;
  `plain_affine::Error::Malformed { detail == "normal edge affine disposal differs from its canonical schedule" }` `:150`,
  `Error::Unsupported { detail == "disposal of storage the affine schedule does not own" }` `:165`; decode error
  `CodecError::MalformedStructuralFoundation("whole-root discard has no owned affine source")` `:301`. The copyable case permits
  `RetireHome` but not `DiscardRoot` on the same edge (`:258-282`).
- **Fail-open.** none seen.

### owned_match_results.rs (58 lines, 3 tests)
- Purpose: a match hands its owned selected value to one continuation; no copy permission arises (`:1-3`). Source `Choice`/`Item`
  with `consume(selected)`; second consume produces `unestablished` text `"owned storage is used after its value was transferred or
  consumed in machine `build_entry`"` `:44`; an ended arm (`crash Trap`) supplies no join value `:48-58`. No fail-open.

### owned_records.rs (159 lines, 3 tests)
- Purpose: whole move preserves facts for a bounded call; a no-normal borrow keeps the caller's affine owner suspended (`:68`).
  Uses `elaborate_over`, `affine_records`, `point_of`, `bounded` (helpers in `tests.rs`, not read). `moved_record(5)` accepted,
  `moved_record(20)` rejected with a `CallRequirement` row; the third test builds bodies by hand and asserts
  `Terminator::InvokeNoNormal` exists and all cleanup rosters are empty `:149-154`. No fail-open.

### parallel_checks.rs (97 lines, 2 tests)
- **Purpose.** Per-machine proof jobs run in parallel (`worker_budget::ExecutionBudget::with_workers`), but the results and the
  refusal chosen must equal the sequential run.
- **Shape.** `cohort(valid)` `:5` = entry + helper + 30 `add_of_formals` machines; test `:30` runs workers `[1,2,4,8]` and compares
  `format!("{elaboration:?}{result:?}")` byte-for-byte `:60-64`; second test `:72-97` clears `knowledge` on all machines except
  ids 2 and 17 and requires the **lowest-id** failing machine (id 2) with detail `"knowledge rows already present"` `:93-94`.
- **How it decides.** String compare of `Debug` output (`:60`), and machine identity ordering for the chosen refusal.
- **Fail-open.** none seen.

### proof_scope.rs (122 lines, 1 test)
- Purpose: a declared body's `ProofScope` rows (`ProofScopeDeclaration::Callable`, `ProofCallableAssociation`, `ProofCallableHeader`)
  stay tied to the exact owner. `declared_body()` `:8-43` hand-writes the scope; test `:46-121` expects the header's
  `supplies_checked_body` false before and true after acceptance `:53-73`, and three forged states
  (changed `contract` bytes, orphan header with `supplies_checked_body = true`, missing headers) each give
  `Refusal { reason: ForeignIdentity }`. Comment `:47-48` "This is a declaration-custody model, not an invocation or authority
  witness." No fail-open.

### provider_supplies.rs (610 lines, 12 tests)
- **Purpose.** A supplied provider (`Native satisfies Service`) is judged as a root against its exact requirement; part 0 is the runtime
  closure, part 1 the refinement witness (`:1-3`, `:266-281`).
- **Shape.** `typed` `:9` (`frontier_of`), `program(required, supplied, body)` `:18`, `declared` `:38`, `supplied(text, slot)` `:62`
  (returns machine display paths of the accepted closure or `"part N rejected: ..."`/`"part N not supported: ..."`), `CONTRACT` `:176`.
- **How it decides.** Keyed on **display path** `"Native::go"` (`machine.ends_with("Native::go")` `:344`, `display_path(..., "::")`
  `:172`) and on message text: `"provider does not give the supplied requirement exactly one machine"` `:462,533`,
  `"boundary conformer is callable only through its provider slot"` `:553,590`, `"slot that is not a trait or one of its requirements"`
  `:539`, `"whole-trait supply with parameters, parents or refinements"` `:261`, `"a data declaration cannot own a machine; declare
  an ordinary static machine"` `:247`. Rejection part numbers are asserted (`starts_with("part 1 rejected")` `:328,401,416,481`).
- **Cases by shape.** `Supply { slot, provider }` with slot a requirement or a whole trait; 6 provider candidates counted in
  `provider_candidates.len() == 6` `:222` from fixture `tests/fixtures/providers/run/target_selected_implementation/main.omg` `:182`.
- **Stated limits.** `:251-252` inherited-only whole-trait supply is "not vacuously accepted" (unserved).
- **Fail-open.** none seen. `:1-3`/`:178-181`: all candidates still publish when one is selected.

### published_no_return.rs (225 lines, 5 tests)
- Purpose: non-return is a callable fact; an abstract requirement does not expose the stronger private outcome of the body chosen to
  implement it (`:1-2`). `conformance(inline, required, supplied, caller)` `:7`; uses `build_purpose::judge_build_all`. Positive-for-
  private-local: a private or same-unit public `stop()` that crashes ends the caller's path (`:101-122`, `:212-225`; checks
  `unreachable_statements().count() == 1`). A dependency body (two `add_checked_instance` sources, `:128-181`) is **not** read as
  non-returning (`:199-206`). Keyed on `OperationSafety`/`CrashCoverage` rows. No fail-open.

### qualified_terms.rs (1141 lines, 33 tests)
- **Purpose.** Removing a qualification (`as -Domain`) keeps the value and its facts without permitting re-issue; domain casts, domain
  predicates for arrays/records, restoration of domains at handback and after mutable views (`:1-2`).
- **Shape.** `UNIT_DOMAIN` `:28`, `DOMAIN` `:142`, `invocation` `:154`, `consumer` `:176`, `boundary_consumer` `:228`,
  `typing_error` `:376`; `CYCLIC_BYTE_WRITE` `:1104`.
- **How it decides.** Forged-module tests: replace a call argument with an unqualified constant of the same carrier and expect
  `"scalar call argument does not preserve its complete declared type"` `:81`; clear `coercions` and expect
  `"scalar successor changes membership without its explicit cast or checked completion"` `:94`; `catalog.domains[0].establishment_routes
  = Some(Vec::new())` plus a forged `ScalarReturnQualification` expects `"authorized issuer"` `:139`. Typing refusals are exact text
  (`"builtin operator `!=` takes unqualified operands only ..."` `:392`). `removing_a_domain_does_not_authorize_its_reintroduction`
  `:429` expects detail containing `"not one of the domain's establishment routes"`. Row-shape checks via
  `ObligationKind::{CallRequirement, NormalGuarantee, ArrivalInvariant, DataDomain}`.
- **Cases by shape.** Byte arrays `[u16; 8]`, `[u8; 8]`, `[u16; 4]`; variable index through `i32 in Wrapping`; shared/mutable views;
  loops `fresh_buffer_views_end_before_the_next_loop_iteration` `:1082`.
- **Stated limits.** `:635-636` "Qualified scalar loans remain unsupported before this body is read"; test expects
  `RefusalReason::Unsupported` and text `"reference to a qualified scalar"` `:638-641`. `:368-372` boundary membership of a second
  formal: `Unsupported`, `"contract routed contract membership needs its exact normal result subject ..."`.
- **Fail-open (test-side).** Several negatives accept either `DataDomain` unproved **or** non-empty `unestablished`
  (`:761-763`, `:798-800`, `:1054-1055`), so a refusal in the other category would also pass.

### ranges.rs (810 lines, 25 tests)
- **Purpose.** Range propagation through `/`, `%`, `<<`, `>>`, `&`, `|`, casts and guards; each test binds `local = a <op> operand` and
  adds a literal so the add is representable exactly at the propagated bound and one more is unproved (`:1-4`).
- **Shape.** `after(...)` `:10`, `assert_upper(requires, first, operand, upper)` `:43-61` (positive at `MAX - upper`, negative at
  `+1`), `signed_remainder` `:193`, `dividend_less_quotient` `:271`, `branching` `:417`, `after_a_cast` `:492`.
- **How it decides.** Keyed on numbers (the exact upper bound) and the kernel re-decision; `an_exact_cast_transports_the_guard_for_
  the_same_byte_slice` `:64-114` uses `prepare_replay` + `reverify` on every `Certificate`, then two negatives with row text
  `"requires contract for call consume"` `:110`. `an_exact_polynomial_multiple_has_zero_remainder_and_refuses_odd_twins` `:638-753`
  reads 14 sample files `samples/cli/facts/integer_multiple_remainder/{local,field,parameter,match_join,const_name,other_state,
  other_machine}/main.omg` and their `invalid_*` twins via `include_str!` with `../` x8 paths (`:643-701`).
- **Cases by shape.** A closed list of operators: Divide, Remainder, BitwiseAnd, BitwiseOr, ShiftLeft, ShiftRight, Cast.
  Signed remainder test for divisors `>=2` and `<=-2` only; divisors that admit zero or -1 get no bound (`:240-266`).
- **Fail-open.** none seen. `a_unit_divisor_can_hit_the_exclusive_end_of_a_slice` `:795` requires only `OperationSafety` rows
  with text `"cannot prove operation safety"` `:806`.

### rankings.rs (687 lines, 12 tests)
- **Purpose.** Loop progress: finite-integer rankings use kernel-checked edge goals; `terminates by <measure> -> Nat::Descending`
  must strictly decrease on every cycle; ranking views come only from the sealed catalog (`:1`).
- **Shape.** `ranked(signed, tuple, stalls)` `:10-151` builds a lowered one-block recursive machine, then **hand-writes** a
  `TerminalRankedScc::Natural` with `TerminalNaturalCycle`, ranks, and edges (`Strict`/`Preserving`) and pushes `WellFoundedness`/
  `RankComparison` obligations `:133-148`; `typed_with_ranking_catalog(text)` `:263-312` parses the **prefix of
  `source/library/core/nat.omg` up to the comment `"// The core algebraic"`** (`:271-274`) and binds `Nat::Descending` as a module import.
- **How it decides.** `exact_ranking_inventory_rejects_missing_surplus_swapped_and_stale_claims` `:194-258` runs 8 faults (pop rank,
  pop edge, swap ranks, reverse edges, retarget edge, duplicate obligation, drop last obligation, wrong integer type) and requires
  `is_err()`. Refusal text: `"removing strict rank edges"` `:351,393,615`; `"contract selected ranking view is not an admitted compiler
  ranking declaration while reading the body of `countdown`"` or `"... has no admitted resolved selection ..."` `:476-477`.
  Authored lookalike `machine Nat::Descending(..)` and `Nat::BoundedDistance` do not get catalog authority (`:454-519`).
  The subject check texts `"non-collection storage"`, `"cannot embed exactly"`, `"unsigned integer subject"`, `"exactly one subject"`
  `:528-565`.
- **Cases by shape.** Rank carriers: single unsigned, single signed 32-bit, a 2-tuple (`ranked(.., tuple, ..)`); catalog views
  `Nat::Descending`, `Nat::BoundedDistance`, `Slice::Length`.
- **Stated limits.** `:260-262` only the ranking declaration prefix of `nat.omg` is needed.
- **Fail-open (test-side).** Negative tests at `:342-355`, `:611-616` accept two different refusal categories (unproved
  `RankComparison` or a `FallBack` containing `"removing strict rank edges"`).

### reach.rs (187 lines, 5 tests)
- **Purpose.** Which unproved goals are `Incomplete` rather than `Rejected` (`prove::reach`): a goal that depends on a value defined by
  an operation outside the integer rules (e.g. `^`, quotient by a value) in the same machine or behind an inferred input/result is
  `Incomplete`/`NoRuleForGoalForm`; failures on exactly-bounded operations are `NoDerivation` rejections (`:1-4`).
- **Shape.** `incomplete` `:8`, `no_derivation` `:23`, `headroom_of` `:50`, `private` `:38`.
- **How it decides.** `UnprovedCause::NoRuleForGoalForm` vs `NoDerivation`; operators `BitwiseXor` and `Divide` by a value are the
  "unreached" ones (`:65-98`). `:148-153` a private helper's result is "unreached" only when listed in `local_results`.
- **Fail-open.** An `Incomplete` verdict means the program is not accepted. none seen.

### references.rs (1797 lines, 24 tests)
- **Purpose.** Formals passed by reference and the loans a call forms; conflicts between loans; reborrow lineage; returned references;
  shared reference formation owns its permission carrier (`:1-10`).
- **Shape.** `Lending` `:21`, builders `:29-229` (`unit`, `pair`, `records` with a 2-field `Pair` `:43`, `lower_lending` `:62`, `named`,
  `field`, `lend`, `invoke`, `store`, `pair_of`, `bump`, `two`), `accepted` `:231`, `conflicts` `:238`; `HANDOFF` `:1573`,
  `rigid_relay_with` `:1585`, `returned_relay` `:1719`. `LoanConflict` variants asserted: `ExclusiveAndShared` `:376`, `TwiceExclusive`
  `:441`, `ExclusiveOfShared` `:597`, `ChangeWhileBorrowed` `:1035`, `ObserveWhileExclusive` `:1556`.
- **How it decides.** Judged by loan **row kind** and lender: `ObligationKind::Loan { access, lender: Owner|Incoming|Provider|Loan(_) }`
  (`:281-288,582-586`); a forged `access = Shared` on the only loan row yields `ForeignIdentity` `:630-638`. Selected-call tests
  erase `operation.required_contract` and expect detail `"invocation row and lowered call name different targets in machine `relay`"`
  `:788`; erase `result_lifetimes` and expect `"public reference result has an unresolved declared lifetime frontier"` `:826`.
  Origin-source forgery expects `"returned reference header differs from its actual body origins"` `:950`. Plain-affine plan
  errors `"place custody disagrees with its exact type declaration"` `:1207`, `"permission result has no exact reference producer
  declaration"` `:1382`; `OperationSemanticError::StructuralEffectActionShapeMismatch` `:1406`.
- **Cases by shape.** Fixture reads: `samples/cli/facts/reference_arrival_handoff/{call_result,invalid_call_result}/main.omg` read by
  `std::fs::read_to_string` at runtime (`:860-865`, the only runtime file read in this unit); `tests/fixtures/ownership/{run,fail}/...`
  by `include_str!` `:1048,1229,1251,1262`.
- **Stated limits.** `:1204-1207` `plain_affine::plan` must refuse a retagged copy; `:1282-1286` the direct producer lookup and
  row validation "share one lifecycle catalog" (`StructuralEffectSemanticRow::ALL` / `for_tag`).
- **Fail-open.** none seen.

### representation_views.rs (423 lines, 8 tests)
- **Purpose.** `view as @[T]` re-views a complete shared window as another element representation when the target tiles the source
  exactly, both are padding-free, and the count fits `u64`; mutable re-views are refused (`:1-12`).
- **Shape.** `verdict` `:18` (`"accepted"`, `"rejected"`, `"not supported: <detail>"`), `refused` `:28`; tests `:36-423`.
- **How it decides.** Text-keyed refusals `UNTILED = "a whole-representation view whose target elements do not exactly tile its source
  elements"` `:164`, `"expression exclusive storage view without establishment and handback evidence"` `:188`, `"reference formation
  changes its captured referent type"` `:274`, `"an array view without plain element storage and unclaimed backing"` `:296`, `"a byte
  view re-views elements that are not bytes"` `:413`. Alteration test replaces the `element` of the one `EstablishElementView` with a
  one-field wrapper record and requires both a codec `SealMismatch` (`:401-405`) and a checker `ForeignIdentity` (`:409`).
- **Cases by shape.** `i32 -> u8` tiling, `Pair{u16,u16} -> u16`; refused: `u16 -> u32`, padded `{u8,u32} -> u8`, `@mutable [u8]` re-view.
- **Stated limits (important, with owner pointer).** `scalar_reference_recasts_keep_their_backing_and_presented_fields` `:37-107`
  **asserts a refusal**: `"reference descriptor has no current origin account in machine `entry`"` `:88`, with comment `:78-84`
  naming `TASKS.md` item STATE-LOCAL-VALUE-FRONTIER. So the program the test is named for is currently refused, not accepted.
- **Fail-open.** none seen. The competing-write twin `:100-106` accepts either `Rejected{conflicts}` or the same `FallBack`; it
  "must never be accepted" (`:98-99`).

### result_fields.rs (406 lines, 7 tests)
- **Purpose.** Return-field promises (`ensures result.first == input`) bind the exact returned storage and then remain facts about
  that value; a later write changes only its own field (`:1-2`).
- **Shape.** `SOURCE` `:7`, `TWO_CALLS` `:57`; `borrowed_completion_observations_cannot_reuse_input_snapshots` `:91-211` loops 3
  variants `(input, extent, owned_pair)`, forges the contract's `ensures[0].proposition` to the matching `requires` premise
  (found by `ScalarTerm::IntegerField` root and path, or `ViewExtent`), and expects acceptance for an owned pair or an extent
  premise but a `NormalGuarantee { clause: 0 }` `NoDerivation` rejection for a changed **borrowed** field `:192-209`.
  `borrowed_field_guarantees_connect_entry_and_completion_at_each_call` `:214-311` uses `calling_policies::prepared` (outside unit).
  `owned_field_guarantees_keep_the_input_separate_from_working_storage` `:314-406` forges `ContentPlaceVersion::Entry` on the result
  term and expects decode failure `PropositionError::EntryNonInputStructuralPlace`; also format marker 264 unsupported `:372-376`.
- **Fail-open.** none seen.

### result_summaries.rs (283 lines, 7 tests)
- **Purpose.** Private helper result summaries: a caller may use what a private helper returns (no `ensures` written), with every
  certificate kernel-checked; a public/boundary helper crosses only its written contract (`:40-75`).
- **Shape.** `through(helper, limit, entry_requires)` `:5`, `identity` `:20`, `with_local_results` `:27`, `accepted` `:33`.
- **How it decides.** `local_results` list controls which machines are summarised (`&[2]`); summary of two differing returns is
  `result <= 60` (`LessOrEqual`) `:111`; computed result crosses as bounds `:121-147`; summaries compose callees first `:150-171`.
  `private_generic_results_stay_inside_the_verification_unit` `:174-283` varies 4 flags `(public, same_unit, false_result)`
  (`subjects: [None]` vs empty): only a private, same-unit, false-result helper gets summaries (2 machines carry `result == false`
  `:252-255`); others must fail with a `CallRequirement` row whose text contains `"call hold"` and `"direct == false"` `:261-263`;
  templates must all be accepted `:274-281`.
- **Fail-open.** none seen.

### returned_views.rs (1475 lines, 33 tests)
- **Purpose.** A view or reference a boundary (provider) returned, handed on by the machine that called it; provider-owned bytes lie
  outside every input object but the declared lifetime groups keep input loans live (`:1-10`).
- **Shape.** `elaborate` `:14`, `verdict` `:36` (`"accepted"`, detail, or `"rejected: N conflicts"`), constants `RETURNED_VIEW` `:322`,
  `PROVIDER_VIEW` `:681`, `PROVIDER_REFERENCE` `:692`, `PROVIDER_RECORD_REFERENCE` `:698`, `DIRECT_PROVIDER_FIELD` `:705`,
  `PROVIDER_NESTED_RECORD_REFERENCE` `:714`; helpers `provider_permission_cannot_be_claimed_as_owned_storage` `:430`,
  `provider_reference_has_no_captured_address` `:759`, `provider_reference_value_is_unknown` `:826`.
- **How it decides.** Many forged-table checks: change `Lender::Provider` to `Lender::Owner` and require `ForeignIdentity` with
  prefix `"reborrow names a different immediate parent occurrence"` `:470,543,1472`; element loans keep `Lender::Provider` or
  `Lender::Loan(_)` `:523,1452`. Boundary `result_lifetimes` corruptions 0..5 (`:988-1014`) and 0..6 (`:1064-1117`) must make
  `boundary_result_lifetimes`/`boundary_result_slots` err **and** make `judge` not accept (`:1110-1116`, "the sole checker accepted
  corruption"). Descriptor tests call `terminal_semantics::descriptor_results::{DescriptorAnalysis, terminal_borrowed_storage_graph_from}` and
  `descriptor_lifetimes::{descriptor_location}`.
- **Cases by shape.** Dynamic `CallDynamic` operation synthesised by hand `:1164-1176` to get `"returned dynamic view needs its
  declared lifetime contract"` `:1192`. Stored callable test `:579-611` expects
  `"dynamic row returned view exceeds its public input regions in machine `second_view`"`.
- **Stated limits.** `:476` "The provider-backed case also needs its executable call-flow account" (comment); `:621-626`
  `boundary_reference_completion_does_not_invent_a_captured_address`: `captured_storage_family` must return
  `Err("reference descriptor has no supported captured address account")` `:812`.
- **Uses outside unit.** `super::stored_references::run` `:1336,1377`, `super::calling_policies::prepared` `:195`.
- **Fail-open.** none seen.

## Same job elsewhere / duplicated mechanisms in this unit
- `verdict(&str) -> String` helper: 15 definitions across `T` (list above; 8 in this unit). Each re-implements "judge, publish if
  accepted, map refusal to text" with slightly different mapping of `Rejected` (first violation, first unestablished, count of
  conflicts, or just `"rejected"`): `case_payloads.rs:19-40`, `floats.rs:15-29`, `machine_lifetimes.rs:17-28`,
  `owned_dispatch.rs:34-48`, `returned_views.rs:36-45`, `representation_views.rs:18-25`, `loan_regions.rs:143-153`.
- Hand-built `Compilation { calling: None, providers: None, declaration_metadata: &[], entries, subjects, purpose: Product, selected: &[],
  bindings: &[], supplies: &[] }`: 26 times in 23 files here (grepped), against `build_purpose::roster` (`build_purpose.rs:170`).
- Hand-built `run` (judge, publish, interpret): `loan_regions.rs:60` and `stored_references.rs:133` (outside unit) are twins;
  `execute_owned_store` (`case_payloads.rs:555`), `execute_observed` (`compiler_primitive_headers.rs:277`), `executed_source`
  (`contract_formation.rs:261`), `execute_borrowed_replacement` (`mutable_formals.rs:324`) repeat publish+instantiate+invoke.
- The "forge one table row, expect `ForeignIdentity`" pattern (see shared shape) is repeated across 17 files here; each uses a different
  row and exact detail string.

## Unused or one-caller code
- `entryless::judged_typed` (`entryless.rs:11`): grepped `entryless::judged_typed\b` over `T`: 1 use outside its own file
  (`termination.rs`), plus its own use at `entryless.rs:8`.
- `mutable_formals::unsafe_operations` (`:39`): 8 uses (7 in `arithmetic_policies.rs`, 1 own file).
- `rankings::typed_with_ranking_catalog` (`:263`): 17 uses in 4 files (not unused).
- Statements in comments saying "single/shared" mechanism: `entryless.rs:25-27` (first-failure rule); I grepped `whole_subject_result`:
  8 occurrences in 4 files (`build_purpose.rs`, `byte_view_elements.rs`, `entryless.rs`, `type_parameters.rs`).
- Test-only `pub(super)` helpers with only one external user: `entryless::judged_typed` (1). All others above have >= 4 users.

## Stated limits recorded by the tests (quoted, under 20 words)
- `collection_members.rs:651` "Source storage cycles are currently unserved."
- `representation_views.rs:78-84` refusal pinned until TASKS.md STATE-LOCAL-VALUE-FRONTIER restores execution.
- `loan_regions.rs:197-198` "which no slice answers yet" (retained reference in caller storage).
- `qualified_terms.rs:635` "Qualified scalar loans remain unsupported before this body is read."
- `operational_effects.rs:283-285` "a possibly suspending terminal call is still refused by name".
- `operational_effects.rs:707-709` selected recursive call: "must retain its public call association before that lowering is served".
- `ceilings.rs:358-359` "execution through that entry identity remains explicitly unsupported."
- `layout_queries.rs:135-137` pending target prerequisite: "no early certificate may replace that check."
- `provider_supplies.rs:251-252` inherited-only whole-trait supply "must not become an empty accepted supply".
- `floats.rs:9` "Operators, casts and clause comparisons outside the canonical primitive path still refuse."

## Fail-open risks
- In the code under test: none seen in this unit (these are tests; they pin refusals). I did not read the checker implementation.
- In the tests themselves (a regression that changes the refusal would still pass; classified "traced" = I read the assertion):
  1. `operational_effects.rs:287-309` passes with either `FallBack` (named message) or `Accepted` with a crossing (comment `:297-298`).
  2. `representation_views.rs:100-106` and `:87-90` pass with `Rejected{conflicts}` or the named `FallBack`.
  3. `rankings.rs:342-355`, `:611-616`: pass on `RankComparison` unproved OR `FallBack` containing "removing strict rank edges".
  4. `qualified_terms.rs:761-763, 798-800, 1054-1055`: pass on `DataDomain` unproved OR any non-empty `unestablished`.
  5. `compiler_primitive_headers.rs:707-713`: passes if **any** verdict is a `Rejected` with a `DataDomain` row.
  6. `layout_queries.rs:49-58`: for the rejecting case the only requirement is non-empty `unproved`.
  7. `owned_dispatch.rs:42-45` `verdict()` for `Rejected` returns the first `unestablished` detail and panics if empty; not fail-open.
  8. `compiler_primitive_headers.rs:431-438`: asserts at least one verdict is not `Accepted` (does not say which).
- Tests that accept a table that is missing a row: `ceilings.rs:797-801` ("The generic declaration still establishes the relay's
  suspension bound", stated reason). Stated.

## Coverage
Read in full, in order, in chunks of the whole file (one Read call per file, no offset): all 46 ranges in `units/unit_067.tsv`, line 1 to
last line, total 21,801 lines:
`case_payloads.rs` 1-771, `ceilings.rs` 1-979, `collection_members.rs` 1-933, `compiler_primitive_headers.rs` 1-715,
`connected_facts.rs` 1-64, `contract_formation.rs` 1-949, `empty_records.rs` 1-187, `entry_inputs.rs` 1-1044, `entryless.rs` 1-500,
`every_refusal.rs` 1-223, `floats.rs` 1-188, `guard_contradiction.rs` 1-248, `handoffs.rs` 1-45, `hosted_primitives.rs` 1-279,
`indexed_contracts.rs` 1-428, `inferred_bounds.rs` 1-197, `inferred_locals.rs` 1-61, `layout_queries.rs` 1-353,
`lifetime_elision.rs` 1-348, `lifetime_generics.rs` 1-519, `loan_regions.rs` 1-342, `loan_windows.rs` 1-53, `loops.rs` 1-248,
`machine_inputs.rs` 1-102, `machine_lifetimes.rs` 1-785, `mathematical_clauses.rs` 1-462, `mutable_formals.rs` 1-457,
`operational_effects.rs` 1-1047, `optimization.rs` 1-88, `owned_cleanup.rs` 1-401, `owned_dispatch.rs` 1-305,
`owned_match_results.rs` 1-58, `owned_records.rs` 1-159, `parallel_checks.rs` 1-97, `proof_scope.rs` 1-122,
`provider_supplies.rs` 1-610, `published_no_return.rs` 1-225, `qualified_terms.rs` 1-1141, `ranges.rs` 1-810, `rankings.rs` 1-687,
`reach.rs` 1-187, `references.rs` 1-1797, `representation_views.rs` 1-423, `result_fields.rs` 1-406, `result_summaries.rs` 1-283,
`returned_views.rs` 1-1475. Skimmed: none. Not read: none. Additionally read `tests.rs` lines 1-27 and the `mod` list (28-102) for
declarations only (not part of this unit).
Commands run for counts: per-file `grep -c '#\[test\]'` (sum 487); `grep -rEc` for helper uses; `grep -c 'crate::lowering::Compilation {'`
(26 in 23 listed files, 68 in all of `T`); `grep -n "fn verdict("` (15 definitions in `T`); `grep -n "mod "` of `tests.rs`.
Housekeeping: an early `mkdir -p` with a wrong relative path created an empty directory tree `source-rust/.agents/atlas/...`; it held no
files and I removed it (`git status --short` afterwards shows only `.agents/atlas/codebase-2026-11-22/` untracked).

## Five things a maintainer most needs to know about this unit
1. Judgments in most files are pinned to **diagnostic text** (and in `lifetime_elision.rs` a hash-shaped package identity inside the text,
   `:82,115,151,287`); a reworded refusal breaks many tests at once. Fifteen copies of a `verdict()` string-mapping helper exist across `T`.
2. A large block of tests **forge the checker's input** (edit one row of a lowered table, expect `ForeignIdentity` with an exact detail).
   These are the tests that show which tables the checker re-derives instead of trusting (loans, cleanup rosters, suspension plans, rank
   inventories, requirement envelopes, lender fields, scalar-qualification catalogs). Seventeen files here use the pattern.
3. Two tests pin a **currently refused** program rather than an accepted one: `representation_views.rs:37-107` (scalar reference recast,
   `STATE-LOCAL-VALUE-FRONTIER`) and `qualified_terms.rs:593-643` (reference to a qualified scalar). Several others pin `FallBack`/`Unsupported`
   refusals as the present boundary (`operational_effects.rs:287-309,706-723`, `provider_supplies.rs:253-264`, `loan_regions.rs:196-203`).
4. Some negatives are written to pass on **either of two refusal categories** (list under Fail-open risks), so they prove "not accepted" but
   not which stage refused; `rankings.rs`, `qualified_terms.rs`, `operational_effects.rs`, `representation_views.rs` have these.
5. Entry points are chosen by **name or position**: `boundary_calls::entry` (outside unit), machine named `"main"`
   (`operational_effects.rs:35`, `hosted_primitives.rs:35`), `"entry"` (`indexed_contracts.rs:8-11`, `provider_supplies.rs:14`),
   `"build_entry"`; helper names for private-input checks (`entry_inputs.rs:147`); `identity.contains("label")`
   (`machine_lifetimes.rs:67`); and hard-coded machine ids (`id(2)`, `id(3)`). Fixtures reach into `source/library/core/*.omg`,
   `samples/cli/facts/**`, `samples/cli/probes/**`, `tests/fixtures/**` and `P06/tests/fixtures/**` through `include_str!` with
   `../`-chains, and `references.rs:860-865` reads sample files at run time.

## What you are unsure about
- I did not read the helpers these files depend on (`tests.rs` helpers `machine`, `lower`, `judge`, `run`, `verdicts`, `safety`,
  `requirement`, `guarantee`, `published`, `rejected`, `assert_kernel_accepted/checked`, `elaborate_over`, `declare_body_models`;
  `build_purpose.rs`; `boundary_calls.rs`; `stored_references.rs`; `calling_policies.rs`), so claims about what they do are inferred from names
  and call shapes.
- I did not verify that the `include_str!` paths and fixture files exist, nor whether samples referenced
  (`samples/cli/facts/wrapping_integer_arithmetic/local/main.omg`, `.../integer_multiple_remainder/*`, `.../reference_arrival_handoff/*`,
  `samples/cli/probes/bounded_concat`, `record_geometry`) are within the "frozen" `.omg` set the repo rules mention.
- Test counts are from `grep -c '#\[test\]'`, which also counts attribute lines that might sit inside raw strings or comments; I saw none.
- `mutable_formals.rs:10-14` cites `prepared-typed-trees-to-lowered-program/src/read/machine.rs`; I did not check that path exists today
  (the stage directory names in the crate map are numbered).
- I did not check whether `tests.rs` gates these modules with `#[cfg(test)]` or `feature = "test-support"`; the crate map says some
  tests are only compiled with `test-support`.
