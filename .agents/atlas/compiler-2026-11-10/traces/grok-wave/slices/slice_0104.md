slice_0104

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0800 | omega-rust/omega/compilation/realization/provider-planning/src/data_layout_queries/builder/data_layouts.rs:170-185 | CLOSED | Silent skip only when private-callback demands are empty; non-empty demands error or fail in `finish` closure. |
| A0801 | omega-rust/omega/compilation/realization/provider-planning/src/data_layout_queries/builder/private_callback_closure.rs:69-74;184-186 | CLOSED | Enum/mixed roots are not walked; plan-laid callback closure is record-only; descent is record inline fields only. |
| A0802 | omega-rust/omega/compilation/realization/provider-planning/src/program_entry/source_contract.rs:121-128 | CLOSED | `ProgramEntryPhysicalContractPlan::new` rejects empty parameter/result identity strings during entry `settle`. |
| A0803 | omega-rust/omega/compilation/realization/provider-planning/src/provider_identity.rs:22 | CLOSED | Skipping `eligible` is gated by later `provider_row_matches` / row resolution when `application_identity` is set. |
| A0804 | omega-rust/omega/compilation/realization/provider-planning/src/provider_instances.rs:229;256 | CLOSED | Empty inherited list is intentional; syscall/intrinsic rows use `closed_provider_leaf_applications` instead. |
| A0805 | omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/external_binding_rows.rs:167-194 | CLOSED | Rows with empty `calling_applications` fail syscall rejoin / `external_calling_application` at admission. |
| A0806 | omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/provenance_replay.rs:357-391 | CLOSED | Leaf binding without catalog proof is refused at boundary selection when execution identity is missing. |
| A0807 | omega-rust/omega/compilation/realization/provider-planning/src/provider_planning/provenance_replay.rs:175-176;named_provider_plans.rs:154;plan_derivation.rs:112 | CLOSED | Empty interned requirement identity is rejected at provider installation and plan lookup filters. |

### A0800

1. Read `compute_data_layout` in full in `data_layouts.rs:35-244` (read); parent `layout_data_definition` in `builder.rs:277-308` (read).
2. Search `\blayout_data_definition\b` under `omega-rust`: 4 hits (read) — `builder.rs:89`, `builder.rs:277`, `type_reference_layouts.rs:129`, `:251`, `:281`.
3. Search `\bclose_private_callback_layouts\b` under `omega-rust`: 2 hits (read) — definition `private_callback_closure.rs:22`, sole call `builder.rs:256`.
4. At `data_layouts.rs:170-185` (read), `native_policy_identity` and `native_data_identity` errors are swallowed via `.ok()` / `.and_then(... .ok())`; identity row is pushed only in the `if let (Some, Some)` at `211-235` (read).
5. Input to accepting branch: a plan-laid **record** (`90`) whose `geometry.private_callback_demands` is empty (`191-192`) while `native_policy_identity` or `native_data_identity` fails (read).
6. If demands are non-empty and policy is missing, `194-200` errors (read); if demands are non-empty and identity row is missing, `close_private_callback_layouts` / `close_path` can error on missing child identity (`private_callback_closure.rs:262-265`, read).
7. **Verdict: CLOSED** — compile acceptance does not keep a broken plan-laid callback layout; silent omission applies only without callback demands.
8. Unsure: whether any checked-tree ill-formed plan-laid record with empty demands and failed identity can reach `layout_data_definition` without being rejected earlier in Psi validation (not traced).

### A0801

1. Read `close_private_callback_layouts` in full `private_callback_closure.rs:22-164` (read); `reaches_slot` `166-196` (read); `close_path` `198-311` (read).
2. Entry cites `69-74`: non-`Record` root shapes `continue` without adding identity (read). Entry cites `184-186`: `reaches_slot` returns false for non-Record layouts (read).
3. Read `compute_data_layout` branch at `data_layouts.rs:77-81` (read): enum/mixed types use `compute_case_bearing_layout`, not the plan-laid block at `90+`.
4. Input to accepting branch: enum/mixed **root** data symbol when closing callback layouts; such roots are skipped (read).
5. **Verdict: CLOSED** — callback slot closure descends only `DataShape::Record` inline children (`247-307`); enum/mixed roots are not plan-laid in this path (`77-81`) and are not descended in `reaches_slot` (`184-186`).
6. Unsure: none material for compile gate; separate record symbols for variant payload carriers are handled as Record roots, not enum roots (inferred from shape rules, not exhaustively traced in validation).

### A0802

1. Read enclosing function `prepare_entry_calling_contract` in `source_contract.rs:46-131` (read).
2. Search `unwrap_or_default` in `program_entry/`: 2 hits at `source_contract.rs:123` and `:128` (read).
3. Read consumer `calling_plans::settle` in `program_entry.rs:598-745` (read), especially call to `ProgramEntryPhysicalContractPlan::new` at `721-732` (read).
4. Read `ProgramEntryPhysicalContractPlan::new` `program_entry_physical.rs:123-165` (read).
5. Input to accepting branch: `typed.type_identity_text(...)` returns `None` for a physical entry parameter or result; `unwrap_or_default` yields `""` (`121-128`, read).
6. **Verdict: CLOSED** — `new` rejects empty `requirement_identity`, any empty parameter identity, or empty result (`154-164`, read); `settle` surfaces that as compile failure (`732`, read).
7. Unsure: none.

### A0803

1. Read `named_provider` in full `provider_identity.rs:16-30` (read).
2. Search `\bnamed_provider\b` in `omega-rust`: 6 hits (read) — definition `provider_identity.rs:16`; calls at `:66`, `:136`, `:190`; `provider_instances.rs:171`; `adapter_conformance.rs:95` (production). Psi `machine_execution.rs` uses a local variable name only (read grep list, not traced in full).
3. Read `provider_trait_application` `51-112` and `provider_row_matches` `180-260` (read), especially `application_identity` branch `225-241` (read).
4. Read `eligible` definition `psi/compilation/build-time-evaluation/src/product.rs:1938-1940` (read): empty type parameters only.
5. Input to accepting branch: `ProviderPlan` with `application_identity().is_some()` so filter at line 22 does not require `eligible(definition)` (read).
6. **Verdict: CLOSED** — `named_provider` alone does not accept a program; row/machine matching with `application_identity` at `225-241` and single-match at `28-29` block wrong conformances (read). Callers I did not read line-by-line: full `provider_settlement` orchestration.
7. Unsure: whether Psi allows two conformances with identical display path and package such that `named_provider` could return the wrong one before `provider_row_matches` runs (not searched in validation).

### A0804

1. Read `prepare_instance_declarations` in full `provider_instances.rs:141-279` (read).
2. At `229` and `256` (read), `Import` / `VtableField` / `TableFunction` / `VtableSlot` arms yield `Ok(Vec::new())` for inherited instances; `CompilerIntrinsic` / `Syscall` use `closed_provider_leaf_applications` at `175-196` (read).
3. Search caller `prepare_instance_declarations`: 1 hit `provider_instances.rs:81` (read).
4. Input to accepting branch: plan row whose binding is not `CheckedAdapter` (read).
5. **Verdict: CLOSED** — not a silent accept; alternate route supplies leaf applications for intrinsic/syscall rows (`175-196`, read). Inherited instances apply only to checked adapters (`197-228`, read).
6. Unsure: whether any admission path requires non-empty inherited rows for import/vtable bindings (not read in `provider-selection/src/binding/provider_instances.rs`).

### A0805

1. Read `ExternalBindingContext::row` `external_binding_rows.rs:84-208` (read), syscall policy at `167-194` (read).
2. Read `CallingPolicy::native_syscall_for_target` `vocabulary.rs:498-510` (read): `None` for Windows/macOS non-Linux syscall targets.
3. Read admission `rejoin_system_call` `settlements.rs:501-593` and `external_calling_application` `271-309` (read).
4. Input to accepting branch: `ProviderBinding::Syscall` row where `selected.applications` is empty and `native_syscall_for_target` is `None`, so `calling_applications` stays empty but row is still `Ok(Some(...))` at `195-207` (read).
5. **Verdict: CLOSED** — admission requires syscall policy (`518-519`, read) and a matching calling application (`570-571`, read); empty roster does not complete compile.
6. Unsure: whether Terminal artifact encoding rejects empty `calling_applications` before admission (not read).

### A0806

1. Read `inferred_selected_target_compiler_leaf` in full `provenance_replay.rs:357-391` (read); compare to hosted path with `exact_catalog_signature` at `328-348` (read).
2. Read `inferred_compiler_leaf_binding` call site `plan_derivation.rs:75-84` (read).
3. Read `IntrinsicDeclaration::settle` `intrinsic_declarations.rs:274-333` (read): handles Hosted/Fixed/Access, not arbitrary inferred compiler leaves.
4. Read boundary selection `provider-selection/src/boundary.rs:97-112` (read) and `selected_external_execution` `bodies.rs:144-197` (read).
5. Input to accepting branch: non-hosted target, bodiless boundary machine with attached data, single state, sole target origin, passing `satisfied_boundary_requirement` only (`382-387`, read), without catalog signature checks unlike hosted path.
6. **Verdict: CLOSED** — plan may carry `CompilerIntrinsic` binding, but compile refuses boundary realization without execution identity (`boundary.rs:109-111`, read). `IntrinsicDeclaration::settle` is not the gate for this leaf (read).
7. Unsure: earlier stages that might refuse the machine before planning (not traced).

### A0807

1. Read `exact_satisfied_requirement_identity` `provenance_replay.rs:158-181` (read): missing trait yields `String::new()` at `174-176`.
2. Read use sites `plan_derivation.rs:112-142` and `named_provider_plans.rs:154-158`, `242` (read): empty string interned via `mint_plan_row_identity` / `plan.identities.intern` without empty check at push (`139-152`, read).
3. Read closures `source_provider_selections.rs:85-87` (read), `provider_installation.rs:271-276` (read), `adapters.rs:60-67` (read for checked-adapter path).
4. Input to accepting branch: `trait_symbol` / `clause.symbol` with no `trait_definition_by_symbol`, producing empty requirement identity text interned into plan rows (read).
5. **Verdict: CLOSED** — provider installation rejects empty lowered-catalog requirement identity (`271-276`, read); plan selection by identity skips empty (`source_provider_selections.rs:85-87`, read).
6. Unsure: every derivation path that mints empty identity is covered by installation (read only installation and selection, not full compiler product path).

### What I did not read

- Full Psi validation paths for plan-laid layouts, duplicate conformances, and enum variant callback placement.
- Full `provider_settlement`, Terminal artifact validators, and `provider-selection/src/binding/provider_instances.rs` projection.
- `compute_case_bearing_layout` body beyond skim `275-349` (read partial only).
- Hosted vs non-hosted compile product paths end-to-end from compiler crate.

SLICE COMPLETE
