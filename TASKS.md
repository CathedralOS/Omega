# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion plan](wiki/drafts/reference/rust_compiler_completion.md) defines acceptance;
this board identifies remaining work, not a passing baseline.

| Start here | Purpose |
| --- | --- |
| [Immediate product closure](#immediate-product-closure) | Application, sample and canary outcomes and their native-execution blockers. |
| [Reflection](#semantic-reflection) / [embedding](#embedding-and-interpreted-components) / [tests](#requirement-based-tests) | Accepted language and library capabilities awaiting connected implementation. |
| [P1–P5](#p1---authority-roots-and-entry) | Entry/storage, materialization, portable evidence, ABI and Cathedral customers. |
| [Parallel language work](#parallel-language-and-compiler-lanes) | Other accepted language capabilities and their concrete dependencies. |
| [Rust release closure](#rust-compiler-release-closure) | Full-gate evidence and matching-host runs, separate from feature implementation. |
| [Optimizer board](TASKS_OPTIMIZER.md) / [bootstrap board](TASKS_BOOTSTRAP.md) | Separate execution owners; link rather than duplicate their work. |

Keep missing behavior, owning code, necessary dependencies and acceptance.
Current evidence replaces stale diagnoses; a rerun confirming the same result
does not earn another paragraph. Delete completed work and duplicate aliases;
Git preserves history. Keep a revision-bound failure only when it changes
where to resume, and prefer stable symbols/tests over line numbers.

[AGENTS.md](AGENTS.md#workflow) governs task provenance, scoped checks, claims,
strategy pauses and publication. Live assignments belong in the
[claims registry](tools/claims.md), not the board. A paused strategy is not a
blocked language feature. Genuine owner decisions belong in
[OWNER_QUESTIONS.md](OWNER_QUESTIONS.md); implementation difficulty alone does not.

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
[Rust compiler completion plan](wiki/drafts/reference/rust_compiler_completion.md) defines
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
  Use the [migration recipe](wiki/drafts/designs/range_suffix_migration.md). Libraries
  still need migration, including scalar fields in `std/calling.omg`; this is
  distinct from the Binding carrier rename. Migrate compiler source, tests and
  samples too, including Epsilon's
  `bootstrap/5_omega/{parser,representations}.epsilon` and
  `tests/bootstrap/omega-parser`. Repin the changed D closure and run its parser
  gate; this changes Omega syntax, not the bootstrap languages.

  `b9b68383c9` migrated the 35-fixture
  `tests/omega/{pass,fail}/{range,ranges,borrow,borrows}` leg (param suffix ->
  `requires`; field/element bounds -> consumer-state `requires` admission;
  sum payloads -> `case X(v: T in D)`; result suffix -> `ensures` +
  transition-arm return; exclusive float bound -> `f64 in Finite`; fail
  twins re-pinned to requires-contract/domain fragments). Probe-verified
  routes and the ~172-site Range-variant consumer inventory are recorded on
  the wave's claim notes. Remaining corpus legs: ~1,300 suffix sites across
  the other `tests/omega` groups, `source/library`, `samples`,
  native-differential and the bootstrap fixtures; the
  `parse_type_constraint_handles` range arm and its consumers delete last.
  Known migration gaps to route around: requires-admission cannot fold
  binary args or indexed-path guard facts (hoist to a local), case-payload
  `in D` cannot satisfy a callee `x in D`, and stale-write invalidation
  does not reach contract admission.

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

  Reconcile application publication before changing the parent gitlink. The
  recorded pin `ef6682f75f48f9a750bc8f7594cc1f04e78ddf60` contains cumulative
  geometry and scalar scan/RLE/dispatch work; preserve it rather than re-porting.
  Fetch application `main`, publish any remaining local work without force-pushing,
  then pin a tested published commit.

  Run `python samples/apps/squalr/tools/verify.py native --timeout 600 --omega <binary>`
  through ordinary package review on matching hosts. Connect
  **VEC-NATIVE-GROWTH** to partitioned scan results and repeated filtering;
  **BUMP-ALLOCATOR-CANARY** and **PLAN-LAID-VIEWS** supply allocation and placed
  access. Follow the application's actual next missing operation rather than
  waiting for every related task to close.

  Reproduce the port's reported foreign-package `Optional<T>` payload rejection
  in state patterns (`ScanFunctionScalar` and `SnapshotRegionFilter`), then fix
  any remaining resolution/lowering gap and restore the intended sum API.
  Boolean-plus-payload and package-local sum substitutions are not proof of
  parity. Migrate index bounds through **REMOVE-BRACKETED-RANGE-ANNOTATIONS**;
  ordinary marker-data namespaces are an implementation choice, not an owner
  decision. Port work stays with this customer through native acceptance.

  The pinned win64 frontier is
  `native_win64_write_byte_catalog_frontier`: the win64 x86-64 target package
  carries no closed native catalog identity for byte emission, so
  output-bearing sink providers cannot select a `write_byte` row there.
  Result-writing sink paths must keep refusing on win64 until the target
  catalog gains that identity; linux_x86_64 remains the native witness host.

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

  Compiler dependencies: `typed-trees-to-checked-trees/src/execution/terminal_unit/providers.rs`
  admits one provider field, while this entry needs four; lowered
  `attached_unit/providers.rs` excludes scalar-result provider candidates
  needed by window/input/clock operations. Retain one occurrence-owned concrete
  provider receiver across calls, including nested services: a Fused receipt
  establishes service authority, not receiver storage, and
  `ProviderAttachment` is not an ordinary structural argument.
  **ENTRY-CONTENT-ROOTS**, **TR3-TR8** and **STATE-LOCAL-VALUE-FRONTIER** own these
  general repairs. No global provider state or weakened occurrence matching.
  Unverified candidate: `swarm/macw5-macos-app-pub` contains scalar-result
  provider selection and nested call-source custody work. Review it against
  the multi-provider receiver requirement before reuse.

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
  [Rust product gates](wiki/drafts/reference/rust_compiler_completion.md#release-matrix):
  checked semantics, native products for authored targets, and documented
  exit/output on matching hosts. `compiler/tests/samples_compile.rs`, sample
  commands and the actual failing stage own integration; application submodules
  remain **SQUALR-HEADLESS** and language fixtures **CANARY-CORPUS**. The
  harness repeats package preparation and std checking in
  `compile_sample_to_checked`. Reuse unchanged checked inputs under their exact
  identities and measure the complete sweep before reporting sample coverage.

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
  | `dutch_flag` | Native Color-array construction, reads, swaps and case dispatch, Console interaction and exit 70. Its field is already intrinsic `Binding<Console>`. **STATE-LOCAL-VALUE-FRONTIER** and **WRITE-ONLY-BORROW** own composition; replacing nominal enum storage with integers is not acceptance. |
  | `calendar` | Publish unchanged trapping conversions through **ARITHMETIC-POLICY-REALIZATION**, finish ordinary package review, and verify all five grid rows plus the live 20-byte header in capacity-21 buffers. A `contains: 30` banner match is not a rendered calendar. `compiler --test byte_index_carriers` is the arithmetic/bounds/write control, not application acceptance. |
  | `windowed_calculator` | Reproduce and locate the checked-compilation stall: at `50559da3ab9` on Linux x86-64 it exceeded 25 minutes of CPU without a diagnostic while the other 146 maintained mains checked. The cause was not established; do not label it a provider-selection or proof-search defect without evidence. Native GUI acceptance remains **MACOS-APPLICATION-PUBLICATION**. |
  | Other text/index/match samples | Close `binary_search_viz`, `maze_flood`, `prime_sieve`, `multiplication_table`, `dice_histogram`, `dungeon_render`, `mandelbrot{,_zoom}`, `wire_protocol` and `dungeon_crawler_cli` through **NOMINAL-FIELD-FLOW**, **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**, **MATCH-SELECTIVE-LOWERING** or **CML4**, not one task per source permutation. Encoding facts use ordinary domains, not recognized function names. |
  | `cli/proofs/math_proofs` | Ordinary multiset data/slice extraction, selected laws and checked proof terms; keep the false twin rejecting. Equal lengths are not equal contents and `core/seq.omg` is not a Bag implementation. |
  | `calculator_rpn` | Reproduce `Main::add`'s indexed nested-field initialization failure with the actual Console-bearing receiver; the recorded std-free reduction passes. **STATE-LOCAL-VALUE-FRONTIER** owns ordinary indexed reads/local construction. Follow the callee's recorded omission, not just the caller's `UnavailableCallee` label, and preserve the full sample's storage and arithmetic. |
  | Bounded Console input | Finish selected-dispatch/provider/call transport under [bounded input](wiki/spec/resources/bounded_input.md): exact prefix/destination, once-only effects, cleanup, blocking/crash contracts, zero-capacity non-consumption, LF/EOF/Full, untouched tails, alias rejection, caller continuation, failure and invalid-result controls. Keep prefix guards until count-to-extent evidence exists. |

  The [sample reductions](wiki/drafts/measurements/checking_contract_exit_fact_cost.md#the-unavailablecallee-chain-and-why-it-misleads)
  locate the `calculator_rpn` failure at
  `let a: i32 in Saturating = self.stack.slots[0]`. Reproduce comparisons on
  one binary with equivalent package/entry inputs; a dependency-review failure
  does not exercise the root program. The algorithm cohort also reports
  unresolved operator selection and `render_col`'s default-domain requirement:
  route those to **OPERATOR-MACHINE-SUPPLY** and **NOMINAL-FIELD-FLOW** after
  attribution, not one task per phase label. Do not weaken `Utf8` to pass.

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

- **CANARY-CORPUS.** Bring `tests/omega/{pass,fail,run}` and
  `compiler/tests/canary_suite/` to their promised checked/native stages.
  Use the [focused selectors](AGENTS.md#running-one-test); full closure is
  `mbx nextest run -p compiler --test canary_suite --no-fail-fast` on one
  revision with filters unset. Keep detailed logs outside the board; do not
  migrate fixtures during a measured run.

  Prioritize the actual operation named by `omitted at local construction`,
  not the downstream `rejoins 0 Terminal attachment identities` symptom.
  Tight starting groups are structural-call bindings in `recast`, pure-source
  field stores in `float`, and conditional guards/call operations in
  `filesystem`. Their shared owner is
  `typed-trees-to-checked-trees/src/execution/terminal_unit/`; keep each
  repair attached to its unchanged source-to-native customer.

  Distinct remaining probes include
  `control_flow/guarded_leaf_branch_expansion` and
  `text/runtime_alias_string_write` (`[u8; N]::Utf8` identity sharing),
  `capabilities/uses_caller_folder`,
  `domains/explicit_domain_erasure`, `proofs/cauchy_predicates_compile`,
  `providers/external_leaf_dllimport_compile`,
  `control_flow/runtime_branching_helper_string` and
  `core/extent_root_provider_adapter`. Reproduce a selected customer on the
  working base, repair its owning capability, and rerun the same outer command.

  Concrete repair routes:
  - `core/extent_root_provider_adapter`: establish a legitimate build-visible
    selection route for a public core-owned boundary trait. In
    `symbols::find_product_declaration_from_source`, an unqualified operand
    fails `same_product_package_instance` against toolchain-injected core;
    core has no package manifest for an authored dependency alternative.
    Resolve that ownership/visibility route under the package contract, without
    inventing fixture-local authority or weakening exact declaration selection.
  - `capabilities/windows_provides_import_exit`: preserve the producing
    statement for `self.m = self.beeper.beep(self.v)`. The packaged Windows
    fixture reaches `checked-trees-to-lowered-psi/src/unit/attached_unit/composed_control/state_graph/body.rs::statement_continuations`,
    which excludes call-result stores from its plain-field-store exception.
    Keep the ordinary-machine assignment as a control and the actual
    `Binding<R>`/`ForeignBinding` package inputs in the reproduction.
  - `dungeon/runtime_direct_boolean_conjunction_dispatch`: extend the direct
    boolean-decision route from `Conditional` to each ordered `GuardedJumps`
    arm in `unit/attached_unit/composed_control/state_graph/emission/state.rs`.
    One conjunction plus default works; two conjunction arms plus default
    encounter the short-circuit refusal. Reuse
    `emission::boolean_control::emit_inlined_boolean_guard_blocks` with true
    targeting the selected successor and false targeting the next arm/fallback.
    Carry a value or planned decision in each arm draft, including the first,
    and preserve deterministic checked block/edge allocation. Do not merely
    delete the refusal and materialize guards eagerly: the right operand of
    `self.x == 2 && (self.y / self.z) == 1` must not execute when the left is
    false, including `z == 0`. Pin this with native execution and an effectful
    operand control when its upstream admission is available; no trap outcome
    was established by the earlier lowering-only probe.
  - `providers/external_leaf_dllimport_compile`: connect the checked per-call
    authority derivation to ordinary native production. `NormalizedForeign`
    needs an explicit policy row, while the ordinary policy has none outside
    the settled Console/filesystem/time cohorts. Rejoin the program's own
    checked-flow evidence before classification; adding a fixture permission
    or another cohort without that derivation is not acceptance.

  Provider absence on macOS/Windows remains distinct from lowering:
  `toolchain_filesystem_leaves` and `toolchain_time_host_leaves` map only
  linux_x86_64 (darwin rows need an import with an injected argument, e.g.
  `clock_gettime_nsec_np(8)`, and a constant binding kind), and the
  fixture-local `Gui`/`Input`/`Console` boundaries in `host/*` and
  `traits/boundary_trait_effects_host_call` need their own exact providers.
  Preserve nested service nomination through
  `selected_dispatch::program_entry_service_requirements`; targetless checking
  has no selected entry and cannot exercise that nomination. Keep
  `filesystem/runtime_local_host_result_dispatch_exit` as the Linux
  source-effect ordering probe. Borrowed binding parameters and
  `capabilities/uses_caller_folder` are owned by **ENTRY-CONTENT-ROOTS**.

  Follow `CheckedUnitEffectPlans::omissions`,
  `InvalidUnitMachinePlan::omission` and `LocalConstructionTrace` to the
  actual missing operation/facts, not just its phase label. The diagnostic route
  is covered by `checked-trees-to-lowered-psi/tests/unit_plan_omissions.rs`;
  another diagnostic-only change does not close corpus behavior.

  Current integration targets:
  - `text/runtime_stdin_command_branch_exit`: owned `Command` result-to-field
    assignment in `parsed_input`; `execution/terminal_unit/control/statement_sequence.rs`
    still limits that call-result path to primitives. **STATE-LOCAL-VALUE-FRONTIER**
    owns structural result storage and independent lowering. Keep the reader's
    algorithm and run `runtime_stdin_command_branch_exit_canary_runs`.
  - `host/runtime_console_bounded_line_exit`: finish other-host native coverage
    of the full result and mutable field windows, including zero capacity.
    `providers_float_and_console/console_reader.rs::bounded_line_window_results_run_natively_with_zero_extent_nonconsumption`
    now exercises the unchanged customer on macOS ARM64/Linux; Windows is
    explicitly skipped. Run the Linux hosts and connect the Windows witness
    through the supported-host realization work. Require exact exits/output,
    non-consumption and untouched-tail behavior, not discard-result execution.
  - `text/runtime_stdin_line_buffering_exit`: both `echo_line` calls already
    carry `block`; finish borrowed intrinsic-service parameters through
    `write_prefix` and state edges, which still use bare `&mut Console`.
    **ENTRY-CONTENT-ROOTS** owns exact receipt/borrow transport through
    declaration checks, Unit forwarding, lowered states and selected
    `service_custody/parameters.rs`. Do not replace the two-read/helper flow
    with a single-hop owned-Service recognizer.

  - `filesystem/native_wrapper_write_all_result`: on macOS ARM64, store the
    `Filesystem::write_all` result in a field; bad path yields Error and
    the `/tmp` path yields Ok. The native harness requires PASS and no FAIL
    on stdout, not merely exit zero. Keep this customer distinct from
    filesystem release-proof closure.
  - `host/runtime_gui_foreground_window_exit`: select the fixture's exact
    authored Gui requirement/provider, then execute create -> foreground
    query/result store -> destroy -> exit 70 on Windows. The invisible window
    need not become foreground.
  - Integer/string literal dispatch: emitted programs exit 22 for integer
    dispatch and 11 for `"look"`, preserving authored selection order and
    ordinary result/field storage. Reproduce the current omission before
    carrying forward historical phase diagnoses.
  - `wire/runtime_wire_exact_array_without_count_exit`: execute generated
    encoding of `[u32; 4]` without a synthetic count sibling. Require seven
    bytes `00 00 04 01 02 03 04` and exit 70; retain the checked-interpreter
    oracle. Its dedicated native witness remains unwritten.
  Route other failures to **STATE-LOCAL-VALUE-FRONTIER**,
  **MATCH-SELECTIVE-LOWERING**, **GENERAL-CYCLIC-EXECUTION**,
  **NOMINAL-FIELD-FLOW**, **BORROW-PROOF-CONVERGENCE**,
  **OPERATOR-MACHINE-SUPPLY**, **MODULE-NAMESPACE-RESOLUTION**, or
  **ARITHMETIC-POLICY-REALIZATION** as appropriate.
  Audit fixtures against the spec before weakening checks:
  `proof_inductive_climbing_sum` and its unbounded-accumulator negative still
  owe [exact intermediate arithmetic](wiki/spec/language/numeric_values.md)
  bounds even when a theorem uses `embed`.

  Non-corpus integration and package-fixture failures remain in
  [release closure](#rust-compiler-release-closure), under their actual gates.

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
    quotient/remainder endpoints. Integer-division terms now install on
    actuals at every scalar call-actual consumer
    (`prove_ranking_range_call`, `endpoint_pins::argument_sources`,
    `requirements.rs::prove`, `saved_arguments::install`); saved let-locals
    remain inadmissible as call-component actuals (a separate feature).
    Reuse `OpaqueTerm::{ShiftLeft,ShiftRight,BitwiseAnd}` and
    `install_nonpolynomial_terms`, preserving count-within-width obligations,
    floor-division semantics for signed `>>`, and two's-complement `&` bounds.
    Recheck constituent operations at each exact
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
    execution/write contracts. Signature-only callees (boundary/requirement/
    admitted/external) now frame caller-visible writes from their selected
    signature contract rather than an empty body summary, and rank-prefix bars
    admit callees whose complete frame is disjoint from protected carriers.
    Direct and composed checked-body call initializers already
    preserve entry facts through `call_tree_initializer_preserves_entry`;
    mutable writes to protected premise carriers must continue to reject.

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
  **ENTRY-CONTENT-ROOTS**. Unit Console forwarding does not complete computed
  result execution.

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

- **EMBEDDING-SOURCE-TO-HOST.** (new-scope) Deliver compile -> load -> bind ->
  invoke -> host result -> guest result -> close from an ordinary source-authored
  interface package, host implementation, and script entry. Owners: product
  compilation/entry descriptions, `psi/semantics/terminal-interpreter`, and
  selected host adapter realization; expose the library, not command dispatch.
  Use the ratified carrier name `Binding<R>`. The existing
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

  Cathedral's `tools/*-canary/run.sh` harnesses still assert over retired
  `04_typed_trees.json` / `05_machine_contracts.json` dumps. Replace those
  assumptions with checks of the intended behavior or supported artifact
  contracts before citing the canaries as evidence. Source/build migration
  alone cannot repair these assertions; compile-report output is not their
  semantic oracle. Preserve Omega's no-default-debug-dumps contract rather
  than restoring mandatory dumps to satisfy a downstream script.

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

  Carry exclusion envelopes through ordinary custody in
  `target-operations-to-selected-instructions/src/legalization`, image emission,
  COMPONENT-SUBSTRATE replacement, and WIRE-RUNTIME-AND-INSTALLATION.
  Preserve `compiler --test build_behavior_exclusions -E 'test(sink_composition)'`:
  it covers retained native artifacts, host execution, source-free replay, and
  loud-sink rejection. Those controls do not close installation/replacement.

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

  Reuse the structured demand/supply join through
  `VerifiedComponent::export_contracts()` in `verified_components.rs` and the
  `build-scope-topology` fixture. Do not replace derived contract identities
  with surface-name hashes or rebuild the existing join.

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
  canonical host canaries. A boundary-trait leaf
  realizes through the target's DLL linkage, and as a direct syscall only on
  an OS with a stable syscall ABI; on `windows_x86_64` the console and
  process-exit leaves therefore bind kernel32 `ExitProcess` by `DllImport`
  rather than a hosted intrinsic, and the accepted Console/ProcessExit
  binding is consumed by that row whatever its realization. Landed 2026-09-23:
  provider planning follows the machine filter's host fallback, so an
  unprofiled check plans the leaves it selected; source-evaluated import
  settlements are minted on both native routes
  (`native_realization/source_evaluated_imports.rs`, a 256 KiB same-stack
  reserve because the callee runs on the guard-free receiver stack); the
  receiving policy classifies the cohort in
  `terminal_authority_policy/console_host.rs`. Exit-through-console fixtures
  compile, run and exit 70 on a Windows host. Remaining on Windows:
  `write_byte` and `read_byte` stay compiler intrinsics with no hosted
  realization (kernel32 `WriteFile`/`ReadFile` through the same route), and
  the foreign-entry stack reserve is a constant, not a per-callee declaration.

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
  The carrier is declared once, as `Binding<R>` in `source/library/core/binding.omg`
  (`omega::language::core::binding`); `ForeignBinding` remains the distinct
  locator in `external_binding.omg`.
  Verify the real CLI samples and nested-binding helper-return customer after
  the rename; file-name mismatch is not an intentional compatibility policy.
  Owners: target package assembly, `program-entry-plan`, Psi
  `terminal-production`, Omega `compiler/native-realization`,
  `image-emission` and `external-roots`. Source names spell the carrier
  `Binding<R>`; the old service-only `Bound` domain is already retired and
  must not return.

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
  - Complete borrowed `Binding<R>` parameter and argument transport. Migrate
    `capabilities/uses_caller_folder` so `Librarian::archive` takes
    `folder: &Binding<Folder>` and the borrowed `Main::main` lends its field;
    preserve its caller-supplied authority and native service effect. Promote
    `fail/providers/service_borrowed_parameter_rejected` to positive coverage
    when the same general route works through checking, Unit/Terminal calls,
    selected dispatch, interpretation, and native realization. Keep a negative
    control that moving affine `Binding<R>` by value out of borrowed storage
    without replacement rejects. Provider multiplicity, conformance, and fused
    erasure must not change the carrier's declaration-owned affine semantics.

  Acceptance: published `number_guess`, `cli_mvp` and `generic_counters`
  retain their documented behavior with no test-supplied `self` or service
  qualifications. Keep `entry_and_abi::hosted_receiver*` controls.
  Execute `acquires_through_helper_return` through its original `Main::main`
  nested binding/helper-return chain, preserving exact propagated acquires and
  provenance. The fixture currently selects empty `Probe::enter`; that
  publication and checked-report coverage are not Main's native closure.
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

  A probe on `swarm/macw4-plan-laid-views` (`670843530b`) reports an earlier
  admission gap: synthesized
  `PlacedField` accessor boundary machines attach as
  `DataSupplyMode::BoundaryOpaque` (`record_synthesis.rs`), which the
  `CheckedShape` supply gates in `typed-trees-to-checked-trees` `types/mod.rs`
  reject, so no `CheckedBoundaryMachinePlan` exists and `Inspector::inspect`
  reports `MissingBoundaryTarget`. Reproduce on the working base before repairing
  accessor synthesis/admission; retain the later referent-lending obligation.

  Acceptance: one source program establishes, accesses and retires a view
  through interpretation and published native execution. Codec/optimization
  preserve semantics; stale/substituted plan, artifact, backing, range, rights,
  occurrence or lifetime reject before access. Failed establishment returns
  custody and retirement preserves declared resident/vacant state. Preserve
  multi-row order independence, exact type/path/domain rejoin, exclusive overlap
  rejection and shared/disjoint admission. Run matching supported hosts or
  explicitly retain unavailable runtime legs.

- **POST-HANDOFF-WRITER-OWNERSHIP.** (new-scope) Separate portable materialization
  description from consumer implementation under the
  [existing ownership rule](omega-rust/pipeline.md#portable-materialization-and-consumer-ownership).
  `psi/foundation/layout-plans/src/materialization/mod.rs::derive_post_handoff_writer`
  and `src/post_handoff_writer/` currently mix portable input with generated
  writer plans, private context ABI, fragment lowering, and execution. This is
  implementation cleanup, not an owner question or a reason to weaken the firewall.

  Move native writer derivation, plan/invocation carriers, `lower_reusable_fragment`,
  `execute`, and `apply_post_handoff_writes_atomically` to cohesive Omega owners.
  Update `program-entry-plan`, `isa-x86_64`, `isa-aarch64`,
  `executable-installation`, `external-roots`, and `provider-planning` consumers.
  Keep only genuine portable description and semantic validity in Psi; the
  interpreter consumes portable operations through its own execution path, not
  through native fragment lowering. Classify stored-integer write/fit helpers by
  responsibility and share small semantic predicates without sharing the producer
  derivation with its independent checker. Remove obsolete Psi exports, forwarding
  wrappers, and duplicate orchestration; renaming directories alone is not closure.

  Acceptance: a reader can follow portable input to the separate consumer
  derivation, validation, and execution owners, with no Psi-to-Omega dependency.
  Preserve nested field/index writes, exact source/placement/fit custody,
  reusable-fragment identity, guard bytes, and atomic rejection before writes.
  Move and run the relevant `layout-plans` writer/fragment controls with their
  owners, `compiler --test layout_plans`'s `writer_lowering` cases, affected-crate
  checks/Clippy, and architecture tests. Verify bytes and execution on available
  matching hosts; keep **SYMBOLIC-MATERIALIZATION**'s missing Linux AArch64 runtime
  evidence open until that host runs. Do not add a new IR or interpreter subsystem
  merely to relocate the existing machinery.

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
  Preserve exact-mapping revocation/invalidation checks and retry custody.
  Remapping must reject retained foreign pins, changed geometry or widened
  rights, and establish a fresh mapping era before replacement backing is used.
  Protocols, queues, isolation and translation policy remain provider/OS code;
  do not add compiler-owned drivers or treat a CPU barrier as device completion.

- **PLACED-ACCESS-NATIVE-OPS.** Extend contract-fact index derivation to remaining
  placed-access operations. Reuse
  `ClosedScalarValueContractPlan::requires_bound_interval`, verifier
  `selector_bounded`, native `indexed_stores.rs`, and substitution/authority
  controls in `terminal-interpreter/tests/unit/runtime_index_arguments.rs`.
  The dynamic shared receiver `cells[i].get()` still needs CML4's checked
  argument-path admission and DOMAIN-ISSUER-ROUTES' scalar-wrapper path; follow
  `dynamic_indexed_shared_receiver_lane_pends_on_upstream_legs`.
  Coordinate source fixtures with **REMOVE-BRACKETED-RANGE-ANNOTATIONS**:
  use contract/domain facts, not the revoked annotation syntax.

  Acceptance: source-driven native `&write` and `&mut` calls mutate the
  caller-selected element and leave its neighbors untouched; invalid indices,
  substituted bounds or access authority reject. **PLAN-LAID-VIEWS** separately
  owns provider-backed view establishment and access-plan realization; this
  indexed-store delivery does not complete that larger contract.

- **RECAST-SOURCE-POSITIONS.** Finish native execution of inline recasts in
  guards, call arguments and nested expressions. Reuse their checked admission
  and whole-place source reads, preserving borrow-polarity rejection. The
  [recast contract](wiki/spec/layouts/recasts.md) requires representation
  compatibility, not this source-position restriction.
  Carry checked layout/validity, backing identity, lifetime and access through
  ordinary expression sequencing and temporary loans rather than bypassing
  the recast check. Runtime `recast_views` canaries remain blocked on a
  pre-existing Terminal unit-plan omission at local construction (entry
  establishment rejoins 0 attachment identities), reproduced identically on
  the pre-change base — not recast machinery. Acceptance: valid inline
  equivalents of supported shared
  and mutable recasts check and execute; incompatible geometry/validity,
  access escalation and conflicting backing use reject at the offending
  location. Preserve precise symbolic/boundary-witness footprint refusals.
  Migrate stale imports in `recast_position_fenced` before using it as fresh
  evidence.

- **EXTERNAL-DATA-SCHEMA-CONVERSION.** Finish and verify the authored
  preserving-codec customer under the [codec contract](wiki/spec/layouts/codecs.md),
  not a compiler-special `decode_preserving` path. Admission already recognizes
  `PreservingDecode`; `wire/wire_preserving_decode_relay_exit` validates a
  known byte and retains the exact ordered unknown tail in a borrowed
  `OpaqueWireRemainder`. Its landing established checked compilation, not
  native success. At `b725fb771e` (macOS arm64)
  `versions_wire_and_const_lengths::wire_preserving_decode_relay_exit_canary_runs`
  fails to compile: `Main::main`'s Unit plan is omitted at `state graph:
  state signature: parameter custody shape: owned non-linear record contents`
  (state 1), because `inspect(relayed: Relayed<LocalMessage>)` receives an
  owned record whose `remainder.bytes_and_ordering` is a borrowed `&[u8]`.

  Complete faithful relay through the same selected codec: preserve unknown
  bytes/order while independently handling the validated known value, reject
  incompatible codec identity and invalid known input, and retain the input
  loan while the borrowed remainder is live. Use ordinary library machines and
  existing call/storage/lifetime owners. Acceptance: the source customer runs
  on a matching host, relay output preserves the required opaque bytes/order,
  and codec-substitution, invalid-input and loan-conflict controls reject.
  Preserve unique historical-migration selection; generated-codec verification
  and trust remain with their existing owner.

- **GENERATED-CODEC-INDEPENDENT-VERIFICATION.** Establish sufficient
  independently checked evidence for generated codecs' `Derived` trust under
  [public codec agreement](wiki/spec/layouts/codecs.md#agreement-and-trust).
  The no-authored-policy route now exists, but
  `checked-interpreter/src/interpreter/evaluator/wire_verification.rs`
  compares Floor/Ceiling members and malformed-input probes.
  `build-evaluation/src/admission/wire_protocol.rs` reports `Derived` when
  that probe's gaps are empty. Finite examples do not establish the general
  agreement law; comparing placements or a shared field classifier is not
  independent checking of the codec body.

  Connect general agreement evidence to the exact realization and public
  schema, or retain explicit compiler-admitted trust where it is unavailable.
  Check the authored-policy fallback against the same obligation rather than
  treating policy agreement as proof of every codec behavior.
  Acceptance: a no-policy generated realization earns `Derived` only from
  independently established requirements; witnessed divergences reject and
  unsupported obligations remain honestly admitted. Preserve the useful probe
  controls without presenting more probes as the missing proof. This is an
  evidence-adequacy gap, not an observed incorrect encoding; preserving codecs
  remain with their separate owner.

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

  Exercise request-on/request-off replacement and retry through
  `stage_exact_bytes` / `install_staged_products`. Preserve
  `executable_publication.rs::a_failed_pair_member_leaves_the_previous_pair_intact`;
  receiver mismatch rejection is separate from publication atomicity.

- **PSIIR.** Complete source-free Terminal execution and logical-work bounds
  across canonical encoding, independent reconstruction, interpretation,
  resource analysis, native realization and installation.
  The [encoding contract](wiki/spec/terminal-psi/encoding.md) and
  `tests/architecture/encoding_contract.rs` already cover the codec's closed
  vocabulary, envelopes and mathematical certificates; maintain that coverage
  as the operation owners extend it, not another wire-format project.

  In `terminal-fixed-fuel`, `310b1ca4ba` retained the unbounded-rank cause
  with cycle/operation identity (`UnboundedCycleComponent{component,
  UnboundedRank}` across whole-entry, condensed and rank-multiplied
  interiors; `Unranked`/`InvalidRankedScc` for revisited nodes), and
  `3f797950ea` landed conditional bounds: `parameter_requires_bound`
  relaxes a conjunctive `requires` relation graph (`<=` transfers the
  ceiling, `<` less one, `==` both ways), recovers one achieving simple
  path deterministically, and binds every contract row on it as
  `relevant_preconditions`. Reuse the existing acyclic max-arm,
  ranked-interior, and topology-derived cycle bounds. Remaining:
  disjunctive/implied ceilings (need per-arm support semantics),
  `IntegerMath*` vocabulary clauses (linear checked reasoning),
  guard/path-fact bounds (a separate premise channel, not contract rows),
  and the wait/foreign-edge cause (needs semantic vocabulary first).
  Preserve `InvocationBoundCallee` and `UnboundedCycleComponent` when
  realization/bound evidence is unavailable; do not fabricate a ceiling.

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
  `execution/terminal_unit/`, `checked-trees-to-lowered-psi/src/unit/attached_unit/`
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

  - Implement `terminal-operation-level-trap-crash-site`: choose an operation-site
    representation under [delegated Psi design](AGENTS.md#psi-implementation-and-deferred-human-audit),
    using the [encoding alternatives](wiki/drafts/designs/terminal_operation_trap_crash_site.md)
    as implementation input, not an approval request. Update the observation
    specification, schema version, codec, independent reconstruction, interpreter,
    and native realization together. Retain exact cause, primitive denotation,
    and path-conditioned evidence under the same-cause ceiling; do not fabricate
    a boundary identity or terminator edge. Test source-produced arithmetic and
    conversions after reload, with wrong-site/cause/path and missing-evidence
    rejection. Trapping casts/shifts currently
    refuse runtime preparation; direct Trapping arithmetic in contract position
    remains invalid, not a new predicate term.
  - Wrapping conversions landed at `9085ca9132`: `expression_preparation/
    wrapping_cast.rs` realizes `value as T in Wrapping` for every fixed-integer
    sign/width pair from masked halves — unsigned destinations take the
    `operand mod 2^B` residue (`& (2^B−1)` mask for signed sources, low-63 +
    sign-bit split for `u64`), signed destinations take
    `low | (bit <<% (B−1))` with each half masked to a range the proof
    machinery can derive, avoiding the direct fold and carrier-less `i_C ->
    i_B` casts. Serialized-Terminal execution tests cover every sign/width
    combination; trapping controls intentionally still refuse.
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
  Reproduce `float/float_trapping_*`,
  `expressions/arithmetic_domain_trapping_*` and
  `arithmetic/constant_trapping_shift_value_overflow_traps` before attributing
  their failures;
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
    source obligations. Reuse correlated multiply/forbidden-root derivations,
    `caseTwo` Boolean identity transport, and cast endpoint-substitution laws;
    keep `casts/tests.rs`'s `rule_axioms == 0` controls. Remaining residue is
    behind `rule_instance` fallbacks, including nested identity reversal and
    `Id Two`/`Id Int` crossings, not wholly underived rules.
    Preserve numeric policy and exact operands. Unsupported arithmetic or construction-budget fallback
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
    the controls below, including member-call target resolution inside
    mathematical definitions and ordering over non-integer operands.
    Replace authored-name sort classification in
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

  Reuse `ClosedIntegerRelation`'s exact compound normalization in
  `proof-admission/integer_rules/open_terms.rs` and the free-loop native
  positive/wrong-update controls. `proof_inductive_gauss_sum` and
  `proof_inductive_climbing_sum` still need STATE-LOCAL-VALUE-FRONTIER's
  value-returning cyclic execution beyond their checked-only routes.

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
  Revalidate changed entries' justifications before refreshing source digests;
  a matching digest is not a soundness argument. Remaining work:

  - Preserve supplied `ProofBundle::crash_obligations` and untrusted search in
    `proof-admission::certificate_search`; verification reconstructs obligations
    and replays certificates without searching.
  - Discharge `ExplicitlyTrusted` reconstruction, normalization, scope,
    invalidation and call/cycle-composition rows with checked evidence and
    exact dependencies. Preserve `PROVED_ENTRIES`, dispatch/fact coverage,
    source-closure and unfinished-dependency checks; prove prerequisites
    rather than hiding trusted composition behind a proved leaf.
  - Audit the borrowed-`self` cleanup-hook entry/content claims in
    `validation/affine_cleanup.rs`: receiver and requires checks are visible,
    but the cleanup edge's discharge of the target's entry claims needs an
    explicit argument and substitution control. Coordinate with **CML4**;
    this is a targeted unresolved audit, not an established exploit.
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
  Reuse static checked-shape record/sum formals through `ProofTerm::Scalar`
  and the `erased_record_parameter_exit` control; array-field carriers remain
  unsupported. The dynamic gap is
  `typed-trees-to-checked-trees/src/checks/contracts/dynamic_erased_lane.rs`
  rejecting erased formals on dynamic requirements because their dispatch
  plans carry no proof actuals. These refusals diagnose missing support; they
  do not complete the specified feature.

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
  Consult the [parked IEEE recovery record](wiki/drafts/designs/write_only_borrow_ieee_store_branch.md)
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

  Reuse static `Field`/`FixedIndex` and bounded `RuntimeIndex` path transport.
  Remaining `src/tests/borrow/receiver_access.rs` pins cover local-rooted
  indexed receivers and runtime-indexed parameter receivers/explicit shared
  arguments. An unverified candidate for explicit shared dynamic-indexed arguments
  is `ecfdedb34b` on `swarm/linw4-structural-borrow-identity`; rebase and run
  focused borrow checks before adopting it. Coordinate bracketed-range migration with
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
  [private callbacks](wiki/spec/build/private_callbacks.md). Reproduce and repair
  `callback_terminal_custody::direct_callback_relocation_resolves_to_its_private_function`'s
  reported `fragment import lacks selected call custody` failure. Restore the
  evaluated `ProgramStorageEntry::enter` calling plan in
  `a_package_local_calling_copy_rejects_beside_the_standard_library_entry` so
  that negative reaches its intended check. Neither reported failure establishes
  native artifact, final-address, or installation acceptance.

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

  Reuse `validate_projected_qualification_roster` in
  `terminal-verifier/src/validation/foundation/machine_foundations.rs` and the
  interpreter's `live_claims` custody. Removing the empty-qualification gate is
  insufficient: the authored customer must establish correct case-conditional
  evidence and reject a wrong roster.
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

- **TERMINAL-SLICE-VIEW-VOCABULARY.** (split-of:SLICE-VIEW-LOCAL-ENTRY-ESTABLISHMENT)
  Complete portable lowering and native realization of borrowed non-byte
  `&[T]` views: runtime length, element access and subslices with exact
  backing, extent, element identity and loan custody. Reuse
  `StructuralTypeShape::ElementView`, `EstablishElementView`, `ElementViewLength`,
  `ElementViewRead`, `ElementViewSubslice`, the catalog's `BorrowedSliceView`
  mapping, and `terminal-interpreter/src/element_views.rs`.

  Two distinct source routes remain:
  - `slices/callee_non_byte_view_len_index_subslice` consumes length, elements,
    and subslices but reports `unsupported statement kind` at local construction.
    Repair its checked producer and continue through native execution.
  - State forwarding passes Unit-graph admission but reports
    `InvalidStructuralSuccessorArgument`: the forwarded mutable view creates
    a reborrow place missing from the successor frontier. Complete that custody
    join and retain `checked-trees-to-lowered-psi/tests/non_byte_borrowed_view_admission.rs`
    and byte-view forwarding as controls. An empty callee does not exercise it.

  Core `Slice::index<T [copy]>` settles shared by-value access. Keep
  `entry/service_borrowed_slice_call` distinct from the consuming non-byte case;
  reproduce subsequent failures on the actual Unit-graph route rather than
  inferring them from scalar-graph rejection arms.

  Acceptance: a non-byte-view callee consumes `.len`, indexing and subslicing
  through Terminal production and native realization; bounds, element-type,
  alias and access violations reject. `fletcher_checksum` executes its real
  entry and exits 56; `recursive_sum` establishes sum 50/count 4 and exits 70
  on Linux x86-64, with **SAMPLE-CORPUS** retaining their complete sample
  obligations. Reuse byte-view custody where applicable and keep general
  cyclic/value sequencing with its existing owners.

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

  Validate Cathedral's entry encoding and geometry with a semantic/execution
  oracle: `cathedral/tables.omg::FRAME_MASK` must preserve frame bits 12..51
  (`0x000FFFFFFFFFF000`) without admitting flag bits. The checked-only fixture
  does not distinguish a correct mask from a wrong one. Select authority,
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

- **LOWERING-ROUTE-CONSOLIDATION.** (new-scope) Checked-to-lowered Psi picks
  one of seven whole-module producers per machine
  (`machine_lowering/machine_dispatch.rs::lower_selected_machine`), and the
  checked stage mirrors that with parallel plan rosters chosen by `or_else`
  precedence and a `CompetingCandidates` omission stage. Each family
  re-implements the same operation for one arrangement, so a feature
  combination works in one family and not in another. Do not add new
  arrangements to a family below. Widen the general route and delete the
  family instead. Ranked targets, measured 2026-09-24 against `3f797950ea`:

  1. The guard-complement check exists in six places and the two graph
     families admit different pairs. Unit graphs take `x==true/x==false`,
     builtin `==`/`!=` operand pairs and two-variant case pairs. Scalar
     graphs take only Boolean polarity. The producers are
     `terminal_unit/composed_control/topology.rs::exact_false_fallback` and
     `terminal_scalar/guards/mod.rs::complementary`; the lowering mirrors are
     `state_graph/edges.rs::validate_fallback` and
     `source_custody/guarded_exits.rs::complementary`. The Unit side compares
     authored trees and the scalar side compares checked Boolean rows, so
     unify on checked rows first.
  2. Scalar machines take three routes: the shared-catalog Unit closure, a
     single-member scalar graph, and the scalar call closure.
     `closure.len() == 1` changes the contract mode (float reflexivity) and
     content-effect admission. Start by removing that branch.
  3. Dynamic dispatch has Scalar and Unit copies of every lowerer and
     validator (`unit/dynamic_composed_unit/{join,unit_join}.rs`,
     `dynamic_lanes.rs` against `unit.rs`, forwarded-helper resolvers). The
     checked side has the matching `forwarded_calls.rs` against `unit.rs`.
  4. Composed-graph states re-emit operations through
     `composed_control/emission.rs`, a narrower copy of
     `attached_unit/ordinary_machine` emission.
  5. Eight return families each have their own builder, roster and module
     assembly (`checked_trees::flow::terminal::return_plans`, `returns/`).
  6. Structural Unit Control is a second multi-state control-graph family
     with a countdown-loop recognizer (`unit/structural_unit_control.rs`).
     Widen state-graph admission to cover it rather than extending it.
  7. Nominal cleanup has a one-root route and a separate multi-root route
     (`unit/unit_cleanup.rs`, `unit_cleanup/ordered.rs`), and the checked
     stage builds the nominal plan twice.
  8. There are nine structural-type namespaces, one `ShapeCollector` per
     plan roster, rejoined by hand (`finalize_execution.rs`,
     `attached_unit/bodies.rs::UnitPlans::with_staged`).
  The Terminal module literals already start from
  `TerminalModule::for_entry` (`c62baba3ca`). Acceptance for each target:
  the replaced family and its recognizer are deleted; programs it lowered
  still lower and verify; the pass-canary and run-test groups show no new
  failures.

- **OWNED-SELF-RECEIVER-AFFINE-DISCARD.** (new-scope) An owned `self` receiver
  is removed by `consume_terminal_self_receiver` before cleanup validation,
  regardless of multiplicity. Resolve
  [`owned-self-receiver-implicit-retirement`](OWNER_QUESTIONS.md), then implement
  its accounted disposition in
  `terminal-verifier/src/validation/frontier/terminators.rs` under
  [Terminal ownership](wiki/spec/terminal-psi/ownership.md).

  The `terminal-interpreter --test unit` controls
  `affine_cleanups::scalar_return_performs_affine_discard_only_after_edge_charge`
  and `affine_cleanups::conditional_commits_only_the_selected_affine_cleanup_after_edge_charge`
  construct an Owned, Affine self receiver with explicit `DiscardRoot`.
  They reject with `ScalarReturnAffineDiscardsMismatch` after implicit removal
  of that root. Preserve their edge-charge and once-only-disposal observations.

  Acceptance after the ruling: specification, verifier, and both interpreter
  tests agree on linear and affine receiver disposition without silent custody
  removal. Do not merely repin the tests.

- **CLOSED-SUM-EDGE-DISCARD-EVIDENCE-DISAGREEMENT.** (new-scope) The closed-sum
  arm's answer is stated in lowering: the case dispatch is the subject's
  explicit terminal consumption, so an arm's no-code discards are its cleanup
  evidence without the subject
  (`composed_control/state_graph/edges.rs::validate_bindings`, fed the
  subject's authored position by `cases.rs`). The three
  `optimization-unit-semantics structural_cases::owned_results` tests pass
  again. Remaining: the evidence producer,
  `typed-trees-to-checked-trees/src/execution/terminal_cleanup.rs::build_state_plan`,
  still names the subject on each arm because a dispatch moves no place.
  Treat a closed-sum `transition subject { .. }` over an owned parameter as
  that statement's consumption there, then drop the subtraction in lowering
  so the two facts agree by construction. Keep the equality check itself.

- **AUTHORED-SELECTION-FINALIZATION-GAPS.** (new-scope) Close the authored
  declaration selection occurrences that survive successful checking.
  `typed-trees-to-checked-trees/src/authored_selections/finalization.rs`
  rejects occurrences still `LateBound` at the end of checking. Reproduce:

  - `package-evidence` `terminal_permission_policy::uefi::ordinary_uefi_permission_retains_calling_meaning_omitted_from_accepted_schema_digest`
    reports an unresolved `CheckedCall` selection.
  - `compiler --test canary_suite`'s `float_semantic_twins` and
    `build_runtime_float_semantics_twins_agree` report an unresolved Operator
    selection for the outer `==` in `(nan32 == nan32) == false`.
    In finalization's Binary arm, all three resolvers decline:
    `checked_generic_operator_target`, `checked_operator_target_for_occurrence`
    and the `typed_operator_has_no_authored_selection` builtin fallback.
    Reduced nested comparisons over `bool`, `i32`, and `f32` check; retain
    the full package/customer context rather than a shape-specific repair.

  Acceptance: both customers check with every occurrence resolved to a real
  declaration or to an explicit builtin intrinsic, and an occurrence that
  genuinely selects nothing still rejects. Do not silence the finalization
  check or widen `allow_unresolved_toolchain` to cover authored source.

  The specialized-helper review's write-frame settlement failure belongs to
  **R5**, not declaration selection.

- **BORROWED-STORAGE-RESTORATION.** (split-of:OMEGA-PRODUCT-COMPILER-SOURCE)
  Complete consuming-transform/replacement execution under
  [borrowed-storage invariant windows](wiki/spec/language/ownership.md#borrowed-storage-invariant-windows)
  and [Terminal restoration](wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).
  The guide move-out/restore body already produces checked Move/Store rows and
  verified Terminal operations, including reference-local aliases. Reuse
  `execution/terminal_unit/borrowed_windows.rs`, emission's `BorrowedWindowLedger`
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
    `folded_index_projection::folded_index_faces_the_literal_lowering_boundary`
    is red since `c37c3dbd91`: the literal `items[1]` now gets a checked plan
    and stops at "fixed array literal has no Terminal establishment", while
    the folded `items[0 + 1]` still has no source-independent checked scalar
    control plan. The folded index needs the same checked plan in
    typed-trees-to-checked-trees before the pair meets one boundary again.
  - Complete native shared-reference joins beyond plain shared referents.
    Shared joins over primitive scalars and plain records
    (`abstract_operations::control_flow::address_joins`) lower, replay and
    execute natively: both arms of
    `borrowed_results::{PRIMITIVE,RECORD}_CALL_SOURCE` run on macOS ARM64
    and publish on four targets
    (`mbx nextest run -p omega-native-differential-test --test shared_borrow_joins`).
    Remaining: exclusive and write-only joins (coordinate with
    **WRITE-ONLY-BORROW**), referents that carry references or descriptors,
    reads and projections through a join place, primitive-local origins, and
    loan ends at the view's last use (Omega currently pins origins over the
    join's whole dominated region). Separately, legalization's
    single-provenance `branch_edges` rejects conditional edges threaded by
    combined Psi rewrites, including scalar-only matches.
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
    `checked-trees-to-lowered-psi/tests/call_premise_terminal_boundary.rs` pins
    `covered_requires`'s unrepresentable clause and its rejection in
    `scalar_graph_lowering/graph_preparation.rs`. Replace those limitation
    expectations with canonical reload, independent verification, interpretation,
    and native agreement; retain the same-program clause-free control.
  - Extend call-premise formation across remaining contract owners and
    substitutions: abstract signature declarations, domain/default predicates,
    static callable/evidence arguments, receiver/projection terms and
    citation/induction case guarantees. Abstract **callee** attribution already
    exists in `specification_calls.rs::RequirementOwner::Signature`; that does
    not validate every call occurring **inside** an abstract contract.
    `specification_calls.rs::arguments_select_plainly` excludes evidence
    arguments, and its requirement loop handles `ProofFact::Expression` but
    not `ProofFact::Proposition`. Use `package-evidence`'s
    `contract_expressions::evidence_calls` customer, including
    `requires p: carries(value) requires observes(value; p)`.
    Retain the encoding contract's evidence-argument coordinates: input position,
    callee proposition, source evidence term, and instantiated proposition.
    Establish premises before result formation or guarantee intake, including
    erased/discarded uses; recursion separately requires descent. Preserve
    exact subject/argument identity, complete constructor values versus tag-only
    evidence, and public-contract boundaries. Unsupported evidence is not proof.
  - Complete selected token-body supply outside the admitted expression routes,
    including open-ended ranges and match-pattern equality. Reproduce the
    build-machine refusal before assigning its repair. General structural/domain/
    case-pattern admission belongs to **MATCH-SELECTIVE-LOWERING**. Preserve
    [Equatable's sealed type-owned route](wiki/spec/language/conformances.md):
    visible crowned `==` declarations cannot override it. Check the declaration
    admission of `machine == Card::same(..)` against that rule without inventing
    a competing equality route. Reuse ordinary calls,
    attached receiver loans and once-only operand evaluation; no separate
    operator interpreter or fallback arithmetic. These refusals are
    implementation boundaries, not new semantic prohibitions.
  - Finish per-law normalization coverage atop the explicit selected-conformance
    route in `open_index_expressions.rs`. Preserve separate commutativity and
    associativity permissions in `OpenIndexOperationSelection`, with
    `validation/tests/open_index_algebra_laws.rs` and `type_identity/tests.rs`
    controls. Distributivity and zero/one identities need their own slots and
    consumers. Carry per-law normalization into semantic domain identity through
    `monomorphization::refresh_closed_domain_instance_identities`, with a
    source-rooted fixture rather than only validation-level tests.
    Preserve missing/ambiguous selection, wrong-operation and unproved-law
    controls. Noncommutative operations remain usable without rewrites; AC
    grants neither zero identity nor integer-addition meaning.
  - Preserve `token_bound_machine_operand_selection_exit_canary_runs` as the
    native token/named-call control. `expressions/declared_operator_match_result`
    separately checks three entries (`select_true`, `select_false`, `by_name`);
    its lack of a selected native entry is not a compiler lowering gap.
  - Migrate library/corpus/embedded-test declarations and remove the old parser
    and representation consumers. Use the
    [retirement inventory](wiki/drafts/audits/operator_introducer_retirement_inventory.md)
    for navigation, not as a current census. Tokenless boundary rows depend on
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS**, including generic requirements.
    Trait and domain token signatures already accept ordinary `machine` grammar;
    preserve `validation/tests/trait_token_heads.rs` and parser diagnostics
    while removing the legacy `operator` head. Rejoin
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
    attachments, indexed constraints, and operator homes. Reuse declared-domain
    `CaseMembership` tag tests and exact symbol-based multiplicity matching.
    Preserve exact lexical/package selection
    through specialization, same-leaf competitors, file-local imports and
    private/transitive exposure.
    `module_namespace_residuals` contains resolver-only operator/case tests;
    extend those customers through their remaining consumers rather than adding
    another namespace recognizer.
  - **Design-blocked** on [`unmanaged-root-package-identity`](OWNER_QUESTIONS.md).
    `control_flow/guarded_leaf_branch_expansion` and
    `text/runtime_alias_string_write` expose collisions between unmanaged
    `[u8; N]::Utf8` and injected std declarations: `package_identity: None`
    drops the distinct source owners from the legacy domain key. Reproduce
    through the rooted native-host route; targetless checking injects no std
    and misses the collision. Keep independent collision rejection; host paths
    and source order are not portable owner identities.
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

    Preserve the reported invalid-control reproduction: with
    `data Index<Limit: u32> [copy] where value < Limit, { value: u32; }`,
    an `Index<9>` holding 8 must not pass as an `Index<7>` argument or field.
    Literal-subject applications need distinct identity; runtime subjects need
    a checked compatibility relationship. Repair type identity, not merely the
    expectation, and coordinate adjacent `monomorphization`/`values/` work
    with **FINITE-GENERIC-DISPATCH**.
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
    binding establishment with **ENTRY-CONTENT-ROOTS**. Their native test
    module runs only on macOS ARM64; report unavailable hosts separately.

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
  - Complete package selection and generic-carrier domain identity; generic
    heads still use a data-only identity route. Reuse `DomainHead`'s common
    selection for `u64::AtMost<256>` and `u64 in AtMost<256>`, preserving
    equivalent-application deduplication and distinct-index separation.
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

  - In `execution/terminal_unit/dynamic_scalar_calls/`, replace the static-only
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
    Unverified candidates: `swarm/macw5-finite-dispatch` and
    `swarm/macw6-finite-dispatch` (`54a1609a64`) contain roster-membership and
    forwarded-call work. Reconcile both with the existing checked dynamic plan's
    binding kind and result shape before adopting either.

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
  establish source admission. Review the unverified candidate
  `swarm/macw7-domain-issuer-routes` (`b38ff7561a`) before duplicating that work.

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

  A direct-syscall-transport blueprint is designed and partially implemented:
  reuse `CallingPolicy::LinuxSyscallX86_64`/`LinuxSyscallAarch64`, demand-driven
  provider-plan settlement for boundaries with an actual `BoundaryCall`,
  per-arity syscall constraint rows, `DirectSyscallRealization`, and
  `AdmittedBoundaryExecution::ToolchainSettled` through lowering and
  provider-plan evidence binding. Resume branch:
  `swarm/macw8b-terminal-authority` at `7f856f6d43` (20 files, +496/−60 —
  provider settlement + boundary lowering slice, interrupted mid-work;
  unvalidated).

  Remaining delivery:

  - Connect the bounded Linux filesystem plan to native settlement transport.
    `build-evaluation/src/provider_settlement/canonical_filesystem_host.rs`
    already mints accepted-schema plans for supported positional syscalls.
    `compiler/tests/terminal_authority/filesystem_cohort_witness.rs` calls
    `set_len` and now pins `MissingBoundarySettlement`, not zero provider rows.
    Complete that native realization path without guessing unsupported target,
    path/slice adaptation or host-error mechanisms. General provider planning
    is not the missing implementation for this witness.
    Preserve settled-provenance replay in
    `packages/review/evidence/src/capture/providers/policy/replay.rs`, including
    changed settlement identity, authored-candidate substitution, and unsupported
    demanded-leaf rejection. Do not exempt ordinary `UniqueCoveringCandidate` plans.
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

  - Distinguish conservative ownership ceilings from derived complete frames
    during `execution/selected_execution.rs::refresh_settled_state_write_frames`.
    `package-evidence`'s
    `operators::boundary_selection::review_projects_application_closed_inside_specialized_generic_helper`
    reports a retained `["$P0", "self"]` ceiling versus settled `[]` after
    a helper calls a bodyless boundary. Allow justified refinement without
    silently dropping a derived write. Preserve the unknown-place sentinel
    even in free machines and the by-value ceiling; retain
    `unresolved_no_argument_call_preserves_an_unpassed_parameter` as a control.

  - Extend typed-tree `CallFrameResolver` coverage to full source checking for
    contextual-case, named-state, aggregate-result, and nested-helper composition.
    Reuse `exclusive_reference_origins`, `transparent_call_result_origins`,
    `write_frame_candidate_origins.rs`, and `validation/tests/wire_codec_write_frames.rs`.
    Proven candidate unions must survive receivers, codec arguments and rebinds.
    `carried_reference_origin` admits declared reference leaves of owned storage,
    not arbitrary loads through borrowed carriers. Keep unknown callees, private
    escapes, unproved rebinds, unsupported recursive results, and loads without
    independent origin evidence opaque.

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
  In `progress/origins/tests.rs`, close the finite-alternative gaps in
  `control_flow_route_helper_result_stays_unproven` and
  `dynamic_index_carrier_argument_stays_unproven`. Resolve demanded values
  through guarded prefix routes in `call_result_place`, not just terminal arms.
  Carry the complete subject set through `origins.rs::at_call`,
  `machine_summaries.rs`, and `lineage/transfers.rs` into the existing
  `ParameterLineage::Exact` representation. Every route must be exact; one
  unknown route leaves the whole result unproven.

  Preserve `embedded_call_written_on_the_demanded_path_has_no_exact_origin`
  as a negative: `poke_then_read` writes the demanded carrier before reading
  it, so its result cannot inherit the earlier subject.
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

- **NOMINAL-FIELD-FLOW.** Remove the receiver-specific default-domain exception
  under [ordinary mutable-receiver contracts](wiki/spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains)
  and close the dungeon checking customer. `&mut self` is an ordinary `&mut T`
  argument; this is implementation cleanup, not an owner-blocked design choice.
  Owners: `typed-trees-to-checked-trees/src/semantic/facts/field_domains.rs`,
  `checks/contracts/`, and `flow/call_phases/referents.rs`.

  Consolidate receiver and named mutable-argument domain obligations across
  call admission, entry assumptions, return checks, and fact handback. Remove
  receiver-only ZII filtering and superseded special cases rather than adding
  stronger facts alongside the old path. Keep ZII seeding for actual zeroed
  storage establishment; never infer incoming value validity from allocation.
  Reuse nominal-input and whole-extent collection-field checking. Update comments
  and tests that encode the receiver exception without removing real construction,
  invalidation, or restoration controls.

  Add paired method/free-machine regressions over the same `&mut T`: established
  non-ZII field domains survive valid mutation and return; invalid incoming values,
  unclosed invariant windows, and stale facts after writes reject. Exercise nested
  fields and explicit receiver `requires`/`ensures`; stronger input-only domains
  are not implicit postconditions. The tests must expose the old discrepancy and
  pass through the shared path, not a receiver-only compatibility branch.

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

  Acceptance: equivalent receiver/argument programs have the same checked domain
  obligations, the superseded receiver path is removed, and the unchanged dungeon
  command passes without bypassing field validity. Preserve indexing/view/copy/call transport and finite
  candidate-origin coverage in `tests/contracts/{element_fields,nominal_parameter_fields}.rs`;
  corrupted elements and stale aliases reject at calls, transitions and returns.
  Nominal annotations never resurrect retired facts, unresolved selectors
  never represent universal coverage, and arbitrary incoming storage gains
  no ZII facts. Preserve negative controls while migrating revoked syntax.

- **CML4.** Complete edge-local residual cleanup under the
  [ownership contract](wiki/spec/terminal-psi/ownership.md): outgoing values
  materialize before transfer and disposal, every occurrence has one
  disposition, and independent dying roots clean in reverse declaration order.
  Abandoned partial construction/staging cleans its established prefix in
  reverse establishment order, as the [language rule](wiki/spec/language/ownership.md#construction-and-disposal-order) requires.
  Crash/abort/process-exit abandonment has no cleanup successor.
  Owners: `typed-trees-to-checked-trees/src/execution/control_cleanup.rs`
  and `execution/terminal_unit/cleanup/`, `checked-trees-to-lowered-psi/src/unit/unit_cleanup/`,
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
  - Extend independently reconstructed residual scheduling to construction-local
    roots, partial construction, a second partially moved parameter root, and
    mixed dying roots beyond the supported parameter/temporary pair. Reuse
    `execution/terminal_unit/cleanup/anonymous.rs::append_continuation` and
    the partial-affine carrier's exact complements; keep machine-scoped
    parameter custody distinct from continuation-edge cleanup. No runtime
    liveness flags, expansion of untouched arrays into leaves, or cleanup
    deferred until final return.
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
  preserve values and observable hook order/timing across the composed edges.
  Frontend admission or diagnostic ordering is not that runtime witness. Reject missing,
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
  **CML4** owns residual cleanup. Start at the actual omission in
  `typed-trees-to-checked-trees/src/execution/terminal_unit/`, not a historical
  failure histogram. Preserve record-pattern and fresh record/case operand
  support rather than recreating it.

  `terminal-vocabulary-for-unit-bodies` is delegated implementation work:
  extend or add operations for non-vacated structural replacement, typed recast
  views, and fresh linear-record custody. This item owns integration with
  RECAST-SOURCE-POSITIONS and the existing ownership consumers through source,
  codec, independent verification, and native execution. Preserve bounds, layout,
  loans, and claim lifecycle; do not fake a move window or equate fresh linear
  custody with authority issuance. No owner approval of the IR shape is needed.

  Current source-to-native customers and gaps:
  - Execute `data/record_pattern_{let_exit,bind_all_exit,double_underscore_field}`
    and `control_flow/{arm_pattern_rest_optout_exit,case_pattern_rename_waive_exit,record_pattern_arm_rename_guard_exit}`
    through ordinary package review; native publication alone is not execution.
    Retain owned sum parameter slots for indirect/stack passing, including
    Windows x64's 12-byte `Msg`, and complete serviceful borrowed-call scalar ABI.
  - `state_graph/tests/case_payload_edges.rs` admits
    `OpenResult::Opened { info } -> have(info)` but lacks the `CasePayload`
    Terminal transfer in `state_graph/emission/state.rs`. Carry the exact
    tested payload through verification of `Root::run`.
  - Reconcile authored parameter positions with dense Terminal positions when
    `calls/signatures.rs` omits an unread reference `self`.
    `Random::next_u32(&mut self, state: &mut RandomState)` exposes mismatches in
    store planning and `ordinary_machine/stores.rs`, `state_graph/body.rs`,
    and `emission/structural_scalar_store.rs`. Preserve the authored-to-retained
    relation; admitting the planner alone does not repair lowering.
  - Complete store sources for float reads/arithmetic and policy casts,
    recast reference locals, case and qualified byte-slice literals, nested
    mutable-receiver calls, and local-rooted destinations. `primitive_store`
    must retain the referent behind a `self`-rooted alias;
    `computation_arguments::structural_computation_argument` must compose
    mutable calls without treating them as shared. Coordinate FLOAT-PROVIDERS,
    RECAST-SOURCE-POSITIONS and ARITHMETIC-POLICY-REALIZATION.
  - Add ordinary construction for record members typed `UInt`, `&[u8] in Utf8`,
    and primitive arrays such as `[u32; 4]`; top-level array locals are not
    record-field support. Aggregate replacement must keep the custody checks below.
  Remaining work:

  - Close two distinct receiving cases in
    `abstract-operations-to-target-operations`, exercised by
    `terminal-psi-to-abstract-operations` integration tests:
    `scalar_affine_cleanup::structural_return::omega_preserves_exact_singleton_structural_return_custody`
    reaches `lowering/control_flow.rs`'s rejection of entry claims and an
    observed Owned/Linear structural parameter. Preserve both authority
    premises rather than dropping either check to admit it.
    `partial_affine_call_results::continuations::source_continuations_retain_distinct_result_owners_and_ordered_residuals`
    passes entry admission with Owned/Affine parameters but reaches
    `UnsupportedStructuralArray`. Retain its distinct result owners and
    ordered residuals through target lowering and native execution.

  - Replace competing whole-machine plans with ordinary evaluation/control
    operations. Cover effectful state arguments/returns, dynamic and
    borrowed/projected storage, mixed
    scalar/structural operands and boundary consumers. Selected case edges
    retain the invocation receiver and local ownership. Preserve
    `transition_argument_call_result_derives_the_exact_entry_subject`,
    `jump_operand_mutation_cannot_replay_the_taken_guard` and
    `composed_unit_claims` controls. `machine_lowering/machine_dispatch.rs`
    still rejects simultaneous scalar/Unit dynamic joins. Preserve authored
    guard-operand order and `checks/contracts/guard_operands.rs`'s invalidation
    after an earlier operand's write; do not reintroduce synthesized arm states.
    Delete superseded shape producers as their operations compose; a failed
    custody rejoin must never fall back to a weaker recognizer.
  - Rejoin composed scalar calls with boundary callees and claim transfers
    before structural returns.
    `unit/attached_unit/composed_control/admission.rs::retain_scalar_call`
    still refuses that custody; structural callees with borrowed parameter
    actuals now compose
    (`owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel`
    passes: two borrowed-output writes precede the exact record return,
    finish at 73, and survive fuel exhaustion without replay). Extend
    ordinary ordered operations, not a fallback recognizer. Retain
    record/array/generic return-substitution and affine-transfer rejection
    controls.
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
    `execution/terminal_unit/structural_scalar_store/tests/record_literal_fields.rs`
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
    `typed-trees-to-checked-trees/src/execution/terminal_unit/control/checked_machine.rs`
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
  argument adaptation and immutable result aliases have checked forms.
  Reuse transport-backed `lift` argument maps (selection, permutation,
  repetition, omission), immutable result aliases, and finite-state forwarding
  through the `quotient-result-flow` codec and independent correspondence.
  `terminal_bridge.rs` still admits monomorphic,
  single-state, direct position-preserving rows elsewhere. Complete missing
  correspondence judgments and extend publication and independent
  reconstruction for result computation, preconditioned `define`, and
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
  `b7ea53ff92` settles shared-borrow `&self` requirements through a
  receiver-place forwarding row; `&mut`/write-only/qualified self, family
  rows, and external satisfiers stay closed. Reuse them without treating that
  result as native coverage of every shape.

  Extend the selected-call mechanism to type/lifetime-parameterized
  requirements and borrowed receivers. Lifetime-parameterized direct
  requirements (erased-lifetime telescopes such as `Bytes::head<'a>`) now
  settle through provider-plan requirement views and selected-dispatch
  adaptation, witnessed by the
  `providers/lifetime_boundary_requirement_dispatch_exit` canary;
  `selected-dispatch/boundary_dispatch.rs` still restricts direct requirements
  to nongeneric, receiver-free signatures otherwise, and
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
  arbitrary payload or inventing new literal syntax.

  Complete classification, conversion, directed-rounding and fused-operation
  const customers. `constant/initializer_dependencies.rs::Collector::call`
  admits Machine/State targets but rejects sealed float Operator declarations
  before evaluation. Connect exact selected-call admission to the existing
  `FloatSemantics` dispatch in the checked interpreter; do not add a float engine.
  Reuse `machine_execution/admission.rs::closure_needs_operator_selection`.
  Bodyless catalog contract discharge remains **FLOAT-PROVIDERS**' responsibility.

  `float_semantic_twins` and `build_runtime_float_semantics_twins_agree` first
  need **AUTHORED-SELECTION-FINALIZATION-GAPS**' imported nested-equality repair.
  Keep that earlier selection failure distinct from const-call admission and
  then run the full fixture through evaluation and retained replay.

  Acceptance: those source-authored constant customers evaluate and replay
  with exact selected custody; determined runtime results publish stable bits,
  while undetermined representations and forged receipts reject. A
  payloadless NaN may participate in admitted compile-time/proof reasoning
  without materializing. Retain signed-zero, format and imported-alias controls.
  Owners: build-time evaluation's constant expression, materialization and
  replay paths, with shared FloatSemantics rather than a separate arithmetic
  definition.

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

## Rust compiler release closure

The [completion contract](wiki/drafts/reference/rust_compiler_completion.md) requires
eight passing gates at one clean commit and the four matching-host runs.
The tasks below own evidence closure; capability repairs stay with their
implementation owners. Historical records are attribution leads, not current
baseline results. An unavailable runner, unexplained skip, filtered/empty run,
timeout or exhausted capacity leaves the affected gate open.
Retain applicable checked/cross-target coverage when a runner is unavailable,
but report the missing runtime leg explicitly; it does not close that host row.

- **RC-RELEASE-RECORD-AND-CLOSURE.** Finish the existing release recorder and
  assemble the complete same-commit evidence. Owners:
  `tools/release/release_record.py`, its tests, and the completion contract.
  Two committed JSON records under `tools/release/records/` contain only
  RC-PORTABLE-PSI gate passes and explicitly open closure; do not recreate the
  recorder or claim records are absent.

  The remaining deliverable is complete same-commit evidence, not another
  recorder implementation. Preserve bidirectional command-coverage checks in
  `tools.tests.test_release_record` and `tools.tests.test_release_matrix`,
  the portable formatting command, and required `--native-execution` input.

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
  [Failure attribution](wiki/drafts/measurements/known_baseline_failures.md) is dated
  starting evidence, not a reason to repeat fixed lint/fixture repairs or
  combine results across revisions/hosts. Acceptance: all five commands pass
  at the release commit, with failures and skips accounted for by exact test
  names rather than another chronology on this board.

- **RC-BUILD-AND-PACKAGES.** Close all three build/package command blocks in
  the [completion contract](wiki/drafts/reference/rust_compiler_completion.md#release-matrix):
  the seven-package nextest run, those packages' doctests, and the six compiler
  integration targets (`build_config_granted`, `build_log_facet`,
  `build_target_activation`, `checked_build_machine_identity`,
  `evaluated_via_binding`, `package_compilation_inputs`).
  Owners: build evaluation, package resolution/review/manager and compiler
  handoff. The remaining failure families are FMA transport, exact
  checked-call/review identity, provider-schema agreement and fixture
  rosters. Reproduce a named
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

  Package command fixtures: reproduce `fixture module path ends in ::fixture`
  in `named_workspace_install::cases`. Fix caller-specific child-test selection
  in `packages/manager/tests/support/named_workspace.rs` and exercise all four
  consumers, including `package_inspection`, `offline_package_commands`, and
  `source_diff_commands`.

  THE WHOLE `-p compiler` TEST SURFACE IS UNGATED, and a complete sweep says
  how much that has cost. `land_tight.sh` checks
  `--workspace --all-targets --exclude omega-native-differential-test --exclude
  compiler`, so nothing a landing runs ever executes these targets except
  `canary_suite`. Swept at `869021cf9c7` on macOS arm64, twelve targets at a
  time (an unscoped `-p compiler` links ~150 binaries and exhausts this host's
  disk): of the 110 targets outside `canary_suite`, `samples_compile` and
  `corpus_runner`, **23 carry 178 failing tests**.

  | n | target | n | target |
  | --- | --- | --- | --- |
  | 89 | `native_filesystem_canaries` | 3 | `service_operational_contracts` |
  | 17 | `recast_views` | 3 | `source_evaluated_native_realization` |
  | 10 | `runtime_value_generics` | 3 | `subslice_runtime_end_bounds` |
  | 9 | `plan_laid_repeated_runtime` | 2 | `callback_terminal_custody` |
  | 7 | `build_target_activation` | 2 | `terminal_authority` |
  | 7 | `optimizer_opt_in` | 1 | `build_named_inputs` |
  | 6 | `build_snapshot_outputs` | 1 | `callable_entry_custody` |
  | 4 | `private_joint_progress` | 1 | `joint_call_rankings` |
  | 3 | `layout_plans` | 1 | `literal_dispatch_unit_plan_stops` |
  | 3 | `module_machine_indices` | 1 | `no_selection_golden` |
  | 3 | `pcc_publication` | 1 | `object_artifact_custody` |
  | | | 1 | `object_container_custody` |

  `native_filesystem_canaries` is half the total and fails UNIFORMLY -- 89 of
  89, none passing -- so it is one cause, not eighty-nine. PEELED, by fixing
  each layer locally and reading the next diagnostic; the edits were reverted
  because only the third layer decides anything, and a half-migrated fixture
  set helps nobody.

  1. `public interface selects private domain [u8]::CString`
     (`validation/src/declarations/declaration_visibility.rs`). Not the
     `pub domain` at `source/library/std/macos_gui.omg:32` -- `pub domain`
     records `is_public = true`, verified directly on the lowered trees. The
     ELEVEN fixtures under `tests/omega/pass/objc/` declare their OWN
     `domain [u8]::CString` with no `pub`, and their public interfaces select
     that one. Marking them `pub` clears this layer.
  2. `machine Main::main publishes service reach Console but its checked body
     reaches undeclared service ObjectiveC`. Sixteen objc fixtures declare
     `reaches Console` while their bodies reach `ObjectiveC` too; the same
     under-declaration that stopped `composed_internal_unit_arguments`.
     Declaring `reaches Console + ObjectiveC` clears this layer.
  3. `selected ProgramEntry Binding field Main::objc requires a selected Fused
     provider for boundary ObjectiveC`. This is the real blocker, and it is
     the provider-absence family already recorded on this row for
     `Console`/`Filesystem`/`Input` on macOS -- not a fixture problem. Layers
     1 and 2 are worth landing WITH a provider, not before one, since nothing
     turns green until `ObjectiveC` has a selected Fused provider on
     macos_arm64.

  AND THE GAP HAS A NAMED SHAPE. The canonical toolchain provider-settlement
  mechanism exists and is the right one -- `build-evaluation`'s
  `provider_settlement/` mints a plan per selected target from a reviewed
  table, joined through `with_toolchain_settled_plan` -- but it covers exactly
  TWO boundaries (`canonical_filesystem_host.rs`, `canonical_time_host.rs`)
  and each maps exactly ONE target:

      fn toolchain_*_leaves(target_name) { "linux_x86_64" => ..., _ => &[] }

  There is no settlement at all for `Console`, `ObjectiveC`, `Gui`, `Input`,
  `Clock` or `Disk`, and none for macos_arm64 on the two that exist. So every
  macOS host boundary fails the same way by construction, which is why 89
  canaries fail uniformly.

  Adding darwin rows is not filling in a table, and the reason is sharper than
  "no representation exists". Both leaf types are `{ method: &str, number: i64 }`
  -- a SYSCALL NUMBER -- and darwin's realizations are dylib symbol imports,
  several needing an injected constant argument (`_clock_gettime_nsec_np` with
  a clockid, `objc_msgSend` with a selector).
  `ProviderBinding::Import` DOES already exist, so do not go looking for a
  missing variant. It carries an `EvaluatedForeignImport`, and that type will
  only construct `from_retained_evidence` when an `EvaluatedBindingReceipt`
  COMMITS to the locator it names -- evidence produced by evaluating an
  authored binding machine, exactly as `providers/external_leaf_dllimport_compile`
  does with its `leaf_binding()`. A toolchain settlement table has no authored
  machine and therefore no receipt, which is the same honesty rule these
  modules state for themselves: they mint only what they can name honestly and
  leave the rest uncovered. So the question is not which enum to add but where
  a toolchain-settled import's receipt comes from -- an owner decision about
  what authorizes a compiler-minted foreign binding. The three per-target time
  constants are a separate hole: they carry no kernel mechanism at all, and
  `canonical_time_host.rs` says they "stay uncovered until a constant binding
  kind exists".

  That single missing representation is the root of: these 89
  `native_filesystem_canaries`, the 9 `Binding field` corpus members,
  `time/runtime_time_host_native_darwin_exit`, and the macOS half of the
  release-gate targets on this row. `e76de8e15f2` (2026-09-19) did not cause
  any of it -- it replaced the zero-attachment symptom with a diagnosis naming
  the missing selection, and said so.

  So the 89 are one gap, and it is the same gap the 9 `Binding field` corpus
  members name. Fixing provider selection for these host boundaries is worth
  far more than its row currently suggests: it is 89 tests here plus 9 in the
  corpus.

  `build_named_inputs`, `build_snapshot_outputs` and `build_target_activation`
  are release-gate commands on this very row, so 14 of these sit directly
  under release closure.

- **RC-PCC-REPLAY.** Close the release gate for artifact/`.proof` pairs:
  round-trip valid evidence, reject hostile/substituted evidence before
  PCC-required interpretation or lowering, and keep ordinary non-PCC output
  independently checked. Run nextest and doctests for
  `checked-trees-to-lowered-psi`, `terminal-codec`, `terminal-verifier`,
  `terminal-interpreter` and `terminal-psi-to-abstract-operations` as specified
  by the completion contract. The block is red and includes a terminated
  proof-search case; a harness returning after killing a test is not
  successful termination of that test.
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

RC-SOURCE-SEMANTICS includes compiler integration targets outside `canary_suite`.
Use bounded batches during repair when linking all targets exceeds host capacity;
release acceptance still requires complete coverage. Remaining reported failures:

- `no_selection_golden::native_artifacts::retained_native_bytes_and_metadata_match_every_target_golden`:
  identify the semantic/proof-section drift before updating goldens; verify
  retained bytes, metadata and repeatability on every hosted target.
- `module_machine_indices::indexed_domains::qualified_record_projection_rejects_unproven_constructor_membership`:
  distinguish disproved membership from failure to prove it.
- `module_machine_indices::indexed_domains::separate_packages_cannot_exchange_mutable_domain_qualifications`:
  restore a valid initializer so the negative reaches mutable-recast checking.
- `module_machine_indices::rational::anonymous_comparisons_cannot_hide_undefined_values_or_landed_operands`:
  reconcile finite float comparison acceptance with the numeric contract while
  preserving undefined-value and landed-operand rejection.

- **RC-NATIVE-MATRIX-LINUX-X86-64.** Execute the complete native, source and
  sample gates above on Linux x86-64 and resolve remaining failures through
  their owning capability tasks.

  One attributed defect blocks emitted execution. `9e20e91559` emits an
  unconditional `DT_RELA` plus `DT_RELASZ=0` pair for the empty general
  relocation table but never `DT_RELAENT` (tag 9, `sizeof(Elf64_Rela)` = 24),
  and glibc dereferences `l_info[DT_RELAENT]` whenever `DT_RELA` is present,
  so the image faults in `ld-linux-x86-64.so.2` before `_start`. Masking the
  `DT_RELA` row in the emitted bytes makes the same image exit 70, which
  isolates it. `ElfDynamicTag`
  (`image-elf/src/dynamic_executable/dynamic_table/dynamic_tags.rs`) skips
  from tag 8 to tag 10; emit the entry size beside the pair, or emit neither
  when the table is empty. Verification needs a Linux host.

  **SAMPLE-CORPUS** owns current shared sample failures;
  **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** owns the canonical filesystem plan's
  package-review replay gap. Attribute failures against current code rather
  than treating a historical host record as a fresh reproduction.
  Acceptance: actual emitted ELF execution and all required full commands
  pass, not merely a selected ABI cohort.

- **RC-NATIVE-MATRIX-LINUX-ARM64.** Supply the same complete gate evidence
  for emitted AArch64 ELF programs. A Linux AArch64 runner or named/versioned
  emulation may satisfy the contract, but must execute the target runtime
  legs: an x86-64-built harness that cfg-excludes them does not.
  Cross-target checks plus a single `cli_mvp` QEMU execution do not close
  the full row. Acceptance: required native/source/sample gates pass with
  actual AArch64 execution and explicit, justified skips.
  Include Linux host, time/filesystem and `IntegerAt` projection/write paths;
  checked-target fixtures and cross-emission do not establish runtime behavior.

- **RC-NATIVE-MATRIX-MACOS-ARM64.** Run the complete native/source/sample
  gates on macOS AArch64, executing emitted Mach-O products. Procedure and
  cross-target coverage do not establish a completed matching-host run. Use
  the recorder procedure in `tools/release/README.md`; it takes no `--all`.
  Acceptance: same-commit full-gate results with native observations and exact
  expected skips; Linux emission evidence cannot close this task.

  All three gates are now measured on the matching host.
  `omega-native-differential-test --all-targets` at `0e64720850`: 1158 run,
  1140 passed, 18 failed, 1 skipped (2498.6 s). `-p compiler --all-targets`
  (RC-SOURCE-SEMANTICS) at `04d2099ae1`: 3281 run, 2014 passed, 1267 failed,
  0 skipped (22583.2 s) — the `samples_compile` leg inside it
  (RC-REPRESENTATIVE-PROGRAMS) read 33 run, 11 passed, 22 failed. The full
  1215-name failure list and dominant diagnostic classes are in
  [macos_arm64_rc_gates](wiki/drafts/measurements/macos_arm64_rc_gates.md):
  ProgramEntry/Terminal-attachment rejoins, `runtime policy realization`
  lowering gaps (ARITHMETIC-POLICY-REALIZATION), and domain-admission gaps
  (REMOVE-BRACKETED-RANGE-ANNOTATIONS). No `macos_arm64` record exists under
  `tools/release/records/` — the recorder writes a lane only with a passing
  `--native-execution` observation, so the row stays open on this evidence.

  Use [known baseline failures](wiki/drafts/measurements/known_baseline_failures.md)
  for prior failing names and expected skips, then reproduce at the release
  revision. Partial native/canary runs do not establish complete source/sample
  coverage or general Mach-O correctness. Route missing Unit plans to
  **STATE-LOCAL-VALUE-FRONTIER** while retaining independent host validation.
  Compare `const_fold_unsigned_shift_right_arg` across macOS and Linux when
  attributing the recorded cross-host exit difference.

- **RC-NATIVE-MATRIX-WINDOWS-X64.** Run the complete native/source/sample
  gates on Windows x86-64, executing emitted PE products, including the
  hosted receiver. Procedure evidence is not an executed release row. Use
  the recorder procedure in `tools/release/README.md`.
  Acceptance: same-commit full-gate results with native observations and exact
  expected skips; cross-emission or compiler-only success is insufficient.
  Include `windows_set_file_time_exit_canary_runs`: SetFileTime followed by
  `_stat64` must read back Unix time 1,500,000,000 and exit 70. Its corrected
  unsigned byte assembly is not evidence that Windows execution passed.


- **MACOS-X64-HOST-PROFILE.** Execute the compiler/runtime route on an Intel
  macOS host. Target/provider recognition, exact physical-entry reconstruction,
  accepted package binding, hosted receiver and x86-64 Mach-O import pairing
  have implementations; cross-target checks do not establish host execution.
  Exercise emitted Mach-O entry/exit, receiver and provider/import behavior,
  recording exact commands, commit, observations and justified skips. Attribute
  failures to their capability owners. This additional platform task is separate
  from the completion contract's four required release hosts and does not depend
  on an Alpha bootstrap seed.

  Reproduce the [host record](wiki/drafts/measurements/macos_x64_host_profile.md)'s
  two admission gaps: unselected `ConsoleNativeProvider::exit_process` and an
  unbound `macos_x86_64::ProgramEntry` root. Complete provider settlement and
  root binding before claiming emitted entry/exit, receiver, or import execution.

## Omega-written compiler (after Rust completion)

Finish the Rust [completion contract](wiki/drafts/reference/rust_compiler_completion.md)
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
