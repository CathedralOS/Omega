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

### A view passed from a place is lent as that place, so it conflicts with a loan of its holder
- Category: one idea, several implementations
- Where: `05_typed-trees-to-terminal-psi/src/read/calls.rs:1453` (`lent_storage_is_untouched` with `holds_view`), `lower/calls.rs:65` (`arguments`, the `Borrow` arm), `lower/places.rs:1428` (`lend`), `06_terminal-psi-to-terminal-psi/src/places/loans.rs`
- Saw: the reader says a place that holds a byte view "lends none of its binding's storage: the callee is given the view", and so allows `both(&mut session, session.name)`. Lowering then passes the view as a shared loan of the place `session.name`, and the checker rejects the call: "state `both` receives `session` as both mutable and read-only". The same holds for a view in a case payload. A source audit at `f9bbfeac3f5` also finds that `lend` retains the descriptor slot's root/path without materializing its value before later arguments. [Evaluation order](wiki/spec/language/expressions.md#evaluation-schedule) and [descriptor loans](wiki/spec/terminal-psi/structural_access.md#store-vocabulary) require the captured backing, rather than a loan of the carrier slot. No runtime misbehavior from later-argument replacement has been demonstrated.
- Cost: a correct program is rejected, not refused as unsupported, and the message blames the author. The shared argument producer must capture the descriptor at its evaluation point; weakening overlap checks or reading its source slot only at the eventual call would not preserve that value.
- Reported: 2026-11-13, Claude Code, while lending case payloads; extended 2026-11-14, Codex, while binding view-length call requirements

### Whether a type is a scalar is decided twice, once by its spelling and once by its symbol
- Category: one idea, several implementations
- Where: `typed-trees/src/type_system.rs` (`TypeReferenceNode::primitive_type` calls `PrimitiveType::from_name(name)`; `type_symbol` returns no symbol when the name spells a primitive); `05_typed-trees-to-terminal-psi/src/read/types.rs` (`atom_type` through `SymbolTable::builtin_type_atom`); `symbol-resolved-trees/src/type_system_and_names.rs` (a second `PrimitiveType` with its own `from_name`)
- Saw: typing reads `u32` as an integer because the text of the name is `u32`, with `AtomicU32` and `AtomicU32#AccessField<` mapped to the same carrier by string prefix; the lowered-form checker reads it as an integer because its symbol sits in a compiler-installed root slot. Two enums named `PrimitiveType` restate the answer for the two tree representations
- Cost: a package type spelled like a primitive has to be taught to both routes, and the two can disagree about one type reference. The width spellings were taught to both (`u13` is a name to typing and an installed builtin symbol to the checker), which made them agree without removing the second route. Typing still lets two mismatches through that only the checker stops, as `not-supported` instead of a typing error: `machine f(value: u1) -> bool { value }` (`typing arm value type`) and a suffixed literal outside its carrier such as `1i1` or `8192u13` (`integer literal outside its type`)
- Reported: 2026-11-13, Claude Code, while replacing the eight integer names with a sign and a width

### Three vocabularies still list eight integer widths by name
- Category: branch per shape
- Where: `language-semantics/src/numeric_intrinsics.rs` (`NumericType`, published in Terminal Psi); `numerics/src/float_semantics.rs` (the integer side of float conversions); `omega/packages/review-evidence` (`primitive_type_row`, one wire tag per width)
- Saw: the front end now states an integer as a sign and a width, and these three convert that pair back into one of eight variants at their door, refusing the rest. Stage 02's const evaluator (`preparation_const_evaluation.rs`) keys carriers by type-name strings and folds in `i128`, so it serves widths to 64 bits for shifts and 127 for ranges
- Cost: every width beyond the eight stops at whichever of these it meets first, and each is a published or cross-stage vocabulary that needs its own versioned change
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

### "Does this access cover that one" is written eight times with five different answers
- Category: one idea, several implementations
- Where: `omega-rust/psi/interpreter/terminal-interpreter/src/storage_references.rs:4455`; `omega-rust/psi/compilation/representations/terminal-psi/src/ownership.rs:62`; `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/references.rs:887`; `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_reference_custody.rs:4600` and `selection.rs:877`; `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:5124` and `:14940`; `omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider/reference_outputs.rs:350`
- Saw: four copies agree (equal, or `Owned` covers all, or `MutableBorrow` covers shared and write-only), and the stage 02 and stage 03 legalization copies are the same text; `unit_validation.rs:5124` drops write-only; `selection.rs:877` lets owned or mutable cover shared only; `reference_outputs.rs:350` lets mutable cover shared and gives `Owned` nothing extra; `ownership.rs:62` never admits an `Owned` request
- Cost: nothing at any site says whether its table differs on purpose, so a fix to one rule cannot be checked against the others, and stage 03's audit of stage 02 shares the copied table, so it cannot catch a mistake in it
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Resolution selects an implicit entry by name in one place and child order in another
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs` (`resolve_realizations`, `call_custody_source`)
- Saw: implicit realization selection chooses the state named like the machine's last segment, then `entry`; source-less call custody requires the first State child. The former Build and checker copies are gone, but these two selectors remain.
- Cost: implicit entry selection and source custody can disagree when name order differs from child order; consolidate their exact entry relationship rather than restoring the deleted selectors.
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Integer const identities are formatted and parsed as text across representations
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/representations/typed-trees/src/calls_and_control_flow.rs` and `src/lib.rs`; `omega-rust/psi/interpreter/terminal-interpreter/src/engine_calls.rs` (`family_tuple_member_value`)
- Saw: the string `named(integer-const(N))` is made with `format!`, then each reader strips the prefix and suffix by hand and parses an `i128`; the Terminal interpreter picks a dynamic family member this way, and `terminal-psi/src/boundary.rs:555-556` interns two such strings as literals
- Cost: the surviving producer and consumers agree by spelling rather than a shared typed value; a reader that misses the form fails only at run time (`None`, or "dynamic family tuple identity is not an integer constant").
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A structural path segment has five hand-written byte encoders with three different numberings
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/identity.rs:213` and `.../optimization-unit/src/ownership.rs:171`, `.../selected-instructions/src/effects/program/identity.rs:476`, `.../selected-instructions/src/plan_identity.rs:606`, `.../target-operations/legalized-operations/src/identity/structural_types.rs:208`
- Saw: each file matches on all eight `StructuralPathSegment` variants and writes its own tag bytes; `RuntimeIndex` is 5 in three of them, 6 in the machine-effect identity and 4 in the selected plan identity, where `Field`, `FixedIndex`, `Referent` and `FixedRange` are also renumbered
- Cost: a new path segment kind needs five encoders and at least two decoders (`register-homes/src/recovery/structural_declarations.rs:714`, `selected-instructions/src/effects/program/encoding.rs:1654`) edited by hand, and nothing says which numbering is the reference
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The Omega unit validator carries copies of two Terminal Psi codec checks
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:17909` and `:17846`, against `omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs:8566` and `:11744`
- Saw: `attachment_storage_types` differs between the two files in two lines (visibility and one `use`); `validate_structural_type_graph` is the same walk with a different error type and lookup
- Cost: the type-graph rules live once on each side of the Psi/Omega boundary, so a fix on one side leaves the other accepting or rejecting something different, with no test found tying them together
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The default ranking view is derived differently in resolution and typing
- Category: re-deriving
- Where: `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_machine.rs` (`elaborate_single_subject_default`); `03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs` (`pending_ranking_default`)
- Saw: resolution finds a parameter by name text and accepts a `.len` member; typing finds it by symbol, has no `.len` rule, stops after 128 steps and runs only when resolution left the view empty. The resolution comment still claims to mirror the deleted checker "EXACTLY".
- Cost: two stages own the same default with different selection rules; new rankable carriers can acquire different defaults depending on which stage first recognizes them.
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The "independent" unit validator gets its expected values from the constructor it is checking
- Category: re-deriving
- Where: `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:2654`, `:2670`, `:2375`, against `.../optimization-unit/src/construction.rs:527-608` and `:612-741`
- Saw: `expected_definitions`, `expected_uses` and `expected_ownership` call `optimization_unit::operation_definition`, `operation_uses` and `operation_ownership`, the functions that built the unit; the first two end in a wildcard arm (`_ => None`, `_ => Vec::new()`); the crate header (`semantics/src/lib.rs:4`) calls this "independent semantic and rewrite validation"
- Cost: an operation the constructor forgets to list is missed identically on both sides, so the comparison passes and the next agent trusts a check that cannot fail for that case
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Register and machine-effect catalogs are validated by rebuilding them with the builder that made them
- Category: re-deriving
- Where: `omega-rust/omega/compilation/backend/instruction-set-architectures/isa-x86_64/src/register_model.rs:1381`, `.../isa-aarch64/src/register_model.rs:1082`, `omega-rust/omega/compilation/representations/selected-instructions/src/effects/catalog.rs:1634-1639`; default arms at `.../isa-x86_64/src/machine_effects.rs:1395-1403` and `.../isa-aarch64/src/machine_effects.rs:993-998`
- Saw: each validator calls the canonical builder again and requires equality with the catalog it was handed, which in the callers read came from that same builder; the effect builders end in a `_ =>` arm that declares no reads, no writes, never traps and falls through
- Cost: a new instruction kind that misses the effect table is declared effect-free and passes validation by construction; "validated catalog" reads as an independent check and is not one
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Native lowering accepts module bytes without artifact-bound host producer trust
- Category: a verdict with nothing behind it
- Where: `00_terminal-psi-to-abstract-operations/src/artifact_admission.rs` (`lower_artifact`), `abstract-operations/verified-input/src/lib.rs` (`VerifiedPsiOptimizationInput::lowered`), and `01_abstract-operations-to-abstract-operations/src/verified_unit.rs` (`source_checked_obligation_facts`)
- Saw: the public byte-lowering route canonically decodes a module and can produce accepted obligation facts without an artifact-bound host producer-trust witness. Zero proof fingerprints are not themselves the defect: `product.md#publication-and-separate-consumption` permits reuse of a trusted producer's judgments without transported derivations.
- Cost: an embedding caller can reach native optimization with asserted obligations but no explicit authorization of their producer; repair the ingress trust boundary under `PSI-CLAIM-ADMISSION`, not by replaying proofs on every trusted internal stage handoff.
- Reported: 2026-11-12, Claude Code; narrowed against the ratified execution-admission contract by Codex on 2026-11-13 while auditing accepted facts

### A default compile seals the native artifact without running the replay, and a failed replay under `--both` is reported as declined
- Category: a verdict with nothing behind it
- Where: `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/native_artifact.rs:479-486`, `:529`, `:629-632`; `omega-rust/omega/interpreter/terminal-runtime/src/execution/native_code_layouts.rs:35-37`; `omega-rust/omega/interpreter/terminal-runtime/src/execution.rs:113-115`; `omega-rust/omega/application/cli/src/execution/mod.rs:549-550`; `omega-rust/omega/application/cli/src/commands/run.rs:113-116`
- Saw: `from_emitted_parts` seals with no call to `validate`; outside tests and one example, `validate` is reached only through `inspect_native_code_layouts`, whose single caller runs when the Terminal runtime is handed a native image; the three machine-emission and object-file stage validators are called only from each other and from `function_fragments.rs:24444` inside that replay; a validation error becomes a missing provision, which makes the comparison `Declined` and the command exit with the native code
- Cost: the build-then-replay design reads as if every emitted program was replayed, but an ordinary `omega <root>` never runs it, and when it does run a failure looks like an unavailable comparison
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Stage 05 of the Omega pipeline hands out "Validated" homes and "validation receipts" with no replay behind them
- Category: a verdict with nothing behind it
- Where: `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/home_assignment.rs:1-6` and `:2628-2640`, `.../src/fixed_view_copy_recovery.rs:1-6` and `:1095-1116`, `.../src/runtime_spill_and_split.rs:530`, `:3224`, `:5460`
- Saw: both file headers say the result is "independently" replayed; `assign_register_homes_reusing` computes the plan and builds `RegisterHomeValidationReceipt` from that same plan's hash and counts; `seal_validation` does the same for fixed-view copies; the three `ReplayMismatch` error variants are declared and never constructed
- Cost: a reader of the headers or the type names believes register homes are checked twice, and a wrong home assignment has nothing between it and encoding
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Stage 03 skips the exhaustiveness check for any dispatch run that has a continuation
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs:932-934` (flag set at `:858`)
- Saw: `check_dispatch_run` returns `Ok(())` as soon as one transition in the run has a valid `continuation`, before any arm is examined; no comment gives the reason; whether stage 05 catches a missing arm in such a run was not traced
- Cost: a reader takes stage 03 as the owner of "every case is covered" when for these runs it decides nothing, and the question of who does is left to whoever next breaks it
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

### Proof questions are read, validated and hashed in five places and written in none
- Category: zombie machinery
- Where: `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/proof.rs:193`, `:305`, `:545`; `.../src/construction.rs:73`; `.../src/identity.rs:2269`; readers at `pipeline/01_abstract-operations-to-abstract-operations/src/value_ranges.rs:180`, `.../semantics/src/current_value_ranges.rs:215` and `:325`, `.../semantics/src/unit_validation.rs:1174` and `:9710`
- Saw: `proof_questions` is set to `Vec::new()` at its only two initialisers; `ProofQuestion::new` has no caller in `omega-rust/`, tests included; `ProofQuestionIndexError` is never constructed; the stage 01 value-range rule skips unless it finds exactly one matching question
- Cost: the proof-backed value-range rule and its validator twin can never fire, and they look like the place where proof evidence reaches the back end
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Physical evidence is never derived in production, yet about 10,000 lines exist to derive and replay it
- Category: zombie machinery
- Where: `omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs:161`, `.../native-artifact/src/physical/derivation/evidence.rs:78-79`, `.../native-artifact/src/physical/` (10,168 lines)
- Saw: the only non-test place that sets a scope sets `NativePhysicalEvidenceScope::Unavailable`, and derivation returns at once for that scope; the other two scopes are built only in `native_artifact/tests.rs`; seven `#[expect(dead_code)]` markers say "unwired, not superseded" and point at a task
- Cost: the artifact identity and validation chain carry an evidence slot that is always empty; an agent fixing custody code there is fixing code no shipped compile runs
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

### An `invokes` row that names an ordinary trait is dropped without a word
- Category: a verdict with nothing behind it
- Where: `typed-trees/src/declarations.rs` (`declared_targets`, consumed by `declared_machine_invocations`)
- Saw: `declared_targets` keeps an `invokes` target only when it is a boundary trait or a parameter of boundary type and silently discards any other, so `invokes Handler` over an ordinary trait reads as no target to its consumers. Conformance refinement is now judged by `06_terminal-psi-to-terminal-psi/src/verdict.rs`; that no longer explains the loss.
- Cost: a written entry ceiling can vanish before the checker sees it. Preserve or explicitly reject the authored target instead of silently narrowing the request during extraction.
- Reported: 2026-11-13, Claude Code, while putting `invokes` on the lowered-form checker

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

### Static requirement dispatch retains fields its producer never fills
- Category: stale representation and consumers
- Where: `typed-trees` (`StaticRequirementDispatch`, `selected_conformance_applications`); `04_typed-trees-to-typed-trees/src/passes/specialization.rs` (`selections`)
- Saw: the dispatch producer writes five of nine fields; application fingerprint, commitment, selection application and contract signature stay at their defaults, but copy walks and conformance-signature lookup still consume them
- Cost: readers cannot distinguish an empty commitment from an uncomputed one; retirement needs to reconcile those consumers with the actual specialization evidence, not substitute unchecked defaults
- Reported: 2026-11-13, Claude Code, while serving implementation parameters in the specialization pass

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
