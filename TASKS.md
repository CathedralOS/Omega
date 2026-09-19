# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion plan](wiki/drafts/rust_compiler_completion.md) defines the full
acceptance bar; this board identifies the remaining work, not a passing baseline.

| Start here | Purpose |
| --- | --- |
| [Immediate product closure](#immediate-product-closure) | Unchanged sample/canary programs and the dependencies preventing native execution. |
| [P1–P5](#p1---authority-roots-and-entry) | Entry/storage, materialization, portable evidence, ABI, and Cathedral customers. |
| [Parallel language work](#parallel-language-and-compiler-lanes) | Remaining accepted language surface; independent work can proceed when a product strategy is paused. |
| [Optimizer board](TASKS_OPTIMIZER.md) / [bootstrap board](TASKS_BOOTSTRAP.md) | Separate execution owners, not duplicated here. |

Keep each task's missing behavior, owner, real dependencies, and acceptance
condition. Delete completed work; Git holds checkpoint results and superseded
diagnoses. Link the specification for semantics rather than restating it.
Retain a dated/revision-bound failure only when it determines where to resume;
rerun that customer before assuming the old diagnosis still applies. New
evidence supersedes, never accrues: a landing or rerun replaces the dated
paragraph it makes stale, so an open item states its current frontier once.
Cite revisions as published on `main` (landing rewrites worktree SHAs) and
prefer symbol or test names over line numbers, which drift.
Live assignments belong in the [timestamped, expiring work-claim registry](tools/claims.md),
not historical board prose. Check `python tools/claims.py status` (`python3` on
macOS) before taking over a path; a retained checkpoint is resume material, not
an indefinite claim. Tag an added item's provenance on its first line:
`(new-scope)` for newly discovered work, `(split-of:<parent-item>)` when it
decomposes an existing item. Items added before 2026-09-14 are untagged.

The [workflow](AGENTS.md#workflow) governs scoped checks, strategy pauses, and
landing. A paused strategy is not a blocked language feature. Unsettled owner
decisions belong in [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md); implementation
difficulty alone is not an owner question.

## Ownership firewall

Follow the [connected pipeline](omega-rust/pipeline.md): Psi owns source semantics
through Terminal Psi; Omega owns target realization; backends own physical
details; Cathedral owns OS policy and structures. Complete ordinary operations
and transfers on the common graph, not another source-shape recognizer.

Compiler verification does not require an accepted package lock, and unfinished
native realization does not block source-package installation. Package acceptance,
build authority, and artifact verification remain separate obligations.

## Immediate product closure

These are the next product-level priorities for the maintained Rust
implementation. They take precedence over adding another evidence carrier that
has no exercising program. The finite definition of Rust-product completion is
the [Rust compiler completion plan](wiki/drafts/rust_compiler_completion.md).

- **SQUALR-HEADLESS.** Drive the independently versioned
  [Squalr application](samples/apps/README.md) through its nested package builds
  and native execution: geometry first, then the supplied-byte scan and filtered
  results. Preserve the essentially 1:1 Rust algorithms and complete dependency
  graph. The submodule's TASKS owns port work; this item owns integration and
  compiler blockers exposed by that application.

  The tracked app `7a272a896c85` still declares `console: Console` and pins std
  to `a91d878cb9252647d977c45787969b16e6ef937a`. Update the app to the settled
  [service contract](wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge):
  `Service<Console>`, with intrinsic binding establishment, not `in Bound`.
  Publish application changes in its own repository before updating the gitlink;
  private-repository access is required. Complete ordinary package update/review
  for the exact checkout and selected target. Do not delete acceptance or restore
  a stale checkout-specific lock to bypass review.

  Keep one integration owner and work from the actual application command:

  1. Reconcile the app's source/std pin and package acceptance, preserving its
     geometry and package graph. Recheck publication access rather than treating
     a previous session's 403 as a permanent compiler blocker.
  2. Run the native acceptance below and assign only the failure it now exposes.
     A source-only probe at `76478cbd6e` retained the composed entry and geometry
     bodies; the old missing-Unit-plan diagnosis is not a current native result.
     **ENTRY-CONTENT-ROOTS** owns service establishment and the hosted entry bridge
     in `native-realization`, `program-entry-plan`, and `external-roots`.
     Its existing `canary_suite::entry_and_abi::hosted_receiver*` controls still
     author `Service<Console> in Bound`; passing them does not establish the
     intrinsic-only source contract. Fix that shared path, not an app-specific
     eligibility exception. **MATCH-SELECTIVE-LOWERING** and
     **STATE-LOCAL-VALUE-FRONTIER** own newly witnessed operation joins.
  3. After geometry runs, drive the submodule's supplied-byte scan and repeated
     filtering through the real growable/partitioned storage and result path.
     Keep build-only packages explicitly unported; no fixed-capacity substitute,
     Rust FFI scanner, package flattening, or isolated-helper milestone replaces
     application progress.

  Acceptance: `python samples/apps/squalr/tools/verify.py native --timeout 600 --omega <binary>`
  executes the geometry app and prints `Squalr geometry: PASS`; then the
  supplied-byte path matches Rust's exact addresses/ranges, including overlap,
  tails, empty input, and repeated filtering. Retain results under the app's
  ignored `build/verification/`, with exact app/compiler pins and host.
  Package acceptance, Terminal publication, and hosted-entry test execution alone
  do not close either application milestone. Preserve disjoint receiver/stack/
  continuation storage and missing-establishment rejection. Ordinary artifact
  production has [conditional loading premises](wiki/spec/build/component_publication.md#products-and-authority),
  not a fabricated runtime installation grant or a required Rust supervisor.

- **MACOS-APPLICATION-PUBLICATION.** Close end-to-end acceptance of the
  [macOS publication contract](wiki/spec/build/macos_application.md), using the
  existing `compilation-report/src/package.rs` assembler and checked
  package/executable accessors. Do not build another packager or move assembly
  into Psi or instruction lowering.

  Remaining: execute `window_app`, `window_demo`, and `windowed_calculator`
  on macOS ARM64 through their authored builds and reported bundle paths.
  Start with the [window_app command and review flow](samples/gui/window_app/README.md).
  The last recorded ARM64 probe (`4707fde28b`) cleared staging but stopped at
  `InvalidUnitMachinePlan` for `Main::main`; rerun before assigning the
  current operation join to **GENERAL-CYCLIC-EXECUTION**. The
  `native_filesystem_canaries::gui_and_sample_apps::sample_window*` tests
  supply test-owned package acceptance, not ordinary CLI review. Their
  interactive-app checks only observe early failure or brief process survival;
  do not report them as proof that a window rendered or Finder launched it.

  Acceptance: the three procedural apps compile, publish one validated `.app`,
  and execute on the matching host, with observed window/render and completion
  behavior recorded separately from process-survival smoke coverage. Preserve
  identifier requiredness, deterministic bytes, cross-invocation publication,
  tamper/partial-output rejection, and flat-output regressions using
  `compilation-report` tests and
  `compiler --test build_target_activation -E 'test(activation_identifiers_and_publication)'`.
  Run the GUI cohort with
  `mbx nextest run -p compiler --test native_filesystem_canaries --no-fail-fast --no-tests fail -E 'test(gui_and_sample_apps::sample_window)'`;
  an unavailable ARM64 macOS host leaves runtime acceptance open.

  Native sidecar placement already has
  `pcc_publication::macos_gui_native_pcc_installs_the_inner_sidecar`, but its
  independent receiver verdict is still `Incomplete(UnsupportedEvidence)`.
  **PCC-PRODUCT-PUBLICATION** owns that evidence gap; emitted sidecar bytes do not
  satisfy requested PCC acceptance. Keep the bundle join covered when it closes.
  Application resources and `image_viewer` bundle-relative lookup are outside
  v1; do not silently change the working directory or claim their Finder coverage.

- **SAMPLE-CORPUS.** Close maintained `samples/cli|gui|uefi` programs through
  the [Rust product gates](wiki/drafts/rust_compiler_completion.md#release-matrix):
  checked semantics, each authored target's native product, and documented
  exit/output behavior on its matching host. Application submodules remain
  **SQUALR-HEADLESS** scope; language canary maintenance is **CANARY-CORPUS**.

  Integration owner: `compiler/tests/samples_compile.rs`, the documented sample
  commands, and the actual failing pipeline stage. Retain one customer command
  while its dependencies are repaired; do not close this item with helper tests,
  cross-emission, or another source-shape recognizer. Legitimate surface migration
  must preserve the sample's algorithm, storage, and observable behavior.

  Repair the harness before treating it as ordinary-production coverage:

  - `compile_native_and_publish` still copies package permission rows into an
    explicit receiving policy. Ordinary compilation now defaults that policy to
    absent; remove the harness's artificial requirement, retaining test-owned
    package acceptance and separate explicit receiver-admission controls under
    [the artifact/admission split](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission).
    A harness pass does not complete the user's project review.
  - The runtime oracle and `cli_mvp_preserves_both_lines_with_eof_and_enter`
    discard the published report and run `build_dir/omega-program`. Use
    `checked_native_executable_path()` from that report, including bundled paths;
    do not force every application's output name to fit the test.
  - Migrate bare service fields and `Service<Console> in Bound` to intrinsic
    `Service<Console>` establishment with **ENTRY-CONTENT-ROOTS**. Never weaken
    missing-provider or exact occurrence checks to preserve obsolete examples.

  Resume from focused runs, not the accumulated historical failure counts.
  At `d575c7e8e0` on macOS ARM64, the `cli_mvp` library oracle ran correctly
  and `print_squares` reached unsupported wrapping-u32 multiplication. Earlier
  `print_squares` probes stopped at the nonzero-divisor proof
  `1 <= self.place`; these are different checkpoints, not simultaneous claims
  about today's first failure. The current scalar legalization has no wrapping-
  integer multiply route, while scalar call lowering still reads only authored
  `crash.published()` in `scalar_graph/scalar_graph_lowering/{call_lowering,unit_operations}.rs`.
  **CRASH-CONTRACT** owns consistent inferred-ceiling publication; retain it as a
  dependency rather than another sample-local workaround.

  | Customer | Remaining integration and owner |
  | --- | --- |
  | [`cli_mvp`](samples/cli/basics/cli_mvp/README.md) | Ordinary package review and CLI execution without receiving-policy input; exact two lines, EOF/Enter, exit 0 on the hosted matrix. **ENTRY-CONTENT-ROOTS** owns service/entry custody; **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns any remaining production/admission coupling. |
  | [`print_squares`](samples/cli/basics/print_squares/README.md) | Wrapping arithmetic legalization in `target-operations-to-selected-instructions`, cyclic field/divisor facts under **NOMINAL-FIELD-FLOW**, and complete cyclic plans under **GENERAL-CYCLIC-EXECUTION**. Preserve nine computed rows ending in `081`, byte storage, and exit 0. |
  | `recursive_sum`, `framed_payload`, `dutch_flag` | Native exit 70, 60, and 70 respectively. **STATE-LOCAL-VALUE-FRONTIER** owns indexed primitive/sum storage and replacements; **GENERAL-CYCLIC-EXECUTION** owns typed slice views, recursive/cyclic transfer and ranking. Preserve saved reads, untouched siblings, shared payload loans and affine enum custody. No synthetic field IDs for scalar array elements or invented ranking for unranked cycles. |
  | [`generic_counters`](samples/cli/basics/generic_counters/README.md), `number_guess` | Intrinsic service migration and ordinary CLI/hosted execution, not reimplementation of already-supported counter calls or saturating arithmetic. Keep exit 16 and 70 respectively; macOS library probes are not the remaining hosts' runtime evidence. |
  | Text, indexing and match samples | Recheck `binary_search_viz`, `maze_flood`, `prime_sieve`, `multiplication_table`, `dice_histogram`, `calendar`, `dungeon_render`, `mandelbrot{,_zoom}`, `wire_protocol`, and `dungeon_crawler_cli`. Route actual failures to **NOMINAL-FIELD-FLOW**, **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**, **MATCH-SELECTIVE-LOWERING**, or **CML4**, not one task per source permutation. Encoding facts follow [library domains](wiki/spec/language/domains.md#byte-containers-and-encoding-domains), not recognition of the name `valid_utf8`. |
  | `cli/proofs/math_proofs` | Ordinary core multiset data/slice extraction, selected laws and checked proof terms; `core/seq.omg` is not a Bag implementation and equal lengths do not prove equal contents. Keep the false twin rejecting. |
  | Bounded Console input | Finish [bounded-input](wiki/spec/resources/bounded_input.md) composition in selected-dispatch and ordinary provider/call transport: exact destination/prefix, once-only effects, cleanup and truthful blocking/crash contracts. Keep prefix guards until count-to-extent evidence exists; test zero-capacity non-consumption, LF/EOF/Full, exact bytes and untouched tails, failed reads, alias rejection, invalid results and caller continuation. |

  Focused native command:
  `mbx nextest run -p compiler --test samples_compile --no-fail-fast --no-tests fail -E 'test(=samples_with_documented_exit_run_correctly)'`.
  Set `$env:OMEGA_SAMPLE_RUNTIME_FILTER = 'print_squares'` in PowerShell or prefix
  the command with `OMEGA_SAMPLE_RUNTIME_FILTER=print_squares` on macOS; this
  filter affects only that oracle. Use
  `-E 'test(=cli_mvp_preserves_both_lines_with_eof_and_enter)'` for exact byte/input
  coverage, and `-E 'test(=all_samples_reach_checked_trees)'` for the full checked
  cohort. Unset the filter for complete runtime coverage; an empty selection fails.

  **Scope pause:** resume `print_squares` implementation only with a plan from
  its complete source closure to native execution, not a third isolated helper
  milestone. Independent operation work remains actionable. Use the
  [Terminal production map](omega-rust/psi/compiler/terminal-production/README.md);
  native join/replay belongs to **TRANSLATION-VALIDATION** in `TASKS_OPTIMIZER.md`
  and package latency to **PACKAGE-PREPARATION-REUSE**. Acceptance requires every
  maintained sample to check and every applicable exit/output oracle to execute
  across the required hosted matrix. Record unavailable hosts explicitly;
  scoped reruns do not establish a new complete baseline.

- **CANARY-CORPUS.** Bring `tests/omega/{pass,fail,run}` and their
  `compiler/tests/canary_suite/` owners to the promised checked/native stages.
  **SAMPLE-CORPUS** owns maintained application examples, not this task's
  prerequisite; both use the same compiler operation owners. Repository/library
  passes do not establish corpus health.

  Start with the [focused canary selectors](AGENTS.md#running-one-test).
  For a refreshed distribution or closure run
  `mbx nextest run -p compiler --test canary_suite --no-fail-fast` on a fixed
  revision, with pass/fail filters unset. The last recorded full run,
  `771d0a8c2e` (2026-09-18, macOS ARM64), was 238 passing and 1155 failing
  tests; it is a historical starting point, not the current failure inventory.
  Keep detailed logs outside this board and rerun the affected cohort after a
  repair. Do not sync fixtures during a measured run or treat unavailable hosts
  as passing runtime coverage.

  Follow missing-plan diagnostics to the failing producer:
  `CheckedUnitEffectPlans::omissions` and
  `LoweringError::InvalidUnitMachinePlan::omission` retain the unavailable-callee
  chain; `LocalConstructionTrace` names the local phase/state/statement.
  `checked-trees-to-lowered-psi/tests/unit_plan_omissions.rs` pins this route.
  A phase label is not the cause: inspect the missing value/effect/ownership
  facts before classifying the failure as a control-builder gap. Preserve
  diagnostics, but another diagnostic-only milestone is not corpus progress.

  Route verified failures to existing owners:

  - Ordinary statements, indexed/aggregate values and stored origins:
    **STATE-LOCAL-VALUE-FRONTIER** and **MATCH-SELECTIVE-LOWERING**; cyclic
    transfers and plans: **GENERAL-CYCLIC-EXECUTION**. Complete the common
    sequencer and canonical paths, not another fixture-family recognizer.
  - Bare service fields, obsolete `in Bound` requirements, selected entry and
    exact receiver custody: **ENTRY-CONTENT-ROOTS**. Do not attribute absent
    service establishment to storage layout or fabricate a provisioning row.
  - Declared field/encoding facts: **NOMINAL-FIELD-FLOW**; borrow obligations:
    **BORROW-PROOF-CONVERGENCE**; selected non-array indexing/operators:
    **OPERATOR-MACHINE-SUPPLY**.
  - Trapping and cross-sign conversion cases under `core/numeric_*`:
    **ARITHMETIC-POLICY-REALIZATION**. Other scalar legalization belongs to
    `target-operations-to-selected-instructions`, not a Unit-body workaround.
    Reach and crash-envelope failures retain their actual contract owners.

  Audit fixtures against the spec before weakening checking. The bounded
  `proofs/proof_inductive_climbing_sum` and its negative
  `proofs/inductive_climbing_sum_unbounded_accumulator` demonstrate why
  [Exact intermediate arithmetic](wiki/spec/language/numeric_values.md) still
  owes a carrier bound even when the theorem uses `embed`. Repair invalid
  positive fixtures with real premises and negative controls; do not reclassify
  valid accepted-language programs as checked-only merely to make the suite green.

  The umbrella's `production_compile` still mirrors accepted package permission
  rows into a receiving policy. Remove that coupling from ordinary-production
  fixtures, preserving explicit receiver-admission tests and package acceptance
  under **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**. A test-owned approval is not user
  project review. Acceptance: the complete corpus reaches its declared stages,
  negative controls reject for the intended reasons, runtime oracles execute on
  their matching hosts, and roster/coverage guards remain intact. Remove this
  item only when those checks pass, not when every failure has an owner.

- **TERMINATION-RANKING-CHECKS.** Finish exact rank-range transport under the
  [termination contract](wiki/spec/language/termination.md), without extending
  source-pattern recognizers for each new arrangement of calls and records.
  Owners: `typed-trees-to-checked-trees/src/checks/termination/ranking/`,
  validation's `proof_contracts/contract_entailment/ranking_range/`, and
  `machine_calls/call_cycles/runtime_ranking/`.

  Remaining work:

  - Carry a ranged member's established invariant into ordinary callee
    requirement checking at subordinate component-call sites.
    `checks/contracts/call_bounds/context.rs` currently applies the
    ranking-to-requires bridge only to self-calls. Do not assume an entry
    `requires` remains true after internal arrivals or use the callee's
    requirement as its own proof.
  - Generalize endpoint formation and conservation through exact checked
    value relationships. The independent endpoint fallback still requires one
    state and immutable scalar/direct-field inputs; mixed-component endpoint
    discovery handles direct integer inputs and arithmetic trees. Nested
    reference boundaries and non-polynomial substitutions need their actual
    formation, equality, and write-preservation evidence, not polynomial
    cancellation or positional guesses.
  - Replace residual rank-role discovery limits with explicit arrival
    correspondence where the program supplies enough evidence. Unique nested
    carriers, borrowed roots, moved scalar/slice/record copies, custom
    call-component views, and produced scalar/slice rank facts already have
    implementations; do not rebuild those as new feature slices. Ambiguous
    copies must remain rejected unless the checked correspondence identifies
    the ranked value. A shared nominal type or a convenient decreasing copy
    is not proof of that identity.
  - Use STATE-LOCAL-VALUE-FRONTIER's checked computation route to retire
    generated operand-call states. Do not add termination-only provenance for
    artificial source edges. This dependency does not block independent
    contract-bridge or endpoint work.

  Acceptance: exercise valid named-state and mutually recursive call-component
  customers through ordinary checking/lowering, with exact subject/view/range
  identity. Include a subordinate call requiring the established range,
  endpoint transport across a state arrival, and a valid projected/borrowed
  rank beside unrelated computation. Changed endpoints, stale copied premises,
  intervening direct or nested-call writes, overflowing intermediate arithmetic,
  and nondecreasing cycles must reject. Preserve the private-witness/public-
  guarantee split and the rule that every complete call cycle descends.
  Start with `src/tests/termination/rank_ranges/` in the checked stage
  (especially computed field limits, field coordinates, endpoint arithmetic,
  and call components), plus matching `tests/omega/{pass,fail}/termination/`
  controls; source inspection is not a current passing-test claim.

## Automatic service reach

Finish portable coverage of
[static callback reach dependencies](wiki/spec/language/effects.md#static-callback-reach-dependencies).
The source-side finite-union inference, closed application retention, selected
contract bounds, and two-sided emitted-call replay already exist. Remaining:

- Retain the original dependency and full binder telescope when callees are
  inlined or absent from the emitted machine set. The current
  `prune_incomplete_closed_reach_applications` drops incomplete annotations
  and their dependents; preserving ordinary executable semantics this way is
  not completion of dependency coverage or proof of source correspondence.
- Replace frontend-local template identities with independently checkable
  original-contract projection evidence. Producer-side replay against the
  retained source graph is not source-free verification, and commitment hashes
  identify a projection rather than authenticate its derivation. Reuse exact
  specialization/contract identities and the PCC ledger/bridge work; do not
  invent a parallel selection identity or a second proof system.

Owners: `checked-trees-to-lowered-psi/src/retention/closed_reach_applications.rs`,
`validation/src/machine_calls/static_machine_call_contracts.rs`, and
`terminal-verifier/src/validation/reach_applications.rs`. Keep ordinary semantic
replay distinct from optional producer-correspondence certification; neither
test coverage nor matching hashes justify a full generic-PCC claim.

Acceptance: publish, discard source/producer state, reload, and independently
check the same named traversal with pure and Console callbacks, nested/private
helpers, recursive components, selected generic schemas, and unused selections.
Inlined/missing callees must not silently remove requested evidence coverage.
Reordered calls and equivalent helper extraction preserve interface identity;
adding helper reach changes it and invalidates stale evidence. Reject changed
projections, redirected same-shaped calls, incomplete telescopes, missing direct
boundary declarations, and selections exceeding requirement bounds. Opaque
calls retain conservative contracts; suspension/blocking remain separate.

Keep the existing `service_reach_contracts` lowerer and codec regressions as
controls, with the
[nominal callback](tests/omega/pass/effects/nominal_callback_dependency/README.md),
[generic schema](tests/omega/pass/effects/generic_callback_schema_reach/README.md),
and [linear structural callback](tests/omega/pass/effects/structural_callback_reach/README.md)
customers. The pure callback's ordinary callers and hermetic evaluation must
see its specialized empty row without consuming the original generic definition.

## Semantic reflection

Connect [semantic reflection](wiki/spec/language/reflection.md) to authored Omega
and ordinary checked calls through Terminal replay. The schema, scoped-selection,
and visitation-plan helpers in
`build-time-evaluation/src/machine_execution/reflection/` exist, but their
construction/composition callers are Rust tests rather than a source-driven
reflection route. Reuse them where their contracts fit; helper-only tests do not
complete this task.

First deliver an inspector and serializer through the same source-level query,
policy evaluation, and per-member call mechanism. Psi owns elaboration and
checking; ordinary library code owns encoding/inspection policy. Derive query
authority from the actual lexical/package context, not a caller-supplied authority
flag. Replace nominal-head/attached-name matching in `selection/requirement_resolution.rs`
with complete selected application and callable-contract checking, preserving
qualification, generic arguments, lifetime relationships, and explicit selection.
Generate real field subloans, fresh context reborrows, active-case dispatch, and
ordinary call/resource evidence; a list of resolved operation names is not that
execution plan. Freeze only owned evaluation results and retain their dependencies
for independent replay.

Then complete recursive derivation, explicit runtime metadata/adapters, and
authorized Placed access under the same contract. Use exact pending derivation
identities without assuming their obligations; runtime recursion needs provisioned
work storage. Fixed named callbacks can proceed now. Selection-dependent evaluation
uses Automatic service reach; anonymous machines and format-specific compiler
operations are not dependencies.

Acceptance: compile and run the inspector/serializer from Omega source, publish
and reload their Terminal product, and reject corrupted retained evidence.
Use a 40-field/five-type record with reusable type rules and one member override;
zero/multiple/wrong-type/stale-key/insufficient-contract selections reject.
Exercise owner-delegated visitation across three packages and reject unauthorized
enumeration. Include qualified and erased fields, borrowed owned claims, zero-sized
fields, empty records, distinguishable nullary cases, common fields once, and
active payload only. Test loan escape rejection, dynamic-index relationships,
recursive reference types, snapshot independence, and explicit exhaustion rather
than truncation. Descriptions never create grants or access; adapters, decoding,
and editing still require ordinary authorized access/construction contracts.

## Scoped build execution

Implement [scoped build execution](wiki/spec/build/scoped_execution.md) for
ordinary code generators and the topology composition customer. These are
accepted contracts, not claims of implementation. Existing
`BUILD-ADMISSION-CHECKPOINT` owns retained frontend/generated-source replay;
extend that route, not a second build language or plugin executor. Any semantic
or trust amendment found here or later goes through [owner questions](OWNER_QUESTIONS.md).

- **BUILD-DEPENDENCY-PURPOSES.** Complete the
  [separate checked contexts and prerequisite graph](wiki/spec/build/scoped_execution.md#two-checked-contexts)
  for host/build and product code. Purpose-tagged acquisition, review, lock rows,
  alias isolation, and build-only target selection already exist.

  - Check a package/file separately when both purposes select it.
    `source-files-to-assembled-syntax/src/frontend/mod.rs::build_only_packages`
    currently leaves dual-purpose packages in product scope, while
    `claim_import_scope` rejects a root-local file used in both scopes.
    Source bytes may be shared; checked instances, target rows, generated
    bundles, provider plans, and admission/cache identities may not be
    conflated. Replace the one-scope-per-source model rather than adding
    import-order exceptions.
  - Schedule each helper's own build activation in its correct execution
    profile and prerequisite graph. Package-manager reconciliation/closure
    validation and `package-compilation` currently reject non-root build
    dependency rows. Remove that implementation restriction only with scoped
    activation ownership and cross-purpose cycle detection. An imported
    helper still cannot declare dependencies for the caller or import another
    activation's build entry.
  - Retain exact purpose/profile/target and accepted authority through
    acquisition, review, lock recovery, generated-source handoff, and checking.
    Extend the existing owners; no second dependency resolver or build executor.

  Acceptance: real acquisition and CLI multi-file builds use the same std/helper
  sources in both contexts with different target-scoped implementations, plus
  a helper that needs its own build dependency. Reject cross-purpose cycles
  before affected execution, conflicting within-scope aliases, missing edges,
  and stale or cross-profile cached outputs. Cross-scope aliases may differ;
  removing one edge affects only its authorized selections. Reuse
  `omega/tests/package_commands/build_purposes.rs`,
  `package-manager/tests/dependency_purposes.rs`, and assembled checking's
  `checking/execution_profile_tests.rs`; include physical and generated files.
  Existing dual-edge tests do not establish dual-context target checking.

- **BUILD-PRODUCT-REFERENCES.** Finish
  [non-executing product selection](wiki/spec/build/scoped_execution.md#selecting-product-declarations-without-executing-them).
  Entry/provider/schema queries, opaque descriptions, delegated entry binding,
  and exact-symbol final admission exist. Remaining:

  - Resolve qualified product paths through the query author's authorized
    product dependencies and exact expected slot/requirement/application,
    not the caller's host imports or a same-package name scan. Current
    `checked-interpreter/src/interpreter/evaluator/product_{entries,providers,schemas}.rs`
    implements the narrower same-package route. Preserve the frozen authored
    frontier, purpose/target identity, and visibility across description use
    and final admission; source names and evaluator table indices are not
    durable selection authority.
  - Admit computed description operands and Build receivers through ordinary
    checked call-result authority, effect traversal, and loan accounting.
    `typed-trees-to-checked-trees/src/authored_selections/finalization.rs`
    currently requires retained symbol-rooted places. Do not bypass those
    checks with a Build-specific call-result recognizer or grant authority
    from the declared result type alone.
  - Extend `build-evaluation/src/admission/selection/root_bindings.rs` and
    the existing provider/description owners, with BUILD-DEPENDENCY-PURPOSES
    for separate contexts and BUILD-ADMISSION-CHECKPOINT for source custody.
    Do not add a general compiler-query interface or allow target execution.

  Acceptance: a multi-file foreign helper binds an owner's restricted private
  entry description, including one returned by an ordinary checked helper;
  a qualified query selects a public declaration in an authorized product
  dependency. Wrong scope/target/slot, stale activation, lookalike operations,
  sibling-private enumeration, forged descriptions, description-to-callable
  conversion, and same-build generated/layout cycles reject. Preserve
  `compiler/tests/build_target_activation/foreign_helper_product_queries.rs`.
  Its computed-result negative returns a forged `ProductEntryRef {}`: it must
  still reject after an independently compiler-issued result becomes supported.

- **BUILD-SNAPSHOT-OUTPUTS.** Finish the
  [captured-input and committed-output contract](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem)
  across ordinary compilation, not only package review. Snapshot reads,
  required-output settlement, private-staging cleanup, and audit reporting
  already have implementations and integration tests. Remaining:

  - Bind standalone builds to an explicit caller-authorized source inventory,
    and support named immutable extra inputs assigned to exact dependency
    occurrences. `PreparedCheckedSource::check` currently binds a snapshot
    only when package inputs carry canonical Source metadata;
    `BuildSnapshotRequest` carries an output roster, not a capture request or
    named-input map. Do not silently capture the whole working directory,
    widen a dependency's inputs, or substitute the live-host filesystem.
  - Carry sealed completed artifacts through compiler result construction and
    final publication for artifact-only builds and executable companions.
    The current `build_snapshot_outputs` and CLI audit tests inspect checked
    observations/staged custody; they do not establish a committed compiler
    output set after all requested product checks. Preserve per-target
    outcomes and expose no partially successful set after later failure.
  - Complete host-backed capture/staging assurance and Windows execution
    coverage. Validate coherent capture and race-safe confinement (or the
    exact adequate isolation premise), logical path distinctions, inert links,
    and failure cleanup. One traversal plus matching lengths is not by itself
    proof of an immutable snapshot. Preserve existing macOS controls and
    record unavailable hosts rather than treating source inspection as a pass.

  Owners: `package-compilation/src/source_snapshot.rs`, the existing
  `build-output` and `build-evaluation` custody owners, assembled checking,
  and compiler/compilation-report publication. Reuse
  BUILD-ADMISSION-CHECKPOINT for admitted-source/generated-source replay and
  BUILD-DEPENDENCY-PURPOSES for occurrence identity. No second executor,
  live-host grant extension, or persistent writable cache.

  Acceptance: an acquired ordinary generator reads a narrowed template and
  publishes a required file via both artifact-only and companion builds.
  Run `compiler/tests/build_snapshot_outputs.rs` and
  `omega/tests/package_commands/snapshot_outputs.rs`, then the actual
  compilation/publication route. Cover negative lookups and metadata,
  substitution/link escapes, sealed mutation, cross-occurrence receipts,
  retry, omitted outputs, interruption, final-check failure, and two packages
  or targets using identical logical names. Measure retained state and compare
  the separate-tool route required by the spec; do not infer publication,
  confinement, or usability from observation-only tests.

## Build-level behavior exclusions

Implement [the accepted exclusion contract](wiki/spec/build/behavior_exclusions.md)
for one source library whose checking/no-op assertion selection changes product
crash behavior without edits to public ceilings. Assertions remain ordinary calls;
there is no special Assert cause or global permission to violate callable contracts.
These tasks use existing build selection, portable semantics, provider and
artifact-verification owners, not an assertion-specific interpreter or duplicate IR.

- **BUILD-SEMANTIC-EXCLUSIONS.** Finish
  [semantic absence admission](wiki/spec/build/behavior_exclusions.md)
  for the selected composition. Typed crash/service sets, canonical union,
  static/bounded-dynamic closure checks, provider-body joins, and the checking/
  no-op assertion and logger fixtures exist. Remaining:

  - Record actual evaluated exclusion selections under the live root Build
    authority. `build-evaluation/src/admission/declarations.rs::harvest_behavior_exclusions`
    instead applies selections anywhere in the static call scope unconditionally
    and requires literal crash cases. The spec permits ordinary evaluated
    selection; a helper or untaken branch is not itself an executed selection.
    Reuse checked invocation/occurrence evidence, not a second interpreter or
    syntax-pattern permission.
  - Complete absence evidence for the full admitted entry/call closure,
    including callbacks, cleanup, generated entries, and all admitted dynamic
    targets. Parameter dispatch still reports missing evidence; preserve that
    rejection until its exact target set or conservative contract suffices.
    Sound guard evidence may establish unreachable behavior, but optional
    optimization and broad public ceilings are not absence proofs.
  - Close the assertion customer's ordinary Unit/native lowering gaps through
    CRASH-CONTRACT and the owning pipeline lanes. The native fixture currently
    expects `UnsupportedControlFlow`, not the retired
    `UnsupportedBoundaryCrashContract`; a Terminal verdict is not executable
    completion. Add a direct Unit crash control instead of accepting only the
    scalar-helper workaround.

  Owners: `build-evaluation/src/admission/behavior_exclusions.rs` and
  `checked-compilation-to-terminal-artifact/src/terminal_artifact/behavior_exclusions.rs`,
  with Psi checked operation/guard evidence. BUILD-EXCLUSION-REALIZATION owns
  physical classes and installation, not a duplicate semantic checker.

  Acceptance: compile the unchanged assertion library/public Trap ceilings with
  checking and no-op implementations; only the no-op composition passes a Trap
  exclusion, with optional optimization on and off, through native publication.
  Eager argument traps and unrelated crashes still reject. An ordinary silent
  logger can pass a service exclusion; an actual boundary invocation cannot pass
  merely because its provider is silent. Test conditional/helper selections and
  independently replay retained evidence, rejecting changed entries, providers,
  targets, scopes, policies, and omitted coverage. Distinguish prohibited possible
  behavior from insufficient evidence, and never weaken intermediate contracts.
  Reuse `tests/fixtures/packages/behavior-exclusions/` and compiler tests
  `behavior_exclusions.rs` / `build_behavior_exclusions.rs`.

- **BUILD-EXCLUSION-REALIZATION.** Enforce requested physical-authority exclusions
  under the [exclusion contract](wiki/spec/build/behavior_exclusions.md). The
  retained `BehaviorExclusions` currently carries crash causes and services, not
  physical classes. Add evaluated physical-class selections and their retained
  evidence in `build-evaluation` and `compilation-report`; integrate final checks
  through `compiler/native-realization/src/native_realization/`.

  Reuse `terminal_authority_policy/` and the existing mechanism-closure review.
  Classification is not receiving permission: a requested absence guarantee must
  be checked even with no receiving permission policy. Do not invent a second
  classifier, synthesize receiver approval, or claim that semantic exclusion replay
  establishes physical absence. `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW` owns receiver
  admission; this task owns the independently requested build guarantee.

  Bind the exclusion scope, exact selected mechanisms, classification identity and
  final product to independently replayable evidence. Carry the envelope through
  rebinding/replacement using `COMPONENT-SUBSTRATE` and
  `WIRE-RUNTIME-AND-INSTALLATION` for custody and lifecycle. Keep build-host
  activity and target children separate; build code cannot inspect its unfinished
  executable. Define the source/protocol fields before claiming portable support.

  Acceptance: source-built products distinguish no-Console from no physical output,
  including a silent Console provider. Unknown classifications and mismatched
  evidence cannot count as absence; failed final checks publish no successful
  product. Check with and without receiver permission policy and optional
  optimizations. Source-free replay preserves the same verdict, and replacing a
  benign provider with excluded behavior rejects. Exercise actual provider and
  installation controls on each available Windows/macOS host and report the
  unavailable legs explicitly.

## Checked boundary topology

Implement the [reference-package contract](wiki/spec/packages/topology.md), not
a compiler graph stage. `COMPONENT-SUBSTRATE` owns the complete verified component
consumer below; `WIRE-RUNTIME-AND-INSTALLATION` owns generic executable custody.
Package policy and orchestration stay in ordinary Omega libraries, with native
details in providers. A topology-specific IR or new trusted graph axiom is not
an implementation shortcut.

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver the ordinary Omega build-only topology
  package and payment composition project under the
  [reference contract](wiki/spec/packages/topology.md). Reuse the Rust reference in
  `omega-rust/omega/packages/topology/`: `plan_composition.rs`,
  `plan_verification.rs`, and `deployment_plan/` already implement bounded graph
  normalization, fixed policies, certificates and the versioned codec. Do not
  restart those algorithms or introduce a compiler-owned graph stage.

  Remaining work:

  - Connect admitted component facts to the package producer and independent plan
    consumer. The reference codec decodes a caller-supplied `VerifiedComplete`
    tag/closure digest; `verify_plan` checks that model, not the component artifact
    establishing its inventory. A valid tag is not proof of completeness.
    Reconstruct and bind endpoints, entries, authority, profile and assumptions
    from the actual verified descriptions; reject forged or substituted records.
  - Reuse `compiler/src/compiler/package.rs`'s description producer and
    `build-evaluation/src/provider_settlement/independent_components.rs`'s
    consumer. The shared representation/verifier now lives in
    `backend/artifacts/component-description/`. `COMPONENT-SUBSTRATE` owns
    missing complete-description and evaluated-consumption support, not a parallel
    topology census. Existing `Independent` provider selections have a verified
    component join; they are not unconditionally unsupported.
  - Author the Omega package over admitted input bytes and generic required
    outputs. `tests/fixtures/packages/build-scope-topology` exercises imports and
    output settlement, not payment-plan verification. `BUILD-SNAPSHOT-OUTPUTS`
    owns captured-input and artifact-only publication gaps. Do not bypass input
    confinement to read dependency-adjacent artifacts or accept handwritten
    inventories as a substitute.

  Acceptance: the composition build admits three real component descriptions and
  publishes `payments.plan`; a source-free consumer verifies it against separately
  supplied current `TopologyRequest`. Preserve the
  [policy controls](wiki/spec/packages/topology.md#diagnostics-and-implementation-acceptance):
  direct/indirect bypass witnesses, cycles, instance distinction, disconnected
  selectors, duplicate bindings, stale subjects, forged completeness, missing
  policies and corrupt plans. Unselected policy executables are never loaded.
  Graph typechecking and published bytes establish neither complete component
  facts nor runtime installation; the latter belongs to the next task.

- **TOPOLOGY-PRIVATE-PIPE-INSTALLATION.** Run the
  [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization)
  through an Omega-authored installer and Windows/macOS providers. Dependencies:
  `TOPOLOGY-PLAN-VERIFICATION`, `COMPONENT-SUBSTRATE`, and
  `WIRE-RUNTIME-AND-INSTALLATION`. Keep orchestration in the package, physical
  mechanisms in providers, and generic executable custody in its existing owner.

  Reuse `packages/topology/src/topology_installation.rs` and its subordinate
  mediation, frame and pipe modules. The reference has single-use authorization,
  activation/cleanup/replacement sequencing and real pipe creation. Its
  `a_full_installation_flows_over_real_private_pipes` test still uses
  `SimSupervisor`; it is not three-process confinement. Supply actual executable
  admission and the OS-backed supervisor, bind physical holders rather than
  trusting caller-supplied instance numbers, and add operation/payload schema
  checks beyond the existing bounded frame envelope.

  Acceptance: three checked processes receive exactly their assigned endpoints,
  with all imports covered and general inheritance disabled before entry opens.
  One request/response pair per binding, one outstanding request, no delegation,
  discovery or retry to another peer. Attempt an ungranted invocation and a
  substituted mapping; both must refuse. Invalid frames/EOF/peer failure close
  the binding. Preparation failure returns or cleans custody; partial activation
  retains supervision until quiescence or explicit cleanup failure, never a
  success receipt. Replacement validates current authorization before stopping
  the old generation and quiesces it before starting the new one. Report actual
  Windows/macOS runs and unavailable legs, with exact loader/OS assumptions;
  ordinary process spawn, graph tests and in-process pipe I/O are insufficient.

## Process-exit contract

Finish the [canonical process-exit contract](wiki/spec/language/process_exit.md)
through Terminal Psi and source-free verification. Core/std declarations, exact
canonical recognition in the checked interpreter, and selected hosted exit
realization already exist. They do not establish the portable contract:
`terminal_module/control_flow/termination.rs` has no external-completion
terminator, and the Terminal trace codec admits only an empty terminal-external
group.

Remaining work:

- Psi checking/lowering must retain the exact canonical requirement, bound-domain
  authority, status argument and no-normal-successor outcome. Carry that through
  Terminal representation, codec, independent verifier, observations and
  interpreter. Do not encode successful exit as an ordinary Unit call, crash, or
  provider-name convention.
- Model conditional helper exits compositionally: returning paths retain results,
  cleanup and postconditions; exit paths abandon in-domain obligations without
  discharge receipts or a post-exit frontier. Automatic cleanup must return.
  Root return must settle/transfer task custody, and survivor crossing contracts
  must account for domain death without inventing a whole-process resource census.
- Preserve exact `i32` semantic status and ordered output through provider
  conformance and native realization. Reuse `SelectedProcessExit` custody and the
  canonical host canaries; the current hosted-exit target support excludes Windows.
  Complete and exercise the missing supported-host realization.

Acceptance: source-produced unconditional and conditional/helper exits replay
independently after serialization. Tampered identities, arguments, completion or
provider evidence reject. Cover missing authority/reach, lookalike requirements,
returning/aborting substitutes, missing progress premises, cleanup exits and
unsupported survivor contracts. Interpreter exit ends only its simulated domain;
native tests record each available host's status mapping and preceding output.
Report unavailable hosts, not a cross-target emission as a runtime pass.

The ordinary-production/receiver-admission split is already implemented; further
receiver work stays in `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW`. General completion
syntax and other terminal services are not prerequisites.

## P1 - Authority, roots, and entry

Owners include
`wiki/spec/resources/authority.md` and
`wiki/spec/resources/storage.md`.

- **ENTRY-CONTENT-ROOTS.** Finish authored receiver entry under the
  [entry contract](wiki/spec/build/entry_roots.md), principally the settled
  [intrinsic service validity](wiki/spec/build/component_publication.md#service-bindings-and-era-entry)
  migration and activation/completion lifecycle. Owners: target package source
  assembly, `program-entry-plan`, Psi `terminal-production`, Omega
  `compiler/native-realization`, `backend/images/image-emission`, and
  `backend/runtime/external-roots`. Targetless checks select no physical entry;
  deployment cannot substitute a semantic continuation for the physical adapter.

  - Replace qualification-based service admission with exact closed `Service<R>`
    identity. `core/service.omg` still declares `Bound`, and
    `typed-trees/src/typed_trees/calls/service.rs` requires it. Retire that
    service-only domain and its acceptance paths, not general domains. Carry exact
    requirement, occurrence and selected-plan custody through checking, Terminal,
    erased Fused fields, native settlement and independent replay. No default-domain
    feature, bare-trait alias or fabricated establishment row is needed.
  - Migrate library, samples, canaries and Squalr from bare boundary-trait fields
    and `Service<R> in Bound` to the intrinsic carrier; reject bare fields during
    source checking rather than after native bridge planning. Preserve negative
    controls. Epsilon's separately specified sealed Console is not this surface.
  - Complete receiver nominal-cleanup and callback/signal occupancy through actual
    activation/completion. Reuse `receiver_eligibility.rs`,
    `image-emission/src/hosted_receiver.rs` and
    `ProgramLocalRootInstallationLedger`, including installed aggregate extent
    materialization. Do not recreate existing bridges: macOS ARM64, Linux x86-64,
    Linux ARM64 and Windows x86-64 routes exist. Windows currently has storage-only
    coverage; its exit-provider gap belongs to the process-exit task. Records,
    arrays (including record arrays), IEEE leaves and zero-valid first sum cases
    already have eligibility/bridge coverage; investigate a concrete rejected
    receiver before adding another storage profile.

  Acceptance: `number_guess` with `console: Service<Console>` exits 70, and
  `cli_mvp`/`generic_counters` retain behavior without service qualifications.
  Run published processes with no test-supplied `self`, retaining host-gated
  `entry_and_abi::hosted_receiver*` checks and explicit unavailable-host results.
  Missing/incompatible supply, ordinary literal/zero-based service construction,
  lookalikes, redirected continuations, non-ZII state, bad backing/alignment,
  overlapping partitions and stale occurrence/epoch reject. Move/borrow/record
  forwarding preserves distinct service applications and affine custody; unused
  type declarations alone demand no provider.

  Missing/substituted service or receiver evidence must reject after erasure and
  replay. Erased fields cannot erase initialization or cleanup obligations.
  Compose application, bridge, provider and newly admitted callback stack demand;
  loader stack-size metadata is not remaining-stack evidence. Preserve exact
  target/source contracts and runtime storage/continuation joins; installation
  owns actual occurrence custody, not compilation. Descriptor-call customers use
  their separate descriptor dependency; do not expand into Independent service
  installation merely to migrate Fused carriers.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Execute the source-authored two-surface UEFI
  bootstrap under [source-owned firmware adapters](wiki/spec/build/uefi_entry.md#authored-firmware-definitions-and-adapters).
  Keep physical firmware arrival distinct from the semantic program continuation;
  the compiler emits only the necessary entry shell and generic target primitives.

  - Replace `target/src/uefi_{system_table,boot_services,loaded_image}/`'s Rust
    field catalogs with target-package declarations, evaluated layout policies,
    constants and integrity checks. Migrate `backend/plans/program-entry-plan`
    and `external-roots/src/platform_bringup/uefi_bootstrap/` consumers, then
    delete the duplicate production catalogs.
  - Replace `program_entry_physical/exact_uefi.rs`'s duplicated physical-policy
    recipe with source-derived plan/evidence replay. Preserve exact accepted
    package/contract identity and arrival assumptions; a source digest does not
    establish plan correctness.
  - Connect the authored bootstrap, semantic child emission and physical shell
    through `compiler/native-realization`. The
    `optimized_semantic_wrapper_object/` staging entrance still has no callers;
    additional isolated binder milestones do not advance this customer. Its
    semantic-entry validator currently admits only a receiver-free Unit source,
    whereas the retained storage-roots canary has a receiver: preserve the actual
    selected source shape, not a test-authored substitute.

  Resume from
  `entry_and_abi::program_entries_and_image_validation::uefi_entry_machine_plan_produces_terminal_artifact`:
  the claim-pinned cycle now publishes Terminal Psi. The semantic calling-plan
  application identity check is also repaired; neither is a remaining blocker.

  Acceptance: authored layouts feed actual firmware projections/calls and the
  bootstrap reaches its continuation through emitted entry code with checked
  stack, root custody and return behavior. Malformed geometry/header integrity,
  foreign occurrence, wrong package/target and overlapping image/storage reject.
  Preserve scoped firmware authority and independently replay the source/plan/
  realization join. Layout helpers, source preflight and Rust-constructed contracts
  do not close this task; missing general layout/call/custody support must be fixed
  in its owner, not replaced with a firmware-specific intrinsic.

- **UEFI-OS-HANDOFF.** Implement the
  [Boot Services-to-OS handoff](wiki/spec/build/uefi_entry.md#returning-application-versus-os-handoff)
  as ordinary target-package machines, replacing the bodyless whole-protocol
  promise in `std/targets/uefi_x86_64/handoff.omg`. Reuse the behavior and
  regression controls in
  `external-roots/src/platform_bringup/uefi_bootstrap/{os_handoff_cycle.rs,get_memory_map/,exit_boot_services/}`;
  the Rust cycle currently has test callers, not an emitted authored route.
  Delete superseded production sequencing when the source route covers it.
  The compiler owns entry/stack-transition mechanics, not a handoff-loop intrinsic.

  Depends on `UEFI-PHYSICAL-SEMANTIC-ENTRY` for source-derived layouts, the
  shell and scoped firmware leaves; reuse `ENTRY-CONTENT-ROOTS`'s service-carrier
  migration. Acquire/grow map storage, retain the freshest snapshot/key, retry
  stale keys with a decreasing explicit bound, and transfer custody only on
  successful exit. Preserve firmware lifetime, surviving-stack evidence, allocation
  lineage and final-map obligations; exhaustion returns the target-defined error.

  Acceptance: evolve `build/uefi_os_handoff_invocation` from binding-only coverage
  into authored execution through native emission and a firmware or controlled
  provider harness. Cover grow-then-stale-key retry, bounded exhaustion, malformed
  map geometry, foreign/stale evidence, lost custody, forbidden Boot Services
  use after exit, and successful nonreturning transfer to the exact selected OS
  entry. Rust ledger tests and a selected bodyless provider are not execution
  evidence; do not hide the protocol in a destructor or special multi-call leaf.

- **AP-BRINGUP.** Execute Cathedral's secondary-processor startup under the
  [startup contract](wiki/spec/build/external_roots.md#secondary-processor-startup).
  Cathedral owns discovery, dispatch, acknowledgement, retry and cancellation;
  selected hardware boundaries supply explicit premises. The compiler checks
  installed entry, placement, resources and evidence, not an APIC/firmware driver.

  `external-roots/src/platform_bringup/secondary_processor.rs` now carries the
  three-way `SecondaryProcessorStartupVerdict`: `complete_secondary_processor_startup`
  returns pending custody only on `DefiniteNondispatch`, keeps the account
  invoked and held on `DispatchUnconfirmed` (neither withdrawable nor
  reissuable; the returned carrier still answers a later definitive receipt),
  and marks started on `ConfirmedArrival`. Only definite nondispatch permits
  immediate withdrawal. Still open before the authored route: the
  cancellation/settlement leg below, and timeout cannot release
  stack/state/code or erase an outstanding attempt.

  Add cancellation/settlement without requiring `SecondaryProcessorStarted`:
  establish both no current use and no possible late arrival before release.
  Confirmation must come from a checked handshake or admitted provider guarantee
  bound to the exact invocation/entry, not construction of a success record.
  Preserve atomic custody across acknowledgement/cancellation races.
  `BOUNDARY-ISSUANCE` owns the general issuance review, not this concrete repair.

  Reuse installed-code, per-processor stack/state and retirement joins. Bind the
  provider-declared profile to its selected contract; low-memory/vector geometry
  is mechanism-specific, not a universal startup model. Do not add a compiler
  boot driver to support another provider.

  Acceptance: authored Cathedral startup reaches the installed entry with visible
  placed bytes and dedicated nonoverlapping resources. Check definite nondispatch,
  timeout then late arrival, cancellation before confirmation, arrival/cancellation
  races, stale/foreign/replayed evidence, resource conflicts and retirement.
  Resources remain held while any admitted attempt can reach them. An emitted
  trampoline or a Rust test constructing receipts is not the customer witness.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Execute a content-bearing
  program through checked source, Terminal Psi, provider selection and native
  realization under [content conservation](wiki/spec/resources/content_custody.md).
  Reuse `checked-trees-to-lowered-psi/src/proofs/content_conservation.rs`,
  Terminal claim/frontier verification and Omega's
  `terminal-psi-to-abstract-operations/src/provider_installation/replay.rs`.
  Normalized equations, identity reshuffles and partition-composition lowering
  exist; do not restart those mechanisms.

  Replace declaration-only coverage with an invoked route: an established owned
  input is forwarded/partitioned under an authored theorem, and a selected
  boundary accepts the exact residual while the caller retains or returns the
  rest. `core/content_conservation_contract` and
  `core/content_retained_custody_round_trip` currently have empty entry bodies;
  neither proves this integration. Use canonical core content identities, not
  lookalike test-local algebra declarations.

  Preserve exact subject/revision, projection/algebra, geometry, lineage, route
  and installed occurrence through source-free replay. The caller may use the
  callee's partition theorem only after its exact successful invocation.
  Provider acceptance of custody does not establish the residual arithmetic.
  `BOUNDARY-ISSUANCE` owns fresh supply and receipt ingress; existing-input
  conservation can proceed without waiting for every fresh-issuance route.

  Acceptance: actual source/native execution carries nonempty content claims;
  every surviving claim has a reconstructed introduction or admitted issuance
  and every exit/residual is accounted for. Reject overlap, gaps, wrong
  projection/unit/lineage, stale invocation, substituted theorem/arguments,
  and authority inferred solely from equal scalar totals or compact fingerprints.
  Keep unrepresented runtime-indexed owned extraction rejected, not approximated
  by an arbitrary element or a helper-only proof.

- **BOUNDARY-ISSUANCE.** Complete exact fresh-supply and receipt ingress under
  [external roots and issuance](wiki/spec/resources/authority.md#external-roots-and-issuance).
  This is distinct from conservation of existing accounts and from
  `DOMAIN-ISSUER-ROUTES`'s source target selection; it need not wait for all
  conservation integration to finish.

  Reuse Psi `checks/content/retained_custody.rs`, qualification evidence and
  Terminal claims, then join provider planning/native settlement to the installed
  occurrence. Derive geometry from exact parameters, callable-entry places and
  result paths. Admit only the provider's backing custody/freshness premises;
  never accept interval arithmetic as an opaque provider fact. Multiple results
  need one separated supply relation, and transferred input is not fresh supply.
  Preserve the existing rejection of construction that lacks an issuance witness.

  Audit installation, retirement, startup, interrupt and callback receipt ingress
  in `backend/runtime/external-roots`. Trace each accepted record to checked
  execution or the exact selected admitted provider contract. Public Rust
  constructors are neither proof of source forgeability nor evidence that this
  source-to-runtime join exists; do not solve the task by changing visibility alone.
  `AP-BRINGUP` owns its concrete arrival/cancellation state repair.

  Acceptance: source-issued content retains geometry, backing, issuer, lineage,
  route and exact occurrence through independent replay. Reject forged source
  construction, foreign/replayed receipts, substituted geometry and duplicate
  fresh supply, while legitimate provider issuance and identity-preserving
  transfers succeed without re-minting capacity.

## P2 - Materialization and placed access

- **PLAN-LAID-VIEWS.** Connect actual placed-view establishment, use and retirement
  to executable and interpreter inputs under
  [placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement).
  Existing `ArtifactSections` admission, codec replay and native-realization
  optimization retain exact rosters; do not rebuild that evidence transport.

  Continue from `terminal-psi-to-abstract-operations/src/artifact_admission.rs`,
  `compiler/native-realization`, and Terminal interpreter input custody.
  `try_into_native_input` and `TerminalExecution::start_verified_module`
  currently reject nonempty placed-view rosters because they cannot supply the
  referent under the required custody. Add the ordinary source/provider
  establishment route, preserving exact layout, qualified backing, range,
  access/profile and lifetime joins; a roster or pointer is not this authority.
  Keep unsupported consumers rejecting until they carry it. Preserve the
  verified native-input boundary enforced by architecture checks.

  Extend `compiler/tests/access_plans/source_access_policies.rs`'s
  `direct_placed_view_input_survives_codec_and_native_replay` into a source
  program that establishes a view, performs a checked access and retires it
  through published native execution. That test already optimizes/emits a
  fragment and lends backing from C, but its authored consumer is empty;
  it does not demonstrate source establishment or access.

  Acceptance: valid views retain the same semantics through codec, optimization,
  interpretation and native execution. Stale/substituted plan, artifact, backing,
  range, rights, occurrence or lifetime rejects before access; failed establishment
  returns custody and retirement preserves the declared resident/vacant state.
  Run each available supported host leg and explicitly report unavailable ones,
  without treating cross-target emission as physical execution.

- **SYMBOLIC-MATERIALIZATION.** Complete symbolic field/index materialization
  and its target-dependent realization as one
  [derived consumer](wiki/spec/layouts/plans.md#derived-consumers) of a
  normalized plan: paths and bounds stay exact until assignment, and physical
  lowering may choose instruction bytes and a context register but not which
  semantic slot a write addresses. One bounded recursion already classifies
  every record level's children — direct sums, literal `[S; N]` sum arrays,
  literal `[R; N]` record arrays, and record fields still reaching sums
  (`project_record_level_children` in
  `omega-rust/omega/backend/layout/src/sum_materialization/mod.rs`, under
  `CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT`); the carrier fold joins them in one
  field-keyed namespace (`SymbolicFieldInnerLayout::from_recursive_sum_paths`
  in `omega-rust/psi/foundation/layout-plans/src/symbolic_values/mod.rs`), and
  build-time evaluation retains, replays and fingerprints the same custody.
  The record-array shape now runs the pinned writer-lowering leg:
  `record_array_symbolic_materialization_realizes_on_both_linux_isas` in
  `omega-rust/omega/compiler/compiler/tests/layout_plans/writer_lowering.rs`
  projects a leaf level carrying a direct sum beside a literal `[R; N]` record
  array, folds the `RecordArray` carrier, composes `field At + index * stride
  + interior offset` for `members[i].<path>` writes, and lowers/replays the
  post-handoff writer on both Linux ISAs (native execution on x86-64). Carry
  record depth as data and extend that owner rather than adding
  depth-specific implementations.

  Remaining work:

  - Target-dependent placement. Both projection entry points in
    `sum_materialization/mod.rs` reject an outer field carrying a `Bits`,
    `IntegerAt` or repeated placement ("uses target-dependent fragment,
    stored-integer, or repeated placement"). A symbolic path must cross the
    whole [placement vocabulary](wiki/spec/layouts/plans.md#placement-vocabulary),
    not only whole-field `At` entries.
  - Shapes the recursion still fences: an array reaching sums through more
    than one literal element hop — nested arrays or mixed elements — and any
    non-literal array length.
  - The Linux aarch64 native leg has never executed. That harness selects its
    guarded C driver by `cfg(target_arch)`, so only the x86-64 arm has run;
    every other host takes the emission-only path.

  Acceptance: nested field and index canaries realize on both Linux ISAs, each
  executing the lowered fragment on its matching host and comparing the image
  with the Rust reference writer. Both ISAs agree on the normalized fragment
  fingerprint and the writer invocation while emitting their own bytes.

  Flag: each child shape got its own channel instead of one classified child
  row. `ConventionalRecordSumPathsLayoutReport` carries `paths`,
  `child_sum_layouts`, `child_sum_array_layouts` and
  `child_record_array_layouts`; the recursive `Leaf` repeats three of them;
  `SymbolicFieldInteriorLayout` has a matching variant per kind; and
  `build-time-evaluation/src/layouts/layout_plans/` now holds 17
  `ValidatedConst*` materialization and selection types. The last three shapes
  landed as one commit each, every one adding a report field, a carrier
  variant, a validated type and its own recursion test, and each fenced shape
  above would add another. The general mechanism is one child row carrying its
  own path segment — a field hop, or a literal index hop with a count and
  stride — beside the child's recursive report, so repetition and depth are
  both data on one channel.

## P3 - Terminal Psi, PCC, and observation

- **PCC-PRODUCT-PUBLICATION.** Deliver native evidence for the
  [optional proof product contract](wiki/spec/proofs/publication.md). Both
  products publish and check today. `build_native_proof_sidecar`
  (`omega-rust/omega/compiler/compilation-report/src/pcc.rs`) emits a native
  `.proof` beside the flat executable and inside `Contents/MacOS/`, carrying
  the placed-image evidence section — the declared executable-text and
  initialized-data extents plus the complete placed executable-region and
  data-region inventories over those bytes, with region and gap digests,
  addresses, fingerprints and both inventory seals
  (`pcc/native_evidence.rs`) — and `verify_native_proof_sidecar` replays the
  section against the exact artifact bytes, rejecting a section that lies
  about them and an envelope that relabels the semantic profile. Section
  version 2 additionally binds each claimed `ImportThunk` row to the declared
  target's closed thunk form: only (x86_64, Coff) `jmp [rip+disp32]` and
  (aarch64, MachO) `ADRP/LDR/BR X16` are thunk claims at all, the claimed
  extent and footprint must equal the closed form's at decode, footprint
  registers must belong to the declared architecture, and replay re-derives
  the thunk opcodes from the committed bytes — on Mach-O additionally
  requiring the decoded pointer load to pair exactly one committed
  `ImportBindingSlot`, which is the first leg verifying what bytes do rather
  than only where they sit. That still establishes custody and thunk
  realization only, which is why the sidecar offers
  `omega.native-placed-image-coverage.v1` rather than a behavioral guarantee,
  and why every native pair still ends
  `Incomplete(UnsupportedEvidence { product: Native })`. The contract fixes
  three outcomes with no partial success, so a coverage-only pair can never be
  `Complete`.

  Remaining work: standalone native semantic and correspondence checking,
  owned by the native semantic and certification owners rather than
  `compilation-report`, which owns the envelope and the sidecar.

  - Instruction rows decoded from the published text and checked against the
    closed semantics of the declared target.
  - Entries and incoming edges over those same bytes, plus the PE thunk's
    `.rdata` slot pairing (the IAT's custody is not in the data inventory).
  - Premise availability and lowering correspondence: either transform the
    Terminal obligations the Psi product carries into native rows, or prove the
    native obligations directly. Hashes of producer validation reports and an
    unrelated valid Psi artifact establish neither.
  - The verdict and the guarantee it licenses. `verify_native_proof_sidecar`
    returns `Incomplete` at its tail even after a fully replayed section; a
    completed leg returns `Complete` under a guarantee naming what the
    behavioral evidence establishes, and a target whose leg is unbuilt keeps
    reporting `Incomplete` rather than a custody-only success.

  Acceptance: native-only standalone checking returns a real verdict from the
  artifact bytes, the companion and a pinned policy after the source and Psi
  artifacts are deleted. The existing refusals survive unchanged: arbitrary
  native bytes paired with valid Psi and recomputed producer hashes, a tampered
  pair, a relabeled semantic profile and a stale sidecar all reject, and the
  receiver never inherits the producer's admission profile. Claims beyond the
  bounded `omega.terminal-verified-module.v1` guarantee depend on
  **PROOF-KERNEL-CORE**, **PROOF-CERTIFICATION-BRIDGE** and completed profile
  rules, not a new policy DSL.

  **MACOS-APPLICATION-PUBLICATION** owns bundle execution acceptance.

- **PSIIR.** Extend Terminal Psi only in complete vertical slices through
  canonical encoding, independent reconstruction, verification,
  interpretation, resource analysis, native lowering, artifact custody, and
  installation. [Terminal specification subjects](wiki/README.md#current-specification-subjects)
  own the vocabulary. The [encoding contract](wiki/spec/terminal-psi/encoding.md)
  still owes complete operation and proof-node byte tables: it gives a
  physical layout for 3 operation tags (65, 66, 68), while
  `terminal-codec/src/sections/semantic_module/block_wire/operation_tags.rs`
  defines 72 and `sections/proof_bundle/proof_node_codec.rs` owns the proof
  nodes. The implementation's codec is not a substitute for those tables.

  Acceptance: source and producer state can be discarded before an
  independent verifier reconstructs every obligation and executes or lowers
  the same artifact, and the encoding contract specifies every operation and
  proof-node form the codec accepts.

  Native/external execution, ABI, fixed native resource, and final-code replay
  claims additionally require exact final-realization evidence. Preserve
  complete standalone products without hidden `CheckedCompilation` state;
  checked API/capability results and opaque executable supply cannot establish
  those claims. Physical optimization replay belongs to
  `TRANSLATION-VALIDATION` in `TASKS_OPTIMIZER.md`.

- **GENERAL-CYCLIC-EXECUTION.** Complete the
  [cyclic control contract](wiki/spec/terminal-psi/control_flow.md) and
  [separate safety/progress rules](wiki/spec/language/termination.md) for
  `print_squares` and the Console writer. Owners:
  `terminal-verifier/src/validation/control_flow.rs`, Psi's shared
  `execution/unit/` and `checked-trees-to-lowered-psi/src/unit/attached_unit/`,
  and Omega's ordinary `lowering/control_flow/` plus receiving graph replay.
  Unit/scalar/aggregate functions already share that native graph; do not
  recreate the deleted Unit planner or unsigned-countdown native route.

  Composed cyclic Unit plans, path-scoped store invalidation, stored-field
  equations, storage-observation block invariants and checked wrapping-update
  bound transport already exist. The guarded divisor bound is covered by the
  `cyclic_field_divisor_*` controls in
  `checked-trees-to-lowered-psi/src/tests/cyclic_byte_literal_calls.rs`; do not
  re-derive a counter/divisor invariant. None of this carries the decimal
  conversion loop through Terminal production yet, and the verifier admits a
  cyclic machine only through the shape allowlist in
  `validation/control_flow/unranked_cycles.rs`.

  Remaining work:

  - Resume at the indexed byte store. On 2026-09-18 (macOS ARM64)
    `text/runtime_number_to_decimal_exit` compiled under
    `pass_canaries_compile`, but
    `content_text_and_carriers::runtime_number_to_decimal_exit_canary_runs`
    failed before the native run: Terminal production refuses `Main::main`
    with `InvalidUnitMachinePlan`, "attached Unit closure is missing a checked
    transitive machine plan", omission "local construction stopped at
    statement sequence: scalar field store sequence, state 7". The index
    counts the `machine_states` span whose slot 0 is the entry body, so state
    7 is `digit_write`, not `digit_div`. Its
    `self.out[self.p] = narrow_u32_to_u8_wrapping(self.ch as u32)` reaches the
    `BoundedOwned` byte-sequence branch of
    `execution/unit/structural_scalar_store/mod.rs`, which reads both operands
    from the pure `scalar_expressions` plan through `bound_expression_at`:
    `AssignmentIndex` is bound and `AssignmentValue` is not, so the store
    sequence collapses. `values/scalar/computations.rs` does record an
    `AssignmentValue` computation root for an `Indexed` target, and the scalar
    field lanes of the same builder accept `Computation` and same-statement
    `ScalarResult` values; the two byte-store lanes accept only a pure value.
    Which gate in `values/scalar/expression_plans.rs` (target type reference,
    `assignment_target_primitive_type`, or `lower_return_expression`) drops
    the pure binding is unmeasured; re-probe before relying on it. Resolve the
    store value once for every destination lane, not through a byte-store
    call-result arm.
  - Lowering still gives a live scalar call result only a `Return` or
    `LocalInitializer` role in `emission/call_source_custody.rs`, so a store
    consuming its own statement's call result has no authored destination
    there. `filesystem/native_close` stops at that point. The checked-stage
    half exists (`structural_scalar_store/tests/call_results.rs`).
    **MATCH-SELECTIVE-LOWERING** works the same `values/scalar` and emission
    paths; check the claims registry first.
  - Indexed byte-field writes need composed cyclic-Unit and customer closure
    coverage. Reuse the ordinary native bounded-field store and exact
    live-length replay, not a new byte-view adapter.
    `compiler/tests/byte_field_replacement/indexed.rs` covers direct and nested
    source writes only.
  - Extend bounded safety/proof admission to qualified and partial owned
    custody, structural results, projected claims and effectful calls.
    `unranked_cycles.rs` admits claims pinned on owned entry parameters and
    call requirement/crash rosters; it still refuses claim transfers, content
    reshuffles and partition compositions, qualified parameters, and any
    structural result other than a plain scalar case. Preserve dominance,
    exact successor transfers, ownership frontiers, current-iteration guards
    and test-fuel suspension/resumption. General cyclic invariants/ranking
    views need retained evidence; guarded-crash checking must not enumerate
    unbounded paths. Finite fuel or a relaxed shape check is not a safety
    proof.
  - Source production must compose projected helpers, Console structural
    operands, indexed/aggregate writes and computed results without state
    duplication.

  Acceptance: the unchanged customer reaches native exit/output on the hosted
  matrix, with exact caller/callee resource composition and independently checked
  ranking where declared. Corrupt arrivals, ownership, guards, effects and proof
  groups reject. The unchanged decimal loop is the next source acceptance, then
  the native customer. **SAMPLE-CORPUS** owns whole-customer execution and its
  current scope pause; interpreted loops or isolated graph tests cannot close
  it. The store-value repairs are ordinary value transport shared with
  **STATE-LOCAL-VALUE-FRONTIER**; they stay here only as this customer's resume
  point.

  Flag: two mechanisms here grow per customer shape. The verifier's
  `unranked_cycles.rs` (`eligible`, `cycle_operation_eligible`) is a 780-line
  allowlist of machine, place, terminator and operation shapes with 27 commits
  since 2026-09-07, most admitting one more shape; its own comments say
  eligibility carries no proof authority and the per-arrival frontier
  comparison is the custody proof. Completing that comparison for cyclic
  arrivals would let the allowlist be deleted instead of widened. The producer's
  `proofs/scalar_block_invariants/lockstep.rs` recognizes exactly one update
  pair (`divisor = divisor / d`, `counter = counter + 1` under `counter < N`,
  at most 64 clauses) to make the decimal loop's bound inductive. Its output is
  a proposal the verifier re-proves, so it is not a trust hole, but it is a
  recognizer for one customer's arithmetic, not a general strengthening rule.
- **CRASH-CONTRACT.** Carry invocation-specific crash obligations through
  operators, nested structural paths, calls, cycles, execution and package review.
  Owners include `facts/operator_crashes.rs`, `CrashPlan::checked_operators`,
  captured operands in `flow/expression.rs`, and Terminal/native evidence
  consumers. Source checking alone is not portable proof. Retain exact selected
  requirement, saved actuals, Match arm and surviving route; do not invent calls,
  infer semantic crashes from emitted traps, or narrow opaque contracts by
  inspecting providers.

  Checked operator crash sites, entry provenance in
  `facts/crash_entry_values.rs`, inferred ceilings on lowered machine
  contracts, guarded boundary ceilings with independent actual-argument
  substitution, `TerminalTraceV1` boundary crash rows and per-site package
  review rows already exist. Terminal also carries operation-level crash
  contracts (`TerminalModule::operation_crash_contracts`), substituted by
  `terminal-verifier/src/validation/crash/operation_contracts.rs` and produced
  by `checked-trees-to-lowered-psi/src/retention/operation_crash_contracts.rs`.
  That producer reaches an emitted operation only through the selected IEEE
  and selected integer comparison occurrence rosters, and no source customer
  replays yet. `omega inspect-terminal` on `operators/crash_routes` (its canary
  rows are check-only and never lower) rejects `safe`/`may_crash` at `selected
  comparison has no complete provider plan evidence` and `wrapper` at `direct
  scalar call crash continuation lacks a checked scalar term`. On the product
  route, `selected-dispatch`'s `validate_selected_operator_terminal_custody`
  still refuses every checked program with a crash-qualified operator use.

  Remaining work:

  - Supply provider evidence for the integer boundary comparison on the Omega
    side: a selected ProviderPlan or an explicit builtin-realization commitment
    for `Comparison::equal(i32, i32)`. The fixture's `boundary machine ==` has
    none, so the checked use carries an empty `provider_plan_commitment` and
    `expression_preparation/source_custody/comparisons.rs` refuses to emit it.
    Then add an Omega consumer that rejoins
    `selected_integer_comparison_occurrences` the way
    `float_comparisons::associate` does, and lift the nonempty-roster refusal
    in `checked-compilation-to-terminal-artifact/src/terminal_artifact.rs`. Do
    not realize a selected comparison as the builtin one.
  - Give the remaining crash-qualified uses a replayable Terminal carrier. A
    named `Namespace::requirement(...)` use has no emitted-operation join and
    fails closed in the producer, as do a non-scalar or miscounted operand
    roster and a call operation. `wrapper`'s direct scalar call continuation
    lacks a checked scalar term
    (`scalar_graph/scalar_graph_lowering/call_lowering.rs`). A guarded float
    operator route has no structured form because `CheckedBooleanExpression`
    has no IEEE ordering over scalar float formals; `proofs/crash_routes.rs`
    rejects it. A generic operator or a guard through a structural formal
    keeps identity only. Surviving routes are invocation-specific and may
    carry no portable `scalar_expression` after conservative `Truth` widening;
    copying checked rows onto `MachineContract` alone would not establish
    replay meaning.
  - Finish boundary crash outcomes. The trace profile observes a declared
    ceiling at its call operation; the runtime trace and refinement join that
    resolves one invocation's outcome
    ([observations](wiki/spec/terminal-psi/observations.md#reconstructed-rows))
    is absent. Omega carries a verified boundary crash contract on the
    boundary declaration, but target lowering's Unit, borrowed and
    aggregate-result call lanes (`operations.rs`, `borrowed_calls.rs` and
    `aggregate_results.rs` under
    `abstract-operations-to-target-operations/src/lowering/control_flow/`)
    refuse nonempty `crash_continuations` with `UnsupportedControlFlow`, the
    error `compiler/tests/behavior_exclusions.rs` pins for its crash-bearing
    native fixture. Extend the source-to-execution controls in
    `checked-trees-to-lowered-psi/tests/scalar_boundary_arguments.rs` while
    preserving exact call sites, guard actuals, abandoned claims, staged
    writeback and no-result/no-cleanup behavior. Source crash predicates lower
    through fixed-width `ScalarTerm`; proof-only mathematical terms do not
    imply an authored mathematical guard route.
  - Carry qualified scalar results and the remaining normal-contract
    vocabulary through ordered boundary completion. Reuse the machine-entry
    and scalar normal-guarantee path pinned in
    `checked-trees-to-lowered-psi/tests/unit_scalar_result_source/boundary_wrappers/`.
    State/control contracts, mutable snapshots crossing state joins, and
    field/arithmetic predicates still need their evidence joins. Preserve
    authored callee contracts regardless of whether a helper is a direct
    closure root or a transitive dependency; a checked call identity is not
    contract proof. Mutable scalar inputs still need the shared
    signature/storage path beyond the invocation-entry read checker. Do not
    infer normal guarantees from crash ceilings or use current storage as an
    entry snapshot.
  - Package contract review still needs exact carrier/value custody for
    declaration and result projections through indexes, case payloads and
    generic field substitution beyond ordinary declaration-owned field paths.
    Reuse Psi's exact carrier/case relation; do not manufacture a machine
    owner from the classifier. Saved or call-produced result tags without a
    live predicate need ordinary value/effect custody; do not replay
    initializers or callee bodies to recover a tag after its evaluation
    point. `package-evidence/tests/callable_policy/case_membership.rs` is the
    existing source-to-recovery control. The retired `proposition`
    declaration surface belongs to **PROOF-CONTRACT-MIGRATION**, not an
    independent membership-extension task.
  - Entry provenance still widens a surviving route to `Truth` for
    `start..end` index leaves, `Opaque` and `ContentConservation` leaves, and
    element reads below a collection root. Extend it only with proven
    origins: divergent arrivals, unresolvable cycles and other unknown
    provenance must remain conservative, and current spelling/live storage is
    not a saved actual. This owns **MATCH-SELECTIVE-LOWERING**'s
    crash-qualified equality dependency and shares entry snapshots with
    **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: source `operators/crash_routes` and crash-qualified float controls
  retain exact surviving-route evidence through independent Terminal replay and
  execution; package projections already carry the site rows. Safe uses
  discharge each route; changed guards, captures, substitutions, sites and stale
  writes reject. Preserve examined/discharged routes and caller coverage, not
  only the final cause set.

  Flag: selected-operator custody is growing one roster per operator kind.
  `LoweredPsi` holds `selected_ieee_float_comparison_occurrences`,
  `selected_ieee_float_fma_occurrences` and
  `selected_integer_comparison_occurrences`, each with its own replay module in
  `lowered-psi-to-terminal-psi/src/boundary_operator_custody/` and its own Omega
  association (`float_comparisons/`, `float_fma/`); the first bullet adds a
  third. Named uses and every non-comparison operator still have no join. One
  occurrence row from a checked `operator_use` or `named_use` to its emitted
  operations, operand mapping and provider commitment, with one Psi replay and
  one Omega rejoin, would give those uses a carrier without another roster.

- **ARITHMETIC-POLICY-REALIZATION.** (new-scope) Give the executable
  arithmetic policies of [numeric values](wiki/spec/language/numeric_values.md)
  their Terminal form. Psi checking accepts them, but Terminal production
  stops at the three gaps below, so no program using them reaches a native
  artifact. The largest is Trapping, which Terminal Psi cannot express:
  [structural predicates](wiki/spec/terminal-psi/structural_predicates.md)
  requires executable Trapping operations to "carry their primitive
  denotation and path-conditioned crash site, checked against the published
  same-cause ceiling", and Terminal Psi attaches crash continuations only to
  call operations. A producer therefore may not expand one into a guard and a
  `Crash` terminator, and the repair crosses the firewall.

  Remaining work:

  - Add a Terminal Trapping operation family with its `terminal-verifier`
    rule, `terminal-interpreter` case and Omega realization.
    `checked-trees-to-lowered-psi/src/expression_preparation/`
    (`prepare_expression.rs`, `bindings/mod.rs`) refuses `IntegerTrappingCast`
    with "requires runtime policy realization", and
    `typed-trees-to-checked-trees/src/values/scalar/expression_facts.rs::checked_integer_binary_kind`
    has no Trapping arm for any arithmetic or shift operator, so a Trapping
    shift, whose out-of-range count the spec makes an executable trap
    condition, gets no value fact. The Trapping refusal in
    `scalar_graph/scalar_contracts/namespace.rs` is contract-position and
    stays: direct Trapping arithmetic forms no predicate term.
  - Realize modular conversion with a signed source or target;
    `prepare_expression.rs` lowers only unsigned-to-unsigned
    `IntegerWrappingCast`. Do not retry expression-level composition:
    truncation toward zero is not the modular image of a negative dividend,
    a same-width sign reinterpretation needs a value-level select that
    Lowered Psi has no operation for, and masking plus an exact cast needs a
    bitwise range the spec denies.
  - Carry Boolean-to-integer conversion in `CheckedScalarExpression`.
    Checking admits the surface and retains its `0..=1` range, but no
    expression node holds it, so the initializer has no value fact.

  Acceptance: the six `core/numeric_*` pass canaries and every
  `source/library/core/numeric_conversion.omg` machine ending in a Trapping
  conversion compile and execute their trap routes, an independent verifier
  replays each crash site against the published ceiling, and no policy is
  silently weakened into another. Move the four boundaries pinned in
  `checked-trees-to-lowered-psi/tests/integer_policy_realization.rs`, each
  paired with an admitted neighbour differing in one coordinate. The
  `float/float_trapping_*` and `expressions/arithmetic_domain_trapping_*`
  families are customers too, but no repro separates their
  `InvalidUnitMachinePlan` stop from GENERAL-CYCLIC-EXECUTION's; rerun them
  before attributing it.

- **PROOF-KERNEL-CORE.** Finish the common mathematical term/declaration model
  and independent checker in Psi under the
  [selected foundation](wiki/spec/proofs/foundation.md) and its
  [W-based profile](wiki/spec/proofs/inductive_profile.md). Customer: library
  theorems about arbitrary types/predicates and dependent witnesses, not another
  extension to the bounded `Proposition` enum. Source elaboration and
  certificate consumers use this model rather than invent parallel truths;
  search stays outside the checker.

  `proof-admission/src/mathematical_core/` implements the pinned reference
  core with its strict layer, typed function eta, `Two`, `Id`, `W`,
  universe-polymorphic declarations with exact assumption closure, and the
  derived indexed and set-quotient schemes as ordinary checked declarations.
  [Kernel metatheory](wiki/spec/proofs/kernel_metatheory.md) argues the
  combined rules at paper level (not mechanized) and lists the witnessing
  tests and measured receipts. `accept_certificate` denotes each bounded
  `ProofNode` certificate into the core and records `Judged` or `Refused` in
  `MathematicalCoreDecision`. This does not deliver the customer: every
  kernel theorem and scheme is a Rust-built term (`scheme_dsl.rs`),
  `terminal-codec`'s `encode_mathematical_certificate` and
  `decode_mathematical_certificate` have no caller outside tests, and the only
  source fixture written against the kernel route proves scalar `==` symmetry
  and transitivity (`pass/proofs/kernel_theorem_equality_certificates`, driven
  by `compiler/tests/proof_kernel_canaries.rs`).

  Remaining work:

  - Give the bounded arithmetic families kernel meaning, so each is a
    certificate producer whose output the kernel checks or an explicitly
    justified checked rule (see Flag). Integer carrier, order and equality
    declarations are landed in `mathematical_core/bounded_denotation.rs`:
    `Int`, `IntLt`/`IntLe` and `Int`-valued term constants are interned
    assumptions with exact statements — closed mathematical terms intern
    by the shared evaluator's exact value — and the order/equality rules
    cite one fixed roster (`eq_le`, `lt_le`, `le_trans`/`lt_trans`/
    `lt_le_trans`/`le_lt_trans`, `lt`/`le_subst_left`/`_right`) while
    `Id` symmetry/transitivity, the `Equal`↔`IntegerMathEqual` citation
    crossing and single-equation `ValueEqualityTransport` over `Int` are
    `J`-re-decided, not assumed. Still to do: the remaining `rule_axiom`
    families (discreteness, subtract-order, the bound-witness rules,
    `ContentConservation` transitivity, transport inside non-`Int`
    propositions) and open arithmetic — non-closed `IntegerMathTerm`
    equations still name opaque `Int` constants, so `x + 0 = x` assumes
    its conclusion.
  - Check indexed-scheme applications produced from source declarations, per
    [declaration correspondence](wiki/spec/proofs/inductive_profile.md#declaration-correspondence-and-strict-logic):
    exact parameters, indices, payloads, case constraints and recursive uses.
    The vector, mutual, nested, derivation and level-instantiation families in
    `proof-admission/tests/indexed_*.rs` are hand-built terms. Negative
    recursion, bad universes and illegal strict elimination reject at the
    source declaration; source invalidity, unsupported valid encoding and
    producer defects stay separate errors. No kernel test covers negative
    recursion: `W A B` cannot state it, so the control needs an elaborated
    source declaration.
  - Make the kernel changes PROOF-CONTRACT-MIGRATION's elaborator forces, and
    no others. `signature.rs::Declaration` carries position only and each
    `MathematicalCertificate` carries its whole signature; source names and
    `boundary let` trust identities are the elaborator's to attach, and exact
    closure must still survive import and serialization. A new rule family
    arrives with the denotation that lets acceptance re-decide it and a
    source-level customer, not on its own. No second primitive indexed or
    strict-inductive checker, and no untyped wrapper-deletion shortcut for eta.

  Acceptance: the foundation's
  [migration examples](wiki/spec/proofs/foundation.md#migration-acceptance),
  each with its invalid control, reach a source-free kernel judgment from
  Omega source through PROOF-CONTRACT-MIGRATION: a universe-polymorphic theorem
  over arbitrary predicates with dependent pairs, identity transport and
  induction; strict same-statement conversion with relevant witnesses kept
  distinct; malformed universes, capture-changing substitution and illegal
  elimination rejecting; exact assumption closure through declaration
  types/statements without relying on unfolding. Measure term size, retained
  storage and checking cost on those source-produced terms in the application
  checker; do not claim feasibility from empty receipts or compiler-authored
  success flags. Structural round trips do not establish meaning, and no
  verified-profile claim precedes these controls.

  Flag: the bounded denotation still reaches `Judged` on part of the
  arithmetic families by assuming each conclusion. `bounded_denotation.rs`
  denotes the integer order and equality rules through a fixed roster of
  named `Π` laws over `Int` (`integer_law`), `J`-derives `Id` symmetry and
  transitivity on the denoted crossing, and interns closed mathematical
  terms by exact evaluated value — but discreteness, subtract-order, the
  bound-witness rules, `ContentConservation` transitivity and transport
  outside the `Int` vocabulary remain per-instance `rule_axiom`s whose
  statements carry no arithmetic a receiver could audit, and open
  arithmetic has no law roster at all.

  PCC-CANONICAL-SEMANTIC-LEDGER owns the soundness status of trusted checker
  rows and PROOF-CERTIFICATION-BRIDGE owns loop correspondence. Reopen W only
  through `OWNER_QUESTIONS.md` on demonstrated requirements, cost or audit
  failure. General source punctuation is not a kernel blocker.

- **PROOF-CONTRACT-MIGRATION.** Migrate the proof surface to
  [ordinary machine contracts and trait bundles](wiki/spec/proofs/contracts.md#machines-and-bundles)
  and implement the selected
  [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md),
  elaborating to `PROOF-KERNEL-CORE`'s terms, not a second general logical
  representation. Owners: Psi syntax/resolution/typing, contract proof
  semantics, Terminal evidence/codec/replay, and core mathematical traits.
  Only Rust-built kernel legs of cases 3-5 exist, in `proof-admission`, and
  `terminal-codec`'s mathematical certificate wire has no production caller.
  The parser (`tokens-to-syntax-trees/src/declarations/parse_declaration.rs`)
  has no top-level `let`, `boundary let` or arrow type, no `core::Level`,
  `Type`, `Strict` or `Squash` declaration exists, and the dedicated
  `proposition` declaration with its named-witness call lanes
  (`typed-trees-to-checked-trees/src/proof/proof_output_calls.rs`) still
  carries `core/int.omg` and 37 files under `tests/`.

  Remaining work:

  - Parse, resolve, type and elaborate closed parameterized top-level `let`,
    dependent function types, curried prefix application, core-named
    universes/level binders and `boundary let` assumptions. Preserve ordinary
    local bindings, complete machine calls and executable callback selection.
    Add no quantifier keywords, and do not substitute declaration enumeration
    or an optional-returning decider for mathematical quantification.
  - Replace formula declarations and hidden-witness calls with ordinary
    contracts and named witness/law bundles, preserving exact substitution,
    result/path availability, erasure, validity and transitive assumptions
    across trait calls and artifacts. Do not widen the old selected-witness or
    trait-named-witness surfaces independently; retain useful checking rules,
    not mandatory wrapper syntax.
  - Elaborate values and computation under
    [executable demand](wiki/spec/proofs/mathematical_bindings.md#assumptions-and-executable-demand),
    enforced consistently at source use, evaluation and lowering. Conversion
    never executes an effect, and a result-bearing axiom is an explicit
    checked declaration, not a missing-provider slot or a domain qualifier.
  - Preserve [call preconditions](wiki/spec/language/machines.md#call-preconditions)
    during mathematical application and theorem citation, including logical
    evidence supplied through binders; never restore a proof-only exemption.
    Induction also needs its exact decreasing edge. **OPERATOR-MACHINE-SUPPLY**
    owns the shared call-checker repair and the `core/nat.omg` migration.
  - Migrate core relations, quotients, samples and tests, remove obsolete
    parser/carrier/codec routes, and reject retired spellings. Two other
    owners assign encodings here: the replacement proof rows of
    `omega-rust/omega/packages/review/evidence/EVIDENCE_SCHEMA.md`, and
    QUOTIENT-THEOREM-LIFT's congruence-only `lift` wire payload.

  Acceptance: actual proof scripts and false twins pass through source,
  Terminal serialization and independent replay, under the
  [publication contract](wiki/spec/proofs/publication.md), for all five cases:

  1. Composition of two witness/law bundles preserves exact substitutions and
     distinct witnesses.
  2. A higher-order theorem over arbitrary mathematical predicates or
     functions passes every
     [delivery control](wiki/spec/proofs/mathematical_bindings.md#delivery-controls),
     its machine-shaped hypothesis supplied from derived Π-term evidence and
     not only a named proof declaration.
  3. Nonconstructive existence uses an explicit axiom and supplies no
     executable witness without checked realization, including an erased
     theorem that mentions the chosen value; squashed existence derived from
     another squashed witness needs no choice assumption. Constrained records
     are established at construction with exact evidence dependencies, and
     relevant dependent witness bundles stay distinct from strict predicate
     gating.
  4. Accepting and denying policies distinguish the same theorem, with exact
     transitive assumptions surviving import, erasure, serialization and replay.
  5. A Cauchy/quotient proof uses the
     [set-quotient interface](wiki/spec/proofs/quotients.md#set-quotient-foundation)
     and its derived items with no hidden extensionality or kernel reduction.
     A quotient-refusing policy accepts the quotient-free closure of the
     representative operations and congruence and rejects the
     assumption-bearing quotient proof. That control depends on transitive
     closure through helper statements/types; existing Rat comments do not
     establish it.

  Candidate naming syntax is not a prerequisite. These controls do not
  establish full mathematical coverage.

- **PROOF-CERTIFICATION-BRIDGE.** A functional claim about a ranked loop must
  be checked against the generated loop, not inherited from termination, under
  the [publication contract](wiki/spec/proofs/publication.md): a ranked
  `Natural` component whose accumulator update is wrong fails its post-loop
  `ensures` or scalar block invariant while its unchanged control-cycle
  certificate still verifies. That holds today for order claims on one form.
  A single-state free machine re-entered by named backedges proves
  `ensures result <= previous` at the source
  (`typed-trees-to-checked-trees/src/checks/contracts/exits/cyclic_headers.rs`),
  lowers with a strengthened header invariant
  (`checked-trees-to-lowered-psi/src/proofs/scalar_block_invariants/cyclic_guarantees.rs`),
  executes natively, and its wrong-step twin rejects
  (`pass/proofs/runtime_ranked_accumulator_guarantee_exit`,
  `fail/proofs/ranked_accumulator_guarantee_wrong_step_twin`). No arithmetic
  accumulation claim reaches a generated loop:
  `proofs/proof_inductive_gauss_sum` and `proofs/proof_inductive_climbing_sum`
  remain in `CHECKED_ONLY_PASS_CANARIES`, and their step-false twins refute the
  wrong update in Psi validation only.

  Remaining work:

  - Ring evidence at the Terminal verifier. The fixtures claim
    `result * 2 == acc * 2 + n * (n + 1)` over `u64 in Wrapping` and an `embed`
    sum over a two-subject `Nat::BoundedDistance` rank. The kernel derives the
    order `rank <= previous`; it cannot derive `sum + rank = initial`.
    `verify_normalization` (`proof-admission/src/admission/normalization.rs`)
    has no Terminal consumer; routing quotient and ring-law evidence through it
    is shared with **PCC-CANONICAL-SEMANTIC-LEDGER**.
  - A generated loop that carries the claim. Both fixtures are `&mut self`
    machines with no scalar graph; they stop in the attached Unit closure at
    `call statement shape: call count without a statement sequence`
    (`typed-trees-to-checked-trees/src/execution/unit/control/checked_machine.rs`).
    Either the attached value-returning cyclic route lands through
    **STATE-LOCAL-VALUE-FRONTIER**'s ordinary evaluation, or the sums are
    restated as free loops.
  - Header-invariant proposal beyond its current reach: immutable exact
    fixed-integer parameters, one state, the returned value only. A
    loop-carried value in a second state, a `self` transition that changes
    storage, and a Wrapping accumulator are not proposed, so a free-loop
    restatement of the Wrapping sums does not prove yet either.

  Acceptance: an arithmetic accumulation claim over a generated loop (the two
  fixtures, or a free-loop restatement of their sums) executes natively, and
  its wrong-update twin fails the functional claim on the preservation
  obligation while the unchanged cycle certificate still answers the identical
  cycle question. Dropping an arrival certificate rejects, and a guarantee the
  loop does not establish stays unproved. Keep the existing controls:
  terminal-verifier `tests/ranked_scc.rs` and
  `tests/ranked_scc/scalar_block_invariants.rs`, `checked-trees-to-lowered-psi`
  `tests::ranked_value_guarantees`, `compiler/tests/pcc_publication.rs`, and
  `tests/architecture/layering.rs`.

  Flag: the transported-guarantee header invariant is searched twice. The
  source prover (`cyclic_headers.rs`, which calls itself "the source analog")
  and the Terminal producer (`cyclic_guarantees.rs`) each propose and prove the
  same conjunct independently, with different reach: the source side returns
  `None` unless the machine has exactly one state. One derivation at any cyclic
  header, recorded by checking as a fact that lowering certifies and the
  verifier still replays, would remove the second search and the single-state
  restriction.

- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace trusted Rust fusion of artifact
  traversal and proof search with a small total canonical-ledger generator plus
  an untrusted certificate producer. The verifier reconstructs goals and only
  checks the supplied route, under the
  [canonical semantic ledger](wiki/spec/terminal-psi/verification.md#canonical-semantic-ledger).
  The [trusted-surface inventory](wiki/spec/terminal-psi/verification.md#trusted-surface-inventory)
  exists in `terminal-verifier/src/trusted_surface.rs` with mechanical
  dispatch and source coverage (`tests/trusted_surface.rs`). It establishes
  coverage, not soundness: two rows are `Proved` by generation-time
  certificates (`fact:boolean-polarity-implications`,
  `fact:branch-condition-transport`), every other row is `ExplicitlyTrusted`,
  and the codec's trust-graph descriptor still names the Rust decoder and
  verifier as trusted judgments.

  Remaining work:

  - Move proof search out of verification. The verifier still searches, for
    example 4096 steps in
    `terminal-verifier/src/validation/crash/entry_requirements.rs`; the
    producer must supply that certificate and the verifier only check it.
  - Convert the `ExplicitlyTrusted` reconstruction, normalization, scope, write
    invalidation and call/cycle composition rows to `Proved`, each with checked
    evidence and explicit dependencies. A proved row cannot hide an unproved
    composition row, and an `Unfinished` row establishes no independent claim.
  - Define the generator as a total definition over canonical Terminal bytes,
    not over an AST decoded by a separately trusted producer. Application
    interpretation uses the common kernel and the selected
    [inductive profile](wiki/spec/proofs/inductive_profile.md); its unfinished
    soundness and encoding proofs are dependencies, not permission to trust
    success.

  Acceptance: a theorem-dependent program obligation verifies after producer
  and source state are deleted, under one accepting and one rejecting
  assumption policy. Wrong goals, wrong profile identities and omitted
  transitive assumptions reject. A mathematical theorem alone does not
  establish native refinement. Bootstrap discharge belongs to
  `GAMMA-DERIVATION-CHECKER` in `TASKS_BOOTSTRAP.md`; it is not a prerequisite
  and must not force general mathematics into the Gamma checker.

- **PROOF-RELEVANCE-MIGRATION.** Finish `[erased]` noninterference and
  erased-stripped layout under
  [explicit erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings).
  An erased binding stays in semantic and proof identity and contributes no
  runtime storage, tag, ABI transfer or execution; runtime use and
  layout-dependent erasure reject. Data fields, case payload fields, signature
  and state parameters and `let` locals carry the marker through checking:
  `validation/src/proof_contracts/relevance/` rejects runtime reads and
  receivers, layout and the checked calling plan strip erased positions, and
  Terminal lowering rebuilds the stripped scalar and structural namespaces from
  the typed relevance, so an erased parameter that no contract names executes
  natively. No Terminal contract can name an erased binding:
  `checked-trees-to-lowered-psi/src/proofs/crash_routes/scalar_terms.rs`
  rejects it with "crash predicate value position is outside the selected
  scalar namespace".

  Remaining work:

  - Give Terminal contracts a proof-only scalar term for erased formals, as one
    vertical slice: a contract erased-formal roster and per-call erased
    arguments in `terminal-psi`, both inside the contract commitment; one codec
    tag, with old bytes rejecting under the
    [encoding contract](wiki/spec/terminal-psi/encoding.md); substitution of
    the caller's erased actual for the erased formal in
    `terminal-verifier/src/verification/call_composition.rs`; and the producer
    mapping in `typed-trees-to-checked-trees/src/values/scalar/contract_entry.rs`
    and `checked-trees-to-lowered-psi/src/scalar_graph/scalar_contracts.rs`.
    A `requires` naming an erased binding is verifier-only: an erased binding
    cannot determine runtime data or control, so it never becomes a crash
    route, and the interpreter and native lowering need no evaluation path. An
    erased actual is limited to the existing `ScalarTerm` closure over
    literals, caller values, field reads and the caller's own erased formals;
    anything else rejects at the initializer.
  - Give a caller's own erased formal, forwarded through a call, its erased
    argument row.
  - Carry `requires` on composed-route state contracts, so an erased state
    parameter reached through a named transition has a contract term.
  - Erased `self`, `const` and `mut` bindings refuse a checked calling plan
    (`execution/unit/types/mod.rs::strips_erased_parameter`). Keep the refusal
    unless the specification gives them a meaning.

  Acceptance: `pass/relevance/erased_parameter_proof_only` and
  `erased_parameter_named_transition_forward` leave
  `CHECKED_ONLY_PASS_CANARIES` and run natively (the first exits 70). A call
  whose erased actual violates the callee's `requires` rejects in source-free
  verification. An erased actual outside the admitted closure, a missing or
  substituted erased-argument row, and pre-change codec bytes reject. The
  `fail/relevance/` runtime-read and receiver controls keep rejecting.

  A partial build of the first slice was parked on the unpublished local branch
  `work/terminal-erased-term-parked` (def0c1be67): per-machine
  `erased_scalar_formals`/`erased_call_arguments` rosters with proof-only
  `ScalarTerm::Value` identities and no new term variant. It is not on
  `origin`; use it if reachable, otherwise build from the description above.
  Erased-field cleanup belongs to
  **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**.

## P4 - ABI, borrowing, and callbacks

- **NORMALIZED-ABI-LOWERING.** Finish target-independent signature
  normalization and target-owned calling/layout realization for aggregates,
  dynamic values, callbacks, and foreign boundaries. `&dyn Trait` boundary
  parameters normalize to the two-word `{instance, table}` descriptor shape
  (`provider-planning/src/calling_policy_plans/value_shapes.rs::value_shape_from_type`),
  and `TargetUnitOperation::NormalizedForeignCall` lowers, replays against the
  boundary declaration and legalizes with its provider custody. None of it
  emits: `target-operations-to-selected-instructions/src/selection/construction/scalar_graph.rs`
  rejects the legalized kind, no target declares a normalized-foreign
  constraint row, both ISAs reject the selected form, and machine emission
  reports an unsupported relocation shape.

  Remaining work:

  - Emit normalized foreign calls as one bounded slice, in order: selection,
    register homes, machine emission, object import plans, image custody,
    physical derivation. `compiler/tests/efb3_flat_record_probe.rs` pins the
    Terminal precondition; **EVALUATED-FOREIGN-BINDINGS** owns locators and
    import evidence.
  - Widen foreign arguments and results. The scalar lane admits fixed-width
    integers only. The structural lane admits one source-rooted borrowed flat
    record, and only while the scalar lane is empty, because the Terminal
    machine declaration does not retain the authored order of scalar and
    structural formals
    (`abstract-operations-to-target-operations/src/lowering/unit/boundary_call/normalized_foreign.rs`).
    Retain the authored parameter position so a mixed signature rejoins its
    plan rows, then add owned aggregates and dynamic descriptors.
  - Dynamic descriptor calls. Target lowering produces
    `StoreDynamicDescriptor`, the stored, rebound and parameter dynamic calls
    and the `...WithDynamicArguments` calls, and
    `target-operations-to-selected-instructions/src/legalization` consumes none
    of them. **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** owns the ordinary
    indirect-call operand they need.
  - Callback transport. The common route rejects every request carrying a
    callback in `native-realization/src/native_realization/object_emission.rs`;
    **CALLBACK-PRIVATE-MATERIALIZATION** owns that route.

  Acceptance: every call's ABI is independently reconstructible from its
  declaration and the selected calling policy, and no target placement appears
  in Terminal Psi. A source-produced foreign call with mixed scalar and record
  arguments executes on a matching host; substituted plan rows, placements and
  provider bindings reject at each stage that retains them. Stored
  dynamic-reference layouts stay equal to the normalized calling-policy shapes
  (`calling_policy_plans::borrowed_dynamic_trait_record_fields_retain_both_descriptor_words`);
  layout agreement alone does not close native transport.

  Flag: `TargetUnitOperation`
  (`target-operations/src/target_operations/operations/unit.rs`) has 12 call
  variants split by result kind, argument kind and dispatch source (`Call`,
  `ScalarCall`, `StructuralScalarCall`, `StructuralResultCall`,
  `Structural{Scalar,Unit}CallWithDynamicArguments`, `StoredDynamicScalarCall`,
  `Dynamic{Scalar,Unit}Call`, `DynamicParameter{Scalar,Unit}Call`,
  `NormalizedForeignCall`), and the foreign call adds separate scalar and
  structural lanes that cannot mix. That is one producer family per signature
  permutation, not a normalized signature. The general form is one call
  operation carrying an ordered per-position parameter class, a result class
  and a callee source, realized by the target's calling policy.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Complete
  [representation agreement](wiki/spec/build/opaque_representations.md) at
  independently compiled by-value exchanges. Selection and review exist.
  `representation-planning` admits a selection only after the transitive
  inert-carrier and copy checks
  (`src/representation_selection/carrier_closure.rs`); calling-policy closure,
  general layout and compatibility boundaries derive the carrier from that one
  selection list (`evaluate_compatibility_boundary_entry_plan`); and
  dependency-first review
  rejoins each consumer's foreign opaque use to the producer's own declaration,
  availability rows and selection, rejecting a consumer-local application with
  `SelectedApplicationMismatch`
  (`PackagePolicyRepresentation::rejoin_foreign_demands`,
  `package-manager/tests/opaque_boundary_agreement.rs`). That is compile-time
  and review-time policy. No native artifact carries the selected application,
  and no opaque by-value crossing is transported or executed.

  Remaining work:

  - Physical transport in Omega lowering and the backend: move the selected
    carrier's bytes through argument, result and nested-field placements. An
    affine or linear value keeps one semantic occurrence while bytes are copied
    for placement; only a checked semantic copy creates another occurrence.
  - Artifact custody: bind the strong selected-application commitment into
    native artifacts, installation records and replay, so producer and
    consumer artifacts compare it at each actual by-value edge.
  - Replacement: carry the application through replacement compatibility,
    stable-handle eras and independently replaceable provider contracts.
    **COMPONENT-SUBSTRATE** owns the component closure these attach to.
  - Cleanup-owning carriers need the separate versioned lifecycle relationship
    the specification reserves. Until it exists every selection stays `Inert`
    and a cleanup-owning carrier rejects.

  Acceptance: independently compiled producer/consumer and historical-selection
  canaries cover sealed `Ptr<T>` target semantics, proof-only `Real`,
  `EfiSystemTable`, provider/replay drift, cleanup, and multiplicity;
  incompatible by-value exchanges and replacements reject before execution.
  Equal size/alignment or a compact fingerprint never establishes agreement.

- **WRITE-ONLY-BORROW.** Finish `&write T` under
  [write-only authority](wiki/spec/terminal-psi/structural_access.md#write-only-authority)
  through calls/results, dynamic dispatch, cleanup and native execution. Every
  artifact Psi emits today already replays natively through ordinary reference
  preparation in
  `target-operations-to-selected-instructions/src/legalization/scalar_graph_input/`:
  projected `&write`/`&mut` receivers and arguments, disjoint argument pairs,
  re-forwarding, literal-indexed element and field stores, IEEE value stores,
  head-of-body `let` subloans with restored parents, and reference carriers
  through installation
  (`tests/native-differential/tests/terminal_psi_indexed_receivers/`). The open
  work is upstream: for the shapes below Psi produces no artifact, mostly with
  `machine has no source-independent checked scalar control plan`
  (`checked-trees-to-lowered-psi/src/machine_lowering/machine_dispatch.rs`).
  **STRUCTURAL-BORROW-IDENTITY** owns the common reference ABI.

  Remaining work. The list was recorded 2026-09-15 and Psi borrow planning has
  changed since; rerun `tests/native-differential/tests/zz_wob_probe.rs`, which
  prints each candidate's outcome, before assuming a row still fails.

  - A borrow held in a `let` is usable only as a head-of-body receiver. Make it
    an ordinary place alias in checked execution planning
    (`typed-trees-to-checked-trees/src/execution/`): `let` borrows after other
    statements, a held scalar or aggregate borrow as a call argument or store
    root (`held[2] = 17` rejects while `held[1].replace()` composes), and a
    `&mut` receiver call on the parent while a disjoint subloan is live.
  - Stores through a borrow beyond primitive leaves: a projected-element scalar
    store on a borrowed array (`records[1].value = 17` under `&mut`) and whole
    aggregate or `[copy]` sum replacement.
  - An owned record's field lent as a call argument. Its exclusive form is the
    owned-root subloan rule in **STRUCTURAL-BORROW-IDENTITY**.
  - Shared `&` scalar callee bodies: "scalar callee has no checked executable
    body" (`checked-trees-to-lowered-psi/src/scalar_graph/scalar_call_closure/callee.rs`).
  - Runtime indexes. Terminal Psi has `WriteOnlyIndexedPrimitiveStore` with
    verifier, codec and interpreter support. No Psi producer emits it, a
    declared `[0..=3]` index range still yields no plan, and Omega rejects it
    with `LoweringError::UnsupportedIndexedPrimitiveStore`
    (`terminal-psi-to-abstract-operations/src/lowering/machine/operation/effects.rs`)
    because the abstract inventory has no runtime-index carrier or bounds
    obligation.
  - `&mut dyn` dispatch.
  - Computed IEEE stores: a source-selected floating operation, its result
    transport and an ordinary store, retaining format, selected occurrence and
    result evidence through Psi's `execution/unit/selected_ieee_float.rs` and
    Omega's shared graph/provider route. Widening store admission is not the
    repair; **FLOAT-PROVIDERS** owns the operations. A 135-file slice for this
    was parked on an unpublished local branch `write-only-borrow`
    (71a647f464); it is not on `origin`. Ask the coordinator whether it still
    exists before re-implementing.

  Acceptance: writes affect the original caller referent across calls and
  register/stack passing; reads through write-only access, bare `&write`
  forwarding, `&write`-to-`&mut` widening and same-root `&write` argument pairs
  reject. Cover exact width/write coverage, untouched neighbors, runtime
  signed/Boolean/floating sources, restoration/return behavior, access
  substitution and independent artifact replay. Observe computed floating
  stores on the caller, not only in checking or a copied frame home. Add each
  shape to the `terminal_psi_indexed_receivers` suite, not a store-specific
  emitter. Run both Linux target runtime legs when available and record
  unavailable hosts; cross-emission is not matching-host execution.

  Flag: the `let`, store and owned-field rows fail at one place, not three. The
  attached Unit planner admits bodies by statement shape
  (`typed-trees-to-checked-trees/src/execution/unit/control/checked_machine.rs`:
  a `take_while` over leading `LocalData` statements, a `borrow_alias_prefix`,
  and tests such as `local_count != 1 || calls.len() != 1 ||
  statements.len() != 2`), so every new arrangement of lets, calls and stores
  needs another case. The general mechanism is ordinary statement sequencing
  over places and loans, which **STATE-LOCAL-VALUE-FRONTIER** names; closing
  rows here one by one extends the recognizer. Separately, `zz_wob_probe.rs`
  reached `main` in two `wip(write-only-borrow): interrupted mid-slice`
  commits: one test sweeps 21 candidate sources and asserts 4, so the other 17
  can change outcome unobserved. Promote passing candidates into named suite
  tests with caller-storage observation, pin the rest as rejections, and
  delete the probe.

- **STRUCTURAL-BORROW-IDENTITY.** Enforce the settled
  [structural borrow identity contract](wiki/spec/terminal-psi/structural_access.md)
  through call argument preparation and native validation/replay. The
  [target signature checks](omega-rust/omega/pipeline/abstract-operations-to-target-operations/README.md#references-calls-and-storage)
  now replay embedded callee plans, projected argument/home identity and
  standalone receiving entrances against signatures derived from the
  declarations. Every borrowed access keeps `BorrowedReference`, an inline byte
  field cannot satisfy a parameter by shape equality, and an exclusive view
  reaches an ordinary call from a non-entry block parameter. One caller form
  has no admitted route: an owned root cannot lend an exclusive projected
  subloan.

  Remaining work:

  - Admit `Owned` storage as the parent of a mutable or write-only projected
    loan. A projected shared borrow rooted in an owned non-entry block arrival
    lowers, legalizes and replays. Making only the argument exclusive rejects
    with `StructuralCallContractMismatch` in
    `validate_psi_optimization_unit_with_admitted_cycle_machines`, before the
    source-to-target join runs. The rule is `structural_arguments_match`
    (`optimization-unit-semantics/src/unit_validation/operation_contracts/structural_access.rs`):
    `static_borrowed_path` admits mutable-to-any, shared-to-shared and
    write-only-to-write-only parents, `unrestricted_mutable_subloan` requires
    `source.access == MutableBorrow`, and `Owned` appears in neither. The
    restriction covers every owned root, including an established record home,
    not only block arrivals. The Terminal verifier's
    `is_unrestricted_mutable_subloan` and `is_unrestricted_write_only_subloan`
    (`terminal-verifier/src/validation/structural_operations/structural_arguments.rs`)
    also require a borrowed caller parameter as parent; change both sides
    together.
  - The [reborrow table](wiki/spec/terminal-psi/loans.md#reborrow-lineage-and-access)
    has rows for Read, Mutable and Write-only parents and none for an owned
    root. State the owned-parent rule there before changing the validators.
    If the specification is genuinely silent, file it in `OWNER_QUESTIONS.md`
    and mark this step blocked; no such question exists today.
  - Do not retry by relaxing `aggregate_borrows::argument` or the legalization
    aggregate route. Lowering and legalization already admit the form, so that
    edit lowers the program and then fails at the same unit-semantics rule.
  - Record runtime results for both Linux targets. The target-lowering
    substitution matrices cover linux_x64 and linux_arm64, but the board has
    recorded native `terminal_psi_indexed_receivers` runs on macOS arm64 and
    Linux x86-64 only. `primitive_store_return` is the second native leg.

  Acceptance: caller-visible writes, forwarded references, legal synchronized
  shared observations, write-only non-reading, and register/stack pointer
  passing work on both Linux targets, including an owned local's field lent
  `&mut` and `&write`. Independently formed or substituted access/shape/
  placement pairs reject; shared physical shape never authorizes access
  substitution. Direct-home controls use owned semantics or test rejection of
  borrowed copies. A following callee seeing the staged write is not
  caller-visible writeback.

  Flag: projected-argument admission is a roster of access pairs and path
  shapes, kept in two places with different path grammars. For an owned source
  `structural_arguments_match` admits an empty path, a field-only path,
  `[FixedIndex]`, `[FixedIndex, FixedIndex]` or a partial affine path under
  the `Unit` policy and any path under `Projected`, and the policy is chosen by
  call kind (`CallUnit` against `CallStructuralScalar`), so the admitted paths
  depend on the callee's result. The verifier's
  `is_bounded_structural_scalar_store_path` admits fields followed by at most
  one index: `[Field, FixedIndex]` fits that grammar and not the `Unit`
  roster, `[FixedIndex, FixedIndex]` the reverse. The general rule is parent
  custody from the loan table, requested access, and any type-resolved static
  `Field`/`FixedIndex` path, stated once in `terminal-semantics` and used by
  both validators.

  DESIGN-BLOCKED (2026-09-18, measured): the owned-parent projected subloan
  above needs an owner decision before either validator changes. The
  [reborrow table](wiki/spec/terminal-psi/loans.md#reborrow-lineage-and-access)
  has rows for `Read`, `Mutable` and `Write-only` parents and none for `Owned`;
  `loans.md` contains no occurrence of "owned" or "ownership" at all, and
  `structural_access.md` states no reborrow relation for an owned parent. No
  `OWNER_QUESTIONS.md` entry covers reborrow, owned roots, owned parents or
  subloans. Filing one is itself blocked today: that file requires "an
  independently motivated product requirement or credible external use case",
  and no authored customer exists -- `tests/omega/pass` has none and shipped
  `source/` has none. Two near misses are NOT customers: Cathedral's
  `installer.omg` lends `&mut self.pml4` from an already-`&mut self` receiver,
  a Mutable parent, and
  `pass/dependent/mutable_referent_alias_corruption_restored_compile` projects
  from a `&mut Level` parameter. The owned root is not uniformly excluded:
  `path_shape_matches` admits owned-to-owned partial-affine projections and
  `shared_affine_loan` admits an owned affine root lending a whole-root shared
  borrow; only the owned-parent EXCLUSIVE projected subloan has no route. The
  rejection is unpinned by any test --
  `subloan_rejects_access_amplification_and_wrong_multiplicity` varies the
  argument and parameter access to `Owned`, never the source -- and should stay
  unpinned while the intended direction is to admit the form. The path cited
  above is stale: the crate is
  `omega-rust/omega/semantics/optimization-unit-semantics/`, not
  `representations/`, and the three rules are `let` bindings inside one
  function, not `fn`s, so `fn`-anchored greps find nothing.

- **BORROW-PROOF-CONVERGENCE.** Make ordinary borrow checking proof-producing
  under the [loan contract](wiki/spec/terminal-psi/loans.md): relational
  evidence may establish disjointness or containment between existing places
  and occurrences, and never creates, extends, duplicates or widens a loan.
  Owners: `typed-trees-to-checked-trees/src/checks/borrows/` and the
  certificate rows in `checked-trees/src/checked_trees/borrow.rs`.

  Index extents compare as normalized bounds inside one replayed selector
  session (`overlap/indexes.rs`, `overlap/segments.rs`), `overlap/premises.rs`
  supplies ordering premises from the forming scope's own `requires` rows, and
  forming-loan and statement-mutation admissions retain replayable
  `Structural` or `Premised` certificates. Bounds admit an integer, one
  immutable symbol plus a constant, or the canonical sum of two distinct
  immutable exact-domain symbols plus a constant
  (`validation::immutable_integer_bound_sum`, also unfolded through immutable
  locals bound to bound-shaped initializers so generated index hoists reach
  the sum); `pass/borrows/borrow_premised_sum_index_mut` witnesses the
  admission. That is still not general proof-derived compatibility: exactly
  one premise answers a query, multi-term arithmetic and negative or
  repeated coefficients stay unknown, and certificates are checked-stage
  records that no lowering or Terminal reader consumes.

  Remaining work:

  - Admit disequality. `premises.rs` decomposes `<`, `<=`, `>`, `>=`, `==` and
    `&&` only, so the guide's ordinary case
    ([Borrow Facts](wiki/language_guide/chapter_2_ownership_borrowing_moves.md#borrow-facts)),
    `&mut items[i]` beside `&mut items[j]` under in-range `i != j`, has no
    premise form. `fail/borrows/borrow_unknown_index_pair_mut` pins the
    unproven pair; no pass canary states the premise.
  - Take premises from every establishment point the loan contract names, not
    only machine-entry and state `requires`: dominating guards, callee
    `ensures`, domain membership and theorem-call conclusions, each valid for
    the captured value and place versions at formation. `premises.rs` states
    that it mirrors `ranges::requirements::seed_state_requires`; read the
    range checker's established facts (`checks/ranges/guards.rs`,
    `incoming_guards.rs`, `requirements.rs`) through one shared reader instead
    of growing a second collector. Mutable, computed and foreign subjects stay
    unproven until that reader supplies version evidence.
  - Retain certificates for call judgments. `checks/borrows/calls/` consults
    stated premises for argument/argument, argument/loan and receiver/argument
    conflicts and records nothing, so a premise-dependent call admission
    cannot be replayed.
  - Range-premise read sets (`checks/ranges/facts/dependencies/reads.rs`) stay
    incomplete for requirement-dispatched calls, machine-valued and nested
    static applications, quotient and private-layout operations,
    `CompareExchangeOnce`, and authored non-arithmetic operators. Admit one
    only with a complete footprint and operation stability: explicit arguments
    do not establish all callee reads, and preserved numeric captures must
    stay independent of later source writes. The builtin bound-meaning floor
    in `record_dependencies` (`facts/dependencies.rs`) decides what may be a
    range premise. It is not a read-set limit; do not widen it under this item.

  Acceptance: `tests/omega` pass canaries admit two mutable element loans
  under `i != j`, and a write and an exclusive call operand beside a borrowed
  symbolic window under a guard-established ordering; every admission replays
  from its retained certificate. An absent, non-strict, stale, reordered or
  tampered premise, a write inside the borrowed extent, and a second mutable
  loan licensed only by proven containment all reject. No certificate extends
  a lifetime, duplicates a loan or replaces resource accounting. Start from
  `src/tests/borrow/checks/premised_disjoint_writes.rs` and
  `src/tests/borrow/certificates/`. **CANARY-CORPUS** routes borrow-obligation
  failures here.

  Flag: the read-dependency part has no customer. Fourteen consecutive changes
  each admitted one more expression form into `reads.rs` (now 1,652 lines) and
  touched only that directory and crate unit tests; no `tests/omega` or sample
  file changed, the authored-arithmetic change records that no source compiles
  differently and that its witness fabricates an operator-use row production
  never emits, and nothing under `checks/borrows/` reads `RangeFacts`. An
  incomplete read set is conservative, because any write then retires the
  fact. Resume from a corpus program that loses a fact it needs, not from the
  next refused node kind. The same change recorded one candidate: builtin
  `items[low + 0u64..high]` cannot prove its start bound in
  `checks/ranges/indexes/validation.rs`; rerun it before relying on that.

  DESIGN-BLOCKED (2026-09-18, measured) for the disequality and call-certificate
  bullets only; endpoint formation and rank-role discovery remain ordinary
  engineering. (1) Disequality: `decompose_premise_expression`
  (`checks/borrows/overlap/premises.rs`) decomposes `And`, `Less`,
  `LessOrEqual`, `Greater`, `GreaterOrEqual` and `Equal` and has no `NotEqual`
  arm, so a stated `i != j` is dropped before the consult at
  `overlap/indexes.rs`. `BorrowCompatibilityPremiseRelation` is the closed
  triple LessOrEqual/StrictlyBefore/Equal inside a PERSISTED certificate that
  Terminal replay consumes positionally, and `premise_orientation_proves` is a
  total 3x3 match over constant-offset shifts; disequality orders nothing, so it
  has no cell, and the only other derivation, `Structural`, retains no premises.
  Not the blocker: `has_builtin_decomposed_guard_meaning` already accepts
  `NotEqual` in its boolean-equality branch. Evidence landed as
  `fail/borrows/borrow_stated_index_disequality_mut`, which moves to `pass/`
  unchanged once the decision lands. (3) Call certificates: `calls/conflicts.rs`
  already threads `stated_premises` through its consult sites and discards the
  `CapturedPlaceCompatibility`, and the capture half exists
  (`*_with_selector_snapshot` / `*_from_selector_snapshot`). What is missing is
  a persisted row. `CheckedBorrowCompatibilityCertificate` requires a valid,
  distinct `forming_loan`/`active_loan` pair, and an argument/argument conflict
  has neither, so retention needs a new row type, a new arena, its own
  `*_matches_resources` drift validation, and a change to
  `AcceptanceSummary::accepted`'s certificate count in
  `checked-trees/src/checked_trees/admissibility/statement.rs`, which alters
  admissibility accounting for every statement. A real customer exists and
  retains nothing today:
  `premised_disjoint_writes.rs::stated_ordering_premise_admits_exclusive_argument_before_borrowed_window`.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Realize target-owned private callback
  slots natively under the
  [private-callback contract](wiki/spec/build/private_callbacks.md). Checked
  compilation and the Terminal product already close for
  `source/library/std/tests/callback_materialization_closure.omg`: two
  `NativePlace::Field` placements selected through exact conformances, one
  `BoundaryCall`, and one canonical thunk artifact per placement
  (`compiler/tests/callback_terminal_custody.rs`,
  `reachable_private_callback_registrar_binds_its_terminal_occurrence`). No
  native product exists for any callback. `emit_realization_object`
  (`native-realization/src/native_realization/object_emission.rs`) rejects
  every request carrying a callback thunk or native callback, so the
  direct-parameter witness
  `direct_callback_relocation_resolves_to_its_private_function`, which expects
  a realized image, cannot pass either.

  Remaining work:

  - Lower each thunk's Terminal artifact to machine code inside the same
    realization. `produce_callback_thunk_artifact`
    (`checked-compilation-to-terminal-artifact/src/native_proposal/mod.rs`)
    stops at `finalize_terminal_artifact`; `NativeCallbackThunkSettlement`
    carries an artifact, a lowering receipt and an entry plan, never bytes.
  - Give retained realization a private-function channel.
    `build_object_artifact_with_private_functions`
    (`image-emission/src/object_artifact/construction.rs`) has no non-test
    caller. `emit_optimized_fragments` builds its object through
    `build_function_fragment_object_artifact`, which has no such channel, and
    `validate_private_functions` admits at most one private function where
    the fixture needs two.
  - Retain the admitted registrar context in the target plan. Normalized
    foreign call legalization
    (`target-operations-to-selected-instructions/src/legalization/scalar_graph_input/target/normalized_foreign.rs`)
    fails closed on a non-empty `callback_materializations` roster because it
    cannot replay one.
  - Add a layout-field address destination. `CallbackAddressDestination`
    (`machine-code/src/machine_code/calls/callbacks.rs`) has only `Register`
    and `OutgoingStack`; `validate_callback_address_bytes` and relocation
    replay cover those two on x86-64 and aarch64.
  - Authenticate the complete plan application and replay the
    authored-use-to-Terminal-operation join in the native receiving stages.
    The target-side application commitment and placement index are producer
    provenance only; see
    [callback custody boundaries](omega-rust/omega/compiler/native-realization/README.md#callback-custody-boundaries).
  - Then delete the rejections instead of widening them:
    `reject_unconsumed_callbacks` (`native_product/admission.rs`), the
    one-direct-callback and field-cohort arms of `admitted_native_callbacks`
    (`retained_native_product.rs`), and the optimization-selection arm of
    `lower_realization_optimization_stage`.

  Acceptance: the two-slot registrar fixture and the direct-parameter witness
  each produce a native image in which object and final-image replay bind the
  private symbol, relocation, executable region and patched address to the
  same function and native destination. Source holds no raw code pointer and
  no placement authority is duplicated. Missing, duplicate, reordered,
  substituted and shape-incompatible materializations reject, and a private
  slot has no source projection, read, write or address. Registration outcome
  and lifetime belong to **REGISTERED-CALLBACK-LIFETIME**.

  Flag: the callback body route is a fixed-shape cohort, not machine lowering.
  `lower_bounded_callback_identity_machine`
  (`checked-trees-to-lowered-psi/src/machine_lowering.rs`) admits a
  `u64 -> u64` identity return with no bindings, or a `u64 -> Unit` body whose
  only operation is `Complete`; the second matches the fixture's empty `{ }`
  provider. `validate_direct_callback_thunk_shape` lists the same two leaves,
  and `validate_private_functions` rejects any body with a call, parameter
  ABI, port effect or boundary settlement. A window procedure has several
  parameters, calls and a result. Lower the selected machine through ordinary
  machine lowering rooted at the callback entry, and validate the thunk
  against the requirement's signature and inbound entry plan, not against a
  list of admitted bodies.


  The `callback_terminal_custody` suite ran 0 of 5 before 2026-09-18, and not
  for a callback reason: since 2f30c89f04 gave `windows_x86_64` a closed
  physical-contract package, binding its `ProgramEntry` pulls the authored
  target contract and its bundled `std::calling` module into the program,
  while the fixtures still carried their own copy of `calling.omg`, so each
  compile died on duplicate declarations before reaching any callback
  behavior. Composing them like the Windows hosted-receiver fixture, with the
  standard library as an ordinary dependency, takes the suite to 5 of 6; the
  remaining failure is the documented wall, callback ABI transport absent
  from the common instruction pipeline. Other fixtures that bind
  `windows_x86_64::ProgramEntry` beside a package-local library copy may have
  broken the same way at that commit, and one had: `calling_policy_plans` was
  45 of 59. `hosted_entry_contract_seed` seeds the authored target contract
  into every target-selected compilation, needing no build declaration and no
  entry binding, so any fixture carrying its own copy of a bundled
  standard-library source declares it twice. Eleven calling-vocabulary tests
  copied the calling module and three macOS entry tests copied the target
  contract itself; taking the standard library as an ordinary dependency, and
  checking the bundled contract under its custody as the Linux and Windows
  fixtures already did, returns that suite to 59 of 59. One of those tests had
  been asserting a rejection the compile never produced, so it passed while
  observing nothing, and now reaches exactly that one diagnostic.
- **REGISTERED-CALLBACK-LIFETIME.** Model successful registration as a linear
  external root, and unregister as the operation that ends it before code and
  component leases release, under
  [registration and lifetime](wiki/spec/build/private_callbacks.md#registration-and-lifetime)
  and [opaque retention](wiki/spec/build/component_publication.md#opaque-retention-and-quarantine).
  Capacity bounds live registrations, not emitted thunks.

  `component-publication/src/callback_registration.rs` holds the runtime
  ledger: private-entry attribution, registration lease, `lower_registration`,
  `unregister_and_quiesce` and `release_component_era`. Its only caller is
  `tests::package_registration_owns_exact_component_era_lease_through_replacement`
  in the same crate, which sequences the ledger by hand. No Omega source,
  build declaration or canary expresses a registration or an unregister, and
  registration capacity is named only in `component-publication` and
  `external-roots`.

  Remaining work:

  - An authored registrar customer under the linked contract: success yields
    the linear registration holding the exact live-registration capacity
    occurrence, failure returns that capacity with no root, and unregister
    consumes the registration. Use ordinary linear custody; add no
    registration-specific checker rule.
  - Omega: join that boundary outcome to the ledger, so root admission, lease
    acquisition, quiescence and lease release follow the program's operations
    and not a Rust caller's sequence.
  - **CALLBACK-PRIVATE-MATERIALIZATION** must supply a native callback entry
    before any foreign invocation can be witnessed.

  Acceptance: one authored program registers, observes a rejection and retries,
  replaces a registration by reusing the returned slot and capacity, and
  unregisters with quiescence before lease release. A dropped registration, a
  second unregister, a stale or cross-occurrence registration and reuse of
  spent capacity reject. On Windows a real foreign callback enters the
  registered machine; other hosts report the leg unavailable. The ledger test
  is not the customer witness. **FFIVAL** names the same host-gated run.

  Flag: the previous acceptance counted rejection, retry, replacement and
  cleanup as covered by that Rust test, although no program can reach the
  ledger. It is supporting machinery until an authored registrar drives it.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Execute outbound arguments that a
  foreign callee retains after return, beyond callbacks, under
  [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody), with
  explicit call-scoped, lifetime-borrowed, moved and snapshot dispositions.
  Every retained pointer needs exact stable backing, range, access, lifetime
  and revision provenance; unknown or mutable ambient backing rejects.

  Checking already derives the disposition from the authored contract. A
  consumed owned source or one exact shared lifetime-bound source is recorded;
  borrow-only, mutable lifetime-bound and ambiguous sources reject
  (`typed-trees-to-checked-trees/src/tests/content/retained_content_custody.rs`,
  `core/content_retained_custody_round_trip`,
  `fail/core/content_retained_custody_from_borrow`). The shared-borrow row
  lowers to `terminal_psi::RetainedBorrowCustody`
  (`checked-trees-to-lowered-psi/src/retention/retained_borrow_custody.rs`).
  It cannot be invoked: the Terminal verifier rejects every boundary call whose
  declaration carries a `RetainedBorrow` guarantee with
  `RetainedBorrowBoundaryIsNotExecutable`
  (`terminal-verifier/src/validation/structural_operations/unit_operation/boundary_calls.rs`).

  Remaining work:

  - Psi/Terminal: admit the invoked retained-borrow call. Keep the caller's
    loan live for the result's lifetime, bind it to the exact result
    occurrence, and then delete the verifier rejection.
  - Psi: the snapshot disposition has no source or checked form. It needs the
    contract's explicit permission for independent copying and persistent
    demand counted per live occurrence.
  - Omega: native realization materializes a retained pointer only from an
    established storage claim, and each slot carries root, range, access,
    lifetime and revision or lease provenance.
    `ProgramLocalExtentRegistry::retain_foreign_argument_{borrowed,moved,snapshot}`
    (`external-roots/src/program_local/program_local_extents/retained_foreign_arguments.rs`)
    already records that and rejects unheld, provider-issued, stale-era,
    out-of-range and excess-rights backing. Drive it from the Terminal custody
    row.

  Acceptance: one authored boundary per retaining disposition runs natively on
  an available host: a moved buffer redeemed by its completion, a shared loan
  retained for an explicit lifetime while a conflicting write rejects, and a
  permitted snapshot. Retention from a call-scoped borrow, a mutable
  lifetime-bound source, an ambiguous source mapping, or unknown, stale,
  out-of-range or excess-rights backing rejects. The moved disposition's
  conserved-content route belongs to
  **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS**; registration custody
  belongs to **REGISTERED-CALLBACK-LIFETIME**.

  Flag: that ledger has no caller outside its own tests, and its disposition
  is whichever Rust method the caller picks. The contract assigns that choice
  to the authored types, so the Terminal custody row must select it.

## P5 - Cathedral over general Omega primitives

- **BUMP-ALLOCATOR-CANARY.** Build a package-level allocator over one qualified
  `Extent` under the [allocation contract](wiki/spec/resources/allocation.md):
  two coexisting allocations, exact cleanup/recomposition, and reset only after
  full return. Use it to discover the real `Vec<T>` contract; do not add
  allocator semantics to the compiler.

  `tests/omega/pass/memory/bump_allocator_canary` checks that chain, a guarded
  fallible request, a one-buffer `BumpVec` reservation, content-free growth
  (`grow` rebuffers via `merge` + `split`) and one resident
  place/read/retire; six `fail/memory/bump_allocator_*` controls pin the
  rejections. Its header records the contract edges found so far (the note that
  an indexed domain application does not parse in proof-fact position is stale
  since `pass/contracts/proof_fact_indexed_domain_application`). This is source
  checking only: the fixture is on the `CHECKED_ONLY_PASS_CANARIES` roster,
  `Main::main` is empty, and `ExtentPartition`/`ResidentStorage` are
  fixture-local boundary traits with no conformer or selected provider.
  Split/merge conservation and resident establishment are asserted boundary
  laws, and nothing lowers or executes.

  Remaining work:

  - Container. A `Vec<T>`-style owner needs elements, a length and
    content-preserving growth. Elements need the source
    `Initialize`/view/retire route of
    [placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement)
    (evaluated plan of `P` over `T`, Stable-supply admission), which
    `PLAN-LAID-VIEWS` owns. `source/library` declares neither that family nor
    `ResidentContentTransfer<P, T>`; the fixture's `ResidentStorage` is a
    stand-in to replace when the route exists. Growth is currently
    content-free rebuffering only: `merge` demands `Vacant` parts, so a
    buffer with a live resident cannot fold back for a resize — pinned by
    `fail/memory/bump_allocator_grow_with_live_resident`. Element transfer
    across the fold waits on the same placed-access route as elements.
  - Custody-carrying sums. A destructured case payload and a call-result
    record's fields do not surface their declared `in Granted` domains. A
    sum-typed fallible request, an optional retired slot and a retired-buffer
    list construct but cannot be consumed, and each record field is restated
    through an exactly typed `let` whose siblings then share custody.
    `NOMINAL-FIELD-FLOW` owns declared-field evidence; repair it there.
  - Counted residual. `split`'s law never pins `result.taken.length`, a
    content-carrying machine's `ensures` admits no scalar equality, and a
    `requires` bound neither carries a subtraction's lower bound nor survives
    consumption of the backing. Each request's residual is a caller-stated
    premise, and post-reset reuse is reachable only through a runtime guard.
    A second edge surfaced in `grow`: the requires discharger does not
    reduce an inline constructor's field to the caller premise when a
    sibling field binds a call-result local (`Bump { tail: widened, ... }`
    leaves `... .remaining` unreduced), so `grow` re-carves through the
    boundary `split` rather than reusing `allocate`.
  - Partition theorems. No checked body splits one `Granted` extent into two;
    the fixture delegates that step to a boundary. Returning `Granted` custody
    from two consumed qualified inputs rejects as ambiguous at a boundary and
    in a state, so merge takes one `Split` record and folding retained buffers
    back has no spelling. Settle both with
    `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS`' invoked partition route.
  - Strategy borrow. The contract has allocation borrow the strategy
    exclusively; the fixture threads `Bump` by value because a `&mut` carve
    rejected. Retry it through `BORROWED-STORAGE-RESTORATION`'s move-out/replace
    window before keeping the by-value shape.

  Acceptance: a package container over the bump chain places, reads and retires
  elements and grows while retaining its old buffer until reset, with the
  allocator still ordinary package source. It checks and executes through the
  interpreter and a supported native host with a selected backing provider;
  report unavailable hosts. A live allocation or resident at reset, dropped
  custody, double placement, a wrong `Resident` index and an unrouted `Resident`
  introduction reject. Route each new contract edge to its owning task.

  Flag: `Extent::Resident<P, T>` is declared in `core/extent.omg` with no
  predicate and no `established by`, so
  [domains](wiki/spec/language/domains.md#exact-coercion-and-erasure) lets `as`
  introduce it although placed access fixes its route set. The landed fence,
  `append_unevidenced_establishment_diagnostics` in
  `typed-trees-to-checked-trees/src/facts/index_compatibility.rs`, refuses only
  a typed `let` whose value is an `ExpressionNode::Call`. A cast supplies its
  own instance and passes, and any package boundary may name `Resident` in a
  result type, as both fixtures do. The general mechanism is the existing
  routed-qualification rule: declare the family's routes when the placed
  operations exist, then delete the call-shaped check.

- **ADDRESS-TRANSLATION-CANARY.** Continue Cathedral's page-table hierarchy,
  backing, policy, installation and teardown in Omega source under
  [mapping and reclamation](wiki/spec/resources/extents.md#mapping-and-reclamation).
  Cathedral owns table formats, walk policy and lifecycle; Omega owns mapping
  authority, custody checking and the provider crossings. Cathedral's existing
  numeric page-walk validation grants no mapping authority.

  `tests/omega/pass/memory/address_translation_canary` checks an owned and a
  borrowed-source `TranslationAuthority` contract (`Pending` -> `Installed` ->
  reusable `Granted` custody plus a linear `Shootdown` debt) carrying authored
  obligation sets, beside a stand-in `cathedral/` package: a fully backed
  four-level hierarchy, entry encoding, the 9-bit level walk, three
  `TranslationHardware` boundary crossings, and a second level of mapping
  authority (`install_huge`/`remove_huge` store the terminal at depth 2, so a
  2MiB leaf needs no `pt` table; `cathedral_huge_install_and_teardown` drives
  the identical authority cycle). Eight `fail/core/translation_*`
  controls pin custody misuse, and `psi/foundation/extents/src/mapping/` is the
  Rust conservation model a provider reads. This is source checking only: the
  fixture is on the `CHECKED_ONLY_PASS_CANARIES` roster, `Main::main` is empty,
  and no conformer or provider is selected. The driver calls the bodyless
  authority crossings and `CathedralPageTables::install`/`remove` side by side,
  passing geometry as inert `addr` values, so no stored entry backs the
  `Installed` qualification and nothing joins the authored obligation sets to
  the Rust model's receipts.

  Remaining work:

  - Make the package the realization the authority's receipts stand behind. Its
    install and remove routes must establish `Installed` and release custody
    for the exact consumed extents through provider selection and the
    `extents::mapping` receipt join, not through a parallel call.
  - Multi-page installation. `install`/`remove` cover the mapping's first page.
    The per-page loop is a ranked cycle whose body computes `level_index` and
    crosses `TranslationHardware::store_entry`. `preserves_rank` in
    `validation/src/machine_calls/call_cycles/runtime_ranking/prefix.rs` admits
    inert statements, disjoint stores, and statement-position calls to
    checked-body callees with inert arguments and complete disjoint write
    frames. A `let` whose initializer calls a machine and any call to a bodyless
    boundary or requirement callee reject with "a write, call, or alias
    invalidates the entry-relative ranking"; probes on 2026-09-18 rejected in
    both named-state and call-component form.
    [Termination](wiki/spec/language/termination.md#ranking) applies ordinary
    contracts to calls outside the component, and
    `TERMINATION-RANKING-CHECKS` owns the repair. Do not respell the loop.
  - Demand-grown intermediate tables need the split/conservation surface
    `BUMP-ALLOCATOR-CANARY` uses and inherit its open edges.
  - Reading an entry back is placed access. Until `PLAN-LAID-VIEWS` supplies a
    source establishment route, the hardware edges stay boundary requirements
    and the package never consumes its own entries.
  - Execution needs selected providers for the authority, hardware and
    `Shootdown::discharge` crossings on a freestanding image, which depends on
    `UEFI-PHYSICAL-SEMANTIC-ENTRY` and `UEFI-OS-HANDOFF`. The repository has no
    QEMU harness.

  Acceptance: QEMU installs and tears down Cathedral-owned multi-page mappings
  with explicit `Extent` and TLB custody, and mapped access exists only between
  activation and unmap. Source use after map, unmap before activation, a lost
  shootdown, carrier construction, borrowed-source reclaim, a replayed
  activation, and a receipt for another mapping or a stale era reject. A
  checked-only fixture or a Rust receipt test is not the witness.

  The `cathedral/` directory is a stand-in inside the compiler corpus;
  Cathedral's repository owns the real package. Do not add page-table or TLB
  types to the compiler.

- **EXCEPTION-ROOTS-AND-TIMER.** Run Cathedral's fatal exception entries on
  dedicated critical stacks, its descriptor-table installation, and a minimal
  timer root whose hard handler only acknowledges, records and wakes ordinary
  work, under [interrupt obligations](wiki/spec/build/interrupt_obligations.md)
  and [hardware materialization](wiki/spec/build/hardware_materialization.md).
  Cathedral owns exception coverage, gate and IST policy, the controller, the
  timer device and the table's semantic validator. The compiler owns installed
  root records, the generic checked writer, entry/exit realization and the
  checked `lidt` contract.

  `external-roots/src/interrupts/` holds Rust ledgers for entry admission,
  nesting, epoch stages and settlement (`interrupt_entries.rs`) and for table
  member admission, a derived gate-offset writer plan, written-table
  validation, publication through the `lidt` edge and published-vector
  dispatch (`interrupt_table.rs`). `core/interrupt.omg` declares the mask and
  acknowledgement obligations, and the instruction catalog in
  `language-core/src/inline_assembly/` holds the deriver-only `lidt` contract.
  Every caller of those ledgers is a test inside `external-roots`. No authored
  program installs an exception or timer root, no backend stage emits a
  hardware entry/exit stub (`iretq` exists only in the instruction catalog),
  and the repository has no QEMU harness.

  `begin_published_interrupt_entry` now dispatches the member's declared
  obligation: a `FatalException` member's entry arrives unconditionally —
  the processor fault owes no declared stack-nesting edge or parent depth
  bound — and its settle halts the ledger (`InstalledRootLedger::halted_by`),
  so the interrupted chain it preempted can never resume ordinary work.

  Remaining work:

  - Authored roots. Cathedral's fatal-exception and timer entry machines
    install as external roots through target-declared requirements and provider
    selection, with deriver-owned entry/exit code, critical-stack arrival and
    [machine-state evidence](wiki/spec/build/machine_state_evidence.md) in the
    emitted image. Owners: `backend/machine-emission`,
    `calling-conventions/src/stack_realizations/` and `external-roots`.
  - Descriptor table. Author it as an ordinary source layout whose split
    entry-offset fields the generic post-handoff writer
    (`executable-installation/src/executable_installation/post_handoff_writer.rs`)
    materializes. The package's validator produces the established value and
    the checked `lidt` provider edge publishes it. The flag below names the
    ledger code this replaces.
  - Timer. The device source, tick record and wake are package code. The
    acknowledgement settles through `InterruptAcknowledgement::complete`, whose
    LAPIC/x2APIC reach waits on `BOUNDED-INSTALLATION-REACH-ROWS`.
  - Execution needs the post-exit environment from
    `UEFI-PHYSICAL-SEMANTIC-ENTRY` and `UEFI-OS-HANDOFF`, stack bounds from
    `TR3-TR8` and a QEMU harness. External interrupts stay disabled until the
    complete exception floor is installed.

  Acceptance: QEMU reports timer ticks over owned output and halts between
  ticks, and at least one deliberately raised fault reaches its fatal entry on
  its dedicated critical stack. Publication with a missing member, a table
  written by another writer or realization, a replayed publication or
  acknowledgement, a forgotten or double completion, and retirement of a root
  the table still names reject. Rust ledger tests and emitted but uninstalled
  stubs are not the witness.

  Flag: `external-roots/src/interrupts/interrupt_table/` is a compiler-owned
  Rust model of the x86-64 IDT. `InterruptTableGateDescriptor` carries selector,
  gate kind, privilege and IST slot; `InterruptTableProfile` requires one
  distinct dedicated stack class per vector;
  `descriptor_table_staged_image` builds the table bytes; and
  `validate_written_descriptor_table` decodes each gate, requires an IST slot
  and mints `EstablishedInterruptTable`. Hardware materialization assigns that
  validator and established value to the consumer package ("these policies are
  not compiler-owned types"), and interrupt obligations says Omega does not
  choose exception coverage or IST policy. No code outside the crate's tests
  calls it. The general mechanism is a source-authored table layout, the
  generic writer and a Cathedral validator; the compiler keeps root records,
  the IST-to-stack-class join that stack selection derives, and the `lidt`
  contract.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Finish
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach)
  for component contracts and for the completion route an opaque carrier owns.
  Concrete reach and conservative bounds stay separate; selected provider
  execution and token era, not row equality, authorize invocation. Parsing and
  checking of `reaches <= Bound`, the package-review fence on ordinary public
  callables, selected-row resolution
  (`provider-planning/src/provider_planning/installation_reach.rs`) and
  root-closure substitution (`external-roots/src/root_entry/root_validation.rs`)
  exist. A selected realization that itself retains an unresolved
  installation-bound requirement rejects there; no nested substitution step
  exists, so keep that rejection.

  Remaining work:

  - Completion route. `InterruptAcknowledgement::complete` in
    `source/library/core/interrupt.omg` still declares `reaches PortIo`, and
    `compiler/tests/calling_policy_plans/opaque_boundaries.rs` pins it as not
    installation-bound. Migrating the declaration is a verified one-line
    change that turns that suite red, because every realization of the
    installation-bound entry settles the linear acknowledgement and retains
    the nested bounded row, and no satisfier for `complete` can be authored
    while a checked body cannot discharge the linear receiver, so the route
    now waits on [owner question 5](OWNER_QUESTIONS.md). The rejection is
    driven from authored source by
    `opaque_boundaries.rs::selected_realization_with_an_unresolved_installation_bound_row_rejects`,
    so losing the fence is a red test whichever route is chosen. The
    component-contract bullet below is independent of this and claimable.
    [Interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime)
    requires a bounded row beneath `MachineControl + PortIo`, and
    `InstalledInterruptCompletionRoute` in `external-roots` rejects a completion
    requirement that has no installed resolution. Provider-planning tests
    resolve the bounded spelling only for a test-local `[copy]` lookalike.
    Migrate the shipped requirement and join its invocation to the carrier's
    exact provider execution, policy and token lineage; an x2APIC provider must
    not receive `PortIo`. Receiver-bearing requirement selection is
    `TOP-LEVEL-BOUNDARY-REQUIREMENTS`' work.
  - Component contracts. `component-description` publishes one
    `service_ceiling` that unions the module's concrete root reach with every
    installation dependency's upper bound (`derive_component_inventory`).
    Neither `verify_component`/`realizes_selected_plan` nor
    `provider_planning/independent_components.rs` checks unresolved rows. Keep
    concrete reach and bounds separate in the description and apply the
    selected-row rejection to `Independent` joins. The `COMPONENT-SUBSTRATE`
    description carrier this waited on now exists.

  Acceptance: from the shipped core requirement, PIC completion resolves to
  `PortIo` and LAPIC/x2APIC completion to `MachineControl` through checked
  source, Terminal Psi and the installed route. Cross-provider settlement with
  equal rows, a replayed token or era, an `Independent` component exporting an
  unresolved row, and final admission with any unresolved row reject.

  The board-hygiene question in `OWNER_QUESTIONS.md` covers whether this item
  folds into `COMPONENT-SUBSTRATE`.

## Parallel language and compiler lanes

- **BORROWED-STORAGE-RESTORATION.** (split-of:OMEGA-PRODUCT-COMPILER-SOURCE)
  Carry [borrowed-storage invariant windows](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
  from the source checker through lowering, Terminal verification and native
  realization. `typed-trees-to-checked-trees/src/checks/multiplicity/borrowed_windows.rs`
  opens a window on the exact resolved storage place for a consuming move
  through an exclusive chain, closes it at a same-typed store, and rejects an
  open window at returns and non-crash transition edges. The guide canary
  `pass/ownership/move_keyword_field_assignment` checks, and
  `checked-interpreter/tests/borrowed_restoration.rs` executes a round trip and
  a consuming transform with caller-visible contents. That canary is on the
  checked-only roster: nothing transports the restoration debt below checked
  trees, and [Terminal ownership](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration)
  has no producer or verifier for it. The product parser still avoids the
  pattern by borrowing the lexer's stream.

  Remaining work:

  - Lowering: transport the exact place/loan restoration debt through
    `checked-trees-to-lowered-psi` using the ordinary partial-move, store and
    control-flow relationships, without replacing the caller's storage by a
    staged copy.
  - Terminal: represent the window so `terminal-verifier` reconstructs it from
    operations, loan authority and control flow, then execute it in the Terminal
    interpreter and supported native targets.
  - Checker: a move inside a match arm or on a transition edge still takes the
    plain rejection, so branch-local extraction/repair and the reconvergence
    agreement rule are unimplemented. A suspending or blocking call across an
    open window rejects outright. Contained-loan transport and
    recoverable-failure paths have no regression.

  Acceptance: a consuming transform followed by replacement executes with
  caller-visible updated contents and exact-once custody in the Terminal
  interpreter and supported native targets, beside disjoint sibling work, repair
  on both branches and contained-loan transport. Missing repair on one
  returning branch, early return, stale field use, overlapping borrows,
  wrong-place replacement, repeated extraction and whole-owner cleanup reject,
  and tampered Terminal evidence fails independent replay. Outcome controls
  cover recoverable failure, suspension/resume/cancellation custody and
  crash/process-exit abandonment without invented rollback or survivor
  guarantees. `src/tests/multiplicity/borrowed_restoration.rs` holds the
  checker half. Do not add a recognizer for adjacent move/assignment
  statements, weaken nominal-drop restrictions, or delete the rejection gate
  wholesale; unsupported paths stay rejected until their evidence exists.

- **MATCH-SELECTIVE-LOWERING.** Complete the [value-dispatch
  contract](wiki/spec/language/patterns.md) for owned/nonnumeric results with
  parameter/projected/borrowed/linear custody, structural/case/domain patterns
  and coverage. Owned selection already transports interleaved live owners,
  authored state joins, projected affine children, whole state parameters,
  chained selection results, fresh structural call products and linear sources
  (a whole place, one projected child, or a whole carrier's claim frontier).
  Shared-borrow joins carry record and linear-record referents over named local
  or parameter roots with field and literal fixed-index segments, and plan a
  primitive-referent carrier. Do not rebuild those slices. Owners:
  `validation/src/value_custody/expression_types/{match_dispatch,result_type}.rs`,
  checked scalar computation/result continuations, Terminal production and
  canonical package-review contract/index projection. Preserve a
  once-evaluated subject, ordered first match, branch-local execution, exact
  result owners, exact origins and actual death edges; do not flatten
  conditional ownership into a statement-wide move roster.

  Remaining work:

  - Owned arms that forward or move existing affine custody through a call,
    receiver-bearing and subject calls still need their exact residual
    transport; they reject today. Record arms whose fields move existing
    affine children now check: each projected field leaf joins the transfer
    roster under its arm and source root carrying its own declared member
    type, overlapping moved paths on one arm reject as double consumption,
    and the lowering receipt verifies each projected leaf's claim frontier
    against its leaf type rather than the result type. What remains is leaf
    emission — `RecordFieldValue::Structural` arguments must carry an empty
    `path`, so leaf fields need per-arm extraction into whole places (edge
    arguments to block parameters to `EstablishRecord`), which sits in
    `src/unit`/`src/emission`;
    `owned_match_record_arm_children_check_but_await_leaf_emission` pins the
    boundary.
  - Claim-bearing bodies do not lower. The checker joins a whole affine root
    with linear children by naming each frontier claim, but lowering stops at
    "machine has no source-independent checked scalar control plan" for a
    machine whose body holds claim-bearing custody
    (`tests/value_dispatch/owned_results/linear_child_carriers.rs` pins it).
  - Borrowed-result consumers. A `&Payload` selection forwards its joined place
    to a call and executes. A `&u64` selection plans its carrier but has no
    consumer: a call rejects with the same control-plan diagnostic, an unread
    view rejects with "structural local carried a borrow event with no recorded
    loan", and `primitive_reference_read` admits only state parameters, so a
    borrowed primitive local has no read spelling. That spelling is an owner
    decision `OWNER_QUESTIONS.md` does not yet carry; do not invent one here.
    A direct `let view: &Payload = &a` outside a match builds no
    structural-value root, so the machine has no control plan
    (`established_reference_local_call_rejection_pins_the_checker_gap` in
    `tests/value_dispatch/borrowed_results.rs`).
  - Remaining borrowed joins are separate questions, not a wider carrier:
    exclusive (`&mut`) arms have affine custody of their own, and case-bearing
    referents stay outside the record-shaped frontier. Dynamic indexes, ranges,
    case members and computed roots in a borrowed target stay rejected.
  - Borrowed subject. Indexed affine tag observation reaches checked trees
    without copying the element (`typed-trees-to-checked-trees`
    `tests::multiplicity::borrowed_case_payloads`). Retain the once-captured
    borrowed subject and exact source-state loan closure when producing and
    replaying successor plans; checker acceptance is not Terminal lifecycle
    publication. `SAMPLE-CORPUS`' `dutch_flag` native command still stops at the
    missing transitive `Main::main` plan; preserve its exit-70 oracle and
    explicit copyable swap values.
  - Result types. Selected-operator and semantic-domain result types need full
    instantiated identity; input predicates are not arithmetic result facts.
    Indexed predicate/theorem and package membership applications need exact
    static arguments. Qualified callable-entry signatures, predicate/routed
    membership and erasure require real transport rather than payload-only
    projection. **OPERATOR-MACHINE-SUPPLY** owns declared operator execution;
    **CRASH-CONTRACT** owns crash-qualified equality; numeric landing is in
    **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: `checked-trees-to-lowered-psi --test suite` (`value_dispatch::`),
  `omega-native-differential-test --test scalar_case_results`, corresponding
  checker/interpreter controls, and the
  [float Match native customer](omega-rust/omega/compiler/compiler/float_realization.md#operation-and-control-custody)
  preserve effects, skipped trapping arms, overlapping patterns, complete
  coverage and independent replay. Keep `match_anonymous_result_landing` and
  `numeric_operand_destinations` as source probes. Wrong qualifications,
  ownership, selected result types and incompatible arms must reject before an
  outer bare-carrier cast; source validity alone is not native completion.

  Flag: admission here grows one source shape at a time. `is_record_value` in
  `typed-trees-to-checked-trees/src/values/scalar/computations/structural_values.rs`
  accepts a fixed list of arm expression kinds and admits
  `ExpressionNode::Borrow` only beneath a `Match`, which is why the direct
  borrowed local above cannot lower while the same borrow as a match arm can.
  The shared-borrow carrier rule is restated in three places that must widen
  together: validation's `selected_shared_borrow_place`, the checked
  `shared_record_reference` and lowering's `shared_borrow_record_referent`. The
  general mechanism is one checked place/loan establishment for a borrowed or
  owned source in any position, consumed alike by match joins, `let` and calls,
  which lowering and the verifier replay instead of re-canonicalizing the
  authored target.

- **OPERATOR-MACHINE-SUPPLY.** Implement the
  [machine token-binding and executable-supply contract](wiki/spec/language/expressions.md#executable-supply)
  from Psi parsing through native call realization, and remove the separate
  `operator` introducer instead of keeping two permanent source forms. A fixed
  token after `machine` already reaches every declaration representation.
  Resolution rejects duplicate owner-local shapes and operand tuples without a
  semantic home (`lowering/machine/token_bindings.rs` in symbol resolution),
  use sites select by operand type, and the checked stage rewrites each
  resolved binary use into an ordinary call on the declaration's entry state
  (`typed-trees-to-checked-trees/src/operators/token_bound_machine_calls.rs`).
  Token-bearing `boundary machine` signatures lower to the existing boundary
  slot, bare bodyless signatures are admitted only by exact catalog custody,
  and a bodyless nonboundary machine rejects at its declaration. That does not
  establish the contract: the introducer still parses, 208 `operator`
  declarations remain in `.omg` sources, only binary positions have body
  supply, both pass canaries are checked-only, and the three settled rules
  below are not enforced.

  Remaining work:

  - [Call preconditions](wiki/spec/language/machines.md#call-preconditions).
    `typed-trees-to-checked-trees/src/checks/contracts.rs` skips
    `check_call_requires` when caller and callee are both proof machines.
    Remove that skip and its mathematical application/citation counterparts;
    repair valid induction by carrying premises for the exact recursive
    arguments, not by exempting recursive-component calls. Then migrate
    `core/nat.omg`'s `Nat::{subtract,less_or_equal}` from `operator` plus
    `satisfies` pairs to declaration-owned bodies; `Nat::saturating_sub` stays
    the separate total operation. Controls:
    `proofs/nat_exact_subtraction_requires_order` and a named-call twin reject
    absent order even when the result is only mentioned in a proof term,
    discarded or erased; equal operands and predecessor at one accept; a
    decreasing recursive call with an unmet premise and a premise-satisfied
    call without descent both fail; a runtime-representable contracted machine
    owes the same obligations in runtime, admitted evaluation and proof use.
  - [Licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization).
    `validation/src/value_custody/type_references/open_index_expressions.rs`
    still finds its operation through `resolve_satisfied_checked_operator`,
    and the structural judgment's algebra-evidence join consumes the result.
    Require the caller's explicit conformance instead: commutativity for
    reordering, associativity for reassociation, each other rewrite its own
    law. Reuse conformance binders, operation selection and checked law slots;
    add no `using` clause, per-expression binder or index-operation registry.
    Migrate the four `IndexAlgebra::plus` fixtures
    (`generics/open_index_local_fact`,
    `generics/open_computed_quantity_result`,
    `fail/generics/open_index_unestablished_equality`,
    `fail/generics/open_index_unlicensed_algebra`) to trait requirements and
    named conformances, not to an operator injected into bare `u64`.
    Controls: missing selection, an unproved law or another operation's
    evidence rejects; commutativity alone does not reassociate; a
    noncommutative operation stays usable without rewrites; the constant-three
    operation shows AC implies neither zero identity nor integer addition.
  - Body supply outside binary expressions. `token_bound_machine_calls.rs`
    rejects `[]`, `[..]` and match-arm equality selections of a token-bearing
    machine, and a token use inside a build machine fails closed. Indexing uses
    [ordinary receiver borrowing](wiki/spec/language/expressions.md#indexing-and-ranges):
    route attached `[]`/`[..]` through the loan formation named method calls
    use. Re-author the recorded failure
    `tests/multiplicity/borrowed_observations.rs::indexed_operand_access_preserves_shared_collection_and_owned_index`
    ([baseline note](wiki/drafts/known_baseline_failures.md)) with an attached
    receiver and a separate ordinary-parameter control; do not restore
    wildcard operand re-seeding. Controls: `buffer[index]` and its named
    attached call agree on once-only evaluation, a shared collection loan and
    an owned index moved once; computed and borrowed results, mutable receiver
    authority, returned-loan lifetimes, bounds rejection, conflicting borrows
    and ambiguous adapted candidates are covered; ordinary parameters gain no
    receiver adaptation.
  - Closed-family ownership across packages. The current check is owner-local
    within one program. A free binding needs its home typed (which declared
    operand type owns the family), and only that home's owning package may
    publish it; an unauthorized declaration rejects at its declaration, not at
    a use.
    `validation/src/value_custody/expression_types/operator_validation.rs`
    must query the complete operand tuple: `Wrapped + u64` with only a
    `(Wrapped, Wrapped)` binding reports a builtin overflow obligation instead
    of a missing operator.
  - Package review. The callable identity carries the token
    (`capture/semantics/conformances/policy_callables.rs`);
    `CheckedPackageCallableReview` rows and trait `StateSignature` identities
    do not, and need record/encoding extensions.
  - Native execution. `expressions/declared_operator_match_result` and
    `expressions/token_bound_machine_operand_selection` stay in
    `CHECKED_ONLY_PASS_CANARIES` because native admission rejects borrowed and
    by-value local data arguments to a free machine, independently of token
    supply (**STATE-LOCAL-VALUE-FRONTIER**); by-value operands in a match arm
    also wait on **MATCH-SELECTIVE-LOWERING**'s owned-match custody join. Keep
    the selected-call join in `values/scalar/computations.rs` (checked stage)
    and `scalar_graph/scalar_computations/source_custody.rs` (lowered Psi)
    compositional.
  - Introducer retirement. `operator` parses in
    `tokens-to-syntax-trees/src/declarations/{parse_declaration,operator,domain,trait_definition}.rs`,
    and trait and domain bodies accept a token only through it. 171 of the 208
    declarations are tokenless `boundary operator` rows (124 in
    `core/float_operations.omg`, 29 generic rows in
    `core/{slice,vec,array,fixed_vec,ptr}.omg`, 18 in `tests/omega`); they
    move to `boundary requirement` through **TOP-LEVEL-BOUNDARY-REQUIREMENTS**,
    which executes only public nongeneric receiver-free requirements today.
    The two fused-multiply-add rows already use the requirement-side intrinsic
    bridge (`provider-planning/src/compiler_intrinsics/requirement_view.rs`).
    The two `Nat` and four `IndexAlgebra` declarations follow the first two
    bullets. The other 31, all in `tests/omega` (`operators/` overload and
    duplicate controls, `generics/closed_indexed_quantity`,
    `termination/custom_ranking_*`,
    `termination/computed_measure_authored_operator`,
    `terminal_psi/structural_scalar_trait_operator`), take declaration-owned
    bodies or trait `machine <token>` requirements, as do about 360
    declarations embedded in 95 Rust test files. Then invert
    `typed_trees::operator::SpelledOperator` to wrap machine signatures;
    provider planning, build-time `machine_execution/selected_operators.rs`,
    evidence `capture/callables/boundary_operators.rs` and result-domain
    overload dispatch read `OperatorDefinition` today. Preserve exact semantic
    identities or reject stale schema artifacts explicitly.

  Acceptance: wrapped 250 + 10 yields 260u64 in the selected true Match arm,
  the false arm yields 1 without invoking the operator, the named call returns
  260, and checked interpretation, independent Terminal replay and native
  execution agree. Cover token and named calls, generic and stateful bodies,
  once-only ordered operands, private helpers behind a public declaration,
  qualifiers and ordinary contract rejection. Missing body,
  bodyless-plus-satisfier, foreign primitive-family injection, duplicate owner
  shapes and forged compiler primitive identity reject. An unrelated import or
  visible conformance cannot change selection or cause a collision. Selected
  operation, law and call/loan identities survive generic substitution and
  replay. Unsupported paths fail closed, never falling back to builtin
  arithmetic or matching a compiler primitive by leaf name.

  Trait conformance selection, target-default or overridden float provider
  execution (**FLOAT-PROVIDERS**) and canonical compiler float-meaning
  evaluation keep their separate supply routes.

  Flag: `unique_provider_rebinding` in
  `build-time-evaluation/src/machine_execution/admission/selection_authority.rs`
  (called from `const_evaluation/const_generic_calls.rs`) scans every machine
  for exactly one `satisfies` provider of a boundary operator requirement and
  rebinds the use to that body without an authored selection;
  `generics/authored_const_call_operator_selected_provider` has no
  `build.omg`. That is the implicit unique-satisfier search the supply
  contract forbids, and a second provider in an unrelated package turns the
  accepted program into a rejection. The general mechanism is the explicit
  provider selection `machine_execution/selected_operators.rs` consumes.

- **MODULE-NAMESPACE-RESOLUTION.** Finish the
  [module/name contract](wiki/spec/language/modules.md) in
  `syntax-trees-to-symbol-resolved-trees/src/preparation/`
  (`module_normalization.rs`, `generic_data/`) and
  `build-time-evaluation/src/const_evaluation/const_initializers.rs`. The
  [source pipeline map](omega-rust/psi/pipeline/README.md#resolution-and-closed-instance-normalization)
  owns the landed probes. `module_normalization.rs` no longer fences any
  module-owned form: constant attachments, specialized foreign templates,
  open-template indices, trait defaults, operator homes and qualified case
  membership in domain facts pass normalization and resolution
  (`tests/module_namespace_residuals.rs` in that crate). Most of those tests
  assert only that resolution succeeds; none follows the selected declaration
  through typing, checking or Terminal.

  Remaining work:

  - Carry exact lexical/package selection for those forms through typed and
    checked trees and Terminal artifacts, with per-use exposure under
    specialization and owner-local imports, and add same-leaf, private and
    transitive-exposure controls for each.
  - Complete declaration evaluation, including unused initializers:
    specialized provider applications, target-dependent declarations, and
    floating/NaN identities, which need determined bits. Constrained constants
    still fence aggregate values, non-integer index arguments,
    carrier-polymorphic families, open applications, non-domain constraints
    and unprovable facts (`generic_data/const_evaluation/facts.rs`).
  - Extend concrete failure discharge in `const_initializers/invocations.rs`
    beyond ordinary scalar invocations. Casts, indexed reads, call-produced
    values and other origins the concrete probe cannot decide still widen
    conservatively; they need checked evidence
    (`typed-trees-to-checked-trees/src/facts/crash_entry_values.rs`, shared
    with **CRASH-CONTRACT**), not successful interpretation or provider-body
    inspection. `machine_execution/admission/closure_validation.rs` fences
    every authored `requires` except root parameter-domain premises.
  - `const_generic_expressions/value/match_dispatch.rs` needs nonconstant
    divisor integrality beyond singleton sign intervals, nonzero proofs beyond
    retained lattice gaps, and exact fractional-warning evidence for
    independent dispatch operands, through rational bounds and correlations,
    not arm enumeration or evaluation of skipped subjects. Preserve exact
    selected operators (**OPERATOR-MACHINE-SUPPLY**) and proof arguments.

  General array-value execution (dynamic selectors, borrowed projections and
  slices, array-producing cycles, boundary and indirect results) is a
  dependency owned by **STATE-LOCAL-VALUE-FRONTIER** and Omega's
  `abstract-operations-to-target-operations` aggregate-result lowering
  (`lowering/control_flow/aggregate_results.rs`), not a namespace fallback. A
  selected aggregate home cannot stand for an unevaluated value.

  Acceptance: `compiler --test module_machine_indices` (`nominal::`,
  `value_dispatch::`, source-free `machine_initializers::`),
  `terminal-psi-to-abstract-operations --test scalar_array_construction` and
  `omega-native-differential-test --test scalar_array_results` exercise exact
  source selection through independent artifacts and matching-host execution.
  Preserve the `qualified_declarations`, `qualified_constants`,
  `match_constant_indices`, `nominal_constant_bodies` and
  `module_array_constant_indices` customers. Under
  [file-local imports](wiki/spec/language/modules.md#import-scope-and-exposure),
  same-leaf competitors, private/transitive exposure, invalid unused
  initializers and unproved indexing reject. Keep
  `fail/modules/{runtime_aggregate_index,runtime_fixed_array_index}` until
  their materialization obligations are met.

  Flag: `signature_free_trait_candidates`
  (`selection/signature_free_requirements.rs`) claims to mirror
  `SymbolTable::select_namespace_candidate`, which ends at the unmoduled pool,
  but adds a last tier returning every same-leaf trait that passes only the
  resolution-stratum check. A bare `Trait::requirement` can pool a trait its
  file never imported, and an unrelated package's same-leaf trait turns the
  use into `TraitNotUnique`. Use the ordinary namespace selection.

- **RUNTIME-VALUE-GENERICS.** Implement the settled
  [runtime-capable versus const binder contract](wiki/spec/language/generics.md#value-binders-and-const-requirements)
  for APIs whose result or stored qualification depends on an input value.
  `TypeParameterKind::Value` runs through syntax, symbol-resolved and typed
  trees with its own template identity and review-evidence tags; machine
  declarations and trait requirement signatures parse `<Count: u32>`. A
  runtime argument becomes one trailing ordinary parameter of a single shared
  specialization (`typed-trees-to-checked-trees/src/monomorphization/`),
  `requires` contracts bind the realized subject, and `const` binders still
  reject runtime inputs. `compiler/tests/runtime_value_generics.rs` replays
  captured, forwarded, reassigned, guard-established and structural subjects
  from Terminal artifacts, with five native scenarios on macOS ARM64. None of
  this lets a type depend on a runtime subject:
  `monomorphization/body_rewriting/type_parameter_substitution.rs` rejects a
  runtime-bound binder in every type position, and data declarations parse no
  value binder (`GenericParameterSyntax::TypeAndConst` in
  `tokens-to-syntax-trees/src/parameters/parse_generic_parameters.rs`).

  Remaining work:

  - Range bounds in parameter, result and local qualifications now admit a
    runtime-bound subject: `-> u64[0..=Bound]` rebinds to the realized
    trailing parameter inside the shared specialization, the caller's
    inferred result indexes on the captured argument (`u64[0..=n]`), and the
    strict scope-atom arithmetic engine discharges declared `u64[0..=n]`
    bounds through Terminal publication and replay. Layout-determining uses
    (array extents, `const` positions) still reject;
    `fail/generics/value_generic_runtime_static_bound` is retired and
    `pass/generics/value_generic_runtime_result_bound` is the acceptance
    fixture. Remaining: domain-index qualifications, and bound subjects that
    exist only under a dominating guard rather than a declared type.
  - Parse and check value binders on data declarations:
    `data Index<Limit: u32> { value: u32 [0..Limit]; }` owes the range at
    construction, erases a proof-only index, and keeps an executable index as
    ordinary data, an argument or an existing descriptor field.
  - Module-owned forms, once **MODULE-NAMESPACE-RESOLUTION** supplies exact
    lexical selection. No source-spelling fallback, and no runtime value used
    as a static cache key.
  - Native legs for three scenarios. Each stops identically without a value
    binder, so repair the owning route, not this item: a receiver method whose
    realized subject owes `requires` stops at the `entry_claims` gate in
    `abstract-operations-to-target-operations/src/lowering/control_flow.rs`;
    a `let mut` primitive local beside a Console receiver stops with
    `SourceCustodyMismatch` in the selected-instruction
    `legalization/source/scalar_graph/terminator.rs`; a structural subject
    over a record local beside a provider receiver gets no checked Unit plan
    (**STATE-LOCAL-VALUE-FRONTIER**). Native receivers spell
    `console: Service<Console> in Bound`; a bare `Console` field stops in
    `image-emission/src/hosted_receiver.rs` (**ENTRY-CONTENT-ROOTS**). Only a
    macOS ARM64 host runs the native module; other hosts report a skip.

  Acceptance: `<Count: u32>` accepts static and runtime arguments when the
  caller establishes its obligations; `<const Count: u32>` still requires a
  static specialization. One dynamic machine body handles distinct runtime
  counts without per-value code generation. Parameter and result
  qualifications and a value-indexed scalar field preserve the same captured
  subject, including after reassignment of its source variable.
  Equality-guarded uses retain their proof; stale relationships, invalid
  bounds, duplicate or lost linear custody and unsupported static-only uses
  reject. Add no heap boxing or stack reservation of a range's maximum, and do
  not read integer finiteness as a specialization request. Check source
  diagnostics, representation and interpreter/native replay for each
  supported slice before widening it.

  Begin with scalar binders and fixed-representation uses, not dynamic stack
  layouts or automatic SIMD specialization. **FINITE-GENERIC-DISPATCH** owns
  finite families and dynamic interfaces; this item depends on neither it nor
  general reflection.

  Flag: range bounds were the fence and are now scope-admitted; every other
  type-position use of a runtime-bound binder still classifies as a
  static-only use. The value-indexed data-field case (`u32 [0..Limit]`) is
  the bullet-2 fence, and the suite's two "indexed scalar field" scenarios
  index a `[u8; 8]` receiver field with a literal, which involves no
  value-indexed type.

- **STRUCTURAL-GENERIC-MATCHING.** Implement
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference),
  and [canonical ranges](wiki/spec/language/generics.md#canonical-integer-range-matching)
  for bounded containers deriving static backing from a declared length type.
  Three Psi pieces exist, each keyed by the retained
  `IntegerRangeNormalization` and not by a rendered spelling: declared-range
  call inference
  (`typed-trees-to-checked-trees/src/monomorphization/range_arguments.rs`),
  build-time folding of closed endpoint calls
  (`build-time-evaluation/src/const_evaluation/range_endpoints.rs`), and
  data-level `where Binder == <range shell>` equations that recover an omitted
  const or type binder in either direction
  (`syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/equations.rs`).
  The three pieces now share one canonical-range admission: declared-range
  call inference runs before the const-call probe, evaluates each retained
  call endpoint through the shared endpoint evaluator, and writes the folded
  decimal literal back into canonical syntax, so `u64[0..=limit()]` and
  `u64[0..limit() + 1]` arguments on data fields and machine parameters
  select the same `u64[0..=256]` instance and recover its `Capacity`;
  conflicting explicit binders and calls that keep no canonical range still
  reject. That is not the general contract: `equations.rs` matches only a
  range shell or a single name (`Structure::{RangeShell, Name}`), equations
  exist on data templates only, and `T == i32` is decided only as a case
  `where` fact on a closed data instance.

  Remaining work:

  - Static type equality in Psi type-role resolution and checking: `where`
    disjunctions such as `T == u16 || T == u32` on machines and requirements,
    rejection of type/value mixtures, the unspecialized body checked under
    every admitted alternative, and a static branch's equality fact dropped
    at its join. No machine-level customer or control exists.
  - Structural matching by exact constructor and parameter position for fixed
    arrays and declared generic applications, and the same equations on
    machine applications combined with existing argument/result inference.
    An open endpoint binds as one whole expression (`0..=N` may bind
    `Limit + 1`); solving `N * 2 == 256` stays outside.
  - Endpoint folding for the forms `range_endpoints.rs` still leaves authored:
    nominal/policy-qualified parameters, trait-operator owners
    (owner-sensitive typed operations), applications that need inference or
    carry type/machine/evidence binders, and comparisons or negation in
    Boolean arguments. Use the shared admission plan and
    `typed-trees/src/typed_trees/type_system/closed_numeric.rs`. Do not add an
    arithmetic evaluator, infer layout from flow bounds, or use the i64
    compatibility interval in `validation` as type identity.
  - Runtime `Value` binders in data equations, which reject today as not
    statically recoverable. They depend on RUNTIME-VALUE-GENERICS; static
    matching proceeds first.
  - One normalizer for source equality, generic matching, canonical type
    identity, static evaluation/layout and artifact readers. Synthesized
    instances deduplicate by `ClosedArgumentIdentity`; their display name
    stays diagnostic-only.
  - `tests/omega` canaries for the constructed direction (`Bytes<256>`
    binding `Length`). Only the `omitted_data_binders.rs` unit tests cover it.

  Flag: endpoint folding grows by one admitted expression form per slice.
  `require_closed_boolean_argument`
  (`build-time-evaluation/src/machine_execution/admission/selection_authority.rs`)
  admits Boolean literals, `&&`/`||` and calls and rejects comparisons and
  negation; integer arguments pass only the context-free
  `closed_integer_value_in` query. The last four slices (domain-qualified
  parameters, Boolean arguments, static applications, template-bound rounds)
  each added one form with its own pass/fail canary pair, and each form in
  the third bullet would be another. The general mechanism is one evaluation
  of the whole endpoint expression as a constant position through the shared
  evaluator, with one selection-custody walk over every expression form, so
  endpoint coverage equals the evaluator's coverage.

  Acceptance: `generics/omitted_data_binder_range_equation` (TinyBytes binds
  omitted Capacity before layout; `u64[0..257]` and `u64[0..=256]` select one
  instance) and `generics/declared_range_endpoint_inference` (checked,
  Terminal and hosted native entry) stay as regressions. New customers:
  primitive type equality and its static branches check all admitted
  alternatives; an array- or application-structured equation binds an omitted
  binder on a data template and on a machine application. Repeat and explicit
  binder conflicts, absent/ambiguous endpoints, occurs cycles and
  type/value-kind mismatch reject in each new structure. Explicit larger
  compatible bounds stay distinct from exact type equations. Local flow
  narrowing cannot alter inferred layout, and arbitrary domain predicates do
  not collapse nominal identity. Preserve const staging, initialization,
  stack supply and artifact replay.

  The range fixture's remaining Terminal and native stops are ordinary storage
  work, not generic matching: borrowed-local mutation calls
  (`declared_range_inference_local_effects_retain_pending_terminal_boundaries`),
  parameter-origin moves into locals and bounded leaf stores on local records
  follow STATE-LOCAL-VALUE-FRONTIER; cyclic record establishment
  (`owned_scalar_graphs/record_locals.rs`) follows GENERAL-CYCLIC-EXECUTION.
  Do not add generic-specific storage plans.

- **FINITE-GENERIC-DISPATCH.** Implement the
  [finite specialization contract](wiki/spec/language/generics.md#finite-specialization-boundary)
  and [dynamic method families](wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
  for Squalr-style scanner widths and runtime-selected datatype providers.
  `TypedTrees::finite_signature_family`
  (`typed-trees/src/typed_trees/calls/finite_family.rs`) is the one roster
  authority for the local `dyn` surface and the selected-dispatch boundary. A
  local `dyn` call that spells one closed roster tuple (`erased.code<16>()`)
  selects that row, and
  `typed-trees-to-checked-trees/src/monomorphization/dynamic_families.rs`
  generates every roster tuple's provider specialization from the selected
  conformance. The boundary adapter surface now draws from that same
  authority: `selected-dispatch`'s
  `selected_boundary_family_specializations` reads each selected provider
  plan's checked adapter and queues the requirement's complete declared
  roster into `generate_dynamic_family_specializations` before checking, so
  a source program that selects a family provider settles every roster row
  even when no static call site demanded the tuple (`omega --check` accepts a
  `scan<16>` call under a `{16,32}` roster where it previously rejected
  "partial provider coverage"; off-roster tuples still reject). Missing
  roster bodies and runtime-bound or wrong-template records still reject the
  whole family. `family_tuple` is an exact join coordinate in Terminal rows,
  the codec, the verifier, checked-to-lowered evidence and the Omega custody,
  lowering, image-replay and optimization-unit identity rejoins. That does
  not establish runtime selection or a native customer: every call names its
  tuple statically, and the Omega joins have seen only test-constructed
  nonempty tuples.

  Remaining work:

  - Runtime-capable family calls in Psi checking
    (`execution/unit/dynamic_scalar_calls/`): a `Value` argument proven a
    roster member selects its row through generated dispatch among the closed
    bodies, and an unproven argument rejects. Depends on
    RUNTIME-VALUE-GENERICS, not generic JIT execution. Begin with one scalar
    binder, a common concrete result and dispatch around a region-sized
    operation.
  - One source-produced family through Omega native tables and image replay,
    with `tests/omega` pass, fail and run canaries. None exist.

  Acceptance: a source program dispatches widths 16/32/64 from a runtime value
  through one selected conformance with no handwritten suffix-method family,
  and executes natively. Source alternative order and duplicates normalize
  deterministically. Missing, wrong-width, mixed-provider or shape-substituted
  rows reject. Const-only calls still reject dynamic inputs without a checked
  bridge. Short explicit tuple sets preserve correlations and do not enumerate
  arbitrary ranges; target-ineligible bodies and invented fallbacks reject.
  Preserve parameter effects, index identity and once-only moves/cleanup
  through selection, forwarding, storage and replay. Escaping variable-shaped
  results require an explicit sum or eligible owned/borrowed descriptor, not
  implicit allocation. Retain compile/code-size evidence and scan-loop
  dispatch placement before claiming an improvement over explicit branches.
  No new reflection API or arbitrary generic virtual method is needed.

- **DOMAIN-ISSUER-ROUTES.** Implement the exact-machine half of the
  [requirement and exact-machine routes](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
  and the [private issuer catalogs](wiki/spec/resources/authority.md#private-issuer-routes)
  for public qualifications issued by owner-selected validators without an
  artificial trait. Requirement routes exist: `established by
  Trait::requirement` normalizes to
  `DomainEstablishmentRoute::{CheckedRequirement, BoundaryRequirement}` in
  `syntax-trees-to-symbol-resolved-trees/src/selection/domain_establishment.rs`,
  checking introduces `AuthorizedRouteEstablishment` evidence
  (`typed-trees-to-checked-trees/src/facts/qualification_evidence.rs`), and
  package review publishes the route rows. That does not establish machine
  targets or private catalogs: the route sum has no concrete-machine kind,
  resolution rejects every path that is not one exact `Trait::requirement`,
  and a public domain's route is recorded as an ordinary `PublicInterface`
  declaration selection with no issuer-authorization exception for a private
  target.

  Remaining work:

  - Psi route normalization: a closed target kind with exact declaration
    identity for free and attached concrete machines, resolved signature-free
    without consulting an expected result. Ambiguity across overloads or
    declaration kinds rejects.
  - Checked membership and Terminal qualification evidence: introduce the
    declared provenance only for the exact result subject of the named
    machine's own invocation, after carrier, predicate and custody checks
    that do not assume the qualification being introduced.
  - Private catalogs: a public domain may name an author-accessible private
    requirement or machine. Package source-selection capture
    (`packages/review/evidence/src/capture/api/domains/`) owns the limited
    private-route metadata exception; interface and proof evidence retain
    route kind, owner, declaration and dependencies without making the
    private type consumer-nameable.

  Module-owned targets depend on MODULE-NAMESPACE-RESOLUTION; do not weaken
  its rejection fences to accept same-spelled identities.

  Acceptance: exact free/attached issuer calls establish only their authorized
  result provenance after carrier/predicate/custody checks; public ordinary
  requirements still permit valid downstream conformers. Public wrappers
  forward issued values without becoming issuers. Reject ambiguous target
  kinds/overloads, self-justifying membership, forged result/route evidence,
  direct-call borrowing of admitted requirement authority, and outside private
  call/conformance access. Artifact/package replay retains exact private
  issuer identity and dependencies without exposing private types or hiding
  admissions. Resource capacity and classification-specific boundary-route
  restrictions remain unchanged.

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Finish receiver admission under
  [artifact production versus receiver admission](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission)
  and the settled
  [filesystem control/lifecycle policy](wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority).
  The production/admission split exists: the receiving permission policy is
  `Option` at every join from
  `packages/manager/src/operations/compile_project.rs` through native
  realization, absence makes no receiver-admission claim, and an unclaimed
  artifact cannot satisfy explicit admission replay. The canonical
  `FilesystemHost` cohort table and its permission/mechanism row emitters
  exist in
  `native-realization/src/native_realization/terminal_authority_policy/filesystem.rs`.
  Neither is established at a customer. No named customer has been observed
  emitting without receiving-policy input, only tests call the facet-cohort
  row emitters, and package review still publishes the broad `Filesystem`
  class.

  The explicit admission replay is now pinned at the operations join:
  `accepted_lock` shows an accepted project emits an artifact carrying no
  receiver-admission claim with no supplied policy — the replay rejects it —
  and binds the supplied policy's exact identity when given one, admitting
  only under that identity and rejecting under any other.

  Windows x86_64 observation at `e07e5c7a25` (2026-09-17, Windows host): the
  `cli_mvp` outer command, `--check`, `audit packages`, and the filtered
  `samples_compile` probe all stop inside the fresh review's checked
  compilation — `selected ProgramEntry establishment rejoins 0 Terminal
  attachment identities; expected one` — under both `windows_x86_64` and
  `macos_arm64` targets, producing no review findings; details in the
  [cli_mvp README](samples/cli/basics/cli_mvp/README.md). The probe fixture has
  test-owned entry bindings for `macos_arm64`/`linux_x86_64` only, and std's
  `windows_x86_64` target def authors no entry contract.

  Remaining work:

  - Rerun `cli_mvp`, console-exit-app, the Squalr native route and the
    Cathedral native smoke after ordinary package acceptance with no
    receiving-policy input, and report each one's next unrelated blocker
    without claiming its end-to-end completion.
    `samples/cli/basics/cli_mvp/README.md` and
    `tests/fixtures/packages/console-exit-app/README.md` still describe the
    removed gate. SAMPLE-CORPUS and CANARY-CORPUS own the harness copies that
    mirror accepted package rows into an explicit receiving policy.
  - Receiver rows for the exact replacement: every admitted leaf has one exact
    mechanism/contract row, unknowns and duplicates reject, exercised classes
    fit independently supplied service permissions, and explicit empties
    retain service reach and exact review identity. Do not fabricate a broad
    union to complete the table.
  - Retire the transitional broad `Filesystem` summary only after that
    replacement closes: the `FilesystemHostService` arm of
    `packages/review/evidence/src/capture/authority.rs::dangerous_authority_class`,
    its triage consumers in `packages/manager/src/review/audit/triage/`, and
    the "one transitional broad filesystem row" assertion in
    `packages/manager/tests/standard_library_package_resolution.rs`.
  - The release cohort (`close`, `find_close`, `close_handle`) is blocked. Its
    evidence-bound explicit-empty row needs a retained occurrence, and no
    authored source can produce one;
    `compiler/tests/terminal_authority/filesystem_release_witness.rs` pins
    that stop. Which customer earns the row is the constrained
    build-filesystem chain question in [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md).
    FILESYSTEM-RELEASE-CONTRACT owns the occurrence evidence. Generic close
    need not be supported to admit a separately proved constrained
    occurrence.

  Flag: the console-exit-app witness was narrowed when the gate was removed.
  `omega/tests/package_commands/console_exit_permission.rs` dropped its exit
  status assertion and now checks only that two gate diagnostics are absent
  from stderr, so a compile that fails at any later stage still passes it.
  Assert the emitted artifact or the named next blocker. See also the flag on
  FILESYSTEM-RELEASE-CONTRACT before extending the release-row settlement.

  Acceptance: after ordinary package acceptance the four customers above emit
  with no receiving-policy input. The same program rejects at explicit
  admission under a denying policy and admits under a sufficient accepted
  policy; an absent policy never yields an admission receipt. Forged
  classifications, invalid proofs and violated requested physical exclusions
  still reject. No PCC request becomes mandatory.

- **FILESYSTEM-RELEASE-CONTRACT.** Implement the
  [bounded occurrence-specific release proof](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof)
  for open/query/close through checked flow and native realization replay:
  the exact object/argument contract, handle/alias preservation through
  intervening calls, one applicable release and no later use. Authority
  classes alone are not preservation evidence. A build-time leg exists. The
  checked interpreter retains a constrained `open_path_handle` /
  `final_path_name_by_handle` / `close_handle` chain as
  `FilesystemSourceNativeHandleQueryChainReplayRecord`
  (`checked-interpreter/src/filesystem_replay/native_query_chains.rs`),
  `build-evaluation` rehydrates it, and `native-realization` derives one
  `FilesystemOrdinaryReleaseContract` per retained occurrence
  (`terminal_authority_policy/filesystem.rs`) and merges the bound rows in
  `native_product/realization.rs`. It establishes nothing at a customer: no
  authored build can produce the chain, and no proof exists for a program's
  own open/query/close occurrence.

  Remaining work:

  - A producer. Whether the build vocabulary may issue the constrained chain,
    or the row is earned only by program-side release, is the constrained
    build-filesystem chain question in [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md).
    Add no further consumers of the record until it is answered;
    `compiler/tests/terminal_authority/filesystem_release_witness.rs` pins
    the stop.
  - Program-side native acceptance through
    `tests/omega/pass/filesystem/windows_canonicalize_exit`. It stops at
    `structural field store: scalar field type`
    (`typed-trees-to-checked-trees/src/execution/unit/structural_scalar_store/`):
    `self.unit_result = self.fs.write_all(..)` stores a structural
    `UnitResult` field, `StructuralScalarFieldStore` covers scalar fields
    only, and Terminal production refuses the attached Unit closure in
    `checked-trees-to-lowered-psi/src/unit/attached_unit/call_closure.rs`.
    The fixture's transitive closure needs nested structural sum construction
    and extraction (`UnitResult::Error` carries `ErrorKind`), borrowed case
    observation and whole nominal receiver replacement, including
    match-produced field assignments. Further isolated prerequisites for this
    fixture are paused: resume with a plan covering that closure through
    shared state/value planning. Reuse recursive layouts and referent
    identity. A primitive field store, a fresh call-result home or a
    borrowed-storage snapshot cannot substitute for receiver replacement.
    This is an implementation scope pause, not a language-design blocker.
    `filesystem/native_close` is past the scalar call-result store; rerun it
    before assuming a stop.

  Flag: the landed join narrows the emitted program's release mechanism from
  evidence about a different execution. The contracts come from the compile's
  own build filesystem replay record, an interpreter trace of `build.omg`.
  `providers/settlements/source_imports.rs::classify_terminal_mechanism` then
  prefers the bound explicit-empty key for every demanded mechanism whose
  provider method is named `close`, `find_close` or `close_handle`, whenever
  any contract exists. No program call occurrence, handle flow or accepted
  `FilesystemHostService` binding is joined. The spec requires the constraint
  identity bound "to the derivation and exact occurrence". The general
  mechanism is a checked-flow derivation over the program's own occurrence,
  carried as Terminal evidence and rejoined per call site at realization.

  Acceptance: a constrained ordinary close has one evidence-bound empty row.
  Failed acquisition, escape, stale/substituted proof, invalidating calls,
  reused aliases and attached deferred deletion prevent narrowing. External
  pending-deletion completion alone leaves ordinary close empty. Keep this
  bounded proof separate from general owned-handle design.
  TWO-AXIS-TERMINAL-AUTHORITY-REVIEW owns the receiver rows and the broad
  `Filesystem` summary.

- **R5.** Finish exact inferred may-write summaries and relational candidates
  in `validation/src/machine_calls/calls/write_frames/`. The inference returns
  complete caller-visible paths or fails closed as opaque. It carries finite
  candidate origin sets through transparent call results, alias bindings and
  divergent exclusive-alias locals, solves transition cycles by permuted frame
  equations, and does not count a reference-free by-value state parameter as
  a write-capable cycle root
  (`type_capabilities.rs::parameter_may_carry_write`). Precision still depends
  on which source shape mentions a reference: a mention outside the admitted
  shapes sinks the whole frame.

  Remaining work:

  - Converge the categories that still sink: unresolved receivers,
    boundary-result origins, conditional helper-body case refinement, mutable
    case-state transfer, graph-level aggregate result routes, type-generic
    carrier substitution (trait-level applications such as `Device<Cell>`
    select no inspectable signature), computed reference arguments outside
    proven helper-result relations, and other unsupported expression shapes.
    Each has landed slices; rerun its `write_frame_*` tests in
    `typed-trees-to-checked-trees/src/tests/termination/` to find the residue.
    Prefer shared fixpoint and alias reasoning over syntax-shape exceptions.
  - A divergent exclusive-alias local keeps its candidate set only for writes
    and proven rebinds. A reborrow, call argument, transport into another
    binding, reference-typed interior write or unproven rebind still makes
    the summary opaque (`state_write_walk.rs`).
  - `type_may_carry_write` counts every `Named`, generic, array and slice type
    as write-capable; only state parameters get the reference-free test.
  - `permuted_cycle_frames.rs` hands statement calls a fresh summary memo
    while the prefix walk shares `complete_state_summaries`. Unifying them
    changes the solve-versus-walk route for a cyclic state calling a cached
    callee that calls back; settle that route before sharing the memo.

  Flag: precision is added one source shape at a time. `write_frames/` is 51
  files and about 17,000 lines, its checked-stage tests are 23 `write_frame_*`
  modules named after shapes, and most of its 19 commits since 2026-09-15
  admit one more mention kind and leave the rest failing closed. The general
  mechanism is the candidate-origin relation that now exists, applied
  uniformly to every reference-valued expression, binding, argument and
  result through one fixpoint, so a new mention kind needs no new rule.

  Acceptance: all supported finite source shapes converge without widening
  permissions, and unsupported recursion fails explicitly. Keep
  `omega --check source/psi/gates/parser/main.omg` completing its cycle solves,
  with `cli_mvp` and `nqueens` frames unchanged.

- **TPR6.** Finish subject-bearing progress-premise normalization through
  exported bodies, provider plans, recursive calls, and artifact evidence.
  Private ranking witnesses stay outside public identity. Acceptance: every
  used premise is reconstructed for the exact subject and no qualification or
  similarly shaped row mints one implicitly.

  Premise origins. Owners: `typed-trees-to-checked-trees`
  `checks/termination/progress/{origins.rs,lineage.rs,lineage/places.rs}`,
  `flow/value_origins.rs` and `flow/place/resolution.rs`. The backward trace
  already derives exact frozen-input projections for captured constructors,
  owned and nested checked helper results, constructed results and constructor
  operands that select one operand, write-clean mutable helper bindings,
  shared-reference leaves through their slot stores, and nested call-result
  arguments; partition replay follows reference and generic-application leaves
  with exact declared-field provenance. Do not rebuild those as new slices.
  Remaining work:

  - Complete owned value loads through references, additional
    reference-boundary loads, indexed or replaced carriers, and
    reference-bearing helper results with unresolved control-flow or binding
    transfers.
  - Mutable demanded paths, helper bodies that may write the demanded
    projection, write-tainted nested calls, generic or dispatched callees,
    ambiguous or dynamic projections, opaque or overlapping write frames,
    unresolved exclusive aliases and unresolved result routes keep no checked
    guarantee. Admit one only from exact provenance.
  - A mutated aggregate cannot use root correspondence as evidence for its
    previous field values; a may-write frame cannot identify a replacement
    value. Retain opaque prefixes where declared-field provenance is absent.

  Acceptance: those finite projected arrivals and checked helper
  correspondences derive the replacement input's exact premise, while unknown
  writes and reference aliases without exact provenance retain no checked
  guarantee.

  Nested value-call operands. Realize projected nested value-call operands
  guarded by
  `validation/src/machine_calls/calls/expression_scanning/result_realization.rs`
  through the checked/lowered value planning path. Borrow checking can
  transfer owned helper-result projections, but full checking still rejects
  the inner call's result as an unrealized operand. Whole owned record ingress
  and forwarding already execute from encoded Terminal evidence
  (`checked-trees-to-lowered-psi/tests/reference_result_source.rs`); that is
  Terminal acceptance, not native acceptance. The gate also admits nested call
  operands for free scalar callers, bare scalar stores, and member scalar
  stores on a borrowed parameter or receiver. Remaining work:

  - Complete result projections through the shared evaluator and
    result-binding lookup; extend the shared closure to general
    structural-result callees. Carry loans, qualifications, and projected
    claims through structural results without erasing their obligations.
  - Resume at the checked/Terminal representation seam, not another evaluator
    source-shape gate: realize projected owned reference leaves with residual
    carrier cleanup, then nested result operands with their recursive loan
    custody. Structural-element array construction is a further dependency.
  - `select(value: View) -> &mut i32 { value.body }` must move the selected
    permission and dispose the remainder, not create a reborrow whose parent
    dies at return. `EstablishReference` creates a child loan and cannot
    substitute for moving an existing leaf through call/edge/result moves.
  - Keep carrier location distinct from loan occurrence/parent, relocate
    runtime descriptors without copying referents, and reject nested
    reference host interfaces until their custody exists. Reuse
    `validation/src/machine_calls/reference_result_custody.rs` and the typed
    projection/result maps; preserve conservative lifetime unions when
    extending exact runtime origins beyond the whole-record route.

  Acceptance: `select(forward_outer(outer).inner)` and
  `select(forward_array(values)[0])` evaluate each call once, retain the inner
  result home through projection and the outer call, and preserve every
  selected source loan and linear claim. Remove the nested-call gate only when
  those result uses have real producers; a correct declared type or source
  origin alone does not realize a value. Keep the rejection controls for those
  two calls in
  `typed-trees-to-checked-trees/src/tests/borrow/carrier_results.rs` until the
  unchanged sources execute from encoded Terminal evidence, including
  projected moves and array ingress.

  Flag: `result_realization.rs` is now 1391 lines of per-shape admission.
  `unit_scalar_store_assignment_is_supported` (97b01d64f2) and
  `unit_member_scalar_store_assignment_is_supported` (9baac8ad7b, 687 added
  lines) each repeat inside validation the walk the checked Unit producer
  performs for one destination shape, and the producer still rejects what it
  cannot sequence. The general mechanism is the one named above: nested result
  operands become ordinary evaluation-graph computations
  (**STATE-LOCAL-VALUE-FRONTIER**) and the gate is deleted, not widened one
  destination shape at a time.

- **NOMINAL-FIELD-FLOW.** Complete declared-field domain evidence in Psi
  semantic facts, flow transfer, and contract consumption. Collection elements
  need explicit live coverage for their declared field predicates, transported
  through indexing, views, copies, and calls. Mutable calls must preserve or
  establish the appropriate returned field facts; an unchanged nominal type
  annotation cannot restore evidence retired by a write. Do not encode
  universal coverage as an unresolved index or assume arbitrary incoming
  storage is zero-initialized. Owner: `typed-trees-to-checked-trees`
  `checks/{contracts,ranges}/` and
  `flow/{call_phases,mutation,reference_places,transfers}`.

  Already in place; reuse it. A machine's normal return re-proves the declared
  default-domain rows of its readable `&mut` referents, `self`, and a returned
  reference place, establishment-gated domains excluded
  (`checks/contracts/exits/result_domains.rs`). Calls hand those rows back on
  each readable `&mut` actual and `&mut self` receiver
  (`flow/call_phases/referents.rs`). An unknown call frame retires only its
  declared-signature ceiling (`flow/mutation/ceiling.rs`). A `&mut` local
  bound from a checked reference result carries the callee's finite candidate
  origins (`flow/reference_places/result_candidates.rs`; sub-state routes,
  runtime indexes and unresolved callee locals stay conservative). A view
  element write retires only that element's facts.

  Customer probe: `omega --check --target linux_x86_64
  samples/cli/games/dungeon_crawler_cli/main.omg`. At b2ea74973c (macOS ARM64)
  it reported 55 diagnostics: 14 `RoomLookup::find_room_mut` return rows, 14
  `find_room` call rows, 14 `append_exit` + 7 `apply_room` + 2
  `roll_event`/`clear_event` call rows, and 4 index rows. Rerun it before
  relying on these counts.

  Remaining work, in resume order:

  1. The call-side nominal-input and `reference_domains` proofs
     (`checks/contracts/{nominal_inputs,reference_domains}.rs`,
     `local_reference_storage_at_call`) resolve an actual through one exact
     origin only. Proving a row on every candidate origin would close the 23
     `append_exit`/`apply_room`/`roll_event`/`clear_event` call rows.
  2. `RoomLookup::find_room{,_mut}` loop through a sub-state over a runtime
     index, which the candidate trace refuses (28 rows). Close it on the
     sample side (item 5) or with a candidate family for the element loop.
  3. `&mut self` receivers hand back only the ZII-seeded `MachineFieldDomain`
     rows: a callee's `self` entry assumption is ZII-gated, so its return
     cannot guarantee non-ZII rows, and a caller's non-ZII receiver facts
     survive a method call only through frame precision. Widening needs a
     contract decision, not a flow change; none is recorded in
     `OWNER_QUESTIONS.md` yet.
  4. `checks/ranges` retired a slice view's length after a call through one
     element (`clear_room(&mut rooms[0], ..)` then `rooms[1]`). The 55-row
     probe lists no such rows although the sample still has the pattern;
     confirm with a focused fixture and drop this item if it is closed.
  5. Sample side: `RoomLookup` copies an element at a runtime index and passes
     an uninitialized readable `&mut Room` out-parameter where write-only
     `&write Room` is the intended spelling, but validation rejects `&write`
     for constrained records. Unbounded `room_count` leaves the 4 index rows.

  Acceptance: the dungeon's `RoomLookup`, `MazeBuilder`, and game-state calls
  satisfy default field obligations, while corrupted elements and stale
  aliased fields reject at calls, transitions, and returns.

  Flag: `checks/ranges/state_arguments/statements.rs` replays each statement
  to collect transition-argument facts with its own copy of the seeding that
  `checks/ranges/statements.rs` performs in the checking pass. 6c5329be03,
  2267dd4da1 and 154de8c5a1 each repaired one seed the replay had missed
  (ensured call results, name aliases, member stores and subslice windows).
  One statement transfer shared by both passes removes that class of
  divergence; adding a mirror per missed seed does not.

- **CML4.** Complete `EdgeCleanupPlan` after outgoing materialization and
  transfer commitment, including structural sums, nested projections, cycles,
  calls, and partial initialization, under the
  [ownership contract](wiki/spec/terminal-psi/ownership.md). Cleanup follows
  reverse establishment and exact residual custody; trap/abort edges clean
  nothing. Psi owners: `typed-trees-to-checked-trees/src/execution/control_cleanup.rs`
  and `checked-trees-to-lowered-psi/src/unit/unit_cleanup/`. Omega owner:
  `abstract-operations-to-target-operations/src/lowering/`.

  Terminal production carries partial affine residuals on returns, call
  continuations and Jumps for the bounded forms listed in its
  [cleanup note](omega-rust/psi/compiler/terminal-production/README.md#partial-ownership-and-cleanup).
  Native lowering does not realize them: `lowering/function/mod.rs` rejects a
  function containing any Jump with `residual_affine_discards`
  (`UnsupportedPartialAffineContinuation`), and return cleanup admits only
  whole-root `DiscardRoot` actions (`plain_home_cleanup` in
  `lowering/control_flow/terminator.rs`).

  Remaining work:

  - Native: realize residual cleanup on return, Jump and conditional edges of
    the common control graph, including boundary call-result homes and
    projected copies; whole-result disposal does not cover a projected
    result's residuals. Computed scalar bindings, boundary-result projections,
    and cyclic control need their own storage and edge replay without
    delaying cleanup until final return.
  - Native: extend entry-origin scalar continuation storage to
    operation-result values and their exact defining identities; per-call
    argument shuffle snapshots do not preserve a result across earlier calls.
  - Psi: extend anonymous projected helper-result operands to multiple
    producers and other effects within one consumer's argument list and to
    non-Unit consumers, preserving each temporary's exact dying continuation.
  - Psi: extend the type-directed record/array complement to
    construction-local roots and mixed dying-root schedules, preserving
    maximal untouched subtrees, empty complements, and reverse establishment
    order without runtime liveness flags. Entry-parameter cleanup alone cannot
    dispose a temporary's remainder.

  Acceptance: no affine occurrence disappears, duplicates, or is cleaned after
  transfer.

  Flag: `lowering/unobserved_owned.rs::accepts` admits native cleanup of owned
  parameters only when the whole function passes an allowlist of operation
  kinds (integer constants, compares, exact add/subtract, calls, primitive
  locals) and every parameter type is a plain record or array. That is a
  whole-function recognizer, which the stage README says no longer supplies
  support. The general mechanism is a per-action cleanup realization keyed by
  the action's root home on the control-graph successor, which already
  carries `cleanup_actions`.

- **STATE-LOCAL-VALUE-FRONTIER.** Complete ordinary evaluation/value transport
  in Psi argument normalization, checked scalar computations, call/result plans
  and Terminal production. Remaining operands include dynamic/borrowed/projected
  storage, effectful state arguments/returns, wider structural returned calls and
  mixed structural/scalar signatures, including boundary consumers. Materialize
  each value and activate its staged loan at the authored evaluation point.
  Retain exact result owners for shared/mutable/write-only temporary borrows,
  multiple argument producers, self consumers and projected claims; **CML4**
  owns residual cleanup. Replace remaining flat guarded-call hoisting with the
  same evaluation graph, not another source-order family.

  Remaining work:

  - Extend the ordinary producer/consumer join exercised by
    `tests/omega/pass/effects/structural_callback_reach` to extracted
    projections, freshly established returned claims and claims from distinct
    owned inputs, including mixed scalar/structural operands. Returning a
    whole fixed array with several indexed claims is the regression baseline
    in its `projected.omg`; matching claim identities cannot substitute for
    checked content guarantees.
  - Complete caller-specific saved-argument and result facts: nonliteral
    contract arithmetic, borrowed collection lengths, dependent/public-trait
    results and subslice bounds need exact entry observations and
    substitutions. Mutable formals/storage must distinguish their incoming
    value from subsequent writes. **CRASH-CONTRACT** shares the capture path;
    case-qualified, indexed, generic, reference-valued and floating entry
    predicates need exact identities/totality. Unchanged entry observations
    may justify published routes; later writes and current body facts may
    not. Ranked-loop crash guards require independently checked all-path
    invariants, never first-pass facts ignoring backedges.
  - Finish [exact anonymous division/landing](wiki/language_guide/chapter_5_expressions_evaluation.md#exact-anonymous-division-and-landing)
    across generic/evidence-adapted and boundary calls,
    aggregate/parameter/constant destinations, numeric policies, floats and
    proof consumers. Preserve result carrier/policy custody, exact rational
    intermediates and warning origins through suppression/reporting;
    coordinate selected result types with **MATCH-SELECTIVE-LOWERING**.
    Acceptance: `7 / 2 * 2` is 7 with a warning, `7 / 2` cannot land in an
    integer, and typed integer division truncates. `(4097 / 4096) * 4096` is
    4097 with a warning; its `4097u32` form is 4096.
  - Complete [typed quotient/remainder](wiki/language_guide/chapter_5_expressions_evaluation.md#typed-integer-quotient-and-remainder)
    in resolution, selected constant execution and symbolic proof replay.
    `syntax-trees-to-symbol-resolved-trees/src/preparation/generic_data/const_evaluation/`
    and `build-time-evaluation/src/machine_execution/admission/selection_authority.rs`
    must retain authored selection across helper calls instead of folding
    builtin meaning. Controls: `fail/generics/authored_const_operator_requires_selection`,
    `fail/generics/authored_const_call_operator_requires_selection` and
    `pass/generics/authored_const_call_operator_selected_provider`.
    **OPERATOR-MACHINE-SUPPLY** owns executable supply. Nonconstant proof
    `Int` terms need independent evidence beyond source entailment in
    `validation/src/proof_contracts/contract_entailment/arithmetic_judgment.rs`.
    Positive/negative dividend/divisor combinations satisfy the paired integer
    law; zero divisors reject and exact anonymous division remains unchanged.

  Overall acceptance: selected arguments execute left-to-right once; skipped
  calls never execute; serialized/replayed guards and saved values agree with
  native execution. Explicit renamed state transfers preserve contracts,
  selected fields, ownership and cleanup without requiring physical copies;
  implicit cross-state use rejects. Longer dispatches, mixed state signatures,
  borrowed loop formals and mutable scalar carriers use ordinary joins.
  Stale writes, mismatched result origins and wrong normal-exit guarantees
  reject; callee-local IDs and rereads cannot replace captured values.

  Flag: the checked-to-lowered seam is still organized as source-shape
  producer families. `checked_trees/flow/terminal/*_plans.rs` declares 14
  `Checked*MachinePlan` structs, most named for one source shape (for example
  `CheckedPayloadlessGuardedCallReturnMachinePlan`), and
  `checked-trees-to-lowered-psi/src/unit/attached_unit/composed_control/routing.rs`
  picks among five producers by state count and first terminator kind
  (`closed_sum`, `prefixed_control` for four or more states that start with a
  Jump, `nested_control` for four or more states, the shared `state_graph`
  closure, and a default route for fewer states). No board item names their
  removal. The
  general mechanism is one checked evaluation/control graph lowered by one
  producer, with each family deleted as the graph covers it.

  Flag: 4cd64b462b met the `Buffer<0>` acceptance by rebinding an authored
  binary operator use to the evaluated program's only `satisfies` machine
  inside Psi admission (`rebind_selected_provider_operators`), before any
  provider plan exists; two providers reject. Provider selection is Omega's,
  and the evaluation README's stated route is the deferred continuation that
  Omega completes with the actual plan. Uniqueness in one program is not that
  selection; **TARGET-SEMANTIC-APPLICATIONS** owns exact selected execution.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Finish ordinary generic
  `drop<T>` and runtime cleanup invocation after exact owner-attached hook
  selection, under [nominal cleanup](wiki/spec/terminal-psi/ownership.md#nominal-cleanup)
  and [explicit early disposal](wiki/language_guide/chapter_17_drops_and_cleanup.md#explicit-early-disposal).
  Erased fields remain semantically present but never produce runtime cleanup:
  `data_graph_requires_nominal_drop_with_substitutions`
  (`validation/src/value_custody/cleanup.rs`) now skips erased record and case
  members, so an erased-only owner stays an ordinary affine record instead of
  poisoning every Unit lane. Source selection of a reserved `T::drop` already
  rejects (same file). `omega::language::core::drop` is declared
  (`source/library/core/drop.omg`) and a corpus call site checks
  (`tests/omega/pass/drops/core_drop_explicit_consume`; register it in
  `compiler/tests/canary_suite.rs` once that file is unclaimed — its owner
  holds a live claim). The generic `drop<T>` signature is still outside the
  terminal-Psi source slice, so its parameter's hook edge cannot yet be
  lowered or invoked at runtime; explicit early disposal discharges by
  transfer into the consuming machine. **CML4**
  owns residual cleanup order; this item owns the hook target, the generic
  consuming machine and their invocation.

  Acceptance: every path invokes the exact selected hook once or proves the
  value transferred/consumed.

  Flag: a nonempty `drop` body is admitted only as a source-ordered list of
  zero-argument calls to mutually distinct attached helpers whose own bodies
  are empty (`is_exact_executable_drop_body` in
  `validation/src/program_validation/statements.rs`); everything else rejects
  as "outside the executable cleanup slice". The executable slice therefore
  invokes hooks that cannot do work. The general mechanism is to check and
  lower the hook body as an ordinary Unit machine and invoke it as the edge's
  cleanup action, then delete the recognizer.

- **TR3-TR8.** Finish whole-call-graph worst-case stack derivation, exact
  `StackPlan`, nonmoving `StackLease`, suspension/cancellation preservation,
  transactional arguments, park/resume lowering, and the suspension-safe loan
  subset. Bind an authoritative possibly-suspending crossing roster so coordinated
  deletion of both a Terminal site and plan cannot erase a required crossing.
  Follow the [task runtime contract](wiki/spec/build/task_runtime.md) and the
  [call/outcome contract](wiki/spec/terminal-psi/calls_and_outcomes.md) through
  `task-plans`, `provider-planning/src/task_plans/`, provider admission, and a
  real selected runtime.

  Static carriers exist: `task_plans/stack_graphs.rs` derives a sealed WCSU
  `StackPlan` over exact checked-body call edges and rejects a
  possibly-suspending call that has no canonical crossing, and the `task-plans`
  ledger issues a plan-bound `StackLease` and returns the moved arguments and
  the lease whole on every start rejection. Per the
  [task-plans note](omega-rust/omega/representations/task-plans/README.md),
  routed source `Task<T>` establishment, argument marshalling, stack
  provisioning, cancellation conformance and real runtime execution remain.

  Acceptance: stack/control custody is never compiler-owned or lost across a
  suspension edge, and missing crossing demand rejects. Exercise concurrent
  start/park/resume/finish, rejection returning every moved argument and lease,
  cross-instance settlement rejection, and fresh storage eras on reuse. Static
  plans and lifecycle ledger tests alone do not establish executable activation
  or argument conservation.

  Flag: `stack_graphs.rs` adds a child frame only when a call target resolves
  to a checked-body machine state. Requirement slots, machine parameters,
  dynamic descriptor calls and non-checked supply `continue` with no edge, no
  `AdmittedSameStack` contribution (only tests construct one) and no
  rejection, so a frame they place on this stack adds zero bytes to a bound
  published as exact whole-call-graph WCSU. A worst-case bound needs an
  admitted contribution or a rejection for every call it cannot resolve.

- **ATOMIC-MEMORY-MODEL.** Complete the formal atomic/fence axioms and checked
  target refinement under the
  [concurrency contract](wiki/spec/language/concurrency.md). Owning areas are
  proof semantics, normalized atomic events, and target realization. Acceptance:
  reads-from/modification/global-order constraints and fence synchronization
  are independently checked; swap/fetch retain the instruction-observed prior;
  single-attempt failure retains its distinct outcome and custody. Add real
  concurrent-activation controls once TR3-TR8 supplies that execution route.
  Serial instruction tests and bounded exploration do not discharge these
  proof obligations or authorize a weaker acquire without its protocol proof.

- **BLOCKEXEC.** Implement a package-level blocking executor with bounded
  queues, moved custody, linear completion claims, suspension, and provider
  selection. Hung-worker recovery requiring termination must use process
  isolation.

- **QUOTIENT-THEOREM-LIFT.** Admit explicit representative operation,
  congruence theorem, and optional precondition transport for quotient-owned
  operations under [lifting operations](wiki/spec/proofs/quotients.md#lifting-operations).
  No structural or effectful observer crosses the quotient unless its law is
  explicit and checked. Custody-bearing quotients remain fenced.

  Validation already composes the canonical correspondence row on the ordinary
  path, dispatches one bridged form from the composed certificate's evidence,
  rejects a representative or selected theorem whose transitive closure
  reaches an admitted or boundary machine, and receives sealed
  `Quotient::define`/`Quotient::lift` requests on the compiler route. Every
  request still rejects; the `tests/omega/fail/proofs/quotient_*` canaries pin
  the rule rejections through `omega --check`.

  Remaining work:

  - Admit a request. That needs a production caller of
    `install_non_executable_quotient_correspondences`
    (`checked-trees-to-lowered-psi/src/proofs/quotient_correspondence.rs`;
    only its tests call it) and quotient handling in
    `typed-trees-to-checked-trees`, which exits every value path whose call
    carries `quotient_operation`.
  - A canonical wire payload for congruence-only `lift<F, Congruence>`; its
    language-semantics, codec, verifier and review rows belong to
    **PROOF-CONTRACT-MIGRATION**.
  - General adapted lift with result computation, beyond the omission,
    permutation, repetition and closed literals the direct rung covers.
  - The conversion-independent closure over helper types and statements.
  - Generic or private applications, preconditioned `define`, result aliases
    and forwarded result flow, none of which has a canonical row.

  Acceptance: an explicit direct `define` and a transport-backed `lift` admit
  through `omega --check` with their
  [published correspondence](wiki/spec/proofs/quotients.md#published-quotient-correspondence)
  rows rederived on decode, while implicit lifts, missing, surplus or reversed
  roles, admitted or boundary theorem closures, and custody-bearing quotients
  still reject.

- **EVALUATED-FOREIGN-BINDINGS.** Carry the typed compile-time locator values
  for PE, versioned ELF, and Darwin/Mach-O
  (`provider-planning/src/evaluated_via_bindings/`; the string-backed import
  bootstrap is retired) with their normalized locator, evaluated plan, target
  applicability, and producer custody through provider selection and native
  emission. Raw foreign bytes are data, never Omega symbol names or ambient
  lookup authority.

  Extend [normalized-import evidence](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement)
  from fixed-width scalar calls to a
  source-rooted flat-record argument, then ranked control and port-bearing
  artifacts. Acceptance: independent native replay preserves the exact
  survivor/physical-child bijection and rejects missing, duplicate, substituted,
  or role-swapped children. External realization claims require independently
  admitted concrete authority.

- **FLOAT-PROVIDERS.** Complete runtime Boolean/machine operations for exact
  `FloatMeaning`, kernel discharge, and remaining artifact-aware proof sources
  under the [FloatMeaning contract](wiki/spec/terminal-psi/mathematical_values.md#floatmeaning).
  Keep IEEE runtime comparison distinct from mathematical meaning equality;
  NaN payloads erase only in the meaning projection and signed zeros remain
  distinct there. Installed scalar-result providers already execute in the
  Terminal interpreter, and the verifier's provider-result conformance already
  admits scalar provider rows.

  Remaining work:

  - Kernel discharge. Each `FloatSemantics::*` row in
    `numerics/src/float_semantics_catalog.rs` now carries a
    `FloatSemanticKernel` beside its identity (landed 2026-09-19):
    `FloatSemanticOperation::kernel_discharge` evaluates a signature-shaped
    operand list through the bound `FloatSemantics` definition and pairs the
    result with the row's `FloatSemanticContractIdentity`, selected through
    the complete signature by `from_source_identity` — never the leaf
    spelling. Meaning results compare by payload-erased meaning equality;
    `Bool` results keep the IEEE predicates (`NaN` unordered, `+0` == `-0`).
    Remaining: no checked obligation cites the binding yet — the
    semantic-application proof value that would carry the discharged tuple
    through Terminal is still not threaded checked-trees -> Terminal ->
    codec -> verifier.
  - The non-call operation result and call result
    [source classes](wiki/spec/terminal-psi/mathematical_values.md#source-identity).
    Terminal carries `DirectOperationResult`/`DirectCallResult` with codec
    tags 6/8 and independent verifier rejoins, and no checked or lowered
    producer exists. This part is owner-blocked on the named decision
    `float-meaning-use-site-source-identity` in `OWNER_QUESTIONS.md`; do not
    synthesize a source that cannot be tied to an exact operation or call.

  Acceptance: every `FloatSemantics` obligation a `Float::*` slot contract
  cites is discharged through a checked kernel binding, not catalog identity
  alone, and `fail/float/float_semantics_lookalike_grants_no_primitive` still
  rejects. The two open source classes gain a producer the verifier rejoins or
  are retired, as the named decision directs.

  DESIGN-BLOCKED (2026-09-18, measured) for the kernel-discharge bullet as well,
  not only the already-named source-identity bullet. `float_operations.omg`
  carries 212 `FloatSemantics::` mentions, 146 of them inside `ensures` clauses,
  and ZERO have a projection on both sides of the `==`: every one is shaped
  `Float::meaning32(result) == FloatSemantics::add(BINARY32, ...)`.
  `walk_expression`
  (`validation/src/proof_contracts/float_projection_invocations.rs`) forms a
  `ValidatedFloatMeaningEqualityProposition` only when BOTH operands are
  projection invocations, and its `Call` arm rejects only a drifted projection,
  so all 146 contracts are silently accepted and carry no obligation --
  `arithmetic/runtime_float_operations_exit` is green for exactly that reason.
  Wiring is impossible without new proof vocabulary: `CheckedProofOnlyValueType`
  has the single variant `FloatMeaning`, `bind_float_meaning_projection_facts`
  resolves equality operands only from projection invocations and errors
  otherwise, Terminal's `proof/values.rs` carries only projection-sourced
  values, and `FloatSemantic` appears zero times in `terminal-verifier/src` and
  `terminal-codec/src`. A semantic-application proof-value class must be
  threaded checked-trees -> Terminal -> codec -> verifier. The attachment point
  is already named in
  `validation/src/proof_contracts/float_projection_bindings.rs`:
  `semantic_operations::exact_toolchain_float_semantic_contract` is "the hook a
  provider binding will consume once discharge attaches to a row".

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Restore ordinary native
  descriptor invocation and forwarding, beginning with a non-entry helper that
  receives one borrowed two-word descriptor, forwards it once, invokes a
  requirement, and uses its result across computations and branches. The
  dependency is an ordinary indirect-call operand and its ABI, clobber, effect,
  and reach contract, not another whole-body recognizer. Do not resume
  target-only descriptor composition: two such milestones left this native
  customer unsupported.

  `abstract-operations-to-target-operations` now lowers that source customer's
  descriptor parameter to `DynamicParameter{Scalar,Unit}Call` with two pointer
  words per descriptor, and independently replays roster binding, requirement
  slot, dispatch plan, table offset, obligations and result home
  (`lowering/unit/parameter_dynamic.rs`, `tests/dynamic_parameters.rs`). No
  later stage names those operations.

  Remaining work, by owner:

  - `target-operations-to-selected-instructions`: legalized/selected call
    representations and target-to-selected replay for the indirect call.
  - ISA crates: selected encoding/decoding and post-allocation emission.
  - `native-artifact` and image crates: table and relocation custody.
  - Delete the superseded Unit/scalar parameter recognizers with that closure.
    Candidates: `machine_code::ForwardedDynamicParameterCallRecord`, which no
    non-test producer fills, and image-emission's
    `object_artifact/replay/dynamic/forwarded_{descriptor,parameter}.rs`, which
    admit at most one forwarded call per function. Do not substitute raw
    function pointers or unproved devirtualization.

  Acceptance: a source-rooted closed-conformance native differential fixture
  publishes and independently replays Linux x86-64/AArch64, executes on matching
  hosts, selects distinct table implementations at runtime, and preserves the
  original referent and surrounding calculations. Reject substituted instance,
  table, slot, ABI, access, and code-span custody. Receiver-backed executable
  entry provisioning remains a separate dependency for the existing
  receiver-entry canary.

- **TARGET-SEMANTIC-APPLICATIONS.** Complete typed target observations,
  hermetic const evaluation, and [selected realization coverage](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children). Finish
  ordinary selected provider-body execution and earlier generic-application
  evaluation through the [semantic evaluation owner](omega-rust/psi/semantics/build-time-evaluation/README.md#semantic-admission-boundary),
  preserving exact provider selection and independently checked result custody.
  Complete the portable target capsule and its application identity; a checked
  source fold alone does not establish cross-artifact closure. Finish
  artifact-qualified symbolic substitution for separately compiled generics;
  recheck the reachable specialization's actual capability reach, proof
  obligations, target facts, and selected realization after closing every
  argument. Boundary-operator empty telescopes remain distinct from
  boundary-trait calls with no telescope. Acceptance: cross-artifact canaries
  preserve actual reach and transitive open obligations, reject stale or
  substituted applications, and grant no coverage to unresolved arguments.
  Physical-child binding belongs to `TRANSLATION-VALIDATION` in
  `TASKS_OPTIMIZER.md`.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Finish explicit public
  `boundary requirement Owner::name(...);` declarations under
  [machine supply](wiki/spec/language/machines.md#supply),
  [provider selection](wiki/spec/build/provider_selection.md) and
  [requirement identity](wiki/spec/terminal-psi/boundary_calls.md#call-and-requirement-identity):
  external satisfiers, provider selection, and installed execution with era
  replay. Owners: `build/selected-dispatch` (`boundary_dispatch.rs`,
  `selected_dispatch/requirement_adapter.rs`), `build/provider-planning`, and
  Psi lowering plus Terminal for the retained identity.

  One shape executes in both engines: a public, nongeneric, receiver-free
  requirement whose selected plan is a checked `satisfies` adapter or a
  compiler intrinsic, called in value position
  (`pass/providers/checked_boundary_requirement_{dispatch,terminal}_exit`, with
  the unselected and private fences under `fail/providers/`). Settlement
  rewrites the typed call to the adapter entry before lowering, so the
  Terminal module holds an ordinary in-module call and no requirement identity.

  Statement position is now the same route: `_ = Owner::name(...);` settles to
  the selected checked adapter exactly like the value-position call. The
  rewrite journals the statement-table call, retargets its retained flow
  occurrence by caller state, statement index, and call ordinal, keeps
  `discards_result` intact, and restores the authored requirement statement on
  source replay
  (`pass/providers/checked_boundary_requirement_statement_call_exit`, with the
  unselected fence `fail/providers/boundary_requirement_statement_call_unselected`).

  Remaining work:

  - Receiver-bearing and parameterized requirements.
    `is_directly_callable_top_level_requirement` admits no `self`, type or
    lifetime parameter, and the rewrite rejects `forward_receiver` and family
    rows. `core/task.omg` (`Task::finish<T>(self)`, `request_cancel`) and
    `core/interrupt.omg` (`InterruptMaskGuard::restore`,
    `InterruptAcknowledgement::complete`) have no library or canary satisfier
    and reach execution only as installation-bound reach rows
    (**BOUNDED-INSTALLATION-REACH-ROWS**).
    [Interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime)
    names this item for that selection and lineage integration.
  - External satisfiers. `reject_unselected_direct_requirement_calls` fails
    closed for every external binding except a compiler intrinsic. Execute
    `satisfies Owner::name via <binding>` leaves through the same call route
    with the binding **EVALUATED-FOREIGN-BINDINGS** evaluates.
  - Terminal identity and era replay. Conformance rows in
    `terminal_module/boundary/conformances.rs` are trait-keyed, and the
    `..._terminal_exit` harness test asserts that the requirement is absent
    from the artifact. Retain operation, static telescope, signature,
    contract, visibility and the selected conformance row as a canonical kind
    distinct from trait requirements, then bind installed execution to the
    selected provider execution and token era.
  - Respell tokenless boundary operators onto this route:
    `core/float_operations.omg` holds 124 `boundary operator` rows against 2
    `boundary requirement` rows. `compiler_intrinsics/requirement_view.rs`,
    plan stamping, call-row retirement, D29 coverage and review policy rows
    already key on either spelling. **OPERATOR-MACHINE-SUPPLY** keeps the
    inventory and the `operator` introducer's removal.
  - Remove the undifferentiated `MachineSupplyMode::Boundary`, which
    `syntax-trees-to-symbol-resolved-trees/src/lowering/machine.rs` still
    assigns, once those source migrations close.

  Acceptance: value- and statement-position calls, a receiver-bearing
  requirement and an external satisfier each execute in the interpreter and
  natively under one selected plan, and the serialized Terminal artifact
  replays requirement, provider and adapter identity without source.
  Unselected, private, ambiguous, foreign-package and stale-era calls reject,
  as do a substituted adapter and a same-spelled declaration in another
  package. No declaration lowers to `MachineSupplyMode::Boundary`.

  Flag: the route is a post-check rewrite admitted one authored shape at a
  time, and its Terminal test pins the requirement's absence. Adding
  statement position, receivers and external bindings as further rewrite
  cases repeats the recognizer pattern in
  [compositional lowering](AGENTS.md#compositional-lowering). The general
  mechanism is one settled call-target substitution keyed on the
  `FlowCallFact` occurrence, with a Terminal-visible requirement/provider row.

- **BUILD-ADMISSION-CHECKPOINT.** Execute an admitted build machine against one
  coherent frontend/source/authority snapshot and append generated source in a
  later resolution stratum; authored source may not resolve forward into output
  generated by its own build. The checkpoint, the generated-source stratum,
  serialized replay and compiler-owned publication of the retained native
  product exist: a replay record binds root package identity, authored
  declaration role, target profile and admitted build execution profile, and
  drift in any of them, or in source, target or artifact, stops publication
  (`compiler/tests/build_config_granted/checkpoints_and_snapshots.rs`,
  `compiler/tests/build_snapshot_outputs.rs`,
  `omega/tests/package_commands/generated.rs`). Neither restricted-build
  acceptance nor dependency-purpose binding exists.

  Remaining work:

  - Wire [restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance)
    through compiler-derived review, package-manager install/update/audit,
    normalized lock comparison, recoverable review/resume, and build
    execution. `packages/manager/README.md` still lists this as required, and
    no manager or lock type carries a build-host request. Surface direct and
    transitive build-host requests separately from product authority before
    running them; retain accepted request meaning in `omega.lock`, with actual
    invocation grants separate. Benign snapshot/staging builds need no
    restricted-action approval. Do not implement an arbitrary recursive build
    API or a new host protocol as part of this join.
  - Bind dependency purpose on checkpoints and generated handoffs, joining the
    [scoped build work](#scoped-build-execution). This waits on
    **BUILD-DEPENDENCY-PURPOSES** supplying a per-occurrence purpose value:
    `DependencyPurpose` exists only on dependency edges, and the only
    compilation occurrence is the root product-purpose one, so there is no
    purpose fact to bind and no second occurrence to witness drift against.

  Acceptance: initial install and an update adding a restricted helper request
  both stop before its host effect and show package, dependency path,
  operation, logical scope, bounds, and profile/target. Resume with acceptance
  plus a real grant executes, then reviews generated source; rejection or a
  missing grant performs no restricted action. Audit-only inspection never
  adds grants. Unchanged requests need no recurring approval; widened or
  transitive requests do. Resolution-only updates preserve approval meaning,
  source changes remain audit visible, and dependency-owned lock decisions
  cannot authorize host access. Exercise interrupted review/publication and
  stale candidates without retaining secrets or machine-specific grant paths
  in the lock. With purposes bound, replay after serialization reproduces the
  product and purpose drift rejects beside the existing activation controls.

  Preserve the authored/generated boundary: no own-build final-component query
  or hidden post-compilation callback. **BUILD-SNAPSHOT-OUTPUTS** owns the
  committed output set and required-output settlement at publication.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Finish the compiler/library migration
  to explicit ordinary std dependency edges under the
  [toolchain/library contract](wiki/spec/packages/toolchain.md). Std may be
  replaced, split, or absent; only core and compiler-injected vocabulary
  remain toolchain-owned. Migrate package-aware fixtures, keep freestanding
  UEFI roots dependency-free, and retain standalone compatibility only until
  fixtures acquire package roots. Replace std/alloc `Toolchain` classification
  when compiler consumers have exact source-byte catalog entries or explicit
  semantic bindings; the current narrow roles and standalone limits are listed
  [beside package compilation](omega-rust/omega/build/package-compilation/semantic_bindings.md).

  Complete composed-Unit plans for trait-default, float, wire, arithmetic-helper,
  guarded-call, and looping-cast canaries and the target-correct non-Linux
  Console catalog entry. Structural writeback shares the blocker recorded in
  `WRITE-ONLY-BORROW`. Feed consumer-scoped Console, Filesystem, and UEFI
  bindings through normal package-aware compilation.

  Acceptance: removing a dependency rejects its imports/provider selections;
  name, alias, path, or same-spelled declarations cannot restore it, and stale
  or substituted semantic bindings reject without relying on accepted-lock
  replay. `packages/manager/tests/repository_build_declarations.rs` checks each
  packaged canary group's std edge against its imports; extend it as groups
  migrate. `packages/manager/tests/standard_library_package_resolution.rs`
  already holds the Console case: removing the std path dependency rejects the
  consumer with a diagnostic naming the missing `omega_language_std` edge, and
  another alias or path spelling does not restore the import.

  The Console provider selection has no diagnostic of its own, because source
  assembly stops at the import and a selection without the import is not a
  checkable shape. Letting source assembly continue past the missing edge so
  build evaluation could name the selection regresses
  `standard_library_alias_has_no_undeclared_bundled_fallback`: the imported
  type is left undefined in the assembled tree and the rejection moves to a
  later stage that drops the missing-edge diagnostic. A separate selection
  diagnostic is an architectural follow-up, not a change to this shape.

- **COMPONENT-SUBSTRATE.** Implement independently selected component closure
  under the [component publication contract](wiki/spec/build/component_publication.md),
  keeping deployment/update policy in runtime packages or Cathedral.
  Componentization must bind exact imports, exports, services, mappings, stack
  demand, leases, and installed provider closure. Until that carrier is
  complete, an `Independent` selection with no verified component fails at one
  explicit fence (`provider-planning` `selection_provenance.rs`, pinned by
  `package_compilation_inputs/authority_and_build_files.rs::independent_provider_selection_reaches_the_componentization_fence`).

  The [verified-description](wiki/spec/build/component_publication.md#verified-component-descriptions)
  carrier and consumer exist in `backend/artifacts/component-description`:
  `describe_component_facts`, `verify_component` and
  `VerifiedComponent::realizes_selected_plan`. `provider-planning` closes each
  `Independent` plan against exactly one verified component,
  `build-evaluation/src/provider_settlement/independent_components.rs`
  re-verifies attached descriptions, and
  `compiler::published_independent_component_description` publishes one from a
  dependency's own checked compilation
  (`compiler/tests/package_compilation_inputs/independent_components.rs`). This
  is a checked-stage join driven by a test that attaches the description. The
  producer has no non-test caller, and no stage after settlement reads the
  composition mode, so a settled `Independent` edge carries no symbolic import,
  separate artifact or installation obligation into the product.

  Remaining work:

  - Drive the producer from the compiler. A root's `Independent` selections
    are known only inside the sealed check
    (`assembled-syntax-to-checked-compilation/src/checking/execution_settlement.rs`
    hands `build_config.provider_selections` to `settle_checked_providers`, and
    `ComputedBuildConfig` never leaves that crate), while speculative
    attachment is refused by the unmatched-component rule. Add a discovery
    stop returning the evaluated `Independent` selections with the dependency
    package each names, then call the producer from `packages/manager`'s
    `compile_dependency_closure`
    (`src/review/candidate/compilation/package_pass.rs`), which already
    compiles each package as its own root.
  - Admit a component that selects its own checked adapter.
    `derive_component_inventory` reads called requirements only from Terminal
    `BoundaryCall`, fused lowering erases that call, and `verify_component`
    then rejects the retained plan as a smuggled requirement. Today a
    publishable component must seal its requirement with a surviving external
    binding and carry the adapter beside it.
  - Add build vocabulary for accepted assumption digests. Settlement passes an
    empty `accepted_assumptions` set, so every mechanism-bearing component
    rejects.
  - Supply the native facts. Compiler-published descriptions leave
    `stack_demand` and `realization_identity` absent because a Psi capsule has
    no native realization; the native producer `describe_component` sits in
    `component-candidate`, behind the runtime quarantine in
    `tests/architecture/layering.rs`. `ObligationKind` and `CustodyKind` carry
    no mapping or lease row yet.
  - Realize the settled edge, or keep rejecting production: symbolic
    imports/exports, entry/leave and resource demands, and
    installation/replacement obligations, never a silent Fused product.
  - Expose the same consumer to independent admission/replacement and to
    **TOPOLOGY-PLAN-VERIFICATION**. First inventory actual producer/replay
    coverage; implement missing complete facts in their Psi, component, or
    provider owners, not a second topology census.
    **BOUNDED-INSTALLATION-REACH-ROWS** waits on this carrier for its
    component-contract fence.

  Acceptance: an ordinary two-package `omega` build whose root selects
  `Independent` publishes and consumes the dependency's description with no
  test-side attachment. An independent source-free consumer checks
  subject/profile/schema, all entries and outgoing authority, custody and
  inseparable assumptions; corrupt, omitted, early-frontier, forged-complete,
  substituted, stale and unrelated descriptions reject and never fall back to
  Fused. Include startup, callbacks, timers, cleanup and retained providers.
  Reading descriptions grants no callable authority; installation-dependent
  facts remain obligations and require fresh per-occurrence resource/profile
  admission.

  `verify_component` now runs `terminal-verifier` on the decoded module under
  the request's `admission_profile` (a new `ComponentVerificationRequest`
  field, bound into `profile_identity`) and derives the inventory from the
  verified module; the authority scan enumerates every `OperationKind` so a
  future authority-bearing kind stops compiling instead of silently reading
  as empty coverage. An empty admission profile still admits only
  kernel-dischargeable modules.

- **FFIVAL.** Author and run the Windows `user32` boundary-coherence canary: an
  Omega window procedure registered with `RegisterClassEx`, entered through
  `CreateWindowEx`/`WM_NCCREATE` and `DispatchMessage`, and released through
  `DestroyWindow` and `UnregisterClass`, with no raw function pointer or
  Win32-only compiler escape. The canary does not exist yet:
  `pass/host/runtime_gui_*` and `samples/gui/*` dispatch through predefined
  window classes, and `fail/capabilities/blocking_beneath_no_block_root` is
  the only fixture that names this item.

  Blocked on **CALLBACK-PRIVATE-MATERIALIZATION** (no callback thunk reaches
  machine code) and **REGISTERED-CALLBACK-LIFETIME** (no authored registration
  route). Acceptance: on a Windows host the canary builds from its `build.omg`
  through the generic [private-callback](wiki/spec/build/private_callbacks.md)
  route, receives a real foreign callback, recovers per-window state without
  an ambient closure, and unregisters before its code lease is released. Other
  hosts report the leg unavailable.

- **WIRE-RUNTIME-AND-INSTALLATION.** Complete the
  [admitted executable installation contract](wiki/spec/build/executable_installation.md):
  reusable artifact validation, consumed placement authority, W^X and
  instruction-fetch coherence, physical invocation, and the uninstall and
  replacement joins. Keep arbitrary runtime bytes-to-code, JIT, and raw
  executable addresses unsupported. The linear state model exists in
  `omega-rust/omega/backend/runtime/executable-installation/` — admit,
  materialize, freeze, validate, install, retire, quarantine, each failed
  transition returning its inputs — and the final image carries the placed
  executable and initialized-data inventories that
  `image-emission/src/installed_artifact.rs` binds to an `InstalledCode`
  occurrence, rejecting unclassified gaps, a truncated compiler prefix and a
  resolver-claimed uninstalled thunk address. That is custody bookkeeping, not
  installation: `install_validated` and `retire_installed` have no caller
  outside tests and `test_support.rs`, no Omega source names any of these
  states, and loader and provider lifetimes still bind through installation
  records rather than a live installed occurrence.

  Remaining work:

  - The contracted provider operation the placement lifecycle names. Nothing
    performs the write-to-execute transition, the target cache and ordering
    work, or instruction-fetch visibility; `InstallationReceipt` only records
    what a provider would have reported.
  - Physical invocation, and the entry references it hands out. `InstalledCode`
    exposes identity, geometry and `selected_entry_target` reporting; the
    [control-flow integrity](wiki/spec/build/executable_installation.md#control-flow-integrity)
    gate that turns a selection into a sealed requirement-compatible entry
    reference is unbuilt, so nothing calls installed code.
  - The uninstall and replacement joins over that custody, per
    [visibility and retirement](wiki/spec/build/executable_installation.md#visibility-and-retirement):
    visibility before entry, quiescence before retirement, and live-site
    patching through admitted fragments. `uninstall.rs` now owns the
    drain-or-quarantine join — `uninstall_installed` retires a complete drain
    and routes any incomplete drain to `replacement_quarantine.rs` when the
    provider supplies trapping evidence, returning every input when neither
    ending establishes. Still open: the replacement join (live-site patching
    through admitted fragments) has no route.
  - A route from Omega source: no `.omg` file names an admitted artifact, a
    placement or installed code, so no canary reaches any of this.

  Acceptance: an authored program admits a reusable artifact, materializes and
  freezes a placement, validates the exact final bytes and footprint, installs
  through one provider operation and calls into the installed code on a
  matching host; retirement then proves quiescence, execute removal and
  restored write authority before returning the placement, and an incomplete
  drain quarantines the mapping instead. Substituted bytes, a placement spent
  twice, a transplanted validation and an `Unsupported` W^X provider reject.

  **COMPONENT-SUBSTRATE** owns the verified component closure above this; this
  item owns generic executable custody.

  Flag: installation accepts caller assertions where retirement demands facts.
  `retire_installed` requires `authority.required_facts` to be a subset of the
  receipt's `established_facts` — provider-canonical `RetirementFactDigest`
  values the authority names in advance — while `install_validated` admits the
  receipt on two booleans the caller sets, `visibility_complete` and `wx`, with
  no required-facts set at all. The spec has installation validate W^X, cache
  order and instruction-fetch visibility through one contracted provider
  operation, so those claims are currently producer assertions the model
  records rather than checks. The general mechanism is already on the
  retirement side: an authority naming the required provider-canonical facts
  and a receipt that must establish them.

## Omega-written compiler (after Rust completion)

Finish the Rust [completion contract](wiki/drafts/rust_compiler_completion.md)
before starting the product-language migration. Rust can remain a differential
implementation afterward, but neither Rust agreement nor Rust-specific machinery
is bootstrap authority. Bootstrap construction stays on `TASKS_BOOTSTRAP.md`.

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Establish the production compiler as two
  sibling Omega packages: target-neutral phases under `source/psi/` and the
  Terminal-Psi-consuming product under `source/omega/`, with hosted entrypoints
  at `source/omega/{build.omg,main.omg}`. The maintained Rust compiler is the
  differential implementation, not source for this task. Work backward from
  complete Omega behavior in small, live vertical slices; do not create a
  bootstrap-private dialect, file allowlist, or parallel source-to-native path.
  What exists is a lexer and a partial parser: about 3,600 lines across
  `source/psi/{lex,parse,source,syntax,tokens}/`, the parser gate at
  `source/psi/gates/parser/`, and a 70-line `source/omega/main.omg` that drives
  lexing and parsing over console input. `omega --check
  source/omega/main.omg` reaches the end of the Psi checked stage and std
  calling-policy admission before stopping on the first item below.

  Remaining work:

  - Target-profile recognition. The product check stops on `root slot
    alpha_bootstrap::ProgramEntry belongs to unknown target profile
    alpha_bootstrap`, raised by
    `omega-rust/omega/build/build-evaluation/src/admission/selection.rs`
    because `target::TargetProfile` enumerates seven profiles and no Alpha row.
    Recognize the Alpha profile and slot through the ordinary target-package
    route under settled
    [target recognition and availability](wiki/spec/build/configuration.md#target-recognition-and-implementation-availability);
    keep the binding at `source/omega/build.omg:11` and add no parallel
    bootstrap selection mechanism. An inactive recognized Alpha row must not
    demand Alpha realization, unknown profile and slot names must still reject,
    and a selected unimplemented Alpha operation must report not implemented
    without claiming a checked result or emitting an artifact. Rust Alpha
    emission is not part of this task.
  - Exact narrowing with no positive evidence. Checking accepts
    `self.lexer.append_source_byte(value as u8)` where `value` is an unbounded
    `i32` field, contrary to
    [counts and addresses](wiki/spec/language/counts_and_addresses.md); only
    the Unit builder refuses it, pinned by
    `typed-trees-to-checked-trees/src/tests/flow/terminal_unit/call_argument_casts.rs`
    (`exact_cast_argument_without_positive_evidence_stays_omitted`). The repair
    belongs in checking.
  - Std name shadowing. A user declaration spelled like a std one (`ByteRead`,
    `Lexer`) makes std's own machines fail checking: `read_line`'s `store`
    overflow and `MacosArm64::extent_shape`'s range. The three red
    `compiler/tests/calling_policy_plans/macos_entry.rs` tests are the same
    family and the cheapest reproduction.
  - The parser gate's next Unit omission: `statement sequence: call: call
    operation`, state `source_full`, statement 1 of
    `source/psi/gates/parser/harness.omg` — an attached call through a nested
    receiver whose first argument is a copy-enum case literal beside two ranged
    scalars. `Main::main` has no such call, so the product's own next stop
    after target recognition is unmeasured.
  - Native production for `pass/structs/runtime_copy_sum_array_receiver_exit`
    and `pass/calls/runtime_nested_receiver_cast_argument_exit`, both on
    `canary_suite.rs`'s checked-only roster. Their entry statements stop at
    `structural field store: scalar field type`, `local data: structural call
    binding`, and `state graph: terminator: conditional successors: parameter
    transfer`. **STATE-LOCAL-VALUE-FRONTIER** owns those Unit slices.
  - Gate-check cost. A full parser-gate check takes about 6,500 s wall against
    about 1,500 s before `d0371277e3..fc633a98ae`, with sampling inside
    `typed-trees-to-checked-trees` `flow::builder::build_flow_facts` calling
    `validation/src/machine_calls/calls/write_frames/`. Attribute it before
    taking the next slice; iteration at that cost is the practical blocker.
  - Everything after the parser. Resolution, typing, checking, proof and
    Terminal production in `source/psi/`, and the whole `source/omega/`
    consumer, are unwritten.

  Acceptance: the exact Omega source closure implements the complete language
  and production pipeline, passes the shared product suite, and publishes a
  deterministic manifest of every transitive compiler/build input. Bootstrap
  construction of that closure belongs in `TASKS_BOOTSTRAP.md`.

  **BORROWED-STORAGE-RESTORATION** and **CRASH-CONTRACT** own stops split out
  of this item.

  Flag: a checking gap is being closed one stage below where it opens. The
  narrowing cast above is accepted in checking and fenced in the Unit builder,
  and the pinned test asserts the builder's omission rather than a rejection,
  so a program that should not check does check and stops later with a lowering
  message. The gate advances the same way: one statement shape per slice, each
  landing a canary for the arrangement the harness reaches next, the next
  omission again a single statement, while the general mechanism is the
  ordinary statement sequencing **STATE-LOCAL-VALUE-FRONTIER** names.

## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
- Windows AArch64 has no `NativeTarget` constructor, so that ABI combination
  stays unwitnessable until the target vocabulary grows one.
