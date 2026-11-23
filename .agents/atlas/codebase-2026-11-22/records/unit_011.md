# Record: unit_011 (area 10-native-stages-04-10)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d...`. Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/`. "S04" = `04_abstract-operations-to-target-operations/src/target_lowering/`; "S05" = `05_target-operations-to-selected-instructions/src/`. Every file was read in full (Read tool). Counts come from Grep count mode or `-n` searches, command named in each entry.

Common facts for all S04 `control_flow*`/`references`/`scalar`/`unit` code (read): nearly every refusal is `LoweringError::unsupported_control_flow(machine)` (`model.rs:380`), a `#[track_caller]` constructor that records only the machine and the Rust call site (`CompilerCheckSite::caller()`), not the cause. Judgments are keyed on identities (PlaceId, StructuralTypeId, OperationId, ValueId, path segments) and on shapes of `StructuralTypeShape`, not on source spelling. The one spelling-keyed exception is the `reason: &'static str` text in `InvalidStructuralValueInput`, which `coordination.rs:666` compares by string (see that entry).

---

## S04 `control_flow_storage.rs` (3744 lines; read 1-3744)

**Purpose.** A bundle of sub-modules (included by `control_flow.rs` through `use super::*`, `control_flow_storage.rs:12`) that lower storage-related abstract operations to `TargetUnitOperation` rows inside the shared target control-flow graph: access lifecycles, aggregate homes and borrows, atomics, borrowed windows, byte concatenation and byte/element writes, leaf copies, scalar observations, primitive and field reads/stores, record/case/array establishment.

**Shape.** Each module is `pub(crate) mod X { pub(crate) fn lower(operation, site: &mut RowSite, ...) -> Result<(), LoweringError> }`:
- `access_lifecycles::lower` `:28` (BoundaryCall of a compiler-primitive lifecycle).
- `aggregate_borrows::{is_reference, argument}` `:173, :191`.
- `aggregate_results::{value_input, block_home, result_home_layout, value_home_layout, home, publish_home, established_home}` `:304-473`.
- `atomic_access::lower` `:485`.
- `borrowed_windows::{move_field, store_field, window_referent, root_window_loan, window_root, field_path}` `:707-1081`.
- `byte_concatenation::lower` `:1096`.
- `byte_write::{replace, replace_view_descriptor, replace_byte, lower, writable_field, writable_byte_field}` `:1228-1709`.
- `element_view_write::lower` `:1722`.
- `leaf_copy::{transport, copy_extent, byte_carrier_leaf_copy}` `:1805-2022`.
- `observations::{lower, scalar_values, bounded_owner_storage, byte_sequence_operand_gate, removed_scalar_expression, record_view_result}` `:2048-2734`.
- `primitive_storage::{is_primitive_reference, shape, native_shape, byte_field_observation, field_root, field_indices, retain_result, lower}` `:2757-3320`.
- `records::{establish, establish_trivial_affine_local, establish_structural_local, establish_zeroed_structural}` `:3337-3743`.

**Inputs/outputs.** Read: `AbstractOperation` variants, `AbstractFunction` (`arrival_parameter`, `operations()`, `entry_claims`, `blocks`), `StructuralTypeLookup`, `PreparedFunctionSignature`, and `LiveDefinitions` (maps `structural_homes`, `views`, `references`, `integers`, `lengths`, `address_joins`, `block_views`, `owned_arrivals`, `scalar_homes`). Mutate: `LiveDefinitions` (insert/remove homes, views, references) and push `TargetUnitOperation` plus `provenance.operations.push(psi_operation)` through `RowSite::parts()`/`view()`.

**How it decides (all read).**
- Keyed on place identity plus structural-type shape: e.g. `aggregate_results::value_input` `:320-349` picks root type from `live.structural_homes`, then `live.views`, then `function.arrival_parameter`; a borrowed arrival is accepted only for a whole view descriptor (`:335-347`).
- `byte_write::writable_byte_field` `:1520-1532` finds the field by `candidate.id == field`, refuses duplicates or erased fields, then requires the carrier to satisfy a closure `carrier_admitted`.
- Byte/element index and length operands are keyed on scalar type: unsigned, 64 bits (or 8 for byte values), non-address, repeated at `:1189-1193`, `:1323-1327`, `:1607-1618`, `:1687-1697`, `:1769-1773`, `:2694-2698`. This is a repeated hand-written check (6 copies in this file).
- Length operands must have been produced by a matching `StructuralByteSequenceFieldLength` op found by scanning `function.operations()` and comparing place, path, field and result value (`:1168-1174`, `:1591-1594`, `:2679-2685`, `:2988-2990`). Three-to-four near-identical copies of that scan.
- `records::establish` `:3436` requires declared (non-erased) field count equals supplied count; multiplicity `!= Unrestricted` structural inputs are tracked in `consumed` and refuse overlap by `starts_with` on paths (`:3584-3594`).
- Atomic: `ordering_is_legal()`, `observes_resident() && root_access == WriteOnlyBorrow`, field geometry `bytes == integer.bits()/8` and `field_byte_offset % bytes == 0` (`:586-619`).

**Cases by shape (quoted/cited).**
- `observations::lower` `:2080-2118` matches a fixed list: `ByteSequenceRead`, `ByteSequencePredicate`, `ByteSequenceEqual`, length/subslice ops, `ElementViewRead`; `_ => {}` for others, then the second `match` at `:2179-2498` handles a fixed list of scalar operations and ends `_ => return Err(unsupported_control_flow)` at `:2498`.
- `records::establish` `:3375-3435`: accepts `(Record, no case)`, `(Sum, case)`, `(Mixed, case)`, `(FixedArray with literal length equal to element count)`, `(ElementView{access: None} within owned-sequence capacity)`; `_ => Err` at `:3434`.
- `leaf_copy::byte_carrier_leaf_copy` `:1981-2022` hard-codes: parent is a `FixedArray` of `u8` with literal length, path ends in `FixedIndex`, and `index + layout.size <= length`.
- `aggregate_borrows::argument` `:235-268` handles three path forms: empty, all-`Field`, otherwise runtime projection.
- `access_lifecycles::lower` `:90-112` has a fixed expected-operand table per lifecycle operation (Initialize, MoveOut, View, Relocate), at most 4 slots.
- `primitive_storage::lower` handles exactly 7 abstract operations (`:2933-3314`), `_ =>` refuses.
- `byte_concatenation::lower` `:1122` handles exactly two operand shapes (`Whole`, `Field`) and requires `SharedBorrow` for both.

**Same job elsewhere.**
- The "length producer exists" scan appears 4 times inside this file (lines above) and `scalar.rs:929-947` (`observes_count`) does a related job for views.
- `aggregate_borrows::is_reference` (`:173`) and `primitive_storage::is_primitive_reference` (`:2757`) are the same style of predicate for two type families. Grep `aggregate_borrows::` shows callers elsewhere (not counted).
- `borrowed_windows::window_root` and `references::referent_projection` both decide "is this place a valid window root" (read both). Suspected overlap in `root_window_loan` with `terminal_semantics::structural_window_origins` (read call only).
- Preparation stage re-derives some of these checks (suspected: `preparation/legalization_*.rs`, not read).

**Unused / one-caller code.** I did not grep each `pub(crate)` fn individually. One grep: `block_home` is defined at `:373` and a different `owned_store_home::block_home` exists in `unit.rs:1467` (same name, different module, different signature). Not counted further.

**Stated limits.** `:835` comment: "Whole record assignment replaces an owned local's discardable contents just as construction fills its vacancy." No "unsupported/migration" comments seen in this file.

**Fail-open risks.**
- `byte_write::replace_view_descriptor` `:1397-1430`: with `field == None` the destination is accepted if the home is a `shared_readable_byte_view` and path empty; no check that the home is the right multiplicity or has no claims beyond `projected_qualifications.is_empty()`. Classification: open. Wrong program that reaches it: a whole-home descriptor store where the home carries claims; I did not trace callers.
- `observations::lower` `:2080-2118`: for `ByteSequenceSubslice`/`ElementViewSubslice` the inner `match` (`:2103-2111`) does nothing in the `_ => {}` arm, then the later `if let` handles them. Not a risk by itself, but the bool result of `lower_byte_observation` / `lower_element_observation` (`Ok(false)` when the operation is not recognised) is dropped with `?;` (`:2082,2086,2098,2105,2108,2115`). If an operation reached an arm but the callee returned `Ok(false)`, the code goes on to the second `match`, which refuses it at `:2498`. Classification: traced (second match refuses unknown shapes).
- `records::establish` for `StructuralTypeShape::ElementView { access: None }` accepts any `elements.len() <= layout.capacity` without checking element types (`:3422-3431`); element types are then checked per element at `:3475-3486` only via `element_type` and `value_input`. Classification: traced (per-element check exists).
- Everything else in the file returns `Err` on a `None`; none seen beyond the above.

---

## S04 `coordination.rs` (702 lines; read 1-702)

**Purpose.** Whole-plan coordination: admits settlement bindings, provider installation evidence and native-callback arguments once, derives each function's fixed ABI, builds reference catalogs, then lowers every function and attaches contract/call-origin facts.

**Shape.** `lower_to_target_operations_with_settlements_and_installation` `:480` (only public-in-crate entry; one caller, `mod.rs:359`). Private sub-modules: `installed_provider_calls` `:26` (`index_installed_provider_calls`, `index_boundary_calls`, `validate_installed_provider_calls` `:41,72,103`), `native_callbacks` `:220` (`bind_native_callback_arguments` `:229`, `native_callback_application_is_exact` `:307`), `settlement_roster` `:397` (`index_settlement_bindings`, `validate_settlement_roster` `:408,431`).

**Inputs/outputs.** In: `AbstractOperationPlan`, `PsiOptimizationUnit`, target, `BoundarySettlementBinding[]`, optional `AdmittedProviderInstallation`, admitted callback rows, optional `AdmittedLoweringFacts`. Out: `TargetOperationPlan` with functions, `native_callback_arguments`, `publication`, `borrowed_storage_layouts`; calls `function_contract::retain` `:691` and `call_result_origins::retain` `:692`.

**How it decides.**
- Installed vs settled: a boundary is "installed" if any installed call names it; a settlement overlapping an installed boundary is refused (`:440-447`); a call to an installed boundary without installed evidence is refused (`:448-460`); every non-installed boundary must have a settlement (`:461-469`) and extra settlements are refused as `UnusedBoundarySettlement` (`:470-475`). Keyed on `BoundaryMachineId` and the tuple `(MachineId, InvocationSite, BoundaryMachineId)`.
- Installed-call evidence compared field-by-field with the abstract `BoundaryCall` (`:142-215`).
- Callback rows: duplicates counted by `(terminal_operation, placement_index)` (`:245-257`); `registrar_application_commitment == [0; 32]` is refused (`:277`).
- Fused multiply-add is refused wholesale: `NearestIeeeFloatFusedMultiplyAdd` -> `UnsupportedIeeeFloatFmaRealization` (`:548-559`).
- Debug trace gated on env vars `OMEGA_TRACE_NATIVE_BOUNDARY` (`:574`) and `OMEGA_COMPILER_CHECK_SITES` (`:665`). The latter compares `reason == "view discard has no affine descriptor carrier"` (string compare, `:666-667`). Grep `OMEGA_TRACE_NATIVE_BOUNDARY|OMEGA_COMPILER_CHECK_SITES` over `source-rust/*.rs` count mode: 32 occurrences in 11 files.

**Cases by shape.** `validate_installed_provider_calls` handles two operation shapes, `ExternalCompletion` (requires `installed.result == Unit` and empty completion claims/receipts, `:126-131`) and `BoundaryCall`; anything else for an installed key is `UnknownSelectedBoundaryCall` (`:151`).

**Same job elsewhere.** `index_installed_provider_calls`/`index_boundary_calls` overlap with `mod.rs:552-678` `selected_boundary_calls::index` (both walk every `BoundaryCall`/`ExternalCompletion` in every function and key by the same triple; read both). Grep `fn index_boundary_calls|fn index\b` not run; stated as read overlap.

**Unused / one-caller.** `lower_to_target_operations_with_settlements_and_installation`: 1 caller (`mod.rs:359`; Grep `lower_to_target_operations_with_settlements_and_installation` not run, taken from the read of `mod.rs`).

**Stated limits.** Header comment `:1-11` only. `:549-559` FMA refusal has no comment.

**Fail-open risks.**
- `index_installed_provider_calls` `:45-53`: `installation == None` yields an empty installed set, which is correct (no installation), classification: stated by structure.
- `lower_to_target_operations_with_settlements_and_installation` `:489-498`: `verified_input == None` gives `profile = None` and skips the profile-vs-target geometry check. Classification: open; the public raw-plan path (see `function_signature.rs:477-480` comment "Public independent raw-plan metadata has no artifact owner") then lowers without that identity check. `lower_native_body` always passes `Some(&lowering_facts)` (`mod.rs:366`), so on the product path it is traced as covered.

---

## S04 `function_contract.rs` (229 lines; read 1-229)

**Purpose.** Attaches to each target function the instruction-source metadata (`instructions`) and the storage contract (`retain`), so stage 05 needs neither the abstract body nor the optimization unit.

**Shape.** `instructions(machine, block, operations)` `:13` -> `Vec<TargetInstruction>`; `retain(source, unit, target)` `:73`. One `#[test]` `:151` (decomposition keeps slot and annotations).

**Inputs/outputs.** `instructions`: reads the `OptimizationBlock.nodes` (last node is the terminator, dropped by `split_last` `:26`) and zips with `TargetUnitOperation` rows; output rows get `TargetInstructionSource {operation, result, metadata{work, effect, ownership}}`. `retain`: sets `target.structural_contract` when `mixed_structural_scalar_abi.is_some() || !graph.parameters.is_empty() || !declared_places.is_empty()` (`:96-98`).

**How it decides.** Position-based: row `i` pairs with node `i` for the first `ordinary_count` rows, where `ordinary_count` is the position of the first `CleanupCall` row (`:30-33`); count mismatch refuses (`:34-36`); each node may define 0 or 1 values (`:41-49`). Identity roster check in `retain`: machine ids must be equal in all three lists (`:90`).

**Cases by shape.** "one scalar definition at most" per instruction (`:41-49`); cleanup calls must come after all ordinary rows (`:63-65`).

**Same job elsewhere.** Comment `:3-4` says independent correspondence checks "remain in stage 03" (not read).

**Unused / one-caller.** `instructions`: grep not run for this name beyond `function_contract.rs` self; used by `control_flow.rs` (inferred, not read). `retain`: 1 caller (`coordination.rs:691`, read).

**Fail-open risks.** `retain` `:96-99`: when `has_storage` is false, `structural_contract` becomes `None` regardless of `source.structural_parameters`/`entry_claims`; a function with `entry_claims` or `structural_parameters` but no graph parameters and no declared places would lose its contract silently. Classification: open (not traced; `target.graph.parameters` is built elsewhere).

---

## S04 `function_signature.rs` (1218 lines; read 1-1218)

**Purpose.** Evaluates calling-convention plans for function parameters and results, derives fixed scalar and mixed structural-scalar ABIs, builds structural call signatures, and holds the `StructuralTypeLookup` wrapper around native layouts.

**Shape.**
- Free items: `integer_parameters` `:23`, `PreparedFunctionSignature {call_plan, scalar_parameters, parameters}` `:49`, `parameters_by_place` `:63`, `prepare_function_signature` `:72`.
- `scalar_abi`: `derive_mixed_structural_scalar_function_abi` `:175`, `derive_fixed_scalar_function_abi` `:211`, `fixed_native_scalar_shape`, `fixed_native_integer_shape` `:297,300`.
- `structural_signature`: `StructuralCallSignature::{parameter_shape, derive, structural_shapes, plan}` `:348-416`, `ParameterLane {Internal, Foreign}` `:320`, `StructuralCallPlan`.
- `structural_type_lookup`: `StructuralTypeLookup {layouts, projections, borrowed_layouts, catalog, carriers, verified}` `:435`, with `new`, `closed_scalar`, `closed_integer`, `storage_region_source_compatible`, `subtree`, and `impl RecordStorageResolver` `:546`.
- Tests: `native_region_geometry_tests` `:600` with 13 `#[test]` (grep count not run; counted by reading: `:616,771,783,819,827,835,938,967,985,1002,1039,1178,1204` = 13).

**How it decides.**
- `parameter_shape` `:348-369`: owned -> referent shape as-is; borrowed in `Internal` lane with `scalar_storage_type(...).is_some()` -> `borrowed_scalar_reference`, else `borrowed_reference(size, align)`. Keyed on structural type's scalar-storage status and lane.
- `derive` `:378-395`: an opaque type goes through `opaque_borrow_transport(shape, access)` first, bypassing sizes (`:379-384`).
- `closed_scalar` `:475-481`: with `verified` uses `closed_module().closed_types().scalar(..)`; without, `ClosedScalarType::try_from(scalar)`. Comment `:477-479`: "Public independent raw-plan metadata has no artifact owner."
- `realization` `:576-587` always errors `UnresolvedLayout`: comment `:581-583` "An unresolved application is not permission to fetch its old realization."

**Cases by shape.** `derive_fixed_scalar_function_abi` returns `Ok(None)` (no fixed ABI) when the function has structural parameters or entry claims (`:215`), a structural result (`:226`), or any parameter without a fixed native scalar shape (`:237-239`). `derive_mixed_...` returns `None` when there are no structural parameters (`:180`), no scalar result (`:183`), or a non-fixed result shape (`:186`).

**Same job elsewhere.** `ValueShape::for_scalar` wrapped three ways: `fixed_native_scalar_shape` (`:297`), `primitive_storage::native_shape` (`control_flow_storage.rs:2779`), and `scalar::scalar_shape` (`scalar.rs:2252`, which goes through closed types and checks widths). Three entry points for related jobs.

**Unused / one-caller.** Grep `derive_mixed_structural_scalar_function_abi|derive_fixed_scalar_function_abi` count mode over `source-rust/*.rs`: 5 occurrences in 2 files (`coordination.rs` 3, `function_signature.rs` 2): one caller each, both in `coordination.rs`.

**Fail-open risks.**
- `derive_mixed_structural_scalar_function_abi` `:189-195`: `let Ok(prepared) = prepare_function_signature(..) else { return Ok(None) }` swallows any signature error (e.g. unsupported layout) and returns "no mixed ABI". Wrong input that reaches it: a function whose signature cannot be planned. The function is then lowered without `mixed_structural_scalar_abi`, and `function_contract::retain` `:96` can set `has_storage` false. Classification: open (no comment at the site; I did not trace whether `lower_function` re-plans the signature and errors).
- `closed_scalar` `:480` raw-plan fallback accepts concrete scalars with no module ownership: stated (comment `:477-479`).

---

## S04 `mod.rs` (678 lines; read 1-678)

**Purpose.** Declares the target-lowering module tree and holds the stage entry `lower_native_body`, claim-completion boundary check, per-function entry (`function::lower_function`), settlement binding (`provider_evidence`) and selected-call indexing (`selected_boundary_calls`).

**Shape.** `lower_native_body` `:343` (called at `driver/compile_body.rs:38` per crate map); `require_lowering_context` `:465`; `prepare` `:415`; `retained_private_references` `:297`; `OptimizedTargetLoweringRequest` `:337`; `boundary_settlements::claim_completion_only_boundary_is_exact` `:65`; `function::lower_function` `:230`; `provider_evidence::bind_provider_executions` `:500`; `selected_boundary_calls::{SelectedBoundaryCall, SelectedCallsByCall, index}` `:560-572`.

**Inputs/outputs.** `lower_native_body` consumes `OptimizedAbstractBody`, calls `require_lowering_context`, `bind_provider_executions`, `coordination::lower_to_...` (`:359`), `prepare` (`:369`, which runs `preparation::prepare_execution_plan`), `emission_facts::project` (`:379`), optionally `retain_target_operations` (`:383`), and returns `NativeBodyPlan {selection, facts, private_references}`.

**How it decides.**
- `claim_completion_only_boundary_is_exact` `:65-171`: all-or-nothing boolean built from ~14 shape conditions (no scalar args, unit result, linear owned parameters, canonical claim sources equal to `entry_claims`, receipts equal as `(argument_index, claim)` sets). Keyed on `position == index`, structural type id equality, qualification lists after sort/dedup.
- `native_boundaries::unsupported_scalar_call` `:189-227`: a `BoundaryCall` with scalar arguments is refused unless it is selected (installed/entry call) or its realization is one of `HostedExitProcessI32`, `HostedWriteByteI32`, `NearestIeeeFloatArithmetic`, `AccessorScalarAccess`, or `NormalizedForeignCall` (`:210-224`). Keyed on `BoundarySettlementRealization` variant.
- `selected_boundary_calls::index` `:572-677`: a call is "program entry" if `input.program_entry().boundary() == *boundary` (`:622-625`); otherwise installed provider; else `continue` (skip, `:651`).
- `retained_private_references` `:297-330`: scans `NormalizedForeignCall` callbacks and installed-interrupt entries whose `(machine, operation)` is in `plan.installation_operations` (binary search, `:320-323`).

**Cases by shape.** Hosted realizations list at `:210-224`; `bind_provider_executions` three execution kinds (`NativeBinding`, `Provider`, `CompilerBuiltin`, `:511-541`).

**Same job elsewhere.** `coordination.rs:41-101` (see above). `mod.rs:552` `selected_boundary_calls` and `control_flow_calls.rs:2082` (`control_flow::selected_boundary_calls`, grepped `mod selected_boundary_calls`) are two modules with the same name: one builds the index, the other (`resolve` at `:2090`, 3 users: `call_result_origins.rs:79`, `control_flow_operations.rs:323`, `references.rs:1546`, grepped) turns a selected call into an operation.

**Unused / one-caller.** `claim_completion_only_boundary_is_exact`: used in `unit.rs:1105` (read; 1 caller seen). `OptimizedTargetLoweringRequest`: re-exported at `:57`.

**Stated limits.** Header `:1-39` labels stages "Omega Stage 02/03/00/01" which do not match the folder numbers (crate map section 4 notes this). Comment `:294-296`: "External completion transfers carry no callbacks."

**Fail-open risks.**
- `selected_boundary_calls::index` `:651` `None => continue`: a `BoundaryCall` with no installed evidence and not the entry boundary is silently left out of `SelectedCallsByCall`. Classification: traced to `validate_settlement_roster` (`coordination.rs:448-469`), which refuses such a call unless a settlement exists. So an unsettled, uninstalled call is refused there.
- `retained_private_references` `:320-323`: `binary_search(...).is_ok()` presumes `installation_operations` is sorted; a missing/unsorted list would silently drop interrupt entries. Classification: open (sortedness not traced).

---

## S04 `model.rs` (386 lines; read 1-386)

**Purpose.** The `LoweringError` enum (about 130 variants, `:17-310`), two helper enums `AccessCallCustodyError` `:313` and `AccessViewInputTranslationError` `:320`, re-exports of admitted settlement types from `provider_admission` (`:12-14`).

**Shape.** `impl Display` `:341` prints `{:?}` except for `BoundaryRealizationDetail` (prints the reason string). `with_boundary_context` `:363` rewrites a `BoundaryRealizationDetail` reason with the boundary's display name from `application_identity::type_identity::callable_display_path` (`:371-375`): this is a message-only use of a display name, not a decision. `unsupported_control_flow` `:380`.

**How it decides.** Nothing; diagnostic vocabulary. Many variants are per-operator type-mismatch errors (`WrappingAddOperandTypeMismatch` ... `SaturatingRemainderOperandTypeMismatch`, `:287-299`), one per integer operator policy pair, mapped in `scalar.rs:2609-2656`.

**Unused / one-caller.** Not individually grepped; there are many variants. `MultipleNativeCallbackArguments` `:187`: not grepped.

**Fail-open risks.** None seen.

---

## S04 `references.rs` (2741 lines; read 1-2741)

**Purpose.** Tracks, per control-flow position, where each reference descriptor (a pointer-like value stored in a place at a path) points: its root origins, carrier type, referent type and optional access interpretation. Propagates this across copies, calls, arrivals, moves and stored-effect summaries, and lowers reference-forming operations.

**Shape.**
- `borrowed_arrivals` `:29`: `BorrowedSources` (`Arc<BTreeMap<PlaceId, Option<Vec<BorrowedSource>>>>`), `borrowed_source_catalog` `:44`, `capture` `:67`, `merge` `:138`, `storage_sources` `:157`.
- `ReferenceStorage {root, carrier_type, referent, placed}` `:188`; `ReferentProjection` `:195`.
- Free fns: `referent_projection` `:202`, `window_projection` `:248`, `reference_call_argument` `:291`, `contains_reference` `:370`, `leaf_paths` `:374`, `referent` `:460`, `leaf_referent` `:476`, `leaf_type` `:488`, `source_contract` `:498`, `direct_storage_type` `:517`, `reference_source_type` `:548`, `storage_projection` `:586`, `reference_leaf(_following)` `:622,632`, `access_source` `:654`, `access_formals` `:678`, `access_call_custody` `:703`, `normalized_source` `:789`, `normalized_direct_source` `:834`, `establish` `:2000`, `establish_access` `:2089`, `retain_access_result` `:2184`, `release` `:2214`, `discard_owned` `:2253`, `discard_subtree` `:2269`, `window_descriptors` `:2293`, `window_storage` `:2336`, `subtree_leaves` `:2352`, `relocate_leaves` `:2391`, `boundary_result_references` `:2406`, `call_results` `:2442`, `combine_alternatives` `:2672`, `combine_candidate_origins` `:2691`, `entry` `:2711`, `moved_argument_leaves`, `commit_argument_moves` `:1945,1969`.
- Sub-modules: `copies` `:891` (`storage_regions`, `storage_cursor`, `copied_leaves`), `observations` `:1098` (`through_resident`, `observe`, `sources_at_storage`), `arrivals::merge_arrivals` `:1294`, `stored_calls` `:1323` (`StoredEffects`, `catalog` `:1476`, `stage` `:1726`, `commit` `:1932`, `trace_origin` `:1414`).

**Inputs/outputs.** Reads `LiveDefinitions` (`references`, `structural_homes`, `views`, `borrowed_reference_sources`, `access_formals`, `stored_reference_effects`, `reference_position`), function declarations, structural types. Writes `live.references` (insert/remove), `retired_reference_roots`, structural homes; pushes `EstablishReference`, `EstablishAccessReference`, `ReleaseReference` rows. Pulls abstract descriptor analysis from `terminal_semantics::descriptor_results`/`optimization_unit_semantics::descriptor_results` (`abstract_reference_catalog_graph_with_boundary_preservation`, `DescriptorAnalysis`, `:1536`).

**How it decides.**
- Origin/ownership keyed on place identity and path (`PlaceId`, `Vec<StructuralPathSegment>`), referent crossing is the `StructuralPathSegment::Referent` segment (`rposition`/`position` at `:257,593,841,1113`).
- Access: `access_covers`/`admits`/`admits_referent` on `StructuralAccess` (e.g. `:532,609,1677,1805,1913`).
- Merge (`combine_alternatives` `:2672`): alternatives must agree on `referent` and `placed`, otherwise refuse (`:2682-2684`).
- `stored_calls::catalog` `:1476-1720`: per machine builds `Result<Vec<Effect>, &'static str>`; the `Err` string is the refusal reason, later returned as `StoredReferenceCall { reason }` (`:1762`). Reasons are fixed strings, e.g. "a generic call result has no concrete reference origin" `:1694`.
- `stage` `:1726`: a dynamic call (no callee) is refused if any argument has writable reference slots (`:1743-1756`); a callee with no catalog row is accepted unchanged (`:1759-1761`).

**Cases by shape.**
- `leaf_paths` `:374-458`: handles `Reference`, `Record/Sum/Mixed` and `FixedArray`; `_ => {}` ignores other shapes. Hard-coded leaf cap: callers pass `4096` (`:2415`, `:2723`), `leaf_paths` returns `None` on overflow `:390-392,422-424,440-446`.
- `stored_calls::catalog` `:1655-1698`: `DescriptorStoredOrigin` variants `Parameter` (accepted if direct), `Empty`, `Provider` are handled; `Invocation`, `IncomingSlot`, `IncomingContents`, `ProducedContents`, `Literal`, `AccessFormation`, `Carried` each return a fixed "has no physical metadata conversion"-style error.
- `call_results` `:2520-2557`: `ReferenceCallOrigin::{Parameter, Invocation}` only.

**Same job elsewhere.** `borrowed_windows::window_root` / `root_window_loan` (`control_flow_storage.rs:962-1071`) and this file's `window_projection` / `window_descriptors` are two answers to "is this window valid". `reference_source_type` here and `scalar.rs descriptor_operands::descriptor_projection_access` (`scalar.rs:784`) both resolve root type of a place by walking arrival parameter, home, view result (read both). Stored-effect analysis relies on `optimization_unit_semantics::descriptor_results`; suspected overlap with the preparation stage's reference custody (`preparation/legalization_reference_custody.rs`, not read; 13 matches of the env trace names there per Grep count).

**Unused / one-caller.** `pub(crate)` re-exports at `:182-185,1293,1322,1942-1943`. Not individually grepped.

**Stated limits.** `:2232-2233` comment: "Releasing a projected descriptor leaves its enclosing home and sibling fields in place." `:2415`/`:2720` use the literal `4096` as a leaf budget with no comment. Error text `:1693` "stored-reference captured incoming contents have no physical metadata conversion".

**Fail-open risks.**
- `stage` `:1759-1761` `let Some(effects) = ... else { return Ok(staged) }`: a callee with no stored-effect row gets an empty `Staged` (no replacements). Classification: traced in part: `catalog` only creates rows for machines with writable reference slots (`writable_slots`, `:1401-1412`, `is_none_or(...)` treats "unknown" as writable), so machines without writable slots legitimately have no effects. A machine for which `writable_slots` was true but the graph reconstruction failed gets `Err("... could not be reconstructed")` (`:1499`), refused. So not fail-open.
- `borrowed_arrivals::merge` `:138-155`: when the incoming side has no entry for a place, the left side becomes `None` (unknown), which `normalized_source` later refuses (`:798-801`). Not fail-open.
- `release` `:2227-2238`: releasing a view descriptor leaves `live.references` unchanged when `path` is non-empty and does not error if no entry exists for views. Classification: open; wrong input: a release of a projected view leaf that was never established is accepted (no `ok_or` as in the non-view arm `:2240-2242`).
- `discard_subtree` `:2276-2278`: `let Some(contract) = source_contract(...) else { return Ok(false) }` treats a place with no known contract as "nothing to discard". Classification: open.

---

## S04 `scalar.rs` (3041 lines; read 1-3041)

**Purpose.** Lowers scalar and descriptor-observing operations: byte-view, element-view and descriptor-operand resolution (which backing a view descriptor reads), scalar expression algebra (`KnownScalar`, `KnownInteger`), integer binary/shift/widen/cast.

**Shape.** Modules: `byte_views` `:9` (`lower_byte_observation_with_lengths` `:121`, `resolve_byte_view` `:366`, `owned_byte_result`, `completed_descriptor`, `descriptor_result`, `byte_view_referent_size` `:82`), `descriptor_operands` `:737` (`descriptor_projection`, `descriptor_metadata_projection`, `observes_count`, `observes_measured_length`, `storage_backs_view_parameter`, `refusal` `:1053`), `element_views` `:1069` (`ElementViewBackings` `:1442`, `establishment::source` `:1100`, `lower_element_observation_with_lengths` `:1632`, `float_element_read` `:1792`, `resolve_element_view` `:1909`), `expressions` `:2239` (`scalar_shape` `:2252`, `scalar_parameter_location`, `KnownScalar`, `KnownInteger`, boolean/integer comparison helpers), `integer_binary` `:2537`, `integer_operation` `:2713`, `shift` `:2929`. One test `width_tests` `:3001`.

**How it decides.**
- `byte_views::byte_view_referent_size` `:82-98` hard-codes carrier sizes: `BorrowedView` -> 16, `BoundedView` -> 8, `BoundedOwned{capacity}` -> `capacity + 8` (checked, `u32`). Duplicated numbers appear in `control_flow_storage.rs` `:880` (`+8`), `unit.rs:352` (`+8`), and `element_views` `:1375` (16 for stored length, 8 for fixed).
- `resolve_byte_view` `:366-620` tries, in order: descriptor result, owned byte result, byte literal, `EstablishElementView` destination, arrival parameter, `ByteSequenceSubslice` producer; each by scanning `function.operations()` for a producer with matching place (`:419-423,435-441,537-538`). The final fallthrough demands a `ByteSequenceSubslice` producer else `refusal(...)`.
- `scalar_shape` `:2252-2281`: `require_native_parameter` rejects widths not in `{8,16,32,64}` (`:2268`); else bytes = `bits.div_ceil(8)`, alignment `next_power_of_two().min(8)` (`:2279`).
- `scalar_parameter_location` `:2283-2312`: accepts exactly one register location or one stack location at value offset 0 with full byte size.
- `lower_integer_binary` `:2553`: operand type rule: for `Exact Add` uses `admits_exact_add_operands`, for `Exact Subtract` `admits_exact_subtract_operands`, else both operand types must equal the result type (`:2572-2582`). Proof-bearing arithmetic (`arithmetic.obligation().is_some()`) is never constant-folded (`:2586-2592`).
- `lower_shift` folds only when `policy == Wrapping` and both operands are immediates (`:2964-2980`); `Exact` shifts always remain runtime.

**Cases by shape.**
- `lower_byte_observation_with_lengths` `:131-296`: ByteSequenceSubslice, Predicate, Equal, Length, Read; `_ => return Ok(false)` `:296`. `ByteSequenceRead` requires result `u8` and index `u64` unsigned (`:319,329-330`).
- `element_views::resolve_element_view` `:1909-2206`: result descriptor, `EstablishElementView`, block parameter, function parameter, `ElementViewSubslice`; else refuse.
- `establishment::source` handles `ElementViewRegion::{WholeCollection, WholeRepresentation, RepresentationBytes}` (`:1262-1332`) and three `CollectionExtent` variants for windows (`:1399-1426`), refusing `Parameter` extents.
- `integer_operation::try_lower_integer_operation` `:2721-2927`: IntegerConstant, Arithmetic, BitwiseNot, Widen, ExactCast, BitwiseAnd/Or/Xor, Shift; `_ => return Ok(false)`.

**Same job elsewhere.** `byte_views::resolve_byte_view` and `element_views::resolve_element_view` are parallel resolvers (same try-order idea). `lower_integer_binary` here and the `IntegerExactCast` arm inside `observations::lower` (`control_flow_storage.rs:2253-2286`) duplicate `IntegerExactCast` handling with `integer_operation` `:2833-2860` (read both; the latter returns `IntegerExactCastTypeMismatch`).

**Unused / one-caller.** I did not grep each fn. `scalar_abi::fixed_native_integer_shape`: used at `unit.rs:110,1239,2103` (read). `is_immutable_byte_carrier_parameter`: used at `unit.rs:1548-1641` (read).

**Stated limits.** `:2258-2259` "Native geometry belongs to the original artifact. A concrete raw descriptor cannot recover membership missing from its retained catalog." `:1052` `#[track_caller] refusal` prints context only if env `OMEGA_COMPILER_CHECK_SITES` set.

**Fail-open risks.**
- `lower_byte_observation_with_lengths` / `lower_element_observation_with_lengths` return `Ok(false)` for unrecognised operations; callers in `observations::lower` ignore the bool. Classification: traced (later `match` refuses; see `control_flow_storage` entry).
- `observes_count` `:914-948` and `observes_measured_length` `:950-985` return `false` (refuse) on unknown. Not fail-open.
- `refusal` always returns `UnsupportedOperationInScalarFunction`, the same error for every cause: diagnostic precision gap, not acceptance.

---

## S04 `structural_layout.rs` (1493 lines; read 1-1493)

**Purpose.** Computes native byte geometry for structural paths: size/alignment of a type, field offsets, case offsets, element strides, runtime selector lists (`RuntimeIndex`), and reference-region splitting (pointer slots crossed by `Referent` or indexing through descriptors).

**Shape.** Error mapper `geometry_error` `:15`; `RuntimeIndex` re-export `:48`; `StorageProjection` `:51`; `PrimitiveProjection` `:57`; `reference_referent_projection` `:74`; `primitive_leaf_projection` `:117`; `reference_projection_regions` `:165`; `contiguous_primitive_projection` `:241`; `primitive_read_projection` `:305`; `scalar_fields` module `:349` (`static_atomic_geometry`, `static_field_geometry`, `direct_scalar_field_offset`, `direct_scalar_field_geometry`); `structural_shape` `:557`; `scalar_storage_type` `:564`; `structural_sum_layout` `:571`; `field_geometry` `:629`; `bounded_byte_field_geometry` `:719`; `case_projection` `:735`; `resolve_structural_projection_path` `:755`; `ProjectedStorage` `:794`; `runtime_storage_projection` `:847` (core walker); `runtime_descriptor_projection` `:992`; `runtime_projection` `:1018`; `runtime_leaf_copy_projection` `:1032`; `runtime_borrowed_projection` `:1058`; `reference_formation_indices` `:1089`; `storage_address_indices` `:1139`; `checked_align_up_u32` `:1206`. Three `#[test]` (`:1220,1355,1412`).

**How it decides.**
- `runtime_storage_projection` `:847-987`: walks each path segment; `AllElements`, `RepresentationView`, `Referent`, `FixedRange` are refused inside it (`:868-906`); `Field`, `Case`, `FixedIndex`, `RuntimeIndex` produce offsets. Overflow -> `StructuralTypeTooLarge`; out-of-range `FixedIndex >= length` -> `UnknownStructuralType` (`:924-927`). A repeated field with an element stride overrides the stride once (`:942-947`).
- `reference_projection_regions` `:165-238`: a pointer slot must have `byte_size` 8 or 16 and `alignment == 8` (`:199`), element-view carrier slot `byte_size == 16` (`:210`). Magic numbers 8/16 match the pointer-and-length descriptor sizes.
- `direct_scalar_field_geometry` `:471-550`: Mixed shapes read the common-field layout; records walk fields skipping erased ones and ones with explicit storage that are not the target (`:508-510`); fragmented storage returns its byte envelope but refuses atomic fields (`:515-531`).

**Cases by shape.** `contiguous_primitive_projection` `:241-302`: (1) path ending in `FixedIndex`/`RuntimeIndex` into a `ByteSequence` (with per-carrier payload offsets: `BoundedOwned` 8, `BoundedView` 0, `BorrowedView` 0); (2) float-typed field; (3) generic leaf must be `PrimitiveScalar`; `_ => None`. Only `ByteSequence` is special-cased for index-into-carrier.

**Same job elsewhere.** `scalar_fields::direct_scalar_field_geometry` here and `preparation` re-derivations (suspected, not read). `structural_shape` is a thin wrapper of `StructuralLayouts::shape`; `calling_conventions::terminal_layout` owns the real work (not read).

**Unused / one-caller.** `bounded_byte_field_geometry` `:719`: used at `unit.rs:341` (read, 1 seen). `primitive_leaf_projection` `:117`: used at `unit.rs:2699`, and in a test `:1477`.

**Stated limits.** `:46-47` "Later stages must not recover either from an earlier executable body." `:1087-1088` "Neither suffix adds a runtime selector."

**Fail-open risks.** `contiguous_primitive_projection` returns `Option`; callers map `None` to refusal (`.ok_or(...)` `:1176`, `.ok()?` elsewhere). `reference_projection_regions` `:182-189`: if `runtime_descriptor_projection` fails for a non-`Referent` segment it does `continue` (skips the region split) rather than refusing; wrong input: an index segment whose preceding path cannot be projected is treated as plain storage. Classification: open (later projection `runtime_projection` would then error when walking the same path, inferred).

---

## S04 `unit.rs` (2848 lines; read 1-2848)

**Purpose.** Lowers calls and stores for "unit" functions: structural argument roots, boundary settlement calls (builtin and normalized foreign), known-integer tracking, structural call arguments, scalar constants/widen, scalar field stores, write-only primitive stores.

**Shape.** `argument_roots::resolve` `:33`; `boundary_call` `:107` (`normalized_foreign::{lower_normalized_foreign_structural_arguments, ..._scalar_arguments_with_result, lower_normalized_foreign_result, lower_normalized_foreign_scalar_result}` `:151,540,703,782`; `lower_boundary_call` `:826`); `known_integers` `:1350` (`KnownUnitInteger`, `insert_known_unit_integer`, `insert_scalar_home`); `owned_store_home::block_home` `:1467`; `projected_result::{has_store_fragments, source}` `:1487,1510`; `roster_arguments` `:1547` (`lower_structural_argument` `:1570`, `owned_owner_home`, `local_array_loan`, `fixed_array_view_argument` `:1941`); `scalar_definitions` `:2062` (`lower_integer_widen`, `lower_integer_constant`, `lower_boolean_constant`, `lower_ieee_float_constant`); `structural_scalar::lower_field_store` `:2315`; `write_only_primitive_store::lower_write_only_primitive_store` `:2573`.

**How it decides.**
- `lower_boundary_call` `:857-1346`: a normalized foreign call is validated against `binding.boundary_entry_plan` (operand placements and native callbacks) and the descriptor-input guard; builtin realizations are a closed list handled by `match realization` `:1077-1329`. Any mismatch -> `BoundaryRealizationMismatch(boundary)`.
- Builtin scalar-result calls are accepted only for `BoundaryRealization::DirectPortReadU8` and result `u8` (`:1030-1048`).
- `LinuxWriteLine` `:1121-1169`: requires ELF, x86_64 or aarch64, exactly one structural argument that is an established byte-sequence literal in `established_byte_sequences`, `parameter.position == 0`.
- `HostedReadByte` `:1170-1227`: result must be a two-case `Sum` where the second case has exactly one field of `i32` (or bounded-integer within 0..=255), tag at offset 0 with shape `integer(4,4)`.
- `fixed_array_view_argument` `:1941-2060`: accepts either a byte-array window (`fixed_byte_array_window`) or fixed element array extent; computes the backing end with checked arithmetic and refuses if `end > root_byte_size` (`:2036`).
- `lower_structural_argument` `:1570-1861`: tries (1) owner home, (2) established views / block sources, (3) parameters / projected call results / local homes, then falls into runtime projection with `byte_field_capacity` handling.

**Cases by shape.** The `match realization` at `:1077` lists: `MetadataOnlyPort`, `ClaimCompletionOnly`, `LinuxWriteLine`, `HostedReadByte`, `HostedWriteByteI32`, `HostedExitProcessI32` (always refuses), `ProgramImageRetainedSuffixStart | NearestIeeeFloatArithmetic | AccessorScalarAccess | EmptyRunRelease | StaticInterrupt | AccessLifecycle` (all refuse here), `DirectPortReadU8`. A 12-variant list handled by name; unsupported ones return `BoundaryRealizationMismatch`.
- Hard-coded targets: `HostedWriteByteI32` requires `supports_target(target)`; `HostedReadByte` likewise; `DirectPortReadU8` requires `Architecture::X86_64` and result register `X86Rax` (`:1298-1307`).

**Same job elsewhere.** `normalized_foreign::lower_normalized_foreign_structural_arguments` `:151` and `roster_arguments::lower_structural_argument` `:1570` both turn a `StructuralArgument` into `TargetStructuralArgument` with offsets, sources and shape checks. `unit.rs:339-349` `bounded_byte_field_geometry` use and `roster_arguments` `byte_field_capacity` code `:1756-1828` repeat the capacity arithmetic `offset + 8 + capacity <= byte_size`. `scalar_definitions::lower_integer_widen` `:2075` and `integer_operation` `:2798-2832` and `observations::lower` do widen/constant lowering for three different function classes. Name collision: `argument_roots::resolve` `:33` vs `references`-owned `selected_boundary_calls::resolve`.

**Unused / one-caller.** Not individually grepped (no pub-API use outside crate seen).

**Stated limits.** `:1345` `unreachable!("boundary-call routing admits only boundary calls")`; `:2338` `unreachable!("projected field-store lowering receives only field stores")`; `:2596` `unreachable!("whole-root store lowering receives only primitive stores")`. These are panics on routing violations, not refusals.

**Fail-open risks.**
- `lower_normalized_foreign_structural_arguments` `:249-376` `descriptor_formal` branch: when `fixed_array_view_argument` returns `None` and the shared-buffer capacity path does not match, control falls through to the generic projection (`:377`). Classification: traced: generic projection then checks type equality at `:440-451`.
- `lower_boundary_call` `:1059-1076`: the guard is a negated-or condition on realization/result; outside the listed combinations errors. Not fail-open.
- `lower_ieee_float_constant` `:2277-2283` returns `DuplicateValue` when `nonreturning_boundary` (wrong error name but refuses).
- `known_integers::KnownUnitInteger::scalar_type` `:1382-1385`: `unreachable!` on a non-integer home; reached only if a caller stored a non-integer into an integer map. Classification: traced (`insert_scalar_home` `:1445-1447` routes only `Integer` homes to the integer map).

---

## S05 `input.rs` (77 lines; read 1-77)

**Purpose.** Binds the prepared plan to the installations admitted for its program, target and entry before selection reads either.

**Shape.** `SelectionInput {body, entry_execution, stack}` `:4`; `InstallationBindingError` (3 variants) `:18`; `BoundExecutionPlan { prepared, installations }` `:26` with `bind` `:34`, `plan`, `callable_invocations`, `receipt`, `installations`; re-export `BoundInstallations` `:14`.

**How it decides.** `bind` compares identities: `installations.psi != plan.psi`; `installed_entry` psi/target/machine must equal plan's; every `stack_requirements` row's psi/target must equal plan's (`:39-53`). Keyed on `Psi` identity, `NativeTarget` and `MachineId`.

**Unused / one-caller.** Grep `BoundExecutionPlan::bind` over `source-rust/*.rs`: 2 call sites (`select_instructions.rs:39`, `test_support.rs:15`). `SelectionInput` was not grepped.

**Fail-open risks.** `installed_entry.is_some_and(...)`: no installed entry is accepted (correct: optional). Stack-requirement list empty accepted. Classification: stated by type (optional fields).

---

## S05 `installation_resources.rs` (38 lines; read 1-38)

**Purpose.** Wraps `installation_admission::admit_planned_guards` so it receives the plan's identity and installation operation list.

**Shape.** `InstallationRequirements<'a> {interrupts, callbacks, entry_execution, stack}` `:12`; `bind(self, prepared)` `:20`; re-export `PlannedInstallationResources` `:9`.

**How it decides.** Delegated entirely to `admit_planned_guards(BoundInstalledEntry{psi,target,machine}, interrupts, callbacks, entry_execution, stack, &plan.installation_operations)` (callee not read).

**Unused / one-caller.** Grep `InstallationRequirements|BoundExecutionPlan::bind|installation_resources` over `source-rust/*.rs`: `select_instructions.rs:28` (production), `test_support.rs:3`, and one test `tests/physical_publication/mod.rs:321`: one production call site.

**Fail-open risks.** None seen here (returns the callee's `Result<_, &'static str>`).

---

## S05 `instruction_selection/mod.rs` (29 lines; read 1-29)

**Purpose.** Declares the selection module tree (10 submodules `:15-24`), a `failed_display!` macro `:3`, re-exports `native_geometry`, `structural_inputs`, `SelectedInstructionError`, `select_bound_instructions`, `ValidatedSelectedInstructions`.

**Shape.** `failed_display!` writes `"<message>: {:?}"`; used once in `selection.rs:1686`.

**Fail-open risks.** None seen.

---

## S05 `instruction_selection/register_constraints.rs` (72 lines; read 1-72)

**Purpose.** For each function, pins live scalar parameters that arrive in a register to that register's fixed view, so allocation sees the calling-convention constraint.

**Shape.** `selection_constraints(legalized, environment)` `:9` -> `SelectedSelectionConstraints {keys: environment.selected_keys(), fixed_inputs}`; `push_fixed_input` `:46` (dedup).

**Inputs/outputs.** Reads `function.parameters[*].placement.locations`, `value_transport::required_values(function)` (`selection.rs:1885`). Writes one `SelectedFixedInputConstraint` per register-placed required parameter.

**How it decides.** Only a parameter in the `required` value set and with exactly one `ValueLocation::Register { value_byte_offset: 0, .. }` is pinned (`:19-37`); `byte_size` is ignored (`byte_size: _`, `:25`). A register with no `fixed_register_view` in the environment is silently skipped (`:62-64`).

**Cases by shape.** "Exactly one register location at offset 0" (`:21-28`). Stack parameters and multi-register parameters are not pinned.

**Unused / one-caller.** Grep `selection_constraints\(`: 1 call (`selection.rs:2001`).

**Fail-open risks.** `push_fixed_input` `:62-64`: `let Some(fixed_view) = ... else { return; }` drops the constraint if the environment has no fixed view for the register. Effect: a required register-placed parameter is not pinned, so allocation could place it elsewhere. Classification: open (no comment; not traced whether any register used by the ABI lacks a fixed view; `ValidatedTargetRegisterEnvironment` not read). Also parameters with a multi-location placement are not pinned (`:21`); classification: open.

---

## S05 `instruction_selection/selection.rs` (2008 lines; read 1-2008, in two reads)

**Purpose.** Despite the header "We drive instruction selection (`select_instructions`)" (`:1-6`), this file is a collection of small inline modules used by the selection builders in sibling files, plus the stage's driver `select_bound_instructions` `:1997`. It contains atomic access form selection, basic-block ordering, byte-view homes, register-constraint helpers, plan construction, extent-copy sizing, foreign scalar backing analysis, receipt identity, integer-conversion mapping, the error type, parameter-use scan, place-declaration queries and `value_transport::required_values`.

**Shape.**
- `atomic_access::form` `:22` -> `Option<(AtomicForm, PrimitiveAccessGeometry, ClosedScalarType)>`.
- `block_order::derive` `:107`, `ordered_blocks` `:148` (+ 2 tests `:254,293`).
- `byte_view_homes` `:307`: `ByteViewHomes`, `own_view_root`, `view_backing_roots`, `bound_view_roots(_recurse)`, `edge_successors` `:485`.
- `constraints` `:511`: `instruction`, `instruction_from_row`, `require_key_rows` `:570`, `row`, `fixed_input_constraint` `:600`, `store_registers` `:617`.
- `construction` `:667`: `build_plan` `:679`.
- `element_view_homes::ElementViewHomes` `:757`.
- `extent_copy` `:772`: `INLINE_MAX_BYTES = 4096` `:782`, `chunks` `:784`, `bulk_operands` `:796`.
- `field_bytes` `:841`: `FieldBytes::resolve`, `root_type` `:894` (+ test `:934`).
- `foreign_scalar_backings` `:1011`: `ForeignScalarBackings::{new, retain_call, matches}` `:1040,1081,1171`, `inline_footprint` `:1349`.
- `identity::receipt` `:1377`; `integer_conversion::normalization` `:1412`; `model` `:1478` (`ValidatedSelectedInstructions`, `SelectedInstructionValidationReceipt`, `SelectedInstructionError` with 25 variants `:1562-1640`); `parameter_use::referent_used` `:1729`; `place_declaration` `:1826`; `value_transport::required_values` `:1885`; `select_bound_instructions` `:1997`.
- `pub(in crate::instruction_selection) use ...selection_inputs::*` re-exports (15 modules) so sibling files refer to `selection::X_input`.

**Inputs/outputs.** `select_bound_instructions(legalized: &BoundExecutionPlan, environment: &ValidatedTargetRegisterEnvironment)` reads the plan's `scalar_functions`, `borrowed_storage_layouts`, `profile`, `function_abis`, `publication`, `execution`; calls `selection_constraints` (`:2001`), `build_plan` (`:2002`; runs `scalar_graph::build_with_environment` per function, sorts functions by machine `:736`), `identity::receipt` (`:2003`); returns `ValidatedSelectedInstructions { plan: Arc, receipt }`. Doc comment `:1994-1996`: "Nothing from the source module, the abstract program or the optimization unit is passed here."

**How it decides.**
- `block_order::ordered_blocks` `:148-207`: identity of blocks by index in `source.blocks`; picks the lowest-index ready block (`BTreeSet::first`), falling back to the first unselected block in reverse postorder when cycles prevent readiness (`:198-206`). The two unit tests compare against a re-implementation of the previous order for all graphs with up to 4 blocks.
- `atomic_access::form` `:22-101`: all conditions are cross-checks: access form from event, root access equals `location.access`, result presence equals `observes_resident && single_attempt_custody().is_none()`, closed logical type matches, geometry offset/size equal the lowered row's `field_byte_offset` and `u32::from(form.width.bytes())`, offset aligned to size. Any mismatch -> `None`.
- `foreign_scalar_backings::matches` `:1171-1346` walks backward through call sites (`incoming` map) to ground every origin at an inline width; refuses (`Ok(false)`) on opaque ingress (`:1259-1263`), unresolved callable ingress, a machine with no incoming calls, a non-`Parameter` place kind (`:1318-1319`), or a descendant path (`:1299-1306`). Judgment keyed on `PlaceId`+path coordinates and `MachineId`.
- `constraints::store_registers` `:617-665` accepts a store row with 1 or 2 operands, operand 0 `Use`, optional operand 1 an early-clobber `Def` with no tie/fixed view; anything else -> `MissingConstraint(key)`. `bulk_operands` `:796-838` requires exactly 5 operands: indices 0-2 `Use`, 3-4 `Def` early-clobber untied.
- `extent_copy::chunks` `:784-794`: greedy widths 8,4,2,1.
- `require_key_rows` `:570-589` checks the 10 constraint keys exist in the catalog.

**Cases by shape.**
- `extent_copy::INLINE_MAX_BYTES = 4096` (`:782`) is an inline-copy cap with no comment; other files refuse larger copies (`selection_edge_transfers.rs:880`, `selection_views_and_bytes.rs:283` and re-export at `selection_structural.rs:3825`, grepped; 4 sites).
- `integer_conversion::normalization` `:1412-1461` accepts `ScalarRepresentationView`, `IntegerWiden`, `IntegerExactCast`; others `None`. Fixed widths limited to `{8,16,32,64}` for widen (`:1427`); exact-cast uses `exact_cast_has_native_carriers`.
- `register_form_normalization` `:1463-1475`: float `Binary32` -> `ZeroExtendU32`, `Binary64` -> `CopyI64`, other -> integer carrier normalization.
- `byte_view_homes::bound_view_roots_recurse` `:412-483`: a block-parameter view is grounded only if every incoming edge binds it with an empty path (`bound_edges > 0 && unbound_edges == 0`, `:479`); place kinds `Parameter | ByteSequenceLiteral | OperationResult` are roots (`:438-442`).
- `parameter_use::referent_used` `:1729-1824` enumerates ~35 `LegalizedScalarInstructionKind` variants by hand; `_ => false` `:1822` for unlisted variants.

**Same job elsewhere.**
- `byte_view_homes::edge_successors` `:485-509` is another successor enumeration beside `block_order::derive` `:115-131` (same three terminator shapes: Jump, Conditional, StructuralCase).
- `value_transport::required_values` `:1885-1979` and `parameter_use::referent_used` both walk every terminator kind; `foreign_scalar_backings::new` `:1055-1077` walks the same calls (instructions, invoke-no-normal, cleanup calls).
- `field_bytes::root_type` `:894` and `reference_input::root_type` (used at `:38`, in `selection_inputs.rs`, not read) are suspected duplicates of "root type of a place".
- Stage 04 `unit.rs`/`control_flow_storage.rs` repeat much of the same geometry; selection re-derives offsets from the plan rather than reading them (design stated at `:1994-1996`).

**Unused / one-caller.** Grep `block_order::derive|selection_constraints\(|INLINE_MAX_BYTES` over S05 `src`: `block_order::derive` 1 caller (`selection_scalar_graph.rs:3179`); `selection_constraints` 1 caller (`selection.rs:2001`); `INLINE_MAX_BYTES` 4 uses outside its definition (see above). `select_bound_instructions`: re-exported pub(crate) at `mod.rs:28`; caller in `select_instructions.rs` (not in my unit; inferred 1). The `#[allow(unused_imports)]` re-exports at `:668-677`, `:1986-1992` say these may be unused: not grepped.

**Stated limits.** `:1994-1996` doc comment as above. `:79-81` comment: "Preparation closed the cell's storage integer. A Boolean cell keeps its own type; every other atomic cell's logical type is that same integer." `:146-147` "Keep the existing preference for the lowest ready block."

**Fail-open risks.**
- `foreign_scalar_backings::retain_call` `:1128-1140`: if a callable-value invocation site is missing and the caller machine is not in `reached_machines`, the call is ignored (`return Ok(())`). Effect: a callee reached only through that missing site gets no incoming-call row, so `matches` refuses it later (`:1264-1270`, no calls -> `Ok(false)`). Classification: traced (refuses downstream).
- `foreign_scalar_backings::matches` `:1331-1345`: the final answer is `all(grounded)`; an empty origin list for a coordinate leaves `backing == false` and not grounded unless a dependency is, so refusal. Not fail-open.
- `parameter_use::referent_used` `:1822` `_ => false`: any `LegalizedScalarInstructionKind` variant added later that uses a place would be treated as "does not use the referent". Wrong input: a new instruction kind that reads a parameter's referent; the scan would say the referent is unused. Classification: open (callers of `referent_used` are in sibling files, not read).
- `value_transport::required_values` `:1885-1979`: `Terminator::Crash` contributes nothing (`:1967`); values read only by instructions not visiting operands via `visit_scalar_operands` would be missing, then `register_constraints` would skip pinning. Classification: open.
- `block_order::derive` `:110-112,143`: any missing block index -> `custody()` refusal. Not fail-open.
- `place_declaration::declares_*` return `false` on missing declaration. Not fail-open.

---

## Coverage

Read in full (all with the Read tool, in order):
- S04 `control_flow_storage.rs` 1-3744 (3744)
- S04 `coordination.rs` 1-702 (702)
- S04 `function_contract.rs` 1-229 (229)
- S04 `function_signature.rs` 1-1218 (1218)
- S04 `mod.rs` 1-678 (678)
- S04 `model.rs` 1-386 (386)
- S04 `references.rs` 1-2741 (2741)
- S04 `scalar.rs` 1-3041 (3041)
- S04 `structural_layout.rs` 1-1493 (1493)
- S04 `unit.rs` 1-2848 (2848)
- S05 `input.rs` 1-77, `installation_resources.rs` 1-38, `instruction_selection/mod.rs` 1-29, `register_constraints.rs` 1-72 (216)
- S05 `selection.rs` 1-2008 (2008)

Total read in full: 19,304 lines. Skimmed: 0. Not read: none.

Greps run: `selected_boundary_calls::resolve|fn resolve\b` (S04 src), `InstallationRequirements|BoundExecutionPlan::bind|installation_resources` (source-rust), `block_order::derive|selection_constraints\(|INLINE_MAX_BYTES` (S05 src), `derive_mixed...|derive_fixed...` (count, source-rust), `mod selected_boundary_calls|SelectedBoundaryCall\b` (S04 src, head 20), `OMEGA_TRACE_NATIVE_BOUNDARY|OMEGA_COMPILER_CHECK_SITES` (count, source-rust). No other call-site counts were taken; entries say so.

## Five things a maintainer most needs to know about this unit

1. Stage 04 target lowering refuses almost everything with one cause-less error: `LoweringError::unsupported_control_flow(machine)` (`model.rs:380`) is used throughout `control_flow_storage.rs`, `references.rs`, `scalar.rs`. The only cause information is the Rust call site recorded by `#[track_caller]`, printed only when `OMEGA_COMPILER_CHECK_SITES` is set (grep: 32 occurrences in 11 files for the two trace env vars). `scalar.rs:1053` `refusal` always returns `UnsupportedOperationInScalarFunction`.
2. Each control-flow module re-validates the same operand facts by hand: "unsigned 64-bit non-address index/length" appears 6 times in `control_flow_storage.rs` and again in `scalar.rs`; "length produced by a matching `StructuralByteSequenceFieldLength`" appears 4 times in `control_flow_storage.rs`; carrier sizes (`+8`, 16, 8) are hard-coded in `scalar.rs:82-98`, `unit.rs:352`, `control_flow_storage.rs:880` and `scalar.rs:1375`.
3. Silent narrowing points: `function_signature.rs:189-195` swallows any signature error and returns "no mixed ABI"; `register_constraints.rs:62-64` drops a fixed-register pin when the environment has no fixed view; `function_contract.rs:96-99` omits the structural contract when the function has no graph parameters and no declared places. None is commented. These were not traced to a concrete bad program.
4. Stage 05's `selection.rs` is not the selector; it is a collection of inline modules (about 20) plus the 12-line driver at `:1997`. Real instruction building lives in sibling `selection_*.rs` files (not in this unit). Selection reads only the prepared plan and register environment (`:1994-1996`); `BoundExecutionPlan::bind` (`input.rs:34`) only checks identities (psi, target, entry machine) of installations.
5. Reference tracking in `references.rs` is a catalog-plus-live-state design: `borrowed_source_catalog` and `stored_effect_catalog` (built once in `coordination.rs:601-613` from `optimization_unit_semantics::descriptor_results`) are consulted at every call/copy/observation; any unresolved case becomes a per-machine `Err(&'static str)` that is surfaced only when a call to that machine is lowered (`references.rs:1762`). Leaf budget `4096` is a magic number (`:2415`, `:2720`).

## What you are unsure about

- Whether the swallowed errors in point 3 (and `reference_projection_regions` `:182-189` `continue`) can let a wrong program through; the downstream refusals I inferred were not read in the sibling files (`control_flow.rs`, `control_flow_operations.rs`, `control_flow_calls.rs`, `scalar_sources`, `selection_inputs.rs`, `selection_scalar_graph.rs`).
- The call-site counts for most `pub(crate)` functions: I only grepped the names listed above. "Unused" is not asserted for any item.
- Whether `selected_boundary_calls` in `mod.rs:552` (index) and `control_flow_calls.rs:2082` (resolve) are meant to be one module: they share a name in two scopes (grepped).
- Whether `retained_private_references` (`mod.rs:320-323`) can see an unsorted `installation_operations` (the callee that sorts it is outside this unit).
- Counts of tests in `function_signature.rs` (13) were counted by reading `#[test]` markers, not by Grep count mode.
