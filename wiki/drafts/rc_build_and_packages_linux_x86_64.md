# RC-BUILD-AND-PACKAGES — linux x86-64

Record of the `RC-BUILD-AND-PACKAGES` release gate block from
[rust_compiler_completion.md](rust_compiler_completion.md#release-matrix) run on
linux x86-64 (this session's VM, 8 CPUs) at commit `c267df86acb`
(origin/main, 2026-09-20). Commands were the gate block verbatim, with `cargo`
substituted for `mbx` per AGENTS.md. **Gate does not pass: 2 of 3 commands have
failures at this head.**

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `cargo nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast` | 100 | 1,670 tests in 5,463.8s: 1,577 passed (184 slow), **93 failed**, 2 skipped |
| 2 | `cargo test --doc` (same seven crates) | 0 | green — 1 doctest (`resolver_execution::ResolverExecutionBackend::prepare`, compile-fail) |
| 3 | `cargo nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast` | 124/100 | 354 tests: 337 passed, **13 failed**, 4 killed by a 900s measurement timeout — all 4 rerun standalone and pass (105–110s each), so the true tally is **341 passed, 13 failed** |

The prior record (`release_record_e12b9e8e06.md`) cut off mid-run at 355/1,662
all-PASS; this is the first complete measurement of the block. The wall time is
dominated by a handful of package-manager suite legs
(`semantic_binding_review::macos_entry::target_entry_dependency_discovery_requires_explicit_consumer_acceptance`
took 3,574s alone) plus package-evidence suites averaging 20–90s per test.

## Command 1 failures (93)

### package-manager::suite child-harness `::cases` fixtures — 27

All panic at `named_workspace_install/fixture.rs:91` with `child must execute
exactly ...: running 0 tests` — the install/diff/inspection child-process
fixture machinery no longer produces the expected child invocations.

- `named_workspace_install::cases` (9): `duplicate_declared_names_reject_without_publishing`, `named_member_alias_override_does_not_rename_the_selected_package`, `named_member_uses_declared_default_alias_and_member_relative_dependencies`, `named_review_resume_retains_selection_alias_and_exact_revision`, `named_selection_rejects_local_sources_and_member_paths`, `omitted_selection_does_not_guess_a_workspace_member`, `omitted_selection_keeps_root_package_behavior`, +2
- `source_diff_commands::cases` (7): `named_and_relative_members_use_the_accepted_repository_pin_with_cold_storage`, `offline_update_resume_*`, `unavailable_old_git_source_*`, `update_renders_exact_old_git_root_*`, `update_selects_a_build_scope_alias_*`, `update_shared_cross_scope_alias_*`, `update_to_retargets_both_scope_rows_*`
- `offline_package_commands::cases` (6), `package_inspection::cases` (5)

### package-manager::suite repository_build_declarations — 12

Canary standard-library-edge declaration drift:
`arithmetic_canaries_*`, `call_canaries_*`, `capability_and_control_flow_*`,
`executable_samples_declare_canonical_roles_*`, `expression_and_storage_*`,
`filesystem_canaries_*`, `foundational_runtime_canaries_*`,
`operator_and_type_runtime_*`, `ordinary_omega_case_projects_*`,
`small_mixed_runtime_categories_*`, `trait_canaries_*`, `wire_canaries_*`.

### package-manager lib operations + review::candidate::compilation — 6

`operations::check_project::tests::semantic::retained_check_root_uses_final_consumer_bindings_and_requested_entry`;
`operations::compile_project::tests::receiving_admission::accepted_console_customer_receives_admission_only_under_a_sufficient_policy`;
`review::candidate::compilation::tests::discovery_proposes_{the_root_console_exit_permission_as_a_blocking_row,the_root_console_output_and_input_permissions_per_declared_leaf,the_root_filesystem_cohort_permissions_per_declared_leaf}`;
`review::candidate::compilation::tests::review_publishes_the_named_component_description_for_an_independent_selection`.
Representative diagnostic: ``data `Board` field `clock` uses `in Bound`, but no
`domain` named `Bound` is declared for `Service<Console>` (domains are bound to
their storage type)`` — the closed `Service` carrier / authored-domain gate.

### package-manager::suite — other — 12

`capability_conflicts::transaction::exact_compiler_rows_become_candidate_bound_review_conflicts`
(exact u64-overflow obligation in `add_u64`);
`package_evidence_fixtures::compiler_review_evidence::{local_fixtures_issue_compiler_review_evidence_from_resolver_custody,process_exit_fixture_retains_exact_closed_console_leaves_and_unresolved_siblings}`;
`package_lineage_spoofing::same_name_and_symbols_from_another_lineage_cannot_spoof_selected_provider`;
`opaque_boundary_agreement::{independently_compiled_opaque_boundary_agreement_rejoins_foreign_demand,independently_compiled_opaque_boundary_rejects_a_different_consumer_conformance}`;
`package_policy_changes::operation::transitive::transitive_helper_authority_changes_policy_with_the_same_public_ceiling`;
`package_reconstruction_question` (4: `association_paths_and_questions::canonical_question_round_trips_*`, `dependency_claims_and_recovery` ×3);
`token_binding_revision::a_changed_token_binding_is_a_breaking_revision_of_the_reviewed_callables`.
Observed theme: provider-selection operand drift ("provider selection operand
does not resolve to one visible product declaration") and service-reach
`<none>` vs undeclared `RootDir`.

### package-evidence — 36

Lib: `capture::quotients::tests::total_direct_define_projects_one_deterministic_recoverable_review_row`.

Suite, dominated by the routed `Service<R>` carrier gate (`the core Service
carrier is closed; it admits no authored qualification`, `no domain named
Bound`, `outside the first direct field/owned concrete-machine-parameter
rung`), `0 exact checked call-selection rows` fixture drift, and
`boundary_calling_plan_realizations().is_empty()` assertion drift:

- `callable_policy::{flows,mutation}` (4)
- `calling_policy_source` (2), `calling_policy_substitution` (4, `Option::unwrap` on `None` at `calling_policy_substitution.rs:17`)
- `contract_expressions::evidence_calls` (8), `contract_expressions::collection_views` (2)
- `exact_contract_identity::compiler_and_literal_identity::builtin_function_review_rejects_checked_target_symbol_tamper`
- `operational::authored_sources` (`trait Child requires unknown trait Service`), `obligation_ledger` (1)
- `public_api::{data_invariants ×3, module_constant_domain_index_*}`
- `representation_policy::unused_placement_and_semantic_copy_selections_*`
- `selected_provider_policy::{families ×1, signatures ×2}`, `source_custody` (1)
- `terminal_permission_policy::{accepted_console_dependency_*,uefi::*}` (2)
- `trait_contracts::{calls_and_entailment,operational_envelopes}` (2)

## Command 3 failures (13)

- `build_config_granted::checkpoints_and_snapshots::admitted_build_checkpoint_retains_configuration_and_execution_evidence` — source-custody row count 11 vs expected 4.
- `build_config_granted::generated_sources_and_authority::{generated_local_instance_collection_preserves_build_symbol_and_source_custody,generated_target_machines_join_selected_origin_and_provider_default_custody}` — generated-source custody drift.
- `build_log_facet::package_authored_build_log_lookalike_cannot_receive_the_compiler_facet`.
- `build_target_activation::x86_feature_admission` (6): `boundary_operator_and_float_adapters_retain_terminal_execution`, `source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`, `terminal_product_retains_exact_fma_operation_plan_and_x86_admission`, `aarch64_fma_demand_is_not_an_x86_feature_association`, `admitted_x86_fma_demand_retains_exact_plan_associations`, `exact_x86_fma_demand_fails_closed_without_feature_admission`. Observed diagnostic: the bundled `windows_x86_64` entry (`source/library/std/targets/windows_x86_64/entry.omg`) is rejected as not the exact bundled contract — FMA feature-admission leg drift, target-specific but not host-gated (it is diagnostic-level).
- `package_compilation_inputs::artifact_identities_and_entries::{accepted_package_uefi_binding_selects_exact_ordinary_schema,free_process_exit_helper_lowers_without_a_synthetic_attachment}`.
- `package_compilation_inputs::module_template_methods::instantiated_methods_keep_each_package_use_authority` (`duplicate data Envelope`).

## Claims over the failing surfaces at measurement time

`BUILD-PACKAGES-GATE` + `/manager-fixture-migration` + `/evidence-fixture-migration`
(Zergling-129, 21:49Z/23:25Z), `SNAPSHOT-STORAGE` (22:49Z),
`BUILD-DIRECTORY-ALIAS-COLLISION` (23:07Z), `PACKAGE-INPUTS-PSI-FAILURES`
(01:00Z), `TARGET-INFERENCE-AND-PLATFORM-CERTIFICATION` (01:10Z),
`TWO-AXIS-TERMINAL-AUTHORITY-REVIEW` (04:18Z),
`OPTIONAL-STDLIB-SEMANTIC-BINDINGS` (04:23Z), `BUILD-SEMANTIC-EXCLUSIONS`
(04:45Z), `BENCHMARK-PROOF-SUBJECT-CHECKED-CALL-SELECTION` (04:19Z). The bulk of
the red is fixture-migration drift already fenced to the
`BUILD-PACKAGES-GATE` lanes; the `x86_feature_admission` cluster (6) sits on
the target-certification surface.

Also observed: package-evidence suite emits 9 `crate::support` unused-import
warnings; package-manager suite 1 (`capability_conflicts`).
