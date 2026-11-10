slice_0115

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0886 | layout/src/lib.rs:138-143 | CLOSED | `for_target` returns `Ok(None)` only for natural layouts; authored records error or return `Some`, and codec validation rejects inconsistent natural/authored storage. |
| A0887 | layout/src/boundary_values.rs:215-217 | CLOSED | Authored record field extents are checked against `record.byte_size` in Terminal foundation validation before boundary layout consumes `storage`. |
| A0888 | machine-emission/src/exit_contract.rs:2584-2597 | CLOSED | Decoded footprints come from the encoding store; the `NoneV1` catch-all only accepts kinds whose replayed effect is `NoneV1`, after stack/control gates. |
| A0889 | machine-emission/src/exit_contract.rs:2332,2376 | CLOSED | `CopyBytes`/`ScanByteSequence` arms trust catalog-validated encoded footprints; machine-effect catalog validation constrains those semantics upstream. |
| A0890 | object/object-file/src/names.rs:79-85 | CLOSED | Production callers match regions by name and error when the match is not unique, so invalid handles yield `""` and fail validation downstream. |
| A0891 | backend-plan/src/direct_boundary_leaves.rs:89-91 | CLOSED | Early `Ok(())` applies only to non-`Foreign(Ok(_))` executions after the foreign-route identity checks above; intrinsics are not foreign-roster subjects. |
| A0892 | backend-plan/src/direct_boundary_leaves.rs:96,116 | REACHABLE | When `calling_for_requirement` is empty, foreign binding applications are not compared to any selected roster while other checks may still pass. |
| A0893 | backend-plan/src/synchronous_invocations.rs:108,200-205 | UNDECIDED | Unmatched invoke targets add no edge and no diagnostic here; whether admission elsewhere rejects that invoke was not traced to a decision point. |

### A0886

1. Read (in full) `terminal_layout_identity` in `omega-rust/omega/compilation/backend/layout/src/lib.rs:129-190` and `NativeLayoutCatalog::for_target` in `omega-rust/psi/compilation/representations/terminal-psi/src/lib.rs:636-663`.
2. Search `for_target(native` under `omega-rust/`: 3 hits (read: `layout/src/lib.rs:140`, `terminal-psi/semantics/src/storage.rs:2407`, `calling_policy_plans/callback_layout_catalog.rs:182`).
3. Search `terminal_layout_identity` under `omega-rust/`: 6 hits / 4 definitions+uses (production: `terminal_callback_layouts.rs:124`; test: `native_layout_controls.rs:49,52`). Read `close_record` in `terminal_callback_layouts.rs:108-205` — `authored.is_none()` at 160 is the natural-layout branch.
4. Read `foundation_validation.rs:2703-2749` — natural records cannot carry realizations or `TargetLayout` storage; authored records must have realizations matching semantic roster.
5. Entry correction (read): lines 138-143 **do** return `Ok(None)` when `for_target` yields `None`; the digest phrasing “No authored layout row … returns `Ok(None)`” describes the `for_target` contract, not absence of `Ok(None)` at 142.
6. Input to accepting branch: a `StructuralTypeId` whose native catalog row has absent policy and empty realizations (natural layout) for the selected `TargetProfile`.
7. **Verdict: CLOSED.** `for_target` at `lib.rs:642-646` (read) is the only `None` path; authored policy with missing target errors at `lib.rs:655-656` (read). A type that should be authored cannot reach `Ok(None)` without failing validation at `foundation_validation.rs:2704-2748` (read).
8. Uncertainty: did not read every caller of `callback_layout_catalog.rs:182` or `storage.rs:2407` beyond citations.

### A0887

1. Read (in full) `BoundaryValueLayout::record` in `boundary_values.rs:175-265` and `TerminalBoundaryRecipes::record` in `boundary_values.rs:458-523`.
2. Storage reaches the branch at 215-217 via `BoundaryValueRecipe::Record` (`boundary_values.rs:127-133`, read) or `StructuralTypeShape::Record` (`boundary_values.rs:447-448`, read), copying `StructuralRecordStorage::Authored` sizes into `BoundaryRecordStorage` at 518-521 (read).
3. Upstream check (read): `foundation_validation.rs:8138-8197` rejects authored fields whose offsets/extents exceed `record.byte_size`.
4. Input: validated Terminal module record with `Some(storage)` where fields already fit the authored `byte_size`/`byte_alignment`.
5. **Verdict: CLOSED.** Overwrite at 215-217 does not re-validate because structural validation already bound fields to `storage` before layout runs; mismatch at layout time implies an unvalidated module, not a layout-only hole.
6. Uncertainty: did not read full `provider_planning/external_binding_rows.rs` path into `TerminalBoundaryRecipes::value_recipe`.

### A0888

1. Read (in full) `validate_non_return` in `exit_contract.rs:2094-2607`, especially `expected_control` default at 2142 (read) and memory match at 2584-2597 (read).
2. Caller (read): whole-function exit validation at `exit_contract.rs:638-645` supplies instructions already present in the post-allocation machine.
3. Input to `(_, NoneV1, _)` arm: any non-return instruction whose replayed footprint has `MachineEncodedMemoryEffect::NoneV1` and whose `SelectedInstructionKind` is not in the explicit exclusion list at 2586-2596 (read).
4. **Verdict: CLOSED.** Footprints are taken from `encoded.encoded(encoding)` at 2149-2162 (read), not author-provided; semantic/memory pairing for catalog rows is enforced in `selected-instructions/.../validation.rs:2705+` (read). A user program cannot inject a new kind/effect pair here without corrupting compiler artifacts, which earlier stages reject.
5. Uncertainty: did not enumerate every kind that legitimately carries `NoneV1` outside the exclusion list.

### A0889

1. Read `validate_non_return` memory `match` arms at `exit_contract.rs:2324-2376` (read; full function already read above).
2. Compared nearby arms (read): `ZeroBytes` at 2340-2344 checks trap and operand lists; `CopyBytes` and `ScanByteSequence` return `true` with decoded effect only.
3. Upstream (read): `validation.rs:2705-2717` ties `MachineSemanticKind::CopyBytes` to `CopyBytesV1` memory and constraint shape.
4. **Verdict: CLOSED.** Absence of trap/operand checks in the exit-contract arm is harmless because footprints are replay artifacts; catalog validation is the authority before emission reaches this match.
5. Uncertainty: did not read `ScanByteSequence` catalog validation lines beyond identity.rs grep hits.

### A0890

1. Read (in full) `object_symbol_name` in `object/object-file/src/names.rs:79-85`; module comment at 1-8 (read) states fail-closed lookup behavior.
2. Search `object_symbol_name\(` under `omega-rust/`: 3 production call sites — `image-emission/src/image.rs:945,996`, `image-emission/src/entries.rs:3936` (read partial at `image.rs:944-1009`).
3. Input: any `ObjectSymbolHandle` failing `object.layout.symbols.is_valid(symbol)` returns `""` (read).
4. **Verdict: CLOSED.** `image.rs:954-958` and `1001-1009` (read) require exactly one region match; empty name yields `matching != 1` and a diagnostic error, not acceptance.
5. Uncertainty: did not read `entries.rs:3936` caller body.

### A0891

1. Read (in full) `DirectBoundaryLeaf::validate_calling_custody` in `direct_boundary_leaves.rs:25-130`.
2. Search `validate_calling_custody` under `omega-rust/`: 6 hits (production: `provider_rows.rs:208`, `retained_terminal_codec.rs:249,277`; tests: `filesystem_receiver.rs`; defs: `direct_boundary_leaves.rs`).
3. Read `provider_rows.rs:162-209` — `finish` always calls `validate_calling_custody` at 208 (read); `DirectBoundaryExecution::Intrinsic` set in `intrinsic_review.rs:395` (grep only).
4. Input to early accept: `execution` not matching `DirectBoundaryExecution::Foreign(Ok(binding))` (including `Intrinsic(_)`, read enum at 19-22).
5. **Verdict: CLOSED.** Lines 89-91 (read) deliberately limit foreign-roster checks to foreign bindings; lines 26-88 still enforce single-row plan, route identity, and schema alignment. Intrinsics are not foreign ABI subjects.
6. Uncertainty: did not read full intrinsic admission in `intrinsic_review.rs`.

### A0892

1. Read (in full) `validate_calling_custody` foreign section in `direct_boundary_leaves.rs:89-128`.
2. Read `service_schemas.rs:87-98` — `calling_for_requirement` filters `calling_applications` (read).
3. Read `provider_rows.rs:173-194` — `satisfied_schema` clones original schema; `plan.schema.calling_applications.clear()` at 194 does not clear `satisfied_schema` (read).
4. Read `external_binding_rows.rs:172-206` — default calling applications may be synthesized when empty (read).
5. Input: `Foreign(Ok(binding))` where `source.calling_for_requirement(...)` is empty but `binding.calling_applications` is non-empty.
6. **Verdict: REACHABLE.** A direct foreign leaf can carry synthesized binding applications while the retained satisfied schema lists no selected applications for that requirement; lines 96-99 skip length enforcement when `expected.is_empty()`, and lines 116-127 skip selected-policy comparison — custody can succeed without roster fingerprint checks.
7. Uncertainty: did not confirm every production path into `finish` supplies bindings with non-empty apps when schema roster is empty.

### A0893

1. Read `SynchronousBoundaryTarget::matches` at `synchronous_invocations.rs:84-120` (read) and cycle loop at 194-213 (read).
2. Search `validate_selected_synchronous_invocation_cycles` under `omega-rust/`: 4 hits — `synchronous_invocations.rs:16,134`; `target_providers.rs:284` (read header of `validate_synchronous_calls` at 273-309 only).
3. Input to silent branch: `invoked.matches(target)` false for all selected plans → `matching_targets` is `[]` → no edge, no diagnostic at 200-201 (read).
4. Input to `package.is_none()` widen: `Application { package: None, application: None, ... }` uses `package.is_none() || owner == *package` at 108 (read), matching any package owner on trait/receiver names.
5. **Verdict: UNDECIDED.** This function only builds cycle edges among selected plans; whether a synchronous invoke with zero selected matches should fail compilation is not established — `target_providers.rs:291-309` (partial read) resolves checked candidates but was not traced through to an accept/reject decision for unmatched invokes.
6. Uncertainty: full executable admission path for orphaned synchronous invokes; behavior of `selected_checked_invocation_rows` (not read).

## What I did not read

- Full bodies of `callback_layout_catalog.rs`, `terminal-psi/semantics/src/storage.rs`, `image-emission/src/entries.rs:3936`, `intrinsic_review.rs`, `synchronous_invocations.rs` helpers below line 295, and most of `target_providers.rs` after line 309.
- ISA `selected_form_encoding.rs` encoders beyond grep/context for trap operands on `ScanByteSequence`.

SLICE COMPLETE
