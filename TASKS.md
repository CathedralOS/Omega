# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion plan](wiki/drafts/rust_compiler_completion.md) defines the full
acceptance bar; this board identifies the remaining work, not a passing baseline.

| Start here | Purpose |
| --- | --- |
| [Immediate product closure](#immediate-product-closure) | Unchanged sample/canary programs and the dependencies preventing native execution. |
| [P1–P5](#p1---authority-roots-and-entry) | Entry/storage, materialization, portable evidence, ABI, and Cathedral customers. |
| [Parallel language work](#parallel-language-and-compiler-lanes) | Remaining accepted language surface; independent work can proceed when a product strategy is paused. |
| [Compiler throughput](#compiler-throughput) | Measured work reduction; no speculative cache or framework. |
| [Optimizer board](TASKS_OPTIMIZER.md) / [bootstrap board](TASKS_BOOTSTRAP.md) | Separate execution owners, not duplicated here. |

Keep each task's missing behavior, owner, real dependencies, and acceptance
condition. Delete completed work; Git holds checkpoint results and superseded
diagnoses. Link the specification for semantics rather than restating it.
Retain a dated/revision-bound failure only when it determines where to resume;
rerun that customer before assuming the old diagnosis still applies.
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
  [Squalr application](samples/apps/README.md) through its nested workspace and
  package builds. Preserve the essentially 1:1 Rust port, its actual dependency
  graph, and native acceptance; do not flatten packages or substitute a fixed-size
  scanner to fit current lowering. Start with its geometry test application,
  then the real supplied-byte scan/filtered-result path. The submodule's TASKS
  owns port work; this board owns compiler blockers exposed by the unchanged app.

  Acceptance: `python samples/apps/squalr/tools/verify.py native --timeout 600 --omega <binary>`
  must execute the unchanged application and print `Squalr geometry: PASS`.
  Package acceptance and verified Terminal production do not establish native
  execution. Invocation output lives under its ignored `build/verification/`.
  Initialization is explicit and needs private repository access.

  Resume: the tracked app is `7a272a896c85`, with std pinned to
  `a91d878cb9252647d977c45787969b16e6ef937a`. Compiler `bf35e75acb` checks the complete
  17-package graph when the application imports `omega::language::core::service`
  and declares `console: Service<Console> in Bound`; its geometry is unchanged.
  On macOS ARM64 with Python 3.13 and `RUST_MIN_STACK=67108864`, ordinary
  `omega update --project samples/apps/squalr/squalr-tests --target macos_arm64`
  exits 3 at pending Console-read/native-supply/FilesystemHost decisions. The
  available Git identity cannot publish to the private application repository
  (403), so its tracked pin still needs that source correction. Publish it there
  before advancing the gitlink. Preserve the historical lock outside the package
  while completing checkout-local review; restoring it still makes the outer
  native command exit 200 before compilation. No native acceptance is established.

  Do not resume from the application's historical missing-Unit-plan diagnosis.
  At `76478cbd6e`, a targetless compiler-library probe of unchanged app `7a272a896c85`
  and its complete reachable package graph, using std's exact pinned source tree
  `f299035391e73299c6cad3918ada57cb144fb1cc`, retains the composed `Main::main`
  plan and all geometry helper bodies. `compile_to_checked` followed by
  `TerminalProductionRequest::new(&checked, "Main::main").produce_artifact()`
  succeeds on macOS ARM64. This bypasses neither approval nor native admission:
  the probe supplied source bindings only, selected no target, and did not execute
  the app. Complete the explicit review and source publication above, then rerun
  the real native command before assigning another lowering repair. Once geometry
  executes, alternate the port's supplied-byte scan work with the concrete compiler
  gaps that it exposes; Terminal publication alone does not close that milestone.

  Bridge owners: **ENTRY-CONTENT-ROOTS** and
  **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION**, owned by
  `native-realization/src/native_realization.rs`, `program-entry-plan`
  and `external-roots`, under [entry roots](wiki/spec/build/entry_roots.md).
  Preserve the target-backed receiver, private-stack and continuation partitions,
  activation and completion contract under the loading premises below. A
  test-supplied receiver or zero-payload provider layout is not provisioning.

  Keep one integration owner and this application command across the bridge
  and subsequent native/provider failures. Do not restart isolated getter/setter
  helper milestones: the ordinary/composed Psi route now serves this application.
  Preserve its operation order, complete package graph and independent evidence;
  the current `console: Console` field also needs reconciliation with canonical
  `Service<Console> in Bound` establishment. Canonical service eligibility and
  exact field establishment already work; do not add another eligibility path
  or make a missing row authorize an erased-field fallback.

  The integrated conditional bridge at `96854008a0` reuses the exact target
  contracts and unified package review. On macOS ARM64,
  `RUST_MIN_STACK=67108864 cargo nextest run -p compiler --test canary_suite
  --no-fail-fast -E 'test(entry_and_abi::hosted_receiver)'` executes a canonical
  Bound receiver with retained scalar mutation (`A`, exit 0) and distinct process
  exit (`A`, exit 37), and rejects missing establishment and corrupted object
  binding. The implementation additionally checks partition bounds/alignment and
  decodes final bridge instructions independently of the emitter and relocation
  patcher. This remains dependency evidence, not unchanged Squalr acceptance.
  Entry schema discovery proposes review candidates without accepting them;
  explicit missing/stale bindings reject. Automatic source seeding preserves
  ordinary supplier identity rather than relabeling it toolchain-owned.
  Next: finish the relocated application's explicit review, reconcile canonical
  Bound service establishment, and rerun the outer command through the next
  native/provider failure. Do not rebuild the already integrated bridge or
  replace the unified package workflow. These probes do not establish Squalr
  acceptance.

  `image-macho/src/loader_mapping.rs` and `loader_fixups.rs` independently check
  segment mapping, retained payloads, exact eager bind/rebase writes and zero-fill
  exclusion during ordinary image replay. Object relocations and normalized
  import locators supply the expected pointers; these checks do not establish
  the receiver grant or authorize loaded providers.
  The `hosted_receiver` implementation joins these checks to disjoint
  receiver/private-stack/continuation partitions and exact SP/LR and call flow.
  Its claim remains conditional on conforming loading, process-local exclusive
  writable-image storage, one entry activation, admitted provider behavior and
  the declared completion contract; callback occupancy rejects. Preserve these
  premises during integration rather than treating byte checking as an actual
  installed receiver grant. The
  [native product contract](wiki/spec/build/component_publication.md#products-and-authority)
  does not require a live `InstalledCode` or Rust supervisor before compiling
  an ordinary executable. Actual installation/epoch custody remains necessary
  for an installed-runnable claim; a conditional image check cannot create it.
  Do not route artifact production through a fabricated runtime ledger.
  This is implementation work under the entry and
  [service binding contract](wiki/language_guide/chapter_19_capabilities_effects_boundaries.md#service-bindings),
  not an unanswered language decision.
  **MATCH-SELECTIVE-LOWERING** and **STATE-LOCAL-VALUE-FRONTIER** own any newly
  witnessed operation joins. These are engineering dependencies, not owner
  questions. Keep build-only packages explicitly unported.

- **MACOS-APPLICATION-PUBLICATION.** The
  [settled publication contract](wiki/spec/build/macos_application.md)
  implementation is landed; the item stays open for host- and
  dependency-gated acceptance.

  Landed: `builder.identifier` is ordinary Build vocabulary validated in
  `build-evaluation/src/configuration.rs` (`ApplicationIdentifier`);
  `application_intent` travels separately from the PE subsystem word
  through the checked compilation and the retained native proposal, which
  also retains the authored `builder.application` name and identifier.
  Signed macOS GUI image emission requires the identifier on both the
  direct and retained routes; `compilation-report/src/package.rs` stages,
  validates, and installs one whole `<name>.app`; reports expose
  `checked_native_package_path()` separately from
  `checked_native_executable_path()`; the package receipt cross-binds the
  flat executable's v1 container digest; the CLI reports the package root
  and the GUI canaries consume the checked accessors. The four GUI
  `build.omg`s carry authored identifiers.

  Acceptance: the specification's stage-requiredness, deterministic bytes, cross-invocation,
  tampering, partial-output, and flat-regression controls pass; GUI samples carry
  authored identifiers and consumers use reported paths. Validate the procedural
  GUI cohort on macOS, recording unavailable-host coverage explicitly. Resource
  inclusion/lookup for `image_viewer` remains outside v1; do not claim Finder
  runtime coverage for it or silently change its working directory.

  Verified on macOS x86_64 at cf5dcf81e1 (aarch64 execution unavailable on
  this Intel host): `mbx nextest run -p compilation-report` 12/12,
  `mbx nextest run -p build-evaluation -p image-macho -p object-file`
  181/181, `mbx nextest run -p compiler --test pcc_publication` 14/14, and
  `mbx nextest run -p compiler --test build_target_activation -E
  'test(/activation_identifiers_and_publication/)'` 15/15. The suite's two
  remaining failures are the documented x86 FMA provider-transport
  baseline (`wiki/drafts/known_baseline_failures.md`).

  Still open: the procedural GUI cohort (`window_app`, `window_demo`,
  `windowed_calculator`) end-to-end on macOS aarch64 —
  `native_filesystem_canaries` execs the produced aarch64 Mach-O and needs
  an aarch64 host. Its staged sample tests now supply the std package graph
  and test-owned acceptance themselves (`staged_std_package_inputs`), so at
  4707fde28b on macOS ARM64 all six staged samples (`window_demo`,
  `window_app`, `windowed_calculator`, `image_viewer`, `file_journal`,
  `note_vault`) clear staging and stop at `InvalidUnitMachinePlan` for
  `Main::main` ("attached Unit closure is missing a checked transitive
  machine plan"), the same **GENERAL-CYCLIC-EXECUTION** gap the `window_app`
  [outer command](samples/gui/window_app/README.md) reaches after ordinary
  package review; `windowed_calculator` spends 795 s in checking before
  reaching it. The target's other 83 fixtures fail on undeclared service
  reach and decision-17 exact-arithmetic obligations (**CANARY-CORPUS**). The requested
  native `.proof` sidecar inside the package waits on native PCC
  (**PCC-PRODUCT-PUBLICATION**; `pcc.native` publication is `Incomplete`).

- **SAMPLE-CORPUS.** Close the unchanged maintained programs through checked
  semantics and native execution. Start with the documented
  [cli_mvp](samples/cli/basics/cli_mvp/README.md) and
  [print_squares](samples/cli/basics/print_squares/README.md) commands.
  Ordinary CLI review needs real project acceptance; the native sample harness's
  test-owned acceptance does not establish that route. Run
  `mbx nextest run -p compiler --test samples_compile --no-fail-fast` with
  `OMEGA_SAMPLE_RUNTIME_FILTER=cli_mvp` or `print_squares` and
  `-E 'test(=samples_with_documented_exit_run_correctly)'` for the focused
  native probe. In PowerShell set `$env:OMEGA_SAMPLE_RUNTIME_FILTER = 'cli_mvp'`;
  in a macOS shell prefix the command with `OMEGA_SAMPLE_RUNTIME_FILTER=cli_mvp`.
  Old observations below are resume evidence, not a fresh
  baseline after the native planner cuts.

  Checked-compilation staleness: the service-reach evidence tightening
  (`22dc642ab8`, `7d444a9319`) left 135/140 samples failing
  "publishes service reach `<none>` but its checked body reaches undeclared
  services ..." at `2646d0a974`. The missing declarations are now declared:
  132 machine signatures carry their diagnostic-printed sets (`reaches` is
  transitive through machine calls — e.g. `Main::apply` in
  `cli/systems/account_ledger`), following the GUI cohort's pattern at
  `40c3332850`. Six distinct sample defects were fixed with it:
  `framed_payload`/`clamp_sum` explicit `as i32` domain casts,
  `array_index_from_call` i64-widened operands, `math_proofs` `embed()`
  ensures arithmetic, `uefi_hello` `&mut self` receiver, and
  `dungeon_crawler_cli`'s `==`/`!=` guard pair rewritten as a boolean
  transition. A full `all_samples_reach_checked_trees` rerun at
  `69fca41bda` (2026-09-16 UTC, macOS ARM64) reported 12/140 failing —
  `binary_search_viz`, `maze_flood`, `prime_sieve`, `multiplication_table`,
  `dice_histogram`, `calendar`, `dungeon_render`, `mandelbrot`,
  `mandelbrot_zoom`, `wire_protocol`, `dungeon_crawler_cli`, `math_proofs` —
  down from the earlier 19/140 and all inside the attribution families
  below; scoped rechecks are not a new full baseline. Samples still failing
  earlier phases may hide additional undeclared reaches; their owners should
  rerun and read the printed sets.

  Remaining checked-stage dependencies from that run: text/field proofs —
  `binary_search_viz`, `maze_flood`, `prime_sieve`, `multiplication_table`,
  `dice_histogram`, `calendar`, `dungeon_render` (cannot prove byte writes
  preserve the `Utf8` field domain across state edges). Receiver/aggregate
  loans — `dungeon_render`, `wire_protocol` (non-copy transfer out of borrowed
  storage). Index/subslice proofs — `mandelbrot`, `mandelbrot_zoom`,
  `wire_protocol`. `heat_grid` checks again: its `__hoist_N` temps were
  fed widened cyclic arrival facts (`y * 4 + x` analyzed as (0, 23)
  against the synthesized `[0..=11]`) because transition-delivered
  arguments seeded the target parameter's flow environment without
  intersecting the parameter's enforced declared range; guard-narrowing
  arrivals now apply the same clamp `record_assignment` uses for locals.
  Match custody —
  `dungeon_crawler_cli` (case-literal construction and branch-local
  transfer joins unsupported). `math_proofs` — its owned Bag/multiset row.

  Remaining `generic_counters` acceptance: run the
  [documented command](samples/cli/basics/generic_counters/README.md) with ordinary
  project review, and execute the exit-16 oracle on Linux x86-64/AArch64 and
  Windows x86-64. At `838a868432` (2026-09-14 UTC), the macOS ARM64
  compiler-library oracle passes with explicit test-owned acceptance after the
  canonical Bound Console migration; both counter instances and their logic are
  unchanged. `RUST_MIN_STACK=67108864 OMEGA_SAMPLE_RUNTIME_FILTER=generic_counters
  cargo nextest run -p compiler --test samples_compile --no-fail-fast
  -E 'test(=samples_with_documented_exit_run_correctly)'` exercises that route.
  Common wrapping-add and nested mutation/getter regressions publish all four
  targets, but cross-emission is not matching-host execution. Do not resume the
  closed missing-callee, wrapping-add, or physical-custody diagnoses.

  Native resume for `recursive_sum`: at `a1deabd205` (2026-09-14 UTC, macOS ARM64),
  `RUST_MIN_STACK=67108864 OMEGA_SAMPLE_RUNTIME_FILTER=recursive_sum cargo nextest
  run -p compiler --test samples_compile --no-fail-fast
  -E 'test(=samples_with_documented_exit_run_correctly)'` reaches
  `InvalidUnitMachinePlan` for `Main::main` (missing checked transitive plan).
  `recursive_slice_samples_reach_checked_trees` covers the unchanged
  `recursive_sum`, `dual_accumulator_recursion`, `subslice_sum`,
  `slice_accum_probe`, and `framed_payload` sources. The first producer gap is
  indexed primitive-array storage in
  `typed-trees-to-checked-trees/src/execution/unit/structural_scalar_store/build_structural_scalar_store.rs`:
  a primitive element has no record field ID. Extend ordinary primitive access
  with canonical paths as described below, not synthetic fields or helper calls.
  The unchanged i32-slice customer also needs typed scalar views (the existing
  byte-view vocabulary is u8-specific) and borrowed-view scalar loop parameters
  with SliceLength ranking under **GENERAL-CYCLIC-EXECUTION**. Coordinate primitive
  operation constructor/effect tests with the live **GENERAL-LICM** owner before
  changing shared representations. Derived-argument place comparison no longer
  needs a receiver-ancestry fix. The exit-70 native oracle remains open.

  `framed_payload` also reaches the missing `Main::main` Terminal-plan error
  on macOS ARM64 with the same native command and
  `OMEGA_SAMPLE_RUNTIME_FILTER=framed_payload`; its exit-60 oracle remains open.
  Its indexed `self.frame.bytes` stores need the primitive projection work below,
  not weaker receiver/argument overlap checking. The read-only checksum receiver
  and its live shared payload view must remain compatible.

  `dutch_flag` still reaches the missing transitive `Main::main` plan with
  `OMEGA_SAMPLE_RUNTIME_FILTER=dutch_flag` and the native command above (macOS
  ARM64, `67add79c88`). Preserve
  its enum-array swaps and exit-70 oracle; `dutch_flag_sample_reaches_checked_trees`
  isolates source acceptance. Static record/fixed-array paths now retain exact
  case observations through source checking, Terminal and native replay;
  `scalar_case_results::projected_membership` executes nested and owned-record
  reads on macOS ARM64. Continue through runtime-indexed sum reads, whole-value
  enum replacements/swaps and cyclic plan production, not another static-path
  adapter or implicit copyability for affine enums. The owning routes are
  `checked-trees-to-lowered-psi` structural storage and **GENERAL-CYCLIC-EXECUTION**;
  acceptance remains the unchanged sample's native exit 70.

  `cli/basics` documented-exit cohort at `69fca41bda` (2026-09-16 UTC, macOS
  ARM64), `OMEGA_SAMPLE_RUNTIME_FILTER=cli__basics` with the
  `samples_with_documented_exit_run_correctly` selector: 2 of 11 samples
  execute correctly (`cli_mvp`, `generic_counters`). Six reach
  `InvalidUnitMachinePlan` for `Main::main` — `brightness_control`,
  `nested_diagnostics`, `print_number`, `temperature_convert`,
  `text_greeting`, `unit_converter` — the same missing checked transitive
  machine plan family as `recursive_sum`, `dutch_flag`, and `window_app`
  above (**GENERAL-CYCLIC-EXECUTION**). `number_guess` now legalizes: signed
  32-bit saturating add, subtract, and divide reach native realization on
  all four targets (`SaturatingAddI32`/`SubtractI32`/`DivideI32`, clamped
  through a bound scratch; `MIN / -1` yields `MAX` without a fault path;
  the zero-divisor obligation stays proof-discharged), with
  `tests/native-differential/tests/scalar_case_results/i32_saturating_kernels.rs`
  as the executed regression. The sample's next first failure (macOS ARM64):
  `macOS hosted receiver bridge lost exact contract, storage, or entry
  custody` from `image-emission/src/hosted_receiver.rs`, which is the
  **ENTRY-CONTENT-ROOTS** receiver bridge, not arithmetic. Every fixed
  width now legalizes: the saturating kinds carry a `SaturatingCarrier`
  (i8..u64) instead of one name per width, narrow carriers clamp the exact
  64-bit result, i64 add/subtract detect overflow from the left operand's
  sign, and i64 divide never executes the trapping quotient; kernels for
  each width execute on this host and replay on all four targets
  (`scalar_case_results`). The `cli/basics` cohort's first failures (macOS
  ARM64) are now the missing transitive `Main::main` plan
  (**GENERAL-CYCLIC-EXECUTION**: `brightness_control`, `nested_diagnostics`,
  `print_number`, `temperature_convert`, `text_greeting`, `unit_converter`),
  the hosted receiver bridge (`number_guess`), `Utf8` field proofs
  (`multiplication_table`), and `OperationProofUnavailable(ObligationId(25))`
  (`print_squares`); none is a legalization error.
  `number_guess`'s remaining failure is not receiver storage: the bridge
  provisions its `[copy]` record and buffer once the Console field is
  spelled `Service<Console> in Bound`, and the unchanged sample then exits
  70 (macOS ARM64, ab5f28700d, probe reverted). The bare `console: Console;`
  instance field has no establishment row by contract, which is
  [owner question 1](OWNER_QUESTIONS.md); do not migrate the sample or add
  an eligibility path before that answer. `print_squares` now stops at
  `UnsupportedScalarOperation(WrappingIntegerMultiply { u32 })`: no integer
  multiply reaches legalization for any width (641 `*` sites across 238
  sample and corpus files), the next arithmetic family after saturation.
  `multiplication_table` still fails checked-stage `Utf8` field proofs and
  `print_squares` still stops at `OperationProofUnavailable(ObligationId(25))`.

  | Customer/dependency | Remaining work and owning route |
  | --- | --- |
  | `cli_mvp` ordinary CLI and hosted matrix | Finish real project package review with explicit owner acceptance, then run the documented CLI command. macOS ARM64 at `69fca41bda` (2026-09-16 UTC): `omega update --project samples/cli/basics/cli_mvp --target macos_arm64` renders three audit-recommended decision rows — the std external-realization `callable`, the Console `external_supply`, and the `FilesystemHost` `dangerous_capability` — and accepting them via `omega update --resume --project samples/cli/basics/cli_mvp` publishes `omega.lock`. The documented command then passes package acceptance and stops inside native production with `receiving terminal-authority permission policy has no exact row for Console::exit_process` (exit 1): `operations/compile_project.rs` defaults to the empty deny-by-absence receiving policy and no CLI input supplies consumer permission rows. That independently supplied receiving axis is **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** scope; do not mirror accepted package rows into it ([acceptance spec](wiki/spec/packages/acceptance.md) keeps it distinct). At e2447086af the package axis proposes and accepts the exit-permission row, so the command now stops at "omits the accepted permission"; the declaration that selects the receiving policy package is design-blocked on the `receiving-policy-selection` owner question. The macOS ARM64 compiler-library test already publishes and executes the unchanged source with exact two-line output, EOF and Enter, and exit 0. Establish the same customer behavior on the remaining hosted targets; cross-lowering alone is not runtime evidence. Linux x86-64 resume (`5c450f11ff`): the runtime probe passes checked semantics and Terminal publication, then native realization rejects the retained `&mut self` entry — no root-backed bridge provisions receiver storage (`native-realization/src/native_realization.rs` admits only macOS; `source/library/std/targets/` has no `linux_x86_64/entry.omg`). **ENTRY-CONTENT-ROOTS** owns the remaining hosted-receiver bridges ("realize retained receivers on both Linux targets"); the harness's test-owned entry binding is likewise macOS-only (`tests/support/macos_entry_acceptance.rs`). |
  | `print_squares` closure | Checked transitive Unit plans, byte-field presentation and storage-observation invariant scope are available. The probe still stops at `OperationProofUnavailable` for guarded-exit field obligations requiring a stronger counter/divisor invariant, and indexed bounds/increment overflow. Integer contradictions and saved field facts already have checked routes; do not invent another contradiction primitive or rebuild snapshot handling. **GENERAL-CYCLIC-EXECUTION** owns cyclic completion; **NOMINAL-FIELD-FLOW** owns declared field facts. Linux x86-64 resume (`5c450f11ff`): checked semantics pass and Terminal production stops at `OperationProofUnavailable(ObligationId(25))` — the nonzero-divisor obligation `1 <= self.place` on `digit_div`'s `self.sq / self.place`, the guarded-exit lockstep gap named above. |
  | Fixed-range Console input/output | Compose the selected source provider and real byte leaves with original receiver storage, exact returned cases/prefix, once-only effects, and cleanup. Use the ordinary graph and provider replay, not the deleted Unit/boundary planner. Windows byte I/O still needs imported-call/fixup/frame custody; Linux runtime evidence requires matching hosts. |
  | Receiver and aggregate operations | Finish shared/indexed projections, owned/local roots, scalar-result receiver calls, nested sum results and whole replacements, including mixed foreign-result assignments. Extend the shared statement sequencer; **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**, and **CML4** own the corresponding joins. |
  | Text and field proofs | Replace sample-local `Utf8`/compiler-name `valid_utf8` recognition with the [library encoding contract](wiki/spec/language/domains.md#byte-containers-and-encoding-domains). Keep raw bytes and qualify only validated prefixes; no byte-to-character re-encoding, hidden length writeback, or capacity-as-live-length proof. |
  | `cli/proofs/math_proofs` | Supply ordinary core multiset data and slice extraction, exact selected laws, and structural proof terms for indexed values/subslices. `core/seq.omg` is not a Bag implementation; equal lengths cannot establish equal contents. Preserve the false twin. |

  Keep ordinary CLI acceptance separate from the native sample harness's
  explicit test-owned policy; no project review decision was accepted by that
  test. Use `mbx nextest run -p compiler --test samples_compile --no-fail-fast
  -E 'test(=cli_mvp_preserves_both_lines_with_eof_and_enter)'` for exact output
  and both input cases. Installed provider calls share ordinary call transport
  while retaining independently checked provider selection, original boundary
  operands, result and completion custody. Preserve the canonical Bound Console,
  both writes and original 256-byte input buffer when closing the remaining routes.

  **STATE-LOCAL-VALUE-FRONTIER** still owns runtime-indexed primitive storage,
  policy-qualified array elements and native indexed owned/local roots. Reuse
  canonical primitive paths and the ordinary statement sequencer; retain exact
  bounds, root custody and element-sensitive storage observations. Do not invent
  scalar field IDs for array elements. Preserve
  `hosted_receiver_indexed_primitive_storage_survives_state_transition` while
  extending those routes: it covers borrowed helper mutation, a saved earlier
  read, an untouched sibling and a later-state read through real native entry.
  Neither that test's explicit package policy nor the storage capability closes
  the real CLI project's outstanding review or the other hosted runtime legs.

  **Scope pause:** do not resume helper-by-helper expansion for `print_squares`
  until a plan covers its complete source closure and native dependencies. Two
  earlier prerequisite milestones left the same missing Main plan. Preserve its
  source graph; unranked cycles require no invented termination witness.
  Independent operation work remains actionable. Package latency work belongs
  only to **PACKAGE-PREPARATION-REUSE**, not this execution task.

  Regression entrypoints: `tests/native-differential/tests/terminal_byte_views/`
  (`natural_writer`, `mutable_writes`, `byte_input`, `byte_output`),
  `scalar_case_results.rs`, and the selected Console reader controls in
  `compiler/tests/canary_suite/providers_float_and_console/console_reader.rs`.
  Use the [Terminal production map](omega-rust/psi/compiler/terminal-production/README.md)
  for current producers; past fixture publication is not current sample closure.
  General native joins/replay belong to **TRANSLATION-VALIDATION** in
  `TASKS_OPTIMIZER.md`. Keep exact source, ABI, occurrence, access and residual
  cleanup rather than restoring retired fixtures or whole-function families.

  Finish the [bounded-input](wiki/spec/resources/bounded_input.md) integration
  and honest hosted blocking/crash envelope in
  `build/selected-dispatch/src/compiler_intrinsic.rs`. The result schema does
  not itself supply a count-to-extent theorem; retain explicit prefix guards
  until callable relational evidence exists. Acceptance covers zero-capacity
  non-consumption, LF included at the last byte, EOF prefixes, Full without
  overread, repeated chunks, exact bytes including CRLF/NUL/multibyte input,
  failed-read traps, untouched tails, alias rejection, and caller continuation.
  Invalid result cases and unproved text qualification reject. Every maintained
  sample must check and every documented exit/output oracle must run on its
  matching host; record unavailable hosts separately from cross-emission.

- **CANARY-CORPUS.** Bring the language corpus to its promised checked/native
  stages, using `mbx nextest run -p compiler --test canary_suite --no-fail-fast`
  and the focused filters in [AGENTS.md](AGENTS.md#running-one-test).
  Repository/library gates do not establish corpus health. Reconstruct the
  current failure distribution rather than carrying forward the old claim that
  most failures are missing transitive Unit plans. Known dependency categories
  include **SAMPLE-CORPUS**, **NOMINAL-FIELD-FLOW**, selected entries and
  index/subslice proofs. Attribute each failure to its operation owner, not a new
  task per fixture. Passing one earlier checking phase is not full acceptance;
  rerun affected cases after each repair. Remove this umbrella once the corpus
  passes, not merely once every failure has a label.
  Attribution evidence: `CheckedUnitEffectPlans::omissions` records, per
  checked-body machine without a Unit plan, the stage that dropped it and the
  direct callee that was unavailable, and
  `LoweringError::InvalidUnitMachinePlan::omission` renders that chain to the
  machine whose own body failed local construction
  (`checked-trees-to-lowered-psi/tests/unit_plan_omissions.rs`, macOS ARM64).
  A local-construction row also names the ordinary builder's last phase
  (`LocalConstructionTrace` in `execution/unit/control`: signature, state
  contracts, statement sequence, call operations, completion, ...) and the
  call statement it was planning; a multi-state body names the general
  state-graph route's phase, state, and statement instead (`state graph:
  state signature`, `terminator`, ..., or the shared statement kinds); the
  single-state route's guard inside a phase is still not retained, so per-
  fixture reading there starts at that phase. The state-graph route names the
  guard beneath its phase since 9ff722c55f, and the 168-test state-graph
  subset rerun at that commit (macOS ARM64, base 07c1e746fe) splits as:
  operation custody 38 scalar call; conditional successors 33 guard expression
  and 1 parameter transfer; parameter custody shape 23 persistent receiver
  access, 8 borrowed non-view carrier, 2 qualified non-linear parameter;
  parameter signature 13 structural parameter type, 5 attached data shape, 3
  parameter qualifications; prefix initializers 13 bound expression, 3 short-
  circuit boolean; unsupported tail 11 transition chain, 2 single guarded
  transition, 1 jump followed by a transition; jump successor 2 scalar
  arguments; 2 claim-bearing successor; 2 now stop in the shared statement
  sequence at a local-data statement and 6 fail earlier on the `block`
  envelope acknowledgement check from 8508aec01e. The local-data arm of the
  shared statement sequence names its guard group beneath the phase since
  e0dfe91af5, and the 96-test local-data subset rerun at that commit (macOS
  ARM64, base b53585d23c) splits as: 40 structural call binding, 26 scalar
  call binding, 29 scalar local pure initializer and 1 initializer expression;
  none stop elsewhere.
  The 100 single-state stops rerun at 6ee6ad2f3d (macOS ARM64) split as: 36
  call statement shape; 29 in the structural field store (16 pure source, 12
  scalar field type, 1 destination parameter); 17 local-data statements (13
  scalar local pure initializer, 3 scalar call binding, 1 structural call
  binding); 7 call statements; 4 unconsumed nested calls; 3 trivial affine
  locals; 2 signature; 2 provider attachment requirements.
  The 36 call-statement-shape stops (e0c8333c4c, macOS ARM64) are all bodies
  with no admitted statement sequence: 23 keep a different call count than
  their call statements and 13 keep a non-call statement without a unit
  statement call (the roster names its index).
  Distribution reconstructed 2026-09-17 (macOS ARM64, `cargo nextest run -p
  compiler --test canary_suite --no-fail-fast` at 4dcb7723da): 1379 tests,
  223 pass, 1156 fail. By owner: 680 stop at Terminal production with
  `Main::main` lacking a checked transitive Unit plan (by the checked stage's omission roster: 541 are multi-state
  bodies that no composed control builder admits, **GENERAL-CYCLIC-EXECUTION**
  and the state-graph route, and the general route's trace places them at
  outer-call admission (294), a local-data statement in the shared
  statement sequence (96), the terminator (58), the state signature (56),
  operation custody (38), prefix initializers (16) and call statements (10);
  45 single-state Unit bodies stop in the shared statement sequence and 34
  at the call-statement shape; 12 reach a scalar callee with neither a
  registered target nor an ordinary body; the rest stop at signature,
  outer calls, trivial affine locals, or provider attachment requirements); 80 lose the
  macOS hosted receiver bridge's exact contract/storage/entry custody, 36
  select no exact program entry, and 22 fail the `MacosPhysicalEntry::enter`
  schema requirement (**ENTRY-CONTENT-ROOTS**); 66 are
  decision-17 exact-arithmetic overflow obligations and 55 are
  borrowed-storage transfers without an owner replacement
  (**STATE-LOCAL-VALUE-FRONTIER** value transport); 29 cannot prove a
  default-domain `[u8]::Utf8` field requirement on collection elements
  (**NOMINAL-FIELD-FLOW**); 13 index a non-array collection with an
  unsupported selected `requires` and 3 are unresolved authored Operator
  selections (**OPERATOR-MACHINE-SUPPLY**); 20 fail native selection on
  wrapping/saturating scalar operations or SourceCustodyMismatch
  (**WRITE-ONLY-BORROW** / selected legalization); 11 lower with
  `OperationProofUnavailable` (**BORROW-PROOF-CONVERGENCE** obligations);
  10 declare no service reach (Automatic service reach); the remaining ~70
  are singletons named in the run log. Read-only split of the 80 bridge
  declines (plus the cross-target `runtime_console_byte_echo_exit` compile) at
  914fad6e23 (2026-09-17 UTC): all 81 declare a bare `console: Console;` as the
  first `data Main` field, so every decline is the bridge's
  missing-Fused-establishment-row guard (`hosted_receiver.rs:600`,
  [owner question 1](OWNER_QUESTIONS.md)) and none reaches a storage guard;
  with that row, only `runtime_float_constant_store_exit` (f32/f64 leaves)
  tripped the storage-shape guard (`hosted_receiver.rs:632`) until 26484b4162
  admitted IEEE float leaves, while the
  other 80 already fit the admitted shapes (per fixture: 14 console-only, 23
  plain ints/bools, 19 `in Wrapping`/`in Saturating` ints, 3 zero-containing
  ranges, 3 `[u8; N]`/`[i64; N]` arrays, 21 nested zero-valid records of which
  4 are generic instantiations). Eight failures were fixture
  inventory drift from a checkout synced mid-run, not compiler behavior.
  The canary_suite tests that read removed report dumps (capability flow
  sites, the wire compatibility demand, numbered case identities, the
  accepted-axiom trust row) now read checked facts through the compiler API,
  and the capability-manifest entry test was deleted as renderer-only with
  its entry-state fact folded into the checked selected-entry test.
  Refined by the finer guards (reruns of the outer-call and state-graph
  subsets with the finer markers landed through 8fe2af5d9b): none of the 294
  stop at outer-call admission; they pass the statement loop and fail in the
  shared statement sequence's structural scalar field-store sequence, 258
  inside the structural field store (112 at the pure source, 105 at the scalar
  field type, 18 at the destination parameter, 13 at the carrier path and 10
  at the byte-sequence carrier) and 32 at write-frame agreement, where the
  resolver's inferred state write frame differs from the mutation summary
  (Utf8 string fields and record-literal field stores). Synthesized wire codec
  calls now frame exactly their exclusively borrowed argument places instead
  of reaching the ownership floor with the type-name receiver
  (`validation/.../write_frames/wire_codecs.rs`); on macOS ARM64
  `runtime_wire_decode_let_compare_exit` moved from write-frame agreement to
  `structural field store: destination parameter`, while
  `runtime_wire_encode_string_exit` and `runtime_wire_roundtrip_utf8_exit`
  still stop at write-frame agreement because an uninitialized
  `&[u8] in Utf8` local is opaque even with no codec call in the body; 3
  stop at an unconsumed nested call inside an assignment. The terminator stops
  are 34 conditional successors, 22 unsupported tails and 2 jump successors;
  the state-signature stops are 33 parameter custody shapes, 21 parameter
  signatures and 2 claim-bearing successors; 38 stop at operation custody and
  16 at prefix initializers. Of the "eight fixture inventory drift" failures,
  four were dump-reading canaries (since rewritten), two roster umbrellas pass
  after a fixture sync, and `pass_canaries_compile` is a corpus umbrella
  rather than a fixture.
  Fresh full run at ff0d8e4795 (2026-09-17, macOS ARM64, `cargo nextest run -p
  compiler --test canary_suite --no-fail-fast`): 1384 tests, 229 pass, 1155
  fail. 787 stop at checked Unit-plan construction with the declining guard
  named: structural field store 345 (157 pure source, 135 scalar field type,
  29 destination parameter, 14 carrier path, 10 byte-sequence carrier), the
  state-graph route 182 (39 operation custody scalar call, 37 conditional-
  successor guard expression, 24 persistent receiver access, 17 structural
  parameter type, 15 transition chain, 14 prefix-initializer bound expression,
  9 borrowed non-view carrier, 8 attached data shape, the rest singletons),
  local-data statements 152 (68 structural call binding, 52 scalar-local pure
  initializer, 31 scalar call binding), call statement shape 39 (26 call
  count, 13 non-call statement without a unit call), write-frame agreement 27,
  call statements 24, unconsumed nested calls 8, trivial affine locals 4,
  signature 3, provider attachment requirements 2. Outside plan construction:
  79 hosted receiver bridge (owner question 1), 66 exact-arithmetic
  obligations, 55 borrowed-storage transfers, 37 select no exact program
  entry, 24 fail selected legalization at physical staging (wrapping
  shifts/subtracts and `SourceCustodyMismatch`, the **WRITE-ONLY-BORROW**
  bucket), 22 `MacosPhysicalEntry::enter` schema, 20 default-domain field
  requirements, 16 attached Unit closures missing a transitive plan without an
  omission row, 13 non-array `[]` selections, 11 `block` envelope
  acknowledgements (8508aec01e), 11 `OperationProofUnavailable`, 10 service
  reach, 5 scalar callees with neither target nor body; the remainder are
  singletons in the run log. The 112 pure-source stops store
  Wrapping/Saturating arithmetic, atomics, float conversions or a call result
  into a scalar field without a bound pure scalar expression; the 105 scalar-
  field-type stops store into case, record, string or nested-record fields
  (**STATE-LOCAL-VALUE-FRONTIER** value transport). The 32 write-frame stops
  are opaque state write frames, not summary/resolver disagreement
  (`build_mutation_facts` stores what
  `CallFrameResolver::inferred_state_write_frame` inferred):
  `walk_state_write_prefix_inner` fails closed on an initializer-less or
  borrow-bearing record local without stored-origin evidence and on a record
  literal replacing a `self.` field (`stored_origins::assigned_stored_origins`
  accepts only local/parameter roots), both **STATE-LOCAL-VALUE-FRONTIER**,
  and on synthesized wire `encode`/`decode` calls, which have no write-frame
  model and fall to `demand::syntactic_call_written_paths`, whose type-name
  receiver root fails state-relative visibility; the codec route is now closed
  as recorded above.

- **TERMINATION-RANKING-CHECKS.** Complete the documented flow-dependent
  rank-range checks in
  `typed-trees-to-checked-trees/src/checks/termination/ranking/` and
  `validation/src/machine_calls/call_cycles/runtime_ranking.rs`.
  Transfers with diverging copies of rank inputs, and call components with
  internal state arrivals, or slice-length,
  bounded-distance, or custom views need
  exact arrival mappings and preserved premises for ranked subjects and pinned
  endpoints. Mutable premises need live write-frame evidence.
  Custom struct-view ranges involving borrowed or nested projections,
  constrained measure parameters, duplicated record roles, and dependency-free
  initial record arrivals need exact view-application evidence.
  Retire generated operand-call states through
  STATE-LOCAL-VALUE-FRONTIER's checked computation route rather than add ranking
  provenance for those artificial edges. Flow-dependent computed
  endpoint formation needs its own arithmetic proof, not an unchecked
  polynomial. Non-polynomial endpoint substitutions beyond exact input
  forwarding need their own equality evidence.
  Scalar views beyond bare unsigned identity forwarding, and slice lengths
  over projected storage need their produced-rank facts.
  These are implementation gaps, not grounds to weaken the range obligation.

  Acceptance: named-state and call-component rank ranges accept proved
  constraints while changed endpoints and intervening writes invalidate their
  premises. Preserve the private-witness/public-guarantee split described in
  chapter 3 and the
  [termination contract](wiki/spec/language/termination.md). Slice landed at
  f3187a535a on Linux x86-64: custom struct-view rankings now admit a
  dependency-free initial record arrival when the destination formal is the
  unique slot of the rank subject's nominal record type — the claim locates
  the record the view reads while field substitution, membership, endpoint
  pinning and descent still prove independently, so conflicting record
  lineages, foreign owners, out-of-range fields, stalled resets and missing
  entry evidence all keep rejecting. Declared identity measures may carry
  range-constrained parameter and result types: carrier classification
  unwraps range-only `Constrained` shells and the forward is selected only
  when the subject's enforced bounds fit inside every declared range, so an
  unsatisfiable or non-range refinement still fails. Slice landed at
  982dcfaf80 on Linux x86-64: runtime call components admit members whose
  machines carry multiple states — each member's discovered entry telescope
  is transported to the call site so authored subjects and endpoints
  normalize to the atom the site actually holds. Internal arrivals and
  self transitions stay with the member's own witness, requires clauses
  remain entry-site evidence, write-frame protection covers the site's
  non-self formals, mixed-range endpoint conservation still only covers
  entry call sites, and a duplicated entry role stays unbound rather than
  guessing between copies. Slice landed at e5fc9f06d6 on macOS ARM64:
  prefix stores before a transition or component call are judged against
  the premise carriers (`ranking_range_premise_symbols`: subjects,
  endpoints, requires-named inputs and range-constrained entries) located
  through each state's telescope, so a mutable scratch input may be written
  before the edge while a store into any carrier still rejects
  (`pass/termination/rank_range_{,call_}scratch_input_write`,
  `fail/termination/rank_range_{subject,endpoint}_intervening_write`).
  7232f16727 closes the vacuous acceptance that slice opened at runtime
  call sites: prefix stores also protect every slot sharing an entry role
  with another slot and, in mixed-range components, every role-carrying
  slot, so a written copy can no longer make a copy-equality guard dead
  (`fail/termination/rank_range_call_copied_input_write`,
  `pass/termination/rank_range_call_copies_with_scratch_write`).
  Diverging rank-input copies remain rejected by design of the checked
  route (`prepare(remaining, remaining)` then `prepare(left - 1, right)`
  fails "cannot prove rank range"; diverged copies select no convenient
  representative), so admitting them needs an arrival mapping that names
  the ranked copy, not a positional guess. Runtime call components admit
  declared identity measures at b6ebf8073b through the validation-owned
  classification (`declared_identity_view` in
  `ranking_range/identity_views.rs`) that the checked stage's
  `checks/termination/order.rs` now imports
  (`pass/termination/identity_measure_call_component`,
  `fail/termination/identity_measure_call_component_{domain,mixed_view}`);
  Declared scalar views beyond identity (`+`/`*` bodies over the single
  parameter) produce their rank from the body at 89444750cc/ec28993d1c
  under one strict-monotonicity admission shared by validation and the
  checked stage (`declared_scalar_view`, `RankingOrder::CustomScalarView`,
  `RankingRangeMeasure::Computed`): membership, descent and carrier
  formation are proved on the produced polynomial, never assumed
  (`pass/termination/computed_measure_rank_range` and four fail canaries).
  Nonlinear bodies reject because the engine's linear reasoning cannot
  bound their monomials. Runtime call components admit the same computed
  views at 233922f8ee (`RankOrder::DeclaredComputation`; formation proved
  for the source and destination rank at every call and for the initial
  rank at entry; `pass/termination/computed_measure_call_component` plus
  three fail canaries), and every termination-lane fixture from these
  slices is registered in the canary rosters. Runtime call components
  consume a ranged member's own range invariant at subordinate call sites
  (`prove_ranking_range_call` installs the membership and
  carrier-formation facts the member's state-edge judgment proves at each
  internal arrival; the checked stage runs that judgment for every ranged
  member with internal arrivals). Requires clauses remain entry-site
  evidence: a ranged callee's public `requires` at a subordinate site
  still needs the site's own proof because the ranking-to-requires bridge
  in `checks/contracts/call_bounds/context.rs` covers self-calls only;
  extending it is an open leg of this item.
  Struct-view rankings admit nested projection paths and borrowed subjects
  at 49d6f9e87e (`MeasureBodyShape::FieldProjection` carries the exact
  field chain; `order.rs` accepts a subject reaching the root record
  through a `Reference`; `pass/termination/measure_{nested_projection_rank,
  nested_projection_rank_range,borrowed_projection}` plus three fail
  canaries), and the relational range route transports them too:
  validation's `FieldCoordinate` is a projection chain re-resolved from
  the declared measure body (root record owned or reached through one
  reference, owned exact records at each step, u64 leaf), authored member
  chains bind to chain-identified atoms, and every arrival substitutes
  the literal chain rebuilt down to the field
  (`pass/termination/measure_nested_projection_range_{relational,pinned_limit}`,
  `measure_borrowed_projection_range`, three fail canaries). Named-state
  telescope transport for nested or borrowed record roles
  (`fresh_record_carrier` and mapping discovery still assume a direct
  owned nominal slot), reference boundaries inside a chain, and
  call-component struct views remain.
  Slice landed at 914fad6e23 on macOS ARM64: statement-position calls
  before a ranking transition or component call carry write-preservation
  evidence — a checked-body callee with inert arguments and complete
  direct and nested value write frames disjoint from every protected
  premise carrier is admitted, while incomplete or opaque frames,
  premise-carrier writes, authored-operator or effectful arguments, and
  bodyless boundary, requirement, or admitted declarations still reject
  (`pass/termination/rank_range_{,call_component_}prefix_call`,
  `fail/termination/rank_range_{prefix_call_premise_write,call_component_prefix_write}`).
  Still open on this item: diverging rank-input copies,
  exact slice-length/bounded-distance/custom-view arrival mappings, preserved
  premises, named-state transport for nested/borrowed record roles,
  STATE-LOCAL-VALUE-FRONTIER retirement of generated
  operand-call states, independent arithmetic proof for computed endpoints,
  equality evidence for non-polynomial substitutions, and produced-rank facts
  for scalar views and projected slice storage.

## Compiler throughput

These are bounded work-removal tasks, not a new performance framework. Preserve
the [pipeline ownership and independent checks](omega-rust/pipeline.md).
Source inspection identified the costs below; it did not establish their share
of whole-compilation time. Record compared inputs, work counts and timings;
do not claim a faster compiler from a smaller helper alone.

- **CRASH-GUARD-COST.** Investigate repeated classification in
  `typed-trees-to-checked-trees/src/checks/crashes.rs` before adding caches or
  threads. At `0989d752ae` on macOS ARM64,
  `RUST_MIN_STACK=67108864 cargo nextest run -p package-manager --test
  semantic_binding_review --no-fail-fast -E 'test(macos_entry)'` passes in
  540.531 seconds of execution. An unpublished snapshot-local integer-type
  index passed the same route in 532.465 seconds; that single 1.5% difference
  is inconclusive, so the prototype was discarded. See the
  [paired experiment](wiki/drafts/test_cycle_measurements.md#macos-package-review-classification-experiment).
  Slice landed at `d7c486d7ae`: route instrumentation showed crash-guard
  classification is a minor share of the checked stage (the crash checks and
  guard-coverage inference total ~2s of ~700s on the review route); the
  repeated work was three
  independent `StateMutationSummaryCache` builds per check pass (54 builds,
  ~50.6s across 18 invocations). One pass-owned table now serves fact
  construction, borrow-resource replay, statement borrows, and range
  checking, cutting builds to 18 (~16.6s, ~34s of repeated work removed,
  ~5% of the checked stage). Crash-guard positive/negative outcomes for
  parameters, locals and fields are preserved. Slice landed at
  `5521383c54`: `CallFrameResolver` is built once per immutable check
  window (`facts.rs` threads one resolver through flow classification,
  terminal ranking, termination progress and crash-route refinement;
  `finalize_execution.rs`/`selected_execution.rs` build a fresh one after
  the typed-program mutation) instead of ~20 reconstructions; whole-route
  `omega --check` on `terminal_psi/integer_control_contract` stayed at a
  4.90s median over 8 debug runs, so that slice removed rebuild work
  without a measurable route speedup. Remaining candidate:
  `validate_specialized_program`/`build_flow_facts` dominate the stage.
  Acceptance: compare unchanged whole-route inputs with repeated release/debug timings,
  remove material repeated work through existing typed ownership/type facts
  where possible, and preserve positive/negative crash-guard outcomes for
  parameters, locals and fields. Coordinate with live checking-stage work.

Optimizer revision/analysis reuse is tracked only in `TASKS_OPTIMIZER.md`.

## Automatic service reach

Carry [static callback reach dependencies](wiki/spec/language/effects.md#static-callback-reach-dependencies)
through Terminal evidence/replay. Preserve the original generic dependency and
exported binder telescope across the portable boundary. Use the finite union in
`flow-effects::ServiceReachInferencePlan` and checked service-reach facts; reuse
retained static-call contracts and exact specialization commitments rather than
introducing another selection identity. Closed applications must independently
replay exact substitutions from selected public contracts, including nested
generic/private helpers and recursive call components. Preserve direct boundary
declarations, pinned requirement bounds, and independent suspension/blocking
checks. No forwarding syntax, closure machinery, reach-prohibition syntax, or
backend effect inference.

Acceptance: the same named traversal with no-reach and Console callbacks publishes
empty and Console rows to ordinary callers and evaluation admission respectively;
the former evaluates when its other obligations hold. Missing direct boundary
declarations and selections exceeding requirement bounds reject. Cover nested
generic/private helpers, recursive call components, opaque/dynamic conservatism,
and unchanged summaries after call reordering/helper extraction. Adding Console
in a private helper or selected contract changes interface identity and invalidates
stale evidence; a no-reach requirement/evaluation context rejects it. Transitive
reach cannot be hidden by omission or a memberless clause. Fixed operational
markers and suspending-call positions remain enforced for every selection.
Keep `tests/omega/pass/effects/nominal_callback_const_reach/main.omg` checking
through the CLI and its structural/Console negative controls rejecting; constant
evaluation must not consume the source program's generic declarations.
Keep the source-to-canonical-policy regression in
`tests/omega/pass/effects/nominal_callback_dependency/README.md` passing when a
public generic also has closed applications; original interface identity must
not depend on which applications are present. Its private closed-application and
unused-generic controls establish template preservation, not Terminal custody.
Its `service_reach_contracts` command covers publication, reload and interpretation,
including inert declared reach, private helpers, and interleaved binder positions.
Run the README's Cargo/nextest commands on macOS ARM64 for full closed telescopes,
finite substitution, selected public rows, and two-sided emitted-call joins after
source discard.
Producer ownership is `checked-trees-to-lowered-psi/src/retention/closed_reach_applications.rs`;
reload checks are `terminal-verifier/src/validation/reach_applications.rs`.
Selected generic schemas use per-call closed tuples and exact callee application
joins; exercise source-free execution and hostile controls with
`tests/omega/pass/effects/generic_callback_schema_reach/README.md`.
The same projection covers fixed-only type/const applications, isolated identity
callbacks, and unused selected ordinary or generic contracts without emitted bodies.
Keep the [linear structural callback regression](tests/omega/pass/effects/structural_callback_reach/README.md)
publishing and executing after source discard while rejecting stale call and claim
custody independently of its retained reach application.
Unresolved installation selections keep their closed applications: provider
bounds ride inside the conservative selected and owner rows while the
requirement axis stays in the separate installation dependencies, so
substitution replay and stale-row controls need no new representation.
Broader portable coverage for inlined/missing callees and independently
checkable original-contract projection openings
remains required. Template/specialization commitments identify provenance;
they do not authenticate the structured projection. Original finite dependencies
replay against the retained source graph
in `validation/src/static_machine_call_contracts.rs`, but the remaining template
encoding contains frontend-local identities. Full source-contract reconstruction
and generic PCC remain open; ordinary relational replay does not establish them.

## Semantic reflection

Implement [semantic reflection](wiki/spec/language/reflection.md) in Psi schema
construction, hermetic evaluation, checked per-member calls, and Terminal replay,
with ordinary library inspector/encoder policies. Use named callbacks, explicit
context data, and existing callable-family checking; do not add a closure IR or
format-specific compiler. Anonymous syntax is not an implementation dependency.
First deliver owned qualified schema graphs, authorized projections, and scoped
typed selections frozen into independently checked result snapshots. Continue
with recursive derivation, explicit runtime metadata/adapters, and authorized
Placed access under the same contract. Fixed callbacks can proceed independently;
selection-dependent evaluation reach uses the Automatic service reach task above.

Acceptance: one inspector and one serializer use the same visitation mechanism;
a 40-field record with five field types uses five reusable policy rules plus
an explicit same-type member override. Missing, duplicate, wrong-type, stale-key,
and insufficient-contract selections reject. Cover full qualified field types,
ordinary predicate weakening and explicit semantic erasure in selected adapters,
no owned-claim erasure, erased-field runtime exclusion, zero-sized runtime fields,
empty records, distinct nullary cases, common fields once, and active payload only.

Test owner-delegated visitation across three packages and reject unauthorized
library enumeration; descriptions confer no grant or private access. Test fresh
reborrows and escape rejection, dynamic-index schema relationships, snapshot
independence from compiler storage, explicit exhaustion without truncation, and
recursive reference types reusing only the exact pending derivation while still
checking all obligations. Runtime recursive encoders use provisioned work storage,
not hidden stack growth. Retained adapters preserve loans, resources, and exact
Placed access; decoding and editing still require owner construction/update
contracts. Run corruption/replay controls independently of the producer.

## Scoped build execution

Implement [scoped build execution](wiki/spec/build/scoped_execution.md) for
ordinary code generators and the topology composition customer. These are
accepted contracts, not claims of implementation. Existing
`BUILD-ADMISSION-CHECKPOINT` owns retained frontend/generated-source replay;
extend that route, not a second build language or plugin executor. Any semantic
or trust amendment found here or later goes through [owner questions](OWNER_QUESTIONS.md).

- **BUILD-DEPENDENCY-PURPOSES.** Extend `omega-rust/omega/packages/` acquisition,
  review and locks, `omega-rust/omega/build/` declaration projection, and Psi
  package-aware resolution to retain explicit build/product edges and checked
  occurrences. Preserve one root Build and direct discovery. Host libraries use
  their own ordinary dependencies; schedule the combined purpose/profile/target
  prerequisite graph and reject cycles before affected execution. Version the
  lock/review migration without broadening legacy product edges or inferring
  permissions from imports. Acceptance: real acquisition and multi-file builds
  exercise dual-purpose std, conflicting cross-scope aliases, host/product targets,
  missing edges, dependency cycles and cache separation. Removing a build or
  product edge rejects only its authorized selections; source sharing never
  substitutes host output or admission for target evidence.
  CLI audit at c3aca8bbdf (macOS ARM64, product target linux_x86_64):
  dual-purpose std, conflicting cross-scope aliases, one package in both
  purposes, each scope rejecting the other scope's edge with the import,
  file and scope named, product cycles, and edge removal after
  publication all hold; `omega/tests/package_commands/build_purposes.rs`
  pins them through the shipped binary at 380c6f3fdb over
  `tests/fixtures/packages/build-purposes`. Build-scope sources select
  their target-scoped machines against the admitted build execution profile
  (`CheckedCompileRequest::build_execution_profile`, defaulting to the
  compiler host) while product sources keep the product target
  (`build-evaluation/src/admission/target_machines.rs::filter_target_machines_by_scope`
  over `AssembledSyntax::build_scope_sources`). The build scope holds the
  build entry, the root-local helpers it imports, and every physical source
  of a package the root reaches only through build-purpose edges, including
  such a package's own ordinary dependencies
  (`frontend/mod.rs::build_only_packages`, decided from the reconciled
  package graph rather than import order); witnessed by
  `assembled-syntax-to-checked-compilation/src/checking/execution_profile_tests.rs`,
  macOS ARM64 (a free target-scoped machine still cannot be imported by
  name, so helpers keep std's attached-machine spelling). Open: a package
  the root reaches through both purposes stays product scope, so its
  target-scoped rows still select against the product target, and a file
  imported by both scopes is rejected rather than checked twice (the
  cross-scope import diagnostic names the `builder.depend_as`/`build_depend_as`
  declaration that would close the gap); and non-root packages cannot author
  build rows, so cross-purpose cycles and per-helper build activations stay
  unexercised. A build-only package's generated dependency source now joins
  `build_scope_sources` beside that package's physical files
  (`source_assembly.rs::assemble_syntax` reads package ownership from the
  mounted logical path for both), so its target-scoped rows select against
  the build execution profile while a product package's generated source
  keeps the product target; witnessed by the two generated-source tests in
  `execution_profile_tests.rs` and
  `checkpoint/tests.rs::generated_source_joins_the_build_scope_with_its_build_only_owner`,
  macOS ARM64.

- **BUILD-PRODUCT-REFERENCES.** In Psi source selection and the existing Build
  root/provider owners, implement designated product operands and qualified
  non-callable entry/provider/schema descriptions. Depends on purpose-aware
  resolution above and the retained authored checkpoint. Define exact source
  signatures under the accepted roles without inventing a general compiler-query
  world. Acceptance: a multi-file helper receives an owner's restricted private
  entry reference and binds it without executing target code. Wrong scope/target,
  lookalike operations, helper enumeration of caller-private declarations,
  description-to-callable forgery and same-build generated/layout cycles reject.
  `build-evaluation/src/selection/root_bindings.rs` now admits executed
  `roots.bind` requests lexically against the occurrence's own package and
  carries the exact machine symbol into `ProgramEntry` selection, so a foreign
  helper binds its own package's entry through the borrowed root Build while
  caller-private names reject; `compiler/tests/build_target_activation.rs`
  pins both. Terminal production and native entry settlement now rejoin the
  selected machine by exact `SymbolHandle`, so the same-name-across-packages
  fence in `selection.rs` is removed. The compiler-owned `Build.product`
  facet and `BuildProduct::entry(path, slot)` logical query now issue an
  opaque non-callable `ProductEntryRef` description under the query
  occurrence's lexical package without executing product code; `roots.bind`
  accepts a delegated single-name implementation operand only as a retained
  `ProductEntryRef` place, and final admission rejoins the exact selected
  symbol after generation. `BuildProduct::schema(path)` now issues an opaque
  non-callable `ProductTypeSchema` description of the exact product data
  declaration under the same lexical-package rule, with `schema.path()` as
  the single sanctioned inspection; wrong-package queries, authored
  `BuildProduct` lookalikes, authored `ProductTypeSchema` forgeries,
  non-data names, ambiguity, and use as a `roots.bind` operand all reject in
  `compiler/tests/build_target_activation.rs`. `BuildProduct::provider(path)`
  now issues an opaque non-callable `ProductProviderRef` description of the
  exact authored provider declaration (a nominal data type owning at least
  one `satisfies` machine) under the same lexical-package rule, with
  `provider.path()` as the single sanctioned inspection; wrong-package
  queries, authored `BuildProduct` lookalikes, authored `ProductProviderRef`
  forgeries, non-provider names, ambiguity, `roots.bind` operand confusion,
  and static `select_provider` substitution all reject in
  `compiler/tests/build_target_activation.rs`. Remaining: the
  computed-receiver implementation fence once ordinary call-result
  authority and effect/loan traversal can carry that use.

- **BUILD-SNAPSHOT-OUTPUTS.** In build evaluation, its host custody adapters and
  compiler publication, implement coherent captured inventories, narrowed inputs,
  deterministic snapshot reads, fresh append-and-seal staging, linear required
  outputs, and direct artifact-only discovery. Define exact facet signatures and
  protocol tags; no live-host grant extension or persistent writable cache.
  `CheckedCompileRequest.build_snapshot` now binds a
  `package_compilation::capture_package_source_input` inventory (one coherent
  traversal producing both the canonical metadata index and retained bytes) to
  the occurrence: admission materializes a fresh sealed private snapshot as the
  Source grant root, `verify_required_outputs` enforces the declared roster
  linearly against sealed staged-output custody before generated-source
  selection, and the observation records `BuildCapturedSourceInventory` extent
  evidence (schema 76). `compiler/tests/build_config_granted.rs` exercises a
  package build reading a template through the snapshot and completing a
  required file, plus an omitted-required rejection, on macOS. A further
  slice landed at 17034140c0 (Linux x86-64, linw1): deterministic
  snapshot reads, inert symlink handling, and linear required-output
  settlement with 24 tests. The manager's review compilation
  (`packages/manager/src/review/candidate/compilation/package_pass.rs`)
  and the packaged `compiler::compile` route
  (`assembled-syntax-to-checked-compilation/src/checking.rs`, whenever the
  root binding carries the canonical Source metadata index) now bind a
  rosterless `BuildSnapshotRequest`: required outputs stay the obligations
  the build registers through `builder.output.require`, not a second
  roster inferred from the declaration.
  `omega/tests/package_commands/snapshot_outputs.rs` witnesses `omega
  audit packages --offline` on a root that reads a template through
  `builder.source` and completes `artifact.txt` (5-entry captured
  inventory, one settled output, exit 0) and the never-completed control
  (exit 1, nothing published) on macOS ARM64. Non-packaged `omega
  main.omg` compiles still run unbound; the audit report text does not
  surface the inventory. The `require`/`complete`/`fail` facet
  obligations, `OutputCompletion::Retry` custody, `artifact_only()`,
  narrowed negative lookups, deterministic reads, inert symlinks,
  substitution and forged-marker rejection, omitted-required ordering and
  interruption without a committed set all exist
  (`compiler/tests/build_snapshot_outputs.rs`, 24 tests); receipts live in
  the evaluator's private per-activation table, so one occurrence's receipt
  cannot settle another. 4428a61d9d releases the occurrence's private
  captured-source materialization on every exit (halted builds previously
  left read-only `omega-captured-source-*` trees in the host temp dir),
  witnessed through `omega audit packages --offline` for sticky `fail`,
  mixed completion, a forged obligation and an explicit retry in
  `omega/tests/package_commands/snapshot_outputs.rs` (macOS ARM64).
  `omega audit packages` reports the bound snapshot per package at
  306bf11022 (`build-snapshot captured-entries N captured-file-bytes M`,
  `settled-outputs N` with each path, sealed entries under `--details`).
  Retained state after `audit`/`install --offline` on the snapshot route
  and the generated-source handoff route: 0 B temp residue, 0 B private
  cache, lock 2837 B against 2843 B, `build/` 4763 B against 6046 B; the
  separate build-tool-package route the spec keeps as an alternative is
  not implemented, so no measured comparison against it exists.
  bf8b52c77c adds the spec's cross-build comparisons to
  `omega/tests/package_commands/snapshot_outputs.rs` on macOS x86-64:
  two packages settling identically named `templates/banner.tmpl` inputs
  and `artifact.txt` outputs from their own captured inventories and
  staged custody down to each sealed entry's retained bytes, one package
  settling an independent occurrence per requested target off the shared
  source-preparation slot, a dependency's completed output not publishing
  through the root's uncommitted set, and one target's uncommitted set
  reporting its own rejection without hiding another's settlement.
  Remaining: Windows host coverage. Acceptance: an
  ordinary generator reads a template and completes a required file;
  artifact-only and executable-with-companion routes both work. Exercise
  negative lookups, ordering/metadata, symlink and substitution escapes, sealed
  mutation, cross-occurrence receipts, failed completion/retry, omitted required
  members, interruption, and final-check failure without a partial committed set.
  Test isolation and failure on Windows/macOS, explicitly recording unavailable
  hosts. Measure retained input/output state and compare the separate-tool route;
  do not infer confinement or usability from parser tests.

## Build-level behavior exclusions

Implement [the accepted exclusion contract](wiki/spec/build/behavior_exclusions.md)
for one source library whose checking/no-op assertion selection changes product
crash behavior without edits to public ceilings. Assertions remain ordinary calls;
there is no special Assert cause or global permission to violate callable contracts.
These tasks use existing build selection, portable semantics, provider and
artifact-verification owners, not an assertion-specific interpreter or duplicate IR.

- **BUILD-SEMANTIC-EXCLUSIONS.** In `omega-rust/omega/build/`, define exact typed
  configuration and canonical union of crash/service exclusions. Psi checking and
  Terminal evidence own possible semantic operations, closed calls and guard
  proofs; Omega selection and product admission join exact selected implementations
  under the complete entry/dependency closure. Reuse existing evidence rather than
  copy public summaries into a purported actual-behavior verdict. Keep ordinary
  declarations unchanged, forbid effect masking, and reject incomplete or opaque
  evidence conservatively. Depends on the selected-call/entry evidence exercised
  by the customer; missing coverage must remain an explicit implementation limit.

  Acceptance: compile identical library source/public Trap ceilings with checking
  and no-op implementations; under a Trap exclusion only the no-op composition
  passes, with optional optimizations disabled and enabled. Eager predicate traps,
  unrelated traps, callbacks, cleanup and generated entries remain accounted.
  A silent ordinary logger can pass no-Console; a silent provider for an actual
  Console invocation cannot. A build permitting Trap never bypasses a no-crash
  intermediate interface. Independently consume retained evidence and reject
  substituted entries, providers, scopes, targets, policies and missing summaries.
  Report prohibited possible behavior separately from insufficient evidence.

  Resume: `build-evaluation::behavior_exclusions` holds the typed
  `BehaviorExclusion`/`BehaviorExclusions` canonical union and
  `establish_behavior_exclusions`, which walks the unoptimized Terminal static
  call closure per entry and reports prohibited sites (crash terminators,
  boundary fixed service reach and declared crash routes, port writes) separately
  from evidence gaps (dynamic calls, unknown entries/callees/boundaries), never
  consulting in-module public ceilings. Boundary calls now join the selected
  provider-plan facts: a `CheckedAdapter` binding walks the retained candidate
  body whose `candidate_identity` it names, while external bindings, unmatched
  adapters, and unconstrained slots retain the declared contract (unconstrained
  slots additionally walk every retained candidate). Bounded dynamic dispatches
  rejoin the dispatch catalog's exact realization; parameter dispatches remain
  gaps. The product-admission join lives in
  `checked-compilation-to-terminal-artifact::terminal_artifact::behavior_exclusions`,
  which re-lowers each selected entry unoptimized before the artifact is
  admitted and checks callback thunk bodies at their own production site. Unit
  coverage: `cargo nextest run -p build-evaluation behavior_exclusions`.
  The compiled corpus landed at c5e9124522:
  `tests/fixtures/packages/behavior-exclusions/` composes one `assert-kit`
  library (`Assert::check` under a public Trap ceiling, `CheckingAssert`
  and `NoOpAssert` providers) into `checking-app` and `no-op-app` under
  `exclude_crash(CrashCause::Trap)`; through `omega install --offline` and
  `omega --target linux_x86_64` the no-op composition passes exclusion
  admission with `ControlFlowCleanup` enabled and rolled back while the
  checking composition rejects as prohibited in both states, and the
  diagnostic now attributes the crash site through the provider
  (`reached through machine \`CheckingAssert::check\`` plus a
  "declared here" row from `MachineProvenance` in
  `terminal_artifact/behavior_exclusions.rs`;
  `compiler/tests/behavior_exclusions.rs`, macOS ARM64). Open: a Unit
  machine whose own state crashes has no checked Unit plan
  (`InvalidUnitMachinePlan`, so the corpus routes the verdict through a
  scalar helper), boundary requirements carrying a crash contract are
  refused by native lowering (`UnsupportedBoundaryCrashContract` blocks the
  passing composition's native product). `builder.exclude_service<Trait>()`
  landed across psi (c28bec23eb parser/validation/intrinsic/interpreter,
  fb19ed98a3 symbol binding of the type path) and build (4fb56304c6:
  harvest into `AuthoredBehaviorExclusionKind::Service`, per-module
  `ServiceId` resolution on both product routes, and `BoundaryServiceOwners`
  so a boundary call counts its owning service and parent closure). The
  service rows are witnessed on the `logger-kit`/`quiet-logger-app`/
  `sink-app` corpus: a silent ordinary logger passes the exclusion while a
  real invocation behind a silent provider is prohibited with the call
  site and the authored marker spanned. The passing quiet-logger native
  product then stops at `RootConcreteServiceReachMismatch` (a root
  declaring reach it never exercises), reproduced without any exclusion,
  so it is a native-pipeline limit rather than an exclusion verdict.

- **BUILD-EXCLUSION-REALIZATION.** Extend the existing receiving-policy and native
  admission route in `omega-rust/omega/build/`, `omega-rust/omega/semantics/` and
  target backends to enforce excluded physical terminal classes without inventing
  a second classifier. Join semantic exclusions from the preceding task through
  final realization and the component installation/replacement envelope. Reuse
  `COMPONENT-SUBSTRATE` and `WIRE-RUNTIME-AND-INSTALLATION` for actual custody and
  lifecycle; no build-time callback may inspect its own unfinished executable.

  Acceptance: distinguish no-Console from no physical process output, reject
  unknown/changed mechanism classifications and failed final checks without
  successful publication, and preserve exact policy/evidence identity through
  source-free consumption. Rebinding or replacing code with an excluded behavior
  must fail the existing envelope even when the old provider was benign. Keep
  target children and build-host activity scoped separately. Exercise physical
  provider and installation controls on each available supported host, explicitly
  reporting unavailable Windows/macOS coverage; a semantic-only pass establishes
  no physical absence claim. Specify versioned source/protocol fields before
  claiming compatibility. Any unresolved semantic or trust change goes through
  [owner questions](OWNER_QUESTIONS.md), not a weakened implementation verdict.

## Checked boundary topology

Implement the [reference-package contract](wiki/spec/packages/topology.md), not
a compiler graph stage. `COMPONENT-SUBSTRATE` owns the complete verified component
consumer below; `WIRE-RUNTIME-AND-INSTALLATION` owns generic executable custody.
Package policy and orchestration stay in ordinary Omega libraries, with native
details in providers. A topology-specific IR or new trusted graph axiom is not
an implementation shortcut.

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver an ordinary build-only package and
  composition project over prebuilt component artifacts. Depends on the scoped
  build output route and independently verified complete descriptions; missing
  evidence must not be replaced with hand-authored inventories. Implement fixed
  `no_route`/`only_via`, bounded deterministic graph normalization and diagnostics,
  correctness evidence, exact owner-supplied `TopologyRequest`, and versioned
  codec tables/fixtures. Acceptance: the payment graph succeeds and direct/indirect
  bypasses reject with checked witnesses; cover cycles, disconnected sources,
  duplicates, stale subjects, forged completeness, missing owner policies and
  unselected policy executables without loading them. An independent source-free
  consumer reconstructs the same graph, rejects corrupt plans, and replays the
  selected predicates. Typechecking a graph algorithm alone is not correctness
  evidence, and successful publication is not an installation claim.

  Resume evidence: the Rust reference slice landed as `topology-plan`
  (`omega-rust/omega/packages/topology`): canonical model, bounded
  normalization, `no_route`/`only_via` with structural certificates and
  checked violation witnesses, versioned codec with golden
  `tests/fixtures/payment.{request,plan}`, producer `compose_plan`, and
  source-free `verify_plan` covering the acceptance rejection matrix
  (`mbx nextest run -p topology-plan`: 76/76 at 960a736352, macOS x86_64).
  Remaining: the build-only Omega package and composition project itself,
  consuming verified component descriptions over the scoped build output
  route — the crate deliberately does not accept hand-authored endpoint
  inventories as verified evidence.

  Seam map re-probed at 5f74f97ab9 (macOS x86_64; `omega audit packages
  --project <dir> --target linux_x86_64 --offline` against composition
  projects): build-scope nameability landed at 874b31f2a0, retiring the
  first blocker. `use topology::policies` now resolves through a
  `build_depend_as` edge in the root build.omg and in a root-local helper
  source transitively imported by it (fixture
  `tests/fixtures/packages/build-scope-topology`, regression tests in
  `omega/tests/package_commands/inspection.rs`): the imported machine runs
  at build time, the edge records as `[build dependency 0]`, and the same
  spelling under product scope rejects "names build dependency
  `topology`". A scratch composition project combining the import with
  `builder.application` + `builder.artifact_only` +
  `builder.output.require`/`resolve`/`create`/`write`/`close`/`complete`
  on `payments.plan` finishes audit, and leaving the obligation pending
  still rejects "required output `payments.plan` of `build` was declared
  but never completed". `builder.source` reads files inside the
  requester's own package root but rejects `../dependency/...` escapes
  ("build-root path must use canonical relative components"): its
  `read_roots` bind only the requesting root, so evaluated code cannot
  reach descriptions staged beside a dependency package.
  `BuildSnapshotRequest` is now bound on both compile routes (see
  BUILD-SNAPSHOT-OUTPUTS), so the manager's review observation carries the
  captured inventory and settled required outputs.
  `verify_component`/`VerifiedComponent` and the schema-V1
  `ComponentDescription` codec exist in
  `omega-rust/omega/backend/artifacts/component-candidate` (with
  `component-deployment` on the native install side), but
  `describe_component`/`describe_component_facts`/`verify_component` have
  zero callers outside that crate: nothing produces a description in the
  compile path, and no evaluated-build admission surface exists — the
  build prelude exposes `source`/`output`/`log`/`product` facets and no
  component facet, so evaluated Omega cannot name or verify a
  `ComponentDescription` at all (a hand-authored `.desc` the root could
  read through `builder.source` is exactly the hand-authored inventory
  this item forbids) — COMPONENT-SUBSTRATE. `CompositionMode::Independent`
  still rejects at provider-planning's component-closure fence
  (`selection_provenance.rs`), though an `artifact_only` composition
  selects no providers, so that fence is not on the next acceptance's
  path. No Omega-authored topology package exists yet (`source/library/`
  holds only alloc/core/std; `omega-rust/omega/packages/topology` is the
  Rust reference). Next acceptance when the seams land: the composition
  project compiles `use topology::...` under the build scope, admits
  three verified component descriptions, and publishes the checked plan
  as a required artifact of an `artifact_only` build.

- **TOPOLOGY-PRIVATE-PIPE-INSTALLATION.** Build the package-owned installer and
  Windows/macOS pipe adapters for three checked local payment processes. Depends
  on plan verification, complete component admission, and generic executable
  installation. Use one bounded request/response pair per binding, one outstanding
  request, no general inheritance, dynamic delegation, discovery, or retries to
  another peer. Acceptance: independent current request authorization, exact
  all-import binding coverage and confined endpoints precede application entry;
  an actual ungranted invocation and substituted mapping are refused. Preparation
  failure returns/cleans custody; partial activation retains supervision and
  quiesces or reports cleanup failure without a success receipt. Replacement
  quiesces the old generation before starting the new one. Test both supported
  hosts when available and disclose loader/OS assumptions; graph-only tests,
  signatures, and ordinary process spawn do not prove physical confinement.

  Reuse the Rust reference `install/` in `omega-rust/omega/packages/topology`.
  Its supervisor-owned lifecycle issues opaque single-use authority separately
  from copyable request intent, and replacement checks current authorization
  before stopping the old generation. Complete the Omega-authored installer,
  actual three-process executable/component admission and OS-backed supervisor,
  and contract-level operation-schema checking on bounded frames. Reference
  supervisor tests and real pipe I/O alone do not establish process confinement
  or the three-process customer on Windows/macOS.

## Process-exit contract

Implement the [canonical process-exit contract](wiki/spec/language/process_exit.md)
for CLI programs that deliberately exit, including conditional exits through
helpers. Core must declare the exact `ProcessExit::exit_process(i32)` requirement
and separate its authority/reach from console I/O. Psi owns canonical recognition,
checking, conditional control and ownership, Terminal completion, codec,
independent verification, observation, and interpreter support. Omega owns
provider conformance and realization; targets supply domain-bound authority and
physical exit under the same contract. Preserve the current trace distinction;
do not infer terminal meaning from the transitional Unit call or a native syscall.

Acceptance: source-produced unconditional and conditional exit cases retain
exact requirement/argument identity through canonical replay and target
realization. Returning branches retain cleanup and postconditions; exit branches
abandon without discharge receipts. Reject missing authority/reach, lookalike
requirements, returning or aborting substitutes, unmet progress premises, exit
from automatic cleanup, and unsupported survivor contracts. Root return cannot
hide unfinished task custody. The interpreter ends only its simulated domain;
native tests on each available host preserve status mapping and ordered output,
with unavailable-host coverage explicit. Tampered identity, arguments, completion
rows, or provider evidence reject independently. General completion syntax and
other terminal services are not prerequisites.

  Landed: `ProcessExit::exit_process(i32)` is declared in core/std with
  per-target `process_exit_impl` providers and recognized as a canonical
  requirement bound through selected-dispatch hosted custody — the interpreter
  ends its simulated domain with the exact status and drained trace, and native
  realization settles `HostedExitProcessI32` rows under `SelectedProcessExit`
  custody (`tests/omega/pass/host/process_exit_i32_status{,_ordered}`,
  macOS arm64 native run exits 70 with byte-ordered console output; Windows
  provider declared, host execution unavailable). Status mapping is witnessed
  by `process_exit_i32_status_mapped`: semantic statuses outside the host's
  8-bit presentation (-44 taken, 300 untaken) stay exact through the checked
  interpreter and each branch's `SelectedProcessExit` custody, while the
  macOS arm64 physical run presents the low byte (212). At c2a47182f6 the
  ordinary CLI review proposes the root consumer's `Console::exit_process`
  `terminal_permission` row (`process_termination`) as one blocking
  decision (`review/candidate/semantic_bindings.rs`, root-only because
  every package's open permissions propagate into one root policy);
  accepting it publishes a lock whose accepted policy carries the row, so
  `omega --target macos_arm64` on a downstream application now stops only
  at the empty CLI receiving policy ("omits the accepted permission"),
  which is TWO-AXIS-TERMINAL-AUTHORITY-REVIEW's receiving-axis input
  (`PreparedLocalProjectNativeRequest::with_receiving_terminal_authority_permission_policy`
  has no caller under `omega-rust/omega/src`) rather than test-owned
  acceptance (`package_commands console_exit_permission`,
  `tests/fixtures/packages/console-exit-app`). At 30fbd45660 the same
  discovery also proposes the `write_byte` (`process_output`) and
  `read_byte` (`process_input`) rows the nominated Console provider
  declares as compiler-intrinsic leaves, one blocking root-only decision
  each at the selected provider's declared-row granularity (package review
  carries service-level reach; final closure admission owns
  demand-completeness): a provider declaring only the exit leaf proposes no
  byte row (`review::candidate::compilation::tests::discovery_proposes_*`,
  1 against 3 rows), and console-exit-app now writes a line, accepts 3 rows
  into the lock and stops at the empty receiving policy for all three
  (4121d83206). The surface that selects the receiving
  policy package is design-blocked on the `receiving-policy-selection`
  owner question. Open: the CLI receiving policy,
  remaining hosts' physical runs, root-return/task-custody survivor
  contracts, and general completion syntax.

## P1 - Authority, roots, and entry

Owners include
`wiki/spec/resources/authority.md` and
`wiki/spec/resources/storage.md`.

- **ENTRY-CONTENT-ROOTS.** Connect the generated physical entry to the exact
  semantic continuation under the [entry contract](wiki/spec/build/entry_roots.md).
  Owners: target package source assembly, `target::TargetProfile::program_entry_slot`,
  `program-entry-plan`, `external-roots`, and
  `native-realization/src/native_realization.rs`. Targetless checks select
  no entry; deployment cannot substitute a semantic machine for a physical adapter.

  Complete the remaining hosted bridges and receiver lifecycle, not another
  metadata-only calling-plan milestone. The macOS contract is
  `source/library/std/targets/macos_arm64/entry.omg`; its physical arrival and
  `ProgramStorageEntry` are distinct applications. Provider-module imports do
  not load that contract automatically. The `calling_policy_plans macos_entry`
  test exercises signatures, not installed roots.

  The macOS bridge already executes authored receiver processes with no
  test-supplied pointer: scalar/array storage, disjoint record copies, captured
  borrowed-call results, Fused Console, normal return and explicit process exit.
  At `673d24c5e7`, macOS ARM64, `RUST_MIN_STACK=67108864 cargo nextest run -p
  compiler --test canary_suite --no-fail-fast -E
  'test(entry_and_abi::hosted_receiver::)'` exercises that route. Continue from
  `native-realization/src/native_realization.rs` and
  `image-emission/src/hosted_receiver.rs`, not from an assumed absent adapter.

  Remaining: realize retained receivers on both Linux targets and Windows;
  support executable nominal cleanup and callback/signal occupancy through the
  actual activation/completion contract; extend receiver storage beyond the
  admitted plain-record/primitive-array shapes. Corpus demand at 914fad6e23
  (2026-09-17 UTC, the 81 bridge-declined canaries read only): every one is
  blocked first by the row-less bare `console: Console;` field
  ([owner question 1](OWNER_QUESTIONS.md)), not by storage; after that the
  corpus needs nothing else new: the IEEE float leaves it demands
  (`runtime_float_constant_store_exit`, f32/f64) are admitted as top-level,
  array-element and nested zero-valid record storage (zero-filled bits are the
  exact `0.0`), witnessed on macOS ARM64 by
  `entry_and_abi::hosted_receiver::hosted_receiver_provisions_ieee_float_leaves_for_constant_stores`
  (`tests/omega/pass/expressions/runtime_float_receiver_storage_exit`, f64 and
  f32 fields beside a Bound Console, exit 70), and the other 80 fit
  plain/domain/range integers, `[u8; N]`/`[i64; N]` arrays and nested
  zero-valid records (generic instantiations such as `Pair<bool>` and
  `FixedBuffer<4>` included). The private-resolver-storage
  Linux leg of the nominal machine-parameter witness
  (`tests/omega/pass/generics/runtime_nominal_machine_parameter_satisfaction_exit`,
  exit 70 on macOS ARM64 through
  `generics_and_dependent_facts::runtime_nominal_machine_parameter_satisfaction_exit_canary_runs`)
  stops at this receiver bridge on Linux x86-64 and belongs here. Keep the current unsupported
  cases rejecting. These are implementation dependencies under the settled
  contract, not unanswered language decisions.

  Preserve the existing macOS loader-backed RW/NX partitions, private-stack
  switch and saved physical continuation. Compose final application, bridge and
  provider demand with any newly admitted callback occupancy; `LC_MAIN.stacksize`
  is not remaining-stack evidence. Checked receiver eligibility binds the exact
  source owner and retained/erased projection; erasing an unused borrow cannot
  erase initialization, nominal cleanup or Fused establishment obligations.
  Actual occurrence custody remains with installation, not compilation of the
  conditional native product; coordinate that join with
  **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION**.

  Resume evidence: `cea82370e6` adds
  `ProgramLocalExtentRegistry::materialize_aggregate`, which discharges a
  reconstructed epoch aggregate capacity over the group's actual installed
  backing partitions for the same occurrence/epoch. The installation ledger
  re-derives the complete live membership, so stale, substituted,
  foreign-lifecycle, omitted, or repeated member sets reject transactionally
  with their inputs; each backing must equal its member's evaluated interval
  in one shared address space, and the presented receiver partitions must
  compose the exact reconstructed interval set — overlap rejects. Minted
  Extents carry activation loans (`Extent::loan`/`loan_mut`) and `retire`
  completes the occurrence, returning the partition for rejoin into installed
  storage. Witnessed on Linux x86-64 by `cargo nextest run -p external-roots
  --no-fail-fast` (213/213), including
  `aggregate_materialization_discharges_reconstructed_capacity_over_installed_partitions`,
  `aggregate_materialization_rejects_stale_substituted_and_inexact_discharge`,
  and `counted_aggregate_capacity_cannot_discharge_extent_partitions`. The
  native-realization entrance is sibling-owned; the receiver-side bridge and
  macOS contract remain open below.

  Witnessed at ab5f28700d (macOS ARM64): a bare boundary-trait receiver field
  (`console: Console;`) is classified `ProviderBacked` in
  `typed-trees-to-checked-trees/src/execution/unit/types/build_types.rs`,
  lowered to the same Terminal `Erased` field shape as a Bound field,
  admitted by `terminal-production/src/terminal_production/receiver_eligibility.rs`
  without a `fused_service_fields` entry, given no
  `ProgramEntryFusedServiceEstablishment` row by
  `selected-dispatch/src/service_custody/root.rs`, and therefore rejected by
  `image-emission/src/hosted_receiver.rs` (exactly the
  `hosted_receiver_rejects_bare_interface_without_bound_establishment`
  canary). Whether that field is establishable is
  [owner question 1](OWNER_QUESTIONS.md); if it is, the route is a positive
  Psi eligibility witness carried through `NativeProgramEntrySettlement` and
  checked at the bridge, never a missing-row fallback.
  Acceptance: execute an authored receiver entry as a published process with no
  test-supplied `self`. Reject redirected continuation/receiver identities,
  non-ZII state, insufficient/misaligned backing, overlapping partitions, stale
  occurrence/epoch and bypassed provisioning. Retain the native admission
  rejection for unsupported targets and lifecycles. Exercise the
  `traits/runtime_local_named_dyn_stored_exit` customer (exit 70 on both Linux
  targets), subject to its separate descriptor-call dependency. Preserve exact
  symbol/text, source contract, runtime storage and continuation replay.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Finish the two-surface UEFI bridge: the
  target-package physical firmware entry remains distinct from the semantic
  program continuation. Emit and validate the adapter, exact calling plan,
  stack/custody transfer, and return behavior. Application lookalikes and
  cross-target substitutions must reject.

  Resume with checked source through Terminal, child emission, and the entrance
  in `native-realization/src/optimized_semantic_wrapper_object/mod.rs`.
  That wrapper-publication entrance currently has no callers;
  its object tests use constructed contracts, so further isolated binder work
  is paused pending this exercising path. Native declaration preflight is not
  executable bridge acceptance. In `program-entry-plan/src/optimized_semantic_entry/validation.rs`,
  `validate_method` still compares schema application identity with raw ABI-plan
  identity; retain exact source-application custody through the native binder
  instead of dropping this check or adding an upward backend dependency.
  The last full Terminal probe of `build/uefi_program_entry_storage_roots` stopped
  in lowering because `Boot::launch` lacked a checked transitive Unit machine plan.
  Its checked-source/preflight regression is not complete Terminal or firmware
  execution acceptance. Close these joins before claiming an executable bridge.

- **UEFI-OS-HANDOFF.** Implement the nonreturning custody transfer from Boot
  Services to the selected OS entry. The bounded memory-map/key retry loop must
  return all custody on stale-key failure and consume boot-scoped services only
  on success. Acceptance includes stale-key, exhaustion, lost-custody,
  post-exit provider-use, and successful handoff canaries.

  Resume evidence: `6be7b64923` landed the lifecycle-scoped `GetMemoryMap`
  acquisition edge in
  `omega-rust/omega/backend/runtime/external-roots/src/uefi_bootstrap/get_memory_map.rs`,
  the sole issuance boundary for `UefiMemoryMapAcquisition`, over the earlier
  `aabbe797d1` `ExitBootServices` provider edge. Acquisition borrows the pending
  exit invocation's live custody: `EFI_BUFFER_TOO_SMALL` returns provider and
  buffer custody with the required extent for grow-and-retry without spending a
  handoff attempt, `EFI_SUCCESS` seals the exact map key and descriptor
  geometry into evidence the handoff ledger requires before forming
  `UefiOsHandoffMapAcquired`, so the key reaching the exit binding always names
  the most recent firmware map; every other status retains executed custody for
  release. Witnessed on macOS arm64 by external-roots tests (159/159) driving
  real `efiapi` service pointers through fabricated UEFI tables: size probe then
  grow-and-rebind to success, bound exit operand and applied completion
  carrying the acquired key and snapshot, stale-key retry reacquiring a fresh
  map, foreign session/invocation and cross-ledger rejection, and
  contract-violating success cells retaining executed custody.
  `os_handoff_cycle.rs` now lands the caller-side sequencing:
  `drive_uefi_os_handoff_cycle` composes the bounded cycle in the only legal
  order — acquire under the pending exit's custody (grow-and-retry spends no
  attempt), register the sealed acquisition, bind the pending exit to that
  exact key, execute and admit, then apply — and returns terminal resolution
  or stage-exact live custody on every rejection. Witnessed on macOS arm64 by
  external-roots tests (167/167): freshest-key ordering across a
  grow-then-stale-key retry, exhaustion returning releasable provider custody,
  and acquisition/admission/binding rejections re-driving or releasing intact.
  Both runtime legs consume the `program-entry-plan` OS-handoff invocation
  plan: service row, call shape, and status roles come from the retained leg,
  and the exhaustion status is admitted through the plan's error predicate.
  Remaining: the generated custody-transfer adapter and physical shell that
  realize `UefiOsHandoffNativeProvider::handoff` in
  `source/library/std/targets/uefi_x86_64/handoff.omg`; that authored surface
  stays planned and non-invoked, so no Omega program reaches this edge yet.
  Surveyed at a9c92ec31e: the checked-level canary
  `tests/omega/pass/build/uefi_os_handoff_invocation` retains the edge binding
  and stops at `compile_to_checked`; the only via-less boundary-machine
  realization pattern is the compiler-intrinsic catalog (`selected-
  dispatch/src/compiler_intrinsic.rs` through provider planning, native
  realization, target-operation lowering, image emission and artifact
  hashing), whose rows are single syscall stubs, while the handoff is a
  bounded two-call retry loop with a map buffer and a pre-final-attempt stack
  switch that needs the system-table pointer only the physical shell owns.
  That shell does not exist (`optimized_semantic_wrapper_object` has no
  callers; **UEFI-PHYSICAL-SEMANTIC-ENTRY**), and a `ProgramEntry`-bound entry
  has no checked transitive Unit plan, so this edge cannot advance past
  checked until that item lands an emitted shell; it is an implementation
  dependency, not a language decision.

- **AP-BRINGUP.** Complete one secondary-processor entry through the executable
  installation and external-root owners. Acceptance covers low-memory and
  alignment constraints, CPU-regime transitions, placed-byte visibility,
  installed AP entry, and separately accounted per-CPU stack/state. An emitted
  trampoline alone does not satisfy the entry and custody contract.

  Resume evidence: `7ca30f8411` landed the secondary-processor startup ledger
  in `omega-rust/omega/backend/runtime/external-roots/src/secondary_processor.rs`
  over the new `InstalledCode::placement_constraints` and
  `binds_placement_geometry` projections. `bind_secondary_processor_trampoline`
  replays retained installation evidence — admitted startup entry,
  regime/architecture consistency, exact realized extent, startup-vector
  granularity, and the low-memory bound on both extent and declared range —
  before deriving the vector, and the borrow keeps the installed code
  unretirable. `admit_secondary_processor` mints the arrival-to-installed
  regime transition only when the boundary begins in the installed regime on
  the account's own dedicated stack class; shared stack classes and
  overlapping state backing reject. The issued carrier binds installed-code
  identity, context, and artifact; completion accepts only the receipt naming
  that exact carrier, refusals return pending custody for retry, and a started
  processor's account stays held for a later quiescence edge. Witnessed on
  macOS arm64 by `mbx nextest run -p external-roots --lib` (152/152, including
  22 secondary-processor tests over real installed-code custody). The
  quiescence edge landed as
  `SecondaryProcessorStartupLedger::retire_secondary_processor` in
  `omega-rust/omega/backend/runtime/external-roots/src/platform_bringup/secondary_processor.rs`
  (the module moved under `platform_bringup/`): it consumes the
  `SecondaryProcessorStarted` evidence — which now binds installed-code
  identity, context, and artifact — with a provider
  `SecondaryProcessorQuiescenceReceipt` naming it exactly, and returns the
  complete account (boundary, stack class, WCSU, state extent, transition)
  as `SecondaryProcessorRetirement` only when the receipt attests
  quiescence. Foreign, drifted, stale (an earlier startup of a re-admitted
  processor), replayed, unadmitted, pending, and invoked inputs all reject
  transactionally with both inputs returned; a non-quiescent receipt keeps
  the account held and hands the started evidence back. The ledger's
  trampoline borrow is untouched by retirement. Witnessed on macOS arm64 by
  `cargo nextest run -p external-roots --no-fail-fast` (224/224, including
  27 secondary-processor tests:
  `retirement_returns_the_exact_started_account_and_frees_its_resources`,
  `quiescence_refusal_keeps_the_started_account_held_for_retry`,
  `retirement_rejects_never_started_accounts`,
  `retirement_rejects_stale_foreign_or_replayed_started_evidence`, and
  `retirement_rejects_receipts_off_the_exact_started_evidence`). Remaining: a
  provider edge issuing the vector to the target boot protocol and an authored
  Omega surface invoking the entry. The provider edge is design-blocked on
  [owner question 7](OWNER_QUESTIONS.md) (`ap-startup-protocol-ownership`):
  the spec names no boot protocol for x86-64 and the APIC facts are Cathedral-
  owned, so whether the compiler issues INIT/SIPI, calls firmware MP services,
  or only seals a Cathedral-minted startup receipt is an owner decision;
  surveyed at a6cdb2fbd9.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Carry one real
  content-bearing program through checked source, Terminal Psi, provider
  selection, and native realization. Introductions and exits must bind exact
  subject, geometry, lineage, route, and installed occurrence; reshuffles may
  preserve identity, while partitions require authored proof. Acceptance:
  every surviving content claim traces to a reconstructed introduction or
  admitted provider issuance and every residual is accounted for.

- **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION.** Derive enumerable program-local
  content roots from exact installed parameter positions, capacity, and epoch.
  Ordinary results with no parent lineage cannot mint roots. Acceptance:
  aggregate capacity is reconstructed for one artifact instance and lifecycle
  epoch, with no ambient provision or row-equality authority.

  Resume evidence: `external-roots`
  `ProgramLocalRootInstallationLedger::derive_eligible_prebindings` now
  enumerates the complete eligible set from the sealed required-slot closure
  instead of a caller roster, so a cohort cannot seal over an understated
  aggregate. Omitted, extra, duplicate, substituted, and replayed members and
  sealing before derivation all reject transactionally. Witnessed on macOS
  arm64 by `mbx nextest run -p external-roots --lib` (130/130), including
  `epoch_cohort_cannot_seal_before_the_eligible_set_is_derived` and
  `program_local_root_schemas_derive_exact_installed_slots_without_minting`.
  `eda0f330c2` adds
  `ProgramLocalRootInstallationLedger::reconstruct_aggregate_capacity`,
  which replays the complete live established membership of one aggregate
  schema group in one sealed epoch cohort of one installed artifact instance
  and composes the members' evaluated per-occurrence capacities into the
  exact counted sum or separated interval set — accounting evidence only,
  no minting or row-equality authority. Empty, repeated, cross-schema,
  cross-cohort, stale-epoch, foreign-lifecycle, and foreign-installation
  rosters all reject, as does an interval member set whose ranges overlap.
  Witnessed on Linux x86-64 by `cargo nextest run -p external-roots --lib`
  (210/210), including
  `aggregate_capacity_reconstruction_sums_the_live_group_for_one_epoch`,
  `aggregate_capacity_reconstruction_composes_the_interval_member_set`,
  and
  `aggregate_capacity_reconstruction_rejects_mixed_schemas_cohorts_and_installations`.
  Remaining: connect that ledger to actual installed backing, receiver
  partitions, activation loans and completion for the same occurrence/epoch.
  The ordinary macOS `cli_mvp` receiver bridge already executes under its
  explicit loader premises; do not recreate it or treat its conditional image
  correspondence as an installation-ledger receipt. **ENTRY-CONTENT-ROOTS**
  owns the remaining target and lifecycle realization.

- **BOUNDARY-ISSUANCE.** After conservation closes, derive provider issuance
  geometry from exact invocation parameters, entry places, and results. Keep
  ownership, aliasing, issuance, custody, and partition succession distinct;
  providers may attest custody but not computable interval arithmetic.

## P2 - Materialization and placed access

- **PLAN-LAID-VIEWS.** Finish checked and native placement for plan-laid views
  without turning a physical address into semantic ownership. Layout identity,
  backing, range, access, and lifetime must rejoin at every use. Acceptance:
  valid views survive codec/native replay and stale plan, range, access, or
  backing substitutions reject.

  Continue from `artifact_admission.rs` in
  `omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/src/`.
  `ArtifactSections` carries optional exact-ledger replay; ordinary, optimizer,
  and native admission all retain the placed-view roster. Checked extraction
  for consumers without custody support rejects nonempty rosters. Extend the
  existing `direct_placed_view_input_survives_codec_and_native_replay` control
  in `compiler/tests/access_plans.rs` through native-realization optimization
  and end-to-end native placement. The retained roster is semantic custody,
  not backing, range, access, or lifetime authority. At 042f6f27bd that
  control publishes the placed-entry fragment for linux_x64, linux_arm64
  and macos_arm64, ties the derived placement to the target's first
  pointer-argument register, and on a matching host executes it with a
  C-lent referent of the plan's exact geometry (backing, range, access and
  lifetime stay with the driver); stale plan/range/access/backing and
  stale roster/artifact substitutions reject before execution (macOS
  ARM64 ran the leg; Linux hosts not run). Executable-image realization
  stays fail-closed at `try_into_native_input` for a nonempty roster
  because no provider establishment route supplies the view address to an
  image entry; that route (placed-access "Establishment and retirement")
  is the next dependency, and `tests/architecture/layering.rs` pins the
  `VerifiedNativeArtifactInput as NativeRealizationInput` alias any
  plumbing must preserve. Artifact interpretation joins the same custody
  gate: `TerminalExecution::start_verified_module` now rejects a nonempty
  roster with `PlacedViewInputsRequireCustody`, since no scalar,
  structural, or byte-sequence input channel can lend the referent a
  placed row declares, so the entry machine no longer executes with that
  declared input silently unbound.

- **SYMBOLIC-MATERIALIZATION.** Complete symbolic field/index materialization
  and its target-dependent realization. Preserve exact paths and bounds until
  assignment; physical lowering may choose locations but not change semantic
  access. Recursive build-time projection/replay carries record depth as data,
  with a bounded traversal; extend that owner rather than adding depth-specific
  implementations. Target-dependent placement remains fenced until its general
  rules land. Acceptance includes nested field/index canaries on both Linux
  ISAs.

  Resume evidence: `8ba0bd1fff` landed the index hop; `40347fde2c`,
  `78685f4bff`, and `955ce0427a` landed the `SymbolicFieldInnerLayout` carrier
  and bounded recursive path traversal
  (`derive_symbolic_materialization_with_inner_layouts`,
  `omega-rust/psi/foundation/layout-plans/src/symbolic_materialization.rs`); `c045400290` lowers the
  nested writers for `linux_x64` and `linux_arm64`. The compiler tests
  `*_symbolic_materialization_*` in
  `omega-rust/omega/compiler/compiler/tests/layout_plans.rs` now also execute
  the lowered host-ISA fragment natively (`lower_writer_on_both_linux_isas`
  links a guarded C driver via the shared `native_function` harness and
  compares the image with the Rust reference writer). Linux x86-64 host:
  `cargo nextest run -p compiler --test layout_plans symbolic_materialization`
  4 pass with the x86-64 native leg executed. The Linux aarch64 native leg is
  host-gated and has not yet run on a Linux aarch64 host; macOS/Windows/QEMU
  legs were unavailable here. Direct-sum coexistence then landed under the
  general recursive rule: `ConventionalRecordSumPathsLayoutReport` carries the
  record level's own `child_sum_layouts` beside `paths`
  (`omega-rust/psi/foundation/layout-plans/src/layout_reports/mod.rs`), the
  recursive projection emits both child kinds on one `Branch`
  (`omega-rust/omega/backend/layout/src/sum_materialization.rs`), the carrier
  fold joins them in one field-keyed namespace
  (`symbolic_values::SymbolicFieldInnerLayout::from_recursive_sum_paths`), and
  build-time evaluation retains, replays, and fingerprints the direct-sum
  custody under the same bounded traversal
  (`const_record_with_nested_sum_materializable`). Focused coverage:
  `recursive_direct_sums_coexist_with_deeper_paths_on_one_level` in
  `omega/backend/layout/src/sum_materialization/tests/recursive.rs`, the
  extended recursive fixtures in `layout-plans` and `build-time-evaluation`
  tests, and the coexisting `route` field in
  `recursive_sum_symbolic_materialization_realizes_on_both_linux_isas`.
  `eb82c48603` then lifted nested sum arrays under the same general recursive
  rule: every record level may co-locate direct sums, direct nonzero literal
  `[S; N]` sum arrays, and deeper record paths at once
  (`child_sum_array_layouts` on both `ConventionalRecordSumPathsLayoutReport`
  and the recursive `Leaf`; `project_record_level_children` in
  `omega/backend/layout/src/sum_materialization.rs` classifies every level;
  `ValidatedConstRecordLevelSumChildrenMaterialization` in
  `const_record_with_nested_sum_materializable/record_level.rs` retains the
  level's per-index array custody, replay, and fingerprints; the carrier fold
  joins `SumArray` carriers in the same field-keyed namespace). Coverage:
  `recursive_sum_arrays_compose_beside_direct_sums_and_deeper_paths` and the
  extended drift fixture in
  `omega/backend/layout/src/sum_materialization/tests/recursive.rs`,
  `symbolic_recursive_sum_array_materialization_composes_indexed_boundaries`
  in `layout-plans`, and
  `recursive_sum_array_symbolic_materialization_realizes_on_both_linux_isas`
  in `compiler/tests/layout_plans/writer_lowering.rs` (x86-64 native leg
  executed). `382fcc833b` then lifted direct record arrays under the same
  general recursive rule: every record level may also co-locate nonzero
  literal `[R; N]` record fields whose element record still reaches sums
  (`child_record_array_layouts` on `ConventionalRecordSumPathsLayoutReport`
  and the recursive `Leaf` in
  `omega-rust/psi/foundation/layout-plans/src/layout_reports/mod.rs`;
  `project_record_array_row` in
  `omega/backend/layout/src/sum_materialization.rs` retains the element
  record's shared recursive report once beside the literal count and stride;
  `SymbolicFieldInteriorLayout::RecordArray` carriers fold through
  `from_recursive_sum_paths`; build-time evaluation's
  `ValidatedConstRecordArrayFieldMaterialization` retains per-element
  recursive custody, replay, and fingerprints under the same bounded
  traversal). Coverage:
  `recursive_record_arrays_compose_beside_direct_sums_and_deeper_paths` in
  `omega/backend/layout/src/sum_materialization/tests/recursive.rs`,
  `symbolic_recursive_record_array_materialization_composes_element_boundaries`
  and `symbolic_recursive_record_array_paths_stay_symbolic_until_assignment`
  in `layout-plans`, and the record-array drift fixture in
  `build-time-evaluation`'s recursive tests. Next acceptance: run the
  `symbolic_materialization` filter on a Linux aarch64 host, then open
  target-dependent placement.

## P3 - Terminal Psi, PCC, and observation

- **PCC-PRODUCT-PUBLICATION.** Deliver native evidence for the
  [optional proof product contract](wiki/spec/proofs/publication.md). The Psi
  product is delivered and pinned in `compiler/tests/pcc_publication.rs`: both
  off-by-default requests in every combination, adjacent `.psi`/`.psi.proof`
  files with separate sizes and no embedded route, the macOS inner placement,
  exact omitted-dependency possession, the wrong-bytes, premise, policy,
  assumption and stale-sidecar rejections, exhaustion as `Incomplete`,
  receiver-owned policy package and configuration rather than producer hints,
  and explicit pair framing.

  Implement standalone native evidence and checking in `compilation-report` and
  the native semantic and certification owners: reconstruct executable behavior
  against the exact published bytes, covering instructions and entries,
  incoming edges, indirect targets, premise availability and lowering
  correspondence, or supply a direct native proof. Hashes of producer
  validation reports and an unrelated valid Psi artifact cannot establish that
  argument. The blockers are that one missing evidence:
  `compilation-report/src/pcc.rs`'s `verify_native_proof_sidecar` returns
  `Incomplete(UnsupportedEvidence { product: Native })` unconditionally after
  claim-field checking, and `compile_report.rs` refuses every `pcc.native`
  request before publication, so no native sidecar is ever written. Until that
  evidence exists, native-only and both-product publication keep reporting
  `Incomplete` rather than a custody-only success, and ordinary native and
  Psi-only publication keep working.

  Acceptance: a native `.proof` sidecar beside the flat executable and inside
  `Contents/MacOS/`, each with its own reported size; native-only standalone
  checking returning a real verdict from bytes, companion and pinned policy
  after the source and Psi artifacts are deleted; and arbitrary native bytes
  paired with valid Psi and recomputed producer hashes still refused. Claims
  beyond the bounded `omega.terminal-verified-module.v1` guarantee depend on
  **PROOF-KERNEL-CORE**, **PROOF-CERTIFICATION-BRIDGE** and completed profile
  rules, not a new policy DSL. Bundle execution acceptance belongs to the GUI
  cohort.

- **PSIIR.** Extend Terminal Psi only in complete vertical slices through
  canonical encoding, independent reconstruction, verification,
  interpretation, resource analysis, native lowering, artifact custody, and
  installation. [Terminal specification subjects](wiki/README.md#current-specification-subjects)
  own the vocabulary; complete operation/proof-node byte tables remain required
  by the [encoding contract](wiki/spec/terminal-psi/encoding.md).
  Acceptance: source and producer state can be discarded before an
  independent verifier reconstructs every obligation and executes or lowers
  the same artifact.

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

  Extend bounded safety/proof admission to qualified and partial owned custody,
  structural results, projected claims and effectful calls. Preserve dominance,
  exact successor transfers, ownership frontiers, current-iteration guards and
  test-fuel suspension/resumption. General cyclic invariants/ranking views need
  retained evidence; guarded-crash checking must not enumerate unbounded paths.
  Source production must compose projected helpers, Console structural operands,
  indexed/aggregate writes and computed results without state duplication.
  Finite fuel or a relaxed shape check is not a safety proof.

  Acceptance: the unchanged customer reaches native exit/output on the hosted
  matrix, with exact caller/callee resource composition and independently checked
  ranking where declared. Corrupt arrivals, ownership, guards, effects and proof
  groups reject. **SAMPLE-CORPUS** owns whole-customer execution and its current
  strategy pause; interpreted loops or isolated graph tests cannot close it.

  Linux x86-64 resume evidence: `print_squares`'s `Main::main` now declares
  `reaches Console` (the direct-boundary rule in
  [effects](wiki/spec/language/effects.md)); before that, every Console sample
  stopped at source checking with `publishes service reach <none>`. `58dd5c0482`
  retains composed cyclic Unit plans (`Main::main`,
  `ConsoleNativeProvider::read_line`, `console_write_bytes`): the closure
  keeps its checked transitive machine plan through nested/prefixed control
  assembly, cyclic write-frame inference, trivially discardable affine
  boundary results, and exact result custody/cleanup; shared inline byte
  fields present to boundary byte-view parameters end to end. Store fact
  invalidation is now scoped to the canonical written path, so disjoint
  sibling-field observations survive (`repro_cyclic` b0-b8). Scalar field
  stores now publish the exact `field == stored value` equation on the
  canonical write path (`terminal-verifier` `operation_facts` reconstruction;
  `structural_scalar_store` field-obligation tests), so obligations whose
  subject was stored on the dominating path — e.g. `x / self.place` after
  `self.place = 100` — prove through the existing kernel-checked equality
  transport; a covering write expires the equation, so stale bounds cannot
  leak.

  Storage-observation invariant scope is now admitted: scalar block
  invariant predicates may name `IntegerField`/`BooleanField` terms rooted
  at places alive for the whole invocation — machine structural parameters
  and the header's own block parameters — through the shared
  `scalar_block_invariant_scope` telescope used by both module validation
  and producer candidate checking. The producer's `field_bounds` pass
  transports an unprovable cyclic field-read bound obligation through the
  read's exact `value == field` equation into a candidate invariant at
  every externally entered component header; each candidate is still a
  proposal, proved independently at every actual arrival before retention.
  `cyclic_field_divisor_retains_storage_observation_invariant` witnesses
  the slice end to end: `x / self.place` with `place` initialized before
  the loop lowers, reload-verifies, and interprets to its `done` trace.

  The remaining probe stop is the guarded-exit lockstep form: `digit_div`'s
  `self.sq / self.place` reads `place` written by a previous iteration.
  Neither plain `place >= 1` nor `p < 3 -> IntegerField(place) >= 1` alone
  is inductive: the latter admits `p = 0, place = 1`, whose update reaches
  `p = 1, place = 0`. The producer needs a stronger counter/divisor
  relationship, independently checked at updates and every arrival.
  `cyclic_field_divisor_awaits_storage_observation_invariants` retains the
  `OperationProofUnavailable` control; next acceptance is its unchanged
  source publishing, reload-verifying and interpreting successfully.
  Existing order transitivity, exact equality bridges and closed literal
  predicate denotation already discharge incompatible integer guards;
  no new contradiction rule is needed. Equality-cited bounds now join that
  same bounded two-leg search: an exact `counter == 9` store fact weakens
  through checked order legs and contradicts the guarded premise
  `counter < 3`, while closed arithmetic endpoints evaluate through the
  checked closed-relation primitive before denotation.
  `integer_guarded_exit_retains_equality_bound_invariant` witnesses it end
  to end: the retire edge pins `self.counter = 9`, the lockstep invariant
  retains every arrival, the artifact publishes, reload-verifies and
  interprets `step,step,step,done`; a stored bound inside the guard and a
  mutated stored constant both reject. Saved scalar correlations also survive
  writes and mutating calls through `terminal-verifier/src/verification/field_snapshots.rs`:
  live exact field-to-SSA equalities capture affected facts before invalidation,
  without retaining stale field observations or multiplying fact variants.
  Resume at stronger field-bound candidates and checked wrapping-update proofs,
  not snapshot reconstruction or implication plumbing. `field_bounds` now
  carries scoped field comparisons as candidate premises; `integer_selection/implications`
  makes proved consequences available to ordinary arithmetic through existing
  implication introduction/elimination, without a new trusted rule. At
  `00ed8d4fc9`, macOS arm64, run
  `RUST_MIN_STACK=67108864 cargo nextest run -p checked-trees-to-lowered-psi --lib --no-fail-fast -E 'test(nonzero_divisor_certificate) | test(cyclic_byte_literal_calls) | test(scalar_block_invariant) | test(structural_scalar_store)'`.
  `guarded_field_divisor_remains_valid_until_loop_exit` publishes, reload-verifies
  and interprets three iterations plus exit despite clearing the divisor on the
  exit backedge; leaving its guard live rejects both fresh production and old
  proof replay. This is dependency progress, not decimal/native acceptance.
  A local SSA feasibility probe established the three candidate clauses
  `(p < 3 -> place >= 1)`, `(p < 2 -> place >= 10)`,
  `(p < 1 -> place >= 100)` initially and derived division safety, but could
  not prove any of their three preservation obligations from actual wrapping
  add/divide equations. Coordinate the missing checked arithmetic bridge with
  **PROOF-KERNEL-CORE** before synthesizing that strengthening; do not substitute
  exact arithmetic or enumerate loop states. The checked wrapping bridge now
  exists: the affine witness replays unsigned `WrappingIntegerAdd` steps
  backward to an operand and forward to the sum, and `WrappingIntegerDivide`
  steps toward the quotient; `map_integer_affine_bound` maps a strict or
  non-strict root only alongside an independently proved no-wrap conjunct
  (`operand <= maximum - addend`), so bounds transport through wrapping
  updates without substituting exact arithmetic or enumerating loop states.
  `integer_selection/wrapping` discovers those words and assembles the
  root-bound conjunction from cited facts under its own memoization. The
  strengthened counter/divisor clauses still need their preservation
  obligations proved through this bridge; the unchanged decimal loop remains
  the next source acceptance, followed by the native customer.
  Indexed byte-field writes still need composed cyclic-Unit/customer closure
  coverage. Reuse the ordinary native bounded-field store and exact live-length
  replay, not a new byte-view adapter. Direct/nested source writes have a focused
  macOS ARM64 runtime and four-target publication probe:
  `cargo nextest run -p compiler --test byte_field_replacement indexed --no-fail-fast`
  (`RUST_MIN_STACK=67108864`). This does not close the unchanged decimal or Console
  customer; their missing invariant and transitive-call evidence remains above.

- **CRASH-CONTRACT.** Carry invocation-specific crash obligations through
  operators, nested structural paths, calls, cycles, execution and package review.
  Owners include `facts/operator_crashes.rs`, `CrashPlan::checked_operators`,
  captured operands in `flow/expression.rs`, and Terminal/native evidence
  consumers. Source checking alone is not portable proof. Retain exact selected
  requirement, saved actuals, Match arm and surviving route; do not invent calls,
  infer semantic crashes from emitted traps, or narrow opaque contracts by
  inspecting providers.

  Boundary declarations now retain scalar guarded ceilings through canonical
  encoding and independent actual-argument substitution; run
  `cargo nextest run -p checked-trees-to-lowered-psi --test scalar_boundary_arguments --no-fail-fast`.
  Reuse that requirement contract for selected operators, retaining exact occurrence
  evidence. Source crash predicates currently lower through fixed-width
  `ScalarTerm`; proof-only mathematical terms do not imply an authored
  mathematical guard route.
  Boundary crash observation profiles and Omega projection remain open. The
  edge-only `TerminalTraceV1` profile and Omega projection reject nonempty
  boundary routes; crashing providers now refine a ceiling that covers their
  positionally substituted routes (terminal-verifier `calls` provider
  conformance). Extend the source-to-execution controls above
  while preserving exact call sites, guard actuals, abandoned claims, staged
  writeback and no-result/no-cleanup behavior.
  Carry qualified scalar results and the remaining normal-contract vocabulary
  through ordered boundary completion. Reuse the machine-entry and scalar
  normal-guarantee path (`unit_scalar_result_source`'s `ordered_boundary`,
  `ordered_scalar`, `ordered_boolean`, `ordered_computed_boolean`,
  `ordered_nested_boolean`, `ordered_saved_boolean`,
  `ordered_call_produced_boolean`, `ordered_boolean_call_computations`,
  `ordered_boolean_guarantees_compose_through_dependent_joins`, and
  `ordered_boolean_completion_preserves_folded_source_meaning`, and
  `ordered_mutable_boolean_snapshot` tests).
  State/control contracts, mutable snapshots crossing state joins, and
  field/arithmetic predicates still need their evidence joins. Preserve
  authored callee contracts regardless of whether a helper is a direct closure
  root or a transitive dependency; a checked call identity is not contract proof.
  Mutable scalar inputs still need the shared signature/storage path beyond the
  invocation-entry read checker. Do not infer normal guarantees from crash ceilings
  or use current storage as an entry snapshot.

  Package contract review still needs exact carrier/value custody for declaration
  and result projections through indexes, case payloads and generic field
  substitution beyond ordinary declaration-owned field paths.
  Reuse Psi's exact carrier/case relation; do not manufacture a machine owner
  from the classifier. The retired `proposition` declaration surface belongs to
  **PROOF-CONTRACT-MIGRATION**, not an independent membership-extension task.
  Saved or call-produced result tags without a live predicate need ordinary
  value/effect custody; do not replay initializers or callee bodies to recover
  a tag after its evaluation point.
  `cargo nextest run -p package-evidence --test callable_policy case_membership --no-fail-fast`
  is the existing source-to-recovery control. Selected operator crash
  invocations now project through both package crash projections: review emits
  per-site rows (exact selected operator, caller state/statement, published and
  surviving buckets; an empty surviving set is a proved discharge) via
  `capture/behavior/crash/operator_projection.rs`, and callable policy consumes
  the same retained rows at cause level. Regression:
  `cargo nextest run -p package-evidence --test callable_policy --no-fail-fast`.

  Extend `facts/crash_entry_values.rs` beyond immutable stable-content roots to
  state arrivals, rebinding, mutable field versions and receiver/case projections,
  including the separate named-operator use path. Unknown
  provenance must remain conservative; current spelling/live storage is not a
  saved actual. This owns **MATCH-SELECTIVE-LOWERING**'s crash-qualified equality
  dependency and shares entry snapshots with **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: source `operators/crash_routes` and crash-qualified float controls
  retain exact surviving-route evidence through independent Terminal replay and
  execution; package projections already carry the site rows. Safe uses
  discharge each route; changed guards, captures, substitutions, sites and stale
  writes reject. Preserve examined/discharged routes and caller coverage, not
  only the final cause set. The replay gap is concrete: the Terminal verifier
  reconstructs call crash continuations only from an ordinary callee's
  `contract.crash_routes`, operator surviving routes are invocation-specific
  and may carry no portable `scalar_expression` after conservative `Truth`
  widening, and boundary operator declarations do not yet carry a replayable
  Terminal crash contract; direct lowering rejects selected operator crash uses
  at `checked-trees-to-lowered-psi/src/machine_lowering.rs`. Copying checked rows onto
  `MachineContract` alone would not establish replay meaning. Checked scalar
  computations now admit a selected integer boundary comparison such as
  `Comparison::equal(i32, i32)` as
  `CheckedScalarComputationKind::SelectedComparison`:
  `checked_trees/operators/comparisons.rs`'s `selected_integer_comparison`
  joins `selected_float_comparison`, and
  `values/scalar/computations/dispatch.rs` admits either classifier while
  retaining the exact operator use, authored operand order, and joined
  published/surviving site rows. Terminal consumers still resolve selected
  comparisons only through `selected_float_comparison`, so an integer
  `SelectedComparison` rejects at lowering rather than replaying; the lowering
  fence for crash-qualified uses is unchanged. Terminal now carries the
  operation-level contract: `TerminalModule::operation_crash_contracts`
  (`terminal-psi/src/terminal_module/proof/operation_crash_contracts.rs`) rows
  name `(machine, operation)`, the operator's published routes in a
  declaration-local formal namespace (scalar operand ordinal plus one, typed by
  that operand, as boundary crash routes do) and the surviving continuations
  in the machine's value namespace; the codec encodes the roster after scalar
  block invariants under format marker 99, and
  `terminal-verifier/src/validation/crash/operation_contracts.rs` rejects an
  unknown or call/operand-free operation, a noncanonical or non-scalar
  published roster, continuations that differ from the exact operand
  substitution (empty, widened, swapped, recaused), and then applies
  `validate_call_crash_coverage`. Regressions: `cargo nextest run -p
  terminal-verifier --test suite operation_crash_contracts` (7) and `-p
  terminal-codec --test suite operation_crash_contracts` (3, including the
  format-98 layout rejection); macOS ARM64. The producer now writes the row:
  `checked-trees-to-lowered-psi/src/retention/operation_crash_contracts.rs`
  lowers each `CheckedCrashOperatorSite` in the lowered closure at the exact
  operation emission joined to its `operator_use` (today only the selected
  IEEE comparison join), through the same formal-telescope lowering boundary
  declarations use (`proofs/crash_routes.rs::lower_formal_crash_routes`) and
  the verifier's own `substitute_crash_routes` for the continuations; the
  whole-program fence at `machine_lowering.rs` is lifted and replaced by
  fail-closed rejections for a crash-qualified use without a site or without
  an emitted join, a named-use site (no operation join exists), a non-scalar
  or miscounted operand roster, and a call operation. Regression:
  `cargo nextest run -p checked-trees-to-lowered-psi --lib
  operation_crash_contracts` (6). Where the corpus stops next
  (`omega inspect-terminal` on `operators/crash_routes`; its canary rows
  are check-only and never lower): `safe`/`may_crash` reject at
  `comparison has no exact selected IEEE meaning` (no executable Terminal
  meaning for an integer `SelectedComparison`), `wrapper` rejects at
  `direct scalar call crash continuation lacks a checked scalar term`, and a
  guarded float operator route rejects at `guarded crash route is outside
  structured scalar predicate lowering`. Operator published rows now carry
  the guard's structured scalar form over the operator's own formals
  (`facts/crash_plan_facts.rs::derive_authored_operator_crash_buckets`
  through `CrashContractOwner::Operator` and
  `values/scalar/contract_entry/crash_entry.rs::lower_operator_crash_contract_expression`,
  the same reader bodyless signatures use; dense scalar position `k` is the
  Terminal formal `k + 1`), for both `boundary operator` and
  `boundary machine ==` declarations and for spelled and named uses; a
  generic operator or a guard through a structural formal keeps identity
  only. Neither form owns a `contract_plans` machine plan, so the site
  roster is the producer's carrier. Regressions: `cargo nextest run -p typed-trees-to-checked-trees
  --lib facts::operator_crashes::tests` and `-p checked-trees-to-lowered-psi
  --lib operation_crash_contracts` (the guarded integer route lowers to
  `!(0 <= formal 2)` and the verifier accepts its recomputed continuation at
  an `IntegerEqual` operation). The selected integer comparison now has
  the emitted-operation join: `lowered_psi::LoweredSelectedIntegerComparisonOccurrence`
  (`LoweredPsi::selected_integer_comparison_occurrences`, keyed by the same
  checked `operator_use` as the IEEE roster) is recorded by
  `emission/operation_emission.rs` beside the exact `IntegerEqual` /
  `IntegerLessThan` / `IntegerLessOrEqual` it emits for a `SelectedComparison`
  whose `selected_integer_comparison` meaning has an admitted emission. All
  six authored spellings join: `==`, `<` and `<=` emit their own operation
  over the authored order, `>` and `>=` emit the reversed
  `IntegerLessThan`/`IntegerLessOrEqual`, and `!=` emits `IntegerEqual`
  plus one `BooleanNot` over its result, mirroring the checked stage's own
  comparison normalization. The row records that exact mapping
  (`operand_order`, `negated`), and `retention/operation_crash_contracts.rs`
  reindexes the declaration's authored formal telescope into the emitted
  operation's positional one through it, so the verifier's positional
  substitution is unchanged and a mapping that cannot address the emitted
  operand roster exactly fails closed. It joins a site through both
  rosters. Control-flow cleanup keeps the joined operation as a sidecar,
  Terminal production carries the roster on every checked
  product (`selected_integer_comparison_occurrences()`), and Omega's
  `checked-compilation-to-terminal-artifact` refuses a nonempty roster
  (`native realization does not yet consume retained selected integer
  comparison occurrence custody`) rather than realizing a selected
  comparison as the builtin one. Regressions: `cargo nextest run -p
  checked-trees-to-lowered-psi --lib operation_crash_contracts` (11; the
  guarded integer `boundary operator` route lowers through `lower_machine`
  end to end to one row at the emitted `IntegerEqual` whose continuation
  the verifier accepts, the `>` route to a row whose published guard names
  the reversed operation's formal 1, and the `!=` route to a row on the
  equality the `BooleanNot` consumes; a reordered row left in the authored
  telescope is a verifier rejection) and `--lib comparisons::tests` (7; the
  six admitted emissions and the operand-mapping arity refusals). Where
  `omega inspect-terminal` on `operators/crash_routes` stops now:
  `safe`/`may_crash` reject at
  `selected comparison has no complete provider plan evidence` (the
  fixture's `boundary machine == Comparison::equal` has no selected
  ProviderPlan, so the checked use carries an empty
  `provider_plan_commitment` and lowering refuses to emit an unprovided
  selected comparison); `wrapper` still rejects at `direct scalar call
  crash continuation lacks a checked scalar term`. The terminal stage now
  replays the integer roster into `checked_boundary_operator_scope`
  (`lowered-psi-to-terminal-psi/src/boundary_operator_custody/integer_comparisons.rs`,
  keyed by the same checked `operator_use` as the float replay: exact
  application site, requirement operator, non-empty provider commitment,
  comparison, operand mapping, negation and operand type, one exact
  Terminal operation over operands of that type, one `BooleanNot` over its
  result for a negated row, one exact checked application; a stale,
  duplicated or foreign row rejects, including one whose recorded
  `operand_order` or `negated` is not the authored spelling's admitted
  emission, and because builtin integer comparisons emit the same three
  operation kinds the artifact cannot count which were selected, so a
  crash-qualified use without a row stays the producer's fail-closed
  rejection). Regression: `cargo nextest run -p
  checked-trees-to-lowered-psi --lib integer_comparison_replay` (4). The
  next slice supplies the provider evidence for the integer boundary
  comparison on the Omega side (a selected ProviderPlan or an explicit
  builtin-realization commitment for `Comparison::equal(i32, i32)`, then an
  Omega consumer that rejoins `selected_integer_comparison_occurrences` the
  way `float_comparisons::associate` does and lifts
  `checked-compilation-to-terminal-artifact`'s nonempty-roster refusal) so
  `may_crash` reaches the producer-written row; the float operator route still has no structured
  form (`CheckedBooleanExpression` has no IEEE ordering over scalar float
  formals), and `wrapper`'s direct-call continuation is the separate
  checked-scalar-term gap.

- **PROOF-KERNEL-CORE.** Build the common mathematical term/declaration model
  and independent checker in Psi, under the
  [selected foundation](wiki/spec/proofs/foundation.md). Customer: library
  theorems about arbitrary types/predicates and dependent witnesses, not another
  extension to the bounded `Proposition` enum. Represent the reference core's
  universes, dependent terms and strict/relevant distinction once; source
  elaboration and certificate consumers must use it rather than invent parallel
  truths. Keep search outside the checker and preserve useful arithmetic rules
  as certificate producers or explicitly justified checked rules.

  Landed: the Π fragment in `proof-admission/src/mathematical_core.rs`:
  de Bruijn terms, `Type`/`Strict` sorts with closed levels, Π/λ/application
  typing without cumulativity, β weak-head normalization under a step ceiling,
  and typed conversion with strict collapse decided by the shared type's sort.
  Tests witness formation, non-sort/non-function/unbound rejection,
  capture-avoiding substitution, strict-versus-relevant separation, step-ceiling
  refusal, and an exact 51-slot arena receipt for checking the polymorphic
  identity.

  Landed: dependent pairs — Σ formation at the maximum component level
  (`Strict` only when both components are `Strict`), componentwise pair
  checking with the second component checked at the codomain instantiated by
  the first, `fst`/`snd` typing with dependent `snd` result, definitional
  projection reduction, and pair eta in both directions during typed
  conversion. Pair arguments in Π applications are routed through
  componentwise checking so dependent codomains do not need non-dependent
  inference. Tests witness dependent-codomain substitution, pairs flowing
  through call arguments, projection reduction through function redexes,
  relevant-versus-strict Σ formation, neutral pair eta, and non-pair
  projection rejection.

  Landed: typed function eta in conversion. At a `Pi` shared type a lambda
  and a non-lambda convert exactly when the non-lambda applied to the
  fresh de Bruijn variable converts to the lambda's body at the exact
  codomain; a strict `Pi` still collapses by irrelevance before the rule
  is reached. Tests witness both eta directions, strict-domain and
  dependent-codomain Π combinations, eta through dependent type
  arguments, the no-pointwise-collapse control, and refusal of the
  wrapper rule at a non-function shared type.

  Landed: the `Two` primitive — `Two : Type 0` with `zero`/`one`,
  dependent `caseTwo(C, d0, d1, t)` checking `C` as a `Π(_ : Two).
  Type w` family with `w` read from the checked codomain, and
  definitional computation to `d0`/`d1` on each constructor under the
  step ceiling. Strict-codomain and non-universe motives reject (boxing
  owns strict targets); there is no `Two` eta. Certificates carry the
  eliminator through the canonical wire (term tags 10-13) and re-verify
  after decode. Tests witness formation and introduction, branches
  checked at definitionally different constructor landings (large
  elimination), constructor computation and its step-ceiling refusal,
  stuck eliminations on neutral scrutinees, componentwise conversion of
  stuck eliminations, strict/non-universe/wrong-domain motive and
  non-`Two` scrutinee rejection, and that pointwise agreement on both
  constructors grants no function equality.

  Landed: relevant identity — `Id A x y : Type u` formation at the
  carrier's relevant `Type` level (a strict carrier rejects:
  proof-relevant distinction does not exist over a proposition),
  `refl A x : Id A x x` with a checked `ty` annotation so a
  dependent-pair endpoint still checks componentwise, and dependent
  elimination `J(C, d, y, p) : C y p` for `C : Π(y : A). Π(_ : Id A x
  y). Type w` and `d : C x (refl A x)`, where `p`'s inferred identity
  supplies the fixed carrier and left endpoint and the supplied `y`
  must convert to `p`'s recorded endpoint. `J` computes to `d` on
  `refl` as a budgeted step; neutral proofs stay stuck and compare
  componentwise at the left elimination's inferred types. There is no
  identity eta, K or UIP: `refl` never converts to a neutral proof and
  distinct proofs of the same identity stay distinct. Certificates
  carry `Id`/`refl`/`J` through the canonical wire (term tags 14-16)
  and re-verify after decode. Tests witness formation at the carrier
  level (including `Id` over `Type 0` landing at `Type 1`),
  strict-carrier and non-type-carrier rejection, wrong-endpoint
  rejection, a `Σ`-carrier `refl` over a dependent pair, elimination
  proving symmetry (`Id A x y` giving `Id A y x`), reflexive
  computation and its step-ceiling refusal, componentwise conversion
  of stuck eliminations, every malformed-motive and relocated-endpoint
  rejection, the no-UIP controls, and a transport certificate
  re-deciding `J` after wire decode.

  Landed: the W-type `W A B` of well-founded trees — formation,
  `sup` introduction, and dependent `indW` induction computing on
  each constructor under the step ceiling — then level parameters
  (`Δ; Γ ⊢ t : T` scope-checks every `Sort` level against the
  judgment's level arity, no unification) and universe-polymorphic
  declarations whose `Constant` references carry exact level
  instantiation through the canonical wire.

  Landed: derived indexed families in
  `proof-admission/src/mathematical_core/indexed.rs` — the profile's
  encoding `IndexedAt(i, sup a k) ≡ Id I (out a) i × Π(b : B a).
  IndexedAt (next a b) (k b)` and `IW i ≡ Σ (t : W A B). IndexedAt i
  t` as five ordinary universe-polymorphic declarations
  (`IndexedAt`, `IW`, `iwPack`, `isup`, `iindW`) checked
  parametrically by `check_signature` and applied through `Constant`
  spines, not a second primitive inductive checker. `isup`/`iindW`
  are defined terms whose computation is definitional: `iindW Q s i
  (isup a g)` converts to `s a g (b ↦ iindW Q s (next a b) (g b))`
  with the child function a neutral variable, closing through pair
  eta and typed function eta — the dependency the profile names.
  Tests witness the scheme re-checking as a parametric signature,
  the indexing equation unfolding on neutral `sup` nodes,
  computation with an arbitrary neutral child function,
  wrong-index/wrong-description/strict-universe/arity rejections,
  and a theorem certificate re-deciding the eliminator's judgment
  with exact assumption closure. Next: demonstrate Vector length
  indices, derivation indices, and mutual/nested families per the
  acceptance below, then connect the model to source elaboration.

  Landed: the length-indexed vector, mutual and nested families —
  `indexed_vector`, `indexed_mutual` and `indexed_nested` each check
  `isup` construction and `iindW` computation with a visible
  constructor and an arbitrary neutral child function, the nested
  case a level-indexed rose tree whose payload is `Σ(k : Nat). Vec k`
  — plus the conversion reflexivity fast path that keeps shared
  `IndexedAt`/`IW` spines shared. Exact assumption closures and
  budgeted step/arena receipts are pinned per family.

  Landed: level instantiation on the derived scheme in
  `indexed_levels` — a producer declares `vecOf : Π(E : Type u).
  Π(n : Nat). Type u` and `consVec` once at level arity 1, with
  `Parameter(0)` inside the scheme constants' level arguments, and
  one checked signature runs the family at `[0]` (vectors of
  booleans) and `[1]` (vectors of types). `iindW[0,1,0,1]` computes
  on a cons node with the induction hypothesis landing at the
  predecessor index one universe up; wrong element universes,
  wrong level-argument counts, out-of-scope parameters and
  descriptions claimed at mismatched sorts each reject with their
  exact error. The level-polymorphic `consVec` judgment travels as
  a `level_arity`-1 certificate through the canonical wire —
  byte-identical re-encode, independent re-verification, closure
  exactly the three `Nat` axioms — and a forged closed arity
  rejects `UnboundLevelParameter`.

  Landed: the core is inside the checker. Certificate acceptance
  denotes each accepted certificate into the core as
  `Γ ⊢ t : ⟦goal⟧` and the kernel re-decides it, recording
  `MathematicalCoreDecision::Judged` with receipts measuring
  declarations, assumption closure, context depth and arena slots in
  use after checking, or `Refused` for a rule family the denotation
  does not cover, where the bounded rules stand alone. A covered
  certificate the kernel rejects is rejected: the rule labels never
  outvote the kernel. Authored theorem machines witness the route from
  source through the canonical wire to a kernel judgment
  (`pass/proofs/kernel_theorem_equality_certificates` and its false
  twin), checked-only because native target lowering refuses a
  bodyless theorem machine (`UnsupportedControlFlow`). Next, in order:
  widen the denotation so `Refused` stops deciding most certificates,
  each family arriving with its source customer; then an executed
  kernel canary once the producer lowers a theorem machine.

  Landed: the set-quotient scheme in
  `proof-admission/src/mathematical_core/quotient.rs` — the
  [quotient specification](wiki/spec/proofs/quotients.md)'s interface
  authored as thirteen ordinary declarations: `Q`, `project`,
  `setQ`, `sound`, `effective`, `elim` and the propositional
  `beta` law are named assumptions with exact statements, while
  `transport`, `idTrans`, `transportConst` and the ordinary `lift`
  are checked definitions derived from them, and `liftPre` is
  admitted as one more exactly-stated assumption because a
  function-valued motive would need the function extensionality
  the calculus lacks. No `Term` variant, typing rule or conversion
  rule is added; every assumption lands in `assumption_closure`,
  and `beta` being a relevant `Id` keeps a quotient's
  representative unextractable. Tests witness the
  admitted-versus-derived boundary, a malformed congruence
  rejection, and the exact closure over the admitted interface.

  Landed: the real theorem certificate the milestone above named.
  `theorems.rs`'s `identity_substitution` (`subst` built from `J`)
  and `indexed.rs`'s `indexed_correctness` (index soundness proved
  by `iindW` itself) are ordinary checked definitions a certificate
  cites through `Term::Constant`; `terminal-codec` re-encodes the
  certificate byte-identically and `verify_mathematical_certificate`
  re-decides the judgment after decode with the exact assumption
  closure. Tests witness a theorem certificate verifying after wire
  decode, a polymorphic theorem reference round-tripping at its own
  arity, an axiom-dependent theorem keeping its assumption through
  the wire, a theorem-dependent obligation transporting over the
  wire, and a missing dependency rejecting the signature.

  Landed: the derivation context/conclusion-indexed family
  (`indexed_derivation`) — a natural-deduction `Deriv` family whose
  index is a judgment `Σ(Γ : Ctx). Frm`, so `impI` moves the
  context index to the extended context and `impE` selects each
  child's required conclusion by position. `iindW` computes on
  modus ponens with a neutral child function and proves the
  conclusion identity under a dependent motive; wrong judgments
  and malformed descriptions reject. The certificate verifies with
  exact closure over the four grammar axioms and measured cost:
  25_100 budgeted steps checking the `next` family, 3_689 steps
  and 282_055 retained arena slots for the certificate.

  Landed: the bounded certificate route in
  `mathematical_core/bounded_denotation.rs` — a shipped
  `ProofNode` certificate is denoted proposition-by-proposition and
  rule-by-rule into the common core (atoms as `Type 0` assumptions,
  scalar `Equal` as `Id` over a carrier assumption, connectives as
  `Σ`/tagged sums/`Π`, decided primitives as named decision
  assumptions) and re-decided by `verify_mathematical_certificate`;
  uncovered rule families refuse `Unsupported` rather than
  mis-deciding, separating source invalidity, unsupported encodings
  and producer defects. The bounded checker's own rule families
  each live beside their owner in `proof/`. Denoted discharge,
  implication and equality certificates cross the canonical wire
  and re-verify after decode.

  Landed: the combined rule/encoding metatheory and implementation
  evidence in
  [kernel_metatheory.md](wiki/spec/proofs/kernel_metatheory.md) —
  substitution, preservation, normalization and decidable
  conversion argued over the implemented judgments with the
  ceiling's bounded-incompleteness stated, the encoding
  correspondence discharged per obligation, and the trust boundary
  and non-claims named.

  The connect milestone is discharged: real theorem and bounded certificates
  travel the canonical wire and are independently re-decided with exact
  assumption closure. Next: the pinned reference core's strict layer landed at
  b367f54a77 (squash, boxing and strict empty/unit formers with their
  introduction and elimination rules in
  `proof-admission/src/mathematical_core/term.rs`, witnessed by
  `mathematical_core/tests/strict_layer.rs` and the certificate-wire tests,
  macOS ARM64); the source-elaboration seam is owned by
  PROOF-CONTRACT-MIGRATION. A verified-profile claim still requires the full
  pinned core plus the migration item's discriminating controls.

  Implement the pinned reference core and selected
  [W-based profile](wiki/spec/proofs/inductive_profile.md): relevant identity,
  two-element type, W-induction and checked derived indexed families. No second
  primitive indexed/strict-inductive checker. Prove the encoding scheme's
  formation, constructor, dependent-induction and computation correspondence,
  then independently check its declaration applications. Structural round trips
  do not establish meaning. General source punctuation is not a kernel blocker.
  Justify substitution, preservation, normalization and decidable conversion
  for the combined rules, including the landed
  [typed function eta](wiki/spec/proofs/inductive_profile.md#typed-function-eta);
  no untyped wrapper-deletion shortcut.
  Acceptance: independently checked universe-polymorphic dependent functions
  and pairs, strict same-statement conversion and relevant witness separation;
  malformed universes, capture-changing substitution and illegal elimination
  reject. Include exact assumption closure through declaration types/statements
  without relying on unfolding. Measure conversion/storage on these terms; do
  not claim feasibility from empty receipts or compiler-authored success flags.
  Do not expand the framework ahead of its consumers: a new rule family
  arrives with the denotation that lets acceptance re-decide it and a
  source-level customer, not on its own.

  Demonstrate Vector length indices, derivation context/conclusion indices,
  mutual and nested strictly-positive families with definitional constructor
  computation with a visible constructor and arbitrary neutral child function,
  not only concrete lambdas. Include dependent motives and capture/type-mismatch
  controls; pointwise equality alone grants no function equality. Reject malformed
  encodings, negative recursion, bad universes and illegal strict elimination;
  separate source invalidity, unsupported valid
  encoding and producer defects. Measure term size, retained storage and checking
  cost in the application checker, not an assumed nested bootstrap environment.
  Complete rule/encoding metatheory and implementation evidence before claiming
  a verified profile. Reopen W only on demonstrated requirements/cost/audit failure.

- **PROOF-CONTRACT-MIGRATION.** Migrate the proof surface to
  [ordinary machine contracts and trait bundles](wiki/spec/proofs/contracts.md#machines-and-bundles).
  Owners: Psi syntax/resolution/typing, contract proof semantics, Terminal
  evidence/codec/replay, and core mathematical traits. Use `PROOF-KERNEL-CORE`,
  not a second general logical representation. Implement the selected
  [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md): closed
  parameterized top-level `let`, dependent function types, curried prefix
  application, core-named universes/level binders and `boundary let` assumptions.
  Their general certificate integration follows the settled
  [PCC checking/publication contract](wiki/spec/proofs/publication.md).
  Preserve ordinary local bindings, complete machine calls and executable callback
  selection. Do not introduce quantifier keywords or substitute declaration
  enumeration or an optional-returning decider for general mathematical quantification.

  Replace the dedicated formula-declaration and hidden-witness call machinery
  with ordinary contracts and named witness/law bundles, preserving exact
  substitution, result/path availability, erasure, validity, and transitive
  assumptions across trait calls and artifacts. This incorporates the former
  selected-witness and trait-named-witness work; do not widen those old surfaces
  independently. Retain useful checking rules, not mandatory wrapper syntax.
  Include value/computation elaboration here: a total admitted invocation may
  denote mathematically; effectful calls expose outcome contracts, never effects
  executed by conversion. Mathematical result-bearing axioms are explicit in
  checked declarations, not missing-provider slots or a new domain qualifier.
  Enforce executable demand across data, calls and control flow, consistently
  at source use, evaluation and lowering; proof-only references retain trust
  without creating runtime demand.

  Acceptance: actual proof scripts and false twins pass through source,
  Terminal serialization, and independent replay for all five cases:

  1. Composition of two witness/law bundles preserves exact substitutions and
     distinct witnesses.
  2. A higher-order theorem quantifies over arbitrary mathematical predicates or
     functions, not an enumeration of executable declarations. Supply the
     machine-shaped logical hypothesis from derived Π-term evidence, with fixed
     subjects and discharged premises, not only a named proof declaration.
     Reject that supply for reset-through-borrow and executable callback contracts.
     Exercise `greater_than(limit)`, dependent result substitution, shadowing,
     independent universe levels and stable inferred public level telescopes.
     Partial mathematical applications are complete function terms; wrong expected
     types and missing machine arguments reject without implicit runtime closures.
  3. Nonconstructive existence uses an explicit axiom and cannot supply an
     executable witness without checked realization. Cover a chosen `u32`, a
     choice-dependent branch, and an erased theorem that mentions that value.
     Also prove squashed witness existence from another squashed witness and a
     logical hypothesis, with no choice assumption. Establish constrained records
     at construction, retain exact evidence dependencies, and keep relevant
     dependent witness bundles distinct from strict predicate gating.
  4. Accepting and denying policies distinguish the same theorem, with exact
     transitive assumptions surviving import, erasure, serialization, and replay.
  5. A Cauchy/quotient proof uses the selected
     [set-quotient interface](wiki/spec/proofs/quotients.md#set-quotient-foundation),
     with exact universes, set-valued dependent elimination, point computation
     as a law and explicitly admitted effectivity. Check derived lift, coverage,
     proposition induction and pointwise uniqueness; no hidden extensionality
     or kernel reduction. Squashed relations yield only squashed evidence.
     Preserve representative operations/congruence independently: a quotient-refusing
     policy accepts their quotient-free closure and rejects the assumption-bearing
     quotient proof. This control depends on transitive closure through helper
     statements/types; existing Rat comments do not establish that closure.

  Migrate core relations, quotients, samples, and tests, remove obsolete parser/carrier/codec
  routes, and reject retired spellings. Candidate naming syntax is not a
  prerequisite. Do not claim full mathematical coverage from these controls.

- **PROOF-CERTIFICATION-BRIDGE.** A well-founded denotation must be checked
  against the generated loop, not inherited from termination: a ranked
  `Natural` component whose accumulator update is wrong must fail its
  functional claim (post-loop `ensures` or scalar block invariant) while its
  unchanged control-cycle certificate still verifies, under the
  [publication contract](wiki/spec/proofs/publication.md). Today the
  source-level twins (`fail/proofs/inductive_climbing_sum_step_false_twin`,
  `inductive_gauss_sum_step_false_twin`) refute the wrong update in Psi
  validation only; both positive fixtures are checked-only, and
  `checked-trees-to-lowered-psi` does not lower a value-returning ranked
  machine ("has no admitted body (local construction stopped at call
  statement shape)"), so no generated loop carries a functional claim.
  Leg (a) landed on macOS ARM64: the terminal-verifier replay control in
  `tests/ranked_scc/scalar_block_invariants.rs`
  (`ranked_natural_certificate_replays_with_a_correct_accumulator_arrival`,
  `ranked_natural_certificate_replay_rejects_a_wrong_accumulator_arrival`)
  keeps the countdown's `Natural` certificate over a header accumulator
  claiming `rank <= previous` and rejects a wrong accumulator arrival with
  `RejectedEvidence { CertificateConclusionMismatch }` on the preservation
  obligation while the unchanged cycle certificate still answers the
  identical cycle question. The claim is an order the current kernel can
  derive; an arithmetic accumulation claim (`sum + rank = initial`) still
  needs ring evidence, which is the shared law-normalization gap below.
  Leg (b), Terminal half, landed on macOS ARM64: a value-returning ranked
  free machine (`descend(n, previous) terminates by n -> Nat::Descending`,
  returning the accumulator) already lowers through the scalar-graph cycle
  route with its `Natural` certificate; what was missing was the header
  invariant an `ensures` needs after the loop. The producer's
  `proofs/scalar_block_invariants/cyclic_guarantees.rs` now strengthens the
  cyclic header, only when a cyclic machine's guarantee is otherwise
  unprovable, with entry requirements generalized over the header
  parameters, `header == formal` for parameters every in-component arrival
  forwards unchanged, and the guarantee transported through the exit's exact
  equations; the whole strengthening is discarded when any arrival or the
  guarantee still fails, so other modules keep their roster unchanged.
  `checked-trees-to-lowered-psi` `tests::ranked_value_guarantees` pins it:
  `requires n <= previous`/`ensures result <= previous` retains
  `rank <= accumulator /\ accumulator <= previous` at the header, every
  obligation certificate is produced, the module verifies with its cycle
  certificate, dropping an arrival certificate rejects, forwarding the
  latch's `1` constant as the accumulator asks the identical cycle question
  (the retained `Natural` certificate still answers it) but replays with
  `RejectedEvidence { CertificateConclusionMismatch }` on the preservation
  obligation and refuses fresh production, and a guarantee the loop does
  not establish (`result <= n`) leaves the roster empty and the guarantee
  `OperationProofUnavailable`. Leg (b), source half, landed on macOS ARM64:
  for the free-loop form (a single-state machine re-entered by named
  backedges) `checks/contracts/exits/cyclic_headers.rs` proposes, for every
  exit and every expression guarantee, the guarantee transported through
  the exit's returned term over the invocation formals, proves it at the
  invocation arrival from `requires` and at every backedge from the
  conjunction, the re-established `requires` and the arm's guard facts
  under the arrival's simultaneous substitution, and only then discharges
  the exit by that conjunct; any failed obligation discards every proposal
  and the origin diagnostic stands unchanged (`flow/entry_origins.rs` is
  untouched). The arrival proofs run on
  `validation::scoped_arithmetic_implication`
  (`contract_entailment/scoped_arithmetic.rs`): per-proposition rosters
  over the strict engine, so one parameter symbol reads as the formal atom
  in the guarantee and the header atom in the returned term.
  `tests/contracts/cyclic_header_invariants.rs` pins acceptance of
  `requires n <= previous`/`ensures result <= previous`, the constant-step
  and unestablished (`result <= n`) twins, all-or-nothing on a false
  conjunct, and the forwarded-`limit` form;
  `pass/proofs/runtime_ranked_accumulator_guarantee_exit` (`descend(3, 9)`
  returns 1, exit 70, `runtime_ranked_accumulator_guarantee_exit_canary_runs`)
  with `fail/proofs/ranked_accumulator_guarantee_wrong_step_twin` is the
  executable canary and its wrong-accumulator twin, registered in
  `ACTIVE_PASS_CANARIES`/`ROOTED_BACKEND_PASS_CANARIES` and
  `ACTIVE_FAIL_CANARIES`. The route reads exact fixed-integer parameters
  and returns only; a loop-carried value in a second state, a `self`
  transition that changes storage, or a Wrapping accumulator is not
  proposed. The two existing fixtures are further out: their `&mut self`
  machines have no scalar graph and stop in the attached Unit closure at
  `local construction stopped at call statement shape: call count without
  a statement sequence` (`execution/unit/control/checked_machine.rs`), and
  their arithmetic claims (`result * 2 == acc * 2 + n * (n + 1)`, `embed`
  over a two-subject `Nat::BoundedDistance` rank) need the ring evidence
  below. Remaining acceptance: an arithmetic accumulation claim over a
  generated loop (the two existing fixtures, or a free-loop restatement of
  their sums) fails its functional claim on a wrong update while the
  unchanged cycle certificate still verifies, which needs the attached
  value-returning cyclic route or that restatement plus the ring evidence
  below. Law normalization (`verify_normalization`
  in `proof-admission/src/admission/normalization.rs`) has no Terminal
  consumer; routing quotient/ring-law evidence through it is shared with
  **PCC-CANONICAL-SEMANTIC-LEDGER**. Edge decrease, premise, law, and
  component identity changes, separately compiled dependency recheck, the
  absence of shape-selected trust routes, and one-SCC certificates are
  pinned by `terminal-verifier/tests/ranked_scc.rs`, `pcc_publication.rs`
  (including `receiver_replay_never_inherits_the_producer_admission_profile`),
  and `tests/architecture/layering.rs`.

- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace trusted Rust fusion of artifact
  traversal and proof search with a small total canonical-ledger generator plus
  an untrusted certificate producer. The verifier reconstructs goals and only
  checks the supplied route, under the
  [verification contract](wiki/spec/terminal-psi/verification.md).
  Inventory the existing trusted surface in this ledger, not another manifest:
  primitive judgments, checker rules, reconstructed fact kinds, normalization,
  scope, write invalidation and call/cycle composition. Each entry binds exact
  premises/conclusion, dependencies, implementation identity and soundness status
  (`Proved`, `ExplicitlyTrusted`, `Unfinished`). Mechanically fail uncovered
  accepted dispatch/fact kinds; revalidate changes to existing arms too. Coverage
  is not soundness, and unfinished rows cannot establish independent claims.
  Application interpretation uses the common kernel and the selected
  [inductive profile](wiki/spec/proofs/inductive_profile.md). Its unfinished
  soundness/encoding proofs are dependencies, not permission to trust success.
  Bootstrap discharge belongs to `BETA-DERIVATION-CHECKER` in `TASKS_BOOTSTRAP.md`,
  not a prerequisite that forces general mathematics into the Gamma checker.
  End-to-end acceptance uses a theorem-dependent program obligation after
  deleting producer/source state, under accepting and rejecting assumption
  policies. Wrong goals, profile identities and omitted transitive assumptions
  reject. A mathematical theorem alone does not establish native refinement.

- **PROOF-RELEVANCE-MIGRATION.** Finish `[erased]` noninterference and
  erased-stripped layout across remaining carriers. Erased terms remain in
  semantic/proof identity but contribute no runtime storage, tags, ABI
  transfer, or execution. Runtime use and any layout-dependent erasure reject.
  Resume: `[erased]` parses only on data fields and case payload fields
  (`tokens-to-syntax-trees/src/declarations/data.rs`); every runtime use of those
  two carriers rejects in `validation/src/relevance/`, layout strips them, and
  synthesized `Equatable` now skips them. The spec's binding-occurrence
  wording does cover parameters and `let` locals; the bracket grammar is now
  shared across all `name [properties]: Type` bindings and `[erased]` at
  those two sites fails closed with a targeted diagnostic rather than
  parse-and-dropping the marker. At 85f37369d3 `[erased]` is retained on
  signature parameters and `let` locals through the syntax, resolved and
  typed trees, `proof_only_faces` and `is_proof_machine` exempt them,
  `proof_contracts/relevance/runtime_uses.rs` rejects runtime reads and
  treats erased-position arguments and initializers as erased initializers
  (`pass/relevance/erased_parameter_proof_only`,
  `fail/relevance/erased_{parameter,local}_runtime_read`, on the
  checked-only rosters). At 59011569a5 the checked calling plan strips
  erased parameters on both call sides (`strips_erased_parameter`,
  `abi_parameter_count`; `source_position` stays the sparse authored
  index because every consumer rejoins by that index, while caller
  argument ordinals are dense over retained positions; canary_suite
  `layouts_and_pending::erased_parameter_*` pin `keep` -> `[0]`/`[70]`
  and `first`/`second` -> `[0, 2]`/`[7, 20]`); erased `self`/`const`/
  `mut` bindings refuse a plan. At a5353861bb Terminal lowering
  reconstructs every erased-stripped scalar and structural namespace
  independently from the typed relevance (`attached_unit/parameters.rs`,
  `qualifications.rs`, the `source_custody` replay/argument/successor
  readers, `direct_calls.rs` checking the producer's retained
  `argument_count` against its own retained count, `scalar_computations/
  calls.rs`, state-graph admission arity, `scalar_graph_lowering/cycles.rs`),
  and the producer's dense scalar value namespace excludes erased
  positions (`values::scalar::occupies_scalar_position`, name lookups
  that resolve to an erased parameter fail closed), so
  `pass/relevance/erased_parameter_between_runtime_values_exit` and
  `erased_proof_only_typed_parameter_exit` compile natively and exit 70
  on macOS ARM64 on the ROOTED_BACKEND/ACTIVE rosters.
  `erased_parameter_proof_only` stays checked-only because its
  `requires n < bound` names the erased binding and Terminal contract
  propositions (`scalar_contracts.rs`, `crash_routes/scalar_terms.rs`)
  carry no proof-only value term for it ("crash predicate value position
  is outside the selected scalar namespace"). At 0f162ee0b3 named
  transition arguments follow the call rule, so an erased machine
  parameter forwards into an erased state parameter through
  `transition { _ -> store(n, bound) }`
  (`pass/relevance/erased_parameter_named_transition_forward`,
  `fail/relevance/erased_state_parameter_runtime_read`, checked-only).
  Next acceptance: a proof-only scalar term for erased formals in
  Terminal contracts as one vertical slice: `ScalarTerm::ErasedParameter`
  in `semantic-vocabulary`, a contract erased-formal roster and per-call
  `erased_arguments` in `terminal-psi` (both in the contract commitment),
  one codec tag with old bytes rejecting per the encoding contract,
  verifier substitution of erased formals by the caller's erased actual
  term in `call_composition.rs`, and the producer mapping in
  `values/scalar/contract_entry.rs` and `scalar_contracts.rs`. A
  `requires` naming an erased binding is verifier-only (an erased
  binding cannot determine runtime data or control, so it never becomes a
  crash route), and an erased actual is limited to the existing
  `ScalarTerm` closure over literals, caller values, field reads and
  caller erased formals, with anything else rejecting at the initializer.
  The interpreter and native lowering need no evaluation path because
  `requires` is verifier-only today. Resume: that slice is built as
  per-machine `erased_scalar_formals`/`erased_call_arguments` rosters with
  proof-only `ScalarTerm::Value` identities from the top of the identity
  stride (no new term variant), a checked contract namespace of retained,
  then erased, then result positions, `ErasedCallArgument` rows, a
  `proofs/erased_call_arguments` pass, the spec entries and an
  `erased_argument_runtime_call` fail control; the unclaimed halves are
  parked on the local branch `work/terminal-erased-term-parked`
  (def0c1be67 on bdf2b1665a, does not compile alone) with the apply
  scripts and recipe under `.codex-parked/`, waiting on the
  MATCH-SELECTIVE-LOWERING claim over terminal-psi/codec/verifier/
  interpreter. Two limits stay fail-closed: composed-route state
  contracts carry no `requires`, so `erased_parameter_named_transition_forward`
  stays checked-only, and a caller's own erased formal forwarded through
  a call has no row.

## P4 - ABI, borrowing, and callbacks

- **NORMALIZED-ABI-LOWERING.** Finish target-independent signature
  normalization and target-owned calling/layout realization for aggregates,
  dynamic values, callbacks, and foreign boundaries. Acceptance: the ABI is
  independently reconstructible and no target placement leaks back into
  Terminal Psi.

  Resume: `&dyn Trait` boundary parameters now normalize to the two-word
  `{instance, table}` descriptor shape (`Reference`, 2*pointer_size) in
  `provider-planning/src/calling_policy_plans.rs::value_shape_from_type`,
  witnessed by
  `compiler/tests/calling_policy_plans.rs::borrowed_dynamic_trait_parameter_materializes_fat_descriptor_shape`.
  Next dependency: `TargetUnitOperation::NormalizedForeignCall` and the
  dynamic/installed-provider op carriers are produced upstream but have no
  consumer in `target-operations-to-selected-instructions` (currently a
  custody-mismatch catch-all), and the common native route rejects callback
  transport in `native-realization/src/native_realization/object.rs`. The emission
  chain (selection -> register homes -> machine emission -> object import
  plans -> image custody -> physical derivation) is the next bounded slice.
  Normalized foreign lowering, machine-code custody, image replay, and artifact
  derivation still bound foreign arguments/results to fixed-width integers;
  widening them needs a coordinated lane once emission exists. Preserve agreement
  between stored dynamic-reference layouts and normalized calling-policy shapes;
  `calling_policy_plans::borrowed_dynamic_trait_record_fields_retain_both_descriptor_words`
  checks neighboring fields and thin sized-reference controls across the four
  hosted target layouts. Layout agreement alone does not close native transport.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Complete [representation agreement](wiki/spec/build/opaque_representations.md) at
  independently compiled by-value exchanges. Dependency-first review compilation
  now rejoins each consumer's actual foreign opaque uses to the producer review's
  own declaration and opaque/conformance/carrier availability rows, bound to the
  producer review's immutable source instance, with strong selected-application
  equality enforced against the producer's own selection at actual exchanges
  (`PackagePolicyRepresentation::rejoin_foreign_demands`), witnessed through
  the package-manager review path by the two-package canaries in
  `package-manager/tests/opaque_boundary_agreement.rs` (agreement rejoins the
  consumer's `Token` demand to the producer's `Carrier`/`TokenRepresentation`;
  a consumer-local application rejects with `SelectedApplicationMismatch`);
  remaining: physical movement and lifecycle planning, including transitive
  inert-carrier proof and multiplicity checks. Equal size/alignment or compact
  fingerprints cannot establish agreement. External compatibility and
  native-foreign binding materialization now consume the same authoritative
  selection list (`evaluate_compatibility_boundary_entry_plan`), witnessed by
  `calling_policy_plans::compatibility_boundary_materializes_the_selected_opaque_carrier`
  and `calling_policy_plans::compatibility_boundary_rejects_opaque_by_value_without_build_selection`;
  physical transport, artifact custody, and lifecycle/drop planning remain open.

  Carry the application through native artifacts, replacement compatibility,
  stable-handle eras, and independently replaceable provider contracts.
  Acceptance: independently compiled producer/consumer and historical-selection
  canaries cover sealed `Ptr<T>` target semantics, proof-only `Real`,
  `EfiSystemTable`, provider/replay drift, cleanup, and multiplicity; incompatible
  by-value exchanges and replacements reject before execution.

- **WRITE-ONLY-BORROW.** Finish `&write T` through projected aggregates,
  calls/results, dynamic dispatch, cleanup and native execution. Extend ordinary
  reference preparation/receiving replay in
  `target-operations-to-selected-instructions/src/legalization/scalar_graph_input/`
  and image/installation publication; **STRUCTURAL-BORROW-IDENTITY** owns the
  common reference ABI. Reuse `terminal_psi_indexed_receivers` native tests,
  including `publication::` and `primitive_stores::`, rather than a
  store-specific emitter. Cross-emission is not matching-host execution.

  Remaining work includes early loan closure/restored-parent use, escaping
  carriers, dynamic indexes and reference-bearing projections that can be
  located without reading a stored pointer/descriptor. Computed IEEE stores need
  a real source-selected operation, result transport and ordinary store
  composition, not widened admission: Psi's
  `execution/unit/selected_ieee_float.rs` and Omega's shared graph/provider
  route must retain format, selected occurrence and result evidence. The removed
  Unit/FMA planner is not a dependency to rebuild.

  Acceptance: writes affect the original caller referent across calls and
  register/stack passing; reads through write-only access reject. Cover exact
  width/write coverage, untouched neighbors, runtime signed/Boolean/floating
  sources, restoration/return behavior, access substitution and independent
  artifact replay. Observe computed floating stores on the caller, not just
  checking or a copied frame home. Run both Linux target runtime legs when
  available and record unavailable hosts.

  Borrowed byte-view call transport now composes through the ordinary control
  graph: shared views arrive from established literal/subslice producers,
  shared block parameters, or the incoming machine parameter, byte-sequence
  literal establishment lowers in `control_flow/operations.rs`, scalar-ABI
  callers admit literal place rosters, mixed-ABI callers admit borrowed block
  structural parameters, and installation records publish internal Unit calls
  in physical text order rather than source block-roster order.

  `&write` projected aggregates now compose through ordinary calls:
  `aggregate_results::borrowed_arguments` admits `WriteOnlyBorrow` arguments to
  record/sum referents, so projected `&write`/`&mut` receivers, closed-loan
  restored parents, and nested alias chains transport the incoming root pointer
  plus its exact projected byte offset through register and stack ABI slots and
  installation replay. On macOS ARM64
  `mbx nextest run -p omega-native-differential-test --test terminal_psi_indexed_receivers`
  passes 38/38 including caller-storage observation, exact widths, neighbors,
  signed/Boolean/IEEE sources, and access-substitution rejection; both Linux
  legs were not run. `terminal_byte_views` `natural_writer::` (8) is
  unrelated red at this base, owned by the in-flight IRFUEL work;
  `terminal_psi_source` compiles again (1a56e53b8d) and its 8 remaining
  failures are attributed in
  [known baseline failures](wiki/drafts/known_baseline_failures.md).

  Call-boundary validation is converged: expression-call and transition-target
  arguments route through the same `validate_call_argument` projected-subloan
  admission as statement calls, and value-position calls enforce the shared
  reference-access contract — explicit `&write` attenuation for write-only
  parameters, no widening of `&write` authority into readable access. Checked
  terminal plans retain `WriteOnlyBorrow` with exact `Field`/`FixedIndex`
  segments; see `typed-trees-to-checked-trees/tests/write_only_call_arguments.rs`.

  Scalar-leaf projections now compose through ordinary borrow calls:
  `mutate(&mut self.value)` and `fill(&write self.value)` verify, admit through
  Omega optimization, lower, publish and execute on host — one canonical
  leaf-shape resolution joins verifier, codec, interpreter, optimization
  catalog, `scalar_graph_input` reference preparation and target replay.
  Restored-parent `let` bindings still fail upstream in
  `typed-trees-to-checked-trees/src/flow` (live NOMINAL-FIELD-FLOW claim).

  Ordinary borrowed-argument replay now covers more than receivers:
  disjoint `&write` argument pairs, mixed `&write`/`&mut` disjoint field
  arguments, owned-local scalar borrows, explicit `&write` re-forwarding
  through callee parameters, bare `&mut` forwarding, and attached callees
  carrying extra `&write` parameters all publish on the four hosted targets
  and execute on host with exact referent custody
  (`terminal_psi_indexed_receivers::borrowed_arguments::`, 7 tests; host run
  Linux x86-64). Native exports order borrowed parameters after scalars.
  Every artifact Psi currently emits already replays through
  `scalar_graph_input`; probing found no Omega-side gap to fill.

  Early loan closure now replays natively for the `let`-bound shapes Psi
  emits: head-of-body `let` subloans over projected elements and fields
  restore the parent for later calls, two live disjoint subloans close
  independently, the restored parent reaches the same element the closed
  subloan used, and the shape replays inside a borrowed callee, under
  `&write` and `&mut` parents — `terminal_psi_indexed_receivers::
  loan_closures::` (5 tests; published on the four hosted targets,
  executed on macOS arm64).

  Remaining acceptance is upstream in Psi, which produces no artifact for:
  `let` borrow declarations after non-let statements (mid-body or
  sequential lets), scalar and aggregate `let` borrows forwarded as call
  arguments or used as store roots
  (`let held: &write [u16; 4] = &write values; held[2] = 17` is rejected while
  `held[1].replace()` composes), `&mut` receiver calls on a borrowed parent
  while a subloan exists, projected-element scalar stores on borrowed
  arrays (`records[1].value = 17` on `&mut`), whole aggregate or `[copy]` sum
  replacement through borrows, owned-record field borrows as call arguments,
  shared `&` scalar callee bodies ("scalar callee has no checked executable
  body"), escaping borrow-carrying aggregates, dynamic indexes (unbounded
  indexes fail bounds proof; declared `[0..=3]` ranges still produce no
  source-independent plan), `&mut dyn` dispatch, and computed IEEE stores —
  all with `machine has no source-independent checked scalar control plan`
  unless noted. Reads of
  `&write` roots, bare `&write` forwarding, `&write`→`&mut` widening, and
  same-root `&write` argument pairs still reject upstream as required.

  Parked WIP: unmerged local branch `write-only-borrow` at 71a647f464 (over
  348c542350, Windows coordinator checkout) carries the computed-IEEE-stores
  slice — selected IEEE binary operations through the native pipeline plus
  `&write` field reads/computed stores, 135 files. Have the coordinator merge
  it to main through the landing queue before re-implementing that slice.

- **STRUCTURAL-BORROW-IDENTITY.** Enforce the settled
  [structural borrow identity contract](wiki/spec/terminal-psi/structural_access.md)
  through call argument preparation and native validation/replay. Extend the
  [target signature checks](omega-rust/omega/pipeline/abstract-operations-to-target-operations/README.md)
  to embedded callee plans, projected argument/home identity, and standalone
  receiving entrances. All borrowed access modes retain `BorrowedReference`;
  shared physical shape cannot authorize access substitution. Acceptance: caller-
  visible writes, forwarded references, legal synchronized shared observations,
  write-only non-reading, and register/stack pointer passing work on both Linux
  targets. Independently formed or substituted access/shape/placement pairs
  reject; direct-home controls must use owned semantics or test rejection of
  borrowed copies. Do not claim copy equivalence merely because a
  following callee sees the staged write.

  Standalone-receiving-entrance slice landed at 3f48fd6a15:
  `structural_signatures::validate` now replays every standalone receiving
  entrance — the graph's own scalar parameter rows and the published
  `scalar_abi`/`mixed_structural_scalar_abi` — against the signature
  independently derived from the abstract declaration rather than trusting the
  embedded plan. Scalar and result rows must bind the declared value identity,
  scalar type, and canonical plan placement; a published ABI is admitted only
  for the family its producer derives (scalar-only service-free for
  `scalar_abi`, scalar-result with a structural suffix over Boolean/fixed-integer
  parameters for the mixed form), so a coherent plan on an ineligible function,
  either ABI form beside the other, or any substituted row rejects. Borrowed
  structural parameters keep `BorrowedReference` through the shared classifier.
  Witnessed on linux_x64 and linux_arm64 by the substitution matrices in
  `src/tests/scalar_abi.rs` and `src/tests/scalar_primitive_stores.rs`; native
  caller-visible writes and register/stack pointer passing replay through the
  `primitive_store_return` and `terminal_psi_indexed_receivers` differential
  legs. Remaining: the control-flow call path beyond standalone entrances.

- **BORROW-PROOF-CONVERGENCE.** Make ordinary borrow checking proof-producing
  under the [loan contract](wiki/spec/terminal-psi/loans.md), without allowing
  proofs to create or amplify authority. Extend symbolic
  range ordering and containment beyond exact shared immutable boundaries, then
  admit explicit compatibility theorems over
  already-existing places and occurrences. Acceptance: proof evidence can
  establish disjointness/containment but cannot extend lifetime, duplicate a
  loan, or replace ownership accounting.

  Extend range-premise read dependencies beyond the supported selected-call,
  selected-index, and atomic-load cases only when complete footprints and
  operation stability are established. Explicit arguments alone do not
  establish all callee reads;
  preserved numeric captures must remain independent of subsequent source writes.

  First slice landed at e47adbb9eb: `slice_tail_strictly_decreases`
  (`omega-rust/psi/semantics/validation/src/slice_ranking.rs`) now routes the
  tail start and `len` guard bound through the shared immutable-integer-bound
  normalization, admitting `param[k..]` (literal or immutable local copy,
  `k >= 1`) when the guard proves `len >= k` — `entries[step..]` under
  `entries.len >= step` with `let step: u64 = 2` compiles via
  `mbx run -p omega -- --check` on Windows. Second slice landed at
  7683795990 on macOS: `place_segments_containment`
  (`checks/borrows/overlap/segments.rs`) now evaluates `Index` selectors as
  normalized extents inside the shared selector-snapshot session, so
  symbolic and `symbol ± k` windows prove directional containment, same
  extent, and point membership; disjoint or unknown bounds stay `None`.
  Overlap and containment record through one session, so certificates keep
  replaying exact selector positions. Evidence only — admission still gates
  on `non_interfering`, witnessed by
  `proven_containment_never_licenses_a_second_mutable_loan` under
  `tests/borrow/certificates/value_snapshots.rs`. Third slice landed at
  b12f32b87b on macOS: compatibility theorems now carry explicit stated
  ordering premises. `BorrowCompatibilityPremise` pins the exact
  `ContractProofFact` identity, relation, and normalized operands consulted
  at the formation scope (machine-entry and state `requires`; inherited
  rows excluded), and a `Premised` derivation replays that ledger
  positionally against current contracts — missing, reordered, or changed
  tokens fail replay distinctly from selector-snapshot drift. The shift
  algebra admits `L <= R`, `L < R`, and equality over immutable normalized
  bounds with constant offsets; mutable, computed, or foreign subjects stay
  unproven. Evidence only — resource joins, loan identity, access polarity,
  restoration, and multiplicity stay independently enforced, witnessed by
  the tampering cases under
  `tests/borrow/certificates/stated_premises.rs`. Fourth slice landed at
  821eed4fea on Linux x86-64: range-premise read dependencies now admit
  atomic loads. `collect_reads`
  (`checks/ranges/facts/dependencies/reads.rs`) handles
  `ExpressionNode::Atomic` under a `Load` ordering plan: `value` names
  the resident place itself, so its complete footprint is exactly that
  place plus its selector reads. Operation stability is pinned by a
  load-legal `MemoryOrdering`, canonical `Scalar` result custody, and an
  empty `result` -- each rechecked against the ordering plan so a
  writing axis cannot borrow the load's place-shaped footprint. Store,
  swap, read-modify-write, and both compare-exchange shapes still return
  an incomplete read set: their `value` wraps a stored operand or
  instruction-shaped update, so no operand scan describes their reads.
  Evidence only -- preservation and invalidation run through the
  existing place algebra, witnessed by
  `tests/range_atomic_dependencies.rs`
  (`atomic_store_retires_only_the_resident_place_premise`) and
  `facts/dependencies/tests/atomics.rs`: a `requires` premise on
  `self.counter.load(NoOrdering)` survives writes to sibling fields and
  unrelated parameters and is retired by a store to the resident place
  or to `self`. Fifth slice landed at 126bdce303 (Linux x86-64):
  range-premise read dependencies now cover selected calls — an exact
  checked-call join authenticates the occurrence, and operand/receiver
  places materialize as footprints. Selected indexing already uses
  `collect_selected_index_reads` with exact checked statement-use custody
  and recursive operand footprints. Preserve incomplete read sets for open
  ranges and requires-scope operators without statement-use custody; further
  admission needs complete footprints, not a second indexing collector.
  Sixth slice landed at a6c15c7990: stated ordering premises now
  discharge indexed borrow conflicts beyond loan formation — the stated
  premise set is collected once per state in `check_flow_call_borrows` and
  threaded through every compatibility entry point, both segment
  expressions evaluate to one `EvaluatedIndexExtent` inside the shared
  selector session, and a single extent comparator consults stated
  premises only after the structural order fails. Seventh slice landed at
  0c37e8f707 (macOS): authored `[]`/`[..]` applications admit checked read
  dependencies when the exact checked operator-use row at the statement
  occurrence resolves to a single stable selection; nested operands
  recurse through the ordinary read scan, hoisted selector operands read
  their frozen capture identities, and missing, foreign, ambiguous,
  drifted, or requires-scope custody stays incomplete. Eighth slice
  landed at 5f1733161a (macOS ARM64): `collect_reads` now covers builtin
  range windows and compound operands — `items[low..high]` reads the
  collection place plus each valid endpoint, open `items[..high]` windows
  read only their present bounds, and fully constant half-open selectors
  canonicalize to `FixedRange` so extent overlap and disjointness stay
  exact. `Match` subjects, value patterns, and arm values, array-literal
  elements, struct-literal fields, and `ExpressionNode::Range` endpoints
  all recurse through the read scan. Range-indexed builtins are gated on
  the collection's builtin fixed-array/slice geometry resolved under
  `OperatorSpelling::Range` (element-result typing intentionally returns
  none for windows), while selected authored range operators keep exact
  checked-occurrence custody. Incomplete read sets are preserved for
  missing or foreign custody, unresolved or ambiguous selection, and
  absent or invalid range operands. Evidence only — disjointness and
  preservation still run through the existing place algebra and loan
  checks. Witnessed by
  `facts/dependencies/tests/indexes.rs`: 34 focused dependency tests, 79
  range-checker tests, and 231 range integration tests pass; the crate
  suite's 14 failures reproduce identically at base 5006b9314c. Ninth
  slice landed at 74836aa638: `collect_reads` now admits every writing
  atomic axis through its assignment-carrier footprint. `place.store(v,
  ord)` and the `let r = place.op(..)` forms desugar to `target = Atomic
  { .. }`, so `collect_atomic_write_reads`
  (`checks/ranges/facts/dependencies/reads.rs`) reads the carrier
  statement's target as the resident place, each stored operand through
  the operand gate (`value` for store and swap, the fetch operand for
  read-modify-write, the expected and replacement operands for a
  decisive compare-exchange whose `value` is the exact `prior + (prior ==
  expected) * (replacement - prior)` model), and the current local
  `result` destination; ordering legality, scalar result custody, and
  result shape are rechecked per axis, and the model's prior placeholders
  are pinned to the result symbol rather than scanned as reads.
  Incomplete read sets are preserved for a missing carrier, an illegal
  ordering plan, non-scalar custody, a missing or non-current result
  destination, a substituted update model, and the single-attempt
  observing form (`CompareExchangeOnce`). Evidence only — witnessed by
  `facts/dependencies/tests/atomics.rs` (8 tests) and
  `tests/range_atomic_dependencies.rs`
  (`writing_atomic_axes_retire_only_the_resident_place_premise`): a
  `requires` premise on `self.counter.load(NoOrdering)` survives a store,
  swap, `fetch_add`, or `compare_exchange` on `self.other` and is retired
  by the same axis on `self.counter`. Tenth slice: the
  operand gate `collect_operand_reads` now admits authored bound
  arithmetic in selectors. When a bound's subtree lacks builtin meaning
  the gate walks it node by node: a builtin arithmetic node recurses into
  its operands, and an authored `+`/`-`/`*`/`/`/`%` application is
  call-shaped, so `collect_selected_arithmetic_reads` admits exactly the
  operands its exact checked operator-use row at the statement occurrence
  authenticates — one `selected_operator_operands` join now serves the
  `[]`/`[..]` and arithmetic spellings (a single stable `Resolved` row,
  valid selection, intact candidate roster, matching spelling and operand
  count) — and each operand recurses through the gate so a nested
  authored application proves its own custody. The builtin point-selector
  arm of `collect_selector_reads` uses the same gate. A constant-shaped
  application (`1u64 + 0u64`) stays incomplete even with custody because
  `index_place_segment` folds it syntactically to builtin arithmetic's
  coordinate; a non-constant selector keeps its conservative `Index`
  segment, so a fixed-element write still retires the facts. Incomplete
  read sets are preserved for missing, drifted, or requires-scope
  custody, constant-shaped authored applications, authored comparisons
  and other non-arithmetic spellings, and authored arithmetic at an
  expression root (the top-level `record_dependencies` floor is
  unchanged). Evidence only, and no source compiles differently yet:
  window validation still needs builtin bound meaning to prove a
  computed start bound (even builtin `items[low + 0u64..high]` reports
  "cannot prove subslice range start bound"), so this slice closes the
  read-set shape ahead of value semantics for authored operators.
  Witnessed by `facts/dependencies/tests/indexes.rs` and
  `facts/dependencies/tests.rs`: `items[low + step..high]` under an
  authored `u64` `+` reads `low`, `step`, `high`, and the window place,
  survives a write to `unrelated`, and is retired by a write to any
  operand or to `items`; 130 range-checker tests pass (8 new). Eleventh
  slice landed at 33d79f1bdf (macOS ARM64): statement-level borrow
  admission no longer excuses a forming loan whose recorded source owner
  merely matches an exclusive active loan's owner.
  `checks/borrows/statements.rs` admits each forming/active pair only
  through the replayed non-interfering verdict or a replayable
  carried-authority edge — an exclusive active loan, a replayed
  containment verdict placing the forming place inside it (`Same` or
  `RightContainsLeft`), and the exact recorded provenance (a retained
  reborrow's parent handle, or an unretained transfer's rebasing source
  owner; a `DirectRoot` carries nothing). Every admitted pair publishes
  a `CheckedBorrowCompatibilityCertificate`, and certificate replay
  re-derives the edge from the loan rows, so a forged interfering
  certificate without provenance rejects. A rejected certificate ledger
  also no longer silences the substrate: resource and lineage replay
  still runs on a scratch copy so its diagnostics surface beside the
  certificate findings without publishing into a rejected pass.
  Evidence only — no loan is created, no lifetime extended, and
  resource/ownership accounting stays authoritative. Witnessed by
  `checks/borrows/tests.rs` (4 new tests): carried pairs retain
  replayable certificates across passes, an overlapping same-owner loan
  without the edge rejects, a forged interfering certificate fails
  replay, and the edge's truth table pins exclusivity, containment
  direction, and lineage exactness; the crate suite's three remaining
  failures reproduce identically at base 47e5773bb6.

  Twelfth slice landed on macOS ARM64: range-premise read dependencies now
  admit statically applied calls. The `ExpressionNode::Call` arm of
  `collect_reads` (`checks/ranges/facts/dependencies/reads.rs`) no longer
  refuses every `machine_arguments` application;
  `static_application_carries_no_caller_storage` admits an occurrence whose
  static arguments are storage-free — a const literal, or a symbol of kind
  `BuiltinType`, `Data`, or `Const` — because a type or const selection
  substitutes a declaration identity or a compile-time value rather than a
  place: "Specialization substitutes the outer selections and continues
  through nested applications until executable calls are direct; it creates no
  runtime dictionary" ([generics](wiki/spec/language/generics.md)). So
  `identity<u64>(original)`, `identity<Card>(&original)` and
  `scaled<2u64>(original)` read exactly the checked operand accesses the
  unapplied call carries, and an applied `&self` callee still adds the
  canonical receiver place. Incomplete read sets are preserved for a
  machine-valued static argument (`apply<double>`, whose callable body this
  occurrence never authenticated), a nested static application
  (`identity<Pair<u64>>`), an evidence projection, and the unchanged static
  binder, requirement-dispatch, quotient and private-layout guards; open
  ranges and requires-scope operators without statement-use custody are
  untouched. Evidence only — preservation and invalidation still run through
  the existing place algebra. Witnessed by
  `facts/dependencies/tests/calls.rs` (5 new tests:
  `a_type_applied_generic_call_reads_its_established_operand_footprint`,
  `a_const_applied_generic_call_reads_its_operand_footprint`,
  `a_type_applied_self_receiver_call_reads_the_callers_machine_storage`,
  `a_machine_valued_or_nested_static_application_stays_incomplete`,
  `only_storage_free_static_selections_admit_the_applied_call_footprint`,
  the last pinning that the selection's kind decides by retargeting one
  admitted argument at a callable state symbol and at a nested application);
  `cargo nextest run -p typed-trees-to-checked-trees` is 4177 tests, 4174
  passed, 3 failed — the same three failures as base 82159a2836. Still
  incomplete after this slice: statically dispatched requirement calls,
  machine-valued applications, the single-attempt `CompareExchangeOnce`
  observing form, and authored non-arithmetic operator operands, including a
  cast or unary wrapper standing above an authored arithmetic application in a
  selector or bound position.

  Wrapped-operand slice landed on macOS ARM64 (ordered after the
  statically-applied-call slice, which is now on `main`): the range-premise
  operand gate now sees through cast and unary wrappers.
  `collect_operand_reads` (`checks/ranges/facts/dependencies/reads.rs`) walked
  only `Binary` once the builtin bound floor failed, so `items[(low + step) as
  u64]` refused a footprint even though the wrapper contributes no storage of
  its own: "Every evaluated child runs exactly once. Unary operations,
  borrows, casts, membership tests, and member access have one immediate
  runtime child" ([expressions](wiki/spec/language/expressions.md)), and the
  closed token vocabulary binds no spelling to a cast or a `!`/`~`, so neither
  node can be a selected declaration. The walk now descends through both to
  the authored application below, which still has to prove its own exact
  checked operator-use custody. So `items[(low + step) as u64]`, `items[~(low
  + step)]`, `items[(low + step) as u64..high]` and `items[low..(low + step)
  as u64]` read exactly their operands plus the selected element or window
  place. Incomplete read sets are preserved for every family refused before:
  an authored comparison under a wrapper (not an arithmetic spelling), a
  constant-shaped authored application (`(1u64 + 0u64) as u64`, which the
  place algebra folds syntactically), a drifted or ambiguous use row, a
  wrapped operand whose call carries no checked occurrence, and requires-scope
  occurrences without statement-use custody. Evidence only — no second
  indexing collector, and preservation and invalidation still run through the
  existing place algebra. Witnessed by `facts/dependencies/tests/indexes.rs`
  (`a_wrapped_authored_arithmetic_selector_reads_every_operand`,
  `a_wrapper_over_a_refused_operand_family_stays_incomplete`,
  `a_requires_scope_wrapped_arithmetic_selector_has_no_statement_use_custody`)
  and `facts/dependencies/tests/calls.rs`
  (`a_wrapped_arithmetic_selector_still_proves_its_call_operand_footprint`);
  `cargo nextest run -p typed-trees-to-checked-trees` is 4176 tests, 4173
  passed, 3 failed against base 779b8eaffa's 4172 tests, 4169 passed, 3 failed
  — the same three failing names. Next remaining family: a bare authored
  arithmetic application in a point selector (`items[low + step]`) still
  records no footprint, and that refusal sits above the operand gate rather
  than inside it — the identical operands are admitted once a cast gives the
  occurrence builtin index meaning — so the index-meaning probe
  (`has_builtin_index_meaning`) and the `[]` use-row lookup below it are what
  the next slice has to close. Statically applied calls, statically dispatched
  requirement calls, machine-valued applications and the single-attempt
  `CompareExchangeOnce` observing form remain incomplete as before.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Add target-owned private callback slots
  selected through exact conformances and validated layout paths under the
  [private-callback contract](wiki/spec/build/private_callbacks.md). Authenticate
  the complete plan application and replay the authored-use-to-Terminal-operation
  join independently; retained producer digests alone are insufficient. Private
  slots must be absent from source-visible schema and inaccessible as ordinary
  fields or addresses. Acceptance: one outbound registrar closes without a raw
  code pointer or duplicated placement authority.

  Resume (swarm-w8, Linux x86-64): the outbound registrar already closes at
  the checked level. `compile_to_checked` over
  `tests/callback_materialization_closure.omg` plus a reachable
  `WindowRegistrar::register<P::call, P::call>(&self.specification)` use binds
  two `BoundNominalCallbackPlacement`s carrying `NativePlace::Field`
  destinations, exact conformance applications, and an authenticated two-entry
  catalog — no raw code pointer, no duplicated placement authority. At
  `888fb154ff` the telescope admission is landed: `build_call_operation` and
  `build_static_boundary_requirements`
  (`execution/unit/calls/build_calls.rs`, `execution/unit/control/build_control.rs`) discharge each `Machine{Nominal}`
  signature type parameter through a `nominal_machine_use` at the exact site
  (ordinal plus satisfaction row), taking the binder's destination from an ABI
  `native callback` entry where the ordinal declares one and otherwise
  requiring the use's retained `callback_placement` — the evaluated boundary
  calling plan the `PrivateCallbackSlot` conformance supplied. Witness:
  `reachable_private_callback_registrar_binds_its_terminal_occurrence`
  (`cargo nextest run -p compiler --test callback_terminal_custody`) — the call
  emits its `BoundaryCall`, reaches Terminal, and replays the authored-use
  join; failure no longer reads "0 Terminal registrar occurrences". At
  `b1c7dbd56d` the void-body stop is cleared (verified macOS ARM64):
  `lower_bounded_callback_identity_machine`
  (`omega-rust/psi/pipeline/checked-trees-to-lowered-psi/src/machine_lowering.rs`)
  no longer reports "bounded callback body has no checked scalar graph" —
  `lower_bounded_callback_unit_body` admits the one-state `u64 -> Unit`
  complete-only Unit plan beside the `u64 -> u64` identity scalar graph, and
  `validate_direct_callback_thunk_shape` accepts the matching
  `TerminalMachineResult::Unit` + `ReturnUnit` leaf; the witness closes the
  Terminal product with both field-destination thunks decoded. Custody now
  stops at native production, the field-cohort receiving stages: the
  `NativeArtifact` product rejects in `reject_unconsumed_callbacks`
  (`omega-rust/omega/compiler/native-realization/src/native_product/admission.rs`)
  with "native-artifact production cannot discard 2 validated callback
  placement(s) for `WindowProcedure::call`, `WindowProcedure::call`; canonical
  Terminal callback-use custody is not implemented", and the retained route
  `realize_retained_native_artifact` rejects in `admitted_native_callbacks`
  (`retained_native_product.rs`) with "ordinary native realization currently
  admits exactly one direct callback" and, for a single `NativePlace::Field`
  placement, "field callback materialization is outside the direct-parameter
  cohort". The direct-parameter witness
  `direct_callback_relocation_resolves_to_its_private_function` is red at base
  on every host: `emit_realization_object`
  (`native-realization/src/native_realization/object_emission.rs`) rejects any
  callback thunk with "callback ABI transport is not implemented in the common
  instruction pipeline". Next: admit the field cohort through both native
  routes without a raw code pointer.

- **REGISTERED-CALLBACK-LIFETIME.** Model successful registration as a linear
  external root and unregister as the operation that ends it before releasing
  code/component leases. Capacity bounds live registrations, not emitted
  thunks. Acceptance covers rejection, retry, replacement, cleanup, and an
  actual Windows callback after the generic path closes.
  The generic chain is pinned at 4f817ed0a6:
  `component-publication`
  `tests::package_registration_owns_exact_component_era_lease_through_replacement`
  (`mbx nextest run -p component-publication --lib`, macOS x86-64) covers
  private-entry attribution binding, cross-occurrence rejection, provider
  rejection/retry, capacity-occurrence collision, foreign and exact
  component-era lease lowering, unregister retry, quiescence, lease release,
  and a replacement registration reusing the returned slot/capacity.
  Remaining acceptance: an actual Windows callback once the generic path is
  driven from `build.omg` on that host.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Generalize retained outbound arguments
  beyond callbacks with explicit call-scoped, lifetime-borrowed, moved, and
  snapshot dispositions. Every retained pointer needs exact stable backing,
  range, access, lifetime, and revision provenance; unknown or mutable ambient
  backing rejects.

## P5 - Cathedral over general Omega primitives

- **BUMP-ALLOCATOR-CANARY.** Build a package-level allocator over one qualified
  `Extent`, supporting two coexisting allocations, exact cleanup/recomposition,
  and reset only after full return under the
  [allocation contract](wiki/spec/resources/allocation.md). Use it to discover the real `Vec<T>`
  contract; do not add allocator semantics to the compiler.
  Landed at 4d45081529: `tests/omega/pass/memory/bump_allocator_canary` checks
  two coexisting allocations and full-return recomposition over a package
  `ExtentPartition` boundary (Linux x86-64,
  `OMEGA_PASS_CANARY_FILTER=memory/bump_allocator_canary cargo nextest run -p compiler --test canary_suite entry_and_abi::pass_canaries_compile`).
  `exercise` now chains `allocate`/`reset` as ordinary package machines with
  conserved custody, beside the direct-boundary `exercise_boundary` control.
  Discovered contract edges, recorded in the canary header: a record returned
  by a call does not surface its fields' `in Granted` facts on the result's
  field places, so each qualified field is restated through an exactly typed
  `let`, and the restated fields of one record share custody — consuming a
  restated sibling retires the record's other restated facts, so a container
  must order custody or keep the family in one record consumed atomically;
  an ordinary machine cannot restate a boundary's `separate` law as its own
  `ensures`; multi-input recomposition is admitted only through one record
  parameter at a boundary. The bounded-request slice adds the contract's
  counted byte residual to `Bump` and puts the exact capacity requirement on
  `allocate` (`requires length <= strategy.remaining`); new edges: `split`'s
  conservation law never pins `result.taken.length`, a content-carrying
  machine's `ensures` admits only qualification/content-projection/`separate`
  clauses (no scalar residual equality), a `requires` bound does not
  propagate a subtraction's lower bound, and entry `requires` facts do not
  survive the first request's consumption of the backing — so each request's
  residual is a caller-stated premise. At 16fee936df the canary
  establishes, reads and retires one resident element through a package
  `ResidentStorage` boundary (`place` -> `Occupied { storage: Extent in
  Granted & Resident<SlotPlacement, Slot> }`, borrowed `read`, owned
  `retire` -> `Granted & Vacant`), the `ResidentContentTransfer<P,T>`
  provider-issuance route spelled as one concrete application; fail
  canaries `memory/bump_allocator_{reset_with_live_resident,
  resident_dropped,place_into_occupied}` pin the rejections. At
  921c76263e a typed-let restating `Extent in Granted & Resident<P,T>`
  carries the interned instance identity (`facts/field_domain.rs`,
  `flow/transfers.rs`), so `exercise_resident_restated` proves `view` and
  `retire_view` at call `requires` and restating under another index is
  refused as distinct normalized instances
  (`fail/memory/bump_allocator_restated_resident_index_mismatch`). An
  indexed domain application in proof-fact position (`ensures result in
  Granted & Resident<P, T>`) is carried by argument (macOS ARM64): the
  syntax and symbol-resolved `ProofMembershipFact` retain the argument span
  read with the type-position grammar
  (`tokens-to-syntax-trees/src/contracts/facts.rs`); the resolver lowers
  the arguments as child type references and binds them in the owning
  machine, trait-requirement, operator, domain, or data scope
  (`syntax-trees-to-symbol-resolved-trees/src/lowering/domain.rs`,
  `symbols/contracts.rs`, `symbols/domain_facts.rs`); and
  `symbol-resolved-trees-to-typed-trees/src/contracts/proof_facts.rs`
  lowers the arguments, checks their count against the family's index
  binders, and interns `semantic_domain` in a finish pass beside
  domain-constraint normalization
  (`lowerer/tests/machine_contracts.rs` proves the fact's identity equals
  the return constraint's for a closed `Resident<SlotPlacement, Slot>` and
  a generic `Quantity<To>` bound to the requirement's own binder).
  `pass/contracts/proof_fact_indexed_domain_application` compiles the
  shape end to end (`place` ensures the instance; the caller restates it
  through an exactly typed `let` and proves `view`/`retire_view`);
  `fail/contracts/proof_fact_indexed_domain_application_arity` pins the
  count rejection; a `|` alternative and a compiler carry permission still
  reject an application by name. A restating write of a call result now
  joins the callee's `ensures` membership by exact instance (macOS ARM64):
  `typed-trees-to-checked-trees/src/facts/index_compatibility.rs` reads the
  typed `ProofMembershipFact` (`semantic_domain`, `domain_arguments`) on
  the whole reserved `result` from the call's contract row
  (`ProofFacts.contract_calls[..].ensures`) as the value's actual instance
  beside its declared return type, so `let placed: Extent in Granted &
  Resident<OtherPlacement, Slot> = storage.place(..)` where `place` ensured
  `Resident<SlotPlacement, Slot>` is refused at the `let` as distinct
  normalized instances even when `view`/`retire_view` are declared over the
  restated index
  (`fail/contracts/proof_fact_indexed_domain_application_mismatch`, to be
  registered beside `_arity` in `canary_suite.rs`, which was under two
  live claims when the fixture landed). The join could not live in
  `checks/contracts/writes.rs` (`predicate_domain_constraint_identities`
  keeps predicate domains only, so a bodyless family never reaches
  `value_proves_domain`) nor on the semantic `ContractDomainMembership`
  fact: `facts/qualification_evidence.rs::call_contract_evidence` drops an
  `ensures result in D` membership at a boundary requirement without an
  admitted `qualification_authorization`, so that promise never becomes a
  caller fact and the pass fixture proves `view` from the `let`'s
  declared-type seeding (`flow/transfers.rs`), not from the ensures.
  That gap is now closed (macOS ARM64): the same collector refuses a
  restating write whose declared type names an indexed instance of a
  bodyless family when the value is a call carrying no instance of that
  family at all, so `place` ensuring only `result in Granted` no longer
  admits `let placed: Extent in Granted & Resident<SlotPlacement, Slot> =
  storage.place(..)`. Establishment is read from the call itself -- its
  declared return type or an `ensures` on the reserved `result` -- because
  `domains.md` keeps establishment on the value's own route and
  `placed_access.md` fixes `Resident`'s whole route set (initialization
  from `Vacant` and an owned `T`, a `ResidentContentTransfer<P, T>`
  issuance occurrence with its receipt, or forwarding existing custody);
  a declared local type is an obligation, not evidence. Predicate-bearing
  domains keep their `checks/contracts/writes.rs` discharge, and every
  evidenced restatement still compiles: the pass canary above,
  `pass/memory/bump_allocator_canary`, and the `Quantity`/`Indexed`
  fixtures, whose instances arrive from a declared return type, a cast,
  or a declared field. `fail/contracts/
  proof_fact_indexed_domain_application_unevidenced` pins the refusal and
  is registered beside `_mismatch` in `canary_suite.rs` (one roster line,
  that file still under a live claim). Remaining on this route: a cast can
  still introduce a bodyless indexed instance (`as Extent in Granted &
  Resident<P, T>`), which `placed_access.md` does not list as an
  establishment route; refusing it needs the compiler-owned routed
  classification that the library declaration (`pub domain<P, T>
  Extent::Resident<P, T>;`, no `established by`) does not carry, so it is
  a spec/library question rather than an implementation gap. Next
  acceptance: a `Vec<T>`-style container over the chain, which still
  needs compiler-owned `Initialize`/placed-view establishment (plan
  evaluation of `P` over `T`, Stable-supply admission).

- **ADDRESS-TRANSLATION-CANARY.** Continue Cathedral's page-table hierarchy,
  backing, policy, installation, and teardown in Omega source. Existing numeric
  page-walk validation grants no mapping authority. Acceptance: QEMU installs
  and tears down Cathedral-owned mappings with explicit Extent and TLB custody.
  Checked-semantics slice landed at 61d2b4664c:
  `tests/omega/pass/memory/address_translation_canary` drives a
  `TranslationAuthority` boundary contract through owned fixed placement
  (`map` -> `Pending`, `activate` -> `Installed`, `unmap` -> reusable
  `Granted` custody plus a linear `Shootdown` debt discharged only through
  `Shootdown::discharge`), with fail canaries pinning source-custody
  consumption, `Installed`-before-`unmap`, shootdown scope loss, and the
  carrier's opaque construction (macOS arm64,
  `OMEGA_PASS_CANARY_FILTER=address_translation_canary` /
  `OMEGA_FAIL_CANARY_FILTER=translation_` under
  `mbx nextest run -p compiler --test canary_suite`). Provider-visible
  obligation sets landed at 55d2790d01: `MappingGrant`, `PendingMap`,
  `MappedExtent`, `PendingUnmap`, and `MappingReceiptContext` in
  `psi/foundation/extents` each expose the exact install/release fact sets
  a provider's receipt must establish (Linux x86_64,
  `cargo test -p extents`). Borrowed-source custody landed at
  6fd003a9bb (Linux x86_64): a linear `BorrowedMapping` carrier plus
  three new fail canaries. The source-spelled obligation surface
  landed at e02ab0990f: authored install/release obligation sets now
  ride on the translation carriers. Next surface: a Cathedral package
  carrying real page-table installation.

- **EXCEPTION-ROOTS-AND-TIMER.** Materialize all fatal exception entries,
  dedicated critical stacks, IDT installation, and a minimal timer root whose
  hard handler only acknowledges, records, and wakes ordinary work. Acceptance:
  QEMU reports timer ticks over owned output and halts between ticks.
  Landed at eb96f07190: `external-roots` `interrupt_table.rs` accounts one
  descriptor table's complete declared member set — fatal exception entries on
  their own dedicated critical stack classes plus acknowledged interrupts such
  as the timer — over the installed-root ledger. Admission replays retained

  installed-root records, requires interrupt-return exit, the exact declared
  stack class, and the obligation's acknowledgement shape, and retains each
  member's linear handle so entries cannot retire while the table holds them.
  Publication is a separate edge requiring the complete declared set, an
  established table value naming exactly those rows on this installed
  realization, and a receipt for the exact issued carrier; refusal returns the
  established value for retry and issued identities cannot be replayed.
  Landed at 1cdd7e5ac9: the checked `lidt` provider edge is the sole
  `InterruptTablePublicationReceipt` minting boundary — it replays the
  exercised consumer authority's bound identity, installed-realization scope,
  and both scope legs (processor table control plus table publication), then
  replays the declared 10-byte pseudo-descriptor operand against the exact
  established destination before minting; the answer accounts the operand
  read, the `r10` scratch clobber, and the installed descriptor-table
  register state on a published answer. Landed at b0debb7ae2: each
  `InterruptTableMemberPlan` carries the consumer's declared gate
  descriptor (code selector, gate kind, entry privilege, IST slot), the
  complete ledger derives the checked post-handoff writer that resolves
  each member's sealed entry target into the gate's three offset fragments
  over a staged image carrying only declared constant fields, and
  `InterruptTableLedger::validate_written_descriptor_table` replays the
  produced bytes against that writer and installed realization, checks
  selector/IST/attribute/reserved-zero fields and zero fill, joins each IST
  slot through the installed TSS to its declared critical stack class, and
  only then mints the established table (macOS ARM64, 202 `external-roots`
  library tests). Remaining acceptance: the timer device source and the
  QEMU tick/halt surface, which are Cathedral-owned package code over the
  installed-root and table custody above, not compiler types.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Finish unresolved-requirement fences for
  component contracts and the final carrier-owned invocation route. Concrete
  reach and conservative bounds remain separate; selected provider execution
  and token era, not row equality, authorize invocation. Selected rows now
  reject a realization that still retains an unresolved installation-bound
  requirement (`provider-planning` `plans.rs`); the component-contract fence
  waits on the `COMPONENT-SUBSTRATE` carrier.

## Parallel language and compiler lanes

- **MATCH-SELECTIVE-LOWERING.** Complete the [value-dispatch
  contract](wiki/spec/language/patterns.md) for owned/nonnumeric results with
  parameter/projected/borrowed/linear custody, structural/case/domain patterns
  and coverage. Owned call arms and record fields that move existing affine
  children still need their exact residual transport. `5c986466a2` admits
  candidate sources interleaved with other live affine owners: edge arguments
  bind the join frontier positionally and the shared cleanup roster splices
  residual and pass-through parameters per row in establishment order
  (`pass/expressions/owned_match_interleaved_values`, macOS). `06b6da61d6`
  carries that same roster across authored state joins: each ordinary
  successor partitions residual custody per edge through the shared source-run
  splice, transfer sources resolve rebound frontier places, and the checker's
  state-exit locals accept the receipt-backed destination
  (`pass/expressions/owned_match_authored_state`, macOS). `517e86d465` moves
  projected affine children through owned match arms: a field or fixed-index
  chain on a local record or a call's structural product becomes a
  path-bearing owned argument, the exact residual complement dies on the
  selected edge, and the Terminal interpreter replays the verifier's split
  contract so the root carrier leaves storage only after every semantic path
  is discharged (`pass/expressions/owned_match_projected_field`, macOS).
  `9f8c787780` admits whole owned affine state parameters as match sources:
  the checker establishes each source at state entry with its authored
  parameter position as the source ordinal and orders parameters after
  statement locals, lowering resolves the physical structural parameter places
  and splices residual parameter custody through the same positional
  selection-edge roster, and selected parameters stay out of unconditional
  return drops (`pass/expressions/owned_match_parameter_values`, macOS).
  `8f65df406e` projects affine children out of a prior selection's join
  result: the chained source arrives on a join block parameter, both
  downstream contracts root partial-affine residual custody at the target
  block's declared parameter roster, and the chained fixture replays all four
  input combinations with mutation coverage on the residual path, join
  arguments, and origin receipt (macOS). Remaining: borrowed and linear
  custody joins plus the borrowed-subject and operator-result obligations
  below; preserve exact origins and actual death edges. Owners:
  `validation/src/value_custody/expression_types/{match_dispatch,result_type}.rs`,
  checked scalar computation/result continuations, Terminal production and
  canonical package-review contract/index projection. Preserve a
  once-evaluated subject, ordered first match, branch-local execution and
  exact result owners; do not flatten conditional ownership into a
  statement-wide move roster.

  Indexed affine tag observation reaches checked trees without copying the
  element (macOS ARM64, `51dc05abf5`; `cargo nextest run -p
  typed-trees-to-checked-trees --lib --no-fail-fast -E
  'test(multiplicity::borrowed_case_payloads)'`). Retain the once-captured
  borrowed subject and exact source-state loan closure when producing/replaying
  successor plans; checker acceptance is not Terminal lifecycle publication.
  The `dutch_flag` native command above still stops at the missing transitive
  `Main::main` plan; preserve its exit-70 oracle and explicit copyable swap values.

  Selected-operator and semantic-domain result types need full instantiated
  identity; input predicates are not arithmetic result facts. Indexed
  predicate/theorem and package membership applications need exact static
  arguments. Qualified callable-entry signatures, predicate/routed membership
  and erasure require real transport rather than payload-only projection.
  **OPERATOR-MACHINE-SUPPLY** owns declared operator execution;
  **CRASH-CONTRACT** owns crash-qualified equality; numeric landing is in
  **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: `checked-trees-to-lowered-psi --test value_dispatch`,
  `omega-native-differential-test --test scalar_case_results`, corresponding
  checker/interpreter controls, and the
  [float Match native customer](omega-rust/omega/compiler/compiler/float_realization.md#operation-and-control-custody)
  preserve effects, skipped trapping arms, overlapping patterns, complete
  coverage and independent replay. Keep `match_anonymous_result_landing` and
  `numeric_operand_destinations` as source probes. Wrong qualifications,
  ownership, selected result types and incompatible arms must reject before an
  outer bare-carrier cast; source validity alone is not native completion.

- **OPERATOR-MACHINE-SUPPLY.** Implement the
  [machine token-binding and executable-supply contract](wiki/spec/language/expressions.md#executable-supply)
  across Psi parsing, resolution, checked body construction, evaluation,
  lowering, Terminal codec/verifier/interpreter, and native call realization.
  Accept an optional fixed token after `machine`; remove the separate operator
  declaration introducer rather than retaining two permanent source forms.
  Ordinary direct declarations own checked machine bodies. Reuse ordinary body,
  state, contract, and call machinery instead of adding an operator evaluator
  or searching for a unique satisfier. Bodyless requirements retain explicit
  trait/provider supply; exact compiler primitives retain automatic catalog
  supply and proof/runtime eligibility. Migrate core/library/canary declarations,
  including tokenless compiler primitives to ordinary named machines and
  tokenless boundary requirements to the existing required-body form. Preserve
  exact semantic identities or reject stale schema artifacts explicitly; do
  not match a compiler primitive by leaf name or legacy declaration kind.

  Enforce closed-family semantic-home ownership and owner-local duplicate checks.
  At c797755f12 `tests/omega/pass/expressions/declared_operator_match_result/main.omg`
  authors the test-owned `Wrapped` type with
  `machine + Wrapped::add(&Wrapped, &Wrapped) -> u64` owning its body in place
  of the legacy `u8::sum` declaration-plus-satisfier pair; the interpreter leg
  `declared_operator_match_result_canary_interprets_both_arms` returns 260
  in the selected true arm, 1 in the false arm without invoking the
  operator, and 260 through the named call. Operands are borrowed because
  by-value data operands inside a match arm still stop at the owned-match
  custody join (MATCH-SELECTIVE-LOWERING), and the fixture stays
  checked-only until native admission of borrowed local data arguments to a
  free machine lands. Keep the selected-call join in
  `typed-trees-to-checked-trees/src/values/scalar/computations.rs`,
  `computations/integers.rs`, and
  `checked-trees-to-lowered-psi/src/scalar_graph/scalar_source_custody` compositional.

  Acceptance: wrapped 250 + 10 yields 260u64 in the selected true Match arm,
  the false arm yields 1 without invoking the operator, and independent Terminal
  replay and native execution agree. Cover token and named calls, generic and
  stateful bodies, once-only ordered operands, private helpers behind a public
  declaration, qualifiers and ordinary contract rejection. Missing body,
  bodyless-plus-satisfier, foreign primitive-family injection, duplicate owner
  shapes, and forged compiler primitive identity reject. Unrelated imports
  cannot change selection or cause a collision. Trait conformance selection,
  target-default/overridden float provider execution, and canonical compiler
  float-meaning evaluation retain their separate supply routes. Unsupported
  migration/execution paths must fail closed, not fall back to builtin arithmetic.

  Front slice landed (linw1, parser admission only): `parse_machine` accepts an
  optional closed-vocabulary token after `machine`, records it as
  `syntax_trees::item::Machine::spelling`, and rejects tokens on `satisfies`
  realizations, `boundary requirement`, and conformance members
  (`tokens-to-syntax-trees/src/declarations/machines.rs`, `declarations/parse_declaration.rs`,
  `declarations/conformance.rs`; tests in
  `tests/properties_and_requirements.rs`). The token now reaches every
  declaration representation: symbol-resolved, typed, and checked machines
  carry `spelling: Option<OperatorSpelling>` (`None` is the tokenless named
  form) and snapshots print it, and symbol resolution rejects two direct
  machines binding the same token to the same normalized operand shape
  under one owner at the second declaration
  (`syntax-trees-to-symbol-resolved-trees/src/lowering/machine/token_bindings.rs`;
  distinct shapes overload, distinct owners and tokens never collide). A
  token-bearing machine still lowers and executes exactly like its named
  form. At 735d4638bf/648f10c57e use sites select `machine + Owner::name`
  declarations by operand type through a typed operator-signature view
  under the machine's own symbol (`TypedTreeRoots::machine_token_bindings`,
  `lower_token_binding_view`), and resolution rejects a binding whose
  operand tuple omits its semantic home (the attached data, or any
  declared type/domain for a free machine) before the duplicate check
  (`expressions/token_bound_machine_{operand_selection,
  duplicate_shape_rejected,foreign_family_rejected}`). At
  0002df3b6a/c4af6429cc the checked stage binds every resolved binary use
  of a token-bearing machine to an ordinary compiler-synthesized call on
  that declaration's entry state (`operators/token_bound_machine_calls.rs`;
  finalization settles the `Operator` occurrence to the machine symbol),
  `[]`/`[..]`/match-equality selections reject fail-closed, and the pass
  canary executes through the checked interpreter returning 260
  (`token_bound_machine_operand_selection_exit_canary_interprets`). It
  stays checked-only because the native route rejects borrowed (macOS
  receiver bridge) and by-value (Unit-plan admission) local data
  arguments to a free machine independently of token supply, and
  build-time evaluation does not run the binding (a token use inside a
  build machine fails closed). A mixed `Wrapped + u64` operand with only a
  `(Wrapped, Wrapped)` binding reports a builtin overflow obligation
  instead of "no operator" because
  `value_custody/expression_types/operator_validation.rs` asks the
  receiver-only spelling query. At ad9966541f the package-review callable
  identity (`capture/semantics/conformances/policy_callables.rs`) wraps the
  overload coordinate as `token-bound(...)` for a token-bearing machine
  only, so a token-only revision reviews as differing policy and as
  decision-requiring acceptance rows for token-bearing admission claims,
  while tokenless identities stay byte-identical (`package-evidence`
  `fixed_token_binding_joins_the_callable_identity_and_only_when_present`,
  `package-manager` `token_binding_revision`); the Psi overload identity
  stays token-blind because a token never distinguishes named overloads.
  Still open there: the token in the review projection rows
  (`CheckedPackageCallableReview`) and in trait `StateSignature`
  identities, which need record/encoding extensions. Next frontier: cross-package closed-family
  semantic-home ownership (the current check is owner-local within one
  program; the unqualified operand-tuple home needs typing), then `operator` introducer
  removal, supply-mode wiring, Terminal codec, and native call realization
  per the acceptance above. Retirement map (inventory at 36e670e3d0):
  349 `operator` declarations (81 token-bearing), all bodyless because
  `parse_operator` requires `;`. By class: boundary requirements with a
  token respell now, because at d3f038f029 a top-level bodyless
  `boundary machine <token> Owner::name(...);` lowers to the same
  resolved boundary-operator slot the introducer produces
  (`lowering/operator.rs::lower_token_bearing_boundary_signature`;
  machine-only clauses such as `reaches` reject on it), so `satisfies`,
  provider selection, evidence and Terminal lowering are unchanged and
  `providers/checked_fixed_operator_dispatch_exit` passes identically in
  that spelling (2676523303); at 16d5b516d4 every token-bearing boundary
  declaration in `tests/omega` except two and all 22 in
  `core/float_operations.omg` and `core/slice.omg` spell
  `boundary machine <token> Owner::name(...);` with identical check and
  harness outcomes and a normalized std review policy byte-identical
  modulo core's content digest (`evidence/tests/operators/fixed_token_checked_adapters.rs`
  pins that both spellings project identically); a bodyless machine head
  parses a fact-free `crashes Cause;` at b3658b99b9 so
  `fail/operators/selected_crash_invocation` spells the slot too;
  `fail/generics/authored_const_operator_requires_selection` waits on
  `has_no_authored_spelling` in
  `preparation/generic_data/const_evaluation/anonymous.rs` ignoring a
  token-bearing `Item::Machine`; `core/nat.omg`'s
  `Nat::{subtract,less_or_equal}` satisfier pairs are ordinary
  proof-number operators with no catalog identity whose bodied migration
  is design-blocked on the `proof-operator-requires-formation` owner
  question (proof-to-proof calls are exempt from the `requires` prover,
  so the formation-time premise the operator prover enforced would be
  lost); the four `IndexAlgebra::plus` satisfier pairs in `generics/` are
  the PDI3 open-index operation-contract slot, supplied today by an
  implicit unique-satisfier search over a bare `u64` tuple with no
  semantic home, and are design-blocked on the
  `open-index-operation-selection` owner question; tokenless boundary requirements (155
  library, 18 tests) wait on the named `boundary requirement`
  provider/interpreter/Terminal route (TOP-LEVEL-BOUNDARY-REQUIREMENTS):
  at 79a9a2d084/8920cffc24 an intrinsic (or other `via`) satisfier of a
  top-level requirement no longer restates the requirement's contract
  (`conformance/machine_conformance.rs`, mirroring the operator
  satisfier rule) and a direct call to a requirement whose selected plan
  is not a checked adapter fails closed naming the plan
  (`boundary_dispatch.rs::reject_unselected_direct_requirement_calls`),
  but the 126 `F32/F64/I*/U*::*` rows in `core/float_operations.omg`
  (78 float rows with `ensures`, 48 integer-conversion rows, 32 of them
  carrier-qualified) stay on the operator spelling because the
  named-float intrinsic execution bridge is operator-keyed end to end:
  `selected_dispatch/float_intrinsic/*` plans rewrites only from
  `facts.operators.named_uses`, realizations resolve by
  `OperatorDefinition`, review evidence
  (`capture/providers/application_realizations.rs`), D29 coverage
  (`checked-compilation-to-terminal-artifact/application_coverage`) and
  the native proposal (`checked_boundary_operator_scope`) key intrinsic
  rows on operator symbols, and a rewritten requirement call would leave
  its `FlowCallFact` live; migrating them needs an intrinsic requirement
  signature view shared by operators and top-level requirements,
  plan-commitment stamping for direct requirement calls, flow-fact
  retirement, requirement-keyed coverage/proposal/evidence rows, and the
  7 `named_float_rewrites.rs` harness tests moved to requirement-side
  evidence (a user package cannot witness the shape: only toolchain
  custody supplies catalog identity); and the representation inversion (`SpelledOperator` wrapping machine
  signatures directly) needs the provider-planning, build-time
  `selected_operators.rs`, evidence `capture/callables/boundary_operators.rs`
  and result-domain overload dispatch owners (the named-requirement
  execution route now exists for the public receiver-free shape, see
  TOP-LEVEL-BOUNDARY-REQUIREMENTS); tokenless compiler
  primitives (68 library, 28 tests) need the catalog keyed on exact
  declaration/signature identity: at c1fe789968/88939ee7d6/f01d11ded7
  the primitive form is a bare bodyless tokenless `machine` signature
  admitted only by exact declaration custody (Toolchain-origin
  `float_operations.omg` plus a catalog path, lowered to the sealed
  projection declaration), `Float::meaning32/64` in core use it with the
  float harness legs identical, and a user-package lookalike rejects
  ("merely naming a declaration `Float::meaning32` grants no primitive",
  `fail/float/float_meaning_lookalike_grants_no_primitive`); at
  1cd2e7d1ed/d23557f541/2b3882430d the 65 `FloatSemantics::*` definitions
  are bare catalog signatures selected by exact normalized callable
  identity (`numerics/src/float_semantics_catalog.rs`, signature-keyed
  rows so the `from_integer` carriers never collide, identity-only with
  the discharge binding as FLOAT-PROVIDERS' extension point) under the
  same custody rule, with typed shape validation rejecting drift and the
  float harness, std check and std policy (modulo core's digest)
  identical; the lookalike control
  `fail/float/float_semantics_lookalike_grants_no_primitive` is
  rostered. At da974222c6/10cc6e4b62/e9cc9afdcc `Nat::Descending` is a
  bare catalog signature in `core/nat.omg` with a declaration custody row
  in `language-semantics` (`RankingViewId::catalog_declaration`), the
  checker keys `-> View` on the catalog identity and consults every
  declaration at that path so a user lookalike never becomes the builtin
  (`fail/termination/nat_descending_lookalike_grants_no_primitive`), and
  the termination harness, std check and rosters are identical
  before/after; `Nat::BoundedDistance`, `Slice::Length` and
  `Nat::IncreasingTo` have no core declaration and stay spelling-only
  builtins. The corpus surface fixtures (`operators/*surface*`,
  `*overload_signature*`) still author tokenless `operator`; bodyless domain-family and
  carrier-qualified semantic declarations (5 library, 26 tests) migrate
  to declaration-owned bodies with their relational `ensures`, since the
  supply table admits no other nonboundary supply (at c74c9adcbf a
  token-bearing machine attached to a domain, `machine +
  Quantity::Additive::add`, homes in the domain's carrier, gives the
  domain its denotation role, selects as a `DomainPending` family
  candidate under the existing binding-site law and binds its own body
  when selected; the 17 `pass|fail/domains` fixtures and
  `fail/operators/duplicate_spelling_binding` author that form, with two
  expected fragments reworded; bindings whose owner is a compiler-owned
  carrier cannot take the machine form as authored, so at 929e3d5700 the
  five `arithmetic/authored_*`, `field_singleton_authored_equality` and
  `collections/authored_*` fixtures bind their token through a
  fixture-local domain on the compared operand with every fragment kept,
  and at de44f6a2a6 `std/units.omg`'s five `Quantity` bindings are bodied
  `machine <token> Quantity::name` declarations whose bodies compute the
  carrier arithmetic re-qualified with the result unit, with
  generic-domain bindings homing through domain-qualified operands and
  indexed arguments distinguishing operand shapes (d376002425;
  `runtime_std_units_exit` interprets to 70 through the bodies);
  `termination/computed_measure_authored_operator` still authors
  `operator`); the `IndexAlgebra::plus`
  satisfier pairs (4) wait on the unqualified operand-tuple home typing;
  `[]`/`[..]`/comparison positions still reject fail-closed at body
  supply.

- **MODULE-NAMESPACE-RESOLUTION.** Finish the
  [module/name contract](wiki/spec/language/modules.md) in
  `syntax-trees-to-symbol-resolved-trees/src/module_normalization.rs`,
  `build-time-evaluation/src/const_initializers.rs`, and the shared generic
  evaluator. Remaining forms include aggregate-producing initializer expressions,
  open-template indices, specialized module templates,
  foreign/generic constant attachments, trait defaults, operator homes and
  qualified case membership in declared-domain proof facts. Preserve exact lexical/package selection before
  evaluation, per-use exposure under specialization and owner-local imports.
  The [source pipeline map](omega-rust/psi/pipeline/README.md#resolution-and-closed-instance-normalization)
  owns the current probes.
  Witnessed regression (2026-09-16, macOS ARM64, 34cc842d85):
  `resolve_signature_free_requirement` in
  `syntax-trees-to-symbol-resolved-trees/src/selection/signature_free_requirements.rs`
  collects every same-named trait program-wide and filters only by
  resolution stratum, so after 5d134569b6 seeded `core` into hosted
  package-aware compilations a package's own `ExtentRootProvider` collides
  with `core`'s as `TraitNotUnique`
  (`package-evidence` `declared_hardware_service_reach_does_not_infer_physical_authority`).
  Scope the candidates to the occurrence's module/dependency scope, as
  `select_visible_trait_definition` already does for machines.

  Complete specialized generic/provider applications, aggregate-producing,
  constrained/target-dependent and floating/NaN declaration evaluation, including
  unused initializers.
  Extend concrete failure discharge beyond the ordinary scalar invocation route
  in `build-time-evaluation/src/const_initializers/invocations.rs`: builtin
  comparison and logical actuals now transport through saved-entry provenance
  (`typed-trees-to-checked-trees/src/facts/crash_entry_values.rs`), so concrete
  probes decide them from checked evidence rather than widening to Truth. Casts,
  indexed reads, call-produced and other unprovable origins still widen
  conservatively and need their own checked evidence, not successful
  interpretation or provider-body inspection. Concrete
  `requires` discharge remains fenced by `admission/closure_validation.rs`.
  Floating identities need determined bits. Preserve the source-free
  `machine_initializers::` module/index
  and scalar-composition checks in `compiler --test module_machine_indices`.
  `const_generic_expressions/value/match_dispatch.rs` still needs nonconstant
  divisor integrality beyond singleton sign intervals, nonzero proofs beyond
  retained lattice gaps, and exact fractional-warning evidence for
  independent dispatch operands; extend rational bounds and correlations, not
  Cartesian arm enumeration or evaluation of skipped subjects. Preserve exact
  selected operators (**OPERATOR-MACHINE-SUPPLY**) and proof arguments.

  General array-value execution remains a dependency, not namespace fallback:
  dynamic/nonliteral selectors, borrowed projections/slices, array-producing
  cycles, structural block parameters, boundary results and indirect
  arguments/results need real payload/storage and view evidence. Extend the
  shared evaluation sequence in the
  [Terminal production map](omega-rust/psi/compiler/terminal-production/README.md)
  and `lowering/control_flow/aggregate_results.rs`; selected aggregate homes
  cannot stand for unevaluated values. Empty values retain carrier/dimensions;
  a known slice length cannot erase view formation or bounds. Share state/call
  transport with **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: `compiler --test module_machine_indices` (`nominal::` and
  `value_dispatch`), `terminal-psi-to-abstract-operations --test scalar_array_construction`,
  and native `scalar_array_results` exercise exact source selection through
  independent artifacts and matching-host execution. Preserve qualified
  declarations/constants, `match_constant_indices`, `nominal_constant_bodies`
  and `module_array_constant_indices` customers. Foreign exposure follows
  [file-local imports](wiki/spec/language/modules.md#import-scope-and-exposure):
  broad imports expose directly declared public domains only; narrow imports
  select exact declarations. Same-leaf competitors, private/transitive exposure,
  invalid unused initializers and unproved indexing reject. Keep the
  `runtime_aggregate_index`/`runtime_fixed_array_index` negative controls until
  their actual materialization obligations are met.

- **RUNTIME-VALUE-GENERICS.** Implement the settled
  [runtime-capable versus const binder contract](wiki/spec/language/generics.md#value-binders-and-const-requirements)
  for APIs whose result or stored qualification depends on an input value.
  Begin with scalar value binders and fixed-representation data/machine uses,
  not dynamic stack layouts or automatic SIMD specialization. Psi syntax,
  resolution, generic substitution, checked facts and Terminal production own
  the new path; calling/layout and artifact consumers retain the same exact
  runtime subjects alongside static applications. Keep the existing constant
  evaluator's runtime-input rejection for `const` binders. Module-owned forms
  depend on exact lexical selection from MODULE-NAMESPACE-RESOLUTION, not a
  source-spelling fallback or treating a runtime value as a static cache key.

  Acceptance: `<Count: u32>` accepts static and runtime arguments when the
  caller establishes its obligations; `<const Count: u32>` still requires a
  static specialization. One dynamic machine body handles distinct runtime
  counts without per-value code generation. Parameters/results and an indexed
  scalar field preserve the same captured subject, including after reassignment
  of its source variable. Equality-guarded uses retain their proof, and stale
  relationships, invalid bounds, duplicate/lost linear custody, and unsupported
  static-only uses reject. Reuse witnesses where valid; do not add heap boxing,
  reserve a range's maximum on the stack, or interpret integer finiteness as a
  specialization request. Check source diagnostics, representation, and
  interpreter/native replay for the supported slice before widening it.

  Squalr's 16/32/64-byte comparers and const-rotation bridges additionally motivate
  FINITE-GENERIC-DISPATCH below. The initial runtime-binder slice does not depend
  on completing that dynamic-interface work or general reflection.

  Resume evidence (8c0d4dbb45, Linux x86-64, debug `omega --check` on a
  std-free probe): `machine Main::prefix_count<Count: u32>(&self, base: u32)
  -> u32 { Count }` rejects in the parser with "expected `satisfies`, found
  punctuation `>`", because `tokens-to-syntax-trees/src/parameters/parse_generic_parameters.rs`
  reads `<Name: X ...>` only as the `T: Subject satisfies Carrier`
  conformance-binder form; the same program under `<const Count: u32>` with a
  static `<3>` argument compiles. Every representation carries one value-binder
  kind, `TypeParameterKind::Const { type_reference }` (syntax, symbol-resolved
  and typed trees), and conformance, callable-shape and review-evidence
  signature comparisons key on that variant, so a parser-only spelling that
  reuses `Const` would silently strengthen the public binder to `const`.
  First slice landed: `TypeParameterKind::Value { type_reference }` runs
  through syntax, symbol-resolved and typed trees; machine signature generics
  parse `<Count: u32>` into it while `<const Count: u32>` keeps the static
  const binder. Static arguments (literals, const declarations, and a caller's
  forwarded `Value`/`const` binder) specialize through the existing const
  path; canonical template identity encodes the kind distinctly (`V`, tag 5),
  review-evidence codecs carry `Value` as tag 4 in both public and review
  signature records, and conformance/callable-kind matching pairs `Value`
  only with `Value`. The monomorphization slice landed at `b81c62f6fcb`
  (macOS x86-64): a single-segment machine argument naming a caller local
  or parameter resolves against exact lexical scope (state parameters,
  then preceding let declarations, nearest match) in
  `syntax-trees-to-symbol-resolved-trees`, so authored StaticArgument
  selections finalize. Candidate/call-selection state carries
  `runtime_value_bindings` beside the static tuple; the specialization
  stays keyed on the static carrier, gains a trailing ordinary parameter
  per runtime-bound `Value` slot in telescope order, and each call site
  appends the caller's exact subject as a regular argument, including
  forwarded subjects through nested generic callers. Static applications
  keep const substitution and share one template with runtime
  applications; `const` binders still reject runtime inputs; a
  runtime-bound slot in a static type/layout/index/contract/const
  position rejects explicitly. Gates: 27 monomorphization tests (6 new
  `runtime_value_tests`), 469 symbol-resolution/typed-lowering tests,
  `tests/omega/pass/generics/value_generic_runtime_argument` plus the
  `const_generic_runtime_argument` and
  `value_generic_runtime_static_bound` rejections. Reuse the Terminal
  artifact replay coverage in `compiler/tests/runtime_value_generics.rs`
  for captured subjects, reassigned sources, guarded proofs, indexed scalar
  fields, and shared structural-subject bodies. Native leg (macOS ARM64,
  2026-09-17): that file's `native` module compiles each scenario as a
  `macos_arm64` application through the reviewed std package route and
  checks the process exit code: shared dynamic body (exit 11), forwarded and
  reassigned requirement subjects (12), guard-established requirement inside
  a transition state (3), subject flowing through a literal-indexed receiver
  field (38), and subject forwarded across cloned state transitions (7).
  Receivers spell `console: Service<Console> in Bound` with an explicit
  provider selection: a bare `console: Console` field stops at
  `image-emission/src/hosted_receiver.rs:600` ("macOS hosted receiver bridge
  lost exact contract, storage, or entry custody") whenever the entry
  retains its receiver (states or attached fields), which is
  OWNER_QUESTIONS.md question 1, not a value-generic gap. Three scenarios do
  not execute natively yet; each non-generic control stops identically, so
  none is value-generic specific. (1) A receiver method whose realized
  subject owes a `requires` contract (`Main::at<Count: u8>(&self) requires
  Count <= 7`) stops in target lowering at
  `abstract-operations-to-target-operations/src/lowering/control_flow.rs:92`
  (`!function.entry_claims.is_empty()`, rendered
  `UnsupportedControlFlow(MachineId(1))`), exactly as a plain
  `Main::put(&mut self, c: u8, v: u8) requires c <= 7;` does; that path is
  under the WRITE-ONLY-BORROW claim, so the native test drops the contract
  and keeps the subject flow. (2) The suite's reassigned source (`let mut
  source` in `Main::main` beside the Console receiver) stops at
  `target-operations-to-selected-instructions/src/legalization/source/scalar_graph/terminator.rs:86`
  (`Selection(Legalization(SourceCustodyMismatch))`), the known
  receiver-plus-mutable-primitive-local limit; the native test keeps the
  source in a helper machine. (3) A structural subject (`take<t>()` over a
  record local) beside a Console receiver leaves `Main::main` without a
  checked Unit plan (`local construction stopped at statement sequence:
  local data: scalar call binding, statement 1`; marker at
  `typed-trees-to-checked-trees/src/execution/unit/control/statement_sequence.rs:562`,
  declined downstream in the call planner); the same program without the
  Console receiver compiles and runs natively (exit 0), and a non-generic
  `read(t: Token)` stops identically, so this is STATE-LOCAL-VALUE-FRONTIER
  territory (record-local call operands beside a provider receiver). Next
  acceptance: contract-bearing receiver methods and the structural subject
  observed natively once (1) and (3) land, then module-owned forms.

- **STRUCTURAL-GENERIC-MATCHING.** Implement
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference),
  and [canonical ranges](wiki/spec/language/generics.md#canonical-integer-range-matching)
  for bounded containers deriving static backing from a declared length type.
  Psi parser/type-role resolution, generic-data substitution, machine inference,
  canonical type identity, and checked branch facts own the route; static
  evaluation/layout and artifact readers must use the same normalizer. Inference in
  `typed-trees-to-checked-trees/src/monomorphization/range_arguments.rs` must extend
  to named computations, owner-sensitive typed operations and open symbolic
  endpoints through shared semantic evaluation; do not use the i64 compatibility
  interval evaluator as canonical type identity.
  Keep `tests/omega/pass/generics/declared_range_endpoint_inference/main.omg`
  as the checked/Terminal call regression; the source pipeline map retains its CLI command.
  Closed free and type-qualified endpoints (`u64[0..=limit()]`
  and `u64[0..=Limits::capacity(256)]`) fold by resolved machine identity
  before checking in `build-time-evaluation/src/range_endpoints.rs` through the
  shared admission plan. Resolved calls admit closed integer arguments into
  exact builtin integer parameters with closed range refinements, preserving
  carrier checks, selection authority and fractional warnings. Nested calls and surrounding
  integer arithmetic retain exact result carriers and original call selections;
  failed evaluation restores temporary substitutions. Input/result range checks
  use exact concrete values; computed signature bounds are invocation dependencies.
  Named endpoint calls also admit declared argument-free integer domains on
  parameters and results by proving the concrete value through the shared
  domain-fact evaluator (`range_endpoints/integer_type.rs`,
  `const_domain_facts/membership.rs`; canaries
  `generics/declared_range_endpoint_domain_qualified_calls` and
  `fail/generics/declared_range_endpoint_domain_argument_rejected`, not yet
  in the harness roster); authored `requires` clauses and nested-callee
  premises keep the closure fence. Typed operators in endpoint arguments
  and narrow carriers already fold. Boolean literals, `&&`/`||` and
  `bool`-returning helpers fold in endpoint-call argument position through
  the shared scalar evaluator at a49ab393fe
  (`generics/declared_range_endpoint_boolean_arguments`,
  `fail/generics/declared_range_endpoint_boolean_result_rejected`); the
  bound position, bound arithmetic and signature bounds keep the integer
  carrier, and comparisons/negation in Boolean arguments stay outside.
  Fully supplied static generic applications fold at e081667381
  (`u64[0..=identity<256>()]`, const-declaration and Boolean arguments)
  through the prepared program's existing type-position specialization;
  partial applications reject with the monomorphizer's "cannot be derived"
  and inference-needing calls stay outside
  (`generics/declared_range_endpoint_static_applications`,
  `fail/generics/declared_range_endpoint_partial_static_application_rejected`).
  Named endpoint evaluation runs in rounds at 29443ddc65: a round
  prepares the execution program from the current working tree and folds
  what closes, and the driver re-prepares only when a static application
  failed after progress, so templates whose own signature bounds carry
  endpoint calls close under explicit application
  (`generics/declared_range_endpoint_template_bound_calls`; controls
  `fail/generics/declared_range_endpoint_template_bound_{out_of_range,
  unclosable_rejected}`, the latter a parameter-dependent bound no round
  can close). A round without progress restores every published fold.
  Nominal/policy qualifications, trait-operator owners, applications with
  type/machine/evidence binders, and open symbolic endpoints remain. Omitted
  trailing data binders recover from data-level `where Binder == <range
  shell>` equations against the supplied argument's retained
  `IntegerRangeNormalization` (`preparation/generic_data/equations.rs`;
  exclusive ends bind the proof-integer successor; verified equations are
  discharged from the instance; `generics/omitted_data_binder_*` canaries
  pin the selection and each distinct rejection). Still open there:
  recovering a type binder from a range-shell equation (rejects asking for
  an explicit argument), omitted-binder applications nested inside other
  templates, and runtime `Value` binders. Several child-owning arguments to one
  generic no longer panic ("arena span append must be contiguous"):
  `lowering/type_reference.rs` and `lowering/domain.rs` lower every argument or
  constraint before placing it, so each span reaches the arena as one
  contiguous run (`tests/generic_range_arguments.rs`, macOS ARM64). One defect
  remains a stage earlier: `tokens-to-syntax-trees` appends each generic
  argument handle to the shared table as it parses, so a nested application's
  arguments interleave with the enclosing list and the authored spans overlap
  (`Pair<u64[0..=3], Pair<u64[0..=7], u64[0..=15]>>` hands the outer
  application the nested shell as its second argument). Collect the argument
  handles and insert the span once, as the domain argument pack already does.
  Record and case-payload endpoint calls resolve in
  their declaration scope before folding; local bounded record construction and
  field reads retain range obligations through canonical Terminal execution.
  Generic-data range arguments retain structured interval observations from
  `build-time-evaluation/src/range_arguments.rs` and independently replay equality
  after complete typing; substitution retains the original constrained argument.
  Finish open/template-dependent and machine-computed argument ranges, type
  equations and omitted data binders without a source-display identity key.
  Connect borrowed-local mutation calls and transfer-bearing state-transition storage joins;
  `declared_range_inference_local_effects_retain_pending_terminal_boundaries`
  retains the nongeneric borrowed-call reproduction of a missing checked scalar
  control plan. Direct local scalar-field writes compose with copies and moves
  through canonical Terminal execution in `owned_scalar_graphs/record_stores.rs`.
  Bounded leaf stores still need exact range obligations; native owned/local
  field stores still need their physical storage realization.
  Fresh record locals, unrestricted whole/nested copies and affine local moves
  retain guard/argument reads through the scalar graph's ordered statements.
  Copies bind independent block homes; moves retire their source and preserve
  child provenance through wrappers. Selected-edge cleanup normalizes final live
  local homes in declaration order and disposes them in reverse order, after
  selected reads. Parameter-origin moves into locals still need the formal/local
  custody join; supporting an untouched affine formal beside local moves does
  not close that transfer. Reuse ordinary owned edges and the Unit record emitter
  without aliasing copy storage or dropping a moved source.
  Cyclic record establishment still reaches Terminal's `ControlCycle` admission
  fence in `terminal-verifier/src/validation/control_flow/unranked_cycles.rs`;
  close repeated establishment, per-iteration disposal and independent cycle
  replay before admitting it. `owned_scalar_graphs/record_locals.rs` retains that
  boundary beside executing acyclic ownership and evaluation-order controls.
  Ordinary calls and whole local copies/moves already share the storage route;
  do not invent generic-specific plans.

  Extend remaining named computations through build-time admission, using
  `typed-trees/src/typed_trees/type_system/closed_numeric.rs` and its existing
  identity/substitution consumers for the resulting endpoints. Preserve
  authored endpoint landing, exact binder identity and proof-integer exclusive
  normalization; do not introduce another arithmetic evaluator or infer layout
  from flow bounds. The source
  pipeline map records the maintained scalar-inference customer and remaining
  native record boundary. Native scalar field reads still need local record-root
  custody and bounded leaf identity retained through abstract operations, layout,
  instruction selection and independent replay. The primitive-read lowerer in
  `terminal-psi-to-abstract-operations/src/lowering/machine/operation/primitive_storage.rs`
  admits primitive locals or eligible structural parameters, not arbitrary local
  record roots; projected parameter paths are no longer a blanket rejection.
  Extend native coverage to the fixture's `generic_forwarded_bound` and
  `field_bound` calls through its real hosted entry; the existing scalar-only
  entry does not establish local record-field execution. Keep the local
  copy/move/store and bounded-field controls in `owned_scalar_graphs` while
  closing that path.

  Acceptance: TinyBytes' `Length == u64[0..=Capacity]` binds omitted Capacity from
  its supplied type before layout; inclusive/exclusive equivalent intervals
  select identical static capacity without runtime arithmetic overflow. Primitive
  equality and its static branches check all admitted alternatives. Repeat and
  explicit binder conflicts, absent/ambiguous endpoints, occurs cycles, and
  type/value-kind mismatch reject. Extend the existing declared range-only call
  inference while keeping explicit larger compatible bounds distinct from exact
  type equations. Local flow narrowing cannot alter inferred
  layout; arbitrary domain predicates do not collapse nominal identity. Preserve
  const staging, initialization, stack supply, and artifact replay. Runtime endpoint
  applications depend on RUNTIME-VALUE-GENERICS; static matching can proceed first.

- **FINITE-GENERIC-DISPATCH.** Implement the
  [finite specialization contract](wiki/spec/language/generics.md#finite-specialization-boundary)
  and [dynamic method families](wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
  for Squalr-style scanner widths and runtime-selected datatype providers. Psi
  extracts exact tuple rosters from explicit OR/equality constraints, checks each
  body and membership edge, and retains requirement/index/result correspondence.
  Terminal dynamic-call/descriptor owners and Omega native table/replay consumers
  retain the same family rows and ordinary costs. Begin with one scalar value
  binder, a common concrete result, and dispatch around a region-sized operation;
  runtime arguments depend on RUNTIME-VALUE-GENERICS, not generic JIT execution.

  Resume evidence (macw4-finite-dispatch-3, macOS x86-64, 71
  selected-dispatch tests): the roster slice landed at 61b0e258b0 —
  `signature_families.rs` extracts the explicit `Binder == literal || ...`
  roster (order/duplicates normalize, correlated `&&` alternatives keep
  authored groupings, partial tuples/opaque predicates/ranges never
  enumerate), `adapter_rows.rs` publishes one tuple-keyed row per declared
  tuple, statement and expression calls select exactly one settled tuple,
  and the checked interpreter replays by canonical tuple. The row-evidence
  slice then required each roster tuple to be filled by exactly one bare
  value-tuple specialization of the selected provider's own template:
  missing coverage leaves the whole requirement dynamically ineligible,
  and wrong-width, shape-substituted (type/machine/conformance argument
  coordinates or closed conformance applications), duplicated, and
  sibling-provider records all reject rather than lending rows.
  The `Value`-binder requirement slice landed at db52146ce1: trait
  requirement signatures admit `<Width: u32>` through a dedicated parser
  mode (`const` stays static-only, conformance binders stay off
  requirement signatures), and conformance signature matching slices a
  specialized instance's trailing realized `Value` subjects against the
  template's declared carriers so a runtime-bound provider body still
  satisfies its requirement without becoming roster evidence. A runtime
  boundary argument and a roster filled only by carrier-keyed records both
  still reject. The Terminal half landed (macOS ARM64): requirement slots and
  direct/indirect/stored dispatch rows in
  `terminal_module/boundary/dynamic_dispatch.rs` carry `family_tuple` (the
  producer's canonical const identities in binder declaration order, empty
  for a nongeneric requirement), and `ClosedConformanceRow` table rows in
  `terminal_module/boundary/conformances.rs` carry the same tuple inside the
  application commitment (domain `v4`) and report fingerprint;
  `terminal-codec` encodes both under format marker 100 with round-trip,
  byte-identity, and previous-layout rejection tests in
  `dynamic_dispatch_wire.rs` and `closed_conformance_wire.rs`;
  `terminal-verifier` rejoins every direct/indirect/stored dispatch,
  descriptor-parameter slot, rebound row pair, and static requirement
  dispatch to the table row by exact tuple (`tests/dynamic_dispatch.rs`);
  and `checked-trees-to-lowered-psi` fills every table and dispatch row from
  one `checked_requirement_family_tuple` source, rejecting a requirement that
  declares local binders rather than lowering an empty-tuple row. Remaining
  open slice: Omega native table replay of tuple rows
  (`terminal-psi-to-abstract-operations`, `abstract-operations`, and
  `image-emission` replay still rejoin rows by the two identities alone), and
  a Psi producer admitting a finite family into the local `dyn` surface (the
  `selected-dispatch` roster settles only boundary adapter dispatch, which
  that surface excludes), so a nonempty tuple is producible only by tests;
  boundary calls never demand provider specializations, so every roster
  tuple needs one static call site until a dynamic selection generates the
  complete family.

  Acceptance: widths 16/32/64 need no handwritten suffix-method family; source
  alternative order and duplicates normalize deterministically. One selected
  conformance covers every required tuple; missing, wrong-width, mixed-provider,
  or shape-substituted rows reject. Const-only calls still reject dynamic inputs
  without a checked bridge. Short explicit tuple sets preserve correlations and
  do not enumerate arbitrary ranges; target-ineligible bodies and invented
  fallbacks reject. Preserve parameter effects, index identity, and once-only
  moves/cleanup through selection and replay. Escaping variable-shaped results
  require an explicit sum or eligible owned/borrowed descriptor, not implicit
  allocation. Retain compile/code-size evidence and scan-loop dispatch placement
  before claiming an improvement over explicit branches. No new reflection API
  or arbitrary generic virtual method is needed for this slice.

- **DOMAIN-ISSUER-ROUTES.** Implement the
  [requirement and exact-machine routes](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
  and [private issuer catalogs](wiki/spec/resources/authority.md#private-issuer-routes)
  for public qualifications issued by owner-selected validators without an
  artificial trait. Psi route normalization, checked membership, and Terminal
  qualification evidence own target-kind/identity and exact result introduction;
  package source-selection capture owns the limited private-route metadata
  exception. Module-owned targets depend on MODULE-NAMESPACE-RESOLUTION; do not
  weaken its existing rejection fences to accept same-spelled identities.

  Acceptance: exact free/attached issuer calls establish only their authorized
  result provenance after carrier/predicate/custody checks; public ordinary
  requirements still permit valid downstream conformers. Public domains may name
  author-accessible private machines/requirements, and public wrappers forward
  issued values without becoming issuers. Reject ambiguous target kinds/overloads,
  self-justifying membership, forged result/route evidence, direct-call borrowing
  of admitted requirement authority, and outside private call/conformance access.
  Artifact/package replay retains exact private issuer identity and dependencies
  without exposing private types or hiding admissions. Resource capacity and
  classification-specific boundary-route restrictions remain unchanged.

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Finish consumer permission rows and
  exact target-mechanism classification under the settled [filesystem
  control/lifecycle
  policy](wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority).
  Acceptance: every admitted leaf has one exact mechanism/contract row;
  unknowns and duplicates reject; exercised classes fit independently supplied
  service permissions. Explicit empties retain service reach and exact review
  identity. Retire the transitional broad `Filesystem` summary only after
  exact replacement closes. Generic close need not be supported to admit a
  separately proved constrained occurrence; do not fabricate a broad union to
  complete the table. Slice landed at 2212af0bb4 on Linux x86-64: the three
  ordinary-release cohorts emit one evidence-bound explicit-empty mechanism
  row for the direct-syscall mechanism carrying a retained
  `FilesystemOrdinaryReleaseContract` in its checked argument-contract
  coordinate, and the review admits one constrained close leaf with empty
  exercised and permitted classes while the unconstrained sibling under the
  same syscall number stays unclassified. A normalized foreign mechanism now
  carries an argument-contract coordinate beside its admitted calling plan
  (`effects::NormalizedForeignArgumentContract`): the admitted-plan key keeps
  its published bytes, and a checked coordinate binds the same retained
  release contract, so a constrained `kernel32!CloseHandle` import occurrence
  earns the evidence-bound empty row under review while the unconstrained
  import of the same symbol stays unclassified
  (`terminal_authority_review/tests.rs::
  foreign_release_occurrence_review_binds_the_retained_record`, macOS ARM64).
  The settlement wiring landed at a56b3b6265:
  `native-realization/src/native_product/realization.rs` captures the verified
  filesystem replay record, calls
  `filesystem_native_handle_query_release_contracts`, derives
  `filesystem_release_occurrence_mechanism_rows` and merges them into the
  terminal-authority policy, and `settlements/source_imports.rs` now takes
  those contracts as a parameter, so the earlier "nothing hands a compile's
  retained build filesystem replay record" sentence is stale. Remaining: a
  witness that a real compile earns the constrained row end to end, after
  which the broad `Filesystem` summary retires.

- **FILESYSTEM-RELEASE-CONTRACT.** Implement bounded occurrence-specific
  open/query/close evidence through checked flow and native realization replay.
  Prove the exact object/argument contract,
  handle/alias preservation through intervening calls, one applicable release,
  and no later use; authority classes alone are not preservation evidence.
  Acceptance: constrained ordinary close has one evidence-bound empty row;
  failed acquisition, escape, stale/substituted proof, invalidating calls,
  reused aliases, and attached deferred deletion prevent narrowing. External
  pending-deletion completion alone leaves ordinary close empty. Keep this
  bounded proof separate from general owned-handle design; no owner-policy
  blocker remains.

  Progress: the checked-interpreter leg adds the bounded occurrence record
  `FilesystemSourceNativeHandleQueryChainReplayRecord` and the
  `NativeHandleQueryChain` source-input event in
  `psi/semantics/checked-interpreter/src/filesystem_replay` (module
  `native_query_chains.rs`, tests `native_query_chain_tests.rs`). One
  constrained `open_path_handle` under access-zero/full-share/OPEN_EXISTING/
  BACKUP_SEMANTICS-without-DELETE_ON_CLOSE, exact `Resolved` handle
  preservation through admitted `final_path_name_by_handle` and
  `get_last_error` observations, one nonzero `close_handle` retiring the
  identity; acquisition failure, aliased/duplicated outputs, substituted or
  cross-domain inputs, early retirement, missing/ambiguous/late release, and
  lane tampering all reject. `build-evaluation` rehydrates the tag-28 chain
  (`replay_record/rehydration.rs`, `replay_eligibility.rs`) and
  `native-realization` derives one release contract per retained occurrence
  and binds it into a direct-syscall key or a normalized-foreign checked
  coordinate (`terminal_authority_policy/filesystem.rs`); the reviewer admits
  the constrained close under either role. Next leg: the settlement wiring
  named under **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**, so an ordinary compile
  hands its retained record into the settled mechanisms.

  Native acceptance also needs the checked transitive machine plan missing
  from `filesystem/windows_canonicalize_exit`: Terminal production currently
  refuses its attached Unit closure in
  `checked-trees-to-lowered-psi/src/unit/attached_unit/call_closure.rs`. Its source
  dependencies include nested structural sum construction/transport
  (`UnitResult::Error` carries `ErrorKind`) and whole nominal receiver
  replacement (`self.unit_result = ...`). Resolve these through shared
  state/value planning before expecting this fixture to emit; a primitive
  field store or a fresh call-result home cannot substitute receiver replacement.

  Pause further isolated native prerequisite milestones for this fixture.
  Resume execution work with a plan covering its actual transitive closure:
  nested construction and extraction, borrowed case observation, and whole
  nominal replacement, including match-produced field assignments. Reuse
  existing recursive layouts and referent identity; do not erase structural
  operand custody or replace borrowed storage with snapshots. This is an
  implementation scope pause, not a language-design blocker.

- **R5.** Finish exact inferred may-write summaries and relational candidates
  for unresolved receivers, boundary-result origins, conditional helper-body
  case refinement, mutable case-state transfer, graph-level aggregate result
  routes, type-generic carrier substitution,
  computed reference arguments outside proven helper-result relations, and
  other unsupported expression shapes.
  Prefer shared fixpoint and alias reasoning over syntax-shape exceptions.
  Acceptance: all supported finite source shapes converge without widening
  permissions, and unsupported recursion fails explicitly. Since
  a3b3ecd8c4 `parameter_may_carry_write` is `type_may_carry_write &&
  !type_is_reference_free_value` (`write_frames/type_capabilities.rs`):
  a state parameter whose type contains no exclusive reference anywhere
  in its transitive structure and reaches no opaque boundary data is the
  state's own storage and no longer a write-capable cycle root
  (ownership.md, "Borrows and aliases"), so copy enums such as
  `KeywordKind`, `RootItem` and `DataItem` crossing cycle edges let the
  parser and harness components permute and solve;
  `omega --check source/psi/gates/parser/main.omg` went from a 3340.6 s
  kill to 174.4 s, with cli_mvp and nqueens frames byte-identical.
  `type_may_carry_write` itself still counts every `Named` local or
  reference referent as write-capable, and the walk is unmemoized (one
  bounded traversal per query). The equation builder also hands statement
  calls a fresh memo while the prefix walk shares one; unifying them
  changes the solve-versus-walk route for a cyclic state calling a cached
  callee that calls back, so it stays separate.

- **TPR6.** Finish subject-bearing progress-premise normalization through
  exported bodies, provider plans, recursive calls, and artifact evidence.
  Private ranking witnesses stay outside public identity. Acceptance: every
  used premise is reconstructed for the exact subject and no qualification or
  similarly shaped row mints one implicitly.

  Complete owned value loads through references, additional reference-boundary
  loads, indexed or replaced carriers, and reference-bearing helper results
  with unresolved control-flow or binding transfers in
  `checks/termination/progress/{origins.rs,lineage.rs}`. Captured record and
  array constructors now arrive from the selected field or element operand
  (`flow/value_origins.rs`); owned helper results such as
  `context.scheduler = pick(replacement)` now derive the checked callee's
  exact frozen-input projection through the same backward trace
  (`checks/termination/progress/origins.rs`), while `&mut` or mutable
  captures, generic or dispatched callees, and unresolved result routes stay
  unproven. A mutated aggregate
  cannot use root correspondence as evidence for its previous field values;
  a may-write frame cannot identify a replacement value. Reuse
  `lineage/places.rs` partition replay for per-field arrivals through reference
  and unresolved generic leaves with exact declared-field provenance; retain
  opaque prefixes where that provenance is absent. Acceptance: those finite
  projected arrivals and checked helper correspondences derive the replacement
  input's exact premise, while unknown writes and reference aliases without
  exact provenance retain no checked guarantee. Slice landed at da11319b0e on
  Linux x86-64: a checked helper result whose returned expression is itself a
  checked call now recurses through the nested callee's transition-free
  prefix with the same gates
  (`checks/termination/progress/origins.rs`), so per-field arrivals through
  the recursive-proof leaf derive the replacement input's exact frozen
  projection, while mutable inputs, generic or dispatched callees, and
  unresolved result routes keep no checked guarantee.

  Realize projected nested value-call operands guarded by
  `validation/src/machine_calls/calls/expression_scanning/result_realization.rs`
  through the checked/lowered value planning path. Borrow checking can
  transfer owned helper-result projections, but full checking still rejects
  the inner call's result as an unrealized operand. Complete result
  projections through the shared evaluator and result-binding lookup; extend
  the shared closure to general structural-result callees. Carry loans,
  qualifications, and projected claims through structural results without
  erasing their obligations. Acceptance: `select(forward_outer(outer).inner)`
  and `select(forward_array(values)[0])` evaluate each call once, retain the
  inner result home through projection and the outer call, and preserve every
  selected source loan and linear claim. Remove the nested-call gate only when
  those result uses have real producers; a correct declared type or source
  origin alone does not realize a value. Resume at the checked/Terminal
  representation seam, not another evaluator source-shape gate: realize
  projected owned reference leaves with residual carrier cleanup, then nested
  result operands with their recursive loan custody. Whole owned record
  ingress and forwarding are available: at `5374ab3198`, `cargo nextest run -p
  checked-trees-to-lowered-psi --test reference_result_source --no-fail-fast`
  (macOS ARM64, `RUST_MIN_STACK=33554432`) checks canonical encoding,
  independent verification, and fuel-stepped execution of `forward(input)`
  followed by `replace(held.body)`, preserving the transferred leaf and
  restoring the caller's original backing after cleanup. `select(value: View)
  -> &mut i32 { value.body }` must move the selected permission and dispose
  the remainder, not create a reborrow whose parent dies at return. This is
  Terminal acceptance, not native acceptance. Reuse
  `validation/src/machine_calls/reference_result_custody.rs` for ordinary
  completion and independent source replay; preserve conservative lifetime
  unions when extending exact runtime origins beyond the current whole-record
  route. Transfer existing permissions through owned call/edge/result moves
  and residual cleanup; `EstablishReference` creates a child loan and cannot
  substitute for moving an existing leaf. Keep carrier location distinct from
  loan occurrence/parent, relocate runtime descriptors without copying
  referents, and reject nested reference host interfaces until their custody
  exists. Reuse existing typed projection/result maps; structural-element
  array construction remains a further dependency beyond the record route. The
  full-checking rejection controls in
  `typed-trees-to-checked-trees/src/tests/borrow/carrier_results.rs` cover
  nested `select(forward_outer(...).inner)` and
  `select(forward_array(...)[0])` calls, not the simpler `make_view` binding.
  Keep those controls until their corresponding unchanged source cases execute
  from encoded Terminal evidence, including projected moves and array ingress.

- **NOMINAL-FIELD-FLOW.** Complete declared-field domain evidence in Psi
  semantic facts, flow transfer, and contract consumption. Collection elements
  need explicit live coverage for their declared field predicates, transported
  through indexing, views, copies, and calls. Mutable calls must preserve or
  establish the appropriate returned field facts; an unchanged nominal type
  annotation cannot restore evidence retired by a write. Acceptance: the
  dungeon's `RoomLookup`, `MazeBuilder`, and game-state calls satisfy default
  field obligations, while corrupted elements and stale aliased fields reject
  at calls, transitions, and returns. Do not encode universal coverage as an
  unresolved index or assume arbitrary incoming storage is zero-initialized.
  Unknown scalar parameters and locals now bound conversion results by their
  primitive carrier (`values/bounds/sources.rs`; regression
  `unknown_scalar_inputs_bound_conversion_results_by_their_carrier`, Linux
  `cargo nextest run -p typed-trees-to-checked-trees`). Unselected call
  arguments now bound captured scalar return ranges by their declared formals
  — immutable formals by the declared type, mutable formals by the lent
  place's live snapshot first — retained pure scalar operands keep Unit calls
  nested inside a captured callee transparent to stored values, and exact and
  trapping cast bounds meet the operand interval with the proved spelling
  range and destination carrier (`flow/transfers/scalar_values`,
  `values/bounds.rs`; regressions
  `mutable_formal_call_results_read_the_lent_places_incoming_value` and
  `effectful_nested_call_arguments_keep_the_return_bounds_live`, Linux `cargo
  nextest run -p typed-trees-to-checked-trees`). Ensured call-result bounds
  now propagate through bound names — `let i: u64 = idx()`, `i = idx()`, and
  `self.slot = idx()` seed the callee's literal `result` bounds on the bound
  name's label, retired by reassignment like any label-keyed bound
  (`checks/ranges/statements/aliases.rs`; regression
  `call_result_alias_carries_the_ensured_result_bounds`, macOS arm64 `mbx
  nextest run -p typed-trees-to-checked-trees`). Unknown-length slice indexes
  now consult the same contract: a call's ensured inclusive high meets the
  collection's `minimum_length`/`exact_length` floor like a folded literal —
  directly for `s[idx()]`, through bound names' label-keyed bounds,
  guard-seeded `i < K` bounds, inclusive and exclusive range ends, and `len -
  idx()` subtrahends — while an ensured `>= 0` conjunct supplies the signed
  lower half (`checks/ranges/proofs.rs`, `indexes/validation/lower_bounds.rs`,
  `facts/proofs.rs`; regressions
  `call_index_on_unknown_slice_meets_ensured_bounds_against_length_facts`,
  `unknown_slice_index_meets_label_upper_bounds_against_length_facts`,
  `unknown_slice_index_meets_guard_seeded_upper_bounds_against_length_facts`,
  `length_difference_offset_reads_ensured_result_bounds`, macOS arm64 `mbx
  nextest run -p typed-trees-to-checked-trees`). Statement transports keep one
  context per exact storage coordinate, so a view element write retires that
  element's facts and not its siblings' (`flow/transfers.rs`; regressions
  `view_literal_index_write_keeps_sibling_element_coverage` and
  `view_element_corruption_retires_only_that_element`). A machine's normal
  return is now a default-domain consumption point for its readable `&mut`
  referents, `self` included, and for a readable reference return's returned
  place: the exit re-proves the same `StateParameterDomain` and
  `MachineFieldDomain` rows the self-transition arrival check re-proves,
  establishment-gated domains (`established by ..`) excluded
  (`checks/contracts/exits/result_domains.rs`, exits recorded in
  `proof/contracts/calls.rs`; regressions
  `tests/omega/fail/dependent/{mutable_referent_alias_corruption_return_rejected,reference_return_alias_corruption_rejected}`,
  `tests/omega/pass/dependent/mutable_referent_alias_corruption_restored_compile`),
  and `flow/call_phases/referents.rs` hands those rows back after
  `apply_call_invalidations` on each readable `&mut` actual's exact storage
  and on a `&mut self` receiver, while `flow/transfers` establishes a write
  through a local `&mut` alias on the aliased storage
  (`AssignmentWriteTarget::Storage`). The dungeon probe (`omega --check
  --target linux_x86_64 samples/cli/games/dungeon_crawler_cli/main.omg`, macOS
  ARM64, 6c3a89f196 after the sample acknowledged its blocking game loop)
  reports 962 diagnostics against 1731 at dd06a082c2 (2891 with the return
  point alone): 113 `Dungeon::use_event` and 4x112 `MazeBuilder::*` return
  rows plus 3x112 `clear_level`/`carve_room`/`room_mut` call rows over
  `level.rooms[*]`, 14+14+7 `RoomLookup`/`append_exit`/`apply_room` rows, 4
  index rows; `cargo nextest run -p typed-trees-to-checked-trees` 4127 passed
  with the same 3 failures as 6ef27bae4f, and the pass corpus fails the same
  212 fixtures as before. An unknown call frame now retires only the
  declared-signature ceiling -- each `&mut`/`&write` actual's exact storage
  and a `&mut self` receiver, empty for a builtin function -- and only an
  unrepresentable ceiling (a by-value argument that may carry a reference, an
  exclusive actual through a reference local with no single origin) still
  retires every live fact (`flow/call_phases/ceiling.rs`;
  `tests/contracts/call_ceilings.rs`), and the write-frame isolation walk
  treats the `UInt`/`Int` atoms as values so `RandomState { calls: UInt }`
  locals have a frame
  (`validation/write_frames/{isolation,type_capabilities}.rs`). The probe
  reports 503 diagnostics (738 with the atom repair alone): 3x112
  `MazeBuilder::{carve_room,connect,force_quiet_room}` return rows and 112
  `room_mut` call rows over `level.rooms[*]`, 14+14+14+7
  `find_room_mut`/`find_room`/`append_exit`/`apply_room` rows, 4 index rows, 2
  `roll_event`/`clear_event` rows; ttct 4135 passed with the same 3 failures,
  validation 843 with its 1 known failure, pass corpus the same 212. A `&mut`
  local bound from a checked reference result now carries the callee's finite
  candidate origins over the caller's actuals
  (`flow/reference_places/result_candidates.rs`: every entry-state exit
  returning an exclusive-parameter projection through field/case/literal index
  segments, so `level.room_mut(cell)` names `level.rooms[0..15]`; sub-state
  routes, runtime indexes and unresolved callee locals keep the conservative
  treatment). `rebase_local_write_places` consults them, so the unknown-frame
  ceiling (now `flow/mutation/ceiling.rs` inside `call_mutated_places`) and an
  unclassified alias write retire exactly the candidates, the referents
  hand-back and an alias write re-establish a candidate's declared rows only
  where they were live before, and an alias closure that cannot enumerate
  local origins no longer turns a frame unknown
  (`tests/contracts/call_ceilings.rs`). The probe reports 55 diagnostics: 14
  `RoomLookup::find_room_mut` return rows and 14 `find_room` call rows
  (runtime-indexed copies through a sub-state loop), 14 `append_exit` + 7
  `apply_room` + 2 `roll_event`/`clear_event` call rows whose nominal-input
  check still proves through one exact origin, 4 index rows; ttct 4149 passed
  with the same 3 failures, validation 851 with its 1 known failure, pass
  corpus the same 212. Resume order: (1) the call-side nominal-input and
  `reference_domains` proofs
  (`checks/contracts/{nominal_inputs,reference_domains}.rs`,
  `local_reference_storage_at_call`) resolve an actual through one exact
  origin only; proving a row on every candidate would close the 23
  `append_exit`/`apply_room`/`roll_event`/`clear_event` call rows; (2)
  `RoomLookup::find_room{,_mut}` loop through a sub-state over a runtime
  index, which the candidate trace refuses (28 rows) -- sample side (4) or a
  candidate family for the element loop; (3) `&mut self` receivers hand back
  only the ZII-seeded `MachineFieldDomain` rows: a callee's `self` entry
  assumption is ZII-gated, so its return cannot guarantee non-ZII rows and a
  caller's non-ZII receiver facts survive a method call only through frame
  precision -- widening needs a contract decision, not a flow change; (4)
  `checks/ranges` retires a slice view's length after a call through one
  element (`clear_room(&mut rooms[0], ..)` then `rooms[1]`), 15 rows; (5)
  sample side: `RoomLookup` copies an element at a runtime index and passes an
  uninitialized readable `&mut Room` out-parameter where write-only `&write
  Room` is the intended spelling but validation rejects it for constrained
  records, and unbounded `room_count` leaves 4 index rows. Fallout on main at
  b13f81f6bb: thirteen `checked-trees-to-lowered-psi` unit tests are red from
  this tightening -- ten `tests::boundary_byte_buffers::*` (including
  `checked_provider::*`),
  `tests::attached_unit_cases::attached_unit_borrowed_self_roots_an_ordinary_field_argument_beside_pro..`,
  `tests::byte_sequence_write::guarded_mutable_byte_write_keeps_original_field_extent_and_tail`
  and
  `tests::byte_write_loop::byte_write_loop_empty_initialized_view_never_writes`.
  Bisected to 1544a206ec as the first bad commit (its parent 90cde5211e
  passes, macOS ARM64); that commit updated
  `tests/contracts/element_fields.rs`, `tests/range_byte_live_lengths.rs` and
  `tests/element_field_flow_probe.rs` but not these, whose machines return
  while holding readable `&mut` byte buffers and now owe the referent's
  declared field domains.

- **CML4.** Complete `EdgeCleanupPlan` after outgoing materialization and
  transfer commitment, including structural sums, nested projections, cycles,
  calls, and partial initialization. Cleanup follows reverse establishment and
  exact residual custody; trap/abort edges clean nothing. Acceptance: no affine
  occurrence disappears, duplicates, or is cleaned after transfer.
  Implement native boundary call-result residual cleanup, including result homes
  and projected copies;
  whole-result disposal does not cover a projected result's residuals.
  Extend anonymous projected helper-result operands to multiple producers and
  other effects within one consumer's argument list and non-Unit consumers,
  preserving each temporary's exact dying continuation. Extend native
  Terminal Jump residual cleanup beyond acyclic Unit fallthrough with ordinary
  direct-register results and parameter roots: computed scalar bindings, boundary-result
  projections, and cyclic control need their own storage and edge replay without
  delaying cleanup until final return. Extend entry-origin scalar continuation
  storage to operation-result values and their exact defining identities;
  per-call argument shuffle snapshots do not preserve a result across earlier
  calls.
  Extend the
  type-directed record/array complement in
  [ownership contract](wiki/spec/terminal-psi/ownership.md) to construction-local
  roots and mixed dying-root schedules, preserving maximal untouched subtrees,
  empty complements, and reverse establishment order without runtime liveness
  flags. Entry-parameter cleanup alone cannot dispose a temporary's remainder.

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

  Extend the ordinary producer/consumer join in
  `effects/structural_callback_reach` to extracted projections, freshly
  established returned claims and claims from distinct owned inputs, including
  mixed scalar/structural operands. Returning a whole fixed array with several
  indexed claims is the regression baseline in `projected.omg`;
  matching claim identities cannot substitute for checked content guarantees.

  Complete caller-specific saved-argument and result facts: nonliteral contract
  arithmetic, borrowed collection lengths, dependent/public-trait results and
  subslice bounds need exact entry observations and substitutions. Mutable
  formals/storage must distinguish their incoming value from subsequent writes.
  **CRASH-CONTRACT** shares the capture path; case-qualified, indexed, generic,
  reference-valued and floating entry predicates need exact identities/totality.
  Unchanged entry observations may justify published routes; later writes and
  current body facts may not. Ranked-loop crash guards require independently
  checked all-path invariants, never first-pass facts ignoring backedges.

  Finish [exact anonymous division/landing](wiki/language_guide/chapter_5_expressions_evaluation.md#exact-anonymous-division-and-landing)
  across generic/evidence-adapted and boundary calls, aggregate/parameter/constant
  destinations, numeric policies, floats and proof consumers. Preserve result
  carrier/policy custody, exact rational intermediates and warning origins
  through suppression/reporting; coordinate selected result types with
  **MATCH-SELECTIVE-LOWERING**. Acceptance: `7 / 2 * 2` is 7 with a warning,
  `7 / 2` cannot land in an integer, and typed integer division truncates.
  `(4097 / 4096) * 4096` is 4097 with a warning; its `4097u32` form is 4096.

  Complete [typed quotient/remainder](wiki/language_guide/chapter_5_expressions_evaluation.md#typed-integer-quotient-and-remainder)
  in resolution, selected constant execution and symbolic proof replay.
  `generic_data/const_evaluation/` and
  `build-time-evaluation/src/admission/selection_authority.rs` must retain
  authored selection across helper calls instead of folding builtin meaning.
  Use the `authored_const_operator_requires_selection` and
  `authored_const_call_operator_requires_selection` controls: next positive
  acceptance evaluates the selected zero-returning provider to `Buffer<0>`,
  while unrelated declarations leave builtin arithmetic unchanged.
  **OPERATOR-MACHINE-SUPPLY** owns executable supply. Nonconstant proof `Int`
  terms need independent evidence beyond source entailment in
  `validation/src/contract_entailment/arithmetic_judgment.rs`.
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

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Finish ordinary generic
  `drop<T>` and runtime cleanup invocation after exact owner-attached hook
  selection. Erased fields remain semantically present but never produce
  runtime cleanup. Acceptance: every path invokes the exact selected hook once
  or proves the value transferred/consumed.

- **EXTERNAL-ENTRY-STACK-EPOCHS.** Finish exact enter/body/exit stack epochs,
  context-specific provider dispositions, finite nesting, and installed-root
  binding. Acceptance: WCSU, stack leases, artifact entry, and runtime context
  independently rejoin; unresolved or cross-context dispositions reject.

- **TR3-TR8.** Finish whole-call-graph worst-case stack derivation, exact
  `StackPlan`, nonmoving `StackLease`, suspension/cancellation preservation,
  transactional arguments, park/resume lowering, and the suspension-safe loan
  subset. Bind an authoritative possibly-suspending crossing roster so coordinated
  deletion of both a Terminal site and plan cannot erase a required crossing.
  Acceptance: stack/control custody is never compiler-owned or lost across a
  suspension edge, and missing crossing demand rejects under the
  [call/outcome contract](wiki/spec/terminal-psi/calls_and_outcomes.md).
  Follow the [task runtime contract](wiki/spec/build/task_runtime.md) through
  `task-plans`, provider admission, and a real selected runtime. Acceptance also
  exercises concurrent start/park/resume/finish, rejection returning every moved
  argument and lease, cross-instance settlement rejection, and fresh storage
  eras on reuse. Static plans and lifecycle ledger tests alone do not establish
  executable activation or argument conservation.

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
  operations. No structural or effectful observer crosses the quotient unless
  its law is explicit and checked. Custody-bearing quotients remain fenced.

  Validation composes the canonical correspondence row on the ordinary path
  and dispatches one bridged form from the composed certificate's evidence, so
  checking and source erasure cannot disagree about which row a request earns,
  and a selection whose representative or selected theorem has a transitive
  closure reaching an admitted or boundary machine rejects. Every request still
  rejects, and admitting one needs three things: the sealed `Quotient`
  namespace resolving in ordinary call and name validation (today
  `machine_calls/machine_parameters.rs` reports "supplies static machine
  arguments, but its generic callee did not resolve", the value-call path
  resolves nothing, and expression scanning reports an undeclared `Quotient`),
  a production caller of `install_non_executable_quotient_correspondences`,
  and quotient handling in `typed-trees-to-checked-trees`, which bails out of
  every value path carrying a quotient operation. Six rule sources wait on that
  resolution in `validation/tests/quotient_blocked_sources`, where they pin
  their rule from real source; the three the typing stage owns are corpus
  canaries.

  Remaining after that: a canonical wire payload for congruence-only
  `lift<F, Congruence>`, whose language-semantics, codec, verifier and review
  rows belong to **PROOF-CONTRACT-MIGRATION**; general adapted lift with
  result computation, beyond the omission, permutation, repetition and closed
  literals the direct rung covers; the conversion-independent closure over
  helper types and statements; and generic or private applications,
  preconditioned `define`, result aliases and forwarded result flow, none of
  which has a canonical row.

- **EVALUATED-FOREIGN-BINDINGS.** Replace string-backed import bootstrap with
  typed compile-time locator values for PE, versioned ELF, and Darwin/Mach-O.
  Carry normalized locator, evaluated plan, target applicability, and producer
  custody through provider selection and native emission. Raw foreign bytes are
  data, never Omega symbol names or ambient lookup authority.
  Resume: `ConstEvaluable` now admits synthesized closed const-generic data
  instances — closed application origin plus the substituted member walk — so
  a `via` producer's `Binding<O,S,V>` result crosses build-time evaluation as
  an ordinary typed value through
  `evaluate_const_evaluable_machine_symbol_for_invocation_measured` with the
  authored `via` span retained as invocation custody
  (`omega-rust/psi/semantics/build-time-evaluation/src/admission/const_evaluable.rs`,
  `omega-rust/omega/build/provider-planning/src/evaluated_via_bindings.rs`).
  The string-backed bootstrap is retired on the Omega side: no production
  code constructs `ProviderBinding::StringBackedImportBootstrap` (its payload
  is an empty `RetiredStringBackedImportBootstrap`), the calling-plan,
  trust-report, review-policy, and executable-scope carriers dropped their
  variants, provider derivation and review capture reject a typed
  `ExternalBindingIdentity::Import` with "declare a typed locator through an
  evaluated `via` binding producer", and review recovery reports the old tag
  as `RetiredVocabulary` without a version bump (no producible encoding
  changed). The Psi syntax `ExternalBinding::DllImport` variant, its
  snapshot row, its lowering arm, and the two `source_imports.rs` arms are
  gone. The review vocabulary rows that mirrored the typed identity are
  retired (macOS ARM64, 2026-09-17): `PackageReviewExternalBinding::Import`
  and `PackagePolicyExternalBinding::Import` no longer exist, capture
  projects a typed identity through one fallible
  `project_external_binding` that rejects `ExternalBindingIdentity::Import`
  with the evaluated-`via` diagnostic, the review-record and policy
  encoders no longer spell tag 0, and external-supply policy recovery
  reports tag 0 as `RetiredVocabulary` without a version bump
  (`packages/review/evidence/src/{capture/callables/external_supply.rs,
  record/signatures/external_{supply,policy}.rs,
  encoding/recovery/policy/external.rs}`). Remaining deletions, each
  blocked by a live claim on its owning crate at the time of the retirement:
  the uninhabited `ProviderBinding::StringBackedImportBootstrap` variant and
  its vacuous `match *retired {}` arms (effects, package-evidence,
  trust-model, provider-planning — the last claimed by
  TOP-LEVEL-BOUNDARY-REQUIREMENTS), whose last arm is
  `native-realization/src/native_realization/terminal_authority_review/reviewer.rs`
  (crate `src` claimed by WRITE-ONLY-BORROW); and
  `language_semantics::ExternalBindingIdentity::Import` with
  `ExternalBindingMechanism::Import` (crate claimed by
  OPERATOR-MACHINE-SUPPLY/ranking-catalog), whose remaining consumers are
  the typed-trees `ExternalBindingValueSnapshot::Import` snapshot arm, the
  provider-planning `reject_string_backed_import_identities` /
  `external_provider_binding` rejections, the package-evidence
  `project_external_binding` rejection, and the tests that pin those
  rejections. Open after that: the wider acceptance replay.

  Extend [normalized-import evidence](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement)
  from fixed-width scalar calls to a
  source-rooted flat-record argument, then ranked control and port-bearing
  artifacts. Acceptance: independent native replay preserves the exact
  survivor/physical-child bijection and rejects missing, duplicate, substituted,
  or role-swapped children. External realization claims require independently
  admitted concrete authority.

- **FLOAT-PROVIDERS.** Complete runtime Boolean/machine operations for exact
  `FloatMeaning`, kernel discharge, and remaining artifact-aware proof sources.
  Keep IEEE runtime comparison distinct from mathematical meaning equality;
  NaN payloads erase only in the meaning projection and signed zeros remain
  distinct there. Landed: terminal-interpreter boundary dispatch executes
  installed scalar-result providers through the shared suspended scalar frame
  (`call_operations.rs`/`call_frames.rs`, witnessed by
  `terminal_interpreter/scalar_provider_call_tests.rs`), and verifier
  provider-result conformance admits
  `(BoundaryMachineResult::Scalar, TerminalMachineResult::Scalar)` pairs with
  the structural arm's closure (`validation/foundation/provider_result.rs`),
  so codec representation validation and artifact admission accept scalar
  provider rows. Open: kernel discharge. The remaining artifact-aware proof
  sources are owner-blocked on `float-meaning-use-site-source-identity`
  (`OWNER_QUESTIONS.md`): Terminal carries the
  `DirectOperationResult`/`DirectCallResult` identities with codec tags 6/8 and
  independent verifier rejoins, but no checked source class can name either,
  since every checked class is signature-relative and the only artifact carrier
  that matches is the use site of a transported contract, which has no per-use
  canonical proof value. Measured at `a2c6676c37` (macOS ARM64): a caller of a
  float machine yields two `DirectMachineResult` projections and two equalities,
  no call-site row; `DirectOperationResult` has zero producers in the checked
  and lowered crates.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Restore ordinary native
  descriptor invocation and forwarding, beginning with a non-entry helper that
  receives one borrowed two-word descriptor, forwards it once, invokes a
  requirement, and uses its result across computations and branches. Target-only
  consolidation is paused: two composition milestones left this native customer
  unsupported. The missing dependency is an ordinary indirect-call operand and
  its ABI, clobber, effect, and reach contract, not another whole-body recognizer.
  Owners: abstract-to-target signature/graph lowering; legalized/selected call
  representations and target-to-selected replay; ISA selected encoding/decoding
  and post-allocation emission; native-artifact/image table and relocation custody.
  Acceptance: a source-rooted closed-conformance native differential fixture
  publishes and independently replays Linux x86-64/AArch64, executes on matching
  hosts, selects distinct table implementations at runtime, and preserves the
  original referent and surrounding calculations. Reject substituted instance,
  table, slot, ABI, access, and code-span custody. Delete the superseded Unit/scalar
  parameter recognizers with that closure; do not substitute raw function pointers
  or unproved devirtualization. Receiver-backed executable entry provisioning
  remains a separate dependency for the existing receiver-entry canary.

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

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Finish explicit public boundary
  requirement declarations, external satisfiers, provider selection, and
  installed execution/era replay. Remove transitional undifferentiated
  bodyless-machine modes once their source migrations close. At
  c30b2b5ed7/fe13696481/158f7989ef/9b12861ea1 a public, nongeneric,
  receiver-free `boundary requirement Owner::name(...);` with a checked
  `satisfies` adapter executes in both engines: selected-dispatch settles an
  owner-keyed direct-call row for the requirement entry
  (`boundary_dispatch/adapter_rows.rs`), execution settlement journals and
  redirects the direct value-position call (typed call, flow occurrence,
  argument facts) to the adapter entry and rebuilds Unit plans
  (`selected_dispatch/requirement_adapter.rs`), validation admits the direct
  call, and fail controls reject an unselected direct call at settlement and
  keep the fence on private requirements
  (`providers/checked_boundary_requirement_{dispatch,terminal}_exit`,
  `fail/providers/{boundary_requirement_direct_call_unselected,
  private_boundary_requirement_direct_call}`). At 2f7cbe7a12/b0b34b2883
  ordinary scalar-machine contracts carry exact `+`/`-`/`*` integer arithmetic
  over subjects and contextual literals as closed predicates
  (`values/scalar/result_contract.rs` `ContractPredicates::integer_term` onto
  Terminal's exact integer-math terms; wrapping carriers, bitwise or shift
  operators, division, and literal-only arithmetic stay unsupported),
  repairing the Terminal/native legs that 21fc627a6e's erasure-mode removal
  had broken for the contracted customer and the operator sibling
  `checked_boundary_operator_dispatch_exit`; both requirement fixtures are
  ACTIVE+ROOTED with the Terminal leg asserted through one helper, and the
  full-corpus pass leg drops from 222 to 221 failing fixtures with no other
  change. Remaining: statement-position direct calls, receiver-bearing
  requirements such as `Task::finish(self)` beyond installation-bound reach,
  and the tokenless boundary-operator respelling onto this route. Blocking
  note (macOS ARM64, origin/main ee634231e6): the architecture suite is red on
  main, so any landing that gates on the full suite aborts until it is green
  (the shared landing queue itself is unaffected). Three failures were stale
  pins after the countdown-region move and the release-contracts rename,
  repointed here. Two are genuine violations in this item's in-flight
  intrinsic-bridge work and are left for its owner:
  `stable_evidence_and_encoding_exclude_compiler_representations` rejects
  `packages/review/evidence/src/record/contracts/expressions/operator_policy.rs`
  for retaining the compiler representation `typed_trees::` in a stable record
  (added by 5641d2ca1e), and
  `checked_operator_provider_reports_retain_strong_plan_authority` rejects
  `build/selected-dispatch/src/selected_dispatch/float_intrinsic.rs` for not
  joining the strong plan commitment (added by bfbdce31b6, extended by
  2817c60bcc). Both need the owner's fix in their own sources; relaxing the
  architecture rules would be the wrong repair. A third failure,
  `countdown_region_replay_is_independent_of_loop_and_component_producers`,
  belongs to the countdown-induction work instead: a8df0d5139 deleted the
  162-line independent-reconstruction leaf
  `countdown_induction/replay/region.rs` (which carried `fn current_edges`,
  `fn reachable` and the boundary comparisons), replaced it with a 42-line
  custody-derived `region.rs`, and never updated the guardrail; it is being
  resolved on that item.

- **BUILD-ADMISSION-CHECKPOINT.** Execute an admitted build machine against one
  coherent frontend/source/authority snapshot and append generated source in a
  later resolution stratum. Authored source may not resolve forward into output
  generated by its own build. Acceptance includes replay after serialization
  and drift rejection for the full activation.

  Finish compiler-owned publication of the retained native product built with
  generated source. Bind the exact application root, authored declaration role,
  requested target, and source/build/generated/native inputs; validate final
  realization before publishing. Acceptance: serialized replay reproduces the
  product, and source, role, target, or artifact drift prevents publication.
  Join the [scoped build work](#scoped-build-execution): checkpoints and generated
  handoffs also bind dependency purpose and execution profile, and publication
  waits for every required output plus final product checking. Preserve the
  authored/generated boundary; no own-build final-component query or hidden
  post-compilation callback.

  Resume evidence (2026-09-17 UTC, macOS ARM64): serialized replay records
  carry the exact root package identity, authored declaration role, selected
  target profile, and admitted build execution profile
  (`build-evaluation/src/evidence/observations.rs` `BuildReplayActivation`,
  folded into the observation identity and the record version), and
  admission drift-rejects a replay captured under any other activation
  (`build_config_granted/checkpoints_and_snapshots.rs`
  `serialized_replay_record_rejects_activation_drift` and
  `serialized_replay_record_rejects_execution_profile_drift`). The bound
  profile is the request's admitted one
  (`CheckedCompileRequest::build_execution_profile`, the compiler host when
  the request names none), threaded from `CheckedChildExecution` into the
  filesystem scope; it is never inferred from the compiler process and stays
  distinct from the selected target. Remaining: dependency-purpose binding on
  checkpoints and generated handoffs waits on **BUILD-DEPENDENCY-PURPOSES**
  supplying a per-occurrence purpose value — `DependencyPurpose` exists only
  on dependency edges and the only compilation occurrence is the root
  product-purpose one, so there is no purpose fact to bind and no second
  occurrence to witness drift against. Compiler-owned publication of the
  retained native product with generated source, gated on every required
  output plus final product checking, is still open.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Finish the compiler/library migration
  to explicit ordinary std dependency edges. Std may be replaced, split, or
  absent; only core and compiler-injected vocabulary remain toolchain-owned.
  Migrate package-aware fixtures, keep freestanding UEFI roots dependency-free,
  and retain standalone compatibility only until fixtures acquire package
  roots. Replace std/alloc `Toolchain` classification when compiler consumers
  have exact source-byte catalog entries or explicit semantic bindings.

  Complete composed-Unit plans for trait-default, float, wire, arithmetic-helper,
  guarded-call, and looping-cast canaries and the target-correct non-Linux
  Console catalog entry. Structural writeback shares the blocker recorded in
  `WRITE-ONLY-BORROW`. Feed consumer-scoped Console, Filesystem, and UEFI
  bindings through normal package-aware compilation. Acceptance: removing a
  dependency rejects its imports/provider selections; name, alias, path, or
  same-spelled declarations cannot restore it, and stale or substituted
  semantic bindings reject without relying on accepted-lock replay.
  `repository_build_declarations.rs` pins that every wire, arithmetic, calls,
  traits, and float pass root declares exactly the std edge its sources
  import, with no standalone compatibility roster left.
  `standard_library_package_resolution.rs` pins that removing a Console
  consumer's std path dependency rejects it and that re-declaring std under
  another alias or path spelling does not restore the `omega_language_std`
  import. The rejection names the missing dependency edge rather than an
  unresolvable root-relative source path: the `omega_language_std` alias the
  import spelled, the `omega-language-std` package it names (bundled or in
  the closure), the importing `main.omg`, and the root
  `builder.depend(...)`/`depend_as("omega_language_std", ...)` declaration
  that would add the edge (witnessed by
  `removing_the_standard_library_dependency_rejects_the_console_consumer` on
  macOS ARM64). Remaining gap: the Console provider selection gets no
  diagnostic of its own because source assembly stops at the import (a
  selection without the import is not a checkable shape).

- **COMPONENT-SUBSTRATE.** Implement independently selected component closure
  while keeping deployment/update policy in runtime packages or Cathedral. Componentization must
  bind exact imports, exports, services, mappings, stack demand, leases, and
  installed provider closure under the
  [component publication contract](wiki/spec/build/component_publication.md).
  Until that carrier is complete, every
  `Independent` selection fails at one explicit fence.

  Expose the [verified-description consumer](wiki/spec/build/component_publication.md#verified-component-descriptions)
  from the existing verifier for independent admission/replacement and topology.
  First inventory actual producer/replay coverage; implement missing complete
  facts in their Psi, component, or provider owners, not a second topology census.
  Acceptance: an independent source-free consumer checks subject/profile/schema,
  all entries and outgoing authority, custody and inseparable assumptions; corrupt,
  omitted, early-frontier and forged-complete descriptions reject. Include startup,
  callbacks, timers, cleanup and retained providers. Reading descriptions grants no
  callable authority; installation-dependent facts remain obligations and require
  fresh per-occurrence resource/profile admission. First slice landed at
  8d75d9ddc4 on Linux x86-64: `component-candidate` now owns the canonical
  `ComponentDescription` carrier (schema 1, bounded and byte-replayed) and its
  producer `describe_component`/`describe_component_facts`, plus the
  source-free consumer `verify_component` returning the opaque evidence-only
  `VerifiedComponent`. The consumer re-decodes the embedded canonical
  artifact, reconstructs the component subject, replays the subject-bound
  proof-section decode, and re-derives every module-evident row: complete
  entry roster (canonical entry, suspension resumptions, and assumption-bound
  startup/callback/timer/cleanup/retained-provider entries), outgoing
  authority (boundary requirements, port-space writes, the selected entry's
  published service ceiling), custody constraints, the retained provider
  roster with strong plan digests, and the required installation
  obligations. Corrupt, wrong-subject, incompatible-schema, early-frontier,
  forged-complete, omitted, unaccepted-assumption, unsealed-provider, and
  missing-obligation descriptions all reject distinctly. The consumer lives
  in `component-candidate` rather than `terminal-verifier` because the latter
  was under a live `PROOF-CERTIFICATION-BRIDGE` claim this wave; module proof
  admission stays with the existing verifier's purpose-specific carriers.
  `provider-planning` still rejects every `Independent` selection at the
  explicit component-closure fence (`selection_provenance.rs`; pinned by
  `package_compilation_inputs/authority_and_build_files.rs::
  independent_provider_selection_reaches_the_componentization_fence`).
  Landed beside it (macOS ARM64): `derive_component_inventory` exports one
  row per retained checked provider realization
  (`export:requirement:{requirement}|{provider}|{machine}`, derived
  identically by producer and verifier so omitted or invented rows reject),
  and `VerifiedComponent::realizes_selected_plan(&ProviderPlan)` joins a
  selected plan against the verified module with distinct rejections for an
  empty provider type, schema drift, unchecked rows, missing, mismatched,
  duplicated, or unexported realizations. The description carrier, its
  producer facts, and `verify_component` now live in
  `omega-rust/omega/backend/artifacts/component-description` (dependencies:
  effects, semantic-vocabulary, terminal-codec, terminal-psi, sha2), below
  the runtime quarantine that `tests/architecture/layering.rs` keeps between
  the ordinary compiler and package closures and the runtime owners
  (`component_description_stays_below_the_runtime_quarantine` pins it);
  `component-candidate` keeps `describe_component(&ComponentCandidate)` and
  re-exports the API. `provider-planning` gains
  `selected_provider_plan_facts_with_independent_components`: every
  `Independent` plan must be realized by exactly one supplied
  `VerifiedComponent` whose join passes, and every supplied component must
  realize a plan; none, several, an unmatched extra, and each realization
  mismatch reject distinctly and never fall back to fused
  (`provider_planning/independent_components.rs`, 5 tests from a real
  source fixture and a canonical described-and-verified module). The
  3-argument `selected_provider_plan_facts` forwards an empty slice, so
  its callers still reject every `Independent` selection. Landed next
  (macOS ARM64, 264df70d4e..9418736601): `PackageCompilationTargetInputs`
  carries `IndependentComponentDescription` (dependency package, observed
  Terminal subject, canonical description bytes; root, foreign, and
  duplicate attachments reject), and
  `build-evaluation/src/provider_settlement` re-verifies each attached
  description under the build's profile (schema 1, expected subject, no
  accepted assumptions) before calling the four-argument entrance; a
  verification rejection names the dependency package and never falls back
  to fused (`tests/independent_component_settlement.rs`, 9 tests through
  `filter_target_machines` and `settle_checked_providers`).
  `component-description` exposes `test_support` (feature `test-support`)
  for consumer fixtures. Next slice, in order: the compiler builds a
  `ComponentCandidate` for a dependency compiled as its own component,
  describes it, and attaches `IndependentComponentDescription` to the
  root's target inputs (`compiler.rs` stops at `NativeArtifact`); then a
  build vocabulary for accepted assumption digests so mechanism-bearing
  components can be admitted (settlement accepts none today). The
  composition-mode admission failures witnessed beside the fence input
  (`independent_provider_selection_reaches_the_componentization_fence` and
  the two `provider_selection_rejects_*composition_mode*` tests) were
  0e6c25c4dc normalizing bare case values into constructor literals while
  `provider_selection_composition_mode` still read only a `Name`; closed at
  65d71153a9 under BOUNDARY-OPERATOR-FAMILY-SELECTION.

- **FFIVAL.** After the generic callback/runtime path closes, run the Windows
  `user32` boundary-coherence canary with no raw function pointer or Win32-only
  compiler escape.

- **WIRE-RUNTIME-AND-INSTALLATION.** Complete reusable artifact validation,
  consumed placement authority, W^X/coherence, physical invocation, and
  uninstall/replacement joins. Keep arbitrary runtime bytes-to-code, JIT, and
  raw executable addresses unsupported.

  Imported-image placement closed at `2a89f2e039` and `84b9582b78`: the final
  image carries placed executable and initialized-data inventories from every
  writer, Mach-O import lowering records each thunk and binding slot with a
  pairing replay, and `image-emission/src/installed_artifact.rs` rejects
  unclassified gaps, a truncated compiler prefix, and a resolver-claimed
  uninstalled thunk address (`source_evaluated_native_realization/macho_and_terminal_imports.rs`
  and `image-emission/tests/artifacts/installed_artifact.rs`, macOS ARM64).
  Remaining: consumed placement authority, W^X/coherence, physical invocation,
  and the uninstall/replacement joins over that custody; loader/provider
  lifetimes still bind only through installation records, not a live
  installed occurrence.

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

  Acceptance: the exact Omega source closure implements the complete language
  and production pipeline, passes the shared product suite, and publishes a
  deterministic manifest of every transitive compiler/build input. Bootstrap
  construction of that closure belongs in `TASKS_BOOTSTRAP.md`.
  Resume (macOS ARM64, 4bfa009246 on eb6f223ce5): `omega --check
  source/omega/main.omg` runs the complete Psi checked stage and the std
  calling-policy admission and stops on one diagnostic, `root slot
  alpha_bootstrap::ProgramEntry belongs to unknown target profile
  alpha_bootstrap` from `source/omega/build.omg:11` (601.8 s wall at
  dd8bb81386, 704.0 s at a3b3ecd8c4 beside a concurrent gate check);
  that row is OWNER_QUESTIONS 8 and no product-source edit is pending
  behind it. The stops before it are retired: the parser borrows the
  lexer's stream (b30c5ae693; the borrowed-storage owner-replacement
  conflict with `pass/ownership/move_keyword_field_assignment` is
  OWNER_QUESTIONS 9) and build-time member selections confine on their
  exact owner package (43ed6089a2). The generic `selected ProgramEntry
  establishment rejoins 0 Terminal attachment identities; expected one`
  stop was a regression from 8508aec01e (std `read_line`/`read_byte`
  gained `blocks;` while the Unit builder refused blocking boundary
  calls); ccfa48ddae admits them and `omega --check
  samples/cli/basics/cli_mvp/main.omg` compiles again in 248.2 s. Behind
  it the native leg of the cli_mvp library test stops in Terminal
  verification on `CallCrashContinuationUncovered { operation:
  OperationId(5), cause: Trap }` (a private body's inferred crash
  interface lowers to no crash routes,
  `checked-trees-to-lowered-psi/src/proofs/crash_routes.rs`) and, with
  `crashes Trap` authored on the std `read_line` adapter and the sample
  entry, on native lowering's
  `UnsupportedBoundaryCrashContract(BoundaryMachineId(2))`; both are
  CRASH-CONTRACT scope and neither declaration was kept. The parser gate
  (`source/psi/gates/parser/`; f6c762c501 harness reach/span fixes;
  5c40dd26b0 write-frame law, 3340.6 s kill to 174.4 s) completes the
  checked stage and still stops on the establishment rejoin, one Unit
  statement at a time: 587ae15689 gave its receiver a shape (a fixed
  array of a copy sum with payload cases, `TokenStream.tokens: [Token;
  16384]`, is a material array element; `pass/structs/runtime_copy_sum_array_receiver_exit`
  exits 70 in the interpreter and establishes its entry), and
  4bfa009246 retains exact integer cast evidence for call-statement
  arguments so `retain`'s `self.lexer.append_source_byte(value as u8)`
  plans on the `[0..=255]` payload bound the transition transports (the
  nested receiver was never refused; `tests/flow/terminal_unit/call_argument_casts.rs`
  pins the three probe shapes and
  `pass/calls/runtime_nested_receiver_cast_argument_exit` exits 70 in
  the interpreter and establishes its entry). The gate's next omission is
  `statement sequence: call: call operation`, state 3 (`source_full`),
  statement 1: `self.lexer.reject(LexDiagnosticCode::SourceCapacityExceeded,
  length, length)`, an attached call through the nested receiver whose
  first argument is a copy-enum case literal beside two ranged scalars;
  the product `Main::main` has no such call, so its own next stop after
  OWNER_QUESTIONS 8 is unmeasured. Gate check timing moved with main, not
  with these commits: 1538.3 s wall / 1402.2 s user at d0371277e3 became
  6463.8 s / 4370.1 s at 4bfa009246 and 6458.5 s / 4367.7 s with the
  validation change reverted on the same base; a `sample` of the run sits
  in `typed-trees-to-checked-trees` `flow::builder::build_flow_facts ->
  state flow -> transition exits` calling `validation` write frames
  (`permuted_cycle_frames`, `place_paths`, `stored_origins`), so one of
  d0371277e3..fc633a98ae's call-frame commits (ca6e517527, 1544a206ec,
  90cde5211e, f5d4abc290) is the likely cost and is unattributed here.
  Native production of the two new fixtures is not claimed: their entry
  statements stop at `structural field store: scalar field type`, `local
  data: structural call binding` and `state graph: terminator: conditional
  successors: parameter transfer` (a sum-literal state argument), separate
  Unit slices, so both sit on the checked-only roster. Validation still
  accepts an exact `i32 as u8` narrowing with no positive evidence (an
  unbounded field source), contrary to counts_and_addresses.md; the Unit
  builder fails closed on it. A user declaration spelled like a std one
  (`ByteRead`, `Lexer`) makes std's own machines fail checking
  (`read_line` `store` overflow, `MacosArm64::extent_shape` range), the
  same family as the 3 red `calling_policy_plans::macos_entry` tests.
  Known baseline reds met on the way (fc633a98ae): those 3 tests,
  `validation` suite's
  `match_values::fresh_match_containers_cannot_hide_existing_owned_inputs_or_cleanup`,
  `typed-trees-to-checked-trees --lib` 3 failures and 8 clippy errors,
  `canary_suite`'s `discovered_exact_native_coverage_is_consistent` (798
  against 797) and its unused-import clippy errors on macOS, and the
  `control_flow/` pass leg's 23 fixtures.

## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
