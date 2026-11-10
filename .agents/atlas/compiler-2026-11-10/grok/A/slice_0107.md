# slice_0107

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0824 | sd/intrinsic_review.rs:353-365 | UNDECIDED | `accepted` may pass `None` into `declaration.settle` (read); whether a demanded hosted exit still completes compile past `NoClosedCatalogIdentity` was not traced |
| A0825 | sd/numeric_provider.rs:82 | CLOSED | Line 82 rejects any mismatch between candidate and custody numeric tuple, including provider owner slots (read) |
| A0826 | sd/provider_binding.rs:214-218,252 | UNDECIDED | Missing Filesystem/Time/entry accepted rows are skipped without diagnostic (read); downstream review consumers were not fully traced |
| A0827 | sd/selected_dispatch/operator_adapter/application_realization.rs:189-196 | CLOSED | `Ok(None)` only drops optional realization rows; malformed canonical-empty sites error earlier (read) |
| A0828 | sd/selected_execution.rs:88-91 | REACHABLE | Missing permission identity text becomes `""` on export without calling `validate_terminal_authority_permissions` (read) |
| A0829 | ous/access_references.rs:71-107 | CLOSED | Every production caller maps `None` to a hard invalidation error (read) |
| A0830 | csup/.../boundary_fields.rs:138-140,203-205 | REACHABLE | Test harness ignores `Ok(None)` from `resolve_adapter_call`, so unmatched Product boundary calls do not fail settlement (read) |

## A0824

Digest (area_01): no resolved binding for a Console/ProcessExit role lets `settle` proceed with `None` (`sd/intrinsic_review.rs:353-365`, cited at commit `457adf4a11f`; re-located in checkout at `omega-rust/omega/compilation/realization/selected-dispatch/src/intrinsic_review.rs:353-365`, read).

Opened `produce_direct_boundary_leaves` in full (`intrinsic_review.rs:324-407`, read). The inner `accepted` closure (`:353-373`, read) returns `Ok(None)` when `bindings` has no row for the role (`:354-357`) or when no `selected` plan matches the accepted digest (`:359-365`). Those `Option`s feed `declaration.settle` at `:389-393` (read). `declaration.settle` is `IntrinsicDeclaration::settle` in `omega-rust/omega/compilation/realization/provider-planning/src/intrinsic_declarations.rs:274-333` (read); hosted rows may still close when `row.bundled` even if `accepted` is `None` (`:326-328`, read).

Caller search: `\bproduce_direct_boundary_leaves\b` in `omega-rust` — 2 matches (1 definition `intrinsic_review.rs:324`, 1 production call `provider_binding.rs:231`, read). The only omega production entry is `prepare_selected_execution` → `comp/execution_settlement.rs:367` (read).

Upstream trace: `retain_selected_compiler_intrinsic_review_identities` (`intrinsic_review.rs:93-322`) errors when an accepted exit binding is present but unmatched (`:299-303`, read), but `produce_direct_boundary_leaves` receives `resolved_exit_bindings` that may omit a role if resolution never ran. When `settle` yields `Ok(None)`, the leaf still records `DirectBoundaryExecution::Intrinsic(None)` (`:387-395`, read). `derive_selected_intrinsic_settlement_proposals` maps `execution: None` to `NoClosedCatalogIdentity` (`omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/compiler_builtins/retained.rs:254-255`, read); I did not read whether that proposal still lets the product compile.

**Verdict UNDECIDED:** What would decide it is tracing `TerminalCompilerBuiltinSettlement::NoClosedCatalogIdentity` from proposal through native admission/refusal for a program that demands the boundary but supplied no matching accepted binding.

## A0825

Digest: numeric selection with no recorded provider is accepted when the candidate also has none (`sd/numeric_provider.rs:82`, as stated).

Opened `NumericProviderSelection::execution` in full (`omega-rust/omega/compilation/realization/selected-dispatch/src/numeric_provider.rs:16-96`, read). After building parallel candidate/custody tuples, line `:82` requires `candidate == custody || candidate.1 != Some(realization) || candidate.2 != provider` to fail; when both provider slots are `None`, equality holds (read). Mismatch returns the error at `:83-86` (read).

Caller search: `NumericProviderSelection` `execution` is invoked only through `compiler_intrinsic::numeric_execution` (`compiler_intrinsic.rs:108-124`, read), which is called from `intrinsic_review.rs:232-243` and `:374-386` (read) — 2 production call sites.

Input that reaches the branch: a compiler-intrinsic row whose checked external numeric satisfaction has no declaring owner symbol on both candidate and custody views (both `None` at `:74-81`, read).

**Verdict CLOSED:** The closing check is `numeric_provider.rs:82-86` (read); every numeric execution path goes through `compiler_intrinsic::numeric_execution` (read).

## A0826

Digest: missing accepted Filesystem/Time/entry bindings are skipped; consumers not read (`sd/provider_binding.rs:214-218`, `:252`).

Opened `prepare_selected_execution` in full (`provider_binding.rs:107-275`, read). `accepted_binding` at `:214-218` (read) reads `settlement.package_inputs.and_then(|inputs| inputs.accepted_semantic_binding(role))`. The loop `:247-257` (read) pushes `resolve_accepted_service_binding` only when `role.and_then(accepted_binding)` is `Some`; absent roles are silently omitted.

Caller search: `\bprepare_selected_execution\b` under `omega-rust/omega` — 4 matches (1 definition `provider_binding.rs:107`, 1 production call `execution_settlement.rs:367`, plus re-exports in `lib.rs`, read).

`resolve_accepted_service_binding` (`intrinsic_review.rs:52-91`, read) errors if a binding is present but not requirement-only or schema mismatches; it always calls `validate_terminal_authority_permissions` at `:86` (read). I read only the grep listing for `resolved_semantic_binding(` — uses in `omega-rust/omega/packages/review-evidence/...` (inferred count 4 files, not opened).

**Verdict UNDECIDED:** Whether a program that omits a required host-service accepted binding still ships depends on whether review-evidence or admission requires `resolved_semantic_binding(role)` for that role; those consumers were not read.

## A0827

Digest: no authored selection → `Ok(None)`, no realization row; downstream not stated (`application_realization.rs:189-196`).

Opened `derive_checked_expression_operator_application_realization` in full (`omega-rust/omega/compilation/realization/selected-dispatch/src/selected_dispatch/operator_adapter/application_realization.rs:107-261`, read). Module header `:13-22` (read) documents intentional `None` returns for lanes handled elsewhere. After authored-use checks, `:189-196` returns `Ok(None)` when `has_exact_authored_selection` is false (read); `:197-203` returns `Ok(None)` when `selected_use_plan` is missing (read). Wrong arity or non-exact authored use errors at `:178-187` (read).

Caller search: `\bderive_checked_nongeneric_operator_application_realizations\b` in `omega-rust` — 5 matches (1 definition, 1 production call `omega/packages/review-evidence/src/capture/providers/application_realizations.rs:40`, re-exports, read). Parent `derive_checked_expressions` treats `Ok(None)` as skip (`:95-96`, read).

Input reaching `:189-196`: a canonical-empty boundary operator application at an expression site with exactly one exact resolved authored use (`:178-183`, read) but no matching authored declaration selection for that requirement symbol (`has_exact_authored_selection` at `:263-288`, read).

**Verdict CLOSED:** Absence of a row is by design for non-covered lanes; the accepting branch is harmless for compile refusal because invalid sites error before `:189` (read).

## A0828

Digest: missing permission identity text re-interned as `""`; open if `validate_terminal_authority_permissions` is skipped (`sd/selected_execution.rs:88-91`).

Opened `SelectedExecution::terminal_authority_permissions` (`selected_execution.rs:75-98`, read). For each permission, `identities.intern(binding.terminal_authority_permission_identities().text(permission.requirement_identity()).unwrap_or_default())` at `:87-92` (read) substitutes empty string when the index is absent.

`validate_terminal_authority_permissions` in `intrinsic_review.rs:409-434` (read) uses the same `unwrap_or_default()` when counting schema methods at `:414-425`. That validator runs on `resolve_accepted_service_binding` (`:86`) and on retained exit bindings (`:286-288`, read), but not inside `terminal_authority_permissions` (read).

Caller search: method `terminal_authority_permissions` — production uses include `comp/product_execution.rs:459` and `products/checked_compilation.rs:262` (read via grep; full surrounding functions not read). Exported permissions feed `validate_retained_package_terminal_authority_permissions` (`provider-admission/.../terminal_authority_permission_policy.rs:167-198`, read), which also uses `unwrap_or_default()` at `:177` (read).

Input: an accepted semantic binding whose stored terminal-authority permission references an identity index with no text in the binding’s identity table; export rekeys it as `""` (read).

**Verdict REACHABLE:** A package review row could be keyed under the empty requirement string while the binding never passed `validate_terminal_authority_permissions` on that export path (read), so retention matching may not correspond to the intended method name.

## A0829

Digest: `borrowed_view_formation` returns `None` on any mismatch; callers not read in digest (`ous/access_references.rs:71-107`).

Opened `borrowed_view_formation` in full (`omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/access_references.rs:63-120`, read). Each guard returns `None` on structural/placement mismatch (`:71-107`, read).

Caller search: `\bborrowed_view_formation\b` in `omega-rust` — 4 matches: 1 definition, 3 call sites (read):
- `pipeline/02_.../control_flow_storage.rs:135` → `ok_or_else(|| invalid(...))` (`:135-136`, read)
- `pipeline/03_.../legalization_reference_custody.rs:6034` → `ok_or_else(|| invalid())` (`:6034-6035`, read)
- `optimization-unit/.../descriptor_results.rs:101` → `?` inside `Option` chain (`:101-105`, read)

**Verdict CLOSED:** Pipeline callers turn `None` into hard errors on the compilation path (read); no accepting branch consumes `None` as success in production.

## A0830

Digest: test harness sites — `comp/product_boundary.rs` (`assert_absent` evasion), `csup/.../boundary_fields.rs:138-140` and `:203-205` (`Ok(None)`), `csup/.../closed_trait_applications.rs:54-57` (empty coordinate roster).

Opened `resolve_adapter_call` in full (`omega-rust/omega/compilation/compiler/tests/support/checked_interpreter_dispatch/boundary_dispatch/boundary_fields.rs:100-215`, read). Returns `Ok(None)` when no receiver field owns the target (`:138-140`, read) or when no adapter matches and `readable == 0` (`:203-205`, read).

Caller: `settle_selected_boundary_adapter_dispatch` (`boundary_dispatch.rs:246-295`, read) calls `resolve_adapter_call` for every Product machine call and expression call but only propagates `Err` (`:267`, `:294`, read); `Ok(None)` is discarded, so the scan does not reject the program.

Opened `closed_trait_applications.rs:54-57` (`omega-rust/omega/compilation/compiler/tests/support/checked_interpreter_dispatch/boundary_dispatch/adapter_rows/closed_trait_applications.rs`, read): when `coordinates.is_empty()`, adapter rows push through without coordinate projection (`:54-56`, read).

`comp/product_boundary.rs:8-14` (`read`): `assert_absent` is substring search on `include_str!` sources; digest classifies rename evasion as open by construction.

**Verdict REACHABLE:** A Product-checked program containing a boundary call whose receiver field or adapter row does not exist in the selected plan still passes `settle_selected_boundary_adapter_dispatch` because unmatched calls return `Ok(None)` and the caller does not treat that as failure (read).

## What I did not read

- Full `area_01.md` beyond the preamble and rows for A0824–A0830 (read: preamble lines 1–80, table rows 197–205, harness note 205).
- `git show 457adf4a11f:` paths (checkout is later; cited sites re-located by symbol as noted).
- Whether `NoClosedCatalogIdentity` still permits product compilation (A0824).
- Review-evidence modules that call `resolved_semantic_binding` (A0826).
- `comp/product_boundary.rs` beyond the `assert_absent` helper (A0830).
- Psi `prepare_selected_execution` homonym in `omega-rust/psi/` (different crate from `sd/provider_binding.rs`, read).

SLICE COMPLETE
