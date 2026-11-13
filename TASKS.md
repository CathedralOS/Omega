# Tasks

Use [advance](.agents/skills/advance/SKILL.md) and [sample loop](AGENTS.md#the-sample-loop) (`omega run --both`).

The fixture corpus under `tests/omega/` was deleted on the owner's ruling of 2026-11-13 ([tests/README.md](tests/README.md#the-former-corpus)). An acceptance below that names a fixture or a fixture family (`runtime_dutch_flag_partition_exit`, `pass/wire/*_exit`, `modules/`) names programs of that corpus: the ones kept are under `tests/fixtures/` at the same `<tier>/<group>/<name>`, and the rest are read from history. Such an acceptance is met by the named programs passing `omega run --both`, restored beside the tests that prove the item; there is no corpus run and no ledger to consult.

### Development-health outcomes

- **ITERATION-PERFORMANCE.** Restore a usable edit/build/run workflow when a bottleneck measurably obstructs development.
- **VALIDATION-ENVIRONMENT.** Restore bounded test selection or harness feedback while preserving runtime oracles.

Write each item so any agent can start from it alone: current state, files and entry points, the next step, and how to know it is done. Delete completed bullets. [Ownership](omega-rust/pipeline.md): Psi owns source through Terminal Psi; Omega owns realization.

## Priority (human-set only)

- **LOWERED-FORM-CHECKER.** Outranks every item below (owner ruling, 2026-11-12). The [lowered-form checker](wiki/drafts/designs/check_on_lowered_form.md) lowers first and proves on unsealed Terminal Psi. The owner ruled on 2026-11-12 that the old checker (Psi 05, validation and Psi 06, about 790k lines) is removed now, not once every program is covered: none of the obligations it accepts is backed by evidence the proof kernel re-checked, so a program built on its acceptance was never a checked program. A program the lowered-form checker does not govern yet stops compiling until it does. Read that record's "The decision", "The gate" and "Lane order" first.
  - Where it stands: the router (`omega-rust/omega/compilation/compiler/src/checked/checking/lowered_route.rs`) takes the route's verdict as the only verdict of a product compilation. A program the route cannot judge is refused with a "not supported yet" error that names the construct and is never given to Psi 05. Tests: `compiler/tests/lowered_route_{subjects,generics,governance,publication,messages}.rs`.
  - Freeze: no pull request edits `typed-trees-to-checked-trees` (Psi 05), `validation`, `checked-trees-to-lowered-psi` (Psi 06), `checked-trees`, `facts` or `representation-{planning,selections}` except to delete code or move a still-needed item to the crate that survives.
  - Step 1, product route: implement the route-owned provider, external realization and calling-plan custody required by target selection and package review. Use the accepted-source factory's exact artifact, evidence and typed source owner; unsupported authority requests refuse with "no product route yet". Complete bound entry contracts and state declaration identities for stage-04 generated machines. Publish the boundary and external realization declarations the actual retained provider demand requires; the default scalar consumer and a Check-mode proposal request have differed on unused float-table rows, so an all-catalog request does not substitute for the default path. Done when `tests/fixtures/run/calls/scalar_contract_route_exit` runs from the default compile and route-accepted programs pass `omega run --both` with the old stages absent.
  - Step 3, coverage: the progress number is the count of programs the route refuses (`not supported yet`); nothing measures it now that the corpus and its ledger are gone. Take the next refusal from the existing table, build it, prove it with the crate's unit tests and two or three named probes, and land; do not measure first. Lanes in order of programs unblocked: the Build program and the bound target's entry-contract source (the route lowers neither), owned non-`[copy]` data and affine locals, the rest of generics, calls and callee summaries. Complete the installed continuation's logical contracts from its bound entry source.
  - Build program: Build admission (`omega-rust/omega/build-system/build-settings/src/admission/checking.rs`) prepares a snapshot of the typed frontier and has the route judge the Build roster (`structure_elaboration::Compilation` with `purpose: Build`) under invocation publication; the activation runs what that publishes. What still stops `omega --check tests/fixtures/run/calls/scalar_contract_route_exit/main.omg`, in the order it is met: (1) `build_time_evaluation::admit_toolchain_build_activation` refuses every Build activation (`no product route yet: source Build activation admission`): the rule that source cannot construct or store a `Build` value needs an owner on the route; (2) behind it and not yet reached by the witness, Terminal installation has no declaration-selection admission for a Build activation, because `admit_compiler_evaluation` also demands empty service reach, so a request with package authority is refused after judgment. Build selections: a root binding is read and published with its host rows (`psi-terminal-production/src/build_companions.rs`); provider readiness (`Build::await_provider`) and native bindings are refused by name in the reader; every host row names its call by machine, operation and boundary, and the interpreter's boundary-call effect carries the machine, so helper machines of a Build script each keep their own rows. Constants reached by a Build program are refused by the route until `PreCheckEvaluation` evaluates jobs.
  - Rule: build every construct in the route. Never weaken the route to keep a program compiling; a missing verdict is a refusal. Pushed wip worth landing first where it stays out of the frozen stages: single commits on `work/compiler-typed-library-slices` (subject naming) and `work/psi-effect-memory` (bounded planning window).
  - Probe: to show the route one source program without running its Build and without Psi 05, validation or Psi 06, run `mbx run -p compiler --example route_probe -- tests/fixtures/run/calls/scalar_contract_route_exit/main.omg < /dev/null` (any `main.omg`; `--entry NAME`, `--target NAME`). It prints each judged part and `route=accepted|rejected|incomplete|not-supported|no-typed-trees`. It is the way to reach the route while no Build program can run; its limits are in `lowered-discharge/README.md`, "Running the checker on a source program", and `compiler/tests/route_probe.rs` pins it. The probe prints `route=accepted` on that witness again, and `compiler/tests/route_probe.rs` passes with its rejected twin. How it got there: source activation works again and the bundled library resolves: name resolution folds closed scalar constant initializers itself (`02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_initializers.rs`). Psi 03 types universe-sort applications, literal constant arguments, self-storing generic data, domain arguments of applied generic data and per-target machine families itself. Typing no longer stops a program for a call it cannot bind or a literal it cannot land in a declaration the program never reaches: it records the refusal on that declaration (`typed-trees/src/deferred_refusals.rs`) and structural elaboration raises it by name when the declaration is demanded (an entry's closure, a Build selection, or any declaration of a package checked as a subject). The recorded requests are the selection pass's work: calls selected by a value receiver's type (`typing_queries.rs`, `defer_unbound_calls`) and literals assigned to a field, an element or a reference binding (`call_and_product_selections.rs`, `assigned_binding_type`). The same holds for a constructor whose field types typing cannot select (generic data applied at arguments, before any specialization) and for the evaluation requests written in a declaration's types (layout-policy storage, a type written as an expression, a computed array extent, a materialized expression: `build-time-evaluation/src/source_frontier.rs`, `defer_source_jobs`). Root bindings are resolved again (`psi-compiler/src/root_bindings.rs`), and the layout-policy step before Psi 04 records its requests on their declarations like the others. What is recorded and not served is the next work: every declaration with a row is refused where a program reaches it. Still open: a constant whose initializer calls a machine has no unevaluated carrier in complete resolution (it is refused as a non-literal). Psi 04 is one ordered pass list every program crosses (`04_typed-trees-to-typed-trees/README.md`); it has its first annotation pass, record patterns, and still lacks the specialization and selection passes, whose requests structural elaboration refuses by name where a program demands them.
  Acceptance: the old crates are deleted, no program is refused as not supported, and the kept fixtures' tests and `samples/` pass on the route alone.

- **NESTED-MACHINE-UNIFICATION.** Replace `state` with nested machines ([machines.md](wiki/spec/language/machines.md#nested-machines-and-identity), [state_contracts.md](wiki/spec/language/state_contracts.md)).
  - Resolve recursive scopes, elaborate flat identities, and reject runtime capture or code after trailing declarations.
  - Complete [local inference](wiki/drafts/designs/local_inference_and_contracts.md), cyclic arrivals, normal returns, loans, restoration, linear custody, and SCC rank measures across call shapes.
  - Complete the declaration-model migration across linked repositories, samples, the kept fixtures, and both compilers.
  Acceptance: 3-level nesting, helper calls, cycles, and 9-place rows pass `omega run --both`; `state` syntax is deleted.

- **CANONICALIZE-SCALAR-RANGE-CONTRACTS.** Complete [scalar range contracts](wiki/spec/language/domains.md#declaration-and-membership) and remove `T [lo..hi]` / `T [lo..=hi]`.
  - Prove field/case bounds on construction (`construction_predicates.rs`), writes, field-loan returns, and array updates.
  - Check and lower scalar-field loans across case fields, recursive records/references, calls, joins, and [recasts](omega-rust/psi/compilation/validation/recasts.md).
  - Migrate all customers ([range_suffix_migration.md](wiki/drafts/designs/range_suffix_migration.md)) and qualify filesystem oracles with **FILESYSTEM-PLATFORM-SERVICES**.
  Acceptance: all sources pass `omega run --both`; suffix parsing (`parse_type.rs`) and `TypeConstraintNode::Range` are deleted.

- **STATIC-MACHINE-MIGRATION.** (covers **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION**, **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION**) Finish static machines, implementations, `dropped by` and [callback reach](wiki/drafts/designs/deferred_callback_reach.md).
  - Finish const-expression closure, package projection, calling policies and `source/psi/` evidence-binder parity.
  - Qualify `positional_implementation_binder`/`open_const_implementation`/`open_record_const_implementation`/`named_conformance_dispatch`/`generic_calling_policy`/`domain_selected_cleanup` on all hosts.
  - Move Build facets to `Build::*`, retire `machine Data::operation`/attached helpers; finish suspension loans, markers and conditional carry.
  Acceptance: 9-place `dropped by`, `generic_counters`, clock/GUI, `traits/,layouts/,progress/` and callback controls pass `--both`.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** (covers **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION**) Replace `dyn` with [machine values and some](wiki/drafts/designs/machine_values_some.md).
  - Complete callable contracts, package openings, loans, envelopes, indirect lowering and codec migration on all hosts.
  Acceptance: migrated `traits/` and package controls pass `--both` on all hosts; replacement edges are checked.

- **OWNING-SOME-PACKAGES.** Admit owned hidden state under `some` through uniform carriers ([design](wiki/drafts/designs/owning_some_packages.md)); after **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**.
  - Inductive uniformity over fields and stored-callable parameters/results; opened witnesses as construction witnesses; linear packages with by-value consuming operations; one owner family in `alloc` as the first customer.
  Acceptance: two factories into one container, wrong-pool return, captured borrow, restricted crossing, pending-settlement repack and bare-`S` result rejection pass `--both`.

- **OPERATION-RECORD-IMPLEMENTATIONS.** Make implementations ordinary checked records of callable selections and evidence ([design](wiki/drafts/designs/operation_record_implementations.md)).
  - Finish callable selections, typed/erased fields, constraints, open/closed contracts and static/runtime materialization.
  - Unify identity for instantiated defaults, generic constructors, call static arguments. Complete general evidence citation, explicit inherited fields and computed Build providers.
  Acceptance: the design's cases pass `--both`; reusing a machine needs no forwarding member.

- **OWNED-ACTIVATIONS.** Expose activation execution custody with typed replies ([design](wiki/drafts/designs/owned_activations.md)), after the record model.
  - Foundation and core generator, then suspension leases and pause-protocol/per-step progress contracts; rebuild tasks on it.
  Acceptance: a copying generator and a scheduler run as libraries over stepping under `--both`.

- **RUNTIME-PLATFORM-PROVIDERS.** Deliver authorized task/lifecycle and interrupt save/restore packages over exact primitives ([acceptance](wiki/drafts/designs/runtime_provider_authority.md#acceptance)); fake-intrinsic rejection alone is not delivery.

- **BUILD-OWNED-TARGET-SELECTION.** Retire target-prefixed machines for target-keyed Build rows ([provider_selection.md](wiki/spec/build/provider_selection.md#build-owned-target-selection)).
  - Delete source target modifiers/family codecs and migrate `core/targets`, `std/targets`, `source/`, `samples/cli/facts/placed_stable_write/target_body` (a selected provider's placed `Access` write rejects natively: a formal pointer is not backing authority), `audio_playback`, and target fixtures.
  - Transport requirement/implementation identities, lifetimes, const/evidence telescopes, callbacks, and `invokes` ceilings.
  Acceptance: one portable call selects per-target implementations from one Terminal artifact under `omega run --both`.

- **TARGET-REALIZED-FLOAT-ARITHMETIC.** Realize canonical `f32`/`f64` operations from the selected target's declared table ([design](wiki/drafts/designs/target_realized_float_arithmetic.md)); retire their Build provider selection.
  - Add the checked-machine realization entry kind beside catalog intrinsic and unsupported in `target::CanonicalFloatRealization`; profile remaining `select_provider` cost inside this item.
  Acceptance: float/FMA/conversion fixtures keep their documented behavior under `--both`; reports name each realization with its ISA/control admissions.

- **BUILD-OWNED-NATIVE-BINDINGS.** Replace `via` with [Build-owned bindings](wiki/spec/build/foreign_bindings.md#build-owned-native-binding).
  - Finish exact catalog custody and reject false authored backing ([controls](wiki/drafts/designs/runtime_provider_authority.md)); migrate table/vtable, user-intrinsic, generic-trait and Windows `ProcessExit`. Keep executable-provider delivery separate; repair [UEFI/table custody](wiki/spec/build/foreign_bindings.md#migration-acceptance).
  Acceptance: PlaySoundW, ALSA, and AudioQueue bindings pass `omega run --both` on matching hosts, and `via` is removed.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Finish retiring standalone `boundary requirement` declarations ([machines.md](wiki/spec/language/machines.md#supply)).
  - Run the Windows GUI customers (`runtime_{gui,windows}_*`, `window_demo`) via `omega run --both` on a Windows host.
  Acceptance: Windows GUI customers pass `omega run --both` under boundary-trait selection while bad providers reject.

- **OPERATOR-MACHINE-SUPPLY.** Run declared operations/premises; implement [identity](wiki/drafts/designs/index_identity.md) and [integer arithmetic](wiki/drafts/designs/computational_and_mathematical_integers.md).
  - Separate fixed identity from budgeted algebra proofs; carry generic evidence through specialization.
  - Implement [operator-independent domain composition and exact result keys](wiki/spec/language/domains.md#domain-composition-and-operators), including its selection and artifact acceptance controls.
  - Implement `i`/`u`, exact math `/`, quotient/remainder and checked bridges; migrate existing integer-proof fixtures under the design's controls.
  - Retire `operator` ([inventory](wiki/drafts/audits/operator_introducer_retirement_inventory.md)); lower `f64 in D` / `i32 in D`.
  Acceptance: `declared_operator_match_result`, `body_bearing_comparison`, `generic_comparison_body`, `open_index_distributivity` and landing controls pass `--both`.

## Pipeline route

- **PIPELINE-ARCHITECTURE-ENFORCEMENT.** Enforce the single-pass Psi (`00–07`) + Build + per-target Omega (`00–09`) [pipeline](omega-rust/pipeline.md) in `target_batch.rs`.
  - Admit dependency contexts ([scoped_execution.md](wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse)), run `build.omg` and Psi `00–07` once, clear `FIREWALL_ALLOWANCES` in `tools/check_native_boundary.py`, store recursive terms as flat tables in memory ([pipeline](omega-rust/pipeline.md)), and drop 256 MiB stacks (source depth is bounded only by memory and time, per [compiler_request.md](wiki/spec/build/compiler_request.md)).
  Acceptance: two-target builds run Psi `00–07` and Build once and Omega `00–09` once per target with clean boundary guards.

- **VALIDATION-SUBJECTS.** Implement [validation subjects](wiki/drafts/designs/validation_subjects.md): check the selected packages in full and dependencies only through their required semantic closure.
  - Add reusable activation-scoped registrations, Build/Product subjects and direct library checks with preserved owners.
  - Carry the subject set through orchestration, discover dependencies while checking (types, data, proofs, cleanup, conformances, bindings), keep deferred bodies in name resolution; add `Build::validate`, the all-members workspace default, and full validation of the admitted revision.
  Acceptance: draft cases pass; app checking excludes unrelated library bodies and lowers measured Psi 05 work.

- **ORDINARY-LIBRARY-PACKAGES.** Remove mandatory/pinned `std` and hidden entry coupling ([acceptance](wiki/drafts/designs/validation_subjects.md#ordinary-library-migration)); preserve primitive custody and release integrity. Independent of validation delivery.

- **LIBRARY-RESPONSIBILITY-NAMES.** Rename `std` to `system` and `alloc` to `allocation` under the [responsibility boundaries](wiki/spec/build/declarations.md#library-responsibilities-and-names), including declarations, paths, imports, Build registrations, exact sealed references, fixtures, samples and docs. Preserve contracts, owner/provenance checks and narrow primitive custody; migrate known hosted, UEFI handoff and explicit-storage allocation customers without weakening their oracles. Renamed lookalikes gain no privilege. No required collections extraction, container rename or generic element support; independent compiler-decoupling acceptance stays with ORDINARY-LIBRARY-PACKAGES.

- **PROVIDER-SELECTION-AFTER-TERMINAL.** (covers **OPERATOR-BOUNDARY-CALLS**, **TARGET-SEMANTIC-APPLICATIONS**) Settle providers over Terminal Psi in `execution_settlement.rs`.
  - Move `execution_settlement.rs`, `provider_binding.rs`, `operators.rs`, opaque/const/review paths, float/FMA/GUI calls, and target capsules to Terminal Psi.
  Acceptance: `FIREWALL_ALLOWANCES` in `tools/xtask/src/boundary.rs` has no `provider-planning` or `selected-dispatch` rows, and `mbx xtask check-boundary` passes.

## Lowering, checker, and data-flow

- **OPERATION-ARITHMETIC-POLICY.** Implement [policy operators](wiki/drafts/designs/operation_arithmetic_policy.md).
  Acceptance: draft fixtures pass `--both`; `source/psi` has no `in Trapping`.

- **NAMED-DOMAIN-CASTS.** Implement [casts](wiki/drafts/designs/named_domain_casts.md).

- **REQUIRED-AUTOMATIC-DISCHARGE.** Meet the [proof floor](wiki/drafts/designs/required_automatic_discharge.md) in both checkers; add `--floor-only`.

- **PUBLISHED-WRITE-FRAMES.** Publish inferred [write frames](wiki/drafts/designs/published_write_frames.md) in interface identity.

- **RECORD-OR-SUM-DATA.** Implement [record or sum data](wiki/drafts/designs/record_or_sum_data.md) with record decomposition and `let Pattern = expr;`; migrate samples and tests.

- **REQUIRED-ISSUANCE-CHOICE.** Implement [required issuance](wiki/drafts/designs/required_issuance_choice.md) and its review rows.

- **BOUNDARY-SLOT-VIEWS.** Implement [slot views](wiki/drafts/designs/boundary_slot_implementation_views.md) with the two-slot clock fixture.

- **INVOKES-ENTRY-CEILING.** Implement the [entry ceiling](wiki/drafts/designs/invokes_entry_ceiling.md); then strip plumbing rows.

- **TYPED-STATIC-PARAMETERS.** Implement and migrate to [typed static parameters](wiki/drafts/designs/typed_static_parameters.md).

- **ACTIVATION-STORAGE-PLANS.** Implement [storage plans](wiki/drafts/designs/activation_storage_plans.md).

- **OWNED-CLEANUP.** Implement [by-value cleanup](wiki/drafts/designs/owned_cleanup.md).

- **OPEN-CALLABLE-CONTRACTS.** Close [callable contracts](wiki/drafts/designs/open_callable_contracts.md) at runtime and boundary positions.

- **STATIC-BINDER-CONTRACT-DEPENDENCIES.** Implement [static binder contract dependencies](wiki/drafts/designs/static_binder_contract_dependencies.md): one published dependency term per contract axis for static callbacks, substituted from selected published contracts.
  - Unify the `service_reach.rs` binder terms and `operational.rs` selector contexts into one term representation; publish and import it through Psi rows; forward through renamed binders; converge recursive propagation.
  - Add the crash row (public omission means none, private inference, requirement deferral, `crashes none` enforcement, per-cause routes) and the termination row (binder premises on authored `terminates`).
  - Retire the bare `reaches` keyword in both parsers; sweep `tests/fixtures`, `samples`, `tests/psi/parser` and `tests/psi/resolver` to `reaches none`.
  - Add the [result-relation row](wiki/spec/language/effects.md#callback-result-relations): opaque relation per checked call, authored `F::ensures(args, r)` claims discharged from it, selected relation substituted at specialization; immutable arguments first.
  Acceptance: `forward`/`compose`/`choose` fixtures across reach, suspends, blocks, crashes, terminates and carry pass `--both` with negative twins; a dependency survives export, import, binder forwarding and substitution; `for_each` over a crash-free callback publishes no route and over a trapping one publishes `Trap`; the draft's result-relation fixtures pass.

- **CORE-INDEXING-CONTRACTS.** Implement [contract-gated core indexing](wiki/drafts/designs/core_indexing_contracts.md).
  Acceptance: generic sealed access proves whole contracts, retains exact Terminal evidence and passes both engines/negative controls; retire independent bounds admission.

- **LOC-VALUE-CUSTODY.** (covers **IMAGE-SELF-REPLAY-DELETED**) Carry value custody on one path per layer and qualify native execution on `linux_x86_64`.
  Acceptance: retired gates have checking reject twins under `tests/fixtures/fail/facts/`, and the kept runnable fixtures and `samples/` pass natively on `linux_x86_64`.

## Universal interpreter cutover

- **EARLY-TERMINAL-JOBS.** (covers **TERMINAL-LAYOUT-WIRE-EVALUATION**, **TERMINAL-BUILD-SEED**) Route [evaluation](wiki/spec/language/evaluation.md), [embedding](wiki/spec/build/embedding.md), and [scoped Build](wiki/spec/build/scoped_execution.md) through `TerminalProgram::load`.
  - Remove tree cloning in `build-time-evaluation` and run `build.omg` activation, queries, and `restricted_build_grants.rs` in the Terminal interpreter.
  - Run the jobs that execute a machine before checking on Terminal Psi: a `FixedLayout` or access policy's plan and a machine used as a constant domain fact. The tree interpreter that ran them is deleted, so `EvaluationAccounting::evaluate` in `build-time-evaluation` refuses each with "no product route yet".
  Acceptance: `pass/comptime/*_exit`, `pass/layouts/runtime_plan_laid_*_exit`, `pass/wire/*_exit`, and `pass/build/*_exit` pass `--both`.

- **UNIVERSAL-INTERPRETER-CLOSURE.** Finish `omega-rust/omega/interpreter/terminal-runtime/src/execution.rs`.
  - Add a `ForeignBinding::Syscall` adapter; witness `kernel_import_result_chain_exit` on `work/tests-kernel-import-process-exit`.
  Acceptance: audits find no tree visitor, and `omega run --both`, and `--engine native|terminal` pass on all hosts.

## Immediate product closure

- **REQUIRES-UNIFICATION.** Run `tests/psi/*/test-*.sh` over the `where` deletion; each gate's `omega update` peaks near 14 GB single-threaded.
  Acceptance: the parser, resolver, typing and checking gates pass.

- **STAGED-BUILD-CAPABILITIES.** Implement [checked Build stages](wiki/spec/build/scoped_execution.md#checked-build-stages) and attenuated filesystem delegation, retiring `BuildInputs`/`BuildSource`/`BuildOutput`.
  Acceptance: root/dependency Builds, generators, and attenuated filesystem providers pass `omega run --both` with legacy Build paths deleted.

- **LIBRARY-REPRESENTATION-FAMILIES.** Implement [representations](wiki/spec/layouts/plans.md#representation-applications), [aliases](wiki/spec/language/generics.md#transparent-type-aliases), [conversions](wiki/spec/layouts/codecs.md#representation-conversions), and [home layouts](wiki/drafts/designs/home_layout_optimization.md).
  Acceptance: 9-place alias, `FixedLayout<T>` ([region](samples/cli/facts/fixed_region_byte_roundtrip/README.md), [readback](samples/cli/facts/fixed_layout_reference_readback/README.md), [forwarding](samples/cli/facts/implementation_region_forwarding/README.md)), [agreement](samples/cli/facts/representation_agreement/README.md), niche/repair/recast controls pass `--both`.

- **STATE-LOCAL-VALUE-FRONTIER.** (covers **REFERENCE-LOCAL-BINDING-ROUTES**, **BORROWED-STORAGE-RESTORATION**, **MATCH-SELECTIVE-LOWERING**) Complete expression, reference-local, [borrowed window](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows) (`borrowed_windows.rs`), and [match](wiki/spec/language/patterns.md) (`match_dispatch.rs`, `address_joins.rs`) lowering.
  Acceptance: `scalar_tag_field_loan`, `recoverable_replacement_window`, `runtime_dutch_flag_partition_exit`, and `borrowed,window,carried_loan` pass `--both`.

- **MODULE-NAMESPACE-RESOLUTION.** (covers **RUNTIME-VALUE-GENERICS**, **STRUCTURAL-GENERIC-MATCHING**, **FINITE-GENERIC-DISPATCH**) Complete [module selection](wiki/spec/language/modules.md) and [generics](wiki/spec/language/generics.md) (`generic_arguments.rs`, `type_equations/`, `dynamic_families.rs`).
  Acceptance: `modules/`, `runtime_index_application`, `selected_value_application`, `inferred_array_equation_call`, and finite-family fixtures pass `--both`.

- **DOMAIN-ACCESS-AUTHORITY.** (covers **DOMAIN-CARRIER-FORMATION**, **NOMINAL-FIELD-FLOW**, **DOMAIN-ISSUER-ROUTES**) Complete [carrier membership](wiki/spec/language/domains.md#carrier-exact-membership), `[u8; N] in D`, [domain `access Policy`](wiki/spec/resources/access.md), and [repair facts](wiki/drafts/designs/invariant_repair_facts.md).
  - `access Policy` is read: the parser takes the clause, name resolution selects the one named implementation it names, and the typed trees carry it on the domain (`typed_trees::domain::DomainDefinition::issuance`, `compiler/tests/domain_access_policy.rs`). A policy's plan is not evaluated (**EARLY-TERMINAL-JOBS**), so such a domain's issuance is `Planned` and a reader must admit no issuer for it; a policy applied at static arguments (`IssuedBy<..>`) is a deferred refusal on its domain. Next, in order: account for `=D -D +D` in typing (`domain_cast_accounting::account` refuses every one), read `T in D` and judge `as +D` in the lowered-form checker (`structure-elaboration/src/read/{types,casts}.rs` refuse both by name), move core's `AccessPlan` to the spec's `AccessPlan::new(&schema, issuance)`, then migrate the `established by` declarations and delete that clause.
  Acceptance: [domain_carrier_identity](samples/cli/facts/domain_carrier_identity/README.md), Dungeon Crawler, [controlled cell](wiki/language_guide/chapter_8_domains.md#read-and-write-authority), `bodyless_owner_establishment` and `non_owning_reference_erasure` pass `--both`.

- **TERMINATION-RANKING-CHECKS.** (covers **GENERAL-CYCLIC-EXECUTION**, **CRASH-CONTRACT**, **OUTCOME-SPECIFIC-CALL-EVIDENCE**, **UNREACHABLE-BRANCH-DIAGNOSTICS**) Complete [termination](wiki/spec/language/termination.md) (`ranking_range/calls.rs`, `value_origins.rs`), crash/outcome evidence (`operator_crashes.rs`), and [unreachable warnings](wiki/spec/language/state_contracts.md#unreachable-branches).
  Acceptance: `termination/,control_flow/`, `progress_selector/`, `array_max`/`dot_product`/`horner_eval`, crash rows, and unreachable warnings pass `--both`.

- **ATOMIC-MEMORY-MODEL.** Complete atomic/fence semantics under [concurrency](wiki/spec/language/concurrency.md) and [atomic_operations.md](omega-rust/psi/foundation/language-core/atomic_operations.md) in `atomic_events.rs`.
  Acceptance: `atomic_global_order_operations` passes `omega run --both` on all hosts, and concurrent witnesses run with **TR3-TR8**.

<a id="checked-assembly-control-flow"></a>

- **CHECKED-ASM-LABEL-CONTRACTS.** Implement `asm machine`, block declarations, `align_of(value)`, port settlements (`provider_custody.rs`), and [interpreted assembly](wiki/spec/language/assembly.md#interpreted-execution).
  Acceptance: `samples/cli/facts/checked_asm_*`, `asm_runtime_port_msr_final_validation`, `asm_aarch64_system_registers_compile`, and interrupt/AP canaries pass `--both`.

- **SAMPLE-CORPUS.** (covers **CANARY-CORPUS**, **SQUALR-HEADLESS**, **MACOS-APPLICATION-PUBLICATION**) Close `samples/`, [Squalr](samples/apps/README.md), and macOS `.app` bundles ([macos_application.md](wiki/spec/build/macos_application.md)) under the [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  Acceptance: all samples, `squalr/tools/verify.py native`, and `window_demo` `.app` pass on matching hosts.

## P1 - Authority, roots, and entry

<a id="process-exit-contract"></a>

- **ENTRY-CONTENT-ROOTS.** (covers **PROCESS-EXIT-CONTRACT**) Complete provisioned receivers ([entry_roots.md](wiki/spec/build/entry_roots.md)) and [process exit](wiki/spec/language/process_exit.md) (`boundary_calls.rs`, `hosted_receiver.rs`, `std/process.omg`), deleting `core/binding.omg`.
  Acceptance: [selected-provider graphs](wiki/drafts/designs/selected_provider_cycle_graph.md#acceptance), `samples/cli/basics/{number_guess,cli_mvp,generic_counters}`, `pass/capabilities/*`, and `pass/host/process_exit_*` pass `omega run --both`.

## P2 - Materialization and access policies

- **ADDRESS-EXPOSURE.** (covers **DEFERRED-RECAST-ESTABLISHMENT**, **DEVICE-EXTENT-ACCESS**, **SYMBOLIC-MATERIALIZATION**, **NATIVE-CODE-LAYOUT-QUERIES**, **ASSUMED-BINDING-CONTRACTS**) Complete [storage-cast windows](wiki/spec/layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts), [device access](wiki/spec/resources/device_access.md) (`device_extent_access`), `post_handoff_writer/mod.rs`, [`address_of`/`code_layout`](wiki/spec/language/counts_and_addresses.md#address-exposure), and [`assume_binding_contract`](wiki/spec/build/assumed_binding_contracts.md).
  Acceptance: recast establishment, `device_extent_access`, post-handoff writers, `address_of`/`code_layout`, and `assumed_binding_*` pass `omega run --both`.

## P3 - Terminal Psi, proofs, and observation

- **STRUCTURAL-BORROW-IDENTITY.** (covers **WRITE-ONLY-BORROW**, **PLACED-ACCESS-NATIVE-OPS**, **GENERIC-FIXED-ARRAY-VIEWS**, **BORROW-PROOF-CONVERGENCE**, **TERMINAL-SLICE-VIEW-VOCABULARY**) Complete structural loans and slice views, including nested element-view copies.
  Acceptance: `write_only`, `computed_nested_state_loan`, [reference_recast_backing_footprint](samples/cli/facts/reference_recast_backing_footprint/README.md), `disjoint_mut_subslice_windows_compile`, and `fletcher_checksum` pass `--both`.

<a id="normalized-abi-lowering"></a>

- **NORMALIZED-ABI-LOWERING.** (covers **OPAQUE-BY-VALUE-BOUNDARY-ABI**, **FOREIGN-RETAINED-ARGUMENT-BACKING**) Complete [calling plans](wiki/spec/build/calling_plans.md) and [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody) (`boundary_signatures.rs`, `retained_borrow_custody.rs`).
  Acceptance: `samples/cli/facts/{opaque_closed_carrier_exchange,foreign_retained_*,foreign_nested_record_backing}` and `providers/foreign_*` pass `omega run --both`.

- **PROOF-KERNEL-CORE.** (covers **PROOF-CONTRACT-MIGRATION**, **QUOTIENT-THEOREM-LIFT**, **PSI-CLAIM-ADMISSION**) Complete [source proofs](wiki/spec/proofs/foundation.md) and [Psi preparation](wiki/drafts/designs/psi_claim_admission.md). Emit code 36.
  Acceptance: [kernel/admission/performance](wiki/drafts/designs/psi_claim_admission.md#acceptance), [borrowed reads](wiki/drafts/designs/borrowed_proof_inputs.md#acceptance) and existing proof samples through `--both`.

## Build-level behavior exclusions

- **BUILD-SEMANTIC-EXCLUSIONS.** Deliver the [behavior exclusion contract](wiki/spec/build/behavior_exclusions.md) in `behavior_exclusions.rs`, `exclusions.rs`, `retained_terminal.rs`, and `realization.rs`.
  Acceptance: `tests/fixtures/{pass,fail}/build/{crash_exclusion_*,*service_exclusion*}` pass `omega run --both`, the `fail` twins rejecting with their pinned diagnostic, across all hosts.

## Requirement-based tests

- **TEST-GROUP-FILESYSTEM-TARGETS.** Supply the [test runner's private filesystem](wiki/spec/build/testing.md#group-configuration-and-authority) beyond the Linux kernel calls: the interpreter answers only `linux_x86_64`/`linux_arm64` kernel filesystem calls and refuses `canonicalize` (libc `realpath` and its `errno` copy), and the native runner needs Linux user namespaces, so macOS and Windows targets and hosts report the engine unavailable.
  Acceptance: `run/testing/private_filesystem_group_runs_on_both_engines` binds all four targets and passes on both engines on every host, including a test that calls `canonicalize`.

## Checked boundary topology

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver the [checked boundary topology contract](wiki/spec/packages/topology.md) and [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization).
  Acceptance: the topology package verifies `payments.plan` and runs the three-process customer on Windows and macOS.

## Embedding and interpreted components

- **EMBEDDING-SOURCE-TO-HOST.** (covers **INTERPRETED-CATHEDRAL**) Finish [embedding](wiki/spec/build/embedding.md), evidence-bearing qualified host inputs, and interpreted Cathedral.
  Acceptance: embedding host-import/work-pause/replacement fixtures and Cathedral's interpreted startup/event/replacement sequence pass.

## Parallel language and compiler lanes

- **COMPONENT-SUBSTRATE.** (covers **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**, **RUNTIME-SIZED-ACTIVATION-CONTRACT**, **FILESYSTEM-PLATFORM-SERVICES**, **TR3-TR8**, **CHECKED-REPRESENTATION-REUSE**) Complete [authority review](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission), [bounded activation](wiki/spec/resources/activation_storage.md), [filesystem services](wiki/spec/build/platform_services.md), [component installation](wiki/spec/build/component_publication.md), [task runtime](wiki/spec/build/task_runtime.md) (`blocking-executor`), and [checked reuse](wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse).
  Acceptance: `console_exit_process_exit` (warm `< 50%` cold), `bounded_activation`, Windows filesystem fixtures, `Independent` builds, and `blocking-executor` pass `--both`.

## P5 - Cathedral over general Omega primitives

- **EXTENT-LIBRARY-TRANSFORMS.** (covers **CONSERVATION-CONTRACT**, **TERMINAL-CONTENT-CLAIMS**, **BOUNDARY-ISSUANCE**, **BUMP-ALLOCATOR-CANARY**, **UEFI-PHYSICAL-SEMANTIC-ENTRY**, **AP-BRINGUP**, **ADDRESS-TRANSLATION-CANARY**, **EXCEPTION-ROOTS-AND-TIMER**) Complete [extent/run contracts](wiki/spec/resources/typed_runs.md#split-and-merge-contracts), core primitives, Vec/bump, UEFI handoff and Cathedral AP/MMU/interrupt canaries.
  Acceptance: [Vec integration](wiki/spec/resources/allocation.md#dense-collections-over-allocated-backing), extent/content and UEFI/AP/MMU/interrupt canaries pass.

## Rust compiler release closure

- **RC-RELEASE-RECORD-AND-CLOSURE.** (covers **RC-BUILD-AND-PACKAGES**, **MACOS-X64-HOST-PROFILE**) Deliver [4-host release](wiki/drafts/reference/rust_compiler_completion.md), per-root lock/closure/target/acceptance isolation and [CLI](wiki/spec/build/command_line.md).
  Acceptance: `fmt.py --check`, Clippy, nextest, `RC-NATIVE-MATRIX`/`RC-BUILD-AND-PACKAGES`/`sample_native_gate.py` and CLI migration controls pass on 4 hosts.

## Omega-written compiler (after Rust completion)

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Finish `source/psi/` and `source/omega/{build,main}.omg` under [compiler_request.md](wiki/spec/build/compiler_request.md) and [source/psi/README.md](source/psi/README.md).
  Acceptance: `source/omega/{build,main}.omg` passes `tests/product/request/test-request.sh`, `tests/psi/*`, and Rust/Omega differential checks on all hosts.
