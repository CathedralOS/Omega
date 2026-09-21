# RC-BUILD-AND-PACKAGES — linux_x86_64 gate record at `ea698be648`

Release-matrix gate `RC-BUILD-AND-PACKAGES` per
`wiki/drafts/rust_compiler_completion.md` (board stub RC-BUILD-AND-PACKAGES).
**Status: RED at this base.** Re-measured 2026-09-21; supersedes the
`f1675418b1` record (which read 109/1664 + 11/353).

- Host: linux x86-64, runner `cargo` (mbx unavailable)
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`

## Commands and verdicts

```text
cargo nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast
```
**FAIL** — 107 distinct test failures out of 1684 executed, 2 skipped
(4834.4s, exit 100).

```text
cargo test --doc -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager
```
**PASS** — all doc tests green.

```text
cargo nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast
```
**FAIL** — 12 distinct test failures out of 355 executed (843.4s, exit 100).

## Failure inventory

### Compiler test targets (12)

- `build_config_granted` checkpoints_and_snapshots::
  `admitted_build_checkpoint_retains_configuration_and_execution_evidence`;
  generated_sources_and_authority::
  `generated_local_instance_collection_preserves_build_symbol_and_source_custody`,
  `generated_target_machines_join_selected_origin_and_provider_default_custody`
- `build_log_facet` `package_authored_build_log_lookalike_cannot_receive_the_compiler_facet`
  (new since `f1675418b1`)
- `build_target_activation` x86_feature_admission (6, unchanged family):
  `boundary_operator_and_float_adapters_retain_terminal_execution`,
  `source_fma_then_attached_unit_call_stays_inside_one_canonical_mxcsr_envelope`,
  `terminal_product_retains_exact_fma_operation_plan_and_x86_admission`,
  `admitted_x86_fma_demand_retains_exact_plan_associations`,
  `aarch64_fma_demand_is_not_an_x86_feature_association`,
  `exact_x86_fma_demand_fails_closed_without_feature_admission`
- `package_compilation_inputs` artifact_identities_and_entries (2, unchanged):
  `accepted_package_uefi_binding_selects_exact_ordinary_schema`,
  `free_process_exit_helper_lowers_without_a_synthetic_attachment`

### Package suites (107)

Clusters observed, by dominant signature:

- **`repository_build_declarations` (12)** — expectation-vs-corpus drift:
  hard-coded packaged-canary populations disagree with the landed
  `tests/omega/pass` tree (operators expected 10 found 12; domains 28 vs 29;
  traits 32 vs 33; wire 44 vs 46; plus arithmetic/calls/filesystem/
  foundational/expression-storage/capability-control-flow/mixed categories
  and `executable_samples`), and `ordinary_omega_case_projects_…` expects an
  application role the landed fixture names differently
  (`theorem-equality-certificates-exit` vs `kernel-theorem-equality-certificates`).
- **Child-execution fixture drift (27)** — `named_workspace_install::cases` (9),
  `source_diff_commands::cases` (7), `offline_package_commands::cases` (6),
  `package_inspection::cases` (5): each fails "child must execute exactly
  `cases::<name>`: running 0 tests" — the spawned fixture filter no longer
  selects the intended case.
- **`standard_library_package_resolution` (3)** —
  `real_standard_library_has_a_complete_ordinary_review_entry`,
  `real_filesystem_host_schema_accepts_settled_portable_facet_rows`,
  `real_filesystem_host_explicit_empty_policy_retains_reach_and_review_identity`:
  the real `FilesystemHost` provider plan's `close` realization row now has
  0 exact typed machines (service-schema drift).
- **`contract_expressions::evidence_calls` (8)** — every fixture's public
  contract call now rejects "cannot prove requires contract for
  specification call `observes` … establish its selected precondition in an
  independently formed prior requires fact" — the evidence-call leg lost its
  precondition establishment.
- **`contract_expressions::collection_views` (2)** — "contract-call intrinsic
  identity disagrees with its exact checked call-selection row or is not yet
  represented by package review" (collection-view intrinsic projection).
- **`terminal_permission_policy` (9)** — service-schema permission drift
  (console dependency retainment, calling/generics/uefi legs,
  sibling-schema staleness).
- **`calling_policy_substitution` (4) + `calling_policy_source` (2)** —
  inherited conformance arity drift ("conformance `Provider satisfies
  HookProcedure` expects 0 generic argument(s), got 1") and emitted-source
  text drift (`ProcedureBase<Value>` no longer in emitted boundary source).
- **`callable_policy::{flows,mutation}` (4)** — capability-flow/callable
  mutation rows rejecting.
- **`selected_provider_policy::{families,signatures}` (3)** —
  `boundary_calling_plan_realizations()` no longer empty on unused plans;
  boundary-operator signature drift.
- **`public_api` (3)** — `data_invariants_keep_generic_binders_distinct…`
  ("authored Operator declaration selection occurrence 0 remained
  unresolved after successful checking"),
  `module_constant_domain_index_enters_canonical_public_data_artifact`
  (indexed-family count), `data_invariant_review_rejects_checked_ownership_spoofs`.
- **Singletons** — `capture::quotients` (total_direct_define review row),
  `boundary_supply::boundary_bodies` (`FilesystemHost` schema lacks `close`),
  `representation_policy`, `exact_contract_identity::
  builtin_function_review_rejects_checked_target_symbol_tamper`,
  `obligation_ledger` (2), `source_custody` (2),
  `operational::authored_sources`, `trait_contracts` (2),
  `review::candidate::compilation` (4 — `Service` carrier closed /
  provider-selection operand resolution),
  `package_evidence_fixtures::compiler_review_evidence` (2 —
  provider-selection operand resolution),
  `package_reconstruction_question` (4), `package_lineage_spoofing`,
  `opaque_boundary_agreement` (2), `capability_conflicts::transaction`
  (exact-arithmetic proof obligation in `add_u64`),
  `token_binding_revision`, `semantic_binding_review`, and
  `operations::{check_project,compile_project}`.

### Pattern

Three failure families dominate. (1) *Fixture-population/child-execution
drift* in `package-manager` — hard-coded rosters (`repository_build_
declarations`) and spawned-case selectors (`*_commands::cases`,
`package_inspection`) lag the landed corpus; ~39 failures, mechanical.
(2) *Service/provider realization drift* — `FilesystemHost::close` rows and
`Service`-carrier/provider-selection operands across manager and evidence
suites; ~20 failures, feature-owned. (3) *Package-review evidence rows
rejecting on checked-side semantic changes* — evidence-call precondition
establishment, intrinsic-identity projection, permission/calling-policy
substitution drift; ~45 failures, feature-owned. The compiler block is
stable at the `f1675418b1` families plus one new `build_log_facet`
reject.

## Gate verdict

`RC-BUILD-AND-PACKAGES` does not pass on linux x86-64 at `ea698be648`:
119 distinct failing tests across the three command blocks (107 + 12).
The gate stays **open**. Other required hosts (linux_arm64, macos_arm64,
windows_x86_64) are structurally unavailable to this worker.
