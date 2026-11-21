# Tasks

Use the [sample loop](AGENTS.md#the-sample-loop) for a repair and the [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix) for release qualification. Start with the assigned program and its expected result. Delete a completed item when its outcome is verified.

Find Rust owners in the [compiler map](source-rust/README.md#where-code-belongs). The [experimental Rust plan](omega-rust-v2/PLAN.md) and [bootstrap board](TASKS_BOOTSTRAP.md) own their separate implementations. [Code repair concerns](OWNER_CODE_CONCERNS.md) track structural defects.

## Development-health outcomes

- **ITERATION-PERFORMANCE.** Reduce measured delays in the edit/build/run cycle.
  - After real-program bottlenecks, measure arena growth and copied nodes on repeated aggregate [constant substitutions](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/constants/substitution.rs). Preserve per-occurrence metadata when sharing definition content.
  - Measure ascending, descending, repeated and tiny sets of [specializations](source-rust/subsystems/compiler/source-to-psi/pipeline/04_typed-trees-to-prepared-typed-trees/passes/specialization.rs). Count movement in the sorted index before changing storage. Preserve application identity, encounter order and deduplication.
- **VALIDATION-ENVIRONMENT.** Restore missing test coverage on the affected host or service while preserving the original program and oracle.
  - Bind `FilesystemHost`, `TimeHost` and direct foreign services to the exact operation and owner. Keep insufficient or installation-dependent evidence a refusal.
- **CLAIMS-LEGACY-READ-REMOVAL.** After 2026-11-22, once the old claims ref has been quiet for two hours, remove its parallel read, ticket seeding and merged status from [claims](tools/xtask/src/claims.rs). Delete its two migration tests and the [migration instructions](wiki/development/claims.md#migration-from-the-coordination-ref).

## Priority (human-set only)

- **LOWERED-FORM-CHECKER.** Highest priority by owner ruling. Finish the [lowered-form checker](wiki/drafts/designs/check_on_lowered_form.md) for the accepted language. Every valid program must reach publication with its obligations proved, and an invalid program must be refused at the responsible obligation. [Checker entry](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/verdict.rs)

  - Check `source/library/std/main.omg` as a package on each hosted target, then run `mbx xtask sample-gate --filter cli/basics`. Follow the first unsupported construct through the ordinary Build and Product.
  - Run the [portable filesystem program](_recycling/tests/fixtures/pending/filesystem_portable_rules/main.omg) with a 4,200-file `omega_fs_rules_wide/` directory. Preserve its truncate, oversized-read, default-permission and recursive-directory checks. Its expected exit is 42 in both engines.
  - Complete ownership and loan facts for affine locals, mutable sequence backing, views and mixed-origin reference results. Check fixed-array lengths on call results, indexed values and case payloads, and reject out-of-extent indices at the access.
  - Check [domain predicates](wiki/spec/language/domains.md#byte-containers-and-encoding-domains) that call machines with requirements, type parameters or borrowed byte views. Prove ordinary `core::Identity<FloatMeaning>` clauses from the helper body, including `abs_f64` and `sqrt_f64`.

  - Cover full-package checking of generic declarations with lifetimes, bounds, cleanup, carry and computed extents. Preserve the admitted entry's logical contracts and the declaration that owns an unsupported obligation in diagnostics.

  - Carry the checked bounds of `size_of` and `align_of` through arithmetic on them and validate them at installation. Exercise [record geometry](samples/cli/probes/record_geometry), scalar and generic carriers, and the value form `align_of(view)`.

- **NESTED-MACHINE-UNIFICATION.** Finish checking calls and returns in nested machines ([machines.md](wiki/spec/language/machines.md#nested-machines-and-identity), [state_contracts.md](wiki/spec/language/state_contracts.md)).
  - Resolve recursive scopes, elaborate flat identities, and reject runtime capture or code after trailing declarations.
  - Complete [local inference](wiki/drafts/designs/local_inference_and_contracts.md), cyclic arrivals, normal returns, loans, restoration, linear custody, and SCC rank measures across call shapes.
  - Check the linked repositories and both compilers against the same declaration model. Preserve the original execution results of migrated programs.

- **CANONICALIZE-SCALAR-RANGE-CONTRACTS.** Complete [scalar range contracts](wiki/spec/language/domains.md#declaration-and-membership).
  - Prove field and case bounds at construction, on writes, on return of a field loan and after array updates.
  - Check and lower scalar-field loans across case fields, recursive records/references, calls, joins, and [recasts](wiki/spec/layouts/recasts.md).
  - Qualify migrated [range-contract customers](wiki/drafts/designs/range_suffix_migration.md) with their original results, including the filesystem programs under **FILESYSTEM-PLATFORM-SERVICES**.

- **STATIC-MACHINE-MIGRATION.** (covers **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION**, **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION**) Finish static machines, implementations, `dropped by` and [callback reach](wiki/drafts/designs/deferred_callback_reach.md).
  - Finish const-expression closure, package projection, calling policies and Omega-written frontend evidence-binder parity.
  - Evaluate a native requirement's declared calling policy, default an unannotated requirement to Omega's target convention and check the resulting plan against its locator. [Calling policy](wiki/spec/build/calling_plans.md#syscall-binding-calling-policy)
  - Migrate Linux filesystem and clock requirements to authored calling policies. Check `openat` on Linux x86-64 and arm64, its missing-policy refusal, an ordinary unannotated boundary and an explicit foreign import.
  - Remove the required foreign ABI tag from [CallPlan](source/library/std/calling.omg) and its [decoder](source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/src/calling_policy_plans/build_time_decoding.rs). Derive [plan identity](source-rust/subsystems/compiler/psi-to-native/targets/calling-conventions/src/plans/plan_identity.rs) from the normalized contract.
  - Move foreign aggregate classification, register placement, stack and clobber rules into Omega policies. Check their results against target primitives and the existing clang ABI comparisons.
  - Exercise an independently authored x86-64 policy and native callee. Passing 37 in `r10` and 5 on the stack must return 32. Assigning both live arguments to `r10` must reject. [Authorship contract](wiki/spec/build/calling_plans.md#authorship-and-evaluation)
  - Qualify `positional_implementation_binder`/`open_const_implementation`/`open_record_const_implementation`/`named_conformance_dispatch`/`generic_calling_policy`/`domain_selected_cleanup` on all hosts.
  - Move Build facets to `Build::*`, retire `machine Data::operation`/attached helpers; finish suspension loans, markers and conditional carry.
  - Migrate remaining attached machines in samples, targets and std fixtures with their original execution oracles.
  - Recover the authored `Count` and nominal callback contract in [package review](source-rust/subsystems/compiler/tests/fixtures/facts/pass/static_callback_reach_boundary_schema/main.omg). Preserve `Step<2>` and `Step<3>`, the unused effectful selection and the recovered schema. Migrate the free boundary machine under the [supply rules](wiki/spec/language/machines.md#supply).

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** (covers **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION**) Carry dynamic implementations through [machine values and `some` packages](wiki/drafts/designs/machine_values_some.md).
  - Complete callable contracts, package openings, loans, envelopes, indirect lowering and codec migration on all hosts.

- **OWNING-SOME-PACKAGES.** Admit owned hidden state under `some` through uniform carriers ([design](wiki/drafts/designs/owning_some_packages.md)); after **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**.
  - Inductive uniformity over fields and stored-callable parameters/results; opened witnesses as construction witnesses; linear packages with by-value consuming operations; one owner family in `alloc` as the first customer.

- **OPERATION-RECORD-IMPLEMENTATIONS.** Make implementations ordinary checked records of callable selections and evidence ([design](wiki/drafts/designs/operation_record_implementations.md)).
  - Finish callable selections, typed/erased fields, constraints, open/closed contracts and static/runtime materialization.
  - Unify identity for instantiated defaults, generic constructors, call static arguments. Complete general evidence citation, explicit inherited fields and computed Build providers.

- **OWNED-ACTIVATIONS.** Expose activation execution custody with typed replies ([design](wiki/drafts/designs/owned_activations.md)), after the record model.
  - Foundation and core generator, then suspension leases and pause-protocol/per-step progress contracts; rebuild tasks on it.

- **RUNTIME-PLATFORM-PROVIDERS.** Deliver authorized task/lifecycle and interrupt save/restore packages over exact primitives ([acceptance](wiki/drafts/designs/runtime_provider_authority.md#acceptance)); fake-intrinsic rejection alone is not delivery.
  - Bind firmware and a hosted process in one Build. Give only the firmware provider interrupt-mask authority. Firmware must admit the operation and the hosted process must refuse it. See [Build permissions](wiki/spec/build/permissions.md#where-the-operation-can-run).

- **BUILD-OWNED-TARGET-SELECTION.** Retire target-prefixed machines for target-keyed Build rows ([provider_selection.md](wiki/spec/build/provider_selection.md#build-owned-target-selection)).
  - Delete source target modifiers and their declaration/proof-graph family metadata; migrate `samples/cli/facts/placed_stable_write/target_body` (a selected provider's placed `Access` write rejects natively: a formal pointer is not backing authority), `audio_playback`, and target fixtures.
  - Transport requirement/implementation identities, lifetimes, const/evidence telescopes, callbacks, and `invokes` ceilings.

- **TARGET-REALIZED-FLOAT-ARITHMETIC.** Realize canonical `f32`/`f64` operations from the selected target's declared table ([design](wiki/drafts/designs/target_realized_float_arithmetic.md)); retire their Build provider selection.
  - Add checked-machine realization to [the target float table](source-rust/subsystems/compiler/psi-to-native/targets/target/src/float_realization.rs) and exercise it on a selected target. Measure remaining provider-selection cost separately.

- **BUILD-OWNED-NATIVE-BINDINGS.** Replace `via` with [Build-owned bindings](wiki/spec/build/foreign_bindings.md#build-owned-native-binding).
  - Finish exact catalog custody and reject false authored backing ([controls](wiki/drafts/designs/runtime_provider_authority.md)); migrate table/vtable, user-intrinsic, generic-trait and Windows `ProcessExit`. Keep executable-provider delivery separate; repair [UEFI/table custody](wiki/spec/build/foreign_bindings.md#migration-acceptance).
  - Implement [`FunctionFieldBinding`](wiki/spec/build/foreign_bindings.md#function-table-bindings) and `reflect::parameter` for the four `UefiBootServices` table slots, with `this` used only for dispatch. Supply sealed entries for handoff `transfer` and `firmware_return`.
  - Carry a closed `Registrar::register<TimerHandler::call>` requirement through capture, package rejoin, saved-input substitution checks and native callback execution.
  Acceptance: `via` is removed.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Qualify [machine-supplied boundary operations](wiki/spec/language/machines.md#supply) through the Windows GUI provider.
  - Run the [window demo](samples/gui/window_demo/README.md) on Windows with `omega run --both`. Both executions must draw 60 frames, release the window, print `PASS: 60 frames blitted (exit 0)` and exit successfully.

- **OPERATOR-MACHINE-SUPPLY.** Run declared operations/premises; implement [identity](wiki/drafts/designs/index_identity.md) and [integer arithmetic](wiki/drafts/designs/computational_and_mathematical_integers.md).
  - Separate fixed identity from budgeted algebra proofs; carry generic evidence through specialization.
  - Implement [operator-independent domain composition and exact result keys](wiki/spec/language/domains.md#domain-composition-and-operators), including its selection and artifact acceptance controls.
  - Implement `i`/`u`, exact math `/`, quotient/remainder and checked bridges; migrate existing integer-proof fixtures under the design's controls.
  - Remove the root `operator` declaration from the [Rust parser](source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees/declarations/items.rs) and its dependent representation once its source customers use machines. Lower `f64 in D` and `i32 in D` through their declared operators.

## Pipeline route

- **PIPELINE-ARCHITECTURE-ENFORCEMENT.** (covers **LOC-VALUE-CUSTODY**, **IMAGE-SELF-REPLAY-DELETED**) Check the [source and native pipelines](source-rust/subsystems/compiler/README.md) through the shipped compiler entry for multiple targets, multiple callable roots and independently produced components.
  - Trace value custody from checked Psi through native verification on this route. Qualify the resulting executable on `linux_x86_64`.
  - Verify Build effects execute once per admitted context and that matching targets share the checked source and portable program. Exercise differing grants, source inputs and execution profiles under the [dependency context rules](wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse).
  - Bound recursive term processing by available storage and retire the [large worker stack](source-rust/subsystems/compiler/foundation/worker-budget). Exercise deeply nested accepted source and diagnostics at ordinary stack sizes. [Depth requirement](wiki/spec/build/compiler_request.md)

- **VALIDATION-SUBJECTS.** Implement [validation subjects](wiki/drafts/designs/validation_subjects.md): check the selected packages in full and dependencies only through their required semantic closure.
  - Carry the subject set from [project planning](source-rust/subsystems/compiler/build/project_build.rs) into the [source driver](source-rust/subsystems/compiler/source-to-psi/driver/compile_product/judge_product_roster.rs). Check generated and unused declarations owned by a selected subject. Follow dependency bodies only as demanded by types, proofs, cleanup, bindings and conformances.
  - Complete `Build::validate`, all-members workspace selection and full validation of the admitted package revision with reusable, activation-scoped registrations.

- **ORDINARY-LIBRARY-PACKAGES.** Remove mandatory/pinned `std` and hidden entry coupling ([acceptance](wiki/drafts/designs/validation_subjects.md#ordinary-library-migration)); preserve primitive custody and release integrity. Independent of validation delivery.

- **LIBRARY-RESPONSIBILITY-NAMES.** Migrate `std` to `system` and `alloc` to `allocation` across imports, registrations, sealed references, fixtures and docs. Preserve exact primitive ownership and the hosted, UEFI and explicit-storage customers. [Library names](wiki/spec/build/declarations.md#library-responsibilities-and-names)

- **PROVIDER-SELECTION-AFTER-TERMINAL.** (covers **OPERATOR-BOUNDARY-CALLS**, **TARGET-SEMANTIC-APPLICATIONS**) Complete the handoff from [source provider publication](source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi/src/provider_supply) to [native admission](source-rust/subsystems/compiler/psi-to-native/pipeline/00_psi-to-admitted-native-inputs). Bind external realizations and calling plans to the exact requirement, provider, target and published program.
  - Finish provider-backed operator, opaque representation, constant and review cases, including the remaining floating-point and GUI customers. Check provider-aware evaluation of constants under the plan that selected them.
  - Preserve a rootless package's private `SelectedAnswer` choice for public `Answer::read() -> u32` in UEFI review. Recover the exact source contract without inventing a runnable entry.
  - Preserve the selected implementation's authored `reaches ReadCapability` and its checked effects independently of its requirement in [reviewed service rows](samples/cli/probes/reviewed_service_rows).

## Lowering, checker, and data-flow

- **JOIN-PREDICATE-FORMS.** Prove a bound on `x & mask` across a join, and retain a dominating Boolean result over byte views without exhausting the bounded disjunction search. Exercise `verify_masked` and the `left == right && count == 2` case.
- **NEGATIVE-TWIN-REASONS.** (covers **REQUIRES-UNIFICATION**) Make each maintained negative program reach its pinned obligation and reject there. Start with a failing positive/negative pair and preserve its input difference and expected diagnostic.
  - Restore the [nonreturning denotation control](source-rust/subsystems/compiler/tests/fixtures/proofs/fail/ordinary_denotational_call_requires_normal_return/main.omg). Its unused crashing operation and ordinary clause call must reach the [value-denotation check](wiki/spec/proofs/contracts.md#total-arithmetic) and reject for the absence of an observation-free, reach-free, crash-free value.
  - Retain the case-predicate and [nested callback requirement](samples/cli/facts/target_family_callback_guarantee/README.md) oracles on the migrated `requires` syntax. Exercise them through the maintained parser and sample tests.

- **SAME-FAMILY-RECORD-TRANSPORT.** Admit explicitly supplied records across proved-compatible indices without forwarding machines ([contract and acceptance](wiki/spec/proofs/contracts.md#exact-matching-and-evidence-transport)); extends **OPERATION-RECORD-IMPLEMENTATIONS** and indexed-value compatibility.
  - Extend the shared record/callable compatibility judgment in the lowered-form checker, not conformance search. A requirement-bound selection crosses to a provably equal application of the same requirement declaration and stays bound to it; keep dependency-ordered evidence and mutable source handback through specialization, projection and published Psi; migrate the blanket reindexing refusal.

- **WIDTH-INDEXED-INTEGERS.** Implement the [width-indexed integer family](wiki/spec/language/numeric_values.md#width-indexed-integers) across carrier identity, literals, conversions, proofs, Psi encoding and native multiword arithmetic. Exercise `u13` shifted by a runtime count whose high bits change the reduction, and fixed multiword customers with no hidden allocation.

- **INTEGER-CARRIER-FAMILY-CONSTRAINTS.** Implement [integer carrier constraints](wiki/spec/language/numeric_values.md#integer-carrier-families) on domains, value queries and unbounded integers.
  - Carrier-generic domains (`domain<T> T::NonZero requires integer::is_integer<T>() && self != 0;`): read each application's carrier into the domain's requirements, check its eligibility before the value predicate, and reject an ineligible application as invalid, not as an empty domain.
  - Specialize an ordinary value call such as `integer::is_signed_integer<u8>()`. Check the generic definition once per admitted carrier family across widths.
  - Form unbounded `i` and `u` types in source and classify them in the same carrier queries.

- **OPERATION-ARITHMETIC-POLICY.** Implement [policy operators](wiki/spec/language/numeric_values.md#policy-operators) and their [declared policy-domain overloads](wiki/spec/language/numeric_values.md#policy-domains). Preserve exact qualified/plain signatures through selection, lowered-form checking and realization; no implicit conversions or surrounding-policy propagation.
  - Retain each selected operation's exact carrier and policy in canonical callable/index identities. Raw non-Exact recipes require that complete representation before they can publish an exact identity.
  - Verify the SHA-256 and wire customers after static-machine and qualification migration.

- **NAMED-DOMAIN-CASTS.** Implement [casts](wiki/drafts/designs/named_domain_casts.md).

- **REQUIRED-AUTOMATIC-DISCHARGE.** Implement the [required automatic discharge rules](wiki/spec/proofs/contracts.md#required-automatic-discharge) in the lowered-form checker and expose [`--check --floor-only`](wiki/spec/build/command_line.md#checker-conformance). Exercise obligations at the specified floor without hints or extra search and preserve the distinction from stronger optional proof.

- **PUBLISHED-WRITE-FRAMES.** Publish inferred [write frames](wiki/drafts/designs/published_write_frames.md) in interface identity.

- **RECORD-OR-SUM-DATA.** Complete [record or sum data](wiki/drafts/designs/record_or_sum_data.md), including record decomposition and `let Pattern = expr`. Migrate the sample and test customers.

- **REQUIRED-ISSUANCE-CHOICE.** Implement [required issuance](wiki/drafts/designs/required_issuance_choice.md) and its review rows.

- **BOUNDARY-SLOT-VIEWS.** Implement [slot views](wiki/drafts/designs/boundary_slot_implementation_views.md) with the two-slot clock fixture.

- **INVOKES-ENTRY-CEILING.** Implement the [entry ceiling](wiki/drafts/designs/invokes_entry_ceiling.md); then strip plumbing rows.

- **PUBLISHED-NO-RETURN.** Implement [published no-return](wiki/spec/language/state_contracts.md#published-no-return) and [core `Never`](wiki/spec/language/data_and_literals.md#the-uninhabited-type-never): no-normal-successor from an uninhabited result type or an impossible published postcondition, an uninhabited expression accepted at any expected type, and coverage that skips cases with uninhabited payloads.
  - Declare the `UefiOsHandoffTermination` operations `-> Never`, and `StaticInterruptWork::run` `-> Never` in place of `ensures false`.

- **ROLE-SEPARATED-BUILTIN-TYPE-NAMES.** Apply [builtin type roles](wiki/spec/language/modules.md#builtin-type-names) to same-named operation records. A domain annotation selects the builtin carrier and a named machine call can select the supplied record. Diagnose multiple owners eligible in the same role.

- **QUALIFIED-OPERATOR-HOME.** Finish the [operator authority checks](wiki/spec/language/expressions.md#operator-families) on the [cross-package customer](source-rust/subsystems/compiler/tests/fixtures/operators/run/qualified_operator_home). A qualified operator defined in another module of its owning package must keep that module's lexical scope and visibility. Under a restricted domain access policy, selecting the operator must confer no extra establishment, read or write authority. Exercise the private-operator refusal at the caller's `+`.

- **ARRAY-ELEMENT-VIEWS.** Implement [views of a fixed array](wiki/spec/language/expressions.md#views-of-a-fixed-array) and migrate the programs that assumed a conversion.
  - Diagnose an implicit fixed-array borrow used where a slice is required, and point to `&array[..]` or `&mut array[..]`.
  - Full-range formation on a fixed array establishes `len == N` for the lowered-form checker.
  - Migrate remaining `Console::read_line` callers that pass a borrowed fixed array and exercise the explicit view through both engines.

- **APPLIED-BOUNDARY-SERVICE-REACH.** Implement [boundary services with parameters](wiki/spec/language/effects.md#boundary-services-with-parameters): name a boundary trait with parameters in `reaches` and `invokes` rows by its application, and carry that application end to end.
  - Carry applications such as `Sensor<17u64>` and `Sensor<Id>` through resolution, specialization, comparison, publication and separate consumption. Diagnose a missing application at its use.
  - Close boundary parents by substituting the child's arguments into its declared boundary parents, then closing transitively; ordinary trait parents add nothing. `invokes` shares the application resolution but not reach semantics or parent closure.
  - Use the `build_target_provider` family to check the actual reached service and preserve its provider's exact static arguments through package review.

- **TYPED-STATIC-PARAMETERS.** Implement and migrate to [typed static parameters](wiki/drafts/designs/typed_static_parameters.md).

- **ACTIVATION-STORAGE-PLANS.** Implement [storage plans](wiki/drafts/designs/activation_storage_plans.md).

- **OWNED-CLEANUP.** Implement [by-value cleanup](wiki/drafts/designs/owned_cleanup.md).

- **OPEN-CALLABLE-CONTRACTS.** Close [callable contracts](wiki/drafts/designs/open_callable_contracts.md) at runtime and boundary positions.

- **STATIC-BINDER-CONTRACT-DEPENDENCIES.** Implement [static binder contract dependencies](wiki/drafts/designs/static_binder_contract_dependencies.md): one published dependency term per contract axis for static callbacks, substituted from selected published contracts.
  - Unify the `service_reach.rs` binder terms and `operational.rs` selector contexts into one term representation; publish and import it through Psi rows; forward through renamed binders; converge recursive propagation.
  - Add the crash row (public omission means none, private inference, requirement deferral, `crashes none` enforcement, per-cause routes) and the termination row (binder premises on authored `terminates`).
  - Retire bare `reaches` in the maintained parsers and migrate its sample and fixture uses to `reaches none`.
  - Add the [result-relation row](wiki/spec/language/effects.md#callback-result-relations): opaque relation per checked call, authored `F::ensures(args, r)` claims discharged from it, selected relation substituted at specialization; immutable arguments first.

- **CORE-INDEXING-CONTRACTS.** Implement [contract-gated core indexing](wiki/drafts/designs/core_indexing_contracts.md).
  Acceptance: independent bounds admission is retired.

- **CONTRACT-RUNTIME-INDEX-OBSERVATIONS.** Prove [indexed scalar contract observations](wiki/spec/terminal-psi/byte_views.md#indexed-scalar-contract-observations) when the index is a runtime scalar term. Form the bounds obligation before substituting the clause. Exercise std's `send_string<const Count>` with `text[Count - 1] == 0`, including an out-of-bounds refusal.

## Universal interpreter cutover

<a id="early-terminal-jobs"></a>

- **EARLY-TERMINAL-JOBS.** (covers **TERMINAL-LAYOUT-WIRE-EVALUATION**, **TERMINAL-BUILD-SEED**) Run demanded constants, layout and access policies through [ordinary checked evaluation](wiki/spec/language/evaluation.md), using the selected target, source authority and actual inputs. Start at the [source invocation driver](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations.rs).

  - Complete const arguments on machine and type applications when the parameter is domain-qualified, depends on another parameter or reads a constant initialized by a call. Preserve dependency order, cycle refusal and the original evaluation charge. [Argument evaluation](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/constants/arguments.rs)
  - Carry checked generic layout-policy results, target facts and reflected field geometry through plan-laid storage. Exercise a C layout and a constant domain fact end to end. [Layout evaluator](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/fixed_layouts)
  - Replace syntax-based predicate approximation in [static argument canonicalization](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/declarations/generic_data/const_evaluation/values.rs) with evidence for the selected operator, substitutions and receiving domain. Preserve literal decoding and primitive normalization.

- **UNIVERSAL-INTERPRETER-CLOSURE.** Complete standalone [Psi execution](wiki/spec/build/embedding.md#standalone-execution-selection) with the admitted, target-specific host services.
  - Exercise the Linux kernel-import result chain through the [host adapters](source-rust/subsystems/interpreter/host) and process exit. Validate the selected syscall plan, propagate its result to the next operation and refuse an unavailable adapter before issuing it.

## Immediate product closure

- **STALE-SAMPLE-WORKAROUNDS.** Simplify the indexed deposit in [bank ledger](samples/cli/systems/bank_ledger), the transfer tripwires in [account ledger](samples/cli/systems/account_ledger), the caller-side products in [shapes area](samples/cli/proofs/shapes_area) and the stale case-payload notes in [bounded counter](samples/cli/proofs/bounded_counter). Prove each original sample oracle with `mbx xtask sample-gate --filter <sample>`.
  Acceptance: none of the four files describes a fixed bug or keeps code that works around one.

- **STAGED-BUILD-CAPABILITIES.** Complete [delegated filesystem authority](wiki/spec/build/scoped_execution.md#delegated-files-and-configuration) and retire `BuildInputs`, `BuildSource` and `BuildOutput`.

- **CHECKED-BUILD-RESUMPTION.** Carry outstanding caller return obligations and borrowed results across [dependency preparation](wiki/spec/build/scoped_execution.md#checked-build-stages) when the checked successor preserves their storage.
  - Exercise [interpreter resumption](source-rust/subsystems/interpreter/embedding/continuation.rs) through the ordinary Build entry. Keep the registration and a borrow live, use the borrowed result after calling newly mounted code, and make a repeated initializer observable. Require refusal if the backing storage or earlier effect changes. The [repair queue](OWNER_CODE_CONCERNS.md#a-byte-read-can-run-in-a-build-but-prevent-it-from-resuming) describes the byte-read and transition-subject cases.

- **NO-IMPLICIT-COMPILER-STORAGE.** Implement [compiler storage](wiki/spec/build/compiler_storage.md): keep reuse in memory within one build invocation and delete the disk caches.
  - Delete the [compiled review cache](source-rust/subsystems/package-manager/review/candidate/review_cache.rs), the [checked source cache](source-rust/subsystems/compiler/build/inputs/reuse/checked_source_cache.rs) and their per-user store lanes.
  - Trace ordinary builds, review/update and interrupted publication. Keep requested outputs, acquisition and transaction recovery distinct from invocation-owned scratch.
  - Check a diamond dependency with identical inputs and one with different grants or targets. Two invocations in the same process must start independent computations.

  Acceptance: disk-cache reads and writes are gone with absent, populated, corrupt or unwritable cache directories. In-memory reuse stays within its invocation. Clean up scratch after success and handled failure, ignore abandoned scratch after forced termination and recover the publication transaction. Compare deterministic outputs, build time and peak memory before and after the deletion.

- **LIBRARY-REPRESENTATION-FAMILIES.** Implement [representations](wiki/spec/layouts/plans.md#representation-applications), [aliases](wiki/spec/language/generics.md#transparent-type-aliases), [conversions](wiki/spec/layouts/codecs.md#representation-conversions), and [home layouts](wiki/drafts/designs/home_layout_optimization.md).

- **STATE-LOCAL-VALUE-FRONTIER.** (covers **REFERENCE-LOCAL-BINDING-ROUTES**, **BORROWED-STORAGE-RESTORATION**, **MATCH-SELECTIVE-LOWERING**) Carry exact value and backing identity through [borrows and restoration](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows) and [match joins](wiki/spec/language/patterns.md). Prove valid mutable moves and repairs across aliases and suspension. Refuse shared moves, unclosed linear obligations, stale backing and joins with incompatible loan states.

- **MODULE-NAMESPACE-RESOLUTION.** (covers **RUNTIME-VALUE-GENERICS**, **STRUCTURAL-GENERIC-MATCHING**, **FINITE-GENERIC-DISPATCH**) Complete [module selection](wiki/spec/language/modules.md) and [generics](wiki/spec/language/generics.md).
  - Complete package-scoped lookup of module paths and static members in both product compilers. Distinguish `audio\play()` from `audio::play()` and `net\wire\Packet::Header` from `net\wire\Packet\Header`. Check same-named type and operation records in their respective roles, and reject ambiguous eligible candidates. [Name resolution](wiki/spec/language/modules.md#names-and-imports)

  - Run the [three-file module example](wiki/language_guide/chapter_15_modules_imports_visibility.md#modules) through both compilers. Expect 10 in native code and Psi. Moving or splitting a file within its directory preserves imports. Moving it out breaks the old import. Check file-local `use`, generated units' published paths and refusal of a legacy module header.

- **DOMAIN-ACCESS-AUTHORITY.** (covers **DOMAIN-CARRIER-FORMATION**, **NOMINAL-FIELD-FLOW**, **DOMAIN-ISSUER-ROUTES**) Complete [carrier membership](wiki/spec/language/domains.md#carrier-exact-membership), `[u8; N] in D`, [domain `access Policy`](wiki/spec/resources/access.md), and [repair facts](wiki/drafts/designs/invariant_repair_facts.md).
  - Migrate the [resident backing family](samples/cli/facts/resident_backing) to the [core access module](wiki/spec/resources/access.md). Preserve the writes, extraction and intended qualification refusal in each twin.
  - Publish the checked access plan and its field observations, aliases, input order and selected layout. Exercise the original [placed-view call](source-rust/subsystems/compiler/tests/fixtures/layouts/pass/placed_view_call_custody/main.omg) through read and write accessors.
  - Carry exact memberships and selected conformance evidence through record/case casts, qualified foreign inputs and results, and host-issued values. Exercise establishment through the selected requirement.
  - Close access policies applied at static arguments such as `IssuedBy<...>`. Settle domain changes on call results, matches, inferred locals and references at their owning expression.

  - Enforce the evaluated whole-subject reader and writer rules and prove repairs after partial writes to qualified locals and formals.
  - Complete zero initialization of locals with fields whose domains require establishment, including checked issuer evidence for each constructed field.

  - Preserve carrier predicates through borrowed formals, case payloads, array elements, exclusive reference formation and a cast lent directly to a formal.
  - Establish the incoming parameter's default domain and support child domains, aliases and indexed families. Migrate `established by` declarations and delete that clause.

- **TERMINATION-RANKING-CHECKS.** (covers **GENERAL-CYCLIC-EXECUTION**, **CRASH-CONTRACT**, **OUTCOME-SPECIFIC-CALL-EVIDENCE**, **UNREACHABLE-BRANCH-DIAGNOSTICS**) Complete [termination](wiki/spec/language/termination.md), evidence for crashes and other call outcomes, and [unreachable warnings](wiki/spec/language/state_contracts.md#unreachable-branches).
  - Support the remaining forms of a decreasing measure: `Nat::IncreasingTo`, declared measures, bounded ranges, and multiple subjects. Allow measures to read fields and handle the remaining signed, address, and width-generic inputs with the required proofs.
  - Check loop result contracts (`ensures`), erased bindings, and the remaining storage cases. Check ranked non-tail recursion during proof or compile-time evaluation; runtime recursion remains tail-only.
  - Preserve termination promises when a program consumes a separately compiled package.
  - Check [joint rankings across different parameter shapes](wiki/spec/language/termination.md#mutual-ranking-source-contract) when comparing the members’ selected views requires a proved conversion. Exercise a parser that hands an unread slice to a cursor-based payload machine and a twin that can return without consuming input.

- **DEFAULT-SYNCHRONIZATION-CHECKING.** Implement [mandatory synchronization checking](wiki/spec/language/concurrency.md#default-admission-obligation). The current effects/carry, call-dependency and activation plans do not supply the retained protocol contracts or closed-composition deadlock judgment.
  - Prove the [automatic floor](wiki/spec/language/concurrency.md#first-required-automatic-floor) for an executable lock provider and guard. Track aliases, nonrecursive exclusive acquisition, helper and generic calls, callback reentry and preemption through independently published summaries. The default and `--floor-only` modes must cover the stated cases.
  - Compose worker, queue, buffer and execution-lease capacity through initialization, teardown and replacement. Preserve atomic registration, release and reacquisition. A timeout needs a runnable handler.
  - Link separately checked packages. Mutually dependent completion promises must reject, and a cyclic request/reply protocol with a valid initial state must pass. Reject a replacement that reverses the order of waiting and releasing. Treat a runtime callable with an unknown protocol as unknown.
  - Exercise actual contention, release, wakeup and reentry on the accepted programs under controlled scheduling.

- **ATOMIC-MEMORY-MODEL.** Complete atomic events and fences through checking, Psi and native code under the [memory model](wiki/spec/language/concurrency.md) and [atomic operation vocabulary](source-rust/subsystems/compiler/foundation/language-core/atomic_operations.md).

<a id="checked-assembly-control-flow"></a>

- **CHECKED-ASM-LABEL-CONTRACTS.** Implement `asm machine`, block declarations, `align_of(value)`, port settlements (`provider_custody.rs`), and [interpreted assembly](wiki/spec/language/assembly.md#interpreted-execution).
  - Migrate [flag restoration](samples/cli/facts/checked_asm_flag_restore) and customers of colon labels or implicit fall-through to [explicit assembly control flow](wiki/spec/language/assembly.md#explicit-control-flow). Preserve the original output and the authored rejection on invalid edges. Run target-specific code on its target or a supplied Psi model.

- **SAMPLE-CORPUS.** (covers **CANARY-CORPUS**, **SQUALR-HEADLESS**, **MACOS-APPLICATION-PUBLICATION**, **FACT-SAMPLE-DEPENDENCIES**) Run the release programs, [Squalr](samples/apps/README.md) and [macOS application bundles](wiki/spec/build/macos_application.md) under the [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix).
  - Run the four customers of the [fact dependency packages](samples/cli/support/fact_dependencies/README.md) to their specified results, including their negative twins. `build_target_provider` exercises a generic Build helper with computed const arguments and an applied provider. `dependency_type_qualified_call` exercises carrier arithmetic in a qualified callee’s contract.
  - `component_scalar_lending` calls and suspends through its selected provider. `owned_semantic_schema` passes a named-lifetime reference across packages.

  - Restore [Squalr geometry](samples/apps/README.md) through its normal package graph. From `samples/apps/squalr` run `python3 tools/verify.py native --timeout 600 --omega <absolute-omega-path>`. Preserve the seven resolver operations and their state, four target bindings and twelve geometry checks. Require `Squalr geometry: PASS` and exit 0 on the named host.

## P1 - Authority, roots, and entry

<a id="process-exit-contract"></a>

- **ENTRY-CONTENT-ROOTS.** (covers **PROCESS-EXIT-CONTRACT**) Complete [provisioned entry receivers](wiki/spec/build/entry_roots.md) and [process exit](wiki/spec/language/process_exit.md) through the selected provider on each hosted target.

## P2 - Materialization and access policies

- **ADDRESS-EXPOSURE.** (covers **DEFERRED-RECAST-ESTABLISHMENT**, **DEVICE-EXTENT-ACCESS**, **SYMBOLIC-MATERIALIZATION**, **NATIVE-CODE-LAYOUT-QUERIES**, **ASSUMED-BINDING-CONTRACTS**) Complete [storage-cast windows](wiki/spec/layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts), [device access](wiki/spec/resources/device_access.md) (`device_extent_access`), [symbolic storage written after handoff](wiki/spec/layouts/plans.md), [`address_of`/`code_layout`](wiki/spec/language/counts_and_addresses.md#address-exposure), and [`assume_binding_contract`](wiki/spec/build/assumed_binding_contracts.md).

## P3 - Terminal Psi, proofs, and observation

<a id="observation-profile"></a>

- **OBSERVATION-PROFILE.** Derive the [observation profile](wiki/spec/terminal-psi/observations.md) from the validated Psi module, including its identity, ordinary events, crashes and terminal external sites. Encode it in the [wire format](source-rust/subsystems/compiler/psi/artifact/wire). Have an independent consumer compare the decoded rows with the program and reject a changed association.

- **STRUCTURAL-BORROW-IDENTITY.** (covers **WRITE-ONLY-BORROW**, **PLACED-ACCESS-NATIVE-OPS**, **GENERIC-FIXED-ARRAY-VIEWS**, **BORROW-PROOF-CONVERGENCE**, **TERMINAL-SLICE-VIEW-VOCABULARY**) Carry structural loans and slice views through native and Psi, including nested element views. Exercise lending an owned sequence as `&[T]` when its elements themselves retain borrowed views. Preserve each backing and its exact loan through return and cleanup.
  - Run [case-selected slice references](samples/cli/probes/case_selected_slice_reference/main.omg) unchanged with native/Psi agreement: establish the mutable typed slice's actual backing and loan, then preserve the selected child through the return, including a rebound slice. Its recorded refusal precedes the returned-window checks; those checks and the independent projection-bound rejection remain required.
  - Check an unused `Observe<T>` machine taking `&T` under its declared lifetime and requirements. Run closed applications through both engines and reject mutation through a shared formal.

<a id="normalized-abi-lowering"></a>

- **NORMALIZED-ABI-LOWERING.** (covers **OPAQUE-BY-VALUE-BOUNDARY-ABI**, **FOREIGN-RETAINED-ARGUMENT-BACKING**) Complete [calling plans](wiki/spec/build/calling_plans.md) and [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody).

- **PROOF-KERNEL-CORE.** (covers **PROOF-CONTRACT-MIGRATION**, **QUOTIENT-THEOREM-LIFT**, **PSI-CLAIM-ADMISSION**) Complete [source proofs](wiki/spec/proofs/foundation.md) and [Psi claim admission](wiki/drafts/designs/psi_claim_admission.md). Report [coverage code 36](wiki/spec/build/compiler_request.md) with the obligation identity when the prover lacks a rule for its form.

## Build-level behavior exclusions

- **BUILD-SEMANTIC-EXCLUSIONS.** Complete [behavior exclusion verification](wiki/spec/build/behavior_exclusions.md#implementation-acceptance) from authored Build to installed product. Use the existing [native verifier](source-rust/subsystems/compiler/psi-to-native/verification/behavior-exclusions). The [CLI test](source-rust/application/tests/build_prelude_operation_calls.rs) still expects an intermediate refusal. Require a clean selected body to run and a selected transitive `Trap` to reject for the exclusion.
  - Distinguish dormant bodies from selected callbacks, semantic service calls from physical effects and a stale replacement from the accepted installation. Check a saved publication independently before accepting its exclusions.

## Requirement-based tests

- **TEST-GROUP-FILESYSTEM-TARGETS.** Provide [delegated filesystem backing for test groups](wiki/spec/build/testing.md#test-runner-filesystem-slot) through their ordinary provider selection and [test planning](source-rust/subsystems/compiler/build/planning/registered_tests.rs). Retire `tests.service<Group, Slot>()`, its Build operands and the filesystem-specific switch.
  - Exercise canonicalization, metadata, directories and file I/O on the hosted targets. Include the Linux libc `realpath` case and report unavailable engines explicitly.
  - On native and Psi, run two tests that each exclusively create the same path, write and read different bytes, and leave a pre-existing host file there untouched. A child must observe writes through its delegated view. Reject a child's request for an ungranted raw host backing before test effects. Keep the service-free group runnable and check that application roots retain their original provider.

## Checked boundary topology

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver the [checked boundary topology contract](wiki/spec/packages/topology.md) and [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization).

## Embedding and interpreted components

- **EMBEDDING-SOURCE-TO-HOST.** (covers **INTERPRETED-CATHEDRAL**) Finish [embedding](wiki/spec/build/embedding.md), evidence-bearing qualified host inputs, and interpreted Cathedral.

## Parallel language and compiler lanes

- **GENERIC-CALLABLE-FAMILIES.** In the [experimental Rust compiler](omega-rust-v2/PLAN.md), check a callable whose signature introduces its own type parameter. Use one `Visitor = machine<T>(item: T)` at two types and distinguish its binder from an enclosing `Visitor<T>`. [Callable contract](wiki/spec/language/generics.md#invocation-lifetime-families)

- **COMPONENT-SUBSTRATE.** (covers **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**, **RUNTIME-SIZED-ACTIVATION-CONTRACT**, **FILESYSTEM-PLATFORM-SERVICES**, **TR3-TR8**, **CHECKED-REPRESENTATION-REUSE**) Complete [authority review](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission), [bounded activation](wiki/spec/resources/activation_storage.md), [filesystem services](wiki/spec/build/platform_services.md), [component installation](wiki/spec/build/component_publication.md), [task runtime](wiki/spec/build/task_runtime.md) (`blocking-executor`), and [checked reuse](wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse).

## P5 - Cathedral over general Omega primitives

- **EXTENT-LIBRARY-TRANSFORMS.** (covers **CONSERVATION-CONTRACT**, **TERMINAL-CONTENT-CLAIMS**, **BOUNDARY-ISSUANCE**, **BUMP-ALLOCATOR-CANARY**, **UEFI-PHYSICAL-SEMANTIC-ENTRY**, **AP-BRINGUP**, **ADDRESS-TRANSLATION-CANARY**, **EXCEPTION-ROOTS-AND-TIMER**) Complete [extent/run contracts](wiki/spec/resources/typed_runs.md#split-and-merge-contracts), core primitives, Vec/bump, UEFI handoff and Cathedral AP/MMU/interrupt canaries.
  - Rename the [Arena service](source/library/core/arena.omg) to `Allocator` and publish size, alignment, backing, rights, content, lifetime and reclamation under [qualified geometry](wiki/spec/resources/extents.md#qualified-geometry). Distinct origins alone do not invalidate a claim. Check that implicit reclamation finishes infallibly without blocking or suspending.
  - Grow a container with an authorized selected allocator. Within-capacity operations use the backing already held. Growth and failure must account for the allocator state and rejected or replaced backing. Exercise explicit-instance allocators, lost authority and incompatible reclamation.
  - Deliver a [generic Vec](wiki/spec/resources/allocation.md#dense-collections-over-allocated-backing) over a live run and adjacent vacant storage. Exercise linear elements, zero-sized elements with cleanup, contained loans, failed growth and rejected initialization. Preserve order, exactly-once disposal, rejection recovery and constant-cost split/merge. Reconcile [core's carrier](source/library/core/vec.omg) with [the library implementation](source/library/alloc/vec/vec.omg) and keep its scalar growth/close and bump return/reset oracles.

## Rust compiler release closure

- **RC-RELEASE-RECORD-AND-CLOSURE.** (covers **RC-BUILD-AND-PACKAGES**, **MACOS-X64-HOST-PROFILE**) Close all six [release gates](wiki/drafts/reference/rust_compiler_completion.md#release-matrix) on one revision across `linux_x86_64`, `linux_arm64`, `macos_arm64` and `windows_x86_64`. Keep lock, closure, target and acceptance isolated per root. Exercise the [public CLI](wiki/spec/build/command_line.md).
  - Establish repository, source semantics, Build and packages, native matrix, diagnostics and representative programs independently. Record expected outputs, intended rejection reasons and matching-host native runs. Include saved-Psi consumption after the producer exits. A gate with missing coverage stays open.
  - Recheck the [two-borrow provider call](https://github.com/CathedralOS/Omega/pull/3733) under `TargetOperationCorrespondence`. The provider writes 41 and 43 to distinct `&mut i64` arguments. Require both results, validator acceptance, and rejection if the provider record is substituted. [Target checker](source-rust/subsystems/compiler/psi-to-native/verification/stage_validators.rs)

## Omega-written compiler

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Compile the [Omega compiler](source/README.md) through its [public request](wiki/spec/build/compiler_request.md) and [construction application](source/application/construction/README.md).
  - Teach the Omega-written frontend to parse [named callable aliases](wiki/spec/language/machine_values.md#named-structural-callable-types) and represent their signatures. Check a factory returning a named reader and diagnose an inline reader signature at its use.
  - Implement package keys, ordering, graph and request semantics after envelope validation. The [OCREQ staging test](source/application/ocreq/tests/request_staging.py) distinguishes a valid request that reaches provision 25 from an oversized request that exhausts staging at provision 26. Add a successful artifact request while preserving malformed and over-budget outcomes.
  - Run the actual hosted construction application on [closed source sets with authored Build files](source/application/construction/README.md#including-authored-build-sources). Check selected ownership, exact package-relative paths and rejection of a Build row belonging to another package before effects.
  - Connect the selected `alpha_bootstrap` Console and ProcessExit providers and establish the entry's process domain. Compile and execute the construction application on the resulting environment. Follow its own source through missing checked operations, explicit arithmetic qualifications and loans.
  - Use the [construction procedure](source/application/construction/README.md#construct-the-actual-compiler) to produce a compiler tape and rebuild it with that compiler. Compare repeat outputs on identical inputs. The [bootstrap board](TASKS_BOOTSTRAP.md) owns the independent seed routes.
