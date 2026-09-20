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

  The tracked app `4b1f7a6` retains the 12-check geometry baseline and std
  `daa47e2d5048b67840393d2bb05ede58ea38e7c7`. The local app candidate
  `ea6c80735e93ae69d3cb8f7d961fdf2b8413e50b` adds region alignment and expansion.
  With Omega `47db1d16478dc615b5a13155122eec8328f3ef53`, macOS ARM64,
  a release compiler, Python 3.13 and `RUST_MIN_STACK=67108864`,
  `python samples/apps/squalr/tools/verify.py native --timeout 600 --omega <executable>`
  passes all 24 checks: native exit 0, `Squalr geometry: PASS` (167.2 s).
  Omit `--target` for execution; explicit targets only emit an executable.
  Compiler repairs distinguish callee-local storage from structural call inputs
  in Terminal and abstract replay, align physical traversal capacity, and remove
  duplicate ARM64 operand-footprint tables. Source ownership and independent
  checking remain intact.

  Application publication is pending: GitHub denied the configured `NH21B` account write
  access to Squalr-Omega (HTTP 403; repository permissions report `push: false`).
  The app has a local checkpoint commit; it must be published before changing
  this repository's pin. The compiler repairs can be integrated independently.
  The failed app enqueue left no ticket or reservation. Next app integration
  acceptance is publication of the verified app and the parent pin, not
  another helper test.
  Checkout relocation still requires ordinary local-source update/review.
  Windows execution and the supplied-byte scanner remain open.

  The pinned Rust reference is
  `568aa7589b68b2fd4621cc66c6dde23fa14c7f50`; the ordinary 17-package/37-edge
  graph is unchanged. Runtime growable storage remains an implementation
  dependency: `core/vec.omg` declares no construction or storage mechanics.
  **BUMP-ALLOCATOR-CANARY** and **PLAN-LAID-VIEWS** own element establishment
  and content-preserving growth; a fixed-capacity scanner does not satisfy
  this customer.

  Keep one integration owner and work from the actual application command:

  1. Preserve the working geometry command as the compiler integration control,
     and run it on Windows after ordinary target-specific package review.
     Publish application changes in its own repository before updating the
     gitlink. Assign only newly witnessed compiler failures to their existing
     semantic/realization owners.
  2. Drive the submodule's supplied-byte scan and repeated
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
  The sample uses intrinsic `Service<R>` fields and public requirement traits,
  preserving its render loop, arrays and reach declaration. The current first
  dependency is provider wiring, not bundle assembly or an invented attachment.
  At `ea9d3dbfea`, the macOS ARM64 release CLI command
  `omega --target macos_arm64 --build-dir build/window-app-intent samples/gui/window_app/main.omg`
  with `RUST_MIN_STACK=67108864` failed during fresh package checking. The
  checked omission is the entry state's attached data shape: `Console` has
  Fused selection authority, but `Main::clock`, `Main::input`, and `Main::gui`
  have none. Entry diagnostics now name each missing boundary selection before
  looking for its unavailable Terminal attachment.

  Resume with exact declared conformances and selected providers for `Clock`,
  `Input`, and `Gui` under the [provider-selection contract](wiki/spec/build/provider_selection.md).
  The historical `source/library/std/macos_gui.omg` wrapper is not an already
  selected conformer to these sample-owned requirements; matching method names
  cannot establish nominal satisfaction. Preserve actual signatures, storage,
  rendering and effects. Before treating signature selection as executable
  supply, retain one occurrence-owned concrete provider receiver across its
  calls and establish its nested services. Current Fused entry receipts
  establish erased service authority, not that receiver storage;
  `ProviderAttachment` is a specialization witness and cannot serve as an
  ordinary structural argument.
  Verified on `ddc66b61b5` (linux_x86_64 host, self-contained probe packages,
  `--target macos_arm64`): the `Service<R>` + `select_provider` + `roots.bind`
  chain compiles, passes package review, and publishes native output for a
  one-provider-field entry whose selected provider uses unit-result checked
  adapters. Recipe pins: provider is `pub data`; adapters are
  `machine P::m(&mut self, ..) satisfies T::m { }` (trait methods must declare
  `&mut self`, and adapters may not widen `reaches` beyond the requirement —
  a requirement that covers delegation carries `reaches X invokes X`);
  `builder.select_provider<Trait, Provider>()`; every `Service` field anywhere
  needs its trait's selected plan to hold at least one CheckedAdapter row once
  any adapters exist ("routed service field .. no exact Fused
  selected-provider-plan join"), so leaf-only providers cannot join and foreign
  leaves on providers currently fail realization ("no supplied execution and
  stack custody", and `via` must name a satisfies requirement plus a
  `Binding`-returning producer). Calls through a nested carrier's `Service`
  field do not resolve; the field must sit on the receiver itself.
  `typed-trees-to-checked-trees/src/execution/unit/providers.rs` and independent
  lowering/replay currently limit an attachment to one provider field, while
  this entry has four (verified: two `Service` fields with selected providers
  rejoin `0 Terminal attachment identities; expected one`). Lowered
  `attached_unit/providers.rs` also rejects scalar-result provider candidates,
  and every window/input/clock op returns a scalar. These are implementation
  dependencies under **ENTRY-CONTENT-ROOTS**, **TR3-TR8** and
  **STATE-LOCAL-VALUE-FRONTIER**, not language-design blockers or permission to
  inject global provider state.
  After those dependencies and provider settlement, follow remaining checked
  call, array and cyclic execution failures through their existing owners,
  then finish ordinary package review without automatic admissions. The
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

  - Done — `compile_native_and_publish` no longer copies package permission rows
    into an explicit receiving policy: the harness's artificial requirement is
    removed while ordinary compilation defaults the policy to absent, retaining
    test-owned package acceptance and separate explicit receiver-admission
    controls under
    [the artifact/admission split](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission).
    A harness pass does not complete the user's project review.
  - Done — the runtime oracle and `cli_mvp_preserves_both_lines_with_eof_and_enter`
    run the executable the published report receipts through
    `checked_native_executable_path()`, including bundled paths, instead of a
    guessed `build_dir/omega-program`; no application's output name is forced to
    fit the test.
  - Migrate bare service fields and `Service<Console> in Bound` to intrinsic
    `Service<Console>` establishment with **ENTRY-CONTENT-ROOTS**. Never weaken
    missing-provider or exact occurrence checks to preserve obsolete examples.

  Resume from focused runs, not the accumulated historical failure counts.
  At `d575c7e8e0` on macOS ARM64, `print_squares` reached unsupported
  wrapping-u32 multiplication. Earlier
  `print_squares` probes stopped at the nonzero-divisor proof
  `1 <= self.place`; these are different checkpoints, not simultaneous claims
  about today's first failure. The current scalar legalization has no wrapping-
  integer multiply route, while scalar call lowering still reads only authored
  `crash.published()` in `scalar_graph/scalar_graph_lowering/{call_lowering,unit_operations}.rs`.
  **CRASH-CONTRACT** owns consistent inferred-ceiling publication; retain it as a
  dependency rather than another sample-local workaround.

  | Customer | Remaining integration and owner |
  | --- | --- |
  | [`cli_mvp`](samples/cli/basics/cli_mvp/README.md) | Remaining: ordinary review and native execution on Windows x86-64 and both Linux hosts. On macOS ARM64, ordinary update/review/resume at `ba57b10d5e` published the local lock; the `c459b1d25c` release CLI compiled with `--target macos_arm64 --build-dir build/cli-mvp-route samples/cli/basics/cli_mvp/main.omg` and `RUST_MIN_STACK=67108864`, without receiving-policy input. Its emitted executable produced both exact lines, empty stderr, and exit 0 with EOF and Enter; prompts appeared before input and Enter completed the waiting process. Repeat review per checkout and target. **ENTRY-CONTENT-ROOTS** owns remaining service/entry custody; **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns production/admission coupling. |
  | [`print_squares`](samples/cli/basics/print_squares/README.md) | Wrapping arithmetic legalization in `target-operations-to-selected-instructions`, cyclic field/divisor facts under **NOMINAL-FIELD-FLOW**, and complete cyclic plans under **GENERAL-CYCLIC-EXECUTION**. Preserve nine computed rows ending in `081`, byte storage, and exit 0. |
  | `recursive_sum`, `framed_payload` | Native exit 70 and 60 respectively. **STATE-LOCAL-VALUE-FRONTIER** owns indexed primitive storage and replacements; **GENERAL-CYCLIC-EXECUTION** owns typed slice views, recursive/cyclic transfer and ranking. Preserve saved reads, untouched siblings and shared payload loans. No synthetic field IDs for scalar array elements or invented ranking for unranked cycles. |
  | `dutch_flag` | At `6c653e7593`, ordinary macOS ARM64 release CLI review completed, then `omega --target macos_arm64 --build-dir build/dutch-flag-route samples/cli/algorithms/dutch_flag/main.omg` with `RUST_MIN_STACK=67108864` rejected `Main::main`: `structural field store: scalar field type`, state 0. **STATE-LOCAL-VALUE-FRONTIER** must compose indexed copyable enum construction, reads, swaps and case dispatch in `typed-trees-to-checked-trees/src/execution/unit/structural_scalar_store` and composed control; coordinate with **WRITE-ONLY-BORROW** on those producer paths. Preserve the Color array, runtime guards, Console interaction and exit 70; migrate the bare field to intrinsic `Service<Console>`. Native execution remains open; do not substitute integer tags for nominal enum storage. |
  | [`euclid_gcd`](samples/cli/arithmetic/euclid_gcd/README.md) | Remaining: ordinary review and native execution on Windows x86-64 and both Linux hosts. Preserve the live-divisor loop, exact three output lines, empty stderr, EOF/Enter behavior and exit 12. The documented macOS ARM64 update/review/resume and native CLI route passes those checks; repeat review per checkout and target. Keep `euclid_gcd_retains_service_call_entry_plan` and `integer_comparison_publication` as the service-frame and selected-versus-builtin custody controls. |
  | [`generic_counters`](samples/cli/basics/generic_counters/README.md) | Remaining: ordinary review and CLI execution on Windows x86-64 and both Linux hosts. At `b004b477e5`, macOS ARM64 `omega update --project samples/cli/basics/generic_counters --target macos_arm64`, exact review decisions, and `update --resume` published the local lock; `omega run samples/cli/basics/generic_counters/main.omg` then exited 16 with empty stdout and no receiving-policy input. Repeat review in each checkout; do not reuse the temporary checkout's local-source lock or reimplement counter calls. |
  | [`number_guess`](samples/cli/basics/number_guess/README.md) | Remaining: ordinary review and CLI execution on Windows x86-64 and both Linux hosts, preserving seven search steps, exit 70, exact documented output, and intrinsic `Service<Console>`. At `ba57b10d5e`, macOS ARM64 `omega update --project samples/cli/basics/number_guess --target macos_arm64`, exact review decisions, and `update --resume` published the local lock; `omega run samples/cli/basics/number_guess/main.omg` then produced all three expected lines and exit 70 without receiving-policy input. Review again in each checkout; do not reuse its local-source lock or reimplement supported arithmetic. |
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

  `calendar`'s next native dependency is **ARITHMETIC-POLICY-REALIZATION**:
  its numeric helpers reach `checked trapping conversion requires runtime
  policy realization` in `checked-trees-to-lowered-psi/src/expression_preparation/`.
  The unchanged sample's macOS ARM64 native harness (`691e2346b8` plus the
  byte-index repair, `RUST_MIN_STACK=67108864 OMEGA_SAMPLE_RUNTIME_FILTER=calendar
  cargo nextest run -p compiler --test samples_compile --no-fail-fast --no-tests fail
  -E 'test(=samples_with_documented_exit_run_correctly)'`) must advance past
  that refusal. `compiler --test byte_index_carriers` pins original integer
  arithmetic, exact coordinate conversion, live bounds and caller-visible byte
  writes; these do not substitute for calendar execution. Preserve its qualified
  capacity-21 buffers and 20-byte live header.

  Ordinary CLI review remains separate: the last
  `RUST_MIN_STACK=67108864 omega run --target macos_arm64
  samples/cli/simulation/calendar/main.omg` probe (stdin EOF, `b336531455`
  plus the unchanged-delivery repair) exited 200 with six pending package
  policy rows, including a filesystem capability. Complete that review without
  automatic admissions. Native grid/output acceptance remains unverified:
  check all five week rows and the live header, since `contains: 30` also
  matches the banner without proving the grid rendered.

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

  The unchanged `text/runtime_stdin_command_branch_exit` now reaches the owned
  `Command` result-to-field assignment in `parsed_input`; its first remaining
  plan omission is `statement sequence: assignment: call source result type`.
  `execution/unit/control/statement_sequence.rs` in `typed-trees-to-checked-trees`
  still accepts only primitive call results there. **STATE-LOCAL-VALUE-FRONTIER**
  owns structural result storage; preserve the reader's algorithm while adding
  that operation and its independent lowering checks. Resume on macOS ARM64 with
  `mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(runtime_stdin_command_branch_exit_canary_runs)'`.
  The byte-read regression `hosted_read_unused_payload_retains_static_array_extent`
  covers unused scalar case bindings and raw-array lengths separately; it does
  not establish command-reader completion.

  For bounded input, the full-result fixture
  `host/runtime_console_bounded_line_exit` still omits its entry plan at the
  first `read_line(&mut self.line[0..0])` call (macOS ARM64 native probe at
  `d5e62f683d`: `statement sequence: call: call operation`, state 0, statement 4).
  `execution/unit/calls/structural_arguments.rs` only admits byte subslices for
  checked Unit-returning callees, and `calls/byte_subslice.rs` additionally
  requires a shared slice parameter root without field projections. Mutable
  field subslices need ordinary view/borrow transport and independent lowering
  checks under **STATE-LOCAL-VALUE-FRONTIER**. Preserve zero-capacity non-consumption and
  untouched-tail/count checks; the fixed-array discard-result native test
  `fixed_array_line_reader_executes_only_until_its_first_completion` does not
  close that remaining result/subslice composition.

  The two-read echo `text/runtime_stdin_line_buffering_exit` first needs
  `block` on both `echo_line` calls (native test at `df8954126a`, macOS ARM64).
  Its intrinsic-service migration also needs borrowed `Service<Console>`
  parameters through `write_prefix` and its state edges. The declaration
  validator, Unit signatures/forwarding, lowered state admission and selected
  `service_custody/parameters.rs` still fence that route; several also require
  the retired authored `Bound` qualification. Extend ordinary borrow/state
  transport with exact binding receipts, not the single-hop owned-Service
  recognizer, and preserve the helper and two-read algorithm.

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

  Acceptance: the complete corpus reaches its declared stages,
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

  - Extend non-polynomial actual-argument substitutions beyond the existing
    quotient/remainder endpoint transport. Preserve both operands' exact
    identity and independently recheck every constituent operation at each
    arrival; cancellation cannot hide a zero divisor or intermediate overflow.
    Reuse `rank_ranges/field_endpoint_arithmetic.rs` and
    `rank_ranges/call_components.rs`, including their changed-input and
    mixed ranged/unranged controls. Divisor bounds spanning both signs still
    require stronger evidence than an unoriented disequality. The independent
    interval-only fallback still requires one state; do not remove its guard
    without proving every exact arrival. Reuse the relational field-coordinate
    route for readable stored references, covered by
    `termination/stored_reference_endpoint_arrivals` and its reseating twin.
    Formation, arrival equality, and complete write-frame checks remain required;
    polynomial cancellation, equal endpoint intervals, or positional guesses
    cannot supply them. Mixed-component inputs beyond direct integers/arithmetic
    trees still need their actual input correspondence and conservation evidence.
    The stored-exclusive countdown in `compiler/tests/rank_endpoint_borrows.rs`
    checks and runs in the checked interpreter (macOS ARM64, `a647d6afef` plus
    the endpoint repair; `RUST_MIN_STACK=67108864 cargo nextest run -p compiler
    --test rank_endpoint_borrows --no-fail-fast --no-tests fail`). Native
    acceptance remains open: publishing its `walk` through
    `TerminalProductionRequest` reaches "machine has no source-independent
    checked scalar control plan". **STATE-LOCAL-VALUE-FRONTIER** owns that
    next producer dependency; do not replace the stored loan with copied data.
  - Replace residual rank-role discovery limits with explicit arrival
    correspondence where the program supplies enough evidence. Unique nested
    carriers, borrowed roots, moved scalar/slice/record copies, custom
    call-component views, and produced scalar/slice rank facts already have
    implementations; do not rebuild those as new feature slices. Ambiguous
    copies must remain rejected unless the checked correspondence identifies
    the ranked value. A shared nominal type or a convenient decreasing copy
    is not proof of that identity.
    The unchanged named-state customer in
    `compiler/tests/rank_remainder_endpoints.rs` checks and runs in the
    checked interpreter, but native publication still needs exact rank-role
    correspondence and a loop header distinct from the entry state.
    On `e850106f7a1` plus the remainder endpoint repair, publishing `walk`
    through `TerminalProductionRequest` rejects its terminal signature.
    The downstream checked scalar `ranking.rs` and lowerer
    `scalar_graph/scalar_graph_lowering/cycles.rs` also require a one-state
    graph. Resume with the same named-state source and publish/run it natively;
    the separate single-state native regression is not this acceptance.
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

- **BUILD-PRODUCT-REFERENCES.** Finish
  [non-executing product selection](wiki/spec/build/scoped_execution.md#selecting-product-declarations-without-executing-them).
  Entry/provider/schema queries, opaque descriptions, delegated entry binding,
  and exact-symbol final admission exist. Remaining:

  - Finish exact expected provider requirement/application checking and
    visibility across description use and final admission.
    Qualified queries select public declarations in the authorized package's
    product checked instance, never its build copy; bare queries retain the
    same-package product frontier. Source names and evaluator table indices
    are not durable selection authority.
  - Extend the existing provider/description owners, retaining the
    [separate checked contexts](wiki/spec/build/scoped_execution.md#two-checked-contexts) and using
    BUILD-ADMISSION-CHECKPOINT for source custody.
    Do not add a general compiler-query interface or allow target execution.

  Acceptance: a multi-file foreign helper binds an owner's restricted private
  entry description, including one returned by an ordinary checked helper;
  a qualified query or static root operand selects a public declaration in an
  authorized product dependency, retaining that exact checked-instance symbol
  through Terminal production and native publication. Wrong scope/target/slot,
  stale activation, lookalike operations, sibling-private enumeration,
  forged descriptions, description-to-callable
  conversion, and same-build generated/layout cycles reject. Preserve
  `compiler/tests/build_target_activation/{foreign_helper_product_queries,qualified_root_bindings,product_query_paths,product_entry_signatures}.rs`.
  Preserve the ordinary-return-to-local, inline, nested-effect, and generic
  native tests alongside returned-forgery and expression-checking negatives in
  the foreign-helper module, including computed receivers, named lifetimes,
  retained parent loans, and conflicting later-operand accesses.
  Preserve `qualified_provider_selection.rs` as the module-identity regression:
  its native Console-forwarding case must exit 11/37 for the selected module,
  and swapped declarations, same-slot ambiguity and synchronous cycles reject.
  Its scalar/multi-state `Runner` fixture is checked-only. Native follow-through
  still needs the scalar provider and state-call closure owned by
  **TR3-TR8** / **STATE-LOCAL-VALUE-FRONTIER**, plus exact receiver attachment
  under **ENTRY-CONTENT-ROOTS**. Bind each module's `Runner::run` as ProgramEntry
  and require its computed-result branch to execute; a passing Unit-forwarding
  case does not close that acceptance.

- **BUILD-SNAPSHOT-OUTPUTS.** Finish the
  [captured-input and committed-output contract](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem)
  across ordinary compilation, not only package review. Snapshot reads,
  required-output settlement, private-staging cleanup, and audit reporting
  already have implementations and integration tests. Remaining:

  - Complete host-backed capture/staging assurance and Windows execution
    coverage. Mutable-root copies in
    `packages/sources/acquisition/src/tree/filesystem.rs` now check the retained
    file's length/change indicators and final directory-relative identity;
    canonical Source copy/hash checks remain in
    `package-compilation/src/source_snapshot/file_read.rs`. Preserve their
    deterministic edit/replacement regressions and the retained-parent control.
    Per-file observations and the resolver's later live-tree comparison do not
    establish whole-tree atomicity. The remaining capture frontier includes
    directory membership and link changes across traversal, and edits that
    restore compared observations. Validate coherent capture and race-safe
    confinement under the exact isolation premise, logical path distinctions,
    inert links, and failure cleanup. Preserve the CLI narrowed-inventory/native
    customer in `omega/tests/build_input_inventory.rs` and record unavailable
    Windows runtime coverage rather than treating source inspection as a pass.

  Owners: `package-compilation/src/source_snapshot.rs`, the existing
  `build-output` and `build-evaluation` custody owners, assembled checking,
  and compiler/compilation-report publication. Reuse
  BUILD-ADMISSION-CHECKPOINT for admitted-source/generated-source replay and
  the existing `PackageCheckedContext` for occurrence identity. No second executor,
  live-host grant extension, or persistent writable cache.

  Acceptance: an acquired ordinary generator reads a narrowed template and
  publishes a required file via both artifact-only and companion builds.
  Run `compiler/tests/build_snapshot_outputs.rs` and
  `omega/tests/package_commands/snapshot_outputs.rs`, and
  `omega/tests/completed_build_outputs.rs`. Preserve named-slot isolation and
  generated-source-to-publication coverage in `compiler/tests/build_named_inputs.rs`
  and `package-manager`'s `suite` filter `build_named_inputs::`.
  Cover negative lookups and metadata,
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
  static/bounded-dynamic closure checks, provider-body joins, the checking/
  no-op assertion and logger fixtures, and exclusion selections recorded as
  they execute against the live root Build authority exist. Remaining:

  - Complete absence evidence for the full admitted entry/call closure,
    including generated entries and all admitted dynamic targets. Executable
    nominal cleanup now joins the walk: `Return`/`ReturnUnitNominalAffine`
    `cleanup_machine` edges are followed like static calls, an absent target
    is an evidence gap, and callback thunk bodies verify at their own
    production site. Parameter dispatch still reports missing evidence;
    preserve that rejection until its exact target set or conservative
    contract suffices. Sound guard evidence may establish unreachable
    behavior, but optional optimization and broad public ceilings are not
    absence proofs.
  - Close direct Unit crash planning through CRASH-CONTRACT and the owning
    Unit control-flow lanes. `behavior_exclusions` now publishes the ordinary
    no-op package and executes it on the host; its separate `direct-unit.omg`
    control still stops at `DirectCheckingAssert::check`: state-graph jump
    successor, transition form, state 2, statement 0, with no checked
    transitive Unit plan. This also rejects an unselected direct-crash
    candidate in the library. Replace the fail-closed frontier test with the
    actual Trap-exclusion verdict once the body lowers, and realize the
    admitted direct-crash body natively without the scalar-helper workaround.

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
  under the [exclusion contract](wiki/spec/build/behavior_exclusions.md).
  `builder.exclude_physical_authority(PhysicalAuthorityClass::X)` is an executed,
  exact-identity Build selection. Both direct and retained product routes carry
  its canonical union to the existing mechanism-closure adjudication; do not
  rebuild the authoring surface or classifier. Owners:
  `build-evaluation/src/admission/{declarations,behavior_exclusions}.rs` and
  `native-realization/src/{native_product/realization,native_realization/behavior_exclusions,retained_native_product}.rs`.

  Resume evidence (2026-09-20, macOS ARM64, base a10fbe20d0 plus the authored
  physical-exclusion change): `RUST_MIN_STACK=67108864 cargo nextest run
  -p compiler --test build_behavior_exclusions --no-fail-fast` exercises
  source-built publication and native execution. A standard-library Console
  program prints `A` under a ProcessInput exclusion and rejects the same
  composition under ProcessOutput, without a receiver permission policy.
  The quiet product also cross-emits for Windows x64; Windows runtime was not
  exercised. Helpers, skipped calls, exact cases, and canonical unions are
  covered at the source boundary.

  Remaining work:
  - Portable protocol coverage and independent consumer custody for the complete
    physical exclusion envelope; source authoring does not establish it.
  - Envelope custody through rebinding/replacement via COMPONENT-SUBSTRATE and
    WIRE-RUNTIME-AND-INSTALLATION, and the image-emission and foreign-boundary
    legs.
  - Complete provider/replacement controls, including a silent Console provider,
    receiver policy and optimization variations. Exercise Windows runtime and
    installation controls on both Windows/macOS; the macOS ordinary Console
    publication above is not installation/replacement acceptance.

  Next native control: reuse the `sink-app` / `logger-kit` package composition
  from `compiler/tests/behavior_exclusions.rs`, target `macos_arm64`, replace
  `exclude_service<Sink>()` with
  `exclude_physical_authority(PhysicalAuthorityClass::ProcessOutput)`, and
  request `NativeArtifact`. A temporary probe at `ee249ec910` on macOS ARM64
  reaches `Selection(Legalization(SourceCustodyMismatch))` before publication.
  The silent service invocation therefore still needs ordinary native custody
  in `target-operations-to-selected-instructions/src/legalization`, not another
  authority classifier. After that dependency closes, require native execution
  with empty output and independent retained-product replay; preserve the
  existing service-exclusion rejection for the same invocation.

  Reuse `terminal_authority_policy/` and the mechanism-closure review;
  classification is not receiving permission. Do not invent a second classifier,
  synthesize receiver approval, or claim that semantic exclusion replay
  establishes physical absence. `TWO-AXIS-TERMINAL-AUTHORITY-REVIEW` owns
  receiver admission; this task owns the independently requested build
  guarantee.

  Acceptance: source-built products distinguish no-Console from no physical
  output, including a silent Console provider. Unknown classifications and
  mismatched evidence cannot count as absence; failed final checks publish no
  successful product. Check with and without receiver permission policy and
  optional optimizations. Source-free replay preserves the same verdict, and
  replacing a benign provider with excluded behavior rejects. Exercise actual
  provider and installation controls on each available Windows/macOS host and
  report the unavailable legs explicitly.

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
    consumer (reference leg landed): `verify_plan` and `compose_plan` now take
    `components: &[AdmittedComponent]`, and every roster entry naming
    `InstanceRole::Component` must bind an admission whose subject joins the
    owner roster — an unadmitted record rejects as `MissingVerifiedComponent`,
    a verified subject relabeled external rejects as `ExternalSubjectVerified`,
    and any divergent field (verification profile, `VerifiedComplete` closure,
    demanded-assumption roster, endpoint inventory) rejects as
    `Substituted { field }`. Entries, authority and custody bind transitively
    through the completeness closure; endpoints, profile and assumptions bind
    directly from `VerifiedComponent` (`verified_components.rs`,
    `cargo nextest run -p topology-plan`; linux-x86_64). The demand/supply
    contract join is still unbound: `ExportSurface.identity` is an opaque
    string, so a demanded import's contract cannot yet be checked against the
    offered export's requirement identity — that is a `COMPONENT-SUBSTRATE`
    description gap, not a parallel census.
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
  activation/cleanup/replacement sequencing and real pipe creation.

  Unix leg landed (`00eb18ca51`+`1ed6bd1c9c`+`9d917b228d`, linux-x86_64,
  `cargo nextest run -p topology-plan`): `topology_installation` now binds
  physical holders — each member's kernel-attested pipe token (inode and
  direction) — instead of caller-supplied instance numbers; each bound
  contract must register an operation/payload schema
  (`topology_installation/operation_schema.rs`), and a schema violation or
  undecodable frame closes its binding. `LocalProcessSupervisor`
  (`topology_installation/local_supervisor.rs`) admits member images by
  sha256 content and spawns them through `bounded-process`'s
  retained-descriptor handoff — general inheritance stays disabled and each
  member's attested descriptor table is exactly its assignment before entry
  opens. `a_three_process_installation_mediated_over_real_private_channels`
  in `tests/installation.rs` runs the payment customer as three checked
  processes: one granted request/response pair per binding, an ungranted
  endpoint and a substituted mapping refuse, peer failure EOFs the channel,
  and quiescence reaps the roster including the killed member. The sibling
  spawned-supervisor leg (`tests/process_confinement.rs`) rides the same
  token gate and schema registration.

  Remaining legs: Windows and macOS providers are unavailable on this host
  and are unrun — not passing. The unix provider assumes `std::io::pipe`
  anonymous channels (both ends share one inode; direction is probed by a
  zero-length write returning EBADF on read ends), `fcntl(F_SETFD)`
  descriptor retention across `pre_exec`/`exec`, and a re-executed test
  image as the member entry. macOS inherits the same `fcntl`/`fstat` token
  scheme but needs a signed or adhoc member image; Windows needs
  named/anonymous pipe handles behind a handle-inheritance boundary (the
  `StdPipeEnd` token arm exists; `pre_exec` does not — ends must be passed
  as explicit inheritable handles). Executable admission, schema checks,
  and the refusal/close law are platform-neutral and landed.

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
    feature, bare-trait alias or fabricated establishment row is needed. Resume
    evidence (w9, `swarm-w9-entry-content-roots`): the cut is implemented and
    verified on the wave branch — `Bound` is deleted from `core/service.omg`, the
    service classifier requires the exact closed `Service<R>` identity and rejects
    any authored `in <domain>` qualification during source checking, the checked
    carrier's `bound_domain` leg and every downstream plan/custody/test consumer
    are retired, and all `in Bound` spellings are migrated out of library sources,
    product sources, fixtures, samples and canaries. Frontier before landing: the
    two fused-parameter rejoin gates in
    `checked-trees-to-lowered-psi/src/unit/attached_unit/parameters.rs` are part of
    this cut but the path is claimed by TR3-TR8, so publication waits for that
    claim (or a coordinator merge). Bare-field rejection and Squalr migration in
    the next bullet are untouched.
  - Migrate library, samples, canaries and Squalr from bare boundary-trait fields
    and `Service<R> in Bound` to the intrinsic carrier; reject bare fields during
    source checking rather than after native bridge planning. Preserve negative
    controls. Epsilon's separately specified sealed Console is not this surface.
    Resume evidence (w9, `swarm-w9-entry-content-roots-2`): the rejection is
    implemented in `typed-trees-to-checked-trees`' `validate_typed_program`
    (`checking/program_validation.rs`) — data fields, variant payloads, machine
    parameters and returns, and trait signature parameters/returns that name a
    bare boundary trait in value position reject as non-carriers at source
    checking; the one admitted spelling left is a `satisfies` adapter's leading
    self-forwarding receiver slot, which conformance slicing removes before
    arity and which names the satisfied boundary trait itself. The corpus
    (library, canaries, samples) is migrated to `Service<R>`; negative controls
    in `tests/omega/fail` still reject, and fixtures whose signatures required
    public slot contracts promote those declarations rather than weakening the
    gate. Raw-pipeline unit fixtures (no package scope, so `Service` cannot
    resolve) were migrated onto `&'s mut <boundary trait>` receivers instead;
    where a test still exercises true carrier semantics (provider attachment,
    `established by` results, `ensures`-bound out-params), the toolchain
    `service.omg` is injected into the fixture's `SourceMap` so `Service<R>`
    resolves against the real core decl. Residual for bullet 3: raw fixtures
    whose checks need service-activation semantics — `ensures` witnesses on
    boundary out-params, `established by` establishment routes, and direct
    dynamic plans through an attached service field — still reject; the
    intrinsic carrier contract does not yet expose those spellings
    (`Service<R>` admits no authored `in <domain>` qualification and
    `established by` expects the old return shape), so those tests stay red
    until the receiver lifecycle leg lands. Squalr remains the
    coordinator-scoped surface in the bullet above and is untouched here.
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

  Frontier (w9 z57 leg, origin/main 72125c7156, ~15:15Z): claim exited 2 —
  a live same-item claim (linw2-entry-content-roots, expires 18:39Z) and
  bullet-3's fence is triple-covered: `program-entry-plan` by
  UEFI-OS-HANDOFF (20:00Z) and BACKEND-RUNTIME-STARTUP-ENTRY-MECHANICS
  (22:09Z); `external-roots/src/program_local` — the
  `ProgramLocalRootInstallationLedger` the bullet names — by
  EPOCH-RESOURCE-SNAPSHOTS (22:31Z) and
  FOREIGN-RETAINED-ARGUMENT-BACKING (21:42Z). Bullet-1's named blocker
  `checked-trees-to-lowered-psi/src/unit` is also still fenced
  (UEFI-OS-HANDOFF 20:00Z wholesale `src/unit`, WRITE-ONLY-BORROW
  `attached_unit*` 20:07Z) independently of the TR3-TR8 note.
  `terminal-production/.../receiver_eligibility.rs` and
  `image-emission/src/hosted_receiver.rs` themselves were UNCLAIMED — a
  retry whose leg touches only those two files can claim them alone.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Execute the source-authored two-surface UEFI
  bootstrap under [source-owned firmware adapters](wiki/spec/build/uefi_entry.md#authored-firmware-definitions-and-adapters).
  Keep physical firmware arrival distinct from the semantic program continuation;
  the compiler emits only the necessary entry shell and generic target primitives.

  - Replace `target/src/uefi_{system_table,boot_services,loaded_image}/`'s Rust
    field catalogs with target-package declarations, evaluated layout policies,
    constants and integrity checks. Migrate `backend/plans/program-entry-plan`
    and `external-roots/src/platform_bringup/uefi_bootstrap/` consumers, then
    delete the duplicate production catalogs. Loaded Image landed: the
    `EfiLoadedImage` schema and evaluated `EfiLoadedImageLayout::plan` policy
    in `targets/uefi_x86_64/tables.omg` own the 96-byte geometry;
    `target::uefi_loaded_image` replays each evaluated `LayoutPlanReport`
    against recorded schema/plan/native-layout commitments
    (`replayed_uefi_x64_loaded_image_native_layout`), the bounded HandleProtocol
    executor and occurrence validation consume that replay, and
    `canary_suite/entry_and_abi/uefi_loaded_image_layout.rs` binds the authored
    policy's live evaluation to the retained layout. System Table and Boot
    Services catalogs remain duplicate production definitions.
  - Replace `program_entry_physical/exact_uefi.rs`'s duplicated physical-policy
    recipe with source-derived plan/evidence replay. Preserve exact accepted
    package/contract identity and arrival assumptions; a source digest does not
    establish plan correctness. Landed at `db3dfb4302`: consumers now replay
    each retained plan through `replayed_uefi_x64_physical_calling_plan`
    against `UEFI_X64_PHYSICAL_CALLING_PLAN_COMMITMENT` instead of re-deriving
    the recipe; the recipe materialization remains only for contract fixtures
    below the build layer and self-checks against that commitment.
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

  Resume evidence (w9, `swarm-w9--uefi-os-handoff`): the bodyless whole-protocol
  promise is replaced by an authored route in `std/targets/uefi_x86_64/handoff.omg`
  — `UefiOsHandoffLegs` boundary-machine legs own the four scalar-returning Boot
  Services calls and land each status on the legs record; `UefiOsHandoffCycle::run`
  owns the acquire/grow/adopt + stale-key-retry state graph with the explicit
  `UEFI_OS_HANDOFF_ATTEMPT_BOUND` bound and `UEFI_OS_HANDOFF_EXHAUSTION_STATUS`
  exhaustion answer; `UefiOsHandoffTermination` keeps entry/stack transfer and
  firmware return compiler-owned. `build/uefi_os_handoff_invocation` binds
  `Loader::run` and checks end-to-end
  (`checked_uefi_os_handoff_invocation_retains_edge_binding`). Two refusing
  stages are routed: provider derivation now admits a non-hosted target
  package's bodyless `boundary machine` satisfies leaf as a `CompilerIntrinsic`
  row on exact selected target-machine origin custody (hosted targets keep the
  name-keyed catalog gate;
  `selected_target_compiler_leaf_requires_nonhosted_origin_custody` pins it), so
  both `UefiOsHandoffTermination` leaves derive plans and the provider record's
  fused `Service` fields erase; and fused-Service custody rejoin now resolves
  owners through every checked machine's attachment identity, covering records
  attached only to boundary-supply machines which never produce unit plans
  (`attached_data_shape_identity` in `selected-dispatch::service_custody`). First
  refusing emission stage remains attached-Unit closure — the legs' bodied
  boundary machines carry no boundary plan for a unit caller, and
  scalar-returning boundary calls (`BoundaryScalarCall`) have no state-graph
  custody admission
  (`native_uefi_os_handoff_invocation_reports_missing_boundary_plan` pins the
  diagnostic). Next acceptance: lower bodied boundary machines as callees (or
  admit boundary scalar results to unit edges), then emit the cycle and run the
  firmware/controlled-provider harness legs below.

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

  `external-roots/src/platform_bringup/secondary_processor.rs` carries the
  three-way `SecondaryProcessorStartupVerdict` and the settlement leg:
  `complete_secondary_processor_startup` returns pending custody only on
  `DefiniteNondispatch`, keeps the account invoked and held on
  `DispatchUnconfirmed` (neither withdrawable nor reissuable; the returned
  carrier still answers a later definitive receipt), and marks started on
  `ConfirmedArrival`. `settle_secondary_processor_startup` releases an
  invoked, unconfirmed account only when a settlement receipt naming the
  exact outstanding carrier attests both that no later arrival remains
  possible and that nothing executes on its stack or state; either unmet
  premise returns the carrier still outstanding, so a timeout cannot
  release stack/state/code or erase an outstanding attempt. Completion and
  settlement both bind the record's outstanding invocation, so a delayed
  or replayed acknowledgement cannot resolve another attempt.

  `BOUNDARY-ISSUANCE` owns the general issuance review, not this ledger's
  concrete repair. The authored Cathedral route now exists:
  `tests/omega/pass/memory/secondary_processor_canary/` authors the
  `StartupEnvelope` linear boundary data, its `Pending` domain established
  by `SecondaryProcessorEntry::enter`, the `confirm` requirement gated on
  `Pending`, two provider roots whose `Calling` policies grant distinct
  dedicated stack classes, and a roster machine declaring both slots' stack
  and state geometry plus the startup trampoline bytes, alignment, and
  low-memory limit. `compiler/tests/secondary_processor_startup.rs`
  evaluates that authored source, selects each provider plan, resolves its
  `enter` reach to `MachineControl`, replays the validated boundary plan,
  installs the authored trampoline bytes through the artifact → extent grant
  → claim → materialize → freeze → validate → install ladder, binds the
  authored profile to the installed code (`InstalledSecondaryProcessor
  Trampoline` startup vector is the claimed base over the authored
  alignment), and drives the ledger: reach the installed entry with
  `binds_exact_materialized_entry_bytes` proving the placed bytes are the
  authored ones; definite nondispatch withdraws while elapsed time does
  not; dispatch-unconfirmed settles only on a two-premise receipt naming
  the outstanding carrier; a late confirmed arrival still starts; receipts
  replayed across re-admission or minted for another processor refuse;
  overlapping state or shared stack class refuse admission; started
  accounts retire only on a quiescence receipt bound to the started
  record. The ledger surface inside
  `external-roots/src/platform_bringup/secondary_processor` is complete:
  bind, admission, invocation, nondispatch/unconfirmed/arrival verdicts,
  two-premise settlement, replay/foreign/stale rejection, overlapping
  resource refusal, and quiescence retirement are all witnessed in
  `secondary_processor/tests.rs` (plus ledger-side adversarial rows).
  Remaining before full acceptance: (1) native emission of the trampoline
  and entry/exit stub bodies — the installed bytes are authored data, not
  emitted code; this is a new emission lane, not a ledger edit, since
  `secondary_processor.rs` deliberately owns custody only and
  `calling-conventions`' `entry_exit_stub` derives the deriver contract
  (push list, saved-area geometry, iretq exit, contract binding at
  `identity`) without emitting bytes — the leg needs a byte emitter fed
  by that deriver (or an ISA-crate encoding surface for the real-mode
  trampoline) plus wiring so the test installs emitted bytes instead of
  the authored `trampoline_bytes` array in
  `tests/omega/pass/memory/secondary_processor_canary`; (2) real receipt
  ingress under BOUNDARY-ISSUANCE; (3) the Windows/macOS/QEMU legs, which
  are host-unavailable here.

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

  Fenced deficiency (witnessed 2026-09-19): a ProgramEntry root data carrying a
  fused `Service<T> in Bound` field fails selected establishment with
  "rejoins 0 Terminal attachment identities" whenever trait `T` declares any
  value-returning method, even `-> i32`; void-method traits pass. The missing
  `attachment_type_identity` row is produced in t2c
  `execution/unit/composed_control`, owned by ARCHITECTURE-CONTROL-GRAPH's live
  claim — the invoked-route fixtures cannot check until that leg lands.

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

  Ingress audit of installation, retirement, startup, interrupt and callback
  receipt edges in `backend/runtime/external-roots` completed at `8973814b6a`
  on Linux x86-64 (`cargo nextest run -p external-roots --no-fail-fast`: 246/246
  pass): every accepted record traces to checked execution or the exact selected
  admitted provider contract — install/remove/teardown replay retained root
  evidence, interrupt entry/turn/finish rejoins the admitted arrival context and
  declared nesting, mask save/restore is LIFO-exact, table member admission,
  descriptor replay and publication replay the sealed member set, callback
  registration consumes linear `Arc`-provenance capacity, secondary-processor
  lifecycle binds the verified trampoline and account custody, and program-local
  cohort seals re-derive the enumerable set. No record traces to neither; the
  `platform_bringup/uefi_bootstrap` subtree audited read-only under
  `UEFI-PHYSICAL-SEMANTIC-ENTRY`'s claim. The open frontier is the source-issued
  leg above. `AP-BRINGUP` owns its concrete arrival/cancellation state repair.

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

  `TerminalExecution::start_verified_module` binds each direct-entry roster
  row to one exact `TerminalPlacedViewEstablishment` — the provider's loan of
  the qualified referent backing, recorded as a live occurrence and retired
  when the entry invocation completes; missing, duplicated, stale or
  undeclared supplies and overlapping exclusive referents reject at start.
  The executable input boundary now owns the same ordinary route:
  `AdmittedNativeArtifact::try_into_native_input_with_placed_view_establishments`
  joins each direct-entry roster row to exactly one supply and carries the
  bound set inside `VerifiedNativeArtifactInput`, while
  `try_into_native_input` and `prepare_native_realization_input` keep
  rejecting nonempty rosters because no provider establishment reaches the
  image entry shim yet. The referent half of each supply now also rejoins the
  module's own catalogs: the declared structural type must be a declared
  carrier, the path must resolve through that shape graph to a real place,
  and every domain qualification must be a domain the artifact declares over
  that carrier — a supply whose backing, range, or qualifications fails the
  rejoin is a stale or substituted establishment and rejects before access.
  Both execution boundaries apply the same declaration-consistency relation in
  `terminal-semantics/src/placed_view_referent.rs` before binding the loan.
  The source-to-artifact regression rejects undeclared backing, invalid paths
  and forged qualifications at interpretation as well as native admission;
  interpreter controls retain valid nested paths, exact domain carriers,
  alias rejection and retirement. This checks fresh runtime inputs against
  the artifact; it does not supply provider authority or realize field access.
  `placed_view_establishment_binds_each_row_and_rejects_exclusive_overlap`
  exercises the whole join on a multi-row roster: supplies bind each declared
  row in roster order regardless of supply order, a partially answered roster
  and a stale sibling supply reject, an exclusive referent overlapping another
  established referent — equal path or prefix containment — rejects as
  aliasing, and disjoint sub-paths or shared-borrow overlap admit. Rows on
  non-entry machines still fail closed at both boundaries — call-bound
  custody arrives through the caller, a route neither carries. The executable
  boundary now carries the loans one step further:
  `prepare_native_realization_input_with_placed_view_establishments` binds
  the supply inside the reusable prepared input, `realize_image` reopens the
  exact bound set, and `validate_executable_entry_receiver` attaches it to
  the `ValidatedNativeProgramEntrySettlement` the object binder must satisfy —
  while receiverless admissions (free entries and erased receivers) reject a
  nonempty bound set rather than publish an entry whose custody was never
  supplied. The emitted entry boundary does not lend yet:
  `emit_optimized_fragments` fails closed on a settlement carrying bound
  establishments until `bind_hosted_receiver` extends its shim to receive
  each referent. Continue from `compiler/native-realization`'s
  `native_realization/optimized_fragment_projection.rs` and
  `hosted_receiver.rs`: teach the hosted bridge to lend each bound referent
  for the invocation's duration, keeping unsupported consumers rejecting
  until they carry it; a roster or a pointer is not this authority. Preserve
  the verified native-input boundary enforced by architecture checks.

  Extend `compiler/tests/access_plans/source_access_policies.rs`'s
  `direct_placed_view_input_survives_codec_and_native_replay` into a source
  program that establishes a view, performs a checked access and retires it
  through published native execution. The test now establishes and retires the
  roster's view at interpretation, but its authored consumer is still empty:
  `view.status.read()` lowers to a boundary call on the derived `PlacedField`
  accessor, which has no boundary plan — a checked access needs the accessor
  realization (a provider or compiler settlement) before source-level use and
  retirement exist.

  Acceptance: valid views retain the same semantics through codec, optimization,
  interpretation and native execution. Stale/substituted plan, artifact, backing,
  range, rights, occurrence or lifetime rejects before access; failed establishment
  returns custody and retirement preserves the declared resident/vacant state.
  Run each available supported host leg and explicitly report unavailable ones,
  without treating cross-target emission as physical execution.

- **SYMBOLIC-MATERIALIZATION.** Complete symbolic field/index materialization
  and target-dependent realization as one
  [derived consumer](wiki/spec/layouts/plans.md#derived-consumers) of a
  normalized plan. Paths and bounds stay exact until assignment; physical
  lowering chooses instruction bytes and context registers, not semantic slots.
  Extend the existing recursive record/sum owner in
  `omega-rust/omega/backend/layout/src/sum_materialization/mod.rs` and the
  carrier preparation/walker in
  `omega-rust/psi/foundation/layout-plans/src/symbolic_materialization.rs`.
  One ordered `{ outer_layout, children }` report retains field/index hops
  and sum/record interiors; do not add depth-specific implementations or
  per-shape report channels.

  Remaining work:

  - Array lengths still symbolic at layout time remain fenced, and that
    fence is now a recorded design boundary rather than pending capacity:
    a `ConstParameter`/`ConstCall` length can only live inside an unapplied
    template, and `build_layout_plan` never lays a template out — there is
    no runtime layout a passed count could describe, and no bindings
    channel exists to name one. Every concrete use arrives as a synthesized
    closed instance whose substituted members already carry `Literal`
    lengths: `Log<const N>` reached through a member typed `Log<2>`
    materializes under the const-evaluable closed-argument judgment
    (`require_closed_generic_application`, mirrored backend-side in
    `validate_closed_copy_record`), and
    `generic_instance_symbolic_materialization_realizes_on_both_linux_isas`
    in `layout_plans/writer_lowering.rs` lowers instance record paths —
    `Neighbor<two()>` beside `[Neighbor<1>; 2]` — on both Linux ISAs (native
    execution on x86-64). Runtime-bound `value` counts cannot determine a
    static layout at all (the `value_generic_runtime_static_length` fail
    corpus pins the rejection), so the residual fence covers exactly the
    shapes no closed checked identity can name: open templates and
    non-closed applications. The recursive owner pins both shapes of that
    boundary:
    `open_templates_carrying_parameter_lengths_stay_fenced_under_the_recursive_owner`
    covers the unapplied template, and
    `non_closed_member_applications_stay_fenced_under_the_recursive_owner`
    covers a member typed by a non-closed application (`Log<two()>`): it
    joins the ordinary fields, keeps whole-field writes, and rejects
    traversal below the boundary for want of a carrier. Closed
    literal/generic zero-count arrays are no longer this gap.
    Standalone rungs retain their narrower single-hop and nonzero-length
    contracts.
  - Obtain matching-host Linux AArch64 execution evidence — the sole
    residual. The writer harness validates both Linux ISA fragments on every
    host and executes the host-matching one on Linux x86-64/AArch64 or macOS
    AArch64; macOS execution does not close the Linux runtime row. Linux
    x86-64 leg re-verified at `340e2b5ca4`: `cargo nextest run -p compiler
    --test layout_plans -E 'test(~writer_lowering)' — 13/13 green, each test
    replay-validating both ISA fragments and natively executing the x86-64
    writer against the reference image; fence pins
    `open_templates_carrying_parameter_lengths_stay_fenced_under_the_recursive_owner`
    and
    `non_closed_member_applications_stay_fenced_under_the_recursive_owner`
    green under `cargo nextest run -p layout --lib -E 'test(~fenced)'` (3/3).
    Resume on Linux AArch64 with the same filtered run; the harness's
    `cfg`-selected arm branch then executes the AArch64 fragment natively.
    The empty-array regression in `layout_plans/writer_lowering.rs` also
    pins direct/indexed-write rejection, live sibling writes and empty
    nested/generic carriers.

  Acceptance: nested field/index canaries execute on both Linux ISAs and compare
  destination bytes with the reference image, including guard bytes. Both ISAs
  must retain the same normalized fragment fingerprint and writer invocation
  while emitting their own bytes.

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
  than only where they sit. Section version 3 adds the writer-owned
  `.rdata` extent — emitted as `final_import_data_bytes` beside the flat
  executable with its own `PlacedDataRegionInventory` of
  `ImportBindingSlot` rows over the IAT — and replays each Coff thunk's
  decoded `disp32` binding against exactly one placed slot whose address,
  symbol, byte count and section-relative offset match, and vice versa
  (`image-pe::validate_pe_x86_64_import_binding_pairing`). The dynamic ELF
  writer emits the same custody shape: its `.got.plt` enters
  `final_import_data_bytes` with one `ImportBindingSlot` row per bound
  import, each offset grounded in the applied `.rela.plt` `r_offset`
  binding write and each symbol naming the versioned locator's dynsym
  spelling; Mach-O's slots already ride the data inventory and the static
  ELF lane still refuses imports outright. Replay also
  re-derives the container's own declared entry — ELF64 `e_entry`, PE32+
  `ImageBase + AddressOfEntryPoint`, Mach-O 64 `LC_MAIN` mapped through the
  `__TEXT` segment — from the committed bytes alone and requires it to name
  the start of a placed executable region; a loader-visible entry that lands
  off a committed boundary rejects. Replay also re-derives the loadable map
  the container declares to its loader — ELF64 `PT_LOAD` offsets and
  `p_flags`, PE32+ section raw ranges and characteristics, Mach-O 64
  `LC_SEGMENT_64` `fileoff`/`filesize` under `initprot` — and requires the
  declared extents to be pairwise disjoint, the text extent inside
  executable coverage, the data extent inside writable coverage, and the
  import-data extent inside some loadable range; containment is
  one-directional because the emitted layouts legitimately map bytes past
  the extents (ELF headers inside `PT_LOAD`, PE raw padding, Mach-O
  `__TEXT`'s own header). That still
  establishes custody and thunk
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
  - Entries and incoming edges over those decoded rows — the custody half
    (the container-declared entry landing on a placed boundary) is landed;
    the semantic half (control-flow edges and entry obligations inside
    compiler-function regions) is not.
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
  now specifies every operation, terminator, scalar-term, proposition, and
  proof-node form the codec accepts, plus the machine, scalar-block-invariant,
  catalog, and obligation-ledger row layouts, the byte envelope — magic,
  format marker, and vocabulary field — each codec emission opens with, and
  the decode-and-rederive payloads a receiver must reconstruct: canonical
  artifact framing, debug-map file/site rows and subject tags, the
  optimization-execution record, the PCC proof sidecar, and the
  mathematical-certificate judgment a receiver replays in the kernel — its
  postorder term table, declaration signature, context, and the term, sort,
  and level tag spaces.
  `tests/architecture/encoding_contract.rs` pins each closed tag space, the
  module's counted-table declaration order, the artifact and certificate
  framing orders, the section rederivation and byte-for-byte re-encoding
  obligations, and every envelope marker against the `terminal-codec`
  definitions so they cannot drift.

  Acceptance: source and producer state can be discarded before an
  independent verifier reconstructs every obligation and executes or lowers
  the same artifact, and the encoding contract specifies every operation and
  proof-node form the codec accepts. The wire-surface legs are landed
  (`1b5a521a1f`, `ad130023702d`, `43e2780cf0`): the contract now covers every
  emission and payload `terminal-codec` accepts, including the standalone
  certificate envelope. Remaining legs are the execution/interpretation side
  of acceptance — interpretation, resource analysis, native lowering, and
  installation custody — which live outside this codec fence.

  Resource-analysis slice landed: `terminal-fixed-fuel` segment derivation
  composes acyclic conditional and case interiors as the maximum arm
  (`block_to_edge_bound` in `fuel_certification/segment_partition.rs`), so a
  multi-block segment certificate bounds every walk that commits its endpoint
  instead of failing closed (`verify_module` + `derive_fixed_segment_fuel`,
  `cargo nextest run -p terminal-fixed-fuel` 60/60). Open inside that leg:
  invocation-bound callees, ranked-cyclic interiors, and
  relevant-precondition derivation.

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

  - The `check` state's conditional guard now carries a checked form:
    `values/scalar/boolean_lowering.rs` decomposes `BoundedOwned` carrier
    `==`/`!=` against a byte-sequence literal into a live-length
    `IntegerComparison` over `StructuralParameterByteLength` folded with one
    `StructuralParameterIndexedRead` equality per literal byte, so the
    existing `scalar_expressions` `Guard` row admits it without a new variant
    or a `boolean_expression_reads_carrier` arm. Composed bodies also admit
    `ScalarCall`/`BoundaryScalarCall` through `ScalarCallSite`
    (`execution/unit/scalar_targets`), which lifts the ordinary
    `available_target` caller view onto `CheckedComposedUnitControlStatePlan`;
    `digit_write`'s byte-store `ScalarResult` needs that, since
    `CheckedByteSequenceStoreValue` has no `Computation` carrier. On
    2026-09-20 (Linux x86-64) the call-store pair is admitted through
    composed-control emission: `state_graph/body.rs` counts
    `ScalarResult`-valued shared stores as body effects, gives `ScalarCall`
    a binding namespace that skips discarded results, and admits the
    `ScalarCall` + `LocalData`/`Assignment`/`Call` operand pairs;
    `composed_control/admission.rs` retains `ScalarCall` operations
    through source-call custody, callee fingerprint/commitment, and
    per-kind service-reach agreement; `composed_control/scalar_calls.rs`
    collects op-level call targets into the embedded catalog;
    `composed_control/emission.rs` emits `ScalarCall` as
    `OperationKind::Call`/`CallStructuralScalar` through the same
    binding-ordinal and contract checks ordinary machines use.
    `content_text_and_carriers::runtime_number_to_decimal_exit_canary_runs`
    now advances past `digit_write` to the `check` guard and fails at
    `Lowering(Unsupported("indexed reads require a whole byte-view
    parameter"))`: the named check is affirmative — Terminal
    `OperationKind` still lacks a leaf reading bounded-field byte content.
    The t2c equality decomposition emits `StructuralParameterIndexedRead`
    conjuncts with `path=[Field(out)]`, while
    `expression_preparation/prepare_expression.rs` only admits the
    whole-parameter byte view and terminal `ByteSequenceRead` carries no
    field path (`StructuralByteSequenceFieldLength`/
    `StructuralByteSequenceFieldByteStore` are the only field-path byte
    ops). Resume there: add a field-path byte-read leaf through Terminal
    `OperationKind` (parallel to `ByteSequenceFieldLength`, resolving
    through `structural_fields::resolve_byte_length`'s carrier walk) —
    `representations/terminal-psi` is outside this wave's TR3-TR8 claim.
    Sibling canaries `runtime_bounded_carrier_write_read_exit` and
    `utf8_equals_literal_exit` already fail in this closure's custody
    gates at base, so the equality frontier is shared, not
    decimal-specific.
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
  and selected integer comparison occurrence rosters. General call and control
  routes still need the emitted-operation joins below.

  Remaining work:

  - Give the remaining crash-qualified uses a replayable Terminal carrier. A
    named `Namespace::requirement(...)` use has no emitted-operation join and
    fails closed in the producer, as do a non-scalar or miscounted operand
    roster and a call operation used as the selected operator's emitted carrier.
    A guarded float operator still needs structured Terminal guard lowering:
    `proofs/crash_routes/scalar_terms.rs` rejects `IeeeFloatComparison`.
    A generic operator or a guard through a structural formal
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
  association (`float_comparisons/`, `float_fma/`). Named uses and every
  non-comparison operator still have no join. One
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
    and now the checked `TrappingShiftLeft`/`TrappingShiftRight` forms with
    "requires runtime policy realization". Those shift kinds exist
    (`checked_integer_binary_kind` maps `(ShiftLeft|ShiftRight, Trapping)`)
    and evaluate through the primitive's exact shift, so a Trapping shift,
    whose out-of-range count the spec makes an executable trap condition,
    carries a normal-return fact and refuses explicitly at expression
    preparation instead of vanishing from the computation plan as an
    `InvalidUnitMachinePlan` omission; package-evidence projects them onto
    integer-binary vocabulary tags 22-23. The five remaining Trapping
    arithmetic operators keep their no-fact boundary: their check-stage
    refusal is pinned by
    `flow/transfers/byte_sequence_tests.rs::argument_cast_policies_follow_the_callee_parameter_domain`,
    under a live NOMINAL-FIELD-FLOW claim at this writing. The Trapping
    refusal in `scalar_graph/scalar_contracts/namespace.rs` is
    contract-position and stays: direct Trapping arithmetic forms no
    predicate term.
  - Realize modular conversion with a signed source or target;
    `prepare_expression.rs` lowers only unsigned-to-unsigned
    `IntegerWrappingCast`. Do not retry expression-level composition:
    truncation toward zero is not the modular image of a negative dividend,
    a same-width sign reinterpretation needs a value-level select that
    Lowered Psi has no operation for, and masking plus an exact cast needs a
    bitwise range the spec denies.
  - Boolean-to-integer conversion landed at 4133043eab:
    `CheckedScalarComputationKind::BooleanToInteger` owns the authored cast
    occurrence and its evaluated operand and lowers through the ordinary
    conditional selection of destination-typed 0 and 1.

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
  source customers cover scalar `==` symmetry/transitivity, fixed-literal
  discreteness, subtraction rank decrease, its no-underflow obligation and
  guarded addition bounds and nested Boolean result contracts
  (`compiler/tests/proof_kernel_canaries.rs`, `kernel_discreteness.rs`,
  `kernel_subtract_order.rs`, `kernel_add_bound.rs` and `kernel_equality_transport.rs`),
  not general dependent theorems.

  Remaining work:

  - Give the bounded arithmetic families kernel meaning, so each is a
    certificate producer whose output the kernel checks or an explicitly
    justified checked rule (see Flag). Integer carrier, order and equality
    declarations are landed in `mathematical_core/bounded_denotation.rs`:
    `Int`, `IntLt`/`IntLe` and open `Int`-valued term constants are interned
    assumptions with exact statements. Closed mathematical terms intern
    by exact evaluated value; fixed scalar magnitudes have shared signed
    binary definitions, while larger values retain opaque exact-value
    constants. The order/equality rules
    cite one fixed roster (`eq_le`, `lt_le`, `le_trans`/`lt_trans`/
    `lt_le_trans`/`le_lt_trans`, `lt`/`le_subst_left`/`_right`) while
    `Id` symmetry/transitivity and value-equation transport use `J`;
    the `Equal`↔`IntegerMathEqual` citation crossing shares one
    denotation. The separate integer-order substitution rule still cites its
    fixed laws. Boolean values, negation and equality use `Two` and `caseTwo`,
    exposing their operands to the same multi-equation transport as exact
    addition/subtraction. A reflexive expanded result uses `refl`, not an
    assumed implication for that program.
    Discreteness derives adjacent literal order from five fixed numeral laws
    and composes it with the inclusive premise. Exact scalar subtraction
    applies fixed zero and antitonicity laws; evaluated differences retain
    canonical numeral identity and use binary order. The correlated unsigned
    subtraction witness derives its zero lower bound from fixed self-zero
    and non-strict antitonicity laws. Contradictory closed premises use fixed
    irreflexivity and checked empty elimination. These laws remain explicit
    assumptions. Mathematical subtraction shares that operation. Open addition
    also retains its operands; correlated lower and upper bounds use fixed
    addition monotonicity and subtraction cancellation whenever the sum and
    difference stay unreduced, including exact SSA subtraction definitions and
    carrier-endpoint equalities; conclusions whose endpoints both evaluate are
    decided on their canonical constants — strict order by the binary numeral
    laws, equality by `refl`, and a false relation by empty elimination
    through a checked false premise — so a closed right addend no longer
    forces an instance axiom. An open sum over a closed difference
    substitutes a checked numeral-operation equation `add n r = e` —
    interned once per evaluated operand triple — for the applicative
    cancellation step. An already admitted open expression stays opaque
    if composing a child would introduce a resource refusal. Still to do:
    other bound-witness forms, transport through opaque operations, nested
    canonical identity reversal, and Boolean identities requiring case analysis
    rather than structural correspondence. Other operations remain
    opaque; unsupported arithmetic derivations, including `x + 0 = x`, still
    assume their conclusions.
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
  terms by exact evaluated value. Subtraction order, the correlated
  unsigned subtraction lower bound and correlated addition bounds with
  open right addends use fixed arithmetic laws. Remaining bound-witness forms,
  opaque or noncompositional transport and construction-budget fallback still
  use per-instance `rule_axiom`s. Addition and subtraction retain their operands;
  other open arithmetic remains opaque. Fixed laws still require exact
  assumption admission and do not establish arithmetic consistency.

  Resume transport coverage with `cargo nextest run --release -p compiler
  --test kernel_equality_transport --no-fail-fast --no-tests fail` on macOS
  ARM64 (witnessed on base `7216c52a2f` plus this implementation). The nested
  Boolean source reaches a decoded, independently reconstructed
  result obligation and a kernel-checked mathematical wire without an instance
  assumption for its conclusion; missing equations, substituted executable
  operands, a changed J endpoint and the false source twin reject. The next
  transport work must retain a real source obligation needing one of the
  remaining cases above, not add another standalone theorem builder.

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
  Top-level `let`/`boundary let` declarations with binders, dependent arrows
  and prefix applications now parse
  (`tokens-to-syntax-trees/src/declarations/let_definition.rs`), resolve, and
  type into the typed-tree mirror
  (`typed-trees/src/typed_trees/evidence/mathematical.rs`) — arrow- and
  application-typed telescope parameters, curried applications, bindered
  `boundary let`s and call bodies are all pinned by shape-retention tests
  (`lowerer/tests/mathematical_declarations.rs`) — and elaborate to
  `CheckedMathematicalDeclaration` records in
  `typed-trees-to-checked-trees/src/proof/mathematical_declarations.rs`:
  binder carriers classify `core::Level`/`core::Type` by authored name,
  arrows and applications render their nested-Pi identities, and
  `boundary let` records its named-assumption absence, and each record now
  also elaborates to a `mathematical_core::signature::Declaration` list the
  kernel itself re-decides
  (`typed-trees-to-checked-trees/src/proof/mathematical_signature.rs`):
  `core::Level` binders become universe parameters, bare and unapplied
  `core::Type`/`core::Strict` occurrences generalize fresh ones in authored
  order, other resolved carriers intern as `Type 0` assumptions in the
  shared signature prefix, telescopes and arrows fold to `Pi`/`Lambda`
  spines over de Bruijn scope, `core::Squash` forms the proposition, and a
  transparent definition body must inhabit its declared result or the
  program fails with that declaration's own kernel diagnostic. Binder
  property bounds and `Machine`/`Proposition` binder kinds already refuse
  loudly in `mathematical_signature.rs::plan_binder`.
  `checking.rs` now admits programs carrying them: the checked records land
  on `ProofFacts::mathematical_declarations`, and every
  `checked-trees-to-lowered-psi` entrance refuses them with a named
  `PROOF-CONTRACT-MIGRATION` diagnostic (fail canary
  `proofs/mathematical_declaration_lowering_rejected`) until a Terminal
  evidence encoding exists — broader machine-valued body denotation,
  applied carriers, and that encoding are pending legs. No
  `core::Level`, `Type`, `Strict` or `Squash` declaration exists, and the
  dedicated `proposition` declaration with its named-witness call lanes
  (`typed-trees-to-checked-trees/src/proof/proof_output_calls.rs`) still
  carries `core/int.omg` and 37 files under `tests/`.

  Remaining work:

  - Parse, resolve and type of the top-level `let`/`boundary let` grammar —
    closed parameterized declarations, dependent function types, curried
    prefix application, parameter binders and named assumptions — has landed
    structurally; checked elaboration into `CheckedMathematicalDeclaration`
    has landed (binder classification, nested-Pi and application identities,
    named assumptions). Kernel-term elaboration has landed for the admitted
    grammar. Remaining here: extend machine-valued body denotation beyond
    the admitted bounded vocabulary, replace authored-name carrier
    classification with symbol identity once the fixed `core::*`
    declarations exist, and encode the checked signature into Terminal
    evidence so the lowering consumer stops refusing
    (`terminal-codec`'s certificate wire exists but has no production
    caller). Preserve ordinary local bindings, complete
    machine calls and executable callback selection. Add no quantifier
    keywords, and do not substitute declaration enumeration or an
    optional-returning decider for mathematical quantification.
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
  coverage, not soundness: four reconstruction rows are `Proved` by
  generation-time certificates (`fact:boolean-polarity-implications`,
  `fact:successor-path-transport`, `fact:branch-condition-transport`,
  `fact:header-invariant-members`), every other row is `ExplicitlyTrusted`,
  and the codec's trust-graph descriptor still names the Rust decoder and
  verifier as trusted judgments. Dependency edges are closed mechanically:
  a duplicate edge or a claim-bearing row naming an `Unfinished` row fails
  (`UnfinishedDependency`, `DuplicateDependency`). The proved set is recorded
  in `PROVED_ENTRIES` and closed by `check_proved_set`: a row marked `Proved`
  outside the recorded set fails, and a recorded row that regresses or
  disappears fails, so a soundness status change can never hide inside an
  ordinary entry edit (`the_recorded_proved_set_matches_the_marked_rows`).
  The codec implementation surface is
  inventoried like the verifier and representation closures:
  `terminal-codec/build.rs` folds every Rust source under its `src/` into
  `terminal-codec/source-closure`, bound alongside `terminal-codec/Cargo.toml`
  (whose `[lib]`/`[[test]]`/`autotests` declarations fix the compiled surface)
  to the canonical-bytes root and the decoder node so an unbound codec source
  cannot change unobserved. A crate-directory sweep fails on any production
  `.rs` outside `src/`, `build.rs`, or the declared `tests/` target, and the
  module-path and include tokens that could compile a file the closure never
  committed are banned
  (`sections::trust_graph::tests::{codec_source_closure_retains_every_source_exactly,codec_inventory_covers_the_whole_crate_directory,codec_sources_never_escape_the_closure_root}`).

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

  Landed: erased formals carry a proof-only scalar term lane end to end.
  `terminal-psi` contracts publish `erased_scalar_formals`; Call-family
  operations and Jump/Conditional successors carry `erased_arguments` lanes
  inside the contract commitment; the codec encodes them unconditionally so
  pre-change bytes reject; the checker records erased actuals under
  `ErasedUnitCallArgument` and erased transition actuals under
  `TransitionArgument` keyed by authored position; lowering binds the lane from
  the caller's erased namespace; and `terminal-verifier` substitutes each
  erased actual into the callee's `requires` at call sites and edges
  (`call_composition.rs`), rejecting missing, substituted or out-of-closure
  rows. Composed-route state contracts carry `requires`, and selection admits
  a borrowed receiver on a call bearing requirement obligations (obligations
  are provenance-only downstream).

  Remaining work:

  - Erased non-primitive parameters: `erased_scalar_parameter_plans` still
    refuses typed formals such as `Nat` (the
    `erased_proof_only_typed_parameter_exit` canary stays checked-only),
    now by name — `shape_admission`'s
    `validate_erased_runtime_scalar_formals` rejects an erased non-scalar
    formal on a non-proof machine at check time instead of omitting it
    from the calling plan silently (`fail/relevance/erased_nonscalar_parameter`
    pins the surface; proof machines keep their proof-side carriers).
  - Done: the internal-calls lane carries requires-bearing and
    erased-formal callees — composed-control internal targets publish
    `erased_scalar_formals` and `requires`, emission resolves erased
    actuals and allocates one obligation per published row, and the
    published roster stays in canonical order (authored clauses merge
    ahead of the derived parameter-range tail; merged propositions are
    sorted and deduplicated at every publication site).
  - Done: a `&dyn` call to a requirement declaring an erased formal now
    explicitly refuses its erased lane — check-time diagnostic naming the
    requirement and formal (`checks/contracts/dynamic_erased_lane.rs`,
    covering direct bindings and descriptor fields stored in records;
    `fail/relevance/dynamic_erased_formal_lane` pins the surface).
    Carrying the lane through the descriptor would need a proof-actual
    channel on `CheckedDynamicScalarCallPlan` and emitted vtable rows —
    a larger slice than the refusal.
  - Done: erased `self`, `const` and `mut` bindings now refuse by name at
    check time. `strips_erased_parameter` already refused them a calling
    plan, but the omission surfaced only as an unadmitted plan downstream;
    `relevance/shape_admission.rs::validate_erased_binding_qualifiers`
    diagnoses the qualifier on signature parameters (machines and trait
    requirements), `let mut` locals, and reference formals, pinned by
    `fail/relevance/erased_mutable_parameter`. `self [erased]` is not
    expressible in the grammar (receivers take no binding properties); a
    `&mut x [erased]` formal fails as `mut`. Keep the refusal unless the
    specification gives the combination a meaning.

  Acceptance now holds for the erased-term lane:
  `pass/relevance/erased_parameter_proof_only` (exits 70) and
  `erased_parameter_named_transition_forward` run natively; a violating erased
  actual, an actual outside the admitted closure, a missing or substituted
  erased-argument row, and pre-change codec bytes reject in source-free
  verification; the `fail/relevance/` runtime-read and receiver controls keep
  rejecting. An internal-call callee with `requires` admits and discharges:
  `composed_unit_internal_calls` covers the carried lane, obligation arity,
  violating and missing erased actuals, and a two-erased witness pinning the
  second formal's ordinal. New acceptance: an erased non-primitive formal
  reaches the contract term lane, and a dynamic call carries or explicitly
  refuses its erased lane.

  Erased-field cleanup belongs to
  **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP**.

## P4 - ABI, borrowing, and callbacks

- **NORMALIZED-ABI-LOWERING.** Finish target-independent signature
  normalization and target-owned calling/layout realization for aggregates,
  dynamic values, callbacks, and foreign boundaries under the
  [calling-plan contract](wiki/spec/build/calling_plans.md).

  Remaining work:

  - Finish aggregate and descriptor foreign transport. Fixed scalar arguments
    and results now reach independently validated native execution; preserve
    the macOS ARM64 source-to-C oracle:
    `mbx nextest run -p compiler --test source_evaluated_native_realization --no-fail-fast --no-tests fail -E 'test(=scalar_native_arguments::mixed_foreign_scalars_execute_with_exact_argument_and_result_values)'`.
    Other matching-host runtime legs and the mixed scalar/record execution
    below remain open; cross-target selection replay is not runtime evidence.
    Borrowed flat-record projections, owned
    whole-place aggregates, and borrowed dynamic descriptors all compose
    with scalars in retained formal order: an `Owned` argument admits only
    an empty path against the root structural type and joins the plan's
    ABI-classified destination on byte size, alignment, and a
    non-`BorrowedReference` class; a `ByteSequence(BorrowedView)` formal
    borrows the caller's whole stored view or a stored descriptor field and
    joins the plan's two-word by-value destination, witnessed by
    `normalized_foreign_owned_aggregate_arguments_retain_whole_place_and_plan_transport`
    and
    `normalized_foreign_borrowed_view_descriptors_admit_whole_place_and_stored_field`.
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

  Flag: dynamic arguments/dispatch still split the target call inventory by
  signature shape (`target-operations/src/target_operations/operations/unit.rs`).
  Extend the ordinary `Call` and its explicit result custody with argument
  classes and callee sources realized by the target's calling policy as native
  descriptor support lands. Preserve the foreign formal-order mapping and
  independent checks; do not introduce another call family for each newly
  supported signature combination.

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
  through calls/results, dynamic dispatch, cleanup and native execution.
  Source planning belongs to `typed-trees-to-checked-trees/src/execution/`
  and `checked-trees-to-lowered-psi`; native reference preparation belongs to
  `target-operations-to-selected-instructions/src/legalization/scalar_graph_input/`.
  **STRUCTURAL-BORROW-IDENTITY** owns the common reference ABI. Preserve
  original referents and exact place/loan custody across these stages.

  Remaining work:

  - General aggregate and `[copy]` sum replacement beyond literal plain records
    with scalar fields. Preserve displaced custody and whole-value validity;
    the existing ordered field-store decomposition is not a general aggregate
    replacement operation.
  - Domain-qualified byte-field replacement from a runtime source. The named
    `frontier_pins::domain_qualified_field_store_still_misses_the_checked_control_plan`
    control supplies the source; retain its encoding proof and exact source
    value through field-store planning, Terminal replay and native execution.
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

  Resume with the maintained `terminal_psi_indexed_receivers` integration target,
  not the deleted `zz_wob_probe`. Its `frontier_pins` module distinguishes
  required semantic rejections from outstanding producer limitations; move a
  repaired limitation into the relevant execution module with caller-storage
  observation. Preserve `held_borrows`, `indexed_stores`, `owned_subloans`,
  `borrowed_arguments`, and `primitive_stores` as regression coverage, not new
  implementation assignments. Run `mbx nextest run -p omega-native-differential-test
  --test terminal_psi_indexed_receivers --no-fail-fast --no-tests fail` with
  `RUST_MIN_STACK=67108864` on macOS ARM64; retain separate Windows and Linux
  runtime requirements rather than counting four-target publication as execution.
  Extend the shared place/loan sequencer under **STATE-LOCAL-VALUE-FRONTIER**;
  do not reintroduce one producer family per arrangement of calls and stores.

- **STRUCTURAL-BORROW-IDENTITY.** Enforce the settled
  [structural borrow identity contract](wiki/spec/terminal-psi/structural_access.md)
  through call argument preparation and native validation/replay. The
  [target signature checks](omega-rust/omega/pipeline/abstract-operations-to-target-operations/README.md#references-calls-and-storage)
  now replay embedded callee plans, projected argument/home identity and
  standalone receiving entrances against signatures derived from the
  declarations. Every borrowed access keeps `BorrowedReference`, an inline byte
  field cannot satisfy a parameter by shape equality, an exclusive view
  reaches an ordinary call from a non-entry block parameter. Owned parameter
  fields now lend exact mutable/write-only subloans through receiver
  reconciliation, Terminal verification and `structural_arguments_match`.
  This does not establish every projected source form.

  Remaining work:

  - Complete mixed field/index paths and other admitted source owners through
    receiver preparation, native lowering and replay. Literal-indexed and
    mixed `Field`/`FixedIndex` paths now lend to `SharedBorrow` receivers:
    `receiver_calls/mod.rs` admits them for every borrowed target, and the
    lowered/native `exact_borrowed_projection` route plus `argument_custody`
    replay already consume the emitted records
    (`receiver_access::mutable_self_literal_indexed_element_can_supply_shared_receiver`,
    `mutable_self_mixed_field_index_path_can_supply_shared_receiver`,
    `dynamic_indexed_element_still_cannot_supply_shared_receiver`). Explicit
    shared indexed arguments and dynamic `Index` segments still stop in
    `execution/unit/calls`. Terminal's owned-root array and
    construction-local restrictions remain separate; do not infer their
    availability merely from a parameter declaration.
  - Record matching-host runtime results for both Linux targets and Windows.
    `terminal_psi_indexed_receivers::owned_subloans` publishes objects, images
    and installation records for all four hosted targets; its published text
    has run only on macOS ARM64. Cross-emission is not runtime coverage.
    Preserve the broader `terminal_psi_indexed_receivers` and
    `primitive_store_return` controls.

  Acceptance: caller-visible writes, forwarded references, legal synchronized
  shared observations, write-only non-reading, and register/stack pointer
  passing work on both Linux targets, including an owned local's field lent
  `&mut` and `&write`. Independently formed or substituted access/shape/
  placement pairs reject; shared physical shape never authorizes access
  substitution. Direct-home controls use owned semantics or test rejection of
  borrowed copies. A following callee seeing the staged write is not
  caller-visible writeback.

  The duplicated static-path rule is consolidated in
  `terminal-semantics::static_path`, following the settled
  [loan table](wiki/spec/terminal-psi/loans.md#reborrow-lineage-and-access):
  `canonical_structural_path_tip` (declaration-local field ids, unique match)
  and `runtime_structural_path_tip` (runtime field identities) now serve
  `primitive_place`, `call_composition` and `fixed_byte_view`, which keep
  their access, multiplicity, qualification and tip-shape obligations. The
  rule is parent custody and an exact type-resolved `Field`/`FixedIndex`
  path, not a roster selected by the callee's result kind.
  `record_field_carrier` keeps its own walk because it emits each hop's
  runtime identity rather than only the tip; overlapping-loan rejection,
  material write-only path restrictions and the byte-view presentation
  authority are preserved. An owned inline byte field must not acquire a
  standalone type identity or inherit the borrowed-parent view adapter.

- **BORROW-PROOF-CONVERGENCE.** Make ordinary borrow checking proof-producing
  under the [loan contract](wiki/spec/terminal-psi/loans.md): relational
  evidence may establish disjointness or containment between existing places
  and occurrences, and never creates, extends, duplicates or widens a loan.
  Owners: `typed-trees-to-checked-trees/src/checks/borrows/` and the
  certificate rows in `checked-trees/src/checked_trees/borrow.rs`.

  Index extents compare as normalized bounds inside one replayed selector
  session (`overlap/indexes.rs`, `overlap/segments.rs`), `overlap/premises.rs`
  supplies ordering premises from the forming scope's own `requires` rows and
  shared incoming-guard analysis, and
  forming-loan, statement-mutation, and call admissions retain replayable
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

  - Extend establishment to broader callee/domain predicates and theorem-call
    conclusions, valid for the captured value and place versions at formation.
    Ordinary scalar `ensures` now use the contract checker's shared
    `call_guarantees/availability.rs` reader at each statement entry. Exact
    source facts, invocation coordinates and live assignment provenance bind
    immutable whole results; immutable actuals use the existing bound normalizer.
    Preserve `pass/borrows/borrow_returned_window_write_and_call` and
    `certificates/call_premises.rs`. Mutable results, result-field selectors,
    same-statement nested establishment and theorem-only calls still require
    their exact version/substitution evidence. Guard-derived
    window writes and calls use the range checker's shared
    `incoming_guards.rs` and `requirements.rs` readers; immutable owned
    scalar parameters survive renaming and forwarding only with preservation
    at every hop. Mutable, computed and foreign subjects remain unproven until
    the shared reader supplies version evidence. Do not add a second collector.
    Required concrete integer membership now supplies direct `self`/literal
    comparisons through the same Boolean decomposer. Tokens rejoin the exact
    membership, definition and predicate; aliases use typed expansion.
    Indexed theories, nested membership transport, domain arithmetic and
    other establishment points still need their exact substitution/meaning
    evidence. Preserve `pass/borrows/borrow_domain_window_write_and_call`
    and `certificates/domain_premises.rs` when extending them.
  - Range-premise read sets (`checks/ranges/facts/dependencies/reads.rs`) stay
    incomplete for requirement-dispatched calls, machine-valued and nested
    static applications, quotient and private-layout operations,
    `CompareExchangeOnce`, and authored non-arithmetic operators. Admit one
    only with a complete footprint and operation stability: explicit arguments
    do not establish all callee reads, and preserved numeric captures must
    stay independent of later source writes. The builtin bound-meaning floor
    in `record_dependencies` (`facts/dependencies.rs`) decides what may be a
    range premise. It is not a read-set limit; do not widen it under this item.

  Acceptance: `tests/omega` pass canaries exercise each newly supported
  establishment point with disjoint loans, writes, and exclusive call operands;
  every admission replays from its retained certificate. An absent, non-strict,
  stale, reordered or
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

  These are implementation gaps under the settled loan contract, not owner
  design decisions. Preserve `pass/borrows/borrow_stated_index_disequality_mut`
  and the unknown-index negative control while extending premise sources.
  Preserve `pass/borrows/borrow_guarded_window_write_and_call` and its mutable
  forwarding, backedge, and tampered-source controls in `certificates/guarded_premises.rs`.
  Existing compatibility certificates are checked-stage records; their
  relation enum is not a persisted Terminal wire format.
  Broader independent replay also needs authenticated call-prerequisite rosters
  and copy provenance. Source inspection of `checks/contracts/calls.rs` and
  `prover/call_guarantees.rs::captured_place` finds retained payload/context
  trust; adversarial prerequisite replacement/removal and cross-call copy
  substitution probes remain unrun. The immutable borrow-result join additionally
  checks the original assignment's exact call occurrence; do not remove that
  check while generalizing copies.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Realize target-owned private callback
  slots natively under the
  [private-callback contract](wiki/spec/build/private_callbacks.md). Checked
  compilation and the Terminal product already close for
  `source/library/std/tests/callback_materialization_closure.omg`: two
  `NativePlace::Field` placements selected through exact conformances, one
  `BoundaryCall`, and one canonical thunk artifact per placement
  (`compiler/tests/callback_terminal_custody.rs`,
  `reachable_private_callback_registrar_binds_its_terminal_occurrence`). No
  private channel exists but no native product for a callback does:
  callback thunks now lower to machine code inside realization and reach the
  emitted object as private functions, and the direct-parameter witness
  `direct_callback_relocation_resolves_to_its_private_function` still stops
  where `construction::build_plan` rejects the materialized registrar row
  with `Selection(SourceCustodyMismatch)` — the common instruction pipeline
  still carries no callback ABI transport.

  Remaining work:

  - Landed: each thunk's Terminal artifact lowers to machine code inside the
    same realization. `callback_thunks::lower_callback_thunks`
    (`native-realization/src/native_realization/callback_thunks.rs`)
    re-derives each settlement's artifact through the sealed verified input,
    the request's abstract optimization, target lowering, the verified
    physical pipeline and the fragment-emission ladder, then binds the single
    emitted span into a `CompilerPrivateMachineCodeFunction` (identity = the
    settlement's `MachineFunctionIdentity::callback_thunk`, private symbol =
    the settlement's pinned name, `source_psi` = the thunk's Terminal
    identity). Foreign source identities, multi-function thunks and
    call/import-bearing thunk bodies reject inside realization before the
    program's target-stage wall.
  - Landed: retained realization has a private-function channel.
    `build_function_fragment_object_artifact_with_private_functions`
    (`image-emission/src/function_fragments/production.rs`) emits validated
    private functions between the program text and the import tail;
    `validate_private_functions` (`object_artifact/private_functions.rs`)
    admits any number of rows and dedupes identities and symbols instead of
    rejecting a second one; `emit_optimized_fragments`' projection
    (`optimized_fragment_projection.rs`) carries the realized roster. The
    fragment validator joins each retained carrier to its symbol row,
    function-symbol binding and exact text span, and the image-time replay
    of the retained container now accepts the carrier roster on its own
    evidence. `callback_custody` covers a two-slot registrar materializing
    both thunks into one object, plus foreign-identity, duplicate-symbol and
    substituted-roster rejections.
  - Give the common instruction pipeline callback ABI transport. Selection's
    `construction::build_plan`
    (`target-operations-to-selected-instructions/src/selection/construction/`)
    has no callback awareness and rejects the materialized registrar's extra
    private parameter slot with `SourceCustodyMismatch`; the retained roster
    the transport needs already lands on `TargetOperationPlan`.
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

  The authored contract is pinned through the generic program-local chain in
  `checked-trees-to-lowered-psi/tests/registered_callback_lifetime.rs`:
  `Registration [linear]` is the authority token, `domain Registration::Live`
  is established by `Registrar::register` and `Registrar::unregister`, and
  `Live::content` bounds each live registration at one `RegistrationSlot`
  (capacity counts registrations, not thunks). `unregister`'s `in Live`
  argument lowers a `ProgramLocalRootIntroductionSchema` that survives the
  codec and verifier, `VerifiedProgramLocalRootProducerCatalog` exposes the
  producer row, and an interpreted `Customer::run` entry forwards the
  host-installed live registration to the provider boundary on `unregister`.

  A claimed linear boundary result now crosses provider to program:
  `Registrar::register(registration) -> Registration in Registration::Live`
  runs in a Unit statement sequence, the returned registration lands on the
  caller's claim frontier under the result place, and a following
  `Registrar::unregister(registered)` settles that exact occurrence —
  pinned by `interpreted_register_unregister_round_trip_drives_the_ledger`,
  which observes both boundary effects in order through the checked,
  lowered, verified, codec and interpreted chain.

  Remaining work:

  - An authored registrar customer under the linked contract: success yields
    the linear registration holding the exact live-registration capacity
    occurrence, failure returns that capacity with no root, and unregister
    consumes the registration. Use ordinary linear custody; add no
    registration-specific checker rule. One grammar seam still blocks the
    full program: a routed domain cannot authorize a case payload
    (`Registered(registration: Registration in Live)` fails `cannot prove
    requires contract`), so the observed-rejection sum cannot yet be written.
    The installed-provider interpreter path also still gates boundary
    results to claim-free affine shapes (`supported_result` in
    terminal-interpreter `call_operations.rs`); the uninstalled effect path
    admits the claimed linear result.
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

  `tests/omega/pass/memory/bump_allocator_canary` checks that chain, a
  fallible request returning and consuming both `Attempt` cases, tail-ward
  `release` of the newest allocation while an older stays live (a third
  request consumes the restored residual and reset still waits for the
  remaining pair), a `BumpVec` reservation with one optional retained buffer,
  fallible growth from the tail, reset through either retained-slot case,
  full original-capacity reuse after returning both allocations without a
  runtime guard, and one resident place/read/retire; seven
  `fail/memory/bump_allocator_*` controls pin the rejections. Its header
  records the contract edges found so far — the release chain added one:
  reclaim is tail-ward only, because the boundary composes adjacent pairs and
  no gap form expresses an interior hole, so a `Vec<T>`-style owner can
  shrink this backing only from the newest end. This is source
  checking only: the fixture is on the `CHECKED_ONLY_PASS_CANARIES` roster,
  `Main::main` is empty, and `ExtentPartition`/`ResidentStorage` are
  fixture-local boundary traits with no conformer or selected provider.
  Split/merge conservation and resident establishment are asserted boundary
  laws, and nothing lowers or executes. The seventh control pins the
  tail-ward analogue of `reset_with_live_resident`:
  `fail/memory/bump_allocator_release_with_live_resident` rejects a `release`
  of a resident-bearing region at the `returned: Vacant` parameter, before
  its merge is reached. `source/library/alloc` holds only a `.gitkeep`; no
  expected-reject controls live there yet.

  Resume evidence: Linux x86-64, 2026-09-20, the resident-bearing release
  control: the focused command below accepts the fixture, and
  `OMEGA_FAIL_CANARY_FILTER=memory/bump_allocator_` with the
  `proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`
  selector rejects all seven controls. Both use `RUST_MIN_STACK=67108864`.
  Check the actual fixture with
  `OMEGA_PASS_CANARY_FILTER=memory/bump_allocator_canary
  cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail
  -E 'test(=entry_and_abi::pass_canary_coverage::pass_canaries_compile)'`.

  Remaining work:

  - Container. A `Vec<T>`-style owner needs elements, a length and
    content-preserving growth. Elements need the source
    `Initialize`/view/retire route of
    [placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement)
    (evaluated plan of `P` over `T`, Stable-supply admission), which
    `PLAN-LAID-VIEWS` owns. `source/library` declares neither that family nor
    `ResidentContentTransfer<P, T>`; the fixture's `ResidentStorage` is a
    stand-in to replace when the route exists. Growth retains backing but
    still has no elements: `merge` demands `Vacant` parts, so a
    buffer with a live resident cannot fold back for reset — pinned by
    `fail/memory/bump_allocator_grow_with_live_resident`. Element transfer
    waits on the same placed-access route as establishment.
  - Retained storage. Replace the finite slot with genuinely growable storage,
    explicit placed-storage indirection, and a ranking proof over that
    representation. A recursively owned inline `Retired` record has infinite
    layout; Omega does not implicitly box it. The finite fixture exercises
    retained backing and rejection, not a fixed-capacity substitute for Squalr.
    `checks/multiplicity/claim_outcomes/joins.rs` owns exact normal-exit claim
    alternatives and caller substitution; `linear_validation/recorded_events.rs`
    reconstructs custody from source before replaying those alternatives.
    Constructor checks retain selected-case facts and authored operand order.
    Keep inactive payloads distinct from consumed claims and preserve exact
    invocation identity through wrappers. Anonymous linear results without
    known claim evidence, recursive expansion, unresolved array extents and
    outcome-specific partition frontiers remain implementation limits.
    Terminal/native execution needs portable exit-alternative correspondence;
    demanded joined custody currently rejects explicitly in
    `checked-trees-to-lowered-psi/src/machine_lowering.rs`.
  - Partition theorems. No checked body splits one `Granted` extent into two;
    the fixture delegates that step to a boundary. Returning `Granted` custody
    from two consumed qualified inputs still rejects as ambiguous at a boundary,
    so merge takes one `Split` record. Ordinary checked wrappers can fold
    retained backing through that boundary; this is not a checked partition
    implementation. Replace the stand-in through
    `CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS`' invoked partition route.
    The normal-return length laws do not prove that arbitrary requests fit
    backing: that route must establish the geometric request bound and the
    strategy's counted-residual-to-tail invariant before calling a real provider.
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
  member admission, publication through the `lidt` edge and published-vector
  dispatch (`interrupt_table.rs`); table staging, the writer and the
  produced-image verdict are the authored layout plus the generic
  post-handoff writer and the consumer's validator. `core/interrupt.omg`
  declares the mask and acknowledgement obligations, and the instruction
  catalog in
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
    First authored leg landed: `interrupt_table_canary` now carries
    `cathedral::interrupt_roots`, which declares `CriticalStackPolicy`
    (`InterruptReturn` entry control, `X86Long64`, dedicated stack class,
    masked preemption), `FatalExceptionRoot: InterruptEntry +
    Calling<CriticalStackPolicy>`, the acknowledgement's opaque carrier, and
    the provider/mask conformances; its `build.omg` selects the
    representation through the shared program namespace (`use` there
    re-declares the module). The compiler-side test drives that member's
    candidate through `selected_external_root_provider_plan`, real entry
    and mask-guard claims, `ResolvedRootServiceReach::
    from_selected_provider_closure` and the replayed boundary realization
    (`Dedicated` stack class joined to the member's declared class), then
    `validate_external_root` → ledger `install` → `admit_interrupt_table_
    member` for the divide-error vector. The deriver-owned entry/exit stub
    contract now exists:
    `calling-conventions/src/stack_realizations/entry_exit_stub.rs` —
    `derive_x86_64_entry_exit_stub` binds a member's sealed
    `ValidatedX86_64InstalledHardwareEntryFacts` to its exact admitted
    boundary plan (commitment + fingerprint, InterruptReturn, X86Long64,
    matching stack disposition and per-context preemption) and derives
    per-context error-code normalization (hardware-pushed vs
    stub-synthesized), frame/save-area bytes, the saved-state footprint, the
    member-body envelope (exactly the plan's permitted transitive use) and
    the interrupt-return exit realization. Every member candidate's
    `machine_state` column in the compiler test — including the authored
    divide-error member — now carries that derived envelope, which
    `validate_external_root` ceiling-checks against the plan. The
    machine-emission half of the stub-byte leg now exists:
    `machine-emission/src/entry_exit_stub.rs` consumes
    `ValidatedX86_64DeriverStub` plus the exact admitted boundary plan
    (commitment + fingerprint match, uniform per-context error-code
    disposition, GPR-only save roster) and a positional
    `X86_64DeriverStubMemberCall` into emitted bytes — optional `cli` for a
    `Masked` ceiling on trap gates, the synthesized error-code word, the
    save-area pushes, an anchored 16-aligned frame staging member parameters
    (movabs immediates into declared register and stack locations), the
    member `call rel32` left unresolved behind `X86_64DeriverStubRelocation`,
    restores, and `iretq`. Emission self-replays through a grammar decoder;
    `validate_x86_64_deriver_entry_exit_stub` independently replays and
    `resolve_x86_64_deriver_stub_member_call` + its validator seal the field
    once image emission assigns section coordinates. Remaining on this leg:
    image-emission's artifact join still must place the bytes at
    `identity.entry_offset` and seal the relocation (image-emission is
    fenced); non-GPR saves now land — XMM roster entries stage through the
    contract's per-vector 16-byte reserve as a `sub rsp` slot area with
    `movdqu` stores, replayed upward on restore before the pops
    (`emit_x86_64_deriver_entry_exit_stub`); `Indirect` copies and
    non-GPR parameter destinations still reject as deferred seams — the
    copies additionally need the member-call-frame derivation in
    calling-conventions to reserve the copy area and pointer slot, which
    `outgoing_stack_end` currently skips — while stack pieces stage
    through exact-width 4/2/1-byte tail stores; the byte recipe
    lives in machine-emission until a second x86-64 deriver emission moves
    it into the ISA crate; the member's stack column can now bind through
    the emitted stub — `StackLocalEvidence::DeriverStubEntry` bound by
    `bind_installed_deriver_stub_entry_stack`
    (`external-roots/.../stack_demand.rs`) seals the member's terminal
    demand at its certified text offset, replays the sealed `call rel32`
    target equation and the exact installed entry bytes against the
    contract's installed-entry identity and admitted boundary plan, and
    folds the contract-carried `X86_64DeriverStub::peak_entry_overhead_bytes`
    (normalization + save area + ≤15 normalization slack + anchored
    member-call frame + the call's return slot) into the entry's ceiling,
    so `bind_x86_64_target_direct_entry_stack_realization` accepts the
    stub-entry summary and the dedicated-stack demand composes as
    hardware frame + stub overhead + member ceiling; the contract now
    carries `member_call_frame` (outgoing stack-argument extent plus the
    anchored reservation) as the single derivation emission and stack
    accounting share, and the save-area accounting reserves 16 bytes per
    vector register; producing `X86_64DeriverStubEntryEmission` rows during
    real admission still waits on the image-emission join above;
    provider-admitted resource columns and fuel/state receipts remain
    test-admitted shapes; the multi-resolution
    seam now exists — `with_installation_reach_resolutions` keys the roster
    by (requirement identity, provider plan report identity) and
    `installation_reach_resolution_for_plan`/`resolve_installation_reach_for_plan`
    bind a shared identity like `InterruptEntry::enter` through the root's own
    selected plan while unscoped lookups fail closed on ambiguity — so a
    second `InterruptEntry`-inheriting trait no longer trips roster
    uniqueness; remaining seams for the timer member: provider-planning's
    `derive_selected_installation_reach_resolutions` + nested-reach
    substitution (`installation_reach.rs`) still name requirements by
    identity alone, `ResolvedRootServiceReach::from_*` constructors
    (`root_validation.rs`) must adopt the plan-scoped resolvers with the
    candidate's plan identity, `receipt_binding.rs`'s admitted-evidence
    match may still reject a shared requirement that satisfies multiple
    granted plans, and the authored `TimerRoot` + acknowledge/record/wake
    member still needs declaring in `interrupt_table_canary` plus the
    compiler-side `admit_interrupt_table_member` drive; emitted-image
    machine-state evidence still needs the image-emission join above.
  - Descriptor table. The authored half now exists:
    `tests/omega/pass/memory/interrupt_table_canary` is a Cathedral-side
    package whose `InterruptGate` layout splits the entry-offset fields into
    the wired bit placements and whose `InterruptDescriptorTable` layout
    strides 33 gates at 16 bytes; the generic post-handoff writer
    materializes the authored image with the installed roots' sealed
    entries. The package also authors its declared membership and its
    validator: `cathedral::interrupt_validation` declares
    `TableMemberDeclaration` (vector, dedicated stack class, obligation
    flag and descriptor constants), `IstBinding` (the slot → dedicated
    class bindings the installed TSS must provide),
    `InterruptTableMembership::declare` producing the member set, and
    `DescriptorTableValidation::validate` — a `terminates by fuel` state
    machine that scans every vector's decoded gate record and raw slot
    bytes against the declared membership, joins each member's IST slot to
    its dedicated stack class through the installed TSS, and returns
    `TableVerdict::Accepted` or `Rejected { vector, reason }`. The
    compiler test
    (`layout_plans/interrupt_descriptor_tables.rs`) evaluates both
    authored machines through `evaluate_build_time_machine`, decodes the
    written bytes through the authored layout alone, hands the validator
    the verbatim member array, the decoded gates and the staged TSS's IST
    rows, and mints `EstablishedInterruptTable` only on `Accepted` — then
    drives ledger admission → issued carrier → checked `lidt` provider
    edge → `PublishedInterruptTable` end to end. The `InterruptTableProfile`
    and member plans now take all their semantic constants (vector, stack
    class, obligation, descriptor) from the authored declaration; only the
    sealed entry-stub and external-root identities remain fixture
    vocabulary. The compiler-side model has been retired: the ledger no
    longer derives a bespoke writer, stages the table image, or decodes
    produced gates — the authored layout, the generic post-handoff writer
    and `DescriptorTableValidation::validate` carry the produced table,
    and `EstablishedInterruptTable::from_consumer` mints the value only
    on `Accepted`. Member admission is now authored too:
    `cathedral::interrupt_validation` declares `MemberFacts` (the
    record-derived arrival facts — interrupt-return exit, dedicated-class
    arrival, acknowledgement-policy/parameter presence) and
    `MemberAdmissionVerdict` beside `TableMemberAdmission::admit`, which
    rules a member under its declared row and returns `Admitted` or
    `Rejected { vector, reason }`; the compiler test evaluates the machine
    verbatim and mints `InterruptTableMemberAdmission::from_consumer` only
    on `Admitted`, so `admit_interrupt_table_member` replays the binding
    (declared row plus the record's verbatim facts) instead of owning
    policy. What remains in `interrupt_table/` is custody
    plumbing: that admission binding, the
    publication carrier/receipt binding the established value to the
    table, and published-vector dispatch. Remaining on this leg: the
    established-record and publication types
    (`EstablishedInterruptTable`, `InterruptTablePublication*`, the
    admitted-member records) still hold declaration-shaped obligations
    the authored verdict now warrants — relocating them needs authored
    record/machine types for publication so the ledger
    becomes plumbing only; the ledger itself (installed roots, `lidt`
    contract, checked writer) stays compiler-owned.
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

  Flag: `external-roots/src/interrupts/interrupt_table/` carried a
  compiler-owned Rust model of the x86-64 IDT — its bespoke writer plan,
  staged image and `validate_written_descriptor_table` gate decoder are
  retired in favor of the authored layout plus `DescriptorTableValidation`
  verdict. What stays flagged: `InterruptTableGateDescriptor` still carries
  selector, gate kind, privilege and IST slot as Rust fields, and
  `InterruptTableProfile` still requires one distinct dedicated stack class
  per vector — declaration vocabulary the authored
  `TableMemberDeclaration`/`IstBinding` rows now supply, pending authored
  admission and publication record types. Hardware materialization assigns
  the validator and established value to the consumer package ("these
  policies are not compiler-owned types"), and interrupt obligations says
  Omega does not choose exception coverage or IST policy. The general
  mechanism is a source-authored table layout, the generic writer and a
  Cathedral validator; the compiler keeps root records, the IST-to-stack-
  class join that stack selection derives, and the `lidt` contract.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Finish
  [installation-bound reach](wiki/spec/build/external_roots.md#installation-bound-reach)
  for component contracts and for the completion route an opaque carrier owns.
  Concrete reach and conservative bounds stay separate; selected provider
  execution and token era, not row equality, authorize invocation. Parsing and
  checking of `reaches <= Bound`, the package-review fence on ordinary public
  callables, selected-row resolution
  (`provider-planning/src/provider_planning/installation_reach.rs`) and
  root-closure substitution (`external-roots/src/root_entry/root_validation.rs`)
  exist. Closure substitution now substitutes a nested bounded row the same
  closure selected: the realization's resolved row is
  `RealizedMachineContractEnvelope::concrete_service_reach` extended by each
  nested requirement's resolved row, iterated to a fixpoint so substituted
  rows can themselves satisfy still-pending rows. A nested requirement the
  closure never selected still rejects with the same unresolved-row
  diagnostic, so "substitutes every bounded row and rejects unresolved rows"
  ([interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime))
  holds for both sides of the selection boundary. Authored-source pins:
  `opaque_boundaries.rs::selected_realization_substitutes_a_selected_installation_bound_row`
  (positive) and `selected_realization_with_an_unresolved_installation_bound_row_rejects`
  (negative).

  Remaining work:

  - Completion route. `InterruptAcknowledgement::complete` in
    `source/library/core/interrupt.omg` still declares `reaches PortIo`, and
    `compiler/tests/calling_policy_plans/opaque_boundaries.rs` pins it as not
    installation-bound. Migrating the declaration is a verified one-line
    change that turns that suite red, because every realization of the
    installation-bound entry settles the linear acknowledgement and retains
    the nested bounded row, and no satisfier for `complete` can be authored
    while a checked body cannot discharge the linear receiver, so the route
    now waits on receiver-bearing requirement selection, which
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS** owns and which
    [interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime)
    names for this selection and lineage integration. Land the declaration
    with the satisfier, not before it. The rejection is
    driven from authored source by
    `opaque_boundaries.rs::selected_realization_with_an_unresolved_installation_bound_row_rejects`,
    so losing the fence early is a red test.
    [Interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime)
    requires a bounded row beneath `MachineControl + PortIo`, and
    `InstalledInterruptCompletionRoute` in `external-roots` rejects a completion
    requirement that has no installed resolution. Provider-planning tests
    resolve the bounded spelling only for a test-local `[copy]` lookalike.
    Migrate the shipped requirement and join its invocation to the carrier's
    exact provider execution, policy and token lineage; an x2APIC provider must
    not receive `PortIo`. Receiver-bearing requirement selection is
    `TOP-LEVEL-BOUNDARY-REQUIREMENTS`' work.

  Acceptance: from the shipped core requirement, PIC completion resolves to
  `PortIo` and LAPIC/x2APIC completion to `MachineControl` through checked
  source, Terminal Psi and the installed route. Cross-provider settlement with
  equal rows, a replayed token or era, an `Independent` component exporting an
  unresolved row, and final admission with any unresolved row reject.

  `COMPONENT-SUBSTRATE` owns the component description these rows are
  published from; this item owns the reach bound itself and stays separate
  while the completion route above remains open.

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
  checked-only roster: source lowering does not yet produce the restoration
  operations specified by
  [Terminal ownership](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).
  The product parser still avoids the pattern by borrowing the lexer's stream.

  Remaining work:

  - Lowering: transport the exact place/loan restoration debt through
    `checked-trees-to-lowered-psi` using the ordinary partial-move, store and
    control-flow relationships, without replacing the caller's storage by a
    staged copy. The Terminal spelling it must produce now exists:
    `MoveStructuralField`/`StoreStructuralField` (codec tags 77/78).
  - Terminal (landed): `terminal-verifier/src/validation/borrowed_windows.rs`
    reconstructs the restoration debt from operations, loan authority and
    control flow — `MoveStructuralField` opens a per-root hole ledger keyed by
    canonical field path, `StoreStructuralField` discharges an exactly-typed
    whole owned place into it, stale reads/edge arguments/non-crash exits
    reject, and crash exits retain no obligation. `terminal-interpreter`
    replays the consuming transform with caller-visible subtree contents and
    exact-once custody across fuel splits
    (`cargo nextest run -p terminal-verifier --test suite borrowed_storage_windows`,
    `-p terminal-codec --test suite canonical::borrowed_storage_windows`,
    `-p terminal-interpreter --test unit borrowed_storage_windows`;
    linux-x86_64). Omega lowering still rejects both operations via
    `LoweringError::UnsupportedBorrowedStorageWindow` — native realization
    remains open.
  - Checker (landed the reconvergence-agreement leg): a move evaluated inside
    a `match` arm opens a pending debt on that arm's own edge and commits one
    joined hole at the match's join point iff every reachable arm resolves the
    same absent places — nested agreement lifts into the enclosing arm, call
    argument moves attribute to the evaluating arm, dead arms owe nothing, and
    per-edge holes fence the arm's own reads and suspending/boundary calls.
    Arm disagreement, repeated extraction on one edge, moves in
    short-circuit-conditional positions, matches on conditional or transition
    edges, and an open window at the join all keep the plain rejection.
    Match-result custody dispatch (`match_dispatch.rs`) admits borrowed-receiver
    arm values through the transfer check and the result custody join so the
    agreement rule can witness. Open-window checking consumes the existing
    per-call suspension/blocking summaries, including initializer, assignment,
    aggregate and call-argument positions; replacement evaluation is checked
    before the repair store. Quiet checked bodies remain usable even with an
    authored may-ceiling. Nonblocking boundary/service calls, including
    empty-reach boundaries and named/spelled boundary operators hidden behind
    ordinary wrappers, require repair first. The fence follows retained call
    topology and scheduled operator invocations; exact no-service builtins and
    quiet recursive helpers remain usable. Regressions:
    `typed-trees-to-checked-trees --lib` filtered by `borrowed_restoration`
    (55/55, linux-x86_64), `checked-interpreter --test suite` with the same
    filter (4/4), and compiler fail canary
    `ownership/borrowed_storage_boundary_call` (macOS ARM64 — host
    unavailable). Moves on transition edges still reject — they cannot be
    repaired before the edge leaves — and conditional extraction outside the
    arm-agreement shape keeps the rejection rather than treating one path as
    unconditional. Contained-loan transport and recoverable-failure paths have
    no regression.

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
    boundary. Owned indexed leaves share that boundary: the admission gate's
    projected-child rule now follows `constant_integer_value`, so
    `items[0 + 1]` joins and checks exactly like `items[1]` — the
    `match_dispatch` admission tests and the checker's
    `value_dispatch::constant_index_projection` pin the join and the kept
    rejection of indexes without a fixed ordinal, while
    `folded_index_projection` (`tests/value_dispatch/owned_results`) pins both
    spellings stopping at the same scalar-control-plan gap. On 2026-09-19
    (Linux x86-64, base ac1efde2f7) the scoped runs are
    `cargo nextest run -p validation --lib
    value_custody::expression_types::match_dispatch`,
    `-p typed-trees-to-checked-trees --lib value_dispatch`, and
    `-p checked-trees-to-lowered-psi --test suite value_dispatch`, all
    passing; leaf emission still must transport the `FixedIndex` path to
    close it.
  - Claim-bearing bodies do not lower. The checker joins a whole affine root
    with linear children by naming each frontier claim, but lowering stops at
    "machine has no source-independent checked scalar control plan" for a
    machine whose body holds claim-bearing custody
    (`tests/value_dispatch/owned_results/linear_child_carriers.rs` pins it).
  - Borrowed-result consumers. A `&Payload` selection forwards its joined
    place to a call and executes, and an established `&Payload` join lowers
    even unread. A direct `let view: &T = &place` outside a match now
    establishes the shared-borrow carrier directly and executes
    (`established_reference_local_call_forwards_the_direct_borrow`,
    b7d932c341). The primitive consumer lane is admitted: a `&u64` callee
    body plans a source-independent `PrimitiveScalarRead`, a `&u64` formal
    forwards the exact entry place end to end, and `read(view)` retains an
    established `&u64` view as a whole `PrimitiveScalar` `SharedBorrow`
    argument (2e4ee8f08b). Primitive joins now verify and interpret
    (8657ed47f7, 1a17b2938d); do not reopen the retired
    `InvalidBlockStructuralParameter` gap. A focused macOS ARM64 probe at
    a10fbe20d0 using `PRIMITIVE_CALL_SOURCE` from
    `checked-trees-to-lowered-psi/tests/value_dispatch/borrowed_results.rs`
    interpreted both arms as 1/4, but native lowering rejected
    `UnsupportedStructuralBlockParameters` (machine 1, block 12).
    Next acceptance is native execution of both projected-field arms.
    `terminal-psi-to-abstract-operations/src/lowering/block_bindings.rs`
    only accepts byte-sequence borrowed block parameters, and
    `abstract-operations-to-target-operations/src/lowering/control_flow/transfers.rs`
    additionally requires empty argument paths and equal whole-source types.
    Coordinate the latter with WRITE-ONLY-BORROW's shared lowering scope;
    removing only the first gate cannot close the customer.
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
  - Result types. Selected-operator and named operator `-> T` results now
    instantiate to the exact operand reference the first bare-`T` parameter
    position bound, at every expression owner (state, parameter telescope,
    domain predicate); composite returns (`Pair<T>`), dependent predicates
    (`-> u64 [0..=left]`), unbound result parameters and ambiguous selections
    remain unresolved rather than fabricating identity. A bound owned result
    (`T` := `Payload`) now enters the same branch-custody join as a declared
    `-> Payload`: both `match` arms are judged instead of the generic arm
    slipping through untyped — `result_type::tests` and the checker's
    `value_dispatch::semantic_results` pin admitted scalar joins and the
    bound/unbound owned split. Still needed: instantiated shells for composite
    and constrained results, and exact static arguments for indexed
    predicate/theorem and package membership applications. Qualified
    callable-entry signatures, predicate/routed membership and erasure require
    real transport rather than payload-only projection; input predicates are
    not arithmetic result facts. **OPERATOR-MACHINE-SUPPLY** owns declared
    operator execution; **CRASH-CONTRACT** owns crash-qualified equality;
    numeric landing is in **STATE-LOCAL-VALUE-FRONTIER**.

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
  and since `9913f44891` the duplicate check spans both introducers: every
  spelling-bearing `operator` definition — root, `boundary operator`/`boundary
  machine` slots, and domain-homed members — joins the machine set's
  token/owner/shape space, alpha-normalized by each binder's first-occurrence
  position so `combine<T,U>(T,U)` and `combine<A,B>(B,A)` collide at the
  second declaration instead of surfacing as use-site ambiguity. Use sites select
  by operand type, and the checked stage rewrites each resolved binary or
  indexed use into an ordinary call on the declaration's entry state
  (`typed-trees-to-checked-trees/src/operators/token_bound_machine_calls.rs`),
  normalizing `[..]` endpoints first: an omitted start supplies literal zero
  to an integer start parameter and an inclusive end supplies its `end + 1`
  form gated on an integer end operand, while open-ended uses still reject.
  Token-bearing `boundary machine` signatures lower to the existing boundary
  slot, bare bodyless signatures are admitted only by exact catalog custody,
  and a bodyless nonboundary machine rejects at its declaration. That does not
  establish the contract: the introducer still parses, 208 `operator`
  declarations remain in `.omg` sources, only binary and indexed positions
  have body supply, both pass canaries are checked-only, and the three
  settled rules below are not enforced.

  Remaining work:

  - [Call preconditions](wiki/spec/language/machines.md#call-preconditions).
    Finish mathematical term formation beyond concrete machine/state contracts.
    `validation/src/proof_contracts/contract_entailment/specification_calls.rs`
    checks their selected concrete calls before fact intake, rejects circular
    requirement dependencies, and uses positive constructor or exact arithmetic
    evidence. `proofs/contract_call_missing_premise` rejects a contract-only
    reflexive call; `mathematical_call_premises` accepts its premise-bearing twin
    and imports core cancellation and `sub_le`. Ordinary statement calls retain
    the exact-site `call_requirements.rs` judgment and separate induction descent.
    Remaining owners: abstract signatures, domain/default-domain predicates,
    explicit static callable/evidence substitution, conditional/induction and
    general postcondition transport of case-membership guarantees, and
    nominal/propositional premises.
    `proofs/case_call_premises` covers concrete and symbolic tag premises,
    completed concrete-call guarantees, matching and named-state forwarding;
    `case_call_wrong_subject` and `case_citation_wrong_result` reject different
    subjects. Concrete citations use exact target/result bindings and discharge
    their own premises before later calls consume a tag. Extending this to
    abstract callable contracts or induction must preserve that ordering,
    independently establish descent, and never import private strengthening.
    Tag predicates retain exact classifier identities without field equations.
    Structural receiver-call premises and projections outside denotational call
    admission still fail closed; receiver-call opacity cannot substitute arguments. Constructor normalization
    still needs unsupported float/array
    leaves and established qualified defaults; supported Boolean/integer and
    nested data fields must keep complete value rosters, separate from case
    classifiers. Extend formation only with exact selected identity and complete
    substitution across shared consumers; never use unrefuted as proved.
    `opaque_receiver_call_cannot_hide_a_changed_requirement_argument` must retain
    wrong-argument rejection beside an accepted exact-argument twin.
    Finish Terminal transport of call-containing scalar requirements. Reuse
    `contract_application_terms::runtime_body_calls_execute_with_checked_premises`:
    its Boolean `caller(value)` invokes `restricted(saved, value)` under
    `observe(left) == observe(right)`. Checked execution observes both results,
    but `TerminalProductionRequest::new(&checked, "caller").produce_artifact()`
    rejects with `scalar contract contains an unsupported clause` on macOS
    AArch64. The checked contract's missing predicate reaches
    `checked-trees-to-lowered-psi/src/scalar_graph/scalar_contracts.rs::covered_requires`.
    Acceptance is the same program's canonical reload, independent verification,
    interpretation and native execution without dropping its requirement.
    Runtime transport beyond builtin Boolean/integer equality still needs
    citation and induction evidence; nested operands need actual premise
    checking, not a separate lowering refusal counted as coverage.
    These are implementation dependencies, not unanswered language design. Then migrate
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
    binds attached `[]` uses and closed `[..]` uses (`start..end`, `..end`,
    `start..=end`, `..=end`) under the spec's range normalization, but
    match-arm equality selections still reject, open-ended `start..`/`..`
    uses still reject because the omitted endpoint is the collection's
    length -- which a declared `[..]` telescope cannot form -- and a token
    use inside a build machine fails closed. Indexing uses
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
    and trait and domain bodies accept a token only through it. The
    token-binding law already spans both forms (`9913f44891`), so what remains
    is the source migration plus erasing the `operator` representation. 171 of
    the 208
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
  (`tests/module_namespace_residuals.rs` in that crate). The remaining
  operator/case-fact forms mostly assert only resolution success;
  their selected declarations still need following through typing, checking
  and Terminal.

  Constant scalar rejection now carries its selected arithmetic policy: the
  scalar constant evaluator reports an overflowing, dividing-by-zero or
  bad-shift operation as e.g. "Exact integer constant operation overflows,
  divides by zero, or has an invalid shift count" — the policy is part of the
  retained selected-operation evidence, so `module_machine_indices`
  (`comparisons::`, `value_dispatch::`, `computed_declarations::`) and
  `module_domain_indices` controls assert the named policy, and anonymous
  Match and comparison paths keep their all-arm obligations.

  Remaining work:

  - (new-scope) Enforce predicate-domain establishment on local initializers;
    selecting the right domain/carrier is not proof of membership. On unchanged
    `4dc78c119be9` (macOS ARM64), ordinary `compile_to_checked` incorrectly accepts
    `data Choice [copy] { case Empty; case Some(value: u32); }`
    with `domain Choice::NonEmpty requires self in Choice::Some;` and
    `machine read() -> u32 { let value: Choice in Choice::NonEmpty = Choice::Empty; value.value }`.
    `typed-trees-to-checked-trees/src/checks/contracts/writes.rs` checks local
    initializer qualification only when `domain_requires_provenance` is true;
    predicate-only annotations can supply their own unproved facts. Require the
    initializer's obligations before admitting destination facts, retaining
    valid case construction and rejecting `Empty`, stale facts and wrong owners.
    The foreign qualified variant has the same source-checking gap. Runtime
    qualified-record/case helpers also stop at the missing checked scalar control
    plan in Terminal production; neither gap is closed by the native constant
    helper control `scalar_case_results::package_membership::foreign_domain_constant_helpers_execute_after_source_removal`.
    `module_machine_indices::domain_carriers` checks exact carrier selection,
    retained formal predicates and cross-package owner rejection, not proof
    that a caller establishes those predicates.
  - Unmanaged source maps still have no portable package commitment for
    equal module/domain paths across distinct roots. Keep the independent
    collision rejection in `validation/src/proof_contracts/domains.rs` until
    source acquisition supplies exact ownership; do not substitute host paths
    or source-order numbers for package identity. Managed package/scope keys
    already pass through indexed families, cast selection and Terminal.
    Runtime field reads from a qualified `Holder<T>` local still need a
    source-independent checked scalar control plan in **TR3-TR8** and
    **STATE-LOCAL-VALUE-FRONTIER**. The
    `module_machine_indices::indexed_domains::generic_carrier_qualified_fields_keep_their_owner_after_specialization`
    regression retains that checked-source customer alongside the source-free
    constant-record route; do not treat constant projection as runtime storage.
  - Carry exact lexical/package selection for remaining generic type-scoped
    constant attachment heads, indexed domain constraints, operator homes and
    declared-domain case facts through typed and checked trees and Terminal
    artifacts, with per-use exposure under
    specialization and owner-local imports, and add same-leaf, private and
    transitive-exposure controls for each.
  - Complete declaration evaluation, including unused initializers:
    specialized provider applications, target-dependent declarations, and
    selected floating operations/NaN identities,
    which need determined bits through ordinary typed floating provider
    applications. Record-carried constrained
    constants bind `self` through their scalar-decodable fields, so closed
    `self.<field>` predicates discharge at declaration site
    (`module_namespace_residuals::constrained_record_const_discharges_field_domain_facts`
    holds the discharge, refutation and unprojectable-projection controls).
    Array-carried constrained constants bind `self` through their
    scalar-decodable elements, so closed `self[<index>]` membership and
    comparison facts discharge the same way
    (`module_namespace_residuals::constrained_array_const_discharges_element_domain_facts`
    holds the discharge, refutation, out-of-bounds-index and
    non-scalar-element controls). Variant-carried constrained
    constants bind `self` through the selected case's scalar-decodable
    payload fields, so closed `self.<field>` predicates discharge against
    the literal's case
    (`module_namespace_residuals::constrained_variant_const_discharges_case_payload_domain_facts`
    holds the discharge, refutation, unselected-case and
    non-scalar-payload controls). Constrained constants
    still fence whole-aggregate `self` operands, fields and elements
    without a scalar leaf, open applications, non-domain
    constraints and unprovable facts. Carrier-polymorphic scalar constants
    discharge carrier-independent closed predicates; carrier-property bounds
    and operations on abstract `self` still need typed application evidence.
    Routed constraints still require establishment evidence. Alias-owned carrier
    bounds and index tuples need checked application evidence retained across
    expansion; compiler-owned alias atoms need their typed evidence. Neither
    follows from empty predicate replay (`generic_data/const_evaluation/facts.rs`).
    Reuse typed expression evaluation, not untyped fact folding that erases
    operand widths and selected-operation custody.
  - Extend concrete failure discharge in `const_initializers/invocations.rs`
    beyond ordinary scalar invocations, lossless fixed-integer widening and
    proven-index projections through pristine constructed locals.
    Preserve `machine_initializers::computed_table_selector_discharge_reaches_source_free_native_execution`
    and its failure controls: computed indices reuse checked scalar operations
    and facts at the original projection, including saved rows and later selector
    mutation. Partial/value-changing casts, indices without an exact live value,
    call-produced values and other origins the concrete probe cannot decide still widen
    conservatively; they need checked evidence
    (`typed-trees-to-checked-trees/src/facts/crash_entry_values.rs`, shared
    with **CRASH-CONTRACT**), not successful interpretation or provider-body
    inspection. Beyond total fixed-integer widening, argument conversions and
    undirected signed disequality still need live premise transport through the
    ordinary call checker in `typed-trees-to-checked-trees/src/checks/contracts/`.
    Keep `machine_initializers::widened_helper_preconditions_reach_native_execution_after_source_removal`
    and `widened_runtime_call_preconditions_execute_after_source_removal` as the
    source-free interpreter/native widening controls; conversion checking must
    not reinterpret wrapping operand computations or resurrect invalidated facts.
    Preserve `module_machine_indices::machine_initializers::widened_constant_helper_executes_natively_after_source_removal`,
    `indexed_constant_helper_discharge_reaches_source_free_execution`, and
    `constant_helper_preconditions_reach_native_execution_after_source_removal`,
    including nested array aliases and zero, wrapping-to-zero, mutated-operand,
    unselected-constructor-crash and false-precondition rejection controls.
  - `const_generic_expressions/value/match_dispatch.rs` proves nonconstant
    divisor integrality beyond singleton sign intervals, nonzero proofs
    through retained lattice gaps, and per-operand fractional warnings for
    independent dispatch operands through rational bounds, not arm
    enumeration or evaluation of skipped subjects (`match_tests.rs`
    `nonconstant_divisor_lattices_prove_all_arm_integrality`,
    `exact_operand_points_discharge_divisors_beyond_lattice_gaps` and
    `independent_dispatch_operands_retain_each_exact_fractional_warning`
    cover them). Correlated result facts are deliberately not reconstructed
    from branch selection, so no residual dispatch obligation remains.
    Preserve exact selected operators (**OPERATOR-MACHINE-SUPPLY**) and
    proof arguments.

  General array-value execution (dynamic selectors, borrowed projections and
  slices, array-producing cycles, boundary and indirect results) is a
  dependency owned by **STATE-LOCAL-VALUE-FRONTIER** and Omega's
  `abstract-operations-to-target-operations` aggregate-result lowering
  (`lowering/control_flow/aggregate_results.rs`), not a namespace fallback. A
  selected aggregate home cannot stand for an unevaluated value.

  Acceptance: `compiler --test module_machine_indices` (`nominal::`,
  `value_dispatch::`, `constant_attachments::`, source-free `machine_initializers::`),
  `terminal-psi-to-abstract-operations --test scalar_array_construction` and
  `omega-native-differential-test --test scalar_array_results` exercise exact
  source selection through independent artifacts and matching-host execution.
  `omega-native-differential-test --test scalar_case_results floating_constants`
  additionally checks exact floating helper results through source-free
  publication and matching-host execution, including signed zero.
  Its `generic_constants::nested_generic_record_tables_execute_after_source_removal`
  control preserves mixed field/index projections over closed record tables;
  this does not establish dynamic indexing or runtime aggregate storage.
  `compiler --test constant_float_tables` covers exact floating array and
  record-table projections, transitive copies, source-free four-target
  publication and matching-host execution; floating generic indices and
  invalid unused initializers must still reject.
  Preserve the `qualified_declarations`, `qualified_constants`,
  `match_constant_indices`, `nominal_constant_bodies` and
  `module_array_constant_indices` customers. Under
  [file-local imports](wiki/spec/language/modules.md#import-scope-and-exposure),
  same-leaf competitors, private/transitive exposure, invalid unused
  initializers and unproved indexing reject. Keep
  `fail/modules/{runtime_aggregate_index,runtime_fixed_array_index}` until
  their materialization obligations are met.

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
  from Terminal artifacts, with native scenarios on macOS ARM64. None of
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
    fixture. Domain-index qualifications forward through call-bound index
    substitution: a call's const-position binder instantiation (`literal`,
    `const` binder, or runtime `Value` binder) rewrites the callee's declared
    `Coordinate<I>` membership to the caller's bound index in both index
    compatibility and requires-fact instantiation, so
    `machine outer<N: u32>(v: i64 in Coordinate<N>) { relay<N>(v) }` now
    checks while `relay<M>(v)` still rejects.
    Ordered and equality scalar guards establish result and local qualifications
    through the existing write-invalidated arithmetic environment, with independent
    Terminal replay and macOS ARM64 native execution in
    `runtime_value_generics::runtime_bound_result_qualification_follows_the_dominating_guard`.
    `equal_runtime_indices_preserve_the_guarded_result_subject` also checks
    transport between distinct bound subjects under true `==` and false `!=`,
    with a shared dynamic body; writes to either subject retire the equality.
    Remaining: guard-derived parameter qualifications. A stale guard followed
    by `limit = 0` before
    `bounded_result<limit>(value)` still passes `Check`, but artifact production
    rejects with `OperationProofUnavailable`; move that rejection to the
    exact call's source contract check without weakening the artifact gate.
    The regression is `runtime_bound_stale_call_guard_rejects_publication`;
    owning source check is `typed-trees-to-checked-trees/src/checks/contracts/calls.rs`.
  - Parse and check value binders on data declarations:
    `data Index<Limit: u32> { value: u32 [0..Limit]; }` owes the range at
    construction, erases a proof-only index, and keeps an executable index as
    ordinary data, an argument or an existing descriptor field.
  - Module-owned forms, once **MODULE-NAMESPACE-RESOLUTION** supplies exact
    lexical selection. No source-spelling fallback, and no runtime value used
    as a static cache key.
  - Finish the native call/storage routes using ordinary operations, not
    generic-specific substitutes. Keep
    `scalar_case_results::record_reads::call_requirements` as the working
    indexed-field receiver-call control: scalar requirements preserve their
    ordered proof custody without relaxing ownership `entry_claims`.
    A structural subject over a record local beside a provider receiver gets
    no checked Unit plan
    (**STATE-LOCAL-VALUE-FRONTIER**). Native receivers spell
    `console: Service<Console>`; validity is intrinsic to the closed carrier,
    not an authored `Bound` qualification. A bare `Console` field stops in
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
  structural `where Binder == <type>` equations that recover omitted const or
  type binders in either direction. Data and closed explicit machine applications
  use the same matcher
  (`syntax-trees-to-symbol-resolved-trees/src/preparation/type_equations.rs`).
  These consumers share one canonical-range admission: declared-range
  call inference runs before the const-call probe, evaluates each retained
  call endpoint through the shared endpoint evaluator, and writes the folded
  decimal literal back into canonical syntax, so `u64[0..=limit()]` and
  `u64[0..limit() + 1]` arguments on data fields and machine parameters
  select the same `u64[0..=256]` instance and recover its `Capacity`;
  conflicting explicit binders and calls that keep no canonical range still
  reject. Data equations also decompose nested fixed arrays and construct an
  omitted backing type from bound element/extent arguments
  (`type_equations/type_structure.rs`). Closed leaves use `ClosedArgumentIdentity`.
  The `compiler --test array_type_equations` integration target exercises inferred
  and explicit identity through checking and recovered counts through source-free
  Terminal execution, including reverse construction (macOS ARM64). Templates
  retain array equations in syntax; unspecialized applications cannot discard
  those obligations. Closed explicit calls to free and attached machines recover trailing
  type/const arguments from ranges and fixed arrays, construct omitted types,
  and discharge complete supplied tuples. Actual static type arguments survive
  resolution, caller-scope validation, specialization and normalized identity.
  `compiler --test machine_type_equations` exercises source-free Terminal
  execution, receiver mutation and receiver-free attached scalar calls through
  four-target native publication and macOS ARM64 execution, and rejection of
  conflicting tuples, open/noncall selections,
  implicit cleanup, kind mixtures, forwarding, lifetime errors and unmet named
  conformances. Open or retained applications lacking
  their equation syntax reject explicitly. Provisional normalization retains
  pending equations; transitive build-time invocation cannot execute them.

  Lifetime-free declared applications with type and builtin integer/Boolean const
  positions share that traversal with arrays, slices and anonymous references,
  including reverse construction. Machine completion retains selected declaration identity,
  normalizes the completed type roots through ordinary data synthesis, and reuses
  existing instances; matching never discharges a constructor's own constraints.
  `compiler --test application_type_equations` exercises source-free Terminal
  and macOS ARM64 native execution, data/machine inference, and rejection of
  conflicting tuples, false constructor facts, hidden caller binders, and skipped
  nested applications.

  Remaining work:

  - Static type equality in Psi type-role resolution and checking: `where`
    disjunctions such as `T == u16 || T == u32` on machines and requirements,
    rejection of type/value mixtures, the unspecialized body checked under
    every admitted alternative, and a static branch's equality fact dropped
    at its join. No machine-level customer or control exists.
  - Extend constructor matching beyond its lifetime-free type/integer/Boolean-const
    cohort, preserving exact named lifetime and other const-index identities.
    Named lifetimes lack selected lexical binder identity at this pre-resolution
    matcher; neither spelling equality nor erased layout identity can supply it.
    Keep `application_type_equations::reference_equations_` as the anonymous
    reference/slice native control, including value-kind and occurs rejection.
    Write-only static type arguments retain their independent
    **WRITE-ONLY-BORROW** admission dependency.
    Combine machine
    equations with existing argument/result inference rather than requiring a
    closed explicit prefix; retain obligations through generic forwarding and
    retained compilation extensions. Open and late-selected result receiver
    applications, operator supplies,
    conformance realizations, machine/evidence/value binders, and staged
    equation-bearing endpoint calls need their complete admission contexts.
    Result receivers selected during typing reject equation-bearing methods:
    they missed resolution's exact-target equation discharge, even with a full
    explicit tuple. Keep that fence until the obligation follows late selection.
    An open endpoint binds as one whole expression (`0..=N` may bind
    `Limit + 1`); solving `N * 2 == 256` stays outside.
  - Endpoint invocation admission for nominal parameters and Trapping positions,
    trait-operator owners (owner-sensitive typed operations), and applications
    that need inference in data-field types or machine/evidence binders.
    Closed named and structural type/const applications retain their declaration's
    type/lifetime scope in machine and data-field types; ordinary argument
    inference closes machine-owned endpoint tuples without overriding explicit
    type choices. `compiler --test bounded_slice_selectors` exercises a
    computed bound driving declared-range inference through source-free Terminal
    and native execution, with out-of-range argument and store rejection.
    Wrapping/Saturating argument and result transport uses the shared scalar
    evaluator, preserving policy through calls, arithmetic, Match and folded
    endpoint literals. Its `policy_endpoint_values` controls cover native
    inferred capacities and reject implicit policy changes or invalid initial
    landings. Saturating left shift still needs its shared integer primitive;
    partial narrowing keeps its independent conversion-evidence requirement.
    Nested computed bounds retain their type obligations until folding and
    specialization complete, including through nongeneric helpers. Extend the
    existing whole-expression scalar evaluator and shared admission plan;
    retain exact computed-result types and original selection custody. Do not
    add an arithmetic evaluator, infer layout from flow bounds, or use the i64
    compatibility interval in `validation` as type identity.
    A range combined with Wrapping/Saturating also still hits ordinary
    declaration checking in `validation/src/proof_contracts/arithmetic_domains/range_constraints.rs`;
    endpoint admission alone cannot remove that storage/proof boundary.
    Trapping parameters additionally depend on checked failure evidence under
    **ARITHMETIC-POLICY-REALIZATION**. Do not merely add Trapping to
    `range_endpoints/integer_type.rs`: at `59402436c4`, an experimental
    `endpoint(value: u8 in Trapping) -> u64 { (value + 2) as u64 }`
    folds `endpoint(5)`, but `endpoint(255)` reaches interpreter overflow
    instead of rejecting before execution. The shared build-time admission
    floor has no failure-discharge axis, and a concrete checked call's crash
    summary still misses that unsupported arithmetic. Reuse the initializer
    invocation proof route once it covers the primitive; require an admission
    rejection for overflow and source-free native execution of the safe bound.
  - Runtime `Value` binders in data equations, which reject today as not
    statically recoverable. They depend on RUNTIME-VALUE-GENERICS; static
    matching proceeds first.
  - One normalizer for source equality, generic matching, canonical type
    identity, static evaluation/layout and artifact readers. Synthesized
    instances deduplicate by `ClosedArgumentIdentity`; their display name
    stays diagnostic-only.

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

  Borrowed-local mutation calls
  (`declared_range_inference_local_effects_retain_pending_terminal_boundaries`)
  still follow STATE-LOCAL-VALUE-FRONTIER. Do not add generic-specific storage
  plans to resolve that independent lowering boundary.

  The constructed-direction customer in
  `generics/omitted_data_binder_range_equation` checks and evaluates ordinary
  `Bytes<256>` field values and the recovered endpoint; explicit conflicting
  length types and out-of-range initializers reject. Its nested backing array
  still blocks Terminal execution. On macOS ARM64 at `f6adb89fb3` plus this
  fixture, `omega inspect-terminal --machine constructed_value
  tests/omega/pass/generics/omitted_data_binder_range_equation/main.omg`
  with `RUST_MIN_STACK=67108864` reports no checked scalar control plan.
  **STATE-LOCAL-VALUE-FRONTIER** owns nested fixed-array record establishment:
  `execution/unit/mod.rs::scalar_graph_record_shapes` excludes the array,
  and `scalar_graph/scalar_graph_lowering/structural_values.rs` only establishes
  complete record fields. Preserve the backing array and connect its ordinary
  zero initialization before claiming Terminal or native execution; deleting
  storage or only widening the shape filter cannot deliver this customer.

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
  whole family. Correlated multi-binder rosters are covered end to end:
  `where W == 16 && L == 4 || W == 32 && L == 8` generates both authored
  tuples, a call spelling `(16, 4)` selects exactly that row, an
  uncorrelated `(16, 8)` rejects, and a fabricated boundary demand naming an
  open roster now rejects rather than silently generating nothing.
  `family_tuple` is an exact join coordinate in Terminal rows,
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
    with `tests/omega` pass, fail and run canaries. This depends on
    **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**, not merely tuple tests:
    the existing `REBOUND_FAMILY_DYNAMIC_INTEGER_SOURCE` pattern publishes
    verified Terminal, but native staging at `59402436c4` rejects
    `CallDynamicScalar` with `Selection(Legalization(UnsupportedScalarOperation))`.
    The common-graph legalizer admits no descriptor-call family. Complete its
    ordinary indirect-call, table and replay route; the native test must observe
    distinct tuple results and the rebound selected instance, not discard a
    result from a provider that ignores its width.

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

- **DOMAIN-ISSUER-ROUTES.** Finish independent Terminal qualification evidence
  for the [requirement and exact-machine routes](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes),
  including [private issuer catalogs](wiki/spec/resources/authority.md#private-issuer-routes).
  Source checking introduces `AuthorizedRouteEstablishment` in
  `typed-trees-to-checked-trees/src/facts/qualification_evidence.rs`; trace that
  exact subject/invocation through `checked-trees-to-lowered-psi` and the
  Terminal producer/verifier. Source checking and inert package-row recovery
  alone do not close artifact acceptance.

  The source-to-package boundary is covered by `cargo nextest run -p
  package-evidence --test suite -E 'test(public_domains) | test(module_namespaces)'`
  and compiler `package_compilation_inputs` private-catalog controls, exercised
  on macOS ARM64 against main `272a371880`, with `RUST_MIN_STACK=67108864`.
  Private free/attached machine
  and requirement catalogs retain owner and declaration identity; wrappers
  forward issued values, outside private calls/conformances reject, and
  private admission claims remain visible without becoming issuance receipts.
  Module-qualified machine identities
  distinguish same-leaf declarations within one package; substituted module,
  package or callable route rows fail fresh local reconstruction. Preserve
  these source/review contracts while implementing artifact replay.

  Acceptance: a source-free artifact roundtrip independently checks that the
  established qualification belongs to the exact authorized invocation result,
  after carrier/predicate/custody obligations that do not assume the introduced
  qualification. Reject forged result/route evidence, substituted same-spelled
  issuers and direct-call borrowing of admitted requirement authority. Retain
  private issuer identities and dependencies without granting consumer call,
  conformance or private-type access or hiding admissions. Public ordinary
  requirements still permit valid downstream conformers. Resource capacity and
  classification-specific boundary-route restrictions remain unchanged.

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
  Customer coverage remains incomplete. Only tests call the facet-cohort
  row emitters, and package review still publishes the broad `Filesystem`
  class; SQUALR-INTEGRATION retains the current native geometry acceptance.

  The explicit admission replay is now pinned at the operations join:
  `accepted_lock` shows an accepted project emits an artifact carrying no
  receiver-admission claim with no supplied policy — the replay rejects it —
  and binds the supplied policy's exact identity when given one, admitting
  only under that identity and rejecting under any other. The
  `receiving_admission` leg drives the same join through one demanded leaf:
  an accepted customer whose demanded closure is a package-owned
  `Console::exit_process` compiler intrinsic emits with no receiving policy
  and binds no permission-policy identity; the same program rejects under
  the explicit empty policy, under rows substituting the accepted permission
  in either direction (a narrower or wider disposition at an approved
  coordinate), and under rows keyed to foreign coordinates; and it admits
  under the policy rejoining every demanded leaf's exercised classes,
  replaying that recorded admission exactly. The receiving policy is not
  canonicalized to the demanded set: surplus rows at undemanded coordinates
  admit and the emitted artifact binds the supplied policy's identity.

  The toolchain-settled filesystem cohort is now wired at the single provider
  admission join: `validate_source_evaluated_import_coverage` mints the exact
  `filesystem_mechanism_row` for each demanded leaf served by a settled
  facet-cohort method, `admit_native_providers` merges minted rows into the
  effective receiving policy (caller rows win; identical rows dedup; a
  conflicting row on a cohort mechanism rejects as a duplicate mechanism, so
  a forged classification cannot substitute), and the merged identity is what
  the closure review and emitted artifact bind. Ordinary-release cohort
  methods mint nothing, and unknown method names keep the fail-closed
  classification demand. On the permission axis, package-review discovery
  attaches `filesystem_host_permission_rows` to the root consumer's
  `FilesystemHostService` binding proposal so the accepted policy covers the
  demanded leaves. The remaining pipeline blocker is named: a demanded
  canonical `FilesystemHost` leaf resolves to zero selected provider rows at
  the closure review — provider plans derive only from `satisfies`
  conformances, none exists for the canonical host, and a package-authored
  `satisfies`/`select_provider` for a trait named `FilesystemHost` is refused
  as an unknown boundary slot;
  `compiler/tests/terminal_authority/filesystem_cohort_witness.rs` pins that
  stop. The `cli_mvp` and `console-exit-app` READMEs no longer describe the
  removed gate.

  The [cli_mvp README](samples/cli/basics/cli_mvp/README.md) records ordinary
  package review and native macOS ARM64 execution without receiving-policy
  input at `c459b1d25c`; SAMPLE-CORPUS owns its remaining Windows and Linux
  host runs. Preserve that customer and Squalr's recorded native geometry
  control while completing the remaining production/admission matrix.

  Remaining work:

  - Complete console-exit-app, the Cathedral native smoke and the remaining
    host legs of `cli_mvp` and Squalr after ordinary package acceptance with
    no receiving-policy input. Report each one's next unrelated blocker
    without claiming its end-to-end completion.
    Preserve the ordinary-production harness route without projecting
    accepted package rows into a receiving policy; explicit admission tests
    separately supply their receiver policy.
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
    The customer is a program, not a compile: every build
    activation, including a dependency's own, runs against its own package
    snapshot with no ambient host filesystem under
    [inputs and default filesystem](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem),
    and the `$OmegaBuildSourceRoot`/`$OmegaBuildOutputRoot` facets enforce
    that root in `build_paths.rs`. Confined activity inside that box is not
    authority the shipped artifact carries, so restate the acceptance above
    to name a program customer rather than an ordinary compile.
    FILESYSTEM-RELEASE-CONTRACT owns the occurrence evidence. Generic close
    need not be supported to admit a separately proved constrained
    occurrence.

  The console CLI witness requires a nonempty reported native output or the
  exact witnessed physical legalization rejection, preserving the accepted
  project in both cases. At `ffd6cc9009` on macOS ARM64, ordinary package
  review retains all three Console decisions, then compilation without a
  receiving policy rejects with `Selection(Legalization(SourceCustodyMismatch))`.
  Resume through `target-operations-to-selected-instructions/src/legalization`;
  this is an implementation dependency, not a design block. Run
  `RUST_MIN_STACK=67108864 cargo nextest run -p omega --test package_commands --no-fail-fast -E 'test(=console_exit_permission::console_exit_permission_is_an_explicit_decision_that_the_lock_retains)'`
  on macOS; the fixture README gives the project audit command.
  FILESYSTEM-RELEASE-CONTRACT owns the program-side release proof.

  Acceptance: after ordinary package acceptance the four customers above emit
  with no receiving-policy input. The same program rejects at explicit
  admission under a denying policy and admits under a sufficient accepted
  policy; an absent policy never yields an admission receipt. Forged
  classifications, invalid proofs and violated requested physical exclusions
  still reject. No PCC request becomes mandatory.

- **FILESYSTEM-RELEASE-CONTRACT.** Implement the
  [bounded occurrence-specific release proof](wiki/spec/build/permissions.md#bounded-occurrence-specific-release-proof)
  for a program's own open/query/close occurrence through checked flow and native
  realization: exact object/argument contract, handle/alias preservation through
  intervening calls, one applicable release, and no later use. Authority classes
  alone are not preservation evidence. Build execution observations establish
  nothing about the shipped program's calls.

  Carry the program-side derivation as Terminal evidence and rejoin its exact
  occurrence at native realization in
  `native-realization/src/native_realization/terminal_authority_policy/filesystem.rs`.
  Missing evidence must retain the conservative classification, not synthesize
  an empty permission row.

  Native customer:
  `tests/omega/pass/filesystem/windows_canonicalize_exit`. Its recorded stop is
  `structural field store: scalar field type` in
  `typed-trees-to-checked-trees/src/execution/unit/structural_scalar_store/`:
  `self.unit_result = self.fs.write_all(..)` stores a structural `UnitResult`,
  while that path admits scalar fields. The transitive closure also needs nested
  structural sum construction/extraction, borrowed case observation, and whole
  nominal receiver replacement, including match-produced assignments.
  Further isolated prerequisites are paused: resume with a plan covering that
  closure through shared state/value planning, preserving recursive layout and
  referent identity. Rerun `filesystem/native_close` before assuming a stop.

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
    case-state transfer, graph-level aggregate result routes, and computed
    reference arguments outside proven helper-result relations. Other
    unsupported expression shapes remain conservative.
    Each has landed slices; rerun its `write_frame_*` tests in
    `typed-trees-to-checked-trees/src/tests/termination/` to find the residue.
    Prefer shared fixpoint and alias reasoning over syntax-shape exceptions.
  - Carry finite reference candidates through receiver dispatch, named-state
    transfer and wire-codec calls; those consumers still lack contextual
    substitution. Interior reference loads need independent load evidence,
    not the enclosing carrier's path. Unknown rebinds must remain opaque.
    Owners: `state_write_walk.rs`, `caller_aliases.rs`, `demand.rs`.
    Call-argument composition alone does not establish source admission:
    a reference-valued `match` still fails branch-custody joining, and a
    boundary result with two inputs sharing its lifetime rejects in view
    signature validation. Close those borrow-join dependencies before claiming
    the source-to-checked example: choose either mutable input, write through
    the returned alias inside a value expression, then prove a disjoint index
    bound; the overlapping-index variant must reject.

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
  shared-reference leaves through their slot stores including leaf reads
  spelled through an exclusive `&mut` carrier binding when every frontier
  names the leaf's writes exactly, leaf spellings demanded as call operands
  through exclusive bindings replayed from the binding's own provenance, `&`
  bindings declared from literal-indexed carrier leaves resolved through the
  binding's own provenance replay, `&` bindings declared from constructed
  literal leaves — a record-literal member selection or a literal-indexed
  array element — replayed through the same provenance walk, demanded call
  operands spelled through a reference leaf inside an indexed carrier rebased
  to the referent the element's literal store supplied before the premise
  surface drops the index selector it cannot carry, helper-returned
  reference leaves resolved through caller slot stores including callee-local
  binding transfers and nested helper calls, and nested call-result
  arguments; partition replay follows reference and generic-application leaves
  with exact declared-field provenance. Do not rebuild those as new slices.
  Remaining work:

  - Complete owned value loads through references — additional
    reference-boundary loads.
  - Mutable demanded paths, helper bodies that may write the demanded
    projection, write-tainted nested calls, generic or dispatched callees,
    ambiguous or dynamic projections — including a member selection whose
    field symbol never resolved on an indexed temporary — opaque or
    overlapping write frames, unresolved exclusive aliases and unresolved
    result routes keep no checked guarantee. Admit one only from exact
    provenance.
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
  element write retires only that element's facts. Domain-declared
  membership subjects project the carrier itself: `self.num.pos in
  NonZero` and `self[0] in Utf8` resolve to the declared field or element
  type instead of emitting `no resolved subject type`
  (`validation/src/value_custody/expression_types/result_type.rs`).

  `flow/state_values/fields.rs` joins each channel from per-edge
  `EdgeDelivery` records instead of discarding provenance: literals must agree
  on every predecessor, byte predicates intersect, integer bounds union with
  the declared carrier standing in for a bound-free edge. A transition edge
  lacking the field refutes; a call's return edge -- which never captures
  field rows -- forwards the running `predicate_ceiling`, the co-inductive
  premise element stores may assume for the carrier when re-seeding its
  declared classes (`flow/transfers/byte_sequences.rs`). `ValidUtf8` reseeding
  additionally requires the carrier to prove `AsciiOnly`; a store outside the
  class retires it. Integer bounds that keep extending across joins widen to
  the smallest authored integer literal covering the fresh bound rather than
  the whole carrier range, so a converged loop counter keeps the authored
  window (`bounds_growth` threshold widening). Guard arms mint
  `AssignedIntegerBounds` for `place OP literal` conjuncts
  (`flow/exits/guards.rs`), and `values/bounds.rs` carries the exact/wrapping/
  saturating divide arm. `omega --check --target linux_x86_64
  samples/cli/basics/multiplication_table/main.omg` reports no diagnostics.

  Customer probe: `omega --check --target linux_x86_64
  samples/cli/games/dungeon_crawler_cli/main.omg`. It reported 55
  diagnostics at b2ea74973c; the call-side proofs now cover every finite
  reference-result candidate (192fa77d3b), collection elements seed
  whole-extent field domains that runtime-indexed subjects and slice views
  narrow from (5292e6ec8c, which left only the four index rows), and the
  sample's declared `u64 [0..=16]`/`[0..=15]` bounds let the single shared
  statement transfer discharge those (4c09f582f1). The probe reports
  no diagnostics on macOS ARM64 and, after two same-day upstream
  regressions were repaired, again on linux_x86_64 at 8421784e74:
  package-keyed domain semantic identity (3248d8c82c) pooled a package's
  own `[u8;8]::Utf8` against std's `[u8;256]::Utf8` as ambiguous until
  domain references learned the documented own-package tier
  (f900cba191), and operand-order move replay (6662a37929) exposed that
  `expression_result_type_reference` never classified `UInt`/`Int`
  carriers as integer, breaking `calls = calls + 1` restores
  (3bf8be9383).

  Remaining work:

  - `&mut self` receivers hand back only the ZII-seeded `MachineFieldDomain`
    rows: a callee's `self` entry assumption is ZII-gated, so its return
    cannot guarantee non-ZII rows, and a caller's non-ZII receiver facts
    survive a method call only through frame precision. Widening needs a
    contract decision, not a flow change; the question is recorded in
    `OWNER_QUESTIONS.md` as `mutable-self-receiver-declared-field-rows`
    (8421784e74), and the clean probe does not exercise it.
  - `RoomLookup` still passes an uninitialized readable `&mut Room`
    out-parameter where write-only `&write Room` is the intended spelling;
    `&write` admission still rejects constrained records. That spelling is
    **WRITE-ONLY-BORROW**'s surface, not a flow gap here.

  Acceptance: the dungeon's `RoomLookup`, `MazeBuilder`, and game-state calls
  satisfy default field obligations, while corrupted elements and stale
  aliased fields reject at calls, transitions, and returns.

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

  Landed: return-edge cleanup no longer consults the recognizer.
  `plain_home_cleanup` realizes each `DiscardRoot` keyed by the root's own
  evidence — a live affine home, an affine reference leaf, or the arrival
  declaration of an unobserved owned parameter (function or block-entry) that
  never received a home. `accepts` now only decides whether owned arrivals
  suppress their storage homes. Still owed under this flag: residual
  `DiscardResidual`/`InvokeNominal` actions and Jump/conditional edges, and
  retiring `accepts` once no admission decision reads it.

- **STATE-LOCAL-VALUE-FRONTIER.** Complete ordinary evaluation/value transport
  in Psi argument normalization, checked scalar computations, call/result plans
  and Terminal production. Remaining operands include dynamic/borrowed/projected
  storage, effectful state arguments/returns, wider structural returned calls and
  mixed structural/scalar signatures, including boundary consumers. Materialize
  each value and activate its staged loan at the authored evaluation point.
  Retain exact result owners for shared/mutable/write-only temporary borrows,
  multiple argument producers, self consumers and projected claims; **CML4**
  owns residual cleanup. Replace remaining flat guarded-call hoisting with the
  same evaluation graph, not another source-order family. Before removing
  `rewrite_guarded_transition_argument_calls`, preserve call-result facts at
  successor entry and invalidate guard facts after earlier operand mutations.
  The existing `transition_argument_call_result_derives_the_exact_entry_subject`
  and `jump_operand_mutation_cannot_replay_the_taken_guard` regressions must
  pass without synthesized source states. A case-edge helper call must also
  preserve its invocation receiver and local ownership until evaluation ends;
  capture-by-referenced-name alone loses implicit receiver forwarding.

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
    substitutions. An immutable state-local `let` now discharges a callee
    `requires` from the saved value -- `let n = items.len` then
    `inner(items, n)` proves `rest.len <= capacity`, and the observation
    keeps the initializer's arithmetic (`items.len + 1`) -- through
    `ranking_range/saved_arguments` binding each stable local to its
    initializer's polynomial before the boundary substitution. A local
    whose initializer reads a `mut` formal/local, an exclusively borrowed
    carrier or an exclusive-reference holder yields no observation, so a
    mutable carrier's incoming value is never conflated with a later
    write; giving mutable carriers an entry-value atom so that reading
    also discharges remains open, as do dependent/public-trait results
    and subslice bounds. **CRASH-CONTRACT** shares the capture path;
    case-qualified, indexed, generic, reference-valued and floating entry
    predicates need exact identities/totality. Unchanged entry observations
    may justify published routes; later writes and current body facts may
    not. Ranked-loop crash guards require independently checked all-path
    invariants, never first-pass facts ignoring backedges.
    Concrete subslice customer: `compiler --test bounded_slice_selectors`
    retains two inferred endpoint calls over distinct array extents. At
    `fce78a5bcc` plus the binder-carrier repair on macOS ARM64
    (`RUST_MIN_STACK=67108864`), source checking
    and the corresponding `checked-interpreter --test suite borrowed_subslices`
    execution pass, but Terminal production rejects the missing checked scalar
    control plan in `checked-trees-to-lowered-psi/src/machine_lowering`.
    Scalar bounded helpers independently replay and execute natively; that does
    not close slice transport. Replace the explicit unfinished assertion with
    source-free Terminal and native execution when the ordinary join exists.
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
    `fail/generics/authored_const_call_operator_unselected_provider`.
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

  Delete the remaining source-shape producers as ordinary graph operations
  cover their semantics. `checked_trees/flow/terminal/*_plans.rs` still has
  whole-machine shapes such as `CheckedPayloadlessGuardedCallReturnMachinePlan`.
  `machine_lowering/machine_dispatch.rs` rejects simultaneous scalar and Unit
  dynamic joins. Preserve compile-known receiver attachment authority
  and claim transport when consolidating those routes. `composed_unit_claims.rs`
  pins shared linear custody across exclusive arms, sequential settlements,
  and corrupted receipt/fact rejection.
  A failed source/custody rejoin must never fall back to a weaker recognizer.
  Acceptance: one caller combines those ordinary operations, with reordered
  state declarations and inserted computations, while forged edge bindings,
  missing provider authority and conflicting loans still reject. Do not add a
  producer family for the combination.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Ordinary generic
  `drop<T>` now checks, lowers, and invokes the exact owner-attached hook
  selection under [nominal cleanup](wiki/spec/terminal-psi/ownership.md#nominal-cleanup)
  and [explicit early disposal](wiki/language_guide/chapter_17_drops_and_cleanup.md#explicit-early-disposal):
  the call transfers the whole value into the `drop<T>` member
  specialization, and the member's terminator runs the instantiated type's
  `T::drop` attachment once as a nominal affine cleanup edge or proves the
  parameter trivially consumed. `ReturnUnitNominalAffine` is now legal on
  non-entry member machines while the dispatched entry lane keeps its exact
  module closure
  (`terminal-verifier/src/validation/affine_cleanup.rs`), and a completion
  gate rejects checked plans that would return while a nominal-drop local
  still owns custody
  (`typed-trees-to-checked-trees/execution/unit/control/checked_machine.rs`).
  `tests/omega/pass/drops/core_drop_owner_hook` reaches `inspect-terminal`
  green: `drop<Guard>`'s return invokes `Guard::drop` exactly once and
  `drop<Carrier>` discards trivially. Erased fields stay semantically
  present in record shapes and never produce runtime cleanup:
  `data_graph_requires_nominal_drop_with_substitutions`
  (`validation/src/value_custody/cleanup.rs`) skips erased record and case
  members, so an erased-only owner stays an ordinary affine record — even
  when the erased member's own type carries a `drop` attachment — and a
  record literal rejects an erased initializer at lowering ("erased record
  member has no runtime initializer"). Admitting erased-bearing construction
  needs `terminal-codec`'s `validate_establish_record` to tolerate erased
  declarations; that file belongs to PROOF-RELEVANCE-MIGRATION's erased
  formal/contract slice. Source selection of a reserved `T::drop` already
  rejects (cleanup.rs). The corpus call sites are
  `tests/omega/pass/drops/core_drop_explicit_consume` (checked-only) and
  `drops/core_drop_owner_hook`, both registered in
  `compiler/tests/canary_suite.rs`; the native codec route is not yet
  realized. **CML4**
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

  The static carriers exist: `provider-planning`'s `task_call_graph`
  (`task_plans/stack_graphs.rs`) derives a sealed WCSU `StackPlan` over exact
  checked-body call edges, rejects a possibly-suspending call with no
  canonical crossing, and seals every call it cannot resolve — requirement
  slots, machine parameters, dynamic descriptors and non-checked supply —
  into the frame's `UnresolvedCallSite` roster, so the composed demand
  publishes exact only when the roster is empty and `establish_stack_lease`
  refuses a partial projection (458cac408f, 30d545108f). Admission-time
  `CallTargetAssignment`s (`task_plans/call_target_bindings.rs`,
  ac1efde2f7) bind a sealed site to a concrete checked-body subtree: the
  bound subtree joins the retained composition evidence (db8b11d66a) and its
  canonical suspension crossings join the plan roster (2e93adfdc8), so a
  covered projection re-seals exact. The `task-plans` ledger transacts
  `MovedTaskArguments` marshalled under the plan's `TaskArgumentLayout`
  against a plan-bound nonmoving `StackLease`, conserving both on every
  start rejection (35ae31b963, bb66844e56); `TaskRuntimeAdmission` gates
  starts on bounded stack provisioning (316eaa89e5), parks and resumes
  claims only at canonical crossings carrying each live place's
  `LiveCarryDemand` (c1b16cb063, afda8fbe16), and settles `Cancelled` only
  against a recorded then observed cancellation request (fe9e63735b).
  Routed source `Task<T>` establishment exists:
  `lifecycle_ledger/claim_route.rs` (1fedf20882) mints the exact
  `provider`/`activation` field pair a `Task<T>` value carries onto every
  accepted claim, resolves the pair to its live claim on the minting
  instance only, and drives every value-carrying transition —
  cancellation request, park/resume, safe-point observation, settlement —
  through `*_by_route` operations sharing the claim-object checks; foreign
  instances, fabricated pairs, and settled routes all fail closed, and a
  burned activation identity never rebinds. Per the [task-plans
  note](omega-rust/omega/representations/task-plans/README.md), a real
  selected runtime executing the transitions the ledger models remains.

  Acceptance: stack/control custody is never compiler-owned or lost across a
  suspension edge, and missing crossing demand rejects. Exercise concurrent
  start/park/resume/finish, rejection returning every moved argument and lease,
  cross-instance settlement rejection, and fresh storage eras on reuse. Static
  plans and lifecycle ledger tests alone do not establish executable activation
  or argument conservation.

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

  Every `AtomicEvent` operation retains a `reads_from` edge — the
  pre-activation `InitialResidency` or the observed write's operation
  identity — and every write event retains a `modification_after` edge
  naming the order member it immediately follows, both encoded into
  operation identity like the retained ordering. Unit validation replays
  both coherence axioms function-wide through
  `happens_before_atomic_coherence_violation`: a bounded happens-before
  derivation (intra-block position union block dominance) feeds a
  reaching-writes must-analysis, so a `Write` claim resolves only when the
  named write is the modification-order-latest write to the place on every
  execution path — the observation's witness for readers, the immediate
  predecessor for writers. Unit tests pin the refusals on both sides:
  non-dominating, successor, or converging-branch members fail their
  `NotHappensBefore` case; overwritten claims fail `ObservedWriteOverwritten`
  or `PredecessorNotLatest`; partially-written paths refuse
  `InitialResidency`; and a fence joins no modification order yet disturbs
  none. The admitted-ordering matrix, fence legality, instruction-observed
  priors, and single-attempt custody were already independently rechecked.

  Remaining work:

  - `synchronizes_with`/`global_sequential_order`/fence-pair synchronization
    have no checkable content inside one activation — every sw-forming rule
    pairs events across activations — so they land with TR3-TR8's
    concurrent-execution route.
  - Terminal Psi still emits no normalized atomic events; the producer and
    the real concurrent-activation controls wait on TR3-TR8's route.
  - Checked target realization does not exist yet; a weaker acquire remains
    unauthorized without its protocol proof.

- **BLOCKEXEC.** Implement a package-level blocking executor with bounded
  queues, moved custody, linear completion claims, suspension, and provider
  selection. Hung-worker recovery requiring termination must use process
  isolation.

  Contract surface landed as bundled package `blocking-executor` at
  `source/library/blocking-executor/` (`library` lane — the `source/`
  topology gate admits only library/psi/omega owners, and bundled packages
  are consumed through ordinary `builder.depend` edges like `std`):
  `BoundedQueue<T, const N: u64>` with proof-discharged capacity admission;
  `Submission`/`SubmitOutcome` moving custody by value on the accept and
  reject edges; linear `Ticket<T>` completion claims consumed by
  `Ticket::settle`; `suspends; blocks` worker/settle requirements over a
  `WaitSubstrate` word-wait + wake-one/wake-many boundary; and the
  `WorkerProvider` boundary trait selected through ordinary build
  `select_provider` — the package declares its own trait because core's
  `TaskRuntime` is not public outside the bundled library.

  Concrete custody/claim pin landed at
  `tests/omega/pass/blockexec/blocking_executor_custody_claims_compile`:
  the contract surface hand-instantiated at `Token` (the
  `task_lifecycle_operations` convention) — `[linear]` `Submission`,
  `Ticket`, and `Executor`; refused-admission `Rejected` custody return;
  `Ticket::settle` declared `suspends; blocks` parking on the wait word via
  `suspend block`; `WorkerProvider` satisfied by a canary provider and
  selected through `builder.select_provider`, with the routed
  `Service<WorkerProvider>` field established into `Executor` on the
  application root. Registered in `fixture_rosters/task_runtime.rs`; the
  driving test
  `task_runtime::blocking_executor_custody_claims_hold_through_the_concrete_pin`
  compiles it to checked trees and pins linearity, suspension/blocking
  envelopes, and the selected provider plan.

  The package passes `omega --check` at d05ec39a5d + this slice:
  `Executor.runtime` is now the plain closed-carrier spelling
  `Service<WorkerProvider>` (the retired `in Bound` qualification was
  dropped — the closed `Service` carrier rejects authored qualification).
  The package self-check is pinned by
  `task_runtime::blocking_executor_package_checks_with_the_closed_service_carrier`,
  which compiles `source/library/blocking-executor/main.omg` to checked
  trees and re-fails on the retired spelling. The depend-and-consume
  consumer shape (`builder.depend` + `use blocking_executor::executor` +
  unqualified `satisfies WorkerProvider::execute` + `select_provider`)
  checks clean under `omega --check` on a scratch consumer, but the
  checked-compile fixture harness does not wire `builder.depend` rows into
  package inputs — a durable consumer fixture waits on that harness leg.

  Remaining legs: queue/executor machine bodies (generic-machine frontier,
  recorded in `source/library/core/fixed_vec.omg`; a concrete ring over
  `[linear]` slots also has no provable slot-empty fact channel),
  call-returned sums carrying more than one linear case payload (the
  multiplicity checker wants "an explicit outcome mapping" — the
  conserved-claim join machinery), real provider admission for the
  type-attached `boundary requirement`s joining TR3-TR8's execution route,
  depend-edge wiring in the checked-compile fixture harness for a durable
  consumer pin, and the process-isolation boundary for hung-worker
  recovery.

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
  for PE, versioned ELF, and Darwin/Mach-O through the remaining port-bearing
  native paths, preserving the normalized locator, evaluated
  plan, target applicability and admitted provider custody under
  [normalized-import evidence](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement).

  Port-bearing artifacts need a `port_effects` production writer connected
  to native physical evidence and independent replay.

  Acceptance: port-bearing artifacts retain their exact effects.
  Independent native replay rejects missing, duplicate, substituted or
  role-swapped children. Raw foreign bytes remain locator data, never Omega
  symbol names or ambient lookup authority.

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
    The Terminal side now carries the application: `FloatMeaningSource::
    SemanticApplication` (codec tag 10) spells the catalog
    `FloatSemanticContractIdentity`, the declared result format, and the
    operand roster as projection-row references; the verifier rejoins the
    contract through `for_contract_identity`, checks each operand's kind,
    rejoins explicit `Format` arguments to the declared result format (or
    meaning-operand formats when the signature has no `Format`), resolves
    meaning operands to strictly earlier rows, and re-runs
    `kernel_discharge` when every meaning operand is a literal — the
    discharged result rides the shared proof-value space. Equality row
    validation establishes carrier compatibility, not the truth of an
    equality or discharge of a source obligation. The checked side
    now produces the binding (landed 2026-09-19): each sealed
    `FloatSemantics::*` call inside a contract expression —
    declaration-level or transported `ensures` use site — resolves through
    the complete toolchain signature via `from_source_identity` (never the
    leaf spelling) and gains a `CheckedFloatSemanticApplication` row on the
    new `ProofFacts::float_semantic_applications` side table plus a
    transitional `SemanticApplication`-keyed projection row for its result.
    `Format` operands match the const-substituted `FloatFormat` literal
    against the sealed `BINARY32`/`BINARY64` canonical encodings;
    `Meaning` operands rejoin declaration invocations, nested applications,
    or the use site's operand key, and each `==` pairing an application
    emits its equality at the site coordinate (`None` at the declaration).
    Producer and kernel discharge meet in
    `typed-trees-to-checked-trees/src/proof/float_meaning.rs` /
    `checked-trees/src/checked_trees/proof/float_meaning.rs`. Remaining:
    `checked-trees-to-lowered-psi` emission of the application rows into
    Terminal `FloatMeaningSource::SemanticApplication` is still missing,
    but emission alone does not close the source-proof path described below.
  - The non-call operation result and call result
    [source classes](wiki/spec/terminal-psi/mathematical_values.md#source-identity)
    now carry the use-site coordinate settled
    [contract import](wiki/spec/proofs/contracts.md) requires ("caller import
    requires the matching result case and argument/result substitution") and
    gain checked and lowered producers (landed 2026-09-19).
    `CheckedFloatProjectionSourceKey` distinguishes `DirectCallResult`/
    `DirectOperationResult` by a `CheckedFloatUseSite` (owner machine/state,
    statement index, call ordinal); `instantiate_transported_ensures` in
    `typed-trees-to-checked-trees/src/proof/float_meaning.rs` re-binds each
    imported `ensures` equality at every `ContractCallFact`/
    `ContractOperatorUseFact` site — `result` names the producer the exact
    call or FMA operation emits there, each callee parameter names the
    authored argument expression — and `direct_result_float_meaning_
    reflexivity` rejoins authored rows on (owner, expression, use site).
    `checked-trees-to-lowered-psi/src/proofs/float_meaning_projection.rs`
    joins each site to its `LoweredSourceCallOccurrence`/
    `LoweredSelectedIeeeFloatFmaOccurrence` coordinate and emits
    `DirectCallFloatResult`/`DirectOperationFloatResult` only for the
    producer-kind partition the verifier rejoins. Verified on Linux x86-64
    with `cargo nextest run -p checked-trees-to-lowered-psi --lib -E
    'test(/float_meaning_projection/)'` (11/11) and `cargo nextest run -p
    typed-trees-to-checked-trees --lib -E 'test(/float/)'` (96/96): a caller
    `helper(value)` whose machines carry the reflexive `ensures` now holds a
    `DirectCallResult` row at its call site distinct from its own
    `DirectMachineResult`, and two call sites project distinct rows.

  Acceptance: every `FloatSemantics` obligation a `Float::*` slot contract
  cites is discharged through a checked kernel binding, not catalog identity
  alone, and `fail/float/float_semantics_lookalike_grants_no_primitive` still
  rejects. The two open source classes gain a producer the verifier rejoins,
  raised at a use site the checked source key distinguishes.

  This is an implementation gap under the existing FloatMeaning and contract
  import rules, not a language-design blocker.
  The source customer below now passes `compile_to_checked` with the real
  core; changing `3.0f32` to `4.0f32` is disproved. The source checker uses
  `validation/src/proof_contracts/float_projection_bindings/semantic_values.rs`
  for closed meaning-valued applications, sharing exact catalog and format
  recognition with the checked binder. Unknown operands remain unproved;
  selected authored equality cannot acquire builtin meaning semantics.

  ```omega
  use omega::language::core::float_operations;
  machine read() -> u64
  ensures FloatSemantics::add(FloatFormat::BINARY32,
      Float::meaning32(1.0f32), Float::meaning32(2.0f32))
      == Float::meaning32(3.0f32);
  { 7 }
  ```

  Next retain this authored equality as a typed contract proposition with its
  exact owner/use-site obligation, emit the application through
  `checked-trees-to-lowered-psi`, and discharge that obligation through the
  ordinary `CertificateDerived` production and independent replay route.
  The checked equality side table and Terminal mathematical-value rows alone
  do not join `TerminalMachine.contract.ensures` to a proved obligation.
  Do not erase the claim to `Truth`/`Empty` or treat well-formed metadata as
  proof. Acceptance remains this same source reaching source-free verification
  with its contract intact, plus rejection of the false twin.
  Resume on macOS AArch64 with `RUST_MIN_STACK=67108864 cargo nextest run -p
  compiler --test float_semantic_applications --no-fail-fast --no-tests fail`.
  On the source-check implementation based on `0917f9983c`, all five tests pass:
  real-core contracts (including nested f64, NaN and signed-zero controls),
  false contracts, authored equality, unknown inputs, and the source-free
  artifact operand-validation regression. The latter constructs application
  metadata explicitly and does not establish end-to-end proof production. Reuse
  `float_projection_bindings::semantic_operations::exact_toolchain_float_semantic_contract`
  and the signature-selected `numerics::FloatSemanticOperation::kernel_discharge`;
  catalog identity alone is insufficient. An application can produce the
  existing mathematical value type; a missing internal value-source form does
  not by itself require a new source-language type.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Restore ordinary native
  descriptor invocation and forwarding, beginning with a non-entry helper that
  receives one borrowed two-word descriptor, forwards it once, invokes a
  requirement, and uses its result across computations and branches. The
  dependency is an ordinary indirect-call operand and its ABI, clobber, effect,
  and reach contract, not another whole-body recognizer. Do not resume
  target-only descriptor composition: two such milestones left this native
  customer unsupported.

  Source helpers retain scalar bindings around forwarding/dispatch and their
  actual returning control through the shared scalar evaluator
  (`checked-trees-to-lowered-psi/src/unit/dynamic_composed_unit/forwarded_helpers.rs`).
  The source-to-verified-artifact interpreter regression
  `forwarded_descriptor_calculations_execute_through_verified_artifact` observes
  both branches and a computation after final dispatch; substituted helper
  bodies and disagreeing join rosters reject (verified at `87102d60aa` on
  macOS ARM64). Resume on macOS with
  `RUST_MIN_STACK=67108864 cargo nextest run -p checked-trees-to-lowered-psi --lib --no-fail-fast -E 'test(dynamic_composed_unit)'`.
  This establishes semantic execution, not native publication. Extra effects,
  mutable helper locals, and differing helper/requirement result carriers still
  need composition through their ordinary operation owners.

  `abstract-operations-to-target-operations` lowers the descriptor parameter
  to `DynamicParameter{Scalar,Unit}Call` with two pointer
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
  hermetic const evaluation, and [selected realization coverage](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children).
  Connect provider-dependent const-generic arguments (fields, `let`
  annotations, return types) to Omega's actual provider plan through the
  [semantic evaluation owner](omega-rust/psi/semantics/build-time-evaluation/README.md#semantic-admission-boundary).
  `const_evaluation/const_generic_calls.rs` runs before Build and currently
  rejects these uses; it cannot choose a body by scanning visible satisfiers.
  Preserve the dependent application until selected execution is available,
  using `machine_execution/selected_operators.rs` and independently checked
  fold custody, as the fixed-array-length continuation does. If Build itself
  depends on that pending application, report the dependency cycle.
  Acceptance: the same const application selects either of two authored
  provider bodies through distinct build plans and materializes their distinct
  results; adding an unselected satisfier changes neither result nor admission.
  Cover named and token calls, reject absent/substituted selection and forged
  results, and preserve provider-free const application evaluation
  (`tests/omega/pass/generics/authored_const_application_local_destination`).
  Complete the
  portable target capsule and its application identity; a checked
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

  An owned-`self` receiver is now the same route too: `token.consume();` on a
  public `boundary requirement Token::consume(self)` settles a
  receiver-place-keyed `forward_receiver` dispatch row (one row per
  single-place receiver site, keyed on the place's own symbol), and the
  rewrite splices that place in as the adapter's leading argument —
  `Provider::entry(token)` — in both statement and expression position. The
  interpreter binds it as an ordinary call, and Unit construction admits the
  attached adapter through its ordinary structural-argument rungs
  (`tests/fixtures/boundary-requirement-member-call`, harness
  `canary_suite/top_level_requirement_member_call.rs`; Linux x86-64 verified:
  interpreter exit and native-artifact exit both execute the selected
  adapter). Borrowed (`&self`/`&mut self`), multi-place (`self.f.m()`), and
  non-single-symbol receivers stay fenced: settlement leaves them unrowed and
  the direct call rejects.

  Remaining work:

  - Parameterized and projected-receiver requirements.
    `is_directly_callable_top_level_requirement` still admits no type or
    lifetime parameter, and the rewrite rejects family rows and receiver
    paths longer than one member. `core/task.omg` (`Task::finish<T>(self)`,
    `request_cancel`) and
    `core/interrupt.omg` (`InterruptMaskGuard::restore`,
    `InterruptAcknowledgement::complete`) have no library or canary satisfier
    and reach execution only as installation-bound reach rows
    (**BOUNDED-INSTALLATION-REACH-ROWS**).
    [Interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime)
    names this item for that selection and lineage integration.
  - External satisfiers still need interpreter provider execution and
    structural-argument composition. Native scalar-returning imports are no
    longer blocked on foreign-call emission: retain
    `compiler --test efb3_flat_record_probe
    top_level_external_requirement_returns_and_reuses_its_result_natively`
    as the macOS ARM64 control, including its exact requirement identity,
    selected import, returned value and second-call reuse. Other hosts remain
    unverified by that test. Missing import settlement must keep rejecting in
    `external_boundary_requirement_via_exit_canary_keeps_requirement_seam`;
    its ordinary foreign `exit_with` is not canonical ProcessExit evidence.
    The borrowed-record counterpart remains a frontend dependency: at
    `4752d94c3f6` on macOS ARM64, replace the mixed-argument probe's trait with
    `pub data Move {}` and
    `pub boundary requirement Move::shift(delta: i32, p: &Point, bias: i32) -> i32;`,
    remove its Service field, provider selection and `reaches Move` clause,
    and use `Move::shift` for the same two calls. Terminal production rejects `Main::main` at
    `statement sequence: call: call operation, statement 2` in checked
    `execution/unit/`, before native lowering. Preserve the record and its
    initialized fields when completing that join; the scalar control does
    not close it.
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
  `omega/tests/package_commands/generated.rs`). Restricted-build acceptance
  now surfaces compiler-derived requests in review output; dependency-purpose
  binding still does not exist.

  Remaining work:

  - Finish [restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance)
    at the admitted-program evaluator, before a restricted host effect.
    Candidate review already projects normalized request meaning and retains
    explicit decision rows in the lock. The consuming-lock join in
    `packages/manager/src/review/restricted_build_grants.rs` rejects missing,
    changed, or wrong-context consent before consuming a checked result, but
    that is too late to authorize the build action itself. Move the decision
    and actual invocation-grant check ahead of execution, retaining incomplete
    audit state and exact candidate scope for install/update review and resume.
    Audit-only inspection must not issue grants. Exercise newly added, widened,
    transitive, rejected, and interrupted requests through those commands.
    Keep captured immutable inputs and compiler-owned bounded private staging
    outside restricted-action consent; generic sponsors for supplied host
    directories remain restricted. Controls live in
    `packages/manager/src/review/candidate/compilation/tests/restricted_build_grants.rs`
    and `omega/tests/build_input_inventory.rs`. Do not introduce an arbitrary
    recursive build API or a new host protocol as part of this join.
  - Bind restricted-request checkpoints to the existing `PackageCheckedContext`
    and generated handoff. Build/product occurrences already have independent
    producer reviews, lock policy and reconstruction; acceptance for one must
    not authorize a restricted action by the other, even on the same target.
    Exercise checkpoint drift with the dual-role generated-source customer in
    `omega/tests/package_commands/build_purposes.rs`.

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

  Complete composed-Unit plans for trait-default, float, wire,
  arithmetic-helper, guarded-call, and looping-cast canaries and the
  target-correct non-Linux Console catalog entry. The float pass group
  migrated to ordinary `omega_language_std` edges (50 packaged roots, 49
  std consumers under `tests/omega/pass/float`; the std-free
  `named_float_to_integer_no_context_compile` and
  `tests/omega/run/float/sqrt_probe` remain standalone), so its directed
  and twin canaries now reach the package route and expose the open
  composed-Unit and checked-operator legs there. The dependent,
  ownership, constants, calls, arithmetic, and versioning consumers
  followed the same edge (28 roots, all checked-only), and the generics,
  filesystem-straggler, host, and time holdouts joined them (packaged
  counts: generics 37, filesystem 85, host 23/22, time 17) — macOS-gated
  filesystem and fs-time-interop roots bind only `macos_arm64`, the
  windows_* seams bind only `windows_x86_64`, and the interpreter-only
  `runtime_time_host_virtual_exit` binds none. Five of the nine
  remaining proofs consumers migrated; `citation_requires_discharged`,
  `proof_nat_structural_lemmas`, `ring_identity_slot_bridge_compile`,
  and `real_boundary_package_compile` stay bundled because they select
  private `boundary machine` entries in core (`add_zero_right`,
  `add_comm`, `add_assoc`, `mul_comm`, `mul_identity`, `zero`, `one` on
  nat/ring; `real_add_commutative` on real) — the package route rejects
  private-machine selection, so they need a per-root declaration
  exception until core exports them or a semantic binding admits them.
  `proofs/kernel_*` drift stays owned by the kernel items.
  `objc` (18) and `arithmetic/saturating_divide_native` are
  macOS-only and unmigrated here; `fail/` and `run/` corpora plus
  `platform/` member sources still hold bundled spellings. Structural
  writeback shares the blocker recorded in `WRITE-ONLY-BORROW`. Feed
  consumer-scoped Console, Filesystem, and UEFI bindings through normal
  package-aware compilation.

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
  producer now has a non-test caller: `compile_dependency_closure` retains
  every dependency's checked compilation as a component candidate, and when a
  package's compile rejects at the fence the loop publishes each named
  component's description with `compiler::published_independent_component_description`
  and recompiles once with them attached. The compiler reports the evaluated
  `Independent` selections — with the dependency package each names — through
  `IndependentComponentDiscovery`, a write-beside output recorded after build
  evaluation and before settlement, so the roster survives a fence rejection;
  a settled compilation retains the admitted descriptions as custody, and the
  package-evidence replay re-verifies them under the build's admission profile
  instead of hitting the fence again. The retry's abandoned build evaluation
  is real sponsored consumption, reconciled through
  `verify_build_session_accounting`'s discarded-usage roster. A component that
  seals its requirement with its own checked adapter already publishes and
  settles — the landed `pick-component` selects `PickProvider`, a checked
  machine inside the same package, and `derive_component_inventory` retains
  `Pick::mark` as a sealed `BoundaryCall`; the feared fused-lowering erasure
  never materializes. Build vocabulary for mechanism acceptances now exists:
  `builder.accept_component_assumption("<64 hex chars>")` is declared beside
  `exclude_crash` in the build prelude, build-evaluation harvests the declared
  digests statically from the root build machine's typed statements (the same
  rule as `select_provider`), settlement passes the set into
  `verify_independent_components` as the `accepted_assumptions` roster, and
  the settled `CheckedExecution` retains it so the package-evidence replay
  re-verifies under the same acceptances. A consumer that never accepted a
  description's `port-mechanism` digest still rejects it unaccepted; a wrong
  digest leaves it unaccepted and a malformed spelling rejects at the
  consumer's own declaration
  (`package_compilation_inputs/independent_components.rs` pins all three).
  Product emission now reads the composition mode: both Terminal-artifact
  producers (`produce_retained_terminal_artifact` and
  `produce_program_entry_terminal_artifact`) run the
  `terminal_artifact/composition_modes.rs` fence over the retained selection
  provenance and reject a settled `Independent` edge — naming the plan and
  provider — instead of emitting a silently fused artifact
  (`independent_components.rs` pins both product routes; check-level
  compilation, description publication and the component join are
  unaffected). The same file pins the attachment fence's coordinate checks —
  a description naming the root package, a foreign package, or a second
  description for one dependency each reject at
  `with_independent_component_descriptions` before any byte is consulted —
  plus the carried-subject substitution rejection (the expected subject is
  caller-supplied, never read from the bytes), the unmatched-component
  rejection when the named dependency selects `Fused` or selects nothing at
  all, and the settled compilation's custody retention of the admitted
  description beside an empty acceptance roster. Corrupt and truncated
  description bytes each reject inside independent verification rather than
  being treated as complete, and the join is per selection: a second
  dependency's own `Independent` edge stays unmatched — naming its plan —
  while only the first dependency's description is attached. Forged fields
  inside otherwise-canonical bytes are pinned the same way: decoding the
  published description, mutating one field, and re-encoding reaches the
  check inside `verify_component` — an unadmitted schema, a frontier
  preceding artifact closure, a forged export row, an omitted module-derived
  entry or export, a declared assumption the consumer never authored, and an
  entry row bound to an assumption absent from the roster each reject inside
  settlement's independent-verification wrapper. The deeper rosters are
  pinned the same way: a forged import slot names no unsealed requirement,
  an outgoing row sealed to a foreign provider digest, an omitted
  provider-occurrence obligation, one requirement claimed by two provider
  digests, a custody row claiming a fact the artifact lacks, and an
  installation-bound service bound the artifact does not retain each reject
  inside the same wrapper.
  Still, a settled `Independent` edge carries no symbolic import or
  installation obligation into the product past that emission fence.

  Remaining work:

  - Supply the native facts. Compiler-published descriptions leave
    `stack_demand` and `realization_identity` absent because a Psi capsule has
    no native realization; the native producer `describe_component` sits in
    `component-candidate`, behind the runtime quarantine in
    `tests/architecture/layering.rs`. `ObligationKind` and `CustodyKind` carry
    no mapping or lease row yet.
  - Realize the settled edge: symbolic imports/exports, entry/leave and
    resource demands, and installation/replacement obligations carried into
    the product. Until that carrier exists the product-emission fence keeps
    rejecting rather than emitting a silent Fused artifact.
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

  - The contracted provider operation the placement lifecycle names.
    `InstallAuthority` now names the provider-canonical `InstallationFactDigest`
    set the operation must establish and `install_validated` rejects a receipt
    that omits or renames a demanded fact (an established superset installs),
    returning every input — the same required-facts gate retirement already
    used. Still open: no provider performs the write-to-execute transition,
    the target cache and ordering work, or instruction-fetch visibility, so
    `InstallationReceipt` still only records what a provider would report.
  - Physical invocation, and the entry references it hands out.
    `entry_references.rs` owns the
    [control-flow integrity](wiki/spec/build/executable_installation.md#control-flow-integrity)
    gate: `InstalledCode::seal_entry_reference` turns an admitted-entry
    selection into a sealed `InstalledEntryReference` retaining the
    satisfier — the exact installed occurrence and its declared entry — and
    the demanded `EntryContractDigest`, gated on provider-established
    requirement compatibility, instruction-fetch visibility over the entry,
    and demanded `EntryReferenceFactDigest` completion facts. The reference
    borrows `InstalledCode`, so the realization cannot retire while an entry
    remains possible. Still open: no caller invokes installed code through
    one — the provider write-to-execute operation and physical invocation
    remain ahead.
  - The uninstall and replacement joins over that custody, per
    [visibility and retirement](wiki/spec/build/executable_installation.md#visibility-and-retirement):
    visibility before entry, quiescence before retirement, and live-site
    patching through admitted fragments. `uninstall.rs` now owns the
    drain-or-quarantine join — `uninstall_installed` retires a complete drain
    and routes any incomplete drain to `replacement_quarantine.rs` when the
    provider supplies trapping evidence, returning every input when neither
    ending establishes — and `replacement.rs` now owns the patch-then-drain
    replacement join: `replace_installed` demands each patched site be a
    declared entry of the superseded artifact carrying its bound admitted
    fragment, with instruction-fetch visibility and write re-suspension
    established before the superseded custody drains. Still open: no provider
    performs the patching operation itself — the receipts only record what a
    provider would report.
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

  Flag resolved 2026-09-19 on `devin/w9-wire-install-facts` (crate tests 66/66
  pass via `cargo nextest run -p executable-installation`, Linux x86-64):
  `install_validated` now applies the retirement mechanism — `InstallAuthority`
  carries `required_facts` (provider-canonical `InstallationFactDigest`
  values, domain `omega.installation-fact.sha256.v1`) and the receipt's
  `established_facts` must cover them, else the transition rejects and returns
  every input. Next acceptance: a provider operation that actually establishes
  those facts, and physical invocation into installed code.

  Flag resolved 2026-09-19 on `devin/w9-wire-replacement-join` (crate tests
  71/71 pass via `cargo nextest run -p executable-installation`, Linux x86-64):
  `replacement.rs` adds the replacement join — `replace_installed` requires a
  `ReplacementAuthority` scoped to the exact superseded and successor
  realizations, every demanded site to be a declared entry of the superseded
  artifact patched with its bound admitted fragment, and instruction-fetch
  visibility plus write re-suspension plus required `ReplacementFactDigest`
  completion facts; an established patch drains the superseded custody through
  `uninstall_installed` toward retirement or quarantine, and every failed
  transition returns all inputs.

  Flag resolved 2026-09-20 on `devin/w9-wire-install-2` (crate tests 79/79
  pass via `cargo nextest run -p executable-installation`, Linux x86-64):
  the control-flow-integrity gate exists — `seal_entry_reference` replays an
  `EntryReferenceAuthority` scoped to the exact installed occurrence,
  requires the demanded entry to be a declared entry of the installed
  artifact, and requires the receipt to establish requirement compatibility,
  instruction-fetch visibility, and the demanded
  `EntryReferenceFactDigest` completion facts; any refusal returns the
  authority and receipt. `EntryContractDigest` and
  `EntryReferenceFactDigest` are domain-separated provider vocabulary
  (`omega.entry-contract.sha256.v1`, `omega.entry-reference-fact.sha256.v1`).
  Next acceptance: a provider operation that performs the
  write-to-execute/cache-order/fetch-visibility work, a call path consuming
  a sealed entry reference, and an Omega-source route that names an admitted
  artifact.

## Omega-written compiler (after Rust completion)

Finish the Rust [completion contract](wiki/drafts/rust_compiler_completion.md)
before starting the product-language migration. Rust can remain a differential
implementation afterward, but neither Rust agreement nor Rust-specific machinery
is bootstrap authority. Bootstrap construction stays on `TASKS_BOOTSTRAP.md`.

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Establish the production compiler as two
  sibling Omega packages: target-neutral phases under `source/psi/` and the
  Terminal-Psi-consuming product under `source/omega/`, with hosted entrypoints
  at `source/omega/{build.omg,main.omg}`. Two packages rather than two modules
  is settled, not a layout choice: the firewall keeps Psi target-neutral while
  `source/omega/build.omg` selects a provider and binds target roots, and
  [package boundaries](wiki/spec/packages/boundaries.md) make a subsystem
  needing its own dependency-reach set a separate package. The maintained Rust
  compiler is the differential implementation, not source for this task. Work
  backward from complete Omega behavior in small, live vertical slices; do not
  create a bootstrap-private dialect, file allowlist, or parallel
  source-to-native path.
  What exists is a lexer and a partial parser: about 3,600 lines across
  `source/psi/{lex,parse,source,syntax,tokens}/`, the parser gate at
  `source/psi/gates/parser/`, and a 70-line `source/omega/main.omg` that drives
  lexing and parsing over console input. The parser now admits `T in Domain`
  qualified type references on data fields and case payloads —
  `TypeReferenceKind::Qualified` carrying the domain span, OMGPAR7 domain
  columns in the gate observation, three acceptances and six rejections pinned;
  resolution of the domain name remains ahead. `omega --check` on the parser
  gate clears the new states and stops at selected-dispatch service custody
  (`selected ProgramEntry establishment rejoins 0 Terminal attachment
  identities; expected one`), reproduced identically on the pre-slice base, so
  the gate's checked compilation is red before the documented `source_full`
  Unit omission and `test-parser.sh` cannot mint the artifact it runs.
  `omega --check
  source/omega/main.omg` now clears target-profile admission and stops at
  selected-dispatch service custody (`selected ProgramEntry establishment
  rejoins 0 Terminal attachment identities; expected one`) — the same stop
  the parser-gate check reports — observed on Linux x86-64 through
  `cargo run -p omega -- --check source/omega/main.omg`.

  Remaining work:

  - Target-profile recognition (landed at af052a232e): `alpha_bootstrap` is a
    recognized `TargetProfile` reached through the ordinary canonical-name,
    root-slot-owner, and build-case routes; the binding at
    `source/omega/build.omg:11` is unchanged and no parallel bootstrap
    selection exists. An inactive Alpha row demands no realization, unknown
    profile and slot names still reject, and a selected Alpha reports
    "native realization for target profile `alpha_bootstrap` is not
    implemented" from `NativeTarget::from_omega_target_name` — never a
    checked result or artifact; `compiler/tests/alpha_profile_selection.rs`
    pins all three through `compiler::compile`. Rust Alpha emission stays
    out of scope.
  - Std name shadowing. A user declaration spelled like a std one (`ByteRead`,
    `Lexer`) makes std's own machines fail checking. The implicit-case-domain
    capture is fixed: dispatch-arm `Type::Case` classification in
    `symbol-resolved-trees-to-typed-trees`' `exhaustiveness.rs` and domain-fact
    case lookup in `domain_membership.rs` scanned every loaded package by
    spelling, so a consumer's package-private `data ByteRead {}` captured
    `read_line`'s own dispatch as `can fall through`. Both now honor the
    authored-selection `case_type_symbol`/`case_symbol` and `Named.symbol`,
    and spelling fallbacks resolve inside the referencing source's package
    scope (`data_definition_index`). Pinned by `module_machine_indices`
    bare_cases
    `consumer_same_named_data_does_not_capture_dependency_case_dispatch`; a
    `data ByteRead {}` + `console.read_line` repro on linux-x86_64 now clears
    the dispatch and stops at the remaining same-family diagnostics: a
    cross-package `duplicate data ByteRead` check plus `read_line`'s `store`
    overflow and `LinuxX86_64::extent_shape`'s range. The red
    `compiler/tests/calling_policy_plans/macos_entry.rs` tests are the same
    family; that directory is currently claimed by
    OPAQUE-BY-VALUE-BOUNDARY-ABI.
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


## Fuzz findings (coordinator-harvested, swarm wave 9)

New items mined from fuzz legs; each is a confirmed divergence from expected
corpus polarity on origin/main. Claim the named paths under the parent corpus
family and pin the documented behavior with a canary plus the fix if scoped.

- **FUZZ-WEAK-VACANT-QUALIFIER-DROP.** `Weak { e: whole }` where `e` is
  `Extent in Granted` but the field expects `Extent in Granted & Vacant`
  exits 0 — the `Vacant` qualifier drops silently while `Resident` weakening
  rejects. Pin a fail-canary for silent qualifier weakening in Weak.
  Paths: `tests/omega/{pass,fail}/memory/`, weakening checks in
  `omega-rust/psi/semantics/`.

Each fuzz leg's full divergence detail lives in wave-9.outcomes.json entries
with `result: fuzz_report`.


## Mined items (swarm wave 9 mine legs)

Candidates extracted by mine legs from `wiki/drafts/`, `TASKS_OPTIMIZER.md`,
`TASKS_BOOTSTRAP.md`, and `samples/apps/squalr/TASKS.md`, deduplicated by the
coordinator. Each item names its source doc; the mining session's full
`mine_report` verdict is in `build/swarm/w9/wave-9.outcomes.json`. Claim the
named paths, verify the gap is still open on current origin/main (close as
`superseded` if a landing already fixed it), then implement per AGENTS.md
validation scope.

Baseline-failure repairs (source: `wiki/drafts/known_baseline_failures.md`):

- **BASELINE-T2C-INDEXED-OPERAND-ACCESS.** Resolved — superseded on
  `origin/main`. `7ec7ee32e8` routes indexed operand zero through the
  attached-receiver loan (`receiver_self_match` in
  `typed_trees/declarations/operator/indexing.rs`), so
  `machine [] Buffer::index(&self, ..)` admits a `Buffer` place exactly as
  `buffer.at(index)` borrows it, and `b845a7afd7` retains the
  explicit-parameter control (`ordinary_first_parameter_gains_no_receiver_adaptation`
  pins an ordinary `items: &Buffer` first parameter as unresolved at
  `buffer[index]`). Verified: `cargo nextest run -p
  typed-trees-to-checked-trees --lib borrowed_observations` — 8/8 green on
  linux x86-64 at `d05ec39a5d`. The stale `known_baseline_failures.md` row
  remains fenced to the wave's doc owner.
- **BASELINE-VERIFIER-DIGEST-LEDGER.** Resolved — the original drift (twelve digest rows + unregistered `strict_layer.rs`) was already re-recorded on `origin/main` (`d1179e17f6`, `34dc42ee99`, `309b2c5873`). The live residual after `8e2756c5b9` was a stale `proof-admission/src/lib.rs` digest (cited by `formation:module-structure`; revalidated — the diff only declares/re-exports `classicality`, i.e. module structure) plus new file `proof-admission/src/classicality.rs` unregistered under the trusted root; both fixed in `trusted_surface/sites.rs` (digest updated, new `ImplementationSite` with sha256 `6315f5f7…`). `terminal-verifier` trusted_surface suite 9/9 green on linux x86-64 (cargo nextest).
- **BASELINE-CANARY-PASS-CLUSTER.** Four unrepaired pass-canary failures in the canary suite section; triage and repair or retire each with attribution. w9 leg (this wave): `calls/statement_call_recursive_argument_compile` repaired — fixture's `Nat`/`add` collided with `core/nat.omg` exports (added 2026-09-17); renamed to `Peano`/`peano_add`, and the `read_line`/`extent_shape` diagnostics proved collision collateral. `operators/runtime_integer_division_value` repaired — added the missing `build.omg` entry binds and re-scoped operands to `u64` (the realized `ExactIntegerDivide` carrier); signed `i32` exact division remains attributed to the unsigned-quotient/arithmetic-policy lane (t2s fenced by CORPUS-RED-FAMILY this wave). `atomics/atomic_field_declared` GREEN on Linux at 4607987316 — its recorded failure is the macOS hosted-receiver bridge owned by ENTRY-CONTENT-ROOTS; retired here as host-bound. `filesystem/windows_set_file_time_exit` — owned by WINDOWS-SET-FILE-TIME-RESPELL (in flight); doc already names the unsigned-carrier re-spelling. Doc rows in `wiki/drafts/known_baseline_failures.md` were fenced to another wave member; update pending.
- **BASELINE-PACKAGE-COMPILATION-INPUTS.** Two unrepaired failures under `package_compilation_inputs`.
- **BASELINE-NATIVE-DIFF-TERMINAL-PSI-SOURCE.** Three unrepaired failures in the `terminal_psi_source` native-differential lane.
- **BASELINE-NATIVE-DIFF-PIPELINE-OWNERSHIP.** Unrepaired failure in the `pipeline_ownership` native-differential lane.

Language/semantic gaps:

- **FUZZ-CLUSTER-ZERO-BYTE-ARRAY.** Resolved — the empty fixed byte array is a first-class value: `[u8; 0]` admits at check in locals, constants, parameters, returns, record fields, and nested arrays, constructed exactly by `[]` or `""` (`pass/collections/zero_length_byte_array_admission`, `zero_length_byte_array_is_admitted_at_check`). The use-site fences are pinned: no provable index (`x[0]` → "cannot prove index `0` is within length 0"), and fixed-array literals must supply exactly 0 elements/bytes (`fail/data/zero_length_byte_array_{index_rejected,literal_arity_rejected}` and `zero_length_byte_literal_length_rejected`, driven by `zero_length_byte_array_use_fences_reject_at_check`). Non-scalar-leaf `[T; 0]` stays fenced by `InvalidStructuralArrayLength` in the terminal verifier (settled by BASELINE-VERIFIER-ZERO-BYTE-ARRAY-FENCE); a native-route corpus pin for it belongs to the ACTIVE_FAIL roster. Verified: `cargo nextest run -p compiler --test canary_suite` scoped to the two new tests — 2/2 green on linux x86-64 at `ff596a06e6`.
- **CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN.** Dynamic receiver/evidence loan origin across package boundaries.

- **FLOAT-IDENTITY-LITERAL-CARRIER.** Float identity literal carrier semantics.
- **STRUCTURAL-UNIT-LOWERING.** Structural-unit lowering gaps in checked-trees-to-lowered-psi.
- **STRUCTURAL-UNIT-CALL-GRAPH-JOINS.** Call-graph joins for structural units.
- **STAGED-LOCAL-SEQUENCE-LOWERING.** Staged-local sequence lowering attribution and order.
- **TERMINAL-SOURCE-CUSTODY-ORDER.** Terminal source-custody gate ordering.
- **SUCCESSOR-DISCARD-ORDER.** Resolved — same terminal-verifier cleanup-order
  row as EDGE-CLEANUP-ERROR-PRECEDENCE, already repaired on `origin/main`:
  edge validation consumes owned successor sources before the residual and
  trivial discard rosters (`validation/frontier/block_parameters.rs` documents
  the order; `terminators.rs` runs it), and `d96a0fda39` repinned
  `owned_successors_reject_same_arity_aliases_and_transfer_after_disposal` to
  expect `EdgeAffineDiscardsInvalid` — the more precise diagnostic for discard
  evidence naming an already-transferred place. All 26
  `structural_scalar_fields::owned_reads` tests pass at `ff596a06e6`.
- **CANARY-EXACT-ENTRY-SELECTION.** Exact entry selection for division/value canaries and entry binding.

Omega-side / native:

- **X86-FMA-PROVIDER-TRANSPORT.** x86 FMA provider transport (mined by 7 independent legs — highest-consensus gap).
- **I32-REMAINDER-NATIVE-LEGALIZATION.** i32 remainder native legalization.
- **INTEGER-DIVISION-ENTRY-SELECTION.** Integer division entry selection.
- **FLOAT-FMA-NATIVE-TRANSPORT.** Float FMA native transport.

Proof/evidence:

- **PROOF-SEARCH-MEASUREMENT.** Resolved — `check_proof_plan` already tallied obligation mix, certificate-route verdicts, and kernel receipts but discarded them; it now also records emitted-certificate `ProofNode` counts (the draft's storage axis) and whole-run wall-clock microseconds, and `OMEGA_PROOF_MEASUREMENTS` prints one `key=value` line per run for any `omega --check` (e.g. cli_mvp: 772 obligations, 22 certified / 20 uncovered, 148 nodes, 156975 us). Remaining unmeasurable axis is invalidation — needs a store to invalidate — tracked under PROOF-DERIVATION-STORE-INDEX / DERIVATION-RECHECK-CACHE.
- **PROOF-DERIVATION-STORE-INDEX.** Resolved — landed at `68ce33d9de`: `derivation_store.rs` in `psi/semantics/proof` is the lookup substrate `wiki/drafts/proof_search_cache.md` requires — a content arena of untrusted derivation payloads indexed by each obligation's canonical `ProofObligationKey` (BTreeMap, deterministic iteration), generational `DerivationId` handles that can never alias a recycled slot, explicit capacity refusal (`DerivationStoreFull`), and key-granularity `invalidate` since a dependency change changes the key. Lookups return *candidate* evidence the caller re-decides through the admission kernel — never a trusted verdict; consultation inside `check_proof_plan` stays with DERIVATION-RECHECK-CACHE. Verified: `cargo nextest run -p proof --lib` derivation_store suite 6/6 green on linux x86-64.
- **PROOF-OBLIGATION-IDENTITY-KEY.** Semantic identity key for proof obligations.
- **PROOF-INTERCHANGE-IMPORT.** External proof interchange: sort encoding, induction certificate, arithmetic import (3 mined aliases merged).
- **INDUCTIVE-CARRIER-CERTIFICATE.** Inductive carrier certificate production.
  Verified scope: already landed end-to-end — the recursive-component
  certificate is the inductive carrier's base/step/decrease certificate
  ([classicality](wiki/spec/proofs/classicality.md): "a named ranking
  relation, a proved well-foundedness obligation, and a per-edge decrease
  certificate ... the base/step/decrease certificate the matching-logic lane
  calls out"). Production lives in
  `checked-trees-to-lowered-psi/src/proofs/{proof_recursion,control_cycle_proofs}.rs`,
  the wire shape is `terminal-psi/src/artifacts/proof_bundle/recursion.rs`
  (`RecursiveComponentCertificate`/`RecursiveEdgeCertificate`), and admission
  is `proof-admission/src/admission/recursion.rs::verify_recursive_component`
  (artifact/source reconstruction owns the shape; proof bundles discharge it).
  If the mined name instead meant the external matching-logic induction
  certificate, that deliverable is a merged alias of PROOF-INTERCHANGE-IMPORT
  (row above: "sort encoding, induction certificate, arithmetic import") and
  gated behind MATCHING-LOGIC-BOUNDED-SLICE's bounded comparison. No
  independent slice exists here either way.
- **GAMMA-CERTIFICATE-PRODUCTION.** Gamma certificate production + check (includes GAMMA-CERTIFICATE-CHECK).
- **PROOF-RULE-CLASSICALITY-AUDIT.** Audit proof rules for classical/constructive boundary (matching-logic lane). Landed: `wiki/spec/proofs/classicality.md` audits every certificate rule — all are constructive or constructive-by-decidable-domain, `SemanticAxiom` is the only trusted admission, and the proposition grammar cannot express a classical principle. `AcceptedProofRule::foundation` (`proof-admission/src/classicality.rs`) enforces the classification by exhaustive match with tests pinning the boundary. Remaining: classify the obligation-side lemma library in `psi/semantics/proof` and the verifier's semantic-axiom reconstruction inventory.
- **MATCHING-LOGIC-BOUNDED-SLICE.** Bounded matching-logic slice.
- **MATCHING-LOGIC-EXTERNAL-PROOF-IMPORT.** External proof import for matching logic. Source docs `wiki/drafts/matching_logic.md` and `wiki/drafts/matching_logic_sort_encoding.md` are exploratory research notes that authorize no implementation: a checked source proof with a trusted translation still carries a translation admission, an imported statement alone is a foreign-theorem admission, and no independently checked translation exists. Any route first needs the doc's bounded comparison (MATCHING-LOGIC-BOUNDED-SLICE), then a concrete design — a kernel replacement additionally requires its own proposal and an end-to-end proved bridge — and imported rules must respect the constructive/classical boundary `AcceptedProofRule::foundation` enforces (`proof-admission/src/classicality.rs`). The merged interchange territory belongs to PROOF-INTERCHANGE-IMPORT (sort encoding, induction certificate, arithmetic import).
- **DERIVATION-RECHECK-CACHE.** Derivation recheck cache.

Optimizer lane (source: `TASKS_OPTIMIZER.md` + `learned_optimization_policy.md`):

- **LEARNED-OPTIMIZATION-COST-MODEL.** Learned cost model (includes LEARNED-COST-MODEL). Source doc `wiki/drafts/learned_optimization_policy.md` authorizes no implementation, and `wiki/spec/build/optimizations.md` forbids a trainer, training corpus, model evaluator, or inference path in the Rust reference compiler — a premature trainer was already removed (`55ba7f6ab3`). GRAPH-COST-MODEL-STUDY recorded the ranking question's seam (`wiki/drafts/graph_cost_model_study.md`): the validated-candidate, feature-projection, and replay substrate exists; the missing evidence is a versioned workload corpus and measured comparison against the `predicted_cost_delta` baseline, gated on WORKLOAD-CORPUS-AND-MULTIVERSIONING plus the Omega-written product compiler (OMEGA-PRODUCT-COMPILER-SOURCE) with a concrete justification. The doc's own predecessor leg is the model-free BOUNDED-OPTIMIZATION-SEARCH.
- **BOUNDED-OPTIMIZATION-SEARCH.** Bounded candidate search + revalidation at scale (merges BOUNDED-CANDIDATE-SEARCH, CANDIDATE-REVALIDATION-AT-SEARCH-SCALE). Source doc `wiki/drafts/learned_optimization_policy.md` authorizes no implementation: a bounded model-free search over the validated candidate interface and any search-scale revalidation-cost study are far-future extensions gated on the Omega-written product compiler (OMEGA-PRODUCT-COMPILER-SOURCE) plus a concrete justification. The seam such a search would plug into already exists — every pass runs bounded on all five `OptimizationWorkBudget` axes (iterations, rule evaluations, candidates, validation steps, commits), `choose_baseline` selects deterministically over independently validated `ValidatedCandidateSummary` rows with duplicate-candidate rejection and a strictly decreasing convergence measure per commit, `pass_manager/external_policy` replays explicitly supplied decisions on exact context and row equality, candidates are revalidated against the exact input revision they bind (`validate_psi_rewrite_candidate` per iteration; `CandidateContractAxis::Input` rejects stale-input candidates), and `AnalysisManager::commit_revision(validate_retained)` cold-recomputes every supposedly retained analysis and fails `UndeclaredInvalidation` on drift — so revalidation at scale is the existing behavior's measured cost, not a missing mechanism. The versioned workload corpus and measurement protocol its evaluation needs are gated under WORKLOAD-CORPUS-AND-MULTIVERSIONING, with the comparison protocol scoped in `wiki/drafts/graph_cost_model_study.md`.
- **GRAPH-COST-MODEL-STUDY.** Study whether typed operation/state graph
  features can rank optimization candidates before expensive measurement
  (source: `wiki/drafts/learned_optimization_policy.md`; the policy surface
  in `abstract-operations-to-abstract-operations`' `pass_manager/external_policy`
  already projects only independently validated candidates). Study recorded in
  `wiki/drafts/graph_cost_model_study.md`: the structural substrate exists —
  `ValidatedCandidateSummary`/`ExternalCandidateFeatures` rows, the
  `predicted_cost_delta` baseline in `pass_manager/baseline.rs`, and decision
  recording with replay-on-equality. The missing evidence is measured
  prediction improvement over that scalar cost delta on a versioned workload
  corpus. Remaining legs: workload-corpus instrumentation (see
  WORKLOAD-CORPUS-AND-MULTIVERSIONING), a deterministic graph-feature
  projection joined under the versioned decision schema, and the empirical
  comparison itself, which belongs to the Omega-written product compiler per
  the spec's nonauthoritative-policy clause.
- **GENERAL-CYCLIC-EXECUTION-OPTIMIZER.** General cyclic execution optimizer.
  Verified scope: the name re-mines the optimizer half of
  **GENERAL-CYCLIC-EXECUTION** — TASKS_OPTIMIZER.md carries the same-named item
  owning the post-Terminal stages (receiving graph, native selection, replay)
  starting at admitted cyclic input, and this board's TASKS.md:1491 item owns
  the Psi half. The richer item already enumerates the remaining work:
  [ranked callees on projected
  receivers](wiki/spec/language/termination.md#ranked-callees-on-projected-receivers)
  needs composed argument references, call/return, cleanup, callee measure
  checking, and composed resource evidence beyond today's whole-entry-only
  admission — an extend-the-common-graph item, not a new optimizer. No
  independent slice exists here; the implementing surfaces are also already
  under live claims (checked side `execution/unit/state_graph`+`composed_control`
  held by GENERAL-CYCLIC-EXECUTION's live claim; projected-receiver joins on the
  native side overlap `abstract-operations-to-target-operations/src/lowering/control_flow`
  and `execution/unit/receiver_calls` held by sibling claims). Sibling re-mines
  of this surface: RANKED-PROJECTED-RECEIVER-COMPOSITION.
- **WORKLOAD-CORPUS-AND-MULTIVERSIONING.** Workload corpus + multiversing (workload versioning under multiple specialization variants). Source doc `wiki/drafts/learned_optimization_policy.md` authorizes no implementation: these are far-future extensions gated on the Omega-written product compiler (OMEGA-PRODUCT-COMPILER-SOURCE) plus a concrete justification; the versioned workload surface that exists today is BENCHMARKS' `tools/benchmark` records, and specialization-variant identity is SPECIALIZED-VARIANT-IDENTITY-IMPACT's question.
- **SPECIALIZED-VARIANT-IDENTITY-IMPACT.** Identity impact of specialized
  variants — resolved as an answered analysis question.
  `wiki/drafts/specialized_variant_identity_impact.md` records the verdict and
  its claims verified current at `36ffc8af87`/friends: unit identity digests
  complete canonical content (`optimization-unit`), the specialization chain
  re-derives candidate/output identities and rejects `StaleCandidateRevision` /
  `OutputIdentityMismatch` (`state_specialization`), `MachineFunctionIdentity`
  still has exactly `Source | ProgramStorageEntryWrapper | CallbackThunk`
  (function-identity), fragment emission digests the plan (machine-code), and
  object-file symbol lookup fails closed on duplicates. No variant function
  kind exists and none is authorized — a variant-emitting pass remains gated on
  the learned-optimization/workload surface (WORKLOAD-CORPUS-AND-MULTIVERSIONING
  names the same gate). The doc's remaining questions (which specialization
  coordinates a variant kind binds; private symbol naming for variants;
  variant-aware replacement compatibility) are design inputs for that future
  pass, not a bounded leg.

Build/packages:

- **BUILD-PACKAGES-GATE.** RC build-and-packages gate closure work.
- **DELTA-EXHAUSTION-ATTRIBUTION.** Delta compiler exhaustion attribution.
-- **BETA-ENCODER-DEFINITION-PACKAGE.** Beta encoder definition package.
  Resolved at `6e8dd6fa33`: the emitted 116,900-byte package (sha256
  `6bbdd15a…`) is committed as `bootstrap/proofs/beta_encoding/
  definition_package.bin`, bound by `require_beta_encoding_definition_package_
  identity` in `tools/bootstrap/proofs/sources_env.sh`, with
  `tests/bootstrap/proofs-identity.sh` covering the canonical identity check,
  a one-byte-corruption refusal, and README record pins — the owner fixes the
  exact bytes independently of the certificate producer as ACCEPTANCE.md
  requires. The encoding certificate itself is BETA-ENCODING-CERTIFICATE-
  PRODUCTION's surface, not this item's.
- **BETA-RECONSTRUCTION-REFUSAL.** Beta reconstruction refusal. Landed: every
  Beta compiler-exec entrypoint refuses (exit 2) on hosts that cannot run the
  audited Alpha container — `tests/beta/compiler/reconstruction.sh`,
  `compiler-diamond.sh`, and `tools/bootstrap/beta/build.sh`; bound-identity
  refusals stay with `tests/bootstrap/beta-identity.sh`, malformed-source and
  publication refusals with `word-prefix.sh` and the Darwin
  `register-address-regression.sh`. Remaining: none at the shell surface —
  reconstructed-proposition mutation controls belong to the
  GAMMA-DERIVATION-CHECKER certificate acceptance.

Platform/cross-host (structurally gated — document host limits):

- **MACOS-X64-HOST-PROFILE.** macOS x86_64 host profile (Intel gap).
  `TargetProfile::MacosX64` catalogues the host (`macos_x86_64` / `MacosX86_64`,
  `NativeTarget` x86-64 + Mach-O), so `host()` resolves there and the profile
  survives into checked admission, which refuses on the missing
  `targets/macos_x86_64` provider package. Remaining legs: the
  `ProgramEntryPhysicalContractPackage::MacosX64` entry contract +
  `targets/macos_x86_64/` source library, the x86-64 Mach-O writer
  (`image_output.rs` refuses `(MachO, X86_64)` today), the `native_hosted_target()`
  cfg arm in `compiler/tests/canary_suite.rs`, and a real
  x86_64-apple-darwin host run.
- **WINDOWS-SET-FILE-TIME-RESPELL.** Windows SetFileTime respell incl. unsigned carrier (merges FILESYSTEM-WINDOWS-FILETIME-RESPELL).
- **ALPHA-SEED-CONTAINER-NATIVE-VALIDATION.** Alpha seed container native
  validation. Landed: `tests/alpha/container.sh` (+ `container.py`), wired as a
  host-free `alpha-beta-edge.sh` leg, validates both committed containers as
  native executables on any Python-3 host — bound identity for the non-host
  seed too (previously only the host-selected seed was bound), PE32+/Mach-O
  structure, entry in executable code, required loader imports/code signature,
  and the recorded hole offset equal to the tape section's raw extent on
  pristine, stamped, and cross-stamped copies. Seed execution still requires
  the native host: macOS arm64 is covered by the conformance gates, Windows
  x64 by ALPHA-WINDOWS-CONFORMANCE-HOST.
- **ALPHA-WINDOWS-CONFORMANCE-HOST.** Alpha Windows conformance on a Windows
  host. The edge gate's provenance leg now runs the committed forge
  (`tools/bootstrap/alpha/forge.py --check`) on any Python-3 host, including
  non-executing ones — verified on Linux x86-64. Remaining acceptance is the
  seed-execution legs of `tests/bootstrap/alpha-beta-edge.sh` and
  `tests/alpha/reference/diamond-py.sh` on Windows x64 (Git Bash + Python 3):
  `tests/alpha/io-registers.hex` must exit 0 with stdout `ABCDEF` for input
  `AB`, retaining exact bounds/Trap observations and register preservation
  through host I/O.
- **EPOCH-RESOURCE-SNAPSHOTS.** Check epoch-attributed aggregate/resource snapshots
  against the authoritative live-era roster in `effects::ComponentEraEntryLedger`
  and `external-roots` program-local accounting. Acceptance: snapshots match the
  live era's retained resources; foreign or stale era attribution rejects.
  Deployment journals and restart reconstruction belong to the consuming runtime,
  not this task; see [deployment ownership](wiki/spec/build/component_publication.md#deployment-ownership).
- **SHARED-MAPPING-REVOCATION.** Shared-mapping revocation and hostile shared-memory placement/remapping.
- **DEVICE-EXTENT-ACCESS.** Device extent access.
- **EXTERNAL-DATA-SCHEMA-CONVERSION.** External data schema conversion.

Squalr app lane (source: `samples/apps/squalr/TASKS.md`):

- **SQUALR-CLI-COMMANDS.** Squalr CLI commands.
- **SQUALR-ALIGNMENT-STRING-PARSING.** Alignment string parsing.
- **SQUALR-CLONE-SERIALIZATION-PARITY.** Clone serialization parity.
- **SQUALR-GEOMETRY-PARITY.** Geometry parity gaps + debug assertions.
- **SQUALR-NAMED-TRAIT-OPERATORS.** Named trait operators.
- **SQUALR-REGION-ALIGNMENT-EXPANSION.** Region alignment expansion.
- **SQUALR-SEED-PARITY.** Seed parity.
- **SQUALR-TARGETS-AND-THROUGHPUT.** Targets and throughput.


## Mined items (deep-mine sweep, wave 9)

- **AARCH64-BRANCH-RELAXATION** — mined candidate; verify scope then implement.
- **ABI-LAYOUT-REDERIVATION-AUDIT.** Audit that every retained ABI plan
  coordinate on `ForeignCallRelocation` is re-derived at image custody:
  `object_artifact/call_sites.rs` re-derives the call facet via
  `evaluate_call_plan` but never re-derives `boundary_entry_plan.state`;
  compare the retained state facet to the ordinary-entry re-derivation.
- **ALIGNMENT-STRING-PARSING** — mined candidate; verify scope then implement.
- **ALPHA-SEED-MEMSIZE-NATIVE-VALIDATION** — mined candidate; verify scope then implement.
- **ALPHA-SEED-WINDOWS-X64-EXECUTION** — mined candidate; verify scope then implement.
- **ALPHA-WINDOWS-CONFORMANCE** — mined candidate; verify scope then implement.
- **ARTIFACT-AUTHORITY-CHECKS** — mined candidate; verify scope then implement.
- **ASM-CATALOG-FAMILY-EXPANSION** — mined candidate; verify scope then implement.
- **ASM-CATALOG-MEMORY-AND-CONTROL** — mined candidate; verify scope then implement.
- **ASM-HIDDEN-EXIT-AND-MEMORY-CONTRACTS** — mined candidate; verify scope then implement.
- **ASM-INSTRUCTION-CATALOG-EXPANSION** — mined candidate; verify scope then implement.
- **ASM-MEMORY-AND-TRANSFER-CONTRACTS.** Mined candidate; verify scope then implement.
- **ASM-PRIVILEGED-SERVICE-ADMISSION** — mined candidate; verify scope then implement.
- **ATOMICS-ORDERING-EVENT-MODEL** — mined candidate; verify scope then implement.
- **ATTACHED-UNIT-CLOSURE-PLAN** — mined candidate; verify scope then implement.
- **BACKEND-RUNTIME-STARTUP-ENTRY-MECHANICS.** Done: free Unit entries now emit process adapters on linux_x86_64 (`call` + exit_group(231)) and linux_arm64 (`bl` + exit_group(94)) — previously `hosted_unit_entry::prepare` returned `Ok(None)`, leaving ELF `e_entry` pointing at a semantic function that `ret`s into no kernel continuation (process faults). Final-image validation decodes the adapter and checks e_entry→file-bytes round-trip for both targets. Witness: `linux_free_unit_entry_runs_and_completes_with_status_zero` compiles, publishes, and executes the emitted ELF to exit 0 on linux/x86-64; linux_arm64 cross-emission validated on-host (no QEMU harness). Host acceptance on macOS/Windows/QEMU remains a host leg.
- **BACKEND-RUNTIME-STARTUP-MECHANICS** — mined candidate; verify scope then implement.
- **BACKEND-STARTUP-ENTRY-MECHANICS** — mined candidate; verify scope then implement.
- **BACKEND-VOCABULARY-REJECTION-AUDIT** — mined candidate; verify scope then implement.
- **BASELINE-CHECKED-LOWERED-PSI-CLUSTERS** — mined candidate; verify scope then implement.
- **BASELINE-SERVICE-CARRIER-FAILURES** — mined candidate; verify scope then implement.
- **BASELINE-T2C-BOUNDARY-BYTE-BUFFER-REPAIR** — mined candidate; verify scope then implement.
- **BASELINE-T2C-PROVIDER-ATTACHMENT-AND-RESULTS** — mined candidate; verify scope then implement.
- **BASELINE-VERIFIER-CLEANUP-DIAGNOSTIC-ORDER** — mined candidate; verify scope then implement.
- **BENCHMARK-COMPARISON-OCCURRENCE-GATE** — mined candidate; verify scope then implement.
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

  Remaining: three compile-only records already measured at f2f39039da
  (compile_time_ms ~23.8k/24.7k/28.2k, code_size_bytes 1024/16640/8192,
  runtime_ms skipped, `validate` clean) sit ready on the wave-w9 z57
  machine at `~/bench-records-z57/` — copy into
  `tools/benchmark/records/` and commit when the fence clears; refresh
  the host-row matrix (`benchmark.py matrix` into benchmarks.md —
  claimed by BENCHMARK-HOST-ROW-MATRIX). Fence note: `tools/benchmark`
  was held by BENCHMARK-COMPARISON-OCCURRENCE-GATE until
  2026-09-20T22:09:59Z during the z57 leg — records commit under it.
- **BENCHMARK-COMPILE-ONLY-SUBJECTS** — mined candidate; verify scope then implement.
- **BENCHMARK-COMPILE-UNBLOCK-COMPARISON-OCCURRENCES.** Resolved — re-mines
  the producer fix named in BENCHMARK-COMPILE-ONLY-ROWS: the integer
  comparison-occurrence rejection that blocked benchmark subject compiles
  after e48558bd41 ("unify direct call result custody") was repaired at
  76dc49a99e ("distinguish selected comparison custody from builtin
  operations"). `compilation-report`'s `terminal_product::integer_comparisons`
  now counts selected integer occurrences only against the artifact-bound
  checked scope — ordinary builtin comparisons and generated guards need no
  provider row, and explicit `BooleanNot` consumers no longer turn selected
  equality into selected inequality. Independent witness on Linux x86-64 at
  6b610300e8: `cargo nextest run -p compiler --test
  integer_comparison_publication` —
  `selected_comparison_publication_preserves_complete_custody_among_builtins`
  PASS (19.8s). The downstream evidence lives in the sibling row:
  `wrapping_square_sum` compiles and publishes on windows_x86_64,
  macos_arm64 and linux_arm64 at f2f39039da. Sibling re-mines of the same
  resolution: BENCHMARK-COMPARISON-OCCURRENCE-GATE,
  BENCHMARK-STD-COMPARISON-OCCURRENCE-GATE,
  COMPARISON-OCCURRENCE-PRODUCER-COVERAGE, and the
  INTEGER-COMPARISON-OCCURRENCE-* family — the residual surface they name
  (provider coverage for genuinely selected occurrences, std-wide
  verification) is what the publication test pins.
- **BENCHMARK-CROSS-HOST-ROWS** — mined candidate; verify scope then implement.
  Verified scope: re-mines the host-row matrix's runtime legs in
  [wiki/drafts/benchmarks.md](wiki/drafts/benchmarks.md#host-row-matrix) —
  one committed `tools/benchmark/records/` row per catalogued deployment
  profile measured on its own host (linux_arm64 host, macos_arm64,
  windows_x86_64 [peak-RSS leg stays `unavailable` — no `os.wait4`],
  uefi_x86_64 under QEMU/hardware). None is producible on a linux_x86_64
  build host; the local legs are compile-only rows owned by
  **BENCHMARK-COMPILE-ONLY-ROWS** (three measured records already sit at
  `~/bench-records-z57/` on the w9 machine pending commit). The producing
  surfaces are under live claims: `tools/benchmark` is held by
  BENCHMARK-COMPARISON-OCCURRENCE-GATE (expires 22:09Z) and
  `benchmarks.md` + `tools/tests/test_benchmark.py` by
  BENCHMARK-HOST-ROW-MATRIX (22:11Z). A new `linux_x86_64` measured row
  additionally needs the post-f2f39039da compile frontier re-verified on a
  runnable subject. Sibling re-mine names: BENCHMARK-CROSS-TARGET-COMPILE-LEGS,
  BENCHMARK-CROSS-TARGET-COMPILE-ROWS, BENCHMARK-LINUX-ARM64-ROW,
  BENCHMARK-LINUX-X64-ROW-REFRESH, BENCHMARK-MACOS-ARM64-ROW,
  BENCHMARK-PRIME-COUNTER-ROW, BENCHMARK-HOST-ROW-MATRIX.
- **BENCHMARK-CROSS-TARGET-COMPILE-LEGS** — mined candidate; verify scope then implement.
- **BENCHMARK-CROSS-TARGET-COMPILE-ROWS** — mined candidate; verify scope then implement.
- **BENCHMARK-DEPEND-FREE-RUNNABLE-SUBJECT** — mined candidate; verify scope then implement.
- **BENCHMARK-HOST-ROW-MATRIX** — mined candidate; verify scope then implement.
- **BENCHMARK-LINUX-X64-ROW-REFRESH** — mined candidate; verify scope then implement.
- **BENCHMARK-MACOS-ARM64-ROW** — mined candidate; verify scope then implement.
- **BENCHMARK-MEASURABLE-SUBJECT-CORPUS** — mined candidate; verify scope then implement.
- **BENCHMARK-PRIME-COUNTER-ROW** — mined candidate; verify scope then implement.
- **BENCHMARK-PROOF-SUBJECT-CALL-SELECTION** — mined candidate; verify scope then implement.
- **BENCHMARK-PROOF-SUBJECT-CHECKED-CALL-SELECTION** — mined candidate; verify scope then implement.
- **BENCHMARK-PROOF-SUBJECT-SELECTION** — mined candidate; verify scope then implement.
- **BENCHMARK-REJECTED-ROW-RECORDING** — mined candidate; verify scope then implement.
- **BENCHMARK-ROW-RESUMPTION** — mined candidate; verify scope then implement.
- **BENCHMARK-SELECTION-CONTRAST-ROWS** — mined candidate; verify scope then implement.
- **BENCHMARK-SELECTION-ISOLATION-ROWS** — mined candidate; verify scope then implement.
- **BENCHMARK-SELECTION-MATRIX** — mined candidate; verify scope then implement.
- **BENCHMARK-SELECTION-ROW-COVERAGE** — mined candidate; verify scope then implement.
- **BENCHMARK-SELECTION-ROW-MATRIX** — mined candidate; verify scope then implement.
- **BENCHMARK-SELECTION-VARIANT-ROWS** — mined candidate; verify scope then implement.
- **BENCHMARK-STANDALONE-SUBJECT** — mined candidate; verify scope then implement.
- **BENCHMARK-STD-COMPARISON-OCCURRENCE-GATE** — mined candidate; verify scope then implement.
- **BENCHMARK-SUBJECT-CORPUS-EXPANSION** — mined candidate; verify scope then implement.
- **BENCHMARK-SUBJECT-ROW-EXPANSION** — mined candidate; verify scope then implement.
- **BENCHMARK-UEFI-ROW** — mined candidate; verify scope then implement.
- **BENCHMARK-WINDOWS-PEAK-MEMORY** — mined candidate; verify scope then implement.
- **BENCHMARK-WINDOWS-PEAK-RSS** — mined candidate; verify scope then implement.
- **BETA-COMPILER-SEED-REFUSAL** — mined candidate; verify scope then implement.
- **BETA-ENCODING-CERTIFICATE-CHECK** — mined candidate; verify scope then implement.
- **BETA-ENCODING-CERTIFICATE-PRODUCTION** — mined candidate; verify scope then implement.
- **BETA-ENCODING-DEFINITION-PACKAGE** — mined candidate; verify scope then implement.
- **BETA-ENCODING-MUTATION-REJECTION.** Full-subject mutation controls for the Beta encoding certificate per `bootstrap/proofs/beta_encoding/ACCEPTANCE.md`: through the checked-in Stepper, mutate each covered element (source, tape, assertions, theory identity, rule ids/clauses, substitution, premises, sorts, arities, partition joints, endpoints, final-root selection) and require rejection — invalid or exhausted input must not accept after a valid prefix. Harness lives in `tests/gamma/beta-encoding-theory/` as a new run.sh mode using the existing full-subject derivation.
- **BETA-ENCODING-NATIVE-CONTAINER-ACCEPTANCE** — mined candidate; verify scope then implement.
- **BETA-ENCODING-SELECTED-CHAIN-PRODUCTION** — mined candidate; verify scope then implement.
- **BETA-NATIVE-SELF-RECONSTRUCTION** — mined candidate; verify scope then implement.
- **BETA-PE-SEED-REFUSAL** — mined candidate; verify scope then implement.
- **BETA-SEED-EXEC-HOST-REFUSAL** — mined candidate; verify scope then implement.
- **BETA-SEED-REFUSAL-ON-UNSUPPORTED-HOST** — mined candidate; verify scope then implement.
- **BOOTSTRAP-CHAIN-NATIVE-EXECUTION** — mined candidate; verify scope then implement.
- **BOOTSTRAP-EPSILON-EVALUATOR** — mined candidate; verify scope then implement.
- **BOOTSTRAP-HOST-COVERAGE** — mined candidate; verify scope then implement.
- **BOOTSTRAP-MACOS-ARM64-LEGS** — mined candidate; verify scope then implement.
- **BOOTSTRAP-OMEGA-D-COMPILER** — scope verified 2026-09-20: this mined stub
  re-covers **OMEGA-D** in [TASKS_BOOTSTRAP.md](TASKS_BOOTSTRAP.md), the real
  tracked item completing the Epsilon closure
  `bootstrap/5_omega/omega_compiler.epsilon.sources` as the first full Omega
  compiler (currently 16,152 lines / 8 members: representations, request/UTF-8,
  lexical classification, lexer, parser, Alpha tape, scalar compilation,
  outcome). No independent slice exists here: OMEGA-D was live-claimed by
  Zergling-112 (lease expiring 2026-09-20T22:27Z) at verification time, its
  bounded legs are the separate OMEGA-D-* board rows below, and its acceptance
  (interpreted D compiles the Omega C closure to `omega0_compiler_bytecode.tape`)
  additionally depends on OMEGA-PRODUCT-COMPILER-SOURCE (C is currently a
  lexer plus partial parser under `source/psi/`). Sibling re-mines of the same
  OMEGA-D clauses: OMEGA-D, OMEGA-D-COMPILER-REQUEST-TABLES,
  OMEGA-D-ENTRY-ADAPTER, OMEGA-D-ENTRY-ADAPTER-RETIREMENT,
  OMEGA-D-REAL-ENTRY-ROUTE, OMEGA-D-REQUEST-ADMISSION,
  OMEGA-D-REQUEST-AND-ENTRY-ROUTE, OMEGA-D-REQUEST-AND-SCALAR-COMPILATION,
  OMEGA-D-REQUEST-OUTCOME-TABLES.
- **BOOTSTRAP-SEED-EXECUTION-HOSTS** — mined candidate; verify scope then implement.
- **BOUNDED-CANDIDATE-SEARCH** — mined candidate; verify scope then implement.
- **BUILD-DEPEND-PURPOSE-AWARE-LOCKS** — mined candidate; verify scope then implement.
- **BUILD-DIR-ALIAS-AND-RACE-COLLISION-DETECTION** — mined candidate; verify scope then implement.
- **BUILD-DIR-ALIAS-RACE-DETECTION** — mined candidate; verify scope then implement.
  Verified scope: re-mines the race-window residual the landed
  **BUILD-DIR-HOST-ALIAS-COLLISIONS** row already assigns to
  **BUILD-DIR-ALIAS-AND-RACE-COLLISION-DETECTION** — an alias created
  between admission's `overlap_key` check and the first write (e.g. a
  symlink planted inside the window) is invisible to the spelling-level
  fence in `build-evaluation/src/evidence/filesystem_scope.rs`. The
  implementing surfaces are under live claims: request/options admission
  (BUILD-DIR-ALIAS-AND-RACE-COLLISION-DETECTION, expires 22:14Z),
  `filesystem_scope.rs` (BUILD-DIRECTORY-ALIAS-COLLISION, 23:07Z),
  `build-output` (BUILD-DIRECTORY-HOST-ALIAS-RACE-COVERAGE, 23:09Z) and
  `filesystem_scope/preparation.rs` (FILESYSTEM-SNAPSHOT-ISOLATION,
  22:28Z). Sibling re-mine names: BUILD-DIRECTORY-ALIAS-COLLISION,
  BUILD-DIRECTORY-HOST-ALIAS-RACE-COVERAGE,
  BUILD-DIRECTORY-HOST-ALIAS-RACE-ISOLATION, HOST-ALIAS-BUILD-DIR-DETECTION,
  REQUEST-BUILD-DIRECTORY-HOST-ALIAS-COVERAGE.
- **BUILD-DIR-HOST-ALIAS-COLLISIONS** — landed. Root-overlap admission now
  compares spelling-independent keys (`overlap_key` folds `.`/`..`,
  absolutizes relative spellings, resolves the longest existing prefix
  through the host's canonical spelling incl. symlinked ancestors), so a
  `..`/relative/symlinked `snapshot_dir` can no longer alias the source
  root or build write root past the lexical `starts_with` fence, and
  `ensure_write_roots` rejects a `--build-dir` that names the source root
  or one of its ancestors (a covering write root would make read-only
  sources writable). Nested-inside-source default remains admitted.
  The named-input residual also landed (`f0f902d6ef`): each named input's
  read root derives as `<snapshot>.input-N` — a sibling spelling the bind
  fence cannot see — and `ensure_write_roots` now replays the
  spelling-independent `roots_overlap` check against every bound named
  input after inventory binding, so `--build-dir` spelled exactly
  `<snapshot>.input-N` (or a descendant) rejects instead of aliasing that
  input's read-only root; pinned by
  `build_write_root_rejects_alias_of_a_named_input_snapshot` and
  `captured_source_scope_rejects_host_alias_spellings_of_the_roots` (both
  green at `164abfbbdb`). Race-window aliases (symlink created between
  admission and first write) remain the sibling
  BUILD-DIR-ALIAS-AND-RACE-COLLISION-DETECTION scope.
- **BUILD-DIRECTORY-ALIAS-COLLISION** — mined candidate; verify scope then implement.
- **BUILD-DIRECTORY-HOST-ALIAS-RACE-COVERAGE** — mined candidate; verify scope then implement.
- **BUILD-DIRECTORY-HOST-ALIAS-RACE-ISOLATION** — mined candidate; verify scope then implement.
- **C2L-BOUNDARY-BYTE-BUFFER-FAILURES** — mined candidate; verify scope then implement.
- **C2L-FAILURE-TRIAGE** — mined candidate; verify scope then implement.
- **C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES** — mined candidate; verify scope then implement.
- **C2L-UNATTRIBUTED-FAILURE-TAIL** — mined candidate; verify scope then implement.
- **CANARY-ACQUIRES-THROUGH-HELPER-RETURN** — mined candidate; verify scope then implement.
- **CANARY-CORE-NAME-COLLISION** — mined candidate; verify scope then implement.
- **CANARY-DUPLICATE-OVERLOAD-DECLARATIONS** — mined candidate; verify scope then implement.
- **CANARY-NATIVE-WRAPPER-WRITE-ALL-RESULT** — mined candidate; verify scope then implement.
- **CANARY-RUNTIME-GUI-FOREGROUND-WINDOW-EXIT** — mined candidate; verify scope then implement.
- **CANARY-RUNTIME-LITERAL-DISPATCH-EXIT** — mined candidate; verify scope then implement.
- **CANARY-WIRE-EXACT-ARRAY-WITHOUT-COUNT-EXIT** — mined candidate; verify scope then implement.
- **CANDIDATE-REVALIDATION-AT-SEARCH-SCALE** — mined candidate; verify scope then implement.
- **CATHEDRAL-PORTABLE-PROTOCOL-VERIFICATION** — mined candidate; verify scope then implement.
- **CERTIFICATE-ADMISSION-RECORD-BINDING.** Mined candidate — resolved:
  records already bind to exact bytes, and substitution rejects. Verified
  both surfaces at `43176cdc92`: trust-model `TrustAdmission` digests
  `omega.trust-admission.v1` + subject domain + commitment + the underlying
  subject's strong digest (provider-plan/native-evidence bytes), and
  `from_persisted` re-derives the digest from a persisted record rather than
  trusting it; the PCC certificate's native-evidence rows are recomputed by
  the receiver against the actual artifact bytes (region/gap digests,
  addresses, fingerprints, inventory seals, closed-form import-thunk
  re-derivation), so a record authored for one body fails on another — a
  specialized variant cannot inherit the unspecialized body's admission
  record, matching `learned_optimization_policy.md`'s contract clause.
  Sibling stub CERTIFICATE-ADMISSION-RECORD covers the same row family.
- **CHAIN-GATE-LOCAL-PREFIX-BINDING** — mined candidate; verify scope then implement.
- **CHAIN-MANIFEST** — mined candidate; verify scope then implement.
- **CHAIN-MANIFEST-D-OCREQ-REQUEST-BINDING** — mined candidate; verify scope then implement.
- **CHAIN-MANIFEST-GATE-LOCAL-PREFIX-BINDING** — mined candidate; verify scope then implement.
- **CHAIN-MANIFEST-GATE-LOCAL-PREFIX-PACKING** — mined candidate; verify scope then implement.
- **CHAIN-MANIFEST-GATE-LOCAL-PREFIXES** — mined candidate; verify scope then implement.
- **CHAIN-MANIFEST-GATE-PREFIX-BINDING** — mined candidate; verify scope then implement.
- **CHAIN-MANIFEST-OCREQ-BINDING** — mined candidate; verify scope then implement.
- **CHAIN-OCREQ-ENTRY-BINDING** — mined candidate; verify scope then implement.
- **CHECKED-CALL-SELECTION-OCCURRENCE-MATH-PROOFS** — mined candidate; verify scope then implement.
- **CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION** — mined candidate; verify scope then implement.
- **CHECKED-TREES-TO-LOWERED-PSI-UNATTRIBUTED-SET** — mined candidate; verify scope then implement.
- **CHECKER-PROVISION-NATIVE-VALIDATION** — mined candidate; verify scope then implement.
- **CLI-COMMANDS** — mined candidate; verify scope then implement.
- **COMMON-ROUTE-REJECTION-INVENTORY** — mined candidate; verify scope then implement.
- **COMPARE-TEST-SELECTION** — mined candidate; verify scope then implement.
- **COMPARISON-OCCURRENCE-PRODUCER-COVERAGE** — mined candidate; verify scope then implement.
- **COMPILER-BATCH-MANIFEST** — mined candidate; verify scope then implement.
- **COMPILER-EXECUTABLE-PUBLICATION-OPERATION.** Resolved — the compiler's
  executable publication operation exists and is the CLI's only route to
  visible bytes. `omega` compilation stops at the in-memory semantic product;
  `omega/src/compilation/publication.rs` (`publish_compilation` /
  `publish_native_artifact`) is the product-owned operation that then calls
  `CompileReport::publish_retained_native_artifact`
  (`compilation-report/src/compile_report.rs`), which validates the retained
  artifact and manifest, refuses non-local output filenames, requires
  compiler-text and compiler-function validation evidence, self-checks any
  requested PCC pair before a byte installs, and commits one staged tree +
  atomic rename through `executable_publication.rs` so a failed publish
  leaves no half-written executable or stale sidecar;
  `publish_completed_build_outputs` then writes companions and
  `checked_native_executable_path` returns the receipt-bound executable.
  `output_kind` gating matches the spec's report/entry-bridge rule: native
  output requires publication custody, object output has no executable
  receipt, check-only has neither. Coverage:
  `build_target_activation::activation_identifiers_and_publication`,
  `production_manifest_custody`, and the `executable_publication.rs` unit
  tests. The distinct installed-component route is also landed:
  `component-deployment::flat_output::publish_component_flat_output`
  implements component_publication.md's visible-component contract
  (installation replay, sealed bytes + executable mode staging, atomic
  rename, visible-file replay, runnable custody returned on failure) for
  `component_publication::InstalledRunnableComponent` eras — deliberately
  not the CLI's carrier.
- **COMPILER-OBSERVATION-OUTPUTS** — mined candidate; verify scope then implement.
- **COMPILER-OBSERVATION-PRODUCTS** — mined candidate; verify scope then implement.
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
- **COMPILER-PASS-PROFILE-INSTRUMENTATION** — mined candidate; verify scope then implement.
- **COMPILER-PASS-PROFILE-TIMINGS** — mined candidate; verify scope then implement.
- **COMPILER-PASS-PROFILING** — mined candidate; verify scope then implement.
- **COMPOSABLE-PAIR-DESCRIPTORS** — mined candidate; verify scope then implement.
- **COMPUTED-CONSTANT-LEAF-CARRIER** — mined candidate; verify scope then implement.
- **CONCURRENCY-COMPOSITION-EXTRACTION.** Resolved — mined alias of CONCURRENT-PROTOCOL-EXTRACTION's deferred surface; its authorization gate applies (spec defers whole-composition extraction until a concrete protocol or safety-profile customer needs it).
- **CONCURRENT-COMPOSITION-EXTRACTION.** Resolved — mined alias of CONCURRENT-PROTOCOL-EXTRACTION's deferred surface; its authorization gate applies (spec defers whole-composition extraction until a concrete protocol or safety-profile customer needs it).
- **CONCURRENT-PROTOCOL-COMPOSITION-EXTRACTION** — mined candidate; verify scope then implement.
- **CONCURRENT-PROTOCOL-EXTRACTION** — mined candidate; scope verified, authorization gate recorded. Its source surface authorizes no implementation: `wiki/spec/language/concurrency.md` §protocol-proofs states "This extraction remains deferred, not implicit authority supplied by a bounded search or a proposed graph format," and `wiki/language_guide/chapter_18_concurrency.md` §Concurrent Protocol Model defers whole-composition extraction "until a concrete protocol or safety-profile customer needs it." Activation requires such a customer plus the sealed erased model (activation creation/bounds, resource identities, wait/wake edges, priorities, placement, selected provider premises) consumed by ordinary proof machines. Sibling stubs naming the same deferred surface: CONCURRENT-PROTOCOL-COMPOSITION-EXTRACTION, CONCURRENT-PROTOCOL-WHOLE-COMPOSITION, CONCURRENT-WHOLE-COMPOSITION-EXTRACTION, CONCURRENCY-COMPOSITION-EXTRACTION, CONCURRENT-COMPOSITION-EXTRACTION.
- **CONCURRENT-PROTOCOL-WHOLE-COMPOSITION** — mined candidate; scope verified, authorization gate recorded. Same deferred surface as CONCURRENT-PROTOCOL-EXTRACTION: `wiki/spec/language/concurrency.md` §protocol-proofs states "This extraction remains deferred, not implicit authority supplied by a bounded search or a proposed graph format," and `wiki/language_guide/chapter_18_concurrency.md` §Concurrent Protocol Model defers whole-composition extraction — this stub's exact subject — "until a concrete protocol or safety-profile customer needs it." Activation requires such a customer plus the sealed erased model (activation creation/bounds, resource identities, wait/wake edges, priorities, placement, selected provider premises) consumed by ordinary proof machines. Sibling stubs naming the same deferred surface: CONCURRENT-PROTOCOL-EXTRACTION (gate recorded), CONCURRENT-PROTOCOL-COMPOSITION-EXTRACTION, CONCURRENT-WHOLE-COMPOSITION-EXTRACTION, CONCURRENCY-COMPOSITION-EXTRACTION, CONCURRENT-COMPOSITION-EXTRACTION.
- **CONCURRENT-WHOLE-COMPOSITION-EXTRACTION** — mined candidate; verify scope then implement.
- **CONNECTED-ROUTE-GATE** — mined candidate; verify scope then implement.
- **CONST-GENERIC-EXTENT-RANGE-DISCHARGE** — mined candidate; verify scope then implement.
- **CONST-GENERIC-INFERRED-EXTENT-RANGE** — mined candidate; verify scope then implement.
- **CONSTANT-LEAF-EXACT-CARRIER** — mined candidate; verify scope then implement.
- **CONSTRUCTIVE-REAL-FOUNDATIONS** — mined candidate; verify scope then implement.
- **COORDINATOR-OVEROWNERSHIP-AUDIT** — mined candidate; verify scope then implement.
- **CRATE-ROOT-RESPONSIBILITY-AUDIT** — mined candidate; verify scope then implement.
- **CROSS-COMPILER-DIFFERENTIAL** — mined candidate; verify scope then implement.
- **CROSS-COMPILER-DIFFERENTIAL-LANE.** Mined candidate; scope verified at
  `36ffc8af87`. The lane is the cross-check between the maintained Rust
  compiler and the Omega-written product compiler: `omega-rust/README.md`
  retains the Rust producer "for cross-compiler bug finding" and
  OMEGA-PRODUCT-COMPILER-SOURCE declares it "the differential
  implementation". The lane's only present entrypoint is
  `source/psi/test-parser.sh`: it rebuilds the Omega-written parser gate
  (`source/psi/gates/parser/main.omg`) through the freshly built Rust CLI
  (`OMEGA_CLI`/`OMEGA_TARGET`) and hands the minted `omega-program` artifact
  to `source/psi/parse/test_parser.py` for black-box parser-slice
  observations. The lane is red upstream of itself: the Rust check of both
  the parser gate and `source/omega/main.omg` stops at selected-dispatch
  service custody — "selected ProgramEntry establishment rejoins 0 Terminal
  attachment identities; expected one"
  (`omega-rust/omega/build/selected-dispatch/src/service_custody/root.rs`),
  the recorded OMEGA-PRODUCT-COMPILER-SOURCE frontier — so test-parser.sh
  cannot mint the artifact it drives. The stop is pinned behavior for
  unattached Service entries
  (`compiler/tests/source_evaluated_native_realization/linux_dynamic_realization.rs`);
  a `--check` attempt at this revision ran past 240 s without reaching a
  diagnostic, consistent with the ~6,500 s gate-check cost recorded under
  the owner item. No second compiler implementation can produce artifacts
  yet, so no deeper lane harness can exist; the omega0↔omega self-compile
  differential is separately tracked under TASKS_BOOTSTRAP's OMEGA-C.
  Remaining: none inside this row — it unblocks only when
  OMEGA-PRODUCT-COMPILER-SOURCE clears the service-custody gate; sibling
  rows CROSS-COMPILER-DIFFERENTIAL and
  RUST-OMEGA-CROSS-COMPILER-DIFFERENTIAL name the same scope.
- **CROSS-PACKAGE-DYNAMIC-LOAN-ORIGIN** — mined candidate; verify scope then implement.
- **CTTL-FAILURE-ATTRIBUTION** — mined candidate; verify scope then implement.
- **CUSTODY-MATRIX-HARNESS-MIGRATION** — mined candidate; verify scope then implement.
- **CUSTODY-MUTATION-COVERAGE** — mined candidate; verify scope then implement.
- **D-DIAGNOSTIC-ENTRY-ADAPTER-REPLACEMENT** — mined candidate; verify scope then implement.
- **D-OCREQ-ENTRY-BINDING** — mined candidate; verify scope then implement.
- **D-REQUEST-ADMISSION-ROUTE** — mined candidate; verify scope then implement.
- **D-REQUEST-OUTCOME-TABLE-PARITY** — mined candidate; verify scope then implement.
- **D-SCALAR-OPERATION-CLOSURE** — mined candidate; verify scope then implement.
- **DELTA-COMPILER** — mined candidate; verify scope then implement.
- **DELTA-EPSILON-CLOSURE-ACCEPTANCE.** Scope verified at `54d5dc1cb1`: the
  alias names the Delta→Epsilon evaluator closure's *acceptance* surface,
  and that surface is already landed and green — `evaluator_env.sh`
  materializes the canonical packed closure behind bound identity checks,
  `tests/bootstrap/epsilon-identity.sh` refuses corrupted
  manifest/member/driver/entry/receipt shapes, and
  `tests/bootstrap/epsilon-source-closure` + `source-closure.sh` pin the
  exact source manifest against repacking. The unclosed legs of the same
  Delta→Epsilon path — compiling the bound evaluator through the bound
  Delta compiler and executing it — live under `tests/epsilon` (DELTA-COMPILER
  claim, expires ~21:48Z) and require a seed-execution host (macOS arm64 or
  Windows x64). No unowned in-fence slice; this row records the resolution.
- **DELTA-EPSILON-CLOSURE-COMPILE.** Mined candidate; verify scope then implement.
- **DELTA-EPSILON-CLOSURE-EXECUTION** — mined candidate; verify scope then implement.
- **DELTA-EVALUATOR-EXHAUSTION-TRIAGE** — mined candidate; verify scope then implement.
- **DELTA-POST-FRONTEND-ALLOCATION-PROBE** — mined candidate; verify scope then implement.
- **DELTA-POST-FRONTEND-ALLOCATION-STRESS** — mined candidate; verify scope then implement.
- **DEPENDENCY-FREE-BENCHMARK-SUBJECT** — mined candidate; verify scope then implement.
- **DEPENDENCY-FREE-MEASURABLE-SUBJECT** — mined candidate; verify scope then implement.
- **DEPENDENCY-FREE-RUNTIME-BENCHMARK-SUBJECT** — mined candidate; verify scope then implement.
- **DEPENDENT-RELATIONAL-PROOF-SUPPORT.** Resolved — the mined sentence
  (chapter_12: "implementation support for relational proofs and views
  remains narrower") named a real fence: `unsigned_increase_fits` admitted
  composed relational ceilings only for `u64`, so `requires
  self.count < self.cap` proved `self.count += 1` for u64 yet rejected the
  same shape for u8..u32 even when the ceiling operand's own carrier was
  bounded inside the result primitive. `ordered_values::composed_ceiling_gap`
  now carries a ceiling-admission predicate and `operand_carrier_bound`
  resolves the composed ceiling operand's declared bound (literal value,
  place carrier range, binary primitive, usize length); the
  `ResultRepresentable` gate consults the relational route at every integer
  width with the result's `range.high` as the admitted ceiling. The u64 lane
  is unchanged (`None` ceiling admits every operand). Pinned by
  `guard_narrowing::tests::a_strict_place_ceiling_proves_the_increment_for_narrower_carriers`
  (u8/u32/range-declared ceilings prove `+1`, two-hop `count < mid < cap`
  proves `+2`; non-strict, unbound, unrelated, and wider-carrier ceilings
  stay rejected). The chapter sentence is rewritten to state the checked
  contract: strict relational bounds discharge representability through the
  ceiling's carrier at every width; equality through writes and
  solver-general proofs remain the named residual. Sibling rows
  DEPENDENT-RELATIONAL-PROOF-VIEW-SUPPORT / DEPENDENT-RELATIONAL-PROOFS-VIEWS
  / DEPENDENT-VALUES-CHECKER-COVERAGE are re-mines of the same sentence and
  remain for their own slices.
- **DEPENDENT-RELATIONAL-PROOF-VIEW-SUPPORT** — mined candidate; verify scope then implement.
- **DEPENDENT-RELATIONAL-PROOFS-VIEWS** — mined candidate; verify scope then implement.
- **DEPENDENT-VALUES-CHECKER-COVERAGE** — mined candidate; verify scope then implement.
- **DERIVATION-STORE-SEMANTIC-INDEX** — mined candidate; verify scope then implement.
- **DIFFERENTIAL-FRONTEND-DROP-EXPECTATIONS** — mined candidate; verify scope then implement.
- **DIFFERENTIAL-STAGED-LOCAL-SEQUENCE** — mined candidate; verify scope then implement.
- **DIVISION-CANARY-ENTRY-BINDING** — mined candidate; verify scope then implement.
- **DIVISION-VALUE-ENTRY-SELECTION** — mined candidate; verify scope then implement.
- **DOMAIN-REFINEMENT-CHAINS** — mined candidate; verify scope then implement.
- **DUPLICATE-NAMED-MACHINE-OVERLOAD** — mined candidate; verify scope then implement.
- **DUPLICATE-OVERLOAD-RESOLUTION** — mined candidate; verify scope then implement.
- **DURABLE-CODEC-EXTRACTION** — mined candidate; verify scope then implement.
- **DURABLE-CODEC-RELOCATION.** Mined candidate. Upstream: the
  [PIPELINE-OWNER-CONSOLIDATION](TASKS_OPTIMIZER.md) remaining-work bullet in
  TASKS_OPTIMIZER.md: "Move durable codecs out of transforms and coordinators
  with their consuming stage changes: `post_allocation_manifest/codec` and
  `rewrites/allocation_recovery/fixed_view_copy/codec` in the two selected
  stages, and `optimized_semantic_wrapper_object/codec` in `native-realization`
  (see `PIPELINE-OWNER-CONSOLIDATION` for whether that owner survives)."
  Durable artifact encoders/decoders belong beside their durable types in
  `omega-rust/omega/representations/` (precedent: `register-homes`'s
  `register_homes/codec.rs`, `physical-instructions`'s `codec/`,
  `optimization-unit`'s `ledger/codec/`); transforms keep private working
  state, not self-authenticating artifact codecs.

  Verified scope (origin/main ff596a06e6): three codec sites —
  `selected-instructions-to-register-homes/src/assignment/post_allocation_manifest/codec.rs`
  (V9 `PostAllocationOptimizationManifest` codec),
  `selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/codec/`
  (V35 `FixedViewCopyPlan` codec, ~3k lines incl. tests), and
  `native-realization/src/optimized_semantic_wrapper_object/codec.rs`
  (container codec whose encode calls `validate_object`). Their consumers and
  the architecture-gate entrance tables in
  `tests/architecture/optimizer_source_organization/` (protocols.rs,
  requirements/executable/selection_allocation.rs, pipeline_native.rs)
  update in the same lane.

  Remaining: land in dependency order — (1) move the durable identity newtypes
  the manifest references that still live inside transforms
  (`FixedViewCopyIdentity`, `LiteralFoldIdentity`,
  `PressureRematerializationIdentity`, ~10 consumer files each) into their
  representations homes; (2) move `post_allocation_manifest` model+codec into
  `representations/register-homes` (its other field types already live there or
  in `optimization-core`/`selected-instructions`); (3) move
  `fixed_view_copy/codec` with `FixedViewCopyPlan` into its representations
  home — `VirtualFixedConstraintSite` and sibling analysis types it references
  decide whether `representation-selections` hosts it or a new area is named;
  (4) `optimized_semantic_wrapper_object/codec` moves only after
  PIPELINE-OWNER-CONSOLIDATION resolves whether native-realization retains
  that owner — skip it while undecided.
- **DYNAMIC-CALL-OCCURRENCE-SPANS** — mined candidate; verify scope then implement.
- **DYNAMIC-CALL-PHYSICAL-EVIDENCE** — mined candidate; verify scope then implement.
- **DYNAMIC-DISPATCH-ROW-MAPS** — mined candidate; verify scope then implement.
- **DYNAMIC-RETURN-LOAN-ORIGIN** — mined candidate; verify scope then implement.
- **EDGE-CLEANUP-DIAGNOSTIC-ORDER** — mined candidate; verify scope then implement.
- **EDGE-CLEANUP-ERROR-PRECEDENCE.** Resolved — alias of the terminal-verifier cleanup-order row already repaired on `origin/main`: edge validation consumes owned successor sources before the residual and trivial discard rosters (`validation/frontier/block_parameters.rs` documents the order; `terminators.rs` runs it), and `d96a0fda39` repinned `owned_successors_reject_same_arity_aliases_and_transfer_after_disposal` to expect `EdgeAffineDiscardsInvalid` — the more precise diagnostic for discard evidence naming an already-transferred place. The test plus all 26 `structural_scalar_fields::owned_reads` tests pass at `ff596a06e6` on linux x86-64 (`cargo nextest run -p terminal-verifier`). Sibling aliases of the same row (EDGE-CLEANUP-DIAGNOSTIC-ORDER, FRONTIER-EDGE-*, OWNED-SUCCESSOR-*, STRUCTURAL-SUCCESSOR-DISCARD-ORDERING, VERIFIER-EDGE-CLEANUP-PHASE-ORDER, SUCCESSOR-DISCARD-ORDER) remain separate stubs.
- **EFI-MATRIX-PROMOTION** — mined candidate; verify scope then implement.
- **ENCODER-CANDIDATE-CONTINUATION** — mined candidate; verify scope then implement.
- **ENCODER-DEFINITION-PACKAGE** — mined candidate; verify scope then implement.
- **ENTRY-MECHANICS-RUNTIME-CONSOLIDATION** — mined candidate; verify scope then implement.
- **ENTRYPOINT-MODULE-LAYOUT-GATE** — mined candidate; verify scope then implement.
- **EPOCH-AGGREGATE-SNAPSHOTS** — mined candidate; verify scope then implement.
- **EPSILON-BOOTSTRAP-CHAIN** — mined candidate; verify scope then implement.
- **EPSILON-EVALUATOR-BOOTSTRAP-PATH.** Scope verified at `83f5477357` — the mined alias names the Delta→Epsilon evaluator bootstrap path. Its in-fence surface is complete and green on Linux x86-64: `tests/bootstrap/epsilon-identity.sh` materializes the exact packed evaluator closure and refuses corrupted manifest/member/driver/entry/receipt shapes, and the source-closure gate checks the fixture plus the canonical `epsilon_compiler.delta.sources` manifest. The remaining legs — compiling the bound evaluator plus `execution_driver.delta` through the bound Delta compiler and executing the evaluator's entries — live under `tests/epsilon`, currently claimed by DELTA-COMPILER, and seed execution needs an audited host (macOS arm64 or Windows x64, none available here). Sibling stubs on the same path: BOOTSTRAP-EPSILON-EVALUATOR, DELTA-EPSILON-CLOSURE-COMPILE, DELTA-EPSILON-CLOSURE-EXECUTION, DELTA-EPSILON-CLOSURE-ACCEPTANCE, EPSILON-BOOTSTRAP-CHAIN.
- **EPSILON-SCALAR-COMPILATION-EXTENSION** — mined candidate; verify scope then implement.
- **EXACT-PROGRAM-ENTRY-MULTIPLICITY.** Verified on `main`: entry
  multiplicity is already enforced end to end — `admission/selection.rs`
  requires exactly one binding per required catalog slot, exactly one
  ProgramEntry-schema root (a defensive arm while every profile catalog is a
  singleton), a non-generic machine with at most one provisioned `&mut self`,
  and `root_bindings.rs` rejects a re-bound slot or ambiguous implementation;
  integration coverage lives in `build_target_activation`. Landed unit pins
  for the previously unexercised edges: empty root bindings keep the
  migration fallback, an unqualified slot spelling rejects, malformed or
  unknown-profile foreign rows still reject beside a valid selection, and the
  binding walk collects every malformed row's diagnostic in order.
- **EXECUTABLE-PUBLICATION** — mined candidate; verify scope then implement.
- **EXECUTABLE-PUBLICATION-JOIN** — mined candidate; verify scope then implement.
- **EXECUTABLE-PUBLICATION-OPERATION** — mined candidate; verify scope then implement.
- **EXECUTABLE-PUBLICATION-STAGE** — mined candidate; verify scope then implement.
- **EXECUTABLE-PUBLICATION-STEP** — mined candidate; verify scope then implement.
- **FAULT-INJECTED-TARGET-READER** — mined candidate; verify scope then implement.
- **FILESYSTEM-SNAPSHOT-ISOLATION** — mined candidate; verify scope then implement.
- **FINITE-GENERIC-METHOD-FAMILIES** — mined candidate; verify scope then implement.
- **FIXED-ARRAY-ZERO-EXTENT-FENCE** — mined candidate; verify scope then implement.
- **FLOATING-MATCH-SUBJECTS.** Resolved — the mined row re-covered a stale
  limitation note, not missing work. Floating Match subjects are implemented
  end-to-end: `IeeeFloatCompare` (six explicit relations) is retained through
  Terminal, the checked interpreter, and native publication, with NaN and
  signed-zero behavior pinned by `expressions/match_float_interpretation`
  (named/indexed subjects, literal and parameter patterns) and
  `expressions/match_float_patterns` (call subjects and call patterns). This
  leg extended the pin to projected record-field and computed-expression
  subjects via the new `expressions/match_float_subjects` canary and corrected
  the two stale "Boolean/integer subjects" sentences in
  `omega-rust/psi/pipeline/README.md`. Witnessed residual (not float-specific):
  a domain-carried subject (`f32 in Temps`) against plain scalar patterns
  rejects "match pattern is incompatible with its subject" — identical for
  `i64 in Ranks`, so it belongs to the general subject/pattern compatibility
  frontier under MATCH-SELECTIVE-LOWERING, not to this row. Still open on the
  floating match surface (tracked in
  `omega-rust/omega/compiler/compiler/float_realization.md`): crash-qualified
  equality and checked-adapter Match execution.
- **FMA-PROVIDER-PIPELINE-TRANSPORT** — mined candidate; verify scope then implement.
- **FMA-PROVIDER-TRANSPORT** — mined candidate; verify scope then implement.
- **FRONTEND-DROP-CUSTODY-ORDER-REPIN** — mined candidate; verify scope then implement.
- **FRONTIER-EDGE-DIAGNOSTIC-ORDER** — mined candidate; verify scope then implement.
- **FRONTIER-EDGE-ERROR-ORDER** — mined candidate; verify scope then implement.
- **FRONTIER-EDGE-ERROR-ORDERING** — mined candidate; scope verified, resolved — named alias of the terminal-verifier cleanup-order row already repaired on `origin/main`: edge validation consumes owned successor sources before the residual and trivial discard rosters (`validation/frontier/block_parameters.rs` documents the order; `terminators.rs` runs it), and `d96a0fda39` repinned `owned_successors_reject_same_arity_aliases_and_transfer_after_disposal` to expect `EdgeAffineDiscardsInvalid`. EDGE-CLEANUP-ERROR-PRECEDENCE's landed annotation already names `FRONTIER-EDGE-*` stubs among the row's aliases; VERIFIER-EDGE-CLEANUP-PHASE-ORDER carries the same landed resolution.
- **GAMMA-CERT-CHAIN-PRODUCTION** — mined candidate; verify scope then implement.
- **GAMMA-CERT-FULL-CHECK** — mined candidate; verify scope then implement.
- **GAMMA-CERT-NATIVE-CONTAINERS** — mined candidate; verify scope then implement.
- **GAMMA-CERTIFICATE-CHECK** — mined candidate; verify scope then implement.
- **GAMMA-CERTIFICATE-CHECK-UNDER-PROFILE** — mined candidate; verify scope then implement.
- **GAMMA-CERTIFICATE-CHECKING** — mined candidate; verify scope then implement.
- **GAMMA-CERTIFICATE-NATIVE-ACCEPTANCE** — mined candidate; verify scope then implement.
- **GAMMA-CERTIFICATE-NATIVE-CHECK** — mined candidate; verify scope then implement.
- **GAMMA-CERTIFICATE-NATIVE-EXECUTION** — mined candidate; verify scope then implement.
- **GAMMA-CONTAINER-NATIVE-ACCEPTANCE** — mined candidate; verify scope then implement.
- **GAMMA-DERIVATION-CHECKER** — mined candidate; verify scope then implement.
- **GAMMA-NATIVE-CERTIFICATE-ACCEPTANCE** — mined candidate; verify scope then implement.
- **GAMMA-NATIVE-CONTAINER-ACCEPTANCE** — mined candidate; verify scope then implement.
- **GAMMA-PROVISION-NATIVE-ACCEPTANCE** — mined candidate; verify scope then implement.
- **GATE-LOCAL-DRIVER-PREFIX-BINDING** — mined candidate; verify scope then implement.
- **GATE-LOCAL-PREFIX-BINDING** — mined candidate; verify scope then implement.
- **GENERAL-LICM** — mined candidate; verify scope then implement.
- **GENERAL-RELOCATION-ADMISSION** — mined candidate; verify scope then implement.
- **GENERAL-SCHEDULE-RELOCATION** — mined candidate; verify scope then implement.
- **GENERAL-SOURCE-BINDER-SYNTAX.** Resolved — scope verified: the general mathematical binder surface (`let`/`boundary let` telescopes, `core::Level`/`core::Type<u>`/`core::Strict<v>`/`core::Squash` carriers, generalized and authored universe binders, arrow-typed telescope parameters, named assumptions) already landed under the PROOF-CONTRACT-MIGRATION structural legs; the in-fence residual was the bounded machine-valued body denotation in `typed-trees-to-checked-trees/src/proof`. Extended it: `x != y` now denotes `Squash (Not (Id S l r))` through an interned `Not : Π(_ : Type 0). Type 0` assumption — kept at `Type 0`, not `sEmpty` elimination, so inequality composes inside `&&`/`||` like `==` — and `()` interned a dedicated `Unit : Type 0` carrier, so unit binder domains and unit-carried calls denote instead of refusing. Remaining named legs stay with their owners: `core::*` symbol-identity classification (blocked on the fixed `core::*` declarations landing in `source/library/core`), checked-signature encoding into Terminal evidence, member-call `target_symbol` binding inside `let` bodies, and order relations over non-integer operands. Gate on linux x86-64: `cargo check`/`clippy -p typed-trees-to-checked-trees` clean of new warnings; `cargo nextest run -p typed-trees-to-checked-trees` 5008/5009 — `open_range_token_use_rejects_instead_of_falling_back` fails verbatim at base `d82697ffca` (unrelated wave breakage).
- **GENERATED-CODEC-INDEPENDENT-VERIFICATION** — mined candidate; verify scope then implement.
- **GENERIC-DYNAMIC-FAMILY-DISPATCH** — mined candidate; verify scope then implement.
- **GENERIC-RETURNED-VIEW-LIFETIMES** — mined candidate; verify scope then implement.
- **GENERIC-VIRTUAL-CALLS** — mined candidate; verify scope then implement.
- **GENERIC-VIRTUAL-DISPATCH** — mined candidate; verify scope then implement.
- **GEOMETRY-ALIGNMENT-PARSING** — mined candidate; verify scope then implement.
- **GEOMETRY-ALIGNMENT-REGIONS** — mined candidate; verify scope then implement.
- **GEOMETRY-ALIGNMENT-STRING-PARSING** — mined candidate; verify scope then implement.
- **GEOMETRY-CLONE-SERIALIZATION** — mined candidate; verify scope then implement.
- **GEOMETRY-DEBUG-ASSERTIONS** — mined candidate; verify scope then implement.
- **GEOMETRY-NAMED-TRAIT-OPERATORS** — mined candidate; verify scope then implement.
- **GEOMETRY-NATIVE** — mined candidate; verify scope then implement.
- **GEOMETRY-PARITY** — mined candidate; verify scope then implement.
- **GEOMETRY-REGION-ALIGNMENT-EXPANSION** — mined candidate; verify scope then implement.
- **GEOMETRY-WINDOWS-LEG** — mined candidate; verify scope then implement.
- **GEOMETRY-WINDOWS-REVALIDATION** — mined candidate; verify scope then implement.
- **GEOMETRY-WINDOWS-VALIDATION** — mined candidate; verify scope then implement.
- **GLOB-SELF-IMPORTS-REPAIR** — mined candidate; verify scope then implement.
- **GRAPH-COST-EVIDENCE-CORPUS** — mined candidate; scope verified, authorization gate recorded. Re-mines WORKLOAD-CORPUS-AND-MULTIVERSIONING's corpus leg of GRAPH-COST-MODEL-STUDY (a versioned workload corpus is the missing evidence for the `predicted_cost_delta` comparison). Source doc `wiki/drafts/learned_optimization_policy.md` authorizes no implementation: the corpus is a far-future extension gated on the Omega-written product compiler (OMEGA-PRODUCT-COMPILER-SOURCE) plus a concrete justification, and `wiki/spec/build/optimizations.md` forbids trainer-side machinery in the Rust reference compiler. The versioned workload surface that exists today is BENCHMARKS' `tools/benchmark` records; the comparison protocol is scoped in `wiki/drafts/graph_cost_model_study.md`. Sibling stubs on the same gated surface: OPTIMIZATION-WORKLOAD-CORPUS, WORKLOAD-CORPUS.
- **GRAPH-FEATURE-PROJECTION-SCHEMA** — mined candidate; verify scope then implement.
- **HOST-ALIAS-BUILD-DIR-DETECTION** — mined candidate; verify scope then implement.
- **HOSTED-INLINE-ASSEMBLY-AUTHORITY** — mined candidate; verify scope then implement.
- **HOSTED-PLATFORM-RUN-MATRIX** — mined candidate; verify scope then implement.
- **HOSTED-RECEIVER-SERVICE-CARRIER** — mined candidate; verify scope then implement.
- **HOSTILE-SHARED-MEMORY-PLACEMENT** — mined candidate; verify scope then implement.
- **HOSTILE-SHARED-MEMORY-REMAPPING** — mined candidate; verify scope then implement.
- **INDEXED-OPERAND-ATTACHED-RECEIVER** — mined candidate; verify scope then implement.
- **INDEXING-ATTACHED-RECEIVER-BORROW** — mined candidate; verify scope then implement.
- **INLINE-ASSEMBLY-CATALOG-EXPANSION** — mined candidate; verify scope then implement.
- **INTEGER-COMPARISON-OCCURRENCE-PRODUCER** — mined candidate; verify scope then implement.
- **INTEGER-COMPARISON-OCCURRENCE-PRODUCER-COVERAGE** — mined candidate; verify scope then implement.
- **INTEGER-COMPARISON-OCCURRENCE-RETENTION** — mined candidate; verify scope then implement.
- **INTEGER-COMPARISON-OCCURRENCE-STD-COVERAGE** — mined candidate; verify scope then implement.
- **INTEL-MACOS-HOST-PROFILE** — mined candidate; verify scope then implement.
- **INTERNAL-PASS-PROFILE-TIMINGS** — mined candidate; verify scope then implement.
- **INTRINSIC-PHYSICAL-SPAN-ARMS** — mined candidate; verify scope then implement.
- **KNOWN-BASELINE-FAILURES-DOC-REFRESH** — mined candidate; verify scope then implement.
- **KNOWN-BASELINE-FAILURES-REFRESH** — mined candidate; verify scope then implement.
- **LEARNED-COST-MODEL** — mined candidate; verify scope then implement.
- **LEGACY-COMPATIBILITY-WRAPPER-PRUNING** — mined candidate; verify scope then implement.
- **LIFETIME-MULTI-SOURCE-AND-OUTLIVES** — mined candidate; verify scope then implement.
- **LIFETIME-SOURCE-CORRESPONDENCE** — mined candidate; verify scope then implement.
- **LOOKUP-MAP-JUSTIFICATION** — mined candidate; verify scope then implement.
- **LOOKUP-MAP-MEASUREMENT-AUDIT** — mined candidate; verify scope then implement.
- **LOWERED-BOUNDARY-BYTE-BUFFER-FAILURES** — mined candidate; scope verified, family repaired. `wiki/drafts/known_baseline_failures.md`'s own re-reading at d8d48fe4ff records the boundary-byte-buffer group of checked-trees-to-lowered-psi as green now (with crash-member byte entries and the ordered-boolean row); this stub mines a failure family that no longer fails. The live residual families in that crate are already owned: the bare boundary-trait fixture spelling by ENTRY-CONTENT-ROOTS, and the remaining scalar-return custody / provider attachment / attached-unit sets by C2L-BASELINE-FAILURE-ATTRIBUTION and C2L-RESIDUAL-FAILURE-ATTRIBUTION. No independent slice remains on this row.
- **LOWERED-CRASH-MEMBER-BYTE-ENTRIES** — mined candidate; verify scope then implement.
- **LOWERED-OPERATION-PROOF-MACHINE-CALLS** — mined candidate; verify scope then implement.
- **LOWERED-PSI-BASELINE-TAIL** — mined candidate; verify scope then implement.
- **LOWERED-SCALAR-RESULT-SOURCE-CUSTODY** — mined candidate; verify scope then implement.
- **LOWERED-UNIT-FAILURE-ATTRIBUTION** — mined candidate; verify scope then implement.
- **MATCHING-LOGIC-COMPARISON-METRICS** — mined candidate; verify scope then implement.
- **MATCHING-LOGIC-SLICE-COMPARISON** — mined candidate; verify scope then implement.
- **MATCHING-LOGIC-TYPED-TO-ONE-SORTED-ENCODING.** Verified scope
  (Zergling-181): the "typed-to-one-sorted encoding" bullet of the bounded
  comparison in `wiki/drafts/matching_logic.md`, drafted in
  `wiki/drafts/matching_logic_sort_encoding.md` — encode `Nat`, `Int`,
  `addr`, slices, and a user sum into the one-sorted finitary basic fragment
  as membership patterns with disjointness/refinement clauses, definedness
  preconditions for partial operations, junk-model quantifier guards,
  revision (refinement-not-invalidation) rules, and borrow/multiplicity loan
  clauses. Landed (queued on `zergling/z181-...` pending mainline):
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
- **MATCHING-LOGIC-VERTICAL-SLICE** — mined candidate; verify scope then implement.
- **MATCHING-LOGIC-VERTICAL-SLICE-COMPARISON** — mined candidate; verify scope then implement.
  Verified scope: the name re-mines the "possible bounded comparison"
  deliverable of `wiki/drafts/matching_logic.md` — one real vertical slice
  (scalar propositions, equality, quantification, a Terminal state
  transition, a reconstructed refinement obligation) compared on checker,
  translation and theory size, certificate size and checking time, and every
  imported rule/assumption/trusted bridge. That deliverable is
  MATCHING-LOGIC-BOUNDED-SLICE's territory, and the doc is an exploratory
  research note authorizing no implementation (per the landed annotation on
  MATCHING-LOGIC-EXTERNAL-PROOF-IMPORT): a comparison still needs a concrete
  design, and any import route additionally needs a checked translation that
  does not exist. The slice-production leg is under live claims —
  MATCHING-LOGIC-BOUNDED-SLICE (Jarod / swarm-w9-matching-logic-bounded-slice)
  and MATCHING-LOGIC-COMPARISON-METRICS (Devin / swarm-w9-bcf3a6cd) — so no
  independent slice exists here. Sibling re-mines of the same doc surface:
  MATCHING-LOGIC-SLICE-COMPARISON, MATCHING-LOGIC-VERTICAL-SLICE,
  MATCHING-LOGIC-TYPED-TO-ONE-SORTED-ENCODING.
- **MATH-PROOFS-CALL-SELECTION-OCCURRENCE** — mined candidate; verify scope then implement.
- **MATH-PROOFS-DECLARATION-SELECTION** — mined candidate; verify scope then implement.
- **MATHEMATICAL-FOUNDATIONS-REAL** — mined candidate; verify scope then implement.
- **MATHEMATICAL-PREDICATE-PARAMETERS** — mined candidate; verify scope then implement.
- **MODEL-FREE-CANDIDATE-SEARCH** — mined candidate; verify scope then implement.
- **MODULE-CONSTANT-BUILTIN-CARRIER** — mined candidate; verify scope then implement.
- **MODULE-CONSTANT-COMPUTED-CARRIER** — mined candidate; verify scope then implement.
- **MULTI-TARGET-BATCH-MANIFEST** — mined candidate; verify scope then implement.
- **NAMED-TRAIT-OPERATORS** — mined candidate; verify scope then implement.
- **NAMESPACE-AWARE-NORMALIZATION** — mined candidate; verify scope then implement.
- **NATIVE-DIFF-CUSTODY-EXPECTATION-RETARGET** — mined candidate; verify scope then implement.
- **NATIVE-DIFF-FRONTEND-DROP-ORDER** — mined candidate; verify scope then implement.
- **NATIVE-DIFF-HOSTED-RECEIVER-CHECKED-ENTRY** — mined candidate; verify scope then implement.
- **NATIVE-DIFF-HOSTED-RECEIVER-HARNESS-MIGRATION** — mined candidate; verify scope then implement.
- **NATIVE-DIFFERENTIAL-MATRIX** — mined candidate; verify scope then implement.
- **NATIVE-I32-REMAINDER-LEGALIZATION** — mined candidate; verify scope then implement.
- **NATIVE-MATRIX-MATCHING-HOSTS** — mined candidate; verify scope then implement.
- **NON-X86-LAYOUT-RELAXATION** — mined candidate; verify scope then implement.
- **OBLIGATION-NORMALIZED-IDENTITY** — mined candidate; verify scope then implement.
- **OCREQ-ENTRY-BINDING** — mined candidate; verify scope then implement.
- **OCREQ-REQUEST-BINDING** — mined candidate; verify scope then implement.
- **OCREQ-REQUEST-ENTRY-BINDING** — mined candidate; verify scope then implement.
- **OMEGA-C** — mined candidate; verify scope then implement.
- **OMEGA-C-SELF-HOST-EDGE** — mined candidate; verify scope then implement.
- **OMEGA-COMPILER-TAPE-BINDING** — mined candidate; verify scope then implement.
- **OMEGA-D** — mined candidate; verify scope then implement.
- **OMEGA-D-COMPILER-REQUEST-TABLES** — mined candidate; verify scope then implement.
- **OMEGA-D-ENTRY-ADAPTER** — mined candidate; verify scope then implement.
- **OMEGA-D-ENTRY-ADAPTER-RETIREMENT** — mined candidate; verify scope then implement.
- **OMEGA-D-REAL-ENTRY-ROUTE** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-ADMISSION** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-AND-ENTRY-ROUTE** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-AND-SCALAR-COMPILATION** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-OUTCOME-TABLES** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-ROUTE-ENTRY** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-TABLE-COMPLETION** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-TABLES** — mined candidate; verify scope then implement.
- **OMEGA-D-REQUEST-V1-TABLES** — mined candidate; verify scope then implement.
- **OMEGA-D-SCALAR-ALPHA-EMISSION** — mined candidate; verify scope then implement.
- **OMEGA-D-SCALAR-COMPILATION** — mined candidate; verify scope then implement.
- **OMEGA-D-SCALAR-EMISSION-EXTENSION** — mined candidate; verify scope then implement.
- **OMEGA-D-SCALAR-OPERATION-FRONTIER** — mined candidate; verify scope then implement.
- **OMEGA-D-SCALAR-SEQUENCING** — mined candidate; verify scope then implement.
- **OMEGA-D-STANDALONE-REQUEST-ROUTE** — mined candidate; verify scope then implement.
- **OMEGA-ENTRY-MANIFEST-BINDINGS** — mined candidate; verify scope then implement.
- **OMEGA-PARSER-GATE-WINDOWS** — mined candidate; verify scope then implement.
- **OMEGA-PARSER-GATE-WINDOWS-ROUTE** — mined candidate; verify scope then implement.
- **OMEGA-PARSER-GATE-WINDOWS-VALIDATION** — mined candidate; verify scope then implement.
- **OMEGA-PARSER-WINDOWS-ROUTE** — mined candidate; verify scope then implement.
- **OMEGA-PARSER-WINDOWS-ROUTE-VALIDATION** — mined candidate; verify scope then implement.
- **OMEGA-WRITTEN-PRODUCT-COMPILER** — mined candidate; verify scope then implement.
- **OMEGA-WRITTEN-PRODUCT-COMPILER-CHAIN** — mined candidate; verify scope then implement.
- **OPTIMIZATION-CATALOG-EXECUTION-ROUTE** — mined candidate; verify scope then implement.
- **OPTIMIZATION-WORKLOAD-CORPUS** — mined candidate; verify scope then implement.
- **OPTIMIZED-SEMANTIC-WRAPPER-DISPOSITION** — mined candidate; verify scope then implement.
- **OPTIMIZED-SEMANTIC-WRAPPER-OWNERSHIP** — mined candidate; verify scope then implement.
- **OPTIMIZED-SEMANTIC-WRAPPER-RELOCATION** — mined candidate; verify scope then implement.
- **OPTIMIZED-WRAPPER-DISPOSITION** — mined candidate; verify scope then implement.
- **OPTIMIZED-WRAPPER-OBJECT-RELOCATION** — mined candidate; verify scope then implement.
- **OPTIMIZER-RULE-AXIS-GATE** — mined candidate; verify scope then implement.
- **ORPHAN-ENTRANCE-AUDIT** — mined candidate; verify scope then implement.
- **ORPHAN-STAGE-ENTRANCE-AUDIT** — mined candidate; verify scope then implement.
- **ORPHAN-STAGE-OUTPUT-AUDIT** — mined candidate; verify scope then implement.
- **OWNED-SUCCESSOR-CHECK-ORDER** — mined candidate; verify scope then implement.
- **OWNED-SUCCESSOR-DISCARD-ORDER** — mined candidate; verify scope then implement.
- **OWNED-SUCCESSOR-EDGE-CLEANUP-ORDER** — mined candidate; verify scope then implement.
- **OWNED-SUCCESSOR-EDGE-ORDERING** — mined candidate; verify scope then implement.
- **PACKAGE-ADMISSION-PROJECTION-EARLIEST-FACTS** — mined candidate; verify scope then implement.
- **PACKAGE-DYNAMIC-RETURN-LOAN-ORIGIN** — mined candidate; verify scope then implement.
- **PACKAGE-EVIDENCE-OPAQUE-USE-ATTRIBUTION** — mined candidate; verify scope then implement.
- **PACKAGE-EVIDENCE-TRAIT-RESOLUTION-SCOPE** — mined candidate; verify scope then implement.
- **PACKAGE-EVIDENCE-TRAIT-SCOPE-COLLISION** — mined candidate; verify scope then implement.
- **PACKAGE-EVIDENCE-TRAIT-UNIQUENESS-OVERCOLLECTION** — mined candidate; verify scope then implement.
- **PACKAGE-INPUTS-COMPUTED-CONSTANT-LEAF** — mined candidate; verify scope then implement.
- **PACKAGE-INPUTS-LOAN-ORIGIN** — mined candidate; verify scope then implement.
- **PACKAGE-INPUTS-PSI-FAILURES** — mined candidate; verify scope then implement.
- **PACKAGE-LOCK-SOURCE-IDENTITY** — mined candidate; verify scope then implement.
- **PACKAGE-PROJECTION-EVIDENCE-MIGRATION** — mined candidate; verify scope then implement.
- **PACKAGE-REVIEW-HOTSPOT-ATTRIBUTION** — mined candidate; verify scope then implement.
- **PACKAGE-REVIEW-ROUTE-ATTRIBUTION** — mined candidate; verify scope then implement.
- **PACKAGE-REVIEW-ROUTE-COST-ATTRIBUTION** — mined candidate; verify scope then implement.
- **PAIR-RULE-DESCRIPTOR-AXES** — mined candidate; verify scope then implement.
- **PARTIAL-OWNERSHIP-CLEANUP-EXPANSION** — mined candidate; verify scope then implement.
- **PASS-CANARY-UNIT-PLAN-CLASS** — mined candidate; verify scope then implement.
- **PER-RULE-AXIS-ENFORCEMENT** — mined candidate; verify scope then implement.
- **PERSISTENT-CHECKED-SOURCE-CACHE** — mined candidate; verify scope then implement.
- **PHYSICAL-ACCESS-PROFILES** — mined candidate; verify scope then implement.
- **PHYSICAL-ENTRY-BRIDGES** — mined candidate; verify scope then implement.
- **PHYSICAL-ENTRY-END-TO-END** — mined candidate; verify scope then implement.
- **PIN-CONNECTED-PIPELINE-ROUTE** — mined candidate; verify scope then implement.
- **PIPELINE-CRATE-SWEEP** — mined candidate; verify scope then implement.
- **PIPELINE-DOC-LINK-DRIFT** — mined candidate; verify scope then implement.
- **PIPELINE-ORPHAN-ELIMINATION** — mined candidate; verify scope then implement.
- **PIPELINE-OWNER-CONSOLIDATION** — mined candidate; verify scope then implement.
- **PIPELINE-PLACEMENT-AUDIT** — mined candidate; verify scope then implement.
- **PIPELINE-REWRITE-CATALOG-WIRING** — mined candidate; verify scope then implement.
- **PIPELINE-REWRITE-ORPHANS** — mined candidate; verify scope then implement.
- **PIPELINE-REWRITES-OWNERSHIP-AUDIT** — mined candidate; verify scope then implement.
- **PIPELINE-ROUTE-CONFORMANCE-AUDIT** — mined candidate; scope verified, resolved by landed audits. `1ccc88fb51` added `tests/architecture/representation_ownership/route_conformance.rs` pinning the documented program route: every route-table owner link resolves inside its named crate, every pipeline crate on disk is owned by exactly one row, and crate/package names keep the X-to-Y shape (the stale `timing_report.rs` link it caught was repointed to `compile_timings/mod.rs`). `36ffc8af87` added the connectivity leg in `tests/architecture/stage_crate_ownership.rs`: every designed stage entrance reachable at its crate root must have a caller outside its own crate, so the executable route — not only the crate-name chain — stays connected. Both halves of the stub's named audit are landed and pinned.
- **PIPELINE-SPILL-FAMILY-ORPHANS** — mined candidate; verify scope then implement.
- **PIPELINE-WRAPPER-OBJECT-ORPHAN** — mined candidate; verify scope then implement.
- **PKG-INPUTS-FLOAT-IDENTITY-LANDING** — mined candidate; verify scope then implement.
- **PLACE-ACCESS-GEOMETRY** — mined candidate; verify scope then implement.
- **PLACE-ALIAS-ANALYSIS-PRODUCER** — mined candidate; verify scope then implement.
- **PLACE-STORAGE-EXTENT-OWNER** — mined candidate; verify scope then implement.
- **PLACED-ACCESS-NATIVE-OPS** — mined candidate; verify scope then implement.
- **PLATFORM-RUN-LINUX-X86-64** — mined candidate; verify scope then implement.
- **POC-NATIVE-WRAPPER-RELOCATION** — mined candidate; verify scope then implement.
- **POC-ORPHAN-ENTRANCE-AUDIT.** Resolved — the
  [stage-entrance orphan audit](wiki/drafts/stage_entrance_orphan_audit.md)
  now covers the post-allocation chain and `unsequenced_spill_stages/`
  family the POC cluster orbits: every post-allocation stage entrance is
  wired (native-realization physical pipeline, machine-emission
  `function_realization`/`exit_contract`, native-differential
  `optimizer_corpus`/layout stages, architecture coordination markers), all
  19 spill-stage modules' entrances are driven by native-differential
  `register_allocation` tests and architecture gates, and the per-stage
  `*_identity` helpers are internally routed plumbing called by their own
  validators and generalized siblings. No orphans found; the one durable
  note for a future automated gate is that spill codec names (`encode`,
  `decode`) collide workspace-wide and need qualified identities to be
  machine-attributable.
- **POC-REWRITE-ORPHANS** — mined candidate; verify scope then implement.
- **POC-SELECTED-REWRITE-CATALOG** — mined candidate; verify scope then implement.
- **POC-SPILL-FAMILY-DISPOSITION** — mined candidate; verify scope then implement.
- **POC-SPILL-FAMILY-SEQUENCING** — mined candidate; verify scope then implement.
- **POC-WRAPPER-OBJECT-PLACEMENT** — mined candidate; verify scope then implement.
- **PORTABLE-PROCESS-EXIT-OBSERVATION** — mined candidate; verify scope then implement.
- **PORTABLE-REVIEW-LOCK** — mined candidate; verify scope then implement.
- **PRIME-COUNTER-BENCHMARK-ROW** — mined candidate; verify scope then implement.
- **PRIME-COUNTER-I32-REMAINDER** — mined candidate; verify scope then implement.
- **PRIME-COUNTER-REMAINDER-LEGALIZATION** — mined candidate; verify scope then implement.
- **PRIVATE-PIPE-RUNTIME-ENFORCEMENT** — mined candidate; verify scope then implement.
- **PRIVATE-PRODUCER-EVIDENCE-LOAN-ORIGIN** — mined candidate; verify scope then implement.
- **PRIVILEGED-PORT-EFFECT-SETTLEMENTS** — mined candidate; verify scope then implement.
- **PRIVILEGED-SERVICE-ASM-ADMISSION** — mined candidate; verify scope then implement.
- **PROCESS-EXIT-PORTABLE-OBSERVATION** — mined candidate; verify scope then implement.
- **PRODUCER-CHECKER-BOUNDARY-AUDIT** — mined candidate; verify scope then implement.
- **PRODUCER-CHECKER-DECISION-SEPARATION** — mined candidate; verify scope then implement.
- **PRODUCER-CHECKER-DECISION-SHARING-AUDIT** — mined candidate; verify scope then implement.
- **PRODUCER-CHECKER-SHARING-AUDIT** — mined candidate; verify scope then implement.
- **PRODUCER-HISTORY-CUSTODY** — mined candidate; verify scope then implement.
- **PROGRAM-ENTRY-SELECTION-DIVISION** — resolved: superseded on `origin/main` (verified 8479b3ab86). Entry selection already divides on every axis the entry-roots contract names: per-profile matrix selection (foreign rows resolve profile/slot ownership then stay out of the durable projection), exactly-one binding per required slot, free vs. provisioned receiver modes, the two-surface semantic/physical calling-plan check, and a closed `TargetRequiredRootSlotDeclaration` kind enum that keeps build-bound `ProgramEntry` divided from runtime-installed root kinds (foreign kinds reject by name). Witnesses: 12/12 `build-evaluation admission::selection::tests`; `OMEGA_PASS_CANARY_FILTER="program_entry,root_binding"` corpus green. Known residual: `tests/omega/fail/build/program_entry_binding_outside_build` expected.txt predates the "compiler-issued &mut Build receiver" diagnostic — fenced by RC-DIAGNOSTICS-GATE at verification time, so repinning belongs to that lane.
- **PROGRAM-ENTRY-SELECTION-EXACTNESS** — mined candidate; verify scope then implement.
- **PROMOTION-REJOIN-EVIDENCE** — mined candidate; verify scope then implement.
- **PROMOTION-ROLLBACK-REJOIN-LEGS** — mined candidate; verify scope then implement.
- **PROOF-AUTOMATION-WIDENING** — mined candidate; verify scope then implement.
- **PROOF-CACHE-DEPENDENCY-INVALIDATION** — mined candidate; verify scope then implement.
- **PROOF-DERIVATION-STORE** — mined candidate; verify scope then implement.
- **PROOF-DERIVED-LOAN-COMPATIBILITY** — mined candidate; verify scope then implement.
- **PROOF-INTERCHANGE-EXTERNAL-ARITHMETIC** — mined candidate; verify scope then implement.
- **PROOF-INTERCHANGE-INDUCTION-CERTIFICATE** — mined candidate; verify scope then implement.
- **PROOF-OBLIGATION-SEMANTIC-IDENTITY** — mined candidate; verify scope then implement.
- **PROOF-OBLIGATION-SEMANTIC-KEY** — mined candidate; verify scope then implement.
- **PROOF-QUANTIFIER-AUTOMATION** — mined candidate; verify scope then implement.
- **PROOF-SAMPLES-CHECKED-CALL-SELECTION** — mined candidate; verify scope then implement.
- **PROOF-SEARCH-COST-MEASUREMENT** — mined candidate; verify scope then implement.
- **PROOF-SEARCH-DERIVATION-CACHE** — mined candidate; verify scope then implement.
- **PROOF-SUBJECT-CALL-SELECTION** — mined candidate; verify scope then implement.
- **PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION** — mined candidate; verify scope then implement.
- **PROOF-SUBJECT-CHECKED-CALL-SELECTION** — mined candidate; verify scope then implement.
- **PROOF-VALUE-SOURCE-CORRESPONDENCE** — mined candidate; verify scope then implement.
- **PROOFS-SUBJECT-CHECKED-CALL-SELECTION** — mined candidate; verify scope then implement.
- **PROVIDER-ATTACHMENT-MACHINE-PLAN** — mined candidate; verify scope then implement.
- **PSI-BORROWED-LOCAL-CALL-MUTATION** — mined candidate; verify scope then implement.
- **PSI-DOMAIN-FACT-SELECTION-COVERAGE** — mined candidate; verify scope then implement.
- **PSI-FRESH-CONSTRUCTOR-CUSTODY-JOIN** — mined candidate; verify scope then implement.
- **PSI-HOSTED-ENTRY-RECEIVER-PROVISIONING.** Done (linux x86-64 witnessed): respelled the hosted-receiver provisioning canaries off the closed `in Bound` carrier qualification — the core `Service` carrier admits no authored qualification, so `Service<Console>` alone denotes the toolchain service era — and re-pinned the bare-interface rejections on the moved check-stage diagnostic ("the intrinsic `Service<R>` carrier is the only service value spelling"). `select_provider` operands now spell product-scope paths (`omega_language_std::Console` / `omega_language_std::ConsoleNativeProvider`); the same respell was applied to `tests/omega/pass/expressions/runtime_float_receiver_storage_exit/build.omg` under a companion fixture claim. hosted_receiver{,_linux,_linux_arm64} now pass their provisioning, explicit-exit, erased-receiver, record-array, float-leaf, and bare-interface legs; the linux x86-64 legs emit and execute real ELF artifacts (exit 37/0 + stdout "A" witnessed). Remaining: `hosted_receiver_windows.rs` carries the same stale spellings but sits under the RC-WINDOWS-X64-NATIVE-ROW claim; its windows legs and macOS/aarch64 execution legs remain host-gated.
- **PSI-NATIVE-FIELD-STORES** — mined candidate; verify scope then implement.
- **PSI-PARAMETER-ORIGIN-LOCAL-CUSTODY** — mined candidate; verify scope then implement.
- **PURPOSE-AWARE-PACKAGE-LOCKS** — mined candidate; verify scope then implement.
- **QUOTIENT-RUNTIME-REALIZATION** — mined candidate; verify scope then implement.
- **RANKED-CALLEE-NATIVE-COMPOSITION** — mined candidate; verify scope then implement.
- **RANKED-NATIVE-ADMISSION** — mined candidate; verify scope then implement.
- **RANKED-PROJECTED-RECEIVER-COMPOSITION** — mined candidate; verify scope then implement.
- **RC-BUILD-AND-PACKAGES** — mined candidate; verify scope then implement.
- **RC-BUILD-AND-PACKAGES-GATE** — mined candidate; verify scope then implement.
- **RC-CLOSURE-EVIDENCE-RETENTION** — mined candidate; verify scope then implement.
- **RC-DIAGNOSTICS** — mined candidate; verify scope then implement.
- **RC-DIAGNOSTICS-CLOSURE** — mined candidate; verify scope then implement.
- **RC-DIAGNOSTICS-GATE** — mined candidate; verify scope then implement.
- **RC-DIAGNOSTICS-STABILITY** — mined candidate; verify scope then implement.
- **RC-GATE-STABILITY-REPAIR** — mined candidate; verify scope then implement.
- **RC-HOST-RUNNER-LANES** — mined candidate; verify scope then implement.
- **RC-LINUX-ARM64-NATIVE-ROW** — mined candidate; verify scope then implement.
- **RC-LINUX-X86-64-GATE-LEDGER** — mined candidate; verify scope then implement.
- **RC-MATRIX-RUNNER** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-CLOSURE** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-GATE** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-HOST-EXECUTION** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-HOST-LEGS** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-HOST-RUNS** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-HOSTS** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-LINUX-ARM64** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-LINUX-X64** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-LINUX-X86-64** — recorded at
  `wiki/drafts/rc_native_matrix_linux_x86_64.md` (revision 8ae40607a3,
  linux-x86_64): 15 pass / 23 fail across 38 legs; all failures are the
  Service<R>-carrier spelling and entry-binding/ownership fixture-migration
  residuals owned by ENTRY-CONTENT-ROOTS. Re-run the row when those families
  close.
- **RC-NATIVE-MATRIX-MACOS-ARM64** — mined candidate; verify scope then implement.
- **RC-NATIVE-MATRIX-WINDOWS-X64** — mined candidate; verify scope then implement.
- **RC-PCC-REPLAY** — mined candidate; verify scope then implement.
- **RC-PCC-REPLAY-CLOSURE** — mined candidate; verify scope then implement.
- **RC-PCC-REPLAY-GATE** — mined candidate; verify scope then implement.
- **RC-PCC-REPLAY-HOSTILE-EVIDENCE** — mined candidate; verify scope then implement.
- **RC-PLATFORM-RUN-RECORDS** — mined candidate; verify scope then implement.
- **RC-PLATFORM-RUNNER-COVERAGE** — mined candidate; verify scope then implement.
- **RC-PORTABLE-PSI-CLOSURE** — mined candidate; verify scope then implement.
- **RC-PORTABLE-PSI-ENVELOPE** — mined candidate; verify scope then implement.
- **RC-PORTABLE-PSI-GATE.** Resolved — re-mines the release-matrix gate
  `RC-PORTABLE-PSI` (wiki/drafts/rust_compiler_completion.md): the gate
  exists and passes. `compiler::canary_suite
  portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary`
  spawns producer/consumer child legs per RELOAD_CANARIES fixture — one
  process publishes a source-free Terminal Psi envelope and exits, the
  second reconstructs, verifies, and interprets it — plus truncated,
  mutated, and trailing-byte refusal legs rejecting tampered envelopes.
  Witnessed green on Linux x86-64 at `72125c7156` (32.7s, 1/1; test file
  unchanged through `89157bca74`). Sibling
  re-mines of the same row: RC-PORTABLE-PSI, RC-PORTABLE-PSI-CLOSURE,
  RC-PORTABLE-PSI-ENVELOPE, RC-PORTABLE-PSI-RELOAD. The gate is a matrix
  row, not a standalone completion: the release contract still requires
  all eight gates on one clean commit across the four required hosts, and
  the test surface is owned by PORTABLE-TERMINAL-RELOAD work.
- **RC-PORTABLE-PSI-RELOAD** — mined candidate; verify scope then implement.
  Verified scope: re-mines the completed **PORTABLE-TERMINAL-RELOAD** item
  (landed and row-removed at 8ae40607a3, 2026-09-20).
  `canary_suite/portable_terminal_reload.rs` now proves both halves of the
  portable-product boundary for every `RELOAD_CANARIES` fixture: a Terminal
  artifact produced in one process decodes, verifies, and interprets in a
  second, and the consumer refuses truncated envelopes, mutated section
  bytes, and trailing bytes. The generic portable-psi surface legs that
  remain are the sibling re-mines RC-PORTABLE-PSI, RC-PORTABLE-PSI-CLOSURE,
  RC-PORTABLE-PSI-ENVELOPE and RC-PORTABLE-PSI-GATE — no independent reload
  slice exists here.
- **RC-RELEASE-CLOSURE-RUN** — mined candidate; verify scope then implement.
- **RC-RELEASE-RECORD** — mined candidate; verify scope then implement.
- **RC-RELEASE-RECORD-AND-CLOSURE** — mined candidate; verify scope then implement.
- **RC-RELEASE-RECORD-RUN** — mined candidate; verify scope then implement.
- **RC-RELEASE-RECORD-SUBSTRATE** — mined candidate; verify scope then implement.
- **RC-REPOSITORY** — mined candidate; verify scope then implement.
- **RC-REPOSITORY-BASELINE** — mined candidate; verify scope then implement.
- **RC-REPOSITORY-BASELINE-GREEN** — mined candidate; verify scope then implement.
- **RC-REPOSITORY-CLOSURE** — mined candidate; verify scope then implement.
- **RC-REPOSITORY-GATE** — mined candidate; verify scope then implement.
- **RC-REPOSITORY-GATE-CLOSURE** — mined candidate; verify scope then implement.
- **RC-REPRESENTATIVE-PROGRAMS-CLOSURE** — mined candidate; verify scope then implement.
- **RC-REPRESENTATIVE-PROGRAMS-GATE** — mined candidate; verify scope then implement.
- **RC-REPRESENTATIVE-PROGRAMS-GREEN** — mined candidate; verify scope then implement.
- **RC-REPRESENTATIVE-PROGRAMS-PER-HOST** — mined candidate; verify scope then implement.
- **RC-SOURCE-SEMANTICS** — mined candidate; verify scope then implement.
- **RC-SOURCE-SEMANTICS-CLOSURE** — mined candidate; verify scope then implement.
- **RC-SOURCE-SEMANTICS-GATE** — mined candidate; verify scope then implement.
- **RC-WINDOWS-X64-NATIVE-ROW** — mined candidate; verify scope then implement.
- **RECAST-SOURCE-POSITIONS** — mined candidate; verify scope then implement.
- **RECURSIVE-ARGUMENT-OVERLOAD-DECL-DEDUP** — mined candidate; verify scope then implement.
- **RECURSIVE-ARGUMENT-OVERLOAD-DEDUP** — mined candidate; verify scope then implement.
- **RECURSIVE-CALL-FIXTURE-RESOLUTION** — mined candidate; verify scope then implement.
- **REGION-ALIGNMENT-EXPANSION** — mined candidate; verify scope then implement.
- **REMAINING-INTRINSIC-SPAN-ARMS** — mined candidate; verify scope then implement.
- **REPLACEMENT-REJECTION-INVENTORY** — mined candidate; verify scope then implement.
- **REPOSITORY-BASELINE-GATE** — mined candidate; verify scope then implement.
- **REPRESENTATION-OWNERSHIP** — mined candidate; verify scope then implement.
- **REQUEST-BUILD-DIRECTORY-HOST-ALIAS-COVERAGE** — mined candidate; verify scope then implement.
- **RETAINED-ARTIFACT-EXECUTABLE-PUBLICATION** — mined candidate; verify scope then implement.
- **REVIEW-INSTANTIATION-CLONE-FREE-SCRATCH** — mined candidate; verify scope then implement.
- **REVIEW-RESEAL-ELIMINATION** — mined candidate; verify scope then implement.
- **REWRITE-CATALOG-ADMISSION** — mined candidate; verify scope then implement.
- **REWRITE-VALIDATOR-INDEPENDENCE** — mined candidate; verify scope then implement.
- **RO-CODEC-PLACEMENT** — mined candidate; verify scope then implement.
- **RO-S2S-ANCESTRY-WALKS** — resolved: verified on `99b24364c2`. Every
  `live_range_stage()`/`liveness_stage()`/`selected_stage()`/
  `source_legality_stage()`/`source_segment_home_stage()`/`transformation_stage()`
  call in `selected-instructions-to-selected-instructions/src` is a custody
  validator receiving the retained stage object as replay evidence; no
  consumer climbs producer ancestry for current data, and
  `.optimized_target()` survives only as `optimized_target_owner()`, the
  sanctioned proof-input `Arc` handle. New pin
  `selected_stages_read_current_data_not_producer_ancestry` in
  `tests/architecture/representation_ownership.rs` enforces the contract;
  REPRESENTATION-OWNERSHIP in TASKS_OPTIMIZER.md records the leg done.
- **RO-STAGE-ANCESTRY-ELIMINATION** — mined candidate; verify scope then implement.
- **ROOT-FILE-DISCIPLINE** — mined candidate; verify scope then implement.
- **RULE-PROMOTION-EVIDENCE** — mined candidate; verify scope then implement.
  Verified scope: the name re-mines WORKSPACE-ROLLOUT's exact-rule promotion
  territory in TASKS_OPTIMIZER.md — six staged records in
  `omega-rust/omega/representations/optimization-core/promotions/` must each
  carry the [promotion contract's](wiki/spec/build/optimizations.md#release-rollback-and-promotion)
  full evidence set before any `Approved status` completes, gated by
  `exact_rule_rollout_is_complete_and_promotion_gated`. Current state: the
  `Rollback evidence` rejoin legs for CopyPropagation,
  GlobalValueNumbering, ProofCheckElision, and
  SparseConditionalConstantPropagation are under a live claim
  (PROMOTION-ROLLBACK-REJOIN-LEGS); `Measurement evidence` waits on the
  BENCHMARKS native-realization failure; owner approval and the
  `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 mbx test --workspace --no-fail-fast`
  gate are product decisions, not implementable slices here. Sibling
  re-mine: RULE-PROMOTION-EVIDENCE-COMPLETION.
- **RULE-PROMOTION-EVIDENCE-COMPLETION** — mined candidate; verify scope then implement.
- **RUNTIME-CAPABLE-BINDERS** — mined candidate; verify scope then implement.
- **RUNTIME-SIZED-ACTIVATION-CONTRACT** — mined candidate; verify scope then implement.
- **RUNTIME-SIZED-ACTIVATION-STORAGE** — mined candidate; verify scope then implement.
- **RUNTIME-SIZED-ACTIVATION-STORAGE-CONTRACT** — mined candidate; verify scope then implement.
- **RUST-COMPILER-RELEASE-RECORD** — mined candidate; verify scope then implement.
- **RUST-OMEGA-CROSS-COMPILER-DIFFERENTIAL** — mined candidate; verify scope then implement.
- **RUST-PRODUCER-OMISSION** — mined candidate; verify scope then implement.
- **RUST-PRODUCER-RETENTION-POLICY** — mined candidate; verify scope then implement.
- **RUST-PRODUCER-RETIREMENT-GATE** — mined candidate; verify scope then implement.
- **RUST-RELEASE-RECORD** — mined candidate; verify scope then implement.
- **SAMPLES-COMPILE-MULTI-HOST** — mined candidate; verify scope then implement.
- **SCALAR-SCAN-AND-DISPATCH** — mined candidate; verify scope then implement.
- **SCAN-SCALAR-COMPARISON-DISPATCH** — mined candidate; verify scope then implement.
- **SCAN-SCALAR-DISPATCH** — mined candidate; verify scope then implement.
- **SCAN-SCALAR-SCAN** — mined candidate; verify scope then implement.
- **SCHEDULING-RELOCATION-ADMISSION** — mined candidate; verify scope then implement.
- **SCHEDULING-RELOCATION-UNIFICATION** — mined candidate; verify scope then implement.
- **SCOPED-LOOKUP-MAP-AUDIT** — mined candidate; verify scope then implement.
- **SEALED-COMPOSITION-EXTRACTION** — mined candidate; verify scope then implement.
- **SEED-HOST-CHAIN-LEGS.** Mined candidate. The bootstrap chain has two
  audited seed hosts — macOS arm64 (`alpha_arm64_macos`) and Windows x64
  (`alpha_x64_windows.exe`, `bootstrap/0_alpha/`). Every chain leg that
  *executes* the seed must run on each host, and each leg's per-host result
  must be recorded, not assumed.

  Verified leg inventory (origin/main 0cc7a2081a): seed-executing legs are
  `tests/alpha/conformance.sh` + `bounds.py` (behavior leg inside
  `tests/bootstrap/alpha-beta-edge.sh`), the trusted-Beta reconstruction leg
  of the same gate, the complete-chain customer gates
  `tests/bootstrap/omega-parser`, `omega-outcome`, `omega-executable`, and
  the per-rung execution gates under `tests/beta/compiler/`,
  `tests/delta/` and `tests/epsilon/`. Identity gates
  (`alpha/beta/gamma/delta/epsilon/omega/proofs-identity.sh`,
  `source-closure`) and the container/provenance legs are host-independent
  byte checks and need no seed host.

  Guard coverage is incomplete: the bootstrap-level gates and the guarded
  rung gates (`tests/gamma/*`, `tests/epsilon/{refinement,d-composition,
  pair-boundary}`, `tests/delta/internal-boundary`,
  `tests/alpha/{conformance.sh,reference/diamond-py.sh}`) refuse with exit 2
  on hosts without an audited seed, but seventeen seed-executing gates lack
  the guard and hard-fail opaquely on other hosts (`exec`/`Popen` on the
  host-selected container): `tests/beta/compiler/register-address-regression.sh`;
  `tests/delta/{emission,frontend-boundary,generated-function-census,
  lowering-plan,normalization,request-boundary,resource-boundary,
  staged-compiler}/run.sh`; and `tests/epsilon/{array-storage,
  checking-invariants,checking,evaluator-entry,interpreted-omega-experiment,
  runtime-invariants,runtime-references,source-views}/run.sh`. Each needs
  the standard refusal stanza its guarded siblings carry
  (`<gate>: unsupported host; needs macOS arm64 or Windows x64`, exit 2).

  Leg status today: macOS arm64 legs are validated
  ([tests/alpha/README.md](tests/alpha/README.md)). Every Windows x64 leg is
  outstanding: `omega-parser` README records "Windows execution has not yet
  been validated", `tests/alpha` README records "Windows native execution
  remains outstanding", and `bootstrap/0_alpha` README records that PE
  reconstruction and source review "do not establish Windows runtime
  validation". **ALPHA-WINDOWS-CONFORMANCE** owns the Windows edge legs
  (`alpha-beta-edge.sh` + `tests/alpha/reference/diamond-py.sh`); the
  complete-chain customer legs on Windows x64 remain open beyond that item —
  they are also the legs **OFFLINE-REBUILD**'s blank-host reconstruction
  eventually needs on each seed host.

  Remaining: (1) add the refusal stanza to the seventeen unguarded legs
  above — currently fenced by the DELTA-COMPILER claim on `tests/delta` +
  `tests/epsilon` and the BETA-PE-SEED-REFUSAL claim on `tests/beta/compiler`;
  (2) run each seed-executing leg on Windows x64 and record the per-host
  result — every Windows leg stays outstanding until a Windows host validates
  it; no leg may report an unexecuted host result as passing.
- **SEED-PARITY-ALIGNMENT** — mined candidate; verify scope then implement.
- **SEED-PARITY-ASSERTIONS** — mined candidate; verify scope then implement.
- **SEED-PARITY-CLONE-SERIALIZATION** — mined candidate; verify scope then implement.
- **SELECTED-DISPATCH-SERVICE-CARRIER-FIXTURES** — mined candidate; verify scope then implement.
- **SELECTED-OPTIMIZATION-ANCESTRY-ELIMINATION** — mined candidate; verify scope then implement.
- **SELECTED-OPTIMIZATION-ANCESTRY-READS** — mined candidate; verify scope then implement.
- **SELECTED-OPTIMIZATION-ANCESTRY-REMOVAL** — mined candidate; scope verified, already landed. REPRESENTATION-OWNERSHIP's bullet for this crate (`selected_optimization.rs` reaching selections through `ranges.liveness_stage().selected_stage().optimized_target().optimized()`) is closed on main: the staged types expose `selected`/`register_environment`/`selections`/`budget_per_pass`/`liveness`/`ranges`/`legality` directly, `83766d57bf` moved custody reads to the retained `optimized_target_owner` handle, and `tests/ancestry_contract.rs` pins the contract — zero `.optimized_target()` data reads and no unsanctioned `selected_stage()` walks remain (witnessed: `cargo nextest run -p selected-instructions-to-selected-instructions --test ancestry_contract` 1/1 green at 4a6bd936dc, Linux x86-64). Surviving `live_range_stage`/`liveness_stage`/`source_legality_stage`/`source_segment_home_stage`/`transformation_stage` hops are the contract's named custody-validator inputs, not data reads. Sibling stubs on the same settled surface: RO-S2S-ANCESTRY-WALKS, RO-STAGE-ANCESTRY-ELIMINATION, SELECTED-OPTIMIZATION-ANCESTRY-ELIMINATION/-READS, SELECTED-REWRITE-ANCESTRY-REMOVAL, STAGE-ANCESTRY-DIRECT-READS, STAGED-ANCESTRY-ELIMINATION.
- **SELECTED-OPTIMIZATION-CATALOG-ROUTE** — mined candidate; verify scope then implement.
- **SELECTED-OPTIMIZATION-DIRECT-READS** — mined candidate; verify scope then implement.
- **SELECTED-REWRITE-ANCESTRY-REMOVAL** — mined candidate; verify scope then implement.
- **SELECTED-REWRITE-CATALOG-DISPOSITION** — mined candidate; verify scope then implement.
- **SELECTED-REWRITE-CATALOG-EXECUTION** — mined candidate; verify scope then implement.
- **SELECTED-REWRITE-CATALOG-OR-DELETE** — mined candidate; verify scope then implement.
- **SELECTED-REWRITE-CATALOG-ROUTE** — mined candidate; verify scope then implement.
- **SELECTED-REWRITE-CATALOG-WIRING** — mined candidate; verify scope then implement.
- **SELECTED-REWRITES-CATALOG-OR-DELETE** — mined candidate; verify scope then implement. Landed (delete leg): `rewrites/literal_compare` and `rewrites/literal_arithmetic` removed — uncalled standalone producers re-implementing the immediate folds the selected-lowering pair rules already run; both roots are pinned in `optimizer_source_organization::retired_paths`. Remaining: widen pair-rule candidate nomination for the retained-materialization and operand-0 cases under DECLARATIVE-PEEPHOLES; catalog disposition for the other uncalled rewrites modules belongs to the SELECTED-REWRITE-CATALOG-* siblings.
- **SELECTED-STAGE-RULE-CATALOG** — mined candidate; verify scope then implement.
- **SELECTIVE-ARITHMETIC-EXPANSION** — scope verified 2026-09-20: this mined
  stub re-covers open work already owned by **MATCH-SELECTIVE-LOWERING**. Its
  mining source is
  [chapter 6](wiki/language_guide/chapter_6_pattern_matching_dispatch.md)'s
  note that the parser's arithmetic-shaped `match` expansion does not
  implement general selective evaluation; the
  [source processing note](omega-rust/psi/pipeline/README.md#lexing-and-parsing)
  records the gap (Boolean/integer subjects, Boolean/integer/float results,
  structural/domain/payload patterns as explicit limitations). Every leg of
  that expansion — owned/nonnumeric results, structural/case/domain patterns,
  and parameter/projected/borrowed custody — is an enumerated remaining-work
  row of MATCH-SELECTIVE-LOWERING, whose named owners are
  `validation/src/value_custody/expression_types/{match_dispatch,result_type}.rs`,
  the checked scalar continuations, and Terminal production. No independent
  slice exists here: expanding the arithmetic-shaped subset is the owner
  item's work and must not proceed through a parallel claim on the same
  files. Sibling re-mines of the same surface: SELECTIVE-EVALUATION-EXPANSION,
  SELECTIVE-EVALUATION-PARSER, SELECTIVE-EVALUATION-SOURCE-EXPANSION.
- **SELECTIVE-EVALUATION-EXPANSION** — mined candidate; verify scope then implement.
- **SELECTIVE-EVALUATION-PARSER.** Resolved — the mined source claim ("the Rust parser's current arithmetic expansion does not implement general selective evaluation", chapter_6_pattern_matching_dispatch.md) predates the real `match` implementation by two days (written 5f5ec35a63, scalar dispatch retained 9356485a0b). Scalar selective evaluation is implemented end-to-end and pinned by the `expressions/match_*` and runtime `*_dispatch_exit` canaries; the true residual frontier (record/domain/payload match-arm patterns, owned/borrowed custody results) is already tracked under **MATCH-SELECTIVE-LOWERING**. The durable gaps found and fixed at this row: `match` arm `Name {` destructure spelling now rejects with a diagnostic naming `transition` as the owning dispatch form (previously `expected '->'` on `{`), and the two stale pre-split `match` pass fixtures under `tests/omega/pass/domains/` were respelled to `transition` (`match_domain_patterns`, `match_interleaved_domain_data_guard` — both had been silently red since the June chapter-coverage sweep). Sibling mined rows from the same stale sentence (SELECTIVE-ARITHMETIC-EXPANSION, SELECTIVE-EVALUATION-EXPANSION, SELECTIVE-EVALUATION-SOURCE-EXPANSION) cover this same resolved scope.
- **SELECTIVE-EVALUATION-SOURCE-EXPANSION** — mined candidate; verify scope then implement.
- **SEMANTIC-WRAPPER-COORDINATOR-RESIDUE** — mined candidate; verify scope then implement.
- **SEMANTIC-WRAPPER-OBJECT-OWNERSHIP** — mined candidate; verify scope then implement.
- **SEMANTIC-WRAPPER-OWNER-RELOCATION** — mined candidate; verify scope then implement.
- **SEMANTIC-WRAPPER-OWNER-RESOLUTION** — mined candidate; verify scope then implement.
- **SEMANTIC-WRAPPER-OWNERSHIP** — mined candidate; verify scope then implement.
- **SERVICE-CARRIER-FIXTURE-MIGRATION** — mined candidate; verify scope then implement.
- **SERVICE-ERA-REPLACEMENT-SUBSTRATE** — mined candidate; verify scope then implement.
- **SHARED-RECEIVER-LOAN-ORIGIN** — mined candidate; verify scope then implement.
- **SHARED-WORD-PREFIX-NATIVE-RUN** — mined candidate; verify scope then implement.
- **SIGNATURE-FREE-TRAIT-CANDIDATE-SCOPE** — mined candidate; verify scope then implement.
- **SINGLE-PROGRAM-ENTRY-SELECTION** — mined candidate; verify scope then implement.
- **SNAPSHOT-STORAGE** — mined candidate; verify scope then implement.
- **SNAPSHOT-STORAGE-AND-FILTERING** — mined candidate; verify scope then implement.
- **SOURCE-SEMANTICS-SUITE** — mined candidate; verify scope then implement.
- **SPILL-FAMILY-SEQUENCE-OR-DELETE** — mined candidate; verify scope then implement.
- **SPILL-STAGES-OWNERSHIP** — mined candidate; verify scope then implement.
- **SQUALR-CLI-ENTRY-AND-MODEL** — mined candidate; verify scope then implement.
- **SQUALR-CLONE-SERIALIZATION** — mined candidate; verify scope then implement.
- **SQUALR-DEBUG-ASSERTION-PARITY** — mined candidate; verify scope then implement.
- **SQUALR-DEBUG-ASSERTIONS** — mined candidate; verify scope then implement.
- **SQUALR-ENGINE-CRATE-SOURCES** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-DEBUG-ASSERTIONS** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-PARITY-GAPS** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-PARITY-REMAINDER** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-PARITY-RESIDUE** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-WINDOWS-NATIVE** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-WINDOWS-RUN** — mined candidate; verify scope then implement.
- **SQUALR-GEOMETRY-WINDOWS-VALIDATION** — mined candidate; verify scope then implement.
- **SQUALR-PLUGIN-IMPLEMENTATIONS** — mined candidate; verify scope then implement.
- **SQUALR-PLUGIN-PACKAGES** — mined candidate; verify scope then implement.
- **SQUALR-SEED-ALIGNMENT-PARSING** — mined candidate; verify scope then implement.
- **SQUALR-SEED-OPERATOR-PARITY** — mined candidate; verify scope then implement.
- **SQUALR-SEED-REGION-OPERATIONS** — mined candidate; verify scope then implement.
- **SQUALR-SUPPLIED-BYTES-SCAN** — mined candidate; verify scope then implement.
- **SQUALR-WINDOWS-GEOMETRY-VALIDATION** — mined candidate; verify scope then implement.
- **STAGE-ANCESTRY-DIRECT-READS** — mined candidate; verify scope then implement.
- **STAGE-CRATE-OWNERSHIP-AUDIT** — mined candidate; verify scope then implement.
- **STAGE-ENTRANCE-ORPHAN-AUDIT** — mined candidate; verify scope then implement.
- **STAGED-ANCESTRY-ELIMINATION** — mined candidate; verify scope then implement.
- **STAGED-LOCAL-CRASH-LOWERING** — mined candidate; verify scope then implement.
- **STAGED-LOCAL-CRASH-LOWERING-ATTRIBUTION** — mined candidate; verify scope then implement.
- **STALE-CUSTODY-GATE-EXPECTATIONS** — mined candidate; verify scope then implement.
- **STANDALONE-REQUEST-CONTRACT** — mined candidate; verify scope then implement.
- **STARTUP-ENTRY-MECHANICS** — mined candidate; verify scope then implement.
- **STARTUP-ENTRY-MECHANICS-OWNERSHIP.** Resolved by audit at `be03555d17` — startup/entry mechanics already sit under backend runtime ownership per `omega-rust/pipeline.md`: `backend/runtime/external-roots/src/root_entry` owns entry/exit mechanics (validation, admission, provider execution, progress-profile installation) and `platform_bringup` owns UEFI bootstrap + secondary-processor startup; `backend/plans/program-entry-plan` is data-only planning (its lib.rs owns "no emitted bytes, installation state, or legacy backend pipeline"); `_start` symbol resolution under `backend/images/image-{elf,macho}` and `compiler/native-realization/src/entry_settlement` are emission detail and realization orchestration, not mechanics. No placeholder crate owns startup mechanics; `tests/architecture/layering.rs` already pins the external-roots ownership rows. Sibling aliases (STARTUP-ENTRY-MECHANICS, STARTUP-ENTRY-PLACEHOLDER-SWEEP, STARTUP-ENTRY-RUNTIME-MECHANICS, BACKEND-RUNTIME-STARTUP-*, ENTRY-MECHANICS-RUNTIME-CONSOLIDATION) remain separate stubs.
- **STARTUP-ENTRY-PLACEHOLDER-SWEEP** — mined candidate; verify scope then implement.
- **STARTUP-ENTRY-RUNTIME-MECHANICS** — mined candidate; verify scope then implement.
- **STATEMENT-CALL-RECURSIVE-ARGUMENT-DEDUP** — mined candidate; verify scope then implement.
- **STATEMENT-CALL-RECURSIVE-OVERLOAD** — mined candidate; verify scope then implement.
- **STRUCTURAL-GENERIC-INFERENCE** — mined candidate; verify scope then implement.
- **STRUCTURAL-PROOFS-CHECKED-CALL-SELECTION** — mined candidate; verify scope then implement.
- **STRUCTURAL-SUCCESSOR-DISCARD-ORDERING** — mined candidate; verify scope then implement.
- **SUCCESSOR-ARGUMENT-DIAGNOSTIC-ORDER** — mined candidate; verify scope then implement.
- **SUPERVISED-STARTUP-RUNTIME-ENFORCEMENT** — mined candidate; verify scope then implement.
- **SUPPLIED-BYTES-SCAN** — mined candidate; verify scope then implement.
- **T2C-RANK-RANGE-FIELD-ENDPOINTS** — mined candidate; verify scope then implement.
- **TARGET-BATCH-MANIFEST** — mined candidate; verify scope then implement.
- **TARGET-INFERENCE-AND-PLATFORM-CERTIFICATION** — mined candidate; verify scope then implement.
- **TASK-RUNTIME-NATIVE-SUPPORT** — mined candidate; verify scope then implement.
- **TERMINAL-SOURCE-CUSTODY-GATE-ORDER** — mined candidate; verify scope then implement.
- **TERMINATION-FIELD-ENDPOINT-TRIO** — mined candidate; verify scope then implement.
- **TERMINATION-RANK-RANGE-FIELDS** — mined candidate; verify scope then implement.
- **TEST-CYCLE-SELECTION-REMEASUREMENT** — mined candidate; verify scope then implement.
- **TRAIT-MATHEMATICAL-PREDICATE-CONTRACTS** — mined candidate; verify scope then implement.
- **TRANSFORM-CODEC-RELOCATION** — mined candidate; verify scope then implement.
- **TRANSLATION-VALIDATION** — mined candidate; verify scope then implement.
- **TRANSPARENT-TRAIT-REFINEMENTS** — mined candidate; verify scope then implement.
- **TRUSTED-SURFACE-DIGEST-RE-RECORDING** — mined candidate; verify scope then implement.
- **TRUSTED-SURFACE-DIGEST-REFRESH** — mined candidate; verify scope then implement.
- **TRUSTED-SURFACE-DIGEST-RERECORD** — mined candidate; verify scope then implement.
- **TRUSTED-SURFACE-LEDGER-REFRESH** — mined candidate; verify scope then implement.
- **TRUSTED-SURFACE-LEDGER-RERECORD** — mined candidate; verify scope then implement.
- **TV-BOUNDARY-SETTLEMENTS-REPLAY** — mined candidate; verify scope then implement.
- **TV-DYNAMIC-AND-INTRINSIC-SPANS** — mined candidate; verify scope then implement.
- **TV-GENERAL-CALLS-REPLAY** — mined candidate; verify scope then implement.
- **TV-INTRINSIC-SPAN-ARMS** — mined candidate; verify scope then implement.
- **TV-OPERATOR-APPLICATIONS-REPLAY** — mined candidate; verify scope then implement.
- **TV-PRIVILEGED-PORT-EFFECTS** — mined candidate; verify scope then implement.
- **UNSEQUENCED-SPILL-DISPOSITION.** Mined candidate; verify scope then implement.
- **UNSEQUENCED-SPILL-FAMILY-DISPOSITION** — mined candidate; verify scope then implement.
- **UNSEQUENCED-SPILL-STAGE-DISPOSITION** — mined candidate; verify scope then implement.
- **UNSEQUENCED-SPILL-STAGE-TRIAGE** — mined candidate; verify scope then implement.
- **UNSEQUENCED-SPILL-STAGES-DISPOSITION.** Mined candidate. Upstream:
  [TASKS_OPTIMIZER.md](TASKS_OPTIMIZER.md) PIPELINE-OWNER-CONSOLIDATION flag —
  `selected-instructions-to-register-homes/src/unsequenced_spill_stages/` holds
  18 spill families (~25,700 non-test lines) that `stage_register_allocation`
  never calls; SPILL-REALIZATION owns sequencing the ones it needs, the
  executable `assignment/runtime_spill` route supersedes the rest. Sequence a
  staged family behind the executable route or delete it; do not extend both.

  Verified scope (origin/main 3dd805679c): every family is publicly re-exported
  in the stage's `lib.rs` and has external consumers — native-differential
  `pipeline_ownership` stage tests, `tests/architecture/optimizer_source_organization`
  entrance/ladder/retired-paths tables, and machine emission's
  `frame_layout/spill_requirements/`. The families also form a dependency
  chain (e.g. `synthetic_reload_values` consumes
  `ValidatedAbstractSpillInsertion` + `ValidatedReloadValueHomes`), so
  deletions must proceed leaf-consumer-first. Identified duplicate-owner pair:
  `stack_slot_coloring` (interval coloring) vs. `runtime_spill/slot.rs` (the
  executable route's slot reuse).

  Note for the coordinator: this stub is one of four mined duplicates for the
  same directory — UNSEQUENCED-SPILL-STAGE-DISPOSITION,
  UNSEQUENCED-SPILL-STAGE-TRIAGE and UNSEQUENCED-SPILL-STAGES-SEQUENCE-OR-DELETE
  cover the identical ground and should be collapsed into one item.

  Remaining: disposition each of the 18 families — sequence (via
  SPILL-REALIZATION/POC-SPILL-FAMILY-SEQUENCING) or delete with its lib.rs
  re-exports, native-differential consumers and architecture tables. Whole
  territory is currently fenced: `selected-instructions-to-register-homes`
  wholesale (POC-SPILL-FAMILY-SEQUENCING), `optimizer_source_organization`
  (ORPHAN-STAGE-OUTPUT-AUDIT), `pipeline_ownership` (STRUCTURAL-UNIT-CALL-GRAPH-JOINS).
- **UNSEQUENCED-SPILL-STAGES-SEQUENCE-OR-DELETE** — mined candidate; verify scope then implement.
- **VERIFIER-EDGE-CLEANUP-PHASE-ORDER** — mined candidate; scope verified, resolved — alias of the terminal-verifier cleanup-order row already repaired on `origin/main`: edge validation consumes owned successor sources before the residual and trivial discard rosters (`validation/frontier/block_parameters.rs` documents the order; `terminators.rs` runs it), and `d96a0fda39` repinned `owned_successors_reject_same_arity_aliases_and_transfer_after_disposal` to expect `EdgeAffineDiscardsInvalid`. All 26 `structural_scalar_fields::owned_reads` tests pass at `ff596a06e6`. EDGE-CLEANUP-ERROR-PRECEDENCE's landed annotation already names this stub among the row's aliases.
- **WAIT-WAKE-SUBSTRATE** — mined candidate; verify scope then implement.
- **WHOLE-COMPOSITION-EXTRACTION** — mined candidate; verify scope then implement.
- **WHOLE-COMPOSITION-INTERACTION-EXTRACTION** — mined candidate; verify scope then implement.
- **WINDOWS-ALPHA-CONFORMANCE-LEG** — mined candidate; verify scope then implement.
- **WINDOWS-FILE-TIME-CARRIER-RESPELL** — mined candidate; verify scope then implement.
- **WINDOWS-FILE-TIME-UNSIGNED-RESPELL** — mined candidate; verify scope then implement.
- **WINDOWS-PEAK-MEMORY-MEASUREMENT** — mined candidate; verify scope then implement.
- **WINDOWS-SET-FILE-TIME-CARRIER** — mined candidate; verify scope then implement.
- **WINDOWS-SET-FILE-TIME-UNSIGNED-RESPELL** — mined candidate; verify scope then implement.
- **WORKLOAD-CORPUS** — mined candidate; scope verified, authorization gate recorded. Re-mines WORKLOAD-CORPUS-AND-MULTIVERSIONING's corpus leg (the versioned workload corpus that GRAPH-COST-MODEL-STUDY's `predicted_cost_delta` comparison is missing). Source doc `wiki/drafts/learned_optimization_policy.md` authorizes no implementation: the corpus is a far-future extension gated on the Omega-written product compiler (OMEGA-PRODUCT-COMPILER-SOURCE) plus a concrete justification, and `wiki/spec/build/optimizations.md` forbids trainer-side machinery in the Rust reference compiler. The versioned workload surface that exists today is BENCHMARKS' `tools/benchmark` records. Same verdict already recorded on sibling GRAPH-COST-EVIDENCE-CORPUS; other sibling stub on this gated surface: OPTIMIZATION-WORKLOAD-CORPUS.
- **WORKLOAD-MULTIVERSIONING** — mined candidate; verify scope then implement.
- **WORKSPACE-ROLLOUT** — mined candidate; verify scope then implement.
- **WR-REJOIN-LEGS** — mined candidate; verify scope then implement.
- **WRAPPER-OBJECT-OWNERSHIP** — mined candidate; verify scope then implement.
  Verified scope (origin/main c6336b5b02): the name re-mines the
  `optimized_semantic_wrapper_{encoding,object}` orphan-owner surface in
  `omega-rust/omega/compiler/native-realization/` — "wrapper object" is the
  ProgramStorage semantic wrapper object and "ownership" is the question of
  which pipeline/backend owner keeps it. That decision is an enumerated
  remaining-work bullet of PIPELINE-OWNER-CONSOLIDATION in TASKS_OPTIMIZER.md
  ("Move the live part to its backend or representation owner, or delete it");
  the codec move leg belongs to DURABLE-CODEC-RELOCATION /
  REPRESENTATION-OWNERSHIP (explicitly deferred until that owner decision
  lands), and the executable-route leg that would give
  `stage_validated_optimized_program_storage_semantic_wrapper_object` its first
  caller outside its own tests is a bullet of UEFI-PHYSICAL-SEMANTIC-ENTRY.
  No independent slice exists here: the keep/move/delete decision is the owner
  item's work and must not proceed through a parallel claim on the same files —
  PIPELINE-WRAPPER-OBJECT-ORPHAN already claims
  `native-realization/src/optimized_semantic_wrapper_object` under a live
  ticket. Sibling re-mines of the same surface:
  SEMANTIC-WRAPPER-OBJECT-OWNERSHIP, SEMANTIC-WRAPPER-OWNERSHIP,
  SEMANTIC-WRAPPER-OWNER-RELOCATION, SEMANTIC-WRAPPER-OWNER-RESOLUTION,
  SEMANTIC-WRAPPER-COORDINATOR-RESIDUE, OPTIMIZED-SEMANTIC-WRAPPER-OWNERSHIP,
  OPTIMIZED-SEMANTIC-WRAPPER-RELOCATION, OPTIMIZED-SEMANTIC-WRAPPER-DISPOSITION,
  OPTIMIZED-WRAPPER-OBJECT-RELOCATION, PIPELINE-WRAPPER-OBJECT-ORPHAN.
- **WRITE-ONLY-BORROW-RESIDUE** — mined candidate; verify scope then implement.
  Verified scope: re-mines **WRITE-ONLY-BORROW**'s enumerated remaining work
  (TASKS.md:2323): aggregate/[copy]-sum replacement, domain-qualified
  byte-field stores, runtime indexes (`WriteOnlyIndexedPrimitiveStore` has a
  Terminal representation and interpreter but no Psi producer and Omega
  rejects it with `UnsupportedIndexedPrimitiveStore`), `&mut dyn` dispatch,
  and computed IEEE stores (the 135-file draft for that leg was parked on an
  unpublished `write-only-borrow` branch, not on `origin` — confirm with the
  coordinator before re-implementing). The implementation surfaces are under
  live claims: WRITE-ONLY-BORROW/integer-entry-ranges (checked-trees +
  state_graph + scalar_graph, expires 20:07Z), STRUCTURAL-BORROW-IDENTITY
  (receiver_calls + structural call arguments, 21:38Z),
  RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY (scalar_graph_input
  indirect-call legalization, 22:05Z), and FINITE-GENERIC-DISPATCH
  (dynamic_scalar_calls, 21:39Z). The maintained integration target is
  `terminal_psi_indexed_receivers`, not a store-specific emitter; the
  shared place/loan sequencer extension is STATE-LOCAL-VALUE-FRONTIER's. No
  independent slice exists here.
- **ZERO-BYTE-ARRAY-FENCE-PLACEMENT** — mined candidate; verify scope then implement.
- **ZERO-EXTENT-BYTE-ARRAY-ADMISSION** — mined candidate; verify scope then implement.
- **ZERO-EXTENT-BYTE-ARRAY-FENCE** — mined candidate; verify scope then implement.
- **ZERO-LENGTH-BYTE-ARRAY-ADMISSION-FENCE** — mined candidate; verify scope then implement.
- **ZERO-LENGTH-BYTE-ARRAY-FENCE** — mined candidate; verify scope then implement.
- **ZERO-LENGTH-FIXED-BYTE-ARRAY-FENCE** — mined candidate; verify scope then implement.

## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
- Windows AArch64 has no `NativeTarget` constructor, so that ABI combination
  stays unwitnessable until the target vocabulary grows one.
