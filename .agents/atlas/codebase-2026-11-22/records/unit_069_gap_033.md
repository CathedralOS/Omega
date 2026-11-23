# Record: unit_069 gap_033 (area 04-source-stages-06-07)

Paths are relative to `C:/Development/Omega/.agents/worktree/worker-2/`. `P07` = `source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi`. "read" = read in full. No greps were run for this record; every claim is from reading the cited lines. Both ranges are test code, so items 7 (unused code), 8 (stated limits) and 9 (fail-open) mostly report that the ranges define no `pub` items.

---

## P07/src/source_selection_publication.rs, lines 2433-3629 (1,197 lines)

The first reader's record (`records/unit_069.md`) covers lines 1-2432 of this file. Lines 2433-3567 are the end of an inner `#[cfg(test)]` module (the module's header is above 2433, not read by me); lines 3569-3629 are a second module `core_catalog_tests` (3569-3570).

1. Purpose. Unit tests for `prepare` and the `Questions`/`Declarations` types. Each test builds an accepted source, calls `prepare(&source, module, None)`, and then checks that the selection packet (`prepared.questions`: `declarations`, `questions`, `joins`) passes `psi_authorization::selection_questions::validate_selection_questions`. Most then damage one field of a clone of the packet and require validation to refuse with a named message.
2. Shape. Test functions (all `#[test] fn`, nothing `pub`):
   - `static_formation_scope_is_metadata_not_a_product_selection` (2434)
   - `generated_cleanup_calls_keep_the_carriers_actual_selection` (2499)
   - `recursive_declarations_keep_both_entry_and_folded_head_origins` (2639)
   - `specialized_callbacks_publish_the_written_declaration_with_its_applied_meaning` (2712)
   - `anonymous_callable_occurrences_keep_distinct_source_identity` (2771)
   - `domain_declarations_keep_their_subjects_without_splitting_their_meaning` (2920)
   - `requirement_overloads_keep_their_signatures_and_refuse_header_swaps` (3021)
   - `describing_a_formal_does_not_make_its_argument_value_visible` (3098)
   - `callable_alias_capture_keeps_distinct_concrete_applications` (3153)
   - `callable_alias_capture_can_select_only_its_enclosing_type_binder` (3190)
   - `a_hosted_boundary_call_keeps_its_actual_machine_entry_selection` (3234)
   - `copied_record_calls_keep_the_exact_operand_supplier_and_schema` (3320)
   - `named_calls_keep_their_public_header_and_exact_body` (3418)
   - `core_operator_declarations_keep_their_actual_applied_signatures` (3575, in `core_catalog_tests`)
3. I/O. Inputs are helper-built accepted sources: `accepted_source(text, &[])` (2500, 2640, 2713, 3024), `accepted_source_with_publication` (2921), `accepted_source_with_core` (3237), `accept_formation_program` / `closed_requirement_formed_in_build` (2435-2436, 2465-2467, 2482-2489) and `named_calls()` (3099, 3421). These helpers are defined above line 2433 and I did not read them. Outputs are assertions only. The tests also mutate private state directly: `Questions::requirements.insert(key, &changed)` (3396, 3513), `Declarations { .. }` built by struct literal with empty `SymbolRows::new(0)` tables (3577-3586), and `Declarations::new` (3127, 3219).
4. How it decides. Each negative test pins one refusal message and a join key:
   - Cleanup calls: three mutations (author at 2543-2556, target at 2558-2562, storage identity at 2564-2595) and three expected messages (2603, 2608, 2613). The cleanup join is keyed by `CallableInvocationSite::CleanupAction`, original caller and target.
   - Recursion: one machine origin is keyed by `(declaration, machine, head)` (2676-2679). Duplicating it must give "repeats a machine origin" (2700); removing the folded head must give "folded selection lost its original caller, target or actual head" (2707).
   - Overloads: the two `pick` signatures have distinct `normalized_hermetic_symbol_identity` values (3056-3057). Swapping the declaration in a `SelectionJoin::Boundary` must give "boundary differs from its checked semantic declaration" (3091).
   - Domains: two declarations `[u8; 2]::Ready` and `[u8; 3]::Ready` share `semantic_id` (2949-2952) but have different hermetic identities (2961) and nominal identities (2975). A duplicate identity must be refused (3011-3017).
   - Anonymous callables: two callable types with the same meaning but different spans keep distinct symbol identities (2818-2827, 2833). The identities do not change when a trailing comment is added to the source (2836-2875).
   - Copied record calls and named calls: four edits to the retained `source_operand` are tried (3380-3394). The supplier or operand identity gives "changes its retained implementation operand or supplier"; the record parameter or application string gives "changes its retained record parameter or schema" (3405-3409). Changing `required_contract` or the callee on a cloned module gives "exact caller, public header and body association" (3559-3563).
   - Hosted boundary: the primitive call is keyed by the boundary machine id. Selecting the external-completion requirement id instead gives "differs from its exact source selection" (3306-3308). This test asserts `entry != selected` (3281), so the source selection is the state's entry symbol, not the machine symbol.
   - `describing_a_formal...`: the same formal symbol is accepted under `StaticArgumentRole::DeclarationDescription` and refused under `ValueDependency` with "outside its actual lexical context" (3130-3149).
   - `callable_alias_capture_can_select_only_its_enclosing_type_binder`: a binder of a sibling alias is refused for a callable scope that is not its own (3222-3230).
   - Core operator test: `core_integer_operator` is called with operand order swapped. The same selection returns the same row (3610-3615); the reversed selection returns a different row (3619). The row has `DeclarationCustody::Toolchain` and an identity starting `core-integer-policy-operator-v1:` (3621-3622).
   - Two lookups in these tests compare names, not identities. `row.name == "build"` (2451) and `declaration.name == "KeyHash"` (3178) pick rows in the test. Identity comparisons are used for the assertions that matter.
5. Cases by shape. Each test fixes one program shape: a one-parameter recursive `repeat` (2642-2648), two `data ... dropped by` types (2502-2503), a two-overload boundary trait `Input` (3026-3029), `KeyHash<K>` callable aliases (3155-3163), the `record_callback_loop` fixture (3323-3326). The calls test asserts exactly 3 declared calls (3444) and the bodies `["First::pick","First::pick","Second::pick"]` (3480). The core-operator test covers one operation (`GreaterOrEqual`) and one carrier (signed 64) (3588-3600).
6. Same job elsewhere. Packet validation is done by `psi_authorization::selection_questions` (outside this unit; every test here calls it). `callable_declaration_path(&identity, &meaning)` is used at 2749-2750 and 2894; its definition is not in my range. The `fixed positive build fixture + a field edit + refusal text` pattern is repeated in `P07/tests/build_companions.rs` (e.g. 639-692, 904-1001); read.
7. Unused or one-caller. No `pub` items in these ranges. Nothing grepped.
8. Stated limits. Comments in tests: "Publish both complete headers directly. A signature-free named call would test overload selection before reaching this identity owner." (3022-3023); "inactive-width inference itself belongs only to the real core type" (3871-3872 is in the other file; for this file none). The recursion test comment: none.
9. Fail-open risks. None seen in test code that affect compiler behavior. Weak spot of the tests themselves: `calls_with_core_operator` style `.unwrap_or`-free; but `anonymous_callable_occurrences...` builds `occurrences` into a `HashMap` keyed by symbol (2799-2813) and asserts its size is 2 (2814), so it would fail, not pass, on extra matches. `a_hosted_boundary_call...` and the recursion test use `.expect`/`.unwrap` on every lookup, so a missing join fails the test. The `domain_declarations` test's final assertion is only `.is_err()` with no message (3014-3017); it does not pin which check refuses the duplicate. Classified: traced (read).

---

## P07/tests/build_companions.rs, lines 341-1607 (1,267 lines)

Lines 1-340 (imports, `PRELUDE`, `produced`, `accepted`, `accepted_for`, `accepted_frontier*`, `typed*` helpers, `ONE_BINDING`, `published_boundary_calls`) are outside my range and not read; the first reader's record covers them. Helper names used below are therefore taken from call sites only.

1. Purpose. Integration tests for the Build companions that stage 07 publishes beside the Terminal Psi artifact: root bindings (`build_bindings`), requests (`build_requests`), queries, native bindings and locators, provider selections, source selections, and Build protocol use. Each positive program compiles and publishes the expected rows; each negative twin changes one input and must be refused with a named message.
2. Shape. `#[test] fn`s (none `pub`), with their start lines:
   - `a_root_binding_is_published_with_the_entry_it_names` (341)
   - `each_binding_of_one_machine_keeps_its_own_entry` (391)
   - `a_build_with_no_selection_publishes_no_binding_row` (435)
   - `two_machines_that_bind_at_the_same_operation_number_keep_their_own_rows` (467)
   - `a_row_does_not_answer_for_the_same_operation_of_another_machine` (503)
   - `two_machines_that_make_the_same_request_each_publish_its_row` (515)
   - `a_helper_the_entry_never_calls_asks_nothing_of_the_host` (546)
   - `a_boundary_outside_the_build_protocols_is_reported_and_given_no_row` (599)
   - `saved_build_rows_do_not_authorize_changed_actions_or_protocol_families` (639)
   - `a_portable_artifact_without_build_companions_still_round_trips` (697)
   - `an_accepted_build_that_names_no_constant_reads_none` (739)
   - `an_accepted_build_states_each_constant_its_declarations_name` (749)
   - `a_native_binding_publishes_its_closed_locator_shape` (844)
   - `a_selection_row_keeps_the_exact_operation_typing_recorded` (904)
   - `a_native_import_infers_nested_byte_widths_and_publishes_their_shapes` (1004)
   - `an_ordinary_helper_result_keeps_its_closed_native_locator_application` (1045)
   - `a_closed_generic_boundary_call_keeps_its_type_argument` (1062)
   - `distinct_closed_boundary_arguments_keep_distinct_source_declarations` (1112)
   - `inferred_const_arguments_still_have_to_fit_their_declared_carriers` (1198)
   - `a_case_value_pattern_cannot_discard_its_applied_type` (1232)
   - `a_provider_selection_keeps_its_exact_generated_protocol_call` (1323)
   - `product_queries_keep_their_selected_operation_in_value_positions` (1420)
   - `boundary_declaration_facts_do_not_admit_pure_execution` (1452)
   - `public_requirement_borrows_keep_their_source_parameter_positions` (1557)
   - `a_plain_integer_and_a_lookalike_enum_cannot_select_a_provider` (1582)

   Non-test helpers in range: `const TWO_HELPERS` (449-460), `native_binding_prelude()` (773-803), `native_binding_program(build)` (805-822), `published_native_locator(program)` (824-841), `provider_selection_prelude()` (1315-1320), and `mod frontend_fixture` loaded by `#[path = "../../../../../../../tests/support/frontend.rs"]` (1553-1554).
3. I/O. Inputs are Omega source strings run through the helpers, plus `include_str!` of `source/library/core/external_binding.omg` (810-812, 868, 911-913, 1238-1240), `never.omg` and `process_exit.omg` (1457-1463), and `tests/fixtures`-style paths via `../../../../../../../`. Outputs are assertions on `ProducedTerminalProgram` accessors: `build_bindings().calls_for(view)`, `.declarations_for`, `.native_formations_for`, `.queries_for`, `build_requests().calls_for`, `source_selections().questions_for/admit`, `build_protocol_use()`, `constants_read()`, `requirement_review(..)`. The tests mutate typed trees directly via `expression_table` (981-990, 1242-1304). `native_binding_prelude` builds a `BuildTarget` enum, one `public const` per `ProductTarget::ALL` entry and a `build_selection_target` match, from `ProductTarget::ALL` (780-801), so the test prelude mirrors the shipped catalog rather than a hand copy.
4. How it decides. The tests assert what stage 07 keys on:
   - Rows are keyed by machine plus operation plus boundary, not by operation number alone: two helpers bind at the same `(operation, boundary)` but differ in `site.machine` (491-495, 506-509). Also 531-535.
   - A root binding names its entry by path text: slot `"linux_x86_64::ProgramEntry"`, implementation `RootBindingOperand::AuthoredPath("main")` (360-364), `selecting_machine == None` (367).
   - Protocol reach is computed from what the entry can run, not from what the Build source declares: `watch()` is only counted when `request` calls it (546-590). Probe outside the four protocols sets `reaches_outside` and gets no row (599-631).
   - Constants read are decided by names in bodies, called-machine contracts and declared types (749-771); the row carries `declaration` and `value` (768-769).
   - The native vocabulary is decided by exact compiler-owned source: a same-shaped edited `external_binding.omg` (a trailing newline) is refused with "native binding requires the exact compiler-owned `ForeignBinding` declaration" (866-896). The test comment calls this custody ("publication custody", 871).
   - Build selection rows are checked against the expression's recorded operation: replacing it with `Protocol::Select` gives "Build input association names another request or generation" (981-999).
   - A provider operand is accepted only when it is the exact Build vocabulary: `7u64` and a lookalike enum `CustomMode::Independent` are both refused with "BuildTarget or CompositionMode" (1582-1606).
   - Saved artifacts: `record_storage::encode` of an accepted program with Build companions is refused with "saved Build companions require their accepted source protocol correspondence" (651-657). A hand-written record using the layout tag `OMGBLD03` (671-678) with changed action or site family is also refused when decoded (680-690).
   - Pure execution: `compiler_evaluation.invocation_refusal` is "compiler evaluation needs admitted boundary execution" or "... admitted external completion" (1465-1475, 1523), and `admit_compiler_evaluation` returns the same string (1546-1549).
5. Cases by shape. Quoted shapes the tests pin: locator widths `[0,0,0]` for a syscall locator (853) and `[9,10,0]` for a `DllImport` with `"winmm.dll"` and `"PlaySoundW"` (1016, 1058), shown as `Sum` of cases with fixed `Array` fields (1017-1040). The first test requires exactly 3 emitted boundary calls: "the request, the prepare and the bind" (345). The protocol set is "one of the four the toolchain names" (596-597 comment). `a_closed_generic...` asserts one boundary declaration for `Probe::send::<u64>` and `Probe::send(11u64)` (1081-1085) and two for `u64` and `u32` (1140). A const binder `Bytes<const N: u8>` is checked at 3 bytes and refused at 256 bytes with "record const argument does not inhabit its declared carrier" (1211-1228). A bare case pattern with a lifetime argument is refused with "pattern of an applied type" (1305-1311).
6. Same job elsewhere. The test prelude rebuild in `native_binding_prelude` (773-803) duplicates the production `BuildTarget` prelude by construction from `ProductTarget::ALL`; suspected shared with `P07/tests/support/frontend.rs` (not read). Both `a_root_binding...` (355-370) and `each_binding...` (402-410) destructure `BuildSelectionRequest::Entry(binding)` with a panicking `let ... else`; they repeat the same extraction.
7. Unused or one-caller. No `pub` items. `native_binding_program` is used at 851, 1006 and 1046; `published_native_locator` at 852, 1015, 1058 (read). Not grepped.
8. Stated limits. "Explicit widths keep this control focused on publication custody; inactive-width inference itself belongs only to the real core type." (870-871). "Typing emits a destructuring arm here. At the checked frontier, a value-pattern literal for the same case must preserve any application." (1269-1270). "Fresh accepted production remains available." (637). No "unsupported" or "migration debt" comments in the range.
9. Fail-open risks. None seen in compiler behavior. Test-side notes, read, not traced further: (a) `a_helper_the_entry_never_calls...` checks `reaches_outside` only through the entry's reach; the test asserts that a helper the entry never calls gives no host row (569-580), so an unreached helper's boundary call is deliberately accepted without a row. This is stated by the comment at 538-544 ("a boundary such a helper calls is nothing this Build asks of its host"). Classified stated. (b) `saved_build_rows...` leaves other fields of the hand-written record empty (673-678); it demonstrates refusal on decode, not acceptance. (c) Line 642 `use checked_program_to_psi::...` is indented at column 0 inside a function; formatting only.

---

## Coverage

- `source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi/src/source_selection_publication.rs` lines 2433-3629: read in full (1,197 lines).
- `source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi/tests/build_companions.rs` lines 341-1607: read in full (1,267 lines).
- Skimmed: 0 lines. Not read: none of the unit file's ranges. Total 2,464 lines read in full.

## Five things a maintainer most needs to know about this unit

1. Both ranges are test code only; they define no production types. They record which refusal messages and which keys (machine plus operation plus boundary; declaration identity, not name) the stage 07 selection packet and Build companions are held to.
2. In `source_selection_publication.rs`, nearly every negative test works by cloning a valid `SelectionQuestions` packet, editing one join or declaration field, and requiring `validate_selection_questions` to return a specific message (2543-2635, 2695-2708, 3072-3094, 3380-3414, 3535-3565). The messages are the tests' contract.
3. Build rows are per call site: two helper machines binding at the same operation number and boundary publish separate rows told apart by `site.machine` (467-510, 515-536).
4. Saved Build companions are not trusted from bytes: encoding without the accepted source protocol correspondence is refused, and a hand-built `OMGBLD03` record with a changed action or site family is refused on decode (639-692).
5. The native binding vocabulary is tied to the exact `source/library/core/external_binding.omg` text; a trailing-newline edit is refused (866-896). Provider choice operands must be the exact `BuildTarget`/`CompositionMode` values (1582-1606).

## What you are unsure about

- Helper definitions (`accepted_source*`, `named_calls`, `accept_formation_program`, `closed_requirement_formed_in_build`, `callable_declaration_path`, `PRELUDE`, `accepted_frontier*`, `typed*`) sit outside my ranges; I describe them only from call sites.
- The start of the inner `#[cfg(test)]` module that holds lines 2433-3567 is above my range, so its `use` list is unknown.
- I did not grep any name; no call-site counts are given.
