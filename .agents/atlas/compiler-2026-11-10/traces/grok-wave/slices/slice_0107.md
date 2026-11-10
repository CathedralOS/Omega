slice_0107

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0824 | sd/intrinsic_review.rs:353-365 | CLOSED | `accepted`→`Ok(None)` only for missing/unmatched console/process rows already bounded by `retain_selected_compiler_intrinsic_review_identities`. |
| A0825 | sd/numeric_provider.rs:82 | CLOSED | Symmetric `None` provider owners pass only after identical `select` on the same custody tables at both call sites. |
| A0826 | sd/provider_binding.rs:214-218,252 | CLOSED | Skipped roles are absent package inputs; present bindings still must pass `resolve_accepted_service_binding`. |
| A0827 | sd/.../application_realization.rs:189-196 | CLOSED | `Ok(None)` is only in package-review derivation; compile path already rejects non-exact authored uses at :184-187. |
| A0828 | sd/selected_execution.rs:88-91 | CLOSED | `terminal_authority_permissions` only walks `resolved_semantic_bindings`, each validated in `validate_terminal_authority_permissions` first. |
| A0829 | ous/access_references.rs:71-107 | CLOSED | Both production callers turn `None` into a hard lowering/legalization error. |
| A0830 | comp/product_boundary.rs; csup/.../boundary_fields.rs:138-140,203-205; csup/.../closed_trait_applications.rs:54-57 | CLOSED | Test-only harness/guards; no production path treats these as accepting a bad program. |

### A0824

1. Read `produce_direct_boundary_leaves` in full at `omega-rust/omega/compilation/realization/selected-dispatch/src/intrinsic_review.rs:324-407` (read); nested `accepted` closure at :353-373 (read).
2. Search `produce_direct_boundary_leaves` under `omega-rust/`: **2 hits** (definition at `intrinsic_review.rs:324`, sole production caller `provider_binding.rs:231`).
3. Read caller `prepare_selected_execution` at `provider_binding.rs:219-240` (read) and `retain_selected_compiler_intrinsic_review_identities` at `intrinsic_review.rs:93-321` (read), especially binding consumption at :275-316.
4. Input reaching the accepting branch: `resolved_exit_bindings` passed as `bindings`; for console/process roles, `accepted(role)` returns `Ok(None)` when no row has that `role` (:354-356) or when no `selected` provider matches digest and declaration symbol (:359-364). Then `declaration.settle(..., None, None)` or with one side `None` at :389-393.
5. **Verdict: CLOSED.** `retain_selected_compiler_intrinsic_review_identities` errors if a supplied console/process binding is not consumed exactly once (:299-310). `resolved_exit_bindings` therefore only contains bindings already aligned with `selected`. Optional `None` from the package (no binding) is deliberate and does not bypass consumption checks. Entry description matches code at :353-365 (read); line citation matches.

**Unsure:** Downstream handling of `DirectBoundaryExecution::Intrinsic(None)` in `derive_selected_intrinsic_settlement_proposals` was not read in full.

### A0825

1. Read `NumericProviderSelection::execution` in full at `numeric_provider.rs:16-97` (read); line **82** is the custody equality gate (read).
2. Search `numeric_execution` in `selected-dispatch/`: **3 hits** — `compiler_intrinsic.rs:108` (definition), `intrinsic_review.rs:232`, `intrinsic_review.rs:374` (read call sites).
3. Read both call sites: both set `candidate` and `custody` to the same `external_providers` (:234-235, :375-377) (read).
4. Input: after twin `select(self.candidate)?` and `select(self.custody)?`, when provider owner slot is `None` on both sides, `candidate.2 != provider` is false and the check passes alongside `candidate == custody`.
5. **Verdict: CLOSED.** Asymmetric provider presence still fails at :82. Entry matches (read).

**Unsure:** Nothing material.

### A0826

1. Read `prepare_selected_execution` at `provider_binding.rs:107-275` (read); `accepted_binding` at :214-218 and host-service loop at :247-256 (read).
2. Search `prepare_selected_execution` under `omega-rust/omega/compilation/realization/selected-dispatch`: **2 hits** (definition; re-export in `lib.rs`). Production consumer: `execution_settlement.rs:367` (read grep hit, caller not read in full).
3. Read `resolve_accepted_service_binding` at `intrinsic_review.rs:52-91` (read).
4. Input: `role.and_then(accepted_binding)` skips when `package_inputs` is absent or `inputs.accepted_semantic_binding(role)` is `None`; when `Some(binding)`, `resolve_accepted_service_binding` must succeed or compilation errors.
5. **Verdict: CLOSED.** Skip means no package-accepted binding for that role, not a malformed binding. Present bindings are validated. Entry matches :214-218 and :252 (read).

**Unsure:** Whether every deployment that needs Filesystem/Time/entry always supplies package bindings was not traced through `execution_settlement.rs` beyond the grep hit.

### A0827

1. Read `derive_checked_expression_operator_application_realization` at `application_realization.rs:100-261` (read) and `has_exact_authored_selection` at :263-288 (read).
2. Search `derive_checked_nongeneric_operator_application_realizations` under `omega-rust/`: **5 hits**; sole non-test caller `review-evidence/.../application_realizations.rs:40` (read).
3. Read parent `derive_checked_expressions` at :79-99 (read): `Ok(None)` is silently ignored in the row list.
4. Input: after exact authored-use proof at :184-187, if the authored-selection ledger has no matching `AuthoredDeclarationSelectionTarget::Resolved` for the requirement operator, returns `Ok(None)` at :189-195.
5. **Verdict: CLOSED.** Not on the compile/refusal path; only package-review evidence. Resolved-without-selection would be a checking bug, not acceptance here. Entry matches (read).

**Unsure:** Whether Psi checking always populates authored selections when status is `Resolved` was not read in `psi/compilation/validation`.

### A0828

1. Read `SelectedExecution::terminal_authority_permissions` at `selected_execution.rs:75-98` (read); especially `unwrap_or_default` at :88-91.
2. Search `validate_terminal_authority_permissions` under `omega-rust/`: **3 hits**, all in `intrinsic_review.rs` (:86, :286, :409 definition) (read).
3. Read `validate_terminal_authority_permissions` at :409-434 (read); uses the same `unwrap_or_default` at :417 on validate path.
4. Input: only bindings from `resolved_semantic_bindings` via `accepted_semantic_bindings()`; missing identity text becomes `""` when re-interning permissions.
5. **Verdict: CLOSED.** Every binding enters `resolved_semantic_bindings` only through `resolve_accepted_service_binding` (:86) or retain path (:286-290), both calling validate first. Entry matches; concern about skipping validate does not apply to this iterator (read).

**Unsure:** `validate_retained_package_terminal_authority_permissions` later also uses `unwrap_or_default` (`terminal_authority_permission_policy.rs:177`); interaction with empty-string collisions not fully traced.

### A0829

1. Read `borrowed_view_formation` in full at `ous/access_references.rs:63-120` (read).
2. Search `\bborrowed_view_formation\b` under `omega-rust/`: **5 hits** — 1 definition, 4 uses (`descriptor_results.rs:101` not read; `control_flow_storage.rs:135`, `legalization_reference_custody.rs:6034` read).
3. Read callers: `control_flow_storage.rs:135-136` `.ok_or_else(|| invalid("borrowed View lost its Ready interpretation"))` (read); `legalization_reference_custody.rs:6034-6035` same pattern with `invalid()` (read).
4. Input: any structural/plan/carrier mismatch or missing plan/row returns `None`; callers treat that as hard lowering failure, not acceptance.
5. **Verdict: CLOSED.** Entry “returns `None` on any mismatch” matches (read).

**Unsure:** `descriptor_results.rs:101` caller not read.

### A0830

1. Read `product_boundary.rs` guard helpers and tests at :8-14, :716-731, :756-759 (read); `boundary_fields.rs` `resolve_adapter_call` at :99-214 (read); `closed_trait_applications.rs::project` at :11-128 (read).
2. Read `boundary_dispatch.rs` call sites at :257-267 and :284-294 (read).
3. **product_boundary:** `split`/`compact_preparation` limits which substrings `assert_absent` scans — test guard gap only (read), not compile admission.
4. **boundary_fields `Ok(None)`:** at :138-140 (no owning field) and :203-205 (`readable == 0`); callers invoke for side effect of `Err` only, discarding `Ok` (:257-267, :284-294) (read).
5. **closed_trait_applications:** at :54-56, empty `coordinates` re-pushes adapter rows unprojected inside `csup/` test support only (read).
6. **Verdict: CLOSED.** These sites do not decide whether a user program is accepted by the compiler; they are architecture substring tests or checked-interpreter oracle helpers.

**Unsure:** Full corpus of `product_boundary.rs` tests beyond cited regions not read.

### What I did not read

- `execution_settlement.rs` full caller trace into native admission for A0826.
- `derive_selected_intrinsic_settlement_proposals` / `Intrinsic(None)` handling for A0824.
- `descriptor_results.rs` caller of `borrowed_view_formation` for A0829.
- Psi operator authored-selection checking that would decide A0827 REACHABLE vs CLOSED at the checking layer.
- Entire `product_boundary.rs` beyond cited tests for A0830.

SLICE COMPLETE
