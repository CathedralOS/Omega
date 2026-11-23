# Crate map: 09-native-stages-00-03

All paths are relative to `C:/Development/Omega/.agents/worktree/worker-2/`. The native pipeline root is
`P = source-rust/subsystems/compiler/psi-to-native/pipeline`. Line counts come from `crates.tsv` (area column).
Dependency lists come from reading each `Cargo.toml`; "depended on by" comes from a grep for the crate name
and `package = "<name>"` renames across every `Cargo.toml` under `source-rust`, `omega-rust-v2` and `tools`.
Test-function counts come from `grep -rE '#\[test\]'` over each crate directory.

## 1. What the area does

This is the front of the native half of the compiler, the part that runs before any target-specific work.
Stage 00 takes the Psi program the receiving host has authorized, plus a "provider demand" (which logical
service choices the program needs) and the host's provider installation, and checks that all of them name the
same artifact. Stage 01 binds that admitted program to one target: it settles which providers and calling
plans supply each reachable boundary, and walks the program from its entry to find what is reachable. Stage 02
lowers the reachable, checked Psi bodies to `AbstractOperationPlan` (target-neutral abstract operations) and
checks the providers and the entry before any object is built. Stage 03 runs the optional, target-neutral
optimization passes over that plan and publishes the result for stage 04. What comes in is a
`CanonicalTerminalArtifact` with its authorization, a provider demand and installation, and a target; what goes
out is a `ValidatedOptimizedAbstractPlan` (`03.../publication.rs:233`). The native driver
(`source-rust/subsystems/compiler/psi-to-native/driver/`) calls the stages; the stages do not call each other
except stage 02 calling stage 01's output types, and stage 01 calling stage 00's.

## 2. Crates, in pipeline order

### 2.1 psi-to-admitted-native-inputs (stage 00 root)

- Path: `P/00_psi-to-admitted-native-inputs`, 1 file, 98 lines (`admit.rs`).
- Purpose: binds a provider demand to the exact artifact the host authorized. It refuses if the authorized
  artifact identity differs from the one the demand names (read, `admit.rs:15-27`).
- Consumes: `AuthorizedTerminalArtifact` (`psi-authorization`) and `IssuedProviderDemand<Publisher>`
  (`provider-demand/demand.rs:44`). Produces: `AdmittedProviderDemand<Publisher>` (`admit.rs:9`).
- Entry points: `admit` (`admit.rs:14`), `AdmittedProviderDemand::inputs` (`:34`) and `into_parts` (`:38`).
- Production dependencies in this repo: `provider-demand`, `psi-authorization`. Dev: `terminal-codec`, `terminal-psi`.
- Depended on by (2): `psi-to-native`, stage 01 (as `native-inputs`).
- Modules: `admit.rs` only (the sub-crates below sit in subdirectories but are separate packages).
- Tests: 4 test functions in 3 files across the stage 00 directory tree; the one in `admit.rs:46` checks that
  an artifact with a changed debug map is refused as a mismatch.
- Open work: `TASKS.md` item **PROVIDER-SELECTION-AFTER-TERMINAL** links to this directory as the "native
  admission" end of the handoff from source provider publication. `OWNER_CODE_CONCERNS.md`: none.

### 2.2 host-provider-inputs (stage 00)

- Path: `P/00_psi-to-admitted-native-inputs/host-providers`, 1 file, 210 lines (`installation.rs`).
- Purpose: holds what the receiving host supplies for one program on one target: Build binding results,
  package permissions and accepted semantic bindings. A host installs reviewed packages once
  (`InstalledPackageProviders`) and attaches them to each authorized program.
- Consumes: host-supplied bindings (`evaluation_receipts::NativeBindingMaterialization`), `ProductDeclarationScope`.
  Produces: `ProviderExecutionOrigins` (`installation.rs:17`), `ProviderInstallationInputs` (`:25`),
  `InstalledPackageProviders` (`:34`), `TargetProviderInstallation` (`:75`).
- Entry points: `InstalledPackageProviders::new` (`:42`) and `for_program` (`:56`); `TargetProviderInstallation::new`
  (`:82`), `attach` (`:94`), `from_inputs` (`:113`), `into_inputs` (`:138`).
- Production dependencies: `provider-acceptance`, `declaration-scope`, `evaluation-receipts`, `psi-authorization`,
  `semantic-vocabulary`, `target`, `terminal-codec`, `terminal-psi`.
- Depended on by (4 production): `psi-to-native`, `provider-demand`, stage 01, `package-manager`
  (`source-rust/subsystems/package-manager/Cargo.toml:19`). `compiler/build` lists it only under dev-dependencies.
- Modules: `installation.rs` only. Tests: 1 test function.
- Open work: none by name in `TASKS.md` or `OWNER_CODE_CONCERNS.md`.

### 2.3 provider-demand (stage 00)

- Path: `P/00_psi-to-admitted-native-inputs/provider-demand`, 1 file, 176 lines (`demand.rs`).
- Purpose: the closed set of logical provider choices a publisher issues for one portable program. A demand is
  either `Unscoped` (artifact plus root machines, no boundary supply) or `Target` (profile, closed logical
  providers, bound bodies, labels, host origins, optional package-review requirements).
- Consumes: the source compiler's `SourceProviderDemand` (re-exported from `source-provider-publication`) or a saved
  `.psi` program plus a `TargetProviderInstallation`. Produces: `IssuedProviderDemand<Publisher>` (`demand.rs:44`)
  holding `ProviderDemandInputs` (`:17`).
- Entry points: `IssuedProviderDemand::issue` (`:50`), `from_publication` (`:58`, the source-compile path),
  `from_saved_program` (`:132`, the saved-`.psi` path), `inputs` (`:116`), `into_parts` (`:120`).
- Production dependencies: `psi-provider-selections`, `diagnostics`, `declaration-scope`,
  `source-provider-publication` (source-to-psi representations), `host-provider-inputs`, `semantic-vocabulary`,
  `target`, `terminal-codec` (and more `terminal-*` crates; read the head of `Cargo.toml`, truncated at `terminal-codec`).
- Depended on by (3): `psi-to-native`, `psi-to-admitted-native-inputs`, stage 01.
- Tests: 0 test functions. Open work: as 2.1 (**PROVIDER-SELECTION-AFTER-TERMINAL** names the directory).

### 2.4 native-scalar-inputs (stage 00)

- Path: `P/00_psi-to-admitted-native-inputs/scalar-applications`, 3 files, 399 lines
  (`scalar_applications.rs`, `completed.rs`, `completed/tests.rs`).
- Purpose: completes the scalar (compile-time numeric) application values an artifact needs for a deployment and its
  selected provider proposals. `prepare` keeps the evaluated scope with its target adapters so the native entry can
  later add its own demanded scalar roots with `extend`. Build and the interpreter use the same scalar checks in
  `source-rust/subsystems/psi/execution-inputs/scalar-applications` (path read from `psi-to-native/README.md`).
- Consumes: `CanonicalTerminalArtifact`, `ScalarApplicationId` demand, `TerminalTargetProviderProposal`s.
  Produces: `psi_scalar_applications::AdmittedScalarApplications` (via `complete`, `scalar_applications.rs:14`) or
  `ProductScalarCompletion` (`completed.rs:8`, via `prepare`, `scalar_applications.rs:28`).
- Entry points: `complete` (`:14`), `prepare` (`:28`), `ProductScalarCompletion::extend` (`completed.rs:22`).
- Production dependencies: `numeric-intrinsic-admission`, `target-provider-admission`, `selected-provider-plans`,
  `build-scalar-evaluation`, `psi-scalar-applications`, `psi-provider-selections`, `psi-target-layouts`, `runtime-abi`,
  `semantic-vocabulary`, `target`, `terminal-codec`, `terminal-interpreter`, `terminal-psi`, `interpreter-numeric-imports`.
- Depended on by (2): `psi-to-native` (production), stage 01 (dev only).
- Tests: 2 test functions in `completed/tests.rs`. Open work: none by name.

### 2.5 admitted-native-inputs-to-target-bound-program (stage 01)

- Path: `P/01_admitted-native-inputs-to-target-bound-program`, 37 files, 9029 lines (counts include the
  `execution-scope` sub-crate of 800 lines; see 2.6). Library name `admitted-native-inputs-to-target-bound-program`,
  imported elsewhere as `provider_realization` (renamed in manifests).
- Purpose: binds the admitted program to its target. It turns the demand, installation and Build rows into a
  `SelectedProviderRealization` (which provider body, calling plan and external binding serves each boundary),
  settles the program entry and its calling plans, derives which machines/callbacks are reachable, and matches each
  reachable requirement to a provider, refusing competing external bindings.
- Consumes: `AdmittedProviderDemand` (stage 00) and component descriptions (`VerifiedComponent`), plus
  `TerminalNativeRealizationProposal` and the `AuthorizedTerminalArtifact` for native execution. Produces:
  `PublishedSelection<Publisher, SelectedExecution>` (`provider_selection.rs:9`, `selected_execution.rs:10`),
  `PreparedNativeExecution` (`native_execution.rs:14`), `BoundNativeCallable` (`:103`) and, for the program entry,
  `(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy)` from `bind_published_application`.
- Entry points: `select_published_providers` (`provider_selection.rs:43`); `bind_published_application`
  (`published_application.rs:311`) and `bind_component_export` (`:174`); `native_execution::prepare_program`
  (`native_execution.rs:93`) and `BoundNativeCallable::prepare` (`:113`); `plan_target_execution`
  (`execution.rs:77`) and `plan_unscoped_execution` (`:21`); `realize_target_supply`
  (`target_supply/plan_realization.rs:522`); `plan_provider_execution_inputs` (`provider_execution_inputs.rs:61`);
  `complete_calling_scalar_inputs` (`calling_policy_plans.rs:69`).
- Production dependencies: 52 crates in the repository (counted from the `[dependencies]` table). The ones that name the
  stage's neighbours are `psi-to-admitted-native-inputs`, `provider-demand`, `host-provider-inputs`,
  `native-execution-scope`, `target-bound-program`, `verified-optimization-input`, `provider-admission`,
  `provider-correspondence`, `target-provider-admission`, `calling-conventions`, `layout`, `runtime-abi`,
  `terminal-interpreter`, `terminal-semantics`, `terminal-work`, `wire`. Dev: `native-scalar-inputs`.
- Depended on by (3): `psi-to-native`, stage 02 (as `provider-realization`), `compiler/build` (dev only).
- Modules (under `src/`): `execution.rs` (plan a target or unscoped execution); `selected_execution.rs` (the
  `SelectedExecution` record); `provider_selection.rs` (select providers from a published demand);
  `published_application.rs` and `published_application/program_entry.rs` (bind the entry, permissions and plans);
  `program_entry.rs`, `program_entry/` (`calling_plans.rs`, `published.rs`, `visible_parameters.rs`; entry settlement);
  `native_execution.rs` and `native_execution/provider_bindings.rs` (scope plus provider bindings for lowering);
  `target_supply.rs` and `target_supply/` (`plan_realization.rs` 652 lines, `realized_supply.rs` 614, `installation_reach.rs`);
  `provider_execution_inputs.rs` and `provider_rows.rs` (provider rows from Build receipts); `package_review.rs`
  (reviewed native bindings); `calling_policy_plans.rs` and `calling_policy_plans/` (calling policy decoding,
  boundary and callback layouts, `policy_signature.rs`, `build_time_decoding.rs`); `native_bindings/plans.rs`;
  `folded_entry.rs`. A `test-support` feature exposes `test_support::select_realization` (`lib.rs:40-52`).
- Tests: 23 test functions in 11 files, including `tests/source_provider_choices.rs` and `calling_policy_plans/values/tests.rs`,
  `native_bindings/plans_tests.rs`. I did not read them.
- Open work: **CALLING-PLAN item in TASKS.md** under **STATIC-MACHINE-MIGRATION**: "Remove the required foreign ABI tag from
  CallPlan and its decoder" links `calling_policy_plans/build_time_decoding.rs`; and "Evaluate a native requirement's
  declared calling policy" (same group). **PROVIDER-SELECTION-AFTER-TERMINAL** covers the source-to-native handoff.
  **BUILD-OWNED-NATIVE-BINDINGS** and **BUILD-OWNED-TARGET-SELECTION** concern what this stage consumes. `OWNER_CODE_CONCERNS.md`: none.

### 2.6 native-execution-scope (stage 01 sub-crate)

- Path: `P/01_admitted-native-inputs-to-target-bound-program/execution-scope`, 2 files, 800 lines (`scope.rs` 396, `tests.rs` 404).
- Purpose: follows the machines, callbacks and provider boundaries reachable from the selected entry in the checked
  program and returns the scope that bounds later lowering.
- Consumes: `NativeScopeBinding` (from `target-bound-program`). Produces: `NativeExecutionScope`
  (`representations/target-bound-program/native_execution_scope.rs:226`) or `NativeExecutionScopeError` (`scope.rs:50`).
- Entry point: `derive_native_scope` (`scope.rs:12`).
- Production dependencies: `target-bound-program`, `native-execution-reach`, `diagnostics`, `provider-correspondence`,
  `semantic-vocabulary`, `terminal-codec`, `terminal-psi`.
- Depended on by (1): stage 01. Tests: 5 test functions in `tests.rs`. Open work: none by name.

### 2.7 target-bound-program-to-abstract-operations (stage 02)

- Path: `P/02_target-bound-program-to-abstract-operations`, 9 files, 6210 lines; the largest are
  `operations.rs` (1950), `operations/storage.rs` (1861), `selected_input.rs` (1075), `operations/scalars.rs` (655).
  Imported elsewhere as `artifact_lowering`.
- Purpose: lowers the selected, authorized Psi bodies (entry or one component callable) to `AbstractOperationPlan`,
  checks the providers the lowered body demands (callbacks, imports, builtins, package permissions) and compares the
  lowered entry with its Psi declaration. Lowering walks each reachable machine's blocks in order and dispatches
  each operation and terminator; the stage `README.md` says scalar, Unit and structural results share one walk.
- Consumes: `TerminalNativeRealizationProposal` plus `RetainedNativeProviderInputs` (`admit_providers.rs:16`) built from
  stage 01's `PreparedNativeExecution`/`BoundNativeCallable`. Produces: `VerifiedNativeArtifactInput`
  (`selected_input.rs:186`), `PreparedRetainedNativeProviders` (`admit_providers.rs:23`),
  `AdmittedNativeRealization` (`:154`) and the entry settlement. `VerifiedNativeArtifactInput::into_optimization_input`
  (`selected_input.rs:606`) yields `VerifiedPsiOptimizationInput` (`representations/abstract-operations/verified-optimization-input/src/lib.rs:64`),
  which is stage 03's input.
- Entry points (`lower.rs:1-28` re-exports): `lower_native_root` (`admit_providers.rs:83`), `lower_native_callable`
  (`selected_execution.rs:6`), `admit_prepared_native_realization` (`admit_providers.rs:160`), `admit_executable_entry`
  (`entry_receiver.rs:20`).
- Production dependencies: 23 crates. Stage neighbours are `provider-realization` (stage 01), `target-bound-program`,
  `abstract-operations`, `verified-optimization-input`, `operation-evidence`, `boundary-applications`,
  `provider-admission`, `provider-correspondence`, `target-provider-admission`, `installation-conditions`,
  `native-behavior-exclusions`, plus `psi-scalar-applications`, `psi-entry-memory`, `psi-target-layouts`,
  `terminal-psi`, `terminal-semantics`, `terminal-codec`, `psi-authorization`.
- Depended on by (3): `psi-to-native`, stage 03 (dev, with `test-support`), `compiler/build` (dev, with `test-support`).
- Modules: `lower.rs` (crate root, re-exports only); `selected_input.rs` (artifact preparation, access-view
  establishment, native scalar applications, `VerifiedNativeArtifactInput`); `operations.rs` with `operations/scalars.rs`
  and `operations/storage.rs` (module/machine/operation/terminator lowering); `admit_providers.rs` (provider admission);
  `selected_execution.rs` (lower root or callable); `entry_receiver.rs` (entry admission); `test_support.rs`.
  A `test-support` feature exposes whole-module lowering for fixtures (`lower.rs:26-27`).
- Tests: 3 test functions, all in `selected_input.rs`; more coverage sits in stage 03's dev-dependency use of
  `artifact-lowering` and in the driver (inferred, not read).
- Open work: no `TASKS.md` item names this directory. `OWNER_CODE_CONCERNS.md`: none.

### 2.8 abstract-operations-to-abstract-operations (stage 03)

- Path: `P/03_abstract-operations-to-abstract-operations`, 20 files, 21989 lines. Imported elsewhere as `abstract_optimization`.
  Largest files: `validation.rs` (3151), `semantic_analyses.rs` (2371), `global_value_numbering.rs` (2421),
  `control_flow_cleanup.rs` (1749), `countdown_analyses.rs` (1606), `pass_manager.rs` (1594).
- Purpose: the target-neutral optimizer. It admits the verified input, builds a `VerifiedPsiOptimizationUnit`, runs the
  selected pass registries over a deterministic rule order, and publishes a plan with a pre-physical manifest after an
  independent replay. With an empty selection it returns the input plan unchanged and errors if the plan changed
  (`abstract_optimization/mod.rs:112-117`, read).
- Consumes: `VerifiedPsiOptimizationInput` plus `OptimizationSelections`, `PsiOptimizationSelectionProjection` and
  `OptimizationWorkBudget` from `optimization-core`. Produces: `ValidatedOptimizedAbstractPlan`
  (`publication.rs:233`) with `plan()` (`:286`) and `into_lowering_input()` (`:293`, returns `OptimizedAbstractBody`).
- Entry points: `optimize_abstract_operations` (`abstract_optimization/mod.rs:97`; re-exported at `lib.rs:5-7`).
  Under `test-support`, `build_verified_psi_optimization_unit` (`verified_unit.rs:84`). Called from
  `driver/compile_body.rs:30`.
- Production dependencies: `abstract-operations`, `arena`, `optimization`, `optimization-core`, `optimization-unit`,
  `optimization-unit-semantics`, `semantic-vocabulary`, `terminal-codec`, `terminal-psi`, `terminal-semantics`,
  `terminal-work`, `verified-optimization-input`, `wire`. Dev: `psi-authorization`, `source-compiler` (with `test-support`),
  and stage 02 as `artifact-lowering`.
- Depended on by (1): `psi-to-native`.
- Modules (under `src/abstract_optimization/`): `mod.rs` (entry, error type, Tarjan SCC helper); `verified_unit.rs`
  (build the optimization unit from the input); `pass_manager.rs` (`run_registries`, ledgers, decision logs);
  `rules.rs` (rule registry and `PSI_PASS_CATALOG` of nine passes at `:216-250`: sparse conditional constant propagation,
  control-flow cleanup, copy propagation, global value numbering, proof-check elision, dead pure scalar elimination,
  state specialization, representation specialization, loop-invariant scalar motion); one file per pass family
  (`sccp.rs`, `control_flow_cleanup.rs`, `global_value_numbering.rs`, `proof_check_elision.rs`, `specialization.rs`,
  `ranked_rewrites.rs`); analyses (`analyses.rs`, `control_flow_analyses.rs`, `countdown_analyses.rs`,
  `semantic_analyses.rs`, `value_ranges.rs`); validation (`validation.rs`, `ranked_cycles_validation.rs`);
  `publication.rs` (seal and project the result).
- Tests: 12 test functions in 4 files (`control_flow_analyses_tests.rs` 8, `verified_unit.rs` 2, `global_value_numbering.rs` 1,
  `proof_check_elision.rs` 1). The pass files hold far fewer `#[test]` functions than their size suggests; the stage
  is exercised through the driver and the integration tests of other crates (inferred).
- Open work: no `TASKS.md` item names this directory. `OWNER_CODE_CONCERNS.md`: none.

## 3. How data flows through the area

1. A caller (source compile or saved `.psi` open) builds a provider demand:
   `IssuedProviderDemand::from_publication` (`provider-demand/demand.rs:58`) or `from_saved_program` (`:132`); the latter
   attaches a `TargetProviderInstallation` from `host-provider-inputs` (`installation.rs:94`). Called from
   `driver/inputs/published_application.rs:118,132` and `driver/inputs/components.rs:114`.
2. `psi_to_admitted_native_inputs::admit` (`admit.rs:14`) checks the authorization against the demand's artifact.
   Called at `driver/inputs/published_application.rs:87,162`.
3. Stage 01 `select_published_providers` (`provider_selection.rs:43`) plans the provider supply
   (`plan_provider_execution_inputs` -> `realize_target_supply`) and returns a `PublishedSelection<_, SelectedExecution>`;
   `bind_published_application` (`published_application.rs:311`) settles the entry and permissions.
4. `native_scalar_inputs::complete`/`prepare` (`scalar_applications.rs:14,28`) completes scalar values; called from
   `driver/inputs/scalars.rs:22`.
5. `native_execution::prepare_program` (`native_execution.rs:93`) calls `derive_native_scope`
   (`execution-scope/scope.rs:12`) and `provider_bindings::bind`, giving a `PreparedNativeExecution`. Called from
   `driver/inputs/mod.rs:34` and `driver/image/components.rs:160`.
6. Stage 02 `lower_native_root` (`admit_providers.rs:83`) or `lower_native_callable` (`selected_execution.rs:6`) lowers the
   reachable Psi bodies to a `VerifiedNativeArtifactInput`; `admit_prepared_native_realization` (`admit_providers.rs:160`) admits
   the providers; `admit_executable_entry` (`entry_receiver.rs:20`) checks the entry. Called from `driver/mod.rs:95,169,181`
   and `driver/image/plan_callables.rs:359,369`.
7. `driver/compile_body.rs:30` passes `input.into_optimization_input()` to stage 03 `optimize_abstract_operations`.
8. The result `ValidatedOptimizedAbstractPlan::into_lowering_input()` goes to stage 04 `target_lowering::lower_native_body`
   (`driver/compile_body.rs:38`), outside this area.

## 4. Where this map differs from the documents

- `P/03_.../src/abstract_optimization/mod.rs:23-30` names its neighbours "Omega 00" (`target-bound-program-to-abstract-operations`)
  and "Omega 02" (`abstract-operations-to-target-operations`) and the stage itself "Omega 01". The stage directories are numbered
  02 and 04, and the stage is 03.
- `P/02_.../README.md` says provider admission facts are consumed by "stage 04" (line starting "derives each demanded leaf's
  mechanism..."); stage 03 sits between and consumes only the `VerifiedPsiOptimizationInput`, so the fact travels through
  stage 03's input and out again to stage 04 (inferred from `compile_body.rs:30-38`).
- `P/02_.../README.md` says "The stage entry (`lower.rs`) lowers checked Psi". `lower.rs` is 28 lines of re-exports; the lowering is in
  `operations.rs` and `selected_input.rs`.
- `psi-to-native/README.md` says stage 01 is entered at `src/native_execution.rs`; that file holds `prepare_program`, but
  the provider selection step the same table row describes is in `provider_selection.rs` and `published_application.rs`.
- Crate names differ from import names: stages 01, 02 and 03 are imported as `provider_realization`, `artifact_lowering` and
  `abstract_optimization`, and stage 00 root as `native_inputs`, through `package = ` renames (`psi-to-native/Cargo.toml:29,33,34,37`).
  The documents use the directory names.

## 5. What I did not read, and what I am unsure about

Not read: the bodies of `operations.rs`, `operations/storage.rs`, `selected_input.rs` (beyond signatures), every pass file and
`validation.rs` in stage 03, `calling_policy_plans/*`, `provider_rows.rs`, `plan_realization.rs`, `realized_supply.rs`,
`package_review.rs`, and all test bodies. Not opened: the `representations/` crates these stages exchange data through
(`target-bound-program`, `abstract-operations`, `verified-optimization-input`, `optimization-unit`), other than locating type
definitions. The `provider-demand` dependency list is cut off at `terminal-codec` in my reading of its manifest.

Unsure: the exact production dependency count for stage 01 (52) was counted by eye from the manifest; whether stage 02's and
stage 03's thin test counts mean the stages are covered elsewhere (driver or integration tests); and whether `compiler/build`
needs stages 00 to 02 only for tests (its manifest lists them under `[dev-dependencies]`).
