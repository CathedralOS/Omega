# Tasks

Capability priorities and acceptance; see [the sample loop](AGENTS.md#the-sample-loop) and [advance](.agents/skills/advance/SKILL.md).

## Working the board

Each outcome frames a gap as an end-to-end sample witness carried through `Psi 00–08` and `Omega 00–09` (`omega run --both`).

<a id="pcc-removal"></a>

### Development-health outcomes

- **ITERATION-PERFORMANCE.** Restore a usable edit/build/run workflow when a bottleneck measurably obstructs development.
- **VALIDATION-ENVIRONMENT.** Restore bounded test selection or harness feedback while preserving runtime oracles.

## Updating the boards

Update by subtraction (`Delta lines <= 0`): keep discrete lines and delete finished bullets outright.

## Ownership firewall

Follow [the pipeline](omega-rust/pipeline.md): Psi owns source through Terminal Psi (`PSITERM\0`); Omega owns realization.

## Priority (human-set only)

- **NESTED-MACHINE-UNIFICATION.** Replace `state` with nested machines ([machines.md](wiki/spec/language/machines.md#nested-machines-and-identity), [state_contracts.md](wiki/spec/language/state_contracts.md)).
  - Resolve recursive scopes, elaborate flat identities, and reject runtime capture or code after trailing declarations.
  - Complete cyclic arrivals, normal returns, loans, restoration, linear custody, and SCC rank measures across call shapes.
  - Bring `execution/terminal_unit/state_graph` to call-plan parity so `LinuxArm64::plan` fuses.
  - Complete the declaration-model migration across linked repositories, samples, the corpus, and both compilers.
  Acceptance: 3-level nesting, helper calls, cycles, and 9-place rows pass `omega run --both`; `state` syntax is deleted.

- **CANONICALIZE-SCALAR-RANGE-CONTRACTS.** Complete [scalar range contracts](wiki/spec/language/domains.md#declaration-and-membership) and remove `T [lo..hi]` / `T [lo..=hi]`.
  - Prove field/case bounds on construction (`construction_predicates.rs`), writes, field-loan returns, and array updates.
  - Check and lower scalar-field loans across case fields, recursive records/references, calls, joins, and [recasts](omega-rust/psi/compilation/validation/recasts.md).
  - Migrate all customers ([range_suffix_migration.md](wiki/drafts/designs/range_suffix_migration.md)) and qualify filesystem oracles with **FILESYSTEM-PLATFORM-SERVICES**.
  Acceptance: all sources pass `omega run --both`; suffix parsing (`parse_type.rs`) and `TypeConstraintNode::Range` are deleted.

- **STATIC-MACHINE-MIGRATION.** (covers **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION**, **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION**) Complete [static machines](wiki/spec/language/machines.md#static-declarations-and-explicit-state), [named implementations](wiki/spec/language/conformances.md#data-and-code-are-independent), and `dropped by`.
  - Complete const-expression closure, package projection, calling policies, and `source/psi/` evidence-binder parity.
  - Qualify `positional_implementation_binder`, `open_const_implementation`, `open_record_const_implementation`, `named_conformance_dispatch`, `generic_calling_policy`, and `domain_selected_cleanup` on all hosts.
  - Move Build facet calls to `Build::*`, retire `machine Data::operation` and attached helpers, and finish suspension loans.
  Acceptance: 9-place `dropped by` rows, `generic_counters`, clock/GUI samples, and `traits/,layouts/,progress/` pass `--both`.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** (covers **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION**) Complete `dyn` dispatch ([dynamic_dispatch.md](wiki/spec/terminal-psi/dynamic_dispatch.md)).
  - Complete envelope refinement under [known-selection narrowing](wiki/spec/language/effects.md#known-dynamic-selections) and preserve 8-byte dispatch tables across all hosts.
  Acceptance: `python3 tools/corpus_gate.py --both --filter traits/` (`runtime_dyn_*_exit`) passes on Windows, macOS, and Linux.

- **BUILD-OWNED-TARGET-SELECTION.** Retire target-prefixed machines for target-keyed Build rows ([provider_selection.md](wiki/spec/build/provider_selection.md#build-owned-target-selection)).
  - Delete source target modifiers/family codecs and migrate `core/targets`, `std/targets`, `source/`, `samples/cli/facts/**/target_body/main.omg`, `audio_playback`, and target fixtures.
  - Transport requirement/implementation identities, lifetimes, const/evidence telescopes, callbacks, and `invokes` ceilings.
  Acceptance: one portable call selects per-target implementations from one Terminal artifact under `omega run --both`.

- **BUILD-OWNED-NATIVE-BINDINGS.** Implement `Build::bind<Requirement>(builder, target, locator)` ([foreign_bindings.md](wiki/spec/build/foreign_bindings.md#build-owned-native-binding)) and retire `via`.
  - Resolve product requirements, infer `ForeignBinding` const lengths, migrate all `via` sites, and delete `via` parsing.
  Acceptance: PlaySoundW, ALSA, and AudioQueue bindings pass `omega run --both` on matching hosts, and `via` is removed.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Finish retiring standalone `boundary requirement` declarations ([machines.md](wiki/spec/language/machines.md#supply)).
  - Run migrated POSIX (`pass/filesystem/`, `runtime_posix_*`) and Windows GUI (`runtime_{gui,windows}_*`, `window_demo`) customers via `omega run --both`.
  Acceptance: POSIX and Windows GUI customers pass `omega run --both` under boundary-trait selection while bad providers reject.

- **OPERATOR-MACHINE-SUPPLY.** Run declared operations and premises end to end ([executable supply](wiki/spec/language/expressions.md#executable-supply), [contracts.md](wiki/spec/proofs/contracts.md#licensed-normalization)).
  - Implement literal-linked index identities (`N + 0`, `N * 1`) and 64-summand canonical index normalization.
  - Carry generic compatibility through specialization, retire the `operator` introducer ([inventory](wiki/drafts/audits/operator_introducer_retirement_inventory.md)), and lower `f64 in D` / `i32 in D` operator shapes.
  Acceptance: `declared_operator_match_result` and `body_bearing_comparison`/`generic_comparison_body`/`open_index_distributivity` pass `--both`.

## Pipeline route

- **PIPELINE-ARCHITECTURE-ENFORCEMENT.** Enforce the single-pass Psi (`00–07`) + Build + per-target Omega (`00–09`) [pipeline](omega-rust/pipeline.md) in `target_batch.rs`.
  - Admit dependency contexts ([scoped_execution.md](wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse)), run `build.omg` and Psi `00–07` once, clear `FIREWALL_ALLOWANCES` in `tools/check_native_boundary.py`, and drop 256 MiB stacks.
  Acceptance: two-target builds run Psi `00–07` and Build once and Omega `00–09` once per target with clean boundary guards.

- **PROVIDER-SELECTION-AFTER-TERMINAL.** (covers **OPERATOR-BOUNDARY-CALLS**, **TARGET-SEMANTIC-APPLICATIONS**) Settle providers over Terminal Psi in `execution_settlement.rs`.
  - Move `execution_settlement.rs`, `provider_binding.rs`, `operators.rs`, opaque/const/review paths, float/FMA/GUI calls, and target capsules to Terminal Psi.
  Acceptance: two-target builds share one Terminal artifact, and `pass/float/float_computation_route_exit` passes `--both`.

## Lowering, checker, data-flow, and PCC simplification

- **LOC-VALUE-CUSTODY.** (covers **IMAGE-SELF-REPLAY-DELETED**) Carry value custody on one path per layer and qualify the `linux_x86_64` native corpus leg.
  - Retire Psi 06 shape gates (`tools/stage06_shape_gates.txt`) and source-context reads (`tools/stage06_source_context.txt`) using Psi 05 plans.
  Acceptance: retired gates have checking reject twins in `tests/omega/fail/facts/`, and `linux_x86_64` native corpus passes.

- **WIRE-CODEC-LIBRARY.** (covers **SEMANTIC-REFLECTION**) Complete [semantic reflection](wiki/spec/language/reflection.md) and `compact_binary` in `source/library/std/wire.omg`.
  Acceptance: `source/library/std/wire.omg`, the 40-field record, `tests/omega/{pass,fail}/wire/*`, and `wire_protocol` pass `omega run --both`.

## Universal interpreter cutover

- **EARLY-TERMINAL-JOBS.** (covers **TERMINAL-LAYOUT-WIRE-EVALUATION**, **TERMINAL-BUILD-SEED**) Route [evaluation](wiki/spec/language/evaluation.md), [embedding](wiki/spec/build/embedding.md), and [scoped Build](wiki/spec/build/scoped_execution.md) through `TerminalProgram::load`.
  - Remove tree cloning in `build-time-evaluation` and run `build.omg` activation, queries, and `restricted_build_grants.rs` in the Terminal interpreter.
  Acceptance: `pass/comptime/*_exit`, `pass/layouts/runtime_plan_laid_*_exit`, `pass/wire/*_exit`, and `pass/build/*_exit` pass `--both`.

- **UNIVERSAL-INTERPRETER-CLOSURE.** Delete `omega-rust/psi/interpreter/checked-interpreter/` and finish `omega-rust/omega/interpreter/terminal-runtime/src/execution.rs`.
  Acceptance: audits find no tree visitor, and `omega run --both`, `--engine native|terminal`, and `interpreter_migration.json` pass on all hosts.

## High-priority composition repairs

## Immediate product closure

- **REQUIRES-UNIFICATION.** Implement [unified `requires` clauses](wiki/spec/language/state_contracts.md#requirement-clauses), migrate all sources, and delete `where` (`ClauseKind::Where`) in both compilers.
  Acceptance: both compilers accept data/generic/callable/assembly `requires` forms, migrated sources pass `--both`, and `where` rejects.

- **STAGED-BUILD-CAPABILITIES.** Implement [checked Build stages](wiki/spec/build/scoped_execution.md#checked-build-stages) and attenuated filesystem delegation, retiring `depend*` and `BuildInputs`/`BuildSource`/`BuildOutput`.
  Acceptance: root/dependency Builds, generators, and attenuated filesystem providers pass `omega run --both` with legacy Build paths deleted.

- **LIBRARY-REPRESENTATION-FAMILIES.** Implement [representation applications](wiki/spec/layouts/plans.md#representation-applications), [transparent generic aliases](wiki/spec/language/generics.md#transparent-type-aliases), and [conversions](wiki/spec/layouts/codecs.md#representation-conversions).
  Acceptance: 9-place rows for aliases, `FixedLayout<T>` ([fixed_region_byte_roundtrip](samples/cli/facts/fixed_region_byte_roundtrip/README.md), [fixed_layout_reference_readback](samples/cli/facts/fixed_layout_reference_readback/README.md), [implementation_region_forwarding](samples/cli/facts/implementation_region_forwarding/README.md)), and [representation_agreement](samples/cli/facts/representation_agreement/README.md) pass `--both`.

- **DATA-DECOMPOSITION.** Implement [separate data and case decomposition](wiki/spec/language/patterns.md#data-and-case-decomposition) (`case as binding`, `let Pattern = expr;`) and retire flattened case patterns.
  Acceptance: named and positional decomposition pass `omega run --both` across 9-place rows and reject twins with flattened patterns deleted.

- **STATE-LOCAL-VALUE-FRONTIER.** (covers **REFERENCE-LOCAL-BINDING-ROUTES**, **BORROWED-STORAGE-RESTORATION**, **MATCH-SELECTIVE-LOWERING**) Complete expression, reference-local, [borrowed window](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows) (`borrowed_windows.rs`), and [match](wiki/spec/language/patterns.md) (`match_dispatch.rs`, `address_joins.rs`) lowering.
  Acceptance: `scalar_tag_field_loan`, `recoverable_replacement_window`, `runtime_dutch_flag_partition_exit`, and `borrowed,window,carried_loan` pass `--both`.

- **MODULE-NAMESPACE-RESOLUTION.** (covers **RUNTIME-VALUE-GENERICS**, **STRUCTURAL-GENERIC-MATCHING**, **FINITE-GENERIC-DISPATCH**) Complete [module selection](wiki/spec/language/modules.md) and [generics](wiki/spec/language/generics.md) (`generic_arguments.rs`, `type_equations/`, `dynamic_families.rs`).
  Acceptance: `modules/`, `runtime_index_application`, `selected_value_application`, `inferred_array_equation_call`, and finite-family fixtures pass `--both`.

- **DOMAIN-ACCESS-AUTHORITY.** (covers **DOMAIN-CARRIER-FORMATION**, **NOMINAL-FIELD-FLOW**, **DOMAIN-ISSUER-ROUTES**) Complete [carrier membership](wiki/spec/language/domains.md#carrier-exact-membership), `[u8; N] in D` (`field_domains.rs`, `reference_shells.rs`), and [domain `access Policy`](wiki/spec/resources/access.md) (`attached_unit.rs`).
  Acceptance: [domain_carrier_identity](samples/cli/facts/domain_carrier_identity/README.md), Dungeon Crawler, [controlled cell](wiki/language_guide/chapter_8_domains.md#read-and-write-authority), and `bodyless_owner_establishment` pass `--both`.

- **TERMINATION-RANKING-CHECKS.** (covers **GENERAL-CYCLIC-EXECUTION**, **CRASH-CONTRACT**, **OUTCOME-SPECIFIC-CALL-EVIDENCE**, **UNREACHABLE-BRANCH-DIAGNOSTICS**) Complete [termination](wiki/spec/language/termination.md) (`ranking_range/calls.rs`, `value_origins.rs`), crash/outcome evidence (`operator_crashes.rs`), and [unreachable warnings](wiki/spec/language/state_contracts.md#unreachable-branches).
  Acceptance: `termination/,control_flow/`, `progress_selector/`, `array_max`/`dot_product`/`horner_eval`, crash rows, and unreachable warnings pass `--both`.

- **ATOMIC-MEMORY-MODEL.** Complete atomic/fence semantics under [concurrency](wiki/spec/language/concurrency.md) and [atomic_operations.md](omega-rust/psi/foundation/language-core/atomic_operations.md) in `atomic_events.rs`.
  Acceptance: `shared_receiver_atomic_store` and `atomic_global_order_operations` pass `omega run --both` on all hosts, and concurrent witnesses run with **TR3-TR8**.

<a id="checked-assembly-control-flow"></a>

- **CHECKED-ASM-LABEL-CONTRACTS.** Implement `asm machine`, block declarations, `align_of(value)`, port settlements (`provider_custody.rs`), and [interpreted assembly](wiki/spec/language/assembly.md#interpreted-execution).
  Acceptance: `samples/cli/facts/checked_asm_*`, `asm_runtime_port_msr_final_validation`, `asm_aarch64_system_registers_compile`, and interrupt/AP canaries pass `--both`.

- **SAMPLE-CORPUS.** (covers **CANARY-CORPUS**, **SQUALR-HEADLESS**, **MACOS-APPLICATION-PUBLICATION**) Close `samples/`, `tests/omega/` (`corpus_runner.rs`), [Squalr](samples/apps/README.md), and macOS `.app` bundles ([macos_application.md](wiki/spec/build/macos_application.md)) under the [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  Acceptance: all samples, corpus tiers (zero `regressed-from:` rows), `squalr/tools/verify.py native`, and `window_demo` `.app` pass on matching hosts.

## P1 - Authority, roots, and entry

<a id="process-exit-contract"></a>

- **ENTRY-CONTENT-ROOTS.** (covers **PROCESS-EXIT-CONTRACT**) Complete provisioned receivers ([entry_roots.md](wiki/spec/build/entry_roots.md)) and [process exit](wiki/spec/language/process_exit.md) (`boundary_calls.rs`, `hosted_receiver.rs`, `std/process.omg`), deleting `core/binding.omg`.
  Acceptance: `samples/cli/basics/{number_guess,cli_mvp,generic_counters}`, `pass/capabilities/*`, and `pass/host/process_exit_*` pass `omega run --both`.

## P2 - Materialization and access policies

- **ADDRESS-EXPOSURE.** (covers **DEFERRED-RECAST-ESTABLISHMENT**, **DEVICE-EXTENT-ACCESS**, **SYMBOLIC-MATERIALIZATION**, **NATIVE-CODE-LAYOUT-QUERIES**, **ASSUMED-BINDING-CONTRACTS**) Complete [storage-cast windows](wiki/spec/layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts), [device access](wiki/spec/resources/device_access.md) (`device_extent_access`), `post_handoff_writer/mod.rs`, [`address_of`/`code_layout`](wiki/spec/language/counts_and_addresses.md#address-exposure), and [`assume_binding_contract`](wiki/spec/build/assumed_binding_contracts.md).
  Acceptance: recast establishment, `device_extent_access`, post-handoff writers, `address_of`/`code_layout`, and `assumed_binding_*` pass `omega run --both`.

## P3 - Terminal Psi, proofs, and observation

- **STRUCTURAL-BORROW-IDENTITY.** (covers **WRITE-ONLY-BORROW**, **PLACED-ACCESS-NATIVE-OPS**, **GENERIC-FIXED-ARRAY-VIEWS**, **BORROW-PROOF-CONVERGENCE**, **TERMINAL-SLICE-VIEW-VOCABULARY**) Complete [`&write T` and structural borrows](wiki/spec/terminal-psi/structural_access.md), [loans](wiki/spec/terminal-psi/loans.md), and [slice views](wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views) (`view_subslice.rs`).
  Acceptance: `write_only`, `computed_nested_state_loan`, [reference_recast_backing_footprint](samples/cli/facts/reference_recast_backing_footprint/README.md), `disjoint_mut_subslice_windows_compile`, and `fletcher_checksum` pass `--both`.

<a id="normalized-abi-lowering"></a>

- **NORMALIZED-ABI-LOWERING.** (covers **OPAQUE-BY-VALUE-BOUNDARY-ABI**, **FOREIGN-RETAINED-ARGUMENT-BACKING**) Complete [calling plans](wiki/spec/build/calling_plans.md) and [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody) (`boundary_signatures.rs`, `retained_borrow_custody.rs`).
  Acceptance: `samples/cli/facts/{opaque_closed_carrier_exchange,foreign_retained_*,foreign_nested_record_backing}` and `providers/foreign_*` pass `omega run --both`.

- **PROOF-KERNEL-CORE.** (covers **PROOF-CONTRACT-MIGRATION**, **QUOTIENT-THEOREM-LIFT**) Complete [proof checking](wiki/spec/proofs/foundation.md) and [quotients](wiki/spec/proofs/quotients.md) in `mathematical_core/` and `proof_surface.rs`.
  Acceptance: `samples/cli/facts/{closed_integer_bound,negated_mathematical_order,source_indexed_*,erased_reference_contract}` and `proofs/quotient_*` pass `--both`.

## Build-level behavior exclusions

- **BUILD-SEMANTIC-EXCLUSIONS.** Deliver the [behavior exclusion contract](wiki/spec/build/behavior_exclusions.md) in `behavior_exclusions.rs`, `exclusions.rs`, `retained_terminal.rs`, and `realization.rs`.
  Acceptance: `tests/omega/{pass,fail}/build/{crash_exclusion_*,*service_exclusion*}` pass `python3 tools/corpus_gate.py --both` across all hosts.

## Requirement-based tests

- **BUILD-TEST-GROUPS.** Implement the [testing contract](wiki/spec/build/testing.md) and [chapter 23](wiki/language_guide/chapter_23_testing.md) in `prelude.rs`, `configuration.rs`, `program.rs`, and `restricted_build_grants.rs`.
  Acceptance: pure and virtual-filesystem test groups execute on both engines while application outputs exclude test-only roots.

## Checked boundary topology

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver the [checked boundary topology contract](wiki/spec/packages/topology.md) and [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization).
  Acceptance: the topology package verifies `payments.plan` and runs the three-process customer on Windows and macOS.

## Embedding and interpreted components

- **EMBEDDING-SOURCE-TO-HOST.** (covers **INTERPRETED-CATHEDRAL**) Implement [embedding](wiki/spec/build/embedding.md) (`embedding/{program,instance,adapters}.rs`, `composition_modes.rs`) and run Cathedral startup/events/replacement in `terminal-interpreter`.
  Acceptance: embedding host-import/fuel-pause/replacement fixtures and Cathedral's interpreted startup/event/replacement sequence pass.

## Parallel language and compiler lanes

- **COMPONENT-SUBSTRATE.** (covers **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**, **RUNTIME-SIZED-ACTIVATION-CONTRACT**, **FILESYSTEM-PLATFORM-SERVICES**, **TR3-TR8**, **CHECKED-REPRESENTATION-REUSE**) Complete [authority review](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission), [bounded activation](wiki/spec/resources/activation_storage.md), [filesystem services](wiki/spec/build/platform_services.md), [component installation](wiki/spec/build/component_publication.md), [task runtime](wiki/spec/build/task_runtime.md) (`blocking-executor`), and [checked reuse](wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse).
  Acceptance: `console_exit_process_exit` (warm `< 50%` cold), `bounded_activation`, Windows filesystem fixtures, `Independent` builds, and `blocking-executor` pass `--both`.

## Connected scientific library

- **W100-CONNECTED.** Complete `source/library/core/{matrix_layout,float_accuracy,complex,linear_algebra}.omg` and `samples/cli/scientific/w100/` through `omega run --both`.
  Acceptance: those core modules and `samples/cli/scientific/w100/` execute via `omega run --both` against frozen mathematical oracles.

## P5 - Cathedral over general Omega primitives

- **EXTENT-LIBRARY-TRANSFORMS.** (covers **CONSERVATION-CONTRACT**, **TERMINAL-CONTENT-CLAIMS**, **BOUNDARY-ISSUANCE**, **BUMP-ALLOCATOR-CANARY**, **UEFI-PHYSICAL-SEMANTIC-ENTRY**, **AP-BRINGUP**, **ADDRESS-TRANSLATION-CANARY**, **EXCEPTION-ROOTS-AND-TIMER**) Complete [content custody](wiki/spec/resources/content_custody.md), `alloc/{bump,vec}.omg` ([typed_runs.md](wiki/spec/resources/typed_runs.md)), [UEFI handoff](wiki/spec/build/uefi_entry.md), and Cathedral AP/MMU/interrupt canaries.
  Acceptance: `core/content_*`, `typed_run`, `bump_allocator_canary`, `uefi_os_handoff_invocation`, and `secondary_processor`/`address_translation`/`interrupt_table` canaries pass.

## Rust compiler release closure

- **RC-RELEASE-RECORD-AND-CLOSURE.** (covers **RC-BUILD-AND-PACKAGES**, **MACOS-X64-HOST-PROFILE**) Produce the 4-host release record via `tools/release/release_record.py` under [rust_compiler_completion.md](wiki/drafts/reference/rust_compiler_completion.md).
  Acceptance: `fmt.py --check`, Clippy, nextest, `RC-NATIVE-MATRIX`, `RC-BUILD-AND-PACKAGES`, and `sample_native_gate.py` pass across all 4 hosts.

## Omega-written compiler (after Rust completion)

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Finish `source/psi/` and `source/omega/{build,main}.omg` under [compiler_request.md](wiki/spec/build/compiler_request.md) and [source/psi/README.md](source/psi/README.md).
  Acceptance: `source/omega/{build,main}.omg` passes `tests/product/request/test-request.sh`, `tests/psi/*`, and Rust/Omega differential checks on all hosts.
