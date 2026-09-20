# RC-BUILD-AND-PACKAGES — linux_x86_64 gate record at `f1675418b1`

Release-matrix gate `RC-BUILD-AND-PACKAGES` per
`wiki/drafts/rust_compiler_completion.md` (board stub RC-BUILD-AND-PACKAGES).
**Status: RED at this base.**

- Host: linux x86-64, runner `cargo` (mbx unavailable)
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`

## Commands and verdicts

```text
cargo nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast
```
**FAIL** — 109 distinct test failures out of 1664 executed (exit 100).

```text
cargo test --doc -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager
```
**PASS** — all doc tests green.

```text
cargo nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast
```
**FAIL** — 11 distinct test failures out of 353 executed (exit 100).

## Failure inventory

### Compiler test targets (11)

- `build_config_granted` checkpoints_and_snapshots::
  `admitted_build_checkpoint_retains_configuration_and_execution_evidence`
- `build_config_granted` generated_sources_and_authority::
  `generated_local_instance_collection_preserves_build_symbol_and_source_custody`,
  `generated_target_machines_join_selected_origin_and_provider_default_custody`
- `build_target_activation` x86_feature_admission::
  `boundary_operator_and_float_adapters_retain_terminal_execution`,
  `source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`,
  `terminal_product_retains_exact_fma_operation_plan_and_x86_admission`,
  `admitted_x86_fma_demand_retains_exact_plan_associations`,
  `aarch64_fma_demand_is_not_an_x86_feature_association`,
  `exact_x86_fma_demand_fails_closed_without_feature_admission`
- `package_compilation_inputs` artifact_identities_and_entries::
  `accepted_package_uefi_binding_selects_exact_ordinary_schema`,
  `free_process_exit_helper_lowers_without_a_synthetic_attachment`

### Package suites (109 — dominated by `package-evidence::suite`)

Clusters observed:

- **package-evidence::suite** (~80 failures): `calling_policy_source`,
  `calling_policy_lifetimes`, `calling_policy_substitution`,
  `calling_policy_opaque`, `contract_expressions::evidence_calls`,
  `selected_provider_policy::{inherited,families,signatures}`,
  `terminal_permission_policy::{calling,uefi}`,
  `representation_policy::{foreign_demand,used_selection,...}`,
  `authority::toolchain_provenance`, `callable_policy::{flows,mutation}`,
  `exact_contract_identity`, `public_api::{data_invariants,module_constants}`,
  `trait_contracts::operational_envelopes`, `source_custody`,
  `operational::authored_sources`
- **package-manager::suite** (~25): `named_workspace_install::cases` (9 —
  named-member alias/selection family), `offline_package_commands::cases` (6 —
  offline install/update/resume family), `package_inspection::cases` (4),
  `opaque_boundary_agreement`, `package_evidence_fixtures::
  compiler_review_evidence`, `capability_conflicts::transaction`,
  `operations::check_project::tests::semantic`,
  `operations::compile_project::tests::receiving_admission`,
  `review::candidate::compilation::tests` (3)
- **package-manager** `operations::check_project::tests::semantic::
  retained_check_root_uses_final_consumer_bindings_and_requested_entry`

### Pattern

The package-evidence failures cluster on *policy digest/identity drift* —
evidence-review rows that pin exact digests of calling policies, selected
provider policies, permission policies, and toolchain provenance are
rejecting against current encodings. The compiler-side failures cluster on
*x86_feature_admission* (FMA admission rows — the in-flight
FMA-PROVIDER-TRANSPORT surface) plus `artifact_identities_and_entries` entry
selection drift. A review of whether these are digest respells or real
semantic drift belongs to the RC-DIAGNOSTICS/feature-owner lanes.

## Gate verdict

`RC-BUILD-AND-PACKAGES` does not pass on linux x86-64 at `f1675418b1`:
120 distinct failing tests across the three command blocks. The gate stays
**open**; the release record cannot close this row. Other required hosts
(linux_arm64, macos_arm64, windows_x86_64) are structurally unavailable to
this worker.
