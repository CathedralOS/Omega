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
  `ef6682f75f48f9a750bc8f7594cc1f04e78ddf60`, a descendant of freshly fetched
  application `origin/main` at `52bcf254c983a9ae9bf6e0c3661cafd656ca056b`
  (2026-09-21). This is unpublished cumulative work, not divergent ancestry.
  Preserve the geometry and scalar scan/RLE/dispatch code in the pin; do not
  re-port it. Squalr uses only `main`: publish cumulative work there without
  force-pushing, then pin a tested commit on that line.

  Run `python samples/apps/squalr/tools/verify.py native --timeout 600 --omega <binary>`
  through ordinary package review on matching hosts. Connect
  **VEC-NATIVE-GROWTH** to partitioned scan results and repeated filtering;
  **BUMP-ALLOCATOR-CANARY** and **PLAN-LAID-VIEWS** supply allocation and placed
  access. Follow the application's actual next missing operation rather than
  waiting for every related task to close.

  Preserve geometry acceptance on Windows and Linux ARM64, alignment parsing
  and refusal/error-text parity, `set_alignment` calls, native clone/serialization
  (including case-bearing alignment values), and token/named ordering agreement:
  order by base address with equal-base equivalence while `equals` remains
  size-sensitive. At the pin, parsing and named region ordering are absent;
  clone/wire helpers exist but in-package checking does not close application
  execution. Keep these mapped gaps explicit in the app-owned parity work.
  Debug-assertion parity uses the settled assertion/build contracts, not an
  invented implicit debug/release switch.

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

  Also close producer-side failed publication in
  `compilation-report/src/compile_report.rs`: flat publication currently writes
  Psi bytes before their companion, so a later write/replacement failure can
  leave the new artifact beside the old proof. Stage and validate requested
  pairs before success; failure must not associate stale evidence with new bytes
  or downgrade requested PCC to ordinary success. Exercise failure between pair
  members, request-on/request-off replacement and retry, separately from receiver
  mismatch rejection. Successful republication tests do not cover interruption.

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
  - Connect source-established sequence/range predicates to element proof
    use through selected mathematical evidence. The isolated
    `ForAllInRangeFact` / `QuantifiedRangeFact` helpers have no source
    producer or entailment consumer. Preserve exact collection, predicate,
    range and subject identity through mutation and serialization. In-range
    element use must check independently; wrong collection/predicate,
    out-of-range and invalidated facts must reject. Do not introduce a second
    calculus or quantifier keywords to wire these helpers.
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

  Preserve declared borrowed-receiver custody when an attached scalar-returning
  callee does not read `self`: the scalar Graph signature still omits it, while
  call selection prefers Graph over Operations. The Operations receiver fix
  therefore does not close this route. Caller operands and selected callee
  signatures must agree without a dummy field read. Exercise unread and observed
  receiver bodies through Terminal production and native execution, retaining
  duplicate-overload rejection.

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
  (`terminal-interpreter/src/calls.rs`).
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

- **DISPATCHED-CALL-RECOGNIZER-GAP.** Three call recognizers exclude quotient
  and private-layout requests but still accept a call whose
  `static_requirement_dispatch` is set, so a satisfier's private closed
  realization (the rewritten `target_symbol`) flows through them as if it were
  the plain application the call spells:
  `typed-trees-to-checked-trees/src/execution/unit/returns/guarded_call_returns.rs`
  (guarded value-call return plans),
  `validation/src/proof_contracts/proof_embeddings/calls.rs` (`embed` sources)
  and `validation/src/proof_contracts/quotients/relation_plan/theorem_schema_verification.rs`
  (theorem-schema verification, which also compares `machine_arguments` against
  the representative telescope). Every other recognizer in those crates now
  uses `TableCallExpression::selects_only_nominal_route`
  (`6f9a563e19`) and rejects such calls; these three were
  left as authored because narrowing them is a semantic change. Decide per
  site whether a dispatched call is admissible there (the public requirement,
  not the rewritten symbol, is the contract and proof interface per the
  predicate's doc) and either adopt the predicate or document why the
  dispatched realization is the right operand.

  Acceptance: each of the three sites either uses `selects_only_nominal_route`
  with a fixture in which a static-requirement-dispatched call is rejected (or
  handled through the requirement's contract) at that site, or carries a
  comment naming why the dispatched realization is admissible there, with a
  test pinning that admission; the residue grep
  `grep -rn 'private_layout_operation.is_some()' omega-rust --include='*.rs' | grep -v tests`
  then lists only the fingerprint writer in `proof/mathematical_signature.rs`
  and the resolution-stage producer.

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
  - Rejoin composed scalar calls with structural/boundary callees, structural
    arguments and claim transfers before structural returns.
    `unit/attached_unit/composed_control/admission.rs::retain_scalar_call`
    still refuses this custody. Extend ordinary ordered operations, not a
    fallback recognizer. Preserve
    `owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel`:
    two borrowed-output writes precede the exact record return, finish at 73,
    and survive fuel exhaustion without replay. Retain record/array/generic
    return-substitution and affine-transfer rejection controls.
  - Complete structural/Unit control-flow composition with computed successor
    arguments, preserving owned values, loans and ordered operations through
    joins and calls. `unit/structural_unit_control.rs` still rejects checked
    expressions in its specialized scalar-successor route. Extend ordinary
    argument evaluation/custody, not another whole-body recognizer. Preserve
    structural-call publication controls and exercise source-rooted branch/join
    customers through Terminal replay and matching-host native execution;
    **GENERAL-CYCLIC-EXECUTION** owns cyclic control.
    General non-scalar-leaf zero-length arrays still lack native structural
    admission; checked empty construction and the native rejection pin already
    exist. Preserve empty-index/literal-arity rejection as storage support grows.
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
  Preserve `cross_package_visibility`'s public dynamic return with opaque
  producer-selected private evidence, exact receiver-loan origins and rejection
  of consumers directly selecting a dependency's private conformance.

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
  Historical package-input failure probes are
  `free_process_exit_helper_lowers_without_a_synthetic_attachment`,
  `accepted_package_uefi_binding_selects_exact_ordinary_schema` and
  `instantiated_methods_keep_each_package_use_authority`. Fixture/provider and
  name-collision repairs have since landed: reproduce before assigning more
  repair, retaining exact selection and package-use authority checks.

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
Include `terminal_psi_source` and `terminal_psi_source_payloadless_optimizer`;
old baseline exclusions do not remove them from the full native matrix.
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
  The recorded `brightness_control` Windows authored-entry rejection is a
  concrete probe for `basics_samples_compile_from_authored_program_entry_bindings`;
  reproduce it before repair rather than retaining its old failure as current.
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
  Include `windows_set_file_time_exit_canary_runs`: SetFileTime followed by
  `_stat64` must read back Unix time 1,500,000,000 and exit 70. Its corrected
  unsigned byte assembly is not evidence that Windows execution passed.


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


Build/packages:


Platform/cross-host (structurally gated — document host limits):

- **MACOS-X64-HOST-PROFILE.** Execute the compiler/runtime route on an Intel
  macOS host. Target/provider recognition, exact physical-entry reconstruction,
  accepted package binding, hosted receiver and x86-64 Mach-O import pairing
  have implementations; cross-target checks do not establish host execution.
  Exercise emitted Mach-O entry/exit, receiver and provider/import behavior,
  recording exact commands, commit, observations and justified skips. Attribute
  failures to their capability owners. This additional platform task is separate
  from the completion contract's four required release hosts and does not depend
  on an Alpha bootstrap seed.
- **DEVICE-EXTENT-ACCESS.** Complete the admitted device-loan/ordering route
  through actual release and CPU-view restoration under
  [device custody and ordering](wiki/spec/resources/device_access.md).
  `extents::{external_loans,ordering_events}` already carries confinement,
  role-specific event identity, exact coordinates, device, request and runtime
  scope, with coverage/substitution controls. Those records alone do not
  establish a Stable view, completion or emitted ordering.

  Connect the provider's admitted roles to the live external loan: an intersecting
  write invalidates the publication; notification consumes valid publication;
  completion/acquisition restores CPU access only after exact borrower release
  and coherence obligations. Preserve scoped ordering through Terminal and
  target realization, including instruction-free coherent implementations.
  Reject missing/extra/duplicate roles, stale publication, wrong request/device/
  scope/mapping and premature restoration without consuming retry custody.
  Pin failed-start return, failed-completion retention and permitted suspension.

  Exercise the shared-memory customer using an explicit atomic/coherence lease
  or copy-and-validate/revoke-and-validate route as the peer contract requires.
  Hostile writable peers cannot supply Stable access merely by asserting release.
  Protocols, queues, isolation and translation policy remain provider/OS code;
  do not add compiler-owned drivers or treat a CPU barrier as device completion.
- **EXTERNAL-DATA-SCHEMA-CONVERSION.** Finish and verify the authored
  preserving-codec customer under the [codec contract](wiki/spec/layouts/codecs.md),
  not a compiler-special `decode_preserving` path. Admission already recognizes
  `PreservingDecode`; `wire/wire_preserving_decode_relay_exit` validates a
  known byte and retains the exact ordered unknown tail in a borrowed
  `OpaqueWireRemainder`. Its landing established checked compilation, not
  native success. Reproduce
  `versions_wire_and_const_lengths::wire_preserving_decode_relay_exit_canary_runs`
  before attributing a current repair.

  Complete faithful relay through the same selected codec: preserve unknown
  bytes/order while independently handling the validated known value, reject
  incompatible codec identity and invalid known input, and retain the input
  loan while the borrowed remainder is live. Use ordinary library machines and
  existing call/storage/lifetime owners. Acceptance: the source customer runs
  on a matching host, relay output preserves the required opaque bytes/order,
  and codec-substitution, invalid-input and loan-conflict controls reject.
  Preserve unique historical-migration selection; generated-codec verification
  and trust remain with their existing owner.

Squalr app lane (source: `samples/apps/squalr/TASKS.md`):




## Mined items (deep-mine sweep, wave 9)


- **ASM-INSTRUCTION-CATALOG-EXPANSION.** Carry accepted checked assembly
  through the ordinary source-to-native pipeline under
  [assembly](wiki/spec/language/assembly.md) and the
  [catalog](omega-rust/psi/foundation/language-core/inline_assembly.md).
  Catalog/checking coverage is not executable support.

  Resume at `typed-trees-to-checked-trees/src/execution/unit/calls/call_operations.rs`:
  dedicated asm-call planning handles `AsmPortOut`, while
  `asm_value_intrinsic_result_types_reach_the_call_operation_frontier` pins
  missing operation plans for value intrinsics. Complete checked plans,
  lowering, Terminal encoding, independent verification, target realization
  and final-span/state evidence together. Reuse ordinary assignments/transitions
  where they express the operation: canonical unordered `ldr`/`str` already
  lower through typed places. Do not require a new opcode per source spelling
  or a separate asm-only body recognizer.

  First acceptance is the existing `canary_suite/inline_asm.rs` x86 fence,
  interrupt, flags, MSR and control-register byte-emission customers, followed
  by its checked-only pipeline-directive/cache and AArch64 system-register
  fixtures. Preserve exact target, operand widths, authority/reach, clobbers,
  ordering and modeled exits through emitted bytes. Wrong-target, missing
  authority, stale postcondition, invalid saved-place, omitted clobber and
  mismatched evidence controls must reject. Cross-compilation/byte assertions
  do not establish privileged execution on a host.

  Further memory, atomic, barrier-option, cache/TLB and mode-transition families
  need a customer and complete contracted operands/effects, not just more
  recognized mnemonics. Memory access must retain authorized extent/view,
  bounds, alignment, initialization and access permission; numeric addresses
  confer none. Unknown instructions and hidden/unmodeled exits keep rejecting.
  **PRIVILEGED-PORT-EFFECT-SETTLEMENTS** owns the port-read/write provider
  adapter integration; do not duplicate that assignment.

  Preserve the settled build authority route: hosted grants for `port_io` and
  `interrupt_table` are independent; machine-owner authority remains
  freestanding-only. Reach is not authority, and helper calls cannot hide
  obligations. Keep the closed terminal-authority inventory and commitment
  consistent when catalog mechanisms change; exercise an ordinary native
  compile so a production inventory assertion cannot hide behind source checks.

  Embedded target-specific assembly remains the `interpreted-inline-assembly`
  owner question, not a blocker for native support. The checked interpreter's
  unit-return arms in `interpreter/evaluator/statements_and_calls.rs` do not
  establish fidelity for unmodeled halt/register/cache effects. Unsupported
  effects must reject, while genuinely elidable hints retain their catalog
  meaning; do not silently turn all asm into successful no-ops.
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



- **CANARY-ACQUIRES-THROUGH-HELPER-RETURN** — mined candidate; scope verified, real residual — the canary exists and is rostered (`tests/omega/pass/capabilities/acquires_through_helper_return`, in `tests/canary_suite.rs` + `tests/fixture_rosters/reports_and_capabilities.rs`), but the rostered fixture is red on `1fc01bb690`: `pass_canaries_compile` filtered to it fails at native-artifact Terminal production — `InvalidUnitMachinePlan { machine: "Main::main", reason: "attached Unit closure is missing a checked transitive machine plan", omission: "`Main::main` has no admitted body (local construction stopped at signature)" }`. The remaining leg is the checked/lowering gap that stops `Main::main`'s local construction at the signature (authority-propagating helper-return shape reaches no admitted body), not a missing corpus member. Fixture path is under a live same-item claim (Devin / cathr-acquires-helper-return).
- **CANARY-CORE-NAME-COLLISION** — mined candidate; verify scope then implement.

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
- **COMPILER-PASS-PROFILE-TIMINGS.** Preserve timing opt-in through retained
  and direct Terminal production. Both paths in
  `checked-compilation-to-terminal-artifact/src/terminal_artifact.rs`
  construct enabled Psi collectors even when surrounding `CompileTimings`
  is disabled. Carry the request's collection state into the existing
  Psi-owned `TerminalProductionTimings`, without a Psi-to-Omega dependency.
  Internal stage instrumentation and prepared-project flag/report forwarding
  already exist; do not rebuild those mechanisms.

  Acceptance: untimed production performs the same work without optional clock
  collection or retained timing rows; timed production retains its stage ladder
  through direct and prepared-project reports. Preserve exact error propagation,
  stderr-only CLI reporting and absence of debug files. Existing disabled-merge
  tests prove row suppression, not absence of inner measurement; cover the
  actual collection choice on both production paths.

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
- **GENERIC-RETURNED-VIEW-LIFETIMES.** Complete caller-side attribution of
  generic returned views under
  [returned views](wiki/spec/language/lifetimes.md#returned-views).
  Exact selected callable/argument substitution currently admits template-dependent
  results only when the complete instantiated frontier is view-free. Carry the
  instantiated result-to-input relation and exact loans for view-bearing results.
  Owners: `checks/borrows/elision/templates.rs`, `borrow/view_link.rs` and caller
  loan attribution. Reuse the shared complete-frontier query; unresolved structure
  is not an empty frontier, and discarding a result cannot bypass call admission.

  Acceptance: extend
  `generic_frontiers/static_calls.rs::exact_static_callable_substitution_allows_only_closed_view_free_results`
  to its view-bearing `Outcome<i32, Job>` customer, retaining original backing,
  path and access. Reject writes to every possible live source, unrelated/local
  backing, access escalation, missing/conflicting callable substitution and
  unresolved frontiers. Preserve view-free admission, concrete carriers and
  ambiguous-elision rejection. Explicit same-lifetime multi-source unions already
  work; general authored outlives syntax is outside the current contract, not an
  implementation prerequisite.
- **C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT.** Recheck and resolve excessive
  compile time for
  `nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`
  in `checked-trees-to-lowered-psi --test suite`. Historical timeout evidence
  predates further algorithmic repairs; current completion/time is unverified.
  Run the unreduced customer with an explicit test timeout, then profile current
  checking/lowering/certificate work if still slow. Improve measured work without
  abandoning obligations or weakening independent verification. The first 72
  top-level `&&` conjuncts (counting the leading parenthesized triple as one)
  remain a profiling aid, not acceptance.

  Accepted-premise indexing, unchanged-module reconstruction reuse and reachable
  equality-roster selection already exist. Instrument the actual producer;
  `OMEGA_PROOF_MEASUREMENTS` does not account for all certificate work.
  Acceptance: the unreduced test completes its assertions within an ordinary
  test timeout, proof/reconstruction controls remain valid, and subsequent crate
  `--no-fail-fast` validation has no timeout member. Algorithmic repair needs no
  owner decision. Introducing an aggregate proof-work refusal ceiling requires
  the existing `compile-time-proof-work-ceiling` decision in OWNER_QUESTIONS.md.

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
- **HOSTED-BUILTIN-SETTLEMENT-EXPANSION** — mined candidate; verify scope then implement.
- **HOSTED-RECEIVER-SERVICE-CARRIER** — mined candidate; verify scope then implement.
- **HOSTILE-SHARED-MEMORY-PLACEMENT** — mined candidate; verify scope then implement.
- **HOSTILE-SHARED-MEMORY-REMAPPING** — mined candidate; verify scope then implement.
- **LEGACY-COMPATIBILITY-WRAPPER-PRUNING** — mined candidate; verify scope then implement.
- **MODEL-FREE-CANDIDATE-SEARCH** — mined candidate; verify scope then implement.
- **MODULE-CONSTANT-BUILTIN-CARRIER** — mined candidate; verify scope then implement.
- **MULTI-TARGET-BATCH-MANIFEST** — mined candidate; verify scope then implement.
- **NATIVE-DIFF-HOSTED-RECEIVER-HARNESS-MIGRATION** — mined candidate; verify scope then implement.
- **NATIVE-I32-REMAINDER-LEGALIZATION** — mined candidate; verify scope then implement.
- **OBLIGATION-NORMALIZED-IDENTITY** — mined candidate; verify scope then implement.

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
  and call-occurrence spans) belong to TRANSLATION-VALIDATION in
  TASKS_OPTIMIZER.md, not to access profiles.
  Re-verified at `758e8ad9e2` on linux x86-64: both access-profile pins
  intact at `mixed_structural_scalar.rs:184`/`:199` and the foreign-lane
  rejection pin at `physical/derivation/tests.rs:645`; the settled
  verdict stands (dispatcher re-dispatched the resolved name).
- **PHYSICAL-ENTRY-BRIDGES** — mined candidate; scope verified, covered — the resolved sibling PHYSICAL-ENTRY-END-TO-END row names this stub as a re-mine of the "end-to-end physical entry" note in `wiki/language_guide/chapter_3_machines.md` ("selecting and checking an entry does not claim that its native bridge has been installed"), which the note itself assigns to ENTRY-CONTENT-ROOTS. The physical-entry-bridge acceptance leg already passes natively on linux x86-64 at `cdee121ee9` (`samples_with_documented_exit_run_correctly` under `OMEGA_SAMPLE_RUNTIME_FILTER==cli__basics__number_guess`: published process with `Service<Console>` receiver compiles to a native artifact and runs to exit 70); the intrinsic `Service<R>` carrier cut landed at `f705cbdb5`. The epic's remaining bridge legs (receiver nominal-cleanup/completion occupancy, per-host legs) stay with ENTRY-CONTENT-ROOTS and are live-fenced this wave (program-entry-plan, external-roots `ProgramLocalRootInstallationLedger`, image-emission hosted_receiver). No independent slice exists here.
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
- **PRODUCER-CHECKER-BOUNDARY-AUDIT.** mined candidate — resolved: re-mine of the producer/checker seam family already bounded in `wiki/drafts/producer_checker_decision_sharing_audit.md` (verified `12dea522b2`; the ledger names this cluster explicitly — BOUNDARY-AUDIT, DECISION-SEPARATION, DECISION-SHARING-AUDIT, SHARING-AUDIT are the same seam). Every reachable surface is Re-derived or Bound: lock decisions are informational history bound to `changes.fingerprint()`, PCC claim fields are recomputed by `verify_pcc_claim_fields` under the receiver's policy, component descriptions are "trusted for nothing" (re-decode + consumer-supplied admission profile), placed-image evidence extents/digests/seals are re-derived from committed bytes. Re-witnessed at `54e321bdf0`: `derivation_cache.rs:154` still re-runs `candidate.verify()` through the admission kernel on every hit (rejected hits fall through to fresh derivation, counted in `rejected_candidates`), and `independent_components.rs:54` `verify_independent_component_descriptions` still re-verifies under the build's own admission profile. Open residual (exhaustive whole-tree verifier-callsite audit) is recorded on sibling PRODUCER-CHECKER-DECISION-SHARING-AUDIT, not here.
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
- **PRODUCER-CHECKER-DECISION-SHARING-AUDIT** — mined candidate; bounded audit at `8734480a01`, re-verified at `f44a1177ed` (all four mechanisms unchanged), no decision sharing found on the named reuse surfaces. `proof/src/checker/derivation_cache.rs` retains only kernel-accepted certificates and every consult re-runs `candidate.verify()` through the admission kernel — a hit is a re-checked reuse, not a trusted verdict (hits rejected by the kernel fall through to fresh derivation). `component-description`'s `verify` re-derives subject/schema/entries/custody/assumptions from bytes with the expected subject caller-supplied (substitution tests prove independent replay). `build-evaluation/src/provider_settlement/independent_components.rs::verify_independent_component_descriptions` re-verifies every attached description under the build's own admission profile, never the producer's accept. PCC admission replays normalized rows against closed target specs per `machine_state_evidence.md`. Residual: an exhaustive whole-tree audit of every verifier callsite is open, but the four decision-adjacent reuse mechanisms are each independently checked.
- **PRODUCER-HISTORY-CUSTODY** — mined candidate; verify scope then implement.
- **PROVIDER-ATTACHMENT-MACHINE-PLAN** — mined candidate; scope verified, no bounded slice this wave (z175, `500878c473f4c`). The namesake surface — `typed-trees-to-checked-trees/src/execution/unit/providers.rs` — already produces the exact `CheckedProviderAttachmentRequirementPlan` roster (`checked_provider_attachment_requirements` + the composed-leaf variant), pinned across `tests/flow/terminal_unit` and rejoined to authored call sites by c2l `unit/attached_unit/provider_attachments/source.rs`. The residual the name carries is BOUNDARY-ISSUANCE's open frontier — the provider-planning/native-settlement join to the installed occurrence. Plan-side work left for this item is join design across crates, not a file-local patch. providers.rs itself is unclaimed this wave.
  covered — roster already produced in `execution/unit/providers.rs`; residue is cross-crate join design, not a bounded slice
- **PROVIDER-ATTACHMENT-MACHINE-PLAN.** — mined candidate; verify scope then implement.
  covered — roster already produced in `execution/unit/providers.rs`; residue is cross-crate join design, not a bounded slice

- **RECAST-SOURCE-POSITIONS.** Compose admitted representation recasts in
  guard operands, call arguments and nested expressions without requiring a
  reference-typed `let`. `validation/src/value_custody/recasts.rs` still
  admits selected initializer roots and rejects remaining recasts in its
  positional sweep; diagnostic source spans already exist. The
  [recast contract](wiki/spec/layouts/recasts.md) requires representation
  compatibility, not this source-position restriction.
  Carry checked layout/validity, backing identity, lifetime and access through
  ordinary expression sequencing and temporary loans rather than bypassing
  the recast check. Acceptance: valid inline equivalents of supported shared
  and mutable recasts check and execute; incompatible geometry/validity,
  access escalation and conflicting backing use reject at the offending
  location. Preserve precise symbolic/boundary-witness footprint refusals.
  Migrate stale imports in `recast_position_fenced` and coordinate its carrier
  spelling with **BINDING-CARRIER-NAME** before using it as fresh evidence.

- **REVIEW-RESEAL-ELIMINATION.** Remove repeated identity serialization of the
  same unchanged in-memory UEFI semantic-wrapper object across construction,
  encoding and staging validation in
  `native-realization/src/optimized_semantic_wrapper_object`.
  Construction already uses `validate_object_preserving_seal`; encoding and
  trailing staging validation still invoke full object validation. Trace those
  repeated computations before changing them. Preserve independent decoding,
  shape/target/template checks and honest-reseal mutation controls. Acceptance:
  demonstrate fewer duplicate identity computations on the actual staging route,
  identical valid artifacts and unchanged rejection of corrupted/substituted
  encoded objects. Do not remove decode-boundary checking or introduce
  package-acceptance receipts.
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
- **SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT.** (split-of:SAMPLE-CORPUS)
  Complete entry establishment for ordinary slice-view locals and callees.
  On `linux_x86_64`, `linux_arm64` and
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

- **SEALED-COMPOSITION-EXTRACTION** — mined candidate; verify scope then implement.
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




- **NATIVE-WRAPPER-ENCODING-AARCH64.** (new-scope) Resolve the existing
  `aarch64-semantic-wrapper-arrival-shape` decision in
  [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md) before adding a peer encoding.
  `select_optimized_program_storage_semantic_wrapper_encoding` currently
  selects the x86-64 template; its recipe assumes the UEFI indirect Extent
  arrival and caller-owned copies. AArch64 declarations instead use register
  fragments, and current AArch64 profiles expose hosted entry, not
  ProgramStorage entry. Do not assume they need the UEFI wrapper.
  Acceptance after the ruling: either implement the required arrival and
  continuation with architecture-correct branch relocation and independent
  substitution checks, or pin the intended refusal and remove this task.
  AArch64 hosted execution remains with the native matrix owners.

- **TARGET-INFERENCE-AND-PLATFORM-CERTIFICATION.** Enforce canonical target
  spellings at compiler and CLI request boundaries under
  [exact target requests](wiki/spec/build/configuration.md#exact-target-requests).
  `TargetProfile::from_omega_target_name` and `ExplicitTargetSet` still accept
  `linux_x64`, `windows_x64` and `uefi_x64`; the specification requires
  canonical names rather than these aliases. Preserve Host convenience,
  exact-set deduplication/order, independent child outcomes and unsupported
  profile diagnostics. Acceptance: canonical single/multiple requests work;
  aliases, empty sets, wildcards and unknown names reject. Target-neutral
  checking remains distinct from native execution on a matching host.
- **TRANSPARENT-TRAIT-REFINEMENTS.** Complete refinement application and exact
  requirement selection under
  [transparent refinements](wiki/spec/language/conformances.md#transparent-refinements).
  Reach-subset checking, independent clause-local `_` rows, requirement
  forwarding and concrete evidence-binder fit already exist; do not rebuild
  them from the obsolete claim that refinements have no checked consumers.

  Instantiate `refines.arguments`, including reordered/partially applied
  heads, before comparing the selected conformance. Typed trait lowering retains
  them, but `monomorphization/selection/refinement_fit.rs::resolve_bound_carrier`
  keeps only the base symbol and `candidate_bounds.rs` compares the unexpanded
  bound arguments. Targeted signature-free paths must resolve one exact
  requirement, rejecting ambiguity instead of refining every same-named overload.
  Also reconcile `covering_clause` and its targeted-replaces-wildcard test with
  the specified rule that `machine *` applies to every base requirement; a
  targeted clause must not silently discard those constraints.

  Preserve structural-bound (not nominal-target) semantics, inherited axes,
  independent bounded rows, complete-contract fit and the order-independent
  meet of combined refinements before normalization/fingerprinting. Acceptance:
  fitting/nonfitting parameterized applications, exact/ambiguous targets,
  wildcard-plus-targeted constraints and consumed binder fixtures. A declaration
  fixture that never instantiates its binder does not establish usable fit.
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

- **TV-GENERAL-CALLS-REPLAY** — mined candidate; verify scope then implement.
## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
- Windows AArch64 has no `NativeTarget` constructor, so that ABI combination
  stays unwitnessable until the target vocabulary grows one.
