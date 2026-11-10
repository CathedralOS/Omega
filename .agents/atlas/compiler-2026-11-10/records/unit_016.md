# Unit 16 notes

Checkout: `<checkout>/` (main `457adf4a11f`). All paths below are relative to it.

Files covered (all read in full, 16,797 lines):

1. `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow_storage.rs` (1-3715)
2. `.../02_abstract-operations-to-target-operations/src/coordination.rs` (1-645)
3. `.../02_abstract-operations-to-target-operations/src/function_signature.rs` (1-826)
4. `.../02_abstract-operations-to-target-operations/src/lib.rs` (1-701)
5. `.../02_abstract-operations-to-target-operations/src/model.rs` (1-326)
6. `.../02_abstract-operations-to-target-operations/src/references.rs` (1-2686)
7. `.../02_abstract-operations-to-target-operations/src/scalar.rs` (1-2900)
8. `.../02_abstract-operations-to-target-operations/src/structural_layout.rs` (1-1322)
9. `.../02_abstract-operations-to-target-operations/src/unit.rs` (1-2832)
10. `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs` (1-518)
11. `.../03_target-operations-to-selected-instructions/src/legalization_descriptor_inputs.rs` (1-155)
12. `.../03_target-operations-to-selected-instructions/src/legalization_function_inputs.rs` (1-171)

Short names: "S02" = `02_abstract-operations-to-target-operations/src`, "S03" = `03_target-operations-to-selected-instructions/src`.

Common vocabulary used by almost every S02 file (read, inferred from use): `AbstractFunction` (input row set for one machine), `LiveDefinitions` (the per-block mutable state: `structural_homes`, `views`, `references`, `integers`, `lengths`, `address_joins`, `block_views`, `borrowed_reference_sources`, etc., defined in `control_flow.rs`, not in my unit), `RowSite` (a `(function, types, live, operations, provenance)` bundle with `.parts()`/`.view()`), `StructuralTypeLookup` (`function_signature.rs:777`), `LoweringError::unsupported_control_flow(machine)` (`model.rs:320`) which is the catch-all refusal. Each lowering function takes one abstract operation, checks it, pushes a `TargetUnitOperation` into `operations` and the Psi operation id into `provenance.operations`.

---------------------------------------------------------------------------

## 1. S02/control_flow_storage.rs (3715 lines; a bag of 12 inline `pub(crate) mod`s)

### Purpose
Lowers the "storage" family of abstract operations (aggregate results, record/sum/array construction, primitive locals, field and byte/element reads and writes, atomics, access lifecycles, borrowed-window stores) into target operations inside the common control-flow graph. It does not choose layouts itself; it checks that the abstract operation's places, types and scalar widths agree with what is live and records the checked fact as a `TargetUnitOperation`. File header comment: `control_flow_storage.rs:1-10`.

### Shape (module -> entry points)
- `access_lifecycles` (13-154): `lower` (28). Only `AbstractOperation::BoundaryCall` for a compiler-primitive access lifecycle (Initialize / MoveOut / View / Relocate).
- `aggregate_borrows` (156-279): `is_reference` (164), `argument` (182). Builds a `TargetStructuralArgument` for a borrowed record/sum argument.
- `aggregate_results` (281-494): `value_input` (290), `block_home` (393), `result_home_layout` (420), `value_home_layout` (427), `home` (448), `publish_home` (470), `established_home` (484). The shared "make a home requirement for a result" helpers; `sum_layout` (378) private.
- `atomic_access` (496-707): `lower` (505).
- `borrowed_windows` (709-1095): `move_field` (722), `store_field` (825), `window_referent` (955), private `root_window_loan` (975), `window_root` (1043), `field_path` (1086).
- `byte_concatenation` (1097-1222): `lower` (1109).
- `byte_write` (1224-1708): `replace` (1236), `replace_view_descriptor` (1382), `replace_byte` (1568), `lower` (1637), private `writable_field` (1356), `writable_byte_field` (1464).
- `element_view_write` (1710-1786): `lower` (1720).
- `leaf_copy` (1788-2014): `transport` (1802), `copy_extent` (1872), private `byte_carrier_leaf_copy` (1972).
- `observations` (2016-2730): `lower` (2039; scalar/byte/element observation and integer/boolean arithmetic), `scalar_values` (2521), `bounded_owner_storage` (2628), private `byte_sequence_operand_gate` (2648), `removed_scalar_expression` (2700), `record_view_result` (2711).
- `primitive_storage` (2732-3305): `lower` (2911), `field_root` (2817), `field_indices` (2870), `byte_field_observation` (2789), `retain_result` (2895), `is_primitive_reference` (2752), `shape` (2768), `native_shape` (2774).
- `records` (3307-3715): `establish` (3321), `establish_trivial_affine_local` (3625), `establish_structural_local` (3664), `establish_zeroed_structural` (3695).

### Inputs and outputs
Reads `RowSite` pieces (function, `StructuralTypeLookup`, `LiveDefinitions`), `PreparedFunctionSignature` (for parameter shapes), and calls into `super::references` (reference custody), `super::scalar_sources` (runtime index sources), `crate::scalar::{byte_views, element_views, descriptor_operands}` (view resolution), `crate::structural_layout` (offsets), and `terminal_semantics::*` (type-level predicates). Output: pushes `TargetUnitOperation::{AccessLifecycle, AtomicAccess, AtomicFence, StoreStructuralField, EstablishByteSequenceConcatenation, ByteSequenceStore, StructuralByteSequenceFieldStore, StructuralByteViewFieldStore, StructuralByteSequenceFieldByteStore, ByteSequenceWrite, ElementViewWrite, StructuralLeafCopy, ScalarDefinition, StructuralByteSequenceFieldLength/Read, StructuralScalarFieldRead, EstablishPrimitiveLocal, PrimitiveLocalStore, StorageAddress, PrimitiveScalarRead, EstablishRecord/Case/StructuralArray, EstablishTrivialAffineLocal, EstablishStructuralLocal, EstablishZeroedStructural}`; mutates `live.structural_homes`, `live.views`, `live.integers`/`scalar_homes`, `live.references`, `live.lengths`, `live.trivial_affine_locals`.

### How it decides (what the judgment is keyed on)
- Place/identity equality, not spelling: homes are looked up by `PlaceId` in `live.structural_homes` (e.g. 307, 1251, 1414, 3192), arrival parameters by `function.arrival_parameter(place)` (355, 1257, 1538), operation producers by `OperationId` (`home.operation_result() == Some((*producer, result))`, 1079-1082). No `display_name`/string compare/debug-format keyed decisions seen in this file.
- Scalar width/sign rules are tested by value: index/length must be unsigned 64-bit non-address (1199, 1328, 1617-1621, 1766-1768, 2689-2691, 2929, 2991-2992); byte value unsigned 8-bit (1612-1616, 1685-1688, 2976).
- A "length was measured" witness is found by scanning `function.operations` for a matching `StructuralByteSequenceFieldLength` whose `(source place, path, field, result value)` equal the use: 1178-1185, 1596-1599, 2674-2680, 2977-2979. Matching is by place + path (`==` at 1182; `structural_paths_select_same_location` at 1598 and 2979) + field id + value id.
- Access admission: borrow kinds matched by enum (`Owned | MutableBorrow | WriteOnlyBorrow` for writes, 1269-1274, 1487-1494; `SharedBorrow` only for reads, 1147, 1176, 2672); `WriteOnlyBorrow` observation refused (1177, 2673, 2975, 606-608).
- Overlap with standing memberships: `structural_memberships_disjoint_from_write` (1552-1554) -> `InvalidStructuralValueInput` "byte field write overlaps a standing carrier membership" (1555-1559).
- Linear/affine "consumed" bookkeeping in `records::establish` (3411, 3552-3566): keyed on `(PlaceId, path)` with prefix overlap in either direction (3561-3564).
- Window authority (`borrowed_windows::window_root`, 1043-1084; `root_window_loan`, 975-1041) keyed on `StructuralWindowRoot` variant, parameter `place`+`access`+`structural_type` equality, `live.owned_arrivals`, `entry_claims` (1034-1037).

### Cases by shape (fixed lists that refuse the rest)
- `access_lifecycles::lower` (90-112): a 4-slot `expected` carrier array per `TerminalAccessLifecycleOperation` {Initialize, MoveOut, View, Relocate}; requires `TerminalAccessLifecycleExecution::CompilerPrimitive` (58-64) and zero scalar arguments (84).
- `aggregate_borrows::is_reference` (164-180): only `SharedBorrow|MutableBorrow|WriteOnlyBorrow`, `Unrestricted`, referent shape `Record` or `Sum`.
- `byte_concatenation::lower` (1135): exactly two operands `[left, right]`, each `ByteSequenceOperand::Whole` (must be `SharedBorrow`, no runtime indices, `ByteSequence` shape) or `::Field`; result must be `Unrestricted`, no claims, bounded owned byte sequence (1125-1134).
- `byte_write::replace` (1244, 1289): only `ByteSequenceStore` and `StructuralByteSequenceFieldStore`; `_ => unsupported` (1316).
- `leaf_copy::byte_carrier_leaf_copy` (1972-2013): only a `FixedArray` of literal length whose element is a plain `u8` scalar, last path segment `FixedIndex`, and `index + layout.size <= length`.
- `leaf_copy::transport` (1808-1836): `StructuralLeafCopy | MoveStructuralValue | TransformContent`, else unsupported.
- `observations::lower` (2071-2109 then 2170-2493): fixed op list: ByteSequenceRead/Predicate/Equal/Length/Subslice, ElementViewLength/Subslice/Read, IntegerArithmetic, IntegerBitwiseAnd/Or/Xor/Not, IntegerShift, IntegerExactCast, TrappingInteger, BooleanEqual, BooleanNot, IntegerEqual/Less/LessOrEqual; `_ => unsupported` (2492). The fast path at 2178-2182 handles only `IntegerOverflowPolicy::Exact` with `Add|Subtract`; every other arithmetic goes through `try_lower_integer_operation` (2226).
- `primitive_storage::lower` (2921-3299): six operation kinds (StructuralByteSequenceFieldLength/Read, StructuralScalarFieldRead, EstablishPrimitiveLocal, PrimitiveLocalStore, StorageAddress, PrimitiveScalarRead); `_ => unsupported` (3299).
- `records::establish` (3359-3407): `(Record, no case)`, `(Sum, Some case)`, `(Mixed, Some case)`, `(FixedArray with Literal length == elements.len(), None)`; anything else `_ => unsupported` (3406). Quoted structure; no explanatory comment.
- `atomic_access::lower` (523-603): 7 `AbstractAtomicEvent` kinds; `Fence` handled before and a second `Fence` arm is an error (600-602).
- `records::establish_trivial_affine_local` (3646): record with zero fields only.

### Same job elsewhere
- "Is this value u64 unsigned non-address" repeated inline at control_flow_storage.rs:1199, 1328, 1617, 1766, 2689, 2929 and in S02/scalar.rs:233, 326-337, 1581, 1657, 1742, 2432 (read). The one named helper is `scalar::expressions::unsigned_u64_endpoint` (scalar.rs:2420), used only by view subslice endpoints (scalar.rs:573, 1939-1952, 2071).
- "Scalar -> ValueShape": `primitive_storage::native_shape` (2774), `scalar_abi::fixed_native_scalar_shape` (function_signature.rs:654), `scalar::scalar_shape` (scalar.rs:2163), and the inline `match` in `prepare_function_signature` (function_signature.rs:81-90) all do it (read).
- "Place -> (structural type, access)" lookup is done by several functions: `primitive_storage::field_root` (2817), `aggregate_results::value_input` (290), `references::source_contract` (references.rs:497), `references::direct_storage_type` (516), `references::referent_projection` (203), `scalar::descriptor_operands::descriptor_projection_access` (scalar.rs:786) (all read).
- Length-measurement witness scan (see above) appears 4 times here plus `descriptor_operands::observes_count` (scalar.rs:913) and `observes_measured_length` (949).
- Suspected elsewhere (not read): S03 `legalization_*` re-derives the same checks from the optimized unit (see S03 section below).

### Unused / one-caller / "single shared" claims
- `pub(crate) fn is_primitive_reference` (2752): 3 call sites (control_flow_calls.rs:97, 181, 1698) by `Grep` of `is_primitive_reference`.
- `aggregate_borrows::is_reference` / `::argument`: 1 call site each (control_flow_calls.rs:1676-1677) by `Grep` of `aggregate_borrows::`.
- `primitive_storage::native_shape` etc. are used inside this file; not counted individually.
- No comment in this file claims a canonical/shared status.

### Stated limits
None (grep of TODO/FIXME/migration/for now/not yet/temporar/legacy/workaround in this file: no matches). Limits appear only as refusals.

### Fail-open risks
- `observations::lower` first `match` ends `_ => {}` (2108) and the `ByteSequenceLength|...Subslice` arm has inner `_ => {}` (2101): not accept paths. Traced: control then reaches the second `match` and falls to `_ => return Err(unsupported)` (2492) for any op not listed. Fail-closed.
- `observations::scalar_values` (2613-2616): a block parameter of `IeeeFloat` type is `continue`d, i.e. not added to `values`; any later lookup of that value yields `UnknownValue`. Open (did not trace float block parameters).
- `byte_write::replace` 1320: `function.arrival_parameter(*source).is_some() || live.views.contains_key(source)` is the only requirement on the source place besides `lengths` link (1319). A source that is an arrival parameter of any kind passes here; the carrier/access check for the source is not made in this function (later resolved by `resolve_byte_view` elsewhere). Open.
- `aggregate_results::value_input` (355-369): a borrowed arrival (`access != Owned`) is accepted as a value input if path empty and a view descriptor parameter with matching observation type; stated by the error text "a borrowed arrival can supply only its live whole view descriptor" (364-366). Stated.
- `records::establish` (3569): when `reference_bearing && !descriptor_member` leaf relocation runs; for `descriptor_member` field the reference leaves are not relocated (no `subtree_leaves`). Reason not given at the site. Open.
- None other seen.

---------------------------------------------------------------------------

## 2. S02/coordination.rs (645 lines)

### Purpose
Whole-plan entry for lowering `AbstractOperationPlan` to `TargetOperationPlan`: validates that provider-installation evidence, boundary settlements and native callback arguments line up with the boundary calls in the plan, derives per-function ABIs, builds the reference catalogs, then calls `lower_function` for each function. Header `coordination.rs:1-11`.

### Shape
- `lower_to_target_operations_with_settlements_and_installation` (479): the only top-level function.
- `mod installed_provider_calls` (26-225): types `BoundaryCallKey` (36), `InstalledCallsByCall` (38), `BoundaryCallsByKey` (40); fns `index_installed_provider_calls` (43), `index_boundary_calls` (74), `validate_installed_provider_calls` (106).
- `mod native_callbacks` (226-395): `bind_native_callback_arguments` (235), private `native_callback_application_is_exact` (306).
- `mod settlement_roster` (396-477): `SettlementsByBoundary` (405), `index_settlement_bindings` (407), `validate_settlement_roster` (430).

### Inputs and outputs
Inputs: `AbstractOperationPlan`, `NativeTarget`, `&[BoundarySettlementBinding]`, optional `&dyn ProviderInstallationEvidence`, admitted native callbacks, optional `AdmittedLoweringFacts`. Output: `TargetOperationPlan { borrowed_storage_layouts, psi, target, entry, native_callback_arguments, functions }` (587-643). Side effects: stderr prints gated by env vars `OMEGA_TRACE_NATIVE_BOUNDARY` (548) and `OMEGA_COMPILER_CHECK_SITES` (618).

### How it decides
- Boundary calls are keyed by the triple `(MachineId, InvocationSite, BoundaryMachineId)` (36, 81-99). Installed evidence must match scalar and structural argument lists, completion claim sources and receipts by equality (200-215) and the provider must be in `plan.provider_candidates` (211-214).
- Result shape agreement is a `match` over `(AbstractBoundaryResult, OperationResult)` (163-177) and `(AbstractBoundaryResult, BoundaryMachineResult)` (178-199).
- Settlement roster (430-476): settlements for boundaries that appear in installed evidence are an error (439-446); a boundary call that is not in `installed_by_call` requires a settlement (460-468); a settlement with no remaining call is an error (469-474).
- Native callbacks: uniqueness by `(terminal_operation, placement_index)` (248-260) by O(n^2) scan; exactly one matching `BoundaryCall` in the plan (261-273); `registrar_application_commitment == [0;32]` is invalid (277).
- FMA: any `NearestIeeeFloatFusedMultiplyAdd` in the plan returns `UnsupportedIeeeFloatFmaRealization(min op id)` (522-533).

### Cases by shape
`ExternalCompletion` is a separate branch in `validate_installed_provider_calls` (112-139) requiring unit result, empty claim sources/receipts, `BoundaryCompletion::External(*kind)`. Everything else must be a `BoundaryCall` (140-154).

### Same job elsewhere
- The callee/provider selection logic here (`index_boundary_calls`, `validate_installed_provider_calls`) and `lib.rs::selected_boundary_calls::index` (lib.rs:538-700) walk the same boundary calls twice (read both). In S03, `legalization_descriptor_inputs.rs:94-154` (`preserves_boundary`) repeats the descriptor-input guard check that `coordination.rs:1550-1578` (inside the closure passed to `abstract_reference_catalog_graph_with_boundary_preservation`) does (read both).

### Unused / one-caller
- `lower_to_target_operations_with_settlements_and_installation`: 1 call site (lib.rs:443) by `Grep` across `omega-rust`.

### Stated limits
`UnsupportedIeeeFloatFmaRealization` rejection of all FMA (522-533) has no comment; it is the stated limit as an error variant (model.rs:47).

### Fail-open risks
- `index_installed_provider_calls` (47-55): `installation == None` yields an empty set (`unwrap_or_default`). Traced: with no evidence, every boundary call needs a settlement via `validate_settlement_roster` (460-468), so absent evidence is not an accept path.
- Debug closures (`inspect_err`, 547-574, 617-636) only print; they do not change results.
- None other seen.

---------------------------------------------------------------------------

## 3. S02/function_signature.rs (826 lines)

### Purpose
Turns an `AbstractFunction`'s scalar and structural parameters and result into an ABI calling plan (`PreparedFunctionSignature`), derives the two cached ABI forms (`ScalarFunctionAbi`, `MixedStructuralScalarFunctionAbi`), captures native borrowed-storage layouts (`NativeBorrowedStorageLayout`) for boundary buffers, and defines `StructuralTypeLookup`. Header comment 1-10. Despite the file name it hosts four inline modules re-exported by `lib.rs:50-52`.

### Shape
- `integer_parameters` (24), `PreparedFunctionSignature` (50; fields `call_plan`, `scalar_parameters`, `parameters`), `parameters_by_place` (64), `prepare_function_signature` (73).
- `mod borrowed_storage` (161-520): `capture` (175), `capture_type` (232), private `StorageRecipes` (270) with `collect` (278), `collect_members` (376), `inline_byte_geometry` (472).
- `mod scalar_abi` (522-660): `derive_mixed_structural_scalar_function_abi` (532), `derive_fixed_scalar_function_abi` (568), `fixed_native_scalar_shape` (654), `fixed_native_integer_shape` (657).
- `mod structural_signature` (662-767): `ParameterLane {Internal, Foreign}` (677), `StructuralCallSignature` (682; `parameter_shape` 705, `derive` 728, `plan` 758), `StructuralCallPlan` (688).
- `mod structural_type_lookup` (769-826): `StructuralTypeLookup { layouts, catalog }` (777), `subtree` (794).

### Inputs and outputs
Reads `AbstractFunction`, `NativeTarget`, `StructuralTypeLookup`, `calling_conventions::evaluate_call_plan`, `structural_layout::*`. Outputs `PreparedFunctionSignature`, ABI structs, `Vec<NativeBorrowedStorageLayout>`.

### How it decides
- Parameter shape by `StructuralAccess`: owned = referent shape; any borrow = borrowed reference, and the internal lane gives `borrowed_scalar_reference` when `scalar_storage_type` is `Some` (712-725). `ParameterLane::Foreign` is used by the foreign-call path in unit.rs.
- `derive_fixed_scalar_function_abi` is skipped (`Ok(None)`) when there are structural parameters or entry claims (572-574), or result is Structural (583) or any scalar has no fixed native shape (594-596).
- `capture` requests layouts for (a) every boundary structural parameter and result type (179-194), and (b) only when any boundary parameter has `reading_borrow_pair_pattern!()` access, every owned function/block parameter type and every materialized structural result type of every function (195-225).
- `collect` (278-374): `FixedArray` needs literal length and `stride`; `Sum|Mixed` is rejected (`return None`) if any relevant field has authored `storage` (317-323); fragmented fields rejected (408-413).

### Cases by shape
`StorageRecipes::collect` match over `StructuralTypeShape` variants (287-372): PrimitiveScalar, AtomicCell, Reference|ByteSequence|ElementView (opaque), Record, Sum|Mixed, FixedArray. `capture_type` returns `Option` and silently drops a type that fails (228: `filter_map`). `native_tag` for sum cases is the case ordinal (330-333).

### Same job elsewhere
- Scalar-to-`ValueShape` mapping repeated 4 ways (see section 1).
- `derive_fixed_scalar_function_abi` (568-652) and `prepare_function_signature` (73-159) each run `evaluate_call_plan` and each build `ScalarAbiValue` rows from `call_plan.parameters` with the same shape-equality check (`placement.shape != expected_shape` at 125 and 635).

### Unused / one-caller
- `integer_parameters`: 1 call site (control_flow.rs:2808) by `Grep`.
- `parameters_by_place`: 1 call site (control_flow.rs:2576) by `Grep`.
- `structural_layout::root_array_projection_metadata`: 1 call site (function_signature.rs:360) by `Grep`.

### Stated limits
None (grep no matches).

### Fail-open risks
- `derive_mixed_structural_scalar_function_abi` (546-552): `prepare_function_signature` failure is converted to `Ok(None)` (no ABI, error discarded). Traced as far as this file: the function then simply has no mixed-scalar ABI recorded (coordination.rs:638-639 stores `None`); whether another path reports the same error was not traced. Open.
- `borrowed_storage::capture_type` (232-252) / `capture` (228) drop a root whose recipe cannot be built (returns `None`) rather than failing; the consumer `NativeBorrowedStorageLayout` list is therefore a subset. Open (did not trace consumer behaviour for a missing layout).
- `structural_type_lookup::subtree` (794-816) returns `None` for unknown segment kinds - not accept.

---------------------------------------------------------------------------

## 4. S02/lib.rs (701 lines)

### Purpose
Crate root for stage 02. Declares modules, re-exports, and hosts five inline modules: `boundary_settlements`, `function`, `optimized`, `provider_evidence`, `selected_boundary_calls`. Public entry: `lower_optimized_to_target_operations` (lib.rs:417). The header comments (3-39) state the stage's role.

### Shape
- Module declarations 41-48; `pub mod access_view_inputs {}` (41) is empty.
- `boundary_settlements::claim_completion_only_boundary_is_exact` (68): bool predicate for `ClaimCompletionOnly` boundary calls.
- `function::lower_function` (233) plus private `native_boundaries::unsupported_scalar_call` (192).
- `optimized::ValidatedOptimizedTargetOperations` (291; fields `optimized`, `lowering_facts`, `current_program: Arc<TargetOperationPlan>`, `installed_stack_requirements`, `installed_entry_execution`, `provider_installation`), `OptimizedTargetLoweringRequest` (411), `lower_optimized_to_target_operations` (417), builder-ish `with_installed_entry_execution` (302), `with_installed_stack_requirements` (324).
- `provider_evidence::bind_provider_executions` (468).
- `selected_boundary_calls::{SelectedBoundaryCall, SelectedCallsByCall, index}` (526, 535, 538).

### Inputs and outputs
`lower_optimized_to_target_operations` takes `OptimizedAbstractProgram`, `AdmittedLoweringFacts`, request {target, settlements, installation}; refuses if any optimization selection remains for `OptimizationExecutionPhase::TargetOperations` (427-430), or if callback/interrupt cohorts' `psi` differ from the plan's (431-436); calls `bind_provider_executions` then `coordination::lower_to_target_operations_with_settlements_and_installation` (442-450); returns `ValidatedOptimizedTargetOperations` with the plan in an `Arc`.

### How it decides
- `claim_completion_only_boundary_is_exact` (68-174): requires exactly one structural argument per declared parameter, each `Owned`, no path, `Linear` on both sides, equal `structural_type`, caller parameter `qualifications`/`projected_qualifications` equal to boundary-declared plus `requires` (sorted+dedup, 126-129); `completion_claim_sources` must equal exactly `function.entry_claims` mapped (145-157); receipts must be the set-equal of `(argument_index, claim)` (159-173). Keyed on place/ids/set equality.
- `unsupported_scalar_call` (192-230): a boundary call with scalar `arguments` is refused unless selected via `selected_calls`, or its settlement realization is one of `HostedExitProcessI32 | HostedWriteByteI32 | NearestIeeeFloatArithmetic | AccessorScalarAccess` or `NormalizedForeignCall` (213-227).
- `selected_boundary_calls::index` (538-700): chooses the callee/origin per boundary call key: program-entry (`entry_call`, 601-621), installed provider (624-633), target-machine family (634-673). `(Some,Some)` conflicts are errors (623); `(None, None) => continue` (674).
- `bind_provider_executions` (468-514): maps `AdmittedBoundaryExecution::{Provider, CompilerBuiltin}` to bindings; zero identities reject.

### Cases by shape
Quoted: the realization whitelist at 213-227 ("HostedExitProcessI32 | HostedWriteByteI32 | NearestIeeeFloatArithmetic | AccessorScalarAccess"). `with_installed_stack_requirements` requires power-of-two alignment, `required_bytes().is_some()`, and an admitted entry for every row (344-356).

### Same job elsewhere
`index_boundary_calls` (coordination.rs:74) and `selected_boundary_calls::index` (lib.rs:538) both iterate every `BoundaryCall` / `ExternalCompletion` in every function. `ValidatedOptimizedTargetOperations::boundary_origins` (378) constructs `BoundaryOriginFacts` that S03 `legalize_target_operations` consumes (S03/legalization.rs:408-420).

### Unused / one-caller
- `pub mod access_view_inputs {}` (lib.rs:41) is an empty public module. `Grep` of `access_view_inputs::` in `omega-rust`: one hit (psi `06` `retention::access_view_inputs`, an unrelated path). Zero users of this empty module found.
- `SelectedBoundaryCall`/`SelectedCallsByCall` used in control_flow* via `selected_boundary_calls::resolve` (referenced at references.rs:1545 as `super::super::selected_boundary_calls::resolve`) - note: `resolve` is NOT defined in lib.rs (my read of 517-701 shows only `index`); it must be defined elsewhere in the crate (not in my unit). Suspected, not read.

### Stated limits
None by comment.

### Fail-open risks
- `selected_boundary_calls::index` 640-642: `let TargetMachineCallable::Machine(callee) = member.callable() else { continue; };` skips the call (leaves it unselected). Traced: unselected calls need a settlement via `validate_settlement_roster` (coordination.rs:460-468) so an unsettled call errors `MissingBoundarySettlement`. Traced.
- 674 `(None, None) => continue`: same trace.
- None other seen.

---------------------------------------------------------------------------

## 5. S02/model.rs (326 lines)

### Purpose
`LoweringError` (the one error enum for stage 02), two helper error enums, and re-exports of provider-admission types. Header 1-5.

### Shape
`LoweringError` (17-283; ~140 variants), `AccessCallCustodyError` (286), `AccessViewInputTranslationError` (293) with `From` into `LoweringError` (308-312), `diagnostics::debug_display!(LoweringError)` (314), `LoweringError::unsupported_control_flow` (320-325; `#[track_caller]`, records `optimization_core::CompilerCheckSite::caller()`).

### How it decides
Nothing; data only. `UnsupportedControlFlow { machine, site }` (139-142) is the generic refusal and the call-site file/line is captured via `track_caller` (319-324), so the refusal reason is the Rust source line, not a message.

### Unused code
`Grep` for `LoweringError::<Name>` across `omega-rust` (uncapped, content mode) found no construction outside model.rs for these variants: `MultipleNativeCallbackArguments`, `MissingNativeCallbackArgument`, `ConditionalControlFlowRequiresBlockLowering`, `ConditionalConditionMustBeBoolean`, `ConditionalArmBindingTypeMismatch`, `HostedExitProcessUnsupportedTarget`, `UnitFunctionNotStraightLine`, `UnitOperationInScalarFunction`, `RelevantOpaqueStructuralField`, `FunctionHasNoReturn`, `OperationAfterReturn`, `FunctionResultMismatch`, `FunctionResultKindMismatch`, `UnsupportedRuntimeBooleanCondition`, `IeeeFloatFmaOperandMismatch`, `InstalledProviderClaimTransferMismatch`. (Search pattern did not include `Self::` or `use LoweringError::*` forms; "no `LoweringError::X` construction found" is what was verified. I did not search the other stage's own `LoweringError` types; `UnsupportedStructuralReturn` and `UnsupportedStructuralByteSequence` hits are in stage `00`, a different crate.) Partly used: `InvalidInstalledInterrupt` (control_flow_operations.rs:960, 970; control_flow.rs:351), `AccessDynamicCallRequiresBinding` (control_flow_calls.rs:1193).

### Stated limits / Fail-open
None.

---------------------------------------------------------------------------

## 6. S02/references.rs (2686 lines)

### Purpose
Tracks, through control flow, where every reference descriptor ("a stored borrow") points: which storage root, which projection, which access, and for access-view references the "captured/formal" interpretation. Used so later lowering knows, for a place that holds a reference, which owning storage it borrows (`ReferenceStorage`), including across calls (stored-reference effects), copies, joins and block arrivals. Header 1-9 says it does so "without re-running source borrow checking".

### Shape
- Top-level types: `ReferenceStorage { root: ReferenceRoot, carrier_type, referent, placed: Option<TargetAccessInterpretation> }` (189), `ReferentProjection { carrier_type, referent, carrier_path, path }` (196).
- `mod borrowed_arrivals` (30-182): `BorrowedSources = Arc<BTreeMap<PlaceId, Option<Vec<BorrowedSource>>>>` (34), `BorrowedSource` (38), `borrowed_source_catalog` (45), `capture` (68), `merge` (139), `storage_sources` (158). Re-exports at 183-186.
- Free functions: `referent_projection` (203), `window_projection` (249), `reference_call_argument` (292), `contains_reference` (369), `leaf_paths` (373), `referent` (459), `leaf_referent` (475), `leaf_type` (487), private `source_contract` (497), `direct_storage_type` (516), `reference_source_type` (547), `storage_projection` (585), `reference_leaf` (621), `reference_leaf_following` (631), `access_source` (653), `access_formals` (677), `access_call_custody` (702), `normalized_source` (788), `normalized_direct_source` (833), `access_covers` (888), `establish` (1971), `establish_access` (2064), `retain_access_result` (2157), `release` (2187), `discard_owned` (2212), `discard_subtree` (2228), `window_descriptors` (2252), `window_storage` (2295), `subtree_leaves` (2311), `relocate_leaves` (2350), `boundary_result_references` (2365), `call_results` (2401), `combine_alternatives` (2617), `combine_candidate_origins` (2636), `entry` (2656), `moved_argument_leaves` (1938), `commit_argument_moves` (1962).
- `mod copies` (898-1104): `storage_regions` (901), `storage_cursor` (984), `copied_leaves` (1020).
- `mod observations` (1105-1299): `through_resident` (1112), `observe` (1149), `sources_at_storage` (1267).
- `mod arrivals` (1301-1328): `merge_arrivals` (1304).
- `mod stored_calls` (1330-1934): `StoredEffects` (1352), `StoredEffectCatalog = Arc<StoredEffects>` (1366), `catalog` (1477), `Staged` (1721), `stage` (1725), `commit` (1925).

### Inputs and outputs
Reads `LiveDefinitions` (`references`, `structural_homes`, `views`, `borrowed_reference_sources`, `access_formals`, `stored_reference_effects`, `reference_position`, `retired_reference_roots`), `AbstractFunction`s, `StructuralTypeLookup`, and descriptor analysis from `terminal_semantics::descriptor_results` / `optimization_unit_semantics::descriptor_results`. Produces `ReferenceStorage` rows in `live.references` keyed by `(PlaceId, Vec<StructuralPathSegment>)`, `TargetUnitOperation::{EstablishReference, EstablishAccessReference, ReleaseReference}`, `TargetReferenceResult` rows for call results, and the catalogs.

### How it decides
- Reference custody keyed on exact `(carrier PlaceId, path)` coordinates (639-640; 1949-1953) with prefix stripping via `strip_prefix` and `Referent`-segment exclusion (113, 1953, 2247, 2337).
- Joins: `merge_arrivals` (1304-1327) requires equal `carrier_type` and `referent`; differing roots are combined into `ReferenceRoot::Alternatives` (1320-1323; `combine_candidate_origins` 2636-2654 sort+dedup). `combine_alternatives` also requires equal `placed` (2627).
- `borrowed_arrivals::merge` (139-156): a place missing on one side becomes `None` (unknown) - which later makes `normalized_source` refuse (797-800).
- Access admission via `access_covers` (888-896): equal, or authority `Owned`, or `MutableBorrow` covering `SharedBorrow|WriteOnlyBorrow`.
- Stored-reference catalog (1477-1719): candidates are functions with writable reference slots (1485-1489). Graph unresolved -> all candidates stay `Err("stored-reference catalog graph could not be reconstructed")` (1499, 1580-1585). Per-origin conversion refuses Invocation, IncomingSlot/IncomingContents, Literal, AccessFormation origins (1692-1696) and any non-exact projection (1661-1667).
- `stage` (1725-1923): dynamic call (no callee) is refused if any argument has writable reference slots (1742-1756).
- Equality of descriptors with `TargetAccessInterpretation` (`placed`) is decided by `agree`/`reconcile` from `optimization_unit_semantics` (15-16, 2598).
- Budgets: `leaf_paths` capped at `maximum_leaves` (4096 at 2374, `4096 - references.len()` at 2668); `storage_regions` budget = `references.len().max(1) * path.len().max(1)` (928-933).

### Cases by shape
- `leaf_paths` (373-457): handles `Reference`, `Record|Sum|Mixed`, `FixedArray` (literal length only, `length.literal()?` 437); other shapes `_ => {}` (453).
- `reference_call_argument` (292-367): refuses `Owned`, access mismatch, non-`Unrestricted` parameter multiplicity, `Linear` or claim-bearing homes (301-315), `WriteOnlyBorrow` parameter (322).
- `establish` (1971-2062): requires `Affine` multiplicity (1987) and three compatibility cases keyed on whether `storage` is `Some` and whether the last path segment is `FixedRange` (1996-2031).
- `establish_access` (2064-2155): only `SharedBorrow|MutableBorrow` both sides (2087-2095); exactly one `access_views()` plan whose `carrier == referent` (2106-2112).

### Same job elsewhere
- `references::observations::observe` (1149) and `copies::storage_regions` (901) and `window_descriptors` (2252) each resolve a reference leaf by first checking `live.references`, then calling `observe` (955-958, 1170, 2281); same fallback in three places (read).
- `references::leaf_type` (487) vs `StructuralTypeLookup::subtree` (function_signature.rs:794): both call `terminal_semantics::runtime_structural_path_tip`; `leaf_type` additionally falls back to `primitive_projection_declaration` (494).
- `normalized_direct_source` (833) vs `normalized_source` (788): two-level path normalisation.
- Suspected duplicate outside unit (not read): S03 `legalization_reference_custody.rs` (13 hits of the CHECK_SITES env name; 3 mentions of `DescriptorInputs`) re-derives reference custody for audit. `Grep` for `OMEGA_COMPILER_CHECK_SITES` shows 13 lines there.

### Unused / one-caller / "single shared"
By `Grep` of names within the S02 directory:
- `moved_argument_leaves`: 2 call sites (control_flow_calls.rs:1226, control_flow_operations.rs:328).
- `commit_argument_moves`: 1 (control_flow_calls.rs:1279). `stage_stored_effects`: 1 (control_flow_calls.rs:1227). `commit_stored_effects`: 1 (1280).
- `boundary_result_references`: 2 (control_flow_operations.rs:1146, control_flow.rs:771). `discard_owned`: 2 (control_flow.rs:1300, 1310).
- `merge_arrivals`: 3 uses in control_flow.rs (2509, 2873, 3070); `merge_borrowed_arrivals`: 3 (2508, 2878, 3071); `capture_borrowed_arrivals`: 1 (1885).
- No comment calls anything canonical/shared.

### Stated limits
No TODO-style comments (grep none). Limits are only the error strings: "stored-reference invocation origin has no physical metadata conversion" (1692), "captured incoming contents have no physical metadata conversion" (1694), "stored-reference literal origin has no physical metadata conversion" (1695), "stored-reference access formation has no physical metadata conversion" (1696), "dynamic call has no stored-reference effect summary" (1753).

### Fail-open risks
- `stored_calls::stage` 1758-1760: `let Some(effects) = live.stored_reference_effects.machines.get(&callee.machine) else { return Ok(staged) }` - callee with no entry means "no effects". Traced: `catalog` inserts an entry (Ok or Err) for every candidate with writable slots (1490-1503), so a missing entry means the callee has no writable reference slot. Traced, reasonable.
- `stage` 1818-1827: a `Source::Incoming` slot with no live provenance is skipped (`continue`) only if the slot path contains a `Case` segment; otherwise error. If all sources are skipped, `replacement = None` and `commit` removes the destination's descriptor (1915-1920, 1925-1932). What gets through: a call whose incoming reference lives in a sum-case path can silently delete the destination's tracked provenance instead of failing. Open (no comment; I did not trace later use of the missing entry; `observe` may recompute).
- `call_results` 2534-2540: same `Case` skip (`continue`) when a transferred leaf is absent. Open.
- `discard_subtree` 2235-2237: `source_contract` None -> `Ok(false)` (nothing discarded). `discard_owned` caches retirement (2218-2224). Open: a place with no contract is treated as "no references to discard".
- `release` (2200-2202): refuses if there is no entry. Fail-closed.
- `observations::observe` 1170-1176: a resident leaf is reused only if `carrier_type`/`referent` equal, else `None`. Fail-closed.
- None other seen.

---------------------------------------------------------------------------

## 7. S02/scalar.rs (2900 lines)

### Purpose
Lowers scalar-valued abstract operations and resolves "view descriptors" (borrowed byte slices and typed element views) to concrete `TargetByteView` / `TargetElementView` values. Also hosts the small algebra of "known" scalar values (`KnownScalar`, `KnownInteger`) used to fold constants or build target expressions. Header 1-8.

### Shape (inline modules)
- `byte_views` (9-738): `lower_byte_observation_with_lengths` (128), `resolve_byte_view` (367), `owned_byte_result` (623), `completed_descriptor` (655), `descriptor_result` (688), `validate_whole_operand` (706), predicates `is_immutable_byte_parameter` (53), `is_immutable_byte_carrier_parameter` (61), `is_byte_parameter` (79), `byte_view_referent_size` (89), `is_owner_loan` (107), `is_owned_byte_parameter` (117), `block_source` (31).
- `descriptor_operands` (739-1062): `descriptor_projection` (756), `descriptor_metadata_projection` (771), private `descriptor_projection_access` (786), `observes_count` (913), `observes_measured_length` (949), `measured` (984), `storage_backs_view_parameter` (993), `refusal` (1047; `#[track_caller]`, env-gated stderr print, returns `UnsupportedOperationInScalarFunction`).
- `element_views` (1063-2149): `establishment::source` (1094), `ElementViewBackings` (1424; `homes`, `views`, `address_joins`, `selectors`) with `view_result`, `address`, `descriptor`; `is_element_view_parameter` (1528), `lower_element_observation_with_lengths` (1542), `float_element_read` (1698), `element_leaf_offset` (1761), `scalar_leaf_offset` (1777), `resolve_element_view` (1820), `live_aggregate_backing` (2119).
- `expressions` (2150-2438): `scalar_shape` (2163), `scalar_parameter_location` (2187), `insert_value` (2216), `KnownScalar` (2228), `negate_boolean` (2258), `equal_boolean` (2278), `equal_integer` (2327), `order_integer` (2357), `KnownInteger` (2403), `unsigned_u64_endpoint` (2420).
- `integer_binary` (2439-2614): `IntegerBinaryKind` (2449), `lower_integer_binary` (2455).
- `integer_operation` (2615-2830): `try_lower_integer_operation` (2623).
- `shift` (2831-2892): `lower_shift` (2840).
- Re-exports 2894-2900.

### Inputs and outputs
Inputs: `AbstractOperation`, `AbstractFunction`, `StructuralTypeLookup`, target parameters, `ElementViewBackings`, a `values: BTreeMap<ValueId, KnownScalar>` map and `lengths: BTreeMap<ValueId, PlaceId>`. Outputs: mutate `values` (via `insert_value`, which errors on a duplicate id 2216-2224), push to `provenance`, return a `TargetByteView`/`TargetElementView` plus access.

### How it decides
- `resolve_byte_view` (367-621) tries, in order: descriptor result from a completed home (386-408), owned byte result (409-419), byte literal (420-435), `EstablishElementView` destination (436-468), arrival parameter (469-537), `ByteSequenceSubslice` producer (538-621) - each found by scanning `function.operations` for matching `place`/`destination`. Keyed on `PlaceId` equality and shapes.
- `resolve_element_view` (1820-2117) analogous ordering: completed descriptor result, `EstablishElementView`, block parameter, function parameter, `ElementViewSubslice`.
- Measured-length proof: `observes_measured_length` (949-982) requires `values[length]` to be a 64-bit unsigned runtime `ByteSequenceLength`/`ElementViewLength` expression whose source place == the view place and path empty (984-991), or a `ScalarHome` whose `lengths[length] == source` (975-979). `observes_count` (913-947) instead scans `function.operations` for a `ByteSequenceLength`/`ElementViewLength` operation with `same_descriptor_coordinate`.
- Integer rules: exact add/sub use `admits_exact_add_operands`/`admits_exact_subtract_operands` (2474-2484); all others require both operand types equal the result type (2483). Proof-bearing arithmetic (`arithmetic.obligation().is_some()`) is never constant-folded (2488-2494, 2607-2612). Wrapping shifts fold when both immediates (2866-2882).
- Access of a byte view result: from `result_access` (406), or from the arrival parameter (476-488), or from the subslice's source access and result multiplicity (594-607).

### Cases by shape
- `byte_views::byte_view_referent_size` (89-105): `BorrowedView` -> 16, `BoundedView` -> 8, `BoundedOwned{capacity}` -> capacity+8, else `None`. Hard-coded sizes.
- `lower_byte_observation_with_lengths` (138-303): handles `ByteSequenceSubslice`, `ByteSequencePredicate`, `ByteSequenceEqual`, `ByteSequenceLength`, `ByteSequenceRead`; else `Ok(false)` (303).
- `byte_views::lower...` Length for `FixedArray` selected yields an immediate (242-261); `BoundedView{capacity}` yields immediate (270-272); other carriers a runtime expression with `length_byte_offset` 0 for `BoundedOwned`, 8 otherwise (278-285).
- `element_views::establishment::source` (1094-1420): `WholeCollection`, `WholeRepresentation`, `RepresentationBytes` geometry; several explicit refusals, e.g. `byte_window.is_some()` with representation views (1267, 1307), `LiveLength` with representation views (1273-1276).
- `descriptor_operands::descriptor_projection_access` accepts `WriteOnlyBorrow` only when `metadata` is true (800-808) and refuses a `Referent` path through a write-only source (809-816).

### Same job elsewhere
- `resolve_byte_view` (367) and `resolve_element_view` (1820): same descriptor-resolution role for two carrier families, written separately; `resolve_byte_view` delegates to `resolve_element_view` for `EstablishElementView` destinations (450-459).
- `completed_descriptor`/`descriptor_result` at scalar.rs:655/688 vs S03/legalization_scalar_graph.rs:2227/2252 and S03/selection_inputs.rs:2878/2917 (grepped names; the two S03 definitions NOT read: suspected parallel implementations of the same predicate over a different input type).
- `runtime_borrowed_projection`: S02/structural_layout.rs:1076 vs S03/structural_inputs.rs:2061 (grepped; S03 side not read; suspected same job over the optimized unit).
- Unsigned-64-bit checks repeated (see section 1).

### Unused / one-caller
By `Grep`: `storage_backs_view_parameter` 1 caller (control_flow.rs:2180); `byte_view_referent_size` 1 caller (scalar.rs:518, in-file); `is_owned_byte_parameter` 1 caller (control_flow.rs:2638); `is_immutable_byte_parameter` 1 caller (scalar.rs:42, in-file; also called from `is_immutable_byte_carrier_parameter`? no - the latter does not call it); `is_element_view_parameter` callers in control_flow_calls.rs (16, 491, 528, 944, 1712) and control_flow.rs (14, 2636); `completed_descriptor` 2 callers (scalar.rs:697, control_flow_calls.rs:497); `descriptor_result` 2 callers (scalar.rs:387, 1840).

### Stated limits
None (grep none).

### Fail-open risks
- **Dead cycle guard:** `resolve_byte_view` and `resolve_element_view` take `active: &mut [PlaceId]` and test `active.contains(&place)` (383, 1836) to stop cycles, but every caller passes `&mut Vec::new()` (e.g. 152, 314, 1566, 1630) and nothing in scalar.rs ever pushes to `active` (`Grep` of `active.push|active.pop` in scalar.rs: 0 matches). So the guard cannot trigger. Traced: neither function recurses into itself (byte -> element only, element -> neither), so no infinite loop exists today; the guard is vestigial. Traced.
- `observes_count` / `observes_measured_length` return `false` (refuse) on every unknown; no accept path from `None`.
- `resolve_byte_view` 538-540: if no subslice producer found -> refusal with context tuple. Fail-closed.
- `lower_byte_observation_with_lengths` for `ByteSequenceSubslice` (138-156) only resolves and records provenance; the subslice's own checks are in `resolve_byte_view` (541-607). Traced.
- None other seen.

---------------------------------------------------------------------------

## 8. S02/structural_layout.rs (1322 lines)

### Purpose
Computes byte offsets, shapes and runtime index "runs" for walking a path of `StructuralPathSegment`s through a structural type: through fields, sum cases, fixed arrays, element views, byte sequences, and across references. It wraps `calling_conventions::terminal_layout::StructuralLayouts` (size/alignment tables) and maps its errors to `LoweringError` (15-34). Header 1-9.

### Shape
- Types: `StorageProjection` (42), `PrimitiveProjection` (47) with `selector_runs` (54), `ReferenceProjectionRegions` (145) with `selector_runs` (153), `ProjectedStorage { Declaration, Field }` (818) with `declaration` (824), `shape` (849), `RuntimeProjection` alias (994).
- Functions: `reference_referent_projection` (64), `primitive_leaf_projection` (115), `reference_projection_regions` (165), `contiguous_primitive_projection` (236), `primitive_read_projection` (295), `contiguous_primitive_read_projection` (311), `mod scalar_fields` (339-541: `static_atomic_geometry` 346, `static_field_geometry` 376, `direct_scalar_field_offset` 433, `direct_scalar_field_geometry` 461), `root_array_projection_metadata` (547), `structural_shape` (567), `scalar_storage_type` (574), `structural_sum_layout` (581), `structural_field_storage_shape` (590), `structural_field_offset` (613), `structural_field_shape` (629), `field_geometry` (639), `byte_sequence_field_geometry` (705), `bounded_byte_field_geometry` (729), `borrowed_view_field_offset` (745), `case_projection` (759), `resolve_structural_projection_path` (779), `storage_projection_path` (799), `runtime_storage_projection` (875), `runtime_descriptor_projection` (1002), `runtime_projection` (1028), `runtime_leaf_copy_projection` (1050), `runtime_borrowed_projection` (1076), `storage_address_indices` (1105), `runtime_projection_with_leaf_type` (1127), `byte_sequence_shape` (1172), `checked_align_up_u32` (1180).
- Tests (`#[cfg(test)] mod placed_geometry_tests`, 1184-1322): two unit tests (1194, 1248).

### Inputs and outputs
In: root `StructuralTypeId`, a path slice, `StructuralLayouts`, sometimes expected type and requested `StructuralAccess`. Out: `(selected type, ValueShape, byte offset, Vec<(ValueId, stride)>)` where the vector is the runtime-index runs (a `RuntimeIndex{index}` segment pushes `(index, stride)` at 972-974).

### How it decides
- `runtime_storage_projection` (875-992) walks segment by segment; per-segment: `Case` consumes the next `Field` segment (911-913); `AllElements`, `RepresentationView`, `Referent`, `FixedRange` are refused (896-934); `FixedIndex >= literal length` refused (951-954); stride is `align_up(size, alignment)` or an authored repeated stride (961-966).
- `reference_projection_regions` (165-233) splits the path at `Referent`/`FixedIndex`/`RuntimeIndex` crossings that land on a reference (size 8|16, alignment 8, 199) or a view carrier (size 16, align 8, 208) or borrowed byte carrier. `readable` is cleared if any reference is not Shared/Mutable (195-198).
- `reference_referent_projection` (64-113): access check at each crossing: `access == requested` or `MutableBorrow` with Shared/WriteOnly requested (91-97); a `WriteOnlyBorrow` referent may not be followed by another `Referent` (103-107).
- Fragmented/authored-storage fields are refused for scalar geometry (`direct_scalar_field_offset` 439-456; `static_field_geometry` 392-396) but supported inside `direct_scalar_field_geometry` for fragmented non-atomic (501-521).

### Cases by shape
`contiguous_primitive_projection` (236-292): selector on a `ByteSequence` (BoundedOwned payload at +8; views at +0), a float field in a direct-field record, else a general `PrimitiveScalar` leaf. `runtime_projection_with_leaf_type` (1135-1168): byte-sequence roots take a special path.

### Same job elsewhere
S03/structural_inputs.rs has `structural_reference_input::{reference_formation_geometry, runtime_projection_elements, collection_projection, runtime_borrowed_projection}` (referenced from S03/legalization.rs:262-265; grepped, not read): suspected re-implementation of these projections over the optimized unit. Inside this file, `field_geometry` (639) and `direct_scalar_field_geometry` (461) each walk record fields accumulating offsets (read both): same offset arithmetic written twice.

### Unused / one-caller
`primitive_leaf_projection`: 1 call (unit.rs:2657) + 1 test (structural_layout.rs:1306). `root_array_projection_metadata`: 1 (function_signature.rs:360). `storage_address_indices`: 1 (control_flow_storage.rs:3232). `runtime_leaf_copy_projection`: 2 (structural_layout.rs:772, control_flow_storage.rs:1894). `runtime_borrowed_projection`: 3 in S02 (structural_layout.rs:110, control_flow_calls.rs:118, 263). All by `Grep`.

### Stated limits
None.

### Fail-open risks
- `reference_projection_regions` 184-189: if the prefix before a `FixedIndex`/`RuntimeIndex` segment does not resolve (`selected` is `Err`), the code `continue`s (treats the index as an ordinary non-pointer step); only a failing `Referent` returns `None`. What reaches it: a path whose prefix is not a valid descriptor projection but has an index segment. Open: later `runtime_projection` of the remaining path would be expected to refuse but I did not trace it.
- `byte_sequence_field_geometry` 710-712: returns `Ok(None)` ("not a byte field") when the path does not end in `Field`; callers treat `None` as "not bounded/borrowed", e.g. `bounded_byte_field_geometry` callers use `.ok_or(...)`. Traced for unit.rs:341-349 (errors if `None`).

---------------------------------------------------------------------------

## 9. S02/unit.rs (2832 lines)

### Purpose
Lowers calls out of a function to boundaries: builtin boundary settlements (hosted write/read byte, port reads, claim-completion-only, Linux write-line), normalized foreign calls (placement of scalar and structural arguments against a foreign `BoundaryEntryPlan`), structural-argument passing to internal callee machines, scalar constants/widening, scalar field stores and write-only primitive stores. Header 1-10.

### Shape (inline modules)
- `argument_roots` (11-106): `StructuralArgumentRoot` (26), `resolve` (33): finds the declaration/backing shape/source of an argument place among parameters, homes, address-join/block-view block parameters.
- `boundary_call` (107-1315): `lower_boundary_call` (795); private `normalized_foreign` (132-788) with `lower_normalized_foreign_structural_arguments` (151), `lower_normalized_foreign_scalar_arguments_with_result` (535), `lower_normalized_foreign_result` (698), `lower_normalized_foreign_scalar_result` (751).
- `known_integers` (1316-1426): `KnownUnitInteger {Parameter, Immediate, Home, BlockParameter}` (1325), `insert_known_unit_integer` (1395), `insert_scalar_home` (1406).
- `owned_store_home::block_home` (1427-1445). `projected_result` (1446-1512): `has_store_fragments` (1453), `source` (1476).
- `roster_arguments` (1513-2027): `lower_structural_argument` (1536), `owned_owner_home` (1829), `local_array_loan` (1859), `fixed_array_view_argument` (1907).
- `scalar_definitions` (2028-2252): `lower_integer_widen` (2041), `lower_integer_constant` (2155), `lower_boolean_constant` (2198), `lower_ieee_float_constant` (2225).
- `structural_scalar::lower_field_store` (2253-2509; re-exported 2510). `write_only_primitive_store::lower_write_only_primitive_store` (2511-2832).

### Inputs and outputs
Inputs include `settlements: BTreeMap<BoundaryMachineId, BoundarySettlementBinding>`, `boundary_machines`, `native_callbacks`, `parameters_by_place`, `local_homes`, `address_joins`, `block_views`, `established_byte_sequences`, `scalar_values`, etc. (795-825). Output: `TargetUnitOperation::{NormalizedForeignCall, BoundarySettlement, ScalarDefinition, IntegerConstant, BooleanConstant, IeeeFloatConstant, StructuralScalarFieldStore, WriteOnlyPrimitiveStore}` and `TargetStructuralArgument`s.

### How it decides
- `lower_boundary_call` (795-1313) branches on `binding.realization`: `NormalizedForeignCall` (847-954) vs `Builtin(realization)` (955-1313). `execution` must be `AdmittedProvider` for the foreign branch (918-923).
- Normalized-foreign structural arguments (151-532): permitted access matrix keyed on the root's access vs the argument's access (226-237); parameter position equals index (246); descriptor formals (`ByteSequence` BorrowedView/BoundedView) have separate placement rules (249-376); everything else uses `reference_referent_projection` or `resolve_structural_projection_path` (378-418); then destination placement shape checks per access (463-515). `BoundaryRealizationMismatch(boundary)` is the single error for all (e.g. 192, 244, 247, 298, 359, 450...).
- Builtin result: `Unit` requires `declaration.result.is_unit()` (962); `Structural` requires matching type/multiplicity/qualifications, empty `projected_qualifications` and claims (973-981) and uses a sum layout (982-993); `Scalar` only when realization is `DirectPortReadU8` and type is `u8` (996-1023).
- `roster_arguments::lower_structural_argument` (1536-1827) decides in this order: owned owner home (1550-1592), established byte view or block-source immutable byte carrier (1593-1627), parameter/result-source/local home (1629-1653), fixed-array view (1654-1668), byte-field capacity (1722-1795), plain projection (1796-1826).

### Cases by shape
- Builtin realization `match` (1043-1295). Quoted: `BoundaryRealization::ProgramImageRetainedSuffixStart | NearestIeeeFloatArithmetic(_) | AccessorScalarAccess(_) | EmptyRunRelease | StaticInterrupt { .. } | AccessLifecycle { .. } => return Err(BoundaryRealizationMismatch)` (1247-1253) and `HostedExitProcessI32(_) => Err(BoundaryRealizationMismatch)` (1244-1246): these builtin realizations are refused at this call site.
- `HostedReadByte` (1136-1193): result sum must have exactly two cases `[empty, byte]` with `[field]` signed 32-bit (or a bounded integer containing 0 and 255) and tag at offset 0, tag shape `integer(4,4)` (1187-1188); host support via `HostedReadByteRealization::supports_target` (1176).
- `HostedWriteByteI32` (1194-1243): exactly one `i32` scalar argument, unit result.
- `LinuxWriteLine` (1087-1135): ELF with `X86_64|Aarch64` only (1088-1092), one structural argument that must be an established byte-sequence literal (1107-1115).
- `DirectPortReadU8` (1255-1294): `target.architecture == Architecture::X86_64` and result placement exactly `Rax` 1-byte (1264-1273).
- `fixed_array_view_argument` (1907-2026): two cases, "fixed byte array window" (`fixed_byte_array_window`, 1931-1966) or "fixed element array extent" (`fixed_element_array_extent`, 1967-1996), else `Ok(None)`.
- `write_only_primitive_store` (2679-2818): `match value.scalar_type` Integer / Boolean / IeeeFloat each with their own source resolution.

### Same job elsewhere
- `unit.rs::argument_roots::resolve` (33) vs `references::source_contract`/`aggregate_results::value_input` (place -> declaration); 4 callers in control_flow_calls.rs (99, 242, 316, 449) plus 1 here (212).
- `normalized_foreign::lower_normalized_foreign_result` (698) vs the `Builtin` branch's `target_result` computation (960-1024): two code paths build `TargetBoundaryResult` with the same type/multiplicity/qualification comparison (711-715 vs 973-979).
- Scalar-source resolution: `structural_scalar::lower_field_store` uses `resolved_source` (2420), `write_only_primitive_store` re-derives sources by hand per scalar type (2679-2818). Same job (read both).
- Two `has_store_fragments`-like placements checks: `projected_result::has_store_fragments` (1453).

### Unused / one-caller
By `Grep` of each name: `lower_boundary_call` 1 (control_flow_operations.rs:1101); `lower_structural_argument` 1 (control_flow_calls.rs:1772); `local_array_loan` 1 (unit.rs:1635); `lower_integer_widen`, `lower_integer_constant`, `lower_boolean_constant`, `lower_ieee_float_constant` 1 each (control_flow_operations.rs:1220, 1180, 1196, 1210); `lower_field_store` 1 (control_flow_operations.rs:908); `lower_write_only_primitive_store` 1 (921); `argument_roots::resolve` 5 callers (unit.rs:212, control_flow_calls.rs:99, 242, 316, 449).

### Stated limits
None by comment. The refusals at 1244-1253 and 996-1004 (`ResultBearingBoundarySettlementRequiresNativeRealization`) act as the stated limits through error variants.

### Fail-open risks
- `lower_boundary_call` 1311: `_ => unreachable!("boundary-call routing admits only boundary calls")` (panic, not accept). Caller routes (control_flow_operations.rs:1101).
- `MetadataOnlyPort` (1044-1069): requires `operations.last()` to be a matching `PortWrite`; structural arguments only need to be existing parameters. Traced as written: requires the immediately preceding operation; no accept without it.
- `claim_completion_only_boundary_is_exact` returns `false` on any mismatch -> error. Fail-closed.
- `write_only_primitive_store` 2668: `runtime_indices(...).map_err(|_| invalid())` swallows the real error detail; still an error.
- None accept-by-default seen.

---------------------------------------------------------------------------

## 10. S03/legalization.rs (518 lines)

### Purpose
Stage 03 front door for legalization. `legalize_target_operations` (405) re-derives scalar function rosters from the target plan + abstract plan + optimization unit and seals them into a `ValidatedLegalizedOperations` with a receipt; `verify_target_operation_plan` (466) is the public standalone audit that replays the same checks without producing a plan. Header 1-6.

### Shape
- `mod model` (8-215): `legalization_validator_identity` (12; the byte string `omega.terminal-target-legalization-independent-replay.v56`), `ValidatedLegalizedOperations` (19), `LegalizationValidationReceipt` (57; fields identity, validator, optimization_unit, work_schedule, target, function_count), `ReferenceObservationContext` (85), `LegalizationError` (101; 12 variants), `LegalizationError::custody` (175, `#[track_caller]`), `attributed` (183).
- `mod obligations` (216-257): `facts` (221), `accepted_fact` (234), `references_obligation` (246).
- `mod runtime_indices` (258-384): `operands` (270), `defined_type` (300), `projection` (317), `reference_source` (344), `collection_view` (360).
- Re-exports 385-403 of `legalization_scalar_graph` as `scalar_graph_input` and `legalization_source` as `source`.
- `legalize_target_operations` (405), `seal` (440), `verify_target_operation_plan` (466), `audit_target_operation_plan` (483).

### Inputs and outputs
`legalize_target_operations(target: &TargetOperationPlan, abstract_plan, unit: &PsiOptimizationUnit, origins, installed_interrupts, installed_entry_execution, installed_stack_requirements)`: calls `installed_interrupts::validate_target` (414), `validate_boundary_origins` (420), `derive_source_function_rosters` (421), builds `LegalizedOperationPlan` (422-429) and `seal`s. `audit_target_operation_plan` (483-518): `source::validate_source_custody`, `normalized_foreign::validate_native_callback_roster`, `DescriptorInputs::new`, then for each target function finds exactly one abstract and one optimized function with the same `machine` (493-508) and calls `scalar_graph_input::match_input` (509-514).

### How it decides
- Function matching is by `machine` id with uniqueness enforced both sides (493-508).
- `obligations::facts` (221-232) filters `unit.accepted_obligation_facts` by `(machine, owner, obligation)` equality; `accepted_fact` returns the first match `.next()` (240-243) and errors `custody()` if none. `references_obligation` (246-256) scans `optimized.facts` for an `OperationObligationReference` whose `support == operation`.
- `runtime_indices::operands` (270-298): scalar type must be an integer of at most 64 bits (281-283) and an accepted fact must exist for the element's obligation (285).
- `runtime_indices::projection` (317-342): returns empty if no `RuntimeIndex` in path (325-332); `Referent` presence picks `reference_formation_geometry` vs `runtime_projection_elements` (333-340).

### Cases by shape
`runtime_indices::reference_source` strips one trailing `Referent` (351-354). `collection_view` strips a trailing `FixedRange` (374-378).

### Same job elsewhere
S02/control_flow_storage.rs checks the same facts forward during lowering; S03's `audit_target_operation_plan` (483) and `legalize_target_operations` (405) both exist: `legalize_target_operations` does not call `audit_target_operation_plan` (only `verify_target_operation_plan` does, 480; `Grep` of `audit_target_operation_plan`: def 483 + 1 call 480). The audit and the derivation share `match_input`/`derive_source_function_rosters` only via the `source`/`scalar_graph_input` modules (not read).

### Unused / one-caller
By `Grep` across `omega-rust`: `legalize_target_operations` 1 caller (S03/optimized.rs:98) + re-export at lib.rs:712; `verify_target_operation_plan` 1 external caller (omega/compilation/backend/images/image-emission/src/function_fragments.rs:3878) + re-export lib.rs:723; `audit_target_operation_plan` 1 caller (legalization.rs:480).

### Stated limits
None.

### Fail-open risks
- `seal` (440-463) does not verify anything; it hashes `plan` (`legalized_operation_plan_identity`) and stamps a receipt with a validator identity string. The receipt is valid for any plan passed. Traced within the file: only `legalize_target_operations` calls `seal` (430); checks happen before (414-421).
- `accepted_fact` takes the first matching fact and does not reject duplicates (240-243). Open: whether `accepted_obligation_facts` can hold two facts for the same `(machine, owner, obligation)` was not traced.
- None other seen.

---------------------------------------------------------------------------

## 11. S03/legalization_descriptor_inputs.rs (155 lines)

### Purpose
Caches the two descriptor-analysis input graphs (`views`, `references`) once per audit so every function check reuses them. Comment 11-13: "Keep each catalog's immutable input graph for all function checks in this audit."

### Shape
`DescriptorInputs<'a> { native, plan, unit, views: OnceCell<Option<Vec<DescriptorMachine>>>, references: OnceCell<...> }` (14-20); `new` (23), accessors `native`/`plan`/`unit` (37-45), `views` (47), `references` (61), private `operation` (76) and `preserves_boundary` (94).

### Inputs and outputs
Inputs: `TargetOperationPlan`, `AbstractOperationPlan`, `PsiOptimizationUnit`. Output: lazily computed `Option<&[DescriptorMachine]>` via `optimized_catalog_graph` (50) and `optimized_reference_catalog_graph_with_boundary_preservation` (64).

### How it decides
- `operation` (76-92): calls `call_origin::selected_boundary_operation(node, caller, native, plan)`; `.ok()` turns an error into `None`; `Some((op,_))` -> owned replacement operation, `None` -> borrowed original (88-91).
- `preserves_boundary` (94-154): true only if the node is a `BoundaryCall` with empty completion claim sources/receipts (111), the target plan has a `NormalizedForeignCall` row for `(caller.machine, psi_operation)` (114-125), exactly one boundary declaration matches the id (126-135), argument counts and per-argument access equal declaration (136-141), and `descriptor_input_guard(...)` returns `Ok(Some(_))` (142-153); the buffer-layout lookup is `native.borrowed_storage_layouts.iter().find(|layout| layout.structural_type == buffer)` (146-151). Keyed on ids, counts, access enums.

### Same job elsewhere
`preserves_boundary` (94-154) repeats the closure at S02/coordination.rs:1550-1578 (same checks in the same order; S02 uses `crate::borrowed_storage::capture_type` for the layout and S03 uses the stored `borrowed_storage_layouts`) (read both).

### Unused / one-caller / "single shared" claim
`DescriptorInputs` appears in S03: legalization.rs (1 line), legalization_scalar_graph.rs (1), legalization_reference_custody.rs (3), legalization_target.rs (2), this file (2) by `Grep` count. The type is used by 4 other files (not read).

### Fail-open risks
- `operation` 81-91: `selected_boundary_operation(...).ok()` discards the error. A selection error turns into `None` from the closure. Open: how `optimized_catalog_graph` treats a `None` operation (probably the whole graph becomes `None`, and `views()`/`references()` return `None`) was not traced; consumers of `None` not read.
- `preserves_boundary` returns `false` on every unknown -> fails closed.

---------------------------------------------------------------------------

## 12. S03/legalization_function_inputs.rs (171 lines)

### Purpose
Pre-sorts the rows of one `PsiOptimizationFunction` (values, place declarations, arrival parameters, block arrivals, trivial affine locals, structural results) so repeated queries during an audit are binary searches instead of full scans. Header 1-2: "Each query still checks the original role, shape and uniqueness requirements."

### Shape
`FunctionInputs<'function>` (14-22) with `new` (25) building 7 sorted vectors; queries `value_type` (74), `trivial_affine_local` (82), `arrival_parameter` (93), `address_join_parameter` (134), `source_result` (156).

### How it decides
Binary search by `partition_point` on place/value id. `arrival_parameter` (93-132) requires exactly one declaration and exactly one arrival row for the place, and the declaration kind to be `Parameter{position}` matching a function-level parameter or `BlockParameter{block != entry, position}` matching the block's row; otherwise `None`. `source_result` (156-170) requires exactly one structural result for the place, else `custody()` error. `trivial_affine_local` (82-91) requires exactly one candidate.

### Same job elsewhere
S02 does the same lookups on `AbstractFunction` by linear scan (`function.arrival_parameter(place)` used e.g. control_flow_storage.rs:355, 1257; `function.structural_parameter`, `block_structural_parameter`). Not the same code (different input type).

### Unused / one-caller
`FunctionInputs::new` is called at 2 sites: S03/legalization_source.rs:2996 and S03/legalization_scalar_graph.rs:8712 (grep), so the per-function table is built in two separate audits (suspected: built twice per function; not read). Type is passed as a parameter in ~16 signatures in legalization_target.rs, legalization_source.rs, legalization_scalar_graph.rs.

### Fail-open risks
None seen. Every query returns `None`/`Err` on duplicates or absence.

---------------------------------------------------------------------------

- **Coverage:**
  - Read in full: control_flow_storage.rs 1-3715 (3715); coordination.rs 1-645 (645); function_signature.rs 1-826 (826); lib.rs 1-701 (701); model.rs 1-326 (326); references.rs 1-2686 (2686); scalar.rs 1-2900 (2900); structural_layout.rs 1-1322 (1322); unit.rs 1-2832 (2832); legalization.rs 1-518 (518); legalization_descriptor_inputs.rs 1-155 (155); legalization_function_inputs.rs 1-171 (171). Total 16,797 lines read.
  - Skimmed: none.
  - Not read: none.
  - Greps run (all uncapped content or count mode): names of every function counted above, across `omega-rust` or the S02 directory; `active.push|active.pop` in scalar.rs (0 matches); stated-limit words (TODO|FIXME|migration|for now|not yet|temporar|legacy|workaround) in all 12 files (0 matches); `OMEGA_COMPILER_CHECK_SITES|OMEGA_TRACE_NATIVE_BOUNDARY` count (36 lines in 14 files).

- **Five things a maintainer most needs to know about this unit:**
  1. Every refusal in stage 02 is mostly one error, `UnsupportedControlFlow` carrying only the Rust call-site line (`model.rs:139-142, 320-325`), so there are hundreds of near-identical refusals with no message (e.g. control_flow_storage.rs has dozens of `unsupported_control_flow`); finding "why was this program refused" means mapping the `CompilerCheckSite` file/line back to the check. `OMEGA_COMPILER_CHECK_SITES` turns on extra stderr tracing in 14 files.
  2. Storage lowering is operation-shape driven, not generic: each fixed list of shapes (records.rs 3359-3407, observations 2071-2493, primitive_storage 2921-3299, boundary realizations unit.rs 1043-1295) ends in `_ => unsupported`. A new shape needs its own arm in several files; the same "u64 unsigned non-address" and "length was measured" checks are re-spelled inline (see section 1).
  3. Reference custody (`references.rs`) keeps one `ReferenceStorage` per `(PlaceId, path)` in `live.references` and is merged at joins by building `Alternatives` of origins; unknown provenance is represented as `None` and later refuses (`borrowed_arrivals::merge` 139-156 -> `normalized_source` 797-800). The `Case`-path `continue` skips (references.rs:1819-1826, 2534-2540) are the places where missing provenance is dropped instead of refused.
  4. Stage 03's `legalization*.rs` files in this unit are thin: `legalize_target_operations` re-derives rosters and seals with a receipt, and the substantive audit lives in `legalization_scalar_graph.rs` / `legalization_source.rs` / `legalization_target.rs` / `legalization_reference_custody.rs` (not in this unit). `seal` (legalization.rs:440-463) verifies nothing itself.
  5. Several functions exist in parallel for S02 (over `AbstractFunction`) and S03 (over `PsiOptimizationFunction`): `completed_descriptor`/`descriptor_result` (scalar.rs:655/688 vs S03 legalization_scalar_graph.rs:2227/2252 vs selection_inputs.rs:2878/2917), `runtime_borrowed_projection` (structural_layout.rs:1076 vs S03 structural_inputs.rs:2061), and the descriptor-input-guard check (coordination.rs:1550-1578 vs legalization_descriptor_inputs.rs:94-154). A change to one has to be mirrored by hand.

- **What you are unsure about:**
  - What `LiveDefinitions`, `RowSite`, `ScalarSources` and `scalar_sources::runtime_indices` actually do (defined in `control_flow.rs`, outside my unit); I described them from use.
  - `selected_boundary_calls::resolve` (referenced at references.rs:1545) is not defined in lib.rs; I assumed it is elsewhere in the crate and did not look.
  - Whether the `Case`-path `continue` skips in references.rs (1819-1826, 2534-2540) can reach a wrong program (I only read the local code and the `commit` that deletes the entry).
  - Whether `reference_projection_regions` (structural_layout.rs:184-189) skipping a failed index prefix is always caught later.
  - How S03 consumers treat `DescriptorInputs::views()/references()` returning `None` after an `.ok()`-discarded selection error (legalization_descriptor_inputs.rs:81-91).
  - Whether the unconstructed `LoweringError` variants (model.rs list in section 5) are built via `Self::`/glob imports; I only searched `LoweringError::Name`.
  - Whether S03 `accepted_obligation_facts` can contain duplicates making `accepted_fact`'s first-match arbitrary (legalization.rs:240-243).
