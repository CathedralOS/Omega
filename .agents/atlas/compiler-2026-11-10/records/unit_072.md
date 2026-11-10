# Unit 72 notes

Covers (all paths under `omega-rust/psi/compilation/pipeline/`, checkout worker-23 at main 457adf4a11f):

1. `06_checked-trees-to-lowered-psi/src/unit_operation_frame.rs` (8356 lines)
2. `06_checked-trees-to-lowered-psi/src/unit_structural_values.rs` (8411 lines)
3. `07_lowered-psi-to-lowered-psi/src/control_flow_and_retention.rs` (965)
4. `07_lowered-psi-to-lowered-psi/src/lib.rs` (113)
5. `07_lowered-psi-to-lowered-psi/src/psi_optimization.rs` (100)
6. `07_lowered-psi-to-lowered-psi/src/scalar_and_value_passes.rs` (634)
7. `08_lowered-psi-to-terminal-psi/src/lib.rs` (140)
8. `08_lowered-psi-to-terminal-psi/src/result_policy_verification.rs` (528)

All 19,247 lines were read in full (Read tool, in order). Greps are cited with the pattern used. Line numbers below are file-local; "F1" = unit_operation_frame.rs, "F2" = unit_structural_values.rs, "CF" = control_flow_and_retention.rs, "SV" = scalar_and_value_passes.rs, "PO" = psi_optimization.rs, "L7" = stage 07 lib.rs, "L8" = stage 08 lib.rs, "RP" = result_policy_verification.rs.

---------------------------------------------------------------------------------------------------

## 1. F1 `unit_operation_frame.rs` (8356 lines)

### Purpose
Header comment (F1:1-5): "Unified `OperationFrame` emitter for ordinary and composed Unit machine bodies". It lowers each checked Unit "effect operation" (call, store, local establishment, atomic access, register access, reference formation and release, cleanup) into Terminal Psi `Operation`s appended to an `OperationBuffer`, and publishes the structural results/places those operations create. The whole file is one `pub(crate) mod attached_unit { pub(crate) use crate::unit::attached_unit::*; pub(crate) mod operation_frame { ... } }` (F1:7-9) with about 17 nested sub-modules, each adding `impl OperationFrame` blocks. It is `mod unit_operation_frame;` in stage 06 lib.rs:44 and re-exported through `attached_unit.rs:40`.

### Shape (main types and entry points)
- `OperationFrame<'f,'c>` (F1:7342): borrowed bundle of `checked: &CheckedTrees`, `machine`, `state` symbols, `scalar_result`, `parameters: Cow<[StructuralParameterDeclaration]>`, `structural_types: StructuralTypeRoster`, `results: StructuralResults`, `private_places`, `windows: &mut BorrowedWindowLedger`, `evaluation: &mut Evaluation`, `values`, `next_place/value/block/edge` counters, `calls: &mut CallEmissionContext`, `operations: &mut OperationBuffer`, `callees: Callees`, `caller: CallerCustody`.
- `OperationEnvironment` (F1:7389) and `OperationEmission` (F1:7409): the two halves of a frame after `split()` (F1:7455). `reborrow()` for each (F1:7421, 7576). Frames are rebuilt by struct literal in 3 places: `reborrow` (7422), `OperationEnvironment::capture_argument` (7615), `OperationEnvironment::emit_operand` (7665).
- `StructuralResults` enum (F1:2325): `Dense(&mut Vec<(Option<StructuralPlaceDeclaration>, bool)>)` or `StateGraph { state, places }`. Two numbering regimes for call results (ordinary vs composed control state). Grep `StructuralResults::StateGraph|StructuralResults::Dense` in stage 06 src: 17 hits (F1 14, attached_unit.rs 2, unit_composed_control.rs 1).
- `StructuralTypeRoster` (F1:7752): `Owned(&mut Vec)` or `Published(&[..])`.
- `CallInputs` (F1:1037), `CompletedStructuralArgument`/`ArgumentProgress` (F1:1683-1697), `BoundaryParameters` (F1:7333), `Callees` (F1:7371), `CallerCustody` (F1:7382).
- Dispatch: `OperationFrame::lowers` (F1:7773) lists which checked operations the frame lowers directly; `emit` (F1:7800) matches `CheckedUnitEffectOperationPlan` (CaptureAssignmentDestination, RegisterAccess, Store, EvaluateStructuralValue, EstablishLocal, MoveStructuralField, ContinuationCleanup, EstablishViewSubslice, EstablishReference, ReleaseReference, AtomicFence, AtomicAccess; everything else -> `unsupported("operation frame received a call operation")`, F1:7901). Calls go to `emit_call` (F1:1046).
- Sub-modules (with what they hold): `activation_storage` (F1:35; Release/Establish/BorrowBytes/Split/Merge of activation claims), `atomic_access` (F1:314), `boundary_calls` (F1:574), `byte_operands` (F1:840), `calls` (F1:1015; `emit_call`, `bind_scalar_result`, `publish_result`, `publish_completed_result`, `emit_structural_call_result`, `unit_call`, `scalar_call`), `completed_arguments` (F1:1675; `capture_argument`, `selected_memberships`, `emit_snapshot`), `dynamic_calls` (F1:2086), `structural_results` (F1:2311), `suspension_frontier` (F1:2727; `retain` at 2809), `descriptor_tables` (F1:3037), `loan_completions` (F1:3101; `publish_loan_completions`, `publish_call_ensures_guarantees`), `locals` (F1:3330; scalar/array/byte/structural local establishment, window move/store, `store_structural_field`, `rebind_structural_binding`, `continuation_cleanup`), `owned_initialization` (F1:4741), `register_access` (F1:4792; register ops and global descriptor table), `selection_retirement` (F1:4975), `stores` (F1:5146; `store`, primitive/field/byte/indexed/representation-view/sum-field stores), `values` (F1:6867; `evaluate_structural_value`, `establish_structural_value`, `produce_value`, `prepare_value_observation`).
- Free helpers at end: `structural_type_declaration` (F1:8263), `window_place` (8274), `allocate_structural_result` (8288), `last_operation_id` (8315), `block_parameter_declaration` (8326), `structural_parameter_declaration` (8337).

### Inputs and outputs
Reads: the checked fact tables (`checked.facts.flow.*`, `facts.qualifications.content`, `facts.carry.suspension_crossings`, `facts.service_reaches.rows`, `facts.state_locals`, `facts.values.structural_values`), `Evaluation` state (scalar/structural bindings, claims, loan captures, structural_locals/parameters, selection_cleanups), `CallEmissionContext` (claims, machine_ids, boundaries, cleanup_receivers), and the `terminal_semantics` structural-type queries (`runtime_structural_path_tip`, `present_structural_membership_contract`, `store_preserves_domain_content`, ...). Writes: pushes `Operation::unsuspended(...)` into `OperationBuffer`, pushes `ValueDeclaration`s into `values`, `StructuralPlaceDeclaration`s into `private_places`/`results`, registers source-call records (`record_source_call` F1:1435), claim publications (`evaluation.current_claims.publish`), `pending invocation_completion` for external or no-normal completions, and cleanup regions through `evaluation.continue_cleanup_regions`.

### How it decides (what the judgments are keyed on)
- Call route: `match (call.route(), call.result())` (F1:1082-1164): `DescriptorTable`, `ActivationStorage`, `Module`(Unit/Structural/NoNormal | Scalar), `Boundary`(Unit | Scalar | Structural; NoNormal refused F1:1124-1126), `Dynamic`. Keyed on checked route/result enums.
- Callee lookup keyed on machine symbol: `callees.boundaries.iter().find(|b| b.source == target_machine)` (F1:789-796, 661-666), `lookup_machine_id`, `unique_unit_boundary`. Scalar target is cross-checked against signature: `target.entry_state()? != *realization_state` (F1:1609) and primitive-type equality (F1:1610, 1626-1635, 1636-1647).
- Structural identity lookup: `lookup_type_id(self.type_ids, &type_identity)` throughout; `identities.intern(&field_identity.spelling())` for fields (F1:6191, 6724) and `literal.case_identity.spelling()` (F1:6701): keyed on source SPELLING interned into the identity table. Primitive carrier by name: `PrimitiveType::from_name(carrier.name())` (F1:4900, 4919).
- In `byte_descriptor_operand` the shared-descriptor test compares `declaration.identity == identities.intern(&checked.type_identity_text(source.type_identity).unwrap_or_default())` (F1:893-900): identity text re-interned; missing text interns "" (no match => refusal F1:912).
- Store routing keyed on `CheckedStoreLeaf` variant (F1:5176-5270) and `CheckedStoreRoot::{Parameter, Local}`.
- Suspension frontier (F1:2809-3034): keyed on `(machine, state, statement_index, call_ordinal)` against `checked.facts.carry.suspension_crossings`; each live value classified Parameter / Local / CallArgument by `SuspensionCrossingValueOrigin`, then located by structural_local root symbol, structural_parameters position, primitive_storage symbol, or scalar position (`unaddressed_value`).
- Atomic: carrier cell found by `terminal_semantics::atomic_cell_type(...).filter(scalar_type == ...)` (F1:385-393); resident-range proof by `atomic_source_range_decided` (F1:476-485).
- Owned rebind: dominating declaration keyed on `local.statement_index < statement_index && stored_structural_identity(...) == type_identity` (F1:4354-4359).

### Cases by shape (fixed lists that refuse the rest)
- Activation storage: only five actions (F1:79-249). Release: "exactly one continuing claim", `transfer.argument_index == 0 && path.is_empty()` and source must be `Owned` (F1:82-94). Establish: exactly one evaluated `u64` scalar, no structural args (F1:114-124). Outcome carrier must be a `Sum` with exactly two cases (F1:304-310: "activation outcome has no exact case roster"). Split needs one claim + one committed scalar; Merge exactly two claims (F1:184-233).
- Atomic events: Load, Store, ReadModifyWrite {FetchAdd, FetchSub, FetchAnd, FetchOr, FetchXor}, Swap, CompareExchange, CompareExchangeOnce (F1:415-472). `CompareExchangeOnce` must have a structural result and no scalar prior (F1:494-503).
- Register access: `match (operation, inputs.as_slice())` over nine arities (F1:4854-4896), else "register access disagrees with its evaluated operands".
- `copied_sum_value` (F1:6623): source must be a `Parameter` and path only `Field` segments (F1:6630-6645).
- `established_case_literal` (F1:6668): only scalar/IeeeFloat/BoundedInteger/RefinedScalar target fields (F1:6729-6737), operand must be "an exact branch-free scalar operand" (F1:6748-6753).
- `owned_structural_value` (F1:4260): only `StructuralResult`, `Parameter`, `StructuralLocal` sources; `_` => "borrowed-window repair value is not a whole owned result" (F1:4327-4329).
- `store_structural_field` (F1:4050): path may not cross `Referent` or `FixedRange` (F1:4102-4110); multiplicity branch Unrestricted (leaf copy) vs other (capture as arrival) (F1:4166-4241).
- `rebind_structural_binding` (F1:4334): destination Local or Parameter only; shared-borrow ingress rebinding is handled only for ElementView or ByteSequence::BorrowedView families (F1:4459-4474).
- `byte_element_target` (F1:5806): Local with non-empty path => `unsupported("a local byte field element has no store route")` (F1:5816).
- `release_reference`: requires `Reference` shape or `descriptor_cleanup` leaf (F1:8130-8136). `retire_selection_complements`: leaf must be a `Reference` (F1:5044-5048).
- `projection` of `view_element_target`: parameter needs `mutating_access_pair_pattern!()` and `nonlinear_multiplicity_pair_pattern!()` (F1:5852-5861).
- `refine_scalar_local_value`: mutable local with refinement predicates => `unsupported("mutable scalar value refinements require invariant-window custody")` (F1:3488-3492).
- Dynamic call: family operands must be `CheckedCallScalarArgument::Pure` (F1:2199-2208); no-normal dynamic callee refused (F1:2171-2175).

### Same job elsewhere
- Two frontier walkers: `completed_arguments::capture_argument` (F1:1700-1992) loops over `completed_place::plan(...)` frontiers (Selector / Capture / Value) and calls `completed_address::capture_into`; F2 `completed_projection::complete_projection` (F2:431-567) does the same loop and calls `completed_address::capture`. F2:331-333 says "Value reads and dispatch captures share this evaluator" but F1's `capture_argument` keeps its own loop (both read).
- `StructuralLeafCopy` is emitted by hand in F1:1001 (`byte_descriptor_operand`), F1:2078 (`emit_snapshot`), F1:4185 (`store_structural_field`), F1:6658 (`copied_sum_value`), F2:7318 (`copied_leaf_place`). Grep `OperationKind::StructuralLeafCopy|O::StructuralLeafCopy` under stage 06: also `emission.rs:11732`, `unit_composed_control.rs:9190` (not read; suspected same job).
- The "captured_store_place" splice (`windows.captured_store_place` then `receiving_place` for logical and physical, set destination fields) is written three times: `primitive_store` (F1:5414-5433), `representation_view_store` (F1:5752-5778), `scalar_field_store` (F1:6168-6210).
- `release_reference` (F1:8139-8173) and `retire_selection_complements` (F1:5095-5117) each build a `CleanupRegion` choosing `descriptor_retirement` vs `DiscardResidual`.
- `boundary_scalar_call` (F1:647) and `scalar_call` (F1:1580) both build a scalar `ValueDeclaration` from a declared result signature and call `record_source_call`.
- Per the repo test `calls_have_one_emitter_and_one_argument_constructor` (stage 06 shared.rs:2321-2370, read) there must be one direct-call construction and one `structural_argument` constructor; F1 is listed in `CALL_FILES` (shared.rs:2325). That test scans source lines for `" StructuralArgument {"`; it is a text scan, not a type-level guarantee.
- `selected_memberships` (F1:1995) vs `fn selected_memberships` at `emission_cleanup.rs:2369` and the `descriptors::selected_memberships` module (terminal-psi semantics `descriptors.rs:1667`), suspected related, not read.
- Header says "Unified ... for ordinary and composed Unit machine bodies". Grep `OperationFrame` in stage 06 src: count 56 = F1 41, unit_composed_control.rs 9, attached_unit.rs 6. `.emit_call(` callers outside F1: `attached_unit.rs:12140`, `unit_composed_control.rs:846` (grep `\.emit_call\(`). So the "unified" claim has two outside users, both found.

### Unused or one-caller code
- `OperationFrame::consumes_prepared_operands` (F1:7767): grep `consumes_prepared_operands` not run; not asserted either way.
- `emit_snapshot` (F1:2039): grep `emit_snapshot\(` under psi: 1 call (F1:1951) + a re-export alias in `attached_unit.rs:10155` (`emit_snapshot as emit_argument_snapshot`); alias use sites not counted.
- `selected_memberships` (F1:1995): 3 calls in F1/F2 (F1:1895, F1:2053, F2:385, F2:501 = 4 in total across the two files).

### Stated limits (quotes under 20 words)
- F1:3489 "mutable scalar value refinements require invariant-window custody"
- F1:5816 "a local byte field element has no store route"
- F1:1125 "boundary calls cannot carry a module no-normal result"
- F1:2172 "dynamic invocation has no exact checked no-normal callee"
- F1:4109 "owned store operand crosses borrowed storage"
- F1:8135 "loan retirement requires a descriptor carrier"
- F1:6736 "case literal operand requires a scalar carrier"
- Roughly 250 `unsupported(...)`/`LoweringError::Unsupported(...)` sites (grep `unsupported\(|LoweringError::Unsupported\(` count: 250 in F1; the count includes the helper's own uses).

### Fail-open risks
1. Suspension frontier drops live values silently (F1:2809-3034). (a) `let Some(crossing)=... else { return Ok(Default::default()) }` (F1:2813-2828): a call with no matching crossing record gets an empty frontier. Input: any call not listed in `facts.carry.suspension_crossings`. This function runs for every call (`source_call_record`, F1:1403), so "no crossing" is the normal case. (b) Inside the loop: `CallArgument => continue` (2889), mutable/erased/non-primitive parameter => `continue` (2971-2979), non-scalar local => `continue` (2996-2998), `unaddressed_value(...)` None => `continue` (3011-3017). A live value that crossed a suspension but has no located home is dropped from the frontier rather than refused. Status: open (no comment; I did not trace the consumer of `LoweredSourceFrontierBindings`).
2. `publish_call_ensures_guarantees` (F1:3148-3327) skips (`continue`) when the fact has no place, no domain membership, no home, when `source_requirements.represents(...)`, or when `representation.is_complete()` (F1:3178-3228). Effect: a call-ensured membership is not recorded. A dropped fact only removes a membership later consumers could use; it does not add one. Status: open (consumer side not traced).
3. `emit_call` post-processing is skipped when `invocation_completion.is_some()` (F1:1165-1167): by design for nonreturning calls (the completion is held pending). Status: stated by code shape only.
4. `release_reference` skips the whole cleanup when `already_released` (F1:8085-8091), keyed on `cleanup_source_is_retired` and empty path. Status: open (retirement bookkeeping not traced).
5. `retire_selection_complements` skips duplicates by `(selection, place, path)` (F1:5081-5094). Status: traced locally (the earlier entry emitted the region).
6. Dispatch of `bind_scalar_result` drops the value when `discards_result(...)` (F1:1226-1230): by design.

None of these accept a program that violates a proof obligation by themselves; (1) is the one where a missing home yields less retained state instead of a refusal.

---------------------------------------------------------------------------------------------------

## 2. F2 `unit_structural_values.rs` (8411 lines)

### Purpose
Header (F2:1-4): "Structural value construction, record and structural-case emission, element views, reference formation, and completed aggregate storage in attached Unit bodies." It lowers a checked structural value tree (`CheckedStructuralValueHandle`, kind enum `CheckedStructuralValueKind`) into Terminal operations producing a place, tracking ownership claims, memberships (domain qualifications), cleanup, content transformations and join blocks. `pub(crate) use crate::unit::*;` at F2:6; module `attached_unit::structural_values` (F2:8-10).

### Shape
- `Emission<'a,'b,'calls>` (F2:5764): the per-value emitter: `checked`, `machine`, `state`, `statement`, `structural_type`, `multiplicity`, `structural_types`, `type_ids`, `domain_ids/declarations`, `environment: &mut OperationEnvironment`, `calls`, `evaluation`, `values`, counters, `operations`, plus private `sources`, `owners`, `result_access`, `published_places`, `completed_projections`, `selection_route`.
- Entry functions: `emit` (F2:4760) -> `emit_source` (F2:4832); `receive_completed` (F2:4787); `OperationEnvironment::produce_value` (lives in F1, F1:7039, and calls F2's `emit`). `Emission::value` (F2:6393) -> `raw_value` (F2:6448) which matches every `CheckedStructuralValueKind`: NativeCodeLayout, QuotientCall, QuotientAdaptation, ViewSubslice, BorrowedSliceView, MutableSliceView, Reference, Record, FixedArray, Call, Projection, CopiedStructuralPlace, ZeroedScalarArray, Scalar, ScalarArray, ByteLiteralView, ZeroedStructural, Place, Case, StructuralCase, Dispatch (F2:6470-7109).
- Types: `ValueProduction` (F2:4740), `ValueDestination {Binding, Operand, Evaluation}` (F2:4733), `CompletedValue` (F2:4753), `ValueContinuation` (F2:5833; join block + parameters + claims), `FallbackContinuation` (F2:5846), `ProjectionSource` (F2:5853), `CompletionOccurrence`, `ValueCompletion {NoNormal, Awaiting, Complete}` (F2:5804), `NormalDeathReceipt` (F2:5816), `ValueOutcome<T>` (F2:8406), `CompletedOperand` (F2:7929), `CompletedAggregate` (F2:33).
- Sub-modules: `completed_aggregate` (F2:13), `emission` (F2:285) containing `completed_projection` (330), `completed_storage` (819), `completions` (1419), `content_transformations` (1850, with `boundary_correspondence` 1853), `copied_place` (2686), `current_memberships` (2828, with `descriptor_bindings::unchanged` 2837), `dispatch` (3438), `element_views` (3689), `parameters` (3974), `quotient_calls` (4189), `reference` (4237), `residual_complement` (4470), `subslices` (4600); then `field_carrier` (7639), `fixed_array` (7674), `locals::local_binding` (7781), `member_memberships` (7837), `operand_claims` (7924), `record` (8027), `structural_case` (8219).

### Inputs and outputs
Reads checked structural value nodes (`checked.facts.values.structural_values`), ownership/permission events (`flow_facts(checked).ownership`), cleanup control, content-transformation facts (`facts.qualifications.content`), flow `control.operands` for descriptor capture and storage snapshots, domain declarations. Writes operations (`EstablishRecord`, `EstablishStructuralCase`, `EstablishStructuralArray`, `EstablishScalarArray`, `EstablishZeroedStructural`, `TransformContent`, `MoveStructuralValue`, `StructuralLeafCopy`, `EstablishNativeCodeLayout`, comparisons and conditional blocks), `private_places`, claim bindings, `arrival_claims`, `block_entry_claims`, `claim_edge_bindings`, evaluation `structural_locals`, `retired_structural_homes`.

### How it decides
- Source coordinate check: every value node must match `(machine, state, statement_index)` of the emitter or the lowering refuses (F2:4859-4866; F2:6400-6408; F1 counterpart).
- Which carrier an owner has: keyed on place (`owner.value.place == argument.place`) and symbol (`owner.symbol`) in `prepare_owners` (F2:5426-5762).
- Result access: ordered choice at F2:4991-5005: completed source => Owned; borrowed alias; stored descriptor => Owned; view loan; SharedBorrow+Unrestricted; else Owned.
- Borrowed alias recognition (F2:4975-4987) keyed on node kind `Reference{...}` and `lookup_type_id(type_ids, &source.type_identity)? == structural_type`.
- Dispatch arms (F2:3574-3669): keyed on arm position (`position + 1 < arms.len()`) and `CheckedScalarDispatchPattern::{Predicate, Value}`; routing records `SelectedDecision{expression, decision: position, selected}` (F2:3638-3653).
- Memberships: `occurrence_memberships` (F2:2967-3263) keeps a membership only if it still holds at the consuming occurrence; keyed on domain id, binding `semantic_domain`, storage snapshots and constraint contexts (`MembershipEvidence`), `same_contents` (F2:3412) comparing captured references/projections.
- Path matching by spelling: `membership_path_matches` (F2:3351-3409) compares physical path segments against source place segments via `identities.text(*identity) == Some(member.spelling().as_str())` (F2:3372, 3392); `membership_source_path` (F2:3291) rebuilds `CheckedMemberIdentity::of_spelling(identities.text(..))` from the interned text (F2:3302-3309, 3322-3328). Record/case field lookup: `candidate.identity == identities.intern(&identity.spelling())` (F2:8102, 8338; case F2:8294).
- Content ancestry producer lookup keyed on `(state_symbol, statement_index, call_ordinal)` via `call.matches_statement(...)` (F2:2616-2622), and for body transformations on equality of output claim ids (F2:2641-2644).
- Domain error text uses `display_path(machine, "::")` only for diagnostics (F2:1057-1058, 5036-5037).

### Cases by shape (fixed lists / refusals)
- `projection_selector` (F2:594): only `ComputedIndex`, `EvaluatedFixedIndex`, `RuntimeIndex(StructuralField | StorageRead)`; others: "projection frontier has no executable selector" (F2:691-694). Selector must be an integer, non-address (F2:662-668).
- `complete_projection`: `Frontier::Value` => "projection address evaluation cannot manufacture a value copy" (F2:538-541).
- Dispatch comparison (F2:3597-3635): only Boolean and Integer scalar equality; else `unsupported("structural dispatch needs selected floating comparison lowering")` (F2:3629); an arm with `equality_use.is_valid()` is refused (F2:3609-3615).
- `subslice_value` (F2:4616): family only `ByteSequence` or `ElementView` (F2:4632-4645); access must be shared or mutable view; "subslice cannot amplify its completed source access" (F2:4686-4692); completed source must carry no claims (F2:4664-4668).
- `quotient_call` (F2:4194): argument must carry no custody and empty path (F2:4216-4218).
- `quotient adaptation` (F2:6523-6530): "requires a whole custody-free copy value".
- `ZeroedScalarArray` (F2:6814): element must be an integer literal zero (F2:6822-6829).
- `record` (F2:8041) and `structural_case` (F2:8231): field declarations matched by interned member spelling; erased fields skipped in record (F2:8106-8111) but refused in case (F2:8343-8345).
- `fixed_array` (F2:7687): carrier must be `FixedArray`, source carrier a `TypeReferenceNode::FixedArray` (F2:7698-7710).
- `completed_aggregate::append_fields` (F2:140): non-erased declaration count must equal initializer count (F2:153-155).
- `retain_checked_memberships` (F2:1155): "owned membership transport cannot establish routed authority" if a missing domain has `establishment_routes` (F2:1223-1230); non-Unrestricted multiplicity requires Owned custody (F2:1237-1242).
- `prepare_owners` (F2:5426): any partially moved root whose residual complement is non-empty => refusal (F2:5537-5550).
- `ProjectionSource`/`projection_source` (F2:4080): Place source must be "whole owned custody" (F2:4117-4124).
- `reference_value` (F2:4252): source form switch on referent shape: `PrimitiveScalar` with `ScalarParameter` source, `PrimitiveScalar` with `PrimitiveLocal` source, else a general prepared borrowed argument (F2:4359-4404).
- `borrowed_view_join_access` (F2:5385): only `ElementView{access: None}` and `ByteSequence::{BorrowedView,BoundedView}{access: None}`; Linear multiplicity => `None`.

### Same job elsewhere
- `record` (F2:8041-8163) and `structural_case` (F2:8231-8399) are near-identical field loops (both read); F1:6668 `established_case_literal` is a third construction of case field initializers (scalar-only).
- Three hand-built `ValueContinuation` struct literals: `materialize_completed_operand` (F2:7177), `direct_borrow` (F2:7233), `root_value_continuation` (F2:7567) (grep `ValueContinuation \{` under psi: those 3 plus the definition at 5833).
- `complete_value` (F2:1698) and `complete_loan` (F2:1736) both produce a jump to the continuation block with membership join and edge claim aliasing; they differ in claims/reference_backings handling (forward vs record).
- `copied_leaf_place` (F2:7260) vs `emit_snapshot` (F1:2039): both emit `StructuralLeafCopy` with `selected_memberships`-style membership selection.
- `completed_projection::complete_projection` vs F1 `capture_argument` (see F1 section).
- `move_completed_operand` (F2:1288) and `transform_content` (F2:2295) both emit a structural result and call `retire_consumed_structural_arguments` plus claim replacement.
- `current_memberships::descriptor_bindings::unchanged` (F2:2837) is a walk over `control.operands` backing chain; the checked-side validation crate also has `validation::completed_storage` (used at F2:3012, 3118) (not read).

### Unused or one-caller code
- Grep `structural_values::(emit|receive_completed|ValueProduction|local_binding)|ValueProduction` outside F2 (stage 06 src): callers all in F1 (`local_binding` F1:1315, 3586; `receive_completed` F1:4760; `emit` F1:6916, 6987, 7121). So the F2 entry points have exactly 3 `emit` callers and 1 `receive_completed` caller, all in F1.
- `fresh_value_declarations` (F2:5405): used at F2:7170, 7232, 7538 (3 sites, read).
- `ValueCompletion::into_argument`, `Emission::remap_saved_scalar_fields` (F2:6376): used by record/case (read).

### Stated limits
- F2:5547 "selection pass-through of partially moved roots requires retained residual frontier correspondence"
- F2:3629 "structural dispatch needs selected floating comparison lowering"
- F2:5588 "selection home requires an owned ingress frontier"
- F2:2004 "boundary body input needs an exact original projection correspondence, not only joint ancestry"
- F2:1227 "owned membership transport cannot establish routed authority"
- F2:4665 "view descriptor unexpectedly carries an owned content claim"
- F2:2260 "content transformation requires an exact finite owned path"
- ~199 `unsupported(`/`LoweringError::Unsupported(` sites (grep count).
- F2:7065-7069: when `value_facts(...).call_schedule(value)` is `None`, an env var `OMEGA_COMPILER_CHECK_SITES` makes the compiler `eprintln!` a "CHECKSITE" trace (debug hook inside product code; grep `OMEGA_COMPILER_CHECK_SITES` shows it used in 14 files).

### Fail-open risks
1. Dispatch last arm is taken without testing its pattern (F2:3593-3596 and F2:3616-3634): the conditional split is emitted only `if position + 1 < arms.len()`. For the final arm a `Predicate` is evaluated (`self.scalar`) and checked Boolean, and a `Value` pattern is evaluated, but no branch tests them. A program whose final arm is not exhaustive/default would run that arm for non-matching subjects. Status: open (no comment at the site; relies on a checker exhaustiveness guarantee I did not trace).
2. `prepare_owners::available` (F2:5439-5552): `if !symbol.is_valid() { return Ok(true) }` (F2:5442-5444): owners with an invalid (unnamed) symbol are always kept live, skipping the retired-binding, conflicting-selection and vacancy checks. Status: open (the `existing.symbol.is_valid()` handling at 5560 shows unnamed owners exist, but I did not trace where they are created).
3. `content_transformation` (F2:2268-2293): `!node.content_transformation.is_valid()` => `Ok(None)` means "no transformation", so a value whose handle was left invalid skips content derivation silently. Status: inferred benign (handle is checked-stage output), open.
4. `admit_fresh_operand` (F2:1615-1647): if the producer has no declared source claims but `completed_value_paths(...)` is empty, publishes empty claims. Status: traced locally (empty required paths means nothing to publish).
5. `capture_operand_claims` (F2:7987-8024): for `Unrestricted` multiplicity claims are `Vec::new()` by construction (copy values hold no custody). Status: stated by construction.
6. `occurrence_memberships` returns empty memberships when any consumer capture is not `unchanged` (F2:3023-3029): drops facts, conservative.
7. `descriptor_bindings::unchanged` (F2:2837-2914): every unknown/invalid path returns `false`; only a fully verified chain returns `true`. None seen as fail-open.
8. `boundary_body_custody` (F2:1869-2117): `let ContentBodyCustody::Boundary{..} = &derived.custody else { return Ok(ContentTransformationCustody::Derived) }` (F2:1879-1885): any non-Boundary custody is treated as `Derived`. Status: stated by the enum (two-variant match inferred, not read in its definition).

---------------------------------------------------------------------------------------------------

## 3. CF `control_flow_and_retention.rs` (965 lines)

### Purpose
Header (CF:1-4): control-flow cleanup and "proof-value and ranking coverage retention policies". Two top-level modules: `control_flow_cleanup` (CF:6) folds conditional branches on constant Booleans and prunes unreachable machines; `retained_identities` (CF:719) answers "which values/blocks/operations may a pass not delete or rename".

### Shape
- `control_flow_cleanup::cleanup::cleanup(machine, evidence, sidecar_operations, sidecar_values)` (CF:14): per-machine branch folding.
- `control_flow_cleanup::machines::prune(module, application_roots, sidecar_*)` (CF:171): drop unreachable machines (fixpoint over roots and evidence-bound machines).
- `control_flow_cleanup::retention`: `BlockLocalEvidence {blocks, edges, operations, values}` (CF:259), `block_local_evidence[_over_machines]` (CF:282, 286), `retained_machines_with_roots` (CF:368), `machine_retention_roots` (CF:393, private), `machine_transitions` (CF:534, private), `machine_evidence_bound` (CF:577).
- `control_flow_cleanup::cleanup(before: LoweredPsi)` (CF:625): the pass entry (called by PO:34).
- `retained_identities::proof_values` (CF:720): `retain_proposition`, `retain_contract_values`, `retain_erased_arguments`, `call_erased_arguments`, `retain_crash_routes`, `crash_continuations`. `ranked_coverage::{RankedCoverage, ranked_coverage}` (CF:814-826). `record_named_rows` (CF:862), `has_boundary_application` (CF:871), `boundary_application_results` (CF:880), `contains_source_call` (CF:891), `prune_debug_map` (CF:915).

### Inputs and outputs
`cleanup(LoweredPsi)` clones `before`, collects "sidecar" ids from `before.source_call_occurrences` (terminal sites and `frontier_before_call` values) and `selected_comparison_occurrences` (CF:630-646), builds `BlockLocalEvidence` from the module's side tables (named rows, callback registrars, interrupt registrations, scalar qualification coercions/returns/entry ranges, block invariants, crash contracts, proof output calls, reborrow restored uses, `TransformContent` ancestry, closed reach calls, content partition compositions, machine contracts; CF:290-365). It mutates machines and `debug_map` and returns the new `LoweredPsi`.

### How it decides
- Branch folding (CF:20-80): skip machines with `ranked_scc` (CF:20-22). Constant source: only `OperationKind::BooleanConstant` results (CF:26-31). Fold a `Conditional` whose condition is a known literal, but not if the untaken edge id is in `evidence.edges` (CF:54-57). After folding, the set of blocks that become unreachable (`stranded_blocks`, CF:83) is removed only if every one satisfies `removable_block` (CF:132-164): not in `evidence.blocks`, no `structural_parameters`, no operation with `static_reach_binding`, in evidence/sidecar sets, or with a `Structural` result; no scalar result/param in evidence/sidecar values; no edge in evidence. Keyed on ids (BlockId/OperationId/ValueId/EdgeId), not names.
- Machine pruning (CF:178-211): roots from `machine_retention_roots` (module tables: entry, scalar applications, quotient machines, dynamic-dispatch rows, target families, qualification rows, invariants, crash contracts, suspension sites/plans, proof output calls, closed conformance, reborrow uses, access view inputs, evidence lanes, provider candidates, float meaning projections `Direct*` owners, machines with `attachment` or `ranked_scc`; CF:393-532) plus `application_roots`; transitive closure via `machine_transitions` (module invocations, selected evidence uses, cleanup `InvokeNominal`, closed reach application callees; CF:534-575). Then machines not kept are added back if `machine_bound` (evidence or sidecar mention them; CF:224-247), iterating to a fixpoint (CF:182-210).
- After pruning: `proof_scope.callables` rows pointing to removed `Machine(body)` are removed (CF:212-221). `debug.sites` filtered by surviving subjects, `_ => true` keeps other subject kinds (CF:703-711); `debug.semantic` re-identified (CF:712-713).

### Cases by shape
- `cleanup` folds only `Terminator::Conditional` with a literal Boolean (CF:36-48). No other constant source.
- `FloatMeaningSource` root list: six `Direct*` variants add roots; `TransitionalInput`, `ExactBinary32Literal`, `ExactBinary64Literal`, `SemanticApplication` explicitly add none (CF:500-525).
- `machine_transitions` call_structural_pair_pattern!: only that operation family contributes selected evidence uses (CF:539-545).

### Same job elsewhere
- Reachability BFS in `stranded_blocks` (CF:94-110) duplicates the notion in `terminal_semantics::control_graph` (`finished_order`/`reverse_postorder`, control_graph.rs:221, 320); suspected, not read beyond signatures.
- "Values a pass must not touch": `block_local_evidence` here, plus per-pass retained sets in SV (copy propagation SV:139-193, GVN SV:370-399, dead-scalar SV:600-611) and `terminal_semantics::scalar_demand::record_named_rows` (called at CF:863). Four near-copies of "retain contract values, crash routes, erased args, source-call frontier". See SV.

### Unused or one-caller code (greps)
- `retained_machines_with_roots`, `machine_retention_roots`, `block_local_evidence`, `machine_evidence_bound`: grep `retained_machines_with_roots|machine_retention_roots|block_local_evidence|machine_evidence_bound` across omega-rust: all hits in CF only (calls at CF:178, 190, 202, 230, 647). So the retention policy has one consumer: `control_flow_cleanup::cleanup`.
- `ranked_coverage`: grep `ranked_coverage|has_boundary_application|contains_source_call|boundary_application_results`: `ranked_coverage` 2 call sites (SV:140, SV:371); `has_boundary_application` 1 external call (SV:49) + 1 internal (CF:886); `contains_source_call` 2 calls (SV:152, SV:394); `boundary_application_results` 2 calls (SV:560, SV:601).
- `prune_debug_map` (CF:915): 3 calls (SV:344, 564, 631).
- `retain_proposition` etc. are used by CF and SV (SV imports them, SV:126-129, 351-354).
- `RankedCoverage.blocks` etc.: read in SV.

### Stated limits
None with explicit "unsupported" words. `ranked_scc` machines are skipped by branch folding (CF:20) and by constant folding (SV:18), copy propagation and GVN skip when `ranked_coverage` is `None` (SV:139-143, 370-374).

### Fail-open risks
1. `debug.sites.retain(... _ => true)` (CF:710) and `prune_debug_map` `_ => true` (CF:959): debug-map sites whose subject kind is not listed are kept even if their machine was pruned. Effect is on debug metadata only. Status: open.
2. `prune_debug_map` pairs `before.machines.zip(after.machines)` and `old_machine.blocks.zip(new_machine.blocks)` positionally (CF:930-952). Valid only if the pass removes neither machines nor blocks; true for copy/GVN/dead-scalar (they only edit operations/params) as read, so traced for the three callers.
3. Pruning direction: `retained_machines_with_roots` over-retains by listing many tables; a missing table would cause a needed machine to be removed (fail-closed for acceptance but could delete required code). I did not verify the root list is complete. Status: unsure.
4. `removable` returns `false` when `machine.block(&id)` is `None` (CF:126-128): keeps. Fail-closed.

---------------------------------------------------------------------------------------------------

## 4. L7 `07.../src/lib.rs` (113 lines)

### Purpose
Crate root for stage 07 (`lowered-psi-to-lowered-psi`). Comments (L7:1-23) say only `run_psi_optimization` can construct `PsiOptimizationStageResult`.

### Shape
`mod control_flow_and_retention; mod psi_optimization; mod scalar_and_value_passes;` (L7:27-29); `pub(crate) use` of cleanup, retained_identities and the four passes (L7:31-35). `edge_arguments` module (L7:37-80): `drop_removed_positions(arguments: &mut Vec<ValueId>, removed: Option<&Vec<usize>>)` (L7:42) and `drop_removed_positions_across_edges(machine, &BTreeMap<BlockId, Vec<usize>>)` (L7:55). `optimization_error::PsiOptimizationStageError {InvalidSemantic, InvalidDebugMap, InvalidExecutionRecord}` (L7:86-90). Public re-exports `PsiOptimizationStageResult`, `run_psi_optimization`, `PsiOptimizationExecutionIdentity`, `PsiOptimizationExecutionRecord` (L7:111-113).

### How it decides
`drop_removed_positions_across_edges` rewrites argument lists of `Jump` and `Conditional` edges, keyed on target `BlockId` (L7:60-75). `_ => {}` skips every other terminator (L7:76).

### Unused or one-caller
- `drop_removed_positions_across_edges`: grep `drop_removed_positions_across_edges|drop_removed_positions\(` under psi: 1 outside caller (SV:282). `drop_removed_positions` only called inside L7.
- `PsiOptimizationStageError::InvalidDebugMap` (L7:88): grep not run for construction; within unit files it is never constructed (I saw only `InvalidSemantic` constructed, CF:713, 962, SV:117, PO:70; `InvalidExecutionRecord` PO:62). Not counted across omega-rust.
- `run_psi_optimization`: grep count across omega-rust: 8 hits in 4 files (rules.md 1, terminal-production lib.rs 2, PO 2, L7 3). One real caller crate: `psi-compiler/terminal-production/src/lib.rs`.

### Fail-open risks
`drop_removed_positions_across_edges` ignores `StructuralCase`/`InvokeNoNormal` terminator edges (L7:76). If a collapsed block parameter's block were also the target of one of those, argument lists would keep a stale position. Copy propagation guards this: blocks targeted by `StructuralCase` are excluded from collapse (SV:159-160, 187-189, 219-221). `InvokeNoNormal`/`ExternalCompletion` carry no block target in the code read. Status: traced for the one caller.

---------------------------------------------------------------------------------------------------

## 5. PO `psi_optimization.rs` (100 lines)

### Purpose
Coordinator for the Psi optimization stage: runs the selected passes over `LoweredPsi` and seals an execution record.

### Shape
`run_psi_optimization(lowered, selections) -> Result<PsiOptimizationStageResult, ...>` (PO:18); `carrier_identity` (PO:66); `PsiOptimizationStageResult {lowered, execution}` (PO:75; `#[must_use]`; `lowered()`, `selections()`, `execution()`, `into_parts()`).

### How it decides
Iterates `selections.as_slice()` in given order (PO:24); dispatch by `PsiOptimization` enum (PO:32-52): ControlFlowCleanup, SparseConditionalConstantPropagation, CopyPropagation, GlobalValueNumbering, DeadPureScalarElimination run passes; ProofCheckElision, StateSpecialization, RepresentationSpecialization, LoopInvariantScalarMotion are no-ops (PO:48-51). `rewritten` is set true for every selection except those four (PO:25-31), then the output identity is recomputed only if `rewritten` (PO:55-59); else the output identity equals input identity.

### Cases by shape
Four `PsiOptimization` variants are accepted as selections and do nothing (PO:48-51). Grep `PsiOptimization::(ProofCheckElision|...)` shows those variants referenced in 5 files (incl. this file 8 times), meaning other stages (omega 01 rules, optimization-core) implement them in different pipelines; not read.

### Fail-open risks
"Selected but not implemented" optimizations are silently recorded as executed in `PsiOptimizationExecutionRecord` (PO:60-62: record is built from `selections` regardless). Status: open (record meaning not read; input equals output identity in that case, PO:58).

---------------------------------------------------------------------------------------------------

## 6. SV `scalar_and_value_passes.rs` (634 lines)

### Purpose
Four target-neutral passes over `TerminalMachine`s inside `LoweredPsi`: sparse conditional constant propagation (SV:7), copy propagation (SV:123), global value numbering (SV:349), dead pure scalar elimination (SV:569). Each module has a `propagate/number/eliminate(before: LoweredPsi)` entry that clones, edits machines, and fixes the debug map.

### Shape
- `sparse_conditional_constant_propagation::{folding::fold, propagate}` (SV:14, 107).
- `copy_propagation::{copies::propagate, resolve, resolve_in, propagate}` (SV:134, 285, 293, 328).
- `global_value_numbering::{equivalents::deduplicate, reverse_postorder, dominators, number}` (SV:365, 463, 485, 557); `Leader {kind, block, result}` (SV:359).
- `dead_scalar_elimination::{liveness::eliminate, eliminate}` (SV:575, 593).

### Inputs and outputs
Input `LoweredPsi` (module + debug_map + `source_call_occurrences`). Output new `LoweredPsi` (clone edited). Uses `terminal_semantics::{constant_goal_free_scalar_leaf, ScalarLeafLiteral, is_unconditionally_total_scalar, control_graph::finished_order, scalar_demand::{complete_scalar_demand, qualification_parameter_blocks, retained_values_by_machine, ranked_coordinates}}`.

### How it decides
- SCCP (SV:14-77): skips `ranked_scc` machines (SV:18-20). Seeds literals from `IntegerConstant`/`BooleanConstant` results; rewrites an operation into a constant when `constant_goal_free_scalar_leaf(operation, &literals, &value_types)` returns a literal, except operations with `static_reach_binding` or in `boundary_application_demands` (SV:48-55). Fixpoint loop.
- Copy propagation (SV:134-283): requires `ranked_coverage(machine)` is `Some`, else returns (SV:139-143). Builds `incoming_arguments` per target block from `Jump`/`Conditional` edges; blocks targeted by `StructuralCase` are excluded (SV:159-160, 187-189, 219-221). A block parameter collapses to a single source when all (transitively resolved) incoming arguments are the same value (SV:308-318), the parameter is not in `covered_parameters`, `retained_values`, or `coverage.member_uses` (SV:237-242), and the source declaration has the same `scalar_type` and `qualifications` (SV:245-250). Keyed on ValueId and declared type + qualifications.
- GVN (SV:365-461): processes blocks in reverse postorder; only operations that are `is_unconditionally_total_scalar` and have no `static_reach_binding` (SV:413-416). Operation `kind` equality after substituting representatives; leader must dominate and have same scalar type and qualifications (SV:425-432); a result in `retained_values`, `coverage.blocks` or `coverage.member_uses` is never removed (SV:422-424).
- Dead scalar elimination (SV:593-633): delegates to `complete_scalar_demand(..., DropUnusedTotalOperations)` (SV:580-585) with retained values per machine and protected blocks (`qualification_parameter_blocks`), searched by `partition_point`/`binary_search_by_key` over sorted `(owner, ...)` vectors (SV:619-624).

### Cases by shape
- SCCP only folds `IntegerConstant` and `BooleanConstant` seeds (SV:26-34); output constants only Integer/Boolean (SV:61-68). Float constants are not seeds.
- Copy propagation is the only pass that rewrites block parameters and edge argument lists.

### Same job elsewhere
- Dominators and reverse postorder implemented locally in GVN (SV:463-550) while `terminal_semantics::control_graph::{reverse_postorder (control_graph.rs:320), dominators (control_graph.rs:368, returns DominatorTree), finished_order (221)}` exist. GVN's `reverse_postorder` already calls `control_graph::finished_order` (SV:472) but builds its own successor map and its own `dominators` (BTreeSet fixpoint). Grep `control_graph::(dominators|reverse_postorder)` shows 7 outside users (scalars.rs:2375, content_transformations.rs:122, 889, stage 06 retention.rs:6028, and 3 in omega provider-admission). Grep `fn (reverse_postorder|dominators)\b` also finds separate implementations in omega pipeline 01/02 and optimization-unit crates (not read). I did not read control_graph bodies.
- "Retain proof values" preamble repeated: copy propagation SV:139-157 + 190-193; GVN SV:370-399 (includes Crash site_guard loop SV:386-392); dead elimination SV:600-611 (uses different calls: `retained_values_by_machine`, invariants, frontier). All three duplicate set-building from `crash_continuations`/`call_erased_arguments`/`retain_contract_values`.
- `propagate` (SCCP) re-identifies debug map directly (SV:115-118) while copy/GVN/dead call `prune_debug_map` (SV:344, 564, 631).

### Unused or one-caller
- `retained_identities::proof_values::retain_*` callers listed above (read).
- `edge_arguments::drop_removed_positions_across_edges`: 1 caller (SV:282).

### Stated limits
None phrased as unsupported. Passes skip rather than refuse.

### Fail-open risks
1. `copies::resolve_in` (SV:293-322): cycle detection `visiting.insert(value)` false => returns `value` unchanged (fail-closed). `.expect("an inventoried edge argument list is never empty")` (SV:311-313) panics if `raw[value]` is empty; `raw` rows come from `edges.iter().map(...)`, non-empty because `incoming_arguments.get(&block.id)` only exists when at least one edge was pushed (SV:222-231). Also `arguments[position]` (SV:226) panics if an edge has fewer arguments than the block has parameters. Status: traced (panic, not accept).
2. Copy propagation and loops: a block parameter whose incoming edges are all back-edges (a loop header reached from entry via a path that is not an incoming edge of that block) would resolve over those edges only. Whether the entry block can have parameters while also being a jump target I did not trace. Status: unsure.
3. SCCP: `constant_goal_free_scalar_leaf` returning `None` => no fold (fail-closed). `value_types` missing entries are the callee's concern (not read).
4. GVN: `dominators.get(&block_id).is_some_and(...)` fail-closed (SV:427-429). Blocks not reachable are not in `order` and never leaders.
5. Dead elimination: `qualification_parameter_blocks(...)` returning `None` returns `before` unchanged (SV:594-597); same for `retained_values_by_machine` (SV:612-617). Fail-closed.

---------------------------------------------------------------------------------------------------

## 7. L8 `08.../src/lib.rs` (140 lines)

### Purpose
Final target-neutral stage: seal `PsiOptimizationStageResult` into either a portable `PSITERM\0` artifact or an invocation-local code carrier, and bind/rejoin program-entry receipts (L8:1-22).

### Shape
`entry_receipt` module: `ProgramEntryTerminalReceiptError` (L8:35-43; variants TerminalEntryMultiplicity, NonUnitEntry, MissingEntryDeclaration, TerminalIdentity, ArtifactSemanticIdentityMismatch, ContextBridgeMultiplicity, ContextBridgeOccurrenceMismatch), `prepare_program_entry_receipt(declaration: &PreparedNominalRoot, source_signature_identity: [u8;32], artifact)` (L8:66), `rejoin_program_entry_receipt(receipt, artifact)` (L8:90). `publish_artifact::finalize_terminal_program(optimized, publication)` (L8:104). `pub use result_policy_verification::verify_boundary_result_policies` (L8:140).

### How it decides
- `prepare_program_entry_receipt`: `declaration.resolve(artifact.semantic_module())` must find exactly one entry; entry result must be `TerminalMachineResult::Unit` (L8:74-76); entry must have a source declaration (L8:77-80). Receipt records source signature identity, source machine name, source machine, the artifact's semantic identity, and entry id (L8:81-87).
- `rejoin_program_entry_receipt`: only compares `artifact.manifest().semantic()` with `receipt.terminal_psi_identity()` (L8:94-96).
- `finalize_terminal_program`: first `terminal_semantics::activation_storage::validate_module(&module)` over the whole module (L8:113-117), then `TerminalPublication::Portable` => `CanonicalTerminalArtifact::from_parts(module, execution, debug_map.as_ref())`; `Invocation` => `InvocationTerminalCode::new(module)` (L8:118-131). Validation errors are wrapped as `CodecError::MalformedStructuralFoundation(reason)`.

### Unused or one-caller (greps)
- `finalize_terminal_program`: grep across omega-rust: 5 hits in 3 files (L8 3, terminal-production lib.rs 1, omega compiler `product_boundary.rs` 1).
- `prepare_program_entry_receipt|rejoin_program_entry_receipt`: 9 hits in 3 files (L8 5, terminal-production lib.rs 2, custody.rs 2).
- Error variants `TerminalIdentity`, `ContextBridgeMultiplicity`, `ContextBridgeOccurrenceMismatch`: grep `ContextBridgeMultiplicity|ContextBridgeOccurrenceMismatch|NonUnitEntry|TerminalIdentity\(` across omega-rust: each of the first two plus `TerminalIdentity(` appears only at its declaration (L8:39, 41, 42); no construction site anywhere in omega-rust. `NonUnitEntry` constructed once (L8:75).

### Stated limits
L8:12-15: "Source-entry receipt preparation and boundary result-policy verification remain separate from canonical artifact sealing". (Design statement, not a limit.)

### Fail-open risks
`rejoin_program_entry_receipt` (L8:90-98) accepts when the semantic identity matches; it does not re-resolve the entry. Status: stated by name; what the identity covers is outside this unit. Otherwise none seen.

---------------------------------------------------------------------------------------------------

## 8. RP `result_policy_verification.rs` (528 lines)

### Purpose
Independent check that every float-returning boundary call in the Terminal module still has exactly the result adapter (trapping or saturating) its checked source selection demanded, and that adapters match a roster of source occurrences.

### Shape
`verify_boundary_result_policies(checked, module, source_custody) -> Result<(), &'static str>` (RP:277); private `requirement_identity` (RP:500); module `saturation_eligibility` (RP:12) with `verify` (RP:22), `definition`, `unsigned_constant`, `is_byte_one`, `byte_predicates`, `predicate`, `image_comparison`, `constant_conjunction`; `enum Predicate {Finite, Nonzero}` (RP:17).

### Inputs and outputs
Reads `CheckedTrees` (operator selections via `facts.operators.for_each_resolved_expression_selection`, typed declarations, control-flow call tables, machine supply modes), `TerminalModule` (machines, boundary declarations, identities text) and `LoweredSourceCustody.source_call_occurrences`. Output: `Ok(())` or a static error string.

### How it decides
- Expected count `source_policies` (RP:282-304): source occurrences with an `Expression` site whose resolved selection has the same target and a policy other than `None`.
- For each `BoundaryCall` with an IEEE float scalar result (RP:313-326) it finds exactly one source occurrence with the same terminal operation id (RP:327-333), reads the checked selection (RP:337-353), checks target equality with `source.source_target` (RP:365), the unique call in the control table with matching `statement_index`, `call_ordinal`, `authored_expression`, `target_symbol` (RP:368-391), and compares `module.identities.text(declaration.identity)` with `requirement_identity(checked, target)?` (RP:392-399): a TEXT compare of identity strings. Then policy float format equals result format (RP:400-409), the next operation in the block must be exactly `FloatResultPolicy {policy: expected}` (RP:455-461), adapter result differs from raw and has same scalar type and empty qualifications (RP:466-472), and no other operation or the terminator may use the raw result (RP:473-489). Finally `matched == actual && matched == source_policies` (RP:494).
- `saturation_eligibility::verify` (RP:22-74) pattern-matches the definition tree of the "eligible" Boolean: either a Boolean constant equal to the evaluated conjunction of Finite/Nonzero predicates on constant float arguments, or `IntegerEqual(byte_one, AND-tree of ScalarRepresentationView-of-(not(equal(masked bits, constant))) )` yielding exactly the expected predicate list; budget of 4096 definition lookups (RP:48, 81).

### Cases by shape
- `verify` accepts argument types only `IeeeFloat` or `Integer` (non-divisor) (RP:33-44); divisor must be a float (an integer divisor falls to `_ => return false`).
- `image_comparison` recognizes only bit widths 32 and 64 with exponent masks `0x7f80_0000`/`0x7ff0_0000_0000_0000` and magnitude masks `0x7fff_ffff`/`0x7fff_ffff_ffff_ffff` (RP:187-195, 220-224); anything else yields `None`.
- Policies handled: `FloatTrappingNonFinite`, `FloatSaturatingOverflowOnly`; `None` skipped (RP:410-454).
- Eligible-expression shape is exactly the "byte one equals AND-tree" form (RP:62-73): a fixed program shape; any other shape fails closed.

### Same job elsewhere
- `source.matches_checked_origin(checked, origin)` used at RP:297, 345 (both read); the matching "occurrence vs checked selection" is also done in stage 06 emission (suspected; not read).
- `validation::float_saturation_divisor_parameter` (RP:419) is the checked-side twin of the divisor shape.

### Unused or one-caller
`verify_boundary_result_policies`: grep: 4 hits in 2 files (RP 2, L8 2); no caller found in omega-rust outside this crate (no hit in compiler/pilot code). It appears to be exposed but not called by anything in omega-rust. (Grep `verify_boundary_result_policies` count mode returned only these two files.) Status: grepped; callers outside this crate (e.g. xtask or tests in other dirs) not searched.

### Stated limits
None phrased as unsupported; errors: "float call lost its result adapter", "saturating result changed its exact input eligibility", etc.

### Fail-open risks (what wrong program gets through)
1. RP:334-336 `let Some(NominalMachineUseSite::Expression(expression)) = source.source_site else { continue; }`: a float `BoundaryCall` whose unique source occurrence has a non-Expression site is skipped without adapter verification. The `source_policies` count (RP:286-288) also excludes non-Expression sites, so the counts stay consistent and a missing adapter at such a site would not be caught. Reachability: whether a non-Expression site can carry a float result policy was not traced (policies are looked up only via `for_each_resolved_expression_selection(expression, ...)`, which needs an expression). Status: open.
2. RP:354-364: no checked selection for a float call and the target is not an operator declaration => `continue` (unchecked). By design: non-operator machine calls carry no policy. Status: inferred.
3. RP:321-326: a `BoundaryCall` whose result is not a float scalar is skipped. By design.
4. RP:400-402, 411: policy with no float format or `None` => `continue`. If an adapter were nonetheless present it counts in `actual` but not in `matched`, so the final equality fails (RP:494): fail-closed by count.
5. `saturation_eligibility::verify` returns `false` on every unknown (budget exhaustion, non-unique definition, unexpected shape, constants of other format): fail-closed.

---------------------------------------------------------------------------------------------------

## Coverage
- Read in full: F1 1-8356 (8356), F2 1-8411 (8411), CF 1-965 (965), L7 1-113 (113), PO 1-100 (100), SV 1-634 (634), L8 1-140 (140), RP 1-528 (528). Total 19,247 lines.
- Skimmed: none. Not read: none.
- Additional reads outside the unit: `06.../src/shared.rs` lines 2320-2379 (the "one emitter" test). Not part of the unit.

## Five things a maintainer most needs to know about this unit
1. F1 and F2 are the Unit-body lowering core: F1 turns every checked Unit effect operation into Terminal operations (~250 refusal sites), and F2 builds structural values (records, cases, arrays, views, references, dispatch joins) for it. F2's three entry points (`emit` x3, `receive_completed` x1, `local_binding` x2) are all called from F1 only.
2. Two numbering regimes for structural call results (`StructuralResults::Dense` vs `StateGraph`, F1:2325) run through most of both files; ordinary and composed Unit bodies share `OperationFrame` but behave differently in `earlier`, `completed_namespace`, `value_storage`, `completed_value`.
3. Several jobs are written more than once: frontier walking (F1:1823-1991 vs F2:441-566), `StructuralLeafCopy` emission (5 sites in F1/F2 plus 2 elsewhere in stage 06), the captured-store splice (3 copies in F1), `record`/`structural_case`/`established_case_literal`, three `ValueContinuation` constructions, and four passes each rebuilding "values not to touch" in stage 07.
4. Stage 07 passes are conservative by construction: they skip machines with `ranked_scc`, never touch values in `record_named_rows`, contract/crash-route/erased-argument/source-call-frontier sets, and never merge anything but `is_unconditionally_total_scalar` operations. GVN has its own dominators/reverse-postorder while `terminal_semantics::control_graph` already exports both (7 users).
5. Findings worth tracing further: dispatch's last arm is not tested (F2:3593-3634), suspension frontier drops un-located live values (F1:3011-3017), `prepare_owners` treats unnamed owners as always live (F2:5442-5444), `PsiOptimization` no-op variants are recorded as executed (PO:48-62), and `verify_boundary_result_policies` skips non-Expression sites (RP:334-336) and has no caller found in omega-rust.

## What you are unsure about
- Whether the dispatch last-arm behavior is backed by a checker exhaustiveness guarantee (not traced).
- Whether the entry block of a machine can have block parameters and also be a `Jump` target (affects copy propagation's `raw` construction, SV:222-231).
- Whether `retained_machines_with_roots` root list covers every table that can reference a machine (CF:393-532).
- Who consumes `LoweredSourceFrontierBindings` and what an incomplete frontier does downstream (F1:2809-3034).
- The bodies of `terminal_semantics::control_graph::dominators`/`reverse_postorder` (only signatures read), so "duplicate" for GVN is suspected not confirmed.
- Whether anything outside omega-rust (xtask, tests, bootstrap) calls `verify_boundary_result_policies`.
- Counts of `unsupported(` are text-pattern counts and include non-refusal uses of the helper identifier.
