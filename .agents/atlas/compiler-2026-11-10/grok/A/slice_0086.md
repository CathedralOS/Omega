# slice_0086

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0658 | omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/access_lifecycles.rs:326 | REACHABLE | `_ => {}` accepts primitive-scalar, atomic-cell, and byte-sequence structural nodes without runtime-value checks. |
| A0659 | omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/compiler_builtins/retained.rs:210-217 | REACHABLE | Zero matching compiler-intrinsic rows for a demanded identity `continue`s with no diagnostic. |
| A0660 | omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/settlements.rs:290-308 | REACHABLE | When the schema has no declared calling application for the ABI, the retained row's application is returned without schema comparison. |
| A0661 | omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/settlements/source_imports.rs:55-57 | REACHABLE | Multiple selected non-external provider rows can `continue` when `selected_external` is absent. |
| A0662 | omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/static_interrupts/scopes.rs:565-568 | REACHABLE | Only `RegisterAccess` with `InterruptControl(Enable)` is treated as re-enabling interrupts in masked bodies. |
| A0663 | omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/terminal_authority_policy/host_cohorts.rs:127-135 | REACHABLE | Host cohort rows key on `method.name`, not the evaluated import locator, so authority can be narrower than the symbol's behavior. |

## A0658

Digest (read): area_04.md — `PA/` = `omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/`; provider-admission decides outside-provider admission.

Opened `require_runtime_value` in full at `access_lifecycles.rs:298-330` (read). After `Record`, `Sum`, `Mixed`, and non-zero `FixedArray`, the arm `_ => {}` at `:326` returns without error.

Caller search: `rg '\brequire_runtime_value\(' omega-rust` → 4 matches (read): definition `:298`, call from `require_image_contract` at `:260`, recursion at `:324`, and `require_fields` at `:351`.

Opened `require_image_contract` at `:239-296` (read). For non-view lifecycles it requires a record root at `:249-251`, rejects borrowed storage at `:252-258` via `structural_type_has_borrowed_storage_by`, then calls `require_runtime_value` at `:260`.

Opened `structural_type_has_borrowed_storage_by` at `omega-rust/psi/compilation/representations/terminal-psi/src/types_and_values.rs:1221-1248` (read). It walks record and sum trees for `Reference`, `ElementView`, and borrowed byte-sequence carriers; it does not treat `StructuralTypeShape::PrimitiveScalar`, `AtomicCell`, or plain `ByteSequence` carriers as borrowed storage.

Opened `require_fields` at `access_lifecycles.rs:332-355` (read). Structural children recurse through `require_runtime_value`; shapes that hit `_ => {}` add no further checks.

Upstream acceptance: `admit_native_access_lifecycles` at `:107` calls `require_image_contract`; failures become diagnostics via `map_err(fail)`.

Input in plain words: a native access-lifecycle record image whose nested structural carrier is declared as a primitive scalar, atomic cell, or non-borrowed byte sequence (not a record, sum, or mixed tree) passes `require_runtime_value` on that node because the default arm is empty.

Verdict **REACHABLE**: such a lifecycle shape should be rejected as an incomplete runtime whole-value image but is accepted on this branch.

## A0659

Digest (read): area_04.md:37 — provider-admission emits admitted settlements and closure receipts from `ProviderAdmissionRequest` inputs.

Opened `derive_selected_intrinsic_settlement_proposals` in full at `retained.rs:141-285` (read). The loop over `demanded_boundaries` at `:195-279` builds `matches` at `:197-209`. The pattern `let [selected_row] = matches.as_slice() else { ... }` at `:210-217`: when `matches.is_empty()`, neither the error arm (`!matches.is_empty()` at `:211-215`) nor success runs; execution `continue`s with no diagnostic.

Module comment at `:4-9` (read) states unmatched rows are deferred so only an entry that executes the boundary refuses later.

Caller search: `rg '\bderive_selected_intrinsic_settlement_proposals\(' omega-rust` → 4 matches (read): definition `retained.rs:141`, call `retained.rs:436`, re-exports in `compiler_builtins.rs` and `provider_admission.rs`, call in `retained_terminal.rs:803`.

Opened `admit_terminal_intrinsics` at `retained.rs:426-459` (read). It consumes `proposals` from derive and iterates retained entries at `:446-459`; it does not verify every `demanded` identity received a proposal. Empty `matches` yields no proposal for that requirement.

Opened `retained_terminal.rs:650-810` (read). `demands` collects every in-scope boundary identity at `:653-687`, then derives proposals at `:802-810`.

Input in plain words: a preparation pass that demands a callable identity which ought to resolve to exactly one selected compiler-intrinsic row, but selected plans and filters yield zero `CompilerIntrinsic` matches for that text, so derive returns `Ok` with no error and no row for that demand.

Verdict **REACHABLE**: the demanded identity gets no closing diagnostic here and compilation can proceed without an intrinsic proposal for it.

## A0660

Digest (read): area_04.md:41 — provider plans are matched by exact evidence (`PA/settlements.rs:312-325` in digest; line re-located at `:312-325` in current tree).

Opened `external_calling_application` in full at `settlements.rs:271-310` (read). It loads the retained `row.calling_application(abi)` at `:276-280`, checks dispatch projection at `:281-288`, then `let declared = plan.schema.calling_applications.iter().any(...)` at `:290-294`. The commitment and fingerprint comparison at `:301-307` runs only `if declared` at `:295`; if `declared` is false, the function returns `Ok(application)` at `:309` using the retained binding row only.

Caller search: `rg '\bexternal_calling_application\(' omega-rust` → 5 matches (read): definition `settlements.rs:271`, calls in `settlements.rs:426` and `:570`, and `source_imports.rs:95` and `:142`.

Opened `rejoin_normalized_foreign_call` at `settlements.rs:426` (read) and import mechanism derivation at `source_imports.rs:141-144` (read). Both use the returned application for boundary entry plans and foreign call binding without an alternate schema comparison when `declared` was false.

Input in plain words: an external binding row carries a calling application whose ABI requirement identity is absent from the selected provider schema's `calling_applications` list, so the row's commitment and fingerprint are never compared to an independently selected policy.

Verdict **REACHABLE**: a mismatched retained ABI application should be rejected against the schema but is accepted when no schema application is declared.

## A0661

Digest (read): area_04.md:62 — `PA/source_imports.rs` is among provider-admission identity-text sites.

Opened `derive_source_evaluated_mechanisms` at `source_imports.rs:32-188` (read). For each boundary bucket, `:55-57` skips when `rows.selected_external` is `None` via `continue`. The duplicate check `if rows.selected_count > 1` at `:66-70` runs only after a `Some` external row was found.

Opened `join_import_coverage_rows` at `:218-374` (read). At `:299-314`, every matching provider row increments `selected_count`; `selected_external` is set only when the binding is `Import`, `Syscall`, `VtableField`, or `TableFunction`. Two matching non-external rows (for example compiler-builtin or intrinsic bindings) increment `selected_count` to 2 without guaranteeing `selected_external` is `Some`.

Caller search: `rg '\bderive_source_evaluated_mechanisms\(' omega-rust` → 2 matches (read): definition `source_imports.rs:32`, call `settlements.rs:59` in `settle_provider_executions`.

Upstream: `settle_provider_executions` at `settlements.rs:59-64` (read) treats derive success as sufficient to continue boundary settlement.

Input in plain words: a demanded external-binding requirement resolves to two selected provider plan rows that are not import, syscall, or table bindings, so `selected_count` is at least two and `selected_external` stays `None`, and the per-boundary loop continues without reporting the ambiguity.

Verdict **REACHABLE**: duplicate non-external selection should be rejected but is not reported on this path.

## A0662

Digest (read): area_04.md — stage 03 selection includes static interrupt handling; provider-admission includes interrupt installation in `PA/scopes.rs`.

Opened `ConfigurationLease::require` at `scopes.rs:493-520` (read) and `require_masked` at `:556-578` (read). `require` flags `replaces_configuration` operations; `RegisterAccess` uses `changes_configuration` at `:509-510`. Opened `changes_configuration` at `:581-589` (read): it lists GDT load, task register, segment reload, system write, and MSR write; it does **not** include `RegisterOperation::InterruptControl(Enable)`.

`require_masked` at `:565-568` (read) is the sole site in this crate that rejects interrupt re-enablement, matching only `RegisterAccess` with `InterruptControl(Enable)`.

Caller search: `rg '\brequire_masked\(' omega-rust` → 2 matches (read): definition and call at `scopes.rs:232` during interrupt installation.

Opened installation loop at `:228-232` (read). Each installed handler closure gets `require` then `require_masked` on the same machine set.

Repo search: `rg 'InterruptControl' omega-rust/omega/compilation/realization/provider-admission` → only `scopes.rs:567-568` (read). Terminal lowering maps interrupt control to `RegisterAccess` in `terminal-psi` (read: `control_flow.rs:2106-2171`), but mask restoration that does not surface as that operation kind is outside this check.

Input in plain words: a masked static-interrupt installed handler body that restores the interrupt mask through any mechanism other than a `RegisterAccess` event classified as `InterruptControl(Enable)` is not rejected by `require_masked`, while `require` also does not treat `Enable` as configuration-changing.

Verdict **REACHABLE**: nested maskable arrivals could be re-enabled without tripping the only explicit enable check.

## A0663

Digest (read): area_04.md:62 — `PA/host_cohorts.rs` classifies host methods by name; `PA/terminal_authority_policy.rs` routes cohort rows.

Opened `filesystem_mechanism_row` at `host_cohorts.rs:123-135` (read). It calls `settled_filesystem_cohort(&method.name)?` at `:127` and builds disposition from those facets at `:131-134`; it does not read the normalized foreign locator or import evaluator payload.

Opened `settled_filesystem_cohort` at `:44-89` (read). Facets are chosen entirely from the method name string (for example `"read"` → `CONTENT_READ` at `:64-65`).

Opened `cohort_mechanism_row` at `terminal_authority_policy.rs:247-264` (read). Windows targets branch to `windows_imports::mechanism_row(foreign)` at `:255-258` (read: locator-reviewed dispositions in `windows_imports.rs:35-76`). Non-Windows paths use `host_cohorts::filesystem_mechanism_row` at `:260`.

Opened `derive_source_evaluated_mechanisms` at `source_imports.rs:177` (read). `settled_host_cohort_row` at `:190-216` resolves the method by requirement text, then `cohort_mechanism_row(mechanism, schema, method)` at `:211-215`.

Caller search: `rg '\bfilesystem_mechanism_row\(' omega-rust` → 2 matches (read): definition `host_cohorts.rs:123`, call `terminal_authority_policy.rs:260`.

Input in plain words: on a non-Windows target, a demanded import whose service schema method name implies a narrow filesystem facet cohort, while the evaluated import locator and admitted calling plan bind a host symbol whose real behavior is broader than that name, still receives a policy row keyed only on the method name facets.

Verdict **REACHABLE**: terminal authority can be mislabeled relative to the import's actual behavior off the Windows locator-reviewed path.

## What I did not read

- Full bodies of `admit_native_access_lifecycles`, `bind_profile`, and `windows_imports::mechanism_row` beyond the cited regions.
- Corpus or sample programs exercising these branches.
- `prepare_source_evaluated_bindings` and downstream admission error paths for missing compiler-builtin proposals beyond the cited calls.
- Omega-written `source/omega/` provider-admission counterparts, if any.
- `git show 457adf4a11f:` digests for line-by-line digest parity (current tree line numbers matched on inspection).

SLICE COMPLETE
