slice_0101

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0776 | LO/identity/calling.rs:393-397 | CLOSED | Bare tags only feed plan identity hashing; legalization audits full structures, not identity alone |
| A0777 | LO/validation.rs:226 | REACHABLE | `TargetFamily` validation rejects only empty `target`, not a mismatched non-empty string |
| A0778 | TO/descriptor_input.rs:35-48 | REACHABLE | Non-conforming descriptor-shaped calls get `Ok(None)` and carry no byte limit through lowering |
| A0779 | TO/values.rs:185-189 | CLOSED | `WithinRegion` origins intentionally skip suffix extension; lowering gates inconsistent reference shapes earlier |
| A0780 | F/optimization-core/.../baseline.rs:162-178 | REACHABLE | Wire decode admits self-consistent rows whose `Choose` is outside the decoded roster |
| A0781 | F/bounded-process/src/process_child.rs:251 | REACHABLE | On non-Unix non-Windows hosts, any `InvalidInput` from kill is treated as “container already gone” |
| A0782 | I/terminal-runtime/src/execution.rs:103-115 | CLOSED | CLI treats `comparison_unavailable()` as comparison declined, not compilation success (execution/mod.rs:544) |
| A0783 | I/terminal-runtime/.../allocation.rs:360 | REACHABLE | `memset` liveness uses `backing_for_range` without excluding retired backing; handler can succeed after `free` |

### A0776

1. Read `encode_binding` in full at `omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/identity/calling.rs:353-408` (read); lines 393-397 (read) match the entry: five `BoundaryRealization` variants push only a tag byte and discard inner payloads.
2. Search `rg 'calling::encode_binding' /agent/repos/omega/omega-rust` → 1 hit (`identity/scalar_graph.rs:1532`). Search `rg 'fn encode_binding' .../legalized-operations` → 2 definitions (`calling.rs`, `identity.rs` value bindings); production settlement encoding is the `calling.rs` path only.
3. Traced to `legalization.rs:448-449` (read): `legalized_operation_plan_identity(&plan)` seals a receipt; `audit_target_operation_plan` at `legalization.rs:483-488` (read) runs `validate_source_custody` and descriptor audits on the full plan. No decode of settlement tags into a plan was found under `legalized-operations/`.
4. Input reaching the branch: producer-built `BoundarySettlementBinding` values whose builtin realizations carry payload fields that identity encoding ignores.
5. **Verdict: CLOSED** — Acceptance is limited to identity canonicalization; program acceptance goes through pipeline 03 custody and `validate_source`, not hash equality alone.
6. Unsure: whether any non-compiler consumer treats legalized plan identity as sufficient without structural replay (not traced outside `omega/compilation`).

### A0777

1. Read `validate_call_source` in full at `legalized-operations/src/validation.rs:142-231` (read); line 226-228 (read) checks `target.is_empty()` only for `NativeCallOrigin::TargetFamily`.
2. Search `rg '\.validate_source\(' omega-rust` → 22 hits (4 production in `selection_scalar_graph.rs`, 1 in `legalization_source.rs`, 1 in `exit_contract.rs`, rest tests in `legalized-operations/src/tests.rs`).
3. Read callers: `selection_scalar_graph.rs:1928` (read) maps `validate_source` failure to selection error; `legalization_target.rs:1501-1505` (read) custody matches `boundary` and `member` but not `target` string; pipeline 02 sets `target` from `selections.target()` at `02_.../lib.rs:665-671` (read) when originating calls.
4. Input: a `LegalizedScalarCall` whose `NativeCallOrigin::TargetFamily.target` is any non-empty string inconsistent with the resolved family member.
5. **Verdict: REACHABLE** — A plan bearing a wrong but non-empty `target` string should be rejected on identity grounds but passes `validate_source` and the legalization custody check shown.
6. Unsure: whether any in-pipeline producer can emit a mismatched `target` without already failing earlier (only traced lowering origin site).

### A0778

1. Read `descriptor_input_guard`, `descriptor_input_guard_for_call`, and `derive` in full in `target-operations/src/descriptor_input.rs` (read); early `Ok(None)` at 35-47, 73-86, 110, 133-135, 136-140 (read).
2. Search `rg 'descriptor_input_guard' omega-rust` → 4 call sites (2 defs in `descriptor_input.rs`); `descriptor_input_guard_for_call` → 2 call sites (`unit.rs:928`, `legalization_target.rs:3110`).
3. Read callers: `unit.rs:928-950` (read) stores `descriptor_input: Option` allowing `None`; `legalization_target.rs:3110-3125` (read) requires `required == *descriptor_input` so both `None` passes; `references.rs:1569-1577` (read) treats `Ok(None)` as “not a guarded descriptor call” for catalog purposes.
4. Input: a boundary call that performs descriptor read semantics but whose declaration/argument shape is not the exact 3-scalar-parameter form returns `Ok(None)` instead of `Err`.
5. **Verdict: REACHABLE** — Such a call is lowered without a `DescriptorInputGuard` byte limit when shape checks fail open to `None`.
6. Unsure: whether optimization-unit validation always rejects those calls before pipeline 02 (not fully traced).

### A0779

1. Read `projected_family` in full at `target-operations/src/values.rs:175-200` (read); lines 185-188 (read) extend path only when `origin.extent == ReferenceOriginExtent::Projected`.
2. Search `rg 'projected_family' omega-rust` → 3 hits (`values.rs:2`, `references.rs:2546`, `legalization_reference_custody.rs:5620`).
3. Read callers: `references.rs:2476-2480` (read) rejects `Invocation` bindings unless `binding.extent == Projected` on one path; `references.rs:2546` (read) passes `binding.extent` through; `unit_validation.rs:15371` (read) routes `WithinRegion` sources to stricter alternative validation.
4. Input: `ReferenceRoot` with mixed or `WithinRegion` origins and non-empty suffix projection request.
5. **Verdict: CLOSED** — Non-`Projected` extents are intentionally not suffix-extended; inconsistent reference-result shapes are rejected in lowering (`references.rs:2478`) and unit validation (`unit_validation.rs:15365-15375`).
6. Unsure: none for the cited branch.

### A0780

1. Read `BaselineDecisionLog::decode` at `optimization-core/.../baseline.rs:140-184` (read); outcome decoded at 162-173 without roster membership check; builder checks at `record_validated_outcome` 203-208 (read) were not invoked on decode.
2. Search `rg 'BaselineDecisionLog::decode' omega-rust` → hits in `baseline.rs` tests and `optimization-unit/src/manifest.rs:157` (read).
3. Read `manifest.rs:157-202` (read): decoded `decision_log` is stored after identity recompute on the manifest, not revalidated through `record_validated_outcome`; pipeline 01 `pass_manager.rs:1072-1102` (read) cross-checks outcomes when rebuilding external decision points but that is separate from decode.
4. Input: wire bytes forming a self-consistent `OMGBDL` row whose `Choose` candidate is absent from the decoded `considered` vector (identity hash still matches row content).
5. **Verdict: REACHABLE** — Decode accepts that row; trust-boundary consumers of a decoded manifest could inherit an impossible optimization choice unless they re-check like the builder.
6. Unsure: every post-decode consumer path (only manifest decode and pass_manager excerpt read).

### A0781

1. Read `native_container_already_absent` and `terminate` at `bounded-process/src/process_child.rs:124-147, 244-252` (read).
2. Search `rg 'native_container_already_absent' omega-rust` → 3 hits, all in `process_child.rs`.
3. On `cfg(not(unix))` (read 249-251), `ErrorKind::InvalidInput` is always “already absent”; `terminate` treats that as success and sets `container_closed`.
4. Input: `child.kill()` returning `InvalidInput` for a reason other than an already-closed container on a non-Unix, non-Windows host (`cfg(not(any(unix, windows)))` path at 187-191).
5. **Verdict: REACHABLE** — Termination can succeed without killing the child when the error kind is misclassified.
6. Unsure: which shipped targets use the `not(any(unix, windows))` tier (read only the cfg split).

### A0782

1. Read `comparison_unavailable` and `unavailable_before_guest_effects` at `terminal-runtime/src/execution.rs:100-127` (read).
2. Search `rg 'comparison_unavailable' omega-rust` → 5 hits (`execution.rs` plus `cli/.../execution/mod.rs:544`).
3. Read `compare_interpretation` at `application/cli/src/execution/mod.rs:542-549` (read): `comparison_unavailable()` maps to `InterpreterComparison::Declined`, not success.
4. Input: `TerminalRuntimeError` wrapping `MissingProvision` or the four `TerminalInterpretError` variants listed at `execution.rs:109-115`.
5. **Verdict: CLOSED** — This accepts “comparison unavailable,” not a bad program; native/compile paths are not treated as pass because of this branch.
6. Unsure: none for the cited comparison gate.

### A0783

1. Read `AllocationValues::backing_for_range`, `release`, and `Initialize` (`memset`) handler at `host_imports/allocation.rs:49-61, 93-108, 299-368` (read); line 360 (read) uses `backing_for_range` only.
2. Read `TerminalProviderBacking::retire` at `terminal-interpreter/src/storage_providers.rs:117-128` (read); `backing.rs:51-53` (read) resolves live backing for provider storage.
3. Tests (read) at `allocation.rs:405-410`: after `free`, the allocation row remains and `backing_for_range` can still find the address; `retire()` marks backing retired.
4. Input: `memset` on an address whose allocation was `free`d but whose row still matches `backing_for_range`.
5. **Verdict: REACHABLE** — Handler returns `Ok` without requiring non-retired backing; later provider resolution can fail at `backing.rs:51-57`, but the memset import itself accepts the freed address.
6. Unsure: whether admitted Terminal programs can reach this host import sequence (not traced from corpus).

### What I did not read

- Full bodies of `legalization_reference_custody.rs` and `selection_scalar_graph.rs` beyond cited lines.
- Complete optimization-unit validator paths for descriptor-shaped calls before pipeline 02.
- All post-decode uses of `PrePhysicalOptimizationManifest.decision_log` beyond `manifest.rs` and `pass_manager.rs` excerpts.
- Target-specific behavior of `child.kill()` on `cfg(not(any(unix, windows)))` platforms.
- Pipeline stages outside area 05 that might re-encode or decode legalized settlement bindings.

SLICE COMPLETE
