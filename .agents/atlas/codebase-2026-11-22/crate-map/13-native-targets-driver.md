# Crate map: 13-native-targets-driver

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths are relative to it. Root of the area: `source-rust/subsystems/compiler/psi-to-native/` (written `P/` below). Sources: `crates.tsv` and `units.tsv` (files, lines), the `Cargo.toml` of each crate (read), a Python script over every `Cargo.toml` outside `_recycling/` and `.agents/` that counted dependents (ran; production `[dependencies]` only, dev-dependents listed separately), `grep -c '#[test]'` for test counts (ran).

## 1. What the area does

This area is the native compiler's driver, its target descriptions and the verification crates that sit beside the numbered stages. The driver (`P/driver/`, crate `native-compiler`) takes an authorized Psi program (a retained Terminal artifact) plus the receiving host's provider installation and permissions. It runs admission (stages 00 and 01), lowers the entry and private callables through stages 02 to 14 body by body, and either hands back interpreter provisions (runtime admission) or finishes an executable image (stage 15). The `targets/` crates describe each deployment target: profiles, register models, calling conventions, runtime ABI geometry, layout, and per-architecture effects and frame protocols for x86_64 and aarch64. The `verification/` crates check provider, authority, installation, component and emitted-object evidence. They are consumed by the numbered stages and by the driver. The numbered stages (`P/pipeline/`) and the representations (`P/representations/`) are other areas and are not mapped here.

## 2. Crates, in pipeline order

I order them as: target vocabulary first, then the driver, then verification, because the driver depends on both. Dependency counts are production path dependencies on other crates in this repo; "dependents" are production dependents, unless marked dev.

### 2.1 `target` (`P/targets/target`, 13 files, 4228 lines)

- Purpose: closed catalog of deployment targets. It defines `TargetProfile`, the realized `NativeTarget` (architecture, object format, pointer size), program-entry slot contracts, ELF interpreter plans, normalized foreign and component import locators, x86 feature requirements, and application metadata. Nothing downstream infers target policy from the host.
- Consumes: nothing from the compiler pipeline (wire, language-core, layout-plans, record-storage, diagnostics, semantic-vocabulary). Produces: `TargetProfile` (`targets/target/src/target_profile.rs:31`; variants LinuxArm64, LinuxX64, MacosArm64, MacosX64, WindowsX64, UefiX64, CrossPlatformCli, LocalUnchecked, AlphaBootstrap), `NativeTarget` (`target_profile.rs:532`), `NormalizedForeignLocator` and `NormalizedComponentLocator` (`foreign_locator.rs`, `component_locator.rs`), `NormalizedElfInterpreterPlan` (`elf_loader.rs`), `ExecutableImageEmissionRequest` (`image_request.rs`), `CanonicalFloatRealization` (`float_realization.rs`).
- Entry points: re-exports in `targets/target/src/lib.rs:44-80` (`normalize_foreign_locator`, `normalize_elf_interpreter_plan`, `ExplicitTargetSet`, `X86DeploymentFeatures`).
- Dependencies: 6 path crates (wire, language-core, layout-plans, record-storage, diagnostics, semantic-vocabulary). Dependents: 73 (most of the compiler, including `compiler`, `omega`, `package-manager`, `project-build`); dev: installation-admission, installation-binding, native-provider-sponsor.
- Modules: `target_profile.rs` (profiles, entry slots, 1054 lines), `foreign_locator.rs` (787), `x86_features.rs` (589), `target_semantics.rs`, `elf_loader.rs`, `float_realization.rs`, `component_locator.rs`, `explicit_target_set.rs`, `entry_contract.rs`, `image_request.rs`, `linux_filesystem.rs`, `compatibility_fingerprint.rs`.
- Tests: inline `#[cfg(test)]` modules, 52 `#[test]` (grep count). No separate test target.
- Open work: TASKS.md `TARGET-REALIZED-FLOAT-ARITHMETIC` (add checked-machine realization to `float_realization.rs`).

### 2.2 `runtime-abi` (`P/targets/runtime-abi`, 11 files, 2048 lines)

- Purpose: concrete target runtime ABI geometry: two-word descriptor layout, the 16-byte foreign-adapter frame, scalar-reference descriptors, borrowed-storage layouts, static-interrupt offsets, and a few host numeric/network leaves.
- Consumes `NativeTarget`. Produces `FatDescriptorLayout` and `fat_descriptor_layout`/`target_carrier_layout` (`targets/runtime-abi/src/lib.rs:~70-95`), `NativeForeignFrame` (`lib.rs`, `#[repr(C)]`), `NativeBorrowedStorageLayout` (`borrowed_storage.rs`), `ScalarReferenceLayout` and `ScalarStorageCodec` (`scalar_reference.rs`).
- Entry points: `fat_descriptor_layout`, `target_carrier_layout`, `capture_host_x86_flags` (the only `unsafe`, inline asm), `descriptor_import`, `host_integer_division` (re-exports in `lib.rs:50-75`).
- Dependencies: 3 (arena, target, semantic-vocabulary). Dependents: 27 (read from script; includes calling-conventions, layout, native-artifact, terminal-runtime, native-provider-runtime); dev: installation-admission, installation-binding, omega, post-allocation-machine.
- Modules: `borrowed_storage.rs` plus `encoding/` and `decoding.rs`; `scalar_reference.rs`; `descriptor_io.rs`; `host_numeric.rs`; `network_address.rs`; `windows_provider.rs`; `static_interrupts.rs`.
- Tests: inline, 19 `#[test]`.
- Open work: none found by name.

### 2.3 `calling-conventions` (`P/targets/calling-conventions`, 26 files, 11934 lines)

- Purpose: the normalized contract for how values cross a boundary: `CallPlan`, `StatePlan`, `BoundaryEntryPlan`, ABI value classification (Microsoft x64, System V AMD64, AAPCS64, Linux syscall), aggregate and Terminal storage layout, callback materialization, and host operation and import catalog. It also holds the native parameter telescope and process-entry plans.
- Consumes: `terminal-psi` structural types, `layout-plans`, `runtime-abi`, `target`. Produces: `CallPlan` (`targets/calling-conventions/src/plans/vocabulary.rs:640`), `ValidatedBoundaryEntryPlan`, `BoundaryCallingApplication` (`boundary_application.rs`), `NativeParameterTelescope` (`boundary_application/native_telescope.rs:17`), `TerminalCallingTelescope` (`boundary_application/terminal_telescope.rs`), `BoundaryValueGraph` (`boundary_values.rs`), `CallbackMaterialization` (`callback_materializations.rs`).
- Entry points: `evaluate_call_plan` (`plans/call_plan_evaluation.rs:25`), `validate_boundary_entry_plan` (`plans.rs:415`), `evaluate_ordinary_boundary_entry_plan`, `exact_hosted_physical_boundary_entry_plan` (`process_entry.rs:39`), `evaluate_conventional_sum_layout` (`aggregate_layout.rs`), `interrupt_gate_address` (`host_operations.rs`); full re-export list at `src/lib.rs:44-107`. `src/lib.rs` also defines `NativeForeignBridgeRequest` and `PublishedForeignAdapterIdentity`.
- Dependencies: 8 (layout-planning, wire, terminal-psi, runtime-abi, layout-plans, arena, semantic-vocabulary, target). Dependents: 46 (all backend stages, `native-artifact`, `provider-admission`, `project-build`, `terminal-runtime`); dev: native-execution-scope, selected-instruction-analyses.
- Modules (by size): `terminal_layout.rs` (1296), `plans/call_plan_evaluation.rs` (1085, the Rust placement rules), `plans/vocabulary.rs` (877), `boundary_values.rs` (685), `boundary_application.rs` (616) with `callback_catalog.rs`, `native_telescope.rs`, `terminal_telescope.rs`; `structural_shapes.rs` (561); `host_operations.rs` (568); `callback_materializations.rs` (429); `process_entry.rs`; `plans/plan_identity.rs`.
- Tests: `src/plans/tests/*` (placement, SysV records, native policies, entries) and `src/tests/clang_crosscheck/` (compares plans against clang MIR; `clang_mir.rs` 812 lines). 86 `#[test]` (grep). The clang comparison needs a host clang (inferred from the file names; not read in detail).
- Open work: TASKS.md `NORMALIZED-ABI-LOWERING`, and under the calling-policy item: remove the foreign ABI tag and derive `plan_identity.rs` from the normalized contract; move aggregate classification, register placement, stack and clobber rules into Omega policies.

### 2.4 `layout` (`P/targets/layout`, 3 files, 1427 lines)

- Purpose: native layout over target-neutral value recipes and canonical Terminal records for one `NativeTarget` or `TargetProfile`: pointer and descriptor geometry, `LayoutTarget`, nominal layout identities, selected record storage, boundary-value signature shapes.
- Consumes: `TerminalModule` records, `NativeLayoutRealization`, `calling_conventions::NativePlace`. Produces: `TypeLayout`, `LayoutTarget` (`targets/layout/src/lib.rs`), `TerminalRecordStorage` (`lib.rs:196`), `BoundaryValueLayout` and `TerminalBoundaryRecipes` (`boundary_values.rs`).
- Entry points: `LayoutTarget::for_profile`, `pointer_layout`, `fat_descriptor_layout` (`lib.rs`), `append_boundary_value_shape` (`boundary_values.rs:314`).
- Dependencies: 8 (arena, diagnostics, terminal-psi, semantic-vocabulary, layout-plans, runtime-abi, target, calling-conventions). Dependents: 3 (admitted-native-inputs-to-target-bound-program, native-binding-admission, provider-admission).
- Modules: `lib.rs` (230), `boundary_values.rs` (1147), `boundary_values/recipes.rs`.
- Tests: inline, 8 `#[test]` in `boundary_values.rs:616`.
- Open work: none found by name.

### 2.5 `register-model` (`P/targets/register-model`, 10 files, 3524 lines)

- Purpose: declarative physical register model (units, views, classes), per-instruction operand constraint catalogs, reservation profiles, ABI preservation storage groups, with structural validators and SHA-256 identities. No architecture data; the ISA crates supply it.
- Consumes: raw declarations from target crates. Produces: `ValidatedPhysicalRegisterModel` via `validate_physical_register_model` (`targets/register-model/src/lib.rs:298`; struct `PhysicalRegisterModel` at `lib.rs:99`), `RegisterConstraintCatalog` (`constraint_catalog.rs`), `ValidatedRegisterReservationProfile` (`reservation_profiles.rs`), `ValidatedPreservationStorageCatalog` (`preservation_storage.rs`), identity digests (`identities.rs`).
- Entry points: `validate_physical_register_model`, `validate_register_constraint_catalog`, `validate_register_reservation_profile`, `validate_preservation_storage_catalog`, `target_register_environment_identity`.
- Dependencies: 2 (wire, target). Dependents: 28 (allocator, encoders, frame stages, `machine-code`, both target crates); dev: isa-aarch64, isa-x86_64, target-aarch64, target-x86_64. Feature `test-support` (`src/test_support.rs`).
- Modules: `lib.rs` (510), `constraint_catalog.rs` (674), `identities.rs`, `preservation_storage.rs`, `register_vocabulary.rs`, `reservation_profiles.rs`.
- Tests: `src/tests.rs` and `src/tests/{constraint_catalog,declared_identities}.rs`; 19 `#[test]`.
- Open work: none found by name.

### 2.6 `target-x86_64` (`P/targets/target-x86_64`, 33 files, 9982 lines)

- Purpose: x86_64 register identities, SysV and Microsoft ABI constraints, frame protocol, machine-effect catalog, per-form effects, a small instruction-byte decoder, generated-entry decoding, import thunk footprints and inspection helpers. It is the x86_64 data the stages read; the encoder is in `isa-x86_64` (stage 11).
- Consumes: `selected-instructions` types, `register-model`, `calling-conventions`, `normalized-foreign-call-plan`. Produces: the x86_64 `PhysicalRegisterModel` and constraint keys (`src/register_model.rs`, 3707 lines), `x86_64_machine_effect_catalog` (`src/machine_effects.rs:623`), `X86_64FrameProtocolPlan` via `plan_system_v_amd64_frame_protocol` (`src/frame_protocol.rs:44`).
- Entry points: `decode_x86_64_generated_entry` (`src/generated_entry.rs:528`), `decode_one` (`src/instruction_bytes.rs:220`), `validate_x86_64_machine_effect_catalog`, `x86_64_normalized_foreign_call_declaration` (`src/lib.rs:36-41`), `x86_64_import_thunk_footprint` (`src/import_thunk.rs:6`), `hosted_syscall_target`.
- Dependencies: 10 (optimization-selections, semantic-vocabulary, normalized-foreign-call-plan, layout-plans, calling-conventions, diagnostics, register-model, selected-instructions, target, wire). Dependents: 15 (includes register-environment, installation-admission, image-emission, image-macho, x86_64-encoding); dev: post-allocation-machine.
- Modules: `register_model.rs`, `machine_effects.rs` (2054), `instruction_bytes.rs` (825), `generated_entry.rs` (690), `form_effects/` (about 17 per-form files), `inspection.rs`, `frame_protocol.rs`, `preservation_storage.rs`, `constraint_keys.rs`, `hosted_kernel.rs`, `import_thunk.rs`, `write_line.rs`, `float_sizes.rs`, `model_specific_state.rs`.
- Tests: inline, 50 `#[test]`. Dev-depends on `isa-x86_64` and `isa-conformance`.
- Open work: none found by name.

### 2.7 `target-aarch64` (`P/targets/target-aarch64`, 26 files, 6892 lines)

- Purpose: the aarch64 counterpart: AAPCS64 register model, constraint keys, frame protocol, machine-effect catalog, form effects, saturating forms, generated entry decoding.
- Entry points: `aarch64_machine_effect_catalog` (`src/machine_effects.rs:275`), `plan_aapcs64_frame_protocol` (`src/frame_protocol.rs:51`), `decode_aarch64_generated_entry` (`src/generated_entry.rs`), `aarch64_normalized_foreign_call_declaration`, `aarch64_fixed_register_view` (`src/lib.rs`).
- Produces: aarch64 `PhysicalRegisterModel` (`src/register_model.rs`, 2995 lines) and `Aarch64FrameProtocolPlan`.
- Dependencies: 8 (normalized-foreign-call-plan, layout-plans, semantic-vocabulary, calling-conventions, diagnostics, register-model, selected-instructions, target). Dependents: 11 (entry-planning, frame-admission, function-layout, image-emission, isa-aarch64, native-artifact, physical-recipes, post-allocation-machine-to-framed-machine, register-environment, selected-form-encoding-to-resolved-layout, selected-instruction-analyses). It has no `instruction_bytes.rs`, `hosted_kernel.rs` or `import_thunk.rs` (the x86_64 crate does); this is a difference, not a gap I verified against any document.
- Modules: `register_model.rs`, `machine_effects.rs` (1487), `form_effects/` (about 14 files), `inspection.rs`, `generated_entry.rs`, `frame_protocol.rs`, `preservation_storage.rs`, `constraint_keys.rs`, `saturating_forms.rs`, `write_line.rs`.
- Tests: inline, 38 `#[test]`; dev-depends on `isa-aarch64`, `isa-conformance`.
- Open work: none found by name.

### 2.8 `register-environment` (`P/targets/register-environment`, 13 files, 5247 lines)

- Purpose: joins a target's register model, constraint catalog, reservation overlay and selected constraint keys into one validated allocator environment, built once per target and cached process-wide.
- Consumes: `target-x86_64`/`target-aarch64` declarations, `register-model` validators, `selected-instructions` constraint keys. Produces: `ValidatedTargetRegisterEnvironment` (`targets/register-environment/src/lib.rs:148`).
- Entry points: `baseline_target_register_environment` (`src/lib.rs:64`, memoized in a `static Mutex<Vec<..>>`); also the preservation selection helpers named in the crate doc.
- Dependencies: 6 (calling-conventions, register-model, selected-instructions, target, target-aarch64, target-x86_64). Dependents: 14 (allocator, frame, encoder, layout, `published-object-verification`, `native-compiler`); dev: abstract-operations-to-target-operations.
- Modules: `lib.rs` (396), `catalog.rs`, `tests.rs`, `tests/abi_call_clobbers/*` (most of the 5247 lines; per-effect clobber tests), `tests/supported_targets.rs`.
- Tests: 32 `#[test]`, all inline in `src/tests*`.
- Open work: none found by name.

### 2.9 `native-intrinsics` (`P/targets/intrinsics`, 1 file, 285 lines)

- Purpose: names the exact built-in operation selected for a native boundary (`CompilerIntrinsicExecutionIdentity`) so receipts and lowering share one byte identity.
- Produces: `CompilerIntrinsicExecutionIdentity` (`targets/intrinsics/intrinsics.rs:6`), `CompilerAccessorScalarOperation`; `From<numerics::intrinsics::NumericIntrinsic>` conversion (`intrinsics.rs:~50`).
- Dependencies: 4 (language-core, numerics, semantic-vocabulary, terminal-psi) plus `sha2` (external). Dependents: 12 (provider-admission, provider-correspondence, target-bound-program, native-artifact, terminal-authority-receipts, and others).
- Tests: none (0 `#[test]`).
- Open work: none found by name.

### 2.10 `isa-conformance` (`P/tests/isa-conformance`, 4 files, 579 lines)

- Purpose: shared cross-architecture conformance assertions (float result policy, narrow loads, system calls) that the two ISA crates run.
- Entry points: modules `float_result_policy`, `narrow_load`, `system_call` (`tests/isa-conformance/src/lib.rs:28-30`).
- Dependencies: 2 (register-model, selected-instructions). Dependents: 0 production; dev: isa-aarch64, isa-x86_64, target-aarch64, target-x86_64.
- Tests: it is itself test support; 0 `#[test]`.
- Open work: none found by name.

### 2.11 `native-compiler` (`P/`, 29 files, 7658 lines; lib path `psi_to_native.rs`)

- Purpose: the native driver. It receives an authorized Psi program and the host's providers, runs stages 00 to 15 through the stage crates, and returns a native image, retained stage inputs for validators, or interpreter provisions. It is the crate the compiler entry (`compiler`) and `native-provider-sponsor` call.
- Consumes: `RetainedTerminalArtifact` (from `verified-optimization-input`), `AuthorizedTerminalArtifact` (psi-authorization), `PublishedTargetProviders` (provider-demand, stage 00). Produces: `NativeProgram` (`psi_to_native.rs:50`), then `NativeEmission { image: FinishedNativeImage, validations }` (`psi_to_native.rs:43`) or `AdmittedRuntimeProgram` (re-exported from `native_artifact`, `psi_to_native.rs:14`).
- Entry points: `prepare_source_program` (`driver/inputs/published_application.rs:28`), `prepare_saved_program` (`:128`), `review_source_providers` (`:64`); `NativeProgram::emit` (`psi_to_native.rs:75`), `NativeProgram::admit_runtime` (`:91`), `NativeProgram::component_descriptions`; `stage_validators::{StageValidator, StageValidatorRequest}` (`verification/stage_validators.rs:9,105`); feature-gated `test_support::{native_pipeline_invocations, compile_runtime_foreign_adapter}` (`psi_to_native.rs:30-34`).
- Dependencies: 62 path dependencies (read), including the stage crates by alias (`provider-realization` = stage 01, `artifact-lowering` = stage 02, `abstract-optimization` = stage 03, `target-lowering` = stage 04 and so on through `image-emission`, `native-artifact`), plus calling-conventions, register-environment, runtime-abi, target, provider-admission, installation-admission, published-object-verification, component-description, native-behavior-exclusions. Dependents: 2 (`compiler`, `native-provider-sponsor`); dev: native-compiler (self, with `test-support`), native-provider-runtime, native-provider-sponsor.
- Modules:
  - `driver/mod.rs` (225): `compile` (`:28`) and `admit_runtime` (`:39`) both call `inputs::prepare` then `produce` with a stop point (`Image` or `RuntimeAdmission`).
  - `driver/inputs/` (`mod.rs:23` `prepare`, `published_application.rs`, `components.rs`, `scalars.rs`): validate installation assumptions, prepare execution, private entry RAM and scalar applications.
  - `driver/compile_body.rs` (302): `plan_native_body` (`:20`, stages 03 and 04), `prepare_physical_body`, `encode_physical_body`, `layout_encoded_body`, `construct_body_object` (stages 05 to 14).
  - `driver/image/` (`plan_callables.rs` 379, `callback_thunks.rs` 609, `components.rs` 321, `object_emission.rs` 316, `callable_objects.rs`, `body_resources.rs`, `mod.rs`): collect the entry and private callables, emit callback thunks and component products, build the program image input.
  - `driver/runtime/` (`mod.rs:60,79` `plan_host_provisions`, `plan_root_provisions`; `foreign_adapters.rs`): interpreter provisions and foreign adapters.
  - `driver/request.rs`, `driver/observations.rs`: request struct and test-only stage counters.
  - `verification/stage_validators.rs` (296): the three named validators (target-operation correspondence, fixed-frame correspondence, selected-instruction rewrite replay) and their request/result types.
- Tests: no `[[test]]` entries; auto-discovered `P/tests/*.rs`: `native_entry_bridge.rs` (23 tests), `saved_linux_program.rs` (7), `native_foreign_bridge.rs` (4, runs the host C compiler), `foreign_adapter.rs` (1, loads an emitted shared library); `tests/physical_publication/` (4 tests, included via `#[path]` in `psi_to_native.rs:8`); fixtures `tests/fixtures/{retained_scalar_storage,windows-internet-provider}`. Own-file `#[test]` count 39 (grep). The crate-wide grep count of 2070 includes every stage crate beneath `P/` and is not this crate's.
- Open work: TASKS.md `PROVIDER-SELECTION-AFTER-TERMINAL` (handoff from source provider publication to `pipeline/00_psi-to-admitted-native-inputs`), `BUILD-SEMANTIC-EXCLUSIONS` (names the behavior-exclusions verifier), `PIPELINE-ARCHITECTURE-ENFORCEMENT` (multi-target, multi-root, component route through the shipped compiler entry).

### 2.12 `provider-correspondence` (`P/verification/provider-correspondence`, 16 files, 2804 lines) and `direct-boundary-bindings` (`.../direct-boundaries`, 3 files, 1042 lines)

- Purpose: match selected provider plans, boundary applications, callback registrars and candidate bodies to their exact declarations in the published Psi. Admission and native planning use the same checks. The `direct-boundaries` sub-crate checks one selected native binding against its callable, service contract and target ABI, and keeps authored root grants bound to the plan and package.
- Entry points: `select_boundary_applications` (`verification/provider-correspondence/src/boundary.rs:23`), `resolve_installation_reach` (`installation_reach.rs:23`), `select_callback_declarations`, `selected_external_execution` (`lib.rs:12-14`), `callbacks`, `external_providers`, `intrinsic_selection`, and `trust::*` (grant reconstruction); in the sub-crate `VerifiedDirectProviderSelection` (`direct-boundaries/scoped_selection.rs:8`), `NativeRequirementTrust` (`bindings.rs`), `compiler_primitive_schemas`.
- Dependencies: provider-correspondence 19 (native-intrinsics, target-provider-admission, provider-plans, calling-conventions, boundary-applications, terminal-psi, and others); direct-boundary-bindings 13. Dependents: provider-correspondence 9 (admitted-native-inputs-to-target-bound-program, compiler, native-compiler, native-execution-scope, numeric-intrinsic-admission, provider-admission, target-bound-program, target-bound-program-to-abstract-operations, verified-optimization-input); direct-boundary-bindings 5 (stage 01, project-build, provider-correspondence, target-bound-program, verified-optimization-input).
- Tests: provider-correspondence 22 `#[test]` (`trust/tests.rs`, `bodies/callback_roots_tests.rs`); direct-boundaries 3.
- Open work: none found by name.

### 2.13 `target-provider-admission` (`P/verification/target-providers`, 2 files, 1660 lines)

- Purpose: binds Build-selected target provider plans to canonical Terminal candidate rosters, closes checked specializations, synchronous invocation cycles and folded-constant dependencies.
- Produces: `TerminalTargetProviderProposal` and `TerminalTargetProviderSelection` (`target_providers.rs:16,26`). Entry points: `admit_selected_checked_providers` (`:206`), `close_checked_provider_specializations` (`:491`), `validate_selected_service_schemas` (`:412`), `selected_provider_candidate_rows` (`:593`), `synchronous_invocations::validate_selected_synchronous_invocation_cycles`.
- Dependencies: 10. Dependents: 10 (stage 01, native-artifact, native-scalar-inputs, project-build, provider-admission, provider-correspondence, target-bound-program, stage 02, terminal-runtime, verified-optimization-input).
- Tests: 13 inline `#[test]`.
- Open work: none found by name (TASKS.md `build_target_provider` lines are about a sample helper, not this crate).

### 2.14 `native-binding-admission` (`P/verification/native-bindings`, 1 file, 932 lines)

- Purpose: keeps a Build-evaluated native binding (receipt plus full requirement schema) together, so a locator alone grants nothing.
- Produces: `RetainedNativeBindings`, `RetainedNativeBinding`, `NativeBindingInput` (`bindings.rs:12-40`); `RetainedNativeBindings::retain` (`:59`), `identity_digest`.
- Dependencies: 10. Dependents: 3 (native-artifact, provider-admission, verified-optimization-input). Tests: 7 inline.
- Open work: none found by name.

### 2.15 `numeric-intrinsic-admission` (`P/verification/numeric-intrinsics`, 6 files, 918 lines)

- Purpose: binds a reference executor's numeric imports to checked provider rows and exact published Psi.
- Produces: `AdmittedTerminalIntrinsics` (`admission.rs:14`). Entry points: `admit_terminal_intrinsics` (`:49`), `direct_numeric_target` (`direct.rs`), `selection::validate_external_provider_owners`, `signature::numeric_boundary_execution`, `catalog_realization`. `provider-admission` re-exports these (`provider-admission/src/compiler_builtins.rs:9-15`).
- Dependencies: 14. Dependents: 3 (build-execution, native-scalar-inputs, provider-admission). Tests: 5 (`tests.rs`).
- Open work: none found by name.

### 2.16 `terminal-authority-receipts` (`P/verification/provider-admission/authority`, 2 files, 1093 lines) and `provider-stack-contribution` (`.../same-stack`, 2 files, 315 lines)

- `terminal-authority-receipts`: checks and identifies the Terminal authority leaves reviewed against a physical classification and optional receiving permission policy. Types: `TerminalAuthorityPolicyIdentity` (`authority/receipt.rs:17`), mechanism identities (`authority/mechanisms.rs`). Dependencies: 7. Dependents: 3 (native-artifact, native-behavior-exclusions, provider-admission). Tests: 2.
- `provider-stack-contribution`: binds the caller-stack contribution of an opaque provider to its selected plan, requirement and admission receipt. Type: `AdmittedSameStackContribution` (`same-stack/src/lib.rs:86`). Dependencies: 2 (terminal-psi, wire). Dependents: 9 (machine-code, native-artifact, native-compiler, native-stack-demand, normalized-foreign-call-plan, provider-admission, published-runtime-input, register-homes, target-vocabulary). Tests: 3.
- Open work: none found by name.

### 2.17 `provider-admission` (`P/verification/provider-admission`, 55 files, 26700 lines including its two sub-crates' rows in `units.tsv`; crate files 51 by path)

- Purpose: admits the providers the selected entry reaches against the authorized Terminal artifact and the receiving authority policy. It issues the sealed records that target lowering reads: settlements per boundary, installed-interrupt and callback cohorts, task-activation checks, and the terminal-authority review. Roughly half its size is the Windows handle review (`src/terminal_authority_review/windows_handle_*.rs`, about 12 files; `windows_handle_replay.rs` is 2819 lines, `windows_handle_storage.rs` 2122).
- Consumes: `AdmissionSource` (`src/admission_source.rs:31`), `ProviderAdmissionRequest` (`src/model.rs:62`), the canonical Terminal artifact, selected provider plans, `TerminalAuthorityPolicy` and `TerminalAuthorityPermissionPolicy`. Produces: `AdmittedNativeProviders` (`src/admission.rs:84`), `AdmittedBoundarySettlement`, `NativeBoundaryRealization` (`src/model.rs`), `AdmittedProviderInstallation` (`src/provider_installation.rs:116`), `AdmittedInstalledInterruptCohort`, `AdmittedNativeCallbackCohort` (`src/lowering_facts.rs`).
- Entry points: `admit_native_providers` (`src/admission.rs:146`), `current_terminal_authority_policy` (`src/terminal_authority_policy.rs:297`), `terminal_authority_permissions::admit_package_terminal_authority_permissions`, `validate_task_activations`, `prepare_source_evaluated_bindings`, `retain_selected_provider_adapters`, `project_native_callback_arguments` (re-exports at `src/lib.rs:24-83`).
- Dependencies: 38 path crates (read; includes target-bound-program, provider-acceptance, native-binding-admission, numeric-intrinsic-admission, target-provider-admission, provider-correspondence via sub-crates, layout, native-intrinsics, psi-provider-selections, psi-entry-memory, psi-target-layouts, service-surface). Dependents: 12 (stage 01, stage 02, stage 04, stage 05, build-execution, image-emission, installation-admission, native-artifact, native-compiler, package-manager, published-runtime-input, verified-optimization-input); dev: project-build.
- Modules: `admission.rs` (506), `settlements.rs` (612) and `settlements_source_imports.rs`, `compiler_builtins.rs` (922, `compiler_builtins/catalog.rs`), `static_interrupts.rs` with `arrival_helpers.rs` (1038) and `scopes.rs`, `provider_installation.rs` and `provider_installation_replay.rs` (643), `access_lifecycles.rs`, `callback_entries.rs`, `installed_interrupt_helpers.rs`, `native_binding_settlements.rs`, `source_evaluated_bindings.rs`, `task_activations/`, `terminal_authority_policy.rs` (657) with `terminal_authority_policy/{posix_operations,windows_imports,darwin_poll,host_cohorts,classification,external_mechanisms}.rs`, `terminal_authority_permission_policy.rs`, `terminal_authority_review/` (reviewer, context, reachability, Windows handle files).
- Tests: 69 `#[test]` (grep), inline; `task_activations/tests.rs`, `terminal_authority_policy/host_cohorts/tests.rs`.
- Open work: TASKS.md `PROVIDER-SELECTION-AFTER-TERMINAL`; OWNER_CODE_CONCERNS.md has no entry naming it (grepped).

### 2.18 `installation-conditions` (`.../installation-admission/conditions`, 5 files, 482 lines), `bound-installations` (`.../bound-installations`, 1 file, 380 lines), `installation-admission` (`P/verification/installation-admission`, 13 files, 3686 lines)

- `installation-conditions`: platform assumptions supplied for an installation and the measured interrupt-stack conditions. Types: `InstallationEnvironmentAssumption`, `BootstrapEnvironmentAssumption`, `InstalledInterruptStackRequirement` (`conditions/src/lib.rs:13-20`); `encode_installation_assumptions`/`decode_installation_assumptions` (`review.rs:145`). Dependencies 5; dependents 13 (including omega, package-manager, project-build, `native-compiler`, stage 02 and 05). Tests 2.
- `bound-installations`: the interrupt tables, entries, helpers and stack requirements retained for a program. Type `BoundInstallations` (`bound-installations/src/lib.rs:19`). Dependencies 7; dependents 4 (installation-admission, native-compiler, provider-admission, target-operations-to-selected-instructions). Tests 1.
- `installation-admission`: validates installation guards where retained handler bodies meet an installer's frame; reads the selected control graph and operands of a handler. Entry points: `admit_planned_guards` (`src/resources.rs:95`), `BoundInstallationAuthority::verify` (`src/authority.rs:25`), `arrival::compose` (`src/arrival.rs:32`), `cpu::*`, `ram::*`, `selected::*` (`src/lib.rs`). Types: `PlannedInstallationResources`, `EntryInstallationResources`. Dependencies 15 (includes target-x86_64 as a production dependency, `Cargo.toml`; it has no aarch64 equivalent). Dependents 9 (entry-planning, image-emission, installation-binding, native-compiler, object-file, physical-program, post-allocation-machine-to-framed-machine, published-object-verification, target-operations-to-selected-instructions). Tests 14 (`selected/control_tests.rs` and inline).
- Open work: none found by name.

### 2.19 `component-description` (`P/verification/component-description`, 7 files, 2826 lines)

- Purpose: reads a component's published description and independently reconstructs entries, exports, authority and obligations from the embedded Psi artifact.
- Produces: `VerifiedComponent` (`verification.rs:216`). Entry points: `verify_component` (`verification.rs:404`), `decode_component_description`, `encode_component_description_checked`, `description_subject` (`description.rs:14-18`); `VerifiedComponent::verify_native_realization` (`verification.rs:289`).
- Dependencies 5 (wire, service-surface, semantic-vocabulary, terminal-codec, terminal-psi). Dependents 6 (stage 01, compiler, native-artifact, native-compiler, provider-admission, verified-optimization-input); dev: provider-plans, selected-provider-plans. Feature `test-support`.
- Modules: `verification.rs` (976), `wire_format.rs` (531), `inventory.rs`, `records.rs`, `retained_plan.rs`. Tests: `tests.rs`, `tests/`; 5 `#[test]`.
- Open work: none found by name.

### 2.20 `native-behavior-exclusions` (`P/verification/behavior-exclusions`, 3 files, 1116 lines)

- Purpose: verifies Build's behavior exclusions (crash causes, services, physical authority classes) against a selected native composition. Semantic operations and physical mechanism receipts each satisfy their own check.
- Produces: `BehaviorExclusionReport`, `BehaviorExclusionVerdict`, `EvidenceGap` (`closure.rs:148,141,132`). Entry points: `establish_behavior_exclusions` (`closure.rs:248`), `BehaviorExclusions` (`exclusions.rs`).
- Dependencies 7. Dependents 3 (native-compiler, stage 02, verified-optimization-input). Tests 3.
- Open work: TASKS.md `BUILD-SEMANTIC-EXCLUSIONS`.

### 2.21 `published-object-verification` (`P/verification/published-objects`, 2 files, 749 lines) and `published-runtime-input` (`P/verification/runtime-publication`, 1 file, 297 lines)

- `published-object-verification`: rechecks an emitted object against the original stage inputs on an explicit audit request. Only a compile that asked for an audit retains those inputs. Types: `RetainedPhysicalInputs` (`published-objects/src/lib.rs:18`), `NativeStageReplay` (`:87`). Entry points: `verify_retained_target_operations` (`:344`), `verify_retained_fixed_frame` (`:183`), `verify_body_fixed_frame` (`:196`), `verify_retained_selected_optimization` (`:323`), `verify_body_selected_optimization` (`:334`). Dependencies 20 (stages 04, 06, 07, 08, 09, 12, 15 crates by alias). Dependents 1 (`native-compiler`). Tests 0 in the crate (grep); `allocation.rs` holds the allocation correspondence.
- `published-runtime-input`: canonical root, component and host provisions for a Terminal interpreter; native code stays with the publisher. Types: `RuntimeComponent` (`publication.rs:22`), `RuntimeExecutionProvisions` (`:140`); `RuntimeComponent::admit` (`:35`), `validate_component_roster` (`:214`). Its package name is `published-runtime-input` and lib path `publication.rs`. Dependencies 14. Dependents 2 (native-artifact, terminal-runtime). Tests 0.
- Open work: none found by name.

## 3. How data flows through the area

1. Caller (the `compiler` crate) builds a target: `SourceNativeTarget` or `SavedProgramTarget` (`driver/inputs/published_application.rs:9,19`), with a `TargetProfile` from `target`.
2. `prepare_source_program` (`:28`) or `prepare_saved_program` (`:128`) pairs the authorized artifact with provider demand (stage 00 crates) and verifies component suppliers (`driver/inputs/components.rs`; `component-description::verify_component`). Result: `NativeProgram` (`psi_to_native.rs:50`).
3. `NativeProgram::emit` (`:75`) or `admit_runtime` (`:91`) builds a `NativeCompileRequest` with `current_terminal_authority_policy()` (provider-admission) and calls `driver::compile` or `driver::admit_runtime` (`driver/mod.rs:28,39`).
4. `inputs::prepare` (`driver/inputs/mod.rs:23`): checks installation assumptions (installation-conditions), calls `provider_realization::native_execution::prepare_program` (stage 01), admits private entry RAM and scalar applications.
5. `produce` (`driver/mod.rs`): `image::components::bind`/`emit` for component suppliers; `artifact_lowering::lower_native_root` (stage 02); `admit_realization` calls `admit_prepared_native_realization` (reaches `provider_admission::admit_native_providers`, `numeric-intrinsic-admission`, `provider-correspondence`, `target-provider-admission`, behavior-exclusion checks; inferred from dependencies and the crate roots, not traced line by line) and `admit_executable_entry`.
6. If the stop is runtime admission: `runtime::plan_root_provisions` (`driver/runtime/mod.rs:79`) and foreign adapters yield `AdmittedRuntimeProgram` (interpreter provisions; `published-runtime-input` types).
7. Otherwise `image::prepare_program_image` (`driver/image/mod.rs:~16`): `plan_callables::plan_program` runs `plan_native_body` (`driver/compile_body.rs:20`, stages 03 and 04) then the physical pipeline (stages 05 to 14, consulting `target-x86_64`/`target-aarch64`, `register-environment`, `calling-conventions`, `layout`, `installation-admission`) for the entry and each private callable; `callback_thunks` and `object_emission` produce objects.
8. `ProgramImageInput::emit` (stage 15, `native_artifact`) yields `FinishedNativeImage`; `NativeEmission` is returned with `stage_validators::StageValidatorRequest::check` results (`verification/stage_validators.rs:179`), which call `published-object-verification` using the retained `NativeStageReplay` (only retained when validators were requested, `psi_to_native.rs:81`).

## 4. Where this area's map differs from the documents

1. `targets/calling-conventions/README.md` links `../../realization/provider-planning/src/calling_policy_plans.rs` and related files. The directory `P/../realization` does not exist (`source-rust/subsystems/compiler/realization`, checked with `[ -e ]`). The code it describes (`calling_policy_plans/build_time_decoding.rs`) is in `P/pipeline/01_admitted-native-inputs-to-target-bound-program/src/calling_policy_plans/` (TASKS.md line 43 already links that path). The README link is stale.
2. `AGENTS.md` and the task brief list four hosted targets (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`). `TargetProfile` has nine variants, including `MacosX64`, `UefiX64`, `CrossPlatformCli`, `LocalUnchecked`, `AlphaBootstrap` (`targets/target/src/target_profile.rs:31-41`). The four-target statement is about release scope, so this is a scope difference, not a contradiction.
3. `tests/isa-conformance/src/lib.rs` says `isa-aarch64` and `isa-x86_64` depend on it; the manifests also show `target-aarch64` and `target-x86_64` as dev-dependents.
4. `P/README.md` says the body driver coordinates stages 03 to 15 for the entry and private callables. `driver/compile_body.rs` itself only wires stages 03 and 04 in `plan_native_body` and a physical pipeline (`prepare_physical_body` to `construct_body_object`); stage 15 image emission is in `driver/mod.rs` (`image.emit()`) and `driver/image/`. Close to the README, but "stages 03 to 15" is true only across `compile_body.rs`, `driver/image/` and `driver/mod.rs` together (read, not every line).
5. `AGENTS.md` places the Psi interpreter provisions at "runtime admission" in the driver; the code agrees (`driver/mod.rs:39`), nothing to report there.

## 5. Not read, and unsure

Not read in full: the bodies of `register_model.rs` (both architectures), `machine_effects.rs`, `form_effects/*`, `instruction_bytes.rs`, `call_plan_evaluation.rs`, `terminal_layout.rs`, the Windows handle review files, `static_interrupts/*`, `settlements*.rs`, `provider_installation_replay.rs`, `callback_thunks.rs`, `plan_callables.rs`. I read the crate roots, module headers, signatures, manifests and the entry points named above. I did not open `P/representations/` or the numbered stage crates, so statements about what stages 01 and 02 call inside `admit_prepared_native_realization` are inferred from manifests.

Unsure:
- `units.tsv` gives 55 files and 26700 lines for `provider-admission` while `crates.tsv` counts its two sub-crates (`authority`, `same-stack`) separately; I used the `crates.tsv` totals and did not reconcile them (sub-crates add 4 files and 1408 lines when counted per `crates.tsv`, so the parent may be double-counted in my section 2.17 header).
- Whether `clang_crosscheck` tests skip or fail without a host clang (not read).
- Test counts are `#[test]` attribute counts only; `#[tokio::test]`, doc tests and macro-generated tests are not counted. `native-compiler`'s 39 is for its own files, not the stages beneath it.
- Dependent counts include crates under `source-rust/` only if they have a `Cargo.toml` outside `_recycling/`; `omega-rust-v2/` manifests were also walked if present, so a name such as `compiler` could belong to either workspace (not checked).
