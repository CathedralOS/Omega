# Record: unit_066 gap_032 (second pass), area 04-source-stages-06-07

P06 = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program`.
All five files are Rust test modules under `P06/src/tests/`. They contain no production code; "decides" below
means what each test pins about the checker's verdict. Everything here was read in full.

Common shape of these tests (read): a test builds a small Omega source string (often with `format!` to make a
positive/negative twin that differs in one clause), runs it through the stage 06 checker, and asserts either
`"accepted"`, a `RouteVerdict::Rejected { .. }` with a specific obligation kind or statement text, or a refusal
`detail` string. A second family elaborates a program, mutates the lowered Terminal Psi module (swap a measurement
id, change an access, clear claim rows), and asserts the checker does not return `Accepted`. Judgments in the
tests are keyed on obligation kind (`ObligationKind::...`), on `LoanConflict` variants, and on substring matches of
`statement` / `detail` text (source spelling of the refusal message).

## 1. `P06/src/tests/byte_domains.rs`, lines 816-1256 (441 lines)

1. Purpose: tests that casting bytes into a byte domain (`as +Text`, `as -Text`) is accepted only when the
   domain's predicate (`no_nul`, `valid_utf8`, `ascii_only`, a length rule) is provable for the exact backing
   bytes, including sub-slices, fixed arrays, joins and mutable buffers.
2. Shape: helpers `sliced_cast` (`byte_domains.rs:833`), `fixed_text` (`:937`), `refused_cast` (`:956`, exactly
   one `unproved` and every other refusal list empty). Tests: `a_subslice_cast_requires_the_exact_backing_to_preserve_its_predicate`
   (`:852`), `removing_a_domain_keeps_the_backing_bytes_of_a_prefix` (`:877`),
   `a_slice_uses_the_byte_predicate_of_the_current_loop_parameter` (`:894`),
   `a_fixed_byte_array_is_cast_into_its_domain_only_when_its_bytes_are_in_it` (`:973`),
   `a_fixed_byte_array_of_unknown_bytes_is_not_cast_into_its_domain` (`:999`),
   `a_shared_byte_view_join_checks_the_bytes_from_each_branch` (`:1029`), `a_shared_array_prefix_uses_its_own_backing_membership` (`:1067`),
   `a_byte_read_keeps_ascii_facts_about_its_own_view` (`:1105`), `a_variable_byte_store_preserves_the_buffers_utf8_domain` (`:1138`),
   `a_false_edge_retires_a_copyable_view_before_its_backing` (`:1164`), `shared_byte_view_domains_check_lengths_and_keep_their_names` (`:1190`).
   `judge_build` and `judge_portable_byte_build` are defined earlier in the file (`:28`, `:84`), outside this range.
3. Inputs/outputs: source text (some via `include_str!` of fixtures under `P06/tests/fixtures/views/{run,fail}/`,
   `:1030-1075`, `:1139-1142`); outputs are asserts. The join and prefix tests also round-trip the accepted module through
   `terminal_codec::encode_module` / `decode_module` (`:1045-1046`, `:1083-1084`).
4. How it decides (what the tests pin): a cast is refused with an `ArrivalInvariant` obligation whose `statement`
   contains "cannot prove the value of this cast is in the domain it adds" (`:864-870`). Keyed on obligation kind plus
   substring of the statement text. Prefix of a `valid_utf8` text is refused because a prefix may split a UTF-8 sequence
   (`:882-891`), while `no_nul` and `ascii_only` prefixes are accepted (`:879-881`). The join failure expects exactly one
   `unproved` containing `ascii_only` and one `unestablished` whose detail contains "byte-domain qualifications" (`:1058-1061`).
   Mutable store of 128 must fail only `ObligationKind::DataDomain` rows (`:1157-1158`).
5. Cases by shape: predicates tested are only `no_nul`, `valid_utf8`, `ascii_only` and a `self.length == 4` rule
   (`:837`, `:942`, `:1192`). Fixed arrays are `[u8; 3]` only (`:940`).
6. Same job elsewhere: the "exactly one refusal, all other lists empty" pattern is repeated as an inline `matches!` at
   `byte_domains.rs:1052-1061`, `:1090-1099`, `:1124-1133`, `:1150-1158`, `:1222-1236`, in `byte_view_elements.rs:1921-1929`
   and in `calling_policies.rs:413-433`, `:469-489`. `refused_cast` (`:956`) is the only named version in this range.
7. Unused/one-caller: none of the items are `pub`. `verdict` is defined as a private `fn verdict(` in 17 files (grep count
   mode, `fn verdict\(` under `P06/src`), so each test file carries its own copy; the byte_domains one is above line 816.
8. Stated limits: the comment at `:817` "The explicit qualification must form before `forward` can use it." is a
   description, not a limit. The test at `:999` accepts either an elaboration-time `Unproved` refusal or `refused_cast`
   (`:1021-1025`): it does not fix which stage refuses.
9. Fail-open risks: none seen in production code (tests only). One test-weakness note, recorded as stated: `:1021-1025`
   passes if either of two refusal routes fires.

## 2. `P06/src/tests/call_claims.rs`, lines 1-147 (147 lines)

1. Purpose: tests that direct calls carry the exact sibling claims of linear (single-use) values, and that the
   receiving checker reconstructs them instead of trusting declared claim ids or refusing every claimed call (`:1-3`).
2. Shape: `elaborated()` (`:9`) builds `Pair{left,right: Token[linear]}` forwarded through `forward`;
   `transfers()` (`:32`) finds the first `CallStructural`/`CallStructuralWithScalarArguments` with non-empty
   `claim_transfers`; `refuses()` (`:51`) asserts `judge` returns `Err` with `RefusalReason::ForeignIdentity`.
   Tests: `ordinary_call_receiving_preserves_exact_sibling_claims` (`:59`),
   `a_linear_leaf_cannot_lose_its_claim_by_erasing_all_claim_metadata` (`:94`).
3. Inputs/outputs: elaborates via `crate::lowering::elaborate_for` (`:20`) with Build scope and Invocation publication;
   mutates `ClaimTransfer` rows: pop one (`:69`), duplicate (`:74`), foreign claim id `u64::MAX` (`:78`), swap `path` of two rows (`:83-85`),
   change `argument_index` (`:89`).
4. How it decides: a changed claim row is refused with reason `ForeignIdentity`, keyed on the row's identity (claim id,
   path, argument index). The erasure test clears `owns_linear_claim`, every `entry_claims`, `block_entry_claims`,
   result `claims`, `claim_transfers` and `returned_claims` and still expects refusal (`:104-146`), so the checker keys
   on the declared linear types plus program, not on the presence of claim rows.
5. Cases by shape: the fixture has exactly two claim transfers (`assert_eq!(transfers(..).len(), 2)`, `:61`).
6. Same job elsewhere: mutate-the-lowered-module-then-refuse is the pattern of `byte_view_elements.rs:1788-1866` and
   `calling_policies.rs:601-728`. Not compared further.
7. Unused code: nothing `pub`.
8. Stated limits: none.
9. Fail-open risks: none seen.

## 3. `P06/src/tests/byte_view_elements.rs`, lines 1-2074 (2,074 lines)

1. Purpose: tests reading, slicing, comparing, storing and domain-qualifying borrowed byte views and typed slices
   (`view[index]`, `view[a..b]`, `@mutable` windows, `==` on views). The header (`:1-9`) states the rule: a read is
   accepted exactly where the program has shown the index lies inside this view, by a `requires` or guard over this
   view's own `length`.
2. Shape: a private `verdict()` (`:105`) maps a build to `"accepted"`, `"rejected: <first unestablished detail>"`, or the
   refusal detail; imports `judge_build` from `super::byte_domains` (`:11`). About 45 tests. Groups:
   - Representation views `words as @[u8]`: `a_representation_view_count_must_measure_its_captured_descriptor` (`:16`, forges
     `source_length` of `ElementViewRegion::WholeRepresentation`; expects detail "source length is not its view's measurement
     directly before it", `:97-99`); `image_dimensions_establish_the_representation_view_count` (`:122`);
     `representation_views_do_not_gain_writable_access_or_boolean_validity` (`:141`).
   - Reads: `a_byte_is_read_where_the_view_is_known_to_reach_it` (`:176`), `a_byte_past_what_the_view_is_known_to_hold_is_rejected` (`:219`),
     `a_typed_slice_read_needs_a_bound_on_its_own_descriptor` (`:248`), `a_typed_slice_window_needs_bounds_on_its_own_descriptor` (`:264`),
     `a_byte_read_names_the_measurement_made_directly_before_it` (`:288`, windows(2) over operations: `ByteSequenceLength` then
     `ByteSequenceRead` with `measured.place == *source` and `length` equal to the measurement result id, `:305-317`),
     `a_byte_view_index_of_another_width_is_refused_by_name` (`:323`, exact refusal text "typing byte view index that is not a u64 ...").
   - Stored views: `stored_byte_reads_keep_their_field_extent_and_bounds` (`:336`), `a_stored_byte_read_cannot_substitute_a_sibling_field` (`:418`,
     swaps `field` of a `StructuralByteSequenceFieldRead`), `replacing_a_stored_view_does_not_preserve_its_previous_extent` (`:1316`).
   - Sub-slices and windows: `a_subslice_keeps_the_selected_bytes_and_exact_extent` (`:481`), `mutable_windows_reborrow_the_same_backing_without_moving_the_parent` (`:497`),
     `shared_subslices_compose_with_literals_calls_and_stored_views` (`:567`, seven programs), `subslice_bounds_must_belong_to_the_same_view_and_be_ordered` (`:772`),
     `a_subslice_cannot_claim_another_measurement_or_mutable_access` (`:784`, five faults 0..5 applied to `ByteSequenceSubslice`).
   - Qualified ranges: `QUALIFIED_BYTE_RANGE` (`:594`), `QUALIFIED_PREFIX_CAST` (`:630`); tests `:607`, `:616`, `:643`, `:652`, `:743`, `:756`.
   - Arrays as views: `fixed_byte_arrays_can_supply_borrowed_view_parameters` (`:878`), `an_exclusive_array_view_lends_shared_access_after_its_cohort_ends` (`:900`),
     `explicit_array_views_keep_exact_length_and_release_the_original_backing` (`:954`), `typed_whole_array_views_keep_exact_length_and_their_backing` (`:978`),
     `an_array_loan_keeps_its_element_type_and_requested_access` (`:1021`), `an_empty_byte_window_requires_existing_array_backing` (`:1088`; calls
     `terminal_semantics::fixed_byte_storage_window`).
   - Stores: `mutable_byte_stores_preserve_the_descriptor_and_its_bound` (`:1126`), `a_byte_store_needs_mutable_access_and_its_own_index_bound` (`:1157`),
     `a_byte_store_cannot_claim_another_measurement_or_access` (`:1179`, four faults on `ByteSequenceWrite`/`ByteSequenceLength`),
     `a_shared_reference_kept_across_a_byte_store_prevents_the_write` (`:1294`), `byte_store_array_backing_program_is_checked_without_changing_its_operations` (`:1303`, fixture `byte_view_store/main.omg`),
     `writes_through_array_views_retire_old_backing_facts_not_saved_values` (`:1329`), `a_view_write_keeps_facts_about_sibling_fields` (`:1366`),
     `a_shared_array_view_cannot_observe_write_only_backing` (`:1389`, expects `LoanConflict::AccessExceedsParent`).
   - Comparison: `two_byte_views_are_compared_by_their_bytes` (`:1446`), `byte_predicate_reads_keep_exact_measurements_and_shared_access` (`:1467`; rewrites
     `ByteSequenceEqual` into `ByteSequencePredicate` for every `semantic_vocabulary::ByteSequencePredicate::ALL`, expects `FallBack` containing "measurement"/"shared" for faults 1 and 2),
     `a_byte_comparison_names_each_views_own_measurement` (`:1537`), `byte_field_comparisons_use_the_existing_readable_descriptor` (`:1634`),
     `field_byte_comparisons_retain_their_own_measurement_and_access` (`:1796`).
   - Arithmetic through joins: `last_byte_through_a_match` (`:1658`), `a_match_of_a_window_length_and_its_endpoint_keeps_the_last_byte_in_range` (`:1675`),
     `a_match_arm_naming_the_backing_length_leaves_the_last_byte_unproved` (`:1702`, `UnprovedCause::NoDerivation`), `a_call_guarantee_keeps_the_length_of_the_same_byte_window` (`:1732`),
     `loop_arrivals_prove_the_selected_views_own_extent` (`:1763`).
   - Qualified literals and arrivals: `a_qualified_byte_literal_still_proves_its_domain_at_introduction` (`:1872`; expects `unestablished` at `Construct::Edge(_)` with exact detail
     "a byte transfer's source does not establish its primitive byte-domain qualifications", `:1889-1890`), `qualified_byte_arrivals_keep_each_backings_length_and_predicate` (`:1899`),
     `nested_shared_path_calls_keep_their_predicate_and_extent` (`:1938`, a POSIX mkdir-walk shaped program).
   - Whole records from a view: `whole_records_from_a_view_keep_their_index_bound_and_access` (`:1994`).
3b. Inputs/outputs: sources through `judge_build`; for lowered-module faults through `super::build_purpose::{frontier_of, package, roster, symbol}` and
   `crate::lowering::elaborate_for` (`:17`, `:786-806`, etc.), then `judge(Ok(elaboration))`. Reads `OperationKind` variants from `terminal_psi`
   (`ByteSequenceLength`, `ByteSequenceRead`, `ByteSequenceSubslice`, `ByteSequenceWrite`, `ByteSequenceEqual`, `ByteSequencePredicate`, `EstablishElementView`,
   `ElementViewLength`, `StructuralByteSequenceField{Length,Read}`).
4. How it decides (as pinned): a read, store or subslice is accepted only when the bound is about the same view's own measurement. "The bound is on another
   view" is rejected (`:236-244`, `:248-258`, `:1796-1866`). Judgment keyed on place identity (`measured.place == *source`, `:310`), field id (`:399-401`),
   measurement result id (`:312`), `StructuralAccess` (`:1844-1858`). The five subslice faults (`:842-867`) and four store faults (`:1226-1284`) assert only "not Accepted"
   (`!matches!(.., Accepted(_))`, `:870-873`, `:1286-1289`), so they do not pin which refusal. Many positive/negative twins assert the bare strings `"accepted"` /
   `"rejected"` (e.g. `:221-243`).
5. Cases by shape: `for fault in 0..5` / `0..4` fixed fault lists (`:811`, `:1202`, `:1588`, `:1827`); several refusals are asserted by the exact message text, e.g.
   "typing byte view index that is not a u64 while reading the body of `build_entry`" (`:329`), "a subslice without authenticated backing and access" (`:514`),
   "range of qualified bytes without its selected operand type" (`:610`, `:750`, `:766`), "domain cast whose operand's declared type typing does not read yet" (`:646`),
   "copying a slice element requires a copyable element type" (`:2069`). The comparison tests only cover `ByteSequenceOperand::Whole` and `::Field` (`:1501`, `:1841`).
6. Same job elsewhere: `byte_domains.rs` covers byte-domain casts (suspected overlap with `a_qualified_byte_literal_still_proves_its_domain_at_introduction`, `:1872`,
   and `qualified_byte_arrivals_...`, `:1899`; read both, they test different programs). `P06/src/tests/representation_views.rs` and `runtime_indexed_elements.rs` exist
   (grepped file list) and plausibly cover neighbouring view cases; not read in this pass (suspected).
7. Unused code: no `pub` items; the file's private `verdict` is one of 17 copies (grep count above).
8. Stated limits: `:260-262` "A mutable typed slice is not given a window by this read path." (test at `:277-282` accepts a shared reborrow of a mutable slice but not a mutable one).
   `:643-649` the test name and message say an outer qualification cannot supply the missing range type, i.e. refused today. Test comment `:1869` "Adding a primitive byte domain is checked even when its result is unused."
9. Fail-open risks: none seen in production code. Test-strength notes: fault tests at `:870`, `:1286`, `:1624`, `:1862` assert only "not Accepted", so a refusal for an unrelated reason would also pass
   (stated by their form, not by comment). `:1527-1531` the fault-1/2 expectation is substring "measurement" / "shared" on the refusal detail.

## 4. `P06/src/tests/callable_contracts.rs`, lines 1-760 (760 lines)

1. Purpose: tests that callable refinement (a machine selected to fill a callable contract must accept at least its inputs and guarantee at least its outputs, crashes
   and operational limits) is independently re-derived by the checker and re-validated from retained certificates; plus runtime evidence-contract rosters and the
   `block`/`suspend` acknowledgement markers.
2. Shape:
   - Fixtures: `refinement_module()` (`:24`, abstract bodies, `declaration = None`, pushes a `TerminalCallableContract` id 1 with `requires ... >= 5` and a `TerminalMachineDescriptor`),
     `declared_refinement_module()` (`:118`, builds a typed tree by hand: symbols, a `Selected` machine and a `Callback` `RuntimeCallableContract`, then `crate::lowering::elaborate` and `crate::checking::judge`),
     `derive_refinements()` (`:315`, calls `crate::checking::derive_contract_obligation`, `:331`), `install_roster()` (`:453`), `roster_valid()` (`:525`).
   - Tests: `callable_refinement_proves_input_weakening_and_remaps_result_formals` (`:343`), `checker_finalizes_descriptors_and_refuses_mutated_retained_certificates` (`:373`),
     `checker_refuses_operational_strengthening_before_publication` (`:397`), `callable_refinement_refuses_weaker_outputs_and_unpermitted_crashes` (`:426`),
     `runtime_evidence_roster_rejoins_full_meaning_and_exact_subjects` (`:537`), `acknowledged_terminal_paths_keep_nominal_targets_and_scalar_calls_stay_invalid` (`:569`),
     `acknowledgements_use_completed_named_selected_and_boundary_envelopes` (`:619`), `redundant_suspend_markers_still_require_a_complete_source_position` (`:718`).
3. Inputs/outputs: calls `terminal_semantics::calls::callable_refinement::{callable_contract_refinement_goals, descriptor_refinement_goals, validate_descriptor_refinement_evidence, validate_callable_evidence_contracts_in, callable_refinement_context}`
   (`:14-17`) and `crate::checking::{finalize_callable_refinements (:375), derive_contract_obligation (:331), operational::judge (:680)}`. Mutates descriptors' `refinement_evidence`, `refinement_obligations`, contract `requires`/`ensures`, `crash_routes`,
   envelope `blocks`, `published_may_block`.
4. How it decides (as pinned): goals come out in a fixed order `[Input, NormalOutput]` plus `Crash{..}` when the selected machine has a crash route the contract does not permit (`:346-351`, `:440-444`).
   Missing evidence (`:360-362`), an aliased obligation id (`:363-365`), a weakened requirement (`:367-369`), a weaker ensures (`:429-432`) and a mutated certificate conclusion (`:387-393`) are each refused. Operational
   strengthening is refused with exact detail "selected machine may block outside the callable contract" (`:413-416`). Missing block acknowledgement is refused with `RefusalReason::Violation`, detail
   "call may block and requires a block acknowledgement" (`:681-688`), and as a source-level violation "... in machine `build_entry`" (`:709-713`). Roster validity is keyed on lane kind/position, argument subject
   (`parameter_position`), interface argument strings and application argument strings (`:541-562`).
5. Cases by shape: acknowledgement is tested in exactly three shapes: "inferred named", "opaque callable", "boundary" (`:620-647`). The suspend-marker legal positions are listed in the message "a suspend-marked call must be a complete statement,
   simple let initializer, transition subject or terminal expression" (`:758`); legal and one nested negative tested.
6. Same job elsewhere: the retained-certificate refusal here is the checker-side twin of `terminal_semantics::calls::callable_refinement::validate_descriptor_refinement_evidence` (`:355`, `:365`, `:369`, `:432`, `:445`); both are called in the same tests. The hand-built `declared_refinement_module`
   duplicates the typed-tree setup other fixtures get from `super::build_purpose::frontier_of` (`:582`, `:649`): different reason stated at `:116-117` ("real source-issued nominal publication").
7. Unused code: nothing `pub`. Counts not applicable.
8. Stated limits: `:36-37` "These implication controls have no source declaration. Publication uses the separate source-owned fixture below." The abstract form with `descriptors.clear()` and `declaration == None` is expected to finalize without error (`:418-422`), i.e. an abstract form skips refinement; that is by test, not by comment.
9. Fail-open risks: `crate::checking::finalize_callable_refinements` on a module with no descriptors returns `Ok` (`:418-422`): accepting an abstract form with no declared source. Classified "traced to the test only": the test constructs it on purpose (`machines[0].declaration.is_none()`, `:420`); I did not read the production function, so whether a real program can reach that route is open.

## 5. `P06/src/tests/calling_policies.rs`, lines 1-1077 (1,077 lines)

1. Purpose: tests that the checker judges the standard library's calling vocabulary and each hosted target's calling policy (compile-time code that decides argument placement), and tests several storage/domain/frame behaviours that the policy source needs (conjunct knowledge in guards, literal indices, data `requires` restoration, published write frames).
2. Shape:
   - Constants: `VOCABULARY` = `source/library/std/calling.omg` (`:10`), `SHAPES` = `entry_shapes.omg` (`:11`), `TARGETS` four tuples (policy name, plan helper, `targets/<host>/entry.omg`) for WindowsX86_64, LinuxX86_64, LinuxArm64, MacosArm64 (`:13-34`).
   - Helpers: `prepared` (`pub(super)`, `:38`), `judged` (`:73`), `verdict` (`:84`), `describe_verdicts` (`:88`), `typed_hosted_policy` (`:111`), `guarded_index` (`:199`), `keyed_index` (`:295`).
   - Tests: `each_hosted_calling_policy_is_judged` (`:171`), `an_arm_knows_every_conjunct_of_its_guard` (`:226`), `a_bound_every_alternative_establishes_crosses_the_join` (`:251`), `a_literal_index_past_the_extent_is_an_unproved_bound` (`:268`),
     `a_guard_over_the_fields_of_a_formal_reaches_its_arm` (`:317`), `data_with_requires_is_lent_and_restored_at_loan_handback` (`:360`), `an_indexed_record_write_keeps_its_stored_scalar_for_domain_checking` (`:450`),
     `a_call_preserves_only_storage_outside_its_published_write_frame` (`:504`, about 340 lines), `runtime_record_replacement_needs_an_intact_array_and_its_actual_bound` (`:851`), `a_data_clause_that_tests_a_case_is_published` (`:964`),
     `generic_calling_policies_publish_in_their_own_proof_scope` (`:980`).
3. Inputs/outputs: `prepared` picks entry machines whose `display_path(.., "::")` ends with one of the `roots` strings (`:43-51`), freezes with `crate::prepare_fixture`, and calls `crate::lowering::prepare_compilation` with `calling: None, providers: None, purpose: Product` (`:54-66`);
   returns the `TerminalCompilation`. `typed_hosted_policy` wires the std files and a `byte_observation` core file through `SourceScopedTopLevelBinding` imports (`:147-159`). The policy tests slice each target's source text from the substring `"{policy}CallingPolicy satisfies |abi|::CallingPolicy"` to the next `"\nboundary trait "` (`:174-183`).
   `prepared` is used by 4 other test files: `result_fields.rs:233,324`, `returned_views.rs:195`, `table_faults.rs:926` (grep `calling_policies::`, 4 call sites).
4. How it decides: entry selection in `prepared` is by display-path suffix string (`path.ends_with(root)`, `:47-48`), source spelling. `describe_verdicts` maps verdicts to text, and the tests compare those strings. Pinned behaviours: an arm knows every conjunct of `a && b && c` (`:226-243`), a disjunction bound holds only if each alternative bounds
   the index (`:251-261`), a literal index `256` into `[u32; 256]` yields "cannot prove index `256` is within length 256" (`:288`), a guard over fields of a formal reaches its arm but a store between guard and read breaks it (`:341-351`, "what a formal's storage held at entry is one value for the whole invocation", `:312-314`).
   The `@write` parameter of an established value is refused with "parameter write-only reference parameter while reading the signature of `root`" (`:375-379`). A zero-initialized `Positive` cannot be lent as `@mutable` (`:388-397`, `unestablished` non-empty).
   Published write frame: `callee.contract.published_writes == Some(vec![(0, vec![left])])` (`:581-583`), also surviving encode/decode (`:592-599`); an empty forged frame is refused with `RefusalReason::ForeignIdentity`, "published write frame differs from its body" (`:681-690`) and by the decoder `CodecError::MalformedWriteFrame` (`:699-703`);
   an unknown frame stays `None` (`:704-722`); a stale format marker 260 yields `UnsupportedFormatMarker(260)` (`:723-729`). The caller keeps `pair.right == 9` across a call that publishes only `left` (`:510-516`); changing the helper to write `right` fails with one `NormalGuarantee` (`:781-813`).
5. Cases by shape: `TARGETS` is a fixed list of four hosted targets (`:13`). `semantic_indices` "indexes under four" and `fields` "under six conjuncts" are cited as the reason for the width of the conjunct tests (`:223-225`, `:314-315`); the tests cover up to four conjuncts (`:229-232`). `runtime_record_replacement...` uses `[Item; 2]` and bounds 2 / 3 only (`:852-863`, `:918`).
   Quote (`:915`): `"runtime-indexed replacement in an incomplete affine array"` is the refusal suffix when a sibling element was moved out first, via `RefusalReason::Unsupported` (`:904`) returned as `RouteVerdict::FallBack` (`:901`).
6. Same job elsewhere: `typed_hosted_policy` hand-assembles a resolution request with `SourceScopedTopLevelBinding` (`:111-162`), while other tests get a frontier from `super::build_purpose::frontier_of`/`frontier_with_core` (`byte_view_elements.rs:657`, `callable_contracts.rs:582`). Suspected shared purpose, not compared line by line.
   `describe_verdicts` (`:88`) and `verdict` in `byte_view_elements.rs:105` both map a verdict to a string but with different formats (suspected overlap; read both).
7. Unused code: `prepared` is `pub(super)` with 4 external call sites + 3 internal (`:185`, `:77`, `:535`, `:733`, `:771`, `:782`, `:819` are internal uses; I counted by reading, not by command; the 4 external ones are from the grep above).
8. Stated limits: `:263-266` "Lowering used to refuse the out-of-range literal as an unsupported construct." (history note, now an unproved bound). `:532-534` "A portable artifact needs actual package-owned source ... do not give an anonymous invocation a portable identity after preparation." `:976-978` "its bodyless requirement remains a logical declaration." `:1010` "the check form does not borrow the primary entry nomination".
9. Fail-open risks: none seen in production code. Test notes: `each_hosted_calling_policy_is_judged` (`:188-195`) requires the first verdict to be "accepted" and the second to start with "accepted and not published", so the "witness" is accepted but intentionally not publishable as an invocation (stated at `:167-169`).
   `a_call_preserves_only_storage_outside_its_published_write_frame` asserts an unknown (`None`) write frame is preserved as unknown, not as empty (`:714-722`, "an unknown ceiling does not become a complete empty frame"): this guards a fail-open (None treated as no writes), pinned at the codec; the checker side for `None` was not read.

## Coverage

Read in full: `byte_domains.rs` 816-1256 (441 lines); `call_claims.rs` 1-147 (147); `byte_view_elements.rs` 1-2074 (2,074); `callable_contracts.rs` 1-760 (760); `calling_policies.rs` 1-1077 (1,077). Total 4,499 lines.
Skimmed: none. Not read: none.
Greps run: `calling_policies::|byte_view_elements::` over `P06/src` (4 hits, all `calling_policies::prepared`); `fn judge_build|fn judge_portable_byte_build` over `P06/src/tests`; `fn verdict\(` count mode over `P06/src` (17 files, 1 each).

## Five things a maintainer most needs to know

1. All five files are tests; every refusal they pin is by obligation kind, `LoanConflict` variant, refusal reason or an exact/substring message. Many refusals are matched by message text (e.g. `byte_view_elements.rs:329`, `:514`, `:610`, `:646`, `:2069`; `calling_policies.rs:288`, `:377`), so rewording a diagnostic breaks them.
2. A recurring fault-injection pattern (clone the elaborated module, change one id/access/field, assert `!Accepted` or `ForeignIdentity`) checks that the receiving checker rebuilds meaning instead of trusting producer rows: `call_claims.rs:58-147`, `byte_view_elements.rs:811-874`, `:1202-1290`, `:1588-1629`, `:1827-1866`, `calling_policies.rs:601-728`, `:870-895`. Several of these assert only "not Accepted", not which check fired.
3. `byte_view_elements.rs` encodes the rule that a byte read/store/subslice owes `index < length` against the measurement of the identical view, taken directly before it (`:1-9`, `:288-318`); a sibling view, a sibling field or the backing's length never supplies that bound (`:236-244`, `:1702-1729`, `:1782-1792`).
4. `calling_policies.rs` runs the real `source/library/std` calling vocabulary and each of four hosted targets' `entry.omg` through the checker (`:10-34`, `:171-196`); a change to those library files is checked by this test, which slices them by exact text anchors (`:174-183`).
5. Published write frames (`contract.published_writes`) are asserted to be reconstructed from the body on receipt: an empty forged frame is refused by the checker (`calling_policies.rs:681-690`) and by the codec decode (`:699-703`), and an unknown frame stays unknown (`:704-722`).

## What I am unsure about

- I did not read the production functions these tests call (`finalize_callable_refinements`, `judge`, `operational::judge`, the byte-view operation checks), so claims about what the checker does are inferred from what the tests assert, not from the checker code.
- `byte_domains.rs` lines 1-815 (including `judge_build` at `:28` and `judge_portable_byte_build` at `:84`) belong to the other half of the unit and were not read here; helper behaviour is taken from call sites only.
- Which stage refuses in `a_fixed_byte_array_of_unknown_bytes_is_not_cast_into_its_domain` (`byte_domains.rs:999`) is not fixed by the test (either elaboration refusal or one `unproved`).
- The count "17 copies of `fn verdict(`" is a count of definitions with that exact name; I did not compare their bodies.
