# Code Repair Queue

Agents repair obvious code-quality and architecture defects without waiting for
owner sign-off. This queue records concrete structural problems outside the
current repair so another agent can take them through validation and landing.
An entry is neither a new language rule nor a reason to stop the current work.

Use [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md) only for genuinely unsettled language
or high-level architecture choices. Apply the existing
[architecture rules](AGENTS.md#conventions-and-workflow) directly; cross-cutting
scope, conflicting edits and engineering difficulty do not require a decision.
Plain missing functionality belongs in the existing outcome in [TASKS.md](TASKS.md),
not here. A fail-closed "not implemented yet" is not an unsound acceptance.

## How to add one

Add an entry when something would mislead the next agent, or when fixing it
properly reaches beyond the crate you are in. An ordinary bug, a style
preference, or anything you can repair inside your own task does not belong
here: just fix it.

The best moment is your first hour in an area, before you have a change of your
own to defend. Name the defective mechanism and its cost, not a speculative
framework or a list of features that remain unimplemented.

- Add the entry at the top of the list under "Entries"; the list is flat, newest first.
- Name one category from the list below on its `Category` line.
- Keep it to the lines of the template. Link the file and line.
- Add it in the pull request you are already landing; never open one for it.
- Reconcile overlapping reports by the remaining defect; do not preserve duplicates.
- Any repairing agent removes the entry when the fix lands. Remove stale reports
  after verifying that the offending code or premise is gone; do not keep a
  completion log or mistake deleting a report for implementing its feature.

```markdown
### <one line naming the concern>
- Category: <one of the ten below>
- Where: <path:line, crate, or tool>
- Saw: <what is there, concretely>
- Cost: <who it misleads or what it makes expensive>
- Reported: <date>, <agent>, while <what you were doing>
```

## Categories

**Structure**

1. **One idea, several implementations.** Your fix had to be applied in more
   than one place to work.
2. **Knowledge crossing a boundary.** A stage imports something that is not its
   input representation, its output representation or a foundation crate.
3. **Branch per shape.** Logic keyed on where a value sits (local, field,
   parameter, match arm) instead of what it is.
4. **Re-deriving.** A later stage recomputes what an earlier one knew, or a
   stage checks its own output again.
5. **Too big to read.** You had to search the file because you could not read
   it.

**Honesty**

6. **A verdict with nothing behind it.** Something is accepted by default, by
   fallback, or because a lookup came back empty.
7. **Tests that pin the bug.** A fixture that would still pass if the feature
   were broken, or a check that quietly stopped running.
8. **Zombie machinery.** Code only tests call, a retired path still being fed,
   a fallback nobody can remove.

**Direction**

9. **The documents disagree.** You can cite a rule for either answer, or a
   document blesses the thing that is rotting.
10. **Verifying costs more than writing.** You waited longer than you worked.

## Entries

### `Build::bind` accepts a shared `&Build`, where `Build::application` needs `&mut Build`
- Category: a verdict with nothing behind it
- Where: `samples/cli/facts/build_activation_borrow/invalid_*`; Build protocol `BuildBindingOperations::prepare(builder: &Build)` in [omega-rust/omega/packages/build-declarations/src/prelude.rs:116](omega-rust/omega/packages/build-declarations/src/prelude.rs#L116)
- Saw: `machine poke(builder: &Build) { Build::bind(builder, linux_x86_64::ProgramEntry, main); }` is accepted, while `Build::application` through `&Build` is rejected for its mutable receiver. The shared-reference controls of `build_activation_borrow` (`invalid_field`, `invalid_parameter`, `invalid_call_result`, `invalid_other_state`, `invalid_other_machine`, `invalid_target_body`) therefore run their Builds, binding through a stored `&Build`, and stop only in their products. The sample README expects binding to need `&mut Build`; no spec page states which access `Build::bind` needs
- Cost: those six controls cannot show their intended rejection; `Build::bind`'s receiver access needs a spec sentence and then the matching protocol signature
- Reported: 2026-11-14, Claude Code (worker-14), while making the Build hold its lent reference in a record

### Parsing and native completion choose their worker budgets independently
- Category: one idea, several implementations
- Where: [parser worker pool](omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs#L113), [native executor queue](omega-rust/omega/build-system/orchestration/src/compile/native_completion/queue.rs#L10), [prepared machine lowering loop](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lib.rs#L862)
- Saw: parsing owns a separately configured Rayon pool; native completion owns up to four executors, including its caller. Neither receives an invocation-wide worker or live-memory budget. This establishes separate admission policies, not measured overlap or oversubscription. Psi 05 already prepares shared signatures, outcomes and records before each `lower::machine` returns its own `Lowered` output.
- Cost: Separate limits cannot bound the invocation's combined active work or live intermediates. Share CPU and live-work admission across source and native jobs; queue lightweight keys, materialize scratch after admission and distinguish estimated memory reservations from enforced limits. After removing repeated work, trial the measured expensive boundary at Psi 05 or post-inference Psi 06, keeping inner walks sequential. Preserve deterministic IDs, assembly and refusal order. Closure requires bounded nested execution and a measured elapsed-time benefit without unacceptable small-input or memory regression. See the [draft execution admission design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-14, Codex, while comparing Squalr-inspired pipeline proposals at `c445e89a1eead`; threading placement is a proposed experiment, not a measured speedup

### Logical-module discovery discards a full parse that source admission repeats
- Category: re-deriving
- Where: [logical inventory parsing](omega-rust/omega/packages/package-compilation/src/source/source_imports/logical.rs#L185), [inventory output](omega-rust/omega/packages/package-compilation/src/source/source_imports/logical.rs#L211), [selected-source parsing](omega-rust/omega/packages/package-compilation/src/source/source_assembly.rs#L949)
- Saw: discovery runs the ordinary lexer/parser over the owner's eligible captured files, retains only each path and module name, and drops the syntax. Selected files are then parsed again after admission; source reselection also parses under fresh consumer coordinates. Those fresh coordinates protect source ownership and selection ledgers, but the syntax work is repeated over unchanged bytes.
- Cost: Selected files pay for syntax construction twice. Retain immutable syntax and a module/import index within the invocation, separately from semantic owner, scope, stratum and bindings. Pin parse counts, distinct Build/Product identities, changed-input behavior and diagnostic locations. Release unused discovery syntax when ownership permits and measure retained memory against avoided parsing; do not add a persistent cache. See the [draft syntax retention design](wiki/drafts/designs/compiler_work_ownership.md#actionable-repair-references).
- Reported: 2026-11-14, Codex, while comparing Squalr-inspired pipeline proposals at `c445e89a1eead`

### Call-component discovery builds all-pairs reachability to answer component questions
- Category: re-deriving
- Where: [call-component discovery](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/components.rs#L69), [component consumer](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower.rs#L117)
- Saw: `components` allocates an N-by-N Boolean reachability table and walks the call graph from every machine. It then constructs the same component-member vector for each member of a cycle. Consumers need component membership and whether a machine can reach a cycle, not arbitrary all-pairs paths.
- Cost: Sparse graphs pay quadratic reachability storage and repeated traversals. Compute strongly connected components once, keep one ordered member span per component and a dense machine-to-component table, then propagate cycle reachability on the component DAG. Preserve refusal order, terminal-call restrictions and termination checks; exercise chains, self/mutual cycles and callers reaching cycles, and count graph work independently of required emitted code. See the [draft call graph design](wiki/drafts/designs/compiler_work_ownership.md#actionable-repair-references).
- Reported: 2026-11-14, Codex, while comparing Squalr-inspired pipeline proposals at `c445e89a1eead`

### Completing selected bodies can copy the retained typed program
- Category: re-deriving
- Where: [retained-body completion](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L810), [shared typing loop](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1596), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L624)
- Saw: `SeededTypingBase` holds `Arc<TypedTrees>`, but body completion calls `Arc::unwrap_or_clone(retained.typed)` and feeds the candidate to one mutable `Lowerer`. When another snapshot retains that Arc, cloning copies owned arena vectors even if only a small selected body set changes. Sharing the outer Arc does not make the stored tables independently owned.
- Cost: A small continuation can copy unrelated typed storage and cannot return independent body output. Separate immutable declaration/type storage from owned body tables; publish selected replacements through stable handles without flattening unrelated bodies. A shared-snapshot continuation must retain untouched storage, exact authority/context identities and refusal behavior. Measure copied bytes and peak memory; keep signature and inferred-summary dependencies explicit. See the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-14, Codex, while comparing Squalr-inspired pipeline proposals at `c445e89a1eead`

### Each machine check searches the complete branch and carried-proof lists
- Category: re-deriving
- Where: [per-machine discharge inputs](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/lib.rs#L417)
- Saw: after shared summary/input inference, discharge collects every authored branch, then filters and allocates that list separately for each machine. It also searches `carried` from the beginning for each specialization. The producer already supplies the machine identity on each row.
- Cost: Each local check repeatedly scans other machines' metadata. Group branches into stable machine ranges and prepare carried-proof associations once, sized to the actual domain; borrow slices in each check. Preserve order, absence behavior, association meaning and kernel admission. Pin cross-machine isolation and empty groups and count preparation visits as machines grow. Final machine checks are a parallelism candidate only after inference inputs settle. See the [draft checking inputs design](wiki/drafts/designs/compiler_work_ownership.md#experiments-and-acceptance).
- Reported: 2026-11-14, Codex, while comparing Squalr-inspired pipeline proposals at `c445e89a1eead`

### The standard library declares two public things named `DarwinClock`
- Category: one idea, several implementations
- Where: [source/library/std/macos_gui.omg:586](source/library/std/macos_gui.omg#L586) (`public boundary trait DarwinClock`) and [source/library/std/targets/macos_arm64/clock_impl.omg:26](source/library/std/targets/macos_arm64/clock_impl.omg#L26) (`public DarwinClock satisfies Clock`)
- Saw: `use omega_language_std::DarwinClock;` is refused as selecting no one declaration (modules.md, "Resolution": "Ambiguous imported declarations reject"). Five GUI samples (`samples/gui/window_app`, `image_viewer`, ...) write `invokes DarwinClock` and cannot name it; std's own Build names both meanings unqualified (`Build::select_provider<Clock, DarwinClock>` and `Build::bind<DarwinClock::clock_ns>`)
- Cost: no consumer can import either declaration, and which one an unqualified `DarwinClock` means inside std depends on the operand's role. One of the two needs another name
- Reported: 2026-11-14, Claude Code, while moving samples to declaration imports

### A provider default may still name a data type, which the spec says is not a provider value
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/build-time-evaluation/src/product.rs` `ProviderForm::DefaultClosure` (from #3015) and its test `a_provider_default_may_name_the_data_type_that_owns_its_closure`
- Saw: `form` refuses a data type in a Build's choice but accepts one in a `provider_defaults` machine. provider_selection.md, "Declaration and choice", says a target package "supplies ordinary named default provider implementations" and "A data type or runtime provider descriptor is not such a value", with no exception for defaults. `std`'s ProcessExit defaults, its only user, now select the named `NativeProcessExit`; the last user anywhere is `samples/cli/probes/device_extent_access` (`LoopbackDeviceLoanProvider`), which is written in other retired forms too (`via`, target-prefixed machines)
- Cost: a second, looser reading of the Provider position stays served for no library; delete `DefaultClosure` and the `ProviderForm` parameter, turn the test into the refusal twin, and give the probe a named implementation
- Reported: 2026-11-14, worker-15 (Claude Code), while moving std's ProcessExit defaults to a named implementation

### Const evaluation still keys integer carriers by spelling and folds in i128
- Category: branch per shape
- Where: Stage 02's `preparation_const_evaluation.rs`
- Saw: the const evaluator keys integer carriers by type-name strings and folds in `i128`, so it serves widths to 64 bits for shifts and 127 for ranges
- Cost: constant evaluation discards the authoritative integer metadata and narrows otherwise valid width-indexed arithmetic to host-integer limits
- Reported: 2026-11-13, Claude Code, while replacing the eight integer names with a sign and a width

### The lowered form drops declared domains, so only the reader knows a value's type
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/lower.rs` and `policy.rs` (`qualifications: ScalarQualificationSetId::ZERO`); `terminal-psi/src/types_and_values.rs` (`ScalarQualificationCatalog`, `ScalarDomainDeclaration`, `ScalarQualificationCoercion`)
- Saw: Terminal Psi has a catalog for scalar domains, their predicates and the edge where a value is coerced into one, and the lowered route fills none of it. Which domains a value carries, and who may add one, is decided in the reader and erased before the checker sees the program. A domain's predicates do reach the checker, as ordinary contracts and as the invariant of the block a cast continues in, but the nominal part does not.
- Cost: the checker cannot re-derive that `value as +D` was authorized or that an argument carries the domain its parameter declares; a reader bug there is accepted with no row to contradict it, and a published artifact says nothing about domains
- Reported: 2026-11-13, Claude Code, while stating domain predicates as contracts

### Data applied at a lifetime is one record whose stored views name only the data's own binders
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/read/types.rs` (`value_type`, the `Inner<'a>` arm), `read/places.rs` (`lifetime_binders`, `declared_field`), `read/bytes.rs` (`byte_view_type`)
- Saw: `Session<'a>` and bare `Session` are the same `ValueType::Record`, and the field `name: &'storage [u8]` has the type "view of `Session`'s `'storage`" in every value. So `long.name = short.name` between two `Session` values is accepted without anything relating the regions the two were applied at, and `with_alias<'s>(d: &mut Description<'s>, alias: &'s [u8]) { d.alias = alias; }` is refused as a type mismatch because the lifetime argument relates nothing. It is safe today only because a view of a formal's region can never enter a record at all
- Cost: the toolchain's own `dependency::with_alias` cannot be judged, and the first slice that lets a formal's view into a record (or forms a view of local storage) inherits an unchecked exchange between records of different regions. The type of a record value needs its lifetime arguments
- Reported: 2026-11-13, Claude Code, while reading machines with lifetime parameters

### Every view written without a lifetime is one type in every machine
- Category: a verdict with nothing behind it
- Where: `typed-trees/src/type_identity.rs` (`RegionForm::Elided`, no owner and no position); `05_typed-trees-to-terminal-psi/src/read/bytes.rs` (`source_byte_view`), `read/calls.rs` (`view_regions`)
- Saw: `&[u8]` renders one identity wherever it is written, so two formals without a lifetime have the same type and `pick(a: &[u8], b: &[u8]) -> &[u8] { b }` is accepted as a declaration, though lifetimes.md says "Ambiguous elision requires an explicit relation". The call is what refuses it, and only when the two views passed are of different regions
- Cost: the elision rule has no owner at the declaration; a reader of the callee alone cannot tell which input the result is related to, and the diagnostic lands on a caller
- Reported: 2026-11-13, Claude Code, while giving a call's lifetimes the regions of the views it passes

### The reader and its README still justify decisions by what the deleted checker did
- Category: zombie machinery
- Where: `05_typed-trees-to-terminal-psi/src/read/machine.rs` (the retired data-qualified machine spelling), `read/expressions.rs`, `read/calls.rs`, `declare.rs`, `names.rs`; `06_terminal-psi-to-terminal-psi/src/describe.rs`, `verdict.rs`; both crates' `README.md`; `06_terminal-psi-to-terminal-psi/src/lib.rs` (`Knowledge::WithoutBranchConditions`, `Knowledge::LiteralIntervals`)
- Saw: about fifty comments said "the current checker" does this or that; they now say "the deleted checker" did, which is true but leaves the reasoning hollow where the old checker was the reason: a retired spelling is served because refusing it "would only send the same program" to a checker that accepted it, and message wording is pinned to that checker's sentences. The `Knowledge` knob is threaded through discharge only for the comparison example this change deleted
- Cost: a reader cannot tell which behaviours are the specification's and which were kept for agreement with a checker that no longer exists, and the knob is dead weight in every discharge signature
- Reported: 2026-11-13, Claude Code, while removing the two-checker names

### `Console::exit_process` ends the process without publishing `reaches ProcessExit`
- Category: a verdict with nothing behind it
- Where: `source/library/std/console.omg:66` (the requirement, `reaches Console`) and `:96` (its provider); `omega-rust/psi/compilation/representations/typed-trees/src/declarations.rs` (`exact_process_exit_requirement` with `console_adapter`, `nominate_source_completion`)
- Saw: process_exit.md says "Any retained console-exit convenience adapter must obtain legitimate exit authority and publish its additional reach. Console must not inherit `ProcessExit` wholesale." The standard library's `Console::exit_process` declares `reaches Console` and nothing else, and the typed trees decide whether it completes externally from the trait being named `Console` plus a sealed intrinsic or a completion admission. Where neither is present, as in the route probe (`compiler/tests/route_probe.rs`, the `Console` program), the same source call is lowered as a call that returns
- Cost: a ceiling of `reaches Console` does not mean "cannot end the process", so package review's `ProcessExit` row undercounts; and the checker models one source call as returning or as a terminal transfer depending on an admission the program text does not show, so a body judged in one mode says nothing about the other
- Reported: 2026-11-13, Claude Code, while taking external completion (`ProcessExit::exit_process`) to the lowered-form checker

### Nothing makes a new reader site ask before it looks inside a qualified data value
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/read/` (`authority.rs`, and every `ValueType::Record(record, _)` pattern in `places.rs`, `expressions.rs`, `transitions.rs`, `arrays.rs`, `contracts.rs`, `statements.rs`)
- Saw: a type is a carrier and its domains, and about thirty sites match the carrier and discard the domains with `_`. The sites that take a value apart now call `authority::opened`, but the pattern still compiles anywhere, and two sites were typing a literal with whatever qualified type its context expected until this change (`arrays.rs` array literal, `places.rs` record literal). A store's destination is also read through the expression reader, so it asks read authority where only write authority is owed.
- Cost: the next site that projects, compares or constructs a record with `_` silently ignores a domain's reader, writer or issuer rule, and no test fails until someone writes the twin for that exact site
- Reported: 2026-11-13, Claude Code, while reading declared domains on data carriers

### Interface ownership still changes some imported-service checks
- Category: one idea, several implementations
- Where: `omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider/service_reach.rs:39`; `src/execution/host_imports/filesystem.rs:513`; `omega-rust/psi/compilation/terminal-installation/src/provider_and_target_installation.rs:451`
- Saw: native delegation removes the import's owning service from its declared reach before checking delegation (its unit permits `[owning_service]` with `Unprovided`); filesystem admission skips that same identity; assumed-binding operational compatibility ignores an extra supplied ceiling member when it equals the supplied owner. `wiki/spec/language/effects.md` distinguishes catalogue ownership from the requirement's explicit reach. These permission/compatibility checks must not be reused as proof that a declared ceiling is empty.
- Cost: the source boundary producer can retain a real catalogue owner even for `reaches none`, so downstream code must keep ownership, declared reach and actual delegated host permissions separate. This entry records the wider retained consumers; the current boundary work requires an explicit source ceiling and fixes the actual exclusion walk's implicit owner contribution, without treating these checks as source proof or expanding into an assumed-binding/delegation campaign.
- Reported: 2026-11-13, Codex, while carrying selected entry and ordinary Build boundary calls through the surviving lowered route

### "Does every selected external realization have a declaration" is checked two ways, and they disagree on unused ones
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/compiler_builtins/retained.rs:287` (`validate_external_provider_owners`) and `:295` (`validate_retained_external_provider_owners`); caller `omega-rust/omega/compilation/representations/abstract-operations/verified-input/src/native_proposal.rs:338`
- Saw: the first demands an `ExternalProviderConformance` row for every selected realization, the second only for requirements the module retains as a boundary; a six-machine scalar program with no float rows builds natively from the default compile, yet the same module is refused ("`NativeFloats` has no retained external implementation for `Float::add`") when a harness hands the proposal the provider declarations prepared for the old program.
- Cost: proposal validation has two different retained-demand contracts. The Check-mode trial requires the 146 float-table rows while the earlier default scalar compile did not read them; treating the former as the default demand can force unrelated metadata into the product path. The all-catalog investigation is parked while the actual default consumer is reconciled.
- Reported: 2026-11-13, Claude Code, while carrying the lowered-form checker's published module into Terminal production

### Constructing the source compiler repeats an hour of checking before its tests
- Category: verifying costs more than writing
- Where: [tests/product/gate.sh:42](tests/product/gate.sh#L42), [tests/product/gate.sh:70](tests/product/gate.sh#L70)
- Saw: frozen Rust producer b520 checked the 269-unit, 3.8 MB compiler in 4,089 seconds during update; native construction then took 3,796 seconds, including 3,227 seconds in Psi 05, with a peak near 19 GiB; all 191 product cases took 80 seconds afterward
- Cost: a source repair spends hours in producer checking before its executable cases run; these measurements belong to the old checker being removed, and cannot stand for seed-chain or replacement-checker performance
- Reported: 2026-11-12, Codex, while finishing bootstrap compiler construction and storage PRs #2782 and #2788

### Resolution selects an implicit entry by name in one place and child order in another
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs` (`resolve_realizations`, `call_custody_source`)
- Saw: implicit realization selection chooses the state named like the machine's last segment, then `entry`; source-less call custody requires the first State child. The former Build and checker copies are gone, but these two selectors remain.
- Cost: implicit entry selection and source custody can disagree when name order differs from child order; consolidate their exact entry relationship rather than restoring the deleted selectors.
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Stage 05 of the Omega pipeline hands out "Validated" homes and "validation receipts" with no replay behind them
- Category: a verdict with nothing behind it
- Where: `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/home_assignment.rs:1-6` and `:2628-2640`, `.../src/fixed_view_copy_recovery.rs:1-6` and `:1095-1116`, `.../src/runtime_spill_and_split.rs:530`, `:3224`, `:5460`
- Saw: both file headers say the result is "independently" replayed; `assign_register_homes_reusing` computes the plan and builds `RegisterHomeValidationReceipt` from that same plan's hash and counts; `seal_validation` does the same for fixed-view copies; the three `ReplayMismatch` error variants are declared and never constructed
- Cost: a reader of the headers or the type names believes register homes are checked twice, and a wrong home assignment has nothing between it and encoding
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Unchecked arena lookups still conflate stale handles with the zero sentinel
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/foundation/arena/src/arena.rs` (`get`, `get_mut`, `span_or_empty`); `paged.rs` (corresponding paged lookups)
- Saw: unchecked lookups return the shared default or an empty slice for unresolved nonzero handles; mutable lookups can change that default. Handle zero deliberately names absence, including its shared default; generation checks alone cannot establish ownership between arenas using the same generation.
- Cost: consumers without an owning-boundary validation can interpret a stale handle as a missing declaration or mutate the shared default. Those consumers need recoverable validation before using these infallible APIs, without changing legitimate zero-sentinel behavior.
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The Terminal interpreter reports a failed run-time check as a malformed artifact, with one error for 1,329 sites
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/interpreter/terminal-interpreter/src/boundary_values.rs:696`; example at `engine_execution.rs:3691`
- Saw: `malformed()` returns `VerifiedOperationMalformed` and is called 1,329 times across 15 files; an exact add, multiply or divide whose result does not fit ends in `.ok_or(malformed())?`; a program crash is built on only 7 lines (trapping integer, trapping float, crash terminator, host effect); no crate outside the interpreter names the variant
- Cost: an overflow or zero divisor that native code would trap on shows up as "the verified artifact is malformed" with no reason, so a native and Psi disagreement reads as an interpreter failure and the cause must be found by hand
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Typing refuses its own questions as if they waited for the checker
- Category: a stage reaching across a boundary
- Where: `03_symbol-resolved-trees-to-typed-trees/src/lib.rs` (`unavailable_judgment`) and its 26 callers, such as `typing_queries.rs` ("constructor field type", "call entry parameter types") and `type_reference.rs` ("closed const argument declaration custody")
- Saw: every refusal reads "no product route yet: X requires a lowered-form judgment", but most of X is arity, a declared type or a field's owner, which typing decides from declarations; two of them stopped the bundled library (`core::Strict<0>`, `ForeignBinding<N, ...>`) although no proof was involved
- Cost: a reader cannot tell a typing rule that was not rebuilt from a proof the checker must supply, so each stop is investigated from scratch and the wording invites routing typing work to the checker
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing

### A whole-program stage refuses every program for a library construct nobody calls
- Category: a verdict with nothing behind it
- Where: `03_symbol-resolved-trees-to-typed-trees` (the remaining `unavailable_judgment` callers, such as "mathematical call receiving types" and "selected call parameter types" in `call_and_product_selections.rs` and `typing_queries.rs`); `build-time-evaluation/src/source_frontier.rs` (`source_jobs`: the side tables it still refuses whole, such as constant recipes and wire plans); any by-name refusal added to `04_typed-trees-to-typed-trees`
- Saw: these stages see the program together with all of `core` and `std` and have no notion of what an entry demands, so one such construct in an uncalled library declaration stops a program that never reaches it; the gates the route probe met (unbound calls, assignment literals, constructor field types, layout-policy and computed types) now record the refusal on the declaration instead (`typed-trees/src/deferred_refusals.rs`), and the others still refuse the program
- Cost: each remaining gate must be served for the entire library before any program compiles, or be moved onto the same carrier one by one as a program meets it
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing

### A typing unit test is red on `main` and nothing runs it before merge
- Category: a test that pins nothing while it is red
- Where: `03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs`, `construction_destinations::tests::an_assigned_literal_lands_on_its_binding_or_refuses_its_own_machine`
- Saw: at `6f829063b3ed7` the test fails with no local change: `owned = Record {}` is asserted to be built at the type handle written on `owned` and gets a different handle (index 3, not 1), so either landing now inserts an equal copy of the type or the assertion compares identity where it means equality
- Cost: `mbx test -p symbol-resolved-trees-to-typed-trees --lib` cannot be clean for anyone changing typing, so a second regression in the same crate hides behind the first
- Reported: 2026-11-13, Claude Code, while accounting for domain casts in typing

### Two type-table tests expect an array-index refusal the table no longer makes
- Category: a test that pins nothing while it is red
- Where: `typed-trees/src/type_system.rs`, `instantiation_refuses_a_binder_spelling_an_array_length` and `instantiation_applies_the_substitution_to_domain_arguments`
- Saw: at fixed base `44ed2132219fb20261abeab4ef3e61b4ab0f845a`, `mbx test -p typed-trees --lib type_system::types::tests::instantiation_` passes the unchanged-row case and fails these two tests. They expect a bound array extent to refuse; `TypeReferenceTable::instantiate` explicitly forwards a valid named argument as an extent binder instead. The table has no symbol table with which to distinguish that forwarded value binder from the tests' named data type
- Cost: the crate's unit suite is red independently of generated-machine ownership changes; the tests and the substitution owner's contract need to agree without weakening source-level value-index eligibility
- Reported: 2026-11-14, Codex, while attributing the focused generated-application validation failures against its unchanged base

### The parser still accepts a boundary requirement with no `crashes` clause
- Category: a verdict with nothing behind it
- Where: `01_tokens-to-syntax-trees/src/declarations.rs` (refuses a boundary requirement without `reaches`, says nothing about `crashes`); `05_typed-trees-to-terminal-psi/src/read/calls.rs` (`boundary`); `samples/` and `tests/fixtures/`
- Saw: effects.md says a boundary requirement "must likewise write `crashes`". Only the lowered-form checker enforces it, and only at a call. The bundled library now writes the clause on all 400 of its requirements, but 1,745 requirements in 1,062 files under `samples/` and 70 in 40 files under `tests/fixtures/` do not, so the parser cannot refuse the omission until those are migrated, each to the ceiling its providers really have
- Cost: a library or program author learns that the clause is missing only when some caller is refused, and 30 library requirements with no provider and no caller carry `crashes Trap crashes Abort` because nothing in the repository says what narrower ceiling their owner intends
- Reported: 2026-11-13, Claude Code, while writing the bundled library's crash ceilings

### Typing lets a bare literal be stored into a field declared in a domain
- Category: one idea built twice
- Where: `03_symbol-resolved-trees-to-typed-trees` (assignment typing); `05_typed-trees-to-terminal-psi/src/read/rejections.rs` (`mistyped`)
- Saw: `Sealed { seal: Seal { } }` is refused by typing ("record literal field type") because a literal is the bare carrier, but `builder.seal = BuildSeal { };`, a plain argument for a qualified parameter and a plain value for a qualified result all pass typing. The lowered-form reader now rejects each by the rule (one carrier, two domain sets), so nothing is accepted, but typing checks the rule for one form and the reader for all of them
- Cost: "the supplied expression's static type must carry exactly the domains the destination declares" has two owners with different coverage and different wording, so the same mistake reads differently by where it is written
- Reported: 2026-11-13, Claude Code, while declaring the seal that protects the compiler-owned `Build`

### Build selection calls are found and published by the spelling of their protocol operation
- Category: one idea built twice
- Where: `03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs` (finds `BuildBindingOperations` and `bind_native` by name); `psi-compiler/terminal-production/src/build_companions.rs` (`binding`, matches the same names again)
- Saw: typing picks the protocol operation of a Build selection by comparing strings, and publication recovers which operation a boundary call is by comparing the requirement's name a second time. The typed call also carries no static application, so `bind_native<Locator>` reaches the checker as an open requirement although typing knows the locator argument it was written with
- Cost: the host row's operation is decided twice from text, and the native binding cannot be closed at its locator type by the pass that closes every other application until typing records that application on the call
- Reported: 2026-11-13, Claude Code, while closing boundary requirements at the types their calls apply

### An array extent in a generic argument is still closed by its spelling before resolution
- Category: one idea, several implementations
- Where: `02_.../preparation_const_evaluation.rs` (`closed_fixed_array_length`, reached from generic-data closing, type equations and constant evaluation) selects a constant for `[T; NAME]` through `ConstantSelection::select`, which sees only top-level declarations
- Saw: integer widths and plain extents are now read by role in their lexical scope (`resolve_value_in_type_symbol` and the value pass in `symbols_type_references.rs`), so `machine f(count: u64, items: [u8; count])` is refused as a runtime extent. But generic-argument closing (`closed_argument_identity`) still reads an extent such as `Buffer<[u8; count]>` written in that machine through the top-level selection; not yet observed in a program. Deleting the selection breaks `First<[u8; LIMIT]>` in a data declaration, where the top-level constant is the right answer
- Cost: preparation cannot tell a data or constant declaration (top-level scope) from a machine (parameters and locals in scope), so one extent spelling can mean two things; closing generic arguments after resolution, or handing preparation the enclosing value bindings, would remove the second reader
- Reported: 2026-11-14, Claude Code, while reading builtin type names by role

### The scalar contract fixture's inferred call proof is refused by the kernel
- Category: a proof producer and checker disagree
- Where: `tests/fixtures/run/calls/scalar_contract_route_exit/main.omg:26` (the call `settle(product)`)
- Saw: `omega run --both` on the unchanged fixture reports `cannot prove requires contract for call settle from main: inferred input condition`, with `kernel Certificate(IntegerOrderMiddleMismatch)`, before either engine runs. Observed on Windows x86-64 with the scalar receiving-type diagnostic repair, then reproduced at the same line after restoring all changed production files exactly to `44ed2132219fb20261abeab4ef3e61b4ab0f845a` and rebuilding `omega` through `mbx`. The fixture is unchanged, and its contracts and guards were not weakened.
- Cost: a simple scalar contract program cannot provide native/Psi execution evidence, and an invalid proof certificate is surfaced as a caller fact it should establish instead of identifying the proof producer's disagreement with the kernel
- Reported: 2026-11-14, Codex, while checking scalar receiving-type diagnostics
