# slice_0106 (ACCEPT | area_01)

Digest preamble (`digests/area_01.md`, lines 1–8): Area 1 is CLI, build orchestration, compiler coordination, and provider realization under `omega-rust/omega/`; path shorthand `pp/`, `ps/`, `sd/` map to provider-planning, provider-selection, and selected-dispatch.

| id | file:line | verdict | one-line reason |
| A0816 | `omega-rust/omega/compilation/realization/provider-planning/src/service_schema/mod.rs:44` | CLOSED | Only `BoundaryTrait` enters the attachment loop; other schema kinds do not carry `calling_applications` by this owner (read). |
| A0817 | `omega-rust/omega/compilation/realization/provider-selection/src/binding.rs:159` | CLOSED | Zero grant matches is intentional when no owner-scoped grant exists; issuers come only from admitted qualification evidence (read). |
| A0818 | `omega-rust/omega/compilation/realization/provider-selection/src/binding/provider_instances.rs:81` | CLOSED | Empty candidate rosters skip attachment; `required_instances` still errors when a closed requirement has no instance (read). |
| A0819 | `omega-rust/omega/compilation/realization/provider-selection/src/bodies.rs:186` | CLOSED | Returning retained `None` without a provider still fails boundary admission and builtin settlement when a closed identity is required (read). |
| A0820 | `omega-rust/omega/compilation/realization/provider-selection/src/bodies/callback_roots.rs:175` | UNDECIDED | External registration without a selected row is treated as non-foreign; whether every external registration must bind a plan is not traced past callback root selection. |
| A0821 | `omega-rust/omega/compilation/realization/provider-selection/src/bodies/target_bodies.rs:76` | CLOSED | No selected program entry yields an empty reach frontier and `Ok(())` by design at product settlement (read). |
| A0822 | `omega-rust/omega/compilation/realization/provider-selection/src/publication.rs:494` | CLOSED | Skips machines without proof-scope declaration or operations absent from the sorted demand roster during fixed-point demand growth (read). |
| A0823 | `omega-rust/omega/compilation/realization/selected-dispatch/src/compiler_intrinsic.rs:137` | CLOSED | `Unsupported` becomes `None` row custody; later boundary and builtin admission refuse missing closed intrinsic identity (read). |

## A0816

**Cited site (read):** `attach_requirement_calling` at `service_schema/mod.rs:35-66` builds `ServiceSchema::from_declaration` then, at line 44, runs the `calling_plans` filter loop only when `source` is `ProviderSchemaDeclaration::BoundaryTrait(_)`. `BoundaryRequirement`, `BoundaryOperator`, and `SealedAccessor` (enum at `typed-trees/.../service_declarations.rs:3073-3077`, read) never receive attachments here.

**Enclosing function (read):** Full `attach_requirement_calling` and public `attach_calling` (`mod.rs:18-33`) read.

**Callers (search `omega-rust` for `\battach_calling\b`, count mode):** 10 production call sites (`service_declarations.rs:74`, `named_provider_plans.rs:65`, `plan_derivation.rs:326`, `prepared_providers.rs:129`, `provider_settlement.rs:173`, `program_entry.rs:702`, plus four sites with empty `&[]` calling plans). Definition at `mod.rs:18`.

**Upward trace (read):** `settlements.rs:290-308` only compares external ABI to `plan.schema.calling_applications` when a matching entry exists (`if declared`). `direct_boundary_leaves.rs:60-61` rejects relabelling public calling attachments on synthetic direct routes. Non-trait schemas are not intended to populate `schema.calling_applications` at this site; trait calling evidence is supplied separately via settlement `calling_applications` (`provider_settlement.rs:127-156`).

**Input that reaches the branch:** Any prepared service surface whose `ProviderSchemaDeclaration` is not `BoundaryTrait` passes through `attach_calling` with `calling_applications` left empty.

**Verdict CLOSED:** The branch is deliberate schema-kind policy, not a missing rejection check for the other three declaration kinds.

## A0817

**Cited site (read):** `validate_qualification_grants` at `binding.rs:87-169`; line 158-159 `match matches.as_slice() { [] | [_] => {}` accepts zero or one matching granted plan per issuer.

**Enclosing function (read):** Full function and caller `bind_selected_provider_plan_facts` (`binding.rs:58-85`) read.

**Callers (search `\bvalidate_qualification_grants\b` in `omega-rust`):** 1 production call (`binding.rs:76`); re-export in `provider-selection/lib.rs`.

**Upward trace (read):** `bind_selected_provider_plan_facts` invoked from `selected-dispatch/provider_binding.rs:155-173` with `settlement.declarations.issuers` derived from checked facts. Issuers are built only via `FactPlan::admitted_qualification_issuers` (`checked-trees/facts/src/lib.rs:1280-1298`, read) from facts with `QualificationEvidenceOrigin::AdmittedReceipt`. Module header (`binding.rs:11-17`, read) states zero unscoped matches keep fail-closed behavior when the product-owner slot has no grant; ambiguity is rejected at lines 160-164 when `matches.len() > 1`.

**Input:** A qualification issuer whose requirement identity matches no granted selected plan hits the `[]` arm and returns `Ok(())` from the function.

**Verdict CLOSED:** Documented policy for issuers without a matching grant; conflicting grants still error.

## A0818

**Cited site (read):** `project_provider_instances` at `provider_instances.rs:36-194`; line 71 `continue` for non-`CheckedAdapter`/`CompilerIntrinsic`/`Syscall` bindings; line 81-82 skip when `application.candidates.is_empty()`; line 99 `[] => continue` in the candidate slice match.

**Enclosing function (read):** Full function read.

**Callers (search `\bproject_provider_instances\b` in `omega-rust`):** 1 production call in `provider-planning/provider_instances.rs:75` (`project_instances`).

**Upward trace (read):** `prepare_instance_declarations` fills `required_instances` from `closed_requirements` (`provider_instances.rs:265-270`, read). Lines 172-184 error when any required closed requirement lacks exactly one attached instance after processing.

**Input:** An inherited application row with an empty `candidates` vector is skipped without diagnostic; a template `required_instances` entry with zero matching rows still fails.

**Verdict CLOSED:** Skips are limited to optional application rows; closed requirements retain the exactly-one check.

## A0819

**Cited site (read):** `selected_execution_for_identity` at `bodies.rs:167-197`; lines 184-186 when `selected_numeric_provider` returns `None`, `reconcile_numeric_meaning(None, retained)?` then `Ok(retained)`.

**Enclosing function (read):** Full function and `reconcile_numeric_meaning` at `effects/external_providers.rs:153-170` (read).

**Callers (search `\bselected_external_execution\b` in `omega-rust`):** 3 production references (`boundary.rs:98`, `compiler_builtins/retained.rs:228`); internal `selected_execution_for_identity` also at `boundary.rs:284`.

**Upward trace (read):** `BoundaryProviderAdmission::require_numeric` (`boundary.rs:276-298`) passes `None` retained; `Ok(None)` becomes diagnostic at `:293`. `retained.rs:228-255` maps `Ok(None)` to `NoClosedCatalogIdentity`; `compiler_builtins.rs:75-78` errors before admission. `reconcile_numeric_meaning` errors when retained is numeric and no candidate meaning matches (`external_providers.rs:168-169`).

**Input:** Numeric boundary with no external provider candidate and no compatible retained identity is refused downstream even though this site returns `Ok(retained)` when reconcile succeeds.

**Verdict CLOSED:** The permissive return is guarded by reconcile and closing checks on admission paths.

## A0820

**Cited site (read):** `registration_is_foreign` at `callback_roots.rs:166-185`; lines 174-176 `let Some(provider) = provider else { return Ok(false); }` for `External` registrations.

**Enclosing function (read):** `select_callback_declarations` (`callback_roots.rs:100-164`) read in full.

**Callers (search `\bselect_callback_declarations\b` in `omega-rust`):** 2 production call sites (`provider_binding.rs:194`, plus test support).

**Upward trace (read):** `provider_binding.rs:187-207` merges `installed_roots` (`projections.rs:391-421`, read), which errors interrupt installations lacking checked entry or calling plan. For nominal uses with `has_checked_entry`, foreign roots are pushed only when `foreign_registration` is true (`callback_roots.rs:155-157`). External registration without a selected provider row yields `foreign_registration == false`, so the use is not added to roots and no error is emitted in that branch.

**Input:** `CallbackRegistrationDeclaration::External(identity)` where `providers.selected_row(identity)` is `None`.

**Verdict UNDECIDED:** Could not determine whether every such registration must be rejected or may legitimately omit a root without `installed_roots` coverage; did not read all callback admission consumers beyond `provider_binding.rs` and `installed_roots`.

## A0821

**Cited site (read):** `reject_reached_inert_calls` at `target_bodies.rs:65-127`; lines 76-80 build frontier from `entry` only; lines 119-120 return `Ok(())` when `refusals` is empty (including empty frontier).

**Related site (read):** Lines 30-32 skip direct calls with `call.machine == None`; lines 92-97 drop provider-row realizations with no graph position.

**Enclosing function (read):** Full function read.

**Callers (search `\breject_reached_inert_calls\b` in `omega-rust`):** 1 production call via `target_machines.rs:125-130`.

**Upward trace (read):** `execution_settlement.rs:357-364` calls `reject_reached_inert_sibling_calls` with `selected_program_entry.as_ref().map(|entry| entry.machine_symbol())`, so products with no settled entry pass `None` and skip inert-call refusal (digest `area_01.md:259`, read).

**Input:** Product compilation without a selected program entry reaches empty frontier and success.

**Verdict CLOSED:** Matches documented intentional omission when no entry is selected at settlement.

## A0822

**Cited site (read):** `extend_application_demand` in `publication.rs:412-538`; lines 494-497 `continue` when `callable_declaration(Machine(...))` is `None`; lines 503-506 `continue` when the operation is absent from sorted `boundary_application_demands`.

**Enclosing function (read):** `extend_application_demand` and caller loop in `PreparedTargetChoices::select` (`publication.rs:318-328`) read.

**Callers (read):** Invoked only from `select` within the same file during demand fixed-point growth.

**Upward trace (read):** Module header (`publication.rs:12-27`, read) states demand is grown against the Terminal catalog; unreachable machines and operations without demand records do not expand the invoked-owner set. Demand discovery for boundary applications is driven by `reachable` from proof-scope applications (`:460-472`).

**Input:** A machine in `module.machines` without a proof-scope machine declaration, or an operation not listed in `boundary_application_demands`, is skipped when marking providers demanded.

**Verdict CLOSED:** Skips apply to entities outside the demand/roster join used for this fixed point, not to operations already recorded as demanded.

## A0823

**Cited site (read):** `settle_declaration` at `compiler_intrinsic.rs:127-139`; lines 136-137 map `settle` `None` to `SelectedCompilerIntrinsicExecutionIdentity::Unsupported` inside `Ok`, not `Err`.

**Enclosing function (read):** Full `compiler_intrinsic.rs` functions `numeric_execution` and `settle_declaration` read.

**Callers (search `\bsettle_declaration\b` in `omega-rust`):** 2 sites (`compiler_intrinsic.rs:105`, `intrinsic_review.rs:246`).

**Upward trace (read):** `intrinsic_review.rs:262-264` handles `Unsupported` by `accumulate` and `rows.push(None)`; function returns `Err` only when `diagnostics` non-empty (`:315-317`). `select_boundary_applications` (`boundary.rs:98-111`, read) requires `execution.ok_or_else(...)` for compiler-intrinsic realizations on actual Terminal demands. `compiler_builtins.rs:75-78` rejects `NoClosedCatalogIdentity` proposals. Header comment (`intrinsic_review.rs:10-13`, read) describes `None` custody pending package review; downstream demand join still errors when a boundary application requires an identity.

**Input:** Intrinsic `settle` returns `None` for a compiler-intrinsic row; review records `None` in `row_compiler_intrinsic_executions`.

**Verdict CLOSED:** `Unsupported` is not a compile-success path when Terminal boundary demand requires a closed execution identity.

## What I did not read

- Full `produce_direct_boundary_leaves` after `compiler_intrinsic.rs:389` for every `None` settle path.
- All consumers of `schema.calling_applications` beyond sites cited for A0816.
- Complete callback pipeline beyond `provider_binding.rs`, `installed_roots`, and `callback_roots.rs`.
- `reconcile_numeric_meaning` callers beyond `bodies.rs` and `retained.rs`.
- Qualification issuer emission logic in `published_facts.rs` beyond `admitted_qualification_issuers`.
- Corpus or sample programs exercising any REACHABLE hypothesis (no build per STREAM A).

SLICE COMPLETE
