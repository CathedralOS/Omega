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

### Missing locked archives stop every pool acquire or release
- Category: Re-deriving
- Where: [tools/xtask/src/worktree.rs:532](tools/xtask/src/worktree.rs#L532), `list_worktrees`
- Saw: the worktree list already records location, lock and prunable status. The pool skips prunable entries but then runs Git inside a missing locked archive checkout, aborting the complete acquire/release command. Locked archives deliberately retain admin, HEAD and restoration evidence; they must not be pruned or unlocked to make discovery work. Root reproduced this while extracting the borrowed-data repair and temporarily created only reversible `.git` handles for the command, then removed them without changing archive source, locks or absent restoration destinations.
- Cost: an unavailable locked archive prevents unrelated warm pool slots from being acquired or released. Inspect retained Git admin metadata or safely skip unavailable locked checkouts without destroying their recovery evidence.
- Reported: 2026-11-14, Codex, while isolating the checked mutable-data loan prerequisite for semantic policy normalization

### A provider default may still name a data type, which the spec says is not a provider value
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/build-time-evaluation/src/product.rs` `ProviderForm::DefaultClosure` (from #3015) and its test `a_provider_default_may_name_the_data_type_that_owns_its_closure`
- Saw: `form` refuses a data type in a Build's choice but accepts one in a `provider_defaults` machine. provider_selection.md, "Declaration and choice", says a target package "supplies ordinary named default provider implementations" and "A data type or runtime provider descriptor is not such a value", with no exception for defaults. `std`'s ProcessExit defaults, its only user, now select the named `NativeProcessExit`; the last user anywhere is `samples/cli/probes/device_extent_access` (`LoopbackDeviceLoanProvider`), which is written in other retired forms too (`via`, target-prefixed machines)
- Cost: a second, looser reading of the Provider position stays served for no library; delete `DefaultClosure` and the `ProviderForm` parameter, turn the test into the refusal twin, and give the probe a named implementation
- Reported: 2026-11-14, worker-15 (Claude Code), while moving std's ProcessExit defaults to a named implementation

### Compiler operations are recognized by their bare spelling, and no declaration answers them
- Category: one idea, several implementations
- Where: `"code_layout"` in stage 02 `constant.rs:2063,2362`, `selection_authored.rs:392,1265` and stage 03 `statement_lowering.rs:1336`, `expression_lowering.rs:1445`; `"address_of"` in `expression_lowering.rs:1076`; `Quotient::lift`/`define` and `LayoutPlan::place_private` by namespace spelling in `expression_lowering.rs`
- Saw: each stage compares the written name to a string literal, and resolution leaves the call with no target symbol, so the call is unbound in the same way a misspelled name is; telling "unknown name" from "compiler operation" needs a third copy of the list (stage 03 `typing_queries.rs` `COMPILER_CALL_SPELLINGS`)
- Cost: a new compiler operation added in one place and missed in the list would be rejected as an unknown name; one declaration per operation (as `BuiltinFunction` symbols are) would let resolution answer it once
- Reported: 2026-11-14, Claude Code (worker-14), while making a call of an undeclared name a rejection

### Whether a type is a scalar is decided twice, once by its spelling and once by its symbol
- Category: one idea, several implementations
- Where: `typed-trees/src/type_system.rs` (`TypeReferenceNode::primitive_type` calls `PrimitiveType::from_name(name)`; `type_symbol` returns no symbol when the name spells a primitive); `05_typed-trees-to-terminal-psi/src/read/types.rs` (`atom_type` through `SymbolTable::builtin_type_atom`); `symbol-resolved-trees/src/type_system_and_names.rs` (a second `PrimitiveType` with its own `from_name`)
- Saw: typing reads `u32` as an integer because the text of the name is `u32`, with `AtomicU32` and `AtomicU32#AccessField<` mapped to the same carrier by string prefix; the lowered-form checker reads it as an integer because its symbol sits in a compiler-installed root slot. Two enums named `PrimitiveType` restate the answer for the two tree representations
- Cost: a package type spelled like a primitive has to be taught to both routes, and the two can disagree about one type reference. The width spellings were taught to both (`u13` is a name to typing and an installed builtin symbol to the checker), which made them agree without removing the second route.
- Reported: 2026-11-13, Claude Code, while replacing the eight integer names with a sign and a width

### Float semantic evaluation still collapses integer metadata to a fixed-width roster
- Category: branch per shape
- Where: `numerics/src/float_semantics.rs` (the integer side of float conversions)
- Saw: the front end states an integer as a sign and a width, and the float semantic catalog converts that pair back into one of eight variants, refusing the rest. Stage 02's const evaluator (`preparation_const_evaluation.rs`) keys carriers by type-name strings and folds in `i128`, so it serves widths to 64 bits for shifts and 127 for ranges
- Cost: unusual-width conversion proofs stop at the finite semantic catalog, and its published operation identities require a versioned change
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

### A clause the specification rejects can only be reported as not supported yet
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/compilation/representations/terminal-psi/check-tables/src/lib.rs` (`RefusalReason`); `05_typed-trees-to-terminal-psi/src/read/contracts.rs` (`term`, the `Trapping` arm)
- Saw: `ensures result == left +! right` is an error of the program (`wiki/spec/proofs/contracts.md`, "Total arithmetic": an operation that can transfer control "cannot form a proof term"), but the reader's only negative result that is not a storage or ownership judgment is `Unsupported`, so the clause is refused with the same outcome as a construct nobody has built yet. A wholly anonymous `7 +% 1` and a suffix on a qualified operand are reported the same way
- Cost: the author is told the compiler cannot judge a program that the language forbids, and a test cannot tell a rule of the language from a gap in the checker
- Reported: 2026-11-13, Claude Code, while reading suffixed operators in contracts

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

### Native lowering accepts module bytes without artifact-bound host producer trust
- Category: a verdict with nothing behind it
- Where: `00_terminal-psi-to-abstract-operations/src/artifact_admission.rs` (`lower_artifact`), `abstract-operations/verified-input/src/lib.rs` (`VerifiedPsiOptimizationInput::lowered`), and `01_abstract-operations-to-abstract-operations/src/verified_unit.rs` (`source_checked_obligation_facts`)
- Saw: the public byte-lowering route canonically decodes a module and can produce accepted obligation facts without an artifact-bound host producer-trust witness. Zero proof fingerprints are not themselves the defect: `product.md#publication-and-separate-consumption` permits reuse of a trusted producer's judgments without transported derivations.
- Cost: an embedding caller can reach native optimization with asserted obligations but no explicit authorization of their producer; repair the ingress trust boundary under `PSI-CLAIM-ADMISSION`, not by replaying proofs on every trusted internal stage handoff.
- Reported: 2026-11-12, Claude Code; narrowed against the ratified execution-admission contract by Codex on 2026-11-13 while auditing accepted facts

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

### Architecture rules are enforced by searching source text for substrings
- Category: tests that pin the bug
- Where: `omega-rust/omega/compilation/compiler/src/product_boundary.rs`, `omega-rust/omega/build-system/orchestration/src/build_boundary.rs`, `omega-rust/omega/compilation/native-compiler/src/pipeline_boundary.rs`, `omega-rust/omega/compilation/realization/provider-planning/src/source_guards.rs`, `omega-rust/omega/compilation/backend/plans/program-entry-plan/src/source_guards.rs`, `omega-rust/psi/foundation/access-plans/src/tests.rs`, `omega-rust/psi/interpreter/terminal-interpreter/src/embedding.rs`
- Saw: these tests assert that source strings are present, absent or ordered, rather than exercise the owned operation or dependency boundary. The compiler guards pinned fields of the deleted custody and had to lose those assertions when it was removed; they never established that custody had a producer. The stage-05 guards themselves are gone.
- Cost: harmless renaming breaks tests while equivalent forbidden behavior can pass under another spelling. Move actual dependency rules to the boundary checker and use behavior tests for custody and execution claims; do not mechanically preserve old strings or test totals.
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### `pipeline.md` says the default compile runs no stage validators; two run, and one stage header misdescribes its own
- Category: the documents disagree
- Where: `omega-rust/pipeline.md:66-68` and `:91-93`; `omega-rust/omega/compilation/backend/machine-emission/src/exit_contract.rs:153` and `:205-222`; `omega-rust/omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/lib.rs:45-47` against `.../src/validation.rs:630`, `:639` and `.../src/row_encoding.rs:661`, `:670`
- Saw: `stage_whole_function_exit_contract_for_layout`, reached from `native-compiler/src/pipeline.rs`, calls the stage 08 and stage 09 validators, and the stage 08 header says so; the stage 07 header says its replay "never calls the producer's route table or encoders", yet it calls the same dynamic-call template encoders the producer calls. The former corpus-runner comparison is no longer a consumer.
- Cost: an agent can cite the document to remove the stage 08 and 09 checks or to add more, and cannot tell from headers which checks are separate work and which reuse the producer
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

### A generated machine is identified three ways and copied by a hand-kept list
- Category: one idea, several implementations
- Where: `typed-trees` (`MachineSpecialization` rows, `Symbol::generated_from`, `executable_application_template`); `05_typed-trees-to-terminal-psi/src/lib.rs` (`template_of`), `read/calls.rs` (`applies_nothing`), `declare.rs`; `04_typed-trees-to-typed-trees/src/passes/specialization.rs` (`template`, `held_types`)
- Saw: "this machine was generated from that generic machine" is read from the state symbol's `generated_from` in three places and from the `machine_specializations` row in a fourth, and nothing ties the two; the row also keeps eight string and commitment fields that only the deleted checker filled. Separately, an expression copy shares the types its nodes hold, so the pass lists by hand which node kinds hold a type (`held_types`) and which body constructs it cannot copy
- Cost: a pass that sets one marker and not the other produces a machine the reader accepts and the declaration step refuses, or the reverse; a new expression node that holds a type is silently left pointing at the generic machine's type until someone adds it to the list
- Reported: 2026-11-13, Claude Code, while rebuilding specialization as a typed preparation pass

### A typing unit test is red on `main` and nothing runs it before merge
- Category: a test that pins nothing while it is red
- Where: `03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs`, `construction_destinations::tests::an_assigned_literal_lands_on_its_binding_or_refuses_its_own_machine`
- Saw: at `6f829063b3ed7` the test fails with no local change: `owned = Record {}` is asserted to be built at the type handle written on `owned` and gets a different handle (index 3, not 1), so either landing now inserts an equal copy of the type or the assertion compares identity where it means equality
- Cost: `mbx test -p symbol-resolved-trees-to-typed-trees --lib` cannot be clean for anyone changing typing, so a second regression in the same crate hides behind the first
- Reported: 2026-11-13, Claude Code, while accounting for domain casts in typing

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

### A call of a machine nothing declares is refused as "not supported yet", not as an error
- Category: a verdict with nothing behind it
- Where: `typed-trees/src/deferred_refusals.rs` (`DeferredRequest::UnresolvedCallTarget`); `05_typed-trees-to-terminal-psi/src/read.rs` (raises the row as `RefusalReason::Unsupported`); `compiler/tests/route_probe.rs` (`an_unknown_machine_after_a_call_that_never_returns_is_not_accepted`)
- Saw: typing records a call whose name no declaration answers as a deferred row beside rows for constructs no pass serves yet, and the reader raises every row the same way. `undeclared_machine(x);` in a program's own machine therefore reaches the user as "not supported yet ... with no claim that it is wrong" and never as a rejection
- Cost: a misspelled machine name reads as a compiler limitation; state_contracts.md ("an unknown machine name or malformed type is an error wherever it appears") is met only in that the program is not accepted, and the outcome word is `not-supported` where it should be `rejected`
- Reported: 2026-11-13, Claude Code, while accepting statements after a call that never returns

### Lowering signals "this path ended" through the refusal channel
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/lower.rs` (`Lowering::end_path`, `Lowering::reached`); taken in `body`, `lower/control.rs` (`complete`, `short_circuit`, `match_value`)
- Saw: a call that never returns, written inside an expression, ends the evaluation that holds it by returning a `Refusal` up through every `?`, and the construct whose path ended recognizes it by a side field and the refusal's text. Expression lowering returns `Checked<Value>` with no way to say "no value: control left", so the signal borrows the error type
- Cost: any lowering step that swallows a refusal (`.ok()`, `if let Ok`) between the call and its taker would turn a divergence into an internal refusal later; the honest shape is a `Value::Ended(call)` result that every operand consumer handles, which touches each of the ~25 `scalar`/`record` call sites in `lower/`
- Reported: 2026-11-14, Claude Code, while lowering calls that never return inside expressions

### No stage types a result value against the type its machine declares
- Category: a verdict with nothing behind it
- Where: `03_symbol-resolved-trees-to-typed-trees` (no expression has a type there; `typing_queries.rs` answers declared places only); `05_typed-trees-to-terminal-psi/src/read/transitions.rs` and `read/expressions.rs` (`Unserved::Typing("arm value type")`, `"match arm value type"`)
- Saw: `machine f(value: u32) -> bool { value }`, `machine f(value: u1) -> bool { value }` and `machine f(value: bool) -> u1 { value }` all pass typing. Each is stopped only by the lowered-form reader, which calls it `not-supported` because it expects typing to have rejected it
- Cost: an ordinary type error reaches the author as "not supported yet" with no source line, and the reader's 57 `Unserved::Typing` refusals are the only owner of value typing. Making any of them a rejection needs typing to compute expression types, which it does not do today
- Reported: 2026-11-13, Claude Code, while making integer widths symbolic

### A value name in a type is read by its spelling before resolution, past the parameters that hide it
- Category: one idea, several implementations
- Where: `02_.../preparation_integer_widths.rs` (`width_of`) and `02_.../preparation_const_evaluation.rs` (`closed_fixed_array_length`) both close a name through `ConstantSelection::select` before resolution; after it, `symbols_type_references.rs` `resolve_type_symbol` and `assign_fixed_array_length_symbol` look at type parameters and then top-level declarations only
- Saw: in `const count: u64 = 13; machine f(count: u64, v: u<count>)` (or `[u8; count]`) the runtime parameter `count` is read as the constant. modules.md "Resolution" puts local bindings and parameters before imported names. Not reachable as an accepted program today only because the checker refuses every top-level `const` declaration
- Cost: an integer width and an array extent each decide what a value name in a type means, by spelling, in two places; the day `const` declarations are read, a runtime width or extent is silently fixed to a constant. One type-position value-name resolver that sees the enclosing value bindings would serve both
- Reported: 2026-11-14, Claude Code, while landing symbolic integer widths
