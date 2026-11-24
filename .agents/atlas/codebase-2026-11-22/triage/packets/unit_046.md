# Packet unit_046: Psi descriptor results and replay

Status: Packet complete. All correctness observations in record unit_046 and their digest 07 entries have a receipt below. One receipt (11) is still open; the rest are intended behavior, and earlier "stated", "traced" and "closed" classifications are kept.

This packet reviews the correctness observations in [record unit_046](../../records/unit_046.md) (area `07-psi-semantics`, crate `terminal-semantics`, files under `source-rust/subsystems/compiler/psi/semantics/src/descriptors/`, written `D/` below) and the matching bullets in [digest 07](../../digests/07-psi-semantics.md). The survey read commit `4f3c255d`; every receipt below was checked against `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. No supplemental gap ranges are assigned to unit_046 in `second_pass_ranges.tsv`. Items in digest section 5 that cite files outside this unit (`D/replay/case_flow.rs`, `D/replay/engine.rs:2632`, `D/replay/summary_interface.rs`) belong to other packets and are not reviewed here.

Outcomes use the packet vocabulary: fixed, intended, duplicate, feature gap, reproduced, still open. "Intended" means the code refuses or stays conservative for the case the record worried about, with the source or test that shows it.

## Receipts

### 1. Descriptor graphs default to an empty list

- **Evidence:** record `D/descriptor_results.rs` "Fail-open risks" bullet 1 and "Five things" item 2; digest 07 lines 132, 171, 269 and section 10 item 12 (line 291).
- **Current source:** `D/descriptor_results.rs:1214-1223` (`terminal_graph`, `terminal_graph_from`) and `:1662-1668` (`terminal_reference_graph`, `terminal_borrowed_storage_graph`) still call `unwrap_or_default()`.
- **Contract:** a machine whose descriptor graph could not be built must not be read as "nothing to check".
- **Trace:** every production caller of the four builders reaches the graph through `descriptor_machine` or `DescriptorAnalysis`, and both report a missing machine. `Replay::machine` (`D/replay/engine.rs:763-770`) returns `None` for a missing or duplicated row, so `payload_targets` (`D/replay/analysis.rs:827-839`) and the other analysis queries return `None`, and `case_is_established` (`:340-351`) returns `false`. The callers turn that into a refusal:
  - `psi/semantics/src/content_transformations.rs:137` → `current_selected_payload_contract` errors with "selected payload backing is unresolved" (`D/descriptor_results/payload_history.rs:520-523`).
  - `psi/semantics/src/calls/evidence_calls/terminal_inputs.rs:160-161` errors with "runtime evidence body has no current descriptor machine".
  - `psi/artifact/wire/src/semantic_module.rs:1119-1140` errors when `analysis.result_slots(row)` is `None`.
  - `psi/artifact/wire/src/foundation_validation.rs:3186` refuses when `case_is_established` is false, and `:7033` refuses when `element_payload_memberships` is `None`.
  - `psi/semantics/src/references/reference_origins/memberships.rs:110` and `psi-to-native/verification/provider-admission/src/access_lifecycles.rs:109` pass the graph to replay-based queries that error on a missing input (`bind_profile` maps each `None` to a lifecycle diagnostic).
  - `psi-to-native/.../ranked_cycles_validation.rs:819` calls a different, native `graph::terminal_graph`.
- **Outcome: intended.** The empty list loses the reason for the failure, but no traced consumer accepts a program because of it. The digest's correction that `terminal_graph_from` also defaults (section 10 item 12) still holds.

### 2. `terminal_definition` treats unlisted place kinds as locals

- **Evidence:** record `D/descriptor_results.rs` "Fail-open risks" bullet 2 and "What you are unsure about" bullet 2; digest 07 line 133.
- **Current source:** `D/descriptor_results.rs:1723-1782`; the catch-all is `:1781`. The unlisted kinds of `StructuralPlaceKind` (`foundation/semantic-records/src/storage.rs:735-762`) are `Result`, `ProviderAttachment` and `TrivialAffineLocal`.
- **Contract:** `DescriptorDefinition::Local` lets `Replay` leave a write out of the callee's published write frame (`D/replay/engine.rs:727-728`), so only storage the caller cannot reach may be `Local`.
- **Trace:** `TrivialAffineLocal` is callee-local storage, and `terminal_flow.rs:77` already treats it as independently owned. A `Result` place reaches the caller through result slots, not as caller storage. `ProviderAttachment` is the only kind that could name storage outside the callee, and published Psi never contains one: wire validation refuses every machine that has one with "provider-backed attachment specialization is incomplete" (`psi/artifact/wire/src/foundation_validation.rs:3582`, `:12300-12311`), the lowered-form checker refuses it as "a root place kind outside this route" (`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places.rs:2007-2010`), and `plain_affine` refuses "affine provider storage" (`psi/semantics/src/plain_affine.rs:1739-1741`).
- **Outcome: intended** for current inputs. If provider-backed attachments are admitted later, this arm needs an explicit `ProviderAttachment` row; that is part of the same feature, not a present defect.

### 3. `edge_feasibility` and `case_feasibility` disagree on a missing state

- **Evidence:** record `D/replay/analysis.rs` "Fail-open risks" bullets 1-3; digest 07 line 143 (first half).
- **Current source:** `D/replay/analysis.rs:243-273` (`flow_feasibility`), `:274-315` (`edge_feasibility`, `ProvedImpossible` at `:300`), `:316-339` (`case_feasibility`, `Unknown` at `:336`).
- **Contract:** `ProvedImpossible` may be returned only for a position the replay proved unreachable.
- **Trace:** all three return `Unknown` unless `prepare_states` succeeded. `prepare_states_checked` stores `PreparedStateCache::Ready` only after its worklist empties (`D/replay/engine.rs:3043-3288`); every bound, depth limit or unreconstructable step returns `None` and leaves `Failed`. Inside the loop a successor is skipped only when `case_flow::select` proves the case false (`:3221-3233`). A position with no `before` state in a ready snapshot is therefore unreachable, so `ProvedImpossible` is sound. `case_feasibility` returning `Unknown` for the same position is less precise, not unsound.
- **Outcome: intended.** The difference costs precision only.

### 4. `is_descriptor` returns false for an unknown or repeated type

- **Evidence:** record `D/descriptor_results.rs` "Fail-open risks" bullet 3; digest 07 line 134.
- **Current source:** `D/descriptor_results.rs:234-243` (`is_descriptor`) and `:279-295` (`is_shared_descriptor`).
- **Contract:** a type that might be a view must not be treated as plain data.
- **Trace:** neither case reaches these functions in a decoded module. Wire validation builds `StorageTypeGraph::new` over every structural type and refuses a repeated id with "structural type graph repeats an identity" (`psi/artifact/wire/src/foundation_validation.rs:11992-11994`, `:13470`; run from `semantic_module.rs:4210`). It refuses unknown parameter, result and attachment types through `has_structural_type` (`foundation_validation.rs:2671`, `:2685`, `:2818`, `:772`).
- **Outcome: intended.** Validation establishes the precondition. The duplicated "exactly one declaration" scan is a code-quality item, not a correctness one.

### 5. Empty and literal loans pass the compatibility check

- **Evidence:** record `D/descriptor_results/compatibility.rs` "Fail-open risks"; digest 07 line 135.
- **Current source:** `D/descriptor_results/compatibility.rs:92-113`; the `Empty`/`Literal` arm is `:98-101`.
- **Contract:** a loan the callee requires apart from a demanded argument region must not overlap that region.
- **Trace:** the demand is always an exact `Argument` region (`:67-76`). An `Empty` origin names no storage. A `Literal` origin names a `ByteSequenceLiteral` place, which is a separate root that `plain_affine` admits only as claim-free shared byte storage (`psi/semantics/src/plain_affine.rs`, "literal backing requires a claim-free shared byte declaration"). Neither can overlap an argument region. The arms that can overlap (`Argument`) or are unknown (`Provider`, `AccessFormation`, `Invocation`) refuse, and incoming origins propagate upward.
- **Outcome: intended.**

### 6. `terminal_payload_contract` uses the first producing operation

- **Evidence:** record `D/descriptor_results/contents.rs` "Fail-open risks" bullet 2; digest 07 line 136.
- **Current source:** `D/descriptor_results/contents.rs:1399-1444`.
- **Contract:** the contract must come from the only operation that establishes the place.
- **Trace:** a place has only one establishing row in checked Psi. `plain_affine::verify` refuses a place with two custody declarations ("a place has two custody declarations", `psi/semantics/src/plain_affine.rs:1505-1521`), and every operation result and byte literal registers one (`:1595-1640`). The lowered-form checker runs it on every machine (`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/verdict.rs:1015`, `:1072`). Wire validation also refuses duplicate producers for an operation-result place (`foundation_validation.rs:3842-3870`).
- **Outcome: intended.**

### 7. `owned_shape` accepts `AtomicCell`

- **Evidence:** record `D/descriptor_results/owned_shape.rs` "Fail-open risks"; digest 07 line 137. `Opaque => true` stays stated.
- **Current source:** `D/descriptor_results/owned_shape.rs:87`.
- **Contract:** "owned shape" means moving the value exposes no nested reference or view.
- **Trace:** `AtomicCellType` holds only `Boolean`, `Integer` or `BoundedInteger` (`foundation/semantic-records/src/storage.rs:13-17`), so it can carry no descriptor. The same function already accepts atomic fields (`StructuralFieldType::Atomic`, `:51`).
- **Outcome: intended.**

### 8. A scalar write with several destinations leaves an old observation

- **Evidence:** record `D/descriptor_results/payload_history.rs` "Fail-open risks" bullet 4 and "What you are unsure about" bullet 3; digest 07 line 138.
- **Current source:** `D/descriptor_results/payload_history.rs:988-1023` (skip at `:1001-1003`).
- **Contract:** a write that may reach a location must retire every observation there; only a write with one known destination may install a new value.
- **Trace:** each operation that records `scalar_payload_writes` also retires its targets earlier in the same step. `WriteOnlyPrimitiveStore` and `StructuralScalarFieldStore` push a `payload_invalidations` row (`D/descriptor_results/terminal_flow.rs:661-687`), and the history loop applies it first (`payload_history.rs:785-826`): `retire` (`:1449-1458`) drops overlapping scalar observations at every target, and an unresolved target clears every contract. Call postconditions (`terminal_flow.rs:331-345`) are retired by the call's write frame, or by clearing everything (`payload_history.rs:751-783`). Establishment rows (`terminal_flow.rs:1314-1343`, `:1440-1466`) write a fresh producer place, which skips `storage_origins` (`payload_history.rs:992-995`). The skip only declines a weak update after the old value was already retired.
- **Outcome: intended.**

### 9. `TerminalScalarExpressions::expression` reads the first definition

- **Evidence:** record `D/descriptor_results/payload_predicates.rs` "Fail-open risks" bullet 1; digest 07 line 139.
- **Current source:** `D/descriptor_results/payload_predicates.rs:464-496`; `MachineScalarIndex::definitions` (`psi/semantics/src/bytes/byte_sequence_predicates.rs:1702-1719`).
- **Contract:** each scalar value has a single definition.
- **Trace:** the lowered-form checker refuses "the lowered form declares one value twice" (`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/minted.rs:1264-1290`), covering formals, results and operation results.
- **Outcome: intended.**

### 10. A caller adopts the callee's `ensures` clauses

- **Evidence:** record `D/descriptor_results/scalar_contracts.rs` "Fail-open risks"; digest 07 line 140.
- **Current source:** `D/descriptor_results/scalar_contracts.rs:20-42`, `:48-130`.
- **Contract:** [control flow](../../../../../wiki/spec/terminal-psi/control_flow.md): "All normal returns, including returns after a cycle, must establish the published guarantees."
- **Trace:** the checker proves each published `ensures` clause on every `Return` edge of the callee (`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/lib.rs:1605-1620`, `Discharged::EveryExit` at `:183-187`), and reports "cannot prove ensures contract for exit from ..." otherwise (`describe.rs:108-122`). `terminal_postconditions` returns `None` unless the callee is unique and its formals match (`scalar_contracts.rs:88-100`).
- **Outcome: intended.** Callers rely on the callee's checked result.

### 11. Incoming-target call effects skip provider lifetime dependencies

- **Evidence:** record `D/replay/analysis.rs` "Fail-open risks" bullet 6 (`_ => {}` at survey `:1966`); digest 07 line 143 (second half).
- **Current source:** `D/replay/analysis.rs:1938-1981`; the catch-all is `:1978`.
- **Contract:** a descriptor a callee stores must keep alive every caller loan it depends on.
- **Trace:** when a stored effect lands in incoming storage, this arm adds caller inputs only for `IncomingSlot`, `IncomingContents` and `Parameter` origins. It drops the `lifetime_dependencies` of `Provider` and `ProducedContents` origins. The Storage-target path walks those dependencies (`:2104-2126`). In the main flow path the gap is covered: `flow` appends to the step `terminal_flow` built (`:1759-1770`), call arguments are already step inputs (`psi/types/values.rs:744-746`), and this arm sets `may_escape`. `invocation_flow_at` (`:1493-1524`) starts from an empty step. Only cleanup `InvokeOwned` calls reach it (`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places/loans.rs:3004`), and it adds argument inputs only when effects are unobserved (`analysis.rs:1871-1878`).
- **Outcome: still open.** No accepted program has been shown, and this is not reproduced. The proposed repair expands `Provider` and `ProducedContents` dependencies in this arm, matching `:2111-2126`. Its test should be a cleanup callee that stores a provider-derived view into storage behind its argument.

### 12. `indirect_contents_contain_slot` returns false for `Opaque`

- **Evidence:** record `D/replay.rs` "Fail-open risks" bullet 2; digest 07 line 144.
- **Current source:** `D/replay.rs:632-674` (`_ => false` at `:670`).
- **Contract:** a write through an argument that may store a descriptor must be treated as one.
- **Trace:** a selected opaque type has its carrier's shape in Psi, because wire validation refuses a different shape (`same_shape`, `foundation_validation.rs:2172-2178`). A bare `Opaque` shape remains only for an unselected, reference-only pointee ([opaque representations](../../../../../wiki/spec/build/opaque_representations.md), "Demand and uniqueness"). Omega code cannot project into such a pointee. A provider that keeps a borrow in it falls under the boundary contract, and `boundary_call_preserves_descriptor_storage` refuses any `RetainedBorrow` guarantee (`D/descriptor_results/boundary_calls.rs:68-70`).
- **Outcome: intended.** The `may_write_indirect_contents` shared-borrow shortcut (`D/replay.rs:567`) stays traced.

### 13. A provider output that is not a slot of the current graph is skipped

- **Evidence:** record `D/descriptor_results/provider_invocation.rs` "Fail-open risks"; digest 07 line 173.
- **Current source:** `D/descriptor_results/provider_invocation.rs:93-109`.
- **Contract:** every stored provider output must appear in the graph that tracks its slot kind.
- **Trace:** `validate_declared_boundary_reference_outputs` requires the outputs to cover exactly the writable reference and view slots of each parameter (`psi/semantics/src/references/boundary_results.rs:216-217`, using `provider_outputs_cover_writable_slots` at `provider_invocation.rs:5-50`). Wire validation runs it (`foundation_validation.rs:2678`). An output skipped here is therefore a slot of the other kind and appears in that graph.
- **Outcome: intended.**

### 14. Operation kinds that `terminal_flow` treats as writing nothing

- **Evidence:** record `D/descriptor_results/terminal_flow.rs` "Fail-open risks" bullets 1 and 4; "Five things" item 1.
- **Current source:** `D/descriptor_results/terminal_flow.rs:769-776` (no payload write) and `:864-1887` (descriptor derivations, catch-all at `:1887`).
- **Contract:** an operation that writes storage or yields a descriptor must record that write or derivation.
- **Trace:** in the first list, each kind only reads a source or creates its own result (`psi/machines/operation.rs`: `StructuralLeafCopy`/`StructuralCaseLeafCopy` `:280-287`, `EstablishStructuralLocal`/`EstablishZeroedStructural` `:175-176`, `StorageAddress`/`StorageAlignment` `:117-124`), and atomic accesses that modify the cell are matched at `:764-768`. Any other kind that is not a scalar leaf or an effect-free structural row records an unknown write (`:777-797`). In the second match, element views come through `element_view_establish_pair_pattern!`, and calls come through invocation rows. The unlisted kinds yield a scalar result (`ByteSequenceRead`, refused unless its result is `u8` at `foundation_validation.rs:6638-6651`; `ScalarRepresentationView`; selections), an owned value, or a fresh local. Every operation result is still a definition (`terminal_flow.rs:527-561`), and replay gives an unassigned defined slot an unknown value (`D/replay/engine.rs:2363-2380`).
- **Outcome: intended.**

### 15. `terminal_resident_value_transfers` omits operation kinds

- **Evidence:** record `D/descriptor_results/scalar_captures.rs` "Fail-open risks".
- **Current source:** `D/descriptor_results/scalar_captures.rs:223-262`.
- **Contract:** a missing entry must not let an earlier store stand for the current value.
- **Trace:** both consumers, `psi/semantics/src/scalar_target_ranges/storage.rs:41` and `psi-to-native/verification/provider-admission/src/terminal_authority_review/windows_handle_values.rs:396`, read the map through `projected_stored_sources_in` (`D/descriptor_results/payload_versions.rs:684-746`). That function keeps a candidate source only when `require_unchanged_between` proves the storage unchanged up to the read. A missing entry loses a source and makes the query fail, so the cost is precision only.
- **Outcome: intended.**

### 16. Primitive store membership checks

- **Evidence:** record `D/primitive_store_memberships.rs` "Fail-open risks" bullets 3 and 4.
- **Current source:** `D/primitive_store_memberships.rs:241` and `:287`.
- **Contract:** a primitive store must keep each receiving domain's predicate and its issuer restriction.
- **Trace:** at `:241`, only a membership at the written leaf's exact path can be a scalar-carrier domain. Enclosing and overlapping memberships go through `primitive_store_restriction` and the overlap check at `:131-155`. At `:287`, `establishment_routes: None` means "no issuer restriction" (`psi/types/qualifications/scalar.rs:43-44`), and predicate truth is a separate check. Wire validation requires a `range_obligation` exactly when `primitive_store_restriction` reports a restriction (`foundation_validation.rs:7215-7229`), and then runs this function for issuer provenance (`:7244-7258`).
- **Outcome: intended.**

### Kept classifications

The survey already classified these, and current source matches:

| Record section | Current source | Classification kept |
| --- | --- | --- |
| `boundary_calls.rs`: shared-borrow arguments always pass | `D/descriptor_results/boundary_calls.rs:98` | traced |
| `owned_shape.rs`: `Opaque => true` | `D/descriptor_results/owned_shape.rs:85` (comment `:83-84`) | stated |
| `payload_entry.rs`: budget stops the search early | `D/descriptor_results/payload_entry.rs:18-23`, `:79` | closed (fewer facts) |
| `payload_history.rs`: `retire` keeps content-independent memberships | `payload_history.rs:1459-1465` | stated, tested |
| `payload_reachability.rs`: unknown terms impose no demand | `D/descriptor_results/payload_reachability.rs:265` | traced (conservative) |
| `private_parameters.rs`: private roots survive argument-free boundary calls | module comment `D/descriptor_results/private_parameters.rs:1-2` | stated |
| `analysis.rs`: unresolved-address `may_escape` and keep-incoming skips | `D/replay/analysis.rs` comments by `prepare_flow` and `:1898-1909` | stated |

Digest 07 lines 145-147 (`D/replay/case_flow.rs`, `D/replay/engine.rs:2632`, `D/replay/summary_interface.rs`) cite files outside unit_046 and belong to other packets. Duplicate-mechanism and "unused" entries are code-quality leads, not correctness claims, and are not reviewed here.

## Validation

This review used source reading and `rg` only; no compiler or test was built or run. Every path and line above is at `24ad2700685e6cc6062026c87ed5fd24ed415265`. Paths under `psi/`, `source-to-psi/` and `psi-to-native/` are relative to `source-rust/subsystems/compiler/`.
