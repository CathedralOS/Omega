# slice_0104

Claims: **ACCEPT** · **area_01** (preamble: `.agents/atlas/compiler-2026-11-10/digests/area_01.md`, path shorthand `pp/` = `omega-rust/omega/compilation/realization/provider-planning/src/`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0800 | `pp/data_layout_queries/builder/data_layouts.rs:170-185` | CLOSED | Omitted identity is never read in production; with callback demands, sibling paths error instead of finishing silently |
| A0801 | `pp/data_layout_queries/builder/private_callback_closure.rs:69-74`, `:184-186` | UNDECIDED | Record-only descent is read; whether a callback demand can live only inside an enum/mixed payload root is not established here |
| A0802 | `pp/program_entry/source_contract.rs:121-124`, `:128` | CLOSED | `ProgramEntryPhysicalContractPlan::new` rejects empty parameter/result type identity strings before the contract is kept |
| A0803 | `pp/provider_identity.rs:22` | CLOSED | Skipping `eligible` is gated; `provider_row_matches` still requires `application_identity` capture match when the plan carries one |
| A0804 | `pp/provider_instances.rs:229`, `:256` | CLOSED | Empty inherited vectors on non-adapter rows still leave `required_instances` enforced in `project_provider_instances` |
| A0805 | `pp/provider_planning/external_binding_rows.rs:167-194` | CLOSED | Syscall rows with empty `calling_applications` fail at admission rejoin (`native_syscall_for_target` / ABI lookup) |
| A0806 | `pp/provider_planning/provenance_replay.rs:357-391` | CLOSED | Weak leaf inference still hits `IntrinsicDeclaration::prepare` / `settle` and boundary selection errors on missing execution identity |
| A0807 | `pp/provider_planning/provenance_replay.rs:175-176` | UNDECIDED | Boundary-trait `plan_derivation` path mints empty identity without a same-path empty check (unlike the independent-satisfaction loop) |

## A0800

**Cited site (read):** `omega-rust/omega/compilation/realization/provider-planning/src/data_layout_queries/builder/data_layouts.rs:170-185` — `canonical_data_identity` uses `.ok()` on `plan.native_data_identity(...)`; the push into `data_layout_identities` at `:211-236` runs only when both `canonical_data_identity` and `canonical_layout_subject` are `Some`.

**Enclosing function (read):** `compute_data_layout` for plan-laid records (`:90-244`).

**Callers (search `compute_data_layout` under `omega-rust/omega/compilation/realization/provider-planning`, count 3 production):** `builder/type_reference_layouts.rs:227`, `:236`; `builder.rs:302`.

**Upward (read):** `LayoutBuilder::finish` (`builder.rs:255-274`) passes `data_layout_identities` into `close_private_callback_layouts`. If `private_callback_demands` is non-empty and `canonical_layout_subject` is missing, `data_layouts.rs:194-200` returns `Err`. If demands reach a child without a unique closed layout, `private_callback_closure.rs:262-265` errors.

**Consumer search (read):** ripgrep `\b\.data_layout_identities\b` over `omega-rust` → **4** matches, all in `data_layout_queries/builder*` (writes/pass-through only); field is stored on `LayoutPlan` (`layout_plan.rs:253`) with **no** production reads located.

**Plain input:** A plan-laid record with **no** private callback demands where `native_data_identity` fails while layout placement still succeeds.

**Verdict:** **CLOSED** — the digest scenario does not reach a program-acceptance branch that depends on the missing row; the field is not consumed on traced paths, and the demand-nonempty case is fail-closed elsewhere.

## A0801

**Cited sites (read):** `private_callback_closure.rs:69-74` — non-`DataShape::Record` roots `continue`; `:184-186` — `reaches_slot` only walks `Record` children via `inline_data_symbol()`.

**Enclosing functions (read):** `close_private_callback_layouts` (`:22-164`), `reaches_slot` (`:166-196`).

**Callers (search `close_private_callback_layouts`, count 2):** `data_layout_queries/builder.rs:256` only.

**Upward (read):** Enum/mixed types use `compute_case_bearing_layout` (`data_layouts.rs:275-327`), not the plan-laid record branch. Callback closure iterates `data.iter()` roots without special-casing variant payload carriers.

**Plain input (digest):** A callback slot that exists only under an enum/mixed case payload, not as a record root.

**Verdict:** **UNDECIDED** — code shows enum/mixed roots are skipped and descent is record-only; I did not trace whether `PlanLaidGeometry.private_callback_demands` can name such a placement in product source.

## A0802

**Cited sites (read):** `program_entry/source_contract.rs:121-124`, `:128` — `type_identity_text(...).unwrap_or_default()` for parameter and result type identity strings on `PreparedEntryCallingContract`.

**Enclosing function (read):** `prepare_entry_calling_contract` (`source_contract.rs:90-131`).

**Callers (read):** `program_entry.rs` unpacks `parameter_type_identities` / `result_type_identity` and calls `ProgramEntryPhysicalContractPlan::new` at `:721-732`.

**Closing check (read):** `omega-rust/omega/compilation/backend/plans/program-entry-plan/src/program_entry_physical.rs:154-164` rejects `requirement_identity.is_empty()`, any empty entry in `parameter_type_identities`, or `result_type_identity.is_empty()`.

**Plain input:** Physical entry parameters whose type identity text is missing in `TypedTrees` (maps to `""` at the cited site).

**Verdict:** **CLOSED** — every path that publishes the physical entry contract goes through `new`, which errors on empty identity text.

## A0803

**Cited site (read):** `provider_identity.rs:22` — filter uses `(plan.application_identity().is_some() || eligible(definition))`.

**`eligible` (read, re-located):** `omega-rust/psi/compilation/build-time-evaluation/src/product.rs:1938-1940` — `definition.type_parameters.is_empty()`.

**Enclosing function (read):** `named_provider` (`provider_identity.rs:16-30`).

**Callers (search `\bnamed_provider\b` in `omega-rust`, count 9; 1 definition, 5 production in `provider-planning`):** `provider_identity.rs:66`, `:136`, `:190`; `provider_instances.rs:171`; `provenance_replay/adapter_conformance.rs:95`.

**Upward (read):** `provider_row_matches` (`provider_identity.rs:180-249`) — when `plan.application_identity().is_some()`, match requires `typed.portable_conformance_application_identity(...)` text equals `expected` (`:225-241`); when `None`, requires `implementation_capture(...).is_none()` (`:243-248`).

**Plain input (digest):** A named plan with `application_identity` set, matching a conformance with type parameters (not `eligible`) on display path alone.

**Verdict:** **CLOSED** — acceptance through `named_provider` still requires downstream row matching; the application-identity arm is not bypassed.

## A0804

**Cited sites (read):** `provider_instances.rs:229`, `:256` — `_ => Ok(Vec::new())` for bindings other than `CheckedAdapter` in inherited and generic instance rows.

**Enclosing function (read):** `provider_instance_declarations` (`provider_instances.rs:146-279`).

**Callers (read):** `SelectedProviderSource` in same module feeds `provider_selection::project_provider_instances`.

**Upward (read):** `omega-rust/omega/compilation/realization/provider-selection/src/binding/provider_instances.rs:56-71` — non-`CheckedAdapter` / non-intrinsic rows `continue` instance projection; `:172-184` still requires each `template.required_instances` to match exactly one specialized row.

**Plain input (digest):** Plan rows bound as `Import` / `VtableField` / `TableFunction` / `VtableSlot` while the generic template lists closed requirement instances.

**Verdict:** **CLOSED** — empty inherited data on those rows does not skip `required_instances` cardinality at selection bind time.

## A0805

**Cited site (read):** `external_binding_rows.rs:167-194` — for `Syscall`, `compatibility_policy = CallingPolicy::native_syscall_for_target(native_target)`; default applications are extended only when `calling_applications.is_empty()` **and** `compatibility_policy` is `Some`.

**`native_syscall_for_target` (read):** `omega-rust/omega/compilation/backend/calling-conventions/src/plans/vocabulary.rs:498-504` — returns `None` for e.g. Windows/macOS pairs; Linux ELF returns `Some`.

**Enclosing function (read):** `ExternalBindingContext::row` (`external_binding_rows.rs:84-208`).

**Callers (read):** `extract_native_external_binding_rows` (`:15-38`); settlement at `provider_settlement.rs:230-238`.

**Closing checks (read):** `provider_admission/settlements.rs:518-519` — `native_syscall_for_target(target).ok_or_else(...)?` in `rejoin_system_call`; `:271-280` — `external_calling_application` errors when `calling_application(abi)` is missing on the retained row.

**Plain input:** Syscall-bound row for a target with no syscall policy (e.g. `linux_x86_64` syscall plan selected but profile mapping yields no policy, or Windows target) with no selected calling applications.

**Verdict:** **CLOSED** — row emission alone does not complete admission; syscall realization rejoin fails closed on empty/mismatched applications or missing policy.

## A0806

**Cited site (read):** `provenance_replay.rs:357-391` — `inferred_selected_target_compiler_leaf` binds `CompilerIntrinsic { machine }` after loose boundary/bodiless checks (no hosted catalog signature gate).

**Callers (search `\binferred_selected_target_compiler_leaf\b`, count 2):** `provenance_replay.rs:408` via `inferred_compiler_leaf_binding`.

**Upward — derivation (read):** `plan_derivation.rs:75-84` may install the binding on the candidate plan.

**Upward — declaration (read):** `provider_rows.rs:390-401` — `IntrinsicDeclaration::prepare` for `CompilerIntrinsic` rows.

**`prepare_kind` (read):** `intrinsic_declarations.rs:237-238` — non-hosted bundle → `IntrinsicDeclarationKind::Unsupported`; hosted rows run `boundary_row_shape` (`:606-678`) which returns `Ok(false)` on mismatch.

**`settle` (read):** `intrinsic_declarations.rs:274-333` — `Unsupported` → `Ok(None)`.

**Review / selection (read):** `selected-dispatch/intrinsic_review.rs:246-265` — `Unsupported` → `row_compiler_intrinsic_executions` entry `None` without necessarily aborting the whole review loop; `provider-selection/boundary.rs:109-111` — `CompilerIntrinsic` demand requires `execution.ok_or_else(|| error("selected intrinsic has no canonical or admitted execution identity"))`.

**Plain input:** Non-hosted target, bodiless boundary machine with attached data and one state, not in the hosted catalog, inferred as a compiler-intrinsic leaf.

**Verdict:** **CLOSED** — compilation that must **use** the row at a Terminal boundary demand fails at `select_boundary_applications`; weak inference does not yield a closed execution identity on traced demand paths.

## A0807

**Cited site (read):** `provenance_replay.rs:174-175` — `trait_definition_by_symbol` missing → `return String::new()` from `exact_satisfied_requirement_identity`.

**Mint sites (read):** `named_provider_plans.rs:154-158` → `plan_derivation.rs:112-142` via `mint_plan_row_identity` (`plan_derivation.rs:550-554`) with no empty-string guard on the boundary-trait branch.

**Contrasting close (read):** `plan_derivation.rs:263-266` — independent satisfaction skips when `requirement_identity.is_empty()`.

**Other filter (read):** `source_provider_selections.rs:85-86` — empty trait requirement identity → `None` for that lookup shape (not all plan-derivation paths).

**Callers (search `\bexact_satisfied_requirement_identity\b`, count 5):** definition `provenance_replay.rs:158`; `named_provider_plans.rs:154`; `plan_derivation.rs:112`, `:112` import side.

**Plain input:** Clause whose declaring trait symbol does not resolve to a trait definition during replay; identity becomes `""` and is interned into a plan row.

**Verdict:** **UNDECIDED** — I read the empty mint on the boundary-trait derivation path and the partial guards elsewhere; I did not trace every consumer that matches `plan.identities.text(row.requirement_identity)` to prove acceptance or rejection end-to-end for that path only.

## What I did not read

- Full `IntrinsicDeclaration::prepare_kind` hosted table and uefi fixed rows beyond the branches cited for A0806 (`intrinsic_declarations.rs:189-235`).
- `PlanLaidGeometry` authoring rules for enum/mixed callback slots (A0801).
- Entire `provider_admission/compiler_builtins/retained.rs` settlement loop beyond the boundary-selection excerpts (A0806/A0807).
- `git show 457adf4a11f` file snapshots; line numbers are from the current checkout with symbol re-location noted where relevant.

SLICE COMPLETE
