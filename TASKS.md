# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion plan](wiki/drafts/rust_compiler_completion.md) defines the full
acceptance bar; this board identifies the remaining work, not a passing baseline.

| Start here | Purpose |
| --- | --- |
| [Immediate product closure](#immediate-product-closure) | Unchanged sample/canary programs and the dependencies preventing native execution. |
| [P1–P5](#p1---authority-roots-and-entry) | Entry/storage, materialization, portable evidence, ABI, and Cathedral customers. |
| [Parallel language work](#parallel-language-and-compiler-lanes) | Remaining accepted language surface; independent work can proceed when a product strategy is paused. |
| [Rust release closure](#rust-compiler-release-closure) | Full-gate evidence and matching-host runs, separate from feature implementation. |
| [Optimizer board](TASKS_OPTIMIZER.md) / [bootstrap board](TASKS_BOOTSTRAP.md) | Separate execution owners, not duplicated here. |

Keep each task's missing behavior, owner, real dependencies, and acceptance
condition. Delete completed work; Git holds checkpoint results and superseded
diagnoses. Link the specification for semantics rather than restating it.
Retain a dated/revision-bound failure only when it determines where to resume;
rerun that customer before assuming the old diagnosis still applies. New
evidence supersedes, never accrues: a landing or rerun replaces the dated
paragraph it makes stale, so an open item states its current frontier once.
A confirmation that changes nothing is not board evidence: record it on the
claim's notes (`claims.py note`) and in the session verdict instead — a row
carries at most one current verification line, and a resolved or covered row
is not stamped again. A rerun that reproduces the recorded verdict at a newer
revision is such a confirmation: it lands no restamp. Serial witness
paragraphs ("re-verified a second/third time", "fourth witness") are noise —
fold a materially new observation into the row's single verification line and
drop the rest. Reopen a closed row only when the new evidence changes
its frontier.
Where a stamp does land, it stays inside its own row: append it after that
row's last sentence, never splice it mid-sentence or mid-paragraph into the
row's existing prose or a neighbor's, and never leave it as an orphan
paragraph detached from its bullet. A mined stub that resolves as a pure
re-mine of a settled row is folded into that row's sibling list rather than
carrying its own stamp.
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

Prioritize unchanged customer programs reaching native execution over additional
evidence carriers without an exercising program. The
[Rust compiler completion plan](wiki/drafts/rust_compiler_completion.md) defines
the complete product bar; focused successes below do not establish that baseline.

- **REMOVE-BRACKETED-RANGE-ANNOTATIONS.** (new-scope) Delete the revoked
  scalar range suffix, such as `u64 [1..=8]`, and its parallel authored
  type-constraint machinery. [Domains](wiki/spec/language/domains.md#declaration-and-membership)
  govern explicit qualification; contracts and guards supply implicit bound
  facts. Preserve ordinary interval analysis, not a compatibility mode or a new
  compiler-provided range domain.

  Owners: Psi's `tokens-to-syntax-trees/src/type_syntax/parse_type.rs`,
  syntax/resolved/typed Range variants, range-shell generic matching, endpoint
  folding, and `validation/src/proof_contracts/arithmetic_domains/`.
  Migrate libraries, compiler source, tests and samples, including Epsilon's
  `bootstrap/5_omega/{parser,representations}.epsilon` and
  `tests/bootstrap/omega-parser`. Repin the changed D closure and run its parser
  gate; this changes Omega syntax, not the bootstrap languages.

  Acceptance: Squalr's alignment machine returns plain `u64` with
  `ensures result >= 1 && result <= 8` or an explicit domain; its plain local
  retains those facts through calls and joins for division/remainder. An
  implementation returning zero and stale facts after writes reject. Publish
  the application migration before updating its gitlink, then run geometry.
  Reject the suffix on integer/float locals, parameters, results and fields.
  Preserve default-domain/case `where`, domain qualification, slices, range
  membership, fixed arrays and relevance/multiplicity modifiers. Migrate generic
  inference to explicit domains/type arguments without extracting capacities
  from flow facts or inventing implicit variance; retain useful negative
  arithmetic/ownership controls from obsolete syntax tests.

- **SQUALR-HEADLESS.** Drive the independently versioned
  [Squalr application](samples/apps/README.md) through nested package builds and
  native geometry, supplied-byte scanning, and repeat filtering. Its board owns
  the essentially 1:1 Rust port; this item owns compiler integration and unchanged
  application acceptance.

  Reconcile publication first: the parent pins
  `ef6682f75f48f9a750bc8f7594cc1f04e78ddf60`, which is not an ancestor of fetched
  application `origin/main` at `52bcf254c983a9ae9bf6e0c3661cafd656ca056b`
  (2026-09-21). Preserve the geometry and scalar scan/RLE/dispatch work already
  in the pin; do not re-port it or discard either lineage. Squalr uses only
  `main`; publish cumulative work there without force-pushing, then pin a
  tested commit on that line.

  Run `python samples/apps/squalr/tools/verify.py native --timeout 600 --omega <binary>`
  through ordinary package review on matching hosts. Connect
  **VEC-NATIVE-GROWTH** to partitioned scan results and repeated filtering;
  **BUMP-ALLOCATOR-CANARY** and **PLAN-LAID-VIEWS** supply allocation and placed
  access. Follow the application's actual next missing operation rather than
  waiting for every related task to close.

  Acceptance: geometry prints `Squalr geometry: PASS`; scans and repeat filters
  match Rust's exact addresses/ranges, including overlap, tails and empty input.
  Retain the 17-package/37-edge layout and application verification harness;
  record exact compiler/application pins and host. No Rust FFI scanner,
  fixed-capacity substitute, pull-only workaround, or application-specific
  compiler behavior. Source presence, package checking and Terminal publication
  are not native acceptance. Ordinary artifact production has
  [conditional loading premises](wiki/spec/build/component_publication.md#products-and-authority),
  not a fabricated runtime installation grant or mandatory Rust supervisor.

- **VEC-NATIVE-GROWTH.** (split-of:BUMP-ALLOCATOR-CANARY) Implement ordinary
  library `Vec<T>` for Squalr's growable scan results. Owners:
  `source/library/core/vec.omg`, `source/library/alloc/`, and the compiler
  operations exercised by the source. The current empty data/machine surface
  has no runtime storage or construction implementation. Its compiler-owned
  buffer/Arena comments are not the [allocation contract](wiki/spec/resources/allocation.md).

  Acceptance: native `Vec<u32>` takes explicit backing, appends through two
  growths, reads its preserved prefix and new elements, then cleans up and
  returns backing under the allocator contract. Cover empty cleanup, allocation
  failure preserving contents, invalidated loans and duplicate cleanup.
  Growth transfers exact element custody and returns or retains old backing
  explicitly; variable retained storage uses indirection, not an infinitely
  recursive inline record or compiler vector special case.

  **BUMP-ALLOCATOR-CANARY** owns allocation/returned extents;
  **PLAN-LAID-VIEWS** owns establishment/access/retirement;
  **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS** owns content-preserving
  transfer; **BORROWED-STORAGE-RESTORATION** owns move-out/replace when needed.
  Keep the source program as outer acceptance through checking, interpretation
  and native emission, then use that same implementation in Squalr.

- **MACOS-APPLICATION-PUBLICATION.** Finish the
  [macOS publication contract](wiki/spec/build/macos_application.md) for
  `window_app`, `window_demo` and `windowed_calculator`. The
  `compilation-report/src/package.rs` assembler and checked bundle accessors
  already exist; do not build another packager.

  Start with [window_app's ordinary command/review flow](samples/gui/window_app/README.md).
  Its four intrinsic service fields need exact selected providers for
  `Console`, `Clock`, `Input` and `Gui`; its authored build currently supplies
  no explicit provider selections. The historical macOS GUI wrapper is not
  nominal satisfaction of the sample's traits merely because method names
  match. Preserve signatures, receiver storage, rendering and effects under
  [provider selection](wiki/spec/build/provider_selection.md).

  Compiler dependencies: `typed-trees-to-checked-trees/src/execution/unit/providers.rs`
  admits one provider field, while this entry needs four; lowered
  `attached_unit/providers.rs` excludes scalar-result provider candidates
  needed by window/input/clock operations. Retain one occurrence-owned concrete
  provider receiver across calls, including nested services: a Fused receipt
  establishes service authority, not receiver storage, and
  `ProviderAttachment` is not an ordinary structural argument.
  **ENTRY-CONTENT-ROOTS**, **TR3-TR8** and **STATE-LOCAL-VALUE-FRONTIER** own these
  general repairs. No global provider state or weakened occurrence matching.

  Acceptance: all three authored apps publish a validated `.app` and execute
  on macOS ARM64 with observed window/render and completion behavior. Test-owned
  package acceptance and brief process survival do not prove ordinary review,
  rendering or Finder launch. Keep identifier requiredness, deterministic
  bytes, cross-invocation publication, tamper/partial-output rejection and flat
  outputs covered by `compilation-report` and
  `compiler --test build_target_activation -E 'test(activation_identifiers_and_publication)'`.
  Run `mbx nextest run -p compiler --test native_filesystem_canaries --no-fail-fast --no-tests fail -E 'test(gui_and_sample_apps::sample_window)'`;
  unavailable macOS hardware leaves runtime acceptance open.
  **PCC-PRODUCT-PUBLICATION** owns the native sidecar's
  `Incomplete(UnsupportedEvidence)`; retain bundle placement coverage when it
  closes. Resources and `image_viewer` bundle-relative lookup are outside v1;
  do not silently change working directories.

- **SAMPLE-CORPUS.** Close maintained `samples/cli|gui|uefi` through the
  [Rust product gates](wiki/drafts/rust_compiler_completion.md#release-matrix):
  checked semantics, native products for authored targets, and documented
  exit/output on matching hosts. `compiler/tests/samples_compile.rs`, sample
  commands and the actual failing stage own integration; application submodules
  remain **SQUALR-HEADLESS** and language fixtures **CANARY-CORPUS**.

  Preserve each algorithm, storage and observable behavior during legitimate
  surface migration. Migrate remaining bare or authored-`Bound` service fields
  under **ENTRY-CONTENT-ROOTS**, without relaxing provider/occurrence checks.
  The harness already follows checked published executable paths and no longer
  invents receiving permissions; its test-owned package acceptance still does
  not complete ordinary CLI review.

  | Customer | Remaining acceptance and routing |
  | --- | --- |
  | `cli_mvp`, `euclid_gcd`, `generic_counters`, `number_guess` | Complete ordinary review and native CLI execution on Windows x86-64 and both Linux hosts; the recorded macOS ARM64 routes passed. Preserve documented exact output, EOF/Enter and prompt timing where applicable, empty stderr, and exits 0/12/16/70 respectively. Review each checkout/target rather than reusing another checkout's local-source lock. |
  | `print_squares` | Publish and execute the unchanged byte-storage/cyclic program, nine computed rows ending in `081`, exit 0. **NOMINAL-FIELD-FLOW**, **CRASH-CONTRACT** and **GENERAL-CYCLIC-EXECUTION** own facts, envelopes and plans. Wrapping multiplication legalization/selection already exists: rerun the customer, not the obsolete missing-multiply diagnosis. |
  | `recursive_sum`, `framed_payload` | Exits 70/60, preserving saved reads, untouched siblings and shared payload loans. **STATE-LOCAL-VALUE-FRONTIER** and **GENERAL-CYCLIC-EXECUTION** own indexed storage, views and transfers; do not invent scalar-array field IDs or ranking for unranked cycles. |
  | `dutch_flag` | Native Color-array construction, reads, swaps and case dispatch, Console interaction and exit 70. Its field is already intrinsic `Service<Console>`. **STATE-LOCAL-VALUE-FRONTIER** and **WRITE-ONLY-BORROW** own composition; replacing nominal enum storage with integers is not acceptance. |
  | `calendar` | Publish unchanged trapping conversions through **ARITHMETIC-POLICY-REALIZATION**, finish ordinary package review, and verify all five grid rows plus the live 20-byte header in capacity-21 buffers. A `contains: 30` banner match is not a rendered calendar. `compiler --test byte_index_carriers` is the arithmetic/bounds/write control, not application acceptance. |
  | `windowed_calculator` | Reproduce and locate the checked-compilation stall: at `50559da3ab9` on Linux x86-64 it exceeded 25 minutes of CPU without a diagnostic while the other 146 maintained mains checked. The cause was not established; do not label it a provider-selection or proof-search defect without evidence. Native GUI acceptance remains **MACOS-APPLICATION-PUBLICATION**. |
  | Other text/index/match samples | Close `binary_search_viz`, `maze_flood`, `prime_sieve`, `multiplication_table`, `dice_histogram`, `dungeon_render`, `mandelbrot{,_zoom}`, `wire_protocol` and `dungeon_crawler_cli` through **NOMINAL-FIELD-FLOW**, **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**, **MATCH-SELECTIVE-LOWERING** or **CML4**, not one task per source permutation. Encoding facts use ordinary domains, not recognized function names. |
  | `cli/proofs/math_proofs` | Ordinary multiset data/slice extraction, selected laws and checked proof terms; keep the false twin rejecting. Equal lengths are not equal contents and `core/seq.omg` is not a Bag implementation. |
  | Bounded Console input | Finish selected-dispatch/provider/call transport under [bounded input](wiki/spec/resources/bounded_input.md): exact prefix/destination, once-only effects, cleanup, blocking/crash contracts, zero-capacity non-consumption, LF/EOF/Full, untouched tails, alias rejection, caller continuation, failure and invalid-result controls. Keep prefix guards until count-to-extent evidence exists. |

  Use `mbx nextest run -p compiler --test samples_compile --no-fail-fast --no-tests fail -E 'test(=samples_with_documented_exit_run_correctly)'`.
  PowerShell: `$env:OMEGA_SAMPLE_RUNTIME_FILTER = 'print_squares'`; macOS:
  prefix with `OMEGA_SAMPLE_RUNTIME_FILTER=print_squares`. This filter affects
  only that oracle; unset it for complete coverage, and empty selection fails.
  `cli_mvp_preserves_both_lines_with_eof_and_enter` pins exact input/output;
  `all_samples_reach_checked_trees` covers the checked cohort.

  Scope pause: resume `print_squares` only with a plan from its full source
  closure to native execution, not another isolated helper milestone.
  Independent operation work remains actionable; use the
  [Terminal production map](omega-rust/psi/compiler/terminal-production/README.md)
  and **TRANSLATION-VALIDATION** in `TASKS_OPTIMIZER.md`.
  Acceptance requires every maintained sample to check and applicable runtime
  oracles to pass on the required hosted matrix. Record unavailable hosts;
  scoped reruns are not a complete baseline.

- **CANARY-PACKAGE-MODE-SIGNAL.** (new-scope) Replace
  `fixture_declares_ordinary_std` in `compiler/tests/canary_suite.rs`: it
  selects package mode by two source substrings, then hard-wires the repository's
  std path instead of resolving the authored dependency.

  A package-mode fixture must not need an unused std edge. Remove the
  `quotient_define_managed_compile` exception in
  `repository_build_declarations.rs` once the harness accepts its actual
  dependencies. Acceptance: package mode without std works; an invalid authored
  dependency path rejects rather than resolving to the harness's hard-wired std.
  Add a wrong-parent-path negative control based on the previously misresolved
  theorem-equality fixture; its positive fixture now has the corrected path.

- **CANARY-CORPUS.** Bring `tests/omega/{pass,fail,run}` and
  `compiler/tests/canary_suite/` to their promised checked/native stages.
  Use the [focused selectors](AGENTS.md#running-one-test); full closure is
  `mbx nextest run -p compiler --test canary_suite --no-fail-fast` on one
  revision with filters unset. Keep detailed logs outside the board; do not
  migrate fixtures during a measured run.

  Follow `CheckedUnitEffectPlans::omissions`,
  `InvalidUnitMachinePlan::omission` and `LocalConstructionTrace` to the
  actual missing operation/facts, not just its phase label. The diagnostic route
  is covered by `checked-trees-to-lowered-psi/tests/unit_plan_omissions.rs`;
  another diagnostic-only change does not close corpus behavior.

  Current integration targets:
  - `text/runtime_stdin_command_branch_exit`: owned `Command` result-to-field
    assignment in `parsed_input`; `execution/unit/control/statement_sequence.rs`
    still limits that call-result path to primitives. **STATE-LOCAL-VALUE-FRONTIER**
    owns structural result storage and independent lowering. Keep the reader's
    algorithm and run `runtime_stdin_command_branch_exit_canary_runs`.
  - `host/runtime_console_bounded_line_exit`: compose its full result with
    mutable field subslices, including `read_line(&mut self.line[0..0])`.
    `calls/structural_arguments.rs` and `calls/byte_subslice.rs` restrict this
    view/call transport; **STATE-LOCAL-VALUE-FRONTIER** owns the general repair.
    Retain zero-capacity non-consumption, count and untouched-tail checks;
    fixed-array discard-result execution is not this acceptance.
  - `text/runtime_stdin_line_buffering_exit`: both `echo_line` calls already
    carry `block`; finish borrowed intrinsic-service parameters through
    `write_prefix` and state edges, which still use bare `&mut Console`.
    **ENTRY-CONTENT-ROOTS** owns exact receipt/borrow transport through
    declaration checks, Unit forwarding, lowered states and selected
    `service_custody/parameters.rs`. Do not replace the two-read/helper flow
    with a single-hop owned-Service recognizer.

  Route other failures to **STATE-LOCAL-VALUE-FRONTIER**,
  **MATCH-SELECTIVE-LOWERING**, **GENERAL-CYCLIC-EXECUTION**,
  **NOMINAL-FIELD-FLOW**, **BORROW-PROOF-CONVERGENCE**,
  **OPERATOR-MACHINE-SUPPLY**, or **ARITHMETIC-POLICY-REALIZATION** as appropriate.
  Audit fixtures against the spec before weakening checks:
  `proof_inductive_climbing_sum` and its unbounded-accumulator negative still
  owe [exact intermediate arithmetic](wiki/spec/language/numeric_values.md)
  bounds even when a theorem uses `embed`.

  Acceptance: complete corpus reaches declared stages, negatives fail for their
  intended reasons, runtime oracles pass on matching hosts, and roster/coverage
  guards remain intact. Do not demote valid accepted-language programs to
  checked-only to turn the suite green; assigning every failure is not closure.

- **TERMINATION-RANKING-CHECKS.** Finish exact rank-range transport under the
  [termination contract](wiki/spec/language/termination.md). Owners:
  `typed-trees-to-checked-trees/src/checks/termination/ranking/`,
  validation's `proof_contracts/contract_entailment/ranking_range/`, and
  `machine_calls/call_cycles/runtime_ranking/`.

  - Extend non-polynomial actual-argument substitution beyond supported
    quotient/remainder endpoints. Recheck constituent operations at each exact
    arrival; cancellation, equal intervals and unoriented disequality do not
    establish divisor validity, absence of overflow or subject identity.
    The independent interval-only fallback still requires one state.
    Reuse `rank_ranges/{field_endpoint_arithmetic,call_components}.rs` and
    stored-reference arrival/reseating controls; mixed-component inputs need
    actual correspondence and conservation evidence. Preserve formation, exact
    arrival correspondence and complete write-frame checks.
  - Replace residual rank-role discovery limits with explicit arrival
    correspondence. Do not rebuild already-supported nested carriers, moved
    scalar/slice/record copies or produced rank facts; an arbitrary decreasing
    copy or shared nominal type is not the ranked value.
    Publish/run the unchanged named-state `compiler/tests/rank_remainder_endpoints.rs`
    and stored-exclusive `rank_endpoint_borrows.rs` customers natively.
    Their checked-interpreter routes exist; native scalar plans/signatures and
    loop headers distinct from entry remain dependencies on
    **STATE-LOCAL-VALUE-FRONTIER**.
  - Complete rank-preserving boundary/requirement call handling using selected
    execution/write contracts, not a signature-only claim that exclusive writes
    never happen. Direct and composed checked-body call initializers already
    preserve entry facts through `call_tree_initializer_preserves_entry`;
    mutable writes to protected premise carriers must continue to reject.
  - Use **STATE-LOCAL-VALUE-FRONTIER**'s common computation route to retire
    generated operand-call states, not termination-only provenance for artificial
    source edges. Independent endpoint/contract work can proceed.

  Acceptance: named-state and mutually recursive call components check, lower
  and execute with exact subject/view/range identity, including subordinate calls
  needing the range and projected/borrowed ranks beside unrelated computation.
  Changed endpoints, stale copies, direct/nested-call writes, overflowing
  intermediates and nondecreasing cycles reject. Preserve the
  private-witness/public-guarantee split and descent of every complete call
  cycle. Start with `src/tests/termination/rank_ranges/` and matching
  `tests/omega/{pass,fail}/termination/` controls.

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
flag. The selection helper already compares demanded type/lifetime applications.
Complete its source-driven callable-contract checking, including the attached
requirement route in `selection/requirement_resolution.rs`; preserve exact
qualifications, generic arguments, lifetimes, and explicit selection.
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

Staged reflection generates ordinary field calls that can run at runtime. Explicit
metadata is owned descriptive data; retaining it does not implicitly retain
getters, setters, factories or every known type. Exercise borrowed adapter custody
without requiring static lifetimes. Nonaddressable packed/fragmented fields reject
borrowed visitation unless an explicitly selected copied-read operation supplies
the contract. Schema enumeration performs no device access; Placed visitation
must use exact authorized operations.

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

- **BUILD-PRODUCT-REFERENCES.** Finish executable follow-through for
  [non-executing product selection](wiki/spec/build/scoped_execution.md#selecting-product-declarations-without-executing-them).
  Product descriptions, visibility checks, and exact-symbol admission exist.
  Preserve the [separate build/product checked contexts](wiki/spec/build/scoped_execution.md#two-checked-contexts)
  and BUILD-ADMISSION-CHECKPOINT source custody; names and evaluator indices
  are not durable selection authority.

  The remaining customer is the scalar/multi-state `Runner` in
  `compiler/tests/build_target_activation/qualified_provider_selection.rs`.
  Bind each module's `Runner::run` as ProgramEntry and execute its computed-result
  branch. Scalar/provider and state-call closure belongs to **TR3-TR8** /
  **STATE-LOCAL-VALUE-FRONTIER**; exact receiver attachment belongs to
  **ENTRY-CONTENT-ROOTS**. Unit Console-forwarding success (11/37) does not
  complete this route.

  Acceptance: selected module identity survives product descriptions, ordinary
  helper returns, Terminal production, and native execution. Preserve
  `foreign_helper_product_queries`, `qualified_root_bindings`,
  `product_query_paths`, `product_entry_signatures`, and
  `qualified_provider_selection` controls in the same suite, including computed
  receivers, named lifetimes, parent loans, and conflicting later operands.
  Reject unauthorized private/sibling enumeration, stale or forged descriptions,
  wrong scope/target/slot, swapped or ambiguous declarations, description-to-callable
  conversion, and same-build generated/layout cycles. Delegated private entry
  descriptions remain usable without granting enumeration authority.

- **BUILD-SNAPSHOT-OUTPUTS.** Complete the
  [captured-input and committed-output contract](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem)
  across ordinary compilation. Extend existing snapshot reads, required-output
  settlement, private staging, and publication rather than adding an executor.

  Owners: `package-compilation/src/source_snapshot/`,
  `packages/sources/acquisition/src/tree/filesystem.rs`, build-output,
  build-evaluation, and compiler publication. Per-file observations and later
  live-tree comparisons do not establish coherent whole-tree capture. Cover
  directory membership/link races and edits restoring compared observations
  under the exact isolation premise; fail when that premise cannot be supplied.
  Preserve logical path distinctions, inert links, retained-parent controls,
  and failure cleanup. Existing planted-alias regressions are controls, not proof
  of confinement across every check/use interval during mutation and cleanup.
  Reuse BUILD-ADMISSION-CHECKPOINT and PackageCheckedContext
  for source custody and occurrence identity; no live-host grant extension or
  persistent writable cache.

  Acceptance: an acquired generator reads a narrowed template and publishes a
  required file through artifact-only and companion builds. Preserve
  `compiler/tests/{build_snapshot_outputs,build_named_inputs}.rs`,
  `omega/tests/package_commands/snapshot_outputs.rs`,
  `omega/tests/{completed_build_outputs,build_input_inventory}.rs`, and
  package-manager's `suite` filter `build_named_inputs::`.
  Cover negative lookups/metadata, substitution/link escapes, sealed mutation,
  cross-occurrence receipts, retry, omitted outputs, interruption, final-check
  failure, and identical logical names across packages/targets. Exercise Windows
  runtime behavior; source inspection and cross-compilation do not establish it.
  Measure retained state and compare the spec's separate-tool route.

## Embedding and interpreted components

Implement the [embedding contract](wiki/spec/build/embedding.md) as an ordinary
library, not a CLI wrapper or scripting dialect. Work from one source-to-host
customer through real lifetime and component behavior. Existing `Service` names
in implementation evidence elsewhere on this board describe the current code;
the accepted carrier name is `Binding`. Do not rewrite observed diagnostics until
the implementation migration actually lands.

- **BINDING-CARRIER-NAME.** (new-scope) Migrate the compiler-known runtime
  carrier from `Service<R>` to `Binding<R>` and the unrelated native locator from
  `Binding<...>` to `ForeignBinding<...>`. Owners: `source/library/core/service.omg`,
  `source/library/core/external_binding.omg`, declaration-identity/semantic-binding
  consumers, diagnostics, samples, and corpus readers. Preserve exact canonical
  establishment, affine custody, provider selection, and foreign-locator
  semantics; do not add a spelling-based privilege or compatibility alias.
  Acceptance: the actual source-to-native and source-to-Psi binding customers use
  the new names, missing/forged establishment and wrong locator identities still
  reject, and an ordinary same-spelled user type gains no compiler authority.
  Apply normal exact schema/declaration identity migration, not cross-version
  evidence compatibility guessed from equal spelling or layout.

- **EMBEDDING-SOURCE-TO-HOST.** (new-scope) Deliver compile -> load -> bind ->
  invoke -> host result -> guest result -> close from an ordinary source-authored
  interface package, host implementation, and script entry. Owners: product
  compilation/entry descriptions, `psi/semantics/terminal-interpreter`, and
  selected host adapter realization; expose the library, not command dispatch.
  Use the ratified carrier name after **BINDING-CARRIER-NAME**. The existing
  `terminal-interpreter/tests/unit/embedding_lifetimes.rs` probes are scalar,
  source-free mechanics, not this acceptance. Share immutable admitted programs
  across independently reclaimable instances. Two same-trait slots must address
  different host objects; absent/wrong/unauthorized bindings reject before effects.
  Include host-supplied invalid input/precondition rejection and native result
  schema rejection, not only console output. Then connect public descriptions and
  checked dynamic invocation through the same adapters; requested type metadata
  reuses [Semantic reflection](#semantic-reflection), not a second registry.
  Dynamic lookup selects retained checked entries, never new generic instantiations
  or private access. Inspect/update live objects only through exposed operations
  at valid access points; detached documents require ordinary validated
  construction/application and grant no authority over live storage.
  No mandatory source compiler, filesystem, global runtime, or thread is needed
  to load an already produced Psi artifact. Keep compilation authority separate.

- **EMBEDDING-RESOURCE-LIFETIMES.** (new-scope) Extend that customer through a
  borrowed view, an owned host resource, asynchronous host completion, and an
  explicitly contracted callback. Owners: embedding runtime/adapters and existing
  Terminal loan/custody/execution semantics; depends on **EMBEDDING-SOURCE-TO-HOST**.
  Acceptance: pause inside an invariant window and reject conflicting inspection
  and mutation, then resume without repeated effects; also reject an edit that
  preserves the type invariant but invalidates retained stronger facts. Retained
  results/callbacks prevent premature close, dropping the invocation handle
  does not release debt, stale/wrong-runtime handles reject, and completion is
  exact-once with duplicate/late response controls. Exercise failure after a host
  effect and unsupported abandonment without retry or fabricated cleanup.
  Close one instance while another remains usable. Admission must reject resource
  modes whose abandonment cannot be contained. A whole-runtime busy wrapper may
  be an initial floor, not the long-term concurrency/replacement architecture.

- **PSI-COMPONENT-REPLACEMENT.** (new-scope) Supply the interpreter-backed
  execution/install/publication provider for a guest that replaces its own
  independent subcomponent without returning from its main machine. Reuse
  **COMPONENT-SUBSTRATE**'s closed import/export and replacement evidence plus
  **EMBEDDING-RESOURCE-LIFETIMES**; do not create a second module manager.
  Owners: component execution/publication library and interpreter instance/entry
  custody; application code owns acquisition, migration, and update policy.
  Acceptance: ordinary root/nested build composition produces a replacement Psi
  component; the running guest stages it and publishes under delegated authority.
  New entries use v2, a paused old call/session continues v1, and v1 retires only
  after all pins/dispositions settle. Missing update authority, incompatible
  schema, widened authority, premature retirement, and handler substitution on
  resume reject. Exercise both shared external state and an explicit state
  disposition, preserving unaffected components. Loading alone never publishes;
  the outer host must not implement the guest's update coordinator for the test.

- **INTERPRETED-CATHEDRAL.** (new-scope) Drive actual Cathedral startup,
  memory establishment and visible device work, then event delivery while it
  remains live, then guest-owned component replacement. Depends on the connected
  embedding/component tasks and the necessary existing boot/entry/resource
  capabilities; Cathedral owns OS code, the host supplies contracted mechanisms.
  Use the same shared algorithms/state machines in native and interpreted routes;
  select providers/build composition rather than add execution-mode conditionals.
  A host routine replacing Cathedral's boot, scheduler, or update policy does not
  count. Report each delivered slice and its real remaining dependencies rather
  than claim a boot banner demonstrates the full system. Unchanged assembly-
  bearing boot is OWNER-BLOCKED on `interpreted-inline-assembly` in
  **OWNER_QUESTIONS.md**; generic embedding work is not blocked. Do not silently
  require removing OS assembly or promise an emulator. Simulation is evidence
  under the selected environment model, not native timing/hardware correctness.

## Requirement-based tests

Implement [the settled testing contract](wiki/spec/build/testing.md) through
ordinary requirements, Build selections, and verified Terminal Psi execution.
No test attribute, special method name, test-only calling mode, or parallel
compiler pipeline. The [guide](wiki/language_guide/chapter_23_testing.md) gives
the source shape; these items track implementation, not further design.

- **BUILD-TEST-GROUPS.** (new-scope) Deliver the service-free end-to-end path:
  `builder.tests.group<ExactRequirement>()`, normalized group enablement,
  package-local concrete satisfaction discovery, separate runner roots, and
  Terminal Psi execution gating successful build publication. Own registration
  in `omega-rust/omega/build/`, compose existing Psi production/verification and
  interpretation through `omega-rust/omega/compiler/`, and report invocation
  outcomes through the normal compile result. Preserve exact product-reference
  identity, private visibility, and source/target provenance; do not discover by
  strings or infer generic applications. Add the optional ordinary std testing
  requirement without compiler recognition or an implicit dependency.
  Acceptance: an ordinary project build discovers two tests without per-test
  registration; a failing check prevents publication; disabling its group reports
  not-run; a second requirement in the same trait stays a distinct group.
  Cover duplicate/inherited-identity registration, non-runnable signatures,
  unresolved generics, dependency non-discovery, and exhaustion versus failure.
  Run a no-std project. Application Psi/native output excludes test-only roots,
  while an ordinary native harness can explicitly call a visible test machine.
  Receiver/service execution remains fail-closed until BUILD-TEST-AUTHORITY;
  isolated discovery or interpreter helper tests do not close this item.

- **BUILD-TEST-AUTHORITY.** (split-of:BUILD-TEST-GROUPS) Extend that same project
  path with group-scoped providers, ordinary provisioned test receivers, fresh
  mock state, and restricted-test review before execution. Owners are existing
  Build/provider and entry-establishment code, package review/lock acceptance,
  the Terminal interpreter's service boundary, and compiler publication. Reuse
  their authority and custody rules; do not put build policy in constant-evaluator
  admission or add a second approval file. Acceptance: two tests receive fresh
  virtual filesystems and established Binding fields; missing bindings
  reject. A child requesting host filesystem/network/process access cannot widen
  the root grant or escape virtual backing. Install/update surfaces new test
  requests, locked builds cannot approve them, and acceptance without an executor
  grant still rejects. Test selected-provider and target mismatches, unsupported
  execution with no native fallback, disabled-to-enabled rechecking, and crash
  containment without invented unwind/rollback. Production provider selection
  and emitted application roots remain unchanged by mock configuration.

## Build-level behavior exclusions

Implement [the accepted exclusion contract](wiki/spec/build/behavior_exclusions.md)
for one source library whose checking/no-op assertion selection changes product
crash behavior without edits to public ceilings. Assertions remain ordinary calls;
there is no special Assert cause or global permission to violate callable contracts.
These tasks use existing build selection, portable semantics, provider and
artifact-verification owners, not an assertion-specific interpreter or duplicate IR.

- **BUILD-SEMANTIC-EXCLUSIONS.** Complete absence evidence for the full admitted
  entry/call closure, including generated entries and remaining dynamic-target
  forms. Reuse exact conformance-application joins and nominal-cleanup traversal.
  Missing target coverage must reject as insufficient evidence. Sound guard
  facts may establish unreachable behavior; optional optimization and broad
  public ceilings are not absence proofs.

  Owners: `build-evaluation/src/admission/behavior_exclusions.rs`,
  `checked-compilation-to-terminal-artifact/src/terminal_artifact/behavior_exclusions.rs`,
  and Psi operation/guard evidence. BUILD-EXCLUSION-REALIZATION owns physical
  classes and installation, not a duplicate semantic checker.

  Close direct Unit crash planning through CRASH-CONTRACT and Unit control-flow
  owners. `compiler/tests/behavior_exclusions.rs` still has
  `missing_direct_unit_plan_does_not_establish_absence`, expecting
  InvalidUnitMachinePlan. Replace that sentinel with the actual Trap-exclusion
  verdict, and realize the admitted direct-crash body natively without a
  scalar-helper substitute.

  Acceptance: unchanged checking/no-op assertion implementations with public
  Trap ceilings differ correctly under exclusion, through native publication
  with optimizations on/off. Eager argument traps and unrelated crashes reject.
  An ordinary silent logger may pass a service exclusion; an actual boundary
  invocation cannot pass merely because its provider is silent.
  Exercise conditional/helper selections and independent replay, rejecting
  changed entries/providers/targets/scopes/policies and omitted coverage.
  Preserve `tests/fixtures/packages/behavior-exclusions/` and compiler tests
  `behavior_exclusions.rs` / `build_behavior_exclusions.rs`. Distinguish
  prohibited behavior from insufficient evidence without weakening contracts.

- **BUILD-EXCLUSION-REALIZATION.** Finish physical exclusions through native
  custody, installation, and replacement. Executed Build selections, canonical
  unions, mechanism classification, and direct/retained replay exist. Owners:
  `build-evaluation/src/admission/{declarations,behavior_exclusions}.rs` and
  `native-realization/src/{native_product/realization,native_realization/behavior_exclusions,retained_native_product}.rs`.
  TWO-AXIS-TERMINAL-AUTHORITY-REVIEW separately owns receiving permission.

  Upgrade `sink_composition_physical_exclusion_reaches_native_custody_frontier`
  in `compiler/tests/build_behavior_exclusions.rs` from its SourceCustodyMismatch
  sentinel to native empty-output execution and independent retained-product
  replay. Ordinary custody in `target-operations-to-selected-instructions/src/legalization`
  is the dependency; preserve the semantic service-exclusion rejection for
  that same invocation. Carry exclusion envelopes through image emission,
  COMPONENT-SUBSTRATE replacement, and WIRE-RUNTIME-AND-INSTALLATION.

  Acceptance: silent and output-producing providers receive distinct physical
  verdicts, with/without receiving permission policy and optional optimization.
  Unknown classifications cannot prove absence; substituted providers/evidence
  reject and failed final checks publish no successful product. Preserve the
  existing foreign-boundary and source-free replay controls. Exercise actual
  Windows/macOS runtime and installation legs; passing sentinel or cross-target
  tests do not establish them. Reuse mechanism-closure review, not a second
  classifier or synthesized receiver approval.

## Checked boundary topology

Implement the [reference-package contract](wiki/spec/packages/topology.md) as
ordinary Omega policy/orchestration over verified component facts and provider
mechanisms. COMPONENT-SUBSTRATE owns component completeness;
WIRE-RUNTIME-AND-INSTALLATION owns generic executable custody. No compiler graph
stage, topology-specific IR, or new trusted graph axiom.

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver the Omega build-only package and payment
  composition project. Reuse `omega-rust/omega/packages/topology/` normalization,
  policy, certificate, and codec implementations, plus the existing component
  description producer/admission in compiler, build-evaluation, and
  `backend/artifacts/component-description/`.

  Connect topology's demand/supply contract join to
  `VerifiedComponent::export_contracts()`. The verified API already supplies
  structured contract identities; `verified_components.rs` still derives
  offered contracts from opaque export identity strings. This is a consumer
  gap, not an absent description representation; do not parse those strings
  or recreate the representation.

  Author the package over admitted input bytes and generic required outputs.
  `tests/fixtures/packages/build-scope-topology` establishes import/output
  plumbing, not payment-plan verification. BUILD-SNAPSHOT-OUTPUTS owns
  confinement/publication dependencies. Dependency-adjacent live reads and
  handwritten inventories are not substitutes for admitted facts.

  Acceptance: a composition build admits three real component descriptions and
  publishes `payments.plan`; a source-free consumer verifies it against a
  separately supplied current TopologyRequest. Preserve the spec's
  [policy controls](wiki/spec/packages/topology.md#diagnostics-and-implementation-acceptance):
  missing/substituted components, stale subjects, forged completeness,
  direct/indirect bypass, cycles, instance distinction, duplicate bindings,
  disconnected selectors, missing policies, and corrupt plans. Unselected
  policy executables are never loaded. Plan bytes do not establish installation.

- **TOPOLOGY-PRIVATE-PIPE-INSTALLATION.** Run the
  [three-process payment customer](wiki/spec/packages/topology.md#first-executable-realization)
  through an Omega-authored installer and Windows/macOS providers. Dependencies:
  TOPOLOGY-PLAN-VERIFICATION, COMPONENT-SUBSTRATE, and
  WIRE-RUNTIME-AND-INSTALLATION. Package code owns orchestration, providers own
  physical mechanisms, and the existing installation owner owns generic custody.

  Reuse `packages/topology/src/topology_installation.rs` and its mediation,
  frame, schema, pipe, and supervisor modules. Rust reference tests exercise
  kernel-attested channel identity, executable admission, and cleanup; they do
  not replace authored component programs and installer orchestration.
  Complete explicit inheritable-handle transfer on Windows and validated/signed
  member-image and descriptor transfer on macOS. General inheritance stays
  disabled; each member receives exactly its admitted endpoint assignment.

  Acceptance: admit exact component images and endpoint mappings before entry;
  allow a schema-checked request/response pair per binding. Reject ungranted
  endpoints and substituted mappings; malformed frames or peer failure close
  the binding. Failed setup and quiescence clean the entire roster, including
  killed members. Exercise activation/replacement custody under the installation
  contract. Record actual Windows/macOS execution, not cross-target emission.

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

- **ENTRY-CONTENT-ROOTS.** Finish intrinsic receiver activation/completion
  under [entry roots](wiki/spec/build/entry_roots.md) and
  [binding validity](wiki/spec/build/component_publication.md#bindings-and-era-entry).
  Owners: target package assembly, `program-entry-plan`, Psi
  `terminal-production`, Omega `compiler/native-realization`,
  `image-emission` and `external-roots`. Existing `Service<R>` source names
  migrate under **BINDING-CARRIER-NAME**; the old service-only `Bound` domain
  is already retired and must not return.

  - Finish activation/result/out-parameter joins using exact closed requirement,
    occurrence and selected-plan validity. Reconcile residual
    `ensures`/`established by` fixtures with intrinsic binding semantics:
    adjust an obsolete fixture rather than authorize a source-created binding.
    Relevant consumers include `facts/qualification_evidence.rs`,
    `checks/content/call_results.rs` and source domain-route construction.
    Ordinary library authority domains remain separate; unused type declarations
    alone demand no provider.
  - Complete nominal receiver cleanup and callback/signal occupancy through
    actual activation/completion. Reuse `receiver_eligibility.rs`,
    `image-emission/src/hosted_receiver.rs` and
    `ProgramLocalRootInstallationLedger`, including installed aggregate
    extents. Four hosted bridges and aggregate eligibility already exist;
    investigate concrete failures instead of adding storage profiles.
    Windows process-exit realization belongs to the process-exit task.
  - Carry exact requirement/occurrence/plan custody through source checking,
    erased Fused fields, Terminal, native settlement and independent replay.
    Finish remaining bare-carrier fixture/application migration without weakening
    source rejection or exact application identity. Installation, not compilation,
    owns actual occurrence custody.

  Acceptance: published `number_guess`, `cli_mvp` and `generic_counters`
  retain their documented behavior with no test-supplied `self` or service
  qualifications. Keep `entry_and_abi::hosted_receiver*` controls.
  Execute on matching hosts and report unavailable runtime legs explicitly.
  Missing/incompatible supply, forged zero/literal construction, lookalikes,
  redirected continuations, non-ZII receiver state, bad backing/alignment,
  overlapping partitions and stale occurrence/epoch reject. Forwarding preserves
  distinct applications and affine custody; erasure removes neither
  initialization nor cleanup obligations.
  Compose application, bridge, provider and callback stack demand—loader
  stack-size metadata is not remaining-stack evidence. Targetless checks select
  no physical entry, and deployment cannot substitute a semantic continuation
  for its adapter. Descriptor-call customers keep their own dependency; Fused
  migration does not require an Independent-installation project.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Execute the source-authored two-surface
  bootstrap under [UEFI adapters](wiki/spec/build/uefi_entry.md#authored-firmware-definitions-and-adapters).
  The target package owns firmware layouts, integrity checks, calling policies
  and bootstrap machines; the compiler owns the necessary physical shell and
  generic ABI/entry primitives.

  Connect source bootstrap, semantic continuation and physical arrival through
  `compiler/native-realization` and
  `external-roots/src/platform_bringup/uefi_bootstrap/`.
  System Table, Boot Services and Loaded Image layouts already replay
  source-minted commitments from `targets/uefi_x86_64/tables.omg`;
  physical calling-plan replay, production semantic-wrapper callers and a
  provisioned mutable semantic receiver also exist. Do not restart those legs.

  Resume the complete customer from
  `entry_and_abi::program_entries_and_image_validation::uefi_entry_machine_plan_produces_terminal_artifact`
  through emitted bootstrap execution on firmware or a controlled provider.
  **UEFI-OS-HANDOFF** additionally needs native realization of its exact
  termination/stack-transfer edges; source admission alone does not supply them.

  Acceptance: authored layouts feed actual firmware projections/calls and the
  emitted bootstrap reaches the selected continuation with checked stack,
  roots, storage and return behavior. Malformed geometry/header integrity,
  foreign occurrence, wrong package/target and overlapping image/storage reject.
  Retain scoped firmware authority and independent source/plan/realization replay.
  Layout helpers, source preflight, source digests and Rust-constructed
  contracts are not this execution witness. Repair missing general capability
  in its owner, not with another firmware-protocol intrinsic.

- **UEFI-OS-HANDOFF.** Execute the authored
  [Boot Services-to-OS handoff](wiki/spec/build/uefi_entry.md#returning-application-versus-os-handoff)
  in `std/targets/uefi_x86_64/handoff.omg`.
  `UefiOsHandoffCycle::run` already expresses acquire/grow/adopt and bounded
  stale-key retry; bodied boundary callees and its cyclic source route lower.
  The pinned next refusal is
  `native_uefi_os_handoff_invocation_reports_termination_catalog_frontier`:
  `UefiOsHandoffTermination::transfer`/`firmware_return` have selected
  compiler-intrinsic rows but no closed native catalog identity.

  **UEFI-PHYSICAL-SEMANTIC-ENTRY** owns those entry/stack-transition mechanics,
  source layouts and scoped firmware leaves. Keep allocation, map acquisition,
  status handling and retries in ordinary target-package code; delete
  superseded Rust production sequencing when the authored route covers it.
  Reuse controls in
  `uefi_bootstrap/{os_handoff_cycle.rs,get_memory_map/,exit_boot_services/}`.

  Acceptance: `build/uefi_os_handoff_invocation` runs through native emission
  and a firmware/controlled-provider harness: grow then retry a stale key,
  bounded exhaustion with the target error, malformed map geometry,
  stale/foreign evidence, lost custody, forbidden Boot Services use after exit,
  and nonreturning success to the exact selected OS entry. Preserve freshest
  map/key, surviving-stack evidence, allocation lineage and final-map obligations.
  Ledger tests, a selected bodyless provider, or a special multi-call leaf do not
  establish authored execution; no destructor hides the fallible protocol.

- **AP-BRINGUP.** Complete Cathedral's authored secondary-processor startup
  under [external roots](wiki/spec/build/external_roots.md#secondary-processor-startup).
  Cathedral owns discovery, dispatch, acknowledgement, retry and cancellation;
  selected hardware boundaries supply premises. The compiler checks installed
  entry, placement, resource custody and evidence, not an APIC/firmware driver.

  The `external-roots/src/platform_bringup/secondary_processor` ledger,
  emitted x86-64 startup trampoline, source canary and
  `compiler/tests/secondary_processor_startup.rs` integration already exist.
  Remaining: real provider/hardware receipt ingress with **BOUNDARY-ISSUANCE**,
  and matching platform/emulator execution evidence. Use the emitted trampoline
  and exact installed entry, not test-authored replacement bytes or
  Rust-constructed arrival receipts as the final witness.

  Preserve the lifecycle: definite nondispatch permits withdrawal;
  dispatch-unconfirmed holds resources and permits a later exact arrival.
  Cancellation releases only with exact outstanding-invocation evidence that
  no later arrival is possible and nothing executes on the stack/state.
  Started accounts retire through exact quiescence. Timeout alone grants none
  of these facts. Bind each provider-declared profile and mechanism-specific
  vector/low-memory geometry to its selected contract.

  Acceptance: authored startup reaches installed code on dedicated,
  nonoverlapping stack/state. Exercise nondispatch, timeout then late arrival,
  cancellation before confirmation, arrival/cancellation races, foreign/stale/
  replayed evidence, resource conflicts and retirement. Keep resources while any
  admitted attempt can reach them; report unavailable host/emulator legs
  explicitly.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Execute nonempty content
  claims through source, Terminal, provider selection and native realization
  under [content conservation](wiki/spec/resources/content_custody.md).
  Reuse `checked-trees-to-lowered-psi/src/proofs/content_conservation.rs`,
  Terminal frontier verification and
  `terminal-psi-to-abstract-operations/src/provider_installation/replay.rs`;
  normalized equations and partition lowering already exist.

  Replace the empty entries in `core/content_conservation_contract` and
  `core/content_retained_custody_round_trip` with an invoked route: an
  established owned input is forwarded/partitioned under an authored theorem;
  a selected boundary accepts the exact residual while the caller retains or
  returns the rest. Use canonical core content identities, not lookalike
  test algebra. Attachment production has changed since the old service-root
  refusal; rerun this customer before naming its next compiler blocker.

  Acceptance: source/native execution preserves exact subject/revision,
  projection/algebra, geometry, lineage, route and installed occurrence through
  independent replay. A partition theorem is usable only after its exact
  successful invocation; provider custody acceptance is not residual arithmetic.
  Reconstruct each introduction/admitted issuance and account for every
  exit/residual. Overlap, gaps, wrong projection/unit/lineage, stale invocation,
  substituted theorem/arguments and authority inferred from scalar totals or
  fingerprints reject. Keep unrepresented runtime-indexed owned extraction
  rejected. Existing-input conservation need not wait for every fresh-issuance
  route under **BOUNDARY-ISSUANCE**.

- **BOUNDARY-ISSUANCE.** Join source fresh-supply evidence to provider planning,
  native settlement and the exact installed occurrence under
  [external roots and issuance](wiki/spec/resources/authority.md#external-roots-and-issuance).
  Source issuance/construction checks and runtime receipt ingress validation
  already exist; they do not complete this cross-stage join.

  Reuse `checks/content/{retained_custody,call_results}.rs`, qualification
  evidence, Terminal claims, selected provider plans and
  `external-roots` settlement. Derive geometry from exact arguments,
  callable-entry places and result paths; admit the provider's backing/freshness
  premises, never interval arithmetic as an opaque provider fact. Multiple
  results require separated supply; transferred input is not freshly minted
  capacity. **DOMAIN-ISSUER-ROUTES** owns source route selection,
  **AP-BRINGUP** its hardware arrival customer, and conservation of existing
  accounts remains independent work.

  Acceptance: source-issued content retains geometry, backing, issuer, lineage,
  route and installed occurrence through independent replay. Forged source
  construction, foreign/replayed receipts, substituted geometry and duplicate
  supply reject; legitimate issuance and identity-preserving transfers succeed
  without re-minting capacity.

## P2 - Materialization and placed access

- **PLAN-LAID-VIEWS.** Complete executable placed-view establishment, access
  and retirement under [placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement).
  Direct-entry interpreter/native admission already joins exact roster rows to
  provider-supplied referents, checks declaration consistency, aliasing and
  qualifications, and carries loans into native entry settlement.

  The next bridge is `compiler/native-realization`'s
  `native_realization/optimized_fragment_projection.rs` and
  `image-emission/src/hosted_receiver.rs`: emission rejects a settlement with
  bound establishments because the entry shim cannot lend each referent yet.
  Carry each loan for the invocation's duration and retire it at completion.
  Non-entry machines additionally need call-bound custody from the caller.
  Keep unsupported routes rejected; a roster or pointer is not authority.

  Extend `compiler/tests/access_plans/source_access_policies.rs`'s
  `direct_placed_view_input_survives_codec_and_native_replay` to actual source
  access. Its consumer is empty: `view.status.read()` needs realization of the
  derived `PlacedField` accessor, not just an admitted roster. Reuse the
  existing `ArtifactSections`, codec, optimization and verified-native-input
  boundary rather than rebuilding admission.

  Acceptance: one source program establishes, accesses and retires a view
  through interpretation and published native execution. Codec/optimization
  preserve semantics; stale/substituted plan, artifact, backing, range, rights,
  occurrence or lifetime reject before access. Failed establishment returns
  custody and retirement preserves declared resident/vacant state. Preserve
  multi-row order independence, exact type/path/domain rejoin, exclusive overlap
  rejection and shared/disjoint admission. Run matching supported hosts or
  explicitly retain unavailable runtime legs.

- **SYMBOLIC-MATERIALIZATION.** Obtain the missing matching-host Linux AArch64
  execution evidence for the existing recursive
  [derived consumer](wiki/spec/layouts/plans.md#derived-consumers).
  Owners: `backend/layout/src/sum_materialization/`,
  `psi/foundation/layout-plans/src/symbolic_materialization.rs` and
  `compiler/tests/layout_plans/writer_lowering.rs`.
  Both Linux ISA fragments already replay in the harness; recorded Linux
  x86-64 and macOS AArch64 runs do not close Linux AArch64 runtime acceptance.

  On Linux AArch64 run
  `mbx nextest run -p compiler --test layout_plans -E 'test(~writer_lowering)'`
  and `mbx nextest run -p layout --lib -E 'test(~fenced)'`.
  Acceptance: nested field/index writes match reference destination and guard
  bytes, with the same normalized fragment identity/invocation across ISA-specific
  bytes. Retain empty-array, sibling-write and nested/generic controls.

  Open templates, non-closed member applications and runtime value counts in
  static layout remain deliberate rejection boundaries—not missing symbolic
  capacity. Closed generic applications already substitute literal lengths;
  do not add depth-specific reports or turn a host-validation task into an
  implementation project.

## P3 - Terminal Psi, PCC, and observation

- **PCC-PRODUCT-PUBLICATION.** Deliver standalone native behavioral evidence
  under [optional proof products](wiki/spec/proofs/publication.md).
  `compilation-report/src/pcc.rs` already publishes/verifies native sidecars,
  including bundle placement, image/data inventories, entry/load-map custody and
  bounded import-thunk semantics. These coverage checks still end in
  `Incomplete(UnsupportedEvidence { product: Native })`, not behavioral success.

  Native semantic/certification owners must:
  - Decode published instructions against the declared target's closed semantics.
  - Reconstruct control-flow edges and entry obligations inside function regions;
    a container entry landing on a placed boundary proves only the custody half.
  - Establish premise availability and lowering correspondence, either by
    translating checked Terminal obligations or proving native obligations
    directly. Producer-report hashes or unrelated valid Psi prove neither.
  - Return `Complete` only under a guarantee supported by that behavioral
    evidence; unbuilt target legs remain `Incomplete`.

  Reuse the existing envelope rather than putting semantic checking in
  `compilation-report`. Acceptance: artifact bytes, companion and the receiver's
  pinned policy suffice after deleting source and Psi. Arbitrary native bytes
  paired with valid Psi/recomputed producer hashes, tampering, relabeled profiles
  and stale sidecars reject. Bind evidence after byte-changing finalization and
  check permitted loading/relocation correspondence; a matching file hash does
  not establish loaded-image semantics. Omitted dependencies must be pinned by
  exact identity and independently possessed by the receiver; preserve
  receiver-owned admission. Broader mathematical guarantees depend on
  **PROOF-KERNEL-CORE**, **PROOF-CERTIFICATION-BRIDGE** and their completed
  profiles, not a policy DSL. **MACOS-APPLICATION-PUBLICATION** owns actual bundle
  execution, not this evidence.

- **PSIIR.** Complete source-free Terminal execution and logical-work bounds
  across canonical encoding, independent reconstruction, interpretation,
  resource analysis, native realization and installation.
  The [encoding contract](wiki/spec/terminal-psi/encoding.md) and
  `tests/architecture/encoding_contract.rs` already cover the codec's closed
  vocabulary, envelopes and mathematical certificates; maintain that coverage
  as the operation owners extend it, not another wire-format project.

  Remaining resource analysis is in `terminal-fixed-fuel`: derive bounds from
  relevant preconditions and retain precise absence-of-bound causes, including
  unbounded rank and the wait/foreign edge preventing closure. Acyclic
  conditional/case segments, condensed ranked interiors and topology-identified
  unranked cycles already have derivation. An invocation-bound callee without
  retained realization/bound evidence is genuinely open: preserve
  `InvocationBoundCallee` rather than fabricate a ceiling or classify every
  refusal as an implementation bug.

  Acceptance: discard source/producer state, independently reconstruct all
  obligations, then interpret or lower the same artifact with exact resource
  and installation custody. Native/ABI/final-code claims require final-realization
  evidence, not checked API metadata or opaque supply. Preserve recompute-and-
  compare bounds and truthful unavailable results. Specific source/native
  gaps belong to the operation owners below and **TRANSLATION-VALIDATION** in
  `TASKS_OPTIMIZER.md`; codec fixture drift belongs to repository closure,
  not repeated wire-design tasks.

- **GENERAL-CYCLIC-EXECUTION.** Complete
  [cyclic control](wiki/spec/terminal-psi/control_flow.md) and
  [safety/progress checking](wiki/spec/language/termination.md) for the unchanged
  decimal/Console loop and `print_squares`.
  Owners: `terminal-verifier/src/validation/control_flow/`, Psi's
  `execution/unit/`, `checked-trees-to-lowered-psi/src/unit/attached_unit/`
  and expression preparation. The same-name optimizer-board task owns native
  receiving/replay coverage. Unit/scalar/aggregate functions already share the
  common native graph; no countdown or whole-Unit fallback.

  - Carry bounded-field byte reads through expression lowering and the portable
    vocabulary. The checked literal-equality guard retains
    `StructuralParameterIndexedRead` with a field path, but
    `expression_preparation/prepare_expression.rs` requires a whole byte-view
    parameter. `runtime_number_to_decimal_exit_canary_runs` exposes this join;
    `runtime_bounded_carrier_write_read_exit` and `utf8_equals_literal_exit`
    are related controls. Use exact carrier/path/live-length evidence, not a
    decimal-specific leaf.
  - Close the customer over existing scalar-call/store transport and cyclic
    bounded-field writes. `compiler/tests/byte_field_replacement/indexed.rs`
    already covers source writes and cyclic native publication; helper coverage
    is not the unchanged customer's output. Coordinate obsolete range-suffix
    fixtures with **REMOVE-BRACKETED-RANGE-ANNOTATIONS**.
  - Replace remaining cyclic shape exclusions with independently checked
    arrival/custody relations: qualified and partial owned values, structural
    results, claim transfers/reshuffles/partition compositions and effectful
    calls. Preserve dominance, exact successors/frontiers, current-iteration
    guards and test-fuel suspension/resumption. Extend only when the retained
    proof establishes the case; fuel or a relaxed allowlist proves no safety.
  - Compose projected helpers, Console structural operands, indexed/aggregate
    writes and computed results without source-state duplication.

  Architecture trap: `unranked_cycles.rs` keeps growing per source shape
  although its eligibility check grants no proof authority. Complete the
  per-arrival custody checking before retiring superseded restrictions.
  The producer's `proofs/scalar_block_invariants/lockstep.rs` also recognizes
  a particular divisor/counter update. Its proposals are independently proved,
  but general strengthening should not become one recognizer per customer.
  Existing `cyclic_field_divisor_*` controls already establish that arithmetic
  case; do not repeat the isolated invariant milestone.

  Acceptance: unchanged decimal source reaches Terminal, then its native
  exit/output on the hosted matrix with exact caller/callee resources and
  ranking where declared. Corrupt arrivals, custody, guards, effects and proof
  groups reject. **SAMPLE-CORPUS** owns the full customer and current
  `print_squares` scope pause; **STATE-LOCAL-VALUE-FRONTIER** owns shared value
  transport. Interpreted loops or checked-only canaries do not exercise the
  complete lowering repair.

- **CRASH-CONTRACT.** Carry invocation-specific crash obligations through
  selected operators, nested values, calls, cycles, execution and package review.
  Owners: `facts/{operator_crashes,crash_entry_values}.rs`,
  `CrashPlan::checked_operators`, `flow/expression.rs`,
  lowered `retention/operation_crash_contracts.rs` and independent Terminal/
  native consumers. Keep selected requirement, saved actuals, Match arm and
  surviving route; do not infer semantic causes from traps or inspect opaque
  providers to narrow contracts.

  - Generalize selected-use to emitted-operation joins beyond existing IEEE/
    integer comparison rosters. Named uses, structural operand telescopes and
    call carriers need exact operand mapping, provider commitment and Psi/Omega
    replay through one compositional occurrence relation, not another roster
    per operator. Guarded float/generic/structural-formal routes need portable
    predicate vocabulary; copying identity or conservative `Truth` rows is
    not replay of the original guard.
  - Finish invocation-outcome trace/refinement joins under
    [reconstructed observations](wiki/spec/terminal-psi/observations.md#reconstructed-rows).
    Target Unit/borrowed/aggregate call lanes already carry crash continuations;
    do not restore the old blanket-refusal diagnosis. Exercise actual outcome,
    guard substitution, abandoned claims, staged writeback and no-result/
    no-cleanup behavior through the source-to-execution route, using
    `scalar_boundary_arguments.rs` and `behavior_exclusions.rs`.
  - Carry qualified scalar results, state/control contracts, mutable entry
    snapshots across joins and field/arithmetic guarantees through ordered
    completion. Reuse `unit_scalar_result_source/boundary_wrappers/`.
    Preserve transitive callees' authored contracts; current mutable storage is
    not an entry snapshot, and crash ceilings imply no normal guarantee.
  - Complete package-review projections through indexes, case payloads and
    generic fields using Psi's exact carrier/case relation. Keep
    `package-evidence/tests/callable_policy/case_membership.rs` as the
    source/recovery control; saved/result tags need ordinary value custody,
    not re-executed initializers or callee bodies.
  - Extend entry provenance for opaque/content leaves, dynamic indices and
    whole-collection reads after element writes only with proven origins.
    Fixed-index/range disjointness already exists. Divergent arrivals,
    unresolvable cycles and unknown origins remain conservative.
    Share snapshots with **STATE-LOCAL-VALUE-FRONTIER** and the crash-qualified
    equality dependency of **MATCH-SELECTIVE-LOWERING**.

  Acceptance: source `operators/crash_routes` and crash-qualified float controls
  preserve exact examined/discharged routes and caller coverage through
  independent Terminal replay, execution and package review. Safe uses discharge
  each route; changed guards, captures, substitutions, sites and stale writes
  reject. Proof-only mathematical terms do not authorize executable guard use;
  retired proposition declarations remain **PROOF-CONTRACT-MIGRATION** scope.

- **ARITHMETIC-POLICY-REALIZATION.** (new-scope) Complete executable policies
  from [numeric values](wiki/spec/language/numeric_values.md) through Terminal,
  interpretation and native realization. Owners:
  `checked-trees-to-lowered-psi/src/expression_preparation/`, Terminal
  operation/observation vocabulary and its independent checking/realization.

  - Trapping sites are owner-blocked on
    `terminal-operation-level-trap-crash-site` in
    [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md). The current profile has edge and
    boundary-call crash rows, not an ordinary trapping operation's site.
    Do not fabricate a boundary identity or terminator edge. Once settled,
    carry primitive denotation and path-conditioned crash evidence under the
    same-cause ceiling through all consumers. Trapping casts/shifts currently
    refuse runtime preparation; direct Trapping arithmetic in contract position
    remains invalid, not a new predicate term.
  - Realize nontrivial signed/mixed-sign wrapping conversions beyond identity,
    widening and supported unsigned narrowing. Truncation toward zero does not
    implement a negative value's modular image; bit masking alone supplies no
    proof of the exact-cast range.
  - Complete signed/mixed-sign saturating conversion beyond existing admitted
    cases. Boolean-to-integer and unsigned narrowing already lower; reuse them
    as controls. A signed saturating subtraction is not the unsigned clamp
    identity. Missing representation/realization is implementation work, not a
    reason to mark this whole row owner-blocked.

  Acceptance: the six `core/numeric_*` canaries and Trapping conversions in
  `source/library/core/numeric_conversion.omg` reach native execution with
  policy-correct success/failure and independent crash-site replay.
  Advance `checked-trees-to-lowered-psi/tests/integer_policy_realization.rs`
  controls beside valid neighbors differing in one relevant coordinate.
  Reproduce `float/float_trapping_*` and
  `expressions/arithmetic_domain_trapping_*` before attributing their failures;
  a generic missing-plan diagnostic does not identify arithmetic policy.
  Never silently weaken one policy into another.

- **PROOF-KERNEL-CORE.** Finish independently checked mathematical proofs
  under the [foundation](wiki/spec/proofs/foundation.md) and
  [W-based profile](wiki/spec/proofs/inductive_profile.md). Owner:
  `proof-admission/src/mathematical_core/`; source integration belongs to
  PROOF-CONTRACT-MIGRATION. The core and derived schemes exist as Rust-built
  terms. [Combined metatheory](wiki/spec/proofs/kernel_metatheory.md) is
  argued at paper level, not mechanized; helper judgments do not establish
  source coverage or a complete verified profile.

  Remaining work:

  - Replace bounded denotation's per-instance `rule_axiom` fallbacks with
    checked derivations or explicitly justified checked rules, driven by real
    source obligations. Remaining cases include cast bounds, correlated
    forbidden roots/multiply bounds, nested canonical identity reversal and
    Boolean identities needing case analysis. Preserve numeric policy and
    exact operands. Unsupported arithmetic or construction-budget fallback
    must not be reported as independently proved merely because the kernel
    checks a term assuming its conclusion. Existing fixed integer laws also
    remain explicit assumptions, not a consistency proof.
  - Check source-produced indexed-scheme applications against exact
    parameters, indices, payloads, case constraints and recursive uses.
    The hand-built `indexed_*.rs` tests are useful controls, not a source
    encoding. Reject negative recursion at elaboration and distinguish an
    invalid declaration from unsupported valid encoding or producer failure.
    Complete the profile's required scheme/correspondence justification;
    structural round trips alone do not prove induction or computation.
  - Supply only kernel additions demanded by that source path. Preserve exact
    assumption closure through declaration types/statements, imports and wire
    encoding, independently of unfolding and erasure. Do not introduce a
    parallel primitive-indexed/strict checker or untyped eta rewriting.

  Acceptance: the [migration examples](wiki/spec/proofs/foundation.md#migration-acceptance)
  reach source-free kernel judgments from Omega source: a universe-polymorphic
  theorem over arbitrary predicates with dependent pairs, identity transport
  and induction; strict same-statement conversion without identifying relevant
  witnesses; malformed universes, capture-changing substitution and illegal
  elimination reject. Retain `compiler/tests/kernel_equality_transport.rs`
  and its false-source, changed-operand and changed-equality-endpoint controls.
  Measure term size, retained storage and checking cost on source-produced
  evidence in the application checker. PCC-CANONICAL-SEMANTIC-LEDGER owns
  trusted-row soundness; PROOF-CERTIFICATION-BRIDGE owns loop correspondence.
  Any demonstrated need to change the selected calculus goes to
  `OWNER_QUESTIONS.md`, not an implementation shortcut.

- **PROOF-CONTRACT-MIGRATION.** Deliver general mathematics from Omega
  source through Terminal evidence and independent checking, using
  [machine contracts and trait bundles](wiki/spec/proofs/contracts.md#machines-and-bundles)
  and the selected [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md).
  Owners: Psi syntax/resolution/typing, contract checking, Terminal evidence
  and codec, and `source/library/core/`; use PROOF-KERNEL-CORE's terms.

  Top-level mathematical `let`/`boundary let` parse, resolve and receive
  bounded kernel-signature checking. The production frontier is
  `checked-trees-to-lowered-psi/src/proofs/mathematical_declarations.rs`:
  it rejects every declaration-bearing program because Terminal evidence
  does not carry the checked signature. The mathematical certificate codec
  exists but is not connected to that producer.

  Remaining work:

  - Connect the checked signature to Terminal evidence and receiver checking;
    extend machine-valued body denotation and applied carriers as required by
    the controls below. Replace authored-name sort classification in
    `typed-trees-to-checked-trees/src/proof/mathematical_{declarations,signature}.rs`
    with canonical symbol identity and supply the fixed `core::Level`,
    `Type`, `Strict` and `Squash` declarations.
  - Replace the surviving `proposition`/hidden-witness routes, including
    `proof_output_calls.rs` and their core/test consumers, with ordinary
    contracts and witness/law bundles. Preserve exact substitutions,
    path/result availability, witness identity, validity and assumption closure.
    Remove the retired parser/carrier/codec paths rather than widening them.
  - Implement mathematical term application and theorem-only logical
    hypotheses without changing executable callback selection, ordinary local
    bindings or complete machine calls. Add no quantifier keywords or
    declaration-catalog substitute for arbitrary mathematical terms.
    Trait requirements also need mathematical argument substitution:
    `typed-trees`' `TraitRequirement` currently carries only lifetime and type
    arguments, not predicate/function terms.
    Calls and recursive citations still owe exact preconditions and descent;
    OPERATOR-MACHINE-SUPPLY owns the shared call repair and Nat migration.
  - Enforce [executable demand](wiki/spec/proofs/mathematical_bindings.md#assumptions-and-executable-demand)
    at source use, evaluation and lowering: an axiom is not a missing provider,
    and choice-dependent control cannot execute just because its branches
    contain constants. Preserve assumption closure even when proof use erases.
  - Migrate core relations, quotients and examples. Include the replacement
    proof rows assigned by
    `omega-rust/omega/packages/review/evidence/EVIDENCE_SCHEMA.md` and
    QUOTIENT-THEOREM-LIFT's congruence-only lift payload.

  Acceptance is source → Terminal serialization → independent checking under
  the [publication contract](wiki/spec/proofs/publication.md), with false
  twins for each case:

  1. Two composed witness/law bundles preserve substitutions and distinct
     relevant witnesses.
  2. A higher-order theorem over arbitrary predicates/functions passes the
     [delivery controls](wiki/spec/proofs/mathematical_bindings.md#delivery-controls),
     including derived Π-term evidence for a machine-shaped logical hypothesis.
  3. Nonconstructive existence permits an erased proof reference but not
     unjustified executable extraction or branching. Squash-to-strict reasoning
     needs no choice. Gated record construction proves its coupling; relevant
     dependent pairs remain distinct from strict predicate gating.
  4. Accepting and denying policies distinguish the same theorem using its
     exact transitive closure through statements/types, import, erasure and wire.
  5. Cauchy/quotient reasoning uses the
     [set-quotient interface](wiki/spec/proofs/quotients.md#set-quotient-foundation)
     without hidden extensionality or new reduction. A quotient-refusing policy
     accepts representative operations and congruence with quotient-free
     closure, and rejects the assumption-bearing quotient theorem. Rat's
     implementation comments do not establish that separation.

- **PROOF-CERTIFICATION-BRIDGE.** Check functional guarantees against generated
  loops, independently of termination, under the
  [publication contract](wiki/spec/proofs/publication.md). Owners:
  `typed-trees-to-checked-trees/src/checks/contracts/exits/cyclic_headers.rs`,
  `checked-trees-to-lowered-psi/src/proofs/scalar_block_invariants/cyclic_guarantees.rs`,
  and their independent Terminal verifier.

  The single-state free-loop order claim has a native positive and wrong-step
  control. Arithmetic accumulation (`result == acc + remaining`) checks at
  source but lacks a Terminal proof of
  `(acc + 1) + (remaining - 1) == acc + remaining`; dropping the unproved
  header proposal leaves `OperationProofUnavailable`. The attached
  `proof_inductive_gauss_sum` and `proof_inductive_climbing_sum` fixtures are
  still checked-only and also need STATE-LOCAL-VALUE-FRONTIER's ordinary
  value-returning cyclic execution.

  Carry the source derivation as checked facts that lowering certifies and the
  receiver checks, instead of independently searching for the same invariant
  in both stages. Extend this route to multi-state carried values and
  storage-changing self transitions. PROOF-KERNEL-CORE owns the open-term
  arithmetic derivation; share licensed normalization with
  PCC-CANONICAL-SEMANTIC-LEDGER. Do not invent an arithmetic axiom for each loop,
  discard exact overflow obligations, or infer residue order/disequality from
  the existing Wrapping equality support.

  Acceptance: an arithmetic accumulation fixture or faithful free-loop
  restatement runs natively; the wrong-update twin fails preservation while
  its unchanged cycle certificate still answers the same termination question.
  Missing arrival evidence and an unestablished guarantee reject. Retain
  `terminal-verifier/tests/ranked_scc/`, `ranked_value_guarantees`,
  `compiler/tests/pcc_publication.rs` and architecture layering controls.

- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace trusted fusion of artifact
  traversal and proof search with a total canonical-ledger generator and an
  untrusted certificate producer under the
  [verification contract](wiki/spec/terminal-psi/verification.md#canonical-semantic-ledger).
  Owner: `terminal-verifier`, with `terminal-codec` and `proof-admission`.

  The existing `trusted_surface.rs` inventory mechanically covers dispatch,
  reconstructed facts, dependencies, implementation sources and soundness
  statuses. Reuse it; coverage and a `Proved` label are not themselves proofs.
  Remaining work:

  - Move search out of verification, including the 4096-step search in
    `validation/crash/entry_requirements.rs`. Producers supply certificates;
    receivers reconstruct questions and check the supplied route.
  - Discharge `ExplicitlyTrusted` reconstruction, normalization, scope,
    invalidation and call/cycle-composition rows with checked evidence and
    exact dependencies. Preserve `PROVED_ENTRIES`, dispatch/fact coverage,
    source-closure and unfinished-dependency checks; prove prerequisites
    rather than hiding trusted composition behind a proved leaf.
  - Define the generator over canonical Terminal bytes, not a producer-decoded
    AST accepted by assertion. The common kernel and selected inductive
    profile's unfinished soundness/encoding obligations remain dependencies,
    not permission to assume generator success.

  Acceptance: a theorem-dependent program verifies after source and producer
  state are removed, under accepting and rejecting assumption policies.
  Wrong goals, profiles, scopes, premises, missing obligations and omitted
  transitive assumptions reject; normalization and erasure cannot hide
  dependencies. Use `terminal-verifier/tests/trusted_surface.rs` and codec
  trust-graph controls alongside the connected certificate case. This is not
  native refinement or bootstrap proof discharge; GAMMA-DERIVATION-CHECKER
  remains independently owned by `TASKS_BOOTSTRAP.md`.

- **PROOF-RELEVANCE-MIGRATION.** Complete carrier-independent erased arguments
  under [explicit erased bindings](wiki/spec/proofs/contracts.md#explicit-erased-bindings).
  Static scalar and proof-only argument lanes exist. Two implementation gaps
  remain: `validation/src/proof_contracts/relevance/shape_admission.rs`
  rejects erased runtime-record/enum formals, and
  `typed-trees-to-checked-trees/src/checks/contracts/dynamic_erased_lane.rs`
  rejects erased formals on dynamic requirements because their dispatch plans
  carry no proof actuals. These refusals diagnose missing support; they do not
  complete the specified feature.

  Carry the exact erased subjects/actuals through static and dynamic call plans,
  Terminal contracts and independent call composition without adding runtime
  storage, ABI arguments or execution. Preserve witness identity, scope,
  multiplicity and assumptions. Reuse the existing erased scalar/proof-term
  routes; the exploratory dynamic-lane draft is not a new language prerequisite.
  Erased-field custody/cleanup belongs to
  CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.

  Acceptance: an erased record/enum formal and a dynamic trait call using
  erased evidence check and execute with stripped runtime signatures.
  Replace `fail/relevance/erased_nonscalar_parameter` and
  `dynamic_erased_formal_lane`'s implementation-limit expectations with
  positive coverage. Missing, substituted, out-of-scope or violating proof
  actuals still reject after serialization. Retain the static
  `erased_parameter_proof_only`, `erased_parameter_named_transition_forward`
  and `composed_unit_internal_calls` controls, plus runtime-read, receiver and
  layout-dependent-erasure rejection. Do not relax semantically invalid
  qualifier combinations merely to remove an implementation fence.

## P4 - ABI, borrowing, and callbacks

- **NORMALIZED-ABI-LOWERING.** Finish aggregate and descriptor foreign
  argument/result transport under the
  [calling-plan contract](wiki/spec/build/calling_plans.md). Owners:
  `abstract-operations-to-target-operations`,
  `target-operations-to-selected-instructions` and backend call-frame emission.

  Target lowering and replay already retain owned whole-place aggregates,
  borrowed descriptors and scalar arguments in formal order. Instruction
  legalization and selection still restrict structural arguments to borrowed
  field projections passed as one pointer:
  `legalization/scalar_graph_input/normalized_foreign.rs::structural_argument_at`
  and `selection/scalar_call_abi/normalized_foreign.rs::validate`.
  Realize the selected plan's aggregate locations, descriptor words and result
  custody through those consumers and emission; do not substitute an owned
  aggregate's indirect placement for a semantic borrow.

  Acceptance: convert
  `source_evaluated_native_realization::record_native_arguments::mixed_scalar_and_record_arguments_stop_at_legalization_custody`
  from its current rejection pin to a source-to-C execution oracle on a matching
  host. Preserve the existing `scalar_native_arguments` oracle and test exact
  argument/result values, stack/register placement, formal order, and rejection
  of substituted plans, placements and provider bindings. Each stage must
  independently reconstruct the ABI; Terminal Psi retains no target placement.
  Dynamic descriptors belong to RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY;
  private callbacks belong to CALLBACK-PRIVATE-MATERIALIZATION. Use ordinary
  call operands and explicit custody, not one call family per signature shape.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Connect selected opaque representations
  to executable by-value exchanges under
  [representation agreement](wiki/spec/build/opaque_representations.md).
  Selection, independent rederivation, package attribution and installation
  commitment checks exist; they do not move the carrier's bytes.

  `provider-planning/calling_policy_plans/boundary_signatures.rs::opaque_representation_movement`
  derives exact parameter/result and nested-field placements, but its consumers
  remain planning, review and tests. Connect those movements to Omega's provider
  call-frame transport and backend emission. Preserve one semantic occurrence
  for affine/linear values even when placement copies bytes; only checked
  semantic copying creates another occurrence.

  Acceptance: independently compiled producer/consumer programs execute opaque
  argument, result and nested-field exchanges. Reconstruct exact application
  agreement at each composition edge; reject provider, carrier, target,
  lifecycle, multiplicity and historical-selection drift. Preserve
  `calling_policy_plans/opaque_boundaries.rs`, `opaque_boundary_agreement`
  and installation-record substitution controls. Equal layout or a compact
  fingerprint is not agreement. Keep sealed `Ptr<T>` target semantics,
  `EfiSystemTable` and erased proof-only `Real` distinct: proof-only use does
  not acquire a runtime representation demand.
  COMPONENT-SUBSTRATE owns replacement compatibility and stable-handle eras;
  carry these same application commitments into that path. V1 admits only
  `Inert` carriers: direct or nested cleanup-owning carriers must continue to
  reject, not acquire an unspecified lifecycle through this transport repair.

- **WRITE-ONLY-BORROW.** Finish `&write T` under
  [write-only authority](wiki/spec/terminal-psi/structural_access.md#write-only-authority),
  preserving original referents and exact place/loan custody. Owners:
  `typed-trees-to-checked-trees/src/execution/`, `checked-trees-to-lowered-psi`
  and native reference preparation in instruction legalization.

  Remaining source-to-native work: general aggregate and `[copy]` sum
  replacement beyond scalar-field record literals; runtime domain-qualified
  byte-field replacement with encoding evidence; guarded/computed runtime-index
  production; and computed IEEE store transport through FLOAT-PROVIDERS.
  PLACED-ACCESS-NATIVE-OPS owns native realization of the retained indexed-store
  operation. Its source fixture still needs the revoked bracketed-range
  migration owned by REMOVE-BRACKETED-RANGE-ANNOTATIONS. Route mutable dynamic
  dispatch through FINITE-GENERIC-DISPATCH, common reference identity through
  STRUCTURAL-BORROW-IDENTITY, and sequencing through STATE-LOCAL-VALUE-FRONTIER.
  Consult the [parked IEEE recovery record](wiki/drafts/write_only_borrow_ieee_store_branch.md)
  before duplicating work; its unpublished tip is not available in this checkout.

  Acceptance: move repaired `terminal_psi_indexed_receivers/frontier_pins`
  limitations to caller-storage execution controls. Cover exact width, untouched
  neighbors, runtime signed/Boolean/floating sources, restoration/return custody,
  non-observation and independent replay. Reject reads, bare write-only
  forwarding, readable widening and overlapping exclusive arguments. General
  replacement must preserve displaced custody and whole-value validity, not
  merely decompose more source patterns. Observe computed stores in the original
  caller, not a copied frame home. Run
  `mbx nextest run -p omega-native-differential-test --test terminal_psi_indexed_receivers --no-fail-fast --no-tests fail`
  on matching hosts, including both Linux targets and Windows/macOS; the
  recorded macOS ARM64 run requires `RUST_MIN_STACK=67108864`.
  Cross-publication is not execution.

- **STRUCTURAL-BORROW-IDENTITY.** Complete source-owner/projection routes under
  [structural access](wiki/spec/terminal-psi/structural_access.md), retaining
  original caller storage through call preparation, native placement and
  independent receiving replay. Owners: checked execution's `receiver_calls`
  and `calls/computation_arguments`, lowered structural calls, target
  `structural_call_arguments` and image `argument_custody`.

  Static `Field`/`FixedIndex` shared receivers and explicit shared scalar-call
  arguments are supported. Remaining `src/tests/borrow/receiver_access.rs`
  pins cover local-rooted indexed receivers and runtime-indexed parameter
  receivers/explicit shared arguments. `CheckedUnitStructuralPathSegment`
  has no runtime-index variant: retain a checked selector/value and bounds
  relationship, not a widened path predicate or trusted byte offset. Coordinate
  those fixtures' bracketed-range migration with
  REMOVE-BRACKETED-RANGE-ANNOTATIONS. Owned-root and construction-local admission
  remain separate obligations; reuse `terminal-semantics::static_path`.

  Acceptance: repair the omission pins and execute caller-visible
  projected/forwarded writes, owned-field mutable/write-only subloans, legal
  synchronized shared observations and register/stack reference passing.
  Reject copied borrowed homes and substituted access/type/projection/placement
  evidence; shape equality does not create authority or a standalone field
  type. Preserve `terminal_psi_indexed_receivers` and `primitive_store_return`.
  Complete matching-host coverage, especially the unrecorded Linux AArch64 and
  Windows legs; another callee observing a staged copy is not caller writeback.

- **BORROW-PROOF-CONVERGENCE.** Carry ordinary borrow compatibility from
  checked certificates to independent portable replay under
  [loans](wiki/spec/terminal-psi/loans.md).
  Owners: `typed-trees-to-checked-trees/src/checks/borrows/`,
  `checked-trees/src/checked_trees/borrow.rs`, checked-to-lowered publication
  and Terminal verification. Current checking supports immutable normalized
  bounds, requires/domain predicates, incoming guards, immutable whole-result
  call guarantees and transparent propositions. Two premises can already
  compose through a shared middle. These certificates remain checked-stage
  records, not portable authority.

  Retain exact formation, captured places/resource identities, source
  establishment and consulted premise order through publication. Reconstruct
  availability, dominance and value/place versions independently. Broader
  callee/domain predicates, mutable or projected results, same-statement and
  theorem-call establishment need exact substitution/version evidence.
  Reuse existing contract/range readers, including the original assignment's
  exact call occurrence; do not collect a second set of guarantees. Include
  adversarial call-prerequisite removal/replacement and cross-call copy
  substitution in the replay controls.

  Read-footprint expansion is paused for lack of a demonstrated customer:
  first recheck the recorded `items[low + 0u64..high]` lost-bound candidate.
  An incomplete footprint conservatively retires facts after writes; admitting
  more expression kinds without preserving a needed source fact is not progress.
  The builtin bound-meaning gate is distinct from the read-footprint check.

  Acceptance: each new establishment has source pass coverage for disjoint
  loans, writes and exclusive call operands, with independently replayed
  certificates. Missing, stale, reordered/substituted or insufficient premises,
  overlapping mutation and containment-only second exclusive loans reject.
  Proof never creates, widens, duplicates or extends authority. Preserve
  `src/tests/borrow/checks/premised_disjoint_writes.rs`, the `certificates/`
  suite and stated, sum, guarded, domain, returned-window, proposition and
  chained-premise corpus controls. CANARY-CORPUS routes borrow-obligation
  failures here; these are implementation gaps, not unsettled language rules.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Complete native realization under
  [private callbacks](wiki/spec/build/private_callbacks.md). The bounded direct
  parameter route now reaches native artifact, final-address and installation
  replay; `callback_terminal_custody::direct_callback_relocation_resolves_to_its_private_function`
  is a success witness, not a fragment-import rejection.

  Remaining: multiple callbacks and layout-field destinations; authenticated
  complete plan applications and independent authored-use-to-Terminal-operation
  correspondence; ordinary callback bodies and inbound signatures through
  proposal validation, native lowering and image replay. Psi's
  `machine_lowering/bounded_callbacks.rs` already delegates ordinary machine
  lowering. The remaining exact single-`u64` identity/Unit recognizer is in
  `checked-compilation-to-terminal-artifact/src/native_proposal/mod.rs`;
  native thunk/image consumers also reject call-bearing bodies. Replace those
  shape restrictions with requirement, ABI and call-custody checking, not more
  admitted body families. Close hosted private-stack callback occupancy and
  ordinary-product admission restrictions only with their missing custody.

  Owners: native proposal construction, `native-realization`'s
  `retained_native_product` and `callback_thunks`, selected-call ABI transport,
  and image private-function/relocation replay. See
  [receiving custody limits](omega-rust/omega/compiler/native-realization/README.md#callback-custody-boundaries).
  Acceptance: the direct witness and
  `source/library/std/tests/callback_materialization_closure.omg` two-slot
  registrar produce native images binding exact function, symbol, relocation,
  executable region and destination through object/final replay. Missing,
  duplicate, reordered, substituted, overlapping or incompatible materializations
  reject. Private slots remain inaccessible to source. Registration outcome
  and lifetime belong to REGISTERED-CALLBACK-LIFETIME.

- **REGISTERED-CALLBACK-LIFETIME.** Complete an authored registrar lifecycle
  under [registration and lifetime](wiki/spec/build/private_callbacks.md#registration-and-lifetime)
  and [opaque retention](wiki/spec/build/component_publication.md#opaque-retention-and-quarantine).
  Owners: checked execution/result planning, Terminal result qualification and
  claim replay, installed-provider interpretation, and
  `component-publication/src/callback_registration.rs`.
  Routed-domain authorization of registration sum payloads, interpreted
  register/unregister ledger integration and installed-provider minting of bare
  linear result claims exist; do not reopen the resolved payload-authorization
  question.

  Carry the success/rejection sum through executable planning, case-conditional
  qualifications/claims and an authored provider/customer. The installed-provider
  path still rejects projected result qualifications and admits affine results
  only without claims/qualifications
  (`terminal-interpreter/src/terminal_interpreter/call_operations.rs`).
  Success must join the exact live-registration capacity occurrence to the
  external root and code/component leases; rejection preserves that capacity
  without a root. Teardown requires quiescence before lease release.
  Capacity counts live registrations, not emitted thunks; successful teardown
  returns the same capacity occurrence.
  Use ordinary custody, not registration-specific checker rules.

  Acceptance: one authored program rejects, retries, replaces using returned
  capacity, and unregisters. Dropped, repeated, stale/cross-occurrence
  registrations and spent-capacity reuse reject. Preserve
  `checked-trees-to-lowered-psi/tests/registered_callback_lifetime.rs` and
  the interpreted ledger tests, but manual ledger sequencing or injected
  provider-conformance rows do not close this authored acceptance.
  CALLBACK-PRIVATE-MATERIALIZATION supplies the native entry; FFIVAL owns
  the Windows foreign-invocation customer. Other hosts must report that leg
  unavailable rather than claim execution.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Connect authored retained-argument
  custody to executable native backing under
  [outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody).
  Owners: checked content/call planning, checked-to-lowered retention,
  Terminal boundary validation, native call-site marshaling, and
  `external-roots/src/program_local/program_local_extents/retained_foreign_arguments.rs`.

  Shared retained-borrow custody attaches to the invoked Terminal boundary;
  verification checks its exact shared source and retained result loan.
  `retain_foreign_argument_under_custody` selects shared lifetime retention
  from that row, but still has only test callers. Complete the authored
  source-to-Terminal-to-native call-site connection, materializing pointers
  only from established stable root/range/access/lifetime/revision-or-lease
  custody. Reuse the admitted result-`ensures`, nominal-match and structural-result
  catalog path rather than recreating those gates.

  Add the permitted-snapshot source/checked contract and persistent demand per
  live occurrence; copying must be explicitly allowed, without invented identity
  preservation or write-back. Connect moved backing through CONSERVATION-CONTRACT
  and TERMINAL-CONTENT-CLAIMS. Moved/snapshot registry helpers are test-only;
  promote a route only when authored custody selects it.

  Acceptance: authored moved, lifetime-borrowed and permitted-snapshot boundaries
  run natively. Completion redeems moved backing; a live shared loan excludes
  conflicting writes. Call-scoped, mutable lifetime-borrowed or ambiguous retention and
  unknown/stale/out-of-range/excess-rights backing reject. Preserve
  `retained_content_custody` and `retention/retained_borrow_custody` controls.
  Registration-specific lease custody remains REGISTERED-CALLBACK-LIFETIME.

## P5 - Cathedral over general Omega primitives

- **BUMP-ALLOCATOR-CANARY.** Deliver an executable package allocator over a
  qualified `Extent` under [allocation](wiki/spec/resources/allocation.md).
  Strategy stays ordinary Omega source. Reuse `source/library/alloc/bump.omg`
  and the `bump_allocator_canary` package-dependency fixture: the package exists,
  but `Main::main` is empty, no storage provider is selected, and its `BumpVec`
  exercise carries capacity rather than elements.

  Complete these connected parts:

  - Connect `ExtentPartition` to the invoked partition/conservation route
    owned by CONSERVATION-CONTRACT and TERMINAL-CONTENT-CLAIMS. Prove aligned
    request geometry and the counted-residual/tail relationship; returned
    length laws alone do not establish that a request fits. Preserve exact
    alternatives, caller substitution and independent custody reconstruction.
  - Replace fixture-local `ResidentStorage` with PLAN-LAID-VIEWS' evaluated
    layout/access, establishment, element transfer and retirement. A resident
    marker is not establishment; live residents cannot merge as vacant.
  - Implement temporary exclusive strategy borrowing through
    BORROWED-STORAGE-RESTORATION, ending at allocation return so allocations
    coexist without a permanently borrowed compiler arena.
  - Implement explicit retired storage, traversal and full recomposition.
    `RetiredSlot` holds only one old buffer. Current `release`/`shrink`
    restore the allocatable tail immediately; that specialized reuse behavior
    is not the specified monotonic release, which retains returned extents
    until reset. Preserve the contract rather than treating tail-adjacent return
    as the general return implementation.
  - Supply real backing and execute the package in the interpreter and on a
    supported native host; record unavailable hosts separately.
    VEC-NATIVE-GROWTH consumes this allocator and owns
    the element-bearing container customer.

  Acceptance: coexisting allocations, unchanged state on failed requests,
  exact cleanup/return, and reset to original backing only after every allocation
  and resident returns. Live reset, dropped custody, double placement, wrong
  resident indices and unrouted introduction reject. Preserve the
  `fail/memory/bump_allocator_*` and live-resident controls. An empty entry,
  finite reservation exercise or checked-only pass is not executable acceptance.

- **ADDRESS-TRANSLATION-CANARY.** Execute Cathedral-owned mapping
  installation and teardown under
  [mapping and reclamation](wiki/spec/resources/extents.md#mapping-and-reclamation).
  The corpus stand-in at `tests/omega/pass/memory/address_translation_canary`
  declares `CathedralTranslationProvider` requirement coverage and threads
  the owned `Mapping` through table install/remove before activation/release.
  It remains checked-only: `Main::main` is empty and bodyless hardware/receipt
  crossings have no selected executable provider joining the authored
  obligations to `extents::mapping` receipts.

  Finish that provider/receipt connection for exact source/destination custody,
  mapping identity and era, including borrowed-source carrier transport.
  Carried-loan mutation ceilings exist; borrowed install/remove routes retaining
  return/field qualifications remain missing. Reproduce their current diagnostic
  rather than assuming the fixture header's broad carried-loan blocker.
  Extend the one-leaf routes to multi-page mappings with ordinary ranked loops.
  TERMINATION-RANKING-CHECKS owns contract-aware boundary-call preservation;
  nested checked-body calls in inert initializers are already supported.
  Demand-grown tables depend on BUMP-ALLOCATOR-CANARY, placed read-back on
  PLAN-LAID-VIEWS, and fixture range migration on REMOVE-BRACKETED-RANGE-ANNOTATIONS.

  Validate target-correct Cathedral entry encoding and geometry before execution:
  `cathedral/tables.omg::FRAME_MASK` is currently `2^52`, not the documented
  `2^52 - 2^12`, so it discards ordinary frame-address bits. Select authority,
  hardware and shootdown realizations on a freestanding image through
  UEFI-PHYSICAL-SEMANTIC-ENTRY and UEFI-OS-HANDOFF, with a QEMU harness.

  Acceptance: install/tear down multi-page mappings in QEMU with explicit
  `Extent` and TLB custody. Mapped access exists only after activation and
  before unmap; reuse waits for completion. Source use after map, premature
  unmap, lost shootdown, forged carriers, borrowed-source reclaim, replayed
  activation and wrong-mapping/stale-era receipts reject. Preserve the eight
  `fail/core/translation_*` controls. A checked fixture or Rust receipt test
  is not execution. Cathedral owns the real package, formats, walk policy and
  lifecycle; no page-table/TLB types belong in the compiler.

- **EXCEPTION-ROOTS-AND-TIMER.** Execute Cathedral's fatal entries on
  dedicated critical stacks and a timer handler that acknowledges, records
  and wakes ordinary work, under [interrupt obligations](wiki/spec/build/interrupt_obligations.md),
  [hardware materialization](wiki/spec/build/hardware_materialization.md)
  and [final machine-state evidence](wiki/spec/build/machine_state_evidence.md).
  Cathedral owns exception coverage, gate/IST policy, controller/device
  protocols, table validation and fatal behavior. Compiler mechanisms own
  sealed installed roots, checked writers/instructions, entry/exit realization
  and custody.

  Reuse `tests/omega/pass/memory/interrupt_table_canary` and
  `compiler/tests/layout_plans/interrupt_descriptor_tables.rs`: authored
  root selection, table layout, member admission and validation exist, as do
  stub emission/replay with GPR/XMM saves, stack fragments and indirect copies.
  The source entry is empty; tests supply installed bytes, authority and
  resource receipts. Remaining:

  - Join `machine-emission` stub bytes and member-call relocation to real image
    text, installation and external-root admission. Bind exact entry/body
    coordinates, installed bytes, stack overhead and final transitive state
    evidence; retain separate stack, fuel and state columns.
    `X86_64DeriverStubEntryEmission` currently has only test producers.
    Unsupported ABI/backing must reject, not fabricate borrowed caller storage.
  - Complete authored table publication and remove compiler-owned table-policy
    fields/checks from `InterruptTableGateDescriptor`,
    `InterruptTableProfile` and established/publication records. Reuse the
    package validator/member verdict and generic writer; retain exact content,
    installed-root, publication-authority and checked-`lidt` joins. Root custody
    and the instruction contract remain compiler-owned.
  - Replace acknowledgement-only fixture bodies with Cathedral's fatal halt,
    controller/timer setup, tick record and wake behavior. Join completion to
    exact acknowledgement lineage through BOUNDED-INSTALLATION-REACH-ROWS and
    TOP-LEVEL-BOUNDARY-REQUIREMENTS. Shared inherited requirement identities
    must keep resolving under their own selected provider plan.
  - Integrate UEFI-PHYSICAL-SEMANTIC-ENTRY / UEFI-OS-HANDOFF, TR3-TR8 stack
    provisioning, and a reproducible QEMU harness. Keep external interrupts
    disabled until the exception floor is installed; NMI, machine-check and
    physical-failure assumptions remain explicit.

  Acceptance: QEMU boots the authored customer, reports timer ticks through
  owned output, halts between ticks, and sends a deliberately raised fault to
  its fatal handler on the dedicated stack without resuming ordinary work.
  Missing members, wrong writer/realization or changed table bytes, replayed
  publication/acknowledgement, forgotten/double completion, and retirement while
  the table names the root reject. Ledger tests and uninstalled stub bytes are
  not the witness. Record emulator/firmware prerequisites and unavailable hosts.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Finish
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach)
  for component contracts and opaque-carrier completion. Selection and nested
  closure substitution already exist in `provider-planning/installation_reach.rs`
  and `external-roots/src/root_entry/root_validation.rs`; preserve the selected
  nested-row and unresolved-row controls in `calling_policy_plans/opaque_boundaries.rs`.

  The shipped `InterruptAcknowledgement::complete` in `core/interrupt.omg`
  still declares fixed `reaches PortIo`, and its test pins that limitation.
  Migrate it to the ratified bound beneath `MachineControl + PortIo` together
  with a selected satisfier that discharges the linear receiver and retains
  exact provider execution, policy and token lineage. TOP-LEVEL-BOUNDARY-REQUIREMENTS
  owns source requirement selection/discharge; owned receiver dispatch already
  exists, so do not recreate it. A test-local copyable lookalike does not close
  the real linear-carrier route.
  COMPONENT-SUBSTRATE supplies the admitted component contract needed to reject
  unresolved exported rows. This item owns the bound/substitution integration,
  not another component implementation.

  Acceptance: the shipped core requirement resolves PIC completion to `PortIo`
  and LAPIC/x2APIC completion to `MachineControl` through source, Terminal Psi
  and `InstalledInterruptCompletionRoute`. Cross-provider settlement despite
  equal rows, replayed token/era, an Independent component with an unresolved
  export, and final admission with any unresolved row reject. Conservative bounds
  grant no authority; exact selected execution and lineage authorize invocation.

## Parallel language and compiler lanes

- **BORROWED-STORAGE-RESTORATION.** (split-of:OMEGA-PRODUCT-COMPILER-SOURCE)
  Complete consuming-transform/replacement execution under
  [borrowed-storage invariant windows](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
  and [Terminal restoration](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).
  The guide move-out/restore body already produces checked Move/Store rows and
  verified Terminal operations, including reference-local aliases. Reuse
  `execution/unit/borrowed_windows.rs`, emission's `BorrowedWindowLedger`
  and Omega's `lowering/machine/operation/borrowed_windows.rs`.

  Replace the producer's leading move-local/restore-only recognizer with
  ordinary operation sequencing and explicit place/value custody. Connect
  consuming calls and replacement values, disjoint sibling work and multi-block
  joins. Independently reconstruct equal open-place frontiers and closure at
  required exits, retaining evaluation order, nested/live-arm agreement,
  contained loans and outcome obligations. Whole-root, indexed, referent,
  scalar-field and nominal-drop fences stay until their evidence exists.

  Complete target/native realization without replacing caller storage by a
  staged copy. Promote `ownership/move_keyword_field_assignment` from
  checked-only when its full-product route succeeds. Reobserve that command's
  current failure rather than retaining the retired entry-multiplicity blocker.
  The product parser and **BUMP-ALLOCATOR-CANARY** consume this mechanism.

  Acceptance: a consuming transform then replacement executes with
  caller-visible updated contents and exact-once custody in Terminal
  interpretation and supported native targets, including sibling work, repair
  on both branches and contained-loan transport. Missing repair, early return,
  stale reads, overlapping loans, wrong-place repair, repeated extraction and
  whole-owner cleanup reject; tampered artifacts fail independent replay.
  Cover recoverable failure, suspension/resume/cancellation and crash/process-exit
  abandonment without invented rollback or survivor guarantees. Preserve the
  checker/interpreter `borrowed_restoration` suites, ledger controls and
  `fail/ownership/borrowed_storage_boundary_call`.

- **MATCH-SELECTIVE-LOWERING.** Complete
  [value dispatch](wiki/spec/language/patterns.md) for owned/borrowed/linear
  results, structural/case/domain patterns and coverage. Owners: validation's
  `value_custody/expression_types/{match_dispatch,result_type}.rs`, checked
  value continuations, Terminal production and package-review contract/index
  projection. Preserve once-only subject evaluation, authored first-match
  order, selected-arm-only execution, exact result/loan origins and death edges.

  Remaining work:

  - Carry existing affine custody through branch-local calls/receivers and
    effectful subjects; emit projected record fields and fixed-index leaves
    through per-arm extraction and exact successor places. Resume from
    `value_dispatch/owned_results/{call_product_arms,folded_index_projection,linear_child_carriers}.rs`:
    checking record-child joins or literal/folded index equivalence does not
    supply leaf emission or claim-bearing body plans.
  - Complete native shared-reference joins. Direct borrowed locals and
    primitive shared joins already lower, verify and interpret. Omega's
    `block_bindings.rs` still limits borrowed block parameters to byte views;
    target `control_flow/transfers.rs` rejects projected borrowed edges,
    although projected owned edges now have a route. Repair both consumers,
    coordinating with **WRITE-ONLY-BORROW**, and observe both arms of
    `borrowed_results::PRIMITIVE_CALL_SOURCE` natively.
  - Complete exclusive and case-bearing borrowed joins under their own custody,
    and borrowed-subject successor lifecycle publication. Dynamic indexes,
    ranges, case members and computed borrowed roots need real place/loan
    evidence, not a wider unchecked carrier. Retain exact source-state loan
    closure; checking a borrowed tag does not publish its lifecycle.
  - Instantiate composite/constrained result shells and exact static arguments
    for indexed predicates/theorems and package memberships. Bare bound
    operator `-> T` results already retain the operand's exact type; unresolved
    dependent shells must not export declaration-local identities.
    Qualifications, routed membership, callable signatures and erasure need
    actual transport. **OPERATOR-MACHINE-SUPPLY** owns operation execution,
    **CRASH-CONTRACT** crash-qualified equality, and
    **STATE-LOCAL-VALUE-FRONTIER** numeric landing and sequencing.

  Generalize checked place/loan establishment across matches, locals and calls;
  do not expand an arm-expression roster or flatten branch ownership into a
  statement-wide move list. The old direct-borrow-local refusal is not a
  remaining limitation.

  Acceptance: close the `checked-trees-to-lowered-psi --test suite value_dispatch`
  gaps and native `scalar_case_results` /
  [float Match customers](omega-rust/omega/compiler/compiler/float_realization.md#operation-and-control-custody).
  Preserve effects, skipped trapping arms, overlapping patterns, full coverage,
  and independent replay. Retain `match_anonymous_result_landing`,
  `numeric_operand_destinations`, and `dutch_flag`'s native exit-70 oracle with
  explicit copyable swaps. Wrong qualifications, ownership, result types and
  incompatible arms reject before an outer bare-carrier cast. Checking or
  interpretation alone does not close native execution.

- **OPERATOR-MACHINE-SUPPLY.** Complete
  [declaration-owned executable supply](wiki/spec/language/expressions.md#executable-supply),
  [call preconditions](wiki/spec/language/machines.md#call-preconditions), and
  [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization)
  through independent Terminal replay and native execution, then retire the
  separate `operator` introducer.

  Remaining work:

  - Carry call-containing scalar requirements into Terminal without dropping
    premises. Resume from
    `contract_application_terms::runtime_body_calls_execute_with_checked_premises`:
    checked interpretation covers `restricted(saved, value)` under
    `observe(left) == observe(right)`, but the test stops before Terminal.
    Re-witness the recorded `scalar contract contains an unsupported clause`
    at `scalar_contracts::covered_requires`; require canonical reload,
    independent verification, interpretation and native agreement.
  - Extend call-premise formation across remaining contract owners and
    substitutions: abstract signature declarations, domain/default predicates,
    static callable/evidence arguments, receiver/projection terms and
    citation/induction case guarantees. Abstract **callee** attribution already
    exists in `specification_calls.rs::RequirementOwner::Signature`; that does
    not validate every call occurring **inside** an abstract contract.
    Establish premises before result formation or guarantee intake, including
    erased/discarded uses; recursion separately requires descent. Preserve
    exact subject/argument identity, complete constructor values versus tag-only
    evidence, and public-contract boundaries. Unsupported evidence is not proof.
  - Complete selected token-body supply outside the admitted expression routes,
    including open-ended ranges and match-pattern equality. Reproduce the
    build-machine refusal before assigning its repair. Reuse ordinary calls,
    attached receiver loans and once-only operand evaluation; no separate
    operator interpreter or fallback arithmetic. These refusals are
    implementation boundaries, not new semantic prohibitions.
  - Finish per-law normalization coverage atop the explicit selected-conformance
    route in `open_index_expressions.rs`. It currently records an algebra only
    when both commutativity and associativity slots exist; reordering must need
    only its commutativity law, while reassociation needs associativity.
    Preserve missing/ambiguous selection, wrong-operation and unproved-law
    controls. Noncommutative operations remain usable without rewrites; AC
    grants neither zero identity nor integer-addition meaning.
  - Execute `expressions/declared_operator_match_result` and
    `expressions/token_bound_machine_operand_selection` natively; both remain
    checked-only. Coordinate local-data argument custody with
    **STATE-LOCAL-VALUE-FRONTIER** and owned selective execution with
    **MATCH-SELECTIVE-LOWERING**, retaining the checked/lowered selected-call joins.
  - Migrate library/corpus/embedded-test declarations and remove the old parser
    and representation consumers. Use the
    [retirement inventory](wiki/drafts/operator_introducer_retirement_inventory.md)
    for navigation, not as a current census. Tokenless boundary rows depend on
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS**, including generic requirements.
    Trait/domain token signatures need ordinary `machine` grammar: the trait
    parser still reads token spelling only after `operator`. Rejoin
    `SpelledOperator`, provider planning, selected build-time execution,
    package evidence and result-domain dispatch to the surviving declarations.
    Preserve semantic identities or explicitly reject stale schemas.
    Nat declaration-owned bodies, closed-family package ownership and attached
    index receiver adaptation already exist.

  Acceptance: the selected true-arm wrapped `250 + 10` and named call produce
  `260u64`; the false arm produces `1` without invoking the operation. Checked
  execution, reloaded/independently verified Terminal and native execution agree.
  Cover generic/stateful bodies, private helpers behind public declarations,
  qualifiers, ordered once-only operands, receiver/result loans and ordinary
  contract rejection. Missing body, bodyless-plus-satisfier supply, duplicate
  owner shapes, foreign primitive-family injection and forged primitive identity
  reject; unrelated imports/conformances cannot alter selection or collide.
  Call premises apply in runtime, admitted compile-time evaluation and proof
  term formation alike. Nat named/token calls require order even in
  erased/discarded proof terms;
  equal operands and predecessor at one accept, while missing premises and
  missing recursive descent reject independently. Operation, conformance/law
  and call/loan identities survive substitution and replay. Trait-selected
  supply, **FLOAT-PROVIDERS** and canonical compiler primitives keep their
  separate routes.
- **MODULE-NAMESPACE-RESOLUTION.** Carry the exact selections required by the
  [module/name contract](wiki/spec/language/modules.md) through checking,
  constant evaluation and source-independent artifacts. Owners:
  `syntax-trees-to-symbol-resolved-trees/src/preparation/`
  (`module_normalization.rs`, `generic_data/`), validation's
  `proof_contracts/domains.rs`, and build-time evaluation's
  `const_evaluation/const_initializers/`. Module normalization and
  predicate-domain local-initializer checking already exist; resolution-only
  tests do not establish downstream execution.

  Remaining work:

  - Finish typed/checked/Terminal transport for generic type-scoped constant
    attachments, indexed constraints, operator homes and declared-domain case
    facts. Preserve exact lexical/package selection through specialization,
    same-leaf competitors, file-local imports and private/transitive exposure.
    `module_namespace_residuals` contains resolver-only operator/case tests;
    extend those customers through their remaining consumers rather than adding
    another namespace recognizer.
  - Supply portable package identity for unmanaged roots before permitting
    equal module/domain paths across them. Keep `domains.rs`'s independent
    collision rejection until then; host paths and source-order numbers are
    not portable identity.
  - Complete declaration evaluation, including unused initializers:
    specialized provider/target applications, authored NaN identity bits, and
    constrained constants beyond scalar-decodable record/array/case leaves.
    Whole-aggregate subjects, open applications, carrier-property constraints
    on abstract `self`, alias-owned bounds/index tuples and compiler-owned
    alias atoms need exact typed application evidence. Routed constraints still
    require establishment. Reuse typed evaluation and retain selected operator
    meaning and widths; empty predicate replay or a computed payload is not
    qualification evidence (`generic_data/const_evaluation/facts.rs`).
  - Extend checked invocation admission beyond canonical scalar snapshots and
    its established lossless-widening/proven-index routes. Value-changing casts,
    call-produced projection subjects and other undecidable origins must keep
    conservative crash ceilings until checked evidence discharges them.
    Coordinate `facts/crash_entry_values.rs` and ordinary call-premise transport
    with **CRASH-CONTRACT** and **OPERATOR-MACHINE-SUPPLY**; successful
    interpretation or provider-body inspection is not admission evidence.
    Further argument conversions need exact live-premise transport; lossless
    widening and signed nonzero forwarding are working controls, including
    `signed_nonzero_call_preconditions_execute_after_source_removal`.

  Runtime qualified-record reads and general array-value execution belong to
  **STATE-LOCAL-VALUE-FRONTIER** / **TR3-TR8** and target
  `lowering/control_flow/aggregate_results.rs`. The
  `generic_carrier_qualified_fields_keep_their_owner_after_specialization`
  test distinguishes its checked runtime `Holder<T>` customer from the
  source-free constant projection that already works. Do not substitute an
  aggregate's selected home for its unevaluated contents.

  Acceptance: source-free publication, independent verification and matching-host
  execution in `compiler --test module_machine_indices` (especially
  `nominal`, `constant_attachments`, `value_dispatch`, `machine_initializers`),
  `compiler --test constant_float_tables`,
  `terminal-psi-to-abstract-operations --test scalar_array_construction`, and
  native-differential `scalar_array_results` / `scalar_case_results`.
  Preserve exact floating bits including signed zero, mixed field/index
  projections, selected arithmetic policies, and all-arm obligations without
  evaluating skipped subjects. Keep zero/wrapping-to-zero, stale operand,
  wrong-owner, false-precondition, invalid-unused-initializer and unproved-index
  rejection, including `predicate_domain_initializer_*`. The
  `qualified_declarations`, `qualified_constants`, `match_constant_indices`,
  `nominal_constant_bodies` and `module_array_constant_indices` customers
  remain; runtime-index fail fixtures change only when their materialization
  obligations are met.

- **RUNTIME-VALUE-GENERICS.** Complete
  [runtime-capable value binders](wiki/spec/language/generics.md#value-binders-and-const-requirements)
  and [captured index identity](wiki/spec/language/generics.md#runtime-index-identity-and-storage)
  for result and stored qualifications. Machine value binders already share
  one dynamic body with ordinary arguments and captured-subject/guard transport.
  Data headers parse value binders, and a checked descriptor-field slice
  enforces construction/default-domain and later write obligations.

  Remaining work:

  - Preserve exact runtime application subjects through data construction,
    parameters/results, stored qualifications and artifact publication.
    `runtime_value_generics::data_value_binder_arguments_carry_no_static_identity`
    currently expects `Index<9>` to assign into `Index<7>` without relating
    those arguments. Shared layout/code does not prove captured indices agree.
    Correct application compatibility, bind construction arguments to their
    actual subjects, and retain executable indices as ordinary data, arguments
    or descriptors. Erase proof-only uses only after checking their obligations;
    `lowering/data.rs` currently synthesizes ordinary fields unconditionally.
  - Complete domain/index qualification transport with exact value versions and
    static-only gates. Migrate legacy range-shell fixtures through
    **REMOVE-BRACKETED-RANGE-ANNOTATIONS**, without extending revoked syntax or
    counting those tests as general domain-index support. Preserve forwarding,
    reassignment and live/stale guard/equality controls. Runtime inline-array
    extents and const positions remain rejected.
  - Carry module-owned forms through exact lexical/package selection with
    **MODULE-NAMESPACE-RESOLUTION**. Runtime values are never static cache keys.
  - Execute the value-indexed data/storage customer through verified Terminal,
    interpretation and native ordinary call/storage routes. Coordinate
    record-local/provider composition with **STATE-LOCAL-VALUE-FRONTIER** and
    binding establishment with **ENTRY-CONTENT-ROOTS**. Native fixtures still
    spell `Service<Console>`; migrate to ratified `Binding<R>`. Their native
    test module runs only on macOS ARM64; report unavailable hosts separately.

  Acceptance: `<Count: u32>` accepts static/runtime arguments under proved
  obligations, while `<const Count: u32>` remains static. One dynamic machine
  body handles distinct counts. Parameter/result qualifications and an actual
  value-indexed data field retain the captured subject after source reassignment;
  equality permits only the relationships it proves. Invalid construction,
  bounds, stale/mismatched indices, unsupported static-only uses and duplicated/
  lost linear custody reject in source checking and independent artifact replay.
  Preserve `runtime_value_generics`' shared-body, domain-index forwarding and
  guard/equality controls; its descriptor-field tests currently check source,
  not execution, and literal array-index tests are not value-indexed types.
  Exercise interpreter and native behavior for each supported slice. No hidden
  boxing, maximum-range stack reservation, runtime per-value code generation or
  specialization inferred from integer finiteness. Begin with fixed
  representation; **FINITE-GENERIC-DISPATCH** separately owns finite families.

- **STRUCTURAL-GENERIC-MATCHING.** Complete
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference)
  and [canonical domain indices](wiki/spec/language/generics.md#canonical-domain-index-matching)
  through specialization, layout and source-free artifacts. Reuse
  `preparation/type_equations/`, `machine_equations.rs` and canonical argument
  identity; no second evaluator or generic-specific storage plan.
  **REMOVE-BRACKETED-RANGE-ANNOTATIONS** owns retiring range-shell inference.

  Remaining work:

  - Check unspecialized machine/requirement bodies under every admitted
    type-equality alternative, including finite disjunctions and branch-local
    equalities that cannot escape joins. Closed case-equation decisions do
    not establish this general route.
  - Finish domain-application identity after matching. Declared domain heads
    and constrained spellings already match; constrained indexed domains also
    have `ClosedConstraintIdentity::IndexedDeclaration`. Join both spellings
    to exact declaration/carrier/index identity, including package selection
    and generic carriers. `generic_data/arguments.rs` still takes a data-only
    identity route for generic heads and rejects generic-carrier domain identity.
    Re-drive `domain_application_arguments_reach_instance_identity_after_binding`:
    its expectation that both spellings fail predates constrained identity
    support. Do not present that stale pin as a newly observed failure.
    Distinct domain indices gain no implicit variance; compatibility evidence
    does not rename their identities.
  - Support selected named-lifetime identity and remaining const-index kinds;
    anonymous reference/slice and integer/Boolean application matching already
    exist. **WRITE-ONLY-BORROW** owns write-only type admission.
  - Combine equations with argument/result inference instead of requiring
    closed explicit machine tuples. Preserve obligations through forwarding,
    retained compilation extensions, late-selected receivers, operator/
    conformance supply and machine/evidence/value binders. Keep rejection of
    undisclosed equations until those routes can discharge them; selection
    during typing must not bypass the equation check.
  - Evaluate computed const arguments through ordinary semantic evaluation and
    invocation admission, retaining declared carrier, arithmetic policy,
    selected operation and canonical identity. Match an open index as a whole;
    no arbitrary equation solving, predicate maximum search or flow-derived
    capacities. Share normalization across equality, matching, layout and
    artifact readers, deduplicating equivalent closed tuples.
    **ARITHMETIC-POLICY-REALIZATION** owns trapping-call exclusion;
    **RUNTIME-VALUE-GENERICS** owns runtime subjects, not const-folding them.

  Acceptance: the spec's TinyBytes infers Capacity from `u64::AtMost<256>`
  before layout; equivalent closed computations select one instance. Data/
  machine array and application equations infer omitted arguments while
  preserving all obligations. Repeated/explicit conflicts, missing indices,
  occurs cycles, kind mismatches, false constructor predicates and unmet
  conformances reject. Flow narrowing cannot alter layout or domain identity.
  Extend `array_type_equations`, `machine_type_equations` and
  `application_type_equations` through source-free Terminal/native checking.

  Preserve the domain-index `Bytes` migration in
  `generics/omitted_data_binder_range_equation`, its nested backing array and
  invalid-initializer controls; migrate its remaining range-shell half under
  that owner. The current `constructed_value` only casts a scalar qualification.
  Complete a customer actually constructing and accessing the nested backing;
  **STATE-LOCAL-VALUE-FRONTIER** owns that storage and borrowed-local mutation,
  not a generic-specific plan. Const staging, initialization and stack supply
  remain separate obligations.

- **FINITE-GENERIC-DISPATCH.** Complete runtime selection under the
  [finite specialization contract](wiki/spec/language/generics.md#finite-specialization-boundary)
  and [dynamic method families](wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
  for scanner widths and runtime-selected datatype providers.
  `TypedTrees::finite_signature_family` owns the roster;
  `monomorphization/dynamic_families.rs` specializes selected providers.
  Static tuple selection, correlated multi-binder rosters, boundary family
  coverage and Terminal tuple identities already exist.

  Remaining work:

  - In `execution/unit/dynamic_scalar_calls/`, replace the static-only
    `dynamic_family_tuple` path for runtime-capable calls with checked roster
    membership and generated selection among the closed bodies. Begin with
    one scalar binder, one common concrete result and dispatch around a
    region-sized operation. Reuse **RUNTIME-VALUE-GENERICS**' existing scalar
    argument/guard machinery; its unfinished general value-indexed data support
    is not a blanket prerequisite. Unproved membership rejects; no JIT or
    invented fallback.
  - Carry a source-produced family through native tables and independent image
    replay, using **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**'s ordinary
    indirect-call mechanism. The rebound `CallDynamicScalar` route still lacks
    common-graph legalization. Parameter descriptor calls have legalization
    and replay but still lack selected instruction construction; do not conflate
    those stages. Resume from `dynamic_composed_unit/finite_family.rs` and
    `REBOUND_FAMILY_DYNAMIC_INTEGER_SOURCE`, replacing its ignored-width,
    discarded-result probe with observed distinct tuple results and the actual
    rebound selected instance. Test-constructed tuple rows do not close this.

  Acceptance: source pass/fail/run cases dispatch widths 16/32/64 from a runtime
  value through one selected conformance and execute natively without
  handwritten suffix methods. Alternative order and duplicates normalize
  deterministically; short explicit tuple sets retain correlations rather than
  enumerating ranges or cross-products. Missing/wrong-width/mixed-provider/
  shape-substituted rows, unproved membership, target-ineligible bodies and
  const-only dynamic inputs without a checked bridge reject. Preserve effects,
  index identity and once-only moves/cleanup through selection, forwarding,
  storage and independent artifact/image replay. Escaping variable-shaped
  results require an authored sum or eligible descriptor, not implicit boxing.
  Record compile/code-size and scan-loop dispatch placement before claiming an
  improvement over explicit branches. No general generic virtual-method or
  reflection extension belongs here.

- **DOMAIN-ISSUER-ROUTES.** Finish source-driven, independently replayed
  qualification establishment for
  [requirement/exact-machine routes](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
  and [private issuer catalogs](wiki/spec/resources/authority.md#private-issuer-routes).
  Structural route catalogs, result bindings, codec and present-binding validation
  exist; scalar catalogs and issuer checks on qualification introduction exist too.
  Do not rebuild them or treat package-row recovery as artifact acceptance.

  The remaining ordinary-call producer gap is in
  `checked-trees-to-lowered-psi/src/unit/attached_unit.rs` and its `catalog.rs`:
  ordinary attached units publish no conformance applications, and
  `call_result_qualification_establishments` emits only boundary-requirement
  bindings backed by an exact `CallEnsures` admitted receipt. Retain the
  missing ordinary callee/conformance identities and connect checked establishment
  evidence to its exact invocation result. Establish a source-produced qualified
  boundary-result roundtrip too; manually assembled Terminal fixtures do not
  establish source admission.

  Acceptance: source examples serialize, reload without source and independently
  validate requirement, exact-machine and boundary establishment, including
  private issuers and authorized selected payloads. Prove carrier, predicate
  and custody obligations without assuming the introduced qualification.
  Reject forged, missing or rebound establishment evidence, substituted
  same-spelled/module/package issuers, and direct calls borrowing admitted
  requirement authority. The verifier currently skips empty establishment lists;
  validating present rows alone does not prove evidence completeness.

  Preserve private owner/declaration identity and dependencies without granting
  consumer call/conformance/private-type access or hiding admissions; public
  ordinary requirements must still admit valid downstream conformers. Keep
  resource-capacity and classification-specific boundary restrictions.
  Extend `terminal-verifier/tests/calls/qualification_establishments.rs`
  and `tests/straight_line/scalar_qualifications.rs` with source-produced
  evidence. Retain `package-evidence --test suite` public-domain/module-namespace
  and compiler `package_compilation_inputs` private-catalog controls.
  Reuse the existing issuer surfaces rather than parallel metadata.

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Complete ordinary package review,
  retained permissions, selected-mechanism classification and explicit receiver
  admission under the
  [production/admission split](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission)
  and [filesystem policy](wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority).
  No receiving permission policy means no admission claim, not denial or
  approval. Project/package acceptance must not manufacture receiver approval.

  The optional receiver-policy path and its acceptance/rejection controls exist.
  Filesystem cohort emitters also have production callers:
  `providers/settlements/source_imports.rs` emits mechanism classifications;
  `packages/manager/src/review/candidate/semantic_bindings.rs` attaches
  consumer permission rows. The merge in `native_realization/providers/mod.rs`
  is a **mechanism-classification** policy, separate from the optional receiver
  permission policy; preserve both exact identities.

  Remaining delivery:

  - Connect the bounded Linux filesystem plan to native settlement transport.
    `build-evaluation/src/provider_settlement/canonical_filesystem_host.rs`
    already mints accepted-schema plans for supported positional syscalls.
    `compiler/tests/terminal_authority/filesystem_cohort_witness.rs` calls
    `set_len` and now pins `MissingBoundarySettlement`, not zero provider rows.
    Complete that native realization path without guessing unsupported target,
    path/slice adaptation or host-error mechanisms. General provider planning
    is not the missing implementation for this witness.
  - Drive console-exit-app, Cathedral native smoke, `cli_mvp` and Squalr through
    ordinary package acceptance to native production without receiver-policy
    input. Coordinate their application owners, preserving existing host
    successes and reporting actual remaining blockers. The CLI
    `console_exit_permission` test currently accepts either nonempty output
    or `Selection(Legalization(SourceCustodyMismatch))`; that green test is not
    evidence of delivered native output. Resume at the physical legalization
    owner if that rejection persists.
  - Replace the broad filesystem review summary with exact classes through
    discovery, retained acceptance, classification and admission. Then remove
    the `FilesystemHostService` arm of
    `packages/review/evidence/src/capture/authority.rs::dangerous_authority_class`,
    dependent triage in `packages/manager/src/review/audit/triage/`, and the
    transitional-row assertion in `standard_library_package_resolution.rs`.
    **FILESYSTEM-RELEASE-CONTRACT** owns occurrence-bound release evidence;
    generic close must not inherit an empty classification. Build filesystem
    observations are not shipped-program lifecycle evidence.

  Acceptance: all four customers produce without receiving input and make no
  admission claim. The same demanding program rejects explicit denying input
  and admits independently supplied sufficient input, retaining its exact
  identity, including surplus undemanded rows. Every demanded leaf has one
  exact mechanism/contract classification fitting service permissions;
  unknown, conflicting/duplicate, cyclic or substituted classifications,
  stale evidence and violated physical exclusions reject. Preserve explicit
  empty classifications, service reach and exact review identity. No mandatory
  PCC request or synthesized receiver-policy file.

- **FILESYSTEM-RELEASE-CONTRACT.** Connect the program's checked open/query/close
  derivation to native authority classification under the
  [bounded occurrence-specific release contract](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof).
  Successful acquisition must establish the exact object/argument contract;
  every intervening call must preserve the handle and aliases; applicable
  returning paths release once with no later use. Query-only or empty authority
  is not a preservation contract, and build execution proves nothing about the
  shipped program's lifecycle.

  The native policy helpers in
  `native-realization/src/native_realization/terminal_authority_policy/filesystem.rs`
  already bind an occurrence commitment into syscall/foreign-import mechanism
  identities and emit a constrained empty row, but only tests call them.
  Implement the checked-flow derivation, retained Terminal evidence and independent
  validation, then rejoin that exact occurrence at realization. Hashing an asserted
  proof is not checking it. Changed arguments, flow, calls or contracts require
  re-establishment; absent proof uses only a justified conservative classification
  or rejects, never an invented empty row.

  Drive `tests/omega/pass/filesystem/windows_canonicalize_exit` through native
  execution (expected exit 70 on Windows), including its failed-open control.
  Its stored `UnitResult` customer still needs general structural sum
  replacement, borrowed case observation, whole nominal receiver replacement
  (including match-produced assignments), and nested layout/custody through
  **STATE-LOCAL-VALUE-FRONTIER** / **NOMINAL-FIELD-FLOW**. The scalar-store path
  does not provide this; existing `StoreStructuralField` repairs an opened
  borrowed hole, not arbitrary nominal replacement. Resume prerequisite work
  with that full customer plan, not another isolated source-shape recognizer.
  The earlier entry-attachment/fused-provider diagnostics at `069276b986dc`
  have not been rerun; re-drive the fixture before treating them as current.
  Migrate its remaining `Service<R>` spelling with **BINDING-CARRIER-NAME**.

  Acceptance: constrained close earns one evidence-bound empty row; failed
  acquisition, escape, reused aliases, second close, invalidating calls and
  stale/substituted evidence reject narrowing. Preserve Windows shared-delete
  behavior: another actor's pending deletion may complete on ordinary close,
  but deletion attached to this handle excludes the empty classification.
  `filesystem/native_close` closes an arbitrary integer, so retain it as an
  unconstrained control, not proof of a successful acquisition. General owned
  handles are not a prerequisite. **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns
  permission/review rows and the broad filesystem-summary replacement.

- **R5.** Complete compositional caller-visible may-write inference in
  `validation/src/machine_calls/calls/write_frames.rs` and its subordinate
  origin/fixpoint owners. A complete frame must name every possible write;
  unknown origins remain opaque and invalidate affected facts. This supplies
  the exact preservation checks required by
  [state contracts](wiki/spec/language/state_contracts.md) and
  [progress lineage](wiki/spec/language/termination.md#subject-preservation).

  Candidate-origin unions, computed call/match receivers, named-state transfer,
  aggregate helper-result projections, stored leaf rebinds and bound-reference
  wire arguments already have implementations and tests. Reuse those paths,
  rather than treating each source arrangement as another unsupported feature.
  Remaining concrete joins:

  - `demand.rs` still rejects a receiver mentioning a divergent local before
    contextual substitution, despite supporting a directly computed receiver.
    Carry the proven finite candidate set through the same receiver relation;
    retain opacity for unknown callees, private escapes and unproved rebinds.
  - `demand.rs` and `state_write_walk.rs` reject wire arguments mentioning a
    divergent alias before `wire_codecs.rs` can resolve its candidate set. Remove that
    mismatch by carrying checked argument origins through the caller and
    synthesized-codec frame. Member/indexed reference loads still need their
    own load evidence; the enclosing carrier's path is not the referent.
  - Carry the receiver/codec repairs through contextual-case, named-state and
    aggregate-result composition, preserving existing `write_frame_*` controls.
    Add full source-checking witnesses where coverage stops at typed-tree frame
    inference; retain conservative rejection at reference boundaries lacking
    independent origin/load evidence and unsupported recursive result routes.

  Architectural repair: share candidate-origin propagation and fixed-point
  composition across expressions, bindings, arguments and results. Do not add
  another recognizer per reference spelling or replace a complete-or-opaque
  summary with a guessed write set.

  Acceptance: extend `typed-trees-to-checked-trees/src/tests/termination/`
  `write_frame_*` coverage through full source checking, not only typed-tree
  resolver probes. A helper choosing either mutable input, writing through its
  result in a value expression, must preserve a disjoint index-bound fact and
  invalidate the overlapping one. **MATCH-SELECTIVE-LOWERING** owns remaining
  reference-branch custody joins. The old multi-input lifetime-signature
  blocker is no longer general: `borrow/view_link.rs` now maps explicit
  output lifetimes to the union of matching inputs, including bodyless
  signatures. Reuse that relation; frame-only success does not establish
  full source admission or executable result transport.
  Preserve transition-cycle convergence on
  `omega --check source/psi/gates/parser/main.omg` and the `cli_mvp`/
  `nqueens` frame controls; finite permutations must not become opaque merely
  because a state parameter changes position.

- **TPR6.** Finish exact subject-bearing progress-premise normalization through
  exported bodies, provider plans, recursive calls and artifact evidence under
  [subject preservation](wiki/spec/language/termination.md#subject-preservation).
  Private ranking witnesses remain outside public identity; a qualification
  or similarly shaped row cannot mint a premise.

  Extend the shared backward provenance trace and finite lineage partitions in
  `typed-trees-to-checked-trees/src/checks/termination/progress/`
  (`origins.rs`, `lineage.rs`, `lineage/places.rs`) and `flow/value_origins.rs`.
  Owned loads through references and exact written callee projections already
  recover their actual replacement input. Remaining gaps are additional reference
  boundaries lacking independent load evidence, unresolved control-flow result
  routes, generic/dispatched callees and dynamic/unresolved projections.
  Start with the unproven controls in `progress/origins/tests.rs`:
  `control_flow_route_helper_result_stays_unproven`,
  `dynamic_index_carrier_argument_stays_unproven` and
  `embedded_call_written_on_the_demanded_path_has_no_exact_origin`.
  Admit finite cases from exact provenance; unknown writes, aliases or routes
  retain no guarantee. A may-write frame does not identify replacement contents,
  and aggregate root correspondence cannot resurrect overwritten field evidence.
  Generic field traversal is not the same gap as generic callee reasoning.

  Complete compositional nested structural-result operands through the shared
  checked/lowered evaluation path with **STATE-LOCAL-VALUE-FRONTIER**.
  `checked-trees-to-lowered-psi/tests/reference_result_source.rs` already
  executes selected reference leaves, projected record arguments and
  `select(forward_outer(outer).inner)` from encoded Terminal evidence.
  The superficially similar rejection fixtures in `tests/borrow/carrier_results.rs`
  use inline constructors, not the supported earlier-local record route.
  Close those unchanged inline-construction cases and
  `select(forward_array(values)[0])`, including structural-element array
  ingress, residual carrier cleanup and recursive loan/qualification/linear-claim
  custody. Retire superseded shape gates in
  `validation/src/machine_calls/calls/expression_scanning/result_realization.rs`
  as real evaluation/result producers become available, rather than duplicating
  the producer's walk for each destination spelling.

  Preserve carrier locations, loan occurrences/parents and result homes. Move
  selected permissions; `EstablishReference` creates a child loan and cannot
  replace transferring a leaf whose parent would die at return. Relocate runtime
  descriptors without copying referents; preserve conservative lifetime unions
  and reject unsupported nested-reference host interfaces.
  Acceptance: projected calls evaluate once, retain result homes until consumption,
  and preserve selected loans and linear claims through source-free execution
  and independent replay. Every consumed progress premise must reconstruct its
  exact subject; missing, stale or substituted provenance grants none.
  Keep rejection fixtures until their complete custody is implemented.
  Terminal execution is not native acceptance.

- **NOMINAL-FIELD-FLOW.** Finish receiver default-field contract handling and
  its dungeon customer under
  [default domains and invariant windows](wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization).
  Owner: Psi `typed-trees-to-checked-trees` semantic field facts,
  `checks/contracts/`, and `flow/call_phases/referents.rs`.

  Receiver widening is blocked by
  [`mutable-self-receiver-declared-field-rows`](OWNER_QUESTIONS.md):
  current `&mut self` entry assumptions, return checks and call handback use
  only ZII-admitted `MachineFieldDomain` rows. Established non-ZII receiver
  facts otherwise survive through precise frames or authored contracts.
  Do not widen entry assumptions or handback independently of the owner ruling.
  Readable mutable arguments and whole-extent collection-field coverage already
  have a shared checked route; preserve it rather than rebuilding it.

  Re-drive `omega --check --target linux_x86_64 samples/cli/games/dungeon_crawler_cli/main.omg`,
  retaining `RoomLookup`, `MazeBuilder` and game-state field obligations.
  Coordinate remaining bracketed-range migration with
  **REMOVE-BRACKETED-RANGE-ANNOTATIONS** and any `RoomLookup` output migration
  with **WRITE-ONLY-BORROW**. Its output still spells `&mut Room`, but
  constrained-record `&write` admission is not wholly missing:
  `tests/contracts/nominal_parameter_fields.rs` positively checks declared
  nested invariants. Re-probe actual store/forwarding limits; ZII-valid local
  storage is not invalid merely because no explicit initializer was written.
  The historical `Filesystem::host` provider-join failure is not a current
  measured blocker; canonical plan minting now exists. Attribute any reproduced
  package/native stop to its owner, not field-fact machinery.

  Acceptance: apply the receiver ruling consistently at entry, call and return,
  then close the dungeon checking customer without bypassing its field
  obligations. Preserve indexing/view/copy/call transport and finite
  candidate-origin coverage in `tests/contracts/{element_fields,nominal_parameter_fields}.rs`;
  corrupted elements and stale aliases reject at calls, transitions and returns.
  Nominal annotations never resurrect retired facts, unresolved selectors
  never represent universal coverage, and arbitrary incoming storage gains
  no ZII facts. Preserve negative controls while migrating revoked syntax.

- **CML4.** Complete edge-local residual cleanup under the
  [ownership contract](wiki/spec/terminal-psi/ownership.md): outgoing values
  materialize before transfer and disposal, every occurrence has one
  disposition, and dying roots clean in reverse establishment order.
  Crash/abort/process-exit abandonment has no cleanup successor.
  Owners: `typed-trees-to-checked-trees/src/execution/control_cleanup.rs`
  and `execution/unit/cleanup/`, `checked-trees-to-lowered-psi/src/unit/unit_cleanup/`,
  and Omega's `abstract-operations-to-target-operations/src/lowering/`.

  Bounded parameter/local/result partial moves already reach encoded Terminal
  execution. Multiple ordinary or boundary-produced projected temporaries can
  share a Unit consumer (`partial_affine_result_source::anonymous_projected_operands_share_one_dying_continuation`).
  Native target lowering admits no-code residual actions on Jump and
  scalar/Unit-return edges; a lowering test is not native execution coverage.

  Remaining work:

  - Carry residual evidence through conditional and structural-case
    successors and structural returns. `SuccessorEdge`,
    `StructuralCaseSuccessorEdge` and `ReturnStructural` retain only
    whole-root discard lists; `ReturnUnitPartialAffine` already exists.
    Update the encoding contract and representation, producer, codec,
    independent verifier/interpreter and native consumers together.
  - Fix whole/residual scheduling across dying roots. The multi-temporary
    path in `execution/unit/cleanup/anonymous.rs::append_continuation`
    appends residuals in operand order, and the test above expects that
    order; it does not establish the required reverse-establishment schedule.
    Complete construction-local roots, partial construction and mixed dying
    roots, retaining maximal untouched subtrees and empty complements.
    No runtime liveness flags, expansion of untouched arrays into leaves, or
    cleanup deferred until final return.
  - Generalize projected temporary cleanup beyond empty, effect-free Unit
    consumers: compose other argument effects, non-Unit consumers,
    borrowed/owned temporary mixtures and retained claims/qualifications
    through **STATE-LOCAL-VALUE-FRONTIER**'s ordinary evaluation path.
    Each temporary retains its producer and exact dying continuation.
  - Close source-to-native storage and replay for result homes, projected
    results/copies, computed values across calls and cyclic control.
    `lowering/unobserved_owned.rs::accepts` still recognizes whole functions
    to suppress arrival homes; it no longer governs all return cleanup.
    Retire that support dependency through per-place storage/action evidence,
    not another allowlist. Executable nominal calls belong to
    **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP** and must preserve this
    schedule.

  Acceptance: source-produced artifacts reload and independently reconstruct
  exact complements and complete transfer/cleanup partitions; native runs
  preserve values and cleanup timing across the composed edges. Reject missing,
  duplicate, reordered, overlapping, wrong-root/type and post-transfer cleanup.
  Fuel exhaustion commits no disposal and retry cannot repeat one.
  Preserve `terminal-verifier`'s `jump_edge_residual_discards_close_the_projected_argument_root_in_order`
  and `owned_successors_reject_same_arity_aliases_and_transfer_after_disposal`
  controls. No affine occurrence disappears or survives past its required edge.

- **STATE-LOCAL-VALUE-FRONTIER.** Complete compositional evaluation and
  value/storage transport through Psi argument normalization, checked
  computations/call plans and Terminal production, with independent replay
  and native realization. Follow
  [evaluation order](wiki/spec/language/expressions.md#evaluation-schedule)
  and [argument/result custody](wiki/spec/terminal-psi/calls_and_outcomes.md#argument-and-result-ordering).
  Retain exact producer, parameter position, result owner and loan activation;
  **CML4** owns residual cleanup.

  Remaining work:

  - Replace synthesized guarded-call states and competing whole-machine
    plans with ordinary evaluation/control operations. Cover effectful state
    arguments/returns, dynamic and borrowed/projected storage, mixed
    scalar/structural operands and boundary consumers. Selected case edges
    retain the invocation receiver and local ownership. Preserve
    `transition_argument_call_result_derives_the_exact_entry_subject`,
    `jump_operand_mutation_cannot_replay_the_taken_guard` and
    `composed_unit_claims` controls. `machine_lowering/machine_dispatch.rs`
    still rejects simultaneous scalar/Unit dynamic joins;
    `rewrite_guarded_transition_argument_calls` still synthesizes states.
    Delete superseded shape producers as their operations compose; a failed
    custody rejoin must never fall back to a weaker recognizer.
  - Extend returned structural custody to extracted projections, freshly
    established claims and claims from distinct owned inputs. Preserve
    shared/mutable/write-only temporary loans, self consumers and content
    guarantees independently of claim identity.
    `effects/structural_callback_reach/projected.omg` supplies a whole-array
    forwarding baseline, not extracted-projection or native closure.
  - Complete aggregate field replacement: nonliteral aggregate sources,
    nested sums, borrowed case observation and whole nominal receiver
    replacement, including match-assigned values. The customer is
    `filesystem/windows_canonicalize_exit`'s stored `UnitResult`.
    `execution/unit/structural_scalar_store/tests/record_literal_fields.rs`
    pins rejection of nonliteral records and structural members; scalar-field
    decomposition is not aggregate replacement. `borrowed_windows.rs`'s
    `StoreStructuralField` repairs an opened hole, not general overwrites.
    Coordinate **FILESYSTEM-RELEASE-CONTRACT**, **WRITE-ONLY-BORROW** and
    **NOMINAL-FIELD-FLOW**; do not dispose a moved value twice.
  - Complete borrowed local record calls and subsequent observations without
    the root-expression shape gate in
    `values/scalar/computations/structural_values.rs::is_record_value`.
    `declared_range_inference_local_effects_retain_pending_terminal_boundaries`
    pins a record-copy/borrowed-mutation customer still lacking its scalar
    control plan. Primitive borrowed-local mutation already has encoded
    execution coverage in `borrowed_scalar_call_source`.
  - Complete exact saved-value snapshots across actual writes/exclusive
    exposure, plus dependent/public-trait results and subslice bounds.
    `validation/src/proof_contracts/contract_entailment/ranking_range/saved_arguments.rs`
    already accepts stable unwritten mutable carriers; `mut` alone is not
    the gap. Share capture with **CRASH-CONTRACT**, preserving case/index/
    generic/reference/float identities and totality. Current body facts
    cannot impersonate entry observations; loop invariants cover backedges.
    `compiler --test bounded_slice_selectors` pins separate endpoint-signature,
    window-statement-sequence and main scalar-control-plan omissions.
    Replace those expectations with source-free Terminal and matching-host
    native slice execution; scalar endpoint success does not close transport.
  - Carry [exact anonymous arithmetic](wiki/language_guide/chapter_5_expressions_evaluation.md#exact-anonymous-division-and-landing)
    through newly admitted generic/evidence-adapted, boundary, aggregate,
    parameter/constant, float and proof routes, preserving rational
    intermediates, result carrier/policy custody and warning origins/suppression.
    Keep [typed quotient/remainder](wiki/language_guide/chapter_5_expressions_evaluation.md#typed-integer-quotient-and-remainder)
    separate. Authored const/helper selection already has resolver and
    build-time admission controls; preserve them rather than folding builtin
    meaning. **OPERATOR-MACHINE-SUPPLY** owns executable supply.
    Nonconstant proof-`Int` arithmetic still needs independent evidence beyond
    source entailment in `validation/src/proof_contracts/contract_entailment/arithmetic_judgment.rs`.

  Acceptance: one composed caller tolerates reordered/renamed states and
  inserted computations; selected operands execute left-to-right once,
  skipped calls never execute, and saved values/normal guarantees agree in
  replay and native execution. Explicit state transfers preserve custody
  without mandatory copies; implicit cross-state use rejects. Forged
  bindings/origins, stale snapshots, conflicting loans, missing provider
  authority and incorrect guarantees reject. Keep `7 / 2 * 2 == 7` and
  `4097 / 4096 * 4096 == 4097` with their warnings, reject fractional
  integer landing, and retain typed truncation (the `4097u32` case is 4096),
  signed quotient/remainder laws and zero-divisor rejection.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Complete ordinary
  consuming `drop<T>` and exact owner-attached hook execution under
  [nominal cleanup](wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
  The generic consumer already selects and lowers eligible hooks, including
  bodies that read/write `self` and call ordinary helpers. Do not restore a
  cleanup-body recognizer or expose reserved `T::drop` as an authored call.

  Remaining work:

  - Realize executable cleanup in the common native graph:
    `abstract-operations-to-target-operations/src/lowering/control_flow/terminator.rs::plain_home_cleanup`
    rejects `InvokeNominal`. Preserve the exact receiver, hook contract,
    action order, result homes and continuation across the real call;
    no-code disposal is not a substitute. **CML4** owns residual partitions
    and mixed dying-root schedules.
  - Reject reads through consumed owners using ordinary ownership/access
    checking, not a special case for `drop`. Re-witness
    `frontend_drop_expectations::core_drop_use_after_consume_is_currently_admitted`:
    it currently expects `g.handle` after `drop(g)` to compile and return 7.
    `flow/ownership/moves/observations.rs` does not record a read of the base
    name, while `linear_validation/recorded_events.rs` checks dead custody on
    transfers/consumption. Replace the admitted-read expectation; include an
    ordinary consuming helper and a valid snapshot copied before consumption.
    The fixture has not been rerun during this board audit.
  - Compose contextual requirements and dying local owners through ordinary
    cleanup edges. `checked-trees-to-lowered-psi/src/unit/unit_cleanup.rs::patch_nominal_cleanup_member`
    still excludes nonempty caller/hook prerequisites, and
    `typed-trees-to-checked-trees/src/execution/unit/control/checked_machine.rs`
    rejects nominal-drop locals left owned at return. Retain independently
    checked exact-place prerequisites; never infer new caller demands.
  - Complete erased-bearing record construction without runtime evidence
    storage or cleanup. Both Unit and scalar-graph record emitters reject
    erased initializers, while
    `terminal-codec/src/sections/semantic_module/module_foundation_validation/value_foundations.rs::validate_establish_record`
    demands a relevant initializer for every declaration. Reconcile semantic
    fields and proof custody with the physical roster, preserving erased
    subjects, multiplicity and provenance. **PROOF-RELEVANCE-MIGRATION** owns
    erased formal/actual transport. This is distinct from an owned dynamic
    descriptor whose hidden runtime payload still requires cleanup.

  Acceptance: `drops/core_drop_owner_hook{,_body}` and
  `drops/core_drop_explicit_consume` reach encoded, independently checked
  Terminal and native execution. Observe a nonempty hook's effect exactly
  once at early disposal and normal scope exit, with valid contextual
  prerequisites and an erased-field control producing no runtime cleanup.
  Missing/forged hook identity or premises, duplicate consumption and reads
  after consumption reject. Preserve ordinary helper calls and reserved-hook
  selection rejection. `tests/native-differential/tests/frontend_drop_expectations.rs`
  uses the checked-tree interpreter and explicitly does not observe hook
  effects; its compile/interpret successes are not native cleanup acceptance.

- **TR3-TR8.** Connect activation planning and lifecycle accounting to a
  selected runtime executing ordinary named machines under the
  [task-runtime contract](wiki/spec/build/task_runtime.md) and
  [suspension contract](wiki/spec/terminal-psi/calls_and_outcomes.md#suspension).
  Owners: `provider-planning/src/task_plans/`, `task-plans`, suspension
  production/checking, and the selected runtime implementation.

  Reuse the sealed call-graph plans, argument marshalling, nonmoving leases and
  provider-instance claim ledger. Call-side suspension markers already require
  site/plan pairs; `suspension_call_plan_rejects_coordinated_site_and_plan_deletion`
  pins that control. The [implementation note](omega-rust/omega/representations/task-plans/README.md)
  distinguishes those static/accounting mechanisms from executable activation.

  - Join final physical frame/spill demand, alignment, entry overhead and complete
    live call chains to the exact `StackPlan` and actual provider backing.
    Checked-local/park-frontier layout is not final physical WCSU. Preserve
    unresolved-call refusal and same-stack versus separately provisioned demand.
  - Execute one source-selected runtime route: install marshalled arguments,
    start a distinct activation on its fixed nonmoving stack, reach checked
    suspension points, park/resume the same invocation and return actual outcomes.
    Bind provider selection, invocation receipt, runtime instance, storage era and
    source `Task<T>` route. Ledger transitions alone do not run a machine.
  - Extend exact suspension-frontier production beyond bounded scalar joins to
    receiver, persistent and structural places and live claims. Retain their
    Terminal identities and independently checked storage lifetime, pinning,
    aliasing, address stability and cancellation-outcome evidence. Keep crossing
    completeness through serialization/lowering; unsupported frontiers reject.

  Acceptance: concurrent start/park/resume/finish with observable work and moved
  arguments; rejection conserves caller-owned arguments/leases; foreign-instance
  or stale-era settlement rejects and reuse requires fresh eras. Parking produces
  no result/cleanup, and resumption repeats no committed work. Cancellation needs
  a recorded request observed at a declared safe point, never arbitrary unwinding.
  Preserve CPU/thread requirements and reject reclaim while child custody remains.
  Runtime custody, physical backing ownership and linear settlement authority stay
  distinct; a compiler/provider-owned continuation is not source-addressable.

- **ATOMIC-MEMORY-MODEL.** Complete the formal atomic/fence model and checked
  source-to-target realization under
  [concurrency](wiki/spec/language/concurrency.md) and the
  [atomic vocabulary contract](omega-rust/psi/foundation/language-core/atomic_operations.md).
  Owners: Psi event/evidence production and independent Terminal checking;
  Omega event validation, optimization preservation and target refinement.

  The abstract-event checker currently validates bounded single-function
  reads-from/modification-predecessor claims using sequencing, dominance and
  reaching writes. Its tests construct abstract events directly; the
  `atomic_global_order_operations` source fixture stops at checked trees.
  Neither establishes a concurrent memory model or a connected native producer.

  - Complete independently checkable `sequenced_before`, `reads_from`,
    `modification_order`, `synchronizes_with`, `happens_before` and
    `global_sequential_order` obligations, qualifying fence synchronization and
    outcome-sensitive RMW behavior. Preserve settled source guarantees; raise
    genuinely unsettled formal choices through the governing specification owner.
    Model/checker work and relation witnesses do not require a running scheduler.
  - Produce normalized events through checked source, Terminal Psi and abstract
    operations, retaining exact operation, place, ordering, outcome and custody.
    Add encoding, independent verification and source-correspondence controls.
    A fence remains an event even when no instruction is emitted.
  - Add target operations, realization and checked refinement for admitted
    widths/alignment and orderings. Retain exact decode/encode and round-trip
    obligations. Swap/fetch return the instruction-observed prior, not a separate
    load. Preserve observing/non-observing and decisive/single-attempt behavior,
    failure ordering, `Uncommitted` custody and retry-work attribution.
  - Use **TR3-TR8** for real concurrent publication, qualifying fence pairs,
    global-order and compare-exchange contention controls. That dependency gates
    runtime witnesses, not all preceding model, producer or target work.

  Acceptance: source-free checking binds every retained relation, place, ordering,
  outcome and realization; missing/substituted evidence rejects. Retain serial
  coherence and source-ordering controls, then add matching-host instruction and
  concurrent execution coverage. Serial tests and bounded exploration are not a
  formal memory-model proof. Keep the strong `Receive` baseline unless an exact
  protocol proof admits weakening; device/DMA and interruption ordering retain
  their separate contracts.

- **BLOCKEXEC.** Implement the ordinary package-level blocking executor in
  `source/library/blocking-executor/` under
  [task-runtime library contracts](wiki/spec/build/task_runtime.md#library-and-foreign-providers)
  and [foreign execution](wiki/spec/build/foreign_bindings.md).
  It owns bounded queues, moved submissions and linear completion claims, not a
  new compiler task-runtime mode.

  The package has contract declarations, not working queue/worker bodies.
  `compiler/tests/canary_suite/task_runtime.rs` checks package consumption,
  provider selection, linear-slot take/restore and conditional claim returns.
  These are checked-tree fixtures, not execution. The slot example uses literal
  index zero; the admission example passes a pre-existing ticket into `submit`.
  They refute blanket claims that slot vacancy or multi-case linear returns are
  unavailable, but do not establish a bounded FIFO or real claim issuance.

  - Implement generic bounded queue and executor bodies, with indexed occupancy,
    head/length/capacity proofs and surviving ownership of every queued payload.
    Exercise the actual combined type/const `[T; N]` methods through ordinary
    **RUNTIME-VALUE-GENERICS**; do not keep the obsolete blanket attached-method
    blocker or manufacture an executor-only lowering path.
  - Implement accepted/refused admission, provider-instance ticket issuance and
    exactly-once settlement using ordinary conserved-claim/outcome evidence.
    Acceptance must mint a claim against the actual executor, not receive a
    fabricated/pre-existing ticket as the canary does. Rejection returns the
    complete moved submission; close requires empty queue and no live claims.
  - Connect selected worker and wait/wake providers to **TR3-TR8** execution.
    Preserve thread affinity and the actual loans through suspension, including
    settlement's executor borrow; passing only a wait word does not verify that
    borrow or its instance relationship. Cancellation requests retain the claim.
  - For bounded recovery from hung workers, implement actual process-isolated
    execution with the exact parent endpoint, child manifest, custody and
    termination evidence required by provider selection. The empty
    `IsolatedWorkerProvider` marker and inherited conformance test supply no
    isolation evidence; never terminate an in-process worker to reclaim custody.

  Acceptance: consume the package through its normal dependency/build path and
  execute queued work, full-capacity refusal, FIFO delivery, park/wake and
  settlement with linear payloads. Reject duplicate/foreign/stale tickets, lost
  custody and premature close/reclaim. Exercise isolated hung-worker recovery
  separately; checked declarations and provider-plan selection alone do not pass.

- **QUOTIENT-THEOREM-LIFT.** Complete checked operation correspondence and
  publication under [lifting operations](wiki/spec/proofs/quotients.md#lifting-operations).
  Managed direct `Quotient::define` and transport-backed `Quotient::lift`
  already have checked-only source canaries. Reuse their post-termination
  admission and proof-only request handling; mathematical admission is not
  executable realization.

  The local relation/result-flow checks are broader than their canonical
  publisher. In validation's `proof_contracts/quotients/relation_plan/`,
  argument adaptation and immutable result aliases have checked forms, but
  `terminal_bridge.rs` still admits monomorphic, single-state, direct
  position-preserving rows. Complete missing correspondence judgments and
  extend publication and independent reconstruction for adaptation,
  aliases/forwarding, result computation, preconditioned `define`, and
  generic/private applications; reuse the local judgments already implemented.
  Retain complete application, relation, precondition and theorem-role evidence.
  Do not multiply source-shape recognizers to publish each arrangement.

  Representative/theorem eligibility still requires checked termination,
  purity, crash freedom and exact hermetic identities; theorem closures must
  be bodyful and free of admitted assumptions. Check helper types/statements
  transitively, not only visible bodies. General assumption-closure machinery
  and the congruence-only `lift<F, Congruence>` canonical payload belong to
  **PROOF-CONTRACT-MIGRATION**; coordinate that payload's codec, verifier and
  package-review consumers rather than inventing a second wire form.

  Acceptance: package-backed direct `define` and transport-backed `lift`
  admit through ordinary `omega --check`, not only an isolated lowering
  helper. Their [published correspondence](wiki/spec/proofs/quotients.md#published-quotient-correspondence)
  rederives on decode and supports the broader admitted applications above.
  Preserve rejection of implicit lifts, missing/surplus/reversed roles,
  invalid laws, unproved termination, non-hermetic identities, admitted or
  boundary theorem closures, and custody-bearing quotients.
  **QUOTIENT-RUNTIME-REALIZATION** owns execution separately.

- **QUOTIENT-RUNTIME-REALIZATION.** Give admitted quotient operations a
  checked executable value/call path under the
  [representative realization contract](wiki/spec/proofs/quotients.md#representative-layer-and-executable-realization).
  The bounded direct correspondence forms already provide a starting point;
  this work need not wait for every generalization in QUOTIENT-THEOREM-LIFT.

  Checked contracts, ranges and termination, and subsequent nested/mixed-call
  lowering still exclude `quotient_operation` calls. Terminal validation
  rejects nonempty retained correspondence tables with
  `NonExecutableQuotientCorrespondence`. Replace those exclusions only
  where a checked representation/operation judgment licenses the ordinary
  representative call. Retaining a mathematical correspondence row alone
  does not establish all executable custody.

  Preserve the representative ABI and permitted uncanonicalized constant
  materialization without exposing representative-sensitive structural
  observers. Axiomatic operations supply no algorithm; executable use needs
  checked computation or a realization with correspondence. Keep custody-
  bearing quotients fenced and fact-only admission free of runtime calls,
  dictionaries or fuel charges.

  Acceptance: direct `define` and transport-backed `lift` customers reach
  source-free Terminal checking and matching-host native execution, with
  result/argument/relationship substitution and missing correspondence
  rejected independently. Preserve refusal of illicit representative
  extraction and unsupported realization. The quotient-refusing policy
  control additionally depends on PROOF-CONTRACT-MIGRATION's transitive
  assumption closure through helper types/statements; do not confuse that
  integration dependency with the initial executable route.

- **PRIVILEGED-PORT-EFFECT-SETTLEMENTS.** Connect checked adapter port I/O
  to native production under [consumer-owned settlement](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement).
  PE/ELF/Mach-O evaluated locator custody is a separate, implemented route;
  privileged instructions belong to checked `asm`, not a foreign-binding
  instruction language.

  `TargetUnitOperation::PortWrite`, x86 encodings and independent
  port-effect custody readers exist. Ordinary selection, register-home/
  post-allocation transport and machine-fragment emission still do not
  produce the port effects: image-emission's
  `function_fragments/production.rs` publishes an empty roster.
  Carry an installed selected adapter's exact `PortIo` requirement,
  service/port/value, operation ordinal and byte span through that route.
  Preserve the effect-before-settlement join through object rebasing, final
  image and installation evidence; reuse the existing readers in
  native-artifact's `physical/derivation/provider_custody.rs`.

  Use a positive fixture whose checked adapter owns the selected requirement.
  Direct-root privileged operations must continue to reject without provider
  custody; do not turn their negative fixtures into a bypass. The
  `asm_runtime_port_msr_final_validation` customer also needs its actual
  selected bindings; unrelated machine-control/root-binding gaps do not
  belong to this port transport repair. Port read (`in`) additionally needs
  its Terminal operation and producer route; `DirectPortReadU8` settlement
  metadata alone does not supply them.

  Acceptance: an authored selected adapter produces and independently
  validates its exact native effects. Missing, duplicate, substituted,
  role-swapped or unconsumed records, changed service/port/value/target,
  detached spans and altered bytes reject. Keep unsupported-target refusal.
  Separate emitted-byte verification from execution, which requires an
  appropriately admitted privileged environment, not raw I/O in a hosted test.

- **FLOAT-PROVIDERS.** Complete checked source and independently replayed
  `FloatSemantics` obligations used by the actual `Float::*` slot contracts,
  under [FloatMeaning](wiki/spec/terminal-psi/mathematical_values.md#floatmeaning).
  Closed Meaning-valued applications and authored equality ensures already
  reach produced artifacts and independent proof replay; reuse that route.
  Its checked binder, evaluator and Terminal application carrier specialize
  Meaning results and Format/Meaning operands. Complete Boolean,
  classification and integer-conversion contract discharge through the
  appropriate proof carriers, not by admitting those results as FloatMeaning.

  Owners: checked `proof/float_meaning.rs`, validation's
  `float_projection_bindings`, checked/lowered contract production and
  Terminal semantic/proof replay. Preserve exact declaration/signature,
  owner/use-site, result and argument correspondence; shared numerical kernels
  and well-formed metadata alone do not establish a source obligation.

  Acceptance: actual slot-contract customers retain and discharge their
  obligations through produced source-free artifacts, including non-Meaning
  results and imported call/operator results. False claims, lookalike
  declarations, altered operands/formats/results and missing evidence reject.
  Preserve IEEE Boolean comparison separately from structural meaning
  equality, including NaNs and signed zeros. Keep
  `produced_artifact_verifies_authored_float_meaning_ensures` and its forged/
  missing-evidence controls. Native FMA transport belongs to
  **X86-FMA-PROVIDER-TRANSPORT**; constants to **FLOAT-IDENTITY-LITERAL-CARRIER**.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Finish ordinary native
  borrowed-descriptor invocation and forwarding under
  [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md).
  Start with the non-entry helper that forwards a descriptor, dispatches
  through it, and uses the result across calculations and branches.
  `forwarded_descriptor_calculations_execute_through_verified_artifact`
  establishes source-to-artifact interpreter execution; Target/Legalized
  descriptor ABI replay also exists. Instruction selection still rejects
  `LegalizedScalarInstructionKind::DynamicParameterCall`.
  The next milestone must advance this native customer, not repeat
  target-only descriptor composition.

  Connect the ordinary indirect-call operand through selected construction/
  replay, register allocation, ISA encoding and emission, then exact immutable
  table, relocation and final-image custody. Preserve the requirement-owned
  erased caller shape, ABI/clobber/effect/reach obligations, original referent
  and selected table through joins; Unit calls have no invented result home.
  Tables, zero trap slots and nonoverlapping descriptor/result storage must
  remain independently reconstructible. Remove superseded forwarded-call
  recognizers/records as the ordinary route replaces them, rather than adding
  another whole-body pattern or unproved devirtualization.
  Complete source composition through ordinary ordered-operation/storage
  owners: `unit/dynamic_composed_unit/forwarded_helpers.rs` still rejects
  extra effects, mutable helper locals and different helper/requirement result
  carriers. Retain their own call/result identities and access/effect evidence.

  Acceptance: a source-rooted closed-conformance fixture publishes and
  independently replays Linux x86-64/AArch64, executes on matching hosts,
  selects distinct implementations at runtime, and preserves surrounding
  calculations and writes to the original referent. Reject substituted
  instance/table/slot/ABI/access/relocation/code-span evidence. Retain the
  interpreter and legalization mutation controls; include an additional
  ordered effect, mutable local and differing helper/requirement scalar result.
  Receiver-entry provisioning is a separate dependency of that entry customer,
  not of a non-entry helper.
  Owned erased cleanup remains with
  **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**.

- **TARGET-SEMANTIC-APPLICATIONS.** Complete the portable
  [target-semantic capsule](wiki/spec/language/evaluation.md#target-semantic-capsule)
  and artifact-qualified
  [application closure](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  Provider-dependent const applications already defer and fold under exact
  selected execution. Preserve named/token, field/let/return, alternate-provider
  and unselected-provider controls rather than rebuild that route.

  Retain sealed versioned target semantics, exact selected plans,
  implementations and arguments as replayable inputs shared with realization.
  Typed observations name their actual address-space/layout/format/profile
  subject and reject unresolved subjects; compiler-host state supplies none.
  Reuse ordinary typed arguments and compiler-materialized target data where
  they express the customer. The spec does not approve a general
  `TargetSemantics::*` source API; do not add one merely to implement the
  capsule. A genuinely missing observation capability needs a concrete
  customer and the normal owner-escalation audit.

  Extend package manager review's `symbolic_boundary_applications` beyond
  explicitly supplied direct-type-binder requests to the complete reachable
  specialization set, preserving original mappings and complete canonical
  arguments for replay. Recheck actual reach, transitive proof obligations,
  target facts, admission and selected realization after substitution.
  A reviewed closed demand alone grants no Terminal/native, coverage or
  installation authority; complete-set composition is not an install/update
  prerequisite. Preserve the distinction between an operator's canonical
  empty telescope and a boundary-trait call with no telescope. Physical-child
  binding stays with TRANSLATION-VALIDATION in `TASKS_OPTIMIZER.md`.

  Acceptance: source-free cross-artifact customers reproduce exact selected
  application/target identity and all reachable obligations; missing, extra,
  stale, substituted or unresolved mappings cannot acquire coverage.
  Retain provider-free evaluation, distinct selected const results and
  false-result/unselected controls. Reject a build dependency cycle rather
  than searching visible satisfiers or silently choosing a body.
  Checked-source folding or a manually supplied closure request alone does
  not close this acceptance.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Complete selected execution and
  source-free identity for explicit `boundary requirement Owner::name(...);`
  under [machine supply](wiki/spec/language/machines.md#supply),
  [provider selection](wiki/spec/build/provider_selection.md) and
  [requirement identity](wiki/spec/terminal-psi/boundary_calls.md#call-and-requirement-identity).
  Existing checked/intrinsic adapter routes cover value/statement calls and
  owned receivers; deeper projected receivers also have checked interpretation.
  Reuse them without treating that result as native coverage of every shape.

  Extend the selected-call mechanism to type/lifetime-parameterized
  requirements and borrowed receivers. `selected-dispatch/boundary_dispatch.rs`
  still restricts direct requirements to nongeneric, receiver-free signatures;
  `selected_dispatch/requirement_adapter.rs` separately rejects family rows.
  Compose external satisfiers with structural arguments and interpreter
  provider execution. Preserve the mixed borrowed-record external customer,
  its initialized fields and reused returned value; the macOS ARM64 scalar
  control `top_level_external_requirement_returns_and_reuses_its_result_natively`
  does not establish that broader route or other-host execution.

  Replace shape-specific post-check adapter rewrites with settled call-target
  substitution keyed on the actual flow occurrence. Retain the requirement's
  package-qualified operation, complete static telescope, signature, contract,
  visibility, selected provider/adapter and era through ordinary lowering.
  This is a canonical requirement kind distinct from trait-keyed conformance
  rows. The current Terminal canary's helper observes an ordinary adapter call
  and no requirement boundary declaration, despite its renamed
  `...retains_requirement_occurrence` test; its name is not identity evidence.

  Coordinate `Task::finish`/cancellation and interrupt completion with
  BOUNDED-INSTALLATION-REACH-ROWS and COMPONENT-SUBSTRATE for selection and
  lineage, not special compiler-owned lifecycle policy.
  **OPERATOR-MACHINE-SUPPLY** owns source supply migration and retirement of
  the undifferentiated `MachineSupplyMode::Boundary`; do not recreate the
  already retired float-operator inventory here.

  Acceptance: value, statement, receiver and external-provider customers
  execute in the interpreter and natively under one exact selected plan;
  serialized source-free replay retains requirement/provider/adapter identity.
  Reject unselected, private, ambiguous, foreign-package, substituted and
  stale-era authority, including same-spelled declarations in another package.
  Keep missing-import settlement rejection; an ordinary foreign exit is not
  canonical ProcessExit evidence.

- **BUILD-ADMISSION-CHECKPOINT.** Finish command-level
  [restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance)
  and executor-grant integration across install/update/review/resume and
  consuming compilation. The pre-effect checkpoint is implemented in
  `checking/build_continuation.rs::AdmittedBuildCheckpoint::execute`;
  `check_project`, `check_locked_sources` and `compile_project` supply it.
  Reuse its refused/granted-effect sentinel controls rather than rebuild the
  gate or move consent back after execution.

  Supply a real restricted-request command customer, including a dependency
  used for both Build/Product purposes and its generated-source handoff.
  Manager review sessions currently use compiler-owned private staging;
  empty request sets do not demonstrate this acceptance. Bind requests and
  grants to exact package occurrence, dependency path, checked purpose,
  target/execution profile, operation, logical scope and bounds. Product
  acceptance cannot authorize the same package's build effects.

  Acceptance: initial installation and newly added, widened or transitive
  requests stop before their restricted effects and expose those coordinates.
  Acceptance plus a genuine executor grant resumes the exact candidate, then
  reviews generated source; missing/refused/cross-purpose grants perform no
  restricted action. Audit inspection acquires no host authority. Unchanged
  request meaning needs no recurring approval; source changes remain visible.
  Resolution-only updates preserve approval meaning. Exercise interrupted
  review/publication and stale candidates without storing secrets or machine-
  specific grant paths in the lock. Dependency locks confer no host authority;
  already performed host
  effects have no implicit rollback.

  Preserve coherent source/authority snapshot replay, purpose-drift refusal
  and authored-before-generated resolution. Captured immutable inputs and
  bounded compiler-private staging remain benign, while supplied host
  directories remain restricted. No recursive build API, own-build
  final-component query or hidden post-compilation callback.
  **BUILD-SNAPSHOT-OUTPUTS** owns committed output sets and required-output
  settlement at publication.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Finish ordinary explicit std/alloc
  dependency migration under the [toolchain/library contract](wiki/spec/packages/toolchain.md).
  Std may be replaced, split or absent; only core and the specified
  compiler-injected vocabulary retain toolchain authority. Standalone
  std/alloc still receive broad `Toolchain` classification in
  `source-files-to-assembled-syntax/src/source/source_storage.rs`.
  Remove that fallback as remaining consumers acquire exact source-byte
  catalog roles or accepted semantic bindings, not by relabeling a directory.

  Migrate remaining corpus/member imports and package roots, including
  bundled proof, host/objc, fail and run consumers, through ordinary
  dependency edges and target-correct Console, Filesystem and physical-entry
  bindings. Keep freestanding roots dependency-free. The existing narrow
  roles, including macOS x64 entry, are documented
  [beside package compilation](omega-rust/omega/build/package-compilation/semantic_bindings.md).
  Route exposed composed-call, ownership, entry and proof failures to their
  capability owners, retaining each customer's intended checking/execution
  acceptance; unrelated blocked features do not stop independent migrations.
  Private core proof declarations remain private: obtain an authorized proof
  route with PROOF-CONTRACT-MIGRATION/PROOF-KERNEL-CORE, not a per-fixture
  visibility exception.

  Acceptance: removing a dependency rejects its imports/provider selections;
  aliases, paths, names and same-spelled substitutes confer no authority.
  Rejoin exact package, declaration, schema and any role-required selected
  plan; stale/substituted bindings reject independently of lock replay.
  Extend `repository_build_declarations.rs`'s recursive member/import checks
  as fixtures migrate and preserve the missing-edge/alias controls in
  `standard_library_package_resolution.rs`. Early missing-import rejection
  is sufficient; do not weaken assembly to manufacture a second diagnostic
  for a provider selection whose import already failed.

- **COMPONENT-SUBSTRATE.** Deliver independently selected component products
  and their admission/replacement closure under the
  [component publication contract](wiki/spec/build/component_publication.md).
  Runtime packages or Cathedral own deployment, acquisition and update policy.

  Description publication and checked `Independent` settlement already have a
  production path through package-manager dependency compilation,
  `compiler::published_independent_component_description` and
  `component-description::verify_component`. The current product boundary
  remains `checked-compilation-to-terminal-artifact`'s
  `terminal_artifact/composition_modes.rs`: both Terminal and native products
  reject settled independent edges because symbolic imports/exports,
  entry/leave resources and installation/replacement custody are absent.
  Remove that fence only when the requested independent product exists; never
  emit a silently fused substitute.

  Carry the exact import/export, complete entry/outgoing-authority, service,
  mapping, lease, stack and retained-provider inventories into the product.
  `CustodyKind` and `ObligationKind` already have Mapping/Lease vocabulary;
  supply actual producer facts, independent reconstruction and per-occurrence
  discharge, not more enum-only scaffolding. Connect native
  `component-candidate::describe_component`'s derived stack and realization
  identity to publication. A Psi-only capsule correctly has no native facts;
  do not fabricate them to fill the fields.

  Independent admission/replacement must consume the same verified-description
  contract as **TOPOLOGY-PLAN-VERIFICATION**, retaining exact artifact, profile,
  authority and resource correspondence. `component-deployment` currently
  binds candidate/installed-byte custody directly, not a verified description;
  preserve those checks while joining the shared contract. Topology's simulated
  replacement-profile controls do not establish loaded execution or retirement.
  **BOUNDED-INSTALLATION-REACH-ROWS** needs this complete component contract;
  **WIRE-RUNTIME-AND-INSTALLATION** owns platform installation/execution and
  **PSI-COMPONENT-REPLACEMENT** owns the interpreted embedding customer.

  Acceptance: an ordinary two-package build selecting `Independent` emits
  the independent products and publishes/consumes the dependency description
  without test-side attachment. A source-free consumer checks exact subject,
  profile, schema, all entries/outgoing authority, custody, retained providers
  and inseparable assumptions, including startup, callbacks, timers and cleanup.
  Preserve corrupt/truncated, substituted/stale/unrelated, early-frontier,
  omitted/forged inventory, duplicate dependency/plan and unaccepted-assumption
  rejection. Reading a description grants no callable or installation authority;
  installation facts require fresh per-occurrence admission.

  Exercise actual authorized stable-slot replacement: compatible imports/exports
  and behavior exclusions, fresh candidate resource/profile admission, new calls
  entering the new era, old sessions retaining theirs, and retirement only after
  every activation, registration, returned object and claim has a valid disposition.
  Pin state migration/retention, coexistence capacity and failure behavior.
  Unauthorized, incompatible, weaker-policy, underprovisioned and premature
  operations reject without losing custody. Continuity-free replacement need not
  drain every old era before publication; stronger continuity needs its explicit
  contract. Keep model-level checks distinct from this execution acceptance.

- **FFIVAL.** Complete and run the Windows user32 boundary-coherence
  customer at `tests/omega/pending/host/user32_window_procedure_registration`,
  through its authored build root and the generic
  [private-callback protocol](wiki/spec/build/private_callbacks.md).
  Reproduce its pinned `cannot prove requires contract` diagnostic before
  assigning a repair; generic result-payload qualification transport exists,
  while this customer uses a by-reference outcome.

  REGISTERED-CALLBACK-LIFETIME owns outcome/capacity lifecycle;
  CALLBACK-PRIVATE-MATERIALIZATION owns callback/private-layout realization.
  Supply the package-owned user32 provider/adapter, exact native ABI/layout
  and explicit per-window context protocol. The fixture currently declares
  an abstract boundary and a contextual procedure, not completed foreign
  bindings to Win32's actual callback.

  Acceptance on Windows: actual registration, `WM_NCCREATE` and dispatched
  callback entry, deterministic rejection/retry, teardown and reuse of returned
  capacity, with quiescence before releasing code leases. No raw callable
  address, implicit closure or Win32-specific compiler escape. Promote the
  pending canary only when its corresponding acceptance works; checking and
  diagnostic watchers alone do not establish foreign invocation. Other hosts
  explicitly report this runtime leg unavailable.

- **WIRE-RUNTIME-AND-INSTALLATION.** Connect the admitted executable
  installation lifecycle to an authored/package provider route and a
  matching-host executor under [executable installation](wiki/spec/build/executable_installation.md).
  Reuse `executable-installation`'s linear transitions, required-fact gates,
  sealed entry references, replacement/quarantine joins and
  `image-emission/installed_artifact.rs` custody binding.
  `OwnedImageProvider` performs convention-controlled buffer operations and
  its driver composes their gates; its resident-byte borrow does not establish
  physical invocation or platform execute/cache operations.

  Complete the receiving-provider/executor join: exact admitted content,
  placement, audience and installed occurrence; target-appropriate
  write/execute and fetch-visibility completion; invocation through a
  requirement-compatible sealed entry; live-root/provider custody through
  return, retirement and replacement. Keep platform operations with their
  provider rather than adding OS policy to the generic lifecycle model.
  `ConventionOnly` is permitted when honestly reported; do not universally
  require hardware enforcement or accept `Unsupported`. Ordinary executable
  file emission is not receiving installation. The lifecycle states are not
  prescribed source type names and do not by themselves require new grammar.

  Acceptance: an authored program reuses an admitted artifact across distinct
  linear placements, materializes/freezes/validates exact final bytes and
  footprint, installs through the contracted provider and actually invokes on
  a matching host. Retirement requires quiescence, execute removal, restored
  write authority and demanded facts; incomplete drain quarantines.
  Replacement patches only declared sites with bound admitted fragments and
  establishes visibility/write re-suspension before draining old custody.
  Reject substituted bytes/authority/contracts, transplanted validation,
  double-spent placement, missing completion facts and unsupported W^X;
  failed transitions return their inputs. Keep arbitrary bytes-to-code,
  JIT and raw executable-address routes unsupported.
  **COMPONENT-SUBSTRATE** owns component closure; this row owns generic
  executable custody.

## Rust compiler release closure

The [completion contract](wiki/drafts/rust_compiler_completion.md) requires
eight passing gates at one clean commit and the four matching-host runs.
The tasks below own evidence closure; capability repairs stay with their
implementation owners. Historical records are attribution leads, not current
baseline results. An unavailable runner, unexplained skip, filtered/empty run,
timeout or exhausted capacity leaves the affected gate open.

- **RC-RELEASE-RECORD-AND-CLOSURE.** Finish the existing release recorder and
  assemble the complete same-commit evidence. Owners:
  `tools/release/release_record.py`, its tests, and the completion contract.
  Two committed JSON records under `tools/release/records/` contain only
  RC-PORTABLE-PSI gate passes and explicitly open closure; do not recreate the
  recorder or claim records are absent.

  Fix its incomplete contract mapping before trusting a closure result:
  RC-BUILD-AND-PACKAGES omits the required six-target compiler integration
  command, and RC-DIAGNOSTICS omits `proof_and_domain_canaries` from its
  exact test name. The existing
  `ManifestPinsContract.test_every_gate_command_appears_in_the_contract`
  fails on the latter and checks only recorder-to-contract inclusion, so it
  cannot catch a missing command. Require complete command coverage.
  Align the formatting invocation with AGENTS.md's portable `tools/fmt.py`
  route without reducing coverage. Correct the host procedure drafts'
  nonexistent `--all` option and missing required `--native-execution`.

  Acceptance: recorder tests detect omitted commands, stale filters,
  incomplete runs and inconsistent closure. Produce and revalidate records
  containing the commit, pinned toolchain, host OS/architecture, exact commands,
  outcomes, elapsed time and expected skips, plus emulator/version when used.
  Close only after all eight gates and four platform rows pass as the contract
  requires. RC-PORTABLE-PSI remains the cross-process reload/tamper gate even
  though its implementation task is complete; RC-DIAGNOSTICS remains the full
  negative corpus, not a rerun of the old repaired eleven-fixture subset.
  Preserve release evidence and maintained regression gates with their owners
  before deleting the temporary completion plan. Rust remains a comparator;
  bootstrap/product-source work has its own board.

- **RC-REPOSITORY-CLOSURE.** Establish one clean-commit pass of the repository
  gate: whole-workspace formatting, all-target Clippy with warnings denied,
  architecture tests, all-target check, and workspace library tests. Use the
  completion contract's full scope and current portable formatting route;
  do not exclude failing crates. Owners: repository gates and each failing
  crate. Repair attributed failures through their capability owners, then
  record the complete result through RC-RELEASE-RECORD-AND-CLOSURE.
  [Prior baseline](wiki/drafts/rc_repository_baseline_linux_x86_64.md) and
  [failure attribution](wiki/drafts/known_baseline_failures.md) are dated
  starting evidence, not a reason to repeat fixed lint/fixture repairs or
  combine results across revisions/hosts. Acceptance: all five commands pass
  at the release commit, with failures and skips accounted for by exact test
  names rather than another chronology on this board.

- **RC-BUILD-AND-PACKAGES.** Close all three build/package command blocks in
  the [completion contract](wiki/drafts/rust_compiler_completion.md#release-matrix):
  the seven-package nextest run, those packages' doctests, and the six compiler
  integration targets (`build_config_granted`, `build_log_facet`,
  `build_target_activation`, `checked_build_machine_identity`,
  `evaluated_via_binding`, `package_compilation_inputs`).
  Owners: build evaluation, package resolution/review/manager and compiler
  handoff. The [retained record](wiki/drafts/rc_build_and_packages_linux_x86_64.md)
  at `75650d2e94` supersedes the board's old 107+12-failure census:
  its remaining families include FMA transport, exact checked-call/review
  identity, provider-schema agreement and fixture rosters. Reproduce a named
  remaining failure on the selected base before assigning its repair; do not
  suppress review requirements or stale-service diagnostics to make fixtures
  pass. Acceptance: all three complete blocks pass and retain their evidence
  at the release commit; recorder coverage depends on RC-RELEASE-RECORD-AND-CLOSURE.

- **RC-PCC-REPLAY.** Close the release gate for artifact/`.proof` pairs:
  round-trip valid evidence, reject hostile/substituted evidence before
  PCC-required interpretation or lowering, and keep ordinary non-PCC output
  independently checked. Run nextest and doctests for
  `checked-trees-to-lowered-psi`, `terminal-codec`, `terminal-verifier`,
  `terminal-interpreter` and `terminal-psi-to-abstract-operations` as specified
  by the completion contract. The
  [prior record](wiki/drafts/rc_pcc_replay_linux_x86_64.md) is red and includes
  a terminated proof-search case; a harness returning after killing a test is
  not successful termination of that test.
  C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT owns that resource defect; package tests
  and capability owners repair their remaining attributed failures.
  Acceptance: complete successful commands at the release commit, including
  source-free hostile controls. Native behavioral correspondence stays with
  PCC-PRODUCT-PUBLICATION and PCC-CANONICAL-SEMANTIC-LEDGER; placed-image digest
  checks alone do not close it.

For each native host task below, run all of RC-NATIVE-MATRIX
(`mbx nextest run -p omega-native-differential-test --all-targets --no-fail-fast`),
RC-SOURCE-SEMANTICS (`mbx nextest run -p compiler --all-targets --no-fail-fast`)
and RC-REPRESENTATIVE-PROGRAMS
(`mbx nextest run -p compiler --test samples_compile --no-fail-fast`).
Unset canary/sample filters. Verify emitted programs' required exit/output
observations and publish the host record at the agreed release commit.
Cross-target emission does not substitute for execution. These are independently
deliverable host runs, not four implementations of the gate.

- **RC-NATIVE-MATRIX-LINUX-X86-64.** Execute the complete native, source and
  sample gates above on Linux x86-64 and resolve remaining failures through
  their owning capability tasks. The
  [host record](wiki/drafts/rc_native_matrix_linux_x86_64.md) is a bounded
  subset, not the full gate. The
  [sample record](wiki/drafts/rc_representative_programs_linux_x86_64.md) and
  [print_number control](wiki/drafts/rc_representative_programs_closure_0f75a0.md)
  preserve entry/provider, lowering and default-domain failure leads.
  Recheck them before repair; the recorded `self.out requires [u8; N]::Utf8`
  failure must not be removed by weakening the field's domain.
  Acceptance: actual emitted ELF execution and all required full commands
  pass, not merely a selected ABI cohort.

- **RC-NATIVE-MATRIX-LINUX-ARM64.** Supply the same complete gate evidence
  for emitted AArch64 ELF programs. A Linux AArch64 runner or named/versioned
  emulation may satisfy the contract, but must execute the target runtime
  legs: an x86-64-built harness that cfg-excludes them does not.
  The [retained record](wiki/drafts/rc_native_matrix_linux_arm64.md) combines
  cross-target checks and a single `cli_mvp` QEMU execution; neither closes
  the full row. Acceptance: required native/source/sample gates pass with
  actual AArch64 execution and explicit, justified skips.

- **RC-NATIVE-MATRIX-MACOS-ARM64.** Run the complete native/source/sample
  gates on macOS AArch64, executing emitted Mach-O products. The
  [host draft](wiki/drafts/rc_native_matrix_macos_arm64.md) records procedure
  and cross-target coverage, not a completed matching-host run. Use the
  corrected recorder procedure, not its stale `--all` command.
  Acceptance: same-commit full-gate results with native observations and exact
  expected skips; Linux emission evidence cannot close this task.

- **RC-NATIVE-MATRIX-WINDOWS-X64.** Run the complete native/source/sample
  gates on Windows x86-64, executing emitted PE products, including the
  hosted receiver. The
  [host draft](wiki/drafts/rc_native_matrix_windows_x86_64.md) is procedure
  evidence, not an executed release row. Use the corrected recorder procedure.
  Acceptance: same-commit full-gate results with native observations and exact
  expected skips; cross-emission or compiler-only success is insufficient.


## Omega-written compiler (after Rust completion)

Finish the Rust [completion contract](wiki/drafts/rust_compiler_completion.md)
before starting the product-language migration. Rust can remain a differential
implementation afterward, but neither Rust agreement nor Rust-specific machinery
is bootstrap authority. Bootstrap construction stays on `TASKS_BOOTSTRAP.md`.
- **OMEGA-PRODUCT-COMPILER-SOURCE.** After the Rust release-completion gate,
  complete the Omega-written production compiler as separate `source/psi/`
  and `source/omega/` packages, retaining `source/omega/{build.omg,main.omg}`
  as product roots. Psi owns target-neutral parsing, resolution, typing,
  checking, proof and Terminal Psi production; Omega consumes Terminal Psi for
  provider selection, realization, ABI and artifact emission. Package separation
  and the firewall are settled.

  The current root drives a lexer and partial whole-file parser, then exits;
  it produces no compiler artifact. Extend the connected product in ordinary,
  compositional vertical slices through the complete language and requested
  products. No bootstrap-private dialect, file/AST allowlist, source-shape-only
  lowering path or parallel source-to-native route. D in `bootstrap/5_omega/`
  is the separate Epsilon-written implementation; Rust is a nonauthoritative
  differential comparator.

  Resume from a fresh product-root check and freshly build/run
  `source/psi/test-parser.sh` with explicit `OMEGA_CLI` and `OMEGA_TARGET`.
  This is a parser golden-observation gate, not a two-compiler differential
  harness. Preserve acceptance/rejection, lexical handoff, structural identity,
  capacity and determinism controls; Python only decodes/compares observations.
  Cached artifacts and checked-source success do not establish native execution.
  Reproduce and attribute the previously reported package-name-shadowing,
  entry-custody and expensive-checking issues before assigning repairs; their
  old diagnostics and timings are not a current frontier. Route general
  value-transport/native gaps to **STATE-LOCAL-VALUE-FRONTIER** and existing
  borrowed-storage/crash gaps to **BORROWED-STORAGE-RESTORATION** and
  **CRASH-CONTRACT**. Do not infer invalid source from a lowering omission:
  the nested-receiver narrowing-cast fixture carries range evidence.

  Acceptance: the exact package-resolved source closure implements the complete
  language and production pipeline under the
  [compiler product contract](wiki/spec/build/compiler_request.md), passes the
  shared product suite and Rust/Omega differential behavior checks, and publishes
  a deterministic manifest of every transitive compiler/build input. Preserve
  requested-product/target and admission boundaries; partial parser acceptance
  is not compiler completion. C's ordinary Alpha obligation remains required;
  Rust Alpha emission is not a prerequisite. Bootstrap construction and
  `omega0 → omega` closure remain on `TASKS_BOOTSTRAP.md`.


## Mined items (swarm wave 9 mine legs)

Candidates extracted by mine legs from `wiki/drafts/`, `TASKS_OPTIMIZER.md`,
`TASKS_BOOTSTRAP.md`, and `samples/apps/squalr/TASKS.md`, deduplicated by the
coordinator. Each item names its source doc; the mining session's full
`mine_report` verdict is in `build/swarm/w9/wave-9.outcomes.json`. Claim the
named paths, verify the gap is still open on current origin/main (close as
`superseded` if a landing already fixed it), then implement per AGENTS.md
validation scope.

Baseline-failure repairs (source: `wiki/drafts/known_baseline_failures.md`):

- **BASELINE-PACKAGE-COMPILATION-INPUTS.** Scope verified at `1f7301b71020` — the
  gap is still open but the count was stale: `cargo nextest run -p compiler --test
  package_compilation_inputs --no-fail-fast` reports 194 run, 191 passed, **three**
  failed (401s, linux x86-64), not two. They are three distinct causes, not one
  surface, so they do not close together:
  - `artifact_identities_and_entries::free_process_exit_helper_lowers_without_a_synthetic_attachment`
    — "provider selection operand does not resolve to one visible product declaration".
  - `artifact_identities_and_entries::accepted_package_uefi_binding_selects_exact_ordinary_schema`
    — "authored Call declaration selection occurrence 1672 remained unresolved".
  - `module_template_methods::instantiated_methods_keep_each_package_use_authority`
    — "duplicate data `Envelope`".
  The first two are provider/selection resolution; the third is a package-use
  authority collision on an instantiated template method. Re-run the command above
  before attributing any of them, since the roster moves.
  covered — each remaining failure is attributed to an owning leaf; no repair slice under this name
- **BASELINE-NATIVE-DIFF-TERMINAL-PSI-SOURCE.** Three unrepaired failures in the `terminal_psi_source` native-differential lane.
- **BASELINE-EXTERNAL-ROOTS-FIXED-FUEL-CEILINGS.** (new-scope) Unrepaired
  failure in `external-roots`:
  `stack_and_fuel::fixed_fuel::tests::installed_natural_cycle_safe_point_catalog_binds_to_one_occurrence`
  expects the ranked fixture's five segment ceilings to bind as
  `[1, 3, 3, 3, 1]` and observes `[1, 25769803776, 25769803776, 25769803776, 1]`
  (macOS AArch64, `cargo nextest run -p external-roots` 248 run / 247 passed
  at origin/main d4a908ec95 plus the 0e7dbf60de build repair). 25769803776
  is 6·2^32: `7591b2607c` ("terminal-fixed-fuel: bound segments through
  ranked cyclic components", whose body records its local tests as still
  pending) now charges a `TerminalRankedScc::Natural` component as
  rank-max-plus-one member visits, and the fixture's rank carrier is `u32`,
  so the component bound is taken over the carrier range rather than the
  countdown the external-roots binding expects. The failure was
  unobservable while `external-roots` did not build (20bd592af1 removed the
  journal types 2d8c5136cc consumed); the fuel crate's own suite is green
  (`cargo nextest run -p terminal-fixed-fuel` 60/60). Owner: the
  terminal-fixed-fuel ranked-segment lane (**PSIIR** resource-analysis leg)
  decides whether a ranked component's segment ceiling is the carrier-wide
  bound or the verified countdown; the external-roots expectation follows
  that decision. Repair is one of the two files
  (`terminal-fixed-fuel/src/fuel_certification/{outcome_bounds,segment_partition}.rs`
  or `external-roots/src/stack_and_fuel/fixed_fuel/tests.rs`), not both.
- **BASELINE-NATIVE-DIFF-PIPELINE-OWNERSHIP.** Unrepaired failure in the `pipeline_ownership` native-differential lane. Current red state is a
  compile-broken test target at `62c502f9f6` (linux x86-64,
  `cargo nextest run -p omega-native-differential-test --test
  pipeline_ownership` exits 101 before running): four callers pass
  `selected.optimized_target()`/`x86.optimized_target()` —
  `&ValidatedOptimizedTargetOperations` — where
  `validate_optimized_selection_custody` takes
  `&Arc<ValidatedOptimizedTargetOperations>` (the
  `optimized_target_owner()` accessor already yields that Arc; call sites
  at `stages/realization/structural_units/structural_return.rs:33`,
  `stages/selection/custody.rs:62`, `validation.rs:400,409` carry the
  pre-change spelling), and `fixtures/ordinary_graph_controls.rs:32`
  matches `LegalizedScalarTerminator` without the `Crash { .. }` arm the
  variant added. Both repairs are confined to
  `tests/native-differential/tests/pipeline_ownership{,.rs}`, which is
  live-fenced to STRUCTURAL-UNIT-CALL-GRAPH-JOINS (exp 20:13Z) — no
  unclaimed slice exists this wave; the repair ownership stays with that
  claim's holder and the lane's underlying red legs are enumerated below.

Language/semantic gaps:

  **CLOSED by measurement 2026-09-21 (macOS arm64):**
  `cargo nextest run -p omega-native-differential-test --test pipeline_ownership
  --no-fail-fast` is **392/392**. Both recorded compile breaks are repaired
  upstream -- `optimized_target_owner()` and the
  `LegalizedScalarTerminator::Crash` arm -- so the harness leg this row records
  as unbuildable now builds and passes whole. No code change was needed; the row
  was stale, not blocked.

- **FUZZ-CLUSTER-ZERO-BYTE-ARRAY.** Resolved — the empty fixed byte array is a first-class value: `[u8; 0]` admits at check in locals, constants, parameters, returns, record fields, and nested arrays, constructed exactly by `[]` or `""` (`pass/collections/zero_length_byte_array_admission`, `zero_length_byte_array_is_admitted_at_check`). The use-site fences are pinned: no provable index (`x[0]` → "cannot prove index `0` is within length 0"), and fixed-array literals must supply exactly 0 elements/bytes (`fail/data/zero_length_byte_array_{index_rejected,literal_arity_rejected}` and `zero_length_byte_literal_length_rejected`, driven by `zero_length_byte_array_use_fences_reject_at_check`). Non-scalar-leaf `[T; 0]` stays fenced by `InvalidStructuralArrayLength` in the terminal verifier (settled by BASELINE-VERIFIER-ZERO-BYTE-ARRAY-FENCE); a native-route corpus pin for it belongs to the ACTIVE_FAIL roster. Verified: `cargo nextest run -p compiler --test canary_suite` scoped to the two new tests — 2/2 green on linux x86-64 at
`ff596a06e6`; re-verified 3/3 zero_length canaries (admission, use-fence
rejection, native-route `InvalidStructuralArrayLength` pin) green at
`50cd9a2769`.
- **CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN.** Resolved — the umbrella
  row for the cross-package dynamic-evidence loan-origin cluster closed
  by SHARED-RECEIVER-LOAN-ORIGIN (resolved at `e76d715c8e`). The
  recorded failure
  `cross_package_visibility::public_dynamic_return_may_carry_private_
  producer_selected_evidence` ("state `code` requires an exact retained
  loan origin for its shared receiver") passes after the
  retained-lineage/borrow-evidence family landed; re-verified on this
  host: all 21 `cross_package_visibility` tests pass at `dcfb595098`
  (linux x86-64) with zero loan-origin diagnostics. No independent
  slice remains. Sibling stubs on the same surface:
  PACKAGE-CROSS-VISIBILITY-LOAN-ORIGIN (resolved),
  PACKAGE-DYNAMIC-RETURN-LOAN-ORIGIN, DYNAMIC-RECEIVER-LOAN-ORIGIN
  (resolved — receiver loan-origin checks live in
  `checks/borrows/calls/receiver.rs` and cover shared and dynamic
  receivers alike; the dynamic-return witness passes with zero
  loan-origin diagnostics, re-verified green on the same 21/21
  `cross_package_visibility` run at `2a07fef5a853` (z148). Its earlier
  `checks/borrows/` fence expired unlanded; the directory is now held
  by BORROW-PROOF-CONVERGENCE, exp 06:46Z).


- **NEW-RBRA-PASS-CALLS.** Inserted row, scope verified at `416e9dd7e6`
  (planner-scoped to `tests/omega/pass/calls`) — no pending work assignable
  to this name on the scoped surface: the directory holds 190 fixtures,
  every one carrying `main.omg` (`calls/` is referenced 346 times across
  `canary_suite.rs`'s rosters); 182 are package-form members carrying
  `build.omg` and 8 are compile-only. Witness on Linux x86-64:
  `OMEGA_PASS_CANARY_FILTER=calls/ cargo nextest run -p compiler --test
  canary_suite
  entry_and_abi::pass_canary_coverage::pass_canaries_compile` ran at
  `416e9dd7e6` — 174/190 green; 16 members fail compile, every diagnostic
  inside an already-owned baseline family: nine on "selected ProgramEntry
  establishment rejoins 0 Terminal attachment identities … omitted at
  local construction" (the recorded in-window missing-plan family owned by
  SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT / PROGRAM-ENTRY-SELECTION-EXACTNESS
  — `runtime_{inline,value_call_direct,value_call_statement}_recursive_walk_exit`,
  `runtime_mut_ref_forward_exit`, `runtime_call_enum_field_value`,
  `runtime_call_enum_value`, `runtime_nested_named_conversion_alias_exit`,
  `runtime_contained_call_value`, `runtime_transition_argument_call_value`),
  four on "native-artifact production requires one exact selected program
  entry" (`runtime_call_enum_field_{with_args,with_mut_arg}`,
  `runtime_call_enum_sequence`, `runtime_transition_subject_call_guard`),
  one on the host-gated Fused-provider refusal
  (`runtime_arm_target_host_result_exit` — `FilesystemHost`), one on
  receiver provisioning (`nested_machine_continuation` — "no executable
  nominal cleanup"), and one on the missing transitive machine plan
  (`runtime_call_guard` — "has no admitted body"). The `RBRA` token occurs
  nowhere in the tree or boards; the series is a pass-corpus
  surface-migration sweep whose deltas belong to the owning rows. Nothing
  to implement under this name until a concrete contract identifies the
  migration.

- **NEW-RBRA-PASS-TERMINATION.** Inserted row, scope verified at `891eb5c584`
  (planner-scoped to `tests/omega/pass/termination`) — no pending work on the
  scoped surface: the directory holds 114 fixtures, every one carrying
  `main.omg` and named on a canary roster (`termination/` is referenced 234
  times across `canary_suite.rs`'s checked-only/active/fail rosters); four
  package-form members carry `build.omg` and 18 carry `README.md`. Witness on
  Linux x86-64: `OMEGA_PASS_CANARY_FILTER=termination/ mbx nextest run -p
  compiler --test canary_suite` pass-coverage gate green at `891eb5c584`.
  The `RBRA` token occurs nowhere in the tree or boards; the only named
  siblings in the series are NEW-RBRA-PASS-RECAST-GENERICS (holds
  `tests/omega/pass/{recast,generics}`) and NEW-RBRA-STD-LIBRARY-MIGRATION —
  the series appears to be a pass-corpus surface-migration sweep, and this
  corpus is already fully rostered and green. Nothing to implement under
  this name until a concrete contract or failing customer identifies the
  delta.

- **BASELINE-NATIVE-DIFF-TERMINAL-PSI-SOURCE.** Resolved — the lane is fully
  green: `cargo nextest run -p omega-native-differential-test --test
  terminal_psi_source --no-fail-fast` reports 90 run / 90 passed / 0 skipped
  on linux x86-64 at `17fec446ef2` (the emitted images install and execute
  natively, so the three named failures are repaired, not skipped). The
  repairs arrived through the harness-migration lane: `aecc5533d35` moved the
  hosted receiver harness into the differential tests, then `30f4189a58c`
  (retired artifact policy), `8d9fa4ca8e7` (effect handler on terminal
  resume), `294b6cfbf4a` (erased-formals scalar term lane), `27f345e527a`
  (direct Unit parameter custody gate), `b972133cade` (checked program-entry
  route) and `c9e496b426a` (complete emitted image text) staged the target to
  match.
  Re-check this row by TARGET NAME, not `--all-targets`: the owning crate does not
  build clean as a whole, so the obvious command reports a build failure and hides
  the green lane. At `17fec446ef23`, `cargo check -p omega-native-differential-test
  --all-targets` still fails with five errors, every one in the unrelated
  `pipeline_ownership` target — four `E0308` against `optimized/validation.rs`'s
  `Arc` handle (`83766d57bf6`) and `E0004` for `LegalizedScalarTerminator::Crash`
  (`bf8769cce13`). The crate is excluded from the landing gate
  (`--exclude omega-native-differential-test`), so nothing else watches it. The
  sibling `--test terminal_psi_source_payloadless_optimizer` is also green, 3/3.
- **FLOAT-IDENTITY-LITERAL-CARRIER.** Complete admitted floating constant
  evaluation/materialization under [constants](wiki/spec/language/constants.md#materialization).
  Four arithmetic operators, six comparisons and public/imported constant
  identity already have a connected route. Reuse it, preserving exact
  declaration/import identity and one-time format landing.

  Separate payloadless NaN meaning, usable in proof and compile-time
  computation, from runtime bytes requiring canonicalization, explicit bits
  or an exact selected realization. `const_generic_expressions/value.rs`
  and `const_initializers/materialize.rs` still reject computed NaNs at
  their current boundaries. Complete that distinction without choosing an
  arbitrary payload or inventing new literal syntax. For the remaining
  selected named-operation customers (classification, conversion, directed
  rounding and fused operations), first exercise the existing general
  evaluator; the binary-operator match alone does not establish which calls
  are unsupported.

  Acceptance: those source-authored constant customers evaluate and replay
  with exact selected custody; determined runtime results publish stable bits,
  while undetermined representations and forged receipts reject. A
  payloadless NaN may participate in admitted compile-time/proof reasoning
  without materializing. Retain signed-zero, format and imported-alias controls.
  Owners: build-time evaluation's constant expression, materialization and
  replay paths, with shared FloatSemantics rather than a separate arithmetic
  definition.
- **STRUCTURAL-UNIT-LOWERING.** Scope verified on `a4ffd1aff8` — structural-unit
  lowering in checked-trees-to-lowered-psi is landed for the bounded subset
  (`unit/structural_unit_control.rs`: multi-state claim-free affine structural
  control, two-frontier joins, ranked `TerminalRankedScc` countdown; structural
  arguments also ride `scalar_structural_calls` and `expression_preparation`
  structural bindings). The remaining gaps are deliberate capacity fences, all
  inside `src/unit/`: at most two checked conditional states, joins admit
  exactly two incoming frontiers and at most one join state, graphs must be
  acyclic outside the ranked countdown lane, and specialized scalar successors
  accept only parameter-sourced arguments ("specialized scalar successor does
  not support checked expressions"). Every implementing surface is live-fenced
  this wave: `src/unit` is claimed under UEFI-OS-HANDOFF (exp 20:00Z) and the
  native continuation — the `UnsupportedControlFlow(MachineId(..))` fence the
  five `structural_units` pipeline-ownership legs stop at — is claimed under
  sibling **STRUCTURAL-UNIT-CALL-GRAPH-JOINS** (exp 20:13Z). No independent
  unclaimed slice exists here this wave; the residual stays on the claim
  holders above.
- **STRUCTURAL-UNIT-CALL-GRAPH-JOINS.** Call-graph joins for structural units.
- **STAGED-LOCAL-SEQUENCE-LOWERING.** Staged-local sequence lowering attribution and order.
- **CANARY-EXACT-ENTRY-SELECTION.** Exact entry selection for division/value canaries and entry binding.

Omega-side / native:

- **X86-FMA-PROVIDER-TRANSPORT.** Carry checked/legalized scalar FMA
  occurrences through ordinary selected-instruction transformation,
  register-home assignment, machine planning and image publication.
  Reuse the feature-custodied encoder in `machine-emission/src/x86_fma.rs`
  and independent artifact readers; their test-driven fragments are not a
  connected provider-execution route.

  Preserve exact operation/operand/result identities, selected provider and
  AVX+FMA3 admission, XMM homes, canonical MXCSR save/install/restore, and
  per-occurrence physical coverage. Remove the object, entry and optimization
  fences only as their required transport is independently checked, not by
  bypassing them. The production native realization still rejects retained
  FMA before instruction selection.

  Acceptance: the source-evaluated-import/FMA customer pinned by
  `retained_x86_fma_and_source_evaluated_import_stop_at_fma_transport`
  publishes and executes on a matching admitted host, including nested
  foreign-call control custody. Both formats distinguish fused from
  separately rounded cancellation; absent/wrong-profile feature admission,
  operand/provider substitution and corrupted emitted evidence reject.
  Report cross-target byte replay separately from hardware execution.

Proof/evidence:

- **PROOF-INTERCHANGE-IMPORT.** External proof interchange: sort encoding,
  induction certificate, arithmetic import (3 mined aliases merged).
  Re-verified at `8e870505f7`: all cited surfaces still hold —
  `tools/matching-logic-sort-encoding/sort_encoding.py` (check subcommand
  live), `proof-admission/src/admission/recursion.rs`
  `verify_recursive_component` (the row's `proof/` path corrected: the
  carrier certificate lives in the proof-admission crate), `classicality.rs`
  `AcceptedProofRule::foundation`, and `admission/` still carries only
  evidence/normalization/recursion routes — no external import route has
  appeared.
  Original audit (`e76d715c8e`) — per-alias disposition:
  (a) **sort encoding** — landed by MATCHING-LOGIC-TYPED-TO-ONE-SORTED-ENCODING:
      `tools/matching-logic-sort-encoding/sort_encoding.py` emits the clause
      inventory + evidence record and `check` enforces definedness coverage,
      intended-model inhabitedness, revision refinement, loan disjointness,
      injectivity/tag disjointness, memberships, and fixpoint certificates
      (test 8/8 green); remaining acceptance = wiring into the bounded
      comparison harness, fenced to MATCHING-LOGIC-BOUNDED-SLICE's live
      claim on `tools/matching-logic-slice/`;
  (b) **induction certificate** — the internal carrier certificate is
      already landed (admission/recursion.rs `verify_recursive_component`,
      per INDUCTIVE-CARRIER-CERTIFICATE's verified row); the external
      matching-logic certificate has no importer;
  (c) **arithmetic import** — `proof-admission` owns internal checked
      integer rules (closed_integer, affine, cast, forbidden_root, shift)
      plus the `mathematical_core` term model + kernel; no external
      arithmetic import route exists — `admission/` has only
      evidence/normalization/recursion routes, and
      `AcceptedProofRule::foundation` admits a single trusted admission
      (`SemanticAxiom`), pinned by
      `no_accepted_rule_stands_on_a_classical_foundation`.
  None of the three authorizes implementation: each route first needs
  MATCHING-LOGIC-BOUNDED-SLICE's bounded comparison, then a concrete design
  (a kernel replacement needs its own proposal + end-to-end proved bridge),
  and imported rules must respect the classicality boundary. An
  independently checked translation — not a trusted import — is the only
  sound route, per MATCHING-LOGIC-EXTERNAL-PROOF-IMPORT's verified row.
- **PROOF-RULE-CLASSICALITY-AUDIT.** Audit proof rules for classical/constructive boundary (matching-logic lane). Landed: `wiki/spec/proofs/classicality.md` audits every certificate rule — all are constructive or constructive-by-decidable-domain, `SemanticAxiom` is the only trusted admission, and the proposition grammar cannot express a classical principle. `AcceptedProofRule::foundation` (`proof-admission/src/classicality.rs`) enforces the classification by exhaustive match with tests pinning the boundary. Resolved at `bc772bf7cd7e`: the two stated remaining legs landed at `d0044250f357` — the doc now classifies the obligation-side lemma library and deciders in `psi/semantics/proof` (all constructive-decidable / plan-formation / instrumentation; no decider decides an undecidable relation) and the verifier's semantic-axiom reconstruction inventory row-by-row under `terminal-verifier/src/trusted_surface/` (certificate-gated rows are the only non-trusted emissions and rest on already-classified rules). A residual `use super::*` in `classicality.rs`'s test module is GLOB-SELF-IMPORTS-REPAIR's accounting, fenced to PROOF-KERNEL-CORE (exp 09:49Z).
  covered — audit landed in `wiki/spec/proofs/classicality.md`; residual glob import is GLOB-SELF-IMPORTS-REPAIR's
- **MATCHING-LOGIC-BOUNDED-SLICE.** Bounded matching-logic slice.
- **MATCHING-LOGIC-EXTERNAL-PROOF-IMPORT.** External proof import for matching logic. Source docs `wiki/drafts/matching_logic.md` and `wiki/drafts/matching_logic_sort_encoding.md` are exploratory research notes that authorize no implementation: a checked source proof with a trusted translation still carries a translation admission, an imported statement alone is a foreign-theorem admission, and no independently checked translation exists. Any route first needs the doc's bounded comparison (MATCHING-LOGIC-BOUNDED-SLICE), then a concrete design — a kernel replacement additionally requires its own proposal and an end-to-end proved bridge — and imported rules must respect the constructive/classical boundary `AcceptedProofRule::foundation` enforces (`proof-admission/src/classicality.rs`). The merged interchange territory belongs to PROOF-INTERCHANGE-IMPORT (sort encoding, induction certificate, arithmetic import). Landed (measurement pin): `terminal-codec` test `arithmetic_import::the_imported_arithmetic_derivation_has_a_measured_size_and_step_cost` pins the doc's size/checking-cost evidence row — the imported arithmetic certificate encodes to exactly 716 bytes and re-verification consumes exactly 6 conversion steps (a budget of 5 rejects with `StepCeiling`, 6 suffices). The axiom-closure and inductive-carrier rows were already pinned in the same file / `theorem_certificate.rs`.

Build/packages:

- **BUILD-PACKAGES-GATE.** RC build-and-packages gate closure work.

Platform/cross-host (structurally gated — document host limits):

- **MACOS-X64-HOST-PROFILE.** macOS x86_64 host profile (Intel gap).
  `TargetProfile::MacosX64` catalogues the host (`macos_x86_64` / `MacosX86_64`,
  `NativeTarget` x86-64 + Mach-O), so `host()` resolves there and the profile
  survives into checked admission, which refuses on the missing
  `targets/macos_x86_64` provider package. Landed: the `targets/macos_x86_64/`
  provider package (SysV AMD64 `MacosPhysicalEntry`/`MacosX64Application`
  entry contract over dyld's four-argument `appMain`, console/process-exit/
  filesystem providers, and `core/targets/macos_x86_64/float_impl.omg`), the
  x86-64 Mach-O writer (`MachoIsa` parameterizes the emitter over CPU type,
  page size, thunk form, and relocation application; `image_output.rs` admits
  `(MachO, X86_64)`), and per-ISA validation dispatch (loader mapping, object
  fixups, thunk↔binding-slot pairing) including PCC `native_evidence` arms.
  Dispatch legs landed: `ProgramEntryPhysicalContractPackage::MacosX64` +
  the `program_entry_slot` row (`HostedApplication`, `MacosX64Application`
  boundary schema, `MacosPhysicalEntry::enter`, SystemVAMD64 physical and
  semantic conventions) in `target/src/lib.rs`; the exact contract module
  `exact_macos_x86_64.rs` replaying the authored four-parameter SysV
  boundary plan (rdi/rsi/rdx/rcx → eax) and its package source digest;
  `AcceptedSemanticBindingRole::MacosX64ProgramEntry` plus its
  build-evaluation role/digest arms, package-manager candidate arm, and
  selected-dispatch accepted-role list; the `macos_x86_64` arms in
  `hosted_receiver.rs` (`physical_contract_matches` + the
  SystemVAMD64/`X86Rdi` `receiver_layout` arm); the per-ISA
  installation-record pairing dispatch in `record_validation.rs`
  (`MachoIsa::X86_64` selects `validate_macho_x86_64_import_binding_pairing`,
  ending the aarch64-only fail-closed path); and the
  `cfg(macos, x86_64)` `native_hosted_target()` arm in `canary_suite.rs`.
  The two compile-forced fingerprint arms in `target/src/uefi_boot_services`
  and `target/src/uefi_system_table` (`MacosX64 => 6`) ride along; the UEFI
  claim's listed paths name a `backend/target/` crate that no longer exists.
  86/86 `target` + `program-entry-plan` tests pass on linux x86-64.
  Remaining leg: a real x86_64-apple-darwin host run (requires the Intel
  host; this session ran on linux x86-64).
  **Re-measured 2026-09-21 on macOS arm64: all four code legs listed below as
  remaining are landed, and only the host run is left.** Counted in tree:
  `ProgramEntryPhysicalContractPackage::MacosX64` has 12 references,
  `AcceptedSemanticBindingRole::MacosX64ProgramEntry` 7, `target/src/lib.rs`
  carries 20 `MacosX64` occurrences, the hosted-receiver bridge arm is present
  in `image-emission/src/hosted_receiver.rs` (`MacosX64`, `SystemVAMD64`,
  `X86Rdi`), and `canary_suite.rs` has the
  `cfg(all(target_os = "macos", target_arch = "x86_64"))` `native_hosted_target()`
  arm. The provider package `source/library/std/targets/macos_x86_64/` and
  `core/targets/macos_x86_64/float_impl.omg` both exist, and
  `core/float_operations.omg` imports the latter alongside the other four
  targets. The exact contract module `exact_macos_x86_64.rs` is in
  `program-entry-plan/src/program_entry_physical/`. Second-host witness:
  `cargo nextest run -p target -p program-entry-plan` is **87/87** on macOS
  arm64 (the row previously recorded 86/86 on linux x86-64 only).

  The stale paragraph this replaces listed those four as remaining and fenced;
  the fences it named have long rotated and the work landed.

  Remaining leg: a real x86_64-apple-darwin host run. **Host-blocked, not
  design-blocked** — it needs an Intel Mac, and no audited x86-64 Mach-O Alpha
  seed exists either (`tools/bootstrap/alpha/seed_env.sh` ships exactly three
  containers: `alpha_arm64_macos`, `alpha_x64_linux`, `alpha_x64_windows.exe`,
  which is why `Darwin-x86_64` is correctly absent from
  `ALPHA_SEED_EXECUTABLE`).
  Fence re-audit at `f6bb8e6c2e` (INTEL-MACOS-HOST-PROFILE — the retired
  re-mine stub, verified named alias of this row; its scope paragraph
  orphaned under HOSTED-INLINE-ASSEMBLY-AUTHORITY is folded here): the
  recorded dispatch-time fences have rotated. UEFI-PHYSICAL-SEMANTIC-ENTRY's
  live claim no longer covers `representations/target/src/lib.rs` or
  `program-entry-plan/src/program_entry_physical/` (scoped to the UEFI dirs
  + `optimized_semantic_entry`), so the enum arm + slot row are nominally
  unfenced — but not landable: the new
  `ProgramEntryPhysicalContractPackage::MacosX64` forces
  `AcceptedSemanticBindingRole::MacosX64ProgramEntry` in
  `package-compilation/src/semantic_bindings.rs`, which is held by
  OPTIONAL-STDLIB-SEMANTIC-BINDINGS (dev-88738, exp 04:23Z), while its two
  match consumers in `build-evaluation/admission/selection.rs` are unfenced.
  The `native_hosted_target()` cfg arm's file is held by
  DOMAIN-ISSUER-ROUTES (swarm-w9-ffival, exp 11:40Z); the hosted-receiver
  bridge arm stays fenced by PLAN-LAID-VIEWS (zergling-z27, exp 09:25Z);
  the Intel-host run remains hardware-gated. Earliest reopen ~04:23Z for
  the entry-contract leg.
  Re-verified at `577d6ac2ba`: the enum-arm leg is transitively fenced, not
  just dispatch-time fenced — adding `ProgramEntryPhysicalContractPackage::
  MacosX64` breaks exhaustive per-package matches inside
  `target/src/uefi_boot_services/mod.rs:568` and `target/src/uefi_system_table/
  mod.rs:593`, both inside UEFI-PHYSICAL-SEMANTIC-ENTRY's live claim
  (exp ~07:51Z), so no leg compiles without editing fenced files. The
  `hosted_receiver.rs` and `record_validation.rs` fences recorded above
  (ENTRY-CONTENT-ROOTS, FAULT-INJECTED-TARGET-READER) have expired, but
  both arms' content is the enum variant's downstream consumers and cannot
  land ahead of it. The `canary_suite.rs` arm stays fenced —
  `compiler/tests/canary_suite.rs` is held by CLEANUP-HOOK-SELECTION-
  AND-ERASED-OWNERSHIP (exp ~03:42Z). A slice was drafted and reverted
  (enum arm + `program_entry_slot` row + `program_entry_plan` exact
  replay module + `hosted_receiver` bridge arm); it lands after the UEFI
  fence's claim adds the covering arms or expires.
- **EPOCH-RESOURCE-SNAPSHOTS.** Check epoch-attributed aggregate/resource snapshots
  against the authoritative live-era roster in `effects::ComponentEraEntryLedger`
  and `external-roots` program-local accounting. Acceptance: snapshots match the
  live era's retained resources; foreign or stale era attribution rejects.
  Deployment journals and restart reconstruction belong to the consuming runtime,
  not this task; see [deployment ownership](wiki/spec/build/component_publication.md#deployment-ownership).
- **DEVICE-EXTENT-ACCESS.** Scope verified at d32183a35c — device extent
  access is `wiki/spec/resources/device_access.md` on
  `psi/foundation/extents`. Landed: the external-loan machinery
  (`external_loans/mod.rs`) carries DMA borrows end to end — grant binds
  borrower/direction/space/provenance/rights/completion obligations,
  direction check (DeviceReads→shared loan, DeviceWrites→exclusive),
  exact confinement `ExternalReachReceipt`, and `complete()` binding the
  live loan + receipt + borrower + direction + range. Missing: the
  "Ordering roles" section — publication, cache maintenance,
  notification, completion, and acquisition as distinct protocol roles
  whose discriminant participates in canonical event identity ("equal
  payloads for different roles are not interchangeable"), each event
  binding its exact ranges, mappings, stable device instance, and
  runtime queue/session scope. No ordering-role carrier exists in the
  crate — that is the implementable slice (a `DeviceOrderingRole`
  discriminant + bound event identity alongside `external_loans`).
  Shared-memory IPC and proven-release CPU-view restoration also remain
  spec-only. Claim attempt exited 2: RUNTIME-SIZED-ACTIVATION-STORAGE
  holds `omega-rust/psi/foundation/extents` until 01:34Z
  (Devin / z139-runtime-sized-activation-storage, e3f1f55b).
- **EXTERNAL-DATA-SCHEMA-CONVERSION.** External data schema conversion.
  Landed: `require_wire_compatibility` CompleteMigration now certifies only a
  uniquely bound conversion route — a `FormatMigration` lineage edge bound by
  more than one machine fails migration coverage as ambiguous instead of
  letting the route search pick whichever binding it reached first
  (`wire/wire_compatibility_migration_edge_ambiguous`). Remaining: no codec
  publishes preserving decode (`PreserveUnknown` demands are always
  unsatisfiable); the preserving-decode mode and its remainder custody
  (`Relayed<T>`/`OpaqueWireRemainder` in `wiki/spec/layouts/codecs.md`) are the
  next slice.
  Verified at `ded56393da2` (linux x86-64): the admission half is already
  functional — `wire/wire_compatibility_preservation_met` re-witnessed green
  (OMEGA_PASS_CANARY_FILTER, 1/1 PASS, 48s); an authored
  `PreservingDecode<Policy, Value>` conformance satisfies the demand via
  `admission/wire_protocol.rs::published_preserving_decode`, and `wire.omg`
  already declares `PreservingDecode`, `Relayed<T>` and `OpaqueWireRemainder`.
  The remaining work is the producing half, not detection: the only wire
  decode machinery is the legacy synthesized strict `Schema::decode`
  (`value_custody/wire/decode_call.rs`, v0 subset — i32/i64/u32/u64/bool,
  borrowed text, one-level nested schema); `decode_preserving` is recognized
  nowhere in validation, the checked interpreter (`evaluator/wire_codec.rs`),
  or native codegen. A real preserving decode must validate known members,
  capture unknown-member bytes + ordering sidecar, and bind codec identity
  into `OpaqueWireRemainder` — a multi-crate slice (validation + interpreter
  + codegen), larger than a single bounded leg. Re-verified at
  `4c8ebd7ba8c`: `2b000b60660` landed the admission route's current shape —
  the report scans machine conformances for
  `PreservingDecode<Policy, Local>::decode_preserving` per schema row
  (`wire_protocol.rs:410-448`); `decode_preserving` remains recognized
  only in admission + the `wire.omg` trait declaration, so the frontier
  and remaining legs are unchanged.

Squalr app lane (source: `samples/apps/squalr/TASKS.md`):

- **SQUALR-CLI-COMMANDS.** Scope verified 2026-09-20: submodule item
  `CLI-COMMANDS` (`samples/apps/squalr/TASKS.md`) — port the request/response
  model through squalr-engine-session, squalr-engine and squalr-cli.
  Dependency-blocked, not implementable this wave: the submodule's list is
  ordered execution and this row sits after **SUPPLIED-BYTES-SCAN**, which
  owns the production scan engine this item's acceptance exercises
  ("creates a scan, filters it again and pages exact results through the
  production engine"). Today squalr-engine, squalr-engine-session and
  squalr-cli contain only `build.omg` stubs — there is no engine or session
  surface to route commands through, and the submodule's AGENTS.md forbids
  success stubs or fixed-capacity substitutes. The Squalr integration
  umbrella is also under live claim (SQUALR-HEADLESS, Codex). Real start
  condition: SUPPLIED-BYTES-SCAN lands its engine-api/scanning port, then
  this row ports the command model on top.
  Re-verified at `832c55e69b7` (ffival): dependency state partially moved —
  the submodule pin `5b0307c` now carries the SUPPLIED-BYTES-SCAN surfaces
  (27 .omg across squalr-engine-api + squalr-engine-scanning: scalar
  scanners, element_scan_dispatcher, snapshot filters, RLE encoder), but
  that row stays listed-open in the submodule's ordered list and this
  item's route-through crates are still stubs: squalr-engine and
  squalr-cli carry `build.omg` only, squalr-engine-session has only
  `engine_os_provider.omg` — no request/response command model exists to
  port. Submodule dir was UNFENCED this pass (claimed 8ac5e796 for the
  witness, released); AGENTS.md still forbids success stubs. Real start
  condition unchanged.

- **SQUALR-ALIGNMENT-STRING-PARSING.** Scope verified at `949c153acd73` — the gap is
  real and narrow, and it is an alias of a named parity gap on the sample's own board.
  `samples/apps/squalr/TASKS.md` lists it among four remaining porting gaps ("Rust
  debug-only assertions, alignment string parsing, the set_alignment call-site gate
  ... and named trait operators") beside the recorded evidence that all 12 authored
  geometry checks pass natively on macOS ARM64.
  Measured in the ported source: `squalr-engine-api/src/structures/memory/memory_alignment.omg`
  declares `pub data MemoryAlignment [copy]` with `MemoryAlignment::default()`,
  `MemoryAlignment::from(size: i32)` and `get_size_in_bytes`, and
  `structures/memory/normalized_region.omg` declares `NormalizedRegion::set_alignment`.
  There is no string entry point at all — zero `parse`/`from_str`/`from_string` over
  any alignment type in `samples/apps/squalr/**/*.omg`. So the port can build an
  alignment from an integer but not from authored text, which is what a CLI argument
  needs. The residue is one parse machine plus its rejection cases, not a structural
  port.
  NOTE for anyone picking this up: `samples/apps/squalr` is a git submodule
  (`https://github.com/CathedralOS/Squalr-Omega.git`) and is NOT initialized in a
  fresh checkout — it reads as an empty directory and every SQUALR row looks
  unverifiable until `git submodule update --init samples/apps/squalr` runs. The
  sibling SQUALR-REGION-ALIGNMENT-EXPANSION row shares that precondition.
- **GEOMETRY-ALIGNMENT-PARSING.** Implemented on submodule branch
  `zergling/z157-squalr-alignment-parsing` (tip `833ce36`, commits `c587ab4`
  + `833ce36`) — the parse machine named by SQUALR-ALIGNMENT-STRING-PARSING's
  residue. `memory_alignment.omg` gains `MemoryAlignment::from_str(text:
  &[u8]) -> AlignmentParseResult` porting upstream `FromStr` verbatim: only
  the spelled bytes "1"/"2"/"4"/"8" parse; every other length or byte refuses
  via `AlignmentParseResult::Invalid` (first case — an untouched value reads
  as failure). Named deviation: upstream `Err` echoes the input text; a data
  case cannot retain a borrowed byte view, so `Invalid` drops the echo.
  `AlignmentParseResult::parsed_size` and `from_str_consistent` exercise all
  four spellings plus both refusal shapes in-package, matching the
  wire-codec exercise convention (borrowed-slice receivers do not cross the
  package boundary under selected ProgramEntry establishment). Verified at
  `e7c0099cb2b70` on Linux x86-64: `omega --check` compiles all 30 package
  sources clean against the bundled std. One implementation fix surfaced
  during checking: `text.len == 1` does not establish index bounds — the
  admissible shape is `text.len > 0` enabling `text[0]`, then a state-level
  `len == 1` guard (the `console_write_bytes` pattern). Parent pin NOT
  bumped — submodule integration is the coordinator merge step; the checked-in
  `squalr-tests` `omega.lock` will need ordinary update/review against the
  merged pin. Workspace `omega update` on this host exceeded 15min twice
  (recompiling all package candidates) and produced no review file; the
  direct package check is the scoped witness.
  Re-verified at `e1dc35c9294` (linux x86-64, 2026-09-21): the submodule
  branch `zergling/z157-squalr-alignment-parsing` remains published at tip
  `833ce362927c` on the Squalr-Omega remote, so the landed parse machine
  stands exactly as recorded. The parent pin bump stays the coordinator
  merge step — no new in-scope slice exists under this name; the row is
  stamped, not reopened.
- **SQUALR-CLONE-SERIALIZATION-PARITY.** Clone serialization parity. Scope
  verified at `10d93dd448`, fenced — the residual the app board lists under
  GEOMETRY-PARITY ("clone/serialization", samples/apps/squalr/TASKS.md):
  upstream Squalr `568aa7589b68` derives `Clone`/`Serialize`/`Deserialize`
  across its structures and snapshot types (region filters, normalized
  regions, scan results) so scans can be cloned for successive filtering and
  snapshots serialized to project files; the port carries only three
  structures modules so far
  (`squalr-engine-api/src/structures/{scanning/filters/snapshot_region_filter,
  memory/normalized_region, memory/memory_alignment}.omg`) with no
  clone/serialization leg ported yet. The implementing surface
  `samples/apps/squalr` is dir-fenced by SQUALR-TARGETS-AND-THROUGHPUT
  (~21:39Z) and GEOMETRY-ALIGNMENT-REGIONS (~01:18Z); sibling stub
  SQUALR-CLONE-SERIALIZATION (below, ~7395) re-mines the same residual.
  Claim evidence (z181): claim returned exit 2 on the fenced app dir.
- **SQUALR-GEOMETRY-PARITY.** Geometry parity gaps + debug assertions.
  Re-witnessed `ac4e4eee9b` (z194, Linux x86-64): `Squalr geometry: PASS`,
  native exit 0 through `tools/verify.py native` on the scratch-copied app
  at pin `43329a3` — using `Source::Path` to the checkout's
  `source/library/std` (the post-migration std), since the declared
  git-pinned std `87d8b227` predates `32f5182254` ("reject bare
  boundary-trait value spellings") and now fails `omega update` with 5
  diagnostics (`host: FilesystemHost` etc. are `Service<R>` today). The
  tracked `squalr-tests/omega.lock` is likewise rejected at HEAD
  ("unsupported package lock version"), so BOTH recorded re-entry paths —
  lock-bound run and fresh `omega update` — are red until the submodule's
  std pin and lock advance to a post-`32f5182254` revision; that edit is
  inside `samples/apps/squalr`, wholesale-fenced at this verification
  (GEOMETRY-ALIGNMENT-REGIONS, exp 01:18Z).
  Verified-scope audit (z105, origin/main `e8bbe9fcc0`): the geometry lane's
  authored evidence is complete on one host — submodule TASKS records all 12
  authored geometry checks passing on macOS ARM64 at app `4b1f7a6` with std
  `87d8b227` (`Squalr geometry: PASS`, native exit 0); "Windows was not run",
  and a Windows host is not available in this lane. The enumerated residual
  gaps each map to sibling rows rather than remaining open here: Rust
  debug-only assertions (SQUALR-DEBUG-ASSERTION-PARITY /
  SQUALR-DEBUG-ASSERTIONS stubs at ~7541-7542), clone/serialization
  (SQUALR-CLONE-SERIALIZATION-PARITY), alignment string parsing
  (SQUALR-ALIGNMENT-STRING-PARSING), region alignment/expansion
  (SQUALR-REGION-ALIGNMENT-EXPANSION), named trait operators
  (SQUALR-NAMED-TRAIT-OPERATORS). Implementing surfaces are under live
  claims: `samples/apps/squalr` wholesale under SQUALR-TARGETS-AND-THROUGHPUT
  (21:39Z), clone-serialization region/filter/alignment sources under
  SQUALR-CLONE-SERIALIZATION (22:50Z), submodule+board under
  SQUALR-REGION-ALIGNMENT-EXPANSION, TASKS.md under SQUALR-HEADLESS
  (00:48Z). This row's own deliverable is therefore the Windows validation
  leg (host-gated) plus witnessing that the sibling gaps closed; no
  linux_x86_64-implementable slice exists inside the claimed surfaces.
  Re-verified at `9f48bb2a59` (z85): the picture is unchanged — every
  enumerated gap row is still open with a live owner
  (GEOMETRY-ALIGNMENT-STRING-PARSING 00:04Z, SQUALR-NAMED-TRAIT-OPERATORS
  00:58Z, REGION-ALIGNMENT-EXPANSION item claim 02:01Z), and
  `samples/apps/squalr` stays wholesale dir-fenced
  (SQUALR-TARGETS-AND-THROUGHPUT 05:52Z, GEOMETRY-ALIGNMENT-REGIONS
  01:18Z, SNAPSHOT-STORAGE ×3 to ~05:2xZ); the lock/std-pin re-entry
  repair remains an edit inside that fence and Windows validation still
  has no host.
  Re-verified at `479ceb0e680` (z171): unchanged shape —
  `samples/apps/squalr` stays wholesale dir-fenced
  (REGION-ALIGNMENT-EXPANSION / zergling-z68, exp 07:25Z), gap rows keep
  live owners (SQUALR-NAMED-TRAIT-OPERATORS item claim ~10:24Z,
  GEOMETRY-WINDOWS-VALIDATION item claim ~13:53Z), and no Windows host
  exists in this lane. The own deliverable remains the host-gated
  Windows leg; no linux_x86_64 slice outside a claimed fence exists.
  Re-verified at `90df29812c0` (z34, linux x86-64): both re-entry paths
  stay red — `omega --check squalr-tests/main.omg` at submodule pin
  `5b0307c3` rejects the tracked lock ("cannot prepare accepted
  omega.lock: fresh source key or immutable content differs") and
  `omega update --offline` refuses the recorded std Git pin
  ("offline resolution forbids new or refreshed Git selection"), so the
  std-pin/lock advance remains the required submodule repair. Claim
  drift: the `samples/apps/squalr` wholesale dir-fence has expired —
  no path claim covers it at this check — while the enumerated gap rows
  keep live item claims (SQUALR-NAMED-TRAIT-OPERATORS ~10:24Z,
  GEOMETRY-WINDOWS-VALIDATION ~13:53Z, SQUALR-SEED-PARITY ~15:14Z on
  `wiki/drafts/squalr_seed_parity.md`). The repair still belongs to the
  port lane (submodule publication + gitlink), and Windows validation
  still has no host; no independent slice exists under this name.
  Re-verified at `8f58b6676b00` (z133, linux x86-64): the gitlink has
  advanced `5b0307c3` → `ef6682f75f48` — the submodule republished the
  lock with std Git pin `13433c1a` (post-`32f5182254`, so the recorded
  std-pin staleness is repaired) and landed the region-alignment ports
  (`52bcf25` NormalizedRegion `set_alignment`/`expand`). Both re-entry
  paths stay red on this host anyway: `omega --check
  squalr-tests/main.omg` still rejects the tracked lock ("fresh source
  key or immutable content differs" — the lock's external-local lineage
  keys the publisher's absolute checkout path, so any other checkout
  needs an `omega update` review to re-key), and `omega update
  --offline` still refuses refreshed Git selection on the same pin.
  The residual ask narrows accordingly: per-checkout lock re-keying
  (online `omega update` review) or a portable-lock repair — not
  std-pin advancement. Fence map refreshed: no `samples/apps/squalr`
  path claim; live item claims SQUALR-NAMED-TRAIT-OPERATORS ~10:24Z,
  GEOMETRY-WINDOWS-VALIDATION ~13:53Z, SQUALR-SEED-PARITY ~15:14Z,
  SQUALR-DEBUG-ASSERTIONS ~16:25Z, SEED-PARITY-ASSERTIONS ~11:09Z.
  Windows leg still host-gated; no independent slice exists.
- **SQUALR-NAMED-TRAIT-OPERATORS.** Exercise `NormalizedRegion`'s declared
  `<`, `<=`, `>` and `>=` bindings from application code in
  `samples/apps/squalr`, preserving base-address ordering, equal-base ordering
  equivalence and size-sensitive `equals`. Compare token results with
  `base_address_order` and retain explicit `Order` conformance selection.
  Reproduce the current native customer failure before assigning compiler
  repairs; the old ProgramEntry diagnostic is not a current observation.
  **OPERATOR-MACHINE-SUPPLY** owns supply/grammar, with ordinary data-call
  custody under **STATE-LOCAL-VALUE-FRONTIER**. Acceptance: Squalr's native
  `ordering` verification executes the token comparisons and named-call
  controls against the current submodule revision. Hash support awaits a
  hash-keyed collection customer, not this task.
- **SQUALR-REGION-ALIGNMENT-EXPANSION.** Region alignment expansion.
- **SQUALR-SEED-PARITY.** Resolved — merged alias of SQUALR-GEOMETRY-PARITY's "finish the mapped Rust behavior still absent from the seed" clause, adjudicated at `a3ab15b7611`. The submodule's TASKS.md carries no seed-parity item; the phrase mines the GEOMETRY-PARITY residual list, whose enumerated gaps are each already a sibling row: alignment string parsing (SQUALR-ALIGNMENT-STRING-PARSING), clone/serialization (SQUALR-CLONE-SERIALIZATION-PARITY), region alignment/expansion (SQUALR-REGION-ALIGNMENT-EXPANSION), named trait operators (SQUALR-NAMED-TRAIT-OPERATORS), Rust debug-only assertions (SQUALR-GEOMETRY-PARITY), and the Windows validation leg plus the std-pin `32f5182254` upgrade (both recorded open inside SQUALR-GEOMETRY-PARITY's verified-scope audit). The implementing surface `samples/apps/squalr` stays with the port's own lane; no independent slice exists under this name. Re-verified at `59610bf809`: the submodule board still carries no seed-parity row, and the surface stays fenced — `samples/apps/squalr` under SQUALR-TARGETS-AND-THROUGHPUT (21:39Z) plus a same-item sibling claim `Jarod / swarm-w9-squalr-seed-parity` (02:08Z). Folded record (the prior ledger cleanup): the Zergling-126 ledger at `7241e02227` independently confirmed via a shallow clone of CathedralOS/Squalr-Omega `main` that the submodule board carries exactly four rows — GEOMETRY-PARITY, SUPPLIED-BYTES-SCAN, CLI-COMMANDS, TARGETS-AND-THROUGHPUT — no seed-parity item; live fences then were SQUALR-NAMED-TRAIT-OPERATORS (10:24Z) and SQUALR-CLI-COMMANDS (15:14Z). At fold time the `samples/apps/squalr` dir fence has rotated to SQUALR-DEBUG-ASSERTIONS (~16:25Z) and the draft itself stayed claimed under SQUALR-SEED-PARITY (~15:14Z).
- **SQUALR-TARGETS-AND-THROUGHPUT.** Targets and throughput. Scope
  verified at `7110606f46e5`: the name re-mines the submodule's ordered
  `samples/apps/squalr/TASKS.md` TARGETS-AND-THROUGHPUT item — port native
  reads using a controlled child process, partial-read handling,
  cancellations, SIMD and parallel execution, with scalar fixtures for
  optimized-result checks and total-allocation/throughput measurement
  through result publication (GUI/TUI/installer excluded). Not
  implementable this leg on two axes: the implementing surface is
  wholesale-fenced — `samples/apps/squalr` under GEOMETRY-ALIGNMENT-REGIONS
  (Zergling-112, ~01:18Z) — and the submodule board orders this item after
  CLI-COMMANDS, which is still unstarted ("the CLI main entry is
  intentionally absent until this work starts; do not substitute a
  success stub"). No independent slice exists; re-check once CLI-COMMANDS
  lands and the dir fence clears.


## Mined items (deep-mine sweep, wave 9)

- **AARCH64-BRANCH-RELAXATION.** Mined candidate — resolved, alias of
  NON-X86-LAYOUT-RELAXATION (its row names this stub verbatim:
  "AARCH64-BRANCH-RELAXATION names this same surface — no separate board
  row"; re-verified `f600f8400b7`): the only functioning pins are the
  rejects — `x86_rel8_selected` rejects `Architecture::Aarch64` as
  `UnsupportedTarget` in catalog.rs and `hosted_sequences.rs` emits the
  out-of-range diagnostic. No authorized implementation surface; resolved
  with the parent row.
- **ALIGNMENT-STRING-PARSING.** Mined candidate — scope verified,
  covered. Bare re-mine of SQUALR-ALIGNMENT-STRING-PARSING: the
  alignment-string parsing gap inside the Squalr app's geometry lane
  (TASKS.md:6086; the parity audit at :6088 attributes it to that row).
  Its only implementing surface is the `samples/apps/squalr` submodule,
  held under sibling claims (SQUALR-TARGETS-AND-THROUGHPUT,
  SQUALR-CLONE-SERIALIZATION, SQUALR-REGION-ALIGNMENT-EXPANSION); the
  residual set_alignment call-site gate is a compiler entry-mechanics
  item tracked under GEOMETRY-PARITY, not this row. Sibling re-mine
  stubs on the same surface: GEOMETRY-ALIGNMENT-PARSING,
  GEOMETRY-ALIGNMENT-STRING-PARSING, SQUALR-ALIGNMENT-STRING-PARSING.
- **ASM-CATALOG-FAMILY-EXPANSION.** — mined candidate; verify scope then

- **ASM-CATALOG-FAMILY-EXPANSION** — mined candidate; verify scope then
  implement. Landed slice: the pipeline-directive family — `serialize`
  (x86_64) / `isb` (aarch64) instruction-stream serialization plus `pause`
  (x86_64) / `yield` (aarch64) scheduling hints, all UserChecked/NoAuthority
  zero-operand zero-clobber contracts threaded through the catalog, parser,
  builtin table, statement gate, interpreter unit arm, and terminal-authority
  inventory (row count 545->549 + policy commitment re-pinned); canary coverage
  is check-level with pass and expected-reject fixtures. Remaining: byte-level
  emission assertions once native-artifact production accepts asm-only
  entries (currently unreachable at entry selection), the catalog doc family
  table row (fenced elsewhere this wave), and memory/authority-bearing
  families blocked on UnmodeledMemoryAccess and service admission.
  **The inventory step was missed once and took native compilation down**
  (repaired on main by `b1dc444d9dc9` + `6aca16747fe5`): `b8b858b14472`
  ("asm catalog: contract invd, wbnoinvd and nop") added three
  `BuiltinFunction` variants and their classification rows but left
  `CLOSED_POLICY_ROW_COUNT` at 550 against an enumeration of 553. That assert
  sits inside `committed_policy_mechanisms()`, not a test, so the shipped
  `omega` binary panicked on **every native compile**. Measured while it was
  red: `omega --check` was unaffected (exit 0 on a 16-file subject), while an
  ordinary native compile aborted with "assertion `left == right` failed,
  left: 553, right: 550" — taking out native artifact production, `omega run`,
  every native canary and the whole benchmark corpus, with
  `cargo check --workspace` and the architecture suite still green, which is
  why no landing gate caught it. When this family grows, the row count and the
  commitment `policy_identity_binds_version_and_complete_table` pins must move
  together (545->549, then 549->550 for wbinvd, now 550->553);
  `TERMINAL_AUTHORITY_POLICY_VERSION` stays 7 by this row's own precedent. An
  independent recomputation of the new commitment here matched the landed pin
  byte for byte.
- **ASM-CATALOG-MEMORY-AND-CONTROL.** — mined candidate; verify scope then
  implement. Landed slice: refusal-coverage completion for the two named
  families in `language-core/src/inline_assembly/mod.rs` — the hidden-exit
  list gains the x86 near/far/operand-size return spellings (`retn`/`retw`/
  `iret`/`iretd`/`iretw`), far call/jump forms (`lcall`/`callf`/`jmpf`/`jmpl`/
  `ljmpl`), `int1`, the AArch64 branch-consistent `bc` head, and the
  pointer-authenticated branch/return/debug-return spellings (`braa`/`brab`/
  `braaz`/`brabz`/`blraa*`/`eretaa`/`eretab`/`drps`); the unmodeled-memory list
  gains the non-temporal pair forms, RCpc/limited-ordering acquire-release and
  unprivileged-unscaled variants, the plain exclusive-acquire `ldax`, the
  complete LSE read-modify-write ordering grid (`swp*`/`cas*`/`ld*` suffix
  spaces), the 64-byte accelerator block forms, NEON structure load/store
  (`ld1`-`ld4`/`st1`-`st4` and replicate forms), x86 string/port-string bare and
  dword forms (`movs`/`lods`/`stos`/`scas`/`cmps`/`ins`/`outs` + `*sd`), stack
  and flag-store width forms, far-pointer loads (`lds`/`les`/`lss`/`lfs`/`lgs`),
  `xsave`/`fxsave` families, descriptor-table memory operands
  (`sgdt`/`sidt`/`lgdt`), memory-destination non-temporal stores, and `bound`.
  Partly register-only spellings (`movzx`, `bt*`, `smsw`, SSE's `movsd`/`cmpsd`
  shadows) stay unrecognized — mnemonics classify whole. Remaining: real
  contracts for these families are blocked on the UnmodeledMemoryAccess
  operand-provenance model (sibling ASM-MEMORY-AND-TRANSFER-CONTRACTS), the
  AArch64 `dmb`/`dsb` ordering contracts which need a barrier-option operand
  form, service admission for `svc`-class traps and `syscall`/`sysenter`, and
  catalog test-list updates (tests.rs is claimed elsewhere this wave).
- **ASM-INSTRUCTION-CATALOG-EXPANSION.** — mined candidate; verified scope and
  landed one bounded catalog slice at the work branch. Verified shape at
  `b46b34a87f`: the refusal grid is already complete (HiddenControlExit +
  UnmodeledMemoryAccess lists are exhaustive pins), so expansion means new
  CONTRACTED members of existing shapes, not new refusal coverage. Landed the
  two kind-families the catalog's own doc names as its expansion axis —
  `AsmCacheOperationKind` += `invd` (invalidate without writeback) and
  `wbnoinvd` (writeback without invalidate), both serializing X86_64
  MachineOwner zero-operand contracts beside `wbinvd`; `AsmSchedulingHintKind`
  += `nop` (`Any` target, NoAuthority, elidable — the canonical pipeline
  no-op). Per-member realization chain closed end to end: catalog row + kind
  enum + kind helpers (`omega-rust/psi/foundation/language-core/src/
  inline_assembly/mod.rs`), all `BuiltinFunction` sites including stable
  ordinals 77–79 (`asm#invd`/`asm#wbnoinvd`/`asm#nop`), the statement-form
  gate (`machine_calls/calls/call_gates.rs`), the authority-discharge
  mnemonic map (`effects/asm_discharge.rs` — MachineOwner members only;
  no-authority members deliberately unlisted there), the terminal-authority
  classification + inventory test, and the interpreter comment. Parser and
  interpreter arms are kind-generic (`from_intrinsic_name`), so new members
  need no parser change. Witnessed: `language-core`+`symbols`+`validation`+`
  checked-interpreter` nextest green (incl. extended discharge tests: hosted
  `invd`/`wbnoinvd` name machine-owner authority; hosted `nop` admits; the
  former unknown-mnemonic pin for `nop` moved to the contracted list).
  Fixtures: `asm_cache_maintenance_compile` now spells all three cache ops;
  both pipeline-directive pass fixtures gain `nop`; new fail canaries
  `asm_invd_requires_machine_authority` and
  `asm_wbnoinvd_requires_machine_authority` registered in
  CACHE_OPERATION_FAIL_CANARIES. UNWITNESSED this wave: the canary suite —
  `cargo check -p selected-instructions-to-selected-instructions` fails at
  clean origin/main `29983459ec` (E0061: `crossed_window` gained a
  `CrossingDirection` parameter in `block_edges.rs:293` that caller
  `rewrites/relocation/admission.rs:126` never passes; preexisting, fenced to
  the spill/sequencing workers), so `compiler` and everything downstream are
  unbuildable until that lands. Residual legs recorded on the doc: operand-
  bearing cache/TLB ops (`invlpg`, `clflush`) still refuse until a modeled
  memory-operand contract exists; atomics, mode transitions and AArch64
  system ops stay unrecognized per the same axis paragraph.

  WITNESSED at `7a62e962b2a7` (macOS arm64; the E0061 that blocked the suite
  landed as `a1e8298497f3`). `cargo nextest run -p compiler --test canary_suite
  -E 'test(/inline_asm/)'` is **8 passed / 5 failed** of 13. All eight
  checked-semantics and authority-contract legs pass, including
  `cache_maintenance_reaches_checked_semantics` and
  `pipeline_directives_reach_checked_semantics`, so this row's landed catalog
  slice is green where it owns the outcome. The five reds are the x86
  byte-emission legs — `x86_asm_{fences,msr,interrupt_control,control_registers,
  flags}_*` — and none is a catalog defect: each pins an explicit
  `target_name: linux_x86_64` cross-compile to a native artifact and fails
  before emission with `Lowering(InvalidUnitMachinePlan { machine:
  "Main::main", reason: "attached Unit closure is missing a checked transitive
  machine plan", omission: "`Main::main` has no admitted body (local
  construction stopped at statement sequence: call: call operation, statement
  0)" })`. asm statements therefore pass checking and stop at the lowering
  wall. That omission is the same family CANARY-ACQUIRES-THROUGH-HELPER-RETURN
  records (its variant stops at signature rather than at statement 0) and the
  same wall four `terminal_psi_runnable` legs hit in the macos_arm64 native
  differential row; it is not host-specific — the target is named explicitly.
  Closing these five needs the asm-statement lowering arm, not more catalog
  members.

  **Scoped 2026-09-21 — that arm is a multi-crate chain, not a bounded slice.**
  The checked stage models asm instructions as builtin intrinsic calls, and
  `typed-trees-to-checked-trees/src/execution/unit/calls/call_operations.rs:92`
  carries an operation arm for exactly ONE of them: `AsmPortOut` ->
  `CheckedUnitEffectOperationPlan::PortWrite`. The 30-odd other `Asm*` builtins
  (`symbols/src/builtin/mod.rs:189-235`) have none, which an existing test
  already pins deliberately —
  `t2c/src/tests/contracts/assembly.rs`'s
  `asm_value_intrinsic_result_types_reach_the_call_operation_frontier`, whose
  comment says they stop "where no `CheckedUnitEffectOperationPlan` arm exists
  for it yet".

  Adding one is not one arm. Terminal Psi's `OperationKind`
  (`terminal-psi/.../control_flow/operations.rs`) likewise carries `PortWrite`
  and nothing else asm-shaped, so each new family needs: a checked plan variant,
  a Terminal operation variant, a wire tag plus its
  `wiki/spec/terminal-psi/encoding.md` table row (machine-checked by
  `encoding_contract.rs`), verifier and interpreter arms, and the lowering and
  native-emission legs. That is the same shape as the ElementView descriptor
  sweep, which ran to 40 consumer legs across 6 crates.

  Not design-blocked: `wiki/spec/language/assembly.md:3-7` settles the rule —
  "every accepted instruction has a compiler-owned contract", and "assembly
  remains valid source surface". The one open asm owner question is
  embedded-interpretation asm, which these five legs do not exercise (they
  cross-compile to `linux_x86_64`). It is owned engineering of real size.
- **ASM-MEMORY-AND-TRANSFER-CONTRACTS.** Mined candidate — landed the first
  contracted memory-transfer family: the operand-provenance model is the typed
  Omega place itself. The catalog gains `AsmInstructionShape::MemoryTransfer`
  (`AsmMemoryTransferKind::{Load, Store}`) and contracts the canonical
  unordered AArch64 pair `ldr`/`str`: the memory operand is spelled as an
  ordinary place expression (`ldr <dest>, <place>` lowers to `<dest> = <place>`,
  `str <value>, <place>` to `<place> = <value>`), so provenance, permission and
  exact-type checking are the place's own, exactly as the catalog doc models
  "authorized memory data moves through a typed view index". Bracketed
  `[address]` operands still refuse before the shape applies; width-suffixed
  (`ldrb`/`strh`/...), offset/unscaled, ordered (acquire/release) and
  multi-register spellings stay refused — each is a different contract
  (element-width access, address arithmetic, ordering, or a place pair).
  Witness: new `asm_memory_transfer_compile` pass canary compiles for
  linux_arm64; the `asm_structured_ldr_str` fail canary keeps pinning the
  bracketed refusal; `memory_transfer_contracts_pin_place_operands_and_
  operand_order` + `parses_memory_transfers_as_place_assignments` cover both
  layers. Remaining families: ordered AArch64 acquire/release (`ldar`/`stlr`,
  LSE `swp*`/`cas*`/`ld*`) need ordering contracts in the shape, x86
  memory-destination stores and the exclusive/string/descriptor-table families
  need their own operand rosters, and the cache-with-memory-operand members
  (`invlpg`, `clflush`) admit through this shape once a place is spelled.
- **ASM-HIDDEN-EXIT-AND-MEMORY-CONTRACTS** — mined candidate; verify scope then implement.
- **ASM-PRIVILEGED-SERVICE-ADMISSION.** Mined candidate; scope verified at
  `17fec446ef2` — the admission leg is landed, and what remains is gated
  elsewhere. Re-mines the ASM-CATALOG-FAMILY-EXPANSION residual
  ("memory/authority-bearing families blocked on UnmodeledMemoryAccess and
  service admission"). Verified the admission route end-to-end at this
  revision: `Build.privileged_services.{port_io,interrupt_table}` parses into
  `PrivilegedServicesGrants` (build-evaluation `admission/configuration.rs`),
  flows through `phase_transitions.rs:192` into
  `AsmAuthorityAdmission::from_freestanding(...).with_grants(...)`, and
  `validate_asm_discharge` admits per instruction-authority class — the
  catalog's current three classes are all routable (7 MachineOwner
  instructions, freestanding-only by contract; 1 PortIo + 1 IdtControl under
  the mediated grants). No Mmio-authority or memory-authority instruction
  exists to admit a fourth class for — such a grant would be dead code.
  The blocked families themselves are gated on memory-access modeling:
  `inline_assembly/mod.rs:808` refuses ldr/str/ldp/stp/push/pop as
  `UnmodeledMemoryAccess`, a spec/model item rather than an admission slice.
  Fence note: the admission config surface `build-evaluation/src/admission`
  is path-claimed under TARGET-INFERENCE-AND-PLATFORM-CERTIFICATION
  (Zergling-112) this wave. No independent implementable slice exists.
- **ASM-MEMORY-AND-TRANSFER-CONTRACTS.** Mined candidate; scope
  verified at `a51cb805cc`, resolved — no unfenced slice this wave.
  Re-mines the memory/authority-bearing family clause of the asm
  catalog frontier: the row above (ASM family Residual) records
  "memory/authority-bearing families blocked on UnmodeledMemoryAccess
  and service admission", and resolved sibling
  INLINE-ASSEMBLY-CATALOG-EXPANSION attributes the same clause —
  those families stay blocked on UnmodeledMemoryAccess plus service
  admission per ASM-CATALOG-FAMILY-EXPANSION's Remaining, while the
  byte-level emission assertions wait on native-artifact production
  accepting asm-only entries (currently unreachable at entry
  selection). A memory-and-transfer contract family is exactly that
  blocked leg: load/store/transfer instructions carry memory effects
  `AsmInstructionContract` cannot model until UnmodeledMemoryAccess
  is resolved, and privileged service admission is
  ASM-PRIVILEGED-SERVICE-ADMISSION's separate stub. The implementing
  surfaces are live-fenced: `language-core/src/inline_assembly` +
  `inline_assembly.md` + the builtin intrinsic table + the parser arm
  by ASM-INSTRUCTION-CATALOG-EXPANSION (Zergling-160, 06:00Z), and
  `inline_assembly/mod.rs` + `inline_assembly.md` by
  ASM-CATALOG-MEMORY-AND-CONTROL (z139, 05:55Z). No independent slice
  exists; sibling stubs ASM-CATALOG-MEMORY-AND-CONTROL and
  ASM-HIDDEN-EXIT-AND-MEMORY-CONTRACTS mine the same clause.
- **ASM-INSTRUCTION-CATALOG-EXPANSION** — mined candidate; resolved upstream.
  Re-witnessed at `4252f00ab216`: the bounded catalog slice landed on main at
  `b8b858b14472` ("asm catalog: contract invd, wbnoinvd and nop") plus
  policy-inventory repins `6aca16747fe5`/`0f9a23e3d0d3` — `AsmCacheOperationKind`
  += `invd`/`wbnoinvd` beside `wbinvd` and `AsmSchedulingHintKind` += `nop`,
  all contracted members of existing shapes with the full per-member chain
  (kind enum + helpers, `BuiltinFunction` ordinals 77–79, statement gate,
  MachineOwner discharge map, terminal-authority inventory). The upstream
  board annotation (origin/main ~line 6943) already records the verified
  shape and the wave's unwitnessed-canary caveat (`selected-instructions-to-
  selected-instructions` `crossed_window`/`CrossingDirection` build break,
  fenced to the spill/sequencing workers). Residual legs: operand-bearing
  cache/TLB ops (`invlpg`, `clflush`) stay refused until a modeled
  memory-operand contract exists — sibling row
  ASM-MEMORY-AND-TRANSFER-CONTRACTS; atomics, mode transitions and AArch64
  system ops stay unrecognized per the catalog axis paragraph. No
  independent slice remains under this name.
- **BACKEND-RUNTIME-STARTUP-MECHANICS** — mined candidate; verify scope then implement.
  surfaces are under live claims (UEFI-OS-HANDOFF until 20:00Z,
  UEFI-PHYSICAL-SEMANTIC-ENTRY 22:59Z, OPAQUE-BY-VALUE-BOUNDARY-ABI and
  TV-BOUNDARY-SETTLEMENTS-REPLAY into next day). No independent slice is
  landable from this row. Re-verified at `7110606f46` — the same-item
  claim remains live and no new leg landed since `db3dfb4302`.
  Re-witnessed at `f501d377d8`: the recorded frontier moved upstream —
  `native_uefi_os_handoff_invocation_reports_missing_boundary_plan` no longer
  exists; bodied boundary machines now lower as ordinary Unit callees and
  `&mut` boundary requirements carry caller-side plans, so gap (a) landed.
  The pinned refusal is now
  `native_uefi_os_handoff_invocation_reports_cyclic_control_frontier`
  (cyclic-machine custody: ControlCycle rejection while `self.legs.*`,
  `self.terminal.*`, `self.cycle` receivers and `retain`'s granted extents
  stay outside the bare-`self` envelope). Both pins green here:
  `native_..._cyclic_control_frontier` + `checked_uefi_os_handoff_invocation
  _retains_edge_binding` 2/2 in 54s. UEFI-OS-HANDOFF claim still live
  (exp 10:19Z) covering the canary file and handoff.omg; the verifier's
  cyclic-custody surfaces are fenced under REGISTERED-CALLBACK-LIFETIME and
  CONSERVATION-CONTRACT. Still no landable slice from this row.
  Re-witnessed at `b53c7ea26032`: the recorded frontier moved upstream
  again — the cyclic-control-custody shape now lowers (record-local
  forwarders ride `Loader::run`'s plain block-parameter extents) and the
  pinned refusal is
  `native_uefi_os_handoff_invocation_reports_termination_catalog_frontier`
  ("no closed native catalog identity" for the compiler-owned
  `UefiOsHandoffTermination::transfer`/`firmware_return` edges — a UEFI
  physical-entry lane leg). Both pins green here:
  `native_..._termination_catalog_frontier` +
  `checked_uefi_os_handoff_invocation_retains_edge_binding` 2/2 in 48s.
  UEFI-OS-HANDOFF claim still live (exp 10:19Z) on the canary fixture and
  handoff.omg; REGISTERED-CALLBACK-LIFETIME's verifier fences live to
  ~14:37Z; CONSERVATION-CONTRACT has drained. Still no landable slice
  from this row.
  covered — alias of the settled STARTUP-ENTRY-MECHANICS-OWNERSHIP surface (ENTRY-MECHANICS-RUNTIME-CONSOLIDATION, `be03555d17`)
- **BACKEND-STARTUP-ENTRY-MECHANICS.** — mined candidate; scope verified,
  covered — sibling alias on the settled STARTUP-ENTRY-MECHANICS-OWNERSHIP
  surface adjudicated on the BACKEND-RUNTIME-STARTUP-MECHANICS row above
  (~:8466, audit `be03555d17`, re-verified `7d03d489e3`): entry/exit
  mechanics sit under one owner,
  `omega-rust/omega/backend/runtime/external-roots/src/root_entry/`
  (root_validation, root_admission, provider_execution,
  progress_profile_installation, opaque_callback_replacement — plus
  required_root_slots) and `platform_bringup`; free Unit entries emit
  process adapters and ELF `e_entry` round-trips through final-image
  validation. Re-verified at `ebd58a0544` on linux x86-64: the
  `root_entry/` module layout is intact, `cargo check -p external-roots`
  is clean, and the hosted_unit_entry suite is 7/7 green — note the crate
  now lives at `omega-rust/omega/backend/images/image-emission/src/
  hosted_unit_entry.rs` (moved under `images/` since the prior witness).
  No independent slice exists here. Board hygiene: this item has three
  same-name rows — this one, a bare stub at ~:8503, and a sibling
  adjudication at ~:8580 (verified `138ed79a677`, same verdict from the
  program-entry-lane angle); all reach "no independent slice".
- **BASELINE-CHECKED-LOWERED-PSI-CLUSTERS.** — mined candidate; resolved as drained: the same-name row below carries the triage (57-failure census at bd6cddcb59, closed by attribution into `wiki/drafts/known_baseline_failures.md`); residual ledger refreshed at `e7c0099cb2b7` (2206 run / 2183 pass / 23 fail, member→family mapping current). Repairs stay with the owning lanes named there; no slice under this stub.
  covered — drained; census closed into known_baseline_failures.md, families owned by their named lanes
- **BACKEND-RUNTIME-STARTUP-MECHANICS** — mined candidate; scope verified,
  covered — sibling alias on the settled STARTUP-ENTRY-MECHANICS-OWNERSHIP
  surface recorded on the resolved ENTRY-MECHANICS-RUNTIME-CONSOLIDATION
  row (~TASKS.md:7467, audit at `be03555d17`): entry/exit mechanics sit
  under one owner,
  `omega-rust/omega/backend/runtime/external-roots/src/root_entry/`
  (root_validation, root_admission, provider_execution,
  progress_profile_installation, opaque_callback_replacement) plus
  `platform_bringup` for UEFI bootstrap; the runtime leg was settled by
  BACKEND-RUNTIME-STARTUP-ENTRY-MECHANICS — free Unit entries emit process
  adapters and ELF `e_entry` round-trips through final-image validation
  (`image-emission/src/hosted_unit_entry.rs` pins the exact Linux
  x86-64/ARM64 adapter selection). Re-verified at `83625209125b` on linux
  x86-64: `root_entry/` module layout intact, `cargo check -p
  external-roots` clean, `hosted_unit_entry` suite 7/7 green. No
  independent slice exists here. Sibling aliases: STARTUP-ENTRY-MECHANICS,
  STARTUP-ENTRY-PLACEHOLDER-SWEEP, STARTUP-ENTRY-RUNTIME-MECHANICS,
  BACKEND-STARTUP-ENTRY-MECHANICS. Re-verified at `7d03d489e3` on linux
  x86-64: `root_entry/` still carries the five named modules
  (root_validation, root_admission, provider_execution,
  progress_profile_installation, opaque_callback_replacement) and
  `image-emission/src/hosted_unit_entry.rs` is intact; the settled
  verdict stands (dispatcher re-dispatched the resolved alias).
  The concrete entry-acquisition leg the README named — era-entry-gated sealing of `InstalledEntryReference` on the retained runnable's `InstalledCode` — landed at `be03555d1795` (`entry_acquisition.rs` joins the `ActiveComponentEraEntry` token to the era's retained runnable through `RunnableComponentEraLedger::acquire_installed_entry`, delegates the seal to the executable-installation control-flow-integrity gate, and returns the authority and receipt unchanged on refusal); re-witnessed `cargo nextest run -p component-publication -E 'test(~entry_acquisition)'` 3/3 PASS on linux x86-64 at `483dfea65fc3`. Additional sibling alias on this settled surface: BACKEND-RUNTIME-STARTUP-ENTRY-MECHANICS.
  BACKEND-STARTUP-ENTRY-MECHANICS.
  covered — alias of the settled STARTUP-ENTRY-MECHANICS-OWNERSHIP surface (ENTRY-MECHANICS-RUNTIME-CONSOLIDATION, `be03555d17`)
- **BACKEND-VOCABULARY-REJECTION-AUDIT.** Mined candidate; scope verified at
  cb01abfa42 — audit that every vocabulary operation reaching the backend is
  either legalized+selected or cleanly refused, never silently miscompiled or
  panicked on. The classification point is
  `target-operations-to-selected-instructions/src/legalization/scalar_graph_input/nodes.rs`:
  `admit()` covers 83 `AbstractOperation` variants, falling through to
  `Err(NodeRejection::UnsupportedFamily)` → `LegalizationError::UnsupportedScalarOperation`
  (model.rs:120); `control::validate` classifies terminators with the same
  `_ => SourceCustodyMismatch` refusal. Ordering is the audit's core fact:
  `nodes::validate` runs per-block inside `legalize_target_operations` BEFORE
  `validate_target` and before `source/scalar_graph`'s `instruction()` calls —
  so the `admit(..).ok()`/`filter_map` sites downstream can only ever see
  admitted nodes, never a suppressed UnsupportedFamily. Open audit questions
  for the implementing leg: (a) whether every *admitted* family has selection
  coverage on every ISA (admitted-but-unencodable is the remaining hole class
  — e.g. `NearestIeeeFloatFusedMultiplyAdd` is ingest-refused today, tracked
  by X86-FMA-PROVIDER-TRANSPORT); (b) whether `UnsupportedScalarOperation`
  surfaces as a compile diagnostic end-to-end rather than aborting; (c)
  whether any `match` on `node.operation` outside nodes.rs/control.rs is
  reachable before `nodes::validate` (none found at verify time — all are
  provenance replays under validate_target or per-node dispatch under
  validate). Territory: `target-operations-to-selected-instructions/src/{legalization,selection}`
  + `representations/abstract-operations` (read-only enumeration).
- **BASELINE-CHECKED-LOWERED-PSI-CLUSTERS.** Mined candidate — scope verified, covered — re-mines the checked-trees-to-lowered-psi failure-cluster surface of `wiki/drafts/known_baseline_failures.md` §checked-trees-to-lowered-psi. The cluster ledger is maintained by CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION (fresh member-by-member reading recorded at `6ef64f6dd6`: 2152 run, 2093 passed, 59 failed, 1 SIGTERM blowup), and every cluster family is already owned by a named row — bare `Service<R>` fixture spellings → ENTRY-CONTENT-ROOTS + BASELINE-SERVICE-CARRIER-FAILURES, missing transitive machine plans → GENERAL-CYCLIC-EXECUTION + UEFI-OS-HANDOFF, site_guard crash-namespace + scalar-return custody → WRITE-ONLY-BORROW + C2L-BASELINE/RESIDUAL-FAILURE-ATTRIBUTION, the proof-search blowup → C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT, crash-member byte entries → resolved under LOWERED-CRASH-MEMBER-BYTE-ENTRIES (48/48 green). No independent slice exists here. Re-verified at `7241e022270d`: live fences on the surface include STRUCTURAL-UNIT-LOWERING (`src/unit`, 09:16Z), C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES (10:35Z), PROOF-CERTIFICATION-BRIDGE (`src/tests`, 10:52Z), RC-REPOSITORY-CLOSURE (tests/nominal_affine_source, 12:15Z). Sibling stubs on the same surface: BASELINE-SERVICE-CARRIER-FAILURES, STATE-LOCAL-VALUE-FRONTIER, BASELINE-T2C-PROVIDER-ATTACHMENT-AND-RESULTS, LOWERED-PSI-BASELINE-TAIL, LOWERED-CRASH-MEMBER-BYTE-ENTRIES, RC-REPOSITORY-CLOSURE.
- **BASELINE-SERVICE-CARRIER-FAILURES.** Partially advanced at
  `62c502f9f6` — the bare `Service<R>`-carrier family of
  `known_baseline_failures.md`'s c2l attribution: 33 tests spelled
  `console: Console` / `runtime: TaskRuntime` / `output: Output` in value
  position and rejected under `validate_no_bare_boundary_trait_values`
  (32f5182254). Done (this slice): `tests/unit_plan_omissions.rs`'s 4 bare
  `runtime: TaskRuntime` spellings migrated to `&'s mut TaskRuntime`
  receivers on `Main<'s>`/`Carrier<'s>` per the 0e1977994b raw-pipeline
  recipe. The 3 carrier-semantic members now pass source checking and stop
  at `signature`-phase local construction, joining the missing-transitive-
  machine-plan family (GENERAL-CYCLIC-EXECUTION / UEFI-OS-HANDOFF fences)
  until ENTRY-CONTENT-ROOTS' receiver-lifecycle leg lands; the `&TaskRuntime`
  shared-borrow negative control still pins the same stop. Remaining
  (fenced): `checked-trees-to-lowered-psi/src/tests/{attached_unit_cases,
  composed_operand_catalogs{,/dynamic_unit}, composed_unit_nested_control,
  dynamic_composed_unit, indexed_primitive_storage,
  structural_control_cases}.rs` (21 bare spellings) sit inside
  PROOF-CERTIFICATION-BRIDGE's `src/tests` claim (expires ~2026-09-21T00:51Z)
  — same migration applies there when the fence settles.
- **BASELINE-T2C-PROVIDER-ATTACHMENT-AND-RESULTS** — mined candidate; verify scope then implement.
- **BASELINE-CHECKED-LOWERED-PSI-CLUSTERS** — triaged at bd6cddcb59 (Linux x86-64): `cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast` reports 2146 run, 2088 pass, 57 named failures, 1 non-terminating (`nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`, >1020 s, no nextest timeout). Clusters recorded in `wiki/drafts/known_baseline_failures.md`: bare boundary-trait value fields vs the `Service<R>` gate (33, 32f5182254 — fixture migration under SERVICE-CARRIER-FIXTURE-MIGRATION; the inline `resolve` harness loads no core library, so respelling needs a core-aware resolution path), attached-Unit transitive machine plan on `&mut`-trait provider fields (16 — a distinct Unit-plan admission gate, not the check diagnostic), crash predicate scalar namespace (3 — LOWERED-PSI-BASELINE-TAIL owns `proofs/crash_routes*`), owned-record-return custody (4, unbisected), Registration::Live qualification (1, unbisected). Repairs belong to those owning lanes; this row is closed by the attribution, not by repair. (Draft reading superseded by main's 6ef64f6dd6 census in `known_baseline_failures.md`.)
  covered — drained; census closed into known_baseline_failures.md, families owned by their named lanes
- **BASELINE-SERVICE-CARRIER-FAILURES.** Resolved. The bare
  boundary-trait carrier family is fully migrated: the 21
  `console: Console` and `output: Output` spellings across the lowering
  crate's `src/tests` sources became `Service<R>` carrier fields at
  00a69f066b0, with `checked_source_with_core_service` installing the core
  service source, and `tests/unit_plan_omissions.rs`'s 4
  `runtime: TaskRuntime` spellings became `&'s mut TaskRuntime` receivers at
  37e309e6060. Verified at 00e1da7ae2a on macOS arm64, 2199 run and 2174
  passed with no `validate_no_bare_boundary_trait_values` rejection left in
  the log, and every declared boundary trait in those trees scanned for a
  value-position field. The library members pass, not vacuously: their
  harness ends in a check the fixture must survive before the tests assert
  on the lowered result. The three carrier-semantic `unit_plan_omissions`
  members reach the further stop the item predicted, `signature`-phase local
  construction, joining the missing-transitive-machine-plan family behind
  the **GENERAL-CYCLIC-EXECUTION** and **UEFI-OS-HANDOFF** fences until
  **ENTRY-CONTENT-ROOTS**' receiver-lifecycle leg lands; the shared-borrow
  negative control still pins that stop.

- **BENCHMARK-COMPARISON-OCCURRENCE-GATE** — Resolved at `749794ddeb`.
  The recorded rejection (`Terminal proposal must retain every integer
  comparison occurrence exactly once`, witnessed at `e48558bd41`) came from a
  module-wide census counting builtin/generated `std` comparisons against
  the selected-occurrence roster; `76dc49a99e` rescoped it to the
  checked-boundary scope in `terminal_product/integer_comparisons.rs`
  (also witnessed at `f2f39039da` per BENCHMARK-COMPILE-ONLY-ROWS).
  This leg verified end-to-end on linux_x86_64: `cli_mvp` compiles past
  the gate and `benchmark.py measure` produced a valid measured row
  (`tools/benchmark/records/cli_mvp__linux_x86_64__default.json`:
  compile median 1,681,237 ms, run median 1.94 ms, exit 0 x5,
  8,192-byte image, `validate` clean).
  covered — resolved at `749794ddeb`
- **BACKEND-STARTUP-ENTRY-MECHANICS.** — mined candidate; scope verified at
  `138ed79a677` (linux x86-64) for sibling stub
  **BACKEND-RUNTIME-STARTUP-MECHANICS** (same surface — the runtime startup /
  program-entry lane: `backend/runtime/external-roots` platform_bringup +
  program-entry plan + `component-publication`, the directories earlier waves
  claimed under the two startup-mechanics names). Frontier: those dedicated
  dir-claims have drained, but the entry-plan implementing surface is still
  fenced — `backend/runtime/external-roots/` sits under
  EPOCH-RESOURCE-SNAPSHOTS (11:32Z), and the frontier note at ~1326 still
  stands (UEFI-OS-HANDOFF and the UEFI-program-entry lanes remain live).
  `component-publication/` itself is unfenced tonight, but its tests.rs
  custody-matrix migration belongs to CUSTODY-MUTATION-COVERAGE in
  TASKS_OPTIMIZER.md, not a
  startup-mechanics leg. No independent slice landable from this stub.
- **BASELINE-CHECKED-LOWERED-PSI-CLUSTERS.** — mined candidate; resolved
  as drained (re-verified `7d03d489e3d`): the canonical same-name row below
  carries the triage (57-failure census at `bd6cddcb59`, closed by
  attribution into `wiki/drafts/known_baseline_failures.md`; residual
  ledger refreshed at `e7c0099cb2b7` — 2206 run / 2183 pass / 23 fail).
  Repairs stay with the owning lanes named there; the ledger itself is
  claim-fenced this wave (NEW-KBF-LIB-CLUSTER-AND-STALE-ROWS-REFRESH holds
  the draft path; RC-REPOSITORY-CLOSURE +
  CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION live pathless). No slice under
  this stub.
  covered — drained; census closed into known_baseline_failures.md, families owned by their named lanes
- **BENCHMARK-COMPILE-ONLY-ROWS.** Mined candidate. Upstream:
  [wiki/drafts/benchmarks.md](wiki/drafts/benchmarks.md) — produce committed
  `tools/benchmark/records/` rows for cross-target compile legs
  (`benchmark.py measure --no-run` marks `runtime_ms` skipped): every
  catalogued deployment profile beyond `linux_x86_64` —
  `windows_x86_64`, `macos_arm64`, `macos_x86_64`, `linux_arm64`,
  `uefi_x86_64` — is a compile-only row measurable on any build host whose
  compiler publishes that target's artifact.
  `samples/cli/arithmetic/wrapping_square_sum` (added 3dd805679c) is the
  dependency-free subject — no `depend()` edges; its own project still
  needs one-time package-review settlement per target.
  The e48558bd41 integer-comparison-occurrence rejection
  (BENCHMARK-COMPARISON-OCCURRENCE-GATE /
  BENCHMARK-COMPILE-UNBLOCK-COMPARISON-OCCURRENCES own the producer fix)
  is RESOLVED at f2f39039da — `76dc49a99e` ("distinguish selected
  comparison custody from builtin operations") unblocked it: witnessed
  `wrapping_square_sum` compiling and publishing on windows_x86_64,
  macos_arm64, linux_arm64 (~24-28s each on a linux x86-64 host; the
  dependency-free subject does not reach the deep-pipeline stage where
  the rejection fired).
  Settlement is per-target (`benchmark.py prepare --target <t>` per
  profile). Two catalogued profiles are NOT valid CLI-subject targets:
  `macos_x86_64` and `uefi_x86_64` fail review settlement with "no bound
  required root slot `<target>::ProgramEntry`" — record them as
  non-applicable, not as failed compiles (MACOS-X64-HOST-PROFILE owns
  the x86_64 macOS host-profile gap).

  The three compile-only records are now committed at `52ceeeabb7`
  (BENCHMARK-CROSS-TARGET-COMPILE-ROWS's lane): the
  `wrapping_square_sum__<windows_x86_64|macos_arm64|linux_arm64>__default.json`
  records carry the measured legs
  (runtime_ms skipped; code_size_bytes 1024/16640/8192
  match the w9 measurements), with `d8825aef56` recording the verified
  legs in benchmarks.md and `7e38fc2763` repairing the host-row matrix
  drift. Remaining: none under this name — any further matrix refresh
  stays with BENCHMARK-CROSS-HOST-ROWS, and the fenced prose staleness
  in benchmarks.md's w9/z113 update paragraphs ("await commit once the
  claim frees") belongs to whichever lane next writes that doc.
  covered — compile-only records landed at `52ceeeabb7`; residue is host-gated rows on BENCHMARK-CROSS-HOST-ROWS
- **BENCHMARK-CROSS-HOST-ROWS.** Mined candidate; verify scope then implement.
  Verified scope: re-mines the host-row matrix's runtime legs in
  [wiki/drafts/benchmarks.md](wiki/drafts/benchmarks.md#host-row-matrix) —
  one committed `tools/benchmark/records/` row per catalogued deployment
  profile measured on its own host (linux_arm64 host, macos_arm64,
  windows_x86_64 [peak-RSS leg stays `unavailable` — no `os.wait4`],
  uefi_x86_64 under QEMU/hardware). None is producible on a linux_x86_64
  build host; the local legs are compile-only rows owned by
  **BENCHMARK-COMPILE-ONLY-ROWS** — committed at `52ceeeabb7` (the
  three measured cross-target records are in
  `tools/benchmark/records/`). The producing
  surfaces are under live claims: `tools/benchmark` is held by
  BENCHMARK-COMPARISON-OCCURRENCE-GATE (expires 22:09Z) and
  `benchmarks.md` + `tools/tests/test_benchmark.py` by
  BENCHMARK-HOST-ROW-MATRIX (22:11Z). A new `linux_x86_64` measured row
  additionally needs the post-f2f39039da compile frontier re-verified on a
  runnable subject. Sibling re-mine names: BENCHMARK-CROSS-TARGET-COMPILE-LEGS,
  BENCHMARK-CROSS-TARGET-COMPILE-ROWS, BENCHMARK-LINUX-ARM64-ROW,
  BENCHMARK-LINUX-X64-ROW-REFRESH, BENCHMARK-MACOS-ARM64-ROW,
  BENCHMARK-PRIME-COUNTER-ROW, BENCHMARK-HOST-ROW-MATRIX.
  Re-verified at `a51cb805cc` on linux x86-64: the staged w9 records
  have since landed — `tools/benchmark/records/` now carries
  `wrapping_square_sum` rows for `linux_arm64`, `macos_arm64`, and
  `windows_x86_64`, but all three are `--no-run` compile legs measured
  on a linux x86_64 host (`host.os=linux`, `host.machine=x86_64`,
  `runtime_ms` unmeasured), so the host-native runtime legs this row
  names remain unconsumed: `macos_arm64` and `windows_x86_64` (peak-RSS
  leg stays `unavailable` — no `os.wait4`) still need their own hosts,
  `linux_arm64` needs an arm64 runner, `uefi_x86_64` needs
  QEMU/hardware. Producing surfaces now fenced by
  BENCHMARK-ROW-RESUMPTION (03:42Z) and BENCHMARK-PRIME-COUNTER-ROW
  (05:19Z); the named sibling fences have expired.
  **BENCHMARK-LINUX-X64-ROW-REFRESH landed 2026-09-21** (`z177` lane):
  the missing `wrapping_square_sum × linux_x86_64 × default` record is
  committed as
  `tools/benchmark/records/wrapping_square_sum__linux_x86_64__default.json`
  — measured natively on this host at `e7c0099cb2` (compile median
  33396.8 ms across 3 samples, runtime median 4.37554 ms across 5
  exit-0 samples, 8192 B artifact, peak RSS measured via os.wait4);
  `benchmark.py validate` passes and the host-row matrix is
  regenerated. Remaining legs are the genuinely host-gated ones:
  `linux_arm64` (needs an arm64 runner), `macos_arm64`/`windows_x86_64`
  (need their own hosts; windows peak-RSS stays `unavailable`), and
  `uefi_x86_64` (needs QEMU/hardware).
- **BENCHMARK-CROSS-TARGET-COMPILE-ROWS.** Resolved — re-mine of
  BENCHMARK-COMPILE-ONLY-ROWS (~line 6395), identical deliverable to sibling
  BENCHMARK-CROSS-TARGET-COMPILE-LEGS: committed `tools/benchmark/records/`
  rows for the cross-target compile legs (`benchmark.py measure --no-run`,
  `runtime_ms` skipped). Verified at `97be15c1b59`: the measured
  `wrapping_square_sum__{windows_x86_64,macos_arm64,linux_arm64}__default.json`
  records are landed (`52ceeeabb7`, ancestor of base) plus a
  `macos_x86_64` row; `52ceeeabb7` is an ancestor of base. Any further
  matrix refresh belongs to BENCHMARK-CROSS-HOST-ROWS / BENCHMARK-ROW-RESUMPTION,
  not this name.

  covered — resolved re-mine of BENCHMARK-COMPILE-ONLY-ROWS; records landed
- **BENCHMARK-CROSS-TARGET-COMPILE-LEGS.** Scope verified — re-mine of
  BENCHMARK-COMPILE-ONLY-ROWS (~line 6395), which owns this exact deliverable:
  committed `tools/benchmark/records/` rows for the cross-target compile legs
  (`benchmark.py measure --no-run` on `windows_x86_64`, `macos_arm64`,
  `linux_arm64`; `macos_x86_64`/`uefi_x86_64` are not valid CLI-subject
  targets — they fail review settlement on the unbound `ProgramEntry` slot,
  not the compile). The producer-side blocker it names (integer
  comparison-occurrence rejection) is already resolved at `f2f39039da`;
  the three measured records are committed at `52ceeeabb7` (the
  `wrapping_square_sum__<windows_x86_64|macos_arm64|linux_arm64>__default.json`
  records in `tools/benchmark/records/`), so no independent slice
  exists here. Re-verified at `62c502f9f6` on linux x86-64;
  records confirmed present on the tip `a51cb805cc`.
- **BENCHMARK-LINUX-ARM64-ROW.** Resolved 2026-09-21 — scope verified at
  `7241e02227`; re-mines the linux_arm64 leg of the host-row matrix (see
  BENCHMARK-HOST-ROW-MATRIX / BENCHMARK-CROSS-HOST-ROWS): one committed
  `tools/benchmark/records/` row measured on a linux arm64 host via
  `benchmark.py measure --target linux_arm64`. Verified at this commit:
  `tools/benchmark/records/wrapping_square_sum__linux_arm64__default.json`
  exists but is a cross-compile leg measured on a linux x86_64 host
  (`host.machine=x86_64`; `runtime_ms.status=skipped`, reason `--no-run`;
  compile median 28129.6 ms) — the host-native runtime row this item
  names is not producible on this linux x86-64 box (no arm64 runner).
  The producing surfaces are also fenced this wave:
  `tools/benchmark/records` + `wiki/drafts/benchmarks.md` under
  BENCHMARK-PROOF-SUBJECT-SELECTION (exp ~14:19Z). No independent slice
  exists under this name — the start condition is a seeded linux arm64
  runner plus the records-dir fence settling.
  covered — resolved 2026-09-21; native arm64 measurement is a host-gated leg of BENCHMARK-HOST-ROW-MATRIX
- **BENCHMARK-MACOS-ARM64-ROW.** Mined candidate; scope verified at
  50425f1c70 — re-mines the macos_arm64 leg of the host-row matrix (see
  BENCHMARK-HOST-ROW-MATRIX and BENCHMARK-CROSS-HOST-ROWS' verified
  scope): one committed `tools/benchmark/records/` row measured on a
  macos_arm64 host via `benchmark.py measure --target macos_arm64`. Not
  producible on a linux_x86_64 build host — the row is a per-host runtime
  measurement and this machine has no macOS arm64 route (compile-only
  `--no-run` records belong to BENCHMARK-COMPILE-ONLY-ROWS / cross-target
  legs, not this row). Producing surfaces fenced this wave:
  `tools/benchmark` under BENCHMARK-COMPARISON-OCCURRENCE-GATE (22:09Z),
  `wiki/drafts/benchmarks.md` + `tools/tests/test_benchmark.py` under
  BENCHMARK-HOST-ROW-MATRIX (22:11Z). Prerequisite: a seeded macOS arm64
  host (per SEED-HOST-CHAIN-LEGS' audited host list).
  **The prerequisite is satisfied and the row is measured — 2026-09-21T00:34Z
  on an Apple M4 (darwin/arm64, 10 logical CPUs, rustc 1.100.0-nightly, dev
  profile).** Only the committed record file is outstanding, and solely
  because `tools/benchmark/**` + `wiki/drafts/benchmarks.md` are fenced to
  PRIME-COUNTER-BENCHMARK-ROW (Jarod, exp ~07:21Z); the measurement was taken
  with `--print --records-dir <scratch>` so nothing was written under the
  fence. `benchmark.py measure --root
  samples/cli/arithmetic/wrapping_square_sum/main.omg --target macos_arm64`
  on this host: compile median **17,177 ms** (min 17,088; stages prepare 14.7
  / compile 17,112.9 / publish 45.0), compile peak RSS **117,063,680 B**,
  code size **16,640 B** (`stable: true`), and — the leg no linux host can
  produce — `runtime_ms.status` **measured**, median **2.83 ms** (min 2.65)
  over 5 samples, `run_max_rss` 1,376,256 B, every `exit_code` 0. Cross-check
  against the existing cross-target compile-only record
  (`wrapping_square_sum__macos_arm64__default.json`, `host.os = linux`,
  `runtime_ms.status = skipped`): identical 16,640-byte code size, and its
  24,454 ms median is the Xeon 8559C cross-compile, not this host. The run was
  only possible after `b1dc444d9dc9`/`6aca16747fe5` repaired the closed-policy
  inventory count — before those, every native compile panicked (see
  ASM-CATALOG-FAMILY-EXPANSION). Next action for the fence-holder: commit this
  record under `tools/benchmark/records/` and add the `benchmarks.md`
  coverage entry.
  **macOS arm64 host row recorded 2026-09-21.** The committed
  `wrapping_square_sum__macos_arm64__default.json` was keyed `macos_arm64` but
  MEASURED on a linux x86_64 Xeon with `--no-run`, so its runtime leg was
  skipped and its numbers were a cross-compile. This host is macOS arm64, so the
  real row now exists:
  `wrapping_square_sum__macos_arm64__sel-44c60ac57c66.json` --
  host `darwin` / `arm64` / Apple M4, compile median **19725.2 ms**, compile peak
  RSS 123535360 B, code size 16640 B, and the runtime leg **measured** at
  2.8475 ms over five samples, all exiting 0. `benchmark.py validate` passes and
  the coverage table in `wiki/drafts/benchmarks.md` is regenerated from the
  records (25 -> 26 rows).

  It is a NEW cell, not a refresh of the old one, and that is correct rather than
  a miss: the subject's `build.omg` enables six Psi-phase rules
  (`aeb4d7ee21`), so a measurement of this subject as authored lands in
  `sel-44c60ac57c66`, never in `default`. The new row is the direct cross-host
  counterpart of the existing `wrapping_square_sum__linux_x86_64__sel-44c60ac57c66`
  row, which is the comparison this matrix wants. The stale `__default` row is
  left in place rather than deleted -- removing another host's record is not this
  row's business -- but it should be read as a linux compile-only cell, not a
  macOS one.

- **BENCHMARK-MEASURABLE-SUBJECT-CORPUS.** — mined candidate; resolved —
  covered alias. The name re-mines the benchmarks.md "no measurable
  subject" frontier already owned by resolved sibling
  BENCHMARK-SUBJECT-CORPUS-EXPANSION (~:8855): measurable subjects are
  `wrapping_square_sum` (committed record on six targets +
  linux_x86_64 runtime row), `standalone`, `cli_mvp`, and
  `structural_proofs`; the corpus-expansion residual legs (std-depend
  subjects gated on the selected-provider-plan join, depend-free runtime
  legs on the Process-exit contract, `math_proofs` on checked-call
  selection) stay with that row and the owning items. Sibling stubs on the
  same sentence: BENCHMARK-DEPEND-FREE-RUNNABLE-SUBJECT,
  BENCHMARK-PROOF-SUBJECT-SELECTION. No independent slice.
  Re-verified at `7d03d489e3` on linux x86-64: 10 committed records under
  `tools/benchmark/records/`, corpus roster unchanged.
  covered — alias of resolved BENCHMARK-SUBJECT-CORPUS-EXPANSION
- **BENCHMARK-PRIME-COUNTER-ROW.** Scope verified at `1a772e4ae1`: the
  mined stub names a measured `benchmark.py` row for
  `samples/cli/arithmetic/prime_counter` (README's example subject,
  `--expected-exit 8`). Its recorded blocker is repaired — the `i32`
  remainder now legalizes via `ExactRemainderI64` (landed `3c1ead6df4`,
  re-verified `6d00135b89`: `samples_with_documented_exit_run_correctly`
  compiles and runs prime_counter to exit 8 on linux x86-64). Producing
  the row itself remains fenced: `tools/benchmark` (the `records/` sink)
  is claimed by BENCHMARK-COMPARISON-OCCURRENCE-GATE until 22:09Z,
  `wiki/drafts/benchmarks.md` + `tools/tests/test_benchmark.py` by
  BENCHMARK-HOST-ROW-MATRIX until 22:11Z, and the subject path
  `samples/cli/arithmetic/prime_counter` by PRIME-COUNTER-REMAINDER-LEGALIZATION
  until 2026-09-21T01:14Z. Next action after those leases: run
  `benchmark.py measure --root samples/cli/arithmetic/prime_counter/main.omg
  --target linux_x86_64 --expected-exit 8` and commit the record plus the
  coverage row. Sibling re-mine name: PRIME-COUNTER-BENCHMARK-ROW.
- **BENCHMARK-PROOF-SUBJECT-CHECKED-CALL-SELECTION.** Resolved — the
  checked-call-selection blockage that failed `math_proofs` (named on the
  sibling BENCHMARK-PROOF-SUBJECT-SELECTION row) is fixed. Contract-position
  calls whose spelled callee names no declaration (`Bag(items)`/`Bag(before)`
  atoms) get no Call occurrence from the resolver — the
  `unbound_contract_call` gate deliberately skips them — so the checked
  collector had nothing to bind. `collect_checked_proof_view_call_selections`
  now walks each fact's root subtree as a group, inherits the clause's
  authored exposure from any sibling occurrence (fallback
  PrivateImplementation), and reports such unbound receiverless calls that
  carry no Call-kind occurrence; checked finalization mints one finalized
  ProofView ledger row per exact (span, exposure) call site and attaches it
  as the expression's occurrence — explicit compiler-owned custody instead
  of an unbound hole. Verified at `1f7301b710` + slice, linux x86-64:
  `undeclared_contract_view_calls_finalize_as_proof_view_intrinsics` green;
  resolver pin
  `contract_clause_calls_naming_no_declaration_skip_call_selection` green;
  `-p typed-trees-to-checked-trees --lib` 5038/5038; end-to-end
  `omega --check` on `samples/cli/proofs/{math_proofs,structural_proofs}`
  compiles. Residual: none for the ledger route — the remaining benchmark
  legs (build.omg + ProgramEntry, tools/benchmark record) stay on the
  owning sibling rows. Sibling re-mine names: MATH-PROOFS-CHECKED-CALL-
  SELECTION, PROOFS-SUBJECT-CHECKED-CALL-SELECTION.
  covered — resolved; checked-call-selection fix landed
- **PROOF-SAMPLES-CHECKED-CALL-SELECTION.** Resolved — the stub name the
  sibling rows above cite for `math_proofs`'s checked-call-selection
  blockage; that fix landed on BENCHMARK-PROOF-SUBJECT-CHECKED-CALL-
  SELECTION (verified `1f7301b710`): unbound receiverless
  contract-position calls mint a finalized ProofView ledger row per
  (span, exposure) site via `collect_checked_proof_view_call_selections`.
  Re-verified at `90df29812c0` (linux x86-64):
  `undeclared_contract_view_calls_finalize_as_proof_view_intrinsics`
  green; resolver pin `contract_clause_calls_naming_no_declaration_
  skip_call_selection` green; `omega --check` on
  `samples/cli/proofs/math_proofs/main.omg` compiles. No residual slice.
  covered — resolved; fix landed via BENCHMARK-PROOF-SUBJECT-CHECKED-CALL-SELECTION
- **BENCHMARK-PROOF-SUBJECT-SELECTION.** Mined candidate; scope verified at
  `1a772e4ae1`, owned — re-mines the proof-subject leg of the benchmarks
  frontier (wiki/drafts/benchmarks.md 'no measurable subject'): the only
  `depend()`-free subjects are `samples/cli/proofs/{math_proofs,
  structural_proofs}`, proof-only machines that emit no runtime code (no
  selected `ProgramEntry`), and `math_proofs` fails earlier at checked-call
  selection. Selecting a proof subject for a row therefore means either an
  authored `build.omg` + `ProgramEntry` on a proof sample (both subjects are
  already pinned in `tests/samples_compile.rs`'s roster at
  `cli__proofs__*`) or a landing of the checked-call-selection fix. Those
  claims have expired and the first route already landed:
  `structural_proofs` carries authored `build.omg` + `ProgramEntry`
  bindings (`EXPLICIT_ENTRY_PROOF_SAMPLES` pins it). Selection verified
  live at `3533f7d0e86` on linux x86-64: `python3
  tools/benchmark/benchmark.py measure --root
  samples/cli/proofs/structural_proofs/main.omg --target linux_x86_64
  --no-run --print` emits a conforming record — 3 compile samples, median
  31.7 s, published 8192-byte artifact, runtime `skipped`. The remaining
  leg commits that record under `tools/benchmark/records/` plus the
  `wiki/drafts/benchmarks.md` matrix entry — both fenced by
  BENCHMARK-PRIME-COUNTER-ROW (~05:19Z); `math_proofs`'s
  checked-call-selection fix stays with PROOF-SAMPLES-CHECKED-CALL-
  SELECTION. Sibling stubs on the same sentence: BENCHMARK-PROOF-SUBJECT-
  CALL-SELECTION, BENCHMARK-PROOF-SUBJECT-CHECKED-CALL-SELECTION,
  BENCHMARK-MEASURABLE-SUBJECT-CORPUS, BENCHMARK-DEPEND-FREE-RUNNABLE-
  SUBJECT.
  Re-verified at `e5bbe53956f` (Zergling-52): the authored-entry half is
  fully landed — `samples/cli/proofs/structural_proofs/build.omg` binds
  `ProgramEntry` for all four hosted targets to the inert `Main::main`,
  and its README documents that a target compile produces a native
  artifact (compile + code-size legs only; run behavior unspecified).
  Fence map now: the prior z148 claim expired; the remaining surfaces are
  held by BENCHMARK-PROOF-SUBJECT-CHECKED-CALL-SELECTION (z175, TASKS.md,
  05:33Z) and `tools/benchmark` under PRIME-COUNTER-BENCHMARK-ROW (Jarod,
  07:21Z). Note: a fresh worktree's target compile stops at the
  package-review gate — the recorded scratch-copy ceremony applies; that
  is checkout state, not subject state.
- **BENCHMARK-REJECTED-ROW-RECORDING** — mined candidate; drained — the
  same-name row below carries the landed slice (record-schema
  `applicability` block, `SubjectNotApplicable` settlement route, both
  committed non-applicable rows, extended validator/matrix coverage).
- **BENCHMARK-ROW-RESUMPTION.** Mined candidate; scope verified at
  `39317a770b1f` — re-mines the resumption surface inside benchmark row
  production, both halves of which are already landed contract: (a) the
  package-review settlement ceremony is restartable — `measure` (or the
  explicit `prepare` step) runs `omega update`, rewrites every `pending`
  decision token to `accept` via `accept_pending_decisions`
  (tools/benchmark/benchmark.py:353), then `omega update --resume`
  publishes `omega.lock`, which is deliberately left in place so a
  resumed row does not re-settle (README "Package-review preparation");
  (b) a row's record production is idempotent — rerunning a
  (subject, target, selection) row overwrites its own record rather than
  duplicating it (README "Record schema"). Pinned by
  `tools/tests/test_benchmark.py`'s `accept_pending_decisions` tests
  (pending decision rows rewritten, non-decision `pending` text
  untouched). On this worktree's stale base the suite reads 20/21 — the
  one failure is `test_doc_embeds_the_current_matrix`, the
  benchmarks.md matrix embed lagging newly committed records; that file
  plus `tools/benchmark/records` are live-fenced to
  BENCHMARK-PROOF-SUBJECT-SELECTION (exp 14:19Z), so the repair belongs
  to that lane. No independent slice exists under this name.
  covered — both resumption halves are landed contract; remaining rows host-gated, record surfaces owner-laned
- **BENCHMARK-SELECTION-CONTRAST-ROWS** — mined candidate; verify scope then implement.
  covered — sibling of BENCHMARK-SELECTION-ROW-COVERAGE; record production owner-laned
- **BENCHMARK-REJECTED-ROW-RECORDING.** (new-scope) Give the benchmark record
  schema a row-level non-applicable status. A `(subject, target)` whose
  package-review settlement rejects the subject cannot be committed at all
  today: `settle_package_review` in `tools/benchmark/benchmark.py` raises
  `SystemExit` on failure, and the validator at
  `tools/benchmark/benchmark.py:669-670` admits only per-metric `measured`,
  `unavailable` or `skipped`. So `macos_x86_64` and `uefi_x86_64` CLI
  subjects exist only as HOST_LEGS-declared matrix cells and never as
  records. [benchmarks](wiki/drafts/benchmarks.md) asks for the opposite
  twice, at `:127` and `:136` — record them as non-applicable, not as failed
  compiles.

  Owning files, each confirmed present: `tools/benchmark/benchmark.py` (both
  the record writer and the `matrix` renderer),
  `tools/tests/test_benchmark.py`, `wiki/drafts/benchmarks.md`, and
  `tools/benchmark/records/`.

  Acceptance: a committed record for `wrapping_square_sum` x `uefi_x86_64`
  carrying an explicit non-applicable status with its settlement reason,
  rendered as its own matrix row, with `python3 tools/tests/test_benchmark.py`
  green on the extended schema.

  **Landed 2026-09-21.** The record schema gained an optional row-level
  `applicability` block (`{"status": "non_applicable", "reason": ...}`);
  absent means the pairing applies, so every record written before this
  validates unchanged. `settle_package_review` now raises
  `SubjectNotApplicable` for the exact `no bound required root slot
  \`<target>::ProgramEntry\`` rejection instead of `SystemExit` — any other
  settlement failure still exits — and `measure` turns it into a committed
  record whose four metrics are `unavailable` carrying that reason, which is
  the shape the validator and matrix already understood. Committed both rows
  the doc named: `wrapping_square_sum__uefi_x86_64__default.json` and
  `...__macos_x86_64__default.json`. One design point worth keeping: a
  non-applicable record speaks for one `(subject, target)` pairing, not for
  the leg, so it does NOT retire its host leg's projected row — the existing
  `test_unmeasured_host_legs_stay_explicit` caught that when the first cut
  let a committed record swallow `uefi_x86_64`'s "needs QEMU or UEFI
  hardware" row, and `matrix_rows` now only counts measurable records as
  coverage. `python3 tools/tests/test_benchmark.py` 29/29 (from 21), with the
  eight new cases sentinelled: disabling the applicability validation fails
  three by name, and letting a non-applicable record retire the leg fails two
  more. Re-verified at `12ea4941ebd` (z133, linux x86-64): acceptance holds —
  `wrapping_square_sum__uefi_x86_64__default.json` is committed with
  `applicability.status == "non_applicable"` and the settlement reason, the
  `uefi_x86_64`/`macos_x86_64` rows render on their own matrix lines
  (`benchmarks.md` :63/:60), and `python3 tools/tests/test_benchmark.py`
  reports 29/29. The same-name mined stub above is drained by this row.
- **BENCHMARK-SELECTION-CONTRAST-ROWS.** Mined candidate — covered.
  Sibling re-mine of the selection-row coverage recorded on the
  BENCHMARK-SELECTION-ROW-COVERAGE cluster (this section): contrast
  rows = selection-keyed `tools/benchmark/records/` records (e.g.
  `wrapping_square_sum__linux_x86_64__sel-885944b13b84` vs the pending
  default-selection row). Record production needs `tools/benchmark`
  (fenced by BENCHMARK-ROW-RESUMPTION ~03:42Z) plus the matrix doc/test
  pair — every slice is claimed elsewhere this wave. No independent
  Re-verified at `7241e02227` (z70 ledger, folded under
  the prior ledger cleanup):
  both `sel-` contrast rows still on disk, records census now 10
  (added `structural_proofs` default + `macos_x86_64`/`uefi_x86_64`
  cross rows), `test_benchmark.py` 29/29 green; records/ +
  `benchmarks.md` stay fenced to BENCHMARK-PROOF-SUBJECT-SELECTION
  (exp 14:19Z). Re-verified at `891eb5c5844` during the fold
  (linux x86-64): census still 10 with both `sel-` rows and the
  `structural_proofs`/`macos_x86_64`/`uefi_x86_64` additions intact,
  `python3 tools/tests/test_benchmark.py` re-run 29/29 OK; the records
  surface fence persists (BENCHMARK-PROOF-SUBJECT-SELECTION ~14:19Z)
  plus a new tools/benchmark claim under
  BENCHMARK-REJECTED-ROW-RECORDING (~16:28Z).
  covered — sibling of BENCHMARK-SELECTION-ROW-COVERAGE; record production owner-laned
- **BENCHMARK-ROW-RESUMPTION.** — mined candidate; scope verified at
  `e927421a8b` on linux x86-64 (z181): re-mines the benchmark-row
  production lane — resume committing measured `tools/benchmark/
  records/` rows plus `wiki/drafts/benchmarks.md` matrix entries as
  host/subject coverage becomes producible. The linux_x86_64
  `wrapping_square_sum` default row landed (z177 lane, measured
  `e7c0099cb2`), and the three cross-target compile legs are
  committed at `52ceeeabb7` (all `--no-run` from a linux x86_64
  host). Every remaining row is host-gated: `linux_arm64` needs an
  arm64 runner, `macos_arm64`/`windows_x86_64` need their own hosts
  (windows peak-RSS stays `unavailable`), `uefi_x86_64` needs
  QEMU/hardware. The host-producible record surfaces
  (`tools/benchmark/records` + `wiki/drafts/benchmarks.md`) are
  claim-fenced this wave to BENCHMARK-PROOF-SUBJECT-SELECTION
  (~14:19Z). No unfenced slice exists on this host.

  slice remains under this name.
  covered — both resumption halves are landed contract; remaining rows host-gated, record surfaces owner-laned
- **BENCHMARK-SELECTION-ISOLATION-ROWS.** Resolved — the per-selection row
  isolation the name asks for is the record contract itself and is already
  exercised on `origin/main`: one versioned JSON per
  (subject, target, exact rule selection) — `selection_label` renders the
  empty selection as `default` and a non-empty enable/disable set as a
  `sel-<hash>` label, `test_unsorted_selection_rejected` pins sorted-unique
  exact rule names, and `30d1d3fc56` landed the first non-default row
  (`wrapping_square_sum__linux_x86_64__sel-885944b13b84.json`,
  CopyPropagation disabled) beside the same subject's default-selection
  rows, so no selection's measurements blur into another's. The matrix
  renders selection as its own isolating column
  (wiki/drafts/benchmarks.md). Verified at `1edade1a48` and again at
  `1a772e4ae1` (non-default `sel-885944b13b84` row still on disk):
  `python3 tools/tests/test_benchmark.py` 21/21 green on linux x86-64.
  Open surface left to siblings: more *selection* rows (contrast/variant)
  and the unfilled host legs.
- **BENCHMARK-SELECTION-MATRIX.** Mined candidate; scope verified,
  resolved — named verbatim by resolved sibling
  BENCHMARK-SELECTION-ROW-COVERAGE as a sibling re-mine of the same
  coverage. That coverage is landed: `tools/benchmark/records/` holds
  six committed rows (`cli_mvp__linux_x86_64__default`,
  `wrapping_square_sum` on `linux_x86_64` under two non-default
  selections — `sel-885944b13b84` at `7ec604d7ef`,
  `sel-9c09e32a82fb` at `0969a3ea96a` — and cross-compile rows for
  `linux_arm64`, `macos_arm64`, `windows_x86_64` at `52ceeeabb7b`,
  runtime legs `skipped` per contract); the host-row matrix pins were
  repaired under the same row (`test_unmeasured_host_legs_stay_explicit`
  now pins `cross_platform_cli`/`local_unchecked`) and the
  `wiki/drafts/benchmarks.md` embedded matrix is regenerated to the
  six-row census; `python3 tools/tests/test_benchmark.py` 21/21 green
  on linux x86-64. The remaining gap — a re-measured `linux_x86_64`
  default-selection row at a newer revision — stays with
  BENCHMARK-LINUX-X64-ROW-REFRESH. No independent slice exists;
  sibling stubs BENCHMARK-SELECTION-ROW-MATRIX,
  BENCHMARK-SELECTION-VARIANT-ROWS and BENCHMARK-SELECTION-CONTRAST-ROWS
  mine the same coverage.
- **BENCHMARK-SELECTION-ROW-COVERAGE.** Mined candidate; scope verified,
  coverage landed — `tools/benchmark/records/` now holds three committed
  rows, up from the lone `cli_mvp/linux_x86_64/default` row this stub named:
  `wrapping_square_sum__linux_x86_64__sel-885944b13b84` (non-default
  selection, `CopyPropagation` disabled, compile+runtime measured at
  `7ec604d7ef`, recorded 2026-09-20T17:08Z) and
  `wrapping_square_sum__linux_arm64__default` (cross-compile row, runtime
  leg skipped per contract). The remaining gap — a re-measured
  `linux_x86_64` default-selection row at a newer revision — is
  BENCHMARK-LINUX-X64-ROW-REFRESH's named leg, and record production needs
  `tools/benchmark` (claimed by BENCHMARK-COMPARISON-OCCURRENCE-GATE until
  22:09Z) plus `wiki/drafts/benchmarks.md`/`tools/tests/test_benchmark.py`
  (BENCHMARK-HOST-ROW-MATRIX until 22:11Z). No independent slice exists
  here. Sibling re-mines of the same coverage: BENCHMARK-SELECTION-MATRIX,
  BENCHMARK-SELECTION-ROW-MATRIX, BENCHMARK-SELECTION-VARIANT-ROWS,
  BENCHMARK-SELECTION-CONTRAST-ROWS.
  Re-verified at `97be15c1b5` (z102, linux x86-64): the census has grown to
  ten committed rows — `cli_mvp` + `structural_proofs` linux_x86_64
  defaults, `wrapping_square_sum` defaults on `linux_x86_64`/`linux_arm64`/
  `macos_arm64`/`macos_x86_64`/`uefi_x86_64`/`windows_x86_64` plus its two
  non-default `sel-` contrast rows — and the "remaining gap" this stub
  recorded is closed: `wrapping_square_sum__linux_x86_64__default`
  (source_revision `e7c0099cb2`) was committed at `f5323f461a`, so the
  re-measured linux_x86_64 default-selection row exists and
  BENCHMARK-LINUX-X64-ROW-REFRESH's named leg is discharged. Current
  fences: `tools/benchmark/records` + `wiki/drafts/benchmarks.md` stay under
  BENCHMARK-PROOF-SUBJECT-SELECTION (~14:19Z); the remaining unfilled
  matrix cells stay host-gated per BENCHMARK-ROW-RESUMPTION's row. No
  independent slice.
  Re-verified at `7d03d489e3d` (linux x86-64): census unchanged at ten
  committed rows and `python3 tools/tests/test_benchmark.py` 29/29 green.
  Fence rotation this wave: `tools/benchmark/records` +
  `wiki/drafts/benchmarks.md` still under BENCHMARK-PROOF-SUBJECT-SELECTION
  (exp ~14:19Z), `tools/benchmark/benchmark.py` + README +
  `tools/tests/test_benchmark.py` now under BENCHMARK-REJECTED-ROW-RECORDING
  (Zergling-128, ~16:28Z), BENCHMARK-LINUX-X64-ROW-REFRESH re-held by
  zergling-z186 (~16:56Z). Producing surfaces stay claimed; no slice here.
  Re-witnessed at `12ea4941ebd` (zergling-132, linux x86-64): census
  still ten committed rows — both `sel-` contrast rows, both
  linux_x86_64 defaults and the five cross-target legs intact —
  `python3 tools/tests/test_benchmark.py` 29/29 OK. Fence rotation:
  records/ + benchmarks.md remain under BENCHMARK-PROOF-SUBJECT-
  SELECTION (~14:19Z); BENCHMARK-REJECTED-ROW-RECORDING's
  benchmark.py/test fence has drained; sibling item claims live on
  BENCHMARK-SELECTION-CONTRAST-ROWS (~12:35Z) and
  BENCHMARK-DEPEND-FREE-RUNNABLE-SUBJECT (~17:02Z). Producing surfaces
  stay claimed; no slice here.
- **BENCHMARK-SELECTION-ROW-MATRIX** — mined candidate; verify scope then implement.
- **BENCHMARK-SELECTION-VARIANT-ROWS.** Mined candidate — scope
  verified, coverage landed. Bare re-mine of the variant-selection leg
  already closed by resolved sibling BENCHMARK-SELECTION-MATRIX
  (`9d566263d0`): `tools/benchmark/records/` carries the
  non-default-selection rows this stub names —
  `wrapping_square_sum__linux_x86_64__sel-885944b13b84` and
  `sel-9c09e32a82fb` — beside the default-selection `cli_mvp` row and
  the three cross-compile rows (`linux_arm64`/`macos_arm64`/
  `windows_x86_64` at `52ceeeabb7b`, runtime legs `skipped` per
  contract); `test_benchmark.py` 21/21 green on linux x86-64 and the
  `wiki/drafts/benchmarks.md` embedded matrix is regenerated to the
  six-row census. The remaining gap — a re-measured `linux_x86_64`
  default-selection row at a newer revision — stays with
  BENCHMARK-LINUX-X64-ROW-REFRESH; record production is fenced to
  BENCHMARK-COMPARISON-OCCURRENCE-GATE (`tools/benchmark`) and
  BENCHMARK-HOST-ROW-MATRIX (`wiki/drafts/benchmarks.md` +
  `tools/tests/test_benchmark.py`). No independent slice; sibling stubs
  BENCHMARK-SELECTION-ROW-MATRIX and BENCHMARK-SELECTION-CONTRAST-ROWS
  mine the same coverage.
- **BENCHMARK-STANDALONE-SUBJECT.** Depend-free benchmark subject. Verified scope: a subject with no `depend()` skips the shared std plumbing and is the only compile reaching a published artifact at this revision — `samples/cli/basics/standalone` landed: empty `Main::main` bound to all hosted `ProgramEntry` roots, ~25 s end-to-end compile on w9 Linux x86-64. Runtime leg stays `skipped` (`--no-run`): `ProcessExit` provider authority is bound to the std package identity and a subject-local boundary machine produces no provider plan, so the artifact cannot exit cleanly. Re-verified on 97eeaf222a: `benchmark.py measure --target linux_x86_64 --no-run --print` publishes an 8192-byte artifact, median compile 27.6 s, peak RSS ~146 MB, runtime `skipped`; no committed record row yet. Remaining: the committed record row under `tools/benchmark/records/` and the `wiki/drafts/benchmarks.md` coverage entry are fenced to sibling claims this wave.
- **BENCHMARK-STD-COMPARISON-OCCURRENCE-GATE** — mined candidate; scope verified, resolved — re-mine of BENCHMARK-COMPILE-UNBLOCK-COMPARISON-OCCURRENCES' producer fix (integer comparison-occurrence rejection repaired at `76dc49a99e`; `terminal_product::integer_comparisons` counts selected occurrences only against the artifact-bound checked scope). The "std" residual the name implies — provider coverage for genuinely selected occurrences, std-wide verification — is exactly what the landed publication test pins. Re-verified green at `1edade1a48`: `cargo nextest run -p compiler --test integer_comparison_publication` → `selected_comparison_publication_preserves_complete_custody_among_builtins` PASS (19.9s, linux x86-64). Sibling re-mines named on the parent row: BENCHMARK-COMPARISON-OCCURRENCE-GATE, COMPARISON-OCCURRENCE-PRODUCER-COVERAGE, the INTEGER-COMPARISON-OCCURRENCE-* family.
- **BENCHMARK-SUBJECT-CORPUS-EXPANSION** — mined candidate; verify scope then implement.
  Historical std-dependent probes failed at the `Filesystem::host` provider join;
  re-drive before claiming a current blocker. Any remaining join repair belongs
  to build provider settlement, not NOMINAL-FIELD-FLOW.
- **BENCHMARK-SUBJECT-ROW-EXPANSION** — mined candidate; verify scope then implement.
- **BENCHMARK-WINDOWS-PEAK-MEMORY.** Scope verified at `5e2d355a02c0` —
  names the Windows leg of the versioned peak-memory axis
  (`wiki/drafts/benchmarks.md:51` `peak_memory_bytes` column). The
  machinery is landed in `tools/benchmark/benchmark.py`: on Windows the
  measured child runs inside a fresh job object and
  `_windows_job_peak_bytes` reports the job's `PeakJobMemoryUsed` in
  bytes (`run_measured`/`_windows_job_assign`, :131-200); hosts with no
  RSS route record the metric as unavailable rather than guessing.
  The only remaining leg is the Windows x64 host measurement itself —
  a `benchmark.py measure` run on Windows producing the recorded row —
  host-bound; nothing executable on this linux x86-64 host. Sibling
  stubs on the same surface: BENCHMARK-WINDOWS-PEAK-RSS,
  WINDOWS-PEAK-MEMORY-MEASUREMENT.
- **BENCHMARK-WINDOWS-PEAK-RSS** — mined candidate; verify scope then implement.
- **BENCHMARK-SUBJECT-CORPUS-EXPANSION.** — mined candidate; scope verified
  at `f600f8400b7`, covered and fenced — re-mines the "more measurable
  subjects" frontier behind the benchmark matrix, and every producible leg
  is claimed or gated this wave:
  (a) std-depend subjects (the corpus body): closed at tip — `omega update`
      on `samples/cli/arithmetic/euclid_gcd` (linux_x86_64 host) rejects
      inside the `omega-language-std` candidate check: `routed service
      field Filesystem::host has no exact Fused selected-provider-plan
      join` — the same std-wide settlement frontier already recorded for
      `prime_counter` at `18cebfa1062` (wiki/drafts/benchmarks.md). The
      repair is the selected-provider-plan join family, not a subject fix.
  (b) depend-free subjects: producible — a fresh `standalone` x
      `linux_x86_64` record regenerated and `validate`-clean on this host
      at this tip (1 compile sample, 31,692 ms; peak RSS 151 MB; 8192-byte
      artifact; runtime `skipped`). Committing it needs
      `tools/benchmark/records/` + `wiki/drafts/benchmarks.md`, fenced by
      BENCHMARK-CROSS-HOST-ROWS (~14:04Z); the runtime leg stays `skipped`
      until the Process-exit contract migration recorded on
      DEPENDENCY-FREE-RUNTIME-BENCHMARK-SUBJECT lands.
  (c) proof subjects: `structural_proofs` committed; `math_proofs` stays
      blocked at checked-call selection (PROOF-SAMPLES-CHECKED-CALL-
      SELECTION).
  No unfenced slice exists under this name.
  Historical std-dependent probes failed at the `Filesystem::host` provider join;
  re-drive before claiming a current blocker. Any remaining join repair belongs
  to build provider settlement, not NOMINAL-FIELD-FLOW.

- **BUILD-DEPEND-PURPOSE-AWARE-LOCKS.** — mined candidate; scope verified,
  resolved — the purpose-aware lock landed at `748b07f622` ("packages: split
  dependency declarations into product and build purposes"). Purpose rides
  every resolved edge and the lock record: the canonical source-closure
  subject writes purpose-split authored rows and purpose-tagged edges (text
  v2; legacy v1 decodes as product-only), and locked recovery/comparison in
  `resolution/graph/resolve/locked/comparison.rs` keys every edge by
  (requester, purpose, ordinal), so a `build_depend` row resolves only
  against a selection recorded under Build purpose — a wrong-purpose lock
  edge or a dropped/repurposed row rejects instead of widening scope.
  Witnessed green at `20a11975b8` (linux x86-64): `cargo nextest run -p
  package-manager --test suite -E 'test(~purpose)'` — all 5
  `dependency_purposes` tests pass, incl.
  `purpose_tagged_edges_survive_acquisition_review_lock_and_recovery`,
  `a_wrong_purpose_edge_in_the_lock_text_rejects`, and
  `dropping_or_repurposing_a_build_row_rejects_locked_recovery`. Adjacent
  unrelated failure recorded under unrelated_failures:
  `source_diff_commands::cases::update_to_retargets_both_scope_rows_of_a_dual_purpose_package`
  fails since `3cf600bbe3` (one-integration-binary consolidation) — the
  fixture's child re-runs the suite binary with `--exact cases::<test>`,
  which no longer matches the `source_diff_commands::cases::*` names, so the
  child runs 0 tests.
- **C2L-BASELINE-FAILURE-ATTRIBUTION.** Inserted owner row — the name other rows cite as the owner of the checked-trees-to-lowered-psi residual families (scalar-return custody / provider attachment / attached-unit sets); no `**NAME.**` row previously existed. The attribution leg itself is landed: the member-by-member census `wiki/drafts/c2l_failure_census_d936717f.md` (landed `54e321bdf00`) records 2199 members / 24 failures, every one inside an already-owned `known_baseline_failures.md` family, and the §checked-trees-to-lowered-psi ledger re-read stands at `6ef64f6dd6` (2152 run, 2093 passed, 59 failed, 1 SIGTERM). The repair legs the name covers are separately owned and live: scalar-return custody → C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES (live claim, `tests/owned_record_return_source.rs`, 10:35Z); provider attachment / attached-unit sets → C2L-RESIDUAL-FAILURE-ATTRIBUTION's lane plus WRITE-ONLY-BORROW fences; bare fixture spellings → ENTRY-CONTENT-ROOTS; the unattributed tail → C2L-UNATTRIBUTED-FAILURE-TAIL (verified empty at `f43b4e8869c`). No unowned slice remains under this name — it is a ledger-owning umbrella, not a repair row.
  Claim freshness at `7a62e962b2a7` (zergling-168): the recorded scalar-return
  custody claim (~10:35Z) has drained — that family is currently unfenced on
  the board owner C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES. Live neighbors on
  the crate: CHECKED-TREES-TO-LOWERED-PSI-UNATTRIBUTED-SET (Devin/f8ad694c,
  ~14:02Z) — the tail stub this row verified empty — and
  NEW-C2L-SUITE-ERASED-PROOF-FORMALS-COMPILE-FIX (z78, ~13:53Z) on
  `tests/registered_callback_lifetime.rs`. Umbrella verdict unchanged.
  covered — ledger umbrella; census landed, every family owned by a named row
- **C2L-BOUNDARY-BYTE-BUFFER-FAILURES** — mined candidate; scope verified,
- **C2L-BOUNDARY-BYTE-BUFFER-FAILURES.** — mined candidate; scope verified,
  family repaired. Re-mines the boundary-byte-buffer group of
  checked-trees-to-lowered-psi recorded green at the d8d48fe4ff re-reading
  in `wiki/drafts/known_baseline_failures.md`; re-verified at this revision
  on linux x86-64: `cargo nextest run -p checked-trees-to-lowered-psi -E
  'test(~boundary_byte_buffer)'` — 10/10 PASS. Live residual families in
  that crate stay owned elsewhere (bare boundary-trait fixture spellings by
  ENTRY-CONTENT-ROOTS; scalar-return custody / provider attachment /
  attached-unit sets by C2L-BASELINE-FAILURE-ATTRIBUTION and
  C2L-RESIDUAL-FAILURE-ATTRIBUTION). No independent slice remains; sibling
  stub LOWERED-BOUNDARY-BYTE-BUFFER-FAILURES carries the same resolution.
- **C2L-UNATTRIBUTED-FAILURE-TAIL.** Scope verified at `f43b4e8869c` —
  the tail is measured, and it is empty. The fresh member-by-member
  `checked-trees-to-lowered-psi` census at `d936717fd2`
  (`wiki/drafts/c2l_failure_census_d936717f.md`, landed `54e321bdf00`)
  records 2199 members / 24 failures, every one inside an already-owned
  `known_baseline_failures.md` family: missing transitive machine plans
  (16), unit-plan omissions (3), scalar-return custody (4 — since
  re-spelled onto admitted surfaces at `f43b4e8869c`), the ranked
  safe-point fixed-fuel bound (1), and the proof-search blowup (1).
  Verdict text: "the unattributed tail is still empty." No independent
  slice exists — a census that finds unattributed members is the next
  dispatch's input, and producing it again is a fresh measurement, not a
  residual.
- **CANARY-ACQUIRES-THROUGH-HELPER-RETURN** — mined candidate; scope verified, real residual — the canary exists and is rostered (`tests/omega/pass/capabilities/acquires_through_helper_return`, in `tests/canary_suite.rs` + `tests/fixture_rosters/reports_and_capabilities.rs`), but the rostered fixture is red on `1fc01bb690`: `pass_canaries_compile` filtered to it fails at native-artifact Terminal production — `InvalidUnitMachinePlan { machine: "Main::main", reason: "attached Unit closure is missing a checked transitive machine plan", omission: "`Main::main` has no admitted body (local construction stopped at signature)" }`. The remaining leg is the checked/lowering gap that stops `Main::main`'s local construction at the signature (authority-propagating helper-return shape reaches no admitted body), not a missing corpus member. Fixture path is under a live same-item claim (Devin / cathr-acquires-helper-return).
- **CANARY-CORE-NAME-COLLISION** — mined candidate; verify scope then implement.
- **CANARY-DUPLICATE-OVERLOAD-DECLARATIONS.** Resolved — the
  duplicate-overload canary corpus exists and is driven. Re-verified green
  on linux x86-64 at `d648f6862e47`:
  `cargo nextest run -p compiler --test canary_suite -E
  'test(=surface_and_targets::duplicate_overload_and_visibility_
  admissions_reject) or test(=surface_and_targets::repeated_exact_
  declaration_selection_compiles)'` → 2/2 PASS.
  `surface_and_targets.rs:1016` pins five duplicate-admission fixtures
  with `expected.txt` fragments through checked semantics
  (`duplicate_named_machine_overload_rejected`,
  `duplicate_imported_machine_overload_rejected`,
  `duplicate_trait_requirement_overload_rejected`,
  `imported_name_collides_with_local_data_rejected`,
  `recursive_argument_imported_name_collision_rejected`), and the legal
  half is pinned by `repeated_exact_declaration_selection_compiles`;
  remaining fail-corpus overload rejections (operators/domains) are
  covered by `fail_canaries_reject_with_expected_diagnostic_fragment`.
  Adjacent surface: DUPLICATE-OVERLOAD-RESOLUTION mines the resolution
  rule itself, not this canary-coverage row. No independent slice exists.
- **C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES.** The slice this row was opened
  for has landed; what remains is one dead negative control and two reds owned
  elsewhere. `source_replay_requires_the_exact_affine_return_transfer` passes
  as of `eb376f7340` ("terminal-production: re-verify the checked permission
  ledger before lowering"), which added
  `psi/compiler/terminal-production/src/checked_ledger.rs`, called from
  `terminal_production.rs:593` before lowering.
  `wiki/drafts/known_baseline_failures.md:604-616` already records it repaired.

  Measured at `f44a1177ed`: `cargo nextest run -p checked-trees-to-lowered-psi
  -E 'test(~owned_record_return_source)'` selects **9** tests, not the 4 this
  row's earlier acceptance named, and reads 6 passed, 3 failed.

  All three reds share one root cause: a `retain` body with a discarded-call
  prefix is no longer planned as an ordinary unit-effect plan, it is a
  composed-control plan.
  - `discarded_scalar_invocation_precedes_whole_owned_return` asserts
    `terminal_unit_effects.for_machine(..)` is `Some`. A stale plan-ownership
    assertion; no production change needed.
  - `effectful_discarded_call_writes_before_return_across_fuel` is a real
    production refusal, "composed Unit scalar call requires structural call
    custody", from `src/unit/attached_unit/composed_control/`.
  - `source_replay_rejects_return_parameter_and_carrier_substitution` is a
    **dead negative control**: it panics on
    `plan.structural_result.as_mut().unwrap()` before reaching its first
    assertion, because the `[copy] Record` case now has no ordinary plan to
    tamper with, so its `[Entry; 3]` and `Buffer<Entry>` cases never run
    either. It currently verifies nothing. The production rejection it means
    to pin does exist and is source-derived — mutating the terminator result,
    `plan.result` and `states[0].structural_parameters[1]` together, so the
    plan stays internally consistent, still refuses with "structural return
    exchanged its owned parameter" and "structural graph result signature
    disagrees with source". The repair is test-side: re-point it at whichever
    planner owns the machine, ordinary `machines` or `composed_machines`.

  Fenced, so partition before picking this up: `tests/owned_record_return_source.rs`,
  `tests/guarded_scalar_returns_source.rs` and `src/returns` sit under a live
  C2L-RESIDUAL-FAILURE-ATTRIBUTION claim, and `src/unit` under
  STRUCTURAL-UNIT-LOWERING.

  Acceptance: the `~owned_record_return_source` filter is green across all 9
  members, and the substitution control actually executes its three carrier
  cases rather than panicking before its first assertion.

- **C2L-UNATTRIBUTED-FAILURE-TAIL.** Mined candidate; scope verified, tail
  still empty — fresh member-by-member reading at `23392bc467` (linux
  x86-64): 2183 run, 2127 passed, 56 failed. All 55 FAIL + the
  proof-search blowup map onto the ledger's owned families with identical
  diagnostics (33 bare `Service<R>` spellings, 16 missing transitive
  machine plans, 4 scalar-return custody, 2 ranked safe-point bounds, 1
  blowup). Shrinkage since the `6ef64f6dd6` reading: the 3 site_guard
  crash-namespace rejections and the closed-projection replay member now
  pass — their unbisected suspects sit in the retained-borrow/
  result-contract lane. Re-censused at `e5bbe53956` after the crate moved:
  2199 run, 2175 passed, 23 FAIL + 1 blowup — the bare `Service<R>`
  family is GONE (landed `00a69f066b`, the `src/tests` carrier migration)
  and the missing transitive-plan family collapsed into provider-
  attachment legs. Current reds: `unit_state_graph::provider_attachments`
  ×9 + `provider_attachment_source` ×6 (provider-attachment lane),
  `unit_plan_omissions` ×3, `owned_record_return_source` ×3 +
  `guarded_scalar_returns_source` ×1 (scalar-return custody lane),
  `unit_state_graph::bindings` ×1, and the `mixed_nominal_integer_comparison`
  blowup (SIGTERM >1026s). Full attribution recorded in
  `wiki/drafts/known_baseline_failures.md`; sibling subsets C2L-SCALAR-
  RETURN-SOURCE-CUSTODY-FAILURES / CHECKED-TREES-TO-LOWERED-PSI-
  UNATTRIBUTED-SET remain named on the parent row.
- **CANARY-ACQUIRES-THROUGH-HELPER-RETURN.** Mined candidate — resolved,
  already landed. The stub names the canary
  `tests/omega/pass/capabilities/acquires_through_helper_return` (chapter
  18 nested-acquires: `Vault::pick` mints `Folder::Writable` at the
  `Desktop` boundary, `Backup::stage` and `Main::main` receive it through
  helper returns — the authority-flow report must propagate `acquires`
  up the call graph with helper provenance). The machinery lives in
  `typed-trees-to-checked-trees/src/facts/capabilities.rs`
  (`propagate_nested_capability_flows` fixpoints
  returns/derives/acquires over the service-reach call edges);
  `d96a0fda39`-era board notes recorded it claimed at 23:30Z in the
  InvalidUnitMachinePlan family, but the family failure it was grouped
  with never touched this canary. Re-verified at `e70748c995` on linux
  x86-64: `OMEGA_PASS_CANARY_FILTER=acquires_through_helper_return`
  pass_canaries_compile PASS (18.3s) and
  `capability_flows_retain_exact_direct_and_propagated_sites` PASS —
  both propagated routes (`Backup::stage acquires via Vault::pick`,
  `Main::main acquires via Backup::stage`) pinned by the roster.
  Record: `wiki/drafts/canary_acquires_through_helper_return.md`.
- **CANARY-NATIVE-WRAPPER-WRITE-ALL-RESULT.** Mined candidate; scope
  verified at 1edade1a48 — names the canary
  `tests/omega/pass/filesystem/native_wrapper_write_all_result` (the
  payload-carrying `Filesystem::write_all -> UnitResult` deep-fix guard:
  bad path must deliver `Error`, good path `Ok`, through an assigned
  field). Infrastructure already landed: the fixture carries a deployable
  `build.omg` binding all four hosted `ProgramEntry` roots
  (`builder.roots.bind(<t>::ProgramEntry, Main::main)`), is rostered
  (`fixture_rosters/native_filesystem_canaries.rs:82`,
  `NATIVE_WRAPPER_WRITE_ALL_RESULT`), and has its test leg
  `native_wrapper_write_all_result_passes` in
  `native_filesystem_canaries/native_filesystem_passes.rs:452`. What the
  leg still needs is a macOS arm64 run — the whole
  `native_filesystem_canaries` suite is `#![cfg(target_os = "macos")]` and
  asserts via `compile_exact_macos_entry` + real `/tmp` writes, so the
  PASS cannot be witnessed on a linux_x86_64 host (a compile-only leg is
  already covered by the pass-corpus compile roster). Re-verified at
  `27deadf4122`: the suite's `#![cfg(target_os = "macos")]` gate is
  still in place and all four `ProgramEntry` bindings persist in the
  fixture's `build.omg`, so the only open deliverable remains the
  macOS witness. Prerequisite: a
  seeded macOS arm64 host (SEED-HOST-CHAIN-LEGS' audited list); then run
  `cargo nextest run -p compiler --test canary_suite -E
  'test(=native_filesystem_canaries::native_filesystem_passes::native_wrapper_write_all_result_passes)'`
  there and record the result on this row.
  **macOS arm64 witness recorded 2026-09-21 — and it FAILS.** This row's only
  deliverable was a macOS arm64 run, and this is that host, so the leg is no
  longer host-blocked: it is a measured red.
  `cargo nextest run -p compiler --test native_filesystem_canaries -E
  'test(/native_wrapper_write_all_result/)'` fails after 121 s with
  "selected ProgramEntry establishment rejoins 0 Terminal attachment identities;
  expected one; the machine's unit plan was omitted at local construction at
  `state graph: state signature: parameter signature: attached data shape`
  (state 0)".

  That omission phase is a recorded member of the attached-Unit-plan family
  (`known_baseline_failures.md`), whose producer site is
  `t2c/src/execution/unit/calls/signatures.rs` — NOT the record-literal store
  phase whose guard was repaired at `238ff31237c0c`, which is why that repair
  does not close this one. Build note for whoever re-runs it: `-p compiler` with
  only an `-E` filter still links every test binary in the crate and exhausts
  the disk; name `--test native_filesystem_canaries` to build one.

- **CANARY-RUNTIME-GUI-FOREGROUND-WINDOW-EXIT.** Mined candidate — resolved (fenced residual): scope verified
  2026-09-20 (z164): re-mines `tests/omega/pass/host/runtime_gui_foreground_window_exit`
  — the fixture exists, is authored correctly (intrinsic `Service<Gui>` field,
  all four hosted ProgramEntry binds), and is rostered in `ACTIVE_PASS_CANARIES`
  (`canary_suite.rs`). The dedicated run test
  `runtime_gui_foreground_window_exit_canary_runs` is `#[cfg(windows)]`-gated by
  design (no value assertion possible off the real desktop), so on linux x86-64
  the fixture's only leg is `pass_canaries_compile`. Measured at `739e4e81e9`:
  the compile fails at `selected ProgramEntry Service field Main::gui
  requires a selected Fused provider for boundary Gui`
  (`selected-dispatch/src/service_custody/root.rs`) — an EARLIER stop than the
  ledger's recorded `Lowering(InvalidUnitMachinePlan)` family
  (`known_baseline_failures.md`:152, stale for this member) and the siblings'
  moved ProgramEntry-rejoin stop. Sibling gui canaries
  `runtime_gui_window_{lifecycle,blit}_exit` pass the same leg on the same host
  — the selected linux Gui provider covers their ops but not `foreground_window`
  (the 0-arg value-returning GetForegroundWindow import). The producing surfaces
  (provider-plan production in
  `typed-trees-to-checked-trees/src/execution/unit/*`, Fused custody in
  `selected-dispatch`) sit in PROVIDER-ATTACHMENT-MACHINE-PLAN /
  ENTRY-CONTENT-ROOTS / GENERAL-CYCLIC-EXECUTION lanes — outside this item's
  fixture fence; the windows run leg is host-gated by design.

  Re-verified at `c3e3bfec35`: fixture still
  authored under `tests/omega/pass/host/runtime_gui_foreground_window_exit`
  and rostered in `ACTIVE_PASS_CANARIES` (`canary_suite.rs:4911`); the
  Fused-`Gui`-provider compile stop, the `#[cfg(windows)]` run-leg gating,
  and the producing-surface ownership are unchanged.
  Re-verified at `96bc0ef8104` on linux x86-64 (dev-88738):
  `OMEGA_PASS_CANARY_FILTER=runtime_gui_foreground_window_exit` — 1 fail,
  identical stop (`Main::gui requires a selected Fused provider for
  boundary Gui`); residual stays with the named producing-surface items.
  covered — fixture landed and rostered; run test is Windows-gated
- **CANARY-RUNTIME-LITERAL-DISPATCH-EXIT.** — mined candidate; scope verified
  2026-09-20 (z105): re-mines the `control_flow/runtime_{integer,string}
  _literal_dispatch_exit` pair in the known-baseline-failures InvalidUnitMachinePlan
  family ([wiki/drafts/known_baseline_failures.md](wiki/drafts/known_baseline_failures.md):150).
  The recorded attribution is stale — on `fcfb576fe9` both fixtures still fail
  `pass_canaries_compile` but at a different stage: `selected ProgramEntry
  establishment rejoins 0 Terminal attachment identities; expected one` from
  `selected-dispatch/src/service_custody/root.rs` — neither `machines` nor
  `composed_machines` in `terminal_unit_effects` carries an
  attachment_type_identity for the bound (Main::main, entry state). Both
  fixtures bind all four hosted ProgramEntry roots and pass the Fused-provider
  prerequisite, so the moved failure is now the unit-effects plan emitting no
  attachment identity for a literal-dispatch machine — the producing surfaces
  (`typed-trees-to-checked-trees/src/execution/unit/*`,
  terminal-production receiver eligibility) sit in GENERAL-CYCLIC-EXECUTION's
  unit-plan lane and ENTRY-CONTENT-ROOTS' live claim (00:00Z). Sibling
  family members are individually claimed this wave (CANARY-WIRE-EXACT-
  ARRAY-WITHOUT-COUNT-EXIT 00:33Z, CANARY-ACQUIRES-THROUGH-HELPER-RETURN
  23:30Z); the baseline doc entry needs its refresh when the family
  attribution settles. Re-witnessed 2026-09-21 at `e70748c9954` (linux
  x86-64, `omega --check` on each fixture root): both fixtures still reject
  with the same ProgramEntry-rejoin diagnostic, and the emitted text now
  carries the per-fixture omission cause — the integer fixture's unit plan
  is omitted at `state graph: terminator: unsupported tail: transition
  chain` (state 0; a `transition` chain is not one of the admitted tail
  shapes at `execution/unit/state_graph/mod.rs` — only an empty tail, one
  return expression, one unconditional jump, or an exact when/else pair),
  while the string fixture's unit plan is omitted at `structural field
  store: record literal field`
  (`execution/unit/structural_scalar_store/mod.rs:572`). So the family
  splits into two distinct missing unit-plan admissions — transition-chain
  terminator tail and record-literal field store — both under the same
  `execution/unit/` ownership lane; the ledger refresh has since landed —
  `known_baseline_failures.md`'s re-measurement at `7b224763615` records
  this pair under the moved selected-entry rejoin gate.
  Re-witnessed 2026-09-21 at `891eb5c584` (linux x86-64,
  `OMEGA_PASS_CANARY_FILTER` pass_canaries_compile): identical outcome —
  integer `transition chain`, string `record literal field`, still 0
  rejoined attachment identities; the repair remains in the
  `execution/unit/` unit-plan lane.
  Re-witnessed at `fff3918dc42f` (linux x86-64, zergling-111): identical
  outcome again — the integer fixture's unit plan is still omitted at
  `state graph: terminator: unsupported tail: transition chain` (state 0)
  and the string fixture's at `structural field store: record literal
  field`; both still report 0 rejoined attachment identities in 50s under
  the filtered pass_canaries_compile run.
  covered — repair sits in the `execution/unit/` unit-plan lane (known-baseline family owner), not under this name
- **CANARY-WIRE-EXACT-ARRAY-WITHOUT-COUNT-EXIT.** — mined candidate; scope verified
  2026-09-20 (z180): re-mines `tests/omega/pass/wire/runtime_wire_exact_
  array_without_count_exit`, which already exists and is rostered in
  `ACTIVE_PASS_CANARIES` (compiled by `pass_canaries_compile`): a
  `#0 readings: [u32; 4]` exact-array field wired through generated `encode`
  with no synthetic `<name>_count` sibling, then `exit_process(70)`. On linux
  x86-64 at `0db54f596a` the canary is red at `selected ProgramEntry
  establishment rejoins 0 Terminal attachment identities; expected one`
  (`selected-dispatch/src/service_custody/root.rs`). Fixture bisection at that
  revision: fields + `exit_process` alone pass; declaring the numbered schema
  without calling the codec passes; any `Telemetry::encode`/`decode` call
  inside `Main::main` fails — independent of field count, the `[u32; 4]`
  member, control-flow shape, or codec direction — and a verbatim copy of
  `runtime_wire_roundtrip_primitive_exit` fails identically. Same moved-failure
  family as CANARY-RUNTIME-LITERAL-DISPATCH-EXIT: the unit-effects plan emits
  no `attachment_type_identity` for the bound (`Main::main`, entry state) once
  the entry machine calls a generated codec; the producing surfaces
  (`typed-trees-to-checked-trees/src/execution/unit/*`, terminal-production
  receiver eligibility) sit in GENERAL-CYCLIC-EXECUTION's unit-plan lane and
  ENTRY-CONTENT-ROOTS' live claim — outside this item's fence.
- **CHECKED-CALL-SELECTION-OCCURRENCE-MATH-PROOFS.** Resolved — sibling
  stub of PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION's resolved row (the
  resolved verdict is recorded at `1fc01bb690`):
  `validation/src/proof_contracts/contract_entailment/specification_calls.rs`
  checks selected concrete calls before fact intake and attributes the
  callee's selected precondition to the call's exact subject;
  `proofs/case_call_wrong_subject` rejects `empty_only(other)` when only
  `known in Tree::Empty` is established, `case_citation_wrong_result`
  pins the result side, pass twin `case_call_premises` compiles
  (re-verified green at `f1675418b1`). Remaining owners are the parent's
  own list (abstract signatures, domain predicates, postcondition
  transport of case membership, induction); `samples/cli/proofs/
  math_proofs` is fenced by PROOF-SAMPLES-CHECKED-CALL-SELECTION. No
  independent slice exists here. Re-verified at `fff3918dc42` (linux
  x86-64) (z153): `OMEGA_FAIL_CANARY_FILTER=proofs/case_call_wrong_subject,proofs/case_citation_wrong_result`
  rejects with the recorded fragments and
  `OMEGA_PASS_CANARY_FILTER=proofs/case_call_premises` compiles — 2/2
  green; still no independent slice.
  OCREQ-REQUEST-BINDING. Re-witnessed at `4e716c7844` on linux x86-64
  (CHAIN-OCREQ-ENTRY-BINDING dispatch): `sh
  tests/bootstrap/omega-request/run.sh --identity` PASS — all bound
  identities verified (622,933-byte receipt request, 565,909-byte
  customer, 45-byte expected observation).
  OCREQ-REQUEST-BINDING. Re-witnessed at `832c55e69b` on linux x86-64
  (OCREQ-ENTRY-BINDING dispatch): `sh
  tests/bootstrap/omega-request/run.sh --identity` PASS — identical bound
  identities (622,933-byte receipt request, 565,909-byte customer, 45-byte
  expected observation); executing half stays seed-host-gated.
  covered — sibling stub of PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION's resolved verdict (1fc01bb690)
- **CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION.** Scope verified and leg
  completed — the attribution ledger
  `wiki/drafts/known_baseline_failures.md` §checked-trees-to-lowered-psi
  now carries a fresh member-by-member reading at 6ef64f6dd6 (Linux
  x86-64): 2152 run, 2093 passed, 59 failed, blowup member SIGTERM'd at
  ~892s. Prior family owners reconfirmed at identical panic sites
  (33 bare `Service<R>` spellings, 16 missing transitive machine plans,
  3 site_guard crash rejections, 4 scalar-return custody cases,
  C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT blowup); deltas recorded: the `established by`
  call-result qualification family closed in-window (registered_callback_
  lifetime green; 851052b4f8f / 1fc01bb6907), and two new families opened —
  ranked safe-point segment bounds charge component-scale ceilings
  (3·2³³ / BoundOverflow; unchanged `derive_fixed_safe_point_segments` reads
  39e156c73a0's new verified inputs; post-base 7591b2607c7 is mid-migration
  on the same surface) and closed-projection replay admits invalid/foreign
  member symbols (suspects 39e156c73a0 / 143636cec8a, unbisected). Residual:
  the two new families want a single-test bisect by their owning lanes;
  sibling stub CHECKED-TREES-TO-LOWERED-PSI-UNATTRIBUTED-SET remains open.
  Sibling alias C2L-BASELINE-FAILURE-ATTRIBUTION re-mines this attribution
  surface; scope verified at `27deadf412` — the reading is current
  (C2L-UNATTRIBUTED-FAILURE-TAIL's fresh 23392bc467 census attributes all
  56 reds onto owned families with identical diagnostics), and every
  residual family the alias is named for on the cluster rows is fenced to
  a live claim: scalar-return custody
  (`tests/owned_record_return_source.rs` ×4, plus
  `src/returns`/`terminal-production` source-replay legs) under
  C2L-RESIDUAL-FAILURE-ATTRIBUTION (~04:59Z Sep 21), provider-attachment
  and attached-unit sets under GENERAL-CYCLIC-EXECUTION /
  UEFI-OS-HANDOFF / WRITE-ONLY-BORROW / PROOF-CERTIFICATION-BRIDGE
  per the ledger's fence notes, and the ledger doc itself under
  LOWERED-UNIT-FAILURE-ATTRIBUTION (~01:17Z) and this row's own live
  claim (~01:42Z). No independent slice remains; the open board owners
  are C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES (the four-test family)
  and C2L-RESIDUAL-FAILURE-ATTRIBUTION (in flight).

  Re-verified at `a84ebca972` (linux x86-64, scoped): the attribution has
  drifted in the green direction — the **ranked safe-point segment
  bounds** family the 6ef64f6dd6 reading opened (bisected to
  7591b2607c77) is now GREEN: both
  `ranked_countdown_lowers_to_verified_resumable_interpreter_execution`
  and `ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64`
  pass, repaired by the terminal-fixed-fuel bounded-walk series
  (`0d0f85459ad`/`8c6294fcfc8`/`9b6aed267fe`/`faf902cea48`/
  `94e764a6da6`). `closed_record_projections_replay_exact_sources_carriers_and_all_siblings`
  stays green (closed at 7af30a1f839a). The scalar-return custody family
  is still red at an identical signature —
  `owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel`
  fails `Lowering(Unsupported("composed Unit scalar call requires
  structural call custody"))` — while the other sampled
  owned_record_return_source members pass; ownership unchanged
  (C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES, live ~10:35Z on
  `tests/owned_record_return_source.rs`). The ledger doc needs a
  one-line refresh for the ranked pair whenever its fence next opens
  (LOWERED-UNIT-FAILURE-ATTRIBUTION lane); verdict otherwise holds.

  Re-verified at `23338b3d093c` (linux x86-64): the pending doc refresh
  has landed on main — `known_baseline_failures.md` now carries the
  `e7c0099cb2b7` member-level reading (81b9e64faeb, via this lane's
  sibling item): the ranked-pair closure is recorded, the
  missing-transitive-plan family re-attributed to 18 members
  (provider_attachment_source ×6, unit_plan_omissions ×3,
  guarded_scalar_returns ×1 drained; conformance_applications ×3,
  composed_operand_catalogs ×5, composed_unit_internal_calls ×1 joined),
  scalar-return custody down to the single structural-custody member,
  and a new 4-member `scalar_array_source::cyclic` index-out-of-bounds
  family (`call_lowering.rs:419`) surfaced for the scalar-graph/LICM
  lane. Fence map refreshed: C2L-RESIDUAL-FAILURE-ATTRIBUTION and this
  row's earlier claims have expired; live fences at this reading are
  UEFI-OS-HANDOFF (~10:19Z), C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES
  (~10:35Z on `tests/owned_record_return_source.rs`),
  PROOF-CERTIFICATION-BRIDGE (~10:52Z),
  GENERAL-CYCLIC-EXECUTION-OPTIMIZER (~13:51Z), and sibling
  CHECKED-TREES-TO-LOWERED-PSI-UNATTRIBUTED-SET under live claim
  (~14:02Z). Verdict stands — no independent slice.

  Re-verified at `59e0b5ec22` (linux x86-64, scoped 77-member filter —
  [z70 ledger](wiki/drafts/c2l_baseline_attribution_z70.md)): 72/77 green;
  both recorded closures hold (ranked pair, closed-projection replay),
  and all 5 reds are owned families at identical signatures — the single
  structural-custody member and the 4-member `scalar_array_source::cyclic`
  index-out-of-bounds family (`call_lowering.rs:419`). No new drift.
  Re-verified at `661a4d50c0a` (linux x86-64, same 77-member filter,
  C2L-BASELINE-FAILURE-ATTRIBUTION dispatch): **76/77 — the
  `scalar_array_source::cyclic` family closed.** All five cyclic
  members pass; repair is `891194236af` ("scalar successors keep
  checked and lowered target indices apart"), which fixes the exact
  recorded signature — `lower_scalar_graph_successor` indexed the
  checked state roster by a lowered branch index (`len 1, index 3` at
  `call_lowering.rs`), now rebound onto separate index spaces.
  The single remaining red is the scalar-return custody member
  (`owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel`,
  identical `Lowering(Unsupported("composed Unit scalar call requires
  structural call custody"))` at tests/owned_record_return_source.rs:306);
  its owner claim C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES has drained —
  the family is currently unfenced. The z70 ledger draft is folded into
  this row and deleted (its field note's instruction); the canonical
  ledger `known_baseline_failures.md` stays fenced to
  NEW-KBF-LIB-CLUSTER-AND-STALE-ROWS-REFRESH (~12:26Z). No independent
  slice remains.
- **CHECKED-TREES-TO-LOWERED-PSI-UNATTRIBUTED-SET.** — scope verified and
  bisected at `c267df86acb8` (linux x86-64): the unattributed set is the
  three members the 6ef64f6dd6 reading opened as two new families, now
  fully attributed —
  1. **Ranked safe-point segment bounds** (2 tests, still red):
     `structural_control_cases::ranked_countdown_lowers_to_verified_resumable_interpreter_execution`
     (per-edge segments read `0x600000000` instead of 3 at
     src/tests/structural_control_cases.rs:1497) and
     `ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64`
     (`BoundOverflow` at :1890). First-bad commit **7591b2607c77**
     ("bound segments through ranked cyclic components") — green at its
     parent, red at the commit; the recorded prime suspect `39e156c73a0`
     is green at itself, cleared by test. The break is the derivation
     rewrite's own component-scale charging (`natural_component_geometry`
     in terminal-fixed-fuel `fuel_certification/outcome_bounds.rs`), not
     its inputs. Owning lane: ranked-cycle/fuel (the commit's lane).
  2. **Closed-projection replay admission** (1 test, already closed):
     `expression_preparation::bindings::tests::closed_record_projections_replay_exact_sources_carriers_and_all_siblings`
     — first-bad **090802e8a790** ("evaluate member-read leaves of
     computed aggregate constants in constant position"), green at both
     recorded suspects (`39e156c73a0`, `143636cec8a`); **fixed by**
     **7af30a1f839a** ("replay the complete closed record projection for
     scalar member sources") — green at `c267df86acb8`.
  Residual: family 1 stays red under the ranked-cycle/fuel lane;
  recording the attributions into `wiki/drafts/known_baseline_failures.md`
  is fenced to that doc's live claims (LOWERED-UNIT-FAILURE-ATTRIBUTION,
  CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION). Re-verified at `1805e07c7d`
  (2026-09-21 ~06:02Z): the fence map rotated but the surface stays
  claimed — the ledger doc now sits under RC-REPOSITORY-CLOSURE
  (swarm-z143, exp 13:57Z) and family 1's surface
  `terminal-fixed-fuel/src/fuel_certification` is under PSIIR
  (devin-848972c1, exp 13:59Z). No independent slice remains.
  CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION).
  CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION).
- **CLI-COMMANDS** — mined candidate; verify scope then implement.
  covered — contentless mined stub; CLI surface is documented in AGENTS.md with no named gap
- **COMPARE-TEST-SELECTION.** — mined candidate; verify scope then implement.
- **COMPILER-OBSERVATION-PRODUCTS.** — mined candidate; scope verified,
  resolved as a deliberately closed surface with no authorized slice.
  Verified scope: the surface this name points at is deliberately closed —
  [product boundaries](omega-rust/omega/compiler/compiler/README.md#product-boundaries-and-observations)
  state compilation produces only the requested product and diagnostics
  (`CheckOnly`/`TerminalArtifact`/`RetainedNativeArtifact`/`NativeExecutable`/
  `ObjectContainer`/`BuildArtifacts` on `CompileReport`), never optional
  JSON/HTML/debug dumps, disassembly or timing files; "there is no
  full/output-only observation policy". Per
  [wiki/spec/build/observations.md](wiki/spec/build/observations.md), build
  observations are retained execution facts (canonical input identity,
  attempted operations, output custody) — not a second admission policy or a
  new product kind, and no source doc authorizes one. The existing
  observation-carrying products sit under live claims: `compile_report.rs`
  (CUSTODY-MATRIX-HARNESS-MIGRATION, 23:25Z), `pcc.rs` + crate manifest
  (PCC-PRODUCT-PUBLICATION, 22:38Z), `terminal_product/integer_comparisons.rs`
  (BENCHMARK-COMPARISON-OCCURRENCE-GATE, 22:09Z). A new observation product
  would first need a concrete authorized design. Sibling re-mine name:
  COMPILER-OBSERVATION-OUTPUTS.
  covered — deliberately closed product boundary per compiler README; no authorized design for a new observation product
- **COMPILER-PASS-PROFILE-TIMINGS** — advanced: the omega-side product legs now record into the `CompileTimings` accumulator the checked record carries. `CheckedCompilation::timings_mut` exposes it; `produce_retained_terminal_artifact` records `terminal-production`, `terminal-verification` and `native-realization-proposal` rows via take/put-back; the direct route carries `terminal-production` on `ProgramEntryTerminalArtifact::stage_timings` (merged back in `prepare_native_product`), and `NativeInputReuse` records `native-input-preparation` on cache miss. Remaining legs: enable the accumulator at `checking.rs` (`shared_timings = CompileTimings::default()`; INTERNAL-PASS-PROFILE-TIMINGS's fence), merge the stage ladder into `CompileReport` and print rows under `--timings` (compilation-report + omega CLI fences), then decompose the coarse boundary rows into per-stage rows — finer in-Psi rows need a Psi-owned timing carrier because `terminal-production` cannot depend on `artifacts` under `psi_does_not_depend_on_omega`.
- **COMPILER-PASS-PROFILING** — mined candidate; verify scope then implement.
- **COMPARISON-OCCURRENCE-PRODUCER-COVERAGE.** Resolved — covered sibling
  re-mine named on INTEGER-COMPARISON-OCCURRENCE-PRODUCER-COVERAGE
  (:7898) and BENCHMARK-STD-COMPARISON-OCCURRENCE-GATE (:6868), same
  record: the producer-coverage leg is the landed pin itself —
  `terminal_product::integer_comparisons` (repaired `76dc49a99e`) counts
  selected integer occurrences against the artifact-bound checked scope,
  and `compiler`'s `integer_comparison_publication` suite
  (`selected_comparison_publication_preserves_complete_custody_among_
  builtins`, PASS on linux x86-64 at `f2f39039da`, re-witnessed `1edade1a48`)
  requires complete custody coverage of the producer's selected
  occurrences among builtins — the provider coverage for genuinely
  selected occurrences the name gestures at is exactly what the pin
  exercises. No independent slice exists.
  COMPILER-OBSERVATION-OUTPUTS. Re-audited at `6f91898606` (z161, ~08:33Z)
  under the COMPILER-OBSERVATION-OUTPUTS dispatch: README.md:92 still
  pins "no full/output-only observation policy" and observations.md plus
  the cited product surfaces are unchanged since the row's recorded
  verification (empty diff `3dac85e5cc..HEAD`); the recorded claims have
  all drained — no live claim covers `compile_report.rs`, `pcc.rs`, or
  `terminal_product/integer_comparisons.rs`. The closure is a spec
  boundary, not a fence — a new observation product still needs an
  authorized design before any slice exists.
- **COMPILER-PASS-PROFILE-INSTRUMENTATION.** — mined candidate; scope verified,
  covered — z142's verification draft
  (`wiki/drafts/compiler_pass_profile_instrumentation_z142.md`) already
  adjudicated this stub as a re-mine of sibling COMPILER-PASS-PROFILE-TIMINGS'
  remaining legs, and every enumerated leg is now landed: `c115576398` carries
  the recorded stage-timings ladder into `CompileReport::timings` and wires
  `--timings` → `collect_timings` → `CompileTimings::enabled()` (checking.rs),
  while `cli/compilation.rs` prints `outcome.timings.phases()` +
  `report.timings()` rows plus `total elapsed` on stderr, pinned by
  `timings_are_opt_in_stderr_output_without_debug_files` (omega/tests/
  command_line.rs — re-verified green on this host). The only open residual is
  the per-Psi-stage decomposition, a Psi-owned timing-carrier design decision
  that belongs to the parent COMPILER-PASS-PROFILE-TIMINGS row
  (psi_does_not_depend_on_omega forbids the dependency route).
- **COMPILER-PASS-PROFILE-TIMINGS.** — advanced: the omega-side product legs now record into the `CompileTimings` accumulator the checked record carries. `CheckedCompilation::timings_mut` exposes it; `produce_retained_terminal_artifact` records `terminal-production`, `terminal-verification` and `native-realization-proposal` rows via take/put-back; the direct route carries `terminal-production` on `ProgramEntryTerminalArtifact::stage_timings` (merged back in `prepare_native_product`), and `NativeInputReuse` records `native-input-preparation` on cache miss. Enable+report legs landed (this wave): `CompileRequest::with_timings` reaches `PreparedCheckedSource::prepare`, which builds an `enabled` accumulator when asked; `CompileReport::timings()` carries each target's recorded ladder (`compiler -> tooling` dep edge `artifacts` is downward-legal per `workspace_layering_is_respected`), and `--timings` prints the report's stage rows after the command-level rows. Remaining leg: decompose the coarse boundary rows into per-stage rows — finer in-Psi rows need a Psi-owned timing carrier because `terminal-production` cannot depend on `artifacts` under `psi_does_not_depend_on_omega`; the prepared-project route (`PreparedLocalProjectNativeRequest`/`check_prepared_local_project`) also does not yet thread the flag. Witnessed on linux x86-64 at the pre-`c17b63d7592` green base (main is red there on `crossed_window`'s missing `CrossingDirection` arg — unrelated sibling landing): `timings_request_carries_the_recorded_stage_ladder_to_the_report` + `checked_admission_and_compilation_do_not_write_debug_dumps` pass; `compilation-report` + `assembled-syntax-to-checked-compilation` lib 93/93; `omega --lib` 16/16; architecture layering filter 14/14.
- **COMPOSABLE-PAIR-DESCRIPTORS.** Compose selected-lowering pair-rule descriptors over independent axes instead of enumerated products. Landed: `PairMachineEffects` is now a struct of three axis enums — `PairNonUnitSurface` (isolated vs indexed-pointer-read fold), `PairFaultDischarge` (isolated vs discharged-by-literal vs discharged-by-obligation), `PairUnitDefRelation` (covered vs retired-when-dead vs operand-swapped) — with admission computed as the conjunction of per-axis gates and the eight prior variants expressed as named consts over the product (`literal_fold/pair_rule.rs`); the obligation gate now derives the obligation from the consumer kind's declared field instead of a variant-coupled kind list. `PairOperandShape` is now a struct of `PairLiteralPosition` (right/left/sole `Use` victim) × `PairOperandResult` (surviving operand, swapped operand, constant-of-literal, literal recompute) × `PairTailCustody` (bare, auxiliary `Use`s under zero-provenance custody, scratch `Def`s under occurrence-free custody, or the per-access mixed tail) with the twelve grammars expressed as named consts over the product; `victim_operand`, `fold_immediate`, and the action/validator matchers now read the axes directly — `compute/actions.rs`'s twelve-arm operand-shape match collapsed into one axis-driven admission (head layout from position+result, drop-tail custody from the tail axis) and `compute/constraints.rs`'s `validate_immediate_row` re-derives the row grammar from `(operand_result, result)` so a descriptor mistake still cannot self-certify. `PairUnitEffects` is now a struct of two `PairConsumerBindingAdmission` axes (`consumer_fixed_view`, `consumer_early_clobber`; `tied_to` stays a fixed rejection since no composition can rebuild a shared-home tie) with `ISOLATED`/`BOUND_CONSUMER_OPERANDS`/`BOUND_EARLY_CLOBBER_CONSUMER_OPERANDS` as named consts. Remaining: none — every pair-rule descriptor is axis-composed. Re-witnessed at `ab6ad3e438a` (linux x86-64, pre-rebase): the landed axis decomposition is present and green — `selected-instructions-to-selected-instructions` 342/342 filtered tests pass over rewrites/selected_lowering + pair surfaces; the row now correctly records "Remaining: none".
- **COMPOSABLE-PAIR-DESCRIPTORS.** Compose selected-lowering pair-rule descriptors over independent axes instead of enumerated products. Landed: `PairMachineEffects` is now a struct of three axis enums — `PairNonUnitSurface` (isolated vs indexed-pointer-read fold), `PairFaultDischarge` (isolated vs discharged-by-literal vs discharged-by-obligation), `PairUnitDefRelation` (covered vs retired-when-dead vs operand-swapped) — with admission computed as the conjunction of per-axis gates and the eight prior variants expressed as named consts over the product (`literal_fold/pair_rule.rs`); the obligation gate now derives the obligation from the consumer kind's declared field instead of a variant-coupled kind list. Remaining: `PairOperandShape`'s twelve-variant product (literal position × result kind × auxiliary/scratch tail) and `PairUnitEffects`'s bound-consumer pairs (`BoundConsumerOperands`, `BoundEarlyClobberConsumerOperands`).
- **COMPOSABLE-PAIR-DESCRIPTORS.** Compose selected-lowering pair-rule descriptors over independent axes instead of enumerated products. Landed: `PairMachineEffects` is now a struct of three axis enums — `PairNonUnitSurface` (isolated vs indexed-pointer-read fold), `PairFaultDischarge` (isolated vs discharged-by-literal vs discharged-by-obligation), `PairUnitDefRelation` (covered vs retired-when-dead vs operand-swapped) — with admission computed as the conjunction of per-axis gates and the eight prior variants expressed as named consts over the product (`literal_fold/pair_rule.rs`); the obligation gate now derives the obligation from the consumer kind's declared field instead of a variant-coupled kind list. Remaining: `PairOperandShape`'s twelve-variant product (literal position × result kind × auxiliary/scratch tail) and `PairUnitEffects`'s bound-consumer pairs (`BoundConsumerOperands`, `BoundEarlyClobberConsumerOperands`).

- **CONST-GENERIC-EXTENT-RANGE-DISCHARGE.** — mined candidate; verify scope then implement.
- **CONST-GENERIC-INFERRED-EXTENT-RANGE** — mined candidate; verify scope then implement.
- **CONSTRUCTIVE-REAL-FOUNDATIONS.** Mined candidate — resolved:
  scope verified, owned elsewhere. The name re-mines the constructive
  Real foundation — replacing `source/library/core/real.omg` (N5's
  temporary opaque axiomatic package; "N6/N8 can replace this package
  with the constructive Cauchy quotient without changing consumers'
  contracts"). [mathematics.md](source/library/core/mathematics.md)
  records the state: `cauchy.omg` carries generator/modulus machines in
  the currently expressible pointwise proofs (`doubled_nat_max_modulus`,
  `doubled_nat_max_threshold` compose `M3(e) = nat_max(M1(2e), M2(2e))`
  statically), but the heterogeneous transitivity theorem still awaits
  two entailment-tier capabilities — general function/predicate binders
  and the quotient step — that keep `converges_together_at_triangle_split`
  from verifying (math roster N3). The migration itself is owned on this
  board by **PROOF-CONTRACT-MIGRATION** and **QUOTIENT-THEOREM-LIFT**;
  replacing Real requires the relation, witness, quotient and
  receiving-axiom contracts to survive. No independent slice exists here.
- **COORDINATOR-OVEROWNERSHIP-AUDIT** — mined candidate; scope verified, audit
  artifact exists and its actionable finding is landed. The row re-mines
  `wiki/drafts/coordinator_overownership_audit.md`, which swept every named
  sequencing owner (`compiler`, `terminal-production`, `native-realization`,
  plus the rule-named build/product owners) at `4a6bd936dc` and recorded four
  findings. F1 (unreachable `compiler/src/compiler/native/prepared.rs`) is
  REPAIRED on `origin/main` at `e5492eee179`. Re-verified there: F2
  (`terminal-production`'s 1,181-line `receiver_eligibility.rs` derivation
  resident in the sequencer — placement debt, flagged for relocation when it
  next grows), F3 (`native-realization`'s terminal-authority policy
  subsystem — interim verdict: load-bearing, monitor), and F4 (the orphan
  optimized-semantic-wrapper codec — owned by REPRESENTATION-OWNERSHIP in
  TASKS_OPTIMIZER.md, coordinated with PIPELINE-OWNER-CONSOLIDATION)
  all stand as recorded. Residual relocations are
  sibling items' moves, not audit work; no unclaimed slice remains.
  Re-witnessed at `7b25940907`: F1 repair holds (`native/prepared.rs` absent,
  no dangling references), F2 stands (`receiver_eligibility.rs` still resident
  at 1,181 lines in `psi/compiler/terminal-production`), F4's orphan codec
  (`optimized_semantic_wrapper_object/codec.rs`) remains unrelocated with
  DURABLE-CODEC-EXTRACTION live (exp 07:34Z) and the wrapper dirs fenced to
  UEFI-PHYSICAL-SEMANTIC-ENTRY (exp 08:44Z).
  Re-witnessed at `0a0662ad27a` (z161): all four findings hold — F1's
  `native/prepared.rs` is still absent with no dangling references, F2's
  `receiver_eligibility.rs` is still resident at 1,181 lines in
  `psi/compiler/terminal-production` (unfenced; placement-debt verdict
  unchanged), F3's `terminal_authority_policy` subsystem is still resident in
  `native-realization` and now sits under FILESYSTEM-RELEASE-CONTRACT's claim
  (zergling-z27, exp ~14:20Z), and F4's orphan codec is still unrelocated —
  DURABLE-CODEC-EXTRACTION is no longer live (row exists, no claim) while the
  `optimized_semantic_wrapper_{object,encoding}` dirs stay fenced to
  UEFI-PHYSICAL-SEMANTIC-ENTRY (z88, exp ~08:44Z). No unclaimed slice remains.
  Re-witnessed at `3a82039327` (z150, linux x86-64): all four findings hold —
  F1's `native/prepared.rs` still absent with no dangling references, F2's
  `receiver_eligibility.rs` still resident at 1,181 lines, F3's
  terminal-authority subsystem still resident and now unfenced
  (FILESYSTEM-RELEASE-CONTRACT lapsed; BUILD-EXCLUSION-REALIZATION holds only
  unrelated native-realization paths, ~15:52Z), F4's codec still unrelocated —
  and the wrapper dirs are no longer fenced to UEFI-PHYSICAL-SEMANTIC-ENTRY
  (claim lapsed) while DURABLE-CODEC-RELOCATION is live again (z120,
  ~16:35Z, holding rewrite/selection surfaces). The relocation itself remains
  the owning sibling's slice; the audit carries no unclaimed work.
  The truncated duplicate stub line for this name is removed.
- **CROSS-PACKAGE-DYNAMIC-LOAN-ORIGIN.** Mined candidate; scope verified —
  resolved re-mine of the cross-package dynamic loan-origin cluster already
  closed by SHARED-RECEIVER-LOAN-ORIGIN (resolved at `e76d715c8e`), per
  resolved siblings CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN (:6124) and
  PACKAGE-CROSS-VISIBILITY-LOAN-ORIGIN (:8366): the recorded failure
  `cross_package_visibility::public_dynamic_return_may_carry_private_
  producer_selected_evidence` ("requires an exact retained loan origin for
  its shared receiver") passes after the retained-lineage/borrow-evidence
  family landed — all 21 `cross_package_visibility` tests green at
  `dcfb595098` with zero loan-origin diagnostics (re-verified on the
  sibling rows; the surface is unchanged at `27deadf4122`). No independent
  slice exists; residual umbrella ownership stays on
  CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN.
- **CTTL-FAILURE-ATTRIBUTION** — mined candidate; scope verified, resolved.
  Names the attribution pass over the `typed-trees-to-checked-trees` section
  of `wiki/drafts/known_baseline_failures.md` (last recorded reading:
  `660f5af762` macOS arm64, 4159 run / 6 failed — the long-standing trio
  plus the rank_ranges field-endpoint set). Re-verified green on linux
  x86-64 at `d32183a35c`:
  `cargo nextest run -p typed-trees-to-checked-trees --lib --no-fail-fast`
  → 5014 run, 5014 passed, 0 failed — every recorded member closed
  (`indexed_operand_access_preserves_shared_collection_and_owned_index`,
  `consuming_call_that_returns_an_obligation_transfers_its_origin`,
  `scalar_caller_retains_call_produced_record_local_before_getter`, and
  the three rank_ranges field-endpoint cases all pass), so the residual
  tail is empty and there is nothing left to attribute. The ledger
  section's stale draft rows belong to the live claims already fencing
  `wiki/drafts/known_baseline_failures.md` (LOWERED-UNIT-FAILURE-
  ATTRIBUTION until ~01:17Z, BASELINE-PACKAGE-COMPILATION-INPUTS until
  ~20:19Z); this lane claims no file paths.
  Re-verified at `ded56393da2` (linux x86-64): the custody gate itself is
  unchanged — `selected-dispatch/service_custody` last moved at
  `0e1977994b9` and the stop still emits at `root.rs:145`; the
  `linux_dynamic_realization` pin suite reads 3/4 PASS. The fourth is a
  new unrecorded baseline failure:
  `aggregate_foreign_boundary_members_refuse_at_terminal_entry_establishment`
  no longer reaches the pinned stop — the fixture now refuses earlier,
  inside checked-trees-to-lowered-psi emission with
  `Lowering(Unsupported("record store destination projected beyond its
  authored root"))` (guard added by `9a81cd68774`); the `4607987316f` pin
  is stale. `bde84d1765a`'s one-hop projected-receiver admit is adjacent
  progress but does not touch establishment custody — the lane is still
  red upstream of itself. Re-verified at `a84ebca972` on linux x86-64
  (z146): the service-custody stop is unchanged —
  `selected-dispatch/src/service_custody/root.rs:146` still emits
  "selected ProgramEntry establishment rejoins {} Terminal attachment
  identities; expected one" (line shifted by `3bbc8855339`'s stage
  naming), `source/psi/test-parser.sh` remains the lane's only
  entrypoint, and the `aggregate_foreign_boundary_members` early-refusal
  recorded above is still absent from `wiki/drafts/known_baseline_failures.md`.
  The lane is still red upstream of itself.
  Re-verified at `0a0662ad27a` (linux x86-64): the custody stop is
  unchanged at `root.rs:146` and the pin suite is back to 4/4 PASS —
  `36b4b2c0a0d76` re-pinned `aggregate_foreign_boundary_members_refuse_at_terminal_entry_establishment`
  against the earlier c2l guard, discharging the `ded56393da2` staleness
  note. The z50 parser merge (`e1fed3a9f83ec`: generic parameter lists on
  data declarations, harness growth) landed upstream but does not move
  the custody frontier. No claims on `service_custody`, `source/psi`,
  `source/omega`, or the test-parser surfaces; the only adjacent fence
  is NOMINAL-FIELD-FLOW's `boundary_dispatch.rs` (z30 ~11:23Z), disjoint
  from the stop. The blocker is the item-owned
  OMEGA-PRODUCT-COMPILER-SOURCE frontier, not a fence.
  Remaining: none inside this row.
- **CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN.** Mined candidate — resolved:
  a named sibling in the cross-package dynamic loan-origin cluster closed by
  SHARED-RECEIVER-LOAN-ORIGIN (resolved at `e76d715c8e`); the recorded
  failure
  `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`
  ("requires an exact retained loan origin for its shared receiver") passes
  after the retained-lineage/borrow-evidence family landed. Re-verified at
  `1257982206` (linux x86-64): `cargo nextest run -p compiler --test
  package_compilation_inputs -E 'test(~cross_package_visibility)'
  --no-fail-fast` — 21/21 pass including the named evidence-loan control.
  No independent slice; detail in
  `wiki/drafts/cross_package_dynamic_loan_origin.md`.
- **CROSS-PACKAGE-DYNAMIC-LOAN-ORIGIN.** Mined candidate; scope verified —
  resolved re-mine of the cross-package dynamic loan-origin cluster already
  closed by SHARED-RECEIVER-LOAN-ORIGIN (resolved at `e76d715c8e`), per the
  resolved siblings CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN and
  PACKAGE-CROSS-VISIBILITY-LOAN-ORIGIN: the recorded failure
  `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`
  ("requires an exact retained loan origin for its shared receiver") passes
  after the retained-lineage/borrow-evidence family landed. Re-verified at
  `d6a0625f6b` (macOS arm64, mbx/nextest): all 21 `cross_package_visibility`
  tests pass with zero loan-origin diagnostics; detail in
  `wiki/drafts/cross_package_dynamic_loan_origin.md`. No independent slice
  remains.
- **DEPENDENCY-FREE-RUNTIME-BENCHMARK-SUBJECT.** Mined candidate — resolved,
  gate recorded at `867443a8fd`. The runtime leg needs a depend-free subject
  whose process completes cleanly, and two joined gates stand in the way.
  (a) Process completion on every hosted target is a provider operation:
      `source/library/std/targets/linux_x86_64/entry.omg` completes through
      the `exit_group` syscall sequence and the canonical `ProcessExit`
      provider is bound to the std package identity
      (`source/library/std/process_exit.omg` +
      `targets/*/process_exit_impl.omg`), which a `depend()`-free subject
      cannot import or select.
  (b) A boundary machine declared in the subject's own package produces no
      provider plan — only dependency packages contribute them — so no
      package-local provider route exists either
      (`samples/cli/basics/standalone/README.md`; consistent with
      entry_roots.md's "omission from that complete set denies authority"
      and process_exit.md's "selecting a provider grants no authority").
  The repair is the spec-named migration in the
  [Process-exit contract](#process-exit-contract) section
  — the provider/capability/cross-stage legs that decide whether a root
  package may contribute a provider plan and bind exit authority — not a
  benchmark-harness change. Until it lands, depend-free subjects stay
  `--no-run` with `runtime_ms` honestly `skipped` (the matrix records this).
  Sibling stubs on the same surface: DEPENDENCY-FREE-BENCHMARK-SUBJECT,
  DEPENDENCY-FREE-MEASURABLE-SUBJECT (subject landed `3dd805679c`),
  BENCHMARK-DEPEND-FREE-RUNNABLE-SUBJECT, BENCHMARK-MEASURABLE-SUBJECT-CORPUS,
  BENCHMARK-STANDALONE-SUBJECT.
  Correction (z177, `e7c0099cb2`, linux x86-64): the `--no-run` conclusion
  is stale — `wrapping_square_sum` (depend-free, `3dd805679c`) produced a
  measured runtime row at
  `tools/benchmark/records/wrapping_square_sum__linux_x86_64__default.json`
  (5 exit-0 samples, median 4.37554 ms, published 8192 B artifact), so a
  depend-free subject's process does complete through a route the two
  joined gates above did not cover. The provider-authority analysis may
  still describe depend-free subjects that need `ProcessExit` explicitly;
  this subject's entry evidently does not.
  covered — wrapping_square_sum runtime row measured exit-0 (5 samples); gate analysis superseded by that record
- **DEPENDENCY-FREE-MEASURABLE-SUBJECT.** — mined candidate; resolved as
  covered (verified `fff3918dc42`; re-verified at `796814691e4c` (z140 leg):
  `wrapping_square_sum` subject intact and eight committed records under
  `tools/benchmark/records/` still present, linux_x86_64 default row still
  measured — compile 33,397 ms median, runtime 5× exit-0 at 4.376 ms median,
  8192 B published artifact): the measured depend-free subject is
  landed — `wrapping_square_sum` (`3dd805679c`) with committed records
  under `tools/benchmark/records/` incl. the measured linux_x86_64 default
  row at `e7c0099cb2` (compile 33,397 ms median; runtime leg measured
  exit-0), per the z177 correction on the DEPENDENCY-FREE-RUNTIME-
  BENCHMARK-SUBJECT carrier above. Committing further measured rows is
  BENCHMARK-ROW-RESUMPTION / BENCHMARK-CROSS-HOST-ROWS territory; the
  records path is live-fenced under BENCHMARK-PROOF-SUBJECT-SELECTION.
  No slice under this stub.
  covered — wrapping_square_sum subject and measured records landed (3dd805679c); further rows are BENCHMARK-* territory
- **DEPENDENT-RELATIONAL-PROOFS-VIEWS.** — mined candidate; scope verified at
  `ded56393da`: re-mines the same chapter_12 sentence as resolved sibling
  **DEPENDENT-RELATIONAL-PROOF-SUPPORT** (this section) — "implementation
  support for relational proofs and views remains narrower". The landed
  slice covers strict relational bounds discharging representability through
  the ceiling's carrier at every integer width (`ordered_values::
  composed_ceiling_gap` + `operand_carrier_bound`, pinned by
  `guard_narrowing/tests.rs:65
  a_strict_place_ceiling_proves_the_increment_for_narrower_carriers`).
  This row's distinct residual is the "views" half plus the solver-general
  proofs opening — a multi-session solver/representation leg, not a bounded
  slice landable from this stub. The other named opening, equality facts
  through writes, is DEPENDENT-VALUES-CHECKER-COVERAGE's slice (already
  annotated on this board). No live claim currently fences
  `proof_contracts/ordered_values.rs` or `guard_narrowing`; the neighboring
  DEPENDENT-RELATIONAL-PROOF-VIEW-SUPPORT stub re-mines the same sentence.
- **DEPENDENT-RELATIONAL-PROOF-VIEW-SUPPORT.** — mined candidate; scope
  verified at `0f75a052f0`, covered. Re-mines the same chapter-12 sentence
  as the sibling above ("solver-general proofs and dependent views remain
  narrower", chapter_12_dependent_types.md:18): the views half of
  relational proofs plus solver-general widening is exactly the residual
  DEPENDENT-RELATIONAL-PROOFS-VIEWS records — a multi-session
  solver/representation leg, not a bounded slice landable under this name.
  The landed relational slice (strict bounds discharged through the
  ceiling's carrier, `ordered_values::composed_ceiling_gap` +
  `operand_carrier_bound`) stays where the sibling puts it; equality
  through writes is DEPENDENT-VALUES-CHECKER-COVERAGE's slice. No
  independent slice exists here.
- **DEPENDENT-VALUES-CHECKER-COVERAGE.** Mined candidate; scope verified at
  `33eb8d92ff`: the residual named by the rewritten
  [chapter 12](wiki/language_guide/chapter_12_dependent_types.md) sentence is
  equality facts through writes — `requires self.count == before` +
  `self.count = self.count + 1` still rejects `ensures self.count == before +
  1` ("cannot prove ensures contract for exit from Counter::bump"), while the
  unwritten and stale-equality directions check correctly (soundness holds;
  the write retires the requires row from exit contexts). The slice is
  transport inside `checks/contracts/exits/scalars.rs`: substitute the
  equality fact live at the write's incoming statement context into the
  place's retained `AssignedValue` expression. Note: a live freeform claim by
  `devin-w9-dependent-values` (ticket 6c47f5116a7c, expires ~22:20Z) already
  fences `proof_contracts/default_domains` for this item — coordinate before
  working it. Re-verified at `a43a1929b1` (linux x86-64, 2026-09-21 ~07:23Z):
  the named slice's surface `checks/contracts/exits/` is now fenced outright
  to PROOF-CERTIFICATION-BRIDGE (zergling-136, exp 10:52Z) and the sibling
  `checks/contracts/writes.rs` is under NEW-MNR-LOCAL-INITIALIZER-PREDICATE-
  DOMAIN (zergling-200, exp 15:18Z) — no unfenced slice remains under this
  name until those lanes settle. Re-verified at `416e9dd7e6`
  (z175 leg): the sibling's named work landed at `e8bcd8812989f` ("psi:
  enforce predicate-domain establishment on local initializers") — a local
  `let x: T in D = value` under a predicate-body domain now discharges D's
  proof facts against the initializer before minting the membership, and
  its zergling-200 fence has drained. `cargo nextest run -p
  typed-trees-to-checked-trees -E 'test(~predicate_domain) or
  test(~predicate)'` → 52/52 PASS on linux x86-64, including all seven
  `predicate_domain_initializer_*` pins. `checks/contracts/exits/` stays
  under PROOF-CERTIFICATION-BRIDGE.
- **DIVISION-CANARY-ENTRY-BINDING.** Scope verified at ea025447fe — re-mines
  the surface CANARY-EXACT-ENTRY-SELECTION already owns ("exact entry
  selection for division/value canaries and entry binding"). The missing
  `build.omg` ProgramEntry binds the name gestures at were already
  repaired under the prior canary repair (e5912f303a added binds for
  all four hosted targets to `operators/runtime_integer_division_value`);
  a fresh audit of `tests/omega/pass` shows every rostered
  native-execution division canary now carries binds
  (runtime_{unsigned,signed,i64}_division*, runtime_*modulo*,
  runtime_*divide*, const_fold_unsigned_divide_arg_exit,
  saturating/wrapping_signed_divide_min_by_neg_one,
  calls/runtime_cross_callee_division_exit). The pass-corpus fixtures
  lacking `build.omg` are compile-only (`bounded_guarded_remainder`,
  `runtime_contained_range_write`, `runtime_call_enum_*` etc. compile
  green — witnessed via filtered `pass_canaries_compile`) or use a
  bespoke entry (`saturating_divide_native` compiles through
  `compile_exact_macos_entry`, no roots binds needed). Remaining
  entry-selection correctness is CANARY-EXACT-ENTRY-SELECTION's live
  claim (owner 'Devin / canary-exact-entry-selection', expires
  2026-09-20T21:52Z).
  covered — build.omg binds repaired; residual entry-selection correctness is CANARY-EXACT-ENTRY-SELECTION
- **DIVISION-VALUE-ENTRY-SELECTION.** Mined candidate; scope verified at
  `d936717fd2d` — covered and fenced. The name re-mines
  CANARY-EXACT-ENTRY-SELECTION's own text ("exact entry selection for
  division/value canaries and entry binding"). The binding half is
  landed: `e5912f303a` added `build.omg` ProgramEntry binds for all four
  hosted targets to `operators/runtime_integer_division_value`, and
  sibling DIVISION-CANARY-ENTRY-BINDING's audit confirms every rostered
  native-execution division/value canary now carries binds. The
  selection-exactness residual and the fixture paths are both live this
  wave: PROGRAM-ENTRY-SELECTION-EXACTNESS (Devin / z175, 05:43Z) holds
  the correctness lane, and DIVISION-CANARY-ENTRY-BINDING (Jarod /
  swarm-w9-division-entry, 03:05Z) path-claims the division fixture
  directories. No independent slice exists.
- **DUPLICATE-OVERLOAD-RESOLUTION.** Resolved — the duplicate-overload
  resolution rule is landed and pinned. Re-verified at `7b2594090733`
  (linux x86-64): `validate_named_callable_overload_declarations`
  (`validation/src/machine_calls/callable_overloads.rs:19-70`) resolves
  overload identity by `NormalizedNamedCallableIdentity` (path + parameter
  signature + result dispatch set) for both machines and trait
  requirements; same identity in non-separate scopes rejects with
  "duplicate named {kind} overload ... predicate-only result refinements
  do not distinguish overloads", while scope-separated or identity-
  distinct declarations coexist. Pinned by
  `surface_and_targets::duplicate_overload_and_visibility_admissions_reject`
  (five fixtures green at `00e1da7ae2a`) plus the legal-half
  `repeated_exact_declaration_selection_compiles`. Sibling stub
  CANARY-DUPLICATE-OVERLOAD-DECLARATIONS covers the corpus row. No
  independent slice exists.
- **DUPLICATE-NAMED-MACHINE-OVERLOAD.** — implemented on
  `zergling/z197-duplicate-named-machine-overload`: member calls through an
  attached result-overload family (`self.helper.pick()` with same-named
  `Helper::pick` overloads) no longer fail-closed at the unresolved-value-call
  fence. `MachineScope::attached_call_target` treated the family's ambiguous
  same-name lookup as invisible; it now accepts membership in the ambiguity set
  and keeps the first visible member as the provisional binding, which
  `resolve_named_result_overloads` rebinds to the destination's dispatch set.
  Pins: pass `domains/runtime_result_domain_attached_overload_exit` (native run,
  exit 70 only when each member call binds the matching overload), fail
  `calls/duplicate_attached_machine_overload_rejected` (identical attached
  duplicates still reject at declaration validation). Surfaced but NOT fixed
  (pre-existing, overload-independent): a `&self` member call whose callee body
  never touches `self` lowers with the receiver operand present but the callee's
  scalar-graph signature drops it ("computed borrow disagrees with its selected
  callee signature", checked-trees-to-lowered-psi structural_arguments gate) —
  the run fixture reads `self.seed` so both overloads retain the receiver.

- **DYNAMIC-CALL-OCCURRENCE-SPANS.** Scope verified, resolved — landed at
  `95019d341a9` ("omega: dynamic-call occurrences bind
  dispatch parents and span custody"): every surviving `CallDynamic*`
  produces a coverage occurrence joining the emitted call instruction's
  span, dispatch-parent identity, and role
  (`derive_dynamic_call_span` — now under
  `omega/backend/artifacts/native-artifact/src/physical/derivation/`
  after the crate's relocation — covers direct, stored, forwarded-parameter
  and forwarded-descriptor calls with single-record rejoin,
  non-empty/non-relocated span, and exact-relocation checks).
  Witness green at `28a3cc7fea`:
  `dynamic_call_occurrence_binds_its_dispatch_role_and_
  parent_identity`. Cross-references that cited this item's fence are now
  historical: TRANSLATION-VALIDATION and TV-INTRINSIC-SPAN-ARMS rows (this
  file) describe CallDynamic* occurrences as absent — they predate the
  landing; occurrence-replay residual for the remaining families stays on
  **TV-OPERATOR-APPLICATIONS-REPLAY** per those rows. Sibling stubs naming
  the same surface: DYNAMIC-CALL-PHYSICAL-EVIDENCE,
  DYNAMIC-DISPATCH-ROW-MAPS.
  covered — landed at 95019d341a9 (dynamic-call occurrences bind dispatch parents and span custody)
- **DYNAMIC-RETURN-LOAN-ORIGIN.** Mined candidate; scope verified — resolved
  re-mine of the same cross-package dynamic loan-origin cluster closed by
  SHARED-RECEIVER-LOAN-ORIGIN at `e76d715c8e`: the stub's surface is a
  dynamic return carrying producer-selected evidence across the package
  boundary, pinned by
  `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`
  ("requires an exact retained loan origin for its shared receiver" stopped
  emitting after the retained-lineage/borrow-evidence family landed).
  Re-verified at `d6a0625f6b` (macOS arm64, mbx/nextest): all 21
  `cross_package_visibility` tests pass with zero loan-origin diagnostics;
  detail in `wiki/drafts/cross_package_dynamic_loan_origin.md`. No
  independent slice remains.
- **EFI-MATRIX-PROMOTION.** Mined candidate; scope verified, authorization
  gate recorded — re-mines the hosted-matrix clause of
  `wiki/drafts/rust_compiler_completion.md`: the required matrix is exactly
  the four hosted rows (linux x86-64, linux aarch64, macOS aarch64, Windows
  x86-64) and "freestanding EFI work remains a separately stated target
  milestone until it is promoted into this hosted matrix." Promotion is a
  deliberate matrix revision per the doc's own rule (a support-matrix
  change must revise the finite matrix in the same change, and the hosted
  gates already keep EFI out), and it additionally waits on the UEFI legs
  this board owns separately (UEFI-PHYSICAL-SEMANTIC-ENTRY,
  UEFI-OS-HANDOFF — the source-authored two-surface entry and the
  handoff that make an EFI host row possible at all). No implementable
  slice exists inside the current matrix fence; the promotion decision is
  a milestone statement, not a lane task. Re-verified at `7d03d489e3d9` (swarm-w9-ffival, linux x86-64): the hosted matrix in rust_compiler_completion.md is unchanged — four hosted rows, EFI still a separately stated milestone (:21) — and both named UEFI gates remain open rows (UEFI-PHYSICAL-SEMANTIC-ENTRY live-claimed by z88 to ~08:44Z; UEFI-OS-HANDOFF depends on it). Claim probe on TASKS.md exits 2 under broad board fencing. Promotion stays a milestone decision, not a lane task — nothing new to do here.
  covered — milestone decision in rust_compiler_completion.md, not a lane task; UEFI gates are their own rows
- **EXECUTABLE-PUBLICATION** — mined candidate; verify scope then implement.
- **EXECUTABLE-PUBLICATION-JOIN.** Mined candidate; scope verified, resolved —
  same surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (resolved on
  `origin/main`): the "join" is `CompileReport::publish_retained_native_artifact`
  (`compilation-report/src/compile_report.rs:271`) joining the validated
  retained artifact, manifest, and requested PCC pair into one staged tree that
  `executable_publication.rs` (`publish_exact_bytes`) commits by write-to-tmp,
  exact read-back replay, mode set, and atomic rename — a failed join leaves no
  half-written executable or stale sidecar. `omega/src/compilation/publication.rs`
  (`publish_compilation`/`publish_native_artifact`) is the product-owned route
  into it, `publish_completed_build_outputs` writes companions, and
  `output_kind` gating matches the spec's report/entry-bridge rule. Verified on
  ea025447fe. Sibling stubs on the same resolved surface: EXECUTABLE-PUBLICATION,
  EXECUTABLE-PUBLICATION-STAGE, EXECUTABLE-PUBLICATION-STEP,
  RETAINED-ARTIFACT-EXECUTABLE-PUBLICATION.
  covered — same landed surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (publish_retained_native_artifact)
- **EXECUTABLE-PUBLICATION-OPERATION** — mined candidate; scope verified, resolved — same surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (resolved on `origin/main`): `omega/src/compilation/publication.rs` (`publish_compilation`/`publish_native_artifact`) is the product-owned route calling `CompileReport::publish_retained_native_artifact`, which validates the retained artifact and manifest, refuses non-local output filenames, requires compiler-text/function validation evidence, self-checks a requested PCC pair pre-install, and commits one staged tree + atomic rename through `executable_publication.rs` — a failed publish leaves no half-written executable or stale sidecar. `output_kind` gating matches the spec's report/entry-bridge rule. Sibling stubs on the same resolved surface: EXECUTABLE-PUBLICATION, EXECUTABLE-PUBLICATION-JOIN, EXECUTABLE-PUBLICATION-STAGE, EXECUTABLE-PUBLICATION-STEP.
- **EXECUTABLE-PUBLICATION-STAGE.** Mined candidate; scope verified, resolved — same surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (resolved on `origin/main`): the "stage" is the staged tree + atomic rename committed by `CompileReport::publish_retained_native_artifact` through `executable_publication.rs` after validating the retained artifact and manifest, refusing non-local output filenames, requiring compiler-text/function validation evidence, and self-checking a requested PCC pair pre-install; a failed publish leaves no half-written executable or stale sidecar, and `omega/src/compilation/publication.rs` is the product-owned route into it. Re-verified at `dccdfd1fd11` — record stands; the sibling row's suite witness (`cargo nextest run -p compilation-report executable_publication` 15/15, `ea025447fe`) still covers this pin. Sibling stubs on the same resolved surface: EXECUTABLE-PUBLICATION, EXECUTABLE-PUBLICATION-JOIN, EXECUTABLE-PUBLICATION-STEP, RETAINED-ARTIFACT-EXECUTABLE-PUBLICATION.
  covered — same landed surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (staged tree + atomic rename)
- **EXECUTABLE-PUBLICATION-STEP** — mined candidate; scope verified, resolved — same surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (resolved on `origin/main`): the publication "step" is `publish_compilation`/`publish_native_artifact` in `omega/src/compilation/publication.rs`, which gates on `output_kind`, validates the retained artifact and manifest through `CompileReport::publish_retained_native_artifact`, requires compiler-text/function validation evidence, self-checks a requested PCC pair pre-install, then `publish_completed_build_outputs` commits one staged tree + atomic rename via `executable_publication.rs` — a failed publish leaves no half-written executable or stale sidecar. Verified at `ac4e4eee9b`. Sibling stubs on the same resolved surface: EXECUTABLE-PUBLICATION, EXECUTABLE-PUBLICATION-JOIN, RETAINED-ARTIFACT-EXECUTABLE-PUBLICATION.
  covered — same landed surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (publish_compilation route)
- **FAULT-INJECTED-TARGET-READER** — mined candidate; verify scope then implement.
  covered — Mach-O x86_64 pairing dispatch landed at 769627cc23; host run is MACOS-X64-HOST-PROFILE's
- **EXACT-PROGRAM-ENTRY-MULTIPLICITY.** Retired row — resolved; stub
  restores the pointer so the dangling owner references
  (`TASKS.md` ~:4052, ~:11232) resolve. The row was retired at
  `aceef55188` ("verified row, no remaining work"): entry multiplicity
  is enforced end to end — `admission/selection.rs` requires exactly
  one binding per required catalog slot, exactly one ProgramEntry-schema
  root, a non-generic machine with at most one provisioned `&mut self`,
  and `root_bindings.rs` rejects a re-bound slot or ambiguous
  implementation; integration coverage lives in
  `build_target_activation`, and the landed unit pins cover the
  previously unexercised edges (empty root bindings keep the migration
  fallback, unqualified slot spellings reject, malformed/unknown-profile
  foreign rows reject beside a valid selection, the binding walk
  collects every malformed row's diagnostic in order). What later waves
  attributed under this name — the "selected ProgramEntry establishment
  rejoins 0 Terminal attachment identities" family — is NOT this row's
  surface: it is owned by SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT (the
  `&[T]` view-local frontier; frontier now at the `&[T]`-consuming
  callee — `Slice::index`/`Slice::range` Terminal vocabulary), and the
  exact-symbol selection leg by PROGRAM-ENTRY-SELECTION-EXACTNESS
  (residual implemented on `zergling/z161-program-entry-selection-exactness`).
  No slice exists under this name at `fbf36233c9`.
  Re-verified at `fff3918dc42` (linux x86-64): the pointer row still resolves
  every citation — `admission/selection.rs` (build-evaluation) still owns
  `SelectedProgramEntry` plus the ProgramEntry-schema and
  exactly-one-binding checks, `admission/selection/root_bindings.rs` still
  carries the re-bound/ambiguous rejections, and the misattribution routing
  stands: the "rejoins 0 Terminal attachment identities" family is owned by
  SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT (:16808), the exact-symbol leg by
  PROGRAM-ENTRY-SELECTION-EXACTNESS (:14775). Retired verdict unchanged;
  nothing to implement under this name.
  Re-verified at `796814691e4`: anchors intact — the
  `build-evaluation/src/admission/selection.rs` exact-slot selection +
  `root_bindings.rs` re-bind/ambiguity rejections stand; the "0 Terminal
  attachment identities" attribution stays with
  SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT. No slice under this name.
  covered — retired at aceef55188; pointer-only stub, residuals owned by SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT / PROGRAM-ENTRY-SELECTION-EXACTNESS
- **COMPILER-EXECUTABLE-PUBLICATION-OPERATION.** — mined candidate;
  scope verified, resolved — the same surface the sibling
  EXECUTABLE-PUBLICATION-OPERATION row above already marks covered:
  `omega/src/compilation/publication.rs` is the product-owned route
  (`publish_compilation` → `CompileReport::publish_retained_native_
  artifact`, dispatched at `compilation/mod.rs:279`) — it validates
  the retained artifact + manifest, self-checks a requested PCC pair
  pre-install, and commits one staged tree + atomic rename via
  `executable_publication.rs`. Re-verified at `6f9a1f637e` (z181):
  anchors unchanged; no same-item claim live. Re-verified at
  `c924529921` (linux x86-64): `publication.rs` still owns
  `publish_compilation`/`publish_native_artifact` →
  `publish_retained_native_artifact` (dispatched at
  `compilation/mod.rs:279`); `executable_publication.rs` retains the
  stage/replay/rename discipline; `cargo nextest run -p
  compilation-report executable_publication` 15/15 and `compiler`
  `activation_identifiers_and_publication` 15/15 green.
  covered — publication.rs route + executable_publication.rs discipline landed; suites 15/15 green

- **EXECUTABLE-PUBLICATION-OPERATION.** — mined candidate; scope verified, resolved — same surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (resolved on `origin/main`): `omega/src/compilation/publication.rs` (`publish_compilation`/`publish_native_artifact`) is the product-owned route calling `CompileReport::publish_retained_native_artifact`, which validates the retained artifact and manifest, refuses non-local output filenames, requires compiler-text/function validation evidence, self-checks a requested PCC pair pre-install, and commits one staged tree + atomic rename through `executable_publication.rs` — a failed publish leaves no half-written executable or stale sidecar. `output_kind` gating matches the spec's report/entry-bridge rule. Sibling stubs on the same resolved surface: EXECUTABLE-PUBLICATION, EXECUTABLE-PUBLICATION-JOIN, EXECUTABLE-PUBLICATION-STAGE, EXECUTABLE-PUBLICATION-STEP.
- **EXECUTABLE-PUBLICATION-STAGE.** Mined candidate — resolved at
  `0a0662ad27` (linux x86-64): settled-name marker for the sibling stub
  the EXECUTABLE-PUBLICATION-OPERATION row names on the same resolved
  surface. Anchors re-verified live:
  `omega/src/compilation/publication.rs` still owns `publish_native_artifact` and the
  `publish_compilation` → `CompileReport::publish_retained_native_artifact`
  route; retained-artifact validation, PCC self-check, and the staged
  tree + atomic rename through `executable_publication.rs` are
  unchanged, with the prior green witness at `ea025447fe`
  (compilation-report `executable_publication` 15/15, compiler
  `activation_identifiers_and_publication` 15/15) standing. No leg
  remains under this name.
  covered — same landed surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (staged tree + atomic rename)
- **FAULT-INJECTED-TARGET-READER.** Mined candidate — resolved as
  covered: the surface this claim name has fenced is the image-emission
  installation-record pairing dispatch for x86_64 Mach-O images with
  thunk regions (TASKS.md MACOS-X64-HOST-PROFILE residual). That leg
  landed on main at `769627cc23` — `installation_record/
  record_validation.rs` dispatches Mach-O import thunk↔binding-slot
  pairing per `image.target().architecture`, and the x86_64 arm reaches
  the real validator
  `image_macho::validate_macho_x86_64_import_binding_pairing`
  (imports.rs, landed `5a5046d1db`), replaying each closed
  `jmp [rip+disp32]` thunk against exactly one placed binding slot
  naming the same symbol. Re-verified at `e7c0099cb2` (linux x86-64):
  `cargo nextest run -p image-macho` 31/31 PASS, including
  `x86_64_import_thunks_emit_and_validate_the_closed_jmp_rip_form`,
  `x86_64_mutated_import_thunk_rejects_final_validation`, and the
  end-to-end `x86_64_loader_mapping_accepts_eager_import_storage`
  pairing exercise — HEAD now builds clean (the mid-drift
  `crossed_window` break that forced the prior reading onto
  `3533f7d0e8` is resolved). The remaining x86_64-Mach-O residual — a
  real x86_64-apple-darwin host run — stays host-gated on
  Re-verified at `74c2bfc605` (z181): the Mach-O x86_64 pairing
  dispatch still reaches `image_macho::validate_macho_x86_64_import_
  binding_pairing` from `record_validation.rs` (dispatch on
  `image.target().architecture` intact); no same-item claim live;
  the x86_64-apple-darwin host run stays host-gated on
  MACOS-X64-HOST-PROFILE.
  covered — Mach-O x86_64 pairing dispatch landed at 769627cc23; host run is MACOS-X64-HOST-PROFILE's

  Re-verified again at `0a0662ad27` (z181): row unchanged; the Mach-O
  x86_64 pairing dispatch and validator remain landed, and no
  same-item claim is live.

  MACOS-X64-HOST-PROFILE's row. No unbound slice remains here.
- **NEW-TPV-DECLARATION-ORDER-NORMALIZATION-PIN.** Mined candidate; slice
  landed. The name resolves to the declaration-order normalization
  contract in `topology-plan`: `NormalizedGraph::new` sorts the supplied
  roster by name and resolves `EndpointKey` indices against that canonical
  order ("Input order does not matter — output is canonical"), and the
  emitted `DeploymentPlan` is canonical throughout — but no test pinned
  that producer declaration order never changes the plan. Added
  `declaration_order_normalizes_to_the_identical_plan` to
  `packages/topology/tests/composition.rs`: reversed instance and binding
  declaration orders compose to byte-identical plans and identical policy
  outcomes. `cargo nextest run -p topology-plan --test composition`:
  17/17 PASS on linux x86-64; `cargo fmt` clean. Worked unclaimed — no
  claimable marker existed for this name and the file carries no live
  fence.
- **NEW-C2LPSB-RANGE-FACTS-RECURSION-PROFILE.** Mined candidate; slice
  landed. The name resolves to the recursion profile of the range-facts
  dependency walks: three sites carried the same unnamed `128` bound —
  `record_dependencies` and `collect_reads` recursion guards plus the two
  chain-unrolling loops in `captures.rs` — and `reads.rs` already asked
  for it to be named once. The bound is now
  `dependencies::EXPRESSION_WALK_DEPTH_BOUND`, shared by all eight sites,
  and `tests/depth.rs` asserts against the constant itself so the pin and
  the profile cannot drift. `cargo nextest run -p
  typed-trees-to-checked-trees --lib -E 'test(/depth/) | test(/dependencies/)
  | test(/captures/) | test(/reads/)'`: 197/197 PASS on linux x86-64;
  `cargo fmt` clean. Worked unclaimed — no claimable marker existed for
  this name and `checks/ranges/facts/dependencies*` carries no live fence.
- **GENERAL-SOURCE-BINDER-SYNTAX.** Resolved — scope verified: the general mathematical binder surface (`let`/`boundary let` telescopes, `core::Level`/`core::Type<u>`/`core::Strict<v>`/`core::Squash` carriers, generalized and authored universe binders, arrow-typed telescope parameters, named assumptions) already landed under the PROOF-CONTRACT-MIGRATION structural legs; the in-fence residual was the bounded machine-valued body denotation in `typed-trees-to-checked-trees/src/proof`. Extended it: `x != y` now denotes `Squash (Not (Id S l r))` through an interned `Not : Π(_ : Type 0). Type 0` assumption — kept at `Type 0`, not `sEmpty` elimination, so inequality composes inside `&&`/`||` like `==` — and `()` interned a dedicated `Unit : Type 0` carrier, so unit binder domains and unit-carried calls denote instead of refusing. Remaining named legs stay with their owners: `core::*` symbol-identity classification (blocked on the fixed `core::*` declarations landing in `source/library/core`), checked-signature encoding into Terminal evidence, member-call `target_symbol` binding inside `let` bodies, and order relations over non-integer operands. Gate on linux x86-64: `cargo check`/`clippy -p typed-trees-to-checked-trees` clean of new warnings; `cargo nextest run -p typed-trees-to-checked-trees` 5008/5009 — `open_range_token_use_rejects_instead_of_falling_back` fails verbatim at base `d82697ffca` (unrelated wave breakage). Re-verified at `8734480a01`: the filtered binder/signature/denotation suite passes 128/128 and `open_range_token_use_rejects_instead_of_falling_back` is green again — the unrelated failure has since been repaired.
- **GENERATED-CODEC-INDEPENDENT-VERIFICATION.** Give generated wire codecs a
  route to `Derived` trust that does not depend on an authored grammar
  policy. [codecs](wiki/spec/layouts/codecs.md) realization table (`:17`)
  grants `Derived` for an "Authored or generated body independently checked
  against the public requirement", and its Agreement and trust section
  (`:22-24`) still records "Independent generated-codec verification and
  preserving-codec realizations remain implementation work".

  Today a generated codec with no author-supplied policy is classified
  `WireTrustClass::Admitted { authority: "Omega compiler" }`, carrying the
  evidence line "generated body is not yet independently checked against the
  public codec requirement"
  (`omega-rust/omega/build/build-evaluation/src/admission/wire_protocol.rs:161-186`).
  The only existing route to `Derived` is opt-in and single-generator: an
  authored `CompactBinary::plan` grammar policy evaluated against the schema
  walk sets `policy_verified`
  (`omega-rust/psi/semantics/build-time-evaluation/src/layouts/wire_plans.rs:155-176`,
  landed `a77deb22d6`). So the compiler trusts its own generator by
  assertion wherever no author wrote a policy.

  Acceptance: a synthesized codec with no authored policy reports
  `WireTrustClass::Derived` through a compiler-side independent check — the
  existing pin `synthesized_codec_stays_admitted_without_an_authored_grammar_policy`
  (`wire_protocol.rs:1480`) inverts, with
  `policy_verified_generated_codec_reports_derived_trust` (`:1507`) still
  green. Preserving-codec realizations are named in the same spec sentence
  but are not this row. Re-verified at `fff3918dc42` (z203 leg): the
  item's whole decisive surface — the `trust_class` flip at
  `wire_protocol.rs:167-186` and both pinned tests — sits inside
  `admission/`, dir-fenced live to BUILD-EXCLUSION-REALIZATION
  (~15:52Z); `WireTrustClass::Derived` has no producer anywhere else in
  the tree. Scoped shape for the post-fence implementer: the admission
  layer already reads the schema's PUBLIC field table
  (`typed.wire_schemas()` / `wire_members` give each field's number +
  `FieldShape`), so the independent check re-derives expected
  placements there (`is_varint` → `Varint{tag}` else
  `LengthPrefixed{tag}`, tag-ordered) from the requirement alone and
  rejects on disagreement with the recorded plan — no authored policy
  needed. The check's independence is structural: it consumes only
  recorded public facts in the consumer crate, not the generator's
  internal walk in `build-time-evaluation/layouts/wire_plans.rs`.
  Fence caveat: a dangling helper under a still-Admitted flip would be
  dead machinery, so the check and the pin inversion land together
  once `admission/` opens. No unfenced slice exists.
- **GENERIC-RETURNED-VIEW-LIFETIMES.** Mined candidate — scope verified,
  owner row; residuals fenced or spec-gated. This is the named owner of
  the view-lifetime correspondence surface cited by resolved sibling
  LIFETIME-SOURCE-CORRESPONDENCE (`8ccd793fa8`): `borrow/view_link.rs`
  ("Lifetimes stage 2") resolves an explicit result lifetime to exactly
  one input parameter and its complete matching structural leaves —
  reusing one lifetime across multiple inputs rejects today, and that
  rejection is the deliberate recorded boundary, not a gap. Residuals:
  (1) multi-source leg — every parameter carrying the selected lifetime
  contributes its leaves as possible sources, each supporting the
  returned access — is implementation work on the fenced
  `checks/borrows/` surface (live claim BORROW-PROOF-CONVERGENCE,
  ~06:46Z); (2) outlives leg — general authored outlives bounds have no
  surface: lifetimes.md spells binders only and conformances.md states
  whole-conformance applications do not gain outlives/variance/
  subtyping, so it waits on a spec decision, not a checker gap. No
  unclaimed slice exists this wave; sibling LIFETIME-MULTI-SOURCE-AND-
  OUTLIVES mines the same residual pair. Re-verified at `61eea9d820`
  (linux x86-64, 2026-09-21 ~07:10Z): residual (1) has since LANDED —
  `resolve_signature_view_return_source` routes multi-input same-lifetime
  signatures through `structural_view_return_source`, which unions every
  matching leaf as candidate sources, and the landed tests pass
  (`direct_result_links_the_union_of_inputs_sharing_the_result_lifetime`,
  `direct_result_union_tracks_the_loan_on_every_candidate_source`,
  `carrier_result_same_lifetime_leaves_are_unioned_within_one_input` —
  135/135 filtered green); the recorded BORROW-PROOF-CONVERGENCE fence
  expired and `src/borrow/` is currently unclaimed. Residual (2) is still
  spec-gated (lifetimes.md:33, conformances.md:66). The only open slice
  under this name remains the outlives spec decision.
- **GEOMETRY-ALIGNMENT-REGIONS.** Mined candidate (split-of:
  [samples/apps/squalr/TASKS.md](samples/apps/squalr/TASKS.md) GEOMETRY-PARITY
  "region alignment/expansion" parity gap). Resolved — the gap is already
  ported at Squalr-Omega `52bcf254c983a9ae9bf6e0c3661cafd656ca056b` (on that
  repo's `main`): `squalr-engine-api`'s `NormalizedRegion` gained
  `set_alignment` (forward-distance add to the next multiple, wrapped add at
  the address edge, end address retained — matching the upstream
  early-return/no-op cases) and `expand` (saturating base subtract and size
  add around the released doubling multiply), and `squalr-tests` exercises
  `expand` natively for growth plus low/high saturation. The residual is not
  geometry work: `set_alignment` is source-checked but not exercised because
  a `&mut self` machine taking a data parameter loses the entry attachment
  identity (selected ProgramEntry establishment rejoins 0 Terminal
  attachment identities) — a compiler entry-mechanics gap the app board now
  tracks as "the set_alignment call-site gate" under GEOMETRY-PARITY.
  Re-verified at `f7212dc016f2`: the ported `set_alignment`/`expand`
  pair stands on the pinned submodule revision; the residual stays the
  compiler entry-mechanics gate under GEOMETRY-PARITY, and the squalr
  submodule dir sits under SQUALR-WINDOWS-GEOMETRY-VALIDATION (dev-88738).
  Sibling stubs on the same parity-gaps sentence:
  GEOMETRY-ALIGNMENT-PARSING, GEOMETRY-ALIGNMENT-STRING-PARSING (the
  "alignment string parsing" gap), GEOMETRY-CLONE-SERIALIZATION,
  GEOMETRY-DEBUG-ASSERTIONS, SQUALR-NAMED-TRAIT-OPERATORS.
- **GEOMETRY-ALIGNMENT-STRING-PARSING.** Scope verified at `2a07fef5a85`
  — re-mines the "alignment string parsing" parity gap on
  GEOMETRY-PARITY's sentence (:7790): a `&mut self` machine taking a
  data parameter loses the entry attachment identity (selected
  ProgramEntry establishment rejoins 0 Terminal attachment identities).
  That residual is a compiler entry-mechanics item tracked on the app
  board under GEOMETRY-PARITY ("the set_alignment call-site gate"), not
  a board item here. Its only implementing surface is the
  `samples/apps/squalr` submodule, fenced under sibling claims
  (SQUALR-TARGETS-AND-THROUGHPUT, SQUALR-CLONE-SERIALIZATION,
  SQUALR-REGION-ALIGNMENT-EXPANSION per :6535). No independent slice
  exists. Sibling stubs on the same sentence: GEOMETRY-ALIGNMENT-PARSING,
  GEOMETRY-CLONE-SERIALIZATION, GEOMETRY-DEBUG-ASSERTIONS,
  SQUALR-NAMED-TRAIT-OPERATORS, SQUALR-ALIGNMENT-STRING-PARSING.
- **GEOMETRY-ALIGNMENT-STRING-PARSING.** Mined candidate — scope
  verified, covered. Bare re-mine of SQUALR-ALIGNMENT-STRING-PARSING:
  the "alignment string parsing" parity gap inside the Squalr app's
  geometry lane (samples/apps/squalr/TASKS.md's GEOMETRY-PARITY residual
  list; the parity audit attributes it to that row). Verified sibling
  measured the gap at `949c153acd73`: `memory_alignment.omg` builds an
  alignment only from an integer — zero `parse`/`from_str`/
  `from_string` over any alignment type — so the residue is one parse
  machine plus its rejection cases, not a structural port. Its only
  implementing surface is the `samples/apps/squalr` submodule, held
  under live sibling claims (SQUALR-WINDOWS-GEOMETRY-VALIDATION ~05:49Z,
  GEOMETRY-ALIGNMENT-REGIONS ~01:18Z over `TASKS.md,samples/apps/
  squalr`; SQUALR-GEOMETRY-PARITY-RESIDUE ~03:32Z); the residual
  `set_alignment` call-site gate is a compiler entry-mechanics item
  tracked under GEOMETRY-PARITY, not this row. Sibling re-mine stubs on
  the same surface: ALIGNMENT-STRING-PARSING (covered), SQUALR-SEED-
  ALIGNMENT-PARSING (covered), GEOMETRY-ALIGNMENT-PARSING,
  SQUALR-ALIGNMENT-STRING-PARSING.
- **GEOMETRY-CLONE-SERIALIZATION.** Mined candidate — scope verified,
  covered: re-mines the "clone/serialization" clause of
  SQUALR-GEOMETRY-PARITY's residual list, whose owner row is
  SQUALR-CLONE-SERIALIZATION-PARITY. The residual is consumed at the
  tracked pin `ef6682f75f4` — merge `db64d58` absorbed `5ea4a17`
  ("clone/serialization parity landed in squalr-engine-api"), so the
  structures modules now carry authored Clone/Serialize parity:
  `normalized_region.omg` (`clone`/`clone_from` + wire-codec pair,
  :26-99), `snapshot_region_filter.omg` (`clone`/`clone_from`, :18-23),
  `memory_alignment.omg`. Verification of the owning row's parity
  acceptance stays in the submodule lane (samples/apps/squalr remains
  dir-fenced); no independent slice exists under this name.
- **GEOMETRY-DEBUG-ASSERTIONS.** Scope verified at `069276b986dc` —
  re-mines the "debug-only assertions" parity gap on GEOMETRY-PARITY's
  sentence; named verbatim as a sibling on
  GEOMETRY-ALIGNMENT-STRING-PARSING's resolved row (:7928). The gap maps
  to the SQUALR-DEBUG-ASSERTION-PARITY / SQUALR-DEBUG-ASSERTIONS lane
  (Rust `debug_assert`/`debug_assert_eq` assertions in the geometry
  engine) — an edit inside `samples/apps/squalr`, wholesale-fenced
  under SQUALR-WINDOWS-GEOMETRY-VALIDATION (dev-88738, exp 05:49Z),
  and the identical sibling stub SQUALR-GEOMETRY-DEBUG-ASSERTIONS is
  under a live item claim (zergling-176, exp 11:43Z). No independent
  slice exists here; sibling stubs on the same parity-gaps sentence:
  GEOMETRY-ALIGNMENT-PARSING, GEOMETRY-CLONE-SERIALIZATION,
  SQUALR-NAMED-TRAIT-OPERATORS.
  Re-verified at `32a6a7fa33` (linux x86-64): adjudication unchanged —
  the surface is still an edit inside the `samples/apps/squalr`
  submodule (pin now `ef6682f7`, rotated from the `5b0307c` recorded
  earlier this wave). The recorded fences drained and re-formed:
  SQUALR-DEBUG-ASSERTIONS (z35) now holds the `samples/apps/squalr`
  dir-fence and SQUALR-DEBUG-ASSERTION-PARITY an item claim (both
  ~16:2x-16:45Z, elapsed at this check), with SQUALR-SEED-PARITY on the
  seed draft (~15:14Z); GEOMETRY-PARITY coordination still applies.
  Still no independent slice under this name.
  Re-verified at `32a6a7fa33` (linux x86-64): adjudication unchanged —
  the surface is still an edit inside the `samples/apps/squalr`
  submodule (pin now `ef6682f7`, rotated from the `5b0307c` recorded
  earlier this wave). The recorded fences drained and re-formed:
  SQUALR-DEBUG-ASSERTIONS (z35) now holds the `samples/apps/squalr`
  dir-fence and SQUALR-DEBUG-ASSERTION-PARITY an item claim (both
  ~16:2x-16:45Z, elapsed at this check), with SQUALR-SEED-PARITY on the
  seed draft (~15:14Z); GEOMETRY-PARITY coordination still applies.
  Still no independent slice under this name.
- **GEOMETRY-EVIDENCE-REFRESH.** Scope verified — no independent slice.
  The name re-mines the evidence-retention clause of the squalr
  application acceptance (:143): "retain results under the app's ignored
  `build/verification/`, with exact app/compiler pins and host."
  Refreshing that evidence is a byproduct of a native `verify.py` run on
  a matching host — the linux x86-64 witness is already recorded on
  GEOMETRY-NATIVE (:7839, d82697ffca, `Squalr geometry: PASS`), and the
  Windows leg is GEOMETRY-WINDOWS-VALIDATION's host-bound row. The
  submodule path is wholesale-fenced at verification time
  (`samples/apps/squalr` dir-claimed by SQUALR-WINDOWS-GEOMETRY-VALIDATION,
  dev-88738). Nothing executable exists on this host under this name.
- **GEOMETRY-NATIVE.** Mined candidate (split-of:SQUALR-HEADLESS leg 1 /
  app-board GEOMETRY-PARITY, source:
  [samples/apps/squalr/TASKS.md](samples/apps/squalr/TASKS.md)). Run the
  geometry app natively on hosts beyond the verified macOS ARM64 evidence:
  `python tools/verify.py native --timeout 600 --omega <executable>` inside
  the pinned `samples/apps/squalr` (4b1f7a6) — `omega run --keep
  squalr-tests/main.omg`, exit 0 with `Squalr geometry: PASS`.
  **Linux x86-64 PASS (w9 z57 witness, d82697ffca):** release-profile
  `omega`, Python 3.10 + `tomli` shim, `RUST_MIN_STACK=67108864` —
  `Squalr geometry: PASS`, exit 0, 173.2s end-to-end (vs 167.2s macOS
  ARM64). A debug-profile `omega` does NOT work for this leg: the
  per-checkout `omega update` evaluation took ~62 min and the compile
  leg exceeded the 30-min timeout; the release build compiles+runs in
  ~3 min. Windows x86-64 and Linux ARM64 remain unrecorded.
  Fresh-checkout ceremony (required before any run): the tracked
  `squalr-tests/omega.lock` is bound to the committing checkout's
  canonical path (`ExternalLocal` lineage) and rejects other checkouts
  with "fresh source key or immutable content differs" — copy the app
  tree to scratch (its fenced path is wholesale-claimed by
  SQUALR-TARGETS-AND-THROUGHPUT), remove the stale lock, `omega update
  --project squalr-tests` → accept the six pending decisions in
  `build/package-manager/review-<target>.txt` → `omega update --resume`.
  `verify.py` requires Python >=3.11 (`tomllib`); on 3.10 hosts run with
  a `tomli`-backed `tomllib` shim on `PYTHONPATH`.
- **GEOMETRY-PARITY.** Mined candidate — resolved: the bare name is the
  app board's own canonical row, `samples/apps/squalr/TASKS.md`
  **GEOMETRY-PARITY** at gitlink `5b0307c35` — "Validate the geometry
  application on Windows … then finish the mapped Rust behavior still
  absent from the seed." Its state is unchanged and it has no Omega-side
  slice: Omega + std `87d8b227` pass all 12 authored geometry checks on
  macOS ARM64 (`verify.py native` → `Squalr geometry: PASS`), "Windows
  was not run" — the acceptance leg is Windows-host-gated — and the
  residual parity list (Rust debug-only assertions, alignment string
  parsing, the set_alignment call-site gate, named trait operators) is
  each already a sibling row: GEOMETRY-WINDOWS-LEG /
  GEOMETRY-WINDOWS-VALIDATION / GEOMETRY-WINDOWS-REVALIDATION carry the
  Windows half, SQUALR-ALIGNMENT-STRING-PARSING /
  SQUALR-NAMED-TRAIT-OPERATORS / SQUALR-GEOMETRY-PARITY the gap clauses,
  SQUALR-CLONE-SERIALIZATION-PARITY the clone/serialization clause, and
  the port surface `samples/apps/squalr` is wholesale-fenced this wave
  by REGION-ALIGNMENT-EXPANSION (~07:25Z) with GEOMETRY-WINDOWS-VALIDATION
  item-live (~13:53Z). No unfenced slice exists under this name;
  coordinate on the submodule board.
- **GEOMETRY-WINDOWS-LEG.** Mined candidate — resolved at `9beef2b045`:
  re-mines the same Windows leg of the app repo's GEOMETRY-PARITY acceptance
  that sibling row GEOMETRY-WINDOWS-VALIDATION (~this file, line 7842)
  records: the 12-check geometry evidence is macOS ARM64 only ("Windows was
  not run") and the acceptance is `python tools/verify.py native` on a
  Windows host. Blocked identically two ways — no Windows development host
  exists in this environment, and `samples/apps/squalr` is wholesale-fenced
  this wave (SQUALR-TARGETS-AND-THROUGHPUT exp 21:39Z,
  GEOMETRY-ALIGNMENT-REGIONS exp 01:18Z). A Linux-side
  `--target windows_x86_64` emit leg does not satisfy the run-based
  acceptance and still needs the fenced tree. Owning parent:
  SQUALR-GEOMETRY-PARITY; sibling re-mine stub GEOMETRY-WINDOWS-REVALIDATION
  names the same leg. Re-verified at `6f918986063` (z203 leg): still
  doubly gated — no Windows development host exists in this environment,
  and the `samples/apps/squalr` dir fence has rotated to
  SQUALR-DEBUG-ASSERTIONS (~16:25Z) while the
  GEOMETRY-WINDOWS-VALIDATION item claim stays live (~13:53Z); the
  previously cited wholesale fences (SQUALR-TARGETS-AND-THROUGHPUT,
  GEOMETRY-ALIGNMENT-REGIONS) have drained. Submodule pin remains
  `5b0307c352`.
  Re-verified at `bbffdafe0498` (z116): still doubly gated — no Windows
  development host in this environment, `samples/apps/squalr` remains
  dir-fenced (SQUALR-DEBUG-ASSERTIONS ~16:25Z) with the
  GEOMETRY-WINDOWS-VALIDATION item claim still live (~13:53Z). Submodule
  pin has moved to `ef6682f75f`.
- **GEOMETRY-WINDOWS-REVALIDATION.** Mined candidate — scope verified at
  `94e764a6da`, re-mine of the settled adjacent row
  GEOMETRY-WINDOWS-VALIDATION (scope verified `8734480a01`): both names
  land on the Windows leg of the squalr app's GEOMETRY-PARITY acceptance,
  `python tools/verify.py native --timeout 600` on a Windows host against
  the pinned submodule (gitlink now `5b0307c352`, moved since the sibling
  audit). Recorded evidence covers macOS ARM64 + Linux x86-64; the Windows
  leg stays "was not run". Doubly gated: no Windows development host in
  this lane, and `samples/apps/squalr` is dir-fenced by
  SQUALR-WINDOWS-GEOMETRY-VALIDATION (dev-88738); a Linux
  `--target windows_x86_64` emit leg does not satisfy the run-based
  acceptance. Sibling re-mines of this surface on the board:
  GEOMETRY-WINDOWS-LEG (resolved `9beef2b045`), this stub, and
  SQUALR-WINDOWS-GEOMETRY-VALIDATION (re-verified `7110606f46`,
  ~line 10643) which also records the moved submodule pin.
  No linux_x86_64 slice exists.
- **GEOMETRY-WINDOWS-VALIDATION.** Mined candidate; scope verified at
  `8734480a01`: names the Windows leg of the app repo's GEOMETRY-PARITY
  acceptance ("Validate the geometry application on Windows"; the 12-check
  geometry evidence to date is macOS ARM64 only — "Windows was not run").
  Acceptance is `python tools/verify.py native --timeout 600 --omega
  <executable>` on a Windows host against the nested build.omg graph.
  Blocked two ways: no Windows development host exists in this
  environment, and every touch point in `samples/apps/squalr` sits under
  live claims (dir-fenced by Jarod's SQUALR-TARGETS-AND-THROUGHPUT;
  file-fences from Zergling-61's SQUALR-CLONE-SERIALIZATION). A
  Linux-side emit leg under `--target windows_x86_64` would not satisfy
  the run-based acceptance and still needs the fenced app tree. Owning
  parent: SQUALR-GEOMETRY-PARITY (TASKS.md:~6020); coordinate there on a
  Windows host before working it.
  Re-verified at `577d6ac2ba3` (Zergling-52): pin is now `5b0307c352`;
  fence map refreshed — `samples/apps/squalr` is dir-claimed by
  SQUALR-WINDOWS-GEOMETRY-VALIDATION (Devin / dev-88738, 05:49Z) and
  TASKS.md sits under sibling geometry claims (GEOMETRY-ALIGNMENT-REGIONS
  01:18Z, z175 06:28Z). The leg remains a Windows-host run; nothing
  producible on linux x86-64.
- **GLOB-SELF-IMPORTS-REPAIR** — mined candidate; scope verified, slice landed. The name resolves to `tests/architecture/glob_self_imports.rs`: a per-crate ratchet over files carrying `use super::*;`/`use crate::*;`, whose ceiling table is already empty — so every surviving glob self-import fails `glob_self_imports_never_grow_per_crate`. Repair means removing the glob, not raising ceilings. Residual at `33eb8d92ff`: twelve files across eleven crates (new glob files keep landing, so the residual regrows while the ratchet is red). This slice converted four files to explicit `use super::{names}`/`use super::Name` imports — `component-description`'s `component_description/tests.rs`, `omega`'s `execution/mod.rs`, `machine-emission`'s `startup_trampoline.rs`, and `selected-instructions-to-selected-instructions`' `address_fold/tests.rs` `independence_tests` module — plus corrected the gate's stale "more than two thousand" preamble (crate suites green: 23/23, 4/4, 50/50, 41/41 filtered). Remaining files sit under sibling claims: BUILD-PACKAGES-GATE holds `sources/acquisition` traversal.rs, MACOS-X64-HOST-PROFILE/validators-leg holds image-emission `final_image_validation.rs`, MATCH-SELECTIVE-LOWERING holds validation `result_type.rs`, PROOF-RULE-CLASSICALITY-AUDIT holds proof-admission `classicality.rs`, and RC-REPOSITORY-CLOSURE glob legs hold optimization-unit-semantics `replay.rs`, checked-trees-to-lowered-psi `operation_crash_contracts.rs`, proof `measurement.rs`, and both terminal-verifier files. The gate stays red until those legs land; the ratchet then guards zero. Second slice (z103): the residual regrew to 24 files while red; this pass converted the eleven unfenced survivors — `target`'s `elf_loader`, `foreign_locator`, `target_semantics`, `uefi_loaded_image/{mod,occurrence}`, `x86_features`, `image-emission`'s `final_image_validation`, `build-evaluation`'s `evidence/filesystem_scope/preparation`, and the three `selected-instructions-to-selected-instructions` `*_relocation/tests.rs` `independence_tests` modules (the uefi_boot_services/uefi_system_table quartet was repaired by its claim owner in the interim). Nested `mod tests` globs needed the parent file's own `use` bindings listed explicitly (`use super::{TargetProfile}` / `use super::{Field, LayoutPlacementReport}`); rustc E0432/E0425 drive convergence. Four-crate lib suites 1896/1896 green. Residual at this commit: nine files, all sibling-fenced (BUILD-PACKAGES-GATE, RC-REPOSITORY-CLOSURE glob legs 1-2, RUNTIME-SIZED-ACTIVATION-STORAGE, PROOF-RULE-CLASSICALITY-AUDIT, MATCH-SELECTIVE-LOWERING); the ratchet stays red until they land.
- **HOSTED-INLINE-ASSEMBLY-AUTHORITY** — mined candidate; scope verified, authority question already settled. The catalog in `psi/foundation/language-core/src/inline_assembly/` carries `required_authority` per instruction (`MachineOwner`, `PortIoAuthority`, `IdtControlAuthority`, `None`), per the privileged-services contract in `wiki/spec/build/permissions.md` (separate `MachineControl`/`PortIo`/`Mmio` service identities — listing the service does not establish ownership). The hosted-side authority decision is the implemented v0 discharge: `validation/src/machine_calls/effects/asm_discharge.rs::validate_asm_discharge` rejects every non-`None`-authority asm instruction on non-freestanding builds ("only code that owns the machine may emit privileged instructions; a hosted build would fault at ring 3") and passes freestanding — so hosted inline-assembly authority is denied by contract, not unimplemented. A finer hosted grant is a permissions.md spec change, not a compiler slice on this row.
- **INDEXED-OPERAND-ATTACHED-RECEIVER** — mined candidate; scope verified, covered — same indexing-through-attached-receiver surface as the resolved sibling INDEXING-ATTACHED-RECEIVER-BORROW, which names this stub (verified `669925b8b9`, linux x86-64): `tests/multiplicity/borrowed_case_payloads.rs` exercises `self.kinds[slot]` transitions under `&self`/`&mut self` custody, loan lifetime across successors, and index-argument consumption; `tests/multiplicity/borrowed_observations.rs` pins reborrows into the attached receiver and rejects a borrowed indexed collection moving into an owned receiver; affine extraction still rejects; the indexed operand route through the receiver_self_match loan is additionally pinned by BASELINE-T2C-INDEXED-OPERAND-ACCESS's landing (`7ec7ee32e8`, 8/8 `borrowed_observations` green at `d05ec39a5d`). No independent slice exists here.
- **INDEXING-ATTACHED-RECEIVER-BORROW.** Mined candidate — resolved,
  covered on `origin/main` (verified `669925b8b9`, linux x86-64). The
  indexing-through-borrowed-attached-receiver surface is implemented and
  pinned: `tests/multiplicity/borrowed_case_payloads.rs` exercises
  `self.kinds[slot]` transitions under `&self`/`&mut self` custody
  (borrowed_indexed_affine_case_observation_preserves_array), loan
  lifetime across successors, and consumption of index arguments;
  `tests/multiplicity/borrowed_observations.rs` pins reborrows into the
  attached receiver and rejects a borrowed indexed collection moving
  into an owned receiver; affine extraction still rejects. Scoped run:
  9/9 `borrowed_indexed`/`indexed_case`/`indexed_observation` tests
  PASS. Sibling stub on the same surface:
  INDEXED-OPERAND-ATTACHED-RECEIVER.
- **INDUCTIVE-CARRIER-CERTIFICATE.** Resolved — re-mines the internal
  inductive-carrier certificate cited as already-landed on
  PROOF-INTERCHANGE-IMPORT (:6236): `admission/recursion.rs`
  `verify_recursive_component` /
  `verify_recursive_component_with_machine_parameters`
  (proof-admission:55-95+) validate the component shape, require and
  match the ranking relation, verify the well-foundedness obligation
  through `verify_recursive_obligation`, and check each member's
  certificate; loop-local values never acquire parameter-only proof
  rules. The W-induction certificate is additionally covered end to end
  by `terminal-codec/tests/mathematical_certificate.rs`. The external
  matching-logic induction certificate has no importer — that route runs
  through MATCHING-LOGIC-BOUNDED-SLICE + a concrete design per
  PROOF-INTERCHANGE-IMPORT, not here. No independent slice exists.
- **INLINE-ASSEMBLY-CATALOG-EXPANSION** — mined candidate; verify scope then implement.
- **INTEGER-COMPARISON-OCCURRENCE-PRODUCER-COVERAGE.** Mined candidate —
  resolved, covered. Member of the INTEGER-COMPARISON-OCCURRENCE-*
  re-mine family under BENCHMARK-COMPILE-UNBLOCK-COMPARISON-OCCURRENCES.
  The producer-coverage leg is what the landed pin checks: the
  `terminal_product::integer_comparisons` gate (repaired `76dc49a99e`)
  counts selected integer occurrences against the artifact-bound
  checked scope, and `compiler`'s `integer_comparison_publication`
  suite
  (`selected_comparison_publication_preserves_complete_custody_among_builtins`,
  PASS on linux x86-64 at `f2f39039da`) requires complete custody
  coverage of the producer's selected occurrences among builtins —
  provider coverage for genuinely selected occurrences is the
  residual that pin already exercises. Siblings:
  INTEGER-COMPARISON-OCCURRENCE-{PRODUCER,RETENTION,STD-COVERAGE},
  COMPARISON-OCCURRENCE-PRODUCER-COVERAGE,
  BENCHMARK-STD-COMPARISON-OCCURRENCE-GATE.
- **INTEGER-COMPARISON-OCCURRENCE-STD-COVERAGE.** Resolved — member of the
  INTEGER-COMPARISON-OCCURRENCE-* re-mine family named under
  BENCHMARK-COMPILE-UNBLOCK-COMPARISON-OCCURRENCES. The "std coverage" leg
  it names is covered by composition: the integer-comparison occurrence
  gate (`compilation-report`'s `terminal_product::integer_comparisons`,
  repaired at `76dc49a99e`) runs on every published artifact — all
  std-linked pass canaries and the benchmark subjects exercise it, and
  `wrapping_square_sum` compiled + published through it on
  windows_x86_64 / macos_arm64 / linux_arm64 at `f2f39039da`. The
  dedicated pin is `compiler`'s `integer_comparison_publication` suite
  (`selected_comparison_publication_preserves_complete_custody_among_builtins`,
  witnessed PASS on linux x86-64), which requires complete custody coverage
  for selected occurrences among builtins; provider coverage for genuinely
  selected occurrences is the surviving residual and is what that pin
  already checks. A separate std-wide sweep would duplicate the canary
  corpus's own compile coverage. Siblings: INTEGER-COMPARISON-OCCURRENCE-
  {PRODUCER,PRODUCER-COVERAGE,RETENTION}, COMPARISON-OCCURRENCE-PRODUCER-
  COVERAGE, BENCHMARK-STD-COMPARISON-OCCURRENCE-GATE.
- **INTEL-MACOS-HOST-PROFILE** — mined candidate; verify scope then implement.
  Verified scope: named alias of **MACOS-X64-HOST-PROFILE** (TASKS.md:5962)
  — "Intel gap" is that row's own parenthetical. `TargetProfile::MacosX64`
  already catalogues the host through checked admission (which refuses on
  the missing `targets/macos_x86_64` provider package); the enumerated legs
  are the `ProgramEntryPhysicalContractPackage::MacosX64` entry contract +
  `targets/macos_x86_64/` source library, the x86-64 Mach-O writer
  (`image_output.rs` refuses `(MachO, X86_64)`), the
  `native_hosted_target()` cfg arm in `canary_suite.rs`, and a real
  x86_64-apple-darwin host run. Every implementing surface is under that
  item's live claims this wave (image-macho/image_output.rs +
  macos_x86_64 sources at 00:10Z+1d; final_image_validation.rs +
  installed_artifact.rs at 00:12Z+1d; native_evidence.rs at 00:24Z+1d —
  all Devin / swarm-w9-macos-x64-host-profile).
- **INTERNAL-PASS-PROFILE-TIMINGS** — mined candidate; verify scope then implement.
  Windows host before working it. Re-verified at `3a5db13578`
  (linux x86-64, 2026-09-21 ~05:56Z): both blocks still hold — this lane
  has no Windows host and `samples/apps/squalr` is again dir-fenced
  (REGION-ALIGNMENT-EXPANSION / zergling-z68, exp 07:25Z; the earlier
  SQUALR-TARGETS-AND-THROUGHPUT and SQUALR-CLONE-SERIALIZATION fences
  rotated out but the surface stays claimed), and sibling claim
  SQUALR-WINDOWS-GEOMETRY-VALIDATION (z175) covers this alias on
  TASKS.md until 06:28Z.
- **GLOB-SELF-IMPORTS-REPAIR.** — mined candidate; scope verified, slice landed. The name resolves to `tests/architecture/glob_self_imports.rs`: a per-crate ratchet over files carrying `use super::*;`/`use crate::*;`, whose ceiling table is already empty — so every surviving glob self-import fails `glob_self_imports_never_grow_per_crate`. Repair means removing the glob, not raising ceilings. Residual at `33eb8d92ff`: twelve files across eleven crates (new glob files keep landing, so the residual regrows while the ratchet is red). This slice converted four files to explicit `use super::{names}`/`use super::Name` imports — `component-description`'s `component_description/tests.rs`, `omega`'s `execution/mod.rs`, `machine-emission`'s `startup_trampoline.rs`, and `selected-instructions-to-selected-instructions`' `address_fold/tests.rs` `independence_tests` module — plus corrected the gate's stale "more than two thousand" preamble (crate suites green: 23/23, 4/4, 50/50, 41/41 filtered). Remaining files sit under sibling claims: BUILD-PACKAGES-GATE holds `sources/acquisition` traversal.rs, MACOS-X64-HOST-PROFILE/validators-leg holds image-emission `final_image_validation.rs`, MATCH-SELECTIVE-LOWERING holds validation `result_type.rs`, PROOF-RULE-CLASSICALITY-AUDIT holds proof-admission `classicality.rs`, and RC-REPOSITORY-CLOSURE glob legs hold optimization-unit-semantics `replay.rs`, checked-trees-to-lowered-psi `operation_crash_contracts.rs`, proof `measurement.rs`, and both terminal-verifier files. The gate stays red until those legs land; the ratchet then guards zero. Second slice (z103): the residual regrew to 24 files while red; this pass converted the eleven unfenced survivors — `target`'s `elf_loader`, `foreign_locator`, `target_semantics`, `uefi_loaded_image/{mod,occurrence}`, `x86_features`, `image-emission`'s `final_image_validation`, `build-evaluation`'s `evidence/filesystem_scope/preparation`, and the three `selected-instructions-to-selected-instructions` `*_relocation/tests.rs` `independence_tests` modules (the uefi_boot_services/uefi_system_table quartet was repaired by its claim owner in the interim). Nested `mod tests` globs needed the parent file's own `use` bindings listed explicitly (`use super::{TargetProfile}` / `use super::{Field, LayoutPlacementReport}`); rustc E0432/E0425 drive convergence. Four-crate lib suites 1896/1896 green. Residual at this commit: nine files, all sibling-fenced (BUILD-PACKAGES-GATE, RC-REPOSITORY-CLOSURE glob legs 1-2, RUNTIME-SIZED-ACTIVATION-STORAGE, PROOF-RULE-CLASSICALITY-AUDIT, MATCH-SELECTIVE-LOWERING); the ratchet stays red until they land. Third slice (2026-09-20, measured on `7110606f46`): the residual is down from nine files to **two**, so seven of the fenced legs have since landed. This pass took the one that had come unfenced — `package-source`'s `tree/capture/traversal.rs` `close_tests`, whose BUILD-PACKAGES-GATE fence expired at 21:49Z — converting it to the seven names the module uses, each from its own origin rather than re-exported through `super` (`OsStr`/`OsString`, `PathBuf`, `CapabilityDirectory`, `CapturedEntryObservation`, `SourceResolveError`, `close_captured_directory`); `cargo check -p package-source --all-targets` clean with no unused imports and 9/9 `close_tests` green. The two survivors are both still fenced and both were introduced by their own fence-holder's commit, so they belong to those lanes: `psi/foundation/extents` `activation_claims/tests.rs` (added by ee29067020, RUNTIME-SIZED-ACTIVATION-STORAGE / Devin z139) and `psi/semantics/validation` `value_custody/expression_types/result_type.rs` (added by 3bf8be9383, MATCH-SELECTIVE-LOWERING / zergling-182). Fourth slice (2026-09-21 at `18cebfa106`): `psi/foundation/extents` `activation_claims/tests.rs` converted once RUNTIME-SIZED-ACTIVATION-STORAGE's fence expired at 01:34Z — `use super::*;` replaced by the eight names the module uses (`ActivationClaimBranch`, `ActivationClaimLedger`, `ActivationClaimProvenance`, `ActivationClaimRequest`, `ActivationClaimSiteId`, `ClaimBoundRow`, `ClaimEstablishmentError`, `compose_claim_bounds`). Two traps rustc drove out: `ActivationStorage` is a VARIANT of `ActivationClaimProvenance`, not an importable item, and the lowercase `compose_claim_bounds` is invisible to a capitalized-identifier scan. `cargo check -p extents --all-targets` clean, 52/52 lib tests green. **Residual is now one file**: `psi/semantics/validation` `value_custody/expression_types/result_type.rs`, fenced to MATCH-SELECTIVE-LOWERING (Zergling-126, exp ~07:39Z) — the ratchet goes green when that lane lands. Note the regrowth pattern the earlier slices recorded has stopped: no new glob file landed between the z103 slice and this one.
- **HOSTED-INLINE-ASSEMBLY-AUTHORITY.** — mined candidate; scope verified, authority question already settled. The catalog in `psi/foundation/language-core/src/inline_assembly/` carries `required_authority` per instruction (`MachineOwner`, `PortIoAuthority`, `IdtControlAuthority`, `None`), per the privileged-services contract in `wiki/spec/build/permissions.md` (separate `MachineControl`/`PortIo`/`Mmio` service identities — listing the service does not establish ownership). The hosted-side authority decision is the implemented v0 discharge: `validation/src/machine_calls/effects/asm_discharge.rs::validate_asm_discharge` rejects every non-`None`-authority asm instruction on non-freestanding builds ("only code that owns the machine may emit privileged instructions; a hosted build would fault at ring 3") and passes freestanding — so hosted inline-assembly authority is denied by contract, not unimplemented. A finer hosted grant is a permissions.md spec change, not a compiler slice on this row.
- **INTRINSIC-PHYSICAL-SPAN-ARMS.** Resolved — re-mine of the intrinsic
  span-arm surface already adjudicated on sibling **TV-INTRINSIC-SPAN-ARMS**
  (verified `14e6f8f72e`). Verified at `96b4afed92e5`: every intrinsic
  family that produces coverage occurrences has its span arm — IEEE FMA
  joins `x86_scalar_fma_occurrences` fragments (`derive_fma_span`),
  integer comparisons join `semantic_code_attribution` rows with
  relocation-overlap rejection (`derive_integer_comparison_span`), float
  comparisons take the fragment-publication arm
  (`fragment_comparison::derive` under `fragment_publication`), and
  structural returns arrive through the checked-body call span. All other
  intrinsic realizations produce no occurrences — `checked_boundary_
  operator_occurrences` (`lowered-psi-to-terminal-psi/boundary_operator_
  custody/replay_scope.rs`) replays exactly the four families — so a
  further arm has no demand side; occurrence replay for the remaining
  families is **TV-OPERATOR-APPLICATIONS-REPLAY**'s scope. Sibling stubs
  on the same surface: REMAINING-INTRINSIC-SPAN-ARMS,
  TV-DYNAMIC-AND-INTRINSIC-SPANS.
- **INSTALLATION-ERA-JOURNAL.** Resolved — settled-name marker (do not
  re-mine): the named surface was deliberately deleted, not implemented.
  `20bd592af1` removed `ComponentEraJournal`, its restart fact
  vocabulary, replay roster, tests, exports and journal-only receipt
  accessors from `effects/src/component_eras/` — the owner rejected
  compiler-owned deployment recovery (Cathedral's domain), and that
  commit deleted both mined journal tasks; the entry ledger survives
  byte-identical to `8f9b82fef2`. Re-verified at `b9635834f3`
  (linux x86-64): zero `ComponentEraJournal`/`era_journal` references
  remain in the tree. No leg exists under this name.
- **LIFETIME-MULTI-SOURCE-AND-OUTLIVES.** — mined candidate; scope verified,
- **LIFETIME-MULTI-SOURCE-AND-OUTLIVES.** Mined candidate — scope verified,
  two legs — re-mines the [lifetimes](wiki/spec/language/lifetimes.md)
  returned-view frontier and the [conformances](wiki/spec/language/conformances.md)
  application-matching boundary. Multi-source leg (landed at `9106b1ca037`
  upstream and `2d0de49fe8a8` on the z148 lane):
  `typed-trees-to-checked-trees/src/borrow/view_link.rs` maps an explicit
  result lifetime to EVERY input parameter carrying it — direct reference
  parameters and structurally carried leaves alike — each contributing its
  matching leaves as possible sources of the returned view and each required
  to supply the result's access (`IncompatibleSourceAccess` still rejects a
  restricted sibling). The shared resolver feeds the declaration check and
  the loan attributor unchanged: multi-source signatures resolve to
  `ViewReturnSource::Fields` with one field per (result leaf x matching input
  leaf), and the existing per-field loan path tracks each contributing
  source, so writing any candidate while the view is live rejects
  (`borrow/` multi_source_* canaries on the z148 lane). Elision stays
  single-source: unannotated multiple candidates still reject as
  `ElidedMultipleInputs`. Outlives leg:

  general authored outlives bounds have no surface — lifetimes.md spells
  binders only, conformances.md states whole-conformance applications do
  not gain outlives/variance/subtyping and introducing them requires
  revisiting the application-matching rule; there is no authored syntax or
  semantics to implement, so that leg waits on a spec decision, not a
  checker gap. Dispatch note (`2b69813615`, refreshed):
  the multi-source surface is fenced — `view_link.rs`, `view_link/`, and
  `checks/borrows/` currently sit inside BORROW-PROOF-CONVERGENCE's claim
  (~06:46Z; earlier fences under GENERIC-RETURNED-VIEW-LIFETIMES 22:27Z and
  DYNAMIC-RECEIVER-LOAN-ORIGIN 01:47Z expired unworked). Sibling rows
  LIFETIME-SOURCE-CORRESPONDENCE and GENERIC-RETURNED-VIEW-LIFETIMES record
  the same clause family and the same residual pair.
  binders only, conformances.md states whole-conformance applications do
  not gain outlives/variance/subtyping and introducing them requires
  revisiting the application-matching rule; there is no authored syntax or
  semantics to implement, so that leg waits on a spec decision, not a
  checker gap. Dispatch note (`669925b8b9`): the multi-source surface is
  fenced — `view_link.rs`, `view_link/`, `loans.rs` under
  GENERIC-RETURNED-VIEW-LIFETIMES (22:27Z) and `checks/borrows/` under
  DYNAMIC-RECEIVER-LOAN-ORIGIN (01:47Z); sibling row
  LIFETIME-SOURCE-CORRESPONDENCE is the same clause family and is itself
  claimed (01:51Z).
  The two stale reject-expectation pins the earlier residual named
  (`carrier_result_ambiguous_inputs_and_access_escalation_reject`,
  `direct_result_rejects_ambiguity_between_owned_and_direct_inputs`) and
  the retired `LifetimeMatchesMultipleInputs` variant plus its elision
  diagnostic arm are drained on both lanes (upstream `9106b1ca037`,
  z148 `2d0de49fe8a8`).
- **LIFETIME-SOURCE-CORRESPONDENCE.** Scope verified on `8ccd793fa8` — re-mine
  of the same clause family as sibling GENERIC-RETURNED-VIEW-LIFETIMES
  (annotated dispatch above). `borrow/view_link.rs` ("Lifetimes stage 2")
  resolves an explicit result lifetime to exactly one input parameter and its
  complete matching structural leaves — reusing one lifetime across multiple
  inputs rejects, as the README's lifetime-source-correspondence section
  records; that rejection is the deliberate boundary, not a gap. The residual
  named the multi-source leg — every parameter carrying the selected lifetime
  contributes leaves as possible sources, each supporting the returned access.
  That leg has since landed at `9106b1ca037` ("explicit result lifetime unions
  same-lifetime inputs as view sources"): `view_link.rs` now links an explicit
  result lifetime to every input carrying the name, the returned view's sources
  form the union of those inputs' matching leaves, and each candidate must
  supply the result's access (README section updated in the same commit; the
  unannotated-multiple-carried-sources case still rejects as the deliberate
  boundary). Open residuals: general caller-side generic returned-view
  attribution (README §Lifetime source correspondence, "remain incomplete"),
  and the outlives leg — no authored syntax exists (lifetimes.md spells binders
  only, conformances.md states whole-conformance applications do not gain
  outlives/variance/subtyping), so it waits on a spec decision, not a checker
  gap. Re-verified at `e7c0099cb2b7`: `view_link.rs` and `loans.rs` are
  claim-free, but the witness surfaces stay fenced — `src/checks/borrows` +
  `src/tests/borrow` to BORROW-PROOF-CONVERGENCE (exp 06:46Z) and the crate's
  `tests/` dir to PROOF-CONTRACT-MIGRATION (exp 10:38Z). No independent
  unclaimed slice exists here.
- **LOOKUP-MAP-MEASUREMENT-AUDIT.** Mined candidate — scope verified,
  already landed and enforced; same verdict as sibling row
  SCOPED-LOOKUP-MAP-AUDIT, whose surface this stub re-mines: the
  "measured reason" audit exists as the repeatable architecture gate
  gap. Every implementing surface is live-fenced this wave: `view_link.rs`,
  `view_link/`, `loans.rs`, `loans/`, `checks/borrows/elision*`,
  `tests/multi_source_view_lifetimes.rs` and the crate README are claimed
  under GENERIC-RETURNED-VIEW-LIFETIMES (exp 22:27Z). No independent unclaimed
  slice exists here.
- **LOOKUP-MAP-JUSTIFICATION.** Mined candidate — scope verified,
  already landed and enforced; same verdict as sibling rows
  SCOPED-LOOKUP-MAP-AUDIT and LOOKUP-MAP-MEASUREMENT-AUDIT (adjacent
  stub), whose surface this stub re-mines: the justification audit
  exists as the repeatable architecture gate
  `tests/architecture/scoped_lookup_maps.rs` enforcing the
  `omega-rust/pipeline.md` rule ("scoped symbol-tree lookup is the
  baseline; extra lookup maps require a measured reason") by census —
  every production `HashMap`/`BTreeMap` keyed by an authored-spelling
  token must appear in `JUSTIFIED_LOOKUP_MAP_FILES` with its recorded
  key domain, and the reverse staleness check fails cataloged files
  that no longer declare such a map. The one-shot census it encodes
  lives at `wiki/drafts/lookup_map_justification.md` (run at
  `c2ccb2a202`; zero name-keyed declaration-lookup maps beside the
  scoped symbol tree). Verified green at `66a6ea93f7`:
  `cargo nextest run -p omega-architecture-test --test
  scoped_lookup_maps` 2/2 pass on Linux x86-64. No independent slice
  remains — the gate is self-maintaining; a new name-keyed map without
  a recorded justification fails the build. Sibling re-mine stub on
  the same surface: LOOKUP-MAP-JUSTIFICATION (adjacent row).
  Re-verified at `42759dd529` for the dispatched LOOKUP-MAP-JUSTIFICATION
  re-mine: `cargo nextest run -p omega-architecture-test --test
  scoped_lookup_maps` re-witnessed 2/2 PASS on Linux x86-64
  (`every_name_keyed_lookup_map_file_is_cataloged`,
  `every_cataloged_file_still_observes_a_name_keyed_map`). The
  adjudication is unchanged — no independent slice exists.
  Re-verified at `f72122f71e` (z181): `tests/architecture/scoped_lookup_
  maps.rs` gate and `wiki/drafts/lookup_map_justification.md` census
  both present at base; no same-item claim live (the
  LOOKUP-MAP-MEASUREMENT-AUDIT sibling claim drained ~08:36Z). Field
  note (review f72ba17fce6d..7241e022270d): this stamp had been spliced
  mid-sentence into the paragraph above — z181's second such splice
  (see BENCHMARK-ROW-RESUMPTION); append stamps after the row's last
  sentence.
  Re-verified at `32a6a7fa33` (z199): the gate
  `tests/architecture/scoped_lookup_maps.rs` (JUSTIFIED_LOOKUP_MAP_FILES
  catalog :32; census tests `every_name_keyed_lookup_map_file_is_cataloged`
  :429 and `every_cataloged_file_still_observes_a_name_keyed_map` :448)
  and the `wiki/drafts/lookup_map_justification.md` census are both
  present at base; adjudication unchanged — the gate is
  self-maintaining, no independent slice exists.
- **LOWERED-CRASH-MEMBER-BYTE-ENTRIES.** — mined candidate; scope verified, family repaired. The stub names the crash-member byte-entry group of checked-trees-to-lowered-psi (`tests/crash_member_source/byte_entries.rs`); `wiki/drafts/known_baseline_failures.md`'s own re-reading at d8d48fe4ff already records crash-member byte entries green alongside boundary byte buffers and the ordered-boolean row, and the whole `crash_member_source` suite re-verifies green at this revision (`cargo nextest run -p checked-trees-to-lowered-psi --test suite crash_member_source`: 48/48, linux x86-64). Re-witnessed again at `d6a0625f6b`: 48/48 pass in 61.3s (the `unsupported_mixed_aggregate_equality_shapes_remain_fenced` member is a 60.7s slow pin, not a failure). The live residual families in that crate are already owned: bare boundary-trait fixture spellings by ENTRY-CONTENT-ROOTS, scalar-return custody / provider attachment / attached-unit sets by C2L-BASELINE-FAILURE-ATTRIBUTION and C2L-RESIDUAL-FAILURE-ATTRIBUTION, and the proof-search blowup by C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT. No independent slice remains on this row.
- **LOWERED-OPERATION-PROOF-MACHINE-CALLS.** — mined candidate; scope verified, route already exercised. The stub names operation proofs on lowered machine-call operations and proof-output call custody in checked-trees-to-lowered-psi. Both are implemented and green at e76d715c8e (verified base 6ef64f6dd6): `proofs/operation_proofs.rs::finalize_operation_proofs` discharges call obligations (the previously red `unit_scalar_result_source::boundary_wrappers::ordered_boolean_guarantees::ordered_boolean_call_computations_preserve_normal_guarantees` machine_calls row now passes — the group reads 28/28 green), `proofs/evidence_lowering/proof_output_calls.rs::lower_proof_output_calls` keeps runtime-value bindings on their ordinary scalar Call operation, `terminal-verifier/validation/evidence/proof_output_calls.rs` cross-checks `runtime_call.operation` against the caller's operations, and `proof_recursion.rs::proof_machine_dependency_closure` covers proof machine call reachability (6/6 green). Pins: `evidence_identity_source` suite 22/22 green (cargo nextest, linux x86-64) including `runtime_value_proof_output_links_one_scalar_call_and_executes_once`. The live residuals in this crate are already owned: bare `Service<R>` fixture spellings by ENTRY-CONTENT-ROOTS, transitive machine plans by GENERAL-CYCLIC-EXECUTION/UEFI-OS-HANDOFF, site_guard crash namespace and scalar-return custody by WRITE-ONLY-BORROW integer-entry-ranges, `established by` qualification by BOUNDARY-ISSUANCE, and the proof-search blowup by C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT. No independent slice remains on this row.
  that no longer declare such a map. The one-shot census it encodes —
  this row's namesake artifact — lives at
  `wiki/drafts/lookup_map_justification.md` (run at `c2ccb2a202`; zero
  name-keyed declaration-lookup maps beside the scoped symbol tree).
  Verified green at `66a6ea93f7` per the sibling row: `cargo nextest
  run -p omega-architecture-test --test scoped_lookup_maps` 2/2 pass
  on Linux x86-64. No independent slice remains — the gate is
  self-maintaining; a new name-keyed map without a recorded
  justification fails the build.
- **LOOKUP-MAP-MEASUREMENT-AUDIT** — mined candidate; duplicate stub. The
  family is already drained: the resolved LOOKUP-MAP-MEASUREMENT-AUDIT row
  above and LOOKUP-MAP-JUSTIFICATION both scope-verify this surface as a
  re-mine of SCOPED-LOOKUP-MAP-AUDIT, which carries the resolution — the
  audit exists as the repeatable architecture gate
  `tests/architecture/scoped_lookup_maps.rs` (census of every production
  `HashMap`/`BTreeMap` keyed by an authored-spelling token into
  `JUSTIFIED_LOOKUP_MAP_FILES`, plus the reverse staleness check) with the
  one-shot census at `wiki/drafts/lookup_map_justification.md` (run at
  `c2ccb2a202`), verified green at `771d0469a1c47` on linux x86-64
  (`cargo nextest run -p omega-architecture-test --test scoped_lookup_maps`
  2/2). No independent slice remains — this row is a duplicate name for the
  same covered surface.
- **LOOKUP-MAP-MEASUREMENT-AUDIT.** Mined candidate — scope verified,
  already landed and enforced; same verdict as sibling row
  SCOPED-LOOKUP-MAP-AUDIT, whose surface this stub re-mines: the
  "measured reason" audit exists as the repeatable architecture gate
  `tests/architecture/scoped_lookup_maps.rs` enforcing the
  `omega-rust/pipeline.md` rule ("scoped symbol-tree lookup is the
  baseline; extra lookup maps require a measured reason") by census —
  every production `HashMap`/`BTreeMap` keyed by an authored-spelling
  token must appear in `JUSTIFIED_LOOKUP_MAP_FILES` with its recorded
  key domain, and the reverse staleness check fails cataloged files
  that no longer declare such a map. The one-shot census it encodes
  lives at `wiki/drafts/lookup_map_justification.md` (run at
  `c2ccb2a202`; zero name-keyed declaration-lookup maps beside the
  scoped symbol tree). Verified green at `66a6ea93f7`:
  `cargo nextest run -p omega-architecture-test --test
  scoped_lookup_maps` 2/2 pass on Linux x86-64. No independent slice
  remains — the gate is self-maintaining; a new name-keyed map without
  a recorded justification fails the build. Sibling re-mine stub on
  the same surface: LOOKUP-MAP-JUSTIFICATION (adjacent row).
- **LOWERED-CRASH-MEMBER-BYTE-ENTRIES** — mined candidate; scope verified, family repaired. The stub names the crash-member byte-entry group of checked-trees-to-lowered-psi (`tests/crash_member_source/byte_entries.rs`); `wiki/drafts/known_baseline_failures.md`'s own re-reading at d8d48fe4ff already records crash-member byte entries green alongside boundary byte buffers and the ordered-boolean row, and the whole `crash_member_source` suite re-verifies green at this revision (`cargo nextest run -p checked-trees-to-lowered-psi --test suite crash_member_source`: 48/48, linux x86-64). The live residual families in that crate are already owned: bare boundary-trait fixture spellings by ENTRY-CONTENT-ROOTS, scalar-return custody / provider attachment / attached-unit sets by C2L-BASELINE-FAILURE-ATTRIBUTION and C2L-RESIDUAL-FAILURE-ATTRIBUTION, and the proof-search blowup by C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT. No independent slice remains on this row.
- **LOWERED-OPERATION-PROOF-MACHINE-CALLS.** Mined candidate — scope verified, route already exercised. The stub names operation proofs on lowered machine-call operations and proof-output call custody in checked-trees-to-lowered-psi. Both are implemented and green at e76d715c8e (verified base 6ef64f6dd6): `proofs/operation_proofs.rs::finalize_operation_proofs` discharges call obligations (the previously red `unit_scalar_result_source::boundary_wrappers::ordered_boolean_guarantees::ordered_boolean_call_computations_preserve_normal_guarantees` machine_calls row now passes — the group reads 28/28 green), `proofs/evidence_lowering/proof_output_calls.rs::lower_proof_output_calls` keeps runtime-value bindings on their ordinary scalar Call operation, `terminal-verifier/validation/evidence/proof_output_calls.rs` cross-checks `runtime_call.operation` against the caller's operations, and `proof_recursion.rs::proof_machine_dependency_closure` covers proof machine call reachability (6/6 green). Pins: `evidence_identity_source` suite 22/22 green (cargo nextest, linux x86-64) including `runtime_value_proof_output_links_one_scalar_call_and_executes_once`. The live residuals in this crate are already owned: bare `Service<R>` fixture spellings by ENTRY-CONTENT-ROOTS, transitive machine plans by GENERAL-CYCLIC-EXECUTION/UEFI-OS-HANDOFF, site_guard crash namespace and scalar-return custody by WRITE-ONLY-BORROW integer-entry-ranges, `established by` qualification by BOUNDARY-ISSUANCE, and the proof-search blowup by C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT. No independent slice remains on this row.
- **LOWERED-PSI-BASELINE-TAIL.** Mined candidate; scope verified at
  9beef2b045 — the stub names the remaining checked-trees-to-lowered-psi
  baseline tail (57 FAIL + 1 SIGTERM at bcb0086e22 per
  `wiki/drafts/known_baseline_failures.md`'s six-family attribution).
  Every family is already owned and fenced: (1) stale bare
  `Service<R>`-spelling fixtures, 33 tests → ENTRY-CONTENT-ROOTS
  (`src/tests` additionally under PROOF-CERTIFICATION-BRIDGE, crate
  `tests/` under WRITE-ONLY-BORROW); (2) missing checked transitive
  machine plan, 16 tests → fences GENERAL-CYCLIC-EXECUTION +
  UEFI-OS-HANDOFF; (3) site_guard crash-namespace, 3 tests →
  WRITE-ONLY-BORROW integer-entry-ranges; (4) scalar-return custody,
  4 tests → WRITE-ONLY-BORROW + C2L-BASELINE/RESIDUAL-FAILURE-
  ATTRIBUTION; (5) `established by` qualification, 1 test →
  ENTRY-CONTENT-ROOTS / BOUNDARY-ISSUANCE; (6) proof-search SIGTERM →
  C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT. No unowned slice remains — the tail is the
  union of those owned residuals. Claim attempt on the c2l surface

  `"crash predicate value position is outside the selected scalar
  namespace"` cluster (3 tests —
  `exact_shift_left_certificate_source::bounded_exact_left_shift_uses_only_its_canonical_certificate`,
  `exact_affine_sibling_source::landed_affine_sibling_custody_crosses_source_codec_and_independent_verification`,
  `mixed_shift_source::erased_arithmetic_prefix_still_requires_its_own_certificate`).
  Root cause: `returns/structural_scalar_return/nominal.rs` stages a
  synthetic `CheckedUnitEffectMachinePlan` for scalar-return machines
  needing nominal affine cleanup with `scalar_parameters: Vec::new()`,
  but the staged scratch contract kept the machine's authored
  `requires` prefix; `signature.requires` lowering then crashed against
  the empty scalar lane. The staged contract now carries only its
  derived requires tail (the return lane republishes authored scalar
  requirements onto the real caller itself). Remaining tail: the other
  documented `checked-trees-to-lowered-psi` baseline clusters
  (`provider_attachment`, `composed_operand_catalogs`,
  `dynamic_composed_unit`, `unit_state_graph`,
  `owned_record_return_source`, `unit_plan_omissions`) — see
  `wiki/drafts/known_baseline_failures.md`; several sit under live
  sibling claims, so partition by claim fence before picking up.
- **LOWERED-SCALAR-RESULT-SOURCE-CUSTODY** — mined candidate; verify scope then implement.
- **LOWERED-UNIT-FAILURE-ATTRIBUTION.** Scope verified at `9637703955c` —
  names the unit-* failure attribution leg inside
  `checked-trees-to-lowered-psi`. The attribution is already performed by
  the fresh census (`wiki/drafts/c2l_failure_census_d936717f.md`, landed
  `54e321bdf00`): the unit family is 19 of the 24 live failures —
  `provider_attachment_source::*` (6) + `unit_state_graph::
  provider_attachments::*` (9) + `guarded_scalar_returns_source::
  stored_returned_cases_support_borrowed_refined_getters` (1) all in the
  missing-transitive-machine-plans family owned by
  GENERAL-CYCLIC-EXECUTION/UEFI-OS-HANDOFF, and `unit_plan_omissions::*`
  (3) missing entry claims on omitted local constructions. Writing the
  attribution into `known_baseline_failures.md` is file-fenced by
  RC-REPOSITORY-CLOSURE (live claim); the owning repair lanes
  are fenced separately (c2l `src/unit` by STRUCTURAL-UNIT-LOWERING,
  `src/returns` by C2L-RESIDUAL-FAILURE-ATTRIBUTION). No independent
  slice remains under this name.
  Re-verified at `c267df86acb` (linux x86-64): `cargo nextest run -p
  checked-trees-to-lowered-psi --no-fail-fast` now reads 2187 legs — 2130
  passed, 55 FAIL, plus the same
  `mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  blowup killed by SIGTERM at ~900s (still C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT's).
  Members stay inside the recorded families — 32 bare `Service<R>`
  fixture spellings at `src/tests.rs:83` (ENTRY-CONTENT-ROOTS), missing
  checked transitive machine plans in `provider_attachment_source` (×6),
  `unit_state_graph` (×9) and `unit_plan_omissions` (×3), scalar-return
  custody in `owned_record_return_source` (×4) plus
  `guarded_scalar_returns_source` (×1) — 3 fewer than the 6ef64f6dd6
  reading; member-level re-attribution belongs to the sibling
  attribution claims holding `wiki/drafts/known_baseline_failures.md`
  (LOWERED-UNIT-FAILURE-ATTRIBUTION ~01:17Z, BASELINE-CANARY-PASS-
  CLUSTER ~23:41Z, CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION ~01:42Z), so
  this update stays on the board line and leaves the doc to them.
  Re-verified at `0f75a052f0` (z151, linux x86-64): the tail halved —
  22 named-member fails plus the known SIGTERM vs 55+SIGTERM at
  c267df86acb. `--lib` is fully green (791/791): the `Service<R>`
  stale-fixture family has drained under the service-spelling wave.
  The family-3 crash-namespace trio (`exact_affine_sibling_source`,
  `exact_shift_left_certificate_source`, `mixed_shift_source`
  members), the family-5 `established by` leg
  (`boundary_result_domain_calls`), and the `composed_operand_catalogs`/
  `dynamic_composed_unit` modules all pass in the filtered selection.
  Every remaining fail shares the owned signatures: the
  provider-attachment/transitive-plan cluster reads 19 —
  `provider_attachment_source` (×6), `unit_state_graph::provider_attachments`
  (×9), `unit_plan_omissions` (×3), plus
  `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters`
  whose signature migrated into the same `InvalidUnitMachinePlan`
  "missing a checked transitive machine plan" family (producers
  `src/unit/attached_unit` + t2c `execution/unit/*` — UEFI-OS-HANDOFF
  live to 10:19Z, c2l `src/unit` fenced by STRUCTURAL-UNIT-LOWERING to
  09:16Z); scalar-return custody reads 3 in
  `owned_record_return_source` (4→3) — discarded pure-call elision,
  "composed Unit scalar call requires structural call custody", and a
  replay unwrap — the C2L custody lane's residual (file under
  C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES to 10:35Z, producing
  surfaces under CRASH-CONTRACT 04:27Z + PROOF-RELEVANCE-MIGRATION
  11:29Z). The nominal_affine SIGTERM leg is un-rerun — its file sits
  under RC-REPOSITORY-CLOSURE (12:15Z). Still no unowned slice.
- **LOWERED-UNIT-FAILURE-ATTRIBUTION.** Owns the
  `wiki/drafts/known_baseline_failures.md` attribution sweep the
  sibling rows keep deferring to. Executed at `e7c0099cb2b7` (linux
  x86-64): `cargo nextest run -p checked-trees-to-lowered-psi
  --no-fail-fast` with the C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT blowup member
  filtered out reads 2206 run / 2183 passed / 23 failed / 1 skipped —
  same headline as 7b224763615, different composition. The doc now
  carries the fresh member-level attribution: the missing-transitive-
  machine-plan family shifted to 18 members (provider_attachment_source
  ×6, unit_plan_omissions ×3, guarded_scalar_returns ×1 drained;
  conformance_applications ×3, composed_operand_catalogs ×5,
  composed_unit_internal_calls ×1 joined; unit_state_graph::
  provider_attachments holds at 9 under reworked names), scalar-return
  custody is down to the single structural-custody member, the
  fixed-fuel verdict closed, and a new 4-member family opened —
  `scalar_array_source::cyclic` index-out-of-bounds panic at
  `call_lowering.rs:419` (erased-proof-argument roster zip; entered with
  f0f808f419989/576b9a76dc49a — a lowering bug for the scalar-graph/LICM
  lane, not an authored rejection). Unattributed tail: empty. The
  earlier pass-canary and staged-local refreshes stand; the doc's
  remaining stale spots are now current at this revision.
- **C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT.** Remove the cubic lowering cost that
  makes `nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  never return. **The name is wrong and this row's earlier hypothesis is
  refuted** — measured at `de5798c306`, it is not a proof search, nothing
  diverges, and `proof/src/checker` is not involved.

  What was measured, by bisecting the fixture on two axes with a scratch probe
  timing `lower_typed_trees` and `lower_machine` separately:
  - **The 80-premise `requires` pool is not the driver.** Dropping 48 of the
    80 premises, including all ten redundant `input` upper bounds, changes
    lowering cost by 1.5x (3251ms to 2191ms). No cliff, no sign of subset
    enumeration.
  - **The machine body carries the whole cliff.** Holding premises fixed and
    varying the top-level `&&` conjuncts of `staged`: 24 conjuncts lowers in
    3.1s, 72 in 37.5s, 73 in 37.3s, and **74 exceeds 400s**. It is a count
    threshold, not one conjunct — omitting group 74 and taking six later ones
    (79 conjuncts) also exceeds 240s. Below the cliff the curve is smooth and
    polynomial, about n^2.5 to n^3.
  - **Every obligation converges and succeeds.** Tracing producer calls over
    200ms at the cliff: 192 calls, 99.6s total, **192 of 192 returned a
    proof**; the relaxed fallback was never reached and the kernel's own
    `StepCeiling` was never hit. There is no non-converging obligation to
    contain.
  - Terminating reduced input for future work: the first 73 body conjuncts
    with all 80 premises, 45s total.

  The cost is four compounding centres, none in `proof/src/checker`:
  - `proofs/scalar_block_invariants.rs:48` and
    `proofs/scalar_block_invariants/cyclic_guarantees.rs:40` each call
    `terminal_verifier::reconstruct_terminal_obligations` on the same
    unchanged module, and each reconstruction is itself O(N^2) — about 47% of
    lowering even when the candidate roster is empty and neither loop
    iterates.
  - `terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs:71-84`
    clones **every** `Equal(Value, _)` axiom in the roster into a certificate
    per condition fact, and `condition_fact` runs twice per conditional. A
    chain of N short-circuiting `&&`s gives O(N) conditionals with O(N)
    rosters — O(N^2) kernel work, and the source of ~20,000 certificate
    acceptances.
  - Per-certificate kernel cost is linear in chain length:
    `mathematical_core::typing::infer_type` recurses past depth 260 on one
    certificate from this program.
  - Independently, in checking (16% of the run),
    `typed-trees-to-checked-trees/src/authored_selections/operator_targets.rs:56`
    scans per operator-by-fact pair and `member_targets.rs:369-381` tests
    membership with a `Vec` linear scan.

  **Two of the four centres are closed at `4003c703186`**, measured on the
  terminating reduction: the checking-stage `expression_contains` visited set
  and per-operator scan (7.02s to 3.35s, 2.09x), and the duplicated
  whole-module reconstruction in `retain_provable` (lowering 37.8s to 31.0s;
  a counter inside `reconstruct_terminal_obligations` showed the call count
  fall from 2 to 1, 13.4s to 6.7s, so the whole saving is the removed
  duplicate). Total 51.6s to 41.1s, 1.26x. The same obligations are
  discharged either way — 633 evidence rows and 633 reconstructed obligations
  before and after, on every run. The unreduced fixture still returns no
  verdict, killed at 780s; these two were never predicted to make it
  terminate.

  **The third centre was measured and is misattributed.** Rebuilding the
  roster at `path_facts/conditions.rs:71-84` is real, but it is **0.4-1.4% of
  the `condition_fact` call** and it is *linear* (0.125 us/axiom, flat from
  N=1 to N=801). The call itself is super-linear over the same range (x2 on N
  costs x2.45, then x2.80, then x3.25), so the quadratic is provably
  elsewhere. Counting inside the kernel at one `condition_fact` call:
  `check_node` visits and `context.validate` calls are linear in N, while
  comparisons in `proof-admission/src/proof.rs`'s `record_premise` are
  exactly N(N-1)/2 -- 325 at N=26, 20,100 at N=201, 352,575 at N=801. Each
  comparison is a full structural `Proposition == Proposition`. A certificate
  cites the whole roster by design, so N cited nodes x an O(N) scan of the
  accepted set is the O(N^2), and it is O(N^3) per path.

  **Closed.** `record_premise`'s accepted-premise list now carries an
  index-keyed map of the positions each index occupies, so the proposition
  comparison runs only against rows that already share the citation's index.
  The admitted predicate is untouched -- a citation is new unless a recorded
  premise shares both index and proposition -- and cited axioms carry
  distinct indices, so the pathological case falls from N(N-1)/2 comparisons
  to none. Both copies of the routine were converted (`proof.rs` and
  `mathematical_core/bounded_denotation.rs`); 1316/1316 green across
  proof-admission, proof and terminal-verifier, with the two trusted-surface
  digests re-recorded.

  This is a whole-system win, not a condition-fact one: every certificate the
  kernel accepts went through that scan.

  Hoisting the roster itself was written and validated separately and is
  **not worth landing**: `ValueEqualityTransport` owns its equalities, so each
  certificate must still deep-clone them, and measurement puts the saving at
  ~12% of a term that is at most 1.4% of the call, against churn in five
  digest-pinned files. Narrowing the cited roster to the equations the walk
  actually traversed remains the only other asymptotic lever, and that one
  does alter what the kernel is shown.

  Tail-risk note: across a full `terminal-verifier` run (4,835 calls) the
  roster is tiny -- p50 2, p90 6, p99 42, max 582 -- so this was a deep
  straight-line-path risk, not a present cost in the suite.

  A fourth centre is now what remains of checking, and it is not one the
  original diagnosis named: a profile of the remaining 3.3s shows no frame in
  the repaired walk at all, and the time sits in
  `checks::ranges::indexes::check_expression` and
  `checks::ranges::facts::dependencies::RangeFacts::record_dependencies`,
  each recursing about 25 levels beneath `seed_binary_guard_facts`. The
  diagnosis's estimate that the two checking fixes were worth "most of a
  7-second checking stage" was half right — they were worth exactly half.

  Reproduction note for the reduction: it is the first **72** top-level `&&`
  conjuncts under a split that treats the leading parenthesised triple as one
  conjunct. That reading is the one that reproduces 7s checking, 38s lowering
  and 633 evidence rows; counting the triple separately gives 73.

  **Do not bound the search.** 192 of 192 traced obligations are provable, so
  any bound here abandons obligations the compiler demonstrably proves. The
  other option this row used to offer — refusing fail-closed — is recorded as
  **design-blocked** in OWNER_QUESTIONS.md question 4.

  Also recorded, because several rows depend on it:
  `OMEGA_PROOF_MEASUREMENTS` does not instrument this stage at all. On the
  terminating reduction its whole report is `obligations=1 ...
  decided_elsewhere=1`, with every other counter zero — the ~20,000 kernel
  acceptances and 633 evidence rows this program costs are all produced
  outside `check_proof_plan`. LOOKUP-MAP-MEASUREMENT-AUDIT and the other rows
  routing cost questions here through the retired PROOF-SEARCH-MEASUREMENT are
  pointing at the wrong substrate.

  Acceptance: the unreduced fixture terminates with a verdict under an
  ordinary test timeout, with no obligation abandoned — that is, the repair is
  algorithmic and the traced producer calls still return their proofs — and a
  `--no-fail-fast` run of `checked-trees-to-lowered-psi` reports no SIGTERM
  member.

  **DESIGN-BLOCKED for the remaining leg (verified 2026-09-21).**
  `wiki/spec/proofs/kernel_metatheory.md` speaks only per-conversion, and says
  `DEFAULT_CONVERSION_STEPS` "is a policy default, not part of the calculus";
  `grep -c aggregate` over that file is 0. Nothing states what the compiler owes
  for the AGGREGATE of individually-bounded acceptances, which is exactly what a
  containment bound would have to be. Open as `compile-time-proof-work-ceiling`
  (OWNER_QUESTIONS.md:196) with three unchosen answers. NOTE: this is the
  remaining leg only -- the measured O(N^2) centre was `record_premise`'s linear
  scan and is FIXED, and the other cost centres are landed.

- **MATCHING-LOGIC-TYPED-TO-ONE-SORTED-ENCODING.** Verified scope
  (Zergling-181): the "typed-to-one-sorted encoding" bullet of the bounded
  comparison in `wiki/drafts/matching_logic.md`, drafted in
  `wiki/drafts/matching_logic_sort_encoding.md` — encode `Nat`, `Int`,
  `addr`, slices, and a user sum into the one-sorted finitary basic fragment
  as membership patterns with disjointness/refinement clauses, definedness
  preconditions for partial operations, junk-model quantifier guards,
  revision (refinement-not-invalidation) rules, and borrow/multiplicity loan
  clauses. Landed at `cc4c4feb89`; re-verified on `163670cf6d` (linux
  x86-64, `python3 tools/tests/test_matching_logic_sort_encoding.py` —
  8/8 green):
  `tools/matching-logic-sort-encoding/sort_encoding.py` emits the clause
  inventory plus the evidence record the doc names (fragment, rule and
  semantics versions — sha256 of the source draft — subject digest, target
  capsule, observation profile, bridge graph, admissions, diagnostics) and
  `check` enforces definedness coverage, intended-model inhabitedness,
  revision refinement, exclusive-loan disjointness, reborrow lineage,
  pair-constructor injectivity, sum tag disjointness, declared payload/slice
  memberships, and fixpoint certificates. Pinned cases: `cases/reference.json`
  positive plus seven negatives. Verified:
  `python3 tools/tests/test_matching_logic_sort_encoding.py -v` — 8/8 green
  on linux x86-64. Remaining acceptance: wire the emitted clauses and
  evidence into the bounded comparison harness — fenced to
  MATCHING-LOGIC-BOUNDED-SLICE (`tools/matching-logic-slice/`, live claim);
  no encoding-to-checker translation is admitted authority.
- **NATIVE-DIFF-FRONTEND-DROP-ORDER.** Resolved — the frontend drop-order
  expectations lane is landed and green in
  `tests/native-differential/tests/frontend_drop_expectations.rs`'s
  drop-order block (:705+): authored consume sequences decide hook
  eligibility per exit, multiple drop fences on one declaration emit
  diagnostics in a fixed precedence order, and cleanup ORDER is pinned
  through admission + diagnostic sequencing (the reference interpreter
  runs no observable cleanup yet — a future cleanup-executing
  interpreter must extend, not silently change, the block). Re-witnessed
  at `5246ff65f4c`: `cargo nextest run -p omega-native-differential-test
  --test frontend_drop_expectations` → 26/26 PASS on linux x86-64
  (incl. drop-order-two-hooks, -reverse-explicit, -double-consume,
  -reassign-consumed, -field-path). The residual — runtime cleanup-order
  evidence — is pinned as a deliberate future expectation change, not an
  open board leg.
- **NATIVE-DIFF-HOSTED-RECEIVER-CHECKED-ENTRY** — mined candidate; verify scope then implement.
  on linux x86-64. The comparison-metrics leg landed here (Zergling-50):
  `route.matching_logic_encoding` in `tools/matching-logic-metrics` is
  measured when the bounded slice is present — checker/translation source
  inventories (`*.py`), theory (`checkerRules` from the slice record plus
  `encodingClauses` summed over the sort-encoding corpus with per-case
  diagnostics), certificate bytes, and slice-corpus check timings with
  polarity matching; `validate` checks the measured shape and `measure`
  exits nonzero on encoding mismatches (test_matching_logic_metrics.py
  16/16 green on linux x86-64 at `7176821bc6b`). Remaining acceptance:
  wire the emitted clauses and evidence into the bounded comparison
  harness — fenced to MATCHING-LOGIC-BOUNDED-SLICE
  (`tools/matching-logic-slice/`, `tools/matching-logic-slice-comparison/`,
  live claim); no encoding-to-checker translation is admitted authority.
- **MATCHING-LOGIC-VERTICAL-SLICE.** Landed — re-mines the pending
  candidate-side deliverable of MATCHING-LOGIC-BOUNDED-SLICE (the
  `tools/matching-logic-slice/` checker the slice-comparison record was
  waiting on). `tools/matching-logic-slice/slice_checker.py` is a
  certificate checker for the one-sorted finitary basic fragment (no
  fixpoints — the fragment the draft's completeness citation covers):
  scalar propositions, equality with substitution, quantification with
  definedness and eigenvariable discipline, a declared Terminal state
  transition, and the refinement obligation reconstructed from the
  canonical subject (a producer cannot supply a weaker question).
  `check` verifies a case's derivation tree and reports the axiom
  admissions it consumed; `record` writes the candidate-side columns
  (`omega-matching-logic-slice-record/1`). Pinned cases:
  `cases/reference.json` positive (`exists s'. step(s0,s') /\
  in(counter(s'),Nat)` via transition + membership transport) plus
  seven negatives (weaker goal, undeclared axiom, undefined witness,
  eigenvariable escape, wrong-side equality substitution, half-proven
  membership body, reversed transition). Verified at `c267df86ac`
  (linux x86-64): `python3 tools/tests/test_matching_logic_slice.py` —
  13/13 green. Remaining for the lane: `matching-logic-metrics`
  `route.matching_logic_encoding` flips off `pending` and the
  slice-comparison's candidate columns fill once those records
  regenerate under their own rows. Re-witnessed at `e7c0099cb2`
  (linux x86-64): `python3 tools/tests/test_matching_logic_slice.py` →
  13/13 green; the comparison leg already landed per the sibling row —
  `records/577d6ac2ba.json` carries `route.matching_logic_encoding`
  **measured** (44/44 cases, 0 mismatches, 22 checkerRules / 38
  encodingClauses, 1256 certificate bytes).
  Re-witnessed at `c3dd8016a7` (z175, linux x86-64):
  `python3 tools/tests/test_matching_logic_slice.py` -> 13/13 green
  (Ran 13 tests OK).
- **MATCHING-LOGIC-COMPARISON-METRICS.** Resolved at `577d6ac2ba` —
  the "records regenerate" leg named on MATCHING-LOGIC-VERTICAL-SLICE:
  `tools/matching-logic-metrics/records/577d6ac2ba.json` is the first
  full record with `route.matching_logic_encoding` **measured** (the
  earlier committed records at `05416dd1a0`/`649d7ca380` were
  `--skip-cases` stubs with the route `pending`). All 44 pinned
  positive/negative cases ran `omega --check --offline --timings` on
  linux x86-64 with 0 mismatches; the encoding column reports the slice
  checker + sort-encoding inventories, 22 `checkerRules` / 38
  `encodingClauses`, 1256 summed certificate bytes, and measured
  slice-corpus timings. Verified: `run_metrics.py validate` 3/3 records
  conform to `omega-matching-logic-comparison/1`;
  `test_matching_logic_metrics.py` 16/16 green. Remaining lane legs:
  the slice-comparison harness's candidate columns fill under
  MATCHING-LOGIC-VERTICAL-SLICE's remaining lane legs.
  Re-witnessed at `62a52db5ff`: all three records still on disk,
  `run_metrics.py validate` → 3/3 conform,
  `test_matching_logic_metrics.py` → 16/16.
  Re-witnessed again at `138ed79a677`: validate → 3/3 conform,
  unittest → 16/16.
  Re-witnessed at `0a0662ad27a`: `run_metrics.py validate
  tools/matching-logic-metrics/records/*.json` → 3/3 conform to
  `omega-matching-logic-comparison/1`;
  `tools/tests/test_matching_logic_metrics.py` → 16/16
  (the suite lives under `tools/tests/`, not beside the harness).
  Re-witnessed at `72fc66d6c3267`: `run_metrics.py validate` → 3/3
  records conform to `omega-matching-logic-comparison/1`,
  `tools/tests/test_matching_logic_metrics.py` → 16/16.
- **NATIVE-DIFF-CUSTODY-EXPECTATION-RETARGET.** Resolved — duplicate of the
  already-adjudicated custody-expectation slice. The stub re-mines the
  custody-gate expectation surface left by the landed custody ordering:
  STALE-CUSTODY-GATE-EXPECTATIONS verified at `43104bde655a` that all five
  custody-named fail fixtures still reject with their pinned fragments under
  `fail_canaries_reject_with_expected_diagnostic_fragment`, and the
  `tests/native-differential` custody-order pin
  (`terminal_psi_source/contracts_and_frontend_drop.rs::
  source_statement_custody_gate_runs_after_the_parameter_custody_gate`,
  expecting `LoweringError::Unsupported("scalar source custody has no
  authored statement")` after the parameter custody gate resolves) is intact
  at `72fc66d6c32`. There is no stale custody expectation to retarget; the
  actual expected.txt drift census (12 drifted canaries + silent
  acceptances) is CANARY-CORPUS's named lane, not this row.
- **NATIVE-DIFFERENTIAL-MATRIX.** Mined candidate; scope verified at
  `ac4e4eee9b`: names the native-differential leg of the
  [RC-NATIVE-MATRIX](wiki/drafts/rust_compiler_completion.md#release-matrix)
  gate — `mbx nextest run -p omega-native-differential-test --all-targets
  --no-fail-fast` per hosted row (`wiki/drafts/rc_native_matrix_*.md`;
  linux_x86_64 witnessed red 22/38 at `e76d715c8e`, windows_x86_64 open —
  no runner, macOS ARM64 in flight). The crate is
  `tests/native-differential`; each fenced file family is an owned lane.
  Note: a live same-item claim by `Jarod / swarm-w9-native-differential-matrix`
  (expires ~2026-09-20T23:27Z) already fences
  `tests/native-differential/tests/abstract_publication*`; per-host rows and
  sibling suites are likewise claimed (RC-NATIVE-MATRIX-MACOS-ARM64 legs,
  RC-RELEASE-RECORD-AND-CLOSURE, BASELINE-NATIVE-DIFF-*). Coordinate before working
  it.

  Re-verified at `e7c0099cb2b7` (zergling-182, linux x86-64): the noted
  abstract_publication fence has lapsed and its surface is green —
  `cargo nextest run -p omega-native-differential-test --test
  abstract_publication --no-fail-fast` = 56/56 PASS, including the
  `decision_custody` family RC-RELEASE-RECORD-AND-CLOSURE recorded as
  uncompilable at `f1675418b1` (the `PSI_PASS_CATALOG` 7th-entry and
  `optimized_target()` custody drift are repaired upstream). The harness
  now compiles (`cargo check --all-targets` clean). Still fenced by
  siblings: `pipeline_ownership*` under BASELINE-NATIVE-DIFF-PIPELINE-
  OWNERSHIP, per-host evidence rows under the RC-NATIVE-MATRIX-* lanes.
  **`pipeline_ownership` re-measured 2026-09-21 (macOS arm64): 392/392.** The
  fence this row recorded on that harness is gone and the leg no longer excludes
  itself from the matrix -- it was one of the two binaries the linux row could
  not build, and both now compile.

- **NEW-APR-TRAPPING-SHIFT-REFUSAL-PIN.** Inserted row — minted name (planner
  scope: `tests/omega/fail/arithmetic/trapping_shift_requires_realization` +
  `canary_suite/roster.rs`). Scope verified at `bb192d7ea9eb` on linux
  x86-64: the refusal it names is live — `validate_total_specification_arithmetic`
  (`proof_contracts/arithmetic_domains/total_specification.rs:241`) rejects a
  shift whose LEFT operand selects `in Trapping` inside any contract/proposition
  term ("direct Trapping arithmetic `<<` is illegal in machine `...` requires
  contract: specification terms are total and cannot transfer runtime control"),
  and no existing canary pins the shift-shaped leg (fail/ranges Trapping hits
  are carrier annotations, not contract-term arithmetic). The pin is a
  checked-semantics fail fixture (`requires (left << k) >= 1` on a `left:
  u32 in Trapping` parameter) plus one `CHECKED_ONLY_FAIL_CANARIES` entry —
  which lives in `canary_suite.rs`, not the scoped `roster.rs` (roster.rs is
  the inventory harness that consumes those arrays). Registration is fenced:
  `canary_suite.rs` held by PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION
  (devin-5389, ~16:19Z Sep 21). Landable after that claim drains.
- **NEW-ATC-ENTRY-READBACK-BOUNDARY.** Mined candidate — unresolvable name; no implementing surface found at `0a0662ad27ad`. The token `ATC` occurs as a standalone word nowhere in TASKS.md, TASKS_BOOTSTRAP.md, TASKS_OPTIMIZER.md, `wiki/`, `tools/`, `source/`, `samples/`, or any `*.rs`/`*.omg` file — verified on both this worktree and `origin/main`; the nearest read-back surface (`wiki/drafts/known_baseline_failures.md`'s field-readback fixtures and `tests/omega/pass/control_flow/runtime_straight_line_terminal_field_readback_exit`) carries no ATC naming and is already owned. Nothing exists to scope or implement under this name; recorded here so the mined name is not re-dispatched.
- **NEW-BAC-CHECKPOINT-THROUGH-EXECUTION-REQUEST.** Inserted row, scope
  verified (planner-scoped to
  `assembled-syntax-to-checked-compilation/src/checking.rs`,
  `checking/checked_compilation.rs`,
  `checking/execution_settlement.rs`) — the admitted-checkpoint-to-
  execution-request contract is already implemented and pinned:
  `AdmittedBuildCheckpoint::execute` (build_continuation.rs:290) captures
  `restricted_build_requests` before the checkpoint is consumed, joins
  each request through `RestrictedBuildGrants::admit`, re-verifies the
  executed config's selected symbol against the admitted symbol, and
  `ExecutedBuildCheckpoint` carries the requests into
  `BuiltCheckedProgram` — `check_selected_execution`
  (execution_settlement.rs:325) moves them verbatim into
  `CheckedExecution.restricted_build_requests`, exposed by
  `checked_compilation.rs:625`. Coverage already lives in
  `continuation_tests.rs`: `ungranted_restricted_build_request_waits_`
  `before_its_effect` (grant refusal gates the effect, stamp never
  written) and `granted_restricted_build_request_executes_its_effect`
  (request survives to `checked.restricted_build_requests()`).
  Verified at head fetch on linux x86-64; no live claim fences the three
  scoped files.
- **NEW-BENCHMARK-RECORD-NOTE-DISCIPLINE.** Inserted row — minted name
  (planner scope: `tools/benchmark/records/structural_proofs__linux_x86_64__default.json`
  + `tools/benchmark/README.md` + `wiki/drafts/benchmarks.md`). Scope verified
  at `52d95a9d76f4` on linux x86-64: mines the field-note ruling at
  `wiki/drafts/benchmarks.md:202` (record `notes` must describe the
  measurement — subject/leg/host — not the lane that ran it; re-measurement
  of an existing cell is a refresh row, not a new item). The scoped record's
  note still leads with the `z125` lane label the ruling forbids, and
  README's `notes` schema row carries no discipline text; 5/10 committed
  records also have empty `notes` (describe-only discipline would let a
  re-measure legitimately leave them empty). Landed the README leg;
  the record-note rewrite and any benchmarks.md application marker are
  fenced under BENCHMARK-PROOF-SUBJECT-SELECTION (Zergling-126, ~14:19Z
  Sep 21 — both `tools/benchmark/records` and `wiki/drafts/benchmarks.md`).
  **Applied 2026-09-21.** `structural_proofs__linux_x86_64__default.json`'s
  `notes[0]` led with `z125:`, naming the lane that ran it. Per this row's own
  ruling (`wiki/drafts/benchmarks.md:210` -- keep note fields "descriptive of
  the measurement (subject/leg/host), not of the lane that ran it") the prefix is
  dropped; the descriptive remainder is unchanged. `tools/tests/test_benchmark.py`
  29/29.

- **NEW-BOARD-DUPLICATE-STUB-SWEEP.** Inserted row — sweep executed at
  `96bc0ef81043`+ head fetch (Zergling-52, linux x86-64). Removed 61
  duplicate bare `- **NAME** — mined candidate; verify scope then
  implement.` stub lines accumulated by repeated union-merge pushes,
  keeping the first occurrence of each (61 removable of 343 bare stubs
  across 282 unique names; top offenders were triple-copied
  BUILD-DIRECTORY-*/RC-* rows). Policy: identical text + same name =
  information-free duplicate; `**NAME.**` content rows and differently
  worded stubs kept per keep-both-sides. The upstream cause — diff3
  `|||||||` markers landing as board content — still ships on main; each
  downstream worker strips them on rebase.
  Follow-up sweep at `bb192d7ea9` (linux x86-64, DMS9 tranche):
  retired 176 bare `- **NAME** — mined candidate; verify scope
  then implement.` stubs whose name already has a verdict-bearing or
  scope-verified content row elsewhere on the board — the bare line is
  information-free under the same-union-merge policy; the real row keeps
  tracking the item. Stubs with no same-named row, and non-bare
  "mined candidate; scope verified" mini-rows, are untouched.

- **NEW-LSC-MULTI-SOURCE-LIFETIME-LEAVES.** Inserted row, scope verified at `b868b9ee8f27` (planner-scoped to `typed-trees-to-checked-trees/src/borrow/view_link.rs`) — the multi-source lifetime-leaf machinery is already implemented in that file: `structural_view_return_source` enumerates input leaves via `carried_lifetimes`, an elided output requires exactly one leaf across the frontier (`ElidedMultipleInputs` at `matching.len() != 1`, covering one parameter carrying several unnamed sources), an explicit output lifetime emits one `ViewReturnFieldSource` per matching leaf. **SUPERSEDED — this row was inserted after its own blocker was already gone.** `9106b1ca03725` ("psi: explicit result lifetime unions same-lifetime inputs as view sources") landed the multi-source leg inside `view_link.rs`: an explicit result lifetime now links *every* input carrying the name and emits one `ViewReturnFieldSource` per matching leaf across inputs (`view_link.rs:256-285`, comment at :257-259). `LifetimeMatchesMultipleInputs` and its diagnostic have zero hits in any `.rs` file; the only multi-match rejection left is `ElidedMultipleInputs`, guarded by `output.lifetime.is_none() && matching.len() != 1` (:246-248). The sibling row at :12926 already calls the variant retired. No slice remains here. No live fence covers the file; the cross-file leg needs its own dispatch with `elision.rs` + `loans.rs` in scope.
- **NEW-NATIVE-DIFF-IGNORED-OPERAND-PROBE-INTENT.** Inserted row, scope
  verified (planner-scoped to `tests/native-differential/tests/real_fs.rs`)
  — the ignored-operand probe intent is already implemented and green:
  `IgnoredOperandProbe` inside
  `filesystem_operands_prepare_before_real_authority` calls
  `fs.create(path, (dividend / divisor) as i32)` with a trapping divisor,
  and asserts the operand evaluation halts (`EvaluationHalted(Trap)`,
  operation_tag 1, zero grant refusals) before `create` can touch disk —
  "ignored ABI operands must finish preparation before create can touch
  disk" (`!prepared_file.exists()`). Re-witnessed green at head fetch:
  `nextest -p omega-native-differential-test --test real_fs -E
  'test(~filesystem_operands_prepare)'` PASS. Note the suite's
  "native-differential" name describes the real-vs-hermetic filesystem
  split — probes run under the checked interpreter (`interpret_with_options`
  / `evaluate_granted_build_machine_arguments`), not a produced native
  artifact; an actual native-binary twin of the trap-ordering intent would
  need the artifact-production harness, which this file does not own.
  Sibling surfaces: the neighboring grant probes (InvalidOutputProbe,
  CanonicalizeOutputProbe, CrossDomainProbe) pin the remaining
  preparation-intent quadrants in the same test.
- **NEW-RBRA-MIGRATION-RECIPE.** Inserted row — slice landed. Planner-scoped
  to `wiki/drafts/range_suffix_migration.md`, which did not exist: authored it
  as the migration recipe for REMOVE-BRACKETED-RANGE-ANNOTATIONS (:62) — a
  position-by-position decision table (`-> T [lo..=hi]` → `T in D` or `-> T`
  + `ensures`; `x: T [..]` → `T in D` or `T` + `requires`; exclusive and
  receiver-dependent endpoint forms; extent/const positions reject to const
  extents), the per-file procedure, and the corpus-first ordering against the
  measured 1,598 occurrences / 544 files. The `RBRA` series names the
  REMOVE-BRACKETED-RANGE-ANNOTATIONS legs (NEW-RBRA-EPSILON-PARSER-RETIREMENT,
  NEW-RBRA-STD-LIBRARY-MIGRATION, NEW-RBRA-PASS-RECAST-GENERICS,
  NEW-RBRA-FAIL-DEPENDENT, NEW-RBRA-PASS-TERMINATION); this is the recipe the
  corpus migration executes against.
- **NEW-RBRA-STD-LIBRARY-MIGRATION.** Inserted row, scope verified at `891194236afa` (planner-scoped to `source/library/std/{console,time,calling}.omg` + `source/library/std/targets/{linux_x86_64,linux_arm64,windows_x86_64,macos_x86_64}`) — no migration is pending on the scoped surface: every assigned path is byte-identical between this worktree and `origin/main` (empty `git diff --stat` per file/dir), and the std library already spells the current `Service<R>` carrier vocabulary (`time.omg:951` `host: Service<TimeHost>`; bare boundary-trait value spellings reject under `32f5182254`). The `RBRA` token occurs nowhere in the tree or boards; the only sibling in the series is NEW-RBRA-PASS-RECAST-GENERICS, which holds `tests/omega/pass/{recast,generics}` (15:22Z) — the corpus side of whatever migration the series names. Nothing to implement under this name until a concrete contract or failing customer identifies the delta.
- **NEW-TLBR-PARAMETERIZED-REQUIREMENT-ADMISSION.** Inserted row, scope verified at `c3dd8016a74d` (planner-scoped to `psi/semantics/validation/src/machine_calls/calls/generic_bounds.rs`, byte-identical to origin/main) — the parameterized-requirement admission frontier is `is_directly_callable_top_level_requirement`: a top-level `boundary requirement` may be body-called only when public, nongeneric (`lifetime_parameters.is_empty()` AND `machine_type_parameters(callee).is_empty()`), single-state, and self-free or owned-self; generic/lifetime-parameterized requirements deliberately keep the symbol fence ("receiver custody and obligation transfer are a separate settlement shape"). Widening the predicate is not a slice inside this file: it decides which bodyless symbols may execute, which requires the selected-provider settlement to answer a generic instantiation plus the lifetime-linked return frontier — machinery in selected-dispatch/provider-planning, not validation. The instantiation-bound machinery that an admitted parameterized call would need (`validate_type_parameter_instantiation_bounds` positional pinning + `type_satisfies_declared_property`) already exists and is exercised through the resolved-target rung. No bounded slice remains under the assigned file; the cross-file leg needs a dispatch that includes selected-dispatch's provider resolution.
- **NEW-UPPER-KEBAB.** Resolved — minted name for the uppercase-rejection
  leg of canonical kebab-case package admission
  ([sources.md](wiki/spec/packages/sources.md#requester-local-graph): a
  dependency declares its own canonical package name; default aliases
  convert kebab-case to snake_case). Already enforced and witnessed:
  `PackageName::parse`
  (`omega-rust/omega/packages/manager/src/declarations/identity.rs:9`)
  routes through `build_declarations::ProjectName::parse` and
  `package_names_require_canonical_kebab_case_and_reject_spoofs`
  (`declarations/identity_tests.rs:15`) rejects the uppercase form
  `Arithmetic-kernels` alongside `_`-substitution, edge/double dashes,
  dots, leading digits and the Cyrillic lookalike `arithmetіc-kernels`;
  the declarations read path emits the "must use canonical kebab-case
  spelling" diagnostic (`dependencies/read/error.rs:135`). The snake_case
  counterpart is witnessed by
  `aliases_require_canonical_snake_case_identifiers`. Verified on linux
  x86-64 at `c924529921dd`: nextest `-p package-manager` on both identity
  tests — 2/2 PASS. No slice remains under this name.
- **NON-X86-LAYOUT-RELAXATION** — mined candidate; scope verified at `8734480a01`, no authorized implementation surface. The only function-relative layout rule in the catalog is `X86RelaxConditionalBranchesToRel8V1`, deliberately `Architecture::X86_64`-scoped: selecting it for AArch64 is an explicit `UnsupportedTarget` rejection, not a silent skip (`resolved-layout-to-resolved-layout/src/x86_branch_relaxation/catalog.rs`). Non-x86 branch encodings are single fixed-width forms — there is no short/long rel8-style pair to relax between — and out-of-range AArch64 targets reject at sequence emission (`isa-aarch64/src/hosted_sequences.rs` "target is out of range"). A veneer/trampoline mechanism for >±1MB conditional branches is a different mechanism named only by `machine_state_evidence.md`'s final-artifact validation list; it needs an authorizing spec and a concrete failing customer before it is an item.
- **NON-X86-LAYOUT-RELAXATION.** — mined candidate; scope verified at `8734480a01`, stamp refreshed `138ed79a677` (facts
  unchanged at HEAD), no authorized implementation surface. The only function-relative layout rule in the catalog is `X86RelaxConditionalBranchesToRel8V1`, deliberately `Architecture::X86_64`-scoped: selecting it for AArch64 is an explicit `UnsupportedTarget` rejection, not a silent skip (`resolved-layout-to-resolved-layout/src/x86_branch_relaxation/catalog.rs`). Non-x86 branch encodings are single fixed-width forms — there is no short/long rel8-style pair to relax between — and out-of-range AArch64 targets reject at sequence emission (`isa-aarch64/src/hosted_sequences.rs` "target is out of range"). A veneer/trampoline mechanism for >±1MB conditional branches is a different mechanism named only by `machine_state_evidence.md`'s final-artifact validation list; it needs an authorizing spec and a concrete failing customer before it is an item.
  Alias: **AARCH64-BRANCH-RELAXATION** names this same surface — no separate
  board row exists at `2e1db3ba3e`; both pins re-verified there
  (`x86_rel8_selected` rejects `Architecture::Aarch64` as `UnsupportedTarget`
  in catalog.rs, and `hosted_sequences.rs` emits the out-of-range
  diagnostic). Dispatch it here — already resolved.
- **GEOMETRY-REGION-ALIGNMENT-EXPANSION** — mined candidate; verify scope then implement.
- **HOSTED-BUILTIN-SETTLEMENT-EXPANSION** — mined candidate; verify scope then implement.
- **HOSTED-PLATFORM-RUN-MATRIX** — resolved as already landed; mines the landed BENCHMARK-HOST-ROW-MATRIX row (ee30bfcf1865). `benchmark.py matrix` renders the hosted-platform run matrix: `HOST_LEGS` enumerates every catalogued `TargetProfile` host leg — linux_arm64, linux_x86_64, macos_arm64, macos_x86_64 (structurally blocked pending native realization), windows_x86_64 (peak-RSS leg explicit-unavailable, no os.wait4), uefi_x86_64 (runtime leg unavailable pending QEMU/hardware), plus cross_platform_cli, local_unchecked, and alpha_bootstrap — and `matrix_rows` emits one measured row per committed record plus one explicit row per uncovered leg, so no host leg is implied. `tools/tests/test_benchmark.py` pins TargetProfile drift (21 tests pass); `wiki/drafts/benchmarks.md` renders the matrix. Residual record-row authorship belongs to the fenced tools/benchmark owners, not this stub.
- **HOSTED-RECEIVER-SERVICE-CARRIER** — mined candidate; verify scope then implement.
- **HOSTILE-SHARED-MEMORY-PLACEMENT** — mined candidate; verify scope then implement.
- **HOSTILE-SHARED-MEMORY-REMAPPING** — mined candidate; verify scope then implement.
- **INDEXED-OPERAND-ATTACHED-RECEIVER** — resolved as already landed; mines the resolved BASELINE-T2C-INDEXED-OPERAND-ACCESS row (:5641). `receiver_self_match` (`typed_trees/declarations/operator/indexing.rs:88`) routes indexed operand zero through the attached-receiver loan at HEAD, so `machine [] Buffer::index(&self, ..)` admits a `Buffer` place exactly as `buffer.at(index)` borrows it, and ordinary first parameters gain no receiver adaptation (`ordinary_first_parameter_gains_no_receiver_adaptation` control). Sibling stub INDEXING-ATTACHED-RECEIVER-BORROW (:6033) mines the same row.
- **INTEGER-COMPARISON-OCCURRENCE-PRODUCER** — resolved as already landed. The producer (emission `selected_integer_comparisons` row → lowered roster → `boundary_operator_custody/integer_comparisons` replay → `integer_comparisons::associate` proposals → coverage census) covers std plumbing at HEAD: `benchmark.py measure` over `cli_mvp`/linux_x86_64 with `--accept-admissions` compiles and publishes native output — the e48558bd41 rejection `Terminal proposal must retain every integer comparison occurrence exactly once` no longer fires. Residual row ownership for new benchmark records is `PRIME-COUNTER-BENCHMARK-ROW`'s claim; sibling stubs -COVERAGE/-RETENTION/-STD-COVERAGE mine the same row.
- **INTEGER-COMPARISON-OCCURRENCE-RETENTION** — resolved as already landed; same row as INTEGER-COMPARISON-OCCURRENCE-PRODUCER (see its note at :6036). Retention is the custody leg of that same producer chain: `boundary_operator_custody/integer_comparisons` replays each emitted occurrence strictly and the coverage census rejects any comparison occurrence not retained exactly once, so occurrence retention through the terminal artifact is enforced at HEAD (proven by the `cli_mvp`/linux_x86_64 `benchmark.py measure` compile leg publishing native output at ff782bdf21).
- **LEGACY-COMPATIBILITY-WRAPPER-PRUNING** — mined candidate; verify scope then implement.
- **LOWERED-BOUNDARY-BYTE-BUFFER-FAILURES** — mined candidate; verify scope then implement.
- **MATCHING-LOGIC-SLICE-COMPARISON** — mined candidate; verify scope then implement.
- **MATCHING-LOGIC-VERTICAL-SLICE-COMPARISON** — mined candidate; verify scope then implement.
- **MODEL-FREE-CANDIDATE-SEARCH** — mined candidate; verify scope then implement.
- **MODULE-CONSTANT-BUILTIN-CARRIER** — mined candidate; verify scope then implement.
- **MULTI-TARGET-BATCH-MANIFEST** — mined candidate; verify scope then implement.
- **NATIVE-DIFF-HOSTED-RECEIVER-HARNESS-MIGRATION** — mined candidate; verify scope then implement.
- **NATIVE-I32-REMAINDER-LEGALIZATION** — mined candidate; verify scope then implement.
- **OBLIGATION-NORMALIZED-IDENTITY** — mined candidate; verify scope then implement.
- **PACKAGE-ADMISSION-PROJECTION-EARLIEST-FACTS** — mined candidate; scope
  verified, covered — re-mines the review-projection input-resolution clause
  in `wiki/spec/packages/review.md` ("read each fact from the earliest
  coherent compiler-owned representation that establishes its meaning... only
  final findings enter comparison"). The admission projection already
  satisfies it: `packages/review/evidence/src/capture/` is exclusively
  `project_checked_*` — every projected fact (callable, calling, package,
  boundary-application, selected-provider, representation, conformance, and
  terminal-permission policies) consumes the checked representation, and
  `capture/package/mod.rs` refuses standalone/target-free compilations and
  missing checked facts rather than sourcing them late. Typed/resolved state
  owns structural identity on the same rows as the resolved
  PACKAGE-EVIDENCE-* siblings; the genuinely unfinished ledger joins
  (certificates, transitive open obligations, schema migration, admission
  decisions in `src/ledger/obligation_ledger.rs`) stay named under
  PACKAGE-PROJECTION-EVIDENCE-MIGRATION, not here. No independent slice.
- **PACKAGE-ADMISSION-PROJECTION-EARLIEST-FACTS.** Scope verified at
  `d7f3c43e302` — re-mines the fact-source rule of
  `wiki/spec/packages/review.md:46` + `acceptance.md:126` ("read each
  fact from the earliest coherent compiler-owned representation that
  establishes its meaning; typed/resolved state owns structural identity,
  checked facts own acceptance/effects/proof/witnesses/assumptions").
  That rule is landed in `packages/review/evidence`'s capture joins:
  `src/capture/` reads authored spans from the earliest owning stage and
  rejoins checked custody (Source roles in `capture.md` — e.g.
  `body_call` keeps the authored occurrence joined to checked flow
  "before provider settlement rewrites identity", `trait_parent` keeps
  the typed parent application). Witnessed at `d7f3c43e302`:
  `cargo nextest run -p package-evidence` — 360/373 run passed before
  the 540s bound; the 13 failures are the recorded `Service<R>`-spelling
  fixture-drift family (`callable_policy` fixtures reject `in Bound` on
  `Service<ClockHost>` — "the core `Service` carrier is closed"), owned
  by the ENTRY-CONTENT-ROOTS cluster, not this surface. Write surface
  fenced at verification time: `review/evidence/src/capture` is claimed
  by PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION (dev-88738, exp 04:50Z) and
  the crate dir by PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION (z112).
  No independent slice exists.
- **PACKAGE-ADMISSION-PROJECTION-EARLIEST-FACTS.** Mined candidate —
  scope verified, covered (same verdict recorded on origin/main):
  re-mines the review-projection input-resolution clause in
  `wiki/spec/packages/review.md` ("read each fact from the earliest
  coherent compiler-owned representation that establishes its meaning…
  only final findings enter comparison"). The admission projection
  already satisfies it: `packages/review/evidence/src/capture/` is
  exclusively `project_checked_*` — every projected fact (callable,
  calling, package, boundary-application, selected-provider,
  representation, conformance, and terminal-permission policies)
  consumes the checked representation, and `capture/package/mod.rs`
  refuses standalone/target-free compilations and missing checked facts
  rather than sourcing them late. Typed/resolved state owns structural
  identity on the same rows as the resolved PACKAGE-EVIDENCE-* siblings;
  the genuinely unfinished ledger joins (certificates, transitive open
  obligations, schema migration, admission decisions in
  `src/ledger/obligation_ledger.rs`) stay named under
  PACKAGE-PROJECTION-EVIDENCE-MIGRATION, not here. No independent
  slice.
- **PACKAGE-CROSS-VISIBILITY-LOAN-ORIGIN.** Mined candidate — resolved:
  the name re-covers the cross-package-visibility loan-origin cluster
  already closed by SHARED-RECEIVER-LOAN-ORIGIN (resolved at e76d715c8e (verified base 6ef64f6dd6) —
  `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`
  stopped emitting "requires an exact retained loan origin" after the
  retained-lineage/borrow-evidence family landed). Re-verified on this
  host: all 21 `cross_package_visibility` tests pass at `dcfb595098`
  (linux x86-64) with zero loan-origin diagnostics. The umbrella items
  still naming this surface stay owned where they live:
  CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN and the
  *-LOAN-ORIGIN stubs beside this row.
- **PACKAGE-EVIDENCE-TRAIT-RESOLUTION-SCOPE** — mined candidate; scope verified, covered — same package-evidence contract surface as resolved siblings PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION and PACKAGE-PROJECTION-EVIDENCE-MIGRATION (this section). The resolution-scope leg is already implemented and pinned in `omega-rust/omega/packages/review/evidence/src/capture/`: unique-trait selection rejects non-unique/absent cases at each leg (`provider_schema.rs`, `services/authority.rs`, `calling/application/signature.rs`, `providers/policy/rows.rs`, `terminal_authority_permissions/declarations.rs` — each "has no unique exact declaring trait"), scoped per subject ordinal + selected application + lifetimes + structural arguments. The genuinely unfinished ledger joins (certificates, transitive open obligations, schema migration, admission decisions in `src/ledger/obligation_ledger.rs`) are named under PACKAGE-PROJECTION-EVIDENCE-MIGRATION, not here. Sibling stubs on the same surface: PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION.
  ("requires an exact retained loan origin for its shared receiver" stopped
  emitting after the retained-lineage/borrow-evidence family landed).
  Re-verified at `d6a0625f6b` (macOS arm64, mbx/nextest): all 21
  `cross_package_visibility` tests pass with zero loan-origin diagnostics;
  detail in `wiki/drafts/cross_package_dynamic_loan_origin.md`. No
  independent slice remains.
- **PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION.** — mined candidate; scope
  verified, covered at `c267df86ac` (linux x86-64, claim a8d7bd21 on
  `packages/review/evidence/src/capture` until 04:50Z). The scope-collision
  rejection is already implemented and pinned inside
  `capture/semantics/declarations/provider_schema.rs`: a second
  `TraitDefinition` bound to the reviewed trait's symbol is the scope
  collision, and `provider_requirement_schema` rejects it with "selected
  schema has no unique exact declaring trait" instead of picking one
  (`provider_requirement_rejects_a_scope_colliding_declaring_trait` PASS,
  alongside `provider_requirement_rejoins_its_unique_declaring_trait`).
  Same covered contract surface as resolved siblings
  PACKAGE-EVIDENCE-TRAIT-RESOLUTION-SCOPE and
  PACKAGE-PROJECTION-EVIDENCE-MIGRATION — the genuinely unfinished joins
  (certificates, transitive open obligations, schema migration, admission
  decisions in `src/ledger/obligation_ledger.rs`) are named under
  PACKAGE-PROJECTION-EVIDENCE-MIGRATION, not here. No independent slice
  exists under this stub. Unrelated base red observed in-filter:
  `calling_policy_source::inherited_requirement_retains_declaring_trait_and_concrete_parent_application`
  expects `pub boundary trait ProcedureBase<Value>` in rendered fixture
  source — preexisting drift on the base commit, outside this claim.
- **PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION.** Mined candidate;
  scope verified, covered — the name re-mines the package-evidence contract
  pair in `omega-rust/omega/packages/review/evidence/EVIDENCE_SCHEMA.md`:
  uniqueness ("Every provider conformance demand matches a distinct
  requirement demand, preserving evidence-binder presence, subject ordinal,
  trait, selected application, lifetimes, and structural arguments") and the
  overcollection bound ("Equal applications deduplicate only after complete
  equality while retaining all" distinctions). Both halves are already
  implemented and pinned in `src/capture/`: unique-trait selection rejects
  non-unique/absent cases ("selected schema has no unique exact declaring
  trait" — `semantics/declarations/provider_schema.rs`; "service authority
  has no unique declaring trait" — `semantics/services/authority.rs`;
  "calling application has no unique exact boundary trait" —
  `calling/application/signature.rs`; "lifetime partition has no unique
  declaring-trait application" — `providers/policy/rows.rs`; "accepted
  service has no unique exact trait" —
  `terminal_authority_permissions/declarations.rs`), and deduplication runs
  only on completely-projected sorted rows
  (`providers/application_realizations.rs` "deduplicates only equal complete
  semantic rows", `contracts/facts.rs`, `contracts/propositions/evidence.rs`,
  `representation.rs` dedup keyed on full row + declaration identity).
  Tests pin both directions: `tests/boundary_supply/static_telescopes.rs`
  rejects a duplicated provider demand refining one requirement demand, and
  `src/capture/calling/application/signature/inheritance/tests.rs` keeps
  distinct trait lifetime binders collecting. The genuinely unfinished
  ledger joins (certificates, transitive open obligations, schema migration,
  admission decisions — `src/ledger/obligation_ledger.rs`) are named under
  the sibling resolved stub PACKAGE-PROJECTION-EVIDENCE-MIGRATION, not
  here. Sibling stubs on this surface:
  PACKAGE-EVIDENCE-TRAIT-RESOLUTION-SCOPE, PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION.
- **PACKAGE-PROJECTION-EVIDENCE-MIGRATION.** — mined candidate; scope verified, no independent slice — the name conflates two owned surfaces: the ordinary package-review obligation ledger's unfinished **schema migration** join (`omega-rust/omega/packages/review/evidence/src/ledger/obligation_ledger.rs` lists it beside certificates, subjects, and admission decisions as a separate unfinished join of the ledger row set), and the **contract/bundle encoding migration** that `EVIDENCE_SCHEMA.md` reserves to PROOF-CONTRACT-MIGRATION ("Contract/bundle migration must preserve exact occurrence, substitution, law/member, and witness joins; replacement encodings remain `PROOF-CONTRACT-MIGRATION` work"). Executable evidence projections and nested executable machine applications are explicitly not admitted by adding a review row, so no local implementable slice exists here. Sibling stubs on the same surface: PACKAGE-EVIDENCE-TRAIT-RESOLUTION-SCOPE, PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION, PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION.
- **PACKAGE-PROJECTION-EVIDENCE-MIGRATION** — mined candidate; scope verified, no independent slice — the name conflates two owned surfaces: the ordinary package-review obligation ledger's unfinished **schema migration** join (`omega-rust/omega/packages/review/evidence/src/ledger/obligation_ledger.rs` lists it beside certificates, subjects, and admission decisions as a separate unfinished join of the ledger row set), and the **contract/bundle encoding migration** that `EVIDENCE_SCHEMA.md` reserves to PROOF-CONTRACT-MIGRATION ("Contract/bundle migration must preserve exact occurrence, substitution, law/member, and witness joins; replacement encodings remain `PROOF-CONTRACT-MIGRATION` work"). Executable evidence projections and nested executable machine applications are explicitly not admitted by adding a review row, so no local implementable slice exists here. Sibling stubs on the same surface: PACKAGE-EVIDENCE-TRAIT-RESOLUTION-SCOPE, PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION, PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION.
- **PACKAGE-REVIEW-HOTSPOT-ATTRIBUTION** — mined candidate; verify scope then implement.
- **PACKAGE-PROJECTION-EVIDENCE-MIGRATION** — mined candidate; scope verified, no independent slice — the name conflates two owned surfaces: the ordinary package-review obligation ledger's unfinished **schema migration** join (`omega-rust/omega/packages/review/evidence/src/ledger/obligation_ledger.rs` lists it beside certificates, subjects, and admission decisions as a separate unfinished join of the ledger row set), and the **contract/bundle encoding migration** that `EVIDENCE_SCHEMA.md` reserves to PROOF-CONTRACT-MIGRATION ("Contract/bundle migration must preserve exact occurrence, substitution, law/member, and witness joins; replacement encodings remain `PROOF-CONTRACT-MIGRATION` work"). Executable evidence projections and nested executable machine applications are explicitly not admitted by adding a review row, so no local implementable slice exists here. Re-witnessed at `00f36e8cfa` (linux x86-64): `obligation_ledger.rs:116` still lists certificates, transitive open obligations, schema migration, and local admission decisions as separate unfinished joins, and `EVIDENCE_SCHEMA.md` still reserves replacement encodings to PROOF-CONTRACT-MIGRATION. Sibling stubs on the same surface: PACKAGE-EVIDENCE-TRAIT-RESOLUTION-SCOPE, PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION, PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION.

- **PACKAGE-REVIEW-ROUTE-ATTRIBUTION.** Mined candidate; scope verified at
  `0977a4249e`: the open reading is dependency-route attribution on the
  restricted-build grant join —
  [acceptance.md](wiki/spec/packages/acceptance.md#restricted-build-acceptance)
  requires each request's originating package *and dependency path*, and
  `UngrantedRestrictedBuildRequest`
  (`manager/src/review/restricted_build_grants.rs`) reports package identity,
  purpose, and request meaning but not the occurrence's route. The machinery
  exists: `resolution/graph/reconcile/dependency_paths.rs` computes BFS
  shortest paths with requester/purpose/alias/target steps,
  `decision/policy/document/render.rs` already renders `- path`/`+ path`
  per package, and all three join call-sites hold the source closure
  (`check_project.rs`, `compile_project.rs`, `check_locked_sources.rs`
  under `operations/`). Slice: thread the `DependencyRequestPath` through
  the gap record and its `Display`. **Landed (z103):**
  `UngrantedRestrictedBuildRequest` now carries
  `request_path: Option<DependencyRequestPath>` — the occurrence's
  shortest root-to-package route from `dependency_paths.rs`'s BFS — and
  its `Display` renders `path <root-hex> -> "alias" <target-hex> …` in
  the decision document's form. `ungranted_restricted_build_requests`
  takes the joining `ResolvedPackageSourceClosure`; all four call sites
  (check_project, compile_project, check_locked_sources, and the
  in-compile checkpoint in package_pass.rs) supply it. Pinned by
  `supplied_host_scope_requires_exact_retained_request_and_occurrence`'s
  new route assertions; the four grant-join tests pass at this commit.
  Historical fence record: every touch point sat under live claims —
  `review/restricted_build_grants.rs`,
  `review/candidate`, and `review/decision` are fenced by Zergling-79's
  BUILD-ADMISSION-CHECKPOINT (expires ~2026-09-21T00:05Z), `operations/` by
  Jarod's TWO-AXIS-TERMINAL-AUTHORITY-REVIEW, and `manager/src/lock` +
  `manager/tests/locked_source_checking` by Devin's
  PACKAGE-LOCK-SOURCE-IDENTITY. Coordinate with those owners before working
  it.
- **PHYSICAL-ACCESS-PROFILES.** Resolved — scope verified, already landed. The
  stub names the physical-lane access-profile surface covered at `9ced81e046`
  ("backend: cover every access profile through the mixed structural rejoin"):
  `native-artifact/src/native_artifact/mixed_structural_scalar.rs` pins
  `every_access_profile_rejoins_terminal` (all four `StructuralAccess` profiles —
  Owned, SharedBorrow, MutableBorrow, WriteOnlyBorrow — rejoin the terminal
  machine) and `mismatched_access_profiles_reject`, while
  `physical/derivation/tests.rs` rejects access substitution inside the
  normalized foreign structural lane (`normalized_foreign_structural_signature_requires_the_exact_admitted_lane`)
  and argument-access mutation. Re-verified green at `cdee121ee9`:
  `cargo nextest run -p native-artifact -E 'test(/access_profile/) or test(/mismatched_access/)'`
  — 2/2 pass on linux x86-64. The remaining physical-evidence legs (dynamic-call
  and call-occurrence spans) belong to TRANSLATION-VALIDATION's named remaining
  work under DYNAMIC-CALL-OCCURRENCE-SPANS, not to access profiles.
  Re-verified at `758e8ad9e2` on linux x86-64: both access-profile pins
  intact at `mixed_structural_scalar.rs:184`/`:199` and the foreign-lane
  rejection pin at `physical/derivation/tests.rs:645`; the settled
  verdict stands (dispatcher re-dispatched the resolved name).
- **PHYSICAL-ENTRY-BRIDGES** — mined candidate; scope verified, covered — the resolved sibling PHYSICAL-ENTRY-END-TO-END row names this stub as a re-mine of the "end-to-end physical entry" note in `wiki/language_guide/chapter_3_machines.md` ("selecting and checking an entry does not claim that its native bridge has been installed"), which the note itself assigns to ENTRY-CONTENT-ROOTS. The physical-entry-bridge acceptance leg already passes natively on linux x86-64 at `cdee121ee9` (`samples_with_documented_exit_run_correctly` under `OMEGA_SAMPLE_RUNTIME_FILTER==cli__basics__number_guess`: published process with `Service<Console>` receiver compiles to a native artifact and runs to exit 70); the intrinsic `Service<R>` carrier cut landed at `f705cbdb5`. The epic's remaining bridge legs (receiver nominal-cleanup/completion occupancy, per-host legs) stay with ENTRY-CONTENT-ROOTS and are live-fenced this wave (program-entry-plan, external-roots `ProgramLocalRootInstallationLedger`, image-emission hosted_receiver). No independent slice exists here. Re-verified at `72fc66d6c32` (z203 leg): still covered and the fence roster is intact with fresh holders — STARTUP-ENTRY-MECHANICS (~15:34Z) now holds `external-roots/src/root_entry` + `image-emission hosted_unit_entry.rs`, EPOCH-RESOURCE-SNAPSHOTS (~11:32Z) holds `external-roots/src/program_local/program_local_roots`, PLAN-LAID-VIEWS (~09:25Z) holds `image-emission/src/hosted_receiver*`, EXCEPTION-ROOTS-AND-TIMER (~08:59Z) holds `entry_exit_stub.rs`, and the UEFI legs sit under UEFI-PHYSICAL-SEMANTIC-ENTRY (~08:44Z) / UEFI-OS-HANDOFF (~10:19Z). Re-verified at `53817f8759e5b` (z140 leg): both bridge commits are still ancestors of the base (`cdee121ee9` acceptance leg, `f705cbdb5` carrier cut), and the fence roster remains live — PLAN-LAID-VIEWS still holds `image-emission/src/hosted_receiver*` (~09:25Z), EPOCH-RESOURCE-SNAPSHOTS holds `external-roots` program_local surfaces (~11:32Z), and the UEFI handoff legs sit under UEFI-OS-HANDOFF (~10:19Z). No independent slice exists here. Re-verified at `fe9840771b` (z175 leg): the acceptance leg still passes natively on linux x86-64 — `samples_with_documented_exit_run_correctly` under `OMEGA_SAMPLE_RUNTIME_FILTER==cli__basics__number_guess` → PASS 86.6s. Fence map rotated since the z203 stamp: STARTUP-ENTRY-MECHANICS / PLAN-LAID-VIEWS / EXCEPTION-ROOTS-AND-TIMER / UEFI-PHYSICAL-SEMANTIC-ENTRY drained; live holders on this surface now EPOCH-RESOURCE-SNAPSHOTS (`external-roots/src/program_local`, ~11:32Z) and UEFI-OS-HANDOFF (~10:19Z).
- **PHYSICAL-ENTRY-END-TO-END.** Mined candidate — resolved: the name
  re-mines the "end-to-end physical entry" note in
  `wiki/language_guide/chapter_3_machines.md` ("selecting and checking an
  entry does not claim that its native bridge has been installed"), which
  the note itself assigns to ENTRY-CONTENT-ROOTS. Its acceptance leg now
  passes natively on linux x86-64 at `cdee121ee9`, re-verified `9ff8673b31`:
  `samples_with_documented_exit_run_correctly`
  (`OMEGA_SAMPLE_RUNTIME_FILTER==cli__basics__number_guess`) compiles
  number_guess — a published process with a `Service<Console>` receiver and
  no test-supplied `self` — to a native artifact and runs it to its
  documented exit 70. The intrinsic `Service<R>` cut also landed
  (`f705cbdb5` admits carriers by exact closed identity; `Bound` is gone
  from `core/service.omg`). The epic's remaining bullets — receiver
  nominal-cleanup/completion occupancy and the per-host legs — stay with
  ENTRY-CONTENT-ROOTS and are live-fenced this wave (program-entry-plan,
  external-roots `ProgramLocalRootInstallationLedger`, image-emission
  hosted_receiver). Sibling re-mine names on this surface:
  PHYSICAL-ENTRY-BRIDGES, PHYSICAL-ACCESS-PROFILES.

- **PLACE-ACCESS-GEOMETRY.** Resolved — scope verified, already landed. The stub
  names the geometric half of placed access ([placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement)):
  the referent geometry is the declared-carrier join plus path resolution and
  range/qualification rejoin, landed as
  `terminal-semantics/src/placed_view_referent.rs::validate_placed_view_referent`
  (declared structural carrier, path resolves through the shape graph to a real
  place, qualifications must be declared over that carrier; stale/substituted
  ranges and forged carriers reject) consumed by both execution boundaries,
  with the plan-side geometry in
  `build-time-evaluation/src/layouts/placed_views/` (policy/schema/view exact
  rejoin after typing) and `access-plans/src/placements/` (admission, custody,
  correspondence, resident/borrowed views). Re-verified green at `797e99ead7`:
  `cargo nextest run -p access-plans -p terminal-semantics -E 'test(/placed/) or test(/placement/) or test(/referent/)'`
  — 24/24 pass on linux x86-64. The unlanded placed-access legs are not the
  geometry: `bind_hosted_receiver` lending through the emitted entry shim and
  the `PlacedField` accessor realization are PLAN-LAID-VIEWS' named remaining
  work (native surfaces fenced by PLACED-ACCESS-NATIVE-OPS); the geometric
  request bound for partition routes belongs to CONSERVATION-CONTRACT /
  TERMINAL-CONTENT-CLAIMS under BUMP-ALLOCATOR-CANARY's routing.
- **PLACE-ALIAS-ANALYSIS-PRODUCER.** Landed; re-verified at `baad84f97f` —
  `analyses/semantic/place_aliases.rs:88` still exports `relation`. `AnalysisKind::PlaceAliases` now
  has a producer in `abstract-operations-to-abstract-operations`' analysis
  catalog: `PlaceAliasesAnalysis` carries each machine's complete
  declared-root roster and deduplicated verifier live-claim views (root plus
  projection path, sites as evidence), and `PlaceAliasFunction::relation`
  proves disjoint / overlapping / unknown — distinct roots are disjoint
  absent a `Referent` crossing, which along with unplaceable evidence roots
  returns `Unknown`. Consumer binding in the selected-instructions rewrites
  stays under ALIAS-AWARE-MEMORY.
- **PLACE-STORAGE-EXTENT-OWNER.** Mined candidate; scope verified at
  `0a0662ad27ad` (restamped from `797e99ead7`): names the storage-extent
  ownership surface — `Extent` (linear `base`/`length`,
  `source/library/core/extent.omg`) qualified `Extent in Granted` only
  through an owner-authorized route (`established by
  ExtentRootProvider::grant, ProgramStorageEntry::enter,
  DeviceLoanProvider::complete` — "the domain owner authorizes exactly this
  admitted root crossing"), and resident ownership `Extent::Resident<P,T>`
  over an exact placement per
  `066d3b3472` (re-verified from `797e99ead7`): names the
  storage-extent ownership surface — `Extent` (linear `base`/`length`,
  `source/library/core/extent.omg`) qualified `Extent in Granted` only
  through an owner-authorized route (`established by
  ExtentRootProvider::grant, ProgramStorageEntry::enter,
  DeviceLoanProvider::complete` — "the domain owner authorizes exactly
  this admitted root crossing"), and resident ownership
  `Extent::Resident<P,T>` over an exact placement per
  [placed_access](wiki/spec/resources/placed_access.md) /
  [chapter_20](wiki/language_guide/chapter_20_memory_layout_abi.md#admission-and-placement).
  Still unworkable — re-verified at `3a8203932783` with the live-claim
  map: most fences named in the previous audit have drained
  (PLACED-ACCESS-NATIVE-OPS, UEFI-PHYSICAL-SEMANTIC-ENTRY,
  RUNTIME-SIZED-ACTIVATION-STORAGE, HOSTILE-SHARED-MEMORY-REMAPPING,
  BORROWED-STORAGE-RESTORATION, ENTRY-CONTENT-ROOTS and
  PLACE-ALIAS-ANALYSIS-PRODUCER all lapsed), but the implementing
  surfaces are still co-held: the placed-access route's
  `placed_view_referent` leg stays under PLAN-LAID-VIEWS (~09:25Z
  Sep 21), `psi/foundation/extents` is re-fenced under
  DEVICE-EXTENT-ACCESS (`lib.rs` + `ordering_events`, ~11:04Z Sep 21)
  and NEW-ATC-PROVIDER-CONFORMANCE-STANDIN (`mapping`, ~16:35Z Sep 21),
  and native-realization is partially held by
  BUILD-EXCLUSION-REALIZATION (~15:52Z Sep 21). `extent.omg`,
  terminal-psi ownership/placement, and access-plans
  `owned_placement_lifecycle` are unfenced, but no end-to-end slice
  escapes the held files. Sibling re-mines on this family: PLACE-ACCESS-GEOMETRY,
  PLACED-ACCESS-NATIVE-OPS (claim lapsed), plus the entered-extent siblings
  Currently unworkable — every implementing surface sits inside live
  fences this wave: the extents crate itself (`psi/foundation/extents`)
  is under DEVICE-EXTENT-ACCESS (11:04Z), the placed-access route
  (hosted_receiver + access-plans legs) under PLAN-LAID-VIEWS (09:25Z),
  the alias-analysis crate under PLACE-ALIAS-ANALYSIS-PRODUCER (10:06Z),
  and the UEFI arrival leg under UEFI-PHYSICAL-SEMANTIC-ENTRY (08:44Z);
  RUNTIME-SIZED-ACTIVATION-STORAGE-CONTRACT (09:09Z) and SNAPSHOT-STORAGE
  (15:21Z) hold adjacent board/storage rows. The prior ENTRY-CONTENT-ROOTS
  and PLACED-ACCESS-NATIVE-OPS fences have since drained, but no unfenced
  slice remains. Sibling re-mines on this family: PLACE-ACCESS-GEOMETRY,
  PLACED-ACCESS-NATIVE-OPS, plus the entered-extent siblings under
  ENTRY-CONTENT-ROOTS.
- **PLATFORM-RUN-LINUX-X86-64** — mined candidate; verify scope then implement.
- **PLACED-ACCESS-NATIVE-OPS.** Realize the native indexed primitive store
  handed off by **WRITE-ONLY-BORROW**. The checked producer, Terminal verifier,
  codec/interpreter and verified abstract inventory already carry the runtime
  index, array path, stored value and bounds through optimization. Target
  lowering in
  `abstract-operations-to-target-operations/src/lowering/control_flow/operations.rs`
  still rejects `WriteOnlyIndexedPrimitiveStore` with
  `UnsupportedWriteOnlyPrimitiveStore`.

  Recover the original borrowed parameter address, scale by the element width,
  and preserve independently checked bounds and access authority through the
  ordinary target route. Start with
  `tests/native-differential/tests/terminal_psi_indexed_receivers/indexed_stores.rs`:
  `declared_range_runtime_index_store_reaches_verified_abstract_inventory`
  currently expects that native refusal. Coordinate its source fixture with
  **REMOVE-BRACKETED-RANGE-ANNOTATIONS**: use contract/domain facts, not another
  admission of the revoked annotation syntax.

  Acceptance: source-driven native `&write` and `&mut` calls mutate the
  caller-selected element and leave its neighbors untouched; invalid indices,
  substituted bounds or access authority reject. **PLAN-LAID-VIEWS** separately
  owns provider-backed view establishment and access-plan realization; this
  indexed-store delivery does not complete that larger contract.
- **PRIME-COUNTER-BENCHMARK-ROW.** — mined candidate; verify scope then implement.
- **PRIVATE-PRODUCER-EVIDENCE-LOAN-ORIGIN.** Mined candidate; scope verified —
  resolved re-mine of the same cross-package dynamic loan-origin cluster
  closed by SHARED-RECEIVER-LOAN-ORIGIN at `e76d715c8e`: the stub's surface
  is private producer-selected evidence retained across the package
  boundary, pinned by
  `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`
  and
  `cross_package_visibility::quotient_formation_retains_selected_evidence_as_private_package_custody`
  ("requires an exact retained loan origin" stopped emitting after the
  retained-lineage/borrow-evidence family landed). Re-verified at
  `d6a0625f6b` (macOS arm64, mbx/nextest): all 21 `cross_package_visibility`
  tests pass with zero loan-origin diagnostics; detail in
  `wiki/drafts/cross_package_dynamic_loan_origin.md`. No independent slice
  remains.
  `psi/representations/terminal-psi/terminal_module/control_flow/
  termination.rs` has no external-completion terminator (8 variants,
  exhaustive matches repo-wide), and the terminal-codec deliberately
  writes/rejects the terminal-external group with count zero until the
  source-to-verifier migration retains the terminal transfer (per its
  README fence note). The slice is inherently the parent's cross-stage
  terminator + codec + verifier + interpreter + realization leg — no
  file-local slice exists; coordinate with the parent owner lane.
- **PRIME-COUNTER-REMAINDER-LEGALIZATION.** Mined candidate — resolved:
  the stub re-mines the remainder-legalization half of the resolved
  PRIME-COUNTER-I32-REMAINDER row (swept at `50559da3ab9b5`). Landed at
  `3c1ead6df4`: non-u64 exact divide/remainder now select the signed i64
  entries (`ExactDivideI64`/`ExactRemainderI64` — bare `cqo;idiv` on
  x86-64, `sdiv`/`msub` on AArch64), so `samples/cli/arithmetic/prime_counter`'s
  `i32` remainder legalizes to a native artifact. Re-verified on linux
  x86-64 at `6d00135b89` (i32 `-17 % 5` canary runs to exit 70; the
  sample suite drives prime_counter to exit 8) and `ExactRemainderI64`
  still selected at `f44a1177ed`. The measured-row residual belongs to
  BENCHMARK-PRIME-COUNTER-ROW (fenced: `tools/benchmark` under
  BENCHMARK-ROW-RESUMPTION and that row's own claim). No independent
  slice remains under this name.
- **PRODUCER-CHECKER-BOUNDARY-AUDIT.** mined candidate — resolved: re-mine of the producer/checker seam family already bounded in `wiki/drafts/producer_checker_decision_sharing_audit.md` (verified `12dea522b2`; the ledger names this cluster explicitly — BOUNDARY-AUDIT, DECISION-SEPARATION, DECISION-SHARING-AUDIT, SHARING-AUDIT are the same seam). Every reachable surface is Re-derived or Bound: lock decisions are informational history bound to `changes.fingerprint()`, PCC claim fields are recomputed by `verify_pcc_claim_fields` under the receiver's policy, component descriptions are "trusted for nothing" (re-decode + consumer-supplied admission profile), placed-image evidence extents/digests/seals are re-derived from committed bytes. Re-witnessed at `54e321bdf0`: `derivation_cache.rs:154` still re-runs `candidate.verify()` through the admission kernel on every hit (rejected hits fall through to fresh derivation, counted in `rejected_candidates`), and `independent_components.rs:54` `verify_independent_component_descriptions` still re-verifies under the build's own admission profile. Open residual (exhaustive whole-tree verifier-callsite audit) is recorded on sibling PRODUCER-CHECKER-DECISION-SHARING-AUDIT, not here.
  unworkable — the originally recorded `source/library/std/process_exit.omg`
  package-wiring fence has expired, but the cross-stage route it would feed
  stays claimed: PLACED-ACCESS-NATIVE-OPS (Jarod, ~2026-09-21T06:27Z)
  nominally spans the whole route (terminal-psi src, terminal-interpreter,
  lowered→terminal and terminal→abstract pipelines, native providers,
  backend layout) via a single space-joined path entry that `conflicts()`
  does not match — treat that as claimed intent, not a free window.
  Adjacent surfaces are also live-fenced: TPR6 (~08:07Z) holds
  typed-trees-to-checked-trees checks/termination, PCC-CANONICAL-SEMANTIC-
  LEDGER (~06:20Z) holds terminal-verifier reconstruction,
  STRUCTURAL-UNIT-CALL-GRAPH-JOINS (~04:33Z) and
  GENERAL-CYCLIC-EXECUTION-OPTIMIZER (~03:48Z) hold the
  terminal-psi-to-abstract lowering legs, TWO-AXIS-TERMINAL-AUTHORITY-
  REVIEW (~04:54Z) holds the realization authority policy, and
  BASELINE-NATIVE-DIFF-TERMINAL-PSI-SOURCE (~08:34Z) holds the
  terminal-Psi differential harness. The slice is inherently the parent's
  cross-stage terminator + codec + verifier + interpreter + realization
  leg, not a file-local edit. Coordinate with the parent item's owner
  lane before working it. Re-verified at `d7d4a88331` on linux x86-64:
  `termination.rs` still carries exactly the nine `Terminator` variants
  (Jump, Conditional, StructuralCase, Return, ReturnUnit,
  ReturnUnitPartialAffine, ReturnUnitNominalAffine, ReturnStructural,
  Crash) with no external-completion form, and
  `terminal_trace_v1_profile.rs` still rejects nonzero
  external-termination counts. The verdict stands.
- **PRODUCER-CHECKER-DECISION-SEPARATION** — mined candidate; verify scope then implement.
- **PRODUCER-CHECKER-DECISION-SHARING-AUDIT.** — mined candidate; bounded audit at `8734480a01`, no decision sharing found on the named reuse surfaces. `proof/src/checker/derivation_cache.rs` retains only kernel-accepted certificates and every consult re-runs `candidate.verify()` through the admission kernel — a hit is a re-checked reuse, not a trusted verdict (hits rejected by the kernel fall through to fresh derivation). `component-description`'s `verify` re-derives subject/schema/entries/custody/assumptions from bytes with the expected subject caller-supplied (substitution tests prove independent replay). `build-evaluation/src/provider_settlement/independent_components.rs::verify_independent_component_descriptions` re-verifies every attached description under the build's own admission profile, never the producer's accept. PCC admission replays normalized rows against closed target specs per `machine_state_evidence.md`. Residual: an exhaustive whole-tree audit of every verifier callsite is open, but the four decision-adjacent reuse mechanisms are each independently checked. Whole-tree callsite pass at `a4d396d0de467` (linux x86-64): every verifier entry point is consumer-side re-decision — `verify_mathematical_certificate` (`terminal-codec` `mathematical_certificate_wire` decode callsites), `verify_pcc_claim_fields` + `verify_terminal_artifact_proof` (`proof_sidecar` decode), `verify_psi_proof_sidecar` (`compilation-report` `pcc.rs`/`executable_publication.rs` pre-install re-verify), `verify_independent_component_descriptions` (review replay under the build's own admission profile). All 23 non-test `.verify(` callsites classified: 13 packages/manager transaction/directory calls are consumer self-consistency re-checks (same-directory identity + held mutex), 6 acquisition-traversal calls sit inside `#[test]`, 4 proof/checker calls are the kernel re-run already audited. Producer-flag reads: `VerifiedTerminalModule` and its per-consumer siblings are sealed carriers minted only by `verify_module*` after `validate_module` + proof reconstruction under the caller's `AdmissionProfile` (distinct carriers prevent authority bleed); `policy_verified` on wire plans is minted in-process only after authored-policy/codec-walk agreement (disagreement is a compile error) and consumed only to label report trust class; `admitted`/`accepted` reads are the same compile's own ledgers. `CertificateStatus`/`Verdict` consumers are checker-side construction plus `publication/replay` candidate-decision re-derivation. No callsite trusts a producer verdict; the named residual is closed at this enumeration's depth (every reachable callsite class audited, not a formal completeness proof). Re-verified at `138ed79a67`:
  `derivation_cache.rs` now lives under
  `omega-rust/psi/semantics/proof/src/checker/` (same contract in its doc —
  only kernel-accepted certificates retained, every consult re-decided by
  the admission kernel, the producer never runs);
  `verify_independent_component_descriptions` still re-verifies attached
  descriptions under the build's own admission profile
  (`build-evaluation/src/provider_settlement/independent_components.rs`),
  and the `pcc.rs`/`executable_publication.rs` pre-install re-verify
  callsites remain in `compilation-report`. No callsite change that would
  reopen the closed residual.
  Re-verified at `832c55e69b7` (linux x86-64): all four mechanism pins hold
  — `proof/src/checker/derivation_cache.rs:171` still re-runs
  `candidate.verify()` through the admission kernel per consult,
  `build-evaluation/.../independent_components.rs` still re-verifies attached
  descriptions under the build's own `AdmissionProfile`, and
  `compilation-report`'s `pcc.rs:154`/`228` + `executable_publication.rs:262`
  pre-install re-verifies (`verify_pcc_claim_fields`,
  `verify_psi_proof_sidecar`) remain. Fence map: the audit draft
  `wiki/drafts/producer_checker_decision_sharing_audit.md` is under
  PRODUCER-CHECKER-BOUNDARY-AUDIT (dev-88738, ~09:27Z); none of the four
  audited source files is claimed — row text is the only writable surface.
  Sibling duplicate row below keeps its own record; verdict stands — no
  decision sharing found, residual closed at the recorded enumeration depth.
- **PROGRAM-ENTRY-SELECTION-EXACTNESS.** — mined candidate; residual slice
  resolved on the sibling row below (landed `c17107578f`, re-witnessed
  `832c55e69b`).
- **PRODUCER-CHECKER-DECISION-SHARING-AUDIT** — mined candidate; bounded audit at `8734480a01`, re-verified at `f44a1177ed` (all four mechanisms unchanged), no decision sharing found on the named reuse surfaces. `proof/src/checker/derivation_cache.rs` retains only kernel-accepted certificates and every consult re-runs `candidate.verify()` through the admission kernel — a hit is a re-checked reuse, not a trusted verdict (hits rejected by the kernel fall through to fresh derivation). `component-description`'s `verify` re-derives subject/schema/entries/custody/assumptions from bytes with the expected subject caller-supplied (substitution tests prove independent replay). `build-evaluation/src/provider_settlement/independent_components.rs::verify_independent_component_descriptions` re-verifies every attached description under the build's own admission profile, never the producer's accept. PCC admission replays normalized rows against closed target specs per `machine_state_evidence.md`. Residual: an exhaustive whole-tree audit of every verifier callsite is open, but the four decision-adjacent reuse mechanisms are each independently checked.
- **PRODUCER-HISTORY-CUSTODY** — mined candidate; verify scope then implement.
- **PROGRAM-ENTRY-SELECTION-EXACTNESS** — mined candidate; residual slice
  LANDED on main as `c17107578f` ("omega: dispatch interpreted program entry
  by bound symbol", originally `zergling/z161-program-entry-selection-exactness`);
  re-verified at `0a0662ad27`: `interpret_entry_symbol` is exported at
  checked-interpreter `lib.rs:142` and consumed at
  `omega/src/execution/compilation.rs:82`, witness
  `module_namespaces::selected_program_entry_dispatches_by_exact_symbol_not_spelling`
  present at `module_namespaces.rs:660`.
  Scope verified: per-slot/per-schema selection already resolves the bound
  machine to its exact symbol under the binding occurrence's lexical package
  (`selected_program_entry_machine` + `source_signature().machine_symbol()`);
  the last spelling-based hop was `omega run`'s interpret leg — `interpret_checked`
  passed the machine's bare name to `interpret_entry`, whose
  `find_machine_by_name` returns the FIRST same-named machine in program order,
  so a dependency's same-spelled `launch` could shadow the bound one (or vice
  versa). Fix: `checked_interpreter::interpret_entry_symbol` dispatches on the
  binding's `machine_symbol` through `BuildMachineEntry::Symbol`, mirroring the
  evaluator's existing symbol arm. Witness: `module_namespaces::
  selected_program_entry_dispatches_by_exact_symbol_not_spelling` (root
  `launch(dummy)->i32{7}` decoy vs bound dependency `launch()`); end-to-end
  `omega run --both` on a scratch two-package project — name dispatch printed
  `DIVERGENCE: native 0 vs interp 7`, symbol dispatch agrees at exit 0.
  Note: the `compiler` test binary currently requires a local stub for the
  sibling half-landed `ComponentEraJournalRoster` consumer (`2d8c5136cc9`
  after `20bd592af14`) — unrelated to this change; stub kept uncommitted.
  Re-witnessed at `832c55e69b` on linux x86-64 under this dispatch: the
  sibling consumer has since landed, so no stub is needed —
  `cargo nextest run -p compiler -E 'test(selected_program_entry_dispatches_by_exact_symbol_not_spelling)'`
  compiles clean and passes 1/1 (20.4s).
- **PROOF-AUTOMATION-WIDENING.** Verified `ac4e4eee9b`, re-verified
  `1edade1a480`: this names widening
  the bounded source-automation fragment in `validation/src/proof_contracts/
  contract_entailment.rs` (canonical integer polynomials, substitutions,
  difference-bound closure, congruence, correlated intervals, signed
  remainder bounds, accumulator self-induction, the N3 structural judge).
  Every documented next rung is live-claimed or upstream-gated: the
  inductive-gate leg is under PROOF-CERTIFICATION-BRIDGE's claim
  (`contract_entailment/inductive_judgment.rs`, exp 00:51Z), call-attribution
  widening under PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION
  (`specification_calls`/`refuted_requires`/`call_requirements`; its lease
  has since lapsed — the surfaces are unfenced at `1edade1a480` but the leg
  stays the canonical item's, and the CheckedCall selection regression it
  names is still live in the corpus),
  arithmetic/call-bounds under SIGNED-CALL-PREMISES (`arithmetic_judgment.rs`
  + `argument_tests.rs`, 22:11Z), quantifiers under
  PROOF-QUANTIFIER-AUTOMATION, and the corpus pinning dirs
  (`tests/omega/{pass,fail}/proofs`) are held by the same certification
  owner; the recorded structural next rung — injectivity decomposition of
  payload-carrying constructors — is grammar-gated upstream (struct
  literals still do not parse in contract position, and the pipeline
  grammar surfaces are fenced by DOMAIN-REFINEMENT-CHAINS and friends). No
  unmanned widening lane exists; unspec'd judgment surgery in a fail-closed
  proof engine is exactly what this board must not carry. Row consumed —
  residual is the canonical items'. Re-stamped at `a4d396d0de4` (swarm-w9-ffival): the entailment surface keeps landing under the canonical items — `c431bc813` (residue binders/conjunct guards), `e479fcbc9` (Boolean contract facts as constant arithmetic verdicts), `f491aacba` (runtime-division actuals in rank-range substitution), `874f3ab0e` (open-index operator normalization conformance).
- **CHECKED-CALL-SUBJECT-ENTAILMENT-WIDENING.** (split-of:PROOF-AUTOMATION-WIDENING)
  Own the call-attribution leg that PROOF-AUTOMATION-WIDENING,
  PROOF-QUANTIFIER-AUTOMATION and PROOFS-SUBJECT-CHECKED-CALL-SELECTION all
  route to a retired PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION row, each of them
  then concluding "no independent slice exists". The landed half attributes a
  concretely selected call's precondition to the call's exact subject
  (validation `proof_contracts/contract_entailment/specification_calls.rs`,
  beside `refuted_requires.rs` and `call_requirements.rs`). The residual is the rest of that row's own list —
  abstract signatures, domain predicates, postcondition transport of case
  membership, and induction — together with the CheckedCall selection
  regression PROOF-AUTOMATION-WIDENING records as still live in the corpus.
  Exact-subject substitution across calls is required by
  [state contracts](wiki/spec/language/state_contracts.md#mutation-and-subject-identity) and
  [dependent values](wiki/spec/language/dependent_values.md); widen only to
  what those state — this is a fail-closed judgment.

  Acceptance: a `tests/omega/pass/proofs/` case cites a callee precondition
  through an abstract signature and compiles, its `tests/omega/fail/proofs/`
  twin still rejects when the cited subject is not the call's exact subject
  (as `fail/proofs/case_call_wrong_subject` does for the concrete case), and
  `cargo nextest run -p validation` stays green.

  Re-assigned stub PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION verified at
  `138ed79a677`: the retired row's residual still routes here verbatim —
  landed half stands (`specification_calls.rs` exact-subject attribution at
  `1fc01bb690`), and the residual list is still upstream-gated: abstract
  signatures need contract-position grammar (struct literals do not parse
  there), induction sits under PROOF-CERTIFICATION-BRIDGE's live claim
  (`checks/contracts/exits` + `scalar_block_invariants`, ~10:52Z), and
  `contract_entailment/ranking_range` is under TERMINATION-RANKING-CHECKS
  (~10:50Z). The `specification_calls`/`refuted_requires`/`call_requirements`
  surfaces are unfenced but carry no authorized leg. No independent slice.
- **PROOF-INTERCHANGE-EXTERNAL-ARITHMETIC.** — mined candidate; scope
  verified: merged alias of PROOF-INTERCHANGE-IMPORT's "arithmetic import"
  covered — merged alias of PROOF-INTERCHANGE-IMPORT's arithmetic-import clause; no independent slice
- **PROOF-INTERCHANGE-EXTERNAL-ARITHMETIC.** Mined candidate; scope
  verified, resolved: merged alias of PROOF-INTERCHANGE-IMPORT's
  "arithmetic import"
  clause (the third of its 3 mined aliases, adjudicated at e76d715c8e (verified base 6ef64f6dd6)).
  Re-verified on this host at b972133cad: `proof-admission/src/admission/`
  contains only `evidence.rs`, `normalization.rs` and `recursion.rs` — no
  external import route exists — `integer_rules/` owns the internal checked
  rules (closed_integer, integer_affine, integer_cast,
  integer_forbidden_root, integer_shift), and `AcceptedProofRule` admits
  only internal checked rules plus the single trusted `SemanticAxiom`
  admission pinned by `classicality.rs`. An external arithmetic import is
  a translation or foreign-theorem admission with no independently checked
  translation, so any route runs through MATCHING-LOGIC-BOUNDED-SLICE's
  bounded comparison before a concrete design; imported rules must also
  respect the constructive/classical boundary. No independent slice exists
  here. Sibling alias stub (resolved): PROOF-INTERCHANGE-INDUCTION-CERTIFICATE.
  covered — merged alias of PROOF-INTERCHANGE-IMPORT's arithmetic-import clause; no independent slice
- **PROOF-INTERCHANGE-INDUCTION-CERTIFICATE** — mined candidate; scope verified, merged alias of PROOF-INTERCHANGE-IMPORT, which already names this clause verbatim ("External proof interchange: sort encoding, induction certificate, arithmetic import (3 mined aliases merged)"). Per the matching-logic lane's settled reading, an external induction certificate carries a translation or foreign-theorem admission with no independently checked translation, so any route runs through MATCHING-LOGIC-BOUNDED-SLICE's bounded comparison before a concrete design; `terminal-codec/tests/mathematical_certificate.rs` already covers the internal W-induction certificate end to end. No independent slice exists here. Sibling alias stub (resolved): PROOF-INTERCHANGE-EXTERNAL-ARITHMETIC.
  covered — merged alias of PROOF-INTERCHANGE-IMPORT; internal W-induction certificate already covered
- **PROOF-INTERCHANGE-SORT-ENCODING.** — mined candidate; scope verified:
  merged alias of PROOF-INTERCHANGE-IMPORT's "sort encoding" clause
  (clause (a) of its 3 mined aliases, adjudicated at e76d715c8e) — the
  one already landed by MATCHING-LOGIC-TYPED-TO-ONE-SORTED-ENCODING.
  Re-verified on this host at `0f75a052f0`:
  `tools/matching-logic-sort-encoding/sort_encoding.py` `check` behaves
  per its README on all 8 committed cases — `reference.json` exits 0 and
  the seven violation cases (`exclusive_loan_conflict`,
  `missing_definedness`, `non_injective_pair`, `sum_payload_unknown`,
  `uncertified_fixpoint`, `uninhabited_membership`,
  `widening_revision`) each exit 1 — and
  `proof-admission/src/admission/` still carries only
  `evidence.rs`/`normalization.rs`/`recursion.rs`: no route admits an
  emitted clause into a checked Omega proof, matching the tool's own
  trust note ("comparison input for the bounded-slice harness, not a
  checked translation"). Remaining acceptance is wiring the inventory
  into the bounded comparison — that surface is live-fenced twice over:
  `tools/matching-logic-sort-encoding` itself under
  MATCHING-LOGIC-TYPED-TO-ONE-SORTED-ENCODING (~05:34Z) and the harness
  `tools/matching-logic-slice` under MATCHING-LOGIC-BOUNDED-SLICE
  (~05:30Z). No unfenced slice exists under this name. Sibling alias
  stubs on the same parent: PROOF-INTERCHANGE-EXTERNAL-ARITHMETIC,
  PROOF-INTERCHANGE-INDUCTION-CERTIFICATE.
- **PROOF-QUANTIFIER-AUTOMATION.** Mined candidate; scope verified at `2e1db3ba3e` — the quantifier-automation substrate is landed and was widened today (`a3ab15b7611`): `proof/src/lemmas.rs` carries the `for all i in start..end, P(i)` shape (`ForAllInRangeFact`) with element discharge (`proves_element`/`contains_index`, covering literal, witnessed-literal, and full-extent symbolic indices), the reusable `ProofLemma`/`LemmaFact` registry (`discharging` finds the named lemma whose premises discharge a goal), and `checked-trees/proof/lemmas.rs` holds the durable representation mirror (`LemmaFacts` arena root, `QuantifiedRangeFact`). What is missing is not more vocabulary but the wiring: no check site produces a quantified fact and no entailment surface consults one — both halves are upstream-gated on the entailment surfaces delegated live this wave (`specification_calls`/`refuted_requires`/`call_requirements` under PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION, `arithmetic_judgment` under SIGNED-CALL-PREMISES, `inductive_judgment` under PROOF-CERTIFICATION-BRIDGE), and a producer of "every element satisfies the domain" facts needs the guarded-domain establishment route the same cluster owns. Per the PROOF-CERTIFICATION row's own note, unspec'd judgment surgery in a fail-closed proof engine is off-limits — no unmanned widening lane exists. No independent slice to claim here.
  Re-verified at `c267df86acb`: the substrate is unchanged in substance —
  `ForAllInRangeFact`/`proves_element`/`contains_index` and the `ProofLemma`
  registry still sit in `proof/src/lemmas.rs` (:146/:179/:190/:22), and the
  durable mirror moved to `checked-trees/src/checked_trees/proof/lemmas.rs`
  (`QuantifiedRangeFact` :40, `LemmaFacts` :66 — the row's cited path has
  drifted by one directory). A repo-wide reference scan confirms neither
  type is named outside the two lemmas modules: no check site produces a
  quantified fact and no entailment surface consults one, so the
  no-independent-slice verdict holds.
  Re-verified at `2e5d4a732471` on Linux x86-64: both lemmas carriers still
  landed (`semantics/proof/src/lemmas.rs`, `checked_trees/proof/lemmas.rs`),
  still no quantified-fact producer or entailment consumer, and every
  upstream surface remains live-fenced — `specification_calls`/
  `call_requirements` under PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION (~04:00Z)
  and PROOF-SUBJECT-CALL-SELECTION (~05:39Z), `arithmetic_judgment` under
  the SIGNED-CALL-PREMISES family (~22:38Z).
  Fence-rotation re-audit at `90df29812c` (z161, 08:18Z): the cited
  entailment claims have drained — `specification_calls`,
  `arithmetic_judgment`, and `inductive_judgment` under
  `proof_contracts/contract_entailment/` are now unfenced (only the
  `ranking_range` sibling stays claimed, by TERMINATION-RANKING-CHECKS to
  ~10:50Z), and PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION /
  PROOF-SUBJECT-CALL-SELECTION / SIGNED-CALL-PREMISES are no longer live.
  PROOF-CERTIFICATION-BRIDGE still holds `checks/contracts/exits` and
  `proofs/scalar_block_invariants` to ~10:52Z. The substrate is still
  unwired — no `proves_element`/`contains_index`/`LemmaFacts` caller in
  `validation/` or `typed-trees-to-checked-trees/` — so the surviving
  blocker is the row's substantive gate, not a fence: producing and
  consulting quantified facts is unspec'd surgery on the fail-closed
  proof engine, which the delegated PROOF-CERTIFICATION cluster owns.
  No independent slice exists under this name.
  covered — substrate landed (`a3ab15b7611`); remaining gate owned by the PROOF-CERTIFICATION cluster
- **PROOF-SUBJECT-CALL-SELECTION.** Scope verified, covered — named
  sibling stub of PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION's resolved row,
  same surface as the resolved PROOFS-SUBJECT-CHECKED-CALL-SELECTION
  (this file, adjacent row): a checked/specification call cited as a
  proof subject must
  attribute the callee's selected precondition to the call's exact
  subject. Implemented on `origin/main` at `1fc01bb690`
  (`validation/src/proof_contracts/contract_entailment/specification_calls.rs`
  checks selected concrete calls before fact intake; caller-terms
  attribution diagnostic in `typed-trees-to-checked-trees/src/checks/operators/requires.rs`);
  fail twins reject (`proofs/case_call_wrong_subject`,
  `case_citation_wrong_result`), pass twin `proofs/case_call_premises`
  compiles — filtered corpus re-verified green at `4927883cf353`.
  Remaining owners stay the parent item's own list (abstract signatures,
  domain predicates, postcondition transport, induction). No
  independent slice exists here.
  covered — sibling stub of resolved PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION
- **PROOFS-SUBJECT-CHECKED-CALL-SELECTION.** Scope verified, covered — named sibling stub of PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION's resolved row (re-verified at `4927883cf353`: filtered corpus still green — `proofs/case_call_*` fail twins reject with expected fragments, `proofs/case_call_premises` compiles), which owns this surface: a checked/specification call cited as a proof subject must attribute the callee's selected precondition to the call's exact subject. Implemented on `origin/main` at `1fc01bb690` (`validation/src/proof_contracts/contract_entailment/specification_calls.rs` checks selected concrete calls before fact intake; caller-terms attribution diagnostic in `typed-trees-to-checked-trees/src/checks/operators/requires.rs`); re-verified green at `f1675418b1` on the singular-variant row (`proofs/case_call_wrong_subject` rejects `empty_only(other)` when only `known in Tree::Empty` is established, `case_citation_wrong_result` pins the result side, pass twin `proofs/case_call_premises` compiles). Remaining owners stay the parent item's own list (abstract signatures, domain predicates, postcondition transport of case membership, induction). No independent slice exists here.
- **PROVIDER-ATTACHMENT-MACHINE-PLAN** — mined candidate; scope verified, no bounded slice this wave (z175, `500878c473f4c`). The namesake surface — `typed-trees-to-checked-trees/src/execution/unit/providers.rs` — already produces the exact `CheckedProviderAttachmentRequirementPlan` roster (`checked_provider_attachment_requirements` + the composed-leaf variant), pinned across `tests/flow/terminal_unit` and rejoined to authored call sites by c2l `unit/attached_unit/provider_attachments/source.rs`. The residual the name carries is BOUNDARY-ISSUANCE's open frontier — the provider-planning/native-settlement join to the installed occurrence — and its implementing surfaces are fenced: `external-roots/src/program_local` under EPOCH-RESOURCE-SNAPSHOTS (~11:32Z), `platform_bringup/secondary_processor` under AP-BRINGUP (~13:50Z), `execution/unit/composed_control/topology.rs` under PASS-CANARY-GUARDED-PAIR-FALLBACK (~17:33Z). Plan-side work left for this item is join design across crates, not a file-local patch. providers.rs itself is unclaimed this wave.
  covered — roster already produced in `execution/unit/providers.rs`; residue is cross-crate join design, not a bounded slice
- **PSI-FRESH-CONSTRUCTOR-CUSTODY-JOIN.** Resolved — the custody join for
  fresh (per-edge constructed) selection results is already implemented and
  pinned (re-verified at `5fdd41efd879`). `checks/multiplicity/claim_outcomes.rs`
  `claim_outcomes_for_owned_selection` mints the fresh product's claim inside
  the selection as `Established{claim_identity: Unknown, provenance:
  Unknown}` — its origin intentionally untracked — while parameter sources
  bind the caller's claim at the exact input path and local sources publish
  the established roster identity. `checks/multiplicity/owned_selection.rs`
  completes the join rules: a linear join whose edges all move the same
  consumed claim hands that claim (identity + provenance) to the
  destination; a fresh per-edge product, a distinct source on any arm, or a
  claimless leaf leaves the claim edge-dependent — no identity is minted
  implicitly. Pinned by `claim_outcomes/joins/tests.rs`:
  `conditional_result_join_binds_constructed_argument_fields` (typed
  constructor paths retain actual source claims) and the
  "forwarding and fresh construction remain distinct origins" control.
  Related but separate surfaces: premise origins for constructed results are
  TPR6's progress surface, and static constructor matching is
  PROOF-CONTRACT-MIGRATION's remaining work — neither is this row's
  seam.
- **PROOFS-SUBJECT-CHECKED-CALL-SELECTION.** — mined candidate; scope verified, covered — named sibling stub of PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION's resolved row, which owns this surface: a checked/specification call cited as a proof subject must attribute the callee's selected precondition to the call's exact subject. Implemented on `origin/main` at `1fc01bb690` (`validation/src/proof_contracts/contract_entailment/specification_calls.rs` checks selected concrete calls before fact intake; caller-terms attribution diagnostic in `typed-trees-to-checked-trees/src/checks/operators/requires.rs`); re-verified green at `f1675418b1` on the singular-variant row (`proofs/case_call_wrong_subject` rejects `empty_only(other)` when only `known in Tree::Empty` is established, `case_citation_wrong_result` pins the result side, pass twin `proofs/case_call_premises` compiles). Remaining owners stay the parent item's own list (abstract signatures, domain predicates, postcondition transport of case membership, induction). No independent slice exists here. Re-verified at `d74f2145b9` (linux x86-64): `OMEGA_PASS_CANARY_FILTER=proofs/case_call_premises` pass_canaries_compile 1/1 green; `OMEGA_FAIL_CANARY_FILTER=proofs/case_call_wrong_subject,proofs/case_citation_wrong_result` fail_canaries_reject 1/1 green. Re-verified at `ff2f489bbff` (linux x86-64): pass_canaries_compile + fail_canaries_reject under the same filters both green. Re-verified at `832c55e69b` (linux x86-64): same filtered pair still 2/2 green — no independent slice exists here. Re-verified at `836bb681a26` (linux x86-64) (z153): same filtered pair still 2/2 green — `OMEGA_FAIL_CANARY_FILTER=proofs/case_call_wrong_subject,proofs/case_citation_wrong_result` rejects with the recorded fragments and `OMEGA_PASS_CANARY_FILTER=proofs/case_call_premises` compiles; no independent slice exists here.
- **PROVIDER-ATTACHMENT-MACHINE-PLAN.** — mined candidate; verify scope then implement.
  covered — roster already produced in `execution/unit/providers.rs`; residue is cross-crate join design, not a bounded slice
- **PSI-PARAMETER-ORIGIN-LOCAL-CUSTODY.** Scope verified, resolved —
  the stub re-mines the parameter-origin vs fresh-local-origin custody
  split already landed on `origin/main` at `2091d8659302` ("psi:
  preparation and producer results have independent owners"):
  `checked-trees-to-lowered-psi/src/expression_preparation/source_custody/
  parameters/owned.rs` keeps the two lifetimes separate — affine
  parameter roots must appear verbatim in the expected no-code disposal
  roster (`"owned scalar graph disposal eligibility differs from source
  parameters"` on any drift), while each retained local producer is
  rejoined through `local_roots` (each `EstablishStructuralValue`
  result resolved back to its authored `LocalData` declaration) and
  excluded from the parameter-shaped eligibility comparison. Fresh
  local claim rows still must carry no identity/provenance/obligations,
  matching PSI-FRESH-CONSTRUCTOR-CUSTODY-JOIN's resolved row (minted
  `Established{claim_identity: Unknown, provenance: Unknown}`). Verified
  at `eab5496b9224` (linux x86-64): `cargo nextest run -p
  checked-trees-to-lowered-psi -E 'test(/owned/)'` — 157/162 pass;
  every parameter/local custody pin green (`owned_parameters` 10/10,
  `owned_scalar_graphs::{record_locals,record_stores}`,
  `owned_results` incl. `duplicate_prior_receipts_cannot_launder_a_
  fresh_origin_as_unknown` and
  `projected_parameter_roots_move_the_selected_child_with_exact_
  identity`). The 5 failures reproduce identically at pre-branch base
  `735f1774617` — preexisting wave drift in `owned_record_return_source`
  x4 (`composed Unit scalar call requires structural call custody`,
  discarded-call structural retention) + `unit_plan_omissions` x1, a
  different custody seam owned elsewhere. No independent slice exists
  under this name.

- **RECAST-SOURCE-POSITIONS.** — mined candidate; scope verified, resolved — landed at `92db61544e3` ("recast diagnostics carry the offending cast's source position"): every recast-path diagnostic attaches the authored span of the offending `as` expression via `with_source_span(program.expression_table.source_span(handle))` in `value_custody/recasts.rs` — the stray cast for the positional sweep (pinned by `fail/recast/recast_position_fenced`), the let's initializer for the unspelled reference pun, and the cast for every scalar/slice/byte-region judgment; recorded in `validation/recasts.md`. The distinct remaining leg — admitting recasts in non-`let` positions (guard operands, call arguments, nested expressions) — is the deliberately fenced deeper byte-view rung (L4/L5) in the module header, an authorizing-brief item rather than this stub's bounded scope.
  REGRESSION NOT CLOSED BY THAT LANDING (measured 2026-09-20 at `00ed2cec7c3`,
  reconfirmed at `1f7301b71020`): `92db61544e3` left nine `omega-architecture-test`
  cases red, and they are still red — `symbolic_walk_{weak_guard_spelling_refuses,
- **RECAST-SOURCE-POSITIONS** — mined candidate; scope verified, resolved — landed at `92db61544e3` ("recast diagnostics carry the offending cast's source position"): every recast-path diagnostic attaches the authored span of the offending `as` expression via `with_source_span(program.expression_table.source_span(handle))` in `value_custody/recasts.rs` — the stray cast for the positional sweep (pinned by `fail/recast/recast_position_fenced`), the let's initializer for the unspelled reference pun, and the cast for every scalar/slice/byte-region judgment; recorded in `validation/recasts.md`. The distinct remaining leg — admitting recasts in non-`let` positions (guard operands, call arguments, nested expressions) — is the deliberately fenced deeper byte-view rung (L4/L5) in the module header, an authorizing-brief item rather than this stub's bounded scope.
  REGRESSION NOT CLOSED BY THAT LANDING (measured 2026-09-20 at `00ed2cec7c3`,
  reconfirmed at `1f7301b71020`): `92db61544e3` left nine `omega-architecture-test`
  cases red, and they are still red — `symbolic_walk_{weak_guard_spelling_refuses,
- **RECAST-SOURCE-POSITIONS** — mined candidate; scope verified, resolved — landed at `92db61544e3` ("recast diagnostics carry the offending cast's source position"): every recast-path diagnostic attaches the authored span of the offending `as` expression via `with_source_span(program.expression_table.source_span(handle))` in `value_custody/recasts.rs` — the stray cast for the positional sweep (pinned by `fail/recast/recast_position_fenced`), the let's initializer for the unspelled reference pun, and the cast for every scalar/slice/byte-region judgment; recorded in `validation/recasts.md`. Re-witnessed at `00f36e8cfa` (linux x86-64): `OMEGA_FAIL_CANARY_FILTER=recast_position_fenced` under `fail_canaries_reject_with_expected_diagnostic_fragment` passes. The distinct remaining leg — admitting recasts in non-`let` positions (guard operands, call arguments, nested expressions) — is the deliberately fenced deeper byte-view rung (L4/L5) in the module header, an authorizing-brief item rather than this stub's bounded scope.
  REGRESSION CLOSED — measured red 2026-09-20 at `00ed2cec7c3`, reconfirmed
  red at `1f7301b71020`, then verified green again on linux x86-64 at
  `00f36e8cfa` (`cargo nextest run -p omega-architecture-test -E
  'test(/symbolic_walk/) | test(/boundary_ensures/) | test(/boundary_witness_)'`
  → 12/12 PASS, every named case included): `92db61544e3` had left nine
  `omega-architecture-test` cases red —
  `symbolic_walk_{weak_guard_spelling_refuses,

  recast_footprint_discharges,recast_wide_witness_refuses}`,
  `boundary_ensures_{witness_discharges_recast_footprint,
  witness_too_wide_refuses_recast_footprint,witness_survives_unrelated_internal_call,
  witness_survives_unrelated_intervening_call,equalities_couple_symbolic_recast_witnesses}`
  and `boundary_witness_survives_transitive_disjoint_boundary_frame`. The programs are
  still REFUSED, so this is a precision loss rather than an admission hole: the refusal
  now reads "cannot bound the recast offset `offset` -- the region holds 64 bytes, but no
  declared range, dominating incoming guard, or boundary-ensures witness" instead of the
  footprint refusal "would read past the buffer" the tests assert. The boundary-ensures
  witness transport stopped being found, so the offset never gets bounded and the precise
  tail-overrun diagnosis never forms. Verified pre-existing, not test drift: the same
  nine fail at clean `origin/main` with no local commits. Their assertions are correct as
  written and were deliberately left unrelaxed — relaxing them would mask the regression.
  Regression CLOSED upstream: re-verified green at `faf902cea487` on linux x86-64 —
  `cargo nextest run -p omega-architecture-test -E 'test(~symbolic_walk) or
  test(~boundary_ensures) or test(~boundary_witness)'` → 13/13 PASS (the full
  recast-witness family plus the transitive-frame and equality-coupling cases);
  the witness transport is intact at current tip and the assertions were never
  relaxed.
  and `boundary_witness_survives_transitive_disjoint_boundary_frame`. The programs are
  still REFUSED, so this is a precision loss rather than an admission hole: the refusal
  now reads "cannot bound the recast offset `offset` -- the region holds 64 bytes, but no
  declared range, dominating incoming guard, or boundary-ensures witness" instead of the
  footprint refusal "would read past the buffer" the tests assert. The boundary-ensures
  witness transport stopped being found, so the offset never gets bounded and the precise
  tail-overrun diagnosis never forms. Verified pre-existing, not test drift: the same
  nine fail at clean `origin/main` with no local commits. Their assertions are correct as
  written and were deliberately left unrelaxed — relaxing them would mask the regression.
- **RECURSIVE-ARGUMENT-OVERLOAD-DECL-DEDUP** — mined candidate; scope verified, resolved — same re-mine of the `calls/statement_call_recursive_{argument,overload}_compile` dedup surface the resolved sibling rows carry: `e5912f303a` renamed the argument fixture's local `Nat`/`add` to `Peano`/`peano_add` ending the `core/nat.omg` collision, both pass canaries re-witnessed green on linux x86-64 at `a1daf35f2e` (`OMEGA_PASS_CANARY_FILTER=statement_call_recursive_argument_compile,statement_call_recursive_overload_compile cargo nextest run -p compiler --test canary_suite entry_and_abi::pass_canary_coverage::pass_canaries_compile`, 74s), and the dedup's negative half stays pinned by `surface_and_targets::duplicate_overload_and_visibility_admissions_reject` covering `duplicate_named_machine_overload_rejected` + `recursive_argument_imported_name_collision_rejected`. No independent slice exists; this closes the name-surface sibling set the resolved rows name.
  and `boundary_witness_survives_transitive_disjoint_boundary_frame` — a
  precision loss (refusals no longer reached the footprint diagnosis), not an
  admission hole; boundary-ensures witness transport has since been restored
  upstream so the bounded-offset reasoning forms again.
- **RECURSIVE-ARGUMENT-OVERLOAD-DECL-DEDUP** — mined candidate; scope verified, resolved — same re-mine of the `calls/statement_call_recursive_{argument,overload}_compile` dedup surface the resolved sibling rows carry: `e5912f303a` renamed the argument fixture's local `Nat`/`add` to `Peano`/`peano_add` ending the `core/nat.omg` collision, both pass canaries re-witnessed green on linux x86-64 at `a1daf35f2e` (`OMEGA_PASS_CANARY_FILTER=statement_call_recursive_argument_compile,statement_call_recursive_overload_compile cargo nextest run -p compiler --test canary_suite entry_and_abi::pass_canary_coverage::pass_canaries_compile`, 74s), and the dedup's negative half stays pinned by `surface_and_targets::duplicate_overload_and_visibility_admissions_reject` covering `duplicate_named_machine_overload_rejected` + `recursive_argument_imported_name_collision_rejected`. No independent slice exists; this closes the name-surface sibling set the resolved rows name.

- **RECURSIVE-ARGUMENT-OVERLOAD-DEDUP.** Mined candidate — resolved as an alias of RECURSIVE-ARGUMENT-OVERLOAD-DECL-DEDUP: the name re-mines the same `calls/statement_call_recursive_{argument,overload}_compile` dedup surface that row carries (Peano/peano_add rename at `e5912f303a` ended the `core/nat.omg` collision; negative half pinned by `duplicate_overload_and_visibility_admissions_reject`). Re-witnessed at `9e3edc7be9a3` on Linux x86-64: `OMEGA_PASS_CANARY_FILTER=statement_call_recursive_argument_compile,statement_call_recursive_overload_compile cargo nextest run -p compiler --test canary_suite entry_and_abi::pass_canary_coverage::pass_canaries_compile` → pass (94.6s), and `OMEGA_FAIL_CANARY_FILTER=duplicate_named_machine_overload_rejected,recursive_argument_imported_name_collision_rejected ... surface_and_targets::duplicate_overload_and_visibility_admissions_reject` → pass. No independent slice exists.
- **REGION-ALIGNMENT-EXPANSION.** — mined candidate; verify scope then implement.
  covered — port landed in the squalr submodule pin (`ef6682f75f48`); app-lane residual is SQUALR-REGION-ALIGNMENT-EXPANSION's
- **RECURSIVE-ARGUMENT-OVERLOAD-DECL-DEDUP** — mined candidate; scope verified, resolved — same re-mine of the `calls/statement_call_recursive_{argument,overload}_compile` dedup surface the resolved sibling rows carry: `e5912f303a` renamed the argument fixture's local `Nat`/`add` to `Peano`/`peano_add` ending the `core/nat.omg` collision, both pass canaries re-witnessed green on linux x86-64 at `a1daf35f2e` (`OMEGA_PASS_CANARY_FILTER=statement_call_recursive_argument_compile,statement_call_recursive_overload_compile cargo nextest run -p compiler --test canary_suite entry_and_abi::pass_canary_coverage::pass_canaries_compile`, 74s), and the dedup's negative half stays pinned by `surface_and_targets::duplicate_overload_and_visibility_admissions_reject` covering `duplicate_named_machine_overload_rejected` + `recursive_argument_imported_name_collision_rejected`. No independent slice exists; this closes the name-surface sibling set the resolved rows name.
- **REGION-ALIGNMENT-EXPANSION** — mined candidate; covered — alias stub of the
  landed SQUALR-REGION-ALIGNMENT-EXPANSION port (sibling
  GEOMETRY-REGION-ALIGNMENT-EXPANSION names the same surface). The submodule
  pin advanced to `ef6682f75f48` carrying the port itself (`52bcf25`):
  `squalr-engine-api/src/structures/memory/normalized_region.omg` implements
  `NormalizedRegion::set_alignment` (forward-distance aligned-base move,
  wrapping add at the address edge) and `::expand` (saturating base subtract +
  wrapped doubling), each documented against its upstream revision pin;
  `squalr-tests/main.omg` exercises it natively in the
  `expand_grows_saturating` state (base 100→92, size 16→32, then overflow
  saturating to base 0 / size u64::MAX). Re-verified at `c924529921` by
  checking the pinned submodule's sources directly. No slice remains under
  this name.
  covered — port landed in the squalr submodule pin (`ef6682f75f48`); app-lane residual is SQUALR-REGION-ALIGNMENT-EXPANSION's
- **REMAINING-INTRINSIC-SPAN-ARMS.** Mined candidate; scope verified at
  `739e4e81e97` — resolved as documented on sibling TV-INTRINSIC-SPAN-ARMS
  (verified 14e6f8f72e): the span-arm surface is complete for every intrinsic
  family that produces coverage occurrences — IEEE FMA joins
  `x86_scalar_fma_occurrences` fragments (`derive_fma_span`), integer
  comparisons join `semantic_code_attribution` rows
  (`derive_integer_comparison_span`), float comparisons take the
  fragment-publication arm, structural returns arrive through the
  checked-body call span. Re-verified the demand side at this revision:
  `lowered-psi-to-terminal-psi/.../boundary_operator_custody/replay_scope.rs:102-105`
  replays exactly the same four families (`local_initializers`,
  `structural_returns`, `float_comparisons`, `integer_comparisons`), and the
  span arms are still at `native-artifact/src/physical/operator_applications.rs:56-58`.
  The remaining intrinsic kinds produce no occurrences, so an arm would be
  dead code joining nothing — occurrence production for those kinds is
  TV-OPERATOR-APPLICATIONS-REPLAY's scope, not this stub's. No independent
  slice exists. Fence note: `native-artifact/src/physical` is path-claimed
  under PHYSICAL-ACCESS-PROFILES and the same-surface item
  INTRINSIC-PHYSICAL-SPAN-ARMS is item-claimed (Jarod / swarm-w9) until
  ~05:04Z this wave.
  covered — span-arm surface complete per TV-INTRINSIC-SPAN-ARMS (`14e6f8f72e`)
- **REGION-ALIGNMENT-EXPANSION.** Mined candidate — scope verified,
  owner row for the region alignment/expansion residual of the Squalr
  geometry-parity audit. The gap is concrete:
  `samples/apps/squalr/squalr-engine-api/src/structures/memory/normalized_region.omg`
  carries the seed's own admission — "hashing, alignment adjustment, and
  expansion are not implemented in this seed" — so the residual is the
  normalized-region alignment-adjustment and expansion machines plus their
  rejection cases in the seed port. The implementing edit lives inside
  `samples/apps/squalr`, wholesale dir-fenced this wave by
  SQUALR-WINDOWS-GEOMETRY-VALIDATION (~05:49Z); the earlier
  GEOMETRY-ALIGNMENT-REGIONS dir-fence (~01:18Z) has drained. Sibling
  SQUALR-REGION-ALIGNMENT-EXPANSION is the app-lane row for the same
  residual, and SQUALR-GEOMETRY-PARITY-RESIDUE's resolved audit maps this
  enumerated gap to these rows. No linux_x86_64 slice exists outside the
  claimed fence — coordinate with the squalr port lane before working it.
  covered — port landed in the squalr submodule pin (`ef6682f75f48`); app-lane residual is SQUALR-REGION-ALIGNMENT-EXPANSION's
- **REMAINING-INTRINSIC-SPAN-ARMS** — mined candidate; verify scope then implement.
  covered — span-arm surface complete per TV-INTRINSIC-SPAN-ARMS (`14e6f8f72e`)
- **RETAINED-ARTIFACT-EXECUTABLE-PUBLICATION** — mined candidate; scope verified, resolved — same surface as COMPILER-EXECUTABLE-PUBLICATION-OPERATION (resolved on `origin/main`): the retained-artifact leg is `CompileReport::publish_retained_native_artifact` in `compilation-report/src/compile_report.rs`, which validates the retained artifact and manifest, refuses non-local output filenames, requires compiler-text/function validation evidence, and self-checks a requested PCC pair pre-install before `executable_publication.rs` commits one staged tree + atomic rename — a failed publish leaves no half-written executable or stale sidecar. Witnessed green at `ea025447fe`: `cargo nextest run -p compilation-report executable_publication` 15/15 and the compiler `activation_identifiers_and_publication` suite 15/15. Sibling stubs on the same resolved surface: EXECUTABLE-PUBLICATION, EXECUTABLE-PUBLICATION-JOIN, EXECUTABLE-PUBLICATION-OPERATION, EXECUTABLE-PUBLICATION-STAGE, EXECUTABLE-PUBLICATION-STEP.
- **REVIEW-INSTANTIATION-CLONE-FREE-SCRATCH.** Mined candidate; scope verified
  at `d8041919ad`, owned — names the residual the evidence README already
  records: "Operator and top-level requirement signature capture borrows the
  checked compilation when there are no static parameters to instantiate;
  nonempty static parameter lists retain clone-local instantiation"
  (`omega-rust/omega/packages/review/evidence/README.md`). The clone path is
  `capture/calling/application/signature/instantiation.rs`: `instantiate`
  clones each `type_reference` node, `plan_laid_layouts` rows, and full
  `data_definitions()` entries per recursion. A clone-free slice would read
  those through borrows and route substitutions/lifetimes through the
  bounded scratch the surrounding capture already declares. Claim evidence
  (z181): the item is already claimed — Devin / z36-review-instantiation-
  clone-free-scratch fences
  `review/evidence/src/capture/calling/application/signature` until
  2026-09-21T02:04Z; claim returned exit 2 (same item). Coordinate on the
  owner's branch; no in-fence work attempted. Re-verified at
  `9e3edc7be9` (z137): the z36 fence has expired, but the implementing
  surface stays covered — `review/evidence/src/capture` by
  PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION (04:50Z) and the whole
  `omega-rust/omega/packages/review/evidence` tree by
  PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION (01:23Z). The slice
  remains owned and unfenced-work-free; coordinate after those leases.
  owner's branch; no in-fence work attempted. Implemented by that owner
  (z36): `signature/scratch.rs` now builds an invocation-local `TypedTrees`
  that seeds only the arenas signature/type-identity reads can index —
  trait/data/domain/const/proposition/operator declaration tables, state
  parameters, expression and type-reference arenas, `plan_laid_layouts`,
  `placed_view_plans`, `semantic_domains`, `external_bindings`,
  `machine_specializations`, `open_index_normalizations`, and authored
  declaration-selection custody — while statement, machine-body, measure,
  wire, and proof arenas stay uncloned. `project_application`,
  `declaration_parameters`, and `project_declaration` reify into it instead
  of `compilation.typed.clone()`; `instantiate`'s per-node row writes land
  in the scratch arena while `Ok(*actual)` passthroughs keep source handles.
  The remaining whole-tree clone sites under `capture/callables` and
  `capture/semantics/signatures/policy.rs` belong to their own owners'
  fences.
- **REVIEW-RESEAL-ELIMINATION** — mined candidate; verify scope then implement.
- **ROOT-FILE-DISCIPLINE.** — mined candidate; verify scope then implement.
- **RUNTIME-SIZED-ACTIVATION-CONTRACT.** Connect the ratified
  [bounded activation claim](wiki/spec/resources/activation_storage.md) to
  authored source and Terminal Psi. Use ordinary callable/core-declaration
  mechanisms; an absent `claim` keyword is not an owner-design blocker.
  `psi/foundation/extents/src/activation_claims/` supplies bookkeeping,
  not an executable compiler route.

  Expose compiler-provisioned activation backing with exact activation
  provenance/lifetime, then retain committed extent, bound, release order and
  suspension claim-site rows through checking, lowering, codec and independent
  verification. Keep backing nonmoving with stable materialized addresses for
  the claim lifetime. Owners: ordinary call admission, extent/claim evidence,
  Terminal representation and stack-demand composition. Allocation packages
  can manage already-held backing but cannot mint its grant from an address
  and length. Runtime generic extents and fixed-array establishment are not
  this supply mechanism.

  Acceptance: a source-authored bounded claim supports access, reverse-order
  release and suspension retention under its declared bound; over-bound
  establishment returns checked failure. Escaping claims, missing/duplicate
  sites, wrong provenance, stale loans and invalid release/suspension custody
  reject independently. **FRAME-LAYOUT** in `TASKS_OPTIMIZER.md` owns final
  frame, probe, unwind and native replay. Do not add implicit variable-sized
  locals, provider-backed issuance, a new syntax category or an OS allocator.

- **SAMPLES-COMPILE-MULTI-HOST.** Verified scope — the per-host gate already
  exists as `compiler`'s `samples_compile` suite
  (`all_samples_reach_checked_trees` + per-cohort authored-entry legs +
  `samples_with_documented_exit_run_correctly`); "multi-host" is each
  required host running it, tracked per-sample in the cohort READMEs.
  Witnessed RED on linux x86-64 at `e092723726` (run stopped after the
  failure classes were established; no code was changed here):
  (1) windows_x86_64 authored-entry selection rejects the std entry —
    "target physical entry requirement … require either the exact bundled
    Windows x86-64 contract or one accepted package-owned Windows x86-64
    binding, not `source/library/std/targets/windows_x86_64/entry.omg`"
    (basics, fletcher_checksum, caesar_cipher, format_number legs);
  (2) linux_x86_64/linux_arm64/macos_arm64 "selected ProgramEntry
    establishment rejoins 0 Terminal attachment identities; expected one"
    — owned by SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT. The mechanism this
    paragraph recorded is superseded: `d7a48d7af0` added the
    `BorrowedSliceView` checked shape, so `has_statement_shape` now
    admits the call-produced `&[T]` view local and a `&[T]` formal
    carries the same shape. The frontier moved rather than closed —
    both samples now stop at "statement sequence: call: call operation"
    and still rejoin 0 Terminal attachment identities, because a callee
    that *uses* the view has no Terminal descriptor
    (TERMINAL-SLICE-VIEW-VOCABULARY). The `&[T]` view local
    pattern appears in 12 samples (fletcher_checksum, recursive_sum,
    dual_accumulator_recursion, slice_accum_probe, slice_maximum,
    subslice_sum, framed_payload, clamp_sum, dungeon_crawler modules);
    callee `&[T]` parameters face the same vocabulary gap. The rejoin
    diagnostic now names the recorded omission stage (this commit);
    (fletcher_checksum and likely siblings);
    — attributed under EXACT-PROGRAM-ENTRY-MULTIPLICITY at
    `dbfa1b1702f`: the entry machine leaves the unit plan roster at
    local construction ("call statement shape: call count without a
    statement sequence") because `has_statement_shape`
    (statement_sequence.rs) rejects a call-produced `&[T]` view local —
    `let s: &[i32 in Wrapping] = self.adder.bytes.as_slice();`
    (boundary `Array::as_slice`). LocalData admission covers only
    primitive | structural result | erased exclusive-borrow alias;
    `CheckedUnitStructuralTypeShape` deliberately has no runtime-length
    slice shape and alias formation covers only `ExpressionNode::Borrow`
    exclusive (Mutable/WriteOnly) carriers. The `&[T]` view local
    pattern appears in 12 samples (fletcher_checksum, recursive_sum,
    dual_accumulator_recursion, slice_accum_probe, slice_maximum,
    subslice_sum, framed_payload, clamp_sum, dungeon_crawler modules);
    callee `&[T]` parameters face the same vocabulary gap. The rejoin
    diagnostic now names the recorded omission stage (this commit);
  (3) `cli/basics/print_number` fails checked trees: "cannot prove
    default-domain field requirement for return from Main::main …
    self.out requires [u8; N]::Utf8". These are current-HEAD breaks in the
    entry-establishment / package-owned-binding / default-domain surfaces —
    repairs belong to the owning items (EXACT-PROGRAM-ENTRY-MULTIPLICITY,
    ENTRY-CONTENT-ROOTS, field-obligation rows), not this gate. Remaining
    here once main is green again: run the suite on windows_x86_64,
    macos_arm64, linux_arm64 hosts — host-gated, none producible on this
    machine.
  covered — gate exists as `samples_compile`; remaining legs host-gated (windows/macos/arm64)
- **SCAN-SCALAR-COMPARISON-DISPATCH** — mined candidate; verify scope then implement.
  covered — bare stub; scalar-scan dispatch state tracked on SCOPED-LOOKUP-MAP-AUDIT / squalr lane rows
- **SCAN-SCALAR-DISPATCH.** Verified at `ea698be6482` — resolved. The row
  decomposes the same Squalr pin advance as resolved sibling
  SCAN-SCALAR-SCAN and shares its reopened state: the scalar leg landed on
  the pre-republish lineage (`43329a3`) and went absent when the gitlink
  moved to `5ea4a17f3b`. The residual is now closed by the coordinator
  repin — recorded gitlink `5b0307c352` carries the re-ported scalar
  dispatch on the published lineage: `squalr-engine-scanning/src/scanners/
  element_scan_dispatcher.omg` (explicitly documented "Port of
  ElementScanDispatcher's scanner selection, scalar leg" — Scalar plans
  select scalar scanners; non-scalar/Invalid select nothing), with
  `scanner_scalar_iterative.omg`, `scanner_scalar_single_element.omg`,
  `snapshot_region_filter_run_length_encoder.omg`, and
  `planned_scan_type_scalar.omg` all present. No independent slice
  remains; native-run acceptance stays on the squalr lane's own rows.
- **SCAN-SCALAR-SCAN** — verified e0927237: landed via the squalr
    machine. Re-witness at `4927883cf3` (linux x86-64): class (1)
    reproduces verbatim — `basics_samples_compile_from_authored_program_
    entry_bindings` rejects brightness_control's windows_x86_64 authored
    entry with the same diagnostic in 11s. The deep legs are no longer
    re-witnessable in a bounded run: `all_samples_reach_checked_trees`
    and `algorithm_samples_..._bindings` exceed ~9 min before their
    first assertion, and `omega --check --target linux_x86_64
    samples/cli/basics/print_number/main.omg` emits nothing for >9 min
    (matches the check-time caveat recorded on NOMINAL-FIELD-FLOW —
    scale, not yet evidence of a hang). Classes (2) and (3) stay
    recorded at `e092723726` above, unverified at tip.
  Re-witnessed at `ea78f0e486` (linux x86-64): class (1) still reproduces
  verbatim — `basics_samples_compile_from_authored_program_entry_bindings`
  rejects brightness_control's windows_x86_64 authored entry with the same
  `named-callable(path(WindowsProcessEntry::enter),...)` diagnostic in 11.3 s.
  The suite file itself is claimed by SAMPLE-CORPUS (~10:52Z); the
  windows_x86_64/macos_arm64/linux_arm64 host legs remain unavailable on this
  machine and the repair classes stay with their owning rows.
- **TERMINAL-SLICE-VIEW-VOCABULARY.** (split-of:SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT)
  Give Terminal Psi a borrowed-view vocabulary for non-byte element types, so
  a callee can use a `&[T]` it receives. `d7a48d7af0` made the view local a
  checked shape and a `&[T]` formal carry it, which moved the samples'
  frontier from local construction to the call — but a callee that *uses* the
  view is still omitted. `Adder::fletcher` and `Summer::sum` need `s.len`,
  `s[0]` and `s[1..]` over a non-byte element and have no Terminal
  descriptor; the nine sites landed in `checked-trees-to-lowered-psi` and
  `selected-dispatch` reject with "borrowed slice view has no Terminal
  descriptor" rather than dropping the extent.

  This is a vocabulary gap, not a language decision, and the row should stay
  engineering unless the owner disagrees.
  [byte views](wiki/spec/terminal-psi/byte_views.md) already supplies length,
  read and subslice, but scopes itself to borrowed **byte** views in its title
  and at `:5` ("immutable observations and fixed-extent writes through
  borrowed byte views"). The language side is settled — `Slice::index` and
  `Slice::range` are given in the language guide, chapter 5 lines 244-248 and
  chapter 19 lines 46-50 — so what is missing is the Terminal form of an
  already-decided semantics, plus the spec section that states it. If the
  owner reads the generalization of byte_views.md to arbitrary element types
  as a design decision rather than a transcription, say so and this becomes
  an owner question instead.

  Settle one thing first, because the read operation cannot be written
  without it: **does `Slice::index` require a `[copy]` element?** Three
  sources disagree. `source/library/core/slice.omg:19` and the language
  guide chapter 5 line 244 both declare
  `boundary machine [] Slice::index<T>(items: &[T], index: u64) -> T`, while
  chapter 19 line 46 declares the same machine as
  `Slice::index<T [copy]>`. [ownership](wiki/spec/language/ownership.md)
  makes Affine the default for owned data and permits "move at most once",
  so returning a non-copy `T` by value out of a shared `&[T]` would move out
  of borrowed storage — which points at chapter 19 being right and the
  library declaration being under-constrained. Nothing in the tree settles
  it: the only in-tree uses of `Slice::index` are fail fixtures pinning
  duplicate-operator rejection
  (`fail/operators/root_operator_{duplicate,alpha_equivalent_generic_duplicate}`),
  which carry `<T>` incidentally and decide nothing. `byte_views.md` gives no
  guidance either, since `ByteSequenceRead -> u8` is trivially copyable.
  Resolve this from the checker's actual behaviour if you can; if the checker
  does not decide it, it is an owner question about the core surface, not a
  choice to make while implementing.

  Acceptance: a callee taking `&[T]` for a non-byte `T` reads its length,
  indexes it and takes a subslice, reaching native production; the nine
  "borrowed slice view has no Terminal descriptor" rejections are replaced by
  real descriptors; and `fletcher_checksum` and `recursive_sum` pass
  SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT's acceptance on `linux_x86_64`.
- **SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT.** (split-of:SAMPLES-COMPILE-MULTI-HOST)
  Own the entry-establishment failure class that SAMPLES-COMPILE-MULTI-HOST and
  BORROWED-STORAGE-RESTORATION both route to a retired
  EXACT-PROGRAM-ENTRY-MULTIPLICITY row. On `linux_x86_64`, `linux_arm64` and
  `macos_arm64` the gate reads "selected ProgramEntry establishment rejoins 0
  Terminal attachment identities; expected one" because the entry machine
  leaves the unit plan roster at local construction over a `&[T]` view local
  such as `let s: &[i32 in Wrapping] = self.adder.bytes.as_slice();`. The
  pattern appears in 12 samples, including `samples/cli/text/fletcher_checksum`,
  `samples/cli/arithmetic/recursive_sum`, `samples/cli/collections/slice_maximum`
  and `samples/cli/systems/framed_payload`.

  The view local itself now composes. `CheckedUnitStructuralTypeShape`
  (`checked-trees/src/checked_trees/flow/terminal/structural_type_plans.rs`)
  carries `BorrowedSliceView { element_type_identity }` — no length, because a
  slice's extent is its own stored runtime length — and the local's value is
  `CheckedStructuralValueKind::BorrowedSliceView`, which rejoins the shared
  loan checked borrow admission already records for the lent collection.
  Ordinary statement sequencing establishes it and forwards it whole to a
  `&[T]` formal, and callee `&[T]` parameters carry the same shape. Terminal
  lowering rejects the shape with "borrowed slice view has no Terminal
  descriptor" rather than dropping its extent.

  The frontier is now the `&[T]`-consuming callee. Both samples report
  `statement sequence: call: call operation` (recursive_sum: state 0,
  statement 6) because `Adder::fletcher` / `Summer::sum` have no plan of their
  own: their bodies need `s.len`, `s[0]` and `s[1..]` over a non-byte view, and
  `wiki/spec/terminal-psi/byte_views.md` supplies length, read and subslice
  operations only for borrowed *byte* views. `Slice::index` and `Slice::range`
  are settled at the language level (language guide chapters 5 and 19), so the
  missing piece is the Terminal Psi vocabulary and its spec section, not a
  language decision. Do not add a recognizer for this statement arrangement
  (AGENTS.md, compositional lowering).

  Acceptance: `fletcher_checksum` and `recursive_sum` reach selected
  ProgramEntry establishment on `linux_x86_64` without the "rejoins 0 Terminal
  attachment identities" refusal, and `cargo nextest run -p
  typed-trees-to-checked-trees --lib` stays green.
- **SCALAR-SCAN-AND-DISPATCH.** Mined candidate — scope verified, already
  landed; decomposes the same Squalr-Omega `43329a3` commit as resolved
  sibling SCAN-SCALAR-DISPATCH (scalar scan + run-length encoder +
  element-scan dispatch leg). Re-verified at `9f48bb2a594`: the recorded
  gitlink has advanced to `5b0307c35` and still carries every scalar
  surface — `squalr-engine-scanning/src/scanners/
  element_scan_dispatcher.omg`, `scalar/scanner_scalar_iterative.omg`,
  `scalar/scanner_scalar_single_element.omg`,
  `structures/snapshot_region_filter_run_length_encoder.omg`, plus the
  api-side `scan_function_scalar.omg`,
  `planned_scan_type_scalar.omg`,
  `snapshot_filter_element_scan_plan.omg` (confirmed via `git ls-tree` on
  the recorded pin). The sibling's `omega --check` witness at
  `9e3edc7be9` (scanning 29 files + api 30 files clean) stands — the
  pin's file inventory is what that check consumed. Remaining sibling
  stubs on the same commit: SCAN-SCALAR-COMPARISON-DISPATCH;
  SCAN-SCALAR-SCAN's re-opened note is stale per the sibling's merge
  analysis (`251699c4669d` joined the republished lineage back over
  `43329a3`).
- **SCAN-SCALAR-DISPATCH.** — mined candidate; scope verified, already landed. The stub decomposes Squalr-Omega `43329a3` ("squalr: port scalar scan, run-length encoder, and element-scan dispatch"), whose element-scan dispatch leg names `element_scan_dispatcher.omg` + the scalar scanners. SCAN-SCALAR-SCAN's re-opened note (written when the recorded gitlink sat on the republished `5ea4a17f3b` lineage) is stale: the recorded gitlink `251699c4669d` is merge `db64d58`'s join of that lineage back over `43329a3`, which is its ancestor — `git log` confirms `43329a3` and `251699c` ("merge: adopt wire-schema NormalizedRegion…") carry `squalr-engine-scanning`. Present on the recorded pin: `squalr-engine-scanning/src/scanners/element_scan_dispatcher.omg`, `scalar/scanner_scalar_iterative.omg`, `scalar/scanner_scalar_single_element.omg`, `structures/snapshot_region_filter_run_length_encoder.omg`, plus the api-side `scan_function_scalar.omg` / `planned_scan_type_scalar.omg` / `snapshot_filter_element_scan_plan.omg` surfaces. Verified on linux-x86_64 at `9e3edc7be9` (gitlink 251699c4669d): `omega --check` clean — squalr-engine-scanning 29 files, squalr-engine-api 30 files. Siblings SCALAR-SCAN-AND-DISPATCH, SCAN-SCALAR-COMPARISON-DISPATCH, SCAN-SCALAR-SCAN decompose the same commit and share this state.
- **SCAN-SCALAR-SCAN.** — verified e0927237: landed via the squalr
  pin advance `05416dd1a0` → Squalr-Omega `43329a3` ("squalr: port scalar
  scan, run-length encoder, and element-scan dispatch"). The scalar leg was
  present in that pin: `ScalarIterativeScan` pull driver over
  current/previous u64 windows, `ScannerScalarSingleElement`, and
  `SnapshotRegionFilterRunLengthEncoder` preserving upstream
  stride/byte_advance semantics, selected by `ElementScanDispatcher` for
  Scalar plans. Verified on linux-x86_64 at `e0927237`: `omega --check`
  clean on both packages (squalr-engine-api 24 files, squalr-engine-scanning
  28 files). Re-opened at `1edade1a480` when the gitlink sat on the
  republished `5ea4a17f3b` lineage without the scalar leg — **closed again
  at `94b395ea9c6b`**: the recorded gitlink is now `5b0307c3`, carrying the
  full scalar surface (`scanners/element_scan_dispatcher.omg`,
  `scalar/scanner_scalar_iterative.omg`,
  `scalar/scanner_scalar_single_element.omg`,
  `structures/snapshot_region_filter_run_length_encoder.omg`, api-side
  `snapshot_filter_element_scan_plan.omg`) — the republished lineage's merge
  back over `43329a3` restored it, as sibling SCAN-SCALAR-DISPATCH's row
  recorded for pin `251699c4669d`. Residual retired. Siblings
  SCALAR-SCAN-AND-DISPATCH, SCAN-SCALAR-DISPATCH,
  SCAN-SCALAR-COMPARISON-DISPATCH decompose the same original commit.
  Re-verified at `c924529921d` (linux x86-64): the recorded gitlink has
  advanced again to `ef6682f75f` — `5b0307c3` is its ancestor — and still
  carries every scalar surface: `scanners/element_scan_dispatcher.omg`,
  `scanners/scalar/scanner_scalar_{iterative,single_element}.omg`, the
  run-length encoder now under `scanners/structures/snapshot_region_filter_
  run_length_encoder.omg` (moved with the scanners tree), plus api-side
  `scan_function_scalar.omg`, `planned_scan_type_scalar.omg`,
  `snapshot_filter_element_scan_plan.omg`. The drift is the squalr lane's
  republish cadence, not a reopening.

- **SCOPED-LOOKUP-MAP-AUDIT.** — mined candidate; scope verified, already landed and enforced. The audit exists as the repeatable architecture gate `tests/architecture/scoped_lookup_maps.rs`: it enforces the `omega-rust/pipeline.md` rule ("scoped symbol-tree lookup is the baseline; extra lookup maps require a measured reason") by census — every production `HashMap`/`BTreeMap` keyed by an authored-spelling token (`str`, `String`, `SymbolName`, `InternedName`, `Identifier`, tuple-containing) must appear in `JUSTIFIED_LOOKUP_MAP_FILES` with its recorded key domain (the measured reason), and cataloged files that no longer declare such a map fail the reverse staleness check. The one-shot census it encodes lives at `wiki/drafts/lookup_map_justification.md` (run at `c2ccb2a202`). Verified green on `e12b9e8e06`: `cargo nextest run -p omega-architecture-test --test scoped_lookup_maps` 2/2 pass on Linux x86-64 (`every_name_keyed_lookup_map_file_is_cataloged`, `every_cataloged_file_still_observes_a_name_keyed_map`). No independent slice remains — the gate is self-maintaining: a new name-keyed map without a recorded justification fails the build. Re-verified green at `771d0469a1c47` (linux x86-64, same command 2/2); a third bare mined stub further down this board names the same surface — drained by this row. Deduped under NEW-DEDUPE-SCOPED-LOOKUP-MAP-AUDIT-ROWS: the bare repeat stub and the field-note stub below are deleted (the field note itself asked for deletion), and an orphaned stale Squalr-gitlink fragment glued inside this row is removed — its current resolution lives on the SCAN-SCALAR rows.
  covered — landed and self-enforcing (`tests/architecture/scoped_lookup_maps.rs`)
- **SAMPLES-COMPILE-MULTI-HOST** — mined candidate; verify scope then implement.
  covered — gate exists as `samples_compile`; remaining legs host-gated (windows/macos/arm64)
- **SCOPED-LOOKUP-MAP-AUDIT** — mined candidate; scope verified, already landed and enforced. The audit exists as the repeatable architecture gate `tests/architecture/scoped_lookup_maps.rs`: it enforces the `omega-rust/pipeline.md` rule ("scoped symbol-tree lookup is the baseline; extra lookup maps require a measured reason") by census — every production `HashMap`/`BTreeMap` keyed by an authored-spelling token (`str`, `String`, `SymbolName`, `InternedName`, `Identifier`, tuple-containing) must appear in `JUSTIFIED_LOOKUP_MAP_FILES` with its recorded key domain (the measured reason), and cataloged files that no longer declare such a map fail the reverse staleness check. The one-shot census it encodes lives at `wiki/drafts/lookup_map_justification.md` (run at `c2ccb2a202`). Verified green on `e12b9e8e06`: `cargo nextest run -p omega-architecture-test --test scoped_lookup_maps` 2/2 pass on Linux x86-64 (`every_name_keyed_lookup_map_file_is_cataloged`, `every_cataloged_file_still_observes_a_name_keyed_map`). No independent slice remains — the gate is self-maintaining: a new name-keyed map without a recorded justification fails the build.
  28 files). **Re-opened at `1edade1a480`**: the recorded gitlink moved to
  `5ea4a17f3b` ("pin Squalr with clone/serialization parity",
  `472563ca4c4`), which sits on a republished squalr lineage diverged from
  `43329a3` at `420cabe8` — the published tree carries no scalar-scan
  sources (squalr-engine-scanning is reduced to its package boundary;
  only `snapshot_region_filter.omg` remains under
  `squalr-engine-api/src/structures/scanning/`). The scalar leg therefore
  exists only on the pre-republish lineage. Residual: re-port the scalar
  scan/element-scan dispatch on the published squalr lineage, or have the
  coordinator repin. Siblings SCALAR-SCAN-AND-DISPATCH,
  SCAN-SCALAR-DISPATCH, SCAN-SCALAR-COMPARISON-DISPATCH decompose the same
  original commit and share this reopened state.
  covered — landed and self-enforcing (`tests/architecture/scoped_lookup_maps.rs`)
- **SCOPED-LOOKUP-MAP-AUDIT** — mined candidate; scope verified, already landed and enforced. The audit exists as the repeatable architecture gate `tests/architecture/scoped_lookup_maps.rs`: it enforces the `omega-rust/pipeline.md` rule ("scoped symbol-tree lookup is the baseline; extra lookup maps require a measured reason") by census — every production `HashMap`/`BTreeMap` keyed by an authored-spelling token (`str`, `String`, `SymbolName`, `InternedName`, `Identifier`, tuple-containing) must appear in `JUSTIFIED_LOOKUP_MAP_FILES` with its recorded key domain (the measured reason), and cataloged files that no longer declare such a map fail the reverse staleness check. The one-shot census it encodes lives at `wiki/drafts/lookup_map_justification.md` (run at `c2ccb2a202`). Verified green on `e12b9e8e06`: `cargo nextest run -p omega-architecture-test --test scoped_lookup_maps` 2/2 pass on Linux x86-64 (`every_name_keyed_lookup_map_file_is_cataloged`, `every_cataloged_file_still_observes_a_name_keyed_map`). No independent slice remains — the gate is self-maintaining: a new name-keyed map without a recorded justification fails the build.
  covered — landed and self-enforcing (`tests/architecture/scoped_lookup_maps.rs`)
- **SEALED-COMPOSITION-EXTRACTION** — mined candidate; verify scope then implement.
- **SELECTED-DISPATCH-SERVICE-CARRIER-FIXTURES.** Scope verified at
  `2a9f9c02ad6`, slice landed — names the selected-dispatch libtest family
  recorded by RC-REPOSITORY-CLOSURE's `18cebfa1062`-era census: 64 failures,
  mostly `Service<R>`-spelling stale fixtures inside
  `omega-rust/omega/build/selected-dispatch/` (the `boundary_dispatch`
  test tree). The migration recipe is proven by sibling
  BASELINE-SERVICE-CARRIER-FAILURES (`62c502f9f6`): bare
  `console: Console`/`runtime: TaskRuntime`/`output: Output` value
  spellings migrate to `&'s mut Service<…>` receivers per the
  `0e1977994b` raw-pipeline recipe. Landed on `f2aa7d8df23a`: respelled all
  24 fixture field declarations to `service: Service<Trait>` and made every
  fixture boundary trait `pub` (the carrier requires a public stable slot
  contract). `Service` resolves only against the toolchain declaration, so
  the shared helper installs `source/library/core/service.omg` with
  `SourceOrigin::Toolchain` and parses it under assigned source ids;
  the erasure-binding helper mirrors provider_settlement by authorizing
  exactly each test's selected plans (digest = the selected plan's
  `identity_digest()`, so mutation-then-selection tests bind
  post-mutation). 104/104 `selected-dispatch` lib tests green on Linux
  x86-64 (upstreamed spelling: `typed_with_core_service` +
  `bind_fixture_fused_service_erasures` + `selected_every_plan`). Caveat
  stands per the sibling row: migrated members still stop at
  `signature`-phase local construction, joining the
  missing-transitive-machine-plan family until ENTRY-CONTENT-ROOTS'
  receiver-lifecycle leg lands; `selected-dispatch/src/service_custody.rs`
  is fenced by ENTRY-CONTENT-ROOTS (linw2, exp 09:57Z).
- **SELECTED-REWRITE-ANCESTRY-REMOVAL.** — mined candidate; resolved
  alias of the settled SELECTED-OPTIMIZATION-ANCESTRY-REMOVAL surface
  (carrier row above). Re-verified at `8f58b6676b0` on linux x86-64:
  `cargo nextest run -p selected-instructions-to-selected-instructions
  --test ancestry_contract` → 2/2 pass —
  `staged_types_read_current_data_not_producer_ancestry` and
  `named_stage_hops_stay_at_custody_sites` — still zero
  `.optimized_target()` data reads under
  `rewrites/selected_lowering/` and the stage entrance; staged types
  expose `selected`/`register_environment`/`selections`/
  `budget_per_pass`/`liveness`/`ranges`/`legality` directly and the
  surviving `liveness_stage`/`selected_stage` hops are the pinned
  custody-validator inputs, not data reads. No implementation slice
  remains under this name.
  covered — alias of settled SELECTED-OPTIMIZATION-ANCESTRY-REMOVAL; ancestry_contract 2/2

- **SHARED-MAPPING-REVOCATION.** Mined candidate — resolved on
  `origin/main`: re-mines the shared-custody mapping revocation surface in
  `psi/foundation/extents/src/mapping/mod.rs`, landed at `12e35ef7fdc`
  ("gate zero-copy access on shared-custody mappings behind peer-write
  revocation"). Shared-custody mappings cannot expose mutable access
  (:657) and cannot produce a stable view until a peer-write-revocation
  receipt completes (:680); `begin_peer_write_revocation` (:696) consumes
  the mapping into a linear `PendingPeerWriteRevocation` whose completing
  receipt must bind the exact active mapping, establish the revoked write
  permission, and carry required invalidation facts (:739-761). Pinned by
  `mapping/tests.rs`:
  `shared_mapping_stable_loan_requires_completed_peer_write_revocation`
  and `peer_write_revocation_receipt_binds_the_exact_mapping`. Per spec,
  forced revocation is deliberately out of scope — `extents.md:125`
  requires an explicit fallible provider quiescence/lifecycle protocol,
  not an implicit mapping property. No independent slice exists.
- **SERVICE-CARRIER-FIXTURE-MIGRATION** — recorded at revision 6d00135b89:
  `tests/native-differential/tests/terminal_psi_runnable.rs` migrates its two
  embedded fixtures to `pub boundary trait Console` + `Service<Console>`
  fields, seeding `source/library/core/service.omg` as a Toolchain source in
  `project_source_entry` (new `source` foundation dep). The three
  carrier-spelling legs now pass source checking and stop at the sibling
  `InvalidUnitMachinePlan` unit-admission family. Remaining carrier fixtures
  sit under live fences: coverage.rs (RC-NATIVE-MATRIX-MACOS-ARM64),
  typed-trees `src/tests` (PROOF-CERTIFICATION-BRIDGE family),
  checked-trees-to-lowered-psi tests (WRITE-ONLY-BORROW family).
- **SHARED-RECEIVER-LOAN-ORIGIN.** Resolved — superseded on `origin/main` (verified e76d715c8e, linux-x86_64). The recorded failure `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence` ("state `code` requires an exact retained loan origin for its shared receiver", baseline row at 63f625f942, macOS arm64) now passes; the whole `package_compilation_inputs` run emits zero "loan origin" diagnostics. Fixed by the retained-lineage/borrow-evidence cluster (f8efecfb76/b336531455/2aba180197 family). Residual: 13 unrelated authority/provider-selection failures in the same target remain red under their own items. Re-verified at `0a0662ad27a` (linux x86-64): `cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence` still passes — the resolution is current.
  Re-verified at `3533f7d0e8` (w180, linux x86-64): `sh
  tests/beta/compiler/word-prefix.sh` again passes 736/736 natively and
  the edge-gate wiring + unlabeled refusal message are unchanged.
- **SIGNATURE-FREE-TRAIT-CANDIDATE-SCOPE.** — resolved; the scope-verification
  record is landed: `wiki/drafts/scope_signature_free_trait_candidate_scope.md`
  (`11821e2821`) — the candidate-scope law in
  `syntax-trees-to-symbol-resolved-trees/src/selection/signature_free_requirements.rs`
  is package-scoped via
  `lookup_signature_free_top_level_from_source_matching(..., use_span, ...)`,
  pinned by the 11-member `signature_free` battery (11/11 pass at
  `59e0b5ec22d09`). Named residuals are upstream-gated elsewhere
  (symbolic boundary applications; `same_semantic_name` widening is a
  compatibility decision). No slice under this name at `bb192d7ea9e`.
  Re-verified at `2dbfecd98e49` (z133, linux x86-64): the battery re-runs
  11/11 green and `signature_free_requirements.rs` is unchanged since the
  stamp above (zero upstream edits to the crate since `bb192d7ea9e`).
- **SINGLE-PROGRAM-ENTRY-SELECTION.** Mined candidate; scope verified at
  `9f48bb2a594` — the mechanism exists; the residuals are owned elsewhere.
  Single-entry selection is enforced today by
  `selected-dispatch/src/service_custody/root.rs::derive_fused_program_entry_establishments`:
  the selected ProgramEntry must rejoin exactly one Terminal attachment
  identity and one Terminal structural type (`root.rs:145`, `:152`), and the
  receiver must be one exact record — ambiguity or absence rejects, not a
  silently picked entry. The currently-red legs recorded on the RC gate rows
  are NOT selection-gate gaps: (a) `windows_x86_64` entry.omg rejected by
  the `named-callable(WindowsProcessEntry::enter)` schema — package-binding
  leg; (b) `Service<R>`-fielded ProgramEntry receivers rejoin 0 attachment
  identities — the checked-side establishment leg owned by
  ENTRY-CONTENT-ROOTS (`derive_fused_program_entry_establishments` rejects
  `Service<R>` receivers upstream of selection); (c) the
  `program_entry_binding_outside_build` diagnostic drift is assigned to
  CANARY-CORPUS by the PROGRAM-ENTRY-SELECTION-DIVISION lane.
  No independent slice exists; selection work
  resumes inside the owning lanes.
- **SNAPSHOT-STORAGE** — mined candidate; verify scope then implement.
- **SNAPSHOT-STORAGE-AND-FILTERING.** Scope verified — real item, no
  bounded slice inside this repo's board. The stub names the Squalr
  submodule's SUPPLIED-BYTES-SCAN row (`samples/apps/squalr/TASKS.md:19`):
  port the scalar scan's snapshot storage and snapshot filtering
  infrastructure (RLE filters, independently produced result batches,
  shared snapshot per PORTING.md:9/27) under the pinned Rust revision's
  semantics. Ordered execution places it after GEOMETRY-PARITY, which
  gates it. The submodule path is wholesale-fenced at verification time —
  `samples/apps/squalr` dir-claimed by SQUALR-WINDOWS-GEOMETRY-VALIDATION
  (dev-88738, exp 05:49Z) with same-lane item claims live
  (SQUALR-SUPPLIED-BYTES-SCAN, SQUALR-GEOMETRY-PARITY-RESIDUE).
  Execution belongs to the submodule's own lane under its pin — not a
  parent-repo slice.
- **SQUALR-CLI-ENTRY-AND-MODEL.** Scope verified — real item, no bounded
  slice exists inside this repo's board. The stub names the Squalr
  submodule's CLI-COMMANDS row (`samples/apps/squalr/TASKS.md:26`): port
  the request/response model through squalr-engine-session,
  squalr-engine and squalr-cli, including the intentionally-absent CLI
  main entry — the submodule's AGENTS.md forbids a success stub. Ordered
  execution places it after SUPPLIED-BYTES-SCAN, which gates it. The
  submodule path is wholesale-fenced at verification time
  (`samples/apps/squalr` dir-claimed by SQUALR-WINDOWS-GEOMETRY-VALIDATION
  dev-88738 exp 05:49Z and GEOMETRY-ALIGNMENT-REGIONS z112 exp 01:18Z).
  Execution belongs to the submodule's own lane under its pin — not a
  parent-repo slice.
- **SQUALR-CLONE-SERIALIZATION.** Resolved — implemented under `samples/apps/squalr`
  covered — implemented under samples/apps/squalr (wire-schema NormalizedRegion, Clone halves, roundtrip exercises)
- **SQUALR-CLONE-SERIALIZATION.** Implemented under `samples/apps/squalr`
  (submodule branch zergling/z61-squalr-clone-serialization): `NormalizedRegion`
  fields carry wire schema numbers, so the synthesized `encode`/`decode` pair
  is the upstream `Serialize`/`Deserialize`, exercised in-package by
  `wire_roundtrip` (clone -> encode -> decode -> equals); `clone`/`clone_from`
  are the authored `Clone` halves on `NormalizedRegion` and
  `SnapshotRegionFilter` (`clone_consistent` exercises both in-package);
  `MemoryAlignment` keeps `[copy]` for Clone/Copy and names the case-bearing
  wire-codec gap as the serde deviation. Further named edges, not patched
  over: `&mut`-receiver calls with borrowed arguments and static calls taking
  `&` arguments do not attach under selected ProgramEntry establishment
  cross-package, nested runtime-receiver calls cannot produce record results,
  and the codec's ordered statement calls are only admitted to native closure
  in the entry machine — so the exercises are checked in-package while
  squalr-tests retains `Squalr geometry: PASS` natively (linux_x86_64:
  `omega update` checks all 17 packages; `omega run --keep` exits 0).
  Re-verified at `5b3caaf337` on linux x86-64: the landed submodule branch
  `origin/zergling/z61-squalr-clone-serialization` (tip `5ea4a17`) is an
  ancestor-of-pinned-head delta — +82/-7 lines across
  `normalized_region.omg` (wire schema numbers + `encode`/`decode` +
  `Clone`), `snapshot_region_filter.omg` (`Clone`), and
  `memory_alignment.omg` — building directly on the tracked pin `4b1f7a6`,
  which carries none of it; integration of that branch into the pinned
  app remains the open leg, not any missing machinery.
  Resolved at `5bb9a74842` (z102, linux x86-64): the open leg closed —
  `origin/main`'s `samples/apps/squalr` gitlink now pins `ef6682f`, and
  `5ea4a17` is an ancestor of it (merged at submodule `db64d58`, adopted
  via `251699c` "adopt wire-schema NormalizedRegion; qualify module
  paths", with `5b0307c` advancing the std pins and `ef6682f`
  SQUALR-DEBUG-ASSERTIONS on top). The pinned app carries the wire-schema
  `encode`/`decode`, `Clone` halves, and the in-package `wire_roundtrip` /
  `clone_consistent` exercises the branch recorded. The bare duplicate
  stub ~:17856 names the same resolved row; left standing. No remaining
  slice under this name.
  covered — implemented under samples/apps/squalr (wire-schema NormalizedRegion, Clone halves, roundtrip exercises)
- **SQUALR-DEBUG-ASSERTION-PARITY.** Mined candidate; scope verified at
  `e8bbe9fcc0` against upstream `568aa7589b68`: a re-mine of the
  "Rust debug-only assertions" gap in the app repo's GEOMETRY-PARITY row
  (and TASKS.md:6020 SQUALR-GEOMETRY-PARITY). Upstream `debug_assert!`
  sites in the ported crates live in
  `structures/scanning/filters/snapshot_region_filter.rs` (aligned base,
  size >= value width — the Omega port carries a comment at the same site),
  `structures/structs/valued_struct{,_field}.rs`, and the unported
  scanning/targets-native surfaces. Omega needs no new machinery —
  `configuration.md` excludes a debug/release mode and assertion
  primitive; parity means authored `crash` checks or `requires` clauses on
  the ported machines. Currently unworkable: every ported counterpart sits
  under live claims — re-verified at `9b75533b9c7` (this session): the
  three existing `.omg` files (`snapshot_region_filter.omg`,
  `normalized_region.omg`, `memory_alignment.omg` under
  `squalr-engine-api/src/structures/`) still exist, and the whole
  `samples/apps/squalr` tree remains dir-fenced by Jarod's
  SQUALR-TARGETS-AND-THROUGHPUT (until 21:39Z) and Zergling-112's
  GEOMETRY-ALIGNMENT-REGIONS (until 01:18Z); Zergling-61's
  SQUALR-CLONE-SERIALIZATION file-fence has drained, while
  REGION-ALIGNMENT-EXPANSION and SQUALR-NAMED-TRAIT-OPERATORS hold
  item-level claims on the same lane. Re-verified at `f6cf88be046`: the
  fence map rolled over — the submodule dir is now held only by
  SQUALR-WINDOWS-GEOMETRY-VALIDATION (dev-88738, exp 05:49Z), with the
  same lane's item-level claims (SQUALR-GEOMETRY-PARITY-RESIDUE,
  SQUALR-SUPPLIED-BYTES-SCAN, SQUALR-REGION-ALIGNMENT-EXPANSION) still
  live. Coordinate with GEOMETRY-PARITY's owner lane before working it.
- **SQUALR-DEBUG-ASSERTIONS** — mined candidate; verify scope then implement.
- **SQUALR-ENGINE-CRATE-SOURCES** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-DEBUG-ASSERTIONS** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-PARITY-GAPS** — mined candidate; verify scope then implement.
  covered — contentless stub; geometry parity gaps are GEOMETRY-PARITY / SQUALR-GEOMETRY-PARITY-RESIDUE's lane
- **SQUALR-GEOMETRY-PARITY-REMAINDER** — mined candidate; scope verified at
  `10d93dd448`, covered — re-mines the residual list of owning parent
  item-level claims on the same lane. Coordinate with GEOMETRY-PARITY's
  owner lane before working it.
  Re-witnessed at `5fdd41efd8` (submodule pin `5b0307c`, initialized and
  read-only inspected): the three ported files moved inside the submodule —
  `structures/scanning/filters/snapshot_region_filter.omg`,
  `structures/memory/normalized_region.omg`,
  `structures/memory/memory_alignment.omg` — and the gap stands verbatim:
  `snapshot_region_filter.omg:62` still carries only the comment "Upstream
  debug assertions require an aligned base and size >= value width", no
  `crash`/`requires` clause landed anywhere in the ported surfaces. Fence
  refreshed: `samples/apps/squalr` wholesale under SQUALR-WINDOWS-GEOMETRY-
  VALIDATION (exp 05:49Z), SQUALR-GEOMETRY-PARITY-RESIDUE live (03:32Z),
  SQUALR-NAMED-TRAIT-OPERATORS item claim (10:24Z).
  Fence re-audit at `94b395ea9c6` for retired sibling stub
  GEOMETRY-DEBUG-ASSERTIONS (same gap; its mined row was swept, name
  survives only in the parity-gaps sibling list): submodule pin unchanged
  at `5b0307c`, so the gap evidence stands verbatim. Live fences rotated —
  `samples/apps/squalr` is still wholesale dir-fenced by
  REGION-ALIGNMENT-EXPANSION (zergling-z68, exp 07:25Z);
  Fence re-audit at `138ed79a677` for sibling stub
  **SQUALR-DEBUG-ASSERTIONS** (same re-mine of the Rust debug-only-assertions
  gap — the name's whole surface is this row's): the submodule dir-fence has
  drained, but the lane still sits under item-level claims
  (SQUALR-NAMED-TRAIT-OPERATORS 10:24Z, SQUALR-SEED-PARITY 15:14Z on the seed
  draft) and, per this row's standing note, must coordinate with
  GEOMETRY-PARITY's owner lane before working it — authored `crash`/
  `requires` parity on the ported structures belongs to that lane, not to a
  one-stub slice.
  SQUALR-WINDOWS-GEOMETRY-VALIDATION drained and re-upped as
  GEOMETRY-WINDOWS-VALIDATION (item claim, exp 13:53Z);
  SQUALR-NAMED-TRAIT-OPERATORS item claim still live (exp 10:24Z);
  SQUALR-GEOMETRY-PARITY-RESIDUE drained. The lane stays closed until the
  directory fence opens.
- **SQUALR-GEOMETRY-PARITY-REMAINDER.** — mined candidate; scope verified at
  `10d93dd448`, re-verified at `3533f7d0e8` — the `samples/apps/squalr`
  wholesale fence still stands (GEOMETRY-ALIGNMENT-REGIONS exp 01:18Z +
  SQUALR-WINDOWS-GEOMETRY-VALIDATION exp 05:49Z), so the disposition is
  unchanged — covered; the stub re-mines the residual list of owning parent
- **SQUALR-GEOMETRY-PARITY-REMAINDER** — mined candidate; scope verified at
  `10d93dd448`, covered — re-mines the residual list of owning parent
- **SQUALR-GEOMETRY-PARITY-REMAINDER.** — mined candidate; scope verified at
  `10d93dd448`, re-verified at `3533f7d0e8` — the `samples/apps/squalr`
  wholesale fence still stands (GEOMETRY-ALIGNMENT-REGIONS exp 01:18Z +
  SQUALR-WINDOWS-GEOMETRY-VALIDATION exp 05:49Z), so the disposition is
  unchanged — covered; the stub re-mines the residual list of owning parent
- **SQUALR-GEOMETRY-PARITY-REMAINDER** — mined candidate; scope verified at
  `10d93dd448`, covered — re-mines the residual list of owning parent  Earlier z148
  verification at `74537d6125c` folded the same stub the same way —
  each enumerated gap mapped to a live-claimed sibling row, the parent's
  residuals fenced or host-gated, no linux_x86_64 slice outside a
  claimed fence.

  **SQUALR-GEOMETRY-PARITY** (TASKS.md:6235), which now carries two audits:
  the z105 verified-scope audit (12/12 geometry checks PASS on macOS ARM64
  at app `4b1f7a6` / std `87d8b227`; every enumerated gap maps to a sibling
  row — debug assertions → SQUALR-DEBUG-ASSERTION-PARITY/-ASSERTIONS,
  clone/serialization → SQUALR-CLONE-SERIALIZATION, region alignment →
  SQUALR-REGION-ALIGNMENT-EXPANSION, named trait operators →
  SQUALR-NAMED-TRAIT-OPERATORS) and a z194 re-witness recording a new
  regression: the tracked `squalr-tests/omega.lock` is rejected at HEAD and
  the git-pinned std `87d8b227` fails `omega update` post-`32f5182254`, so
  both recorded re-entry paths are red until the submodule's std pin and
  lock advance — an edit inside `samples/apps/squalr`, wholesale-fenced
  (GEOMETRY-ALIGNMENT-REGIONS 01:18Z, SQUALR-TARGETS-AND-THROUGHPUT). The
  only independent residual is the Windows validation leg, which is
  host-gated per SQUALR-WINDOWS-GEOMETRY-VALIDATION's audit. No
  linux_x86_64 slice outside a claimed fence exists. Sibling re-mine
  stubs: SQUALR-GEOMETRY-PARITY-GAPS, SQUALR-GEOMETRY-PARITY-RESIDUE.
- **SQUALR-GEOMETRY-PARITY-RESIDUE.** Resolved — re-mine of the geometry-parity residual list already adjudicated on sibling SQUALR-GEOMETRY-PARITY-GAPS (adjacent row, verified `12ecbe98f8b`): every enumerated gap is an owned sibling row (alignment string parsing → SQUALR-ALIGNMENT-STRING-PARSING, clone/serialization → SQUALR-CLONE-SERIALIZATION-PARITY, region alignment/expansion → SQUALR-REGION-ALIGNMENT-EXPANSION, named trait operators → SQUALR-NAMED-TRAIT-OPERATORS, debug-only assertions → SQUALR-GEOMETRY-PARITY); the z194-recorded regression — tracked `squalr-tests/omega.lock` rejected at HEAD and git-pinned std `87d8b227` failing `omega update` post-`32f5182254` — is an edit inside `samples/apps/squalr`, wholesale-fenced (SQUALR-TARGETS-AND-THROUGHPUT, GEOMETRY-ALIGNMENT-REGIONS); and the independent residual is the Windows validation leg, host-gated under SQUALR-WINDOWS-GEOMETRY-VALIDATION. No linux_x86_64 slice outside a claimed fence exists.
  Fence re-audit at `ea78f0e486` on the adjudicated row's name
  (SQUALR-GEOMETRY-PARITY-GAPS — its own stub is retired, this row is its
  surviving sibling): the recorded wholesale fence rotated —
  GEOMETRY-ALIGNMENT-REGIONS' hold is gone and `samples/apps/squalr` is now
  dir-fenced by REGION-ALIGNMENT-EXPANSION (zergling-z68, exp 07:25Z);
  SQUALR-WINDOWS-GEOMETRY-VALIDATION holds a live board claim (z175, exp
  06:28Z) for the host-gated Windows leg. Disposition unchanged: every
  enumerated gap remains an owned sibling row and no linux_x86_64 slice
  outside a claimed fence exists.
- **SQUALR-GEOMETRY-PARITY-RESIDUE.** Resolved — re-mine of the geometry-parity residual list already adjudicated on sibling SQUALR-GEOMETRY-PARITY-GAPS (adjacent row, verified `12ecbe98f8b`): every enumerated gap is an owned sibling row (alignment string parsing → SQUALR-ALIGNMENT-STRING-PARSING, clone/serialization → SQUALR-CLONE-SERIALIZATION-PARITY, region alignment/expansion → SQUALR-REGION-ALIGNMENT-EXPANSION, named trait operators → SQUALR-NAMED-TRAIT-OPERATORS, debug-only assertions → SQUALR-GEOMETRY-PARITY); the z194-recorded regression — tracked `squalr-tests/omega.lock` rejected at HEAD and git-pinned std `87d8b227` failing `omega update` post-`32f5182254` — is an edit inside `samples/apps/squalr`, wholesale-fenced (SQUALR-TARGETS-AND-THROUGHPUT, GEOMETRY-ALIGNMENT-REGIONS); and the independent residual is the Windows validation leg, host-gated under SQUALR-GEOMETRY-WINDOWS-VALIDATION. No linux_x86_64 slice outside a claimed fence exists.
- **SQUALR-GEOMETRY-WINDOWS-RUN** — mined candidate; scope verified, re-mine of
  the resolved sibling GEOMETRY-WINDOWS-VALIDATION (`8734480a01`, ~line 7203) —
  same acceptance: `python tools/verify.py native --timeout 600 --omega
  <executable>` on a Windows host against the pinned `samples/apps/squalr`
  (4b1f7a6) build graph; recorded evidence is macOS ARM64 + Linux x86-64
  (`d82697ffca`), Windows remains "was not run". Doubly gated: no Windows
  development host exists in this environment, and `samples/apps/squalr` is
  wholesale dir-fenced by SQUALR-TARGETS-AND-THROUGHPUT. A Linux-side
  `--target windows_x86_64` emit leg would not satisfy the run-based
  acceptance. Owning parent: SQUALR-GEOMETRY-PARITY (~line 6064); siblings on
  the same leg: SQUALR-GEOMETRY-WINDOWS-NATIVE (:8520),
  SQUALR-GEOMETRY-WINDOWS-VALIDATION (:8522), GEOMETRY-WINDOWS-LEG (:7201),
  GEOMETRY-WINDOWS-REVALIDATION (:7202).
- **SQUALR-GEOMETRY-WINDOWS-VALIDATION** — mined candidate; scope verified, re-mine of the resolved sibling row GEOMETRY-WINDOWS-VALIDATION (`8734480a01`, ~line 7016). It names the same acceptance: the Windows leg of the app repo's GEOMETRY-PARITY gate — `python tools/verify.py native --timeout 600 --omega <executable>` on a Windows host against the pinned `samples/apps/squalr` (4b1f7a6) build graph; recorded geometry evidence is macOS ARM64 + Linux x86-64 (`d82697ffca`, `Squalr geometry: PASS`), Windows remains "was not run". Doubly gated: no Windows development host exists in this environment, and `samples/apps/squalr` is wholesale dir-fenced by SQUALR-TARGETS-AND-THROUGHPUT with file-level fences from SQUALR-CLONE-SERIALIZATION. A Linux-side `--target windows_x86_64` emit leg would not satisfy the run-based acceptance. Owning parent: SQUALR-GEOMETRY-PARITY (~line 6064); sibling re-mine SQUALR-WINDOWS-GEOMETRY-VALIDATION (~8256).
- **SQUALR-GEOMETRY-PARITY-RESIDUE.** Resolved — re-mine of the geometry-parity residual list already adjudicated on sibling SQUALR-GEOMETRY-PARITY-GAPS (adjacent row, verified `12ecbe98f8b`): every enumerated gap is an owned sibling row (alignment string parsing → SQUALR-ALIGNMENT-STRING-PARSING, clone/serialization → SQUALR-CLONE-SERIALIZATION-PARITY, region alignment/expansion → SQUALR-REGION-ALIGNMENT-EXPANSION, named trait operators → SQUALR-NAMED-TRAIT-OPERATORS, debug-only assertions → SQUALR-GEOMETRY-PARITY); the z194-recorded regression — tracked `squalr-tests/omega.lock` rejected at HEAD and git-pinned std `87d8b227` failing `omega update` post-`32f5182254` — is an edit inside `samples/apps/squalr`, wholesale-fenced (SQUALR-TARGETS-AND-THROUGHPUT, GEOMETRY-ALIGNMENT-REGIONS); and the independent residual is the Windows validation leg, host-gated under SQUALR-GEOMETRY-WINDOWS-VALIDATION. No linux_x86_64 slice outside a claimed fence exists.
- **SQUALR-GEOMETRY-WINDOWS-NATIVE.** Mined candidate — scope verified at
  `3533f7d0e8`, covered + host-gated: third re-mine of the same Windows
  leg of the app repo's GEOMETRY-PARITY gate as resolved siblings
  SQUALR-GEOMETRY-WINDOWS-RUN (:11218) and SQUALR-GEOMETRY-WINDOWS-
  VALIDATION (:11231, itself re-mining GEOMETRY-WINDOWS-VALIDATION).
  Same acceptance — `python tools/verify.py native --timeout 600
  --omega <executable>` on a Windows host against the pinned
  `samples/apps/squalr` build graph; recorded geometry evidence is macOS
  ARM64 + Linux x86-64, Windows "was not run". Doubly gated: no Windows
  development host exists in this environment and `samples/apps/squalr`
  is wholesale dir-fenced (SQUALR-TARGETS-AND-THROUGHPUT + file fences).
  A Linux `--target windows_x86_64` emit leg does not satisfy the
  run-based acceptance. A same-item claim is live (Devin /
  squalr-geometry-windows-native, exp 02:35Z); nothing independent to
  add while it stands.
- **SQUALR-GEOMETRY-WINDOWS-RUN** — mined candidate; scope verified, re-mine of
  the resolved sibling GEOMETRY-WINDOWS-VALIDATION (`8734480a01`, ~line 7203) —
  same acceptance: `python tools/verify.py native --timeout 600 --omega
  <executable>` on a Windows host against the pinned `samples/apps/squalr`
  (4b1f7a6) build graph; recorded evidence is macOS ARM64 + Linux x86-64
  (`d82697ffca`), Windows remains "was not run". Doubly gated: no Windows
  development host exists in this environment, and `samples/apps/squalr` is
  wholesale dir-fenced by SQUALR-TARGETS-AND-THROUGHPUT. A Linux-side
  `--target windows_x86_64` emit leg would not satisfy the run-based
  acceptance. Owning parent: SQUALR-GEOMETRY-PARITY (~line 6064); siblings on
  the same leg: SQUALR-GEOMETRY-WINDOWS-NATIVE (:8520),
  SQUALR-GEOMETRY-WINDOWS-VALIDATION (:8522), GEOMETRY-WINDOWS-LEG (:7201),
  GEOMETRY-WINDOWS-REVALIDATION (:7202).
- **SQUALR-GEOMETRY-WINDOWS-VALIDATION** — mined candidate; scope verified, re-mine of the resolved sibling row GEOMETRY-WINDOWS-VALIDATION (`8734480a01`, ~line 7016). It names the same acceptance: the Windows leg of the app repo's GEOMETRY-PARITY gate — `python tools/verify.py native --timeout 600 --omega <executable>` on a Windows host against the pinned `samples/apps/squalr` (4b1f7a6) build graph; recorded geometry evidence is macOS ARM64 + Linux x86-64 (`d82697ffca`, `Squalr geometry: PASS`), Windows remains "was not run". Doubly gated: no Windows development host exists in this environment, and `samples/apps/squalr` is wholesale dir-fenced by SQUALR-TARGETS-AND-THROUGHPUT with file-level fences from SQUALR-CLONE-SERIALIZATION. A Linux-side `--target windows_x86_64` emit leg would not satisfy the run-based acceptance. Owning parent: SQUALR-GEOMETRY-PARITY (~line 6064); sibling re-mine SQUALR-WINDOWS-GEOMETRY-VALIDATION (~8256).
- **SQUALR-PLUGIN-IMPLEMENTATIONS.** Scope verified at a4ffd1aff8 —
  re-mines the "plugins/*: implementation unported" row of
  `samples/apps/squalr/README.md`'s port-boundary table. All eight plugin
  packages under `samples/apps/squalr/plugins/` contain only `build.omg`
  package declarations (boundaries + internal edges mirrored from upstream
  `plugins/*/Cargo.toml`); zero `*.omg` implementation sources exist.
  Implementation order follows the package edges: the leaf plugins
  (`squalr-plugin-data-types-24bit`, `-instructions-{arm,powerpc,x86}`,
  `-memory-view-dolphin`, `-binary-symbols`, `-debuggers-native`) carry
  real ports first; `squalr-plugin-builtins` is the aggregator that
  depends on all of them plus `squalr-engine-api` — it lands last. This is
  a multi-session port (each plugin mirrors its upstream crate's behavior
  per PORTING.md parity rules — no placeholder services); it sits after
  the scan engine in PORTING.md's executable progression, so earliest
  legal start is after SUPPLIED-BYTES-SCAN/CLI-COMMANDS settle. The
  whole `samples/apps/squalr` submodule is additionally wholesale-claimed
  this wave (SQUALR-TARGETS-AND-THROUGHPUT 21:39Z,
  SQUALR-CLONE-SERIALIZATION 22:50Z), so every implementable path is
  fenced at verification time. Re-verified at `8f58b6676b`: all eight
  `plugins/*` packages still carry only `build.omg` declarations at the
  pinned submodule commit `5b0307c` (zero implementation `*.omg` sources),
  the ordering gate stands (SUPPLIED-BYTES-SCAN / SQUALR-CLI-COMMANDS
  unsettled), and the whole submodule is again wholesale-fenced —
  SQUALR-DEBUG-ASSERTIONS (z35, exp ~16:25Z). The settled verdict stands.
- **SQUALR-SEED-ALIGNMENT-PARSING.** — mined candidate; scope verified,
  covered. Compound re-mine: "seed" is the port-seed wording on the app
  board's GEOMETRY-PARITY residual list ("the mapped Rust behavior still
  absent from the seed", samples/apps/squalr/TASKS.md) already adjudicated
  by SQUALR-SEED-PARITY (resolved merged alias at `a3ab15b7611`), and
  "alignment parsing" is its enumerated gap owned by
  SQUALR-ALIGNMENT-STRING-PARSING — whose verified row attributes the only
  implementing surface to the `samples/apps/squalr` submodule (under
  SQUALR-TARGETS-AND-THROUGHPUT and GEOMETRY-ALIGNMENT-REGIONS claims) and
  the residual `set_alignment` call-site gate to the compiler
  entry-mechanics lane under GEOMETRY-PARITY. No independent slice exists
  under this name; sibling re-mine names on the same surface:
  ALIGNMENT-STRING-PARSING, GEOMETRY-ALIGNMENT-PARSING,
  GEOMETRY-ALIGNMENT-STRING-PARSING, SQUALR-ALIGNMENT-STRING-PARSING.
- **SQUALR-SUPPLIED-BYTES-SCAN.** — mined candidate; verify scope then implement.
- **SQUALR-SEED-REGION-OPERATIONS** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-WINDOWS-VALIDATION.** Resolved 2026-09-21 — minted
  alias of the same Windows leg of the app repo's GEOMETRY-PARITY gate,
  settled on the adjacent GEOMETRY-WINDOWS-VALIDATION /
  SQUALR-WINDOWS-GEOMETRY-VALIDATION rows. Re-verified at `832c55e69b`
  on linux x86-64: recorded evidence stays macOS ARM64 + Linux x86-64
  (`Squalr geometry: PASS`, native exit 0); the Windows leg requires
  `python tools/verify.py native --timeout 600` on a Windows host and no
  Windows development host exists in this lane. The submodule gitlink
  pins `5b0307c352`; `samples/apps/squalr` carries live sibling claims
  this wave (SQUALR-NAMED-TRAIT-OPERATORS item claim ~10:24Z,
  SQUALR-SEED-PARITY 15:14Z on the draft ledger). A Linux
  `--target windows_x86_64` emit leg does not satisfy the run-based
  acceptance. No linux_x86_64 slice exists under this name.
  Re-verified at `8f58b6676b0` (zergling-132, linux x86-64): unchanged
  host gate — no Windows development host exists in this lane — and the
  submodule gitlink has moved again to `ef6682f75f4` (from `5b0307c352`,
  via SQUALR-DEBUG-ASSERTIONS' crash-parity get_element_count bump), so
  the Windows leg must re-run against the new pin whenever a Windows
  host appears. Fence refresh: `samples/apps/squalr` is dir-fenced by
  SQUALR-DEBUG-ASSERTIONS (~16:25Z); the GEOMETRY-WINDOWS-VALIDATION
  item claim stays live (~13:53Z); the recorded draft
  `wiki/drafts/squalr_geometry_windows_validation.md` named by the
  sibling row is not landed on main. Re-verified at `c924529921` (linux
  x86-64): submodule gitlink still `ef6682f75f4`; host gate unchanged —
  no Windows host in this lane, so the leg stays unmeasurable here.
- **SQUALR-WINDOWS-GEOMETRY-VALIDATION.** Mined candidate — scope
  verified, re-mine of the audited sibling row
  SQUALR-GEOMETRY-WINDOWS-VALIDATION (~line 10273, verified against
  GEOMETRY-WINDOWS-VALIDATION at `8734480a01`). Same acceptance: the
  Windows leg of the app repo's GEOMETRY-PARITY gate —
  `python tools/verify.py native --timeout 600 --omega <executable>` on
  a Windows host against the pinned `samples/apps/squalr` (4b1f7a6)
  build graph; recorded geometry evidence is macOS ARM64 + Linux
  x86-64 (`d82697ffca`, `Squalr geometry: PASS`), Windows remains
  "was not run". Doubly gated: no Windows development host in this
  environment, and `samples/apps/squalr` is dir-fenced by
  SQUALR-TARGETS-AND-THROUGHPUT (+ SQUALR-CLONE-SERIALIZATION file
  fences). A Linux `--target windows_x86_64` emit leg does not satisfy
  the run-based acceptance. Owning parent: SQUALR-GEOMETRY-PARITY.
- **STAGED-LOCAL-CRASH-LOWERING.** Mined candidate — resolved:
  re-mines the explicit-crash leg of the resolved
  STAGED-LOCAL-SEQUENCE-LOWERING row, identical to sibling verdict
  STAGED-LOCAL-CRASH-LOWERING-ATTRIBUTION (this section). Re-verified
  at `9ff8673b310` (linux x86-64): `cargo nextest run -p
  omega-native-differential-test --test terminal_psi_source -E
  'test(~staged_local)'` — 7/7 pass, including
  `checked_source_staged_local_sequences_before_an_explicit_crash`.
  The stale `wiki/drafts/known_baseline_failures.md` entry
  (`UnsupportedControlFlow(MachineId(1))`, expectation from
  `2694d433d3`) belongs to the known-failures doc lane.
- **STAGED-LOCAL-CRASH-LOWERING-ATTRIBUTION.** Resolved — sibling verdict
  named by STAGED-LOCAL-CRASH-LOWERING (this section): the
  attribution leg of the resolved STAGED-LOCAL-SEQUENCE-LOWERING
  surface is green — `cargo nextest run -p omega-native-differential-test
  --test terminal_psi_source -E 'test(~staged_local)'` passes 7/7 on
  linux x86-64 (re-verified `9ff8673b310`), including
  `checked_source_staged_local_sequences_before_an_explicit_crash`.
  The stale `wiki/drafts/known_baseline_failures.md` row
  (`UnsupportedControlFlow(MachineId(1))`, expectation from
  `2694d433d3`, never bisected) belongs to the known-failures doc
  lane — fenced there, not an open leg of this surface.
- **STALE-CUSTODY-GATE-EXPECTATIONS** — scope verified at `10d93dd448d`:
  Re-verified at `7110606f46` under claim e0188317 (exp 05:49Z): the
  submodule gitlink now pins `5b0307c352` (was `4b1f7a6` at the sibling
  audit), so the Windows leg must additionally re-run against the moved
  pin; the host gate is unchanged — still no Windows host in this lane.
  Re-verified at `94e764a6da` (Zergling-108): gitlink still `5b0307c352`;
  the previously-recorded wholesale dir fences on `samples/apps/squalr`
  (SQUALR-TARGETS-AND-THROUGHPUT, GEOMETRY-ALIGNMENT-REGIONS,
  SQUALR-WINDOWS-GEOMETRY-VALIDATION, SNAPSHOT-STORAGE) have all expired —
  the submodule is unfenced at this check — but the operative blocker is
  unchanged: acceptance is a Windows-host `tools/verify.py native` run and
  no Windows host exists in this lane. Record:
  `wiki/drafts/squalr_geometry_windows_validation.md`.
  Re-verified at `8f58b6676b0` (z181): the submodule gitlink moved
  again — now `ef6682f75f` (was `5b0307c352`) — so the Windows leg
  re-runs against the moved pin when a host exists; the named record
  `wiki/drafts/squalr_geometry_windows_validation.md` is not on main.
  `samples/apps/squalr` is wholesale-fenced this wave under
  SQUALR-DEBUG-ASSERTIONS (~16:25Z registry). Operative blocker
  unchanged: acceptance needs a Windows-host `tools/verify.py native`
  run and no Windows host exists in this lane.




- **NATIVE-WRAPPER-ENCODING-AARCH64.** (new-scope) — the optimized program
  storage semantic wrapper encoding lane has no AArch64 implementation.
  `select_optimized_program_storage_semantic_wrapper_encoding`
  (`omega-rust/omega/compiler/native-realization/src/optimized_semantic_wrapper_encoding/mod.rs`)
  calls `encode_x86_64_semantic_unit_wrapper_template` unconditionally, with
  no `Architecture` switch, so on AArch64 the lane cannot produce a template.
  It refuses cleanly rather than miscompiling — the error surfaces as
  `NonCanonicalRequest` — so this is a missing peer implementation, not a
  correctness hole. An AArch64 template is owed: the lane's own module doc
  calls this the "target-owned semantic ProgramStorage wrapper", and
  [entry roots](wiki/spec/build/entry_roots.md) frames the whole surface as
  per-target, a "target-authored bootstrap adapter and physical result map".

  Blocked on owner question 5 (`aarch64-semantic-wrapper-arrival-shape`):
  what that template *is* is undetermined, not merely unwritten. The x86-64
  template exists because the UEFI target authors a by-reference semantic
  arrival — `ValueLocation::Indirect { pointer: Register(X86Rcx), has_copy:
  true, copy_stack_byte_offset: 32 }` and `shadow_bytes = 32` in
  `source/library/std/targets/uefi_x86_64/entry.omg` — so every number in
  `program-entry-plan/src/optimized_semantic_wrapper/recipe.rs` (shadow 32,
  copies into 32/40/48/56, address binds at 32/48) is read off that
  declaration. Both AArch64 targets author the opposite: `extent_value(0, 1)`
  and `extent_value(2, 3)` place each Extent as two `Aarch64X` register
  fragments with no copy, and neither file sets `shadow_bytes`
  (`macos_arm64/entry.omg`, `linux_arm64/entry.omg`, whose comment reads "the
  generated bridge passes the image and initial-storage roots in the first
  four AAPCS64 integer registers"). The wrapper calls its continuation under
  the same plan fingerprint it arrived on, so on AArch64 there is no
  caller-owned copy to re-materialize, and `validate_root_placement` rejects a
  register-fragment placement outright. Separately, no AArch64 profile
  declares `ProgramStorageApplication`/`ImageAndInitialStorage`
  (`target/src/lib.rs` `program_entry_slot` gives `MacosArm64` and
  `LinuxArm64` `HostedApplication`/`None`), so the lane's own input contract
  has no AArch64 instance to construct a test from today.

  The ISA half needs no decision and is on record. Independently assembled
  with Apple clang (`clang -c -arch arm64`, read back with `otool -t`) on
  macOS 24.5.0: `sub sp, sp, #48` is `d100c3ff`, `str x30, [sp, #40]` is
  `f90017fe`, `str x0, [sp]` is `f90003e0`, `add x1, sp, #16` is `910043e1`,
  `bl` is `9400_0000 | imm26`, `b` is `1400_0000 | imm26`, and `ret` is
  `d65f03c0`. The call relocation field is not the x86-64 shape: `BL` carries
  `imm26` in bits [25:0] of the branch word, scaled by 4 and ranged to
  128 MiB either way, sharing the word with opcode `0b100101`.
  `X86_64SemanticUnitWrapperEncodingRequest.relocation_field_byte_width = 4`,
  resolved by overwriting four bytes with `i32::to_le_bytes`, and the
  plan-level `OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1`
  with `byte_width: 4`, cannot describe it. AArch64 resolution is a masked
  merge into the retained opcode word, and its unresolved state is "bits
  [25:0] are zero", not "four zero bytes"; a peer needs its own relocation
  kind rather than the shared 32-bit byte-displacement field.

  Acceptance: once owner question 5 fixes the arrival shape, either the lane
  selects a template per architecture and an AArch64 host encodes its own
  wrapper, with a test pinning both ISAs; or the refusal is pinned as the
  intended contract by a test that names the architecture, and this row is
  removed.

  **DESIGN-BLOCKED (verified 2026-09-21).**
  `wiki/spec/build/entry_roots.md`'s entry-shape section gives only the
  Windows/UEFI x86-64 worked example and defers the rest to "the exact target
  adapter", stating no rule for a register-fragment semantic ProgramStorage
  arrival (no copy, no `shadow_bytes`). `target/src/lib.rs` gives AArch64 only
  `HostedApplication`/`None`, so there is no `ProgramStorageApplication`
  instance to build a request from even if one wanted to.

- **STALE-CUSTODY-GATE-EXPECTATIONS.** Scope verified at `10d93dd448d`:
  the custody-gate expectation slice left by TERMINAL-SOURCE-CUSTODY-GATE-ORDER
  is current, not stale. All five custody-named fail fixtures
  (`core/content_retained_custody_from_borrow`,
  `core/placement_custody_wrong_arity_rejected`,
  `memory/bump_allocator_cast_minted_resident`,
  `memory/bump_allocator_cast_minted_vacant`,
  `proofs/quotient_routed_carrier_content_rejected`) still reject with their
  pinned fragments under
  `fail_canaries_reject_with_expected_diagnostic_fragment` — the landed
  custody ordering repins no custody-gate expectation. The actual stale
  expected.txt census at this revision is 12 drifted canaries in 118.5s
  (was 10 at `e76d715c8e`):
  `expressions/indexed_qualified_call_argument_mismatch`,
  `providers/provider_selection_outside_build`,
  `build/program_entry_binding_outside_build` (known residual already
  assigned to CANARY-CORPUS by PROGRAM-ENTRY-SELECTION-DIVISION),
  `comptime/fuel_exhausted_const_array_length`,
  `generics/colon_bound_rejected`,
  `generics/const_data_machine_call_requires_zero_arguments`,
  `generics/const_data_machine_call_requires_pure`,
  `domains/boundary_operator_mutation_invalidates_domain`,
  `providers/slot_plan_ambiguous`, plus three silent acceptances:
  `ownership/linear_ambiguous_state_result_mapping`,
  `calls/guarded_value_call_terminal_rejected` (both previously recorded),
  and new silent acceptance
  `calls/machine_self_call_recursion_rejected`. Repinning the
  diagnostic-drift fixtures and investigating the silent acceptances is
  CANARY-CORPUS's named lane (its fence ledger stands); no
  custody-specific stale-expectation slice remains on this row.
  Unrelated roster drift also observed: 6 unregistered fail fixtures under
  `tests/omega/fail` (roster.rs inventory check red at base).
- **STARTUP-ENTRY-MECHANICS.** Resolved — named sibling alias of the
  settled STARTUP-ENTRY-MECHANICS-OWNERSHIP cluster (resolved by audit
  at `be03555d17`; ENTRY-MECHANICS-RUNTIME-CONSOLIDATION at :7683 names
  this stub verbatim). Re-verified at `c3dd8016a74d`+ head fetch
  (Zergling-52, linux x86-64, claim 64ad3bc5): `root_entry/` holds six
  modules (root_validation, root_admission, provider_execution,
  progress_profile_installation, opaque_callback_replacement,
  required_root_slots), `tests/architecture/layering.rs` still pins the
  external-roots ownership rows, and `hosted_unit_entry.rs` is intact under
  image-emission. The prior `external-roots/src/root_entry` fence under
  this name has drained; `hosted_receiver*` stays fenced by
  PLAN-LAID-VIEWS (~09:25Z). Entry/exit mechanics consolidate at
  `backend/runtime/external-roots/src/root_entry` (validation,
  admission, provider execution, progress-profile installation) plus
  `platform_bringup` for UEFI bootstrap; `program-entry-plan` is
  data-only and `_start` resolution + `entry_settlement` are emission
  detail, not a second mechanics site. `tests/architecture/layering.rs`
  pins the ownership rows; the runtime side was settled by
  BACKEND-RUNTIME-STARTUP-ENTRY-MECHANICS (free Unit entries emit
  process adapters on linux_x86_64/linux_arm64; ELF `e_entry`
  round-trips through final-image validation). No independent slice
  exists.
  covered — alias of the settled STARTUP-ENTRY-MECHANICS-OWNERSHIP cluster (be03555d17)
- **STARTUP-ENTRY-PLACEHOLDER-SWEEP.** Resolved — named sibling alias of
  the resolved STARTUP-ENTRY-MECHANICS-OWNERSHIP cluster (:7578). The
  sweep for placeholder/shadow startup-entry mechanics is already
  settled: ownership consolidates at
  `backend/runtime/external-roots/src/root_entry` (validation,
  admission, provider execution, progress-profile installation) plus
  `platform_bringup` for UEFI bootstrap; `program-entry-plan` is
  data-only and `_start` resolution + `entry_settlement` are emission
  detail, not second mechanics sites. `tests/architecture/layering.rs`
  pins the ownership rows; the runtime side was settled by
  BACKEND-RUNTIME-STARTUP-ENTRY-MECHANICS (free Unit entries emit process
  adapters on linux_x86_64/linux_arm64; ELF `e_entry` round-trips through
  final-image validation). No placeholder mechanics survive to sweep —
  no independent slice exists.
- **STARTUP-ENTRY-RUNTIME-MECHANICS** — mined candidate; verify scope then implement.
- **STATEMENT-CALL-RECURSIVE-OVERLOAD** — mined candidate; verify scope then implement.
- **STRUCTURAL-PROOFS-CHECKED-CALL-SELECTION** — mined candidate; scope verified, resolved — mis-mined leg: `benchmarks.md` records that of the two depend-free proof subjects, "one fails earlier at checked-call selection" — that is `math_proofs` (undeclared `Bag(items)` calls in `bag_equality_carries`, occurrence 42). `structural_proofs` has no call-selection gap: `omega --check samples/cli/proofs/structural_proofs/main.omg` compiles 4 sources clean at `5b839c31ab` on linux x86-64. The remaining `Bag` repair lives under the math_proofs stubs (PROOF-SAMPLES-CHECKED-CALL-SELECTION family).
- **SUPPLIED-BYTES-SCAN.** Scope verified — real item, no bounded slice exists inside this repo's board. The stub names the Squalr submodule's ordered execution row (`samples/apps/squalr/TASKS.md`): port the scalar scan, snapshot storage, comparison dispatch, RLE encoder and query path through squalr-engine-api + squalr-engine-scanning — a multi-session port inside a submodule whose own AGENTS.md forbids placeholder bodies and requires the unchanged application command as outer acceptance. The submodule path is additionally wholesale-fenced at verification time (SQUALR-TARGETS-AND-THROUGHPUT, exp 21:39Z) and per the submodule's ordering it precedes CLI-COMMANDS, which gates on this row landing first. Execution belongs to the submodule's own lane under its pin — not a parent-repo slice; SQUALR-SUPPLIED-BYTES-SCAN is a sibling stub naming the same row.
- **T2C-RANK-RANGE-FIELD-ENDPOINTS.** Mined candidate — resolved:
  rank-range endpoints expressed as field chains are landed and green.
  `typed-trees-to-checked-trees/src/checks/termination/ranking/ranges/endpoints.rs`
  resolves `ExpressionNode::Member` chains root-to-leaf through declared
  field types (`EndpointInput`), reads the leaf's store-enforced field
  bounds, and re-checks preservation on every self edge
  (`preserved_by`, `prefix_preserves_path`). Re-verified on linux x86-64
  at `e8bbe9fcc0`: `cargo nextest run -p typed-trees-to-checked-trees
  --lib -E 'test(/field_endpoint/)'` — 49/49 pass across
  field_coordinates / field_endpoint_arithmetic / field_endpoint_pins /
  field_arrivals / computed_field_limits. Sibling stub
  TERMINATION-RANK-RANGE-FIELDS names the same surface. Re-verified at
  `771d0469a1`: `e8bbe9fcc0` is an ancestor of base and the endpoint
  machinery now lives under
  `omega-rust/psi/pipeline/typed-trees-to-checked-trees/src/checks/termination/ranking/ranges/endpoints.rs`
  — `EndpointInput` resolution, per-self-edge `preserved_by` re-checks and
  `prefix_preserves_path` all intact.
  covered — field-chain rank-range endpoints landed in checks/termination/ranking/ranges/endpoints.rs; 49/49 green
- **TARGET-INFERENCE-AND-PLATFORM-CERTIFICATION.** Scope verified; audit plus
  one landed fix. The mined name resolves to the exact-target-request
  contract (`wiki/spec/build/configuration.md`): reject `all`, `*`, empty
  sets and inference from source/dependencies/the toolchain catalog, and
  certify selected platforms mechanically (semantics, ABI/layout, resources,
  reach validate — "mechanical closure, not a claim of human testing").
  Already implemented and pinned before this leg: `ExplicitTargetSet`
  (`compiler/request/targets.rs`) rejects empty/`all`/`*`, dedups and orders
  canonically; `CompileRequest::validate_for_execution` (`request.rs`)
  requires exact names on multi-target runs, resolves an omitted name
  through `TargetProfile::host_if_supported` for NativeArtifact and refuses
  with a diagnostic on uncatalogued hosts, and keeps absent targets
  target-neutral for Check/TerminalArtifact; selected-profile certification
  runs through `required_root_slots` binding plus exact physical-contract
  package digests in `build-evaluation/admission/selection.rs`, with
  recognized-but-unrealized `alpha_bootstrap` reporting not-implemented.
  Landed fix: `NativeTarget::from_omega_target_name(None)` resolved through
  raw `NativeTarget::host()`, which panics in `host_architecture` on
  uncatalogued architectures and fabricates `(Aarch64, Coff)` /
  `(X86_64, Elf)` triples on hosts with no catalogued profile (the
  `host_if_supported` contract on `TargetProfile` requires build-scope
  source-selection callers to carry that absence, never panic or name a
  foreign shape); the `None` arm now routes through a new
  `NativeTarget::host_if_supported()` (`TargetProfile::host_if_supported`
  → `native_target`), so target-neutral admission paths
  (`filter_target_machines_by_scope`, `filter_generated_extension`,
  provider settlement's profile-absent fallback entry) refuse with a
  diagnostic instead of panicking or fabricating on uncatalogued hosts.
  Identical triples on every catalogued host. Also refreshed the stale
  `MacosX64` doc comment (the Mach-O x86-64 writer and
  `targets/macos_x86_64` package landed at `5a5046d1dbc`; the slot arm stays
  empty under MACOS-X64-HOST-PROFILE). Residual nits not owned here: the
  uncatalogued-host diagnostic in `request.rs` lists seven profiles and
  omits `macos_x86_64`/`alpha_bootstrap`. Verified at HEAD on linux x86-64:
  `cargo check -p build-evaluation -p provider-planning -p
  package-compilation --all-targets` clean; `nextest -p target` 55/55,
  `-p build-evaluation` 90/90.
- **TERMINATION-FIELD-ENDPOINT-TRIO.** Mined candidate — resolved: the name
  names the three `rank_ranges` field-endpoint failures recorded in
  `wiki/drafts/known_baseline_failures.md` at `660f5af762`
  (`computed_field_limits::field_endpoint_formation_never_uses_final_cancellation_to_excuse_overflow`,
  `field_coordinates::field_endpoints_require_defined_intermediates_and_exact_owned_carriers`,
  `field_endpoint_arithmetic::constant_rank_endpoints_preserve_landing_and_rational_meaning`),
  which that row already assigned to the live TERMINATION-RANKING-CHECKS
  lane. All three pass on linux x86-64 at `54984323b2`
  (`cargo nextest run -p typed-trees-to-checked-trees --lib`, filtered to
  the trio). No independent slice exists; refreshing the stale draft row
  belongs to the RC-REPOSITORY-CLOSURE items.
  Re-witnessed at `b868b9ee8f` (linux x86-64, z161): `cargo nextest run
  -p typed-trees-to-checked-trees --lib -E 'test(~field_endpoint_formation_never_uses_final_cancellation)
  | test(~field_endpoints_require_defined_intermediates) |
  test(~constant_rank_endpoints_preserve_landing)'` — 3/3 pass; the
  trio still stands resolved and the draft row still awaits its refresh
  item.
  Re-witnessed at `8f58b6676b0` (linux x86-64): the same filtered trio run
  passes 3/3 — no drift since the `b868b9ee8f` re-witness; adjudication
  unchanged (TERMINATION-RANKING-CHECKS lane owns the family).
- **TERMINATION-RANK-RANGE-FIELDS.** Resolved — alias of the landed
  T2C-RANK-RANGE-FIELD-ENDPOINTS surface, which already names this stub as
  covering the same work: rank-range endpoints expressed as field chains.
  `typed-trees-to-checked-trees/src/checks/termination/ranking/ranges/endpoints.rs`
  resolves `ExpressionNode::Member` chains root-to-leaf through declared
  field types (`EndpointInput`), reads the leaf's store-enforced field
  bounds, and re-checks preservation on every self edge. Re-verified green
  on linux x86-64 at `0f5ae41e7d` (same command, same 49/49; earlier stamp
  `a4ffd1aff8`):
  `cargo nextest run -p typed-trees-to-checked-trees --lib -E
  'test(/field_endpoint/)'` — 49/49 pass across field_coordinates,
  field_endpoint_arithmetic, field_endpoint_pins, field_arrivals and
  computed_field_limits. Re-verified green at `bbffdafe0498` (z116,
  linux x86-64): same command, same 49/49. No independent slice remains.
  covered — alias of landed T2C-RANK-RANGE-FIELD-ENDPOINTS
- **TRANSLATION-VALIDATION.** — verified `fcef01c59a`: duplicate pointer to the
  live `**TRANSLATION-VALIDATION.**` item in TASKS_OPTIMIZER.md, which now
  carries the verified frontier. Scope findings: `CallDynamic*` and the
  non-FMA/non-integer-compare intrinsics produce no coverage occurrences yet
  (a new occurrence replay family must precede span arms; `physical/` is
  fenced by DYNAMIC-CALL-OCCURRENCE-SPANS this wave), the hosted-builtin
  settlement catalog is complete against the closed three-variant
  `CompilerBuiltinExecution`, privileged port effects are implemented, and
  general calls remain blocked on FRAME-LAYOUT. Row consumed — the item stays
  on the optimizer board.
  Re-verified at `23338b3d093` for the DYNAMIC-CALL-OCCURRENCE-SPANS stub:
  the fenced leg it named is landed — `CallDynamic*` occurrences now bind
  their dispatch parent and span custody at `95019d341a9` and descriptor
  table-window relocation custody at `da97c882017`
  (`physical/derivation/evidence.rs` + `operator_applications.rs`
  `derive_dynamic_call_span` joins all five emitted record families);
  witness `dynamic_call_occurrence_binds_its_dispatch_role_and_parent_identity`
  re-run green on linux x86-64. The claim of this name has drained; the
  recorded residuals stay upstream-gated (e2e `physical_child_replay` waits
  on CallUnitWithDynamicArguments legalization + attachment-identity joins;
  remaining intrinsic occurrences belong to TV-OPERATOR-APPLICATIONS-REPLAY)
  and live on the canonical TRANSLATION-VALIDATION row.
  (field note: DYNAMIC-CALL-OCCURRENCE-SPANS has no row of its own on this
  board — this re-verification rides on TRANSLATION-VALIDATION; readers
  searching for the stub's verdict land here.)
- **TRANSPARENT-TRAIT-REFINEMENTS.** — in progress (branch
  `zergling/z137-transparent-trait-refinements`); parser through typed trees
  land on that branch: `trait Local = Base { machine * reaches; suspends
  false; blocks false; terminates; machine Base::req ...; }` declares a
  transparent refinement — a structural bound over existing base conformance,
  never a conformance target (`satisfies Local` and `C: T satisfies Local`
  conformance positions reject; bound carriers `L satisfies Local` are the
  legal consumer). Clause axes narrow-only: named requirements must name a
  base machine, duplicate names reject, `suspends`/`blocks` may only be
  turned off, and a non-empty clause `reaches` over an empty base row is
  widening. Remaining frontier for the next slice: per-clause `reaches`
  subset checking against the base row's normalized names (clause reach
  names are retained on `TraitRefinementClause.service_reaches`, pending a
  clause-location variant of the reach-row table), `_` reach wildcards, and
  the evidence-binder fit check that consumes the refinement bound.

  **The evidence-binder fit check landed 2026-09-21.** A binder carrying a
  refinement carrier now resolves the carrier through `refines` and checks the
  explicitly selected base conformance against the refinement's clauses, per
  `conformances.md:168-171` ("a static evidence binder may require it and
  receive an explicitly selected `Logger` conformance whose complete contract
  fits"). New module
  `typed-trees-to-checked-trees/src/monomorphization/selection/refinement_fit.rs`;
  7 tests in `src/tests/generics/conformance_binders/refinement_binders.rs`.

  Carrier resolution alone was NOT enough, and shipping it alone would have
  been unsound — it would admit non-fitting conformances silently. Three
  further sites had to resolve the requirement namespace through `refines`,
  because a refinement's own `machines`/`requires` are empty:
  `validation/.../generic_requirement.rs` (call resolution),
  `syntax-trees-to-symbol-resolved-trees/.../children/machines.rs` (the
  binder's child placeholder symbols) and its positional mirror in
  `monomorphization/body_rewriting/evidence_rewrites.rs` — the last two must
  visit in the same order (own machines, refined base, parents). All three are
  guarded on `refines.is_some()`, so ordinary traits are untouched.

  Three readings the spec does not settle, recorded so they can be revisited:
  (1) `terminates;` demands a *published* guarantee — at this stage
  `termination_plan.checked_summary` is still `NoGuarantee`, so `interface`
  identity is the only honest signal, and `effects.md:228-229` ("observing a
  non-waiting run does not" remove a marker) backs declaration over
  observation. **Consequence worth noting: the corpus fixture
  `tests/omega/pass/traits/transparent_refinement_declaration` authors
  `machine * ... terminates;` over a `Sink::write` with no `terminates`, so a
  binder over that trait would now reject. It still passes because nothing
  binds it, but the shipped example would not work if used.**
  (2) A targeted clause REPLACES the wildcard for its own requirement rather
  than meeting with it ("unmentioned ... inherit the base", and the
  order-independent meet is specified only for combining multiple
  refinements). (3) `suspends true` / `blocks true` bind nothing, since they
  only restate what the base already permits.

  Still out of scope: parameterized refinements whose `refines.arguments` are
  not pass-through (`conformance_application_arguments_match_candidate` would
  mis-compare a reordered or partially applied head, and nothing rejects it
  loudly), and the `bound.selected_conformance` branch, which checks subject
  identity but never trait identity — a pre-existing hole of the same shape.

  Verified: `typed-trees-to-checked-trees --lib` 5107/5107; `validation` +
  `syntax-trees-to-symbol-resolved-trees` + `symbol-resolved-trees-to-typed-trees`
  1749/1749. Sentinel: disabling only the fit check fails the five rejection
  tests by name while the two admission tests still pass. **The `.omg` corpus
  fixtures have since been run, once the ElementView legs unblocked
  `-p compiler`: all four `fail/traits/transparent_refinement_*` fixtures still
  reject with their pinned fragments and
  `pass/traits/transparent_refinement_declaration` still compiles — 5/5, no
  regression.**
  Re-verified at `6f918986063` (z181, DYNAMIC-CALL-OCCURRENCE-SPANS
  re-dispatch): all anchors intact — `derive_dynamic_call_span` at
  `physical/operator_applications.rs:237` (called from
  `derivation/evidence.rs:388`), witness
  `dynamic_call_occurrence_binds_its_dispatch_role_and_parent_identity`
  at `derivation/tests.rs:1958`, both landing commits ancestors of
  base. Verdict stands; residuals stay on TRANSLATION-VALIDATION.
  Fresh audit at `c3dd8016a74d` (zergling-168, linux x86-64): the z137
  branch is gone from the remote — parser through typed trees landed on
  main — and two of the three frontier items have since closed:
  per-clause `reaches` subset checking against the base row's normalized
  names landed at `75f8215cf9b10` ("check refinement clause reaches
  names against the base reach row") and clause-location binding plus
  `_` wildcard abstract rows landed at `612061f5b694d` ("refinement
  clause reaches bind at clause location, wildcard mints abstract
  rows"). Re-verified green on this chain: `nextest -p
  symbol-resolved-trees-to-typed-trees -E 'test(~refinement) |
  test(~transparent)'` -> 10/10 PASS (subset rejection, wildcard minting,
  empty-row narrowing, inheritance). The surviving leg is the
  evidence-binder fit check alone: `refines`/`refinement_clauses` still
  have zero consumers below s2t — no reads in checked-trees,
  typed-trees-to-checked-trees, lowered/terminal representations, or the
  verifier (the `refines` hits under semantics/ are English comments).
  Bound carriers `L satisfies Local` remain the legal consumer to
  implement.
- **TRUSTED-SURFACE-DIGEST-RE-RECORDING.** Standing duty, not a one-off: keep
  the trusted-surface digest ledger
  (`omega-rust/psi/semantics/terminal-verifier/src/trusted_surface/sites.rs`)
  matching the bound files, and revalidate before re-recording. The ledger
  pins each bound implementation file's SHA-256 precisely so a change to
  trusted verifier or proof-admission code cannot pass unnoticed, and
  `recorded_digests_match_the_working_tree` fails while any digest disagrees.

  This drifts repeatedly — recorded red at `0f5ae41e7d`, again at
  `ff2f489bbf`, and again at `50559da3ab` with seven stale files across
  `proof-admission/src/mathematical_core/bounded_denotation{,/addition,/subtraction}.rs`,
  `terminal-verifier/src/validation/{affine_cleanup,frontier,scalar_qualifications}.rs`
  and `.../structural_operations/structural_arguments/argument_checks.rs`.
  Each drift is one commit changing a bound file without re-recording, so
  expect this row to come back rather than treating a green run as its end.

  The obligation is the revalidation, not the re-record.
  `tools/trusted_surface_digests.py --write` is mechanical and must only run
  after every citing entry's justification — the `TrustedSurfaceEntry`
  premises, conclusion, dependencies and soundness naming that path — has
  been checked against the actual diff since the last recorded digest
  (`git log -p -S'<recorded digest>' -- .../sites.rs`, then
  `git diff <sha>..HEAD -- <bound file>`). A diff that only strengthens a
  check re-records freely; a diff that relaxes one re-records only if the
  lifted burden demonstrably moves to a dependency the entry already names.
  If a justification breaks, do not re-record that file — report it. A
  partially repaired ledger with an honest report beats a green ledger that
  lies.

  Two observations from the `b260ea749e` pass are open and worth a targeted
  follow-up, neither blocking: dropping the hook-target `entry_claims` /
  `content_entry_claims` pins lets a cleanup hook's borrowed `self` carry
  entry claims the cleanup edge does not visibly discharge, which belongs to
  `formation:machine-validation`; and the new boundary-result-qualification
  route rests on an "established by" authorization that lives under
  `formation:structural-qualification-rosters` for structural domains, which
  that pass did not re-derive.

  Acceptance: `python3 tools/trusted_surface_digests.py` exits 0 and
  `cargo nextest run -p terminal-verifier -E 'test(~trusted_surface)'` is
  green (15/15 at `b260ea749e`), with every re-recorded digest's citing
  justification revalidated in the landing commit's body.

  Duty pass at `18cebfa1062bf` (assigned row TRUSTED-SURFACE-LEDGER-
  RERECORD; the retired stub re-mines this standing duty): the ledger is
  current — `tools/trusted_surface_digests.py` reports "all recorded
  digests match the working tree" and
  `trusted_surface::recorded_digests_match_the_working_tree` PASSes on
  linux x86-64. No drift since the last record, so no entry needed
  revalidation or re-recording this pass.

  Duty pass at `39317a770b` (assigned row TRUSTED-SURFACE-DIGEST-RE-RECORD):
  the ledger had drifted — 24 stale digests traced to four commits
  (`e272856962` erased proof-only formals through call plans, 20 files;
  `d482fc2ebb` IntegerCastBound via fixed cast-identity laws, 4 files;
  `5d182b8075` machine-bound issuer identities; merge `bc0ed1f0f5` of
  `d3d3193d59` produce/check split for crash certificates). Every citing
  entry's justification revalidated: all four diffs add checks or keep a
  checked fallback — none relaxes — so all 24 re-recorded. Two new bound
  files registered (`bounded_denotation/casts.rs` under
  formation:mathematical-core, `casts/tests.rs` test-only). Verified:
  `python3 tools/trusted_surface_digests.py` exits 0 and
- **TRUSTED-SURFACE-DIGEST-RE-RECORDING.** Mined candidate — resolved:
  implemented and landed (e2974a6a800 is an ancestor of origin/main;
  the landed re-record c0b2b6e19f registered integer_operations.rs and
  re-recorded bounded_denotation). Re-verified 2026-09-20 on linux
  x86-64: `cargo nextest run -p terminal-verifier -E
  'test(~trusted_surface)'` — 15/15 pass including
  recorded_digests_match_the_working_tree. Sibling names the same op:
  TRUSTED-SURFACE-DIGEST-RE-RECORD (no row), -REFRESH, -RERECORD.
  The self-audit was red at `0f5ae41e7d` (contradicting the resolved
  siblings' "ledger is current" notes — it drifted since): `e2974a6a80`
  added `bounded_denotation/integer_operations.rs` (uninterpreted
  fixed-integer operations as applicative denotations) and bumped
  `bounded_denotation.rs` without re-recording. Re-recorded in this
  commit: `integer_operations.rs` registered as a ledger implementation
  site (`b2bcfe6f…`) and cited by `formation:mathematical-core`,
  `bounded_denotation.rs` digest re-recorded (`3d15c6eb…`). Green: 9/9
  trusted_surface on linux x86-64 including
  `recorded_digests_match_the_working_tree`.
- **TRUSTED-SURFACE-DIGEST-REFRESH** — mined candidate; verify scope then implement.
- **TRUSTED-SURFACE-DIGEST-RERECORD.** Resolved — scope verified at `0f5ae41e7d`,
  implemented on this row's branch: the ledger had renewed drift, so the
  re-record operation ran for real. `e2974a6a800` split
  `bounded_denotation/integer_operations.rs` out of `bounded_denotation.rs`
  (uninterpreted per-operation function constants applied to denoted
  operands — the `formation:mathematical-core` justification holds: no
  arithmetic law was added, the operations stay opaque). Re-recorded the
  parent digest (`3d15c6eb…`), registered the new site (`b2bcfe6f…`), and
  added the file to the formation's site list. Witness:
  Re-verified at `2ccef088fb73` (linux x86-64): the subset-check leg
  landed upstream at `75f8215cf9b1` —
  `sr2t/declarations/trait_definition.rs:225-260` resolves each clause
  reach to a boundary service and rejects when any covered machine's
  base reach row lacks it ("a refinement narrows, it cannot add a
  reach"), with the empty-base-row rejection at :213-224; `reaches _`
  is already spelled as the independent abstract row at :227-231
  (`continue` — bounded by the inherited row). What is still open: the
  clause-location `ServiceReachRowTable` variant (the code comment's own
  "pending"), and the downstream evidence-binder fit check — the
  evidence surface `typed_trees/evidence/proof_only.rs` is fenced by
  QUOTIENT-RUNTIME-REALIZATION (exp 10:57Z).
- **TRUSTED-SURFACE-DIGEST-RE-RECORDING.** Mined candidate — resolved:
  implemented and landed (e2974a6a800 is an ancestor of origin/main;
  the landed re-record c0b2b6e19f registered integer_operations.rs and
  re-recorded bounded_denotation). Re-verified 2026-09-20 on linux
  x86-64: `cargo nextest run -p terminal-verifier -E
  'test(~trusted_surface)'` — 15/15 pass including
  recorded_digests_match_the_working_tree. Sibling names the same op:
  TRUSTED-SURFACE-DIGEST-RE-RECORD (no row), -REFRESH, -RERECORD.
  The self-audit was red at `0f5ae41e7d` (contradicting the resolved
  siblings' "ledger is current" notes — it drifted since): `e2974a6a80`
  added `bounded_denotation/integer_operations.rs` (uninterpreted
  fixed-integer operations as applicative denotations) and bumped
  `bounded_denotation.rs` without re-recording. Re-recorded in this
  commit: `integer_operations.rs` registered as a ledger implementation
  site (`b2bcfe6f…`) and cited by `formation:mathematical-core`,
  `bounded_denotation.rs` digest re-recorded (`3d15c6eb…`). Green: 9/9
  trusted_surface on linux x86-64 including
  `recorded_digests_match_the_working_tree`.
- **TRUSTED-SURFACE-DIGEST-RERECORD.** Resolved — scope verified at `0f5ae41e7d`,
  implemented on this row's branch: the ledger had renewed drift, so the
  re-record operation ran for real. `e2974a6a800` split
  `bounded_denotation/integer_operations.rs` out of `bounded_denotation.rs`
  (uninterpreted per-operation function constants applied to denoted
  operands — the `formation:mathematical-core` justification holds: no
  arithmetic law was added, the operations stay opaque). Re-recorded the
  parent digest (`3d15c6eb…`), registered the new site (`b2bcfe6f…`), and
  added the file to the formation's site list. Witness:

  `cargo nextest run -p terminal-verifier -E 'test(~trusted_surface)'`
  is 15/15 PASS on linux x86-64.

  Duty pass at `138ed79a677` (assigned row TRUSTED-SURFACE-DIGEST-RE-RECORD):
  the ledger is current — `tools/trusted_surface_digests.py` reports "all
  recorded digests match the working tree" and the 15-test
  `trusted_surface` suite is 15/15 PASS on linux x86-64. No drift since the
  `39317a770b` re-record, so no entry needed revalidation this pass.

  Duty pass at `a5d958d724f1` (assigned row TRUSTED-SURFACE-DIGEST-
  RE-RECORD): the ledger is current — `tools/trusted_surface_digests.py`
  reports "all recorded digests match the working tree" and the 15-test
  `trusted_surface` suite is 15/15 PASS on linux x86-64. No drift since
  the `39317a770b` re-record, so no entry needed revalidation or
  re-recording this pass.

- **TV-GENERAL-CALLS-REPLAY.** — mined candidate; verify scope then implement.
- **TV-INTRINSIC-SPAN-ARMS.** — verified 14e6f8f72e: the span-arm surface
  Sibling re-mine names reaching this duty: TRUSTED-SURFACE-DIGEST-REFRESH,
  -DIGEST-RERECORD, -LEDGER-REFRESH, -LEDGER-RERECORD (removed at
  `af99dc50542c`), and BASELINE-VERIFIER-DIGEST-LEDGER. Ledger currently
  green at `f44a1177ed` — `recorded_digests_match_the_working_tree`
  1/1 PASS; no re-record needed this wave.
  covered — span arms complete for every occurrence-producing intrinsic family; residual is TV-OPERATOR-APPLICATIONS-REPLAY

- **TV-DYNAMIC-AND-INTRINSIC-SPANS** — mined candidate; scope verified,
  resolved — covered alias at `4b8d3f36b7`. The name joins two surfaces,
  both already adjudicated: the intrinsic half re-mines
  **TV-INTRINSIC-SPAN-ARMS**'s closed span-arm surface (every intrinsic
  family that produces coverage occurrences has its arm — IEEE FMA via
  `derive_fma_span`, integer comparisons via `derive_integer_comparison_
  span`, float comparisons via `fragment_comparison::derive`, structural
  returns through the checked-body call span; `operator_applications.rs`
  dispatch still at :50-58), and the dynamic half re-mines the landed
  CallDynamic* occurrence coverage pinned by
  `native-artifact/.../derivation/tests.rs:1959`
  `dynamic_call_occurrence_binds_its_dispatch_role_and_parent_identity`
  (green at `28a3cc7fea`). Re-verified the demand side at this revision:
  `replay_scope.rs` still replays exactly the four families
  (`local_initializers`, `structural_returns`, `float_comparisons`,
  `integer_comparisons`), so a further arm has nothing to join —
  occurrence production for the remaining intrinsic kinds stays with
  **TV-OPERATOR-APPLICATIONS-REPLAY** per both resolved rows. Sibling
  stubs on the same surface: INTRINSIC-PHYSICAL-SPAN-ARMS,
  REMAINING-INTRINSIC-SPAN-ARMS. No independent slice.
  covered — alias joining resolved TV-INTRINSIC-SPAN-ARMS and landed DYNAMIC-CALL-OCCURRENCE-SPANS; residual is TV-OPERATOR-APPLICATIONS-REPLAY
- **TV-GENERAL-CALLS-REPLAY** — mined candidate; verify scope then implement.
- **TV-INTRINSIC-SPAN-ARMS** — verified 14e6f8f72e: the span-arm surface
  for every intrinsic family that produces coverage occurrences is
  complete — IEEE FMA joins `x86_scalar_fma_occurrences` fragments
  (`derive_fma_span`), integer comparisons join
  `semantic_code_attribution` rows with relocation-overlap rejection
  (`derive_integer_comparison_span`), float comparisons take the
  fragment-publication arm, and structural returns arrive through the
  checked-body call span. All other intrinsic realizations produce no
  occurrences — `checked_boundary_operator_occurrences` replays only the
  four families — so an arm has no demand side and adding one before the
  occurrence replay family lands would be dead code joining nothing.
  Occurrence production for the remaining intrinsic and dynamic-call
  operations is **TV-OPERATOR-APPLICATIONS-REPLAY**'s scope;
  `native-artifact/src/physical` is fenced by
  DYNAMIC-CALL-OCCURRENCE-SPANS this wave. Row consumed — the residual
  stays on the live **TRANSLATION-VALIDATION.** item in
  TASKS_OPTIMIZER.md.
  covered — span arms complete for every occurrence-producing intrinsic family; residual is TV-OPERATOR-APPLICATIONS-REPLAY
- **TV-OPERATOR-APPLICATIONS-REPLAY.** Mined candidate; scope verified at
  `a51cb805cc1` — real frontier, fenced this wave. Names the occurrence
  replay residual recorded on resolved sibling TV-INTRINSIC-SPAN-ARMS:
  `lowered-psi-to-terminal-psi/.../boundary_operator_custody/replay_scope.rs:102-105`
  replays exactly four families (local_initializers/IEEE FMA,
  structural_returns, float_comparisons, integer_comparisons); remaining
  intrinsic kinds and the `CallDynamic*` operations produce no occurrences,
  so their span arms would join nothing. Ordered legs: (a) new occurrence
  replay family here with demand/realization companions; (b) span arms
  joining the emitted dynamic-call records (`dynamic_calls`,
  `stored_dynamic_calls`, `dynamic_parameter_calls`, `forwarded_dynamic_*`)
  — descriptor-materializing records additionally need relocation custody
  beyond the single window `derive_span` models (AArch64 table addressing
  emits two windows) plus a conformance-table symbol join;
  parameter-routed calls are register-indirect. Blocking fence:
  `native-artifact/src/physical` is path-claimed under
  PHYSICAL-ACCESS-PROFILES (Devin / dev-88738) this wave, and the adjacent
  surface is item-claimed under INTRINSIC-PHYSICAL-SPAN-ARMS (Jarod,
  ~05:04Z) — the demand/realization companions live inside that fence.
  The replay-side leg alone produces occurrences with no downstream
  consumer; the full slice resumes after the fences drain.
- **WINDOWS-FILE-TIME-CARRIER-RESPELL.** — mined candidate; scope verified,
  covered — the stub is the same "unsigned carrier" clause of
  WINDOWS-SET-FILE-TIME-RESPELL as resolved sibling
  WINDOWS-FILE-TIME-UNSIGNED-RESPELL (merged with
  FILESYSTEM-WINDOWS-FILETIME-RESPELL): the respelling landed at
  `ff782bdf21` — `tests/omega/pass/filesystem/windows_set_file_time_exit/main.omg`
  assembles `st_mtime` through the u64 carrier (`widen_u8_to_u64` per byte,
  `narrow_u64_to_i64_wrapping` once at landing, main.omg:74-81) instead of
  the overflowing `widen_u8_to_i64(byte) << 56` idiom, and the fixture is
  registered in `CHECKED_ONLY_PASS_CANARIES` (`canary_suite.rs:1056`).
  Re-verified on tip `43104bde655` (linux x86-64, source inspection): the
  u64-carrier assembly is still in place and the old idiom does not recur;
  the fixture's native-execution leg stays Windows-gated and unmeasurable
  on this host, as `wiki/drafts/known_baseline_failures.md` records. The
  parent row owns the residual bookkeeping; no independent slice exists
  here. Sibling stubs on the same clause: WINDOWS-FILE-TIME-UNSIGNED-RESPELL,
  WINDOWS-SET-FILE-TIME-CARRIER, WINDOWS-SET-FILE-TIME-UNSIGNED-RESPELL.
- **WINDOWS-FILE-TIME-UNSIGNED-RESPELL.** — mined candidate; scope verified,
  dispatch (~10:19Z). Still no implementation slice exists to claim.
  Earlier same-day verification at `e5bbe53956f` (z148) reached the same
  gate: `CompositionCrossActivationEdges::NotRetained` still publishes
  (composition_model/mod.rs:118-226) and the spec deferral text was
  unchanged (concurrency.md:138).
- **WINDOWS-FILE-TIME-UNSIGNED-RESPELL** — mined candidate; scope verified,
  covered. The stub is the "unsigned carrier" clause of
  WINDOWS-SET-FILE-TIME-RESPELL verbatim (merged with
  FILESYSTEM-WINDOWS-FILETIME-RESPELL), landed at `ff782bdf21` — the
  `windows_set_file_time_exit` fixture assembles the `st_mtime`
  nanos-through-100ns conversion through the u64 carrier
  (`widen_u8_to_u64`/`narrow_u64_to_i64_wrapping`) instead of the overflowing
  `widen_u8_to_i64(byte) << 56` idiom. `wiki/drafts/known_baseline_failures.md`
  names the same respelling and records that the fixture is Windows-gated, so
  neither its failure nor its repair can be measured on a non-Windows host.
  The parent row owns the residual bookkeeping; no independent slice exists
  here. Re-verified at `59610bf809` (linux x86-64): main.omg still assembles
  `st_mtime` through `widen_u8_to_u64` per byte with
  `narrow_u64_to_i64_wrapping` at landing; the old `widen_u8_to_i64`
  idiom does not recur. Sibling stubs resolved on the same clause:
  WINDOWS-FILE-TIME-CARRIER-RESPELL, WINDOWS-SET-FILE-TIME-CARRIER,
  WINDOWS-SET-FILE-TIME-UNSIGNED-RESPELL.
- **WINDOWS-SET-FILE-TIME-RESPELL.** Resolved — the named repair
  landed at `ff782bdf21` and stands at `b868b9ee8f270`:
  `tests/omega/pass/filesystem/windows_set_file_time_exit/main.omg`
  assembles `st_mtime` in the unsigned `u64` carrier
  (`widen_u8_to_u64(byte) << N` per byte, `narrow_u64_to_i64_wrapping`
  once at the landing — `255 << 56` stays Exact-representable only in
  `u64`; the overflowing `widen_u8_to_i64(byte) << 56` idiom does not
  recur), and the fixture is registered in `CHECKED_ONLY_PASS_CANARIES`
  (`canary_suite.rs`). The native-execution leg is Windows-gated — the
  entry's `Main::fs` wants a selected fused `FilesystemHost` provider —
  so it is unmeasurable on this linux x86-64 host and stays attributed
  there (`wiki/drafts/known_baseline_failures.md`,
  the prior canary repair disposition). No slice remains on this
  host. Sibling stubs on the same clause: WINDOWS-FILE-TIME-CARRIER-RESPELL,
  WINDOWS-FILE-TIME-UNSIGNED-RESPELL, WINDOWS-SET-FILE-TIME-CARRIER,
  WINDOWS-SET-FILE-TIME-UNSIGNED-RESPELL, FILESYSTEM-WINDOWS-FILETIME-RESPELL. Re-verified at `8f58b6676b`: `ff782bdf21` is
  an ancestor of base; `main.omg:74-82` still assembles `st_mtime` via
  `widen_u8_to_u64` per byte with `narrow_u64_to_i64_wrapping` at landing,
  and the fixture stays in `CHECKED_ONLY_PASS_CANARIES`
  (`canary_suite.rs:1066`).
- **WINDOWS-SET-FILE-TIME-UNSIGNED-RESPELL.** — mined candidate; scope verified, covered — the stub is the "unsigned carrier" clause of WINDOWS-SET-FILE-TIME-RESPELL verbatim (merged with FILESYSTEM-WINDOWS-FILETIME-RESPELL): `known_baseline_failures.md` already names the respelling — `widen_u8_to_i64(byte) << 56` intermediates (about 1.84e19/4.28e9 against the i64/i32 ceilings) assemble in the unsigned carrier of the field's own width and reinterpret once at landing — and records that `tests/omega/pass/filesystem/windows_set_file_time_exit` is Windows-gated, so neither its failure nor its repair can be measured on a non-Windows host. The parent item owns the leg; no independent slice exists here. Sibling stubs on the same clause: WINDOWS-FILE-TIME-CARRIER-RESPELL, WINDOWS-FILE-TIME-UNSIGNED-RESPELL, WINDOWS-SET-FILE-TIME-CARRIER.
- **WINDOWS-SET-FILE-TIME-UNSIGNED-RESPELL.** — mined candidate; scope verified, covered — the stub is the "unsigned carrier" clause of WINDOWS-SET-FILE-TIME-RESPELL verbatim (merged with FILESYSTEM-WINDOWS-FILETIME-RESPELL): `known_baseline_failures.md` already names the respelling — `widen_u8_to_i64(byte) << 56` intermediates (about 1.84e19/4.28e9 against the i64/i32 ceilings) assemble in the unsigned carrier of the field's own width and reinterpret once at landing — and records that `tests/omega/pass/filesystem/windows_set_file_time_exit` is Windows-gated, so neither its failure nor its repair can be measured on a non-Windows host. The parent item owns the leg; no independent slice exists here. Sibling stubs on the same clause: WINDOWS-FILE-TIME-CARRIER-RESPELL, WINDOWS-FILE-TIME-UNSIGNED-RESPELL, WINDOWS-SET-FILE-TIME-CARRIER. Parent stub **WINDOWS-SET-FILE-TIME-RESPELL** resolved at `138ed79a677` (linux x86-64, source inspection): the `ff782bdf21` respelling still stands — `tests/omega/pass/filesystem/windows_set_file_time_exit/main.omg:74-82` assembles `st_mtime` through `widen_u8_to_u64` per byte with `narrow_u64_to_i64_wrapping` at landing, no `widen_u8_to_i64` recurrence, and the fixture stays registered in `CHECKED_ONLY_PASS_CANARIES` (`canary_suite.rs:1056`); its native-execution leg remains Windows-gated and unmeasurable on this host. The respell item's whole surface closes on that pin.
- **ZERO-BYTE-ARRAY-FENCE-PLACEMENT.** Resolved — re-mines the fence-placement leg of the landed **FUZZ-CLUSTER-ZERO-BYTE-ARRAY** row. Re-verified at `2bbe4727a2c`: the fail fixtures and the `InvalidStructuralArrayLength` fence at `structural_types.rs:51` are unchanged. The fences are placed and pinned: check-time rejection covers unprovable `x[0]` into `[u8; 0]` and non-exact fixed literals (`fail/data/zero_length_byte_array_{index_rejected,literal_arity_rejected}` + `zero_length_byte_literal_length_rejected`, driven by `zero_length_byte_array_use_fences_reject_at_check`), and the non-scalar-leaf `[T; 0]` fence sits in the terminal verifier at `terminal-verifier/src/validation/foundation/structural_types.rs:51` (`InvalidStructuralArrayLength`), mirrored in optimization-unit-semantics — i.e. the placement decision is already made and named. Sibling re-mines of the same cluster: FIXED-ARRAY-ZERO-EXTENT-FENCE, ZERO-EXTENT-BYTE-ARRAY-ADMISSION, ZERO-EXTENT-BYTE-ARRAY-FENCE, ZERO-LENGTH-BYTE-ARRAY-ADMISSION-FENCE, ZERO-LENGTH-BYTE-ARRAY-FENCE, ZERO-LENGTH-FIXED-BYTE-ARRAY-FENCE. The remaining named residual is the native-route corpus pin, which the landed row assigns to the ACTIVE_FAIL roster — not this stub.
- **ZERO-EXTENT-BYTE-ARRAY-ADMISSION.** Resolved — re-mines the admission
  half of the landed **FUZZ-CLUSTER-ZERO-BYTE-ARRAY** row with the same
  record as resolved siblings ZERO-BYTE-ARRAY-FENCE-PLACEMENT and
  ZERO-EXTENT-BYTE-ARRAY-FENCE: `[u8; 0]` admits at check in every value
  position (locals, constants, parameters, returns, record fields, nested
  arrays), constructed exactly by `[]`/`""` — pinned by
  `pass/collections/zero_length_byte_array_admission` and
  `zero_length_byte_array_is_admitted_at_check`. The use-site fences and
  the non-scalar-leaf `[T; 0]` `InvalidStructuralArrayLength` fence
  (`structural_types.rs:51`) stand unchanged; the only named residual —
  a native-route corpus pin — belongs to the ACTIVE_FAIL roster, not
  this stub.
- **ZERO-EXTENT-BYTE-ARRAY-FENCE.** Resolved — re-mines the landed
  **FUZZ-CLUSTER-ZERO-BYTE-ARRAY** row with the same record as resolved
  siblings ZERO-BYTE-ARRAY-FENCE-PLACEMENT and
  ZERO-LENGTH-FIXED-BYTE-ARRAY-FENCE: `[u8; 0]` admits at check in every
  value position, constructed exactly by `[]`/`""`; the use-site fences
  are pinned (unprovable `x[0]` and non-exact fixed literals reject at
  check via `fail/data/zero_length_byte_array_{index_rejected,
  literal_arity_rejected}` + `zero_length_byte_literal_length_rejected`),
  and the non-scalar-leaf `[T; 0]` fence sits in the terminal verifier as
  `InvalidStructuralArrayLength` (`structural_types.rs:51`,
  BASELINE-VERIFIER-ZERO-BYTE-ARRAY-FENCE). The only named residual — a
  native-route corpus pin — belongs to the ACTIVE_FAIL roster, not this
  stub.
- **ZERO-LENGTH-FIXED-BYTE-ARRAY-FENCE** — mined candidate; scope verified,
  resolved — re-mines the landed **FUZZ-CLUSTER-ZERO-BYTE-ARRAY** row
  (TASKS.md:5804). `[u8; 0]` admits at check in every value position,
  constructed exactly by `[]`/`""` (`pass/collections/
  zero_length_byte_array_admission`, `zero_length_byte_array_is_admitted_
  at_check`); the use-site fences are pinned — unprovable `x[0]` index and
  non-exact fixed literals reject at check (`fail/data/
  zero_length_byte_array_{index_rejected,literal_arity_rejected}` +
  `zero_length_byte_literal_length_rejected`, driven by
  `zero_length_byte_array_use_fences_reject_at_check`), and the
  non-scalar-leaf `[T; 0]` fence sits in the terminal verifier as
  `InvalidStructuralArrayLength` (BASELINE-VERIFIER-ZERO-BYTE-ARRAY-FENCE).
  The only named residual — a native-route corpus pin — belongs to the
  ACTIVE_FAIL roster per the landed row, not this stub. Sibling re-mines
  already resolved with this same record: ZERO-BYTE-ARRAY-FENCE-PLACEMENT,
  ZERO-EXTENT-BYTE-ARRAY-ADMISSION, ZERO-EXTENT-BYTE-ARRAY-FENCE;
  remaining stubs: FIXED-ARRAY-ZERO-EXTENT-FENCE,
  ZERO-LENGTH-BYTE-ARRAY-ADMISSION-FENCE, ZERO-LENGTH-BYTE-ARRAY-FENCE.

  independent slice exists here — the enumerated legs are the parent row's
  own remaining-work list.
## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
- Windows AArch64 has no `NativeTarget` constructor, so that ABI combination
  stays unwitnessable until the target vocabulary grows one.
