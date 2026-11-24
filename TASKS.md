# Tasks

Use the [sample loop](AGENTS.md#the-sample-loop) for a repair and the [release matrix](wiki/drafts/reference/rust_compiler_completion.md#release-matrix) for release qualification. Start with the assigned program and its expected result. Delete a completed item when its outcome is verified.

Find Rust owners in the [compiler map](source-rust/README.md#where-code-belongs). The [bootstrap board](TASKS_BOOTSTRAP.md) owns the bootstrap implementations. [Code repair concerns](OWNER_CODE_CONCERNS.md) track structural defects.

## Development-health outcomes

- **CLAIMS-LEGACY-READ-REMOVAL.** After 2026-11-22, once the old claims ref has been quiet for two hours, remove its parallel read, ticket seeding and merged status from [claims](tools/xtask/src/claims.rs). Delete its two migration tests and the [migration instructions](wiki/development/claims.md#migration-from-the-coordination-ref).

## Priority (human-set only)

- **LOWERED-FORM-CHECKER.** Highest priority by owner ruling. Finish the [lowered-form checker](wiki/drafts/designs/check_on_lowered_form.md) for the accepted language. Every valid program must reach publication with its obligations proved, and an invalid program must be refused at the responsible obligation. [Checker entry](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/verdict.rs)

  - Check `source/library/std/main.omg` as a package on each hosted target, then run `mbx xtask sample-gate --filter cli/basics`. Follow the first unsupported construct through the ordinary Build and Product.
  - Complete ownership and loan facts for affine locals, mutable sequence backing, views and mixed-origin reference results. Check fixed-array lengths on call results, indexed values and case payloads, and reject out-of-extent indices at the access.
  - Check [domain predicates](wiki/spec/language/domains.md#byte-containers-and-encoding-domains) that call machines with requirements, type parameters or borrowed byte views. Prove ordinary `core::Identity<FloatMeaning>` clauses from the helper body, including `abs_f64` and `sqrt_f64`.

  - Cover full-package checking of generic declarations with lifetimes, bounds, cleanup and computed extents. Preserve the admitted entry's logical contracts and the declaration that owns an unsupported obligation in diagnostics.

  - Carry the checked bounds of `size_of` and `align_of` through arithmetic on them and validate them at installation. Exercise [record geometry](samples/cli/probes/record_geometry), scalar and generic carriers, and the value form `align_of(view)`.

  - Give an array extent that names a constant read from the target's layout the length each deployment closes, as in `const SIZE: u64 = size_of::<u64>(); machine sized(a: [u8; SIZE]) -> [u8; SIZE] { a }` ([evaluation.md](wiki/spec/language/evaluation.md#target-dependent-applications): an observation "can occupy any position available to an equivalent ordinary constant, including array length"). The [source driver](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/constants/initializers.rs) leaves such a scalar constant unevaluated on purpose, and a scalar read of `SIZE` lowers to its retained application and runs, but the [array extent reader](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/arrays.rs) refuses the extent ("array extent that names a constant with no evaluated array length"), under `omega --check` and `omega run --both` alike. Lowering the extent to that application (`ArrayExtent::Application`) next stops at element reads and array literals, which accept only literal, const-parameter and stored-length extents ("index or literal of a non-array type"), so the application extent must also reach `CollectionExtent`, the checker and native layout.

  - Give an unannotated constant the type of an initializer that only typing can find: a call, another constant or a typed operand, as in `const second = flag();`, `const first = second;` and `const mask = 2u8 + 3;`. The [parser](source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees/declarations/constants.rs) writes the type of a suffixed literal, a cast or a Boolean and refuses the rest as unsupported, because name resolution reads every constant's declared type before typing runs.

  - Carry integer bounds through nested matches on computed subjects. Matching a projected flag from a temporary record with a [selected cleanup](wiki/spec/terminal-psi/ownership.md) retires the record and sets its caller’s marker. Both markers and the selected write reach the caller on native and Psi, but adding one to the stored value after the matches is refused for possible overflow even when its only incoming values are 6 and 10.
  - Let a block-valued arm hold a `crash` or `transition` statement. The [parser](source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees/statements/sequence.rs) refuses these statements.

- **NESTED-MACHINE-UNIFICATION.** Finish checking calls and returns in nested machines ([machines.md](wiki/spec/language/machines.md#nested-machines-and-identity), [state_contracts.md](wiki/spec/language/state_contracts.md)).
  - Resolve recursive scopes, elaborate flat identities, and reject runtime capture or code after trailing declarations.
  - Complete [local inference](wiki/drafts/designs/local_inference_and_contracts.md), cyclic arrivals, normal returns, loans, restoration, linear custody, and SCC rank measures across call shapes.
  - Check the linked repositories and both compilers against the same declaration model. Preserve the original execution results of migrated programs.

- **CANONICALIZE-SCALAR-RANGE-CONTRACTS.** Complete [scalar range contracts](wiki/spec/language/domains.md#declaration-and-membership).
  - Prove field and case bounds at construction, on writes, on return of a field loan and after array updates.
  - Check and lower scalar-field loans across case fields, recursive records/references, calls, joins, and [recasts](wiki/spec/layouts/recasts.md). Every shared scalar recast of a field or borrowed scalar is refused today, although [recasts.md](wiki/spec/layouts/recasts.md#shared-and-mutable-views) admits each (every `u8` representation is a valid `i8`): reading `let seen: @i8 = @cell.other as @i8; $seen` ("reference descriptor has no current origin account", also on [crash_entry_boolean_view/field](samples/cli/facts/crash_entry_boolean_view/field/main.omg) and `/local`), returning `@cell.other as @i8` from `machine view(cell: @Cell) -> @i8` ("call reference result has no complete contents inventory"), and returning `byte as @i8` for `byte: @u8` ("call dynamic returned view has an unsupported backing projection"). Returning `@cell.other` uncast runs.
  - Qualify migrated [range-contract customers](wiki/drafts/designs/range_suffix_migration.md) with their original results, including the filesystem programs under **FILESYSTEM-PLATFORM-SERVICES**.

- **STATIC-MACHINE-MIGRATION.** (covers **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION**, **TRAIT-IMPLEMENTATION-SOURCE-MIGRATION**) Finish static machines, implementations, `dropped by` and [callback reach](wiki/drafts/designs/deferred_callback_reach.md).
  - Finish const-expression closure, package projection, calling policies and Omega-written frontend evidence-binder parity.
  - Evaluate a native requirement's declared calling policy, default an unannotated requirement to Omega's target convention and check the resulting plan against its locator. [Calling policy](wiki/spec/build/calling_plans.md#syscall-binding-calling-policy)
  - Migrate Linux filesystem and clock requirements to authored calling policies. Check `openat` on Linux x86-64 and arm64, its missing-policy refusal, an ordinary unannotated boundary and an explicit foreign import.
  - Remove the required foreign ABI tag from [CallPlan](source/library/std/calling.omg) and its [decoder](source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/src/calling_policy_plans/build_time_decoding.rs). Derive [plan identity](source-rust/subsystems/compiler/psi-to-native/targets/calling-conventions/src/plans/plan_identity.rs) from the normalized contract.
  - Move foreign aggregate classification, register placement, stack and clobber rules into Omega policies. Check their results against target primitives and the existing clang ABI comparisons.
  - Exercise an independently authored x86-64 policy and native callee. Passing 37 in `r10` and 5 on the stack must return 32. Assigning both live arguments to `r10` must reject. [Authorship contract](wiki/spec/build/calling_plans.md#authorship-and-evaluation)
  - Qualify `positional_implementation_binder`/`open_const_implementation`/`open_record_const_implementation`/`named_conformance_dispatch`/`generic_calling_policy`/`domain_selected_cleanup` on all hosts.
  - Move Build facets to `Build::*` and retire `machine Data::operation` and attached helpers.
  - Keep live loans and their backing valid across suspension.
  - Select a conformance member through a static binder that a type parameter applies or that is itself generic. `machine use_map<T, const E: Render<T>>(x: T) -> bool { E::paint(x) }` called as `use_map::<u8, Red>(x)` is refused in preparation ("a static record operand has another declared schema"), and `E::keep::<u8>(x)` or `E::pick::<4>(x)` on a member declared `machine keep<T>` or `machine pick<const N: u64>` is refused in lowering ("closed requirement call has an incomplete bound argument tuple"). Named directly, `Local::keep::<u8>(x)` and `Local::pick::<4>(x)` enter the member instance prepared for their arguments and run. A selection row of a generic member (`Keep::keep = hold;` with `machine hold<T>`) is still refused ("generic machine read without arguments"), as a generic reference row of a boundary trait is.
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
  - Wire the returning UEFI adapter's scoped firmware calls and grant issuance into its published `reaches` and `invokes` ceilings, including the call to `ProgramStorageEntry::enter`. Bind the selected `ProgramImageNativeProvider` address to the actual image mapping and establish custody of the separate initial storage. Check bounds, disjointness and live stack exclusions before forwarding both grants. Check and retain both `Extent in Granted` parameter requirements when publishing the `ProgramStorageEntryNativeProvider` conformance. [UEFI entry authority](wiki/spec/build/uefi_entry.md#firmware-api-authority-and-storage-grants)

- **TARGET-REALIZED-FLOAT-ARITHMETIC.** Realize canonical `f32`/`f64` operations from the selected target's declared table ([design](wiki/drafts/designs/target_realized_float_arithmetic.md)); retire their Build provider selection.
  - Add checked-machine realization to [the target float table](source-rust/subsystems/compiler/psi-to-native/targets/target/src/float_realization.rs) and exercise it on a selected target. Measure remaining provider-selection cost separately.

- **BUILD-OWNED-NATIVE-BINDINGS.** Replace `via` with [Build-owned bindings](wiki/spec/build/foreign_bindings.md#build-owned-native-binding).
  - Finish exact catalog custody and reject false authored backing ([controls](wiki/drafts/designs/runtime_provider_authority.md)); migrate table/vtable, user-intrinsic, generic-trait and Windows `ProcessExit`. Keep executable-provider delivery separate; repair [UEFI/table custody](wiki/spec/build/foreign_bindings.md#migration-acceptance).
  - Implement [`FunctionFieldBinding`](wiki/spec/build/foreign_bindings.md#function-table-bindings) and `reflect::parameter` for the four `UefiBootServices` table slots, with `this` used only for dispatch. Supply sealed entries for handoff `transfer` and `firmware_return`.
  - Carry a closed `Registrar::register<TimerHandler::call>` requirement through capture, package rejoin, saved-input substitution checks and native callback execution.
  Acceptance: `via` is removed.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Qualify [machine-supplied boundary operations](wiki/spec/language/machines.md#supply) through the Windows GUI provider.
  - Run the [window demo](samples/gui/window_demo/README.md) on Windows under native and Psi execution. Both executions must draw 60 frames, release the window, print `PASS: 60 frames blitted (exit 0)` and exit successfully.

- **OPERATOR-MACHINE-SUPPLY.** Run declared operations/premises; implement [identity](wiki/drafts/designs/index_identity.md) and [integer arithmetic](wiki/drafts/designs/computational_and_mathematical_integers.md).
  - Separate fixed identity from budgeted algebra proofs; carry generic evidence through specialization.
  - Implement [operator-independent domain composition and exact result keys](wiki/spec/language/domains.md#domain-composition-and-operators), including its selection and artifact acceptance controls.
  - Implement `i`/`u`, exact math `/`, quotient/remainder and checked bridges; migrate existing integer-proof fixtures under the design's controls.
  - Remove the root `operator` declaration from the [Rust parser](source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees/declarations/items.rs) and its dependent representation once its source customers use machines. Lower `f64 in D` and `i32 in D` through their declared operators.
  - Form the collection loan for a declared `[]` or `[..]` whose collection parameter is a reference ([collection arguments](wiki/spec/language/expressions.md#collection-arguments-and-results)). With `machine [] at(buffer: @Buffer, index: u64)`, `buffer[index]` on a `Buffer` value must lend `buffer` to `at`; typing leaves that occurrence unselected (`needs_collection_view` in `operator_calls.rs`) and lowering refuses it.

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

- **STD-BUILD-LATE-MEMORY.** Find what holds 4–8 GB during the last four minutes of the [public filesystem values build](source-rust/application/tests/fixtures/filesystem/run/public_values/build.omg), which compiles a small std program for each target, and size that storage to what its stage needs. After #5324 the build peaks at 7.9 GB on Windows (603 s) and 7.84 GB on `omega-linux`, but memory sits at 4–8 GB from about 360 s to the end, and no one has traced which stage it belongs to. Measure each stage's peak on that build first. The earlier cost #5324 removed came from std's Windows directory removal, which carries a 4,096-element array through every nested-machine join, so its machines have about 66,000 values each; check whether the late phase scales with the same array. Its test is `portable_filesystem_values`.

- **JOIN-PREDICATE-FORMS.** Prove a bound on `x & mask` across a join, and retain a dominating Boolean result over byte views without exhausting the bounded disjunction search. Exercise `verify_masked` and the `left == right && count == 2` case.
- **NEGATIVE-TWIN-REASONS.** (covers **REQUIRES-UNIFICATION**) Make each maintained negative program reach its pinned obligation and reject there. Start with a failing positive/negative pair and preserve its input difference and expected diagnostic.
  - A twin pins its refusal by message text, so it still passes when the program is refused for another reason that shares the words. The archived V2 compiler declares each layer's refusal kinds once and its tests compare the kind and the span, not the wording (`_recycling/omega-rust-v2/layers/01_tokens-to-syntax-trees/src/errors/parse_error_kind.rs`; `_recycling/omega-rust-v2/representations/typed/src/errors/typing_reason.rs` names a closed subset of typing refusals). Give `Diagnostic` an optional kind and convert the stages one refusal family at a time.
  - Restore the [nonreturning denotation control](source-rust/subsystems/compiler/tests/fixtures/proofs/fail/ordinary_denotational_call_requires_normal_return/main.omg). Its unused crashing operation and ordinary clause call must reach the [value-denotation check](wiki/spec/proofs/contracts.md#total-arithmetic) and reject for the absence of an observation-free, reach-free, crash-free value.
  - Retain the case-predicate and [nested callback requirement](samples/cli/facts/target_family_callback_guarantee/README.md) oracles on the migrated `requires` syntax. Exercise them through the maintained parser and sample tests.

- **SAME-FAMILY-RECORD-TRANSPORT.** Admit explicitly supplied records across proved-compatible indices without forwarding machines ([contract and acceptance](wiki/spec/proofs/contracts.md#exact-matching-and-evidence-transport)); extends **OPERATION-RECORD-IMPLEMENTATIONS** and indexed-value compatibility.
  - Extend the shared record/callable compatibility judgment in the lowered-form checker, not conformance search. A requirement-bound selection crosses to a provably equal application of the same requirement declaration and stays bound to it; keep dependency-ordered evidence and mutable source handback through specialization, projection and published Psi; migrate the blanket reindexing refusal.

- **WIDTH-INDEXED-INTEGERS.** Implement the [width-indexed integer family](wiki/spec/language/numeric_values.md#width-indexed-integers) across carrier identity, literals, conversions, proofs, Psi encoding and native multiword arithmetic. Exercise `u13` shifted by a runtime count whose high bits change the reduction, and fixed multiword customers with no hidden allocation.

- **INTEGER-CARRIER-FAMILY-CONSTRAINTS.** Implement [integer carrier constraints](wiki/spec/language/numeric_values.md#integer-carrier-families) on domains, value queries and unbounded integers.
  - Carrier-generic domains (`domain<T> T::NonZero requires integer::is_integer<T>() && self != 0;`): read each application's carrier into the domain's requirements, check its eligibility before the value predicate, and reject an ineligible application as invalid, not as an empty domain.
  - Specialize an ordinary value call such as `integer::is_signed_integer<u8>()`. Check the generic definition once per admitted carrier family across widths.
  - Form unbounded `i` and `u` types in source and classify them in the same carrier queries.
  - Lower width-indexed domain families. `domain<const N: u64> u<N>::Tag requires N > 3;` with `machine a(x: u13 in Tag) -> u13 in Tag { x }` is refused in lowering ("type domain application without its prepared theory"). Each use must owe the family's predicate at its own width (`13 > 3` holds, `7 > 3` for a `u7` use does not), and `domain<const N: u64, const Limit: u64> u<N>::Bound<Limit> requires N > Limit` must keep `Bound<4>`'s index apart from the carrier's width.

- **OPERATION-ARITHMETIC-POLICY.** Implement [policy operators](wiki/spec/language/numeric_values.md#policy-operators) and their [declared policy-domain overloads](wiki/spec/language/numeric_values.md#policy-domains). Preserve exact qualified/plain signatures through selection, lowered-form checking and realization; no implicit conversions or surrounding-policy propagation.
  - Supply the core domains and their published overloads through ordinary declarations and imports. Migrate bare policy uses in consumers and tests, then remove the resolver's spelling fallback. Check that an imported core domain and a same-named local select their respective operators, and a bare name with no declaration is refused.
  - Retain each selected operation's exact carrier and policy in canonical callable/index identities. Raw non-Exact recipes require that complete representation before they can publish an exact identity.
  - Verify the SHA-256 and wire customers after static-machine and qualification migration.

- **NAMED-DOMAIN-CASTS.** Implement [casts](wiki/drafts/designs/named_domain_casts.md).

- **REQUIRED-AUTOMATIC-DISCHARGE.** Implement the [required automatic discharge rules](wiki/spec/proofs/contracts.md#required-automatic-discharge) in the lowered-form checker and expose [`--check --floor-only`](wiki/spec/build/command_line.md#checker-conformance). Exercise obligations at the specified floor without hints or extra search and preserve the distinction from stronger optional proof.

- **PUBLISHED-WRITE-FRAMES.** Publish inferred [write frames](wiki/drafts/designs/published_write_frames.md) in interface identity.

- **RECORD-OR-SUM-DATA.** Complete [record or sum data](wiki/drafts/designs/record_or_sum_data.md), including record decomposition and `let Pattern = expr`. Migrate the sample and test customers.
  - A `let` pattern whose head names a case, `let Shape::Circle { radius } = shape;`, is refused in [typing](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/data_patterns.rs). [patterns.md](wiki/spec/language/patterns.md#irrefutable-data-bindings) gives `let` "the same named or positional data-decomposition patterns as dispatch" and requires that "live facts must prove the pattern irrefutable"; it does not say whether a case pattern is one of them. A one-arm `match` under a fact that excludes the other cases expresses the same program today.
  - A data pattern decomposes only a named value or a field path of one. The parser refuses `let (a, b) = make();`; evaluate such an initializer once and decompose the saved value.
  - Still refused: a record arm that tests a field (`Header { ok: 0, version } -> ...`) or has an `if` guard, a pattern head with type arguments (`let Pair<u8> { .. } = pair;`), and a positional pattern over a subject whose record type is not declared where the pattern stands.
  - Nest data in a [generic owner](wiki/spec/language/data_and_literals.md#records-sums-and-nested-declarations). The parser refuses a nested declaration whose owner has static parameters ("not supported yet: a data declaration nested in an owner with static parameters"): `data Envelope<T> { body: data Body { case Value(value: T); case Empty; }; }` must make `Envelope<u32>::Body` and `Envelope<bool>::Body` distinct types, `Envelope::<u32>::Body::Value { value: 7 }` must construct one, and an owner's `some` binder passes only through a parameter the nested type declares (`Packet<'a>::Payload<U>`). Inside the owner the parser qualifies only type names; an expression path such as `Operation::GetUser` in the owner's `requires` is not yet qualified.

- **REQUIRED-ISSUANCE-CHOICE.** Implement [required issuance](wiki/drafts/designs/required_issuance_choice.md) and its review rows.

- **BOUNDARY-SLOT-VIEWS.** Implement [slot views](wiki/drafts/designs/boundary_slot_implementation_views.md) with the two-slot clock fixture.

- **INVOKES-ENTRY-CEILING.** Implement the [entry ceiling](wiki/drafts/designs/invokes_entry_ceiling.md); then strip plumbing rows.

- **PUBLISHED-NO-RETURN.** Implement [published no-return](wiki/spec/language/state_contracts.md#published-no-return) and [core `Never`](wiki/spec/language/data_and_literals.md#the-uninhabited-type-never): no-normal-successor from an uninhabited result type or an impossible published postcondition, an uninhabited expression accepted at any expected type, and coverage that skips cases with uninhabited payloads.
  - Declare the `UefiOsHandoffTermination` operations `-> Never`, and `StaticInterruptWork::run` `-> Never` in place of `ensures false`.

- **ROLE-SEPARATED-BUILTIN-TYPE-NAMES.** Apply [builtin type roles](wiki/spec/language/modules.md#builtin-type-names) to same-named operation records. A domain annotation selects the builtin carrier and a named machine call can select the supplied record. Diagnose multiple owners eligible in the same role.

- **QUALIFIED-OPERATOR-HOME.** Finish the [operator authority checks](wiki/spec/language/expressions.md#operator-families) on the [cross-package customer](source-rust/subsystems/compiler/tests/fixtures/operators/run/qualified_operator_home). A qualified operator defined in another module of its owning package must keep that module's lexical scope and visibility. Under a restricted domain access policy, selecting the operator must confer no extra establishment, read or write authority. Exercise the private-operator refusal at the caller's `+`.

- **ARRAY-ELEMENT-VIEWS.** Implement [views of a fixed array](wiki/spec/language/expressions.md#views-of-a-fixed-array) and migrate the programs that assumed a conversion.
  - Diagnose an implicit fixed-array borrow used where a slice is required, and point to `@array[..]` or `@mutable array[..]`.
  - Full-range formation on a fixed array establishes `view.length == N` for the lowered-form checker.
  - Migrate remaining `Console::read_line` callers that pass a borrowed fixed array and exercise the explicit view through both engines.

- **APPLIED-BOUNDARY-SERVICE-REACH.** Implement [boundary services with parameters](wiki/spec/language/effects.md#boundary-services-with-parameters): name a boundary trait with parameters in `reaches` and `invokes` rows by its application, and carry that application end to end.
  - Carry applications such as `Sensor<17u64>` and `Sensor<Id>` through resolution, specialization, comparison, publication and separate consumption. Diagnose a missing application at its use.
  - Close boundary parents by substituting the child's arguments into its declared boundary parents, then closing transitively; ordinary trait parents add nothing. `invokes` shares the application resolution but not reach semantics or parent closure.
  - Use the `build_target_provider` family to check the actual reached service and preserve its provider's exact static arguments through package review.

- **TYPED-STATIC-PARAMETERS.** Implement and migrate to [typed static parameters](wiki/drafts/designs/typed_static_parameters.md).

- **ACTIVATION-STORAGE-PLANS.** Implement [storage plans](wiki/drafts/designs/activation_storage_plans.md).

- **OWNED-CLEANUP.** Implement [by-value cleanup](wiki/drafts/designs/owned_cleanup.md).
  - Dispose an owned sum that dies unmatched by its active case ([construction and disposal order](wiki/spec/language/ownership.md#construction-and-disposal-order): "only the active sum payload"). `data Choice { case A(left: Token, right: Lease); case B(left: Token, right: Token); }` with `Lease dropped by release` and `machine discard(x: Choice) {}` is refused in lowering with "component disposition needs an established active case". Case A must invoke `release` on `A::right` and discard `A::left`; case B discards both fields.

- **OPEN-CALLABLE-CONTRACTS.** Close [callable contracts](wiki/drafts/designs/open_callable_contracts.md) at runtime and boundary positions.

- **STATIC-BINDER-CONTRACT-DEPENDENCIES.** Implement [static binder contract dependencies](wiki/drafts/designs/static_binder_contract_dependencies.md): one published dependency term per contract axis for static callbacks, substituted from selected published contracts.
  - Unify the `service_reach.rs` binder terms and `operational.rs` selector contexts into one term representation; publish and import it through Psi rows; forward through renamed binders; converge recursive propagation.
  - Add the crash row (public omission means none, private inference, requirement deferral, `crashes none` enforcement, per-cause routes) and the termination row (binder premises on authored `terminates`).
  - Add the [result-relation row](wiki/spec/language/effects.md#callback-result-relations): opaque relation per checked call, authored `F::ensures(args, r)` claims discharged from it, selected relation substituted at specialization; immutable arguments first.

- **CORE-INDEXING-CONTRACTS.** Implement [contract-gated core indexing](wiki/drafts/designs/core_indexing_contracts.md).
  Acceptance: independent bounds admission is retired.

- **CONTRACT-RUNTIME-INDEX-OBSERVATIONS.** Prove [indexed scalar contract observations](wiki/spec/terminal-psi/byte_views.md#indexed-scalar-contract-observations) when the index is a runtime scalar term. Form the bounds obligation before substituting the clause. Exercise std's `send_string<const Count>` with `text[Count - 1] == 0`, including an out-of-bounds refusal.

## Universal interpreter cutover

<a id="early-terminal-jobs"></a>

- **EARLY-TERMINAL-JOBS.** (covers **TERMINAL-LAYOUT-WIRE-EVALUATION**, **TERMINAL-BUILD-SEED**) Run demanded constants, layout and access policies through [ordinary checked evaluation](wiki/spec/language/evaluation.md), using the selected target, source authority and actual inputs. Start at the [source invocation driver](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations.rs).

  - Complete const arguments on machine and type applications when the parameter is domain-qualified or depends on another parameter, and on machine applications that read a constant initialized by a call. [Argument evaluation](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/constants/arguments.rs)
  - Carry checked generic layout-policy results, target facts and reflected field geometry through plan-laid storage. Exercise a C layout and a constant domain fact end to end. [Layout evaluator](source-rust/subsystems/compiler/source-to-psi/driver/compile_time_invocations/fixed_layouts)
  - Admit an invocation whose guarded crash route is false for its actual operands ([evaluation.md](wiki/spec/language/evaluation.md): "Admission is invocation-sensitive"). With `machine guarded(x: u8) -> u8 reaches none crashes Trap x == 0 terminates { x }`, `const SAFE: u8 = guarded(3u8);` is refused with "the checked Trap crash route can occur": the generated caller publishes an unguarded `Trap` route for the call, so [crash admission](source-rust/subsystems/compiler/psi/semantics/src/evaluation_crashes.rs) never sees `3 == 0`. `guarded(0u8)` must stay refused, and moving the guard to `x == 3` must swap the two.
  - Lower a static callback that a generic machine supplies from its own nested declarations. In `machine outer<const N: u64>(input: u64) -> u64 { invoke::<helper>(input) machine helper(n: u64) -> u64 { N +% n } }`, `const SMALL: u64 = outer::<3>(2u64);` is refused with "supplied callable retains scalar const inputs" ([callback bindings](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/calls/callback_bindings.rs)); the direct call `helper(input)` freezes 5, and `outer::<9>` must freeze 11.
  - Replace syntax-based predicate approximation in [static argument canonicalization](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/declarations/generic_data/const_evaluation/values.rs) with evidence for the selected operator, substitutions and receiving domain. Preserve literal decoding and primitive normalization.

- **UNIVERSAL-INTERPRETER-CLOSURE.** Complete standalone [Psi execution](wiki/spec/build/embedding.md#standalone-execution-selection) with the admitted, target-specific host services.
  - Exercise the Linux kernel-import result chain through the [host adapters](source-rust/subsystems/interpreter/host) and process exit. Validate the selected syscall plan, propagate its result to the next operation and refuse an unavailable adapter before issuing it.

## Immediate product closure

- **STALE-SAMPLE-WORKAROUNDS.** Simplify the indexed deposit in [bank ledger](samples/cli/systems/bank_ledger), the transfer tripwires in [account ledger](samples/cli/systems/account_ledger), the caller-side products in [shapes area](samples/cli/proofs/shapes_area) and the stale case-payload notes in [bounded counter](samples/cli/proofs/bounded_counter). Prove each original sample oracle with `mbx xtask sample-gate --filter <sample>`.
  Acceptance: none of the four files describes a fixed bug or keeps code that works around one.

- **STAGED-BUILD-CAPABILITIES.** Complete [delegated filesystem authority](wiki/spec/build/scoped_execution.md#delegated-files-and-configuration) and retire `BuildInputs`, `BuildSource` and `BuildOutput`.
  - Supply the next compilation with its [frozen source view](wiki/spec/build/execution.md#generated-source-boundary). Check handwritten and generated Product files together. Migrate the `invalid_authored_selects_generated` control to demonstrate a successful handwritten import and retain the refusal when the running Build attempts that import. Remove `Build::include_source`, its per-file publication route and the generated-only checkpoint extension.
  - Carry the parent's selected filesystem implementation and owned or borrowed backing from the dependency description to the [child Build entry](wiki/spec/build/scoped_execution.md#supplying-a-filesystem-to-a-child-build). Check that an override reaches only its intended child, a helper can transfer an owned view, and a borrow survives only when its backing and access remain valid through child use.

- **FIXED-BUILD-IMPORTS.** Resolve the root's caller-supplied libraries and each child's configured libraries before that Build enters. A missing helper must reject before an earlier observable write, including when its use is conditional. Keep product registration and generated source for [later compilations](wiki/spec/build/scoped_execution.md#checked-build-stages). Migrate callers and probes, then retire self-registration through `Build::build_dependency` and the route that compiles a successor program after `Build::prepare_dependency`.
  - Preserve [ordinary interpreter suspension](source-rust/subsystems/interpreter/embedding/continuation.rs) in the same checked program. Keep a borrow and an outstanding return obligation live across a wait, use the borrowed value after resumption and make a repeated initializer observable.

- **LIBRARY-REPRESENTATION-FAMILIES.** Implement [representations](wiki/spec/layouts/plans.md#representation-applications), [aliases](wiki/spec/language/generics.md#transparent-type-aliases), [conversions](wiki/spec/layouts/codecs.md#representation-conversions), and [home layouts](wiki/drafts/designs/home_layout_optimization.md).

- **STATE-LOCAL-VALUE-FRONTIER.** (covers **REFERENCE-LOCAL-BINDING-ROUTES**, **BORROWED-STORAGE-RESTORATION**, **MATCH-SELECTIVE-LOWERING**) Carry exact value and backing identity through [borrows and restoration](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows) and [match joins](wiki/spec/language/patterns.md). Prove valid mutable moves and repairs across aliases and suspension. Refuse shared moves, unclosed linear obligations, stale backing and joins with incompatible loan states.
  - Accept the program in `scalar_reference_recasts_keep_their_backing_and_presented_fields` ([representation views](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/tests/representation_views.rs)) and run it again: with its returns written `$value`, the storage-flow replay refuses `$scalar` in `entry` with "reference descriptor has no current origin account" ([replay engine](source-rust/subsystems/compiler/psi/semantics/src/descriptors/replay/engine.rs)). The test passed before only because it read the references without `$`. Its overlapping-write twin is refused by the same limitation before its conflict is judged; restore its conflict assertion with the acceptance.
  - Type a `match` that joins references when the binding names no lifetime. `let chosen = match flag { true -> left, false -> right, };` and `let chosen: @mutable [u32] = match …` over two `@mutable [u32]` (or `@[u32]`) formals are refused in lowering with "typing match arm value type"; writing the lifetime, `let chosen: @'a mutable [u32] = match …` with `left, right: @'a mutable [u32]`, lowers and checks. The refusal is the arm type agreement check in [match lowering](source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/read/expressions.rs).
  - Write through a mutable view a `match` joins. `let chosen: @'a mutable [u32] = match flag { true -> left, false -> right, }; chosen[1u64] = value;` over two `@'a mutable [u32]` formals that each `requires ... .length > 1` is refused with "view loan has multiple possible backing regions" ([loan tracking](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/places/loans.rs)), because the joined view's loan must name one backing region. Reading the joined view's length, and writing through a local that reborrows one formal, check and run on both engines (`reborrowed_mutable_slice_local` in [owned record sequences](source-rust/application/tests/owned_record_sequence.rs)).
  - Move out of an element selected by a runtime index and restore it, as [ownership.md](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows) allows for any place reached through `@mutable`. `machine edit(p: @mutable [Cell; 4], i: u64) requires i < 4 { let old = move p[i]; p[i] = move old; }` is refused with "affine projection outside the static structural vocabulary" from the [affine checker](source-rust/subsystems/compiler/psi/semantics/src/plain_affine.rs), while `p[0u64]` and `p.left` are accepted. The restoring store may name another selector the premises prove equal (`requires i == j`, `p[j] = move old`), and a store through a selector proved different (`requires i != j`) displaces that other element.

- **MODULE-NAMESPACE-RESOLUTION.** (covers **RUNTIME-VALUE-GENERICS**, **STRUCTURAL-GENERIC-MATCHING**, **FINITE-GENERIC-DISPATCH**) Complete [module selection](wiki/spec/language/modules.md) and [generics](wiki/spec/language/generics.md).
  - Implement exact-file imports in both compilers. A file, a value and a type can share a name: `|cell|::read` selects the file, `cell.read` the value and `cell::Stored` the type. Check visibility, direct package dependencies and file-local imports. [Name resolution](wiki/spec/language/modules.md#names-and-imports)
  - Run the [file import example](samples/cli/probes/direct_file_imports) in the Omega-written compiler. The private machine must return 7 and the supplied callable must return 9.
  - Check public declaration addresses across file moves in the Omega-written compiler.
  - Apply a value binder to a runtime argument ([generics.md](wiki/spec/language/generics.md#value-binders-and-const-requirements)). `data Index<Limit: u32> [copy] requires value < Limit { value: u32; }` used as `let indexed: Index<limit> = Index { value: 8 };` in `machine probe(limit: u32) -> u32 requires limit >= 9 && limit <= 10` must be accepted and `Index { value: 10 }` rejected ([runtime index application](samples/cli/facts/runtime_index_application)). Static arguments (`Index<5>`) work; the runtime one is refused at `probe::entry::indexed` with "type record const argument does not inhabit its declared carrier": `source_record_application` (`read/source_records.rs`) accepts only an integer argument of the binder's carrier, so the application's `requires` must instead bind `Limit` to the captured value of `limit`.
  - Import generated files by their admitted package-relative filenames in the Omega-written compiler.
  - Migrate sources and fixtures to exact filenames and explicit qualifiers. Reject directory and item imports, backslash paths, and old module headers. [Files and imports](wiki/language_guide/chapter_15_modules_imports_visibility.md#files-and-imports)

- **DOMAIN-ACCESS-AUTHORITY.** (covers **DOMAIN-CARRIER-FORMATION**, **NOMINAL-FIELD-FLOW**, **DOMAIN-ISSUER-ROUTES**) Complete [carrier membership](wiki/spec/language/domains.md#carrier-exact-membership) for record, generic and indexed carriers (lowering already refuses `x in D` when a scalar `x` has another carrier than `D`), `[u8; N] in D`, [domain `access Policy`](wiki/spec/resources/access.md), and [repair facts](wiki/drafts/designs/invariant_repair_facts.md).
  - Implement [field enumeration](wiki/spec/language/reflection.md#field-enumeration) with the same `FieldInfo` as direct selection. Return absence for an out-of-range lookup and remove the fallback to graph node zero.
  - Add the [whole-subject and field-collection overloads](wiki/spec/resources/access.md#access-rule-subjects) to both reader and writer rules. Check a named field against its enumerated entry, and distinguish an empty selection on `u32` from a whole-subject restriction. Remove the core `TypeSchema::root` machine.
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

- **STRUCTURAL-BORROW-IDENTITY.** (covers **WRITE-ONLY-BORROW**, **PLACED-ACCESS-NATIVE-OPS**, **GENERIC-FIXED-ARRAY-VIEWS**, **BORROW-PROOF-CONVERGENCE**, **TERMINAL-SLICE-VIEW-VOCABULARY**) Carry structural loans and slice views through native and Psi, including nested element views. Exercise lending an owned sequence as `@[T]` when its elements themselves retain borrowed views. Preserve each backing and its exact loan through return and cleanup.
  - Check an unused `Observe<T>` machine taking `@T` under its declared lifetime and requirements. Run closed applications through both engines and reject mutation through a shared formal.

<a id="normalized-abi-lowering"></a>

- **NORMALIZED-ABI-LOWERING.** (covers **OPAQUE-BY-VALUE-BOUNDARY-ABI**, **FOREIGN-RETAINED-ARGUMENT-BACKING**) Complete [calling plans](wiki/spec/build/calling_plans.md) and [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody).

- **PROOF-KERNEL-CORE.** (covers **PROOF-CONTRACT-MIGRATION**, **QUOTIENT-THEOREM-LIFT**, **PSI-CLAIM-ADMISSION**) Complete [source proofs](wiki/spec/proofs/foundation.md) and [Psi claim admission](wiki/drafts/designs/psi_claim_admission.md). Report [coverage code 36](wiki/spec/build/compiler_request.md) with the obligation identity when the prover lacks a rule for its form.
  - [Proof kernel core audit](wiki/drafts/designs/proof_kernel_core_audit.md): what #2823 removed, the three general rules still needed, the changes to stage 05's clause readers and the build order.
  - Let Root Build configure the 64-summand expansion limit through the [proof-work policy](wiki/spec/build/configuration.md#proof-work-policy) and name the limit that stopped a proof. [Audit, "Step 4a as built"](wiki/drafts/designs/proof_kernel_core_audit.md#plan-order)
  - Step 4 judges every law of `core/nat.omg` (with `mod`, `div` and `gcd`), `nat_metric.omg`, `int.omg`, `rat.omg` and `cauchy.omg`: 96 laws. Still owed: a law about a machine defined on fuel is provable only if it holds for every amount of fuel ([owner question 7](OWNER_QUESTIONS.md#decrease-of-a-computed-subject)); `nat.omg`'s `<=` and `-` machines are owned by the data declaration, which the compiler rejects; a generic mathematical machine may not dispatch, be cited by another generic machine, or take a concrete machine; `extent.omg`'s `IntervalSet<Nat>` contract moves with step 5. [Audit, "Step 4 as built"](wiki/drafts/designs/proof_kernel_core_audit.md#plan-order)
  - Step 7 reads a quotient of mathematical data, proves equal classes for related representatives and judges `Quotient::lift` and `Quotient::define` by their selected theorems. Still owed: a definition for a lifted operation, so that a clause may apply one; `lift` with a constant, a computed argument, an omitted or a repeated formal; effectivity, which waits on [owner question 8](OWNER_QUESTIONS.md#quotient-mere-proposition-evidence); generic quotients and quotients of data that has storage. [Audit, "Step 7 as built so far"](wiki/drafts/designs/proof_kernel_core_audit.md#plan-order)
  - Put the abstract carriers, trait operations and cited laws of a generic machine's Identity goals in the [mathematical signature](source-rust/subsystems/compiler/source-to-psi/representations/lowered-program/README.md#the-mathematical-signature); `equality::discharge` still builds that table per obligation.
  - Extend alias-typed theorem parameters to accept instantiated citations, evidence projected from bundles and contracts with their own proof parameters. Check recursive citations against their substituted premises and descent obligation, and extend the logical contract beyond fixed integer and Boolean subjects. [Theorem proof parameters](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)

## Build-level behavior exclusions

- **BUILD-SEMANTIC-EXCLUSIONS.** Complete [behavior exclusion verification](wiki/spec/build/behavior_exclusions.md#implementation-acceptance) from authored Build to installed product. Use the existing [native verifier](source-rust/subsystems/compiler/psi-to-native/verification/behavior-exclusions).
  - Distinguish dormant bodies from selected callbacks, semantic service calls from physical effects and a stale replacement from the accepted installation. Check a saved publication independently before accepting its exclusions.

## Requirement-based tests

- **BUILD-TEST-COMPARISON.** Supply each registered test's configured arguments, stdin, service inputs and initial files independently to native and Psi. Check each engine's streams and declared files, values and fields against expectations, and compare those observations with `omega build --compare-tests`. Exercise matching exits with different output, identical wrong stdout, and absent versus empty files through the [registered test runner](wiki/spec/build/testing.md#comparing-native-and-psi-tests). Migrate callers, sample commands and repository-only program comparisons before removing `omega run --both`.

- **TEST-GROUP-FILESYSTEM-TARGETS.** Provide [delegated filesystem backing for test groups](wiki/spec/build/testing.md#test-runner-filesystem-slot) through their ordinary provider selection and [test planning](source-rust/subsystems/compiler/build/planning/registered_tests.rs).
  - Exercise canonicalization, metadata, directories and file I/O on the hosted targets. Include the Linux libc `realpath` case and report unavailable engines explicitly.
  - On native and Psi, run two tests that each exclusively create the same path, write and read different bytes, and leave a pre-existing host file there untouched. A child must observe writes through its delegated view. Reject a child's request for an ungranted raw host backing before test effects. Keep the service-free group runnable and check that application roots retain their original provider.

## Checked boundary topology

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver the [checked boundary topology contract](wiki/spec/packages/topology.md) and [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization).

## Embedding and interpreted components

- **EMBEDDING-SOURCE-TO-HOST.** (covers **INTERPRETED-CATHEDRAL**) Finish [embedding](wiki/spec/build/embedding.md), evidence-bearing qualified host inputs, and interpreted Cathedral.
  - Derive visible source parameters from the [installed entry contract](wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge) across source checking and native entry validation. Retire the fixed zero-or-two parameter switch. Reject missing arguments, forged backing and a mutable byte view of a protected field. Exercise the [Linux argument view](wiki/drafts/designs/structured_program_entry.md#how-linux-supplies-the-editors-pathname) through a static ELF entry. Check the indexed pathname, an empty argument, and refusal of a fabricated view or an out-of-range projection.
  - Admit an explicitly [runtime-bound slot](wiki/spec/build/component_publication.md#first-binding-of-a-runtime-bound-slot). Refuse a transitive use before successful bind, exercise a failed first bind followed by a permitted fallback, and retain V1 on failed replacement. Show a host built without either provider and a later independent supplier admitted by the frozen envelope. Refuse false installer success, use from a first-bind error handler, publication to a second instance with the first instance's authority, and a supplier with an unapproved import. Exercise an application-owned update during a blocked V1 call, a new V2 call in the same instance, and retirement after the read drains.

## Parallel language and compiler lanes

- **GENERIC-CALLABLE-FAMILIES.** In the [Rust compiler](source-rust/README.md#where-code-belongs), check a callable whose signature introduces its own type parameter. Use one `Visitor = machine<T>(item: T)` at two types and distinguish its binder from an enclosing `Visitor<T>`. [Callable contract](wiki/spec/language/generics.md#invocation-lifetime-families)

- **COMPONENT-SUBSTRATE.** (covers **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**, **RUNTIME-SIZED-ACTIVATION-CONTRACT**, **FILESYSTEM-PLATFORM-SERVICES**, **TR3-TR8**, **CHECKED-REPRESENTATION-REUSE**) Complete [authority review](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission), [bounded activation](wiki/spec/resources/activation_storage.md), [filesystem services](wiki/spec/build/platform_services.md), [component installation](wiki/spec/build/component_publication.md), [task runtime](wiki/spec/build/task_runtime.md) (`blocking-executor`), and [checked reuse](wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse).
  - Implement the core [network reach identities](wiki/spec/build/platform_services.md#network-reach) and highlight their `Network` parent in package review. Check a fixed-peer send against a caller-addressed UDP send. A helper that only transfers the sender adds no reach.
  - Carry [host network use and off-machine transmission](wiki/spec/build/permissions.md#physical-network-authority) through provider classification, package review, and physical exclusions. Version the affected catalogs and records. Distinguish recording-only, loopback, remote socket, and device-backed providers.
  - Implement and report [native image loading and foreign execution](wiki/spec/language/effects.md#native-images-and-foreign-calls). Audit every existing `NativeCodeExecution` declaration and caller against its actual operations, including load-time initializers. Migrate evidence and affected consumers before retiring the old identity.

## P5 - Cathedral over general Omega primitives

- **EXTENT-LIBRARY-TRANSFORMS.** (covers **CONSERVATION-CONTRACT**, **TERMINAL-CONTENT-CLAIMS**, **BOUNDARY-ISSUANCE**, **BUMP-ALLOCATOR-CANARY**, **UEFI-PHYSICAL-SEMANTIC-ENTRY**, **AP-BRINGUP**, **ADDRESS-TRANSLATION-CANARY**, **EXCEPTION-ROOTS-AND-TIMER**) Complete [extent/run contracts](wiki/spec/resources/typed_runs.md#split-and-merge-contracts), core primitives, Vec/bump, UEFI handoff and Cathedral AP/MMU/interrupt canaries.
  - Rename the [Arena service](source/library/core/arena.omg) to `Allocator` and publish size, alignment, backing, rights, content, lifetime and reclamation under [qualified geometry](wiki/spec/resources/extents.md#qualified-geometry). Distinct origins alone do not invalidate a claim. Check that implicit reclamation finishes infallibly without blocking or suspending.
  - Grow a container with an authorized selected allocator. Within-capacity operations use the backing already held. Growth and failure must account for the allocator state and rejected or replaced backing. Exercise explicit-instance allocators, lost authority and incompatible reclamation.
  - Deliver a [generic Vec](wiki/spec/resources/allocation.md#dense-collections-over-allocated-backing) over a live run and adjacent vacant storage. Exercise linear elements, zero-sized elements with cleanup, contained loans, failed growth and rejected initialization. Preserve order, exactly-once disposal, rejection recovery and constant-cost split/merge. Reconcile [core's carrier](source/library/core/vec.omg) with [the library implementation](source/library/alloc/vec/vec.omg) and keep its scalar growth/close and bump return/reset oracles.

## Rust compiler release closure

- **WINDOWS-ARM64-TARGET.** The owner ruled on 2026-11-23 that `windows_arm64` (Windows on AArch64, PE/COFF) is the fifth hosted target. Make the compiler produce working Windows ARM64 programs by generalizing the existing AArch64 ([target-aarch64](source-rust/subsystems/compiler/psi-to-native/targets/target-aarch64)) and Windows PE ([image-pe](source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-pe)) mechanisms, not by copying them. Follow [calling plans](wiki/spec/build/calling_plans.md), [entry roots](wiki/spec/build/entry_roots.md), [provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection) and [foreign bindings](wiki/spec/build/foreign_bindings.md).
  - Write hosted receiver observations (`omega run --inputs/--outputs`) on `windows_arm64` through an imported write, as `windows_x86_64` does; the AArch64 receiver entry writes only by Darwin and Linux system calls. Admit `windows_arm64` component DLLs in `ComponentImportLocator` and `emit_pe_shared_component`.
  - Remaining acceptance: on a real `windows_arm64` host, run the native programs and compare termination, stdout and stderr with Psi. Cross-target bytes do not replace this run.

- **RC-RELEASE-RECORD-AND-CLOSURE.** (covers **RC-BUILD-AND-PACKAGES**, **MACOS-X64-HOST-PROFILE**) Close all six [release gates](wiki/drafts/reference/rust_compiler_completion.md#release-matrix) on one revision across `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` and `windows_arm64`. Keep lock, closure, target and acceptance isolated per root. Exercise the [public CLI](wiki/spec/build/command_line.md).
  - Establish repository, source semantics, Build and packages, native matrix, diagnostics and representative programs independently. Record expected outputs, intended rejection reasons and matching-host native runs. Include saved-Psi consumption after the producer exits. A gate with missing coverage stays open.
  - Recheck the [two-borrow provider call](https://github.com/CathedralOS/Omega/pull/3733) under `TargetOperationCorrespondence`. The provider writes 41 and 43 to distinct `@mutable i64` arguments. Require both results, validator acceptance, and rejection if the provider record is substituted. [Target checker](source-rust/subsystems/compiler/psi-to-native/verification/stage_validators.rs)

## Omega-written compiler

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Compile the [Omega compiler](source/README.md) through its [public request](wiki/spec/build/compiler_request.md) and [construction application](source/application/construction/README.md).
  - Teach the Omega-written frontend to parse [named callable aliases](wiki/spec/language/machine_values.md#named-structural-callable-types) and represent their signatures. Check a factory returning a named reader and diagnose an inline reader signature at its use.
  - Expose the ordinary [project-build operation to a release program](wiki/spec/build/declarations.md#releases-with-independent-products). Have the CLI and library share project preparation, admission and publication. Exercise a release caller that consumes two committed artifacts and reports a failed producer before packing.
  - Implement package keys, ordering, graph and request semantics after envelope validation. The [OCREQ staging test](source/application/ocreq/tests/request_staging.py) distinguishes a valid request that reaches provision 25 from an oversized request that exhausts staging at provision 26. Add a successful artifact request while preserving malformed and over-budget outcomes.
  - Run the actual hosted construction application on [closed source sets with authored Build files](source/application/construction/README.md#including-authored-build-sources). Check selected ownership, exact package-relative paths and rejection of a Build row belonging to another package before effects.
  - Connect the selected `alpha_bootstrap` Console and ProcessExit providers and establish the entry's process domain. Compile and execute the construction application on the resulting environment. Follow its own source through missing checked operations, explicit arithmetic qualifications and loans.
  - Use the [construction procedure](source/application/construction/README.md#construct-the-actual-compiler) to produce a compiler tape and rebuild it with that compiler. Compare repeat outputs on identical inputs. The [bootstrap board](TASKS_BOOTSTRAP.md) owns the independent seed routes.
