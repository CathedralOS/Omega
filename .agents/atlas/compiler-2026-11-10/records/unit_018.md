# Unit 18 notes

Files covered (all under `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/`):

1. `legalization_target.rs` (6973 lines)
2. `lib.rs` (724)
3. `optimized.rs` (298)
4. `selection.rs` (1736)
5. `selection_access_lifecycles.rs` (6844)
6. `selection_call_abi.rs` (2055)
7. `selection_edge_transfers.rs` (1645)

Short names below are relative to that `src/` directory. "read" = I read the lines; "grepped" = counted with the Grep tool across `omega-rust`; "inferred" = not read at the site.

---------------------------------------------------------------------------------------------------

## 1. `lib.rs` (724 lines)

**Purpose.** Crate root of "Omega 03": declares the module list, re-exports, and holds three small in-file modules for installed-interrupt (x86 IDT/GDT/TSS) geometry and helper-association checking. The doc comment (lib.rs:3-32) describes the crate as legalization (`legalization*.rs`) then selection (`selection*.rs`) plus shared `structural_inputs.rs`.

**Shape.**
- `installed_interrupt_geometry` (lib.rs:44-217): `InstalledInterruptOperandLayout` (12 u32/u64 fields, lib.rs:53-66) and `derive(declarations, types: [StructuralTypeId; 6])` (lib.rs:104).
- `installed_interrupts` (lib.rs:218-614): `Rows<T>` (sorted `(OperationId, Vec<T>)`, lib.rs:227-250), `Associations<T>` (sorted by `(OperationId, u32)`, `unique()` rejects duplicates, lib.rs:252-268), `PreparedInstallationSources` (lib.rs:270), `PreparedHelperCohort` (lib.rs:321, precomputes a `requirements` answer per helper operation at construction, lib.rs:340-351), `pub struct PreparedInstalledInterruptHelpers` (lib.rs:453; `new` lib.rs:459, `verify` lib.rs:559), `validate_target` (lib.rs:571).
- `interrupt_installation::validate_target` (lib.rs:616-685).
- Module list lib.rs:687-704 (17 `mod` lines); re-exports lib.rs:706-724.
- `failed_display!` macro (lib.rs:34-42) used by the error enums in `optimized.rs` and `selection.rs`.

**Inputs/outputs.** `derive` reads the structural type declarations and the target-layout helpers `structural_inputs::structural_reference_input::{shape, project, primitive_geometry, case_layout, align}`; outputs byte offsets and fixed tags. `PreparedInstalledInterruptHelpers::new` reads the `TerminalModule` (operations, `installed_interrupt_registrations`, boundary machines) and an `AdmittedInstalledInterruptCohort`; outputs a per-operation `Result<(), &'static str>` table. `validate_target` reads the `TargetOperationPlan`, the module, the cohort and the abstract plan and returns `LegalizationError::InstalledInterruptAssociation`.

**How it decides.**
- Geometry is keyed on byte sizes of the six types: table size must be non-zero, <= 4096 and a multiple of 16; GDT 16..=65536 and a multiple of 8; TSS exactly 104 bytes (lib.rs:114-125). Arrival frame must be a sum with exactly 3 empty cases, tag at byte 0, tag width equal to `runtime_abi::CASE_TAG_BYTE_SIZE` (lib.rs:173-194). The tags are hard-coded `no_error_tag: 0, error_tag: 1, opaque_fatal_tag: 2` (lib.rs:211-213) and are not read from the declared case ids.
- Helper association is keyed on `(installation OperationId, callback application_index)` identity (lib.rs:393-396), `kind` equality (lib.rs:423), `TerminalPsiIdentity` equality (lib.rs:564, 581), and boundary id / argument equality (lib.rs:521-523, 605-607). No string comparison.

**Cases by shape.** Stack lease must be a record with exactly two non-erased runtime fields, first a mutable-borrow element view of u64 words (lib.rs:83-88, 131-144); arrival rows likewise two fields (`fields()` lib.rs:74-89). Table/GDT/TSS sizes as above. Install helper `kind == Install` is rejected as "not an arrival token helper" (lib.rs:410-412, 440-442).

**Same job elsewhere.** `Rows`/`Associations` are local sorted-vector indexes (lib.rs:227-268). `InstalledInterruptOperandLayout` is used by `selection_interrupts.rs` (1 hit) and `backend/images/image-emission/src/function_fragments.rs` (3 hits) (grepped `InstalledInterruptOperandLayout`: 7 total incl. lib.rs 3), so the layout is computed in two stages (suspected duplicate derivation, other site not read).

**Unused / one-caller.** `PreparedInstalledInterruptHelpers` (pub): 6 hits total: lib.rs 4, `image-emission/src/function_fragments.rs` 2 (grepped). `verify_target_operation_plan`: 1 external call site `image-emission/src/function_fragments.rs:3878` (grepped; others are the definition `legalization.rs:466`, README, lib.rs). `stage_optimized_instruction_selection`: external callers `native-compiler/src/pipeline.rs` (2 hits) and `pipeline_boundary.rs` (1 hit) (grepped).

**Stated limits.** None in lib.rs beyond error strings.

**Fail-open risks.** none seen. `Rows::get` returns an empty slice on a miss (lib.rs:245-249); every consumer turns empty into an error (`scope()` -> `None`, `installation_scope` -> `Err`, `validate` -> "no admitted installation association", lib.rs:308, 358, 414-416) (traced). `PreparedInstalledInterruptHelpers::new` returns an empty answer table when `cohort.helpers()` is empty (lib.rs:464-470), and `verify` on an empty table errors "no admitted installation association" (lib.rs:443-445) (traced). `interrupt_installation::validate_target` indexes `calls[0]`, `calls[1]` (lib.rs:653-656) without a length check; the type of `calls` is defined in another crate (not read), so a short list would panic rather than accept (inferred).

**Doc/code mismatch (record only).** lib.rs:3-10 says `stage_optimized_instruction_selection` "validates the target register environment"; `optimized.rs:283-298` takes an already `ValidatedTargetRegisterEnvironment` and does not validate it (read). The flat-layout list at lib.rs:26-31 omits `legalization_descriptor_inputs.rs` and `legalization_function_inputs.rs`, which are declared at lib.rs:688-689 (read).

---------------------------------------------------------------------------------------------------

## 2. `optimized.rs` (298 lines)

**Purpose.** Stage entry: runs legalization, derives fixed-register-input constraints, runs selection, then records a custody receipt.

**Shape.** `constraints::selection_constraints` (optimized.rs:16), `push_fixed_input` (optimized.rs:52); `construction::construct_optimized_instruction_selection` (optimized.rs:91); `custody::selection_custody_receipt` (optimized.rs:124); `input::TargetSelectionInput` (12 fields, optimized.rs:160-175); `model::{StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt, OptimizedSelectionPipelineError{Legalization, Selection}}` (optimized.rs:191-272); `pub fn stage_optimized_instruction_selection` (optimized.rs:283).

**Inputs/outputs.** In: `TargetSelectionInput` plus a `ValidatedTargetRegisterEnvironment`. Out: the legalized plan, `ValidatedSelectedInstructions`, and a `Copy` receipt holding identities (psi, target, entry, optimization, projection, manifest, unit, work schedule, register environment, legalized, validator, selected, function count) (optimized.rs:131-147). `selections` and `budget_per_pass` are carried through to the output untouched (optimized.rs:293-294).

**How it decides.** Fixed-input constraints: for each scalar function, for each parameter whose `value` is in `value_transport::required_values(function)` (selection.rs:1624) and whose placement is exactly one `ValueLocation::Register{value_byte_offset: 0}` (optimized.rs:27-34), push a `SelectedFixedInputConstraint` keyed on `(machine, source_value, parameter_index, register)` (optimized.rs:60-65). Duplicates are skipped (optimized.rs:60-67). If `environment.fixed_register_view(register)` is `None` the parameter is silently skipped (optimized.rs:68-70).

**Cases by shape.** Only single-register parameter placements get constraints; stack, multi-fragment or indirect placements get none (optimized.rs:27-34).

**Same job elsewhere.** `selection.rs::constraints::fixed_input_constraint` (selection.rs:594) looks the same rows up by the same 4-tuple (read); that is the consumer side.

**Unused / one-caller.** `selection_constraints` and `construct_optimized_instruction_selection` are `pub(crate)` with one caller each inside this file (read, optimized.rs:109, 288).

**Stated limits.** none.

**Fail-open risks.** optimized.rs:68-70: a required register parameter with no fixed view in the environment gets no constraint and no error. What wrong program gets through: one whose parameter register is not modelled by the environment; whether selection later rejects a parameter that has no fixed input was not traced -> "open".

---------------------------------------------------------------------------------------------------

## 3. `selection.rs` (1736 lines)

**Purpose.** Driver and shared helpers for instruction selection: block ordering, view-root resolution, constraint-row instantiation, whole-plan construction, a foreign-scalar backing analysis, identity receipt, error type, and many `pub(crate) use crate::selection_inputs::*` re-exports that make `selection_inputs` items reachable as `crate::selection::<name>` (selection.rs:8-11, 301-302, 731, 1222, 1454-1455, 1606-1614, 1711; also `selection_call_abi as scalar_call_abi` 1606, `selection_edge_transfers as edge_transfers` 716).

**Shape (modules, line cites).**
- `atomic_access::form` (selection.rs:23): re-derives atomic form/geometry for an `AtomicAccess` row; returns `Option`.
- `block_order::derive` (selection.rs:104) and `ordered_blocks` (selection.rs:144); `#[cfg(test)] tests` (selection.rs:205-299) compare `ordered_blocks` against a quadratic `original_order` on every graph with up to 4 blocks and a reversed 8192-block chain.
- `byte_view_homes` (selection.rs:303): `ByteViewHomes` (7 fields), `own_view_root` (322), `view_backing_roots` (337), `bound_view_roots` (371), `transitive_view_roots` (385), `bound_view_roots_recurse` (406), `edge_successors` (479).
- `constraints` (selection.rs:505): `instruction` (516), `instruction_from_row` (528), `require_key_rows` (564), `row` (585), `fixed_input_constraint` (594), `store_registers` (611).
- `construction::build_plan` (selection.rs:673).
- `element_view_homes::ElementViewHomes` (717, data only).
- `extent_copy` (732): `INLINE_MAX_BYTES = 4096` (742), `chunks` (744), `bulk_operands` (756).
- `field_bytes::{FieldBytes, root_type}` (801-886).
- `foreign_scalar_backings::ForeignScalarBackings` (887-1128).
- `identity::receipt` (1129); `integer_conversion::normalization` (1167); `model` (1223-1453); `parameter_use::referent_used` (1456-1562); `place_declaration` (1563-1605); `value_transport::required_values` (1615-1710); `pub(crate) fn select_instructions` (1725).

**Inputs/outputs.** `select_instructions(legalized, constraints, environment)` -> `ValidatedSelectedInstructions{plan: Arc<SelectedInstructionPlan>, receipt}` (selection.rs:1725-1735). `build_plan` checks `target.target == environment.target()` (selection.rs:679), requires 10 key rows exist (selection.rs:568-581), builds `ForeignScalarBackings` and a `PreparedHelperCohort`, calls `scalar_graph::build_with_environment` per scalar function (selection.rs:693-704; that function is in another file, not read), sorts by machine id (selection.rs:706). `identity::receipt` counts functions/blocks/registers/instructions (+1 per block for the terminator) (selection.rs:1139-1153).

**How it decides.**
- Block order: Kahn-style with a `BTreeSet` of ready blocks, always taking the lowest block index (selection.rs:170-195); when none ready, takes the first unselected block in reverse-postorder of an iterative DFS (selection.rs:152-200). Returns `None` (-> `custody()` error) if the DFS does not reach every block (selection.rs:166-168), i.e. unreachable blocks are rejected in selection.
- View roots: keyed on `PlaceId` and `StructuralPlaceKind::BlockParameter` (selection.rs:326-334, 362-366). For a block-parameter view it scans every block's terminator successors that target the parameter's block (selection.rs:446-472); requires at least one bound edge and zero unbound edges (selection.rs:473-475). Cycles: `explored.insert(parameter)` false -> returns an empty root list for that branch (selection.rs:392-394); an empty total is an error (selection.rs:379-381).
- `store_registers` accepts a row with 1 operand (use) or 2 (second must be a def, early-clobber, untied, no fixed view) (selection.rs:622-637).
- `bulk_operands` demands exactly 5 operands: 3 uses then 2 early-clobber defs, no ties (selection.rs:765-779).
- `foreign_scalar_backings::matches` (selection.rs:951): breadth-first over "coordinates" `(function, StructuralArgument)`; at each, `scalar_storage_origins` gives either `Width(actual)` (must equal `width`) or a source place; for pointer/referent ingress or non-inline storage walks to callers via `incoming` (map machine -> calls) (selection.rs:1018-1075). Result is `true` only when every coordinate is "grounded" (selection.rs:1082-1103). Grounding propagates if ANY dependency is grounded (selection.rs:1089-1094) but the final test is `all` coordinates grounded (selection.rs:1103), so every reachable coordinate needs a path to a backing of the right width (read).
- Opaque ingress: the entry machine and every function whose `call_plan` equals a `Dynamic` call's `call_plan` (selection.rs:913-916, 940-948) -> `Ok(false)` for any parameter-sourced backing in them (selection.rs:1019-1021). This is keyed on call-plan equality, not on identity of the callee.
- `integer_conversion::normalization` (selection.rs:1171): only fixed 8/16/32/64-bit integer widen/exact-cast pairs (selection.rs:1180-1199) or a classified `ScalarRepresentationView` pair (selection.rs:1175-1178).

**Cases by shape.** `INLINE_MAX_BYTES = 4096` (selection.rs:742); `chunks` widths 8,4,2,1 (selection.rs:747); `store_registers` 1 or 2 operands (selection.rs:622); `bulk_operands` 5 operands (selection.rs:765); `register_form_normalization` float32 -> ZeroExtendU32, float64 -> CopyI64 (selection.rs:1212-1218). `parameter_use::referent_used` is a fixed list of ~35 instruction-kind arms and ends `_ => false` (selection.rs:1559): an instruction kind not listed counts as "does not use the referent".

**Same job elsewhere.**
- `selection_edge_transfers.rs` `descriptors::copy` uses `extent_copy::chunks` and `INLINE_MAX_BYTES` (selection_edge_transfers.rs:866-869); same chunking rule is used by the consumer in this unit.
- `value_transport::required_values` (selection.rs:1624) walks instruction operands, terminator operands and block-argument edges; `optimized.rs:22` is its only call in this unit (grepped: `required_values` used in selection.rs and optimized.rs; other crates not checked).
- `block_order::derive` -> 1 caller, `selection_scalar_graph.rs:3094` (grepped).
- `parameter_use::referent_used`: 1 caller, `selection_structural.rs:227` (grepped).
- `ForeignScalarBackings` (grepped, count mode, across omega-rust): selection.rs 3, selection_scalar_graph.rs 3, selection_call_abi.rs 1, selection_inputs.rs 1, selection_access_lifecycles.rs 1 = 9 occurrences in 5 files; no use outside this crate.

**Unused / one-caller.** `ElementViewHomes` struct (selection.rs:722) is data only; its users were not grepped. `FieldBytes::offset()` is `pub(crate) const` (selection.rs:846); not grepped.

**Stated limits.** selection.rs:142-143 "Keep the existing preference for the lowest ready block." (a compatibility comment, not a limit). No "unsupported" comments.

**Fail-open risks.**
- selection.rs:1559 `_ => false` in `referent_used`: a LegalizedScalarInstructionKind that reads a place but is not in the list is treated as not using it. The one caller (selection_structural.rs:227) was not read, so what it does with "unused" is "open".
- selection.rs:392-394 returns an empty root list on revisit; the aggregate empty case errors, but a revisit inside a union of roots just contributes nothing (traced: terminates cycles; roots found via other edges still required).
- Otherwise `foreign_scalar_backings::matches` returns `Ok(false)` (reject) on every unknown shape (selection.rs:982-983, 992, 1011-1013, 1019-1027, 1063, 1077): fail-closed.

---------------------------------------------------------------------------------------------------

## 4. `selection_edge_transfers.rs` (1645 lines)

**Purpose.** After the main selection pass, turns each semantic control-flow edge that carries values into an explicit "bridge" block: it snapshots scalar block arguments, descriptor copies and scalar references, runs the edge's cleanup, then stores them into the target's block parameters and jumps.

**Shape.**
- `construction::prepare(function_index, &mut SelectedFunction, constraints, catalog, environment, edge_work)` (selection_edge_transfers.rs:368) - the only entry (re-exported as `edge_transfers::prepare`, :1598).
- `construction::structural_case::snapshot` (:119): loads each enum-case payload scalar from the case source (frame slot or borrowed pointer) into virtual registers.
- `descriptors::{snapshot (:697), store (:745), copy (:861), bulk_copy (:956), memory_pointer (:831), frame_address (:807)}` and `#[cfg(test)] tests` (:1051-1277).
- `scalar_references::{snapshot (:1476), store (:1581)}`; `addresses::store` (:44); `emission::Emission` (:1293) with `emit`, `store_local_word`, `source_reads`, `instruction_id`; `pointer_type`, `provenance`.
- `EdgeWork{edge, preparation, cleanup}` (:1600); `invalid(function)` (:1606) = `FunctionProjectionMismatch`; `instruction_count` (:1610); `stored_transport` (:1618).

**Inputs/outputs.** Mutates the `SelectedFunction`: appends bridge `SelectedBlock`s (`origin: EdgeTransfer{edge,target}` or `Operation{..}`), new virtual registers, local storage slots (`LocalStorageSlotId::EdgeSnapshot{edge,binding_index}`), memory-access rows; rewrites `successor.block` to the bridge and sets the bindings' transports to `Unused` (:656-668).

**How it decides.**
- Which successors get a bridge: successors with a matching `EdgeWork` (matched on `successor.role == Semantic && work.edge == successor.psi_edge`, :396-402) or any transport/payload work (:427-435). A successor with no work is left alone unless it is the second successor of a conditional terminator, in which case it may be placed as a fall-through: allowed when the target is not the entry block, not already claimed as a fall-through target and the chain of `fallthrough_next` does not return to the source block (:436-459); otherwise a bridge is created.
- `OperationContinuation` successors get a jump-only bridge and are rejected if they carry any edge, work, bindings or case (:464-474).
- Value transport is a two-phase parallel move: phase 0 copies each active argument into a fresh register (snapshot), phase 1 copies the snapshots into the transfer registers after descriptor/scalar-ref snapshots and cleanup instructions (:534-602) (read).
- Copy width: `byte_size > 4096` -> `CopyBytes` via `bulk_operands`; else chunks 8/4/2/1 loads+stores (:866-869; selection.rs:742-754). Pointer displacement: if `environment.admits_pointer_displacement(offset, size)` use it, else emit `AddressOffset` (:839-858 and the same pattern at :253-291).
- Case payload loads: byte size must be 1,2,4,8 and offset aligned, within the slot (`byte_bound`) (:167-184); signed 8/16/32 payloads are sign-extended through `integer_carrier_normalization` (:326-358).

**Cases by shape.** `match shape.byte_size {1|2|4|8 => ..., _ => return Err(invalid())}` (:167-173). Scalar-reference source: descriptor is a fixed 16 bytes (:1495) or canonical codec (:1496). `SelectedStructuralTransport` has four variants; `stored_transport` returns `Some` only for `WholeValue` (:1627-1644), `addresses::address` only for `Address` (:26-38), `scalar_references::snapshot` only `ScalarReference` (:1484-1492). A structural-case successor with any ordinary `bindings` is rejected (:134-136, 407-409).

**Same job elsewhere.**
- Pointer-displacement fallback appears twice in this file: `structural_case::snapshot` :253-291 and `descriptors::memory_pointer` :831-859.
- Constructing a 64-bit unsigned pointer register type by hand appears at :204-206, :243-246 (and `pointer_type` :1443-1449 does it for `Emission`); `selection.rs:641-646, 785-787` do it again (read). 5 hand-written sites, 1 helper.
- The audit of the same bridge facts lives in `selection_scalar_graph.rs` (not read).

**Unused / one-caller.** `prepare` is `pub(crate)`; callers not grepped (re-export only). `EdgeWork` constructed elsewhere (not grepped).

**Stated limits.** none (no comments in the file beyond the header).

**Fail-open risks.** none seen. Every anomaly maps to `invalid(function)` (Err). One notable: line 671 `if !edge_work.is_empty() { Err }` rejects unconsumed edge work (traced, rejects). The error is the single generic `FunctionProjectionMismatch` for all of these failures (:1606-1608), so error identity does not say which check failed.

---------------------------------------------------------------------------------------------------

## 5. `selection_call_abi.rs` (2055 lines)

**Purpose.** Checks, at selection time, that each call row's ABI (placements, fixed registers, structural/borrowed/owned arguments, normalized foreign calls) matches what the calling-convention evaluator would compute, and finds the one register-constraint row that models the call. Also carries small ABI helpers (carrier normalization kinds, float bit-transfer kinds, load kind by size).

**Shape.**
- `borrowed_argument::validate_borrowed_argument` (:29) with `validate_reference_argument` (:478), `borrow_access_permitted` (:500), `projected_loan_shape` (:512), `expected_signature_plan` (:538).
- `input_site` (:577): `installation_call`, `use_cut`, `producer_before_use`, `no_normal_result`.
- `normalized_foreign` (:699): `entry_family`, `private_callback_ordinals`, `structural_parameter_positions`, `borrowed_block_argument_root`, `call_key` (:787), `call_constraint` (:805), `validate` (:819, ~450 lines).
- `owned_argument::validate_owned_arguments` (:1281).
- Top level: `owned_value_shape` (:1420), `accepts_stack_parameter_entry` (:1428), `scalar_stack_placement` (:1446), `register_argument_count/order` (:1474/1481), `call_abi_signature` (:1495), `unit_key`/`no_normal_key`/`invocation_key` (:1550-1643), `empty_aggregate_placement` (:1645), `validate` (:1650), `scalar_shape` (:1964), `integer_carrier_normalization` (:1968), `incoming_float_transfer`/`outgoing_float_transfer` (:1984/2001), `load_transfer` (:2018), `scalar_value_shape/type` (:2032/2036).

**Inputs/outputs.** Reads the `LegalizedScalarFunction`/`LegalizedScalarCall`, the register environment (`fixed_register_view`, `selected_keys`, `constraint`), `calling_conventions::{evaluate_call_plan, validate_boundary_entry_plan, classified_boundary_shape}`. Outputs `Option<()>`/`Result<(), SelectedInstructionError>` (accept/reject) and `Option<RegisterConstraintKey>`.

**How it decides.**
- `validate` (:1650) recomputes the call plan from the argument placements' shapes via `evaluate_call_plan` and requires equality with `call.call_plan` (:1698-1702). For structural arguments requires `source.call_plan.policy == CallingPolicy::native_for_target(environment.target())` (:1721).
- Constraint row choice: `invocation_key` (:1565) collects fixed-register views of all register-carried arguments and results, then picks the candidate key whose row operands match (count, Use/Def split, `fixed_view`) (:1622-1638); success only if exactly one key matches (:1639-1642). Candidate family is by result kind: `call_aggregate`, `call_scalar`, or `call_unit`+`call_unit_mixed` (:1615-1621).
- Borrowed arguments: keyed on `PlaceId`, `StructuralTypeId`, byte offsets and `StructuralAccess` equality; `shape` is chosen by an ordered `if/else if` chain over the argument's "home kind" (byte_field, address_join, storage_loan, block_forward, byte_view, byte_owner, byte_field(again), element_view, established_view_loan, local, projected_loan, else) (:272-356), then compared field by field with the target row (:376-396), then source kind checked against a second `match` (:397-474).
- Normalized foreign: `entry_family` (:711) picks `Call` for Import/Component/Table and `SystemCall` for SystemCall mechanisms; `validate` re-derives callbacks, scalar and structural argument positions and compares to the retained row, then re-validates the boundary entry plan (:1231-1262).

**Cases by shape.**
- `scalar_stack_placement` (:1446): one `ValueLocation::Stack`, class Float or Integer, size 4 or 8 (1 or 2 only for Integer), alignment power of two and offset multiple of alignment.
- `placement_register` (:1528): single register at offset 0, or an indirect register without a stack copy.
- The tail of `validate` accepts exactly three single-location shapes (register of the shape's size 1/2/4/8; 8-byte register for borrowed references; indirect register for borrowed references) and `_ => return Err(invalid())` (:1890-1937).
- `integer_carrier_normalization` (:1968): unsigned 8/16/32 -> ZeroExtend, signed 8/16/32 -> SignExtend, everything else (including 64-bit and non-integers) -> `CopyI64` (:1971-1981).
- `incoming/outgoing_float_transfer`: Binary32 and Binary64 only (:1989-1996, :2006-2013).
- `load_transfer`: sizes 1/2/4/8 only (:2023-2029).
- Hosted entry: `accepts_stack_parameter_entry` (:1428).

**Same job elsewhere.**
- `normalized_foreign::validate` (selection_call_abi.rs:819-1269) re-derives the same facts as the audit in `legalization_target.rs::normalized_foreign::validate` (legalization_target.rs:3077-3317): callbacks (`native_callbacks_at` vs inline loop :863-912), private ordinals (:726-742 vs legalization_target.rs:3148-3161), structural arguments, result shape, `validate_boundary_entry_plan...` (read both). Two implementations of one check at two stages.
- `owned_argument::validate_owned_arguments` (:1281) and the owned-argument reconstruction in legalization (`aggregate_results::call_argument`, referenced at legalization_target.rs:4822) are the same job at two stages (the latter not read).
- `use_cut`/`producer_before_use` (:610-670) vs `legalization_target.rs::unit::aggregate_results` site lookup (`node_site`, referenced :2128, not read): suspected.

**Unused / one-caller (grepped across omega-rust).** `accepts_stack_parameter_entry`: 1 call site (`selection_scalar_graph.rs:6229`). `scalar_stack_placement`: 5 call sites (selection_call_abi.rs:1862; selection_scalar_graph.rs:6235, 6307, 7140, +1 in the same file group; listed 4 external + 1 internal). `register_argument_count`: 1 call (:1786). `incoming_float_transfer`: 4 external calls; `outgoing_float_transfer`: 4 external calls (selection_scalar_graph.rs:702, 2016, 2689, 5135).

**Stated limits.** none in comments. Error strings only: "installation validator must return Boolean and work must have no receiving result" (:1682-1685).

**Fail-open risks.**
- :1428-1430 `accepts_stack_parameter_entry` returns `true` when `call_plan.result.is_none()` without looking at parameters. What gets through: a function with no result and arbitrary parameter placements passes this entry predicate. Whether the one caller (selection_scalar_graph.rs:6229) re-checks parameters elsewhere was not traced -> "open".
- :354-356 default arm `destination_shape` for a shared borrow of a non-primitive with no special home: bounded afterwards by `no_indirect_home` checks (shared borrow, empty path, root==structural type, offset 0, :358-384) -> "traced" (read).
- :1814-1816 `if index == count && placement.locations.is_empty() { continue; }` skips operand checks for an empty result placement (traced: only after `result == call.result_placement` check at :1809).
- Dead branch: :321 `else if let Some((offset, _)) = byte_field` cannot run because :272 already handles `byte_field.is_some()` (read; inference from the code, no test run).

---------------------------------------------------------------------------------------------------

## 6. `legalization_target.rs` (6973 lines)

**Purpose.** The independent "audit" half of legalization: given a `TargetOperationPlan` function and the abstract-operation/optimization-unit function it came from, re-derive what each target row should be from the source and compare, row by row, block by block. It exists so the target plan can be checked at corpus/foreign-artifact boundaries (lib.rs:12-21). It is wired in through `legalization_scalar_graph.rs:8588-8593` (`pub(crate) use crate::legalization_target as target; pub(crate) use target::validate_target;`) (grepped).

**Shape (module map, all read).**
- `byte_view` (:19-235): `Checker::byte_view`, `mutable_byte_view`, `established_byte_view`.
- `control_flow` (:236-1772): `no_normal::validate` (:256), `sources` (:342-871: `call_indices` :347, `SourceInputs` :424, `AvailableSources` :457, `definition` :574, `SourceDominators` :684, `tree_intervals` :692), `structural_cases` (:872-1168: `home_available`, `matches`), `validate` (:1170), `successor_matches` (:1732), `edge_cleanup_matches` (:1759).
- `element_view` (:1773-2388): `element_view_access`, `block_backing_precedes`, `local_home_precedes`, `established_element_window`.
- `expressions` (:2389-2942): `integer_source`, `boolean_source`, `expression`, `boolean`, `operands_agree`, `arithmetic_carrier_admitted`.
- `hosted_scalar` (:2943-3060); `normalized_foreign` (:3061-3318); `scalar_definitions` (:3319-3703: `validate` :3332, `observed_family` :3442, `observation` :3460).
- `unit` (:3704-6900): `aggregate_results` (:3725), `borrowed_windows` (:4366), `direct_calls` (:4470; `results` :4477), `ieee_float` (:4858), `primitive_store` (:5022), `validate_operation` (:5240, a ~1,630-line `match (target, abstracted)`), `runtime_indices_rejoin` (:6870), `scalar_home` (:6885).
- `validate_target` (:6901), `struct Checker` (:6953), `resolve` (:6960), `location_matches` (:6967).

**Inputs/outputs.** Reads `TargetFunction`/`TargetOperationPlan`, `AbstractFunction`/`AbstractOperationPlan`, `PsiOptimizationFunction`/`PsiOptimizationUnit`, `FunctionInputs`, `DescriptorInputs`, reference-custody states. Returns `Result<BlockEntryCustody, LegalizationError>`; all failures are `LegalizationError::custody()` except `ElementViewCustody{machine,operation,place,reason}` (:6601-6606). `OMEGA_COMPILER_CHECK_SITES` env var gates an `eprintln!` diagnostic (:4643-4653).

**How it decides.** Almost every check is "reconstruct the expected row from the abstract side and compare with `==`", keyed on `OperationId` (psi_operation), `ValueId`, `PlaceId`, `BlockId`, `EdgeId`, `StructuralTypeId`, never on names. Typical form: `matches!(&node.operation, AbstractOperation::X{..} if operation == psi_operation && ...)` over `optimized.block_nodes()` (e.g. :2530-2533, :2582-2586). Operands are validated by `AvailableSources::matching(value)` which returns rows defined earlier on the dominator path (:468-488) and requires the target's `Source` to equal the recorded definition. Dominance: `SourceDominators` (iterative Cooper-Harvey-Kennedy on reverse postorder, :717-798) with subtree intervals (`tree_intervals` :692-714).
- Block terminators: `validate` pairs `(TargetControlTerminator, AbstractOperation)` in a single `match` (:1310-1724) with `_ => false`.
- Per-operation: `validate_operation` (:5321-6866), `_ => return Err(custody)` (:6865).

**Cases by shape (fixed lists that refuse the rest).**
- `hosted_scalar::validate` (:2943): only `BoundarySettlement` with unit result and exactly one runtime scalar argument of type `i32`; realizations `HostedWriteByteI32` and `HostedExitProcessI32` only (:2992-3014, :3023-3050).
- `scalar_definitions::validate` (:3332): only `IntegerWiden` into target bit width 16, 32 or 64 (:3367); operand must be Parameter, Immediate, BlockParameter or ScalarHome (:3381-3435).
- `integer_source` accepts only four expression kinds: Immediate, Parameter, ScalarHome, BlockParameter (:2411-2417); same for `boolean_source` (:2426-2432).
- `ProgramImageAddress` only on `NativeTarget::uefi_x64()` with `ProgramImageRetainedSuffixStart` (:5480-5483).
- `StaticInterrupt` operations: `Install` is not allowed here; token access `Owned` only for `FinishCompletion`, else `SharedBorrow` (:5362-5370).
- External completion binding `Checked` origin: only `InstalledProvider` and `TargetFamily` accepted; `Authored`/`ProgramEntry` -> false (:1491-1510).
- `structural_cases::matches` sources: `Home`, `Parameter`, `BorrowedParameter`, `BorrowedBlockParameter` (:1021-1061).
- `StructuralLeafCopy` via `BoundaryCall` only for the run-release contract (`empty_run_release_contract`, :4087).
- Element view representation windows: "representation view has an unsupported source window or access" (:2270), "unsupported live extent" (:2291).

**Same job elsewhere.**
- `arithmetic_carrier_admitted` (:2926-2941) and the inline `carrier_admitted` match in `scalar_definitions::observation` (:3481-3491) test the same `(policy, operator)` condition. They differ: the helper includes `Saturating` + `Multiply` (:2933); the inline copy only has `Saturating` + `Divide|Remainder` (:3483-3485). (read; recorded as a difference, not a verdict.)
- The 4-arm "destination storage matches" `match` appears in `primitive_store::validate` (:5062-5125) and again in `StructuralScalarFieldStore` (:6701-6772), with different guards on the `Parameter/BlockParameter` arm (MutableBorrow|WriteOnlyBorrow at :5083 vs Owned|exclusive at :6721, plus claim checks :6727-6735).
- Scalar-argument-vs-placement check appears in `no_normal::validate` (:300-313) and `direct_calls::validate` (:4767-4780).
- `normalized_foreign::validate` duplicates the selection-stage one (see section 5).
- Dominator computation: `terminal-psi/semantics/src/control_graph.rs:348 pub fn dominates` is another dominance query (grepped, other implementation not read -> suspected duplicate). Within this crate, `sources::dominates` is called 17 times (legalization_target.rs 7, legalization_scalar_graph.rs 10; grepped) and each call constructs `SourceDominators::new(function)` from scratch (:681) -- no cache.

**Unused / one-caller.** `sources::call_indices` (:347): 6 external call sites in `legalization_scalar_graph.rs` (147, 587, 815, 4491, 6121) and `legalization_reference_custody.rs:7461` (grepped). `SourceDominators` is only constructed at :504 and :681 (grepped). `Checker` fields are private; `Checker { ... available: None }` is constructed nowhere in `src/` (grepped `available: None`: 0 hits), so the `available.is_none_or(...)` fallbacks (:2726, :3493, :3568, :3585, :3610, :3633) have no `None` caller in this crate (inferred; other crates cannot build it, it is `pub(crate)`).

**Stated limits.** Error strings only: "representation view has an unsupported source window or access" (:2270-2272), "view source has no matching element-region geometry" (:2253), "borrowed collection cannot supply a static inline window" (:2330). No TODO/migration comments in this file.

**Fail-open risks.**
- :810-812 `SourceDominators::dominates`: `let Some(position) = self.order.get(&block) else { return true; }`. A `block` that is not reachable from the function entry is treated as dominated by every candidate. Combined with `InputScope::admits` (:446-448): when the current block has no dominator interval (`self.interval.is_none()`), every block-scoped row (except same-block operation rows) is admitted. What wrong input gets through: a target plan whose unreachable block uses values defined in other blocks. Traced: the compile path rejects unreachable blocks later in selection (`ordered_blocks` returns `None` if postorder misses a block, selection.rs:166-168), so this is reachable only through the standalone audit (`verify_target_operation_plan`, one external caller, image-emission/src/function_fragments.rs:3878). Status: "traced" for selection, "open" for the audit consumer.
- :2726-2728 `Boolean::Immediate` check uses `self.available.is_none_or(...)`; `None` -> accepts. "traced": no `available: None` constructor in this crate.
- :3493 etc. `is_none_or(|sources| <failure predicate>)`: `None` -> returns Err (fail-closed).
- :6865 `_ => return Err(custody)`, :1723 `_ => false`, :2658 `_ => false`: fail-closed defaults.
- :1273-1274 `block_entries.get(&block.block).cloned().ok_or(...)`: missing entry state -> error (fail-closed).
- `scalar_definitions::observed_family` (:3442) is a fixed list; an abstract operation outside the list never reaches `observation` via the `if guard` at :6549-6551 and falls to later arms / the final `_ => Err` (traced).
- Otherwise none seen.

---------------------------------------------------------------------------------------------------

## 7. `selection_access_lifecycles.rs` (6844 lines)

**Purpose.** Emits selected instructions (as straight-line code with runtime loops/branches) for the four resource "access lifecycle" operations: Initialize, Relocate, View, MoveOut. The generated code reads a runtime "resource profile" structure (regions, stable/external/atomic capabilities, reach lists), validates it, checks it against the operation's placement plan (coverage, alignment congruence, reach), and writes a typed Ready/Rejected result, copying or encoding/decoding the resident image.

**Shape.**
- `initialization` (:23-370: `emit` :36, `stage_image` :281, `commit` :345), `outcome` (:371-550: `write_word`, `PlacementDecision`, `decide` :402, `compare` :481, `write_failure` :507), `relocation::emit` (:551-890), `view::emit` (:5914-6055), top-level `pub(crate) fn emit` (:6073) dispatching on `AccessLifecycleOperands::{Initialize, View, Relocate, MoveOut}` (:6097-6106); MoveOut is emitted inline in `emit` (:6107-6278).
- `resource_profile` (:891-4305): `compatibility` (:898-1938: `run::assess` :932, `assess` :1729, `assess_cell` :1762, `extend_coverage` :1588, `effective_alignment` :1422, `combine_alignment` :1548, `equivalent` :1300, ...), `geometry` (:1939-2287: `ProfileLayout::derive` :2227), `tests` (:2288-3866), then the validator: `Emission` helper struct (:3885) and `validate_layout` (:4189) which emits runtime code that checks the profile image (`bounded_count`, `transfer_rules`).
- `result_layout::ResultLayout::derive` (:4306-4592).
- Top-level helpers: `failure`, `coordinate`, `offset` (:6058-6071), `operand_indices` (:6281), `owner_word` (:6298), `ImageHome` (:6323), `transfer_image` (:6394), `field_region` (:6485), `aggregate_chunks` (:6519), `ScalarImageCodec` (:6625-6844).
- Top-level `#[cfg(test)] pub(crate) mod tests` (:4593-5912).

**Inputs/outputs.** In: a `LegalizedScalarInstruction` of kind `AccessLifecycle{boundary, operands, indices, result}`, the `TerminalAccessLifecyclePlan` found by `(boundary, operation)` (:6092-6096), and a mutable `Builder`. Out: emitted instructions/memory-access rows/local slots on the builder (`Boundary{operation}` scratch slot, `ImageHome::scratch` :6331-6385 which errors if the slot already exists).

**How it decides.**
- Profile layout is not read from names: `ProfileLayout::derive` navigates fixed shapes by position: counted regions (capacity 32), region record of 6 fields `[range, peer, stable, external, atomic, reach]`, `external` sum with 2 cases, `atomic` sum with 2 cases, rules arrays capacity 16, atomic permission record of 12 booleans, reach capacity 32 (:2232-2266). Each node width is checked (u64 = 8 bytes unsigned 64-bit; boolean = 1 byte) (:2170-2175).
- Runtime profile validation (`validate_layout` :4189-4304) emits checks: counts <= capacity, region non-empty, no overflow, within `available`, pairwise disjoint, tag ranges (peer/external/atomic <= 1, stable <= 3), peer/stable exclusivity, at least one capability, external read <= 2 / write <= 1, transfer rules (width in 1..=128 bits, multiple of 8, alignment non-zero power of two, no duplicate width), reach services non-zero.
- Operation-level checks are emitted as runtime code over scratch slots at fixed byte offsets (constants :1141-1159, run scratch :913-917, validation scratch :3876-3882); `COMPATIBILITY_SCRATCH_BYTES = 200` (:1140), `SCRATCH_BYTES = 240` (:913), `VALIDATION_SCRATCH_BYTES = 48` (:3876).
- Initialization/relocation/view decisions use `outcome::decide` (:402): `ready = range & supplied & aligned`, `outcome_tag = 1 ^ ready`, `failure_tag = range * (2 - supplied)`, `copy_length = required * ready` (:431-472) - branchless.
- Result layout keyed on structural shape: result sum must have exactly `[Ready, Rejected]`; `Ready` one field, `Rejected` two (`returned`, `reason`); reason sum has 3 cases `[range, alignment, incompatible]` with `incompatible` empty (:4372-4381, 4515-4520).

**Cases by shape.**
- `result_layout::derive` matches `plan.operation` into Relocate / Initialize / View and rejects MoveOut ("extraction does not have a placement establishment outcome", :4420-4424).
- `initialization::emit` requires `TerminalAccessLifecycleExecution::CompilerPrimitive{profile_type: Some}` and an owned `ReturnedLayout` (:63-84). `view::emit` requires a `Borrowed` returned layout and an 8-byte Ready field (:5954-5960).
- `relocation::emit` requires `TerminalAccessOwner::Run{..}` (:587-595).
- `aggregate_chunks` (:6519): aggregate fields only as whole byte placement (`At`) or fixed arrays with either one base offset or one offset per element; otherwise "resident aggregate requires whole or repeated byte placement" / "resident non-array aggregate needs one complete image placement" (:6529-6531, 6589-6593).
- `ScalarImageCodec::placement` (:6662): widths 1..=8 bytes; placements `[At]`, `[IntegerAt]` (stored width multiple of 8, <= carrier), or non-empty `Bits` fragments (container width 8..=64 multiple of 8) (:6666-6739).
- `ScalarImageCodec::field` rejects widths outside 1..=8 (:6645-6649).
- MoveOut result must be a sum with exactly one case holding `[owner, value]` (:6132-6153).

**Same job elsewhere.**
- `ImageHome { storage, place, pointer, byte_offset }` construction from `leaf_copy::project_operand` repeats in initialization (:130-140), relocation (:733-743), view (:5970-5980, 5984-5994) and MoveOut (:6210-6220).
- Region interval overlap/coverage logic appears in `validate_layout` (disjointness :4222-4242) and `extend_coverage` (:1588-1727) over the same runtime profile layout.
- `run::assess` (:932) and `assess_cell` (:1762) both run `extend_coverage` + `combine_alignment` with different demands.
- Compare/min/arith helpers: `Emission::{compare, equal, at_most, invert, nonzero}` (:3911-3966) vs `outcome::compare` (:481); the macro `bin!` is defined twice (:904-908 and :3870-3874).
- Profile-layout geometry is navigated again via `structural_reference_input` in `result_layout` (:4427, 4513) and `geometry` (:2045-2072) - same `case_layout`/`field_storage_shape` calls.

**Unused / one-caller.** `validate` (:4176) is `#[cfg(test)]` only. `decode_scalar` (:6602) is `#[cfg(test)]` only. `atomic_bits` is `pub(crate)` (:1368) - not grepped for outside users. The top-level `emit` (:6073): called from `selection_scalar_graph.rs` (not grepped).

**Stated limits.** Error strings: "initialization requires a fixed complete image" (:145), "field without primitive transfer geometry acquired a runtime transfer descriptor" (:1434-1436), "resident image exceeds native byte coordinates" (:6065), "resident scalar exceeds the native register carrier" (:6667), "resident aggregate requires whole or repeated byte placement" (:6530). No TODO comments.

**Fail-open risks.**
- :1433-1437 `effective_alignment` errors for `FieldAccess::Inaccessible | Region` (fail-closed).
- :6749-6751 / 6827-6829 fragments with no byte envelope error (fail-closed).
- The emitted runtime code treats a malformed profile as "not well formed" and the caller traps with `operation_control::require_nonzero(.., assessment.well_formed)` (:160, 755, 5022-5023 area, 6239) rather than returning `Rejected`; test `malformed_runtime_counts_tags_and_boolean_bits_never_enable_profile` (:3466) and the `outcome == 4` branch in the initializer test (:2732-2741) show "must not return Rejected" (read the latter).
- Mixed: a count of 0 in `repeat` loops simply skips bodies (:4084-4112); `bounded_count` clamps `count * (count <= capacity)` (:4081) but also requires `bounded` (:4076) so an oversize count invalidates the profile rather than being clamped silently (traced).
- Otherwise none seen in the non-test code.

---------------------------------------------------------------------------------------------------

## Test code in `selection_access_lifecycles.rs` (read after the first draft)

- The test modules (`resource_profile::tests` :2288-3866 and top-level `tests` :4593-5913) contain a small interpreter for selected instructions (`execute_with_frame` :4696-4807, `execute_operation` :4818-4861) that handles a fixed list of ~25 instruction kinds and panics on any other (`ref unexpected => panic!`, :4802). That is a test-only interpreter of selected instructions; whether other interpreters of `SelectedInstructionKind` exist elsewhere was not grepped.
- Tests compare the emitted runtime profile validator (`validate_layout`, :4189) and the emitted compatibility check (`compatibility::assess`) against the `access_plans` normalizers `validate_resource_profile`, `validate_initialization_resources`, `require_stable_region_supply`, `validate_run_relocation_resources` (calls at :3421, 3786, 2908, 2919). Those four functions are defined once each in `psi/foundation/access-plans/src/resources.rs:1236, 405, 427, 1396` (grepped). So the same profile-validity rule exists twice: as Rust in `access-plans` and as emitted selected-instruction code here, with tests as the only link between them in this unit.
- The profile layout used by tests (`Types::profile`, :3054-3073) mirrors `ProfileLayout::derive`'s positional roster (word, boolean, range, peer(2), stable(4), read(3), transfer, rules(16), external, operations(12), atomic_transfer, atomic_rules(16), atomic, reach(32), region, regions(32)).
- Test `malformed_runtime_counts_tags_and_boolean_bits_never_enable_profile` (:3466-3527) writes 12 faults + 12 atomic-operation bits = 2 and asserts the emitted validator returns "not valid" for each.
- `resident_initialization_rejects_before_copy_and_commits_only_the_complete_image` (:5799) exercises `outcome::decide` on 7 `(base, length, supply, outcome, reason)` rows; `decide_constants` (:5888) is a test helper.
- No fail-open behaviour found in the test code; the tests assert rejection preserves both input and backing (:5844-5848).

## Coverage

- `lib.rs` 1-724 read in full (724).
- `optimized.rs` 1-298 read in full (298).
- `selection.rs` 1-1736 read in full (1736).
- `selection_edge_transfers.rs` 1-1645 read in full (1645).
- `selection_call_abi.rs` 1-2055 read in full (2055).
- `legalization_target.rs` 1-6973 read in full (6973).
- `selection_access_lifecycles.rs` 1-6844 read in full (6844), including the two test regions.
- Skimmed: none. Not read: none. Total read: 20275 of 20275 lines.

## Five things a maintainer most needs to know about this unit

1. `legalization_target.rs` is an audit that rebuilds every target row from the abstract plan and compares with `==`; it is reached only via `legalization_scalar_graph.rs:8588-8593` and the one external caller of the standalone verifier (`image-emission/src/function_fragments.rs:3878`). Selection itself does not call it (grepped).
2. Several checks are written twice, at the audit stage and at the selection stage (normalized foreign calls: legalization_target.rs:3077 vs selection_call_abi.rs:819; owned/borrowed call arguments; scalar-vs-placement check at :300 and :4767). They are separate code, not shared.
3. `SourceDominators::dominates` rebuilds the dominator tree on every call (legalization_target.rs:681; 17 call sites) and returns `true` for blocks it never reached (:810-812); in selection, unreachable blocks are separately rejected (selection.rs:166-168).
4. `selection_access_lifecycles.rs` generates runtime code that parses and validates a resource-profile image whose layout is fixed by position (32 regions, 16 rules, 12 atomic permissions, 32 reach entries; :2232-2266). Scratch slot offsets are hard-coded constants (:913-917, 1140-1159, 3876-3882).
5. Failure reporting in selection is coarse: edge transfers return the single `FunctionProjectionMismatch` (selection_edge_transfers.rs:1606), and most other failures are `custody()` with an optional reason string and a `#[track_caller]` site (selection.rs:1432-1451).

## What I am unsure about

- Whether `accepts_stack_parameter_entry` returning `true` when there is no result (selection_call_abi.rs:1428-1430) is re-checked at its one caller (selection_scalar_graph.rs:6229, not read).
- Whether selection rejects a required register parameter for which `fixed_register_view` is `None` (optimized.rs:68-70); the consumers are in files I did not read.
- The type of `calls` in `InstallStaticInterrupt` (lib.rs:653-656) and so whether `calls[0]`/`calls[1]` can panic.
- I read every line, but in a single pass over ~20k lines; the 1,630-line `validate_operation` match (legalization_target.rs:5321-6866) and the borrowed-argument chain (selection_call_abi.rs:29-476) were read for structure, not verified arm by arm against the abstract/target operation definitions (which live in other crates).
- Whether the `dominates` "unreachable block returns true" path (legalization_target.rs:810-812) can be reached with a real plan: I traced that selection rejects unreachable blocks, but not whether the legalizer already guarantees reachability.
- Counts of `scalar_stack_placement` callers: I listed the four lines the grep printed (selection_scalar_graph.rs:6235, 6307, 7140 plus selection_call_abi.rs:1862); I did not recount with a count-mode grep.
