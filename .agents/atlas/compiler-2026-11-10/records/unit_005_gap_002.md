Second pass for unit 5 (gap_002). Ranges covered, all in `omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs` (checkout `<checkout>/`):

- R1: 15862-16769 (908 lines)
- R2: 19779-20169 (391 lines)
- R3: 20490-20703 (214 lines)
- R4: 21664-22122 (459 lines)
- R5: 23202-23424 (223 lines)

Total 2195 lines. Every line of every range was read in full with the Read tool. Extra lines outside the ranges were read only to find headers or to cite the function under test (15815-15861, 19748-19778, 19412-19500, 20169-20190, 20470-20489, 20703-20712, 22850-22975); they are not counted in coverage.

All five ranges are inside `#[cfg(test)]` modules. Nothing here is production code. The notes below say what each test module pins about the production function it calls, citing the production line where I read it. I did not read the production functions in full, only the lines named.

Module map for my ranges (from grep of `mod` / `#[cfg(test)]` in the file):
- R1 is inside `source::structural_fields::tests` (`mod structural_fields` 14464, `#[cfg(test)] mod tests` 14473).
- R2 is inside `source::tests` (`#[cfg(test)] mod tests` 19719), plus its nested `mod temporary_stack` (19976-20182).
- R3 is inside `structural::byte_input::tests` (`mod tests` 20482; `mod byte_input` 20188).
- R4 is inside `structural::primitive_locals::tests` (`mod tests` 21660; `mod primitive_locals` 21265).
- R5 is inside `structural::validation::tests` (`#[cfg(test)] mod tests` 23196; `mod validation` 22492).

I found no contradiction with the earlier notes (`unit_005.md` sections 7a-7f). They place structural_fields tests at 14473-16796 and the `tests` module at 19719, matching what I saw.

---------------------------------------------------------------------------------------------------

## R1. 15862-16769, `structural_fields::tests` (908 lines)

**Starts inside.** The test fn `returned_and_parameter_bounded_owners_use_inline_payload_offset` (header at 15821-15825, read). R1 begins mid-way through the `function.operations = vec![ ... ]` literal at 15854, in the `CallStructural` arm's `requirement_obligations: Vec::new()` line (15862). It ends inside `whole_byte_replacement_rejoins_one_qualified_local_producer` (header 16702, body continues past 16769 into lines that belong to another range).

**1. Purpose.** Unit tests for the structural-field replay predicates (`retained`, `indexed_store_retained`, `indexed_read_footprint_retained`, `replacement_footprints_retained`, `requires_graph_storage_replay`, `control_flow::retained`). Each test builds a small abstract function plus a target function from a fixture, asserts the predicate accepts the exact pair, then corrupts one field at a time and asserts it rejects.

**2. Shape (test fns, with lines).**
- tail of `returned_and_parameter_bounded_owners_use_inline_payload_offset` (15821; my part 15862-15993)
- `indexed_byte_publication_retains_every_operand_and_writable_access` (15996-16115)
- `nested_field_membership_reconstructs_parent_local_carrier_identity` (16118-16176)
- `indexed_field_membership_reconstructs_fixed_array_carrier_identity` (16179-16291)
- `byte_field_read_replays_reference_and_descriptor_loads` (16294-16416)
- helper `owned_field_fixture` (16418-16435)
- `owned_parameter_field_membership_keeps_exact_value_access_and_graph_replay` (16438-16496)
- `observed_owned_parameter_discard_keeps_exact_scalar_exit_accounting` (16499-16559)
- `structural_field_membership_requires_exact_source_result_type_access_path_and_field` (16562-16608)
- `structural_field_membership_requires_one_matching_read_across_all_blocks` (16611-16639)
- `structural_field_membership_requires_unique_exact_parameter_declaration` (16642-16676)
- `structural_field_reads_require_graph_storage_replay` (16679-16699)
- head of `whole_byte_replacement_rejoins_one_qualified_local_producer` (16702-16769+)
- Fixtures used but defined outside my range: `field_fixture` (14620), `field_types` (14748, returns `[ScalarType; 3]`), `scalar_field_read_source_mut` (14756), `record_fields_mut` (14811), `packed_record_field` (14602), `structural_call_result` (14502) (grepped `fn field_fixture|...`, lines above).

**3. Inputs and outputs.** Inputs are in-memory `AbstractFunction` and `TargetFunction` values. Output is `assert!`. The only external helper is `crate::object::object_artifact::test_support::structural_declaration` (imported at 20484 for another range; used here as `structural_declaration(...)` at 15875, 16131, 16711). Grepped `\bfn structural_declaration\b|\bstructural_declaration\(` across omega-rust: 125 occurrences in 23 files, so many crates use test helpers of this name; I did not read the others.

**4. How the tested predicates decide (what the asserts pin).**
- Keyed on identity then exact field equality. `indexed_store_retained` rejects any change to `psi_operation`, destination place, destination path, destination access, field id, index source value or type, value source value or type, length value, or obligation (loop `for corruption in 0..12`, 16060-16096; arms 0..=10 plus `_` = 12 variants, the `_` arm duplicates the operation into a second row).
- Access: indexed byte store is retained for `MutableBorrow` and `WriteOnlyBorrow` destination (loop 16009-16012) and rejected for `Owned` and `SharedBorrow` (16097-16113).
- Uniqueness is keyed on `psi_operation`, not on whole-row equality. In 16611-16639: with block 0 cleared and a copy in block 1, retained is true (16616-16621). With matching rows in both blocks, false (16622-16623). Changing only `field` in block 1's row leaves it false (16624-16630). Only after changing block 1's `psi_operation` too does it become true (16631-16637). So a row with the same `psi_operation` but a different field counts as a second candidate and makes the check fail (inferred from the asserts).
- Parameter declaration must be unique and exact: missing (16646-16648), duplicate (16649-16653), duplicate with different access (16654-16655), and for non-boolean results changing `structural_type`, `access`, `multiplicity` or `position` rejects (16656-16672).
- Typed paths need a declaration catalog: `structural_types = Vec::new()` rejects (16448-16453).
- Owned-parameter reads: retained for `Affine`, `Unrestricted` and `Linear` multiplicity (16440-16446); rejected when source access or parameter access is `SharedBorrow` (16483-16487) or `WriteOnlyBorrow` (16490-16494) while the source declares Owned (as built by the fixture).
- Nested path: reconstructing the path through a nested record needs the child declaration; empty path or an unknown field identity rejects (16159-16169); marking the field `BindingRelevance::Erased` rejects (16170-16174).
- Fixed array: path `[Field(maps_field), FixedIndex(1)]` over a 2-long array is retained; index 2 (out of bounds) rejected (16248-16259); a path starting with `FixedIndex`, repeating a field, or ending in `Case` rejected (16260-16282); replacing the array declaration by a record rejected (16283-16289).
- Footprints for byte-field read: `indexed_read_footprint_retained` requires three accesses: reference load (offset 0, 8 bytes, `ReadPlace`), descriptor load (offset 8, 8 bytes, `ReadPlace`), byte read (offset 8, 1 byte, `ReadByteSequence`) (16386-16393). Dropping the first, dropping the descriptor, or moving the reference load to offset 8 each reject (16403-16415).
- Replacement footprints (R1 head): source read offset 8 / length 0, destination write-span offset 8 / length 0, destination place write offset 0 / 8 bytes (15896-15920). Setting the source read to offset 0 rejects (15929-15937). A `BorrowedView` shared-borrow byte-sequence declaration rejects (15939-15951). After removing the producing call and moving the source into a parameter (Owned, Unrestricted, one qualification), the same accesses are retained (15966-15972), and `MutableBorrow`/`WriteOnlyBorrow` parameter access also retained (15980-15992).
- `requires_graph_storage_replay` is true for any slice that contains a structural field read (16691-16697) and false for an unrelated `BooleanConstant` (16685-16687).
- Cleanup-exit accounting: `control_flow::retained(&Return, &target)` rejects when cleanup actions cleared, a different `DiscardRoot` place, different edge, different source value, or a duplicated block (loop 0..5, 16536-16558).

**5. Cases by shape.**
- The fixture scalars are `field_types()`, a fixed list of three scalar types (14748, not read). The corruption loops iterate fixed small counts (`0..12`, `0..8`, `0..6`, `0..5`, `0..4`) and each `match` has an explicit arm per number plus `_` (counted: 16060 has arms 0-10 and `_`; 16568 has 0-6 and `_`; 16454 has 0-4 and `_`; 16536 has 0-3 and `_`; 16659 has 0-2 and `_`). No arm is missing for any loop bound.
- Byte-sequence fixtures use `ByteSequenceCarrier::BoundedOwned { capacity: 16 }` (15878, 16714) and one `BorrowedView` (16327, 15940).

**6. Same job elsewhere.**
- The `StructuralOperationResult { ... }` literal with eight fields is spelled out by hand at 16719 in R1, and at 19767, 19904, 20583, 21696 in my other ranges. Grepped `StructuralOperationResult \{$` in the file: 8 occurrences total (3 outside my ranges: 15844 plus two others). No shared builder is used across them (read).
- The "corrupt one field, assert reject" loop pattern repeats in every range here (16060, 16454, 16536, 16568, 16659, 19857, 20650, 21872, 21943, 22036, 22085).
- `requires_graph_storage_replay` assertions on the same fixture appear at 14882, 15302, 16055, 16447, 16685-16694 (grepped `requires_graph_storage_replay\(`).

**7. Unused / one-caller.**
- `indexed_store_retained` (def 16872 `pub(super)`): grepped `\bindexed_store_retained\b` across omega-rust: 6 matches; production call 18748 `structural_fields::indexed_store_retained(abstracted, operation, targeted)`; others are the `use` at 14476, the def, and 3 test calls (16050, 16093, 16108).
- `indexed_store_footprint_retained` (17018), `indexed_read_footprint_retained` (17143), `replacement_footprints_retained` (17544): production calls at 18749, 18753, 18771 respectively; tests at 16056, 16395, 15624-15985 (grepped, file only).
- `requires_graph_storage_replay` (def 18405, `pub(super)`): production callers 3618 and 24476 (grepped across omega-rust).

**8. Stated limits.** Assert messages are the only commentary: "the reference load must survive" (16403), "the descriptor load must survive" (16408), "a different pointer slot is not the reference" (16414), "a typed path needs its declaration catalog" (16452). No limit comments in this range.

**9. Fail-open risks.** Test code only; none in this range. What the tests pin as accepted: an uncovered-by-assert case is `retained` on `Unrestricted`/`Affine`/`Linear` owned parameters equally (16440-16446), so the tests expect multiplicity not to matter for owned field reads. "None seen" as production risk in this range.

---------------------------------------------------------------------------------------------------

## R2. 19779-20169, `source::tests` and `source::tests::temporary_stack` (391 lines)

**Starts inside.** The test fn `replay_requirement_is_specific_to_primitive_storage_operations` (header 19750, read from 19748). R2 starts at 19779 inside the `let existing = [AbstractOperation::BooleanConstant { ... }]` literal. R2 ends at 20169 inside `temporary_stack_keeps_tight_under_budget_numeric_controls` (20164-20181), just after `let (rows, store) = encoding(&[0, 8, 16]);`. Lines 20170-20184 closing the module were read as extras.

**1. Purpose.** Tests for three small replay predicates and for the temporary-stack peak replay: `requires_graph_storage_replay`, `ieee_literal_retained`, `byte_concatenation_retained`, `temporary_stack_bytes_for_function`.

**2. Shape.**
- End of `replay_requirement_is_specific_to_primitive_storage_operations` (19750-19828): asserts empty slice and slice of `BooleanConstant` do not require graph storage replay (19782-19783); each of `WriteOnlyPrimitiveStore`, `EstablishPrimitiveLocal`, `PrimitiveLocalStore`, `PrimitiveScalarRead` alone requires it, and also when mixed with the boolean (19784-19827).
- `ieee_literal_publication_requires_unique_operation_value_type_and_bits` (19831-19897): float values `Binary32(0x7fc0_0041)` (a NaN payload) and `Binary64(0x8000_0000_0000_0000)` (negative zero). Retained for exactly one matching target row; rejected for zero rows (19852) and two rows (19853-19856); rejected when `psi_operation`, `result`, value bits (xor 1), or float width changes (loop 0..4, 19857-19895).
- `concatenation_publication_retains_ordered_sources_result_and_bound` (19900-19974): retained for one matching target row; rejected for two identical rows (19947-19950), swapped left/right (19951-19957), changed `capacity_obligation` (19958-19966), changed `result.place` (19967-19973).
- `mod temporary_stack` (19976-20182): helpers `instruction` (19990), `function` (20006), `encoding` (20034), `assert_function_peaks` (20076); tests at 20103, 20108, 20113, 20118, 20164.

**3. Inputs and outputs.** In-memory `SelectedFunctions`, `SelectedFormEncodingRow` vector and `SelectedFormEncodedRowStore`. `encoding(peaks)` makes two rows per entry in `peaks`: instruction id 0 (code byte 0x90) carrying `MachineEncodedStackEffect::TemporaryBytesV1 { stack_pointer: RegisterViewId(4), byte_count: peak }` when `peak != 0`, and instruction id 1 (0xc3) with no stack effect (20034-20074). `function(machine)` builds a function with one block of one `CopyI64` instruction (id 0) and a `ReturnUnit` terminator (id 1) (20021-20029).

**4. How it decides (production, at the lines I read).** `temporary_stack_bytes_for_function` is `#[cfg(test)]` (19420-19421). It looks up the function, then calls `temporary_stack_bytes_for_selected_functions` (19437) and takes the machine's entry (19427-19434). Per function, the peak is the max of `TemporaryBytesV1.byte_count` over its instruction rows (`temporary_stack_peak`, 19486-19499). Rows are consumed in instruction order across all functions; a row whose `instruction` differs from the selected instruction id, or repeats an id inside one function, is an error (19456-19460); a missing row errors (19453-19455); a leftover row errors (19478-19482) (read, outside my range). The test pins:
- peaks for machines (1,0),(2,8) and (1,0),(2,8),(3,16) equal the per-function byte counts, i.e. one function's peak does not leak into another (20104, 20109), with the test title naming "directed_fp_peak".
- reverse order of machines gives the same per-machine peaks (20114).
- a changed instruction id, `rows[..3]` (missing row), an extra row, a duplicated instruction id in a function, and an unknown machine 3 each yield `Err` (20132-20160).
- required == exact for machines 2 and 3: `assert!(required <= exact)` and `assert!(required > insufficient)` with `(2, 8, 7)` and `(3, 16, 15)` (20170-20179).

**5. Cases by shape.** Peaks are `u16` in the fixture (`peaks: &[u16]`, 20035) and `u32` in the result (20097). Stack pointer register view is fixed to `RegisterViewId(4)` (20045). Only `MachineAlternativeFamily::CopyI64` rows are generated (20062).

**6. Same job elsewhere.**
- `ieee_literal_retained` takes a slice of `TargetUnitOperation` (`ieee_literal_retained(&source, std::slice::from_ref(&target))`, 19848-19851; production call `ieee_literal_retained(operation, targeted.graph.operations())`, 18867), while `byte_concatenation_retained` takes an iterator of `&TargetUnitOperation` (`[&target]`, 19946; production 18870). Both do "find the one target row for this `psi_operation` and compare fields"; their argument shapes differ (read at call sites; definitions at 19038 and 19119 not read in this pass).
- `SelectedFunction { ... }` full-field literals in tests: 20007 (R2), 20527 (R3), 21752 (R4), plus 5966 outside; `AbstractFunction { ... }` literals: 20513 (R3), 21706 (R4), plus 1018, 14680 outside (grepped `(SelectedFunction|AbstractFunction) \{$`: 8 total in the file; 5 are inside my ranges). Each lists every field by hand; there is no shared constructor.

**7. Unused / one-caller.** `temporary_stack_bytes_for_function` (19421): grepped `\btemporary_stack_bytes_for_function\b` across omega-rust: 7 matches = 1 definition (19421, `#[cfg(test)]`), 1 `use` (19977), 5 call sites, all inside `temporary_stack` tests (20086, 20124, 20144, 20153, 20171). So the function exists only for these tests; the production path calls `temporary_stack_bytes_for_selected_functions` directly (19413).

**8. Stated limits.** None in the range. Test names carry the intent ("does_not_import_another_functions_directed_fp_peak", 20108).

**9. Fail-open risks.** Test only; none seen. What the tests accept: a function with no stack effect yields peak 0 (`unwrap_or(0)`, 19498; test peaks include 0 at 20104). A row with no footprint in the store is skipped, not an error (`if let Some(footprint) = encoded.footprint(row.footprint)`, 19461-19463; traced: a missing footprint contributes no stack effect and so cannot raise the peak; the tests in R2 never build a row whose `footprint` is absent from the store, so that path is untested in this range).

---------------------------------------------------------------------------------------------------

## R3. 20490-20703, `structural::byte_input::tests` (214 lines)

**Starts inside.** The `use selected_instructions::{ ... }` import list inside `mod tests` (opened at 20482; `use super::cleanup_actions_match` at 20483). I read 20470-20489 as extras (the preceding production function ends at 20479).

**1. Purpose.** One fixture and one test for `cleanup_actions_match`, the check that the terminal affine cleanup actions of a function correspond to the structural results of hosted-read-byte settlements.

**2. Shape.** `fixture()` (20503-20639) returns `(AbstractFunction, SelectedFunction, Vec<TerminalAffineCleanupAction>)`. It builds a sum-type declaration with two cases (case 1 empty, case 2 one `i32` field) (20540-20562), then for operation numbers `[2, 1]` pushes an `AbstractOperation::BoundaryCall` with structural result, a `StructuralPlaceDeclaration` (`OperationResult { producer, structural_type }`), and a `SelectedBoundarySettlement` with payload `HostedReadByte` and layout `calling_conventions::evaluate_conventional_sum_layout(&[], &[vec![], vec![ValueShape::integer(4, 4)]])` (20580-20630). The returned actions are `DiscardRoot(PlaceId 2)` then `DiscardRoot(PlaceId 1)` (20634-20637). Test `read_result_cleanup_requires_exact_read_payload_type_function_and_actions` (20641-20707).

**3. Inputs and outputs.** Calls `cleanup_actions_match(&source, &selected, &actions) -> bool` (def 20219, `pub(in crate::function_fragments)`). At the lines I read (20224-20231) it returns `false` when `selected.structural` is `None` and when `function.result` is not `Unit`.

**4. How it decides (what the test pins).**
- True for the full action list (20644), for each single action alone (`actions[..1]`, `actions[1..]`, 20645-20646), and for the reversed list (20647-20649). So the match is order-insensitive and accepts a subset of the read results' discards (inferred from asserts). The empty slice is not asserted in this range.
- False for these seven mutations (20650-20705): the sum shape changed to a packed record (0); one settlement popped (1); function result changed to a scalar (2); a duplicated action (3); a `DiscardRoot` of unknown place 99 (4); a settlement payload changed to `HostedExitProcessI32` (5); the place declaration's producer changed to operation 3 (6).
- Keys: place identity, producer operation identity, the settlement payload variant, the structural type's shape, and the function result variant.

**5. Cases by shape.** The fixture only has `HostedReadByte` settlements and a sum with exactly cases `[empty, one i32 field]`, matching `hosted_read_byte_declaration_is_valid` (earlier notes, object_artifact.rs:4031-4039: "exactly two cases `[empty, byte]`"). Here the field type is `i32` with layout `integer(4, 4)`. `function.result` is `Unit` only (20520).

**6. Same job elsewhere.** The two cleanup places are matched against settlements by position/identity; the sum declaration check duplicates `hosted_read_byte_declaration_is_valid` (object_artifact.rs:4028), whose use is visible at 20475 (read, `!hosted_read_byte_declaration_is_valid(declaration)` in the production function just above R3).

**7. Unused / one-caller.** `cleanup_actions_match` (20219): grepped `cleanup_actions_match|read_result_cleanup_actions_match` across omega-rust: defined 20219; re-exported at 20965 as `read_result_cleanup_actions_match` (`pub(super) use byte_input::cleanup_actions_match as ...`); one production call at 18994 (`super::structural::read_result_cleanup_actions_match(abstracted, selected, cleanup_actions)`); the other matches are the `use` at 20483 and test calls 20644-20703. One production caller.

**8. Stated limits.** None.

**9. Fail-open risks.** In the test's reach: partial or reordered action lists are accepted by design of the test (20645-20649). Whether an empty action list is accepted is not asserted in this range, so I cannot say. Marked: "open" for the production function (the body after 20231 was not read in this pass).

---------------------------------------------------------------------------------------------------

## R4. 21664-22122, `structural::primitive_locals::tests` (459 lines)

**Starts inside.** The `use` block of `mod tests` (opened at 21660): the first line of R4 is `use abstract_operations::{ AbstractFunction, ... }` (21664). R4 ends at 22122, the line `pub(super) use primitive_locals::operation_retained as primitive_operation_retained;` that follows the end of `mod tests` (22119) and `mod primitive_locals` (22120).

**1. Purpose.** Tests that the replay of an established primitive local (an 8-byte stack slot written through a frame address) is accepted only for one exact shape: producing operation, address instruction, store instruction, slot size/alignment, frame offset, and the pointer-typed argument that publishes a reference to it.

**2. Shape.**
- `fixture()` (21686-21865): abstract function with one `EstablishPrimitiveLocal` (u64 value, `Unrestricted`); selected function with a `Structural` local slot of 8 bytes / alignment 8 (21776-21780), two memory accesses (`AddressLocal` instruction 0, `WritePlace` instruction 1, 21785-21802), two virtual registers (`AbiTransport` and `EntryParameter`, 21805-21827), instructions `FrameAddress` (0), `Store { byte_offset 0, byte_size 8 }` (1) and `ReturnUnit` (2) (21832-21860).
- `initialized_local_requires_exact_producer_address_and_executed_store` (21868-21922): 13 mutations (`0..13`; arms 0..=11 plus `_`) of the selected function, then a duplicated producer and a changed place on the abstract side. All yield `establishment(...) == None`.
- `local_frame_identity_cannot_be_replaced_by_matching_geometry` (21925-21968): `frame_location(homes, 24, slot, 8)` returns `Stack { byte_offset: 16 }` for one home (`frame_offset_bytes 16`, size 8, alignment 8); errors for no homes, two identical homes (21941), frame size 23 (21942, i.e. 16+8 exceeds frame), different operation id or place id, offset 17, offset `u64::MAX - 7` (overflow), size 16, alignment 4 (loop 0..6, 21943-21966).
- `primitive_reference_publication_preserves_original_home_and_total_view` (21971-22067): builds a `TargetStructuralArgument` with path `[Field(identity 2), Referent]`, `root_structural_type` 1, `structural_type` 2, shape `borrowed_reference(4, 4)`, source `EstablishedPrimitiveLocal`, destination a Linux x64 call-plan placement (22001-22029). `argument_matches_source(&argument, source_type, source_scalar (binary32), &types)` is true (22030); false for changing the root type, the view type to a 64-bit or boolean type, access to `MutableBorrow`, clearing the path, or popping the last path segment (loop 0..6, 22036-22052); false for a different source scalar (22053-22058) and for a duplicated type declaration (22059-22066).
- `inherited_root_membership_preserves_exact_primitive_transport` (22070-22118): with a qualification pushed on the result, `establishment` returns `Some((OperationId(1), 8))` (22080-22083); changing multiplicity to `Affine`, adding a qualification-establishment, a projected qualification, or a claim each give `None` (22085-22114); clearing the store's `provenance.values` gives `None` (22115-22117).

**3. Inputs and outputs.** `establishment(function, selected, place) -> Option<(OperationId, u8)>` (def 21404, `pub(super)`) returns the producer operation id and the primitive byte size; `frame_location(homes, frame_size_bytes, slot, size) -> Result<StructuralSourceLocation, Error>` (21591); `argument_matches_source(argument, source_type, source_scalar, types) -> bool` (21618).

**4. How it decides.** From the lines I read: `establishment` returns `None` if the producer is missing, the primitive size is unsupported, `selected.structural` is `None`, or the result's multiplicity is not `Unrestricted` or has projected qualifications, qualification establishments or claims (21409-21415). It is also keyed on the place declaration's producer identity (mutation 10, 21890-21896), the slot id (`LocalStorageSlotId::Structural { operation, place }`), the instruction order, and virtual-register identity (mutations 2-3, 12). `frame_location` keys on slot id equality, a unique home (21599-21601), `home.size_bytes == size` and `home.alignment_bytes == size` (21602-21603). `argument_matches_source` keys on a unique declaration for the type id (21624-21629) and the scalar shape. The inherited qualification is allowed: a qualification on the result still yields `Some` (22079-22083), but every custody-carrying field (multiplicity, establishments, projected qualifications, claims) rejects.

**5. Cases by shape.** `primitive_size(value.scalar_type)` limits locals to the scalar types it accepts (fixture uses `u64`, size 8; I did not read `primitive_size`). `frame_location` compares `home.alignment_bytes != u16::from(size)` (21603): alignment must equal size, so only naturally aligned primitive locals are accepted. The reference test covers `ScalarType::IeeeFloat(Binary32)` as the source with `Unsigned(32)`, `Unsigned(64)` and `Boolean` as wrong views (21979-21989) and expects the matching `view_type` to be a primitive of the same scalar (types list index 2 is `Unsigned(32)`; the mutation to type 3 is `Unsigned(64)` and to type 4 `Boolean`, 22040-22041).

**6. Same job elsewhere.**
- The pointer-origin / store-sequence check mirrors `source::placed_reference::retained` for `EstablishAccessReference` (earlier notes 7a, :13022): "destination slot must be size 8 / alignment 8 (:13050), store is Store{byte_offset:0, byte_size:8}". The fixture here uses the same size-8/alignment-8 slot and `Store { byte_offset: 0, byte_size: 8 }` (21776-21780, 21841-21846). Two separate predicates describe the same "frame address, then 8-byte store" idiom (the earlier-note site was not re-read by me; "suspected" same job).
- A like-named `establishment` fn exists in another crate: `03_target-operations-to-selected-instructions/src/selection_inputs.rs:4044` (`pub(crate) fn establishment`, grepped `\bfn establishment\b`); it is called from `selection_inputs.rs:2513, 5287` and `selection_structural.rs:3121` via `crate::selection::reference_input::establishment`. Not read; "suspected" same job on the selection side (the image-side function replays the selection-side decision).

**7. Unused / one-caller.**
- `establishment` (21404, `pub(super)`): production callers at 21285, 21293, 21548 (in the same module, `primitive_locals::{access?, establishment, frame_location}`); tests 21871-22117 (grepped `\bestablishment\(` across omega-rust; matches in the other crate are a different function).
- `frame_location` (21591): production call at 21583; tests 21937-21964.
- `argument_matches_source` (21618): production call at 21552; tests 22030-22061.
- `operation_retained` re-exported as `primitive_operation_retained` (22122): one production caller at 18829 (grepped `primitive_operation_retained`).

**8. Stated limits.** None in the range; assert messages "mutation {n}" and "custody mutation {n}" (21908, 22112).

**9. Fail-open risks.** Test only; none seen. The `_` arms of the match mutations (21901, 22103, 21961, 22044) are explicit mutation choices, not accept paths.

---------------------------------------------------------------------------------------------------

## R5. 23202-23424, `structural::validation::tests` (223 lines)

**Starts inside.** `mod tests` (opened 23196). R5 begins at the blank line before `use super::validate_incoming_location;` (23203) and ends at 23424, after the closing braces of `mod tests` (23422) and `mod validation` (23423).

**1. Purpose.** Tests for `validate_incoming_location`, which checks that the structural source location recorded for an incoming parameter agrees with the declared access and the calling-convention placement.

**2. Shape.** Helper `borrowed(location)` builds `StructuralSourceLocation::IncomingBorrowedPointer { location }` (23205-23207). Tests:
- `borrowed_register_and_stack_locations_preserve_exact_pointer_residence` (23210-23302): for `SharedBorrow`, `MutableBorrow`, `WriteOnlyBorrow`; register case with `Aarch64X(0)` accepts its own register, rejects `Aarch64X(1)` and rejects `IncomingIndirectPointer` (23216-23250); stack offsets 0, 8, 32, 56 accept both a `ValueLocation::Stack` (size 8, alignment 8) and a `ValueLocation::Indirect` with `Stack` pointer (4 bytes, alignment 2) under shape `borrowed_reference(4, 2)`, and reject an offset +8 and a plain `StructuralSourceLocation::Stack` (23251-23299).
- `borrowed_locations_reject_copies_carrier_width_alignment_and_access_substitution` (23305-23383): shape `borrowed_reference(16, 8)` with `Indirect { Register pointer, copy None, 16, 8 }` accepted for `WriteOnlyBorrow` (23317-23324), rejected for `Owned` (23325-23328); rejected when `copy_stack_byte_offset: Some(0)`, byte size 8, alignment 4, or when the location is a `Register` with value offset 1 or byte size 4 (23329-23368); rejected when the shape is `integer(8, 8)` (23369-23382).
- `owned_indirect_location_keeps_owned_copy_meaning` (23386-23421): shape `integer(16, 8)` with `Indirect { Register pointer, copy Some(0), 16, 8 }` accepts `IncomingIndirectPointer { register }` for `Owned` (23397-23404); rejects an `IncomingBorrowedPointer` for `Owned` (23405-23412) and for `SharedBorrow` (23413-23420).

**3. Inputs and outputs.** `validate_incoming_location(access, placement, location) -> Result<(), Error>` (def 22850, read 22850-22950). `Error::Mismatch("structural incoming location differs from declared access or ABI")` is the single error (22855-22857).

**4. How it decides (production lines read).** Keyed on access first: `Owned` requires exactly one `Indirect` location with `copy_stack_byte_offset: Some(_)`, shape class `Integer`, and byte size and alignment equal to the shape's, then matches register-pointer with `IncomingIndirectPointer` or stack-pointer with `IncomingIndirectStackPointer` by equal register or equal stack offset and alignment (22858-22892). Otherwise the shape class must be `BorrowedReference`, or `BorrowedScalarReference` with shape equal to `borrowed_scalar_reference()` (22894-22902); the source location must be `IncomingBorrowedPointer` (22903-22905); then three arms: a single `Register` at value offset 0 and size 8 equal to the pointer register (only when not a scalar descriptor, 22907-22916); a single `Stack` size 8 / alignment 8 with equal stack offset (22917-22930); a single `Indirect` with `copy_stack_byte_offset: None`, equal pointer, shape byte size and alignment, and `indirect_pointer_supports_copy(pointer)` (22931-22947); anything else errors (22948). Keys are location variant, register identity, stack offset and shape size/alignment.

**5. Cases by shape.** Fixed list at 22906-22948 (three placement shapes) plus the owned branch; everything else is `Err`. The tests only use `MachineRegister::Aarch64X(0|1)` (23216, 23238, 23306, 23349, 23387); no x86 register is exercised in R5. The `scalar_descriptor` branch (`borrowed_scalar_reference()`) is not exercised by any assert in R5 (the only non-`borrowed_reference` shape used is `integer(8, 8)`/`integer(16, 8)`, 23374, 23389).

**6. Same job elsewhere.** `image.rs entry_effects`/`object_artifact.rs` do not duplicate this; the location variants are produced from `graph_structural::indirect_source_location` (installation_record.rs:9-26, earlier notes; one caller at function_fragments.rs:23632, `incoming_location`, outside my ranges). Not re-read.

**7. Unused / one-caller.** `validate_incoming_location` (22850, private to `validation`): grepped `\bfn validate_incoming_location|incoming_location\(` across omega-rust: 19 matches, all in `function_fragments.rs`; production calls at 22580 and 22783 (2 callers); the rest are `use` and tests in R5 (23226-23414). Counts from the Grep call whose per-line output I read: 22580, 22783, 22850, 23203, then 12 test calls plus the `incoming_location(` hit (the 19 total includes `incoming_location(` matches such as 23632 that are not this function).

**8. Stated limits.** None in the range. Test names state the intent ("reject_copies_carrier_width_alignment_and_access_substitution", 23305).

**9. Fail-open risks.** None seen in the tests. In the production function at the lines I read, every unmatched shape ends in `Err` (22868, 22891, 22901, 22904, 22948); there is no `_ => Ok`.

---------------------------------------------------------------------------------------------------

## Coverage

- Read in full: R1 15862-16769 (908), R2 19779-20169 (391), R3 20490-20703 (214), R4 21664-22122 (459), R5 23202-23424 (223). Total 2195 lines.
- Skimmed: 0.
- Not read: 0.
- Commands: Grep (content/count mode, uncapped) over `<checkout>/omega-rust` for `temporary_stack_bytes_for_function`, `cleanup_actions_match|validate_incoming_location|argument_matches_source|frame_location`, `requires_graph_storage_replay|ieee_literal_retained|byte_concatenation_retained`, `read_result_cleanup_actions_match|establishment|primitive_operation_retained`, `indexed_store_retained`, `structural_declaration`, `StructuralOperationResult \{$`, `(SelectedFunction|AbstractFunction) \{$`, and `mod|#[cfg(test)]` (module map).

## Five things a maintainer most needs to know about this unit

1. Every line in these five ranges is test code in a `#[cfg(test)]` module; no production logic lives here (module map grep: 15862-16769 in `structural_fields::tests` 14473, 19779-20169 in `source::tests` 19719, 20490-20703 in `byte_input::tests` 20482, 21664-22122 in `primitive_locals::tests` 21660, 23202-23424 in `validation::tests` 23196).
2. Uniqueness of a replayed target row is keyed on `psi_operation`, not on the whole row: a same-`psi_operation` row with a different field counts as a second candidate and fails the check (16611-16639, inferred from asserts).
3. `temporary_stack_bytes_for_function` (19421) is `#[cfg(test)]` and used only by the five `temporary_stack` tests; production calls `temporary_stack_bytes_for_selected_functions` (19413). The tests pin per-function peaks, in-order row consumption, and rejection of changed, missing, extra and duplicate rows (20102-20181).
4. `cleanup_actions_match` has one production caller (18994 through the alias at 20965) and its test accepts any subset or reordering of the read results' `DiscardRoot` actions (20644-20649); whether an empty list is accepted is not tested in R3.
5. Test fixtures are rebuilt by hand each time: `StructuralOperationResult` literal 8 times in the file (5 inside my ranges), `SelectedFunction`/`AbstractFunction` full literals 8 times in the file (5 inside my ranges), with no shared constructor (grep counts above).

## What you are unsure about

- I did not read the production bodies of `indexed_store_retained` (16872), `ieee_literal_retained` (19119), `byte_concatenation_retained` (19038), `cleanup_actions_match` past line 20231, `establishment` past 21416, or `argument_matches_source` past 21630; the statements above about them come from the assertions in the tests plus the production lines I did read (named in each section).
- The claim "subset or reordering accepted" for `cleanup_actions_match` rests on three asserts (20645-20649); the production body might accept it for a reason unrelated to the subset (for example by matching each action independently), which I did not read.
- The count of 19 for `incoming_location\(|fn validate_incoming_location` (R5 section) mixes `validate_incoming_location` with other fns ending in `incoming_location(`; I listed the individual lines of `validate_incoming_location` from the separate content-mode grep, giving 2 production callers.
- Whether `primitive_size` (used at 21410) limits locals to 8-byte scalars was not checked; the tests only exercise `u64` and the returned size 8 (22082).
