# Record unit_013 (area 10-native-stages-04-10): stage 05 selection files and stage 06 analyses files

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/` unless stated. Claims are "read" unless marked "grepped" or "inferred". `S05` below means `05_target-operations-to-selected-instructions/src/`. `A06` means `06_selected-instructions-to-selected-instructions/analyses/src/`. Within `selection_scalar_graph.rs` the file is cited as `SG`, within `selection_structural.rs` as `ST`, within `selection_views_and_bytes.rs` as `VB`.

Shared pattern for stage 05 files (read): every emitter returns `Result<_, SelectedInstructionError>` and turns any unexpected shape into `SelectedInstructionError::custody()` or `::unsupported_shape(function)`. Values are looked up with `Builder::resolve`, a linear scan keyed on `ValueId` (`SG:7917-7930`). Places are looked up with a linear scan of `builder.transport.pointers: Vec<(PlaceId, VirtualRegisterId)>` (e.g. `ST:5490-5495`).

## 1. `S05/select_instructions.rs` (59 lines)

1. Purpose: library root of stage 05. Turns a stage-04 plan into selected instructions after binding it to its installations.
2. Shape: `select_instructions(input: SelectionInput)` (`:17`); re-exports `SelectionInput`, `OptimizedSelectionPipelineError`, `StagedOptimizedSelectedInstructions` (`:13-14`); `test_support` only under feature `test-support` (`:9-10`).
3. Inputs and outputs: reads `input.body.into_selection()`, `semantic.admission.installed_interrupts()` and `.native_callbacks()` (`:20-30`), `baseline_target_register_environment` (`:36-38`). Calls `InstallationRequirements::bind`, `BoundExecutionPlan::bind`, `select_bound_instructions`, `selection_custody_receipt` (`:28-44`). Returns the staged struct with `NativeBodyFacts::retain_selected_body_facts` in an `Arc` (`:46-52`).
4. Decides on identity: refuses when `target.psi != semantic.program.psi || target.entry != semantic.program.entry` (`:23-27`).
5. Cases by shape: none. 6. Same job: `test_support.rs:15-21` repeats register-environment + bind + select without installation binding.
7. Unused code: `select_instructions` is the crate entry; one external caller per crate map (not re-grepped).
8. Stated limits: none. 9. Fail-open: none seen.

## 2. `S05/selected.rs` (130 lines)

1. Purpose: output record of stage 05 and the custody receipt builder.
2. Shape: `StagedOptimizedSelectedInstructions` (`:17`, fields `body`, `register_environment`, `selections`, `budget_per_pass`, `selected`, `custody`); `StagedOptimizedSelectionCustodyReceipt` (`:61`: `core`, `legalized`, `legalization_validator`, `selected`, `function_count`); `OptimizedSelectionPipelineError` (`:90-96`, variants `Installation(&'static str)`, `BodyBindings(&'static str)`, `RegisterEnvironment`, `Binding`, `Selection`); `selection_custody_receipt` (`:106`); `selected_program()` (`:45`).
3. Inputs and outputs: copies identities (`psi`, `target`, `entry`, `optimization`, `projection`, `manifest`, `optimization_unit`, `work_schedule`, register environment, legalized, selected) into the receipt (`:113-129`).
4. Decides: nothing. 5. none. 6. none.
7. Unused: not grepped. 8. none.
9. Fail-open: none seen. `Display` prints `{self:?}` (`:100`); a message format, not a decision.

## 3. `S05/test_support.rs` (22 lines)

Purpose: test-only entry binding and selecting a plan with supplied `BoundInstallations` (`select_instructions_for_plan`, `:8`). Re-exports `BoundInstallations`, `InstallationBindingError`, `InstallationRequirements` (`:2-3`). Same job as `select_instructions.rs:36-42`. Fail-open: none seen.

## 4. `S05/instruction_selection/selection_scalar_graph.rs` (8,260 lines)

1. Purpose: builds one `SelectedFunction` from a legalized scalar function. Owns the per-function `Builder`, walks blocks in a derived order, dispatches each legalized operation to an emitter, builds terminators (return, jump, conditional branch, structural case, no-normal invoke, process exit, cleanup calls), and finishes with edge-transfer preparation. Structural operations go to `selection_structural` (`SG:6456`).
2. Shape. The file is a header comment plus about 30 nested `pub(crate) mod`s:
   - `accessor_scalar_access::emit` (`:66`); `aggregate_argument::{argument,transport,argument_register}` (`:234,339,706`); `aggregate_memory::{load,store,scratch}` (`:747,779,815`); `aggregate_return::build` (`:830`); `atomic_access::emit` (`:1074`); `boolean_value::{emit,emit_branch_comparison,mixed_integer}` (`:1231,1351,1475`); `byte_input::emit` (`:1664`); `byte_output::emit` (`:1751`).
   - `call_arguments` (trait `InvocationArgumentSource` `:1834`, impls for `LegalizedScalarInstruction` and `TerminalInvocation` `:1845,1900`; `marshal` `:1988`; `register_ordered_operands` `:2038`; `scalar_argument` `:2062`; `structural_argument` `:2104`; `referent_pointer` `:2219`; `validated_call_key` `:2466`; `emit_call` `:2491`); `cleanup_call::emit` (`:2521`).
   - `control::{build,successor,project_block_storage,block_order,branch_suffix}` (`:2610,2982,3095,3176,3182`); `float_result_policy::emit` (`:3262`); `ieee_comparison::emit` (`:3329`); `ieee_float_arithmetic::emit` (`:3620`); `integer_arithmetic::{emit,binary_operands,instruction_scratch,division_scratch}` (`:3688,4399,4645,4669`); `integer_conversion::emit` (`:4683`).
   - `no_normal::{scratch,build}` (`:4742,4765`); `normalized_foreign::{emit,emit_invocation,descriptor_input::emit,scalar_result_buffer}` (`:5097,5120,4924,4979`); `operation_control` (`OpenBlock` `:5505`; `close`, `when_nonzero`, `require_nonzero`, `while_nonzero`, refusal-region helpers, `:5517-5866`); `process_exit::build` (`:5879`); `register_access::emit` (`:6099`); `scalar_call::emit` (`:6196`); `scalar_stack::{address_register,entry,invocation_argument}` (`:6308,6328,6407`); `structural_case::{build,register,memory,frame_address_pointer,successor}` (`:6478,6784,6824,6843,6863`); `unit_call::{emit,retain_result}` (`:6921,7051`).
   - Top level: `block_containing_operation` (`:7191`); `build_with_environment` (`:7208`); `select_function` (`:7231`); `next_index` (`:7821`); struct `Builder` (`:7826`) with fields `installations, backings, environment, pending_provenance, required_values: BTreeSet<ValueId>, transport: structural::Transport, class, constraints, catalog, registers: Vec<VirtualRegister>, pending_result_origins, instructions, implementation_blocks, open_block, next_block, block_body_end, edge_work, definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ClosedScalarType)>`; `impl Builder` (`:7855-8260`: `crash_form`, `edge_work_at`, `settle_provenance`, `resolve`, `register`, `emit_frame_address`, `emit_local_frame_address`, `emit_address_offset`, `emit_load64`, `emit_materialize_i64`, `emit_copy_i64`, `emit_byte_view_address`, `emit_store64`, `emit_store`, `emit`, `emit_integer`, `copy`).
3. Inputs and outputs: reads a `NativeFunction` (legalized scalar function plus geometry), `SelectedSelectionConstraints` (constraint keys), the validated register environment, `ForeignScalarBackings`, `BoundInstallations` (`:7231-7239`). Produces `SelectedFunction { contracts: Arc<SelectedFunctionContracts{outgoing_arguments, local_storage_slots, calls, normalized_foreign_calls, memory_accesses, boundary_settlements, virtual_registers, ...}>, entry_block, blocks }` (`:7770-7804`), then runs `edge_transfers::prepare` (`:7805`) and requires `ids_are_positions()` (`:7813`).
4. How it decides:
   - Operation dispatch is on the variant of `LegalizedScalarInstructionKind`: early emitters (`byte_input`, `byte_output`, `PortWrite`, `AtomicFence`, `descriptor_tables`, `global_descriptor_table`, `register_access`, `accessor_scalar_access`, `atomic_access`, `structural::operation`) then one `match` (`SG:7510-7700`). Variants meant to be handled earlier return `invalid()` there (`:7550-7561`, `:7651-7696`).
   - Constraint rows come from `constraints.keys.<name>` or `keys.for_semantic(MachineSemanticKind::...)` (e.g. `:111-115`, `:1093-1103`). A missing key gives `custody()` or `AtomicFormUnavailable`.
   - Types: equality of `ClosedScalarType` against what `resolve` returns (e.g. `:1255-1257`, `:3757-3764`).
   - Crash form depends on `installations.installed_entry` being present (`:7859-7870`).
   - Fold decisions: `folded_zero` / `folded_immediate` skip an operation when it feeds a branch suffix (`:7425-7430`), keyed on block and operation position.
   - Branch fusion `control::branch_suffix` scans every block's instructions and terminators for other uses of the compare result (`:3182-3249`), keyed on `ValueId`.
5. Cases by shape:
   - Integer widths: `matches!(integer.bits(), 8 | 16 | 32 | 64)` with `Fixed` carrier, repeated at `:1275-1282`, `:1375-1381`, `:3759-3761`, `:3785-3787`, `:4010-4011`, `:4078-4080`, `:4132-4135`, `:4283-4287`; mixed-sign compare excludes address types (`:1491-1497`).
   - Structural case: `layout.tag_byte_offset != 0 || layout.tag_shape != ValueShape::integer(4, 4)` is refused; case tags must equal their ordinal (`:6502-6508`).
   - Activation claim: `cases.len() != 2` or `cases[0].fields.len() != 1` refused (`:1764-1773`, in `ST`).
   - Return: scalar return only for one 8-byte `Register` location with shape `integer(8, 8)` and empty `return_aggregate` keys (`:924-933`).
   - Process exit: exactly one argument of type `i32` signed, kind `ProcessExit` (`:5903-5915`).
   - Cleanup call: exactly one `Structural` argument with `Owned` access and no result, claim transfer or crash continuation (`:2541-2555`).
   - Register access: `GlobalDescriptorTable`, `TaskRegisterActivate/Read`, `SegmentReload/Read` give `custody()` in `register_access` (`:6110-6116`).
   - Parameter byte size per type: Callable 8, Boolean 1, Binary32 4, Binary64 8, Integer 8/16/32/64, else error (`:7289-7306`).
   - Float constants must fit 32 or 64 bits; boolean constants 0 or 1 (`:3717-3736`).
   - `InstallStaticInterrupt` is refused here: "installed table publication requires the complete guarded physical implementation" (`:7646-7649`).
6. Same job elsewhere:
   - Scratch-register creation: `integer_arithmetic::instruction_scratch` (`:4645`), `no_normal::scratch` (`:4742`), `aggregate_memory::scratch` (`:815`, wraps the first), `structural_case::register` (`:6784`), `operation_control::settle_preceding_operations` (inline push, `:5557-5594`), `scalar_stack::address_register` (`:6308`). Other definitions named `scratch` by `grep -rn "fn scratch"` over `S05`: `VB:914` (wraps `instruction_scratch`), `selection_interrupts.rs:796`, `selection_access_lifecycles.rs:6445` (those two not read in this unit; suspected same job).
   - Comparison materialization (`CompareI64` then `MaterializeBoolean*` chosen by (predicate, signed)): `boolean_value::emit` (`:1284-1347`), `IntegerComparisonSequence::compare` (`:1605-1649`), `ieee_comparison::Emitter::compare` (`:3448-3474`).
   - Conditional-branch lowering with a `BooleanNot` chain: `control::build` `Conditional` arm (`:2831-2963`) and `boolean_value::emit_branch_comparison` plus `branch_suffix` (`:1351-1473`, `:3182`).
   - Outgoing-argument slot registration (`SelectedOutgoingArgumentSlot` push): `aggregate_argument::transport` (`:430-438`, `:678-685`), `call_arguments::place_pointer_argument` (`:2440-2448`), `scalar_stack::invocation_argument` (`:6431-6436`), `normalized_foreign::emit_invocation` (`:5191-5203`).
   - Float transfer copy (set `registers[..].class` from the row's operand 1): `aggregate_argument::argument_register` (`:726-737`), `call_arguments::scalar_argument` (`:2085-2097`), `control::build` Return (`:2768-2786`), `normalized_foreign::emit_invocation` (`:5232-5242`).
   - `integer_carrier_normalization` with `CopyI64` fallback, written out at `:200-221`, `:1192-1209`, `:3801-3805`, `:4111-4113`, `:5453-5471`, `:6272-6283`.
   - "Exactly one local slot with this id, size and alignment" check: `:284-296`, `:875-889`, `:6570-6583`.
   - Cleanup calls are emitted in `control::build` `ReturnCleanup` (`:2650-2660`), `aggregate_return::build` (`:1007-1017`) and per-edge in `select_function` (`:7746-7761`).
7. Unused or one-caller (grepped: `grep -rn --include=*.rs NAME .` under `source-rust/`, excluding `_recycling`; counts are lines): `build_with_environment` 2 (definition plus one call); `select_function` 2; `block_containing_operation` 2 (definition plus `SG:7083`); `division_scratch` 6; `instruction_scratch` 15; `register_ordered_operands` 3; `TerminalInvocation` 4. The header imports under `#[allow(unused_imports)]` (`:9-46`, eleven blocks) hide whether each import is used; not checked.
8. Stated limits (quotes under 20 words): "installed table publication requires the complete guarded physical implementation" (`:7648`); "Valid inputs cannot take this edge. Keep a physical trap as a defensive invariant check" (`:2666-2667`); "This borrowed descriptor scratch lane has no action owner." (`:1981-1982`); "Preparation closed the result's type; it must be the accessor's declared scalar." (`:154`).
9. Fail-open risks:
   - `Builder::emit_integer` skips the pinned-operand copy when a register's origin is `InstructionScratch` (`:8227`). A scratch register placed in a position the constraint row pins is not copied. Status: open (not traced to a caller that does this).
   - `block_containing_operation` returns the first block that contains the operation id (`:7195-7205`). If the same `OperationId` appeared in two blocks the first would win. Status: open.
   - Everywhere `resolve` returns `None` the callers map it to an error. None seen that accept on `None`.

## 5. `S05/instruction_selection/selection_structural.rs` (5,926 lines)

1. Purpose: selection of place-backed (structural) operations: parameter and block-parameter homes, record/case/array/local establishment, leaf copy and field reseat, references, descriptor operands, primitive-local reads and writes, encoded and fragmented (bit-packed) scalar storage, activation claims, native-code layout, indirect results, runtime address arithmetic. Re-exports submodules of sibling files as `structural::*` (`ST:446,614-619,938-940,1704,2329-2332`).
2. Shape. Modules and entry functions: `block_views::block_entry` (`:51`); `entry::entry` (`:137`); `atomic_outcome::establish` (`:457`); `descriptor_operands::{source_geometry,completed_view_words,backing,element_stride,stored_address,view_words}` (`:636-865`); `leaf_copy::{copy,reseat,copy_operand,project_operand}` (`:957,1068,1184,1234`); `literals::establish` (`:1436`); `local_storage::{descriptor,fixed_array_argument,byte_field_argument,store,address}` (`:1553-1675`); `activation_storage::emit` (`:1724`); `native_code_layout::establish` (`:1972`); `case_membership::observe` (`:2036`); `descriptor_tables::emit` (`:2141`); `primitive_locals::{write,read,address,borrowed_storage_address,global_descriptor_table}` (`:2351-2707`); `indirect_results::{entry,prepare_call,finish_call,returned}` (`:2766-2854`); `record::{establish,initialize_storage,write_fields,chunk,store,write_structural_operand}` (`:2959-3191`); `reference::{establish,load,load_pointer_word,load_scalar_referent,borrowed_argument}` (`:3226-3450`); `ieee_float_classify::establish` (`:3507`); `scalar_array::establish` (`:3597`); `scalar_case::establish` (`:3666`); `structural_array::establish` (`:3743`); `structural_local::establish` (`:3799`); `zero_fill::whole_home` (`:3827`); `zeroed::establish` (`:3871`); `encoded_scalar::{backing_address,constant_codec,read,write}` (`:3947-4054`); `fragmented_scalar` (`StorageAccess` `:4067`; `constant`, `binary`, `shift`, `mask`, `load_container`, `read`, `read_register`, `write`, `write_register`, `store_container`; `:4129-4525`); `runtime_address::{projection_base,projection_origin,scale,scale_at_site,footprint,footprint_at_origin,cross_references,scalar_reference}` and trait `PreparedPointerChase` (`:4541-4943`); `scalar_store::emit` (`:4956`); `subslice::create` (`:5227`).
   - Top level: struct `Transport` (`:5372`, fields `result_pointer`, `pointers`, `views`, `element_views`, `fragments`, `slots`, `local_slots`, `calls`, `normalized_foreign_calls`, `memory`, `settlements`) with `push_structural_local_slot`, `view`, `element_view` (`:5386-5410`); `invalid` (`:5414`); `transport_register` (`:5418`); `scaled_element_index` (`:5432`); `place_pointer`, `place_pointer_at_site` (`:5475,5484`); `view_element_pointer` (`:5541`); macro `dispatch_kind!` (`:5560`); `operation` (`:5569`); `provenance`, `operation_provenance`, `obligation_provenance` (`:5782-5811`); `memory_pointer`, `displaced_memory_pointer`, `memory`, `memory_at_origin`, `transport_at_byte_offset` (`:5812-5918`); `storage_address` (`:5920`).
3. Inputs and outputs: reads `NativeFunction` geometry through `crate::instruction_selection::selection::*_input` helper modules and `structural_inputs::structural_reference_input` (names only; those files are outside this unit) and the `Transport` tables. Pushes local slots, `(place, pointer-register)` pairs, fragment registers, and `SelectedMemoryAccess` records through `memory_at_origin` (`:5867-5890`).
4. How it decides:
   - Place lookup: linear `.iter().find(|(place,_)| *place == X)` over `transport.pointers` (`:5490-5495` and many others), keyed on `PlaceId`.
   - Home choice in `place_pointer_at_site`: the incoming pointer if present, else exactly one local slot whose `structural_place()` equals the place; two slots give `invalid()` (`:5496-5514`).
   - Memory-origin kind from provenance: `([op], [])` gives operation origin, `([], [edge])` gives edge origin, anything else `invalid()` (`:5516-5525`).
   - Pointer displacement is folded into the access only if `environment.admits_pointer_displacement(byte_offset, byte_size)`, else an `AddressOffset` is emitted (`:5838-5846`).
   - Zero-fill: inline stores up to `zero_fill::INLINE_HOME_MAX_BYTES`, else `ZeroBytes` (`:3008-3010`, `:3879`).
   - Read/store of a scalar: codec comparison `codec != canonical || matches!(codec.byte_size(), 3 | 5 | 6 | 7)` selects the encoded path (`:2491`, `:3088`, `:5163`).
   - Root view versus pointer chase in `projection_origin` keyed on `view_type` / `element_view_type` of the place (`:4620-4637`).
   - Atomic compare-exchange outcome: layout compared with `conventional_atomic_once_layout_closed(...)` (`:487-492`).
5. Cases by shape:
   - Entry owned parameter needs an indirect pointer or inline fragments (`:154-159`); the pointer register case accepts one `Indirect{Register}` location or one 8-byte `Register` for shared, mutable or write-only borrows (`:272-295`).
   - Descriptor table operations are x86-64 only: messages "requires an x86-64 target" (`:2161-2163,2236-2238,2287-2289,2299-2301`).
   - Activation claim: `cases.len() != 2`, `cases[0].fields.len() != 1`, `!cases[1].fields.is_empty()` refused (`:1764-1773`).
   - `fragmented_scalar::container_width`: 8..=64 bits and a multiple of 8, else `invalid()` (`:4207-4222`); `load_container` refuses `width > 8` (`:4253-4257`).
   - `fragmented_scalar::shift` refuses `count >= 64` (`:4181-4183`).
   - `encoded_scalar::read/write` accept Callable, Integer, Boolean, IeeeFloat only (`:3986-3993`, `:4026-4034`).
   - Fixed-width arrays use `chunk()` of 8, 4, 2, 1 byte stores chosen by alignment (`:3143-3152`).
   - `descriptor_operands::backing`: only borrowed byte carriers, element views, bounded-owned byte sequences and fixed byte-extent arrays; the `_ =>` arm is `invalid()` (`:755-808`).
   - `scalar_reference` loads exactly 16 bytes (address and codec) (`:4912-4941`).
   - `scale_at_site`: index bit width up to 64; 8, 16, 32-bit indices are extended with a per-sign kind, `_ => invalid()` (`:4675-4689`).
   - `dispatch_kind!` dispatch in `operation`: `ReleaseReference`, `DecomposeStructuralValue`, `EstablishTrivialAffineLocal` emit nothing and require no result (`:5717-5727`).
6. Same job elsewhere:
   - Three store helpers: `record::store` (`:3155`), `scalar_case::store` (`:3705`), `fragmented_scalar::store_container` (`:4484`), plus `local_storage::store` (`:1652`).
   - Zero-fill pattern (`count` register, cursor scratch, `ZeroBytes`): `zero_fill::whole_home` (`:3827-3859`) and `activation_storage::emit` (`:1801-1821`, `:1903-1924`).
   - Descriptor presentation (store pointer and length into a 16-byte slot, take its address): `local_storage::descriptor` (`:1553`), `subslice::create` (`:5347-5361`), `VB element_subslice::create` (`VB:2223-2248`), `VB element_establishment::establish` (`VB:2021-2026`), `literals::establish` (`:1460-1487`).
   - Subslice creation is written twice: `ST subslice::create` (`:5227`) for bytes and `VB element_subslice::create` (`VB:2077`) for elements. Both compute `byte_offset`, `root_length`, `ExactAddI64`/`ExactSubtractI64` under the same obligation and push to `transport.views` or `transport.element_views`.
   - `place_pointer` (`:5475`) versus `reference::borrowed_argument` (`:3450`) versus `SG call_arguments::referent_pointer` (`SG:2219`) all derive a pointer to a place.
7. Unused or one-caller code: not individually grepped. `scaled_element_index` is used from `VB element_view_write` (`VB:2327`) and `VB element_views` (`VB:2469`) (read). The macro `dispatch_kind!` is used only in `operation` (read).
8. Stated limits: "A whole descriptor store writes its own header. Only explicit backing projections may load and follow the descriptor's pointer." (`:1140-1141`); "Setting the limit bit caps the least-set-bit result at the declared region alignment" (`:2584-2585`); "The index value's closed type is the plan's; the operand row must agree with it." (`:4671`).
9. Fail-open risks:
   - `operation` returns `Ok(false)` ("not mine") when `row.result.is_some()` and no earlier `dispatch_kind!` matched (`:5766-5768`); the caller then continues to the scalar `match`. Traced: `SG:7489-7499` continues and the later `match` returns `invalid()` for structural kinds listed at `SG:7651-7696`, so a structural kind not matched by any `dispatch_kind!` and not in that list would reach `Call(_)` or fall through. Status: traced for the listed kinds; kinds missing from both lists would be a compile error (the `match` in `SG` is exhaustive without `_`).
   - `fragmented_scalar::load_container` and `store_container` record footprints through `StorageAccess`; `Backing` footprint role is not tied to a place (`:4112-4124`). Status: open.
   - None seen of the "returns true on missing row" kind.

## 6. `S05/instruction_selection/selection_views_and_bytes.rs` (2,592 lines)

1. Purpose: selection of byte-sequence and element-view operations: concatenation, byte and extent copy, field reads and stores, view field stores, byte reads, writes, predicates and equality, element-view establishment, subslice, element reads, writes and lengths.
2. Shape (modules): `byte_concatenation::{establish,operand_pointer,operand_span,copy}` (`VB:23,128,167,202`); `byte_copy::{CopyEnd,copy,address,access}` (`:268,275,378,391`); `byte_field_read::{read_byte,payload_pointer}` (`:430,496`); `byte_field_store::{replace,replace_byte,scratch}` (`:552,811,914`); `byte_view_field_store::replace` (`:940`); `byte_views::{write,byte_observation,byte_sequence_read,charge_backing_roots,unsliced_payload_offset,backing_index,backing_pointer,byte_sequence_length,byte_predicate,byte_equal,measured_span_reads}` (`:1093-1559`); `element_establishment::{establish,constant,local_home_root}` (`:1582,2030,2049`); `element_subslice::create` (`:2077`); `element_view_write::write` (`:2267`); `element_views::{element_observation,element_view_read,element_view_length,element_backing_pointer}` (`:2385,2412,2525,2565`).
3. Inputs and outputs: reads `NativeFunction`, `Transport.views: Vec<ByteViewHomes>` and `Transport.element_views`, `byte_view_homes::view_backing_roots`, `FieldBytes::resolve`, `established_view_input::*` (outside unit). Emits `CopyBytes`, `Load8Indexed`, `ScanByteSequence`, `CompareByteSequences`, `ExactAddI64`, `ExactSubtractI64`, `ExactDivideU64`, stores and descriptor slots; records `ReadByteSpan` / `WriteByteSpan` / `ReadElementView` etc. memory roles.
4. How it decides: type checks on `ClosedScalarType::Integer(unsigned 64)` for indices, lengths, counts (e.g. `:55,458-466,865-872,1241-1251,1462-1468`); `u8` for bytes (`:460-465,867-871,1241-1245`); source kind via `readable_byte_type`, `view_type`, `element_view_type`; `byte_copy::copy` switches between one `CopyBytes` and unrolled loads and stores at `extent_copy::INLINE_MAX_BYTES` (`:283`, `:320`); X86 adds `division_scratch` for `ExactDivideU64` keyed on `environment.target().architecture == X86_64` (`:2006-2008`).
5. Cases by shape:
   - `byte_sequence_length`: `length_byte_offset` must be 0 or 8 (`:1411`); `byte_observation` expects 0 for `BoundedOwned` and 8 otherwise (`:1201-1210`); bounded-view carriers return a constant capacity (`:1187-1200`).
   - `element_view_length`: `length_byte_offset != 8` refused (`:2531`).
   - `element_establishment::establish` admits `ElementView` shapes with equal element, or a borrowed byte carrier whose element is unsigned 8-bit (`:1641-1663`); regions `WholeCollection`, `WholeRepresentation`, `RepresentationBytes` handled in a three-arm match (`:1882-2020`); `CollectionExtent::Parameter` refused (`:1888`).
   - `byte_field_store::replace` requires the destination access in `Owned | MutableBorrow | WriteOnlyBorrow` (`:585-592`).
   - `element_subslice::create` requires both shapes be `ElementView` with the same element (`:2106-2109`).
6. Same job elsewhere: `byte_concatenation::copy` (`:202`) and `byte_field_store::replace` (`:784-793`) both emit `CopyBytes` with cursor and byte scratch; `byte_copy::copy` (`:275`) emits `CopyBytes` for whole extents (`:313-318`). `byte_field_store::scratch` (`:914`) wraps `SG integer_arithmetic::instruction_scratch`. Element subslice versus byte subslice (see section 5).
7. Unused: not grepped individually. `byte_concatenation::operand_pointer` is used in `byte_views::byte_predicate` and `byte_equal` (`:1469,1511-1512`) (read).
8. Stated limits: none found by reading.
9. Fail-open: none seen. `charge_backing_roots` falls back to a single `sequence` memory role when `view_backing_roots` returns `None` (`:1316-1323`); traced: `None` means no view roots exist for the place, so the access is charged against the place itself. Status: traced from the code, callers not followed.

## 7. `A06/allocator_availability.rs` (196 lines)

1. Purpose: turns an `AllocatorAvailabilityPolicy` into the set of physical register views the allocator may use without constraint, per register class, and seals it.
2. Shape: `materialize_allocator_availability` (`:122`); `compute::compute_terminal_allocator_availability` (`:30`); `admitted_views` (`:77`); `ValidatedAllocatorAvailability` with `unconstrained_views(class)` (`:173-180`); `AllocatorAvailabilityError` (`:184-191`); macros `seal_plan!`, `validated_plan_carrier!`, `failed_display!`, `receipt_accessor_rows!` (defined in `A06/lib.rs`, outside unit).
3. Inputs and outputs: reads `NativeTarget`, `ValidatedPhysicalRegisterModel`, constraint catalog, `ValidatedRegisterReservationProfile`, selected constraint keys, policy; produces `AllocatorAvailabilityPlan { register_environment, physical, policy, classes }` then `ValidatedAllocatorAvailability`.
4. How it decides: refuses (`RootMismatch`) when architecture, constraint/reservation physical identity, reservation target or the recomputed `target_register_environment_identity` differ (`:39-53`). A view is admitted when `view.allocatable` and none of its `units`/`write_units` is reserved (`:82-89`), keyed on unit identity. Allowlist policy: sorted-unique check, view exists, view environment-allocatable (`:98-113`).
5. Cases by shape: two policies only: `AllEnvironmentAllocatableViewsV1`, `ExplicitUnconstrainedViewAllowlistV1` (`:91,98`).
6. Same job elsewhere: the reserved-unit test appears in `A06/allocation_legality.rs` `view_candidates::CandidateViews::new` (`:635-646`) and `conflicts` (`:930-933`).
7. Unused code (grepped, lines/files): `materialize_allocator_availability` 7 lines in `allocator_availability.rs`, `legality.rs`, `lib.rs`; `AllEnvironmentAllocatableViews` 4; `ExplicitUnconstrainedViewAllowlistV1` 7 (also `constraints.rs`).
8. Stated limits: none.
9. Fail-open: none seen. `unconstrained_views(class)` returns `Option`; its caller `allocation_legality.rs:398-404` maps `None` to `UnknownClass`, but `CandidateViews::new` uses `.unwrap_or(&[][..])` (`:624-626`), which treats a class absent from the availability as having no available views, i.e. refuses candidates. Status: traced, fails closed.

## 8. `A06/fixed_precolored_intervals.rs` (361 lines)

1. Purpose: converts each fixed-register-view constraint on a virtual register into a half-open one-point interval `[start, start+1)`, checking it against the legality analysis.
2. Shape: `analyze_fixed_precolored_intervals` (`:269`); `compute::compute` (`:23`), `derive_function` (`:79`), `reject_early_clobber_fixed` (`:143`), `resolve_point` (`:175`), `require_view` (`:208`), `interval_key` (`:238`); `FixedPrecoloredIntervalError` (`:323-356`); receipt `FixedPrecoloredIntervalValidationReceipt` (`:296`).
3. Inputs and outputs: reads `ValidatedLiveRanges`, `ValidatedAllocationLegality`, policy, budget; outputs `FixedPrecoloredIntervalPlan` sorted by `interval_key`.
4. How it decides: `require_view` demands that exactly one legality span covers the point and its candidate list equals exactly `[view]` (`:216-235`); `resolve_point` demands exactly one fragment covers the point (`:192-204`); an early-clobbered def with a fixed view is refused (`:158-171`). Keyed on block id and point number.
5. Cases by shape: only `Def` operand constraints that are early-clobbered are refused; other `Def`/`Use`/`Entry` sites pass (`:149-157`).
6. Same job elsewhere: fixed-view-at-point lookup also in `allocation_legality.rs` `fixed_views::at_live_point` (`:119-143`).
7. Unused code (grepped): `analyze_fixed_precolored_intervals` 5 lines in `fixed_precolored_intervals.rs`, `lib.rs`, `rewrites.rs` (stage 07; one consumer).
8. Stated limits: error variant `UnsupportedEarlyClobberFixedConstraint` (`:342`).
9. Fail-open: none seen. `reject_early_clobber_fixed` returns `Ok(())` for sites that are not `Def` operands (`:149-157`); that is by design (only a `Def` can be an early-clobbered result). Status: traced from the code.

## 9. `A06/legality.rs` (352 lines)

1. Purpose: staged custody around the allocation-legality analysis: choose which register views allocation may use, compute or admit the legality, and wrap it with a receipt.
2. Shape: `AllocatorAvailabilityRequirement` (`:211`, three variants); `stage_optimized_allocation_legality` (`:238`); `admit_optimized_allocation_legality` (`:249`); `stage_optimized_allocation_legality_with_availability` (`:272`); `StagedOptimizedAllocationLegality` (`:280`) with `for_availability` (`:295`); `StagedOptimizedAllocationLegalityCustodyReceipt` (`:318`); `OptimizedAllocationLegalityCustodyError` (`:340`); private modules `compute`, `custody`, `policies` (`:14,50,73`).
3. Inputs and outputs: reads `StagedOptimizedLiveRanges` and a requirement; produces `StagedOptimizedAllocationLegality` holding ranges, availability, legality, custody receipt.
4. How it decides: `admit_optimized_allocation_legality` accepts a legality computed elsewhere only if its receipt's `ranges`, `register_environment`, `allocator_availability` identities equal the staged ones (`:254-261`), keyed on identity. `FramelessLeafCallerSavedViews` selects views all of whose `units`/`write_units` are in the target's caller-saved set (`:114-133`). `ActiveResident...` selects named views: `["rax","rcx"]` on x86-64, `["x0","x1"]` on aarch64, looked up by name (`:170-186`); missing name gives an error (`:180-184`). `for_availability` reuses the staged legality only if the new availability has the same identity (`:299-303`).
5. Cases by shape: exactly two architectures (`X86_64`, `Aarch64`) in `policies` (`:103-112`, `:170-173`); the rematerialization requirement hard-codes register names "rax", "rcx", "x0", "x1" (source spelling, `:170-173`).
6. Same job elsewhere: none seen.
7. Unused code (grepped): `admit_optimized_allocation_legality` 6 lines in `legality.rs`, `lib.rs`, `selected_optimization.rs`; `stage_optimized_allocation_legality` 8 lines in `allocation_input.rs`, `legality.rs`, `lib.rs`, `selected_optimization.rs`; `FramelessLeafCallerSavedViews` 3 lines in `legality.rs`, `selected_optimization.rs`; `ActiveResidentImmediateU64MultiUseRematerializationV1` 11 lines in `cases.rs`, `legality.rs`, `lib.rs`, `publication_validation.rs`.
8. Stated limits: "Reuse legality only for the same available views." (`:292`) and "Nothing is recomputed" (`:247`).
9. Fail-open: none seen. `admit_optimized_allocation_legality` accepts a legality without recomputing it when identities agree (`:246-247`, stated: "Nothing is recomputed; the analysis must name these exact live ranges"); status: stated.

## 10. `A06/allocation_legality.rs` (1,301 lines)

1. Purpose: for every virtual register, for every live-range fragment, computes which physical register views are legal at each point, compressed into constant "spans", keeping boundaries at fixed-view, early-clobber, tied and architectural-unit changes. This is the sole input to stage 07's choice of homes (inferred from crate map).
2. Shape: private modules in `compute` (`:17`): `early_clobbers::compute` (`:29`), `fixed_views::{for_early_clobber,at_live_point,entry_transitions,reject_ambiguous}` (`:92-183`), `function::{first_unit_diff_point,unchanged_point_limit,compute_reusing}` (`:209,254,332`), `live_points::compute` (`:456`), `view_candidates::{CellCandidates,CandidateViews,unit_active_at_point,unit_active_in_block_at_point,unconstrained,fixed_candidate,conflicts}` (`:584-942`); `compute_terminal_allocation_legality` (`:959`); `validate_roots` (`:1045`); `validate::{FunctionCounts,admit}` (`:1067-1179`); public `analyze_allocation_legality` (`:1187`), `analyze_allocation_legality_reusing` (`:1209`), receipt `AllocationLegalityValidationReceipt` (`:1240`), `ValidatedAllocationLegality` via `shared_plan_carrier!` (`:1252`), `AllocationLegalityError` (`:1259-1296`).
3. Inputs and outputs: reads `ValidatedLiveRanges`, `ValidatedAllocatorAvailability`, physical model, constraint catalog, reservation profile, selected keys, optionally a previous (ranges, legality) pair. Produces `AllocationLegalityPlan { ranges, register_environment, allocator_availability, functions: Vec<Arc<FunctionAllocationLegality>> }` plus a `carried: Vec<bool>` of reused functions, then `admit` computes digests, counts, identity.
4. How it decides:
   - Candidate views at a point: every allocatable, non-reserved, available view whose units are not active in that block at that point (mask test `(mask & active_mask) == 0`, `:798-803`), keyed on architectural unit identity, block id and point number.
   - Fixed view at a point: collect fixed constraints whose site point equals the point (or `Entry` at the first fragment start); two distinct views give `IllegalFixedView` (`:183-196`); the single view must exist, match the register class, and not conflict (`:889-921`).
   - Empty candidate list gives `NoCandidateViews` (`:539-546`, `:59-66`).
   - Reuse: a function's legality is copied from the previous run when its `FunctionLiveRanges` are equal (`:1006-1012`); per register when fragments and fixed constraints are equal and the register's last fragment ends at or before `unchanged_point_limit` (`:375-387`); reuse is disabled when either side has early clobbers (`:344-346`).
   - `previous` is dropped unless target, optimization unit, work schedule, and four receipt identities all agree (`:987-998`).
5. Cases by shape:
   - Mask path only when a class has at most 64 architectural units: "if arch_units.len() <= 64" (`:669`); otherwise the slower `unconstrained` scan (`:805-815`).
   - Entry transitions only when a register has two or more fixed constraints and two or more distinct pinned views; only `Use` operand sites produce transitions (`:148-167`).
   - A `Def` fixed constraint paired with an early clobber is resolved by `for_early_clobber` using `position`, `instruction`, `operand` equality (`:97-115`).
6. Same job elsewhere: interval/point resolution against fragments is repeated in `fixed_precolored_intervals.rs` `resolve_point` (`:175-206`) and `require_view` (`:208-236`) (read). Reserved-unit exclusion duplicates `allocator_availability.rs:82-89`.
7. Unused code (grepped): `analyze_allocation_legality_reusing` 2 lines, only the definition (`allocation_legality.rs`) and a re-export (`lib.rs`); no caller found in `source-rust/` or `tools/` outside `_recycling`. `analyze_allocation_legality` 14 lines across `allocation_legality.rs`, `legality.rs`, `lib.rs`, `literal_fold_rewrites.rs`, `mod.rs`, `reanalysis.rs`, `rewrites.rs`. `compute_function_reusing` is `pub(crate)` re-export at `:945`.
8. Stated limits: none by comment. Raw-pointer comparison `last_candidates_ptr: Option<(*const RegisterViewId, usize)>` (`:489`) is used to skip a content compare when the same cached slice is returned.
9. Fail-open risks:
   - `live_points::compute` merges a new span into the previous one when the candidate slice pointer and length match (`:547-554`); the pointer compare is a shortcut, content compare follows with `||`. Status: traced; both branches require equal content or identical cached storage.
   - `conflicts` returns `false` (no conflict) when a unit is not found in `function.architectural_units` (`let Ok(unit_idx) = ... else { return false }`, `:934-938`). Meaning: a unit never recorded in the live-range analysis is treated as free. Traced: units come from the same `FunctionLiveRanges`; a unit absent from the list is one that no instruction touches. Status: traced to the producer's contract, producer not read (analysis outside unit).
   - `compute_reusing` reuse test compares `fragments` and `fixed_constraints` but not `copy_affinities`, `tied_pairs` or edge connectors (`:383-384`). Reuse is skipped when early clobbers exist (`:344-346`) but not when tied pairs differ; the previous legality would be reused for a register whose tied status changed while its fragments did not. Status: open.
   - `candidates_at_cell` for `AllBaseViews` returns the cached base list when no architectural unit is active at the point (`:766-771`); when no unit has been recorded for the class (`arch_units` empty) every point returns all base views. Status: traced; same producer contract as above.

## 11. `A06/fixed_precolored_split_requirements.rs` (922 lines)

1. Purpose: decides where a virtual register's live range must be cut because two fixed-use views are incompatible, and records the resulting "source segments" with the candidate views each segment may use.
2. Shape: `analyze_fixed_precolored_split_requirements` (`:822`); `compute::cuts::CutIndex` (`:26`; `new`, `boundary`, `require_transition`, `use_transition`, `finish`); `compute::function::derive` (`:209`); `compute::partition::{register,fragment,point_domain,push_segment}` (`:302,407,488,500`); `compute::topology::{FragmentInput,tree}` (`:546,552`); `compute::compute` (`:663`); `site_key`, `site_point`, `early_clobber_uses` (`:885,905,912`); `FixedPrecoloredSplitRequirementError` (`:748-801`).
3. Inputs and outputs: reads ranges, legality, fixed intervals, policy `FixedUseBoundaryRequirementsV1`, budget; produces `FixedPrecoloredSplitRequirementPlan` with per-register, per-fragment `segments { id, start, end, candidates, opening }`, where `opening` is `SourceRangeStartV1`, `IncomingSourceEdgeV1{connector}` or `IncompatibleFixedUseDomainBoundaryV1{incoming, site, destination_view}`.
4. How it decides: within a fragment, walks legality spans; keeps the intersection of candidates; when the intersection becomes empty it requires a fixed `Use` interval starting exactly at that span (`CutIndex::boundary`, `:58-122`) whose candidates equal `[row.view]`, then records a cut (`:443-465`). For an incoming edge whose parent closing domain does not intersect the child's first domain it requires a declared transition `from_view -> (site, to_view)` (`:354-373`, `use_transition` `:145-167`). `use_transition` fails if zero or two source views match (`:159-164`). All keyed on block id, point number, `site_key` tuple and view id.
5. Cases by shape:
   - Tied registers refused: `UnsupportedTiedRegister` (`:325-331`).
   - Cross-block shapes: "tree" topology only. A fragment's connector must come from an already admitted fragment and each target block may have one connector (`:566-576`, `:611-634`); otherwise `UnsupportedCrossBlockRange`.
   - A cut site must be a `Use` operand (`:102-111`, `UnsupportedFixedTransitionAccess`).
   - Exactly one fixed interval at a cut point (`:77-85`, `AmbiguousFixedCutSite`).
6. Same job elsewhere: span walking over fragments also in `allocation_legality.rs::live_points::compute` (`:484-566`) and `topology::tree` re-validates span canonical order (`:590-609`).
7. Unused code (grepped): `analyze_fixed_precolored_split_requirements` 5 lines in `fixed_precolored_split_requirements.rs`, `lib.rs`, `rewrites.rs`.
8. Stated limits (error names, read): `UnsupportedCrossBlockRange`, `UnsupportedTiedRegister`, `UnsupportedFixedTransitionAccess` (`:757-788`).
9. Fail-open risks:
   - `CutIndex::use_transition` is called without checking its result at `:455` (`cuts.use_transition(&prior, site, destination_view);`). Its `bool` is discarded, so a cut that has no declared transition does not fail there. Traced: `cuts.finish` (`:169-189`) only checks that every recorded `used` entry is in the declared set, and `use_transition` records `used` only when it found exactly one matching declared transition (`:165`). A cut with no declared transition is therefore neither recorded nor refused in `fragment`; the `incoming` path (`:358-364`) calls `require_transition` and does refuse. Status: open; no reason is stated at the site.
   - `finish` is checked after the loop with `exhausted` testing that used rows exist in declared rows (`:174-178`); it does not check that every declared transition was used (the name "exhausted" suggests it might). Status: open.

## 12. `A06/fixed_precolored_segment_homes.rs` (1,277 lines)

1. Purpose: gives each "home domain" (a set of source segments of one virtual register joined across block edges) one physical register view, deterministically, avoiding views that overlap with interfering domains and preferring copy-affinity partners.
2. Shape: `assign_fixed_precolored_segment_homes` (`:1111`); private modules `compute::conflicts` (`Conflicts`, `build`, `live_overlap`, `registers_interfere`, `checked_view`; `:13-197`), `compute::functions::derive` (`:212`), `compute::placement::{assign,select,preferred_view,assigned_partner_views,stolen_coalesces,partner_outlooks,residual_base,strands_neighbor,propagate_forced,clique_bound_violated}` (`:255-725`), `compute::roots::validate` (`:744`), `compute::compute` (`:802`); `domains::{Segment,Domain,build,new_domain,intersect}` (`:860-1033`); `error::FixedPrecoloredSegmentHomeError` (`:1039`); `affinity_edges`, `occurrence_point`, `containing_domain` (`:1193,1251,1265`).
3. Inputs and outputs: reads ranges, legality, fixed intervals, split requirements, register environment pieces, policy `MostConstrainedLowestCompatibleViewV1`, budget. Produces `FixedPrecoloredSegmentHomePlan { functions: Vec<FunctionFixedPrecoloredSegmentHomes{assignments}> }`, one `FixedPrecoloredSourceSegmentHome` per segment, sorted by (virtual register, segment).
4. How it decides:
   - Domain building: `SourceRangeStartV1` and `IncompatibleFixedUseDomainBoundaryV1` open a new domain; `IncomingSourceEdgeV1{connector}` joins the domain that closed in the connector's source block, intersecting candidates (`:930-966`).
   - Two domains conflict when they overlap in time in the same block (`live_overlap`, `:130-136`) and their registers appear in `ranges.interference` (binary search, `:151-153`); then each pair of candidate views conflicts when `footprints_overlap` (`:108`).
   - Order: the domain with fewest viable views, then highest conflict degree, then first point, register id, segment id is assigned first (`select`, `:342-385`).
   - View: ranked by (guaranteed affinity matches, votes, fewest stolen coalesces); ties go to the first view in the pool (`preferred_view`, `:388-481`). Pruning by forced-propagation and a clique bound (`:604-725`).
   - No viable view gives `SegmentPressure` (`:287-294`).
5. Cases by shape: one policy value only (`match policy { ...V1 => {} }`, `:826-828`). Clique bound test is a heuristic: "if clique.len() > pool.len() return true" (`:718-720`).
6. Same job elsewhere: view conflict by footprint overlap in `view_candidates::conflicts` (`allocation_legality.rs:923-942`) uses architectural units instead; the two compute "can these two views be live together" two different ways (read both).
7. Unused code (grepped): `assign_fixed_precolored_segment_homes` 5 lines in `fixed_precolored_segment_homes.rs`, `lib.rs`, `rewrites.rs`.
8. Stated limits: none by comment.
9. Fail-open risks:
   - `affinity_edges` skips (`continue`) any copy affinity whose registers, occurrence points or containing domains are missing (`:1213-1233`). Effect: a missing lookup drops a coalescing preference, not a correctness constraint. Status: traced (affinities only rank views; conflicts are built separately).
   - `Domain::first_point` and `first_segment` use `.expect("domain owns a source segment")` (`:898,906`), and `select` uses `.expect("nonempty unassigned domain roster")` (`:384`). A domain with no segments panics instead of refusing. Reached only if `domains::build` produced an empty domain; `new_domain` pushes a segment immediately after (`:967-972`). Status: traced.
   - `conflicts::build` is quadratic over domain pairs (`:89-122`); budget limited through `Work` counters, not a cap. Status: stated by use of `work.*` calls.

## Coverage

Read in full (every line, in order):
- `S05/select_instructions.rs` 1-59 (59)
- `S05/selected.rs` 1-130 (130)
- `S05/test_support.rs` 1-22 (22)
- `S05/instruction_selection/selection_scalar_graph.rs` 1-8260 (8,260)
- `S05/instruction_selection/selection_structural.rs` 1-5926 (5,926)
- `S05/instruction_selection/selection_views_and_bytes.rs` 1-2592 (2,592)
- `A06/allocation_legality.rs` 1-1301 (1,301)
- `A06/allocator_availability.rs` 1-196 (196)
- `A06/fixed_precolored_intervals.rs` 1-361 (361)
- `A06/fixed_precolored_segment_homes.rs` 1-1277 (1,277)
- `A06/fixed_precolored_split_requirements.rs` 1-922 (922)
- `A06/legality.rs` 1-352 (352)
Total read in full: 21,998 lines. Skimmed: 0. Not read: none.

Not read, but cited by name only: the sibling modules `selection_access_lifecycles.rs`, `selection_interrupts.rs`, `selection.rs`, the `*_input` helper modules under `S05/instruction_selection/selection/`, `A06/lib.rs` (macro definitions), `rewrites.rs` and `selected_optimization.rs` in stage 06. Their behavior is inferred from names.

## Five things a maintainer most needs to know about this unit

1. `selection_scalar_graph.rs` and `selection_structural.rs` are files made of nested `mod` blocks that call each other through `super::` and long `crate::instruction_selection::selection::construction::scalar_graph::structural::...` paths. The two files are one mutual-dependency cluster (`SG:6456` re-exports the structural file as `structural`; `ST:14-22` imports `Builder` and `call_arguments` back from the scalar graph).
2. Almost every refusal is `SelectedInstructionError::custody()` with no message (counted in reading: hundreds of `invalid()` returns). Only a few carry text via `custody_because` (e.g. `SG:7647`, `ST:1194`, `ST:4160`). A failed selection usually names a function and operation, not which condition failed.
3. Per-function state is held in `Vec`s scanned linearly: `definitions` (`SG:7926`), `transport.pointers` (`ST:5490`), `transport.local_slots`, `transport.views`. Cost grows with the number of values and places in a function.
4. Several helpers exist in four to six copies (scratch registers, outgoing slot registration, float transfer copy, integer carrier normalization, comparison materialization, descriptor presentation, subslice creation); see items 6 in sections 4 and 5. A fix to one copy does not reach the others.
5. In stage 06 analyses, `analyze_allocation_legality_reusing` has no caller (grepped 2 lines: definition and re-export), and `fixed_precolored_split_requirements.rs:455` discards the result of `use_transition` (no check at that site).

## What I am unsure about

- Whether the helper modules under `S05/instruction_selection/selection/` (`reference_input`, `aggregate_result_input`, `scalar_call_abi`, `value_transport` and others) perform the checks that the emitters assume (for example that geometry matches runtime indices). I read only the call sites.
- Whether the discarded `use_transition` result at `fixed_precolored_split_requirements.rs:455` can let a cut with no declared transition through; I traced `finish` but not the producer of `entry_transitions` against every shape.
- Whether the first-block-wins behavior of `block_containing_operation` (`SG:7195`) matters, i.e. whether an `OperationId` can appear in two source blocks.
- The count of "hundreds of `invalid()` returns" is from reading, not from a grep count.
- I did not grep every `pub(crate)` function in `ST` and `VB` for call sites; section 5 and 6 item 7 are partial.
