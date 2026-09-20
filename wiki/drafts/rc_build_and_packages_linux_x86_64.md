# RC build-and-packages — linux_x86_64 row

Witnessed row of the release-candidate `RC-BUILD-AND-PACKAGES` gate
(`wiki/drafts/rust_compiler_completion.md`) on the Linux x86-64 host.
Recorded at revision `e76d715c8e` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable — Cargo used per AGENTS.md). Every command
below was executed on this host at that revision.

Verdict: **red** — 1540 pass / 105 fail / 2 skipped across the seven
library crates (4724s), 328 pass / 22 fail across the six compiler test
targets (755s), doctests green. Every failure is a fixture-migration or
expectation-drift residual; no leg that reached a produced package product
misbehaved at runtime.

## Commands and results

| leg | command | result |
|-----|---------|--------|
| Library suites | `cargo nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast` | **105 fail / 1540 pass / 2 skip** (4724s) |
| Doctests | `cargo test --doc -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager` | pass (7 targets, 2 doctests, 0 fail) |
| Compiler targets | `cargo nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast` | **22 fail / 328 pass** (755s) |

## Library-suite failures (105: package-manager 52, package-evidence 53)

Grouped by the diagnostic signature the failing leg actually emits — one
assertion mechanism each, not one per test:

| family | legs | signature |
|--------|------|-----------|
| `Service<R>` closed-carrier respell residual | ~45 | fixture sources still spell `in Bound`/`in WeakFair` or bare boundary traits in value position; check now emits "the core `Service` carrier is closed; it admits no authored qualification" / "the intrinsic `Service<R>` carrier is the only service value spelling" / routed-`Service<R>` rung-placement errors. Dominates `review::candidate::compilation` (4), `callable_policy::mutation`/`flows`, `trait_contracts::operational_envelopes`, `terminal_permission_policy::uefi`, `operational::authored_sources` (`trait Child requires unknown trait Service`), `operations::{check,compile}_project` admissions |
| duplicated bundled std fixture | ~35 | package-evidence fixtures that vendor a std copy now collide with the real std library: "duplicate data `SystemVEightbyteClass`/`ValueClass`/`ValueShape`/…" plus one range-invariant store obligation. Dominates `calling_policy_source` (8), `calling_policy_lifetimes` (5), `calling_policy_substitution` (4), `calling_policy_opaque` (2), `authority::toolchain_provenance` (2) |
| `select_provider` operand resolution | ~6 | "provider selection operand does not resolve to one visible product declaration" — package-manager `package_lineage_spoofing`, `package_evidence_fixtures::compiler_review_evidence`, `named_workspace_install::cases` family legs; the operand spelling moved to product-scope paths (PSI-HOSTED-ENTRY-RECEIVER-PROVISIONING) |
| unresolved checked occurrences after check | ~3 | "authored Call/Operator declaration selection occurrence N remained unresolved after successful checking" — `public_api::data_invariants`, `terminal_permission_policy::uefi`, `exact_contract_identity::compiler_and_literal_identity` (target-symbol tamper no longer rejected) |
| package-manager command orchestration drift | ~24 | `named_workspace_install::cases` (9), `source_diff_commands::cases` (7), `offline_package_commands::cases` (6), `package_inspection::cases` (5), `package_reconstruction_question::*` (4), `package_lineage_spoofing` (1), `opaque_boundary_agreement` (2), `token_binding_revision` (1, "[Removed]" claim-row expectation predates token-bound migration), `capability_conflicts::transaction` (1, exact-arithmetic obligation `add_u64` may overflow), `repository_build_declarations` (7, canary std-edge declaration drift after corpus respells) |

## Compiler-target failures (22)

| suite | legs | signature |
|-------|------|-----------|
| `build_target_activation::x86_feature_admission` | 6 | FMA/x86 feature-admission legs (`admitted_x86_fma_demand_retains_exact_plan_associations`, `exact_x86_fma_demand_fails_closed_without_feature_admission`, `aarch64_fma_demand_is_not_an_x86_feature_association`, `boundary_operator_and_float_adapters_retain_terminal_execution`, `source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`, `terminal_product_retains_exact_fma_operation_plan_and_x86_admission`) — provider-plan/x86 admission residuals |
| `package_compilation_inputs::authority_and_build_files` | 8 | `build_time_call_closure_rejects_internal_undeclared_package_selection`, `const_generic_evaluation_requires_direct_authority_before_execution`, `dependency_operator_family_selection_covers_every_overload_atomically`, `dependency_provider_plan_retains_exact_dependency_package_provenance`, `one_root_source_cannot_join_both_dependency_scopes`, `provider_selection_rejects_an_arbitrary_composition_expression`, `provider_selection_rejects_an_authored_composition_mode_lookalike`, `native_package_entrypoint_uses_the_same_reconciled_binding_mode` |
| `package_compilation_inputs::artifact_identities_and_entries` | 4 | `accepted_package_console_binding_closes_linux_intrinsic_without_toolchain_origin`, `accepted_package_uefi_binding_selects_exact_ordinary_schema`, `free_process_exit_helper_lowers_without_a_synthetic_attachment`, `package_native_physical_evidence_gate_borrows_exact_supported_evidence` |
| `build_config_granted::checkpoints_and_snapshots` | 1 | `admitted_build_checkpoint_retains_configuration_and_execution_evidence` — source-custody roster drift (expected 10 custody rows, found 4) |
| `build_config_granted::generated_sources_and_authority` | 2 | `generated_local_instance_collection_preserves_build_symbol_and_source_custody`, `generated_target_machines_join_selected_origin_and_provider_default_custody` |
| `package_compilation_inputs::generated_sources_and_dependencies` | 1 | `generated_dependency_handoff_keeps_build_and_product_occurrences_distinct` |

`build_log_facet`, `checked_build_machine_identity`, and
`evaluated_via_binding` are fully green.

## Remaining owners

All red legs are the documented wave-9 fixture-migration residual families
(`Service<R>` closed carrier spelling, `select_provider` product-scope
operand paths, vendored-std collision, checked-occurrence settlement,
canary declaration drift) — the same families RC-NATIVE-MATRIX-LINUX-X86-64
recorded as owned by ENTRY-CONTENT-ROOTS and the provider-settlement work
(PRIVILEGED-PORT-EFFECT-SETTLEMENTS lane). The gate stays red until those
fixture respells finish; re-run the row when they close.
