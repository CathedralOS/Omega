# Owner Code Concerns

Agents write down structural rot here when they see it, and keep working. The
owner reads this file in a session of its own and decides what to clean up.
Nothing in it is a task, a ruling or a reason to stop: an entry never blocks the
change that reported it.

This is not [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md). That file holds language
and architecture decisions only the owner can make. This one holds things that
look wrong in the code and tooling as they stand.

## How to add one

Add an entry when something would mislead the next agent, or when fixing it
properly reaches beyond the crate you are in. An ordinary bug, a style
preference, or anything you can repair inside your own task does not belong
here: just fix it.

The best moment is your first hour in an area, before you have a change of your
own to defend. Say what you saw, not what you would build.

- Add the entry at the top of the list under "Entries"; the list is flat, newest first.
- Name one category from the list below on its `Category` line.
- Keep it to the lines of the template. Link the file and line.
- Add it in the pull request you are already landing; never open one for it.
- If two entries collide in a merge, keep both.
- Only the owner removes an entry or marks it settled.

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

### The seam that kept typed trees "beside the current checker" still exists, and two examples still compare against it
- Category: zombie machinery
- Where: `omega-rust/omega/compilation/compiler/src/checked/checking/typed_frontier.rs` (`CurrentCheckerVerdict`, `TypedFrontierCheck`, `check_keeping_typed_frontier`); `compiler/examples/route_one_comparison.rs`, `compiler/examples/structure_elaboration_corpus.rs`; `compiler/tests/support/fixture_package_inputs.rs` (`FixtureFrontierCheck`)
- Saw: the seam was built so a harness could run a second checker beside the old one; the old one is deleted, so "the current checker's verdict" it returns is the lowered-form checker's own, and the two examples compare that checker with itself over a corpus that no longer exists. The tests that only read what the checker concludes left the seam in this change; `lowered_route_governance`, `lowered_route_publication` and two tests of `lowered_route_ownership` still use it to read what the product compile reports
- Cost: the names say there are two checkers; an agent reading `current=accepted route=rejected` output or `FixtureFrontierCheck::Rejected` will look for the one that is gone
- Reported: 2026-11-13, Claude Code, while moving the lowered-form checker's tests off the Build

### Suffixed policy operators are read in executable code only
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/pipeline/structure-elaboration/src/read/contracts.rs` (`policy operator in a clause term`, `policy operator in a mathematical term`); `read/expressions.rs` (`suffixed_expression`)
- Saw: typed preparation used to rewrite `a +% b` into casts and the Wrapping add for bodies and contracts alike; that pass was deleted with the old checker (#2823) and every program with a suffixed operator was refused. The body reader now reads the suffix itself; the two contract readers still refuse it, and `+!` is refused everywhere because no type on the route carries `Trapping`
- Cost: a `requires` or `ensures` that writes `+%` (accepted before #2823, `wiki/spec/proofs/contracts.md`, "Total arithmetic") is refused by name, and the clause readers and the body reader each decide what an operator means
- Reported: 2026-11-13, Claude Code, while restoring `+%` in machine bodies

### Nothing makes a new reader site ask before it looks inside a qualified data value
- Category: a verdict with nothing behind it
- Where: `structure-elaboration/src/read/` (`authority.rs`, and every `ValueType::Record(record, _)` pattern in `places.rs`, `expressions.rs`, `transitions.rs`, `arrays.rs`, `contracts.rs`, `statements.rs`)
- Saw: a type is a carrier and its domains, and about thirty sites match the carrier and discard the domains with `_`. The sites that take a value apart now call `authority::opened`, but the pattern still compiles anywhere, and two sites were typing a literal with whatever qualified type its context expected until this change (`arrays.rs` array literal, `places.rs` record literal). A store's destination is also read through the expression reader, so it asks read authority where only write authority is owed.
- Cost: the next site that projects, compares or constructs a record with `_` silently ignores a domain's reader, writer or issuer rule, and no test fails until someone writes the twin for that exact site
- Reported: 2026-11-13, Claude Code, while reading declared domains on data carriers

### Core's `AccessPlan` and `AccessPolicy` are the superseded layout-containing shape
- Category: one idea, several implementations
- Where: `source/library/core/layout.omg` (`AccessPlan`, `AccessPlan::inaccessible`, `AccessPolicy::plan(schema: Schema)`); `wiki/spec/resources/access.md`, "Plans and admitted supply"
- Saw: the spec's plan is `AccessPlan::new(&schema, issuance)` over `reflect::TypeSchema` with `Issuance::Only`/`Unrestricted`, `allow_readers`, `allow_writers`, `IssuedBy` and `IssuedByAny`; core still declares a plan that contains a `LayoutPlan`, takes `layout::Schema`, and has no issuance at all, and `alloc/vec.omg` and `samples/cli/probes/layout_access_plan` are written against it
- Cost: a domain's `access Policy` clause now reaches the typed trees, but no policy in the library can state an issuance, so every domain that selects one stays "planned" and nobody can be shown to issue it
- Reported: 2026-11-13, Claude Code, while reading a domain's `access` clause

### Typing refuses every program that writes `=D`, `-D` or `+D`
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs` (`domain_cast_accounting::account`)
- Saw: the parser reads domain-cast entries, and typing then returns a whole-program "domain cast operand accounting" error for the first one it meets, in any declaration, reached or not; the accounting needs the operand's complete qualified type, which the deleted checker computed
- Cost: no program holding a domain cast reaches the lowered-form checker, so the judgment of `as +D` cannot be built or tested there until accounting has an owner or the refusal moves onto the declaration as a deferred row
- Reported: 2026-11-13, Claude Code, while reading a domain's `access` clause

### Constant operands are still read from a table keyed by spelling
- Category: a verdict with nothing behind it
- Where: `02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs` (`module_constants::lexical_integer_const_values`), built at five call sites in `preparation_const_evaluation.rs`, `preparation_generic_data.rs` and `preparation_machine_equations.rs` and handed to `evaluate_const_argument_expression` and the fact evaluators
- Saw: the table maps every non-module constant's spelled name to its literal value over all root items of the forest, with no source scope; two packages that each keep a private `const LIMIT` leave one entry, so a const-generic argument, range fact or machine equation naming `LIMIT` can read the other package's value. The table also only holds constants written as integer literals, so a constant resolution now folds (`const B: u64 = A;`) is absent from it
- Cost: the new closed-initializer fold selects operands through `ConstantSelection` and never reads this table, but the older evaluators beside it still do; a wrong value there is silent, and the fix is to give those evaluators the same source-scoped selection rather than to repair the table
- Reported: 2026-11-13, Claude Code, while folding closed scalar constant initializers in resolution

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

### Terminal production keeps a request type and readers that nothing can satisfy
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/psi-compiler/terminal-production/src/lib.rs` (`TerminalProductionRequest::produce`), `src/external_providers.rs` (`PreparedExternalProviderDeclarations::prepare`), `src/build_bindings.rs` (`BuildProviderProductionRequest::produce`); callers in `compiler/src/terminal/product_code.rs`, `selected-dispatch/src/provider_binding.rs`, `review-evidence/src/capture/input.rs`, `build-settings/src/activation/preparation.rs`, `build-time-evaluation/src/machine_execution.rs`
- Saw: after Psi 06 was deleted these three entries always return `no product route yet`, but their callers and everything downstream of a produced program (provider selection, package review, native realization, the corpus runner's source controls) still compile as if a program could arrive
- Cost: a reader sees a full production pipeline and cannot tell that no program reaches Terminal Psi; the hand-off from the lowered route's published module has to replace these three entries, not add a fourth
- Reported: 2026-11-12, Claude Code, while deleting Psi 06

### The stage-coverage command still reports a column for the deleted Psi 06
- Category: zombie machinery
- Where: `tools/xtask/src/stage_coverage.rs` (`STAGES`), `tools/testing.md` stage-coverage section
- Saw: `stage-coverage` counts lines under the deleted Psi 06 directory, so that column is always empty; the command exists to measure the old checker and has no reader once Psi 05 and `validation` are gone
- Cost: the coverage table reads as if Psi 06 ran no lines, and the command outlives what it measures
- Reported: 2026-11-12, Claude Code, while deleting Psi 06

### Constructing the source compiler repeats an hour of checking before its tests
- Category: verifying costs more than writing
- Where: [tests/product/gate.sh:42](tests/product/gate.sh#L42), [tests/product/gate.sh:70](tests/product/gate.sh#L70)
- Saw: frozen Rust producer b520 checked the 269-unit, 3.8 MB compiler in 4,089 seconds during update; native construction then took 3,796 seconds, including 3,227 seconds in Psi 05, with a peak near 19 GiB; all 191 product cases took 80 seconds afterward
- Cost: a source repair spends hours in producer checking before its executable cases run; these measurements belong to the old checker being removed, and cannot stand for seed-chain or replacement-checker performance
- Reported: 2026-11-12, Codex, while finishing bootstrap compiler construction and storage PRs #2782 and #2788

### Every Build program runs through the old checker and the old lowering stage
- Category: the documents disagree
- Where: `omega-rust/omega/build-system/build-settings/src/admission/checking.rs:323` (`CheckRequest::check()` on the Build program), `build-settings/src/activation/preparation.rs` (`PreparedBuildExecution::prepare` calls `TerminalProductionRequest::produce`), `omega-rust/psi/compilation/build-time-evaluation/src/machine_execution.rs:5082` and `:5457`, `build-time-evaluation/src/const_initializers.rs:137`
- Saw: a project's Build program is checked by Psi 05, lowered by Psi 06 and run in the terminal interpreter before the product check starts; with Psi 06 deleted, `omega --check` on `tests/omega/pass/arithmetic/anonymous_integer_arith` fails in 0.5 s with `project source discovery failed: no product route yet` and the lowered-form checker is never called (draft #2796)
- Cost: AGENTS.md says the old stages are being removed now, but none of them can go until Build programs are judged and lowered another way; a worker who deletes one in the stated order stops every compilation, not only native output
- Reported: 2026-11-13, Claude Code, while deleting Psi 06

### "Does this access cover that one" is written eight times with five different answers
- Category: one idea, several implementations
- Where: `omega-rust/psi/interpreter/terminal-interpreter/src/storage_references.rs:4455`; `omega-rust/psi/compilation/representations/terminal-psi/src/ownership.rs:62`; `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/references.rs:887`; `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_reference_custody.rs:4600` and `selection.rs:877`; `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:5124` and `:14940`; `omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider/reference_outputs.rs:350`
- Saw: four copies agree (equal, or `Owned` covers all, or `MutableBorrow` covers shared and write-only), and the stage 02 and stage 03 legalization copies are the same text; `unit_validation.rs:5124` drops write-only; `selection.rs:877` lets owned or mutable cover shared only; `reference_outputs.rs:350` lets mutable cover shared and gives `Owned` nothing extra; `ownership.rs:62` never admits an `Owned` request
- Cost: nothing at any site says whether its table differs on purpose, so a fix to one rule cannot be checked against the others, and stage 03's audit of stage 02 shares the copied table, so it cannot catch a mistake in it
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### "Which state does a machine start in" is answered by four different rules
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/build-time-evaluation/src/machine_execution.rs:737` and `:1963`, `.../build-time-evaluation/src/const_evaluation.rs:2904`, `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs:1658` and `:536`, `omega-rust/psi/compilation/validation/src/call_cycles.rs:3248` (used by stage 05 at `flow_state_values.rs:2824`)
- Saw: three identical build-time copies pick the state named like the machine's last `::` segment, else the first state; stage 02 picks that name, else a state named `entry` (`:1658-1672`), and elsewhere the first State child (`:536-541`); the checker takes index 0
- Cost: a machine whose first state is not the one named after it starts in a different state at build time than in the checker, and a fix to the rule has to be found and repeated in six places across three crates
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A folded integer travels as a type named by its digits, and each reader parses the name back
- Category: one idea, several implementations
- Where: written at `omega-rust/psi/compilation/build-time-evaluation/src/const_evaluation.rs:853-856`; read at, for example, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_current_scalar_values.rs:605-611` and `.../src/published_facts.rs:5120-5121`
- Saw: build-time evaluation stores a value as `TypeReferenceNode::Named { symbol: invalid, name: <spelling> }`; a search for `name.as_str().parse::<integer>` finds 20 lines in 16 files across typed-trees, stages 03 to 05, proof and validation (the two read sites cited were opened; the rest are search hits)
- Cost: this is the integer sibling of the `#omega-const:` entry below: there is no value node, so every new reader must remember that an unresolved type name may be a number, and one that forgets treats it as an unknown type
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### An integer const identity is built as text in one crate and parsed back from text in four
- Category: one idea, several implementations
- Where: built at `omega-rust/psi/compilation/representations/typed-trees/src/calls_and_control_flow.rs:990` and `typed-trees/src/lib.rs:1783`; parsed at `calls_and_control_flow.rs:978`, `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/monomorphization.rs:2611`, `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit_arguments.rs:6674`, `omega-rust/psi/interpreter/terminal-interpreter/src/engine_calls.rs:3711`
- Saw: the string `named(integer-const(N))` is made with `format!`, then each reader strips the prefix and suffix by hand and parses an `i128`; the Terminal interpreter picks a dynamic family member this way, and `terminal-psi/src/boundary.rs:555-556` interns two such strings as literals
- Cost: a change to the spelling must be made in four crates at once, and a reader that misses the form fails only at run time (`None`, or "dynamic family tuple identity is not an integer constant")
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A generic parameter's kind is numbered four different ways, and one numbering is parsed back out of bytes the old checker wrote
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:1076-1087`, `.../06_checked-trees-to-lowered-psi/src/proofs.rs:6887-6905`, `omega-rust/psi/compilation/validation/src/machine_calls_core.rs:3081-3088` and `:4090-4105`, `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/calls.rs:995-1011`, `:1239-1245`, `:1479-1482`
- Saw: `TypeParameterKind` becomes Type 0, Const or Value 1, Proposition 2, Machine 3 or 4, Implementation 5 in `retention.rs`; Type 1 to Proposition 6 in `proofs.rs` and at `machine_calls_core.rs:4099`; a third order (Value 5, Machine 3, Proposition 4) at `machine_calls_core.rs:3082`; `proofs.rs:6887` asks the old checker for the `omega.requirement-schema.v3` bytes and Terminal semantics walks those same bytes by hand at `calls.rs:995-1011`
- Cost: no shared constant or type ties the tables together, so a new parameter kind must be repeated by hand in three crates, and the byte layout the Terminal validator parses is owned by the crate being removed
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The four hosted console and exit providers are recognised by name in four places beside the catalog that says it is the only one
- Category: one idea, several implementations
- Where: `omega-rust/psi/foundation/language-core/src/sealed_intrinsics.rs:1-6` (the catalog), `omega-rust/omega/compilation/realization/provider-planning/src/intrinsic_declarations.rs:405-438`, `.../provider-planning/src/provider_planning/provenance_replay.rs:226-260`, `omega-rust/psi/interpreter/checked-interpreter/src/execution.rs:992-999`, `omega-rust/psi/compilation/validation/src/value_custody_core.rs:5255-5259`
- Saw: the catalog header says every stage reads this table instead of keeping its own member-name recognizer; the other four sites each match the literal strings `ConsoleNativeProvider::exit_process`, `::write_byte`, `::read_byte` and `ProcessExitNativeProvider::exit_process` on their own
- Cost: adding or renaming a compiler-owned provider is five edits in four crates, and the catalog header tells the next agent it is one row
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Structural size and alignment is computed three times, and each copy aligns a wide integer to 8 alone but 16 as a field
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/backend/calling-conventions/src/terminal_layout.rs:398`, `:407`, `:458`; `.../calling-conventions/src/structural_shapes.rs:478`, `:487`, `:534`; `omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs:4568`, `:4954`, `:4962`
- Saw: three hand-written walks over the same structural type declarations in two crates; in all three a standalone integer scalar gets `next_power_of_two().min(8)` and the same integer as a record field gets `.min(16)`
- Cost: a layout fix has to be made three times, and the 8 or 16 answer depends on which function is asked, so the copies agree with each other while possibly disagreeing with the ABI
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A structural path segment has five hand-written byte encoders with three different numberings
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/identity.rs:213` and `.../optimization-unit/src/ownership.rs:171`, `.../selected-instructions/src/effects/program/identity.rs:476`, `.../selected-instructions/src/plan_identity.rs:606`, `.../target-operations/legalized-operations/src/identity/structural_types.rs:208`
- Saw: each file matches on all eight `StructuralPathSegment` variants and writes its own tag bytes; `RuntimeIndex` is 5 in three of them, 6 in the machine-effect identity and 4 in the selected plan identity, where `Field`, `FixedIndex`, `Referent` and `FixedRange` are also renumbered
- Cost: a new path segment kind needs five encoders and at least two decoders (`register-homes/src/recovery/structural_declarations.rs:714`, `selected-instructions/src/effects/program/encoding.rs:1654`) edited by hand, and nothing says which numbering is the reference
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Stage 01 builds its control-flow graph from two terminator kinds while every node already lists its successors
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/control_flow_analyses.rs:303-323` and `:195`, `.../src/semantic_analyses.rs:2241-2249`, against `.../optimization-unit/semantics/src/candidates.rs:3224-3238`
- Saw: the stage 01 graph and its value liveness add edges only for `Jump` and `Conditional` (`_ => {}` for the rest), so `StructuralCase` arms get no edge; the rewrite validator's dominators, the sparse constant pass (`semantic_analyses.rs:1838`) and the cycle graph (`ranked_cycles_validation.rs:737-763`) do follow case arms; a third dominator solver sits in `optimization-unit/src/dominance.rs:53`
- Cost: the stage that proposes rewrites and the crate that validates them disagree on which blocks are reachable and dominated; whether a function with a `StructuralCase` reaches the two-kind graph was not traced
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The Omega unit validator carries copies of two Terminal Psi codec checks
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs:17909` and `:17846`, against `omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs:8566` and `:11744`
- Saw: `attachment_storage_types` differs between the two files in two lines (visibility and one `use`); `validate_structural_type_graph` is the same walk with a different error type and lookup
- Cost: the type-graph rules live once on each side of the Psi/Omega boundary, so a fix on one side leaves the other accepting or rejecting something different, with no test found tying them together
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Package review recovery has no arm for four intrinsic tags its own encoder writes
- Category: one idea, several implementations
- Where: `omega-rust/omega/packages/review-evidence/src/encoding/encode/values/providers/intrinsics.rs:14`, `:17`, `:69`, `:101`, against `.../encoding/recovery/policy/intrinsic.rs:14-50`
- Saw: the encoder writes tags 16 (`empty_run_release`), 17 (`static_interrupt`), 11 (`access_lifecycle`) and 8 (`accessor_scalar_access`); the recovery match has arms 0 to 7, 9, 10 and 12 to 15, and its `tag!` macro rejects anything else
- Cost: the encode and recovery tables are kept in step by hand and have already drifted, so a baseline naming one of these four intrinsics cannot be read back; the review cache reads through `.ok()?` (`package-manager/src/review/candidate/review_cache.rs:108`), so nobody is told
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The source span that identifies a called declaration is computed twice, in a different order
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs:493-549` and `omega-rust/psi/compilation/build-time-evaluation/src/const_generic_expressions.rs:43-100`
- Saw: two functions named `call_custody_source` sharing two error strings; stage 02 tests the operator, machine-parameter or trait-state kind before the plain source span, the build-time copy tests the plain span first (`:55`); for a source-less entry, stage 02 takes the first State child and the copy demands exactly one owning machine
- Cost: a constant's recorded call source can differ between resolution and build-time evaluation for the same target, and whoever changes one copy has no pointer to the other
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### "Is this domain exposed to this reference" has a shared predicate and two private rewrites
- Category: one idea, several implementations
- Where: `omega-rust/psi/foundation/symbols/src/table.rs:963` (comment at `:937-942` calls it "this shared predicate"), `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs:2598`, `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs:482`
- Saw: the shared one checks visibility, same package and module, ambient toolchain symbols and what each import binding exposes; the stage 02 copy compares module names and import paths as strings with no visibility or package test; the stage 03 copy compares module symbols, matches import paths as strings and answers false when there is no reference span
- Cost: a change to import or visibility rules made in the symbol table does not reach operator-home selection in stage 02 or domain lookup in stage 03, so the stages can disagree on which domain a name selects
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### `match_pattern_dependencies` is written three times and two copies leave out the destructure fields
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/representations/syntax-trees/src/values_and_types.rs:1507-1534`, `.../symbol-resolved-trees/src/values.rs:2831-2860`, `.../typed-trees/src/values.rs:3113-3142`
- Saw: the syntax-trees copy passes `pattern.fields` and lists every field `Equal` value; the symbol-resolved and typed-trees copies pass `HandleSpan::empty()` for a destructure pattern, so their loop over fields never yields anything, although both pattern structs carry `fields` (`symbol-resolved-trees/src/values.rs:3635`, `typed-trees/src/values.rs:4986`)
- Cost: a caller asking "which expressions does this pattern depend on" gets a different answer depending on which tree it holds, and the leftover field loop makes the two short copies look complete
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Two rules for "this operator use is builtin" disagree about an unresolved operator
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/representations/checked-trees/src/facts.rs:244-268` (`has_builtin_operators`), `.../checked-trees/src/operators_and_proof.rs:477-486` (`resolved_through_builtin_fallback`)
- Saw: the first treats any status other than `BuiltinFallback` as not builtin and answers `true` for every node that is not Binary, Unary or Borrow without looking at its children; the second also counts status `Missing` with zero candidates as builtin; six stage 05 files call one or the other
- Cost: whether an operator with no resolution counts as builtin depends on which helper a check happened to call, and a fix made in one does not reach the other
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Lowering, the proof crate, the checked interpreter and package review all call into the old checker
- Category: knowledge crossing a boundary
- Where: `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/Cargo.toml:30`, `omega-rust/psi/compilation/proof/Cargo.toml:25`, `omega-rust/psi/compilation/psi-compiler/Cargo.toml:19`, `omega-rust/psi/interpreter/checked-interpreter/src/execution.rs:17`, `omega-rust/omega/packages/review-evidence/src/capture/`
- Saw: stage 06 names over 80 distinct `validation::` items in 161 places across 16 of its 19 files (`stored_structural_type_identity` 15 times, `CanonicalPlace`, `linear_claim_frontier`, `canonical_requirement_schema_bytes`); the proof crate names 57 items in 171 places and takes its program as `validation::CheckingInput`; the checked interpreter names 20 on 32 lines; review-evidence names 56 on 144 lines in 68 files; stage 08 links the whole crate for one function, `float_saturation_divisor_parameter`
- Cost: the stage 06 header (`src/lib.rs:11-16`) says lowering reads only stage 05's frozen facts, yet none of these answers is in Checked Psi, so removing the old checker breaks lowering, proof, build-time execution and review capture at once; the entries below name only stages 03, 04, 05 and 08 as importers
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The old checker holds the steps that rewrite the typed program, and six crates call them
- Category: knowledge crossing a boundary
- Where: `omega-rust/psi/compilation/validation/src/machine_calls_core.rs:7704`, `:8935`, `:9506`; `literals_and_struct_literals.rs:350`, `:3026`; `traits.rs:5963`, `:5967`; `type_references_and_wire.rs:3266`; `declarations_core.rs:1580`; `frozen_program_and_queries.rs:2295`
- Saw: at least ten public functions take `&mut TypedTrees` (overload resolution, literal landing, float cast binding, terminal transition normalising, dynamic call targets, open index normalising, source completion); 22 call lines outside the crate, in stage 03 (3), stage 04 (10), stage 05 (1), `build-time-evaluation` (6), `omega/compilation/compiler` (1) and `psi-compiler` (1)
- Cost: these are not checks, so a replacement checker will not bring them along; removing the crate strands six callers or silently drops a rewrite that later stages assume has run
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Build and product services that the Omega side calls live inside the old checker
- Category: knowledge crossing a boundary
- Where: `omega-rust/psi/compilation/validation/src/frozen_program_and_queries.rs:1733` (`execution_projection`) and `:3851` (`product_queries`); `lib.rs:554` (`product_entries`) and `:923` (`native_locators`); `declarations_core.rs:1562` (`source_completion`); `effect_inference.rs:1179`, `:2727`
- Saw: called from `omega/build-system/build-settings/src/admission/checking.rs:107`, `:133`, `:200` and `program.rs:98-99`; `omega/compilation/compiler/src/product_boundary.rs:48` and `checked/checking/source_checking.rs:160`; `psi-compiler/src/source_compilation.rs:785`, `:982`; `terminal-production/src/build_bindings.rs:170`; `checked-interpreter/src/build_machine.rs:1220`
- Cost: program projection, root entry selection and native binding widths have no home outside the old checker, so the build system and the driver cannot drop the dependency until each is moved
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The default ranking view for a single-subject `terminates by` is derived in three stages
- Category: re-deriving
- Where: `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/lowering_machine.rs:2007-2088`, `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs:174-245`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_termination.rs:2911-2937`
- Saw: stage 02 finds the parameter by name text and also accepts a `.len` member; stage 03 finds it by symbol, has no `.len` rule, stops after 128 steps and runs only when stage 02 left the view empty; stage 05 decides again with its own primitive test; the stage 02 comment (`:1997-2005`) says it mirrors the checker "EXACTLY"
- Cost: nothing holds the three in step except that comment, so adding a rankable type means editing three stages, and missing one leaves some machines with a view chosen by a rule the checker does not share
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

### Every obligation becomes an "accepted" fact, and no later stage can tell a proven one from an unproven one
- Category: a verdict with nothing behind it
- Where: `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/verified_unit.rs:194-400` and `:412-425`; readers at `.../01_.../src/rules.rs`, `.../03_target-operations-to-selected-instructions/src/legalization.rs:234-244`
- Saw: `source_checked_obligation_facts` pushes one `AcceptedObligationFact` per obligation reference; the proof fingerprint is 32 zero bytes unless a seal names it, and the comment says no module the current pipeline produces has seals; the only readers of the fingerprint are a measurement printout, the identity hash and one equality against a proof question; stage 03 takes the first matching fact and needs only that it exists
- Cost: the names "accepted" and "source checked" read as proof; stages 01 and 03 remove or omit runtime checks on the strength of a fact that only records that an obligation was stated
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A range obligation the certificate builder cannot cover is accepted by the old checker's interval helpers instead
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/compilation/proof/src/checker.rs:373-398`, `:547`, `:623`, `:689-696`, `:762-787`
- Saw: each of the five `check_bounded_*` functions has a `CertificateVerdict::Uncovered` arm that raises no diagnostic when a local interval fits, or when `validation::occurrence_integer_expression_fits_range` or `validation::arrival_integer_expression_bounds` says so (called at `:382`, `:556`, `:696`, `:787`); no certificate reaches the admission kernel on that path
- Cost: the crate header (`proof/src/lib.rs:14-20`) describes a kernel that "independently re-decides every emitted certificate", which reads as "every accepted obligation was re-decided"; and whoever removes the old checker has to decide what these arms accept afterwards
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

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

### A cloned generic machine is always marked as having its type predicates settled
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/body_cloning_and_rewriting.rs:717-757` and `:993`
- Saw: `let complete = true;` is never reassigned; a `requires` type predicate rejects the clone only when it evaluates to `Some(false)`, an unknown result passes, and `:993` then sets `structural_type_equations_pending = !predicates_closed`, which is always false for a clone
- Cost: build-time evaluation, validation, structure elaboration and stage 05 all branch on that flag (10 read sites by search), so for clones each of them is told "nothing pending" whether or not the predicate was decided
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Stage 03 skips the exhaustiveness check for any dispatch run that has a continuation
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs:932-934` (flag set at `:858`)
- Saw: `check_dispatch_run` returns `Ok(())` as soon as one transition in the run has a valid `continuation`, before any arm is examined; no comment gives the reason; whether stage 05 catches a missing arm in such a run was not traced
- Cost: a reader takes stage 03 as the owner of "every case is covered" when for these runs it decides nothing, and the question of who does is left to whoever next breaks it
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Two places count as the same place when their printed labels are equal
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/compilation/representations/checked-trees/facts/src/lib.rs:1831-1872` (`places_match`), label written at `.../facts/src/contexts_and_places.rs:1272-1341`
- Saw: when root and segments differ, `places_match` writes both places as text and compares the strings; a symbol is written as its bare name (`contexts_and_places.rs:1337`) and every invalid symbol as `unknown` (`:1339`); stage 05 calls it six times in `checks_contracts_prover.rs`, `checks_contracts_calls.rs` and `flow_calls.rs`
- Cost: two different declarations that share a spelling, or two unresolved members, compare equal here; whether a caller can be handed such a pair was not traced, so this is a place to examine and not a shown wrong accept
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A stale or invalid arena handle reads a default row, and a write through one changes the shared default
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/foundation/arena/src/arena.rs:1023-1038`, `:1113`; `paged.rs:462-476`, `:1093-1095`
- Saw: `get` returns the dummy row when the index or generation does not match; `get_mut` returns `&mut self.dummy`; `span_or_empty` turns a span it cannot resolve into an empty slice; the crate header (`lib.rs:20`) gives the reason for handle zero only, not for a stale generation
- Cost: every table in the Psi half sits on these containers, so a dangling handle anywhere looks like an empty declaration, not an error, and the next reader of the dummy sees whatever was last written to it
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The Terminal interpreter reports a failed run-time check as a malformed artifact, with one error for 1,329 sites
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/interpreter/terminal-interpreter/src/boundary_values.rs:696`; example at `engine_execution.rs:3691`
- Saw: `malformed()` returns `VerifiedOperationMalformed` and is called 1,329 times across 15 files; an exact add, multiply or divide whose result does not fit ends in `.ok_or(malformed())?`; a program crash is built on only 7 lines (trapping integer, trapping float, crash terminator, host effect); no crate outside the interpreter names the variant
- Cost: an overflow or zero divisor that native code would trap on shows up as "the verified artifact is malformed" with no reason, so a native and Psi disagreement reads as an interpreter failure and the cause must be found by hand
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Windows x64 and UEFI x64 are the same `NativeTarget` value, so "UEFI only" checks also pass for Windows
- Category: a verdict with nothing behind it
- Where: `omega-rust/omega/compilation/backend/target/src/target_profile.rs:695-711`; checks at `.../images/image-emission/src/function_fragments.rs:1883`, `omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow.rs:476`, `omega-rust/omega/compilation/realization/provider-admission/src/provider_admission/compiler_builtins.rs:176`, `.../isa-x86_64/src/semantic_unit_wrapper_encoding.rs:408`
- Saw: `windows_x64()` and `uefi_x64()` return identical fields (x86-64, COFF, 8-byte pointers) and the type has no operating-system field; at least 8 non-test sites compare against `NativeTarget::uefi_x64()`, one with the message "image residence requires the selected UEFI image"
- Cost: an agent reading those checks believes Windows is refused there; whether it is depends on a separate `TargetProfile` check somewhere else, which each site leaves unstated
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A missing target profile is replaced by the build host at 16 places
- Category: a verdict with nothing behind it
- Where: `omega-rust/omega/build-system/build-settings/src/admission/checking.rs:149`, `:190`; `omega-rust/omega/compilation/compiler/src/checked/target_preparation.rs:104`, `execution_settlement.rs:286`, `:368`, `:412`, `product_execution/component.rs:132`, `terminal/terminal_artifact.rs:238`, `:365`, `:389`; `omega-rust/omega/compilation/realization/provider-planning/src/provider_settlement.rs:246`, `:395`, `target_machines.rs:143`, `:167`, `target_machines/prepared_providers.rs:352`, `provider_planning/provenance_replay/plan_derivation.rs:192`
- Saw: 16 `unwrap_or_else(TargetProfile::host)` calls across three crates; `checking.rs:171-175` reports "Build provider planning requires an admitted execution profile" for the same absence when candidates exist
- Cost: a caller that forgets to pass the target gets host answers with no diagnostic, which only shows up as a wrong result when cross-compiling; nothing says which of the 16 are meant
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Architecture rules are enforced by searching source text for substrings
- Category: tests that pin the bug
- Where: `omega-rust/omega/compilation/compiler/src/product_boundary.rs:8-14`, `omega-rust/omega/build-system/orchestration/src/build_boundary.rs:200-214`, `omega-rust/omega/compilation/native-compiler/src/pipeline_boundary.rs`, `omega-rust/omega/compilation/backend/plans/program-entry-plan/src/source_guards.rs:24-28`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/immutable_checking_input.rs:8-130`, `omega-rust/psi/foundation/access-plans/src/tests.rs:3273-3301`, `omega-rust/psi/interpreter/terminal-interpreter/src/embedding.rs:1040-1058`
- Saw: 52 tests in the three Omega boundary files read 108 `include_str!` sources and assert that strings such as `build_settings::scalar_completion::complete(` are absent or appear in order; the program-entry-plan guard still allows five rows for `selected_entry/service_establishment.rs`, a file that does not exist; the stage 05 "checking never rewrites the typed program" guard asserts that about 30 literal strings such as `"&mut program"` and `"RefCell"` do not appear, and reads two files of the old checker to do it; the last two count substrings (`Fnv1a::new()` appears 3 times)
- Cost: a renamed import or a differently spelled borrow passes while breaking the rule, and a harmless reformat fails; agents learn to edit the string list, the stale rows show nobody notices when a guard stops guarding, and removing the old checker breaks the stage 05 guard for a reason unrelated to what it guards
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The "no string identities in checked trees" guard only sees six exact spellings
- Category: tests that pin the bug
- Where: `omega-rust/psi/compilation/representations/checked-trees/src/lib.rs:97-150` (`checked_trees_carry_no_string_identity_fields`)
- Saw: the test searches the source text for `identity: String`, `identity: Option<String>` and four more spellings, with six exemptions; the crate has 67 public fields typed with `String`, and text identities under other names pass unseen, for example `registrar_declaring_application` (`facts.rs:2161`), `canonical_requirement_overload` (`facts.rs:2307`), `declaring_application` (`flow_terminal.rs:778`), `captured_argument_identities: Option<Vec<Option<String>>>` (`operators_and_proof.rs:1506`)
- Cost: the test passes whatever is added as long as the field is not literally named `identity`, so its assert message ("must carry identity handles, not text identities") reads as a guarantee the crate does not have
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

### The installed-code path is types and plans with no producer and no caller
- Category: zombie machinery
- Where: `omega-rust/omega/compilation/backend/runtime/executable-installation/` (1,062 lines), `omega-rust/omega/compilation/backend/plans/program-entry-plan/Cargo.toml:13` and `src/lib.rs:28`, `:41-47`, `omega-rust/omega/compilation/backend/images/image-emission/src/installation_record.rs:36`
- Saw: no struct literal anywhere builds `InstalledCode`, `CodePlacement`, `FrozenPlacement`, `ValidatedPlacement`, `AdmittedArtifact` or `ArtifactRecord`; the feature `installed-writer` that compiles the 330-line `post_handoff_writer.rs` is enabled by no manifest in the repository; the three exported entry points of `optimized_semantic_entry.rs` and `optimized_semantic_wrapper.rs` (1,183 lines) have no caller beyond their re-export; `InstallationError` in image-emission has no constructor
- Cost: about 2,500 lines look like a working installation pipeline and are compiled, linted and kept in step by hand, some of it not even built by default
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The placement and primitive-request half of `access-plans` is called only by its own tests
- Category: zombie machinery
- Where: `omega-rust/psi/foundation/access-plans/src/placements.rs` (3,028 lines; `admit_access` at `:244`, `admit_owned_access` at `:264`, `adopt_owned_stable` at `:1754`), `primitive_access.rs` (1,251 lines), `plan_policy.rs:694`
- Saw: no file outside the crate names `admit_access`, `admit_owned_access`, the three `adopt_owned_*` functions, `bind_schema_correspondence_to_access_plan`, `effect_footprints_conflict`, `PrimitiveAccessRequest`, `EstablishedOwnedAccess`, `SchemaCorrespondedAccessView` or `AccessAdmission`; inside the crate only `tests.rs` (7,168 lines, 109 tests) calls them; twelve crates depend on the crate for its validators
- Cost: the crate header (`lib.rs:9-15`) says every stable, atomic or external access request is authorized before lowering, and a reader will believe a pipeline stage does that when none calls it
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Exported verifiers that the product never calls, one of them with no caller at all
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/pipeline/08_lowered-psi-to-terminal-psi/src/result_policy_verification.rs:277`, `omega-rust/psi/compilation/representations/terminal-psi/codec/src/mathematical_proof_actual.rs:593`, `:653`, `:843`, `.../codec/src/identity_formations.rs:115`
- Saw: `verify_boundary_result_policies` has 4 hits in `omega-rust`, all in its own crate (definition, re-export, two comments), and it holds that crate's only use of the old checker (`:419`); `verify_mathematical_proof_actual` has a definition and a re-export only; `verify_module_mathematical_actuals`, `verify_indexed_theorem_roots` and `verify_identity_formations` are called only from `omega/compilation/compiler/tests/support/`
- Cost: the stage 08 header (`src/lib.rs:1-7`) says the stage verifies boundary result policies, so a reader assumes that check runs on every program; stage 08 also keeps a dependency on the crate being removed for code nothing reaches
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The proof crate ships a derivation cache, a lemma table, boundary types and a surface report that nothing outside it uses
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/proof/src/derivation_store.rs` (299 lines), `proof/src/checker.rs:2857-2978` and `:6550-6575`, `proof/src/lemmas.rs` (377 lines), `proof/src/boundary.rs` (213 lines), `proof/src/lib.rs:39-109`
- Saw: the one production entry passes `None` for the cache (`checker.rs:6568`); `check_proof_plan` (`:6550`) has no caller; the `lemmas.rs` and `boundary.rs` types occur in no other file; `build_proof_surface_report` is called only from tests in `lib.rs`; `recipe_obligations` is created at `:6578`, passed to five functions as `_recipe_obligations`, never pushed to, and returned as `constant_recipe_obligations`, which stage 05 then reads
- Cost: the crate header (`lib.rs:1-31`) presents caching and lemma vocabulary as working parts, and the store's header says "Persistent" and "serialize" although nothing found puts a cache in front of it; `checked-trees` carries a second copy of the same five lemma names (`operators_and_proof.rs:2054-2104`) that also has no user outside its crate
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### About a thousand lines of "acceptance views" in checked-trees have no caller and can only say accepted
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/representations/checked-trees/src/admissibility_and_borrow.rs:7-1049` (`pub mod admissibility`), re-exported at `checked-trees/src/lib.rs:507`
- Saw: `AcceptanceView`, `StateAcceptance`, `CallAcceptance` and the related names occur 90 times, all in this crate plus one README line; the entry point `state_acceptance` (`:412`) is never called; every `summary()` builds `AcceptanceSummary::accepted(...)`, and `AcceptanceCheck::rejected` (`:730`) is defined and never called
- Cost: the stage 05 README (`05_typed-trees-to-checked-trees/README.md:47`) points readers at these views as the way to gather accepted evidence, and a new consumer would get "accepted" for anything
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Public front-end entry points that nothing calls
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/build-time-evaluation/src/lib.rs:920-926` re-exporting `const_evaluation.rs:1689`, `:2521`, `:4741`; `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs:1767` and `resolution.rs:2366`
- Saw: `evaluate_const_domain_facts`, `evaluate_const_array_lengths` and `evaluate_const_range_endpoints` appear in `omega-rust/`, `tests/` and `tools/` only as definition and re-export (production calls `evaluate_const_array_lengths_in`); `select_data_declarations` is a definition, a doc mention and a re-export; `resolve_numeric_probe` is called only by a test at `resolution.rs:2807`
- Cost: they read as the crates' public API, so the next agent wires a new caller to an entry the real driver never exercises, or keeps them compiling through a refactor for nobody
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Two stage 05 decisions are hard-wired but still written as if they were live
- Category: zombie machinery
- Where: `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts_crash_entry_values.rs:1262-1273` and `.../src/checks_multiplicity_linear_validation.rs:1071-1101`
- Saw: `receiver_call_writes` searches the parameters with `.find(|_parameter| false)`, so it returns true for every callee, though its comment says it depends on whether `self` is mutable (two callers, `:1055` and `:1223`); `owned_receiver` is `false && ...`, so the branch calling `validation::owned_receiver_claims` never runs, and that is the function's only caller in `omega-rust/`
- Cost: both read as working logic, so a reader trusts the comment over the constant, and `owned_receiver_claims` looks like something stage 05 still takes from the old checker when the call is dead
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The specification and AGENTS.md describe `--validator` and `--every-validator`; the compiler has neither
- Category: the documents disagree
- Where: `wiki/spec/build/command_line.md:17`, `:130-151`; `AGENTS.md` ("explicit validator requests"); `omega-rust/omega/application/cli/src/arguments/compile.rs:14-18`
- Saw: the spec defines named stage-output validators and AGENTS.md says verifiers run "on explicit validator requests"; a search of `omega-rust/` and `source/` finds no occurrence of either option, and the only related flag, `--verify`, withholds the lock's accepted checkpoint
- Cost: the rule "default compilation does not re-verify its own stage output" rests on an opt-in that cannot be opted into, so the stage validators have no user-reachable trigger except `omega run --both`
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### `pipeline.md` says the default compile runs no stage validators; two run, and one stage header misdescribes its own
- Category: the documents disagree
- Where: `omega-rust/pipeline.md:66-68` and `:91-93`; `omega-rust/omega/compilation/backend/machine-emission/src/exit_contract.rs:153` and `:205-222`; `omega-rust/omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding/src/lib.rs:45-47` against `.../src/validation.rs:630`, `:639` and `.../src/row_encoding.rs:661`, `:670`
- Saw: `stage_whole_function_exit_contract_for_layout`, reached from `native-compiler/src/pipeline.rs:196`, calls the stage 08 and stage 09 validators on every compile, and the stage 08 header says so; the stage 03 audit is called only from `compiler/tests/corpus_runner.rs:1193`, `:1203`; the stage 07 header says its replay "never calls the producer's route table or encoders", yet it calls the same dynamic-call template encoders the producer calls
- Cost: an agent can cite the document to remove the stage 08 and 09 checks or to add more, and cannot tell from headers which checks are separate work and which reuse the producer
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### An evaluated const operand is recognised separately by at least three readers
- Category: one idea, several implementations
- Where: `omega-rust/psi/compilation/representations/typed-trees/src/type_identity.rs` (`index_argument`, `StaticArgument` arm), and two readers in the validation crate (`close_const_argument`, `validate_types`)
- Saw: build-time evaluation folds a closed record operand into a type reference named `#omega-const:...`; each reader decides on its own whether a `StaticMachineArgument` is a const, and each missed this form, so the program failed at one reader after another
- Cost: a fix at one reader moves the failure to the next; about 96 `ConstExpression` matches suggest more readers exist
- Reported: 2026-11-11, Claude Code, while repairing a record-literal operand `category_error`

### Earlier stages and later representations import the stage 05 output type
- Category: knowledge crossing a boundary
- Where: the `Cargo.toml` dependencies of stage 03, stage 04, `lowered-psi`, stage 08 and `build-time-evaluation` (read with `cargo metadata --no-deps`)
- Saw: stage 04 depends on `checked-trees`, which stage 05 produces after it; the `lowered-psi` representation and stage 08 depend on it too; `build-time-evaluation` depends on the stage 05 crate itself
- Cost: "each stage knows only its input and output" holds on paper while one stage's output type is visible up and down the pipeline
- Reported: 2026-11-12, Claude Code, while mapping what depends on the old checker before its removal

### `terminal-installation` lives under the validation directory but serves the whole back half
- Category: knowledge crossing a boundary
- Where: `omega-rust/psi/compilation/validation/terminal-installation/`
- Saw: about twenty backend, interpreter and CLI crates depend on it; it sits inside the directory of a crate being deleted
- Cost: its location says "old checker" and its users say "shared runtime contract"; it will be swept up or left stranded by the removal
- Reported: 2026-11-12, Claude Code, while mapping what depends on the old checker

### A corpus check can stop running without anything noticing
- Category: tests that pin the bug
- Where: `omega-rust/omega/compilation/compiler/tests/corpus_runner.rs`, fixtures carrying `expected_native_rejection.txt`
- Saw: the reader of that file lived in Python tooling deleted by #2086; for weeks nothing read it, nine ledger rows still claimed a match, and three regressions hid behind it until #2564 restored the reader
- Cost: nothing asserts that every `expected_*` file a fixture carries has a reader, so the next deleted check fails the same silent way
- Reported: 2026-11-11, Claude Code, while repairing three `scalar_reference_*_backing_foreign_export` rows

### `pipeline.md` blesses a shared pool of judgments beside the pipeline
- Category: the documents disagree
- Where: `omega-rust/pipeline.md` ("`psi/compilation/validation/` owns reusable judgments"; "none of these is a pipeline program IR")
- Saw: a third category next to stages and representations, with no limit on size or on who imports it; it grew to 344,661 lines imported by stages 03, 04, 05 and 08
- Cost: the next agent can rebuild the pool after the removal and cite the document for it
- Reported: 2026-11-12, Claude Code, while answering how the old checker got that large

### One fixture takes minutes in the front end on a shared workstation
- Category: verifying costs more than writing
- Where: `omega --check` on a single `tests/omega` fixture; `tests/psi/checking` harness
- Saw: 220 to 420 seconds per fixture at Idle priority with other builds running; one `tests/psi/checking` pass took 2,067 seconds and about 11 GB
- Cost: agents validate with a few named probes instead of the corpus, and skip coverage tools; a wider check is not practical locally
- Reported: 2026-11-12, Claude Code (two workers), while building lowered-form checker lanes

### Build activation, library slices and sibling-target checks accept on the old checker alone
- Where: every `psi_compiler::CheckRequest::check()` caller other than the product check in `omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs` (Build activation, library slice checks, sibling targets)
- Saw: only the product check shows its trees to the lowered-form checker; the others call `check()`, which runs Psi 05 with no second verdict, so a Build program or a library slice is still accepted with no evidence the proof kernel re-checked
- Cost: after the 2026-11-12 cut the compiler reads as if one checker decides everything, while every compilation still depends on these unbacked acceptances until Psi 05 is deleted
- Reported: 2026-11-12, Claude Code, while removing the fall-back to the old checker

### Fail fixtures pin the old checker's wording for a failed call requirement
- Where: `tests/omega/fail/calls/free_machine_named_transition_rejected/expected.txt` and its family
- Saw: the fixture pins "value call to `scan` violates required fact `value == 41`"; the lowered-form checker says "cannot prove requires contract for call scan from Main::main: 40 == 41 ...", so the row is red although the program is rejected for the right reason
- Cost: these rows stay red after the old stages go until the new checker names the callee's clause, and they read as checker failures rather than a wording gap
- Reported: 2026-11-12, Claude Code (two workers), while removing the old checker

### Nothing live judges that an implementation refines its trait requirement
- Category: a verdict with nothing behind it
- Where: `lowered_discharge::prove_callable_refinement` and `structure_elaboration::elaborate_witness`; their only callers are `psi/compilation/validation/src/contract_entailment.rs` (frozen for deletion) and a test support file
- Saw: the lowered-form checker refuses every program that declares a trait (`declaration trait` in `structure-elaboration/src/read/subjects.rs`), so the refinement proof it owns is never run by `judge_compilation`
- Cost: when `validation` is deleted the member-refines-requirement rule has no checker until `judge_compilation` judges each conformance row; 1,344 corpus programs declare a trait
- Reported: 2026-11-13, Claude Code, while closing the generic self-application hole

### The bundled library cannot be loaded since constant evaluation was deleted
- Category: a stage whose producer is gone
- Where: `build-time-evaluation/src/source_frontier.rs` (`evaluate_pre_resolution` returns its input unchanged); Psi 02 `constant.rs` (`validate_scalar_initializer`); `04_typed-trees-to-typed-trees/src/preparation.rs` (`require_no_work`)
- Saw: Psi 02 still requires every scalar constant to be a literal and says richer initializers "arrive with build-time evaluation", but that evaluation (about 36,000 lines) left with the old checker; `source/library/std/targets/uefi_x86_64/handoff.omg:29` (`const A: u64 = B;`) therefore stops every compilation that loads `std`, and Psi 04 accepts only an empty program
- Cost: no source program reaches the lowered-form checker, so no work on the checker can be verified from source until constants have an evaluator and Psi 04 passes a nonempty program through
- Reported: 2026-11-13, Claude Code, while restoring source activation for the route probe

### Typing refuses its own questions as if they waited for the checker
- Category: a stage reaching across a boundary
- Where: `03_symbol-resolved-trees-to-typed-trees/src/lib.rs` (`unavailable_judgment`) and its 26 callers, such as `typing_queries.rs` ("constructor field type", "call entry parameter types") and `type_reference.rs` ("closed const argument declaration custody")
- Saw: every refusal reads "no product route yet: X requires a lowered-form judgment", but most of X is arity, a declared type or a field's owner, which typing decides from declarations; two of them stopped the bundled library (`core::Strict<0>`, `ForeignBinding<N, ...>`) although no proof was involved
- Cost: a reader cannot tell a typing rule that was not rebuilt from a proof the checker must supply, so each stop is investigated from scratch and the wording invites routing typing work to the checker
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing

### An `invokes` row that names an ordinary trait is dropped without a word
- Category: a verdict with nothing behind it
- Where: `typed-trees/src/declarations.rs` (`declared_targets`); `lowered-discharge/src/refinement.rs` (`prove_callable_refinement`)
- Saw: `declared_targets` keeps an `invokes` target only when it is a boundary trait or a parameter of boundary type and silently discards any other, so `invokes Handler` over an ordinary trait reads as no clause at all to every consumer of `declared_machine_invocations`; separately, `prove_callable_refinement` has no caller outside its own tests, so the reach and entry containment it guards is never asked of a real conformance
- Cost: a written entry ceiling can vanish before any checker sees it, and a supplied implementation outside its requirement's ceilings is not refused anywhere in a compile until that operation is wired
- Reported: 2026-11-13, Claude Code, while putting `invokes` on the lowered-form checker

### A whole-program stage refuses every program for a library construct nobody calls
- Category: a verdict with nothing behind it
- Where: `03_symbol-resolved-trees-to-typed-trees` (the remaining `unavailable_judgment` callers, such as "mathematical call receiving types" and "selected call parameter types" in `call_and_product_selections.rs` and `typing_queries.rs`); `build-time-evaluation/src/source_frontier.rs` (`source_jobs`: the side tables it still refuses whole, such as constant recipes and wire plans); any by-name refusal added to `04_typed-trees-to-typed-trees`
- Saw: these stages see the program together with all of `core` and `std` and have no notion of what an entry demands, so one such construct in an uncalled library declaration stops a program that never reaches it; the gates the route probe met (unbound calls, assignment literals, constructor field types, layout-policy and computed types) now record the refusal on the declaration instead (`typed-trees/src/deferred_refusals.rs`), and the others still refuse the program
- Cost: each remaining gate must be served for the entire library before any program compiles, or be moved onto the same carrier one by one as a program meets it
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing

### A generated machine is identified three ways and copied by a hand-kept list
- Category: one idea built twice
- Where: `typed-trees` (`MachineSpecialization` rows, `Symbol::generated_from`, `executable_application_template`); `structure-elaboration/src/lib.rs` (`template_of`), `read/calls.rs` (`applies_nothing`), `declare.rs`; `04_typed-trees-to-typed-trees/src/passes/specialization.rs` (`template`, `held_types`)
- Saw: "this machine was generated from that generic machine" is read from the state symbol's `generated_from` in three places and from the `machine_specializations` row in a fourth, and nothing ties the two; the row also keeps eight string and commitment fields that only the deleted checker filled. Separately, an expression copy shares the types its nodes hold, so the pass lists by hand which node kinds hold a type (`held_types`) and which body constructs it cannot copy
- Cost: a pass that sets one marker and not the other produces a machine the reader accepts and the declaration step refuses, or the reverse; a new expression node that holds a type is silently left pointing at the generic machine's type until someone adds it to the list
- Reported: 2026-11-13, Claude Code, while rebuilding specialization as a typed preparation pass

### An implementation's refinement witness is built twice, and its dispatch row keeps fields nobody fills
- Category: one idea built twice
- Where: `lowered-discharge/src/verdict.rs` (`witnesses`, and the witness parts of `judge_compilation_for`); `typed-trees` (`StaticRequirementDispatch`); `04_typed-trees-to-typed-trees/src/passes/specialization.rs` (`selections`)
- Saw: "this machine refines that requirement" is elaborated and discharged once to carry a generic form's proofs into a generated machine, where a failure is silently skipped, and a second time as the part that gives the verdict; the two share `elaborate_witness` and nothing else. Separately, the dispatch a generated call carries has an application fingerprint, a commitment, a selection application and a contract signature that only the deleted checker filled, so the pass writes four of nine fields
- Cost: every selected implementation is proved twice per compilation, and a reader of the dispatch cannot tell an empty commitment from one that was never computed
- Reported: 2026-11-13, Claude Code, while serving implementation parameters in the specialization pass

### A typing unit test is red on `main` and nothing runs it before merge
- Category: a test that pins nothing while it is red
- Where: `03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs`, `construction_destinations::tests::an_assigned_literal_lands_on_its_binding_or_refuses_its_own_machine`
- Saw: at `6f829063b3ed7` the test fails with no local change: `owned = Record {}` is asserted to be built at the type handle written on `owned` and gets a different handle (index 3, not 1), so either landing now inserts an equal copy of the type or the assertion compares identity where it means equality
- Cost: `mbx test -p symbol-resolved-trees-to-typed-trees --lib` cannot be clean for anyone changing typing, so a second regression in the same crate hides behind the first
- Reported: 2026-11-13, Claude Code, while accounting for domain casts in typing

### The Product admissions of a finished Build are signatures with nothing behind them
- Category: a verdict with nothing behind it
- Where: `build-time-evaluation/src/product.rs` (`native_bindings::collect`, `product_declarations::admit`, `product_policy::*`, `provider_declarations::collect_product_provider_selections`, `retain_provider_defaults`, `RetainedProviderSelection::rejoin`); callers in `psi-compiler/src/source_compilation.rs` (`admit_product_policy`) and `compiler/src/checked/checking/source_checking.rs`
- Saw: the old checker's deletion left each admission as a function that takes the typed program and the Build activation, ignores both, and either refuses or returns an empty list. Until 2026-11-13 each refused any program whose Build ran at all, so six unrelated refusals stood between every project and its Product; the callers still prepare operands and thread results as if producers existed.
- Cost: nobody can tell from the code which Build operations work; the unused parameters and the result types (`AdmittedProductDeclarations`, `NativeBindingSelection`) describe a design that has no implementation and will drift from whatever replaces it
- Reported: 2026-11-13, Claude Code, while taking the witness's finished Build to its Product

### Selected-provider custody is a data model whose "nothing" cannot be constructed
- Category: a verdict with nothing behind it
- Where: `selected-dispatch/src/selected_execution.rs` (`SelectedExecution`), `provider-planning/src/provider_settlement.rs` (`ProviderSettlement`), `provider-planning/src/service_declarations.rs`, `provider-planning/src/target_machines/prepared_providers.rs`, `psi-compiler/terminal-production/src/external_providers.rs`
- Saw: the old checker's deletion kept about thirty retained fields that every product consumer reads (`compiler/src/terminal/terminal_artifact.rs`, `product_execution.rs`, `package_review.rs`), but removed their producers. `ServiceDeclarations` and `ProviderPublicationCatalog` have private fields and no constructor at all; `PreparedExternalProviderDeclarations::prepare` always refuses. So even a program that reaches no boundary requirement cannot be handed on after its providers settle to none.
- Cost: provider settlement can decide a program's supply and still has nowhere to put the answer; rebuilding the producers field by field would restore the old shape, while the consumers only need the plans, the entry and the calling program of the accepted artifact
- Reported: 2026-11-13, Claude Code, while rebuilding target preparation and provider settlement on the accepted Terminal module

### Boundary requirements must write `crashes`, and 390 of the library's 401 do not
- Category: a verdict with nothing behind it
- Where: `source/library/{core,std,blocking-executor}` boundary traits; `01_tokens-to-syntax-trees` (refuses a boundary requirement without `reaches`, says nothing about `crashes`); `structure-elaboration/src/read/calls.rs` (`boundary`)
- Saw: effects.md says a boundary requirement "must likewise write `crashes`" because it has no body to infer from and no selection to defer to. The parser enforces the `reaches` half only, so the library grew 394 requirements with no crash ceiling (core 68, std 316, blocking-executor 10), and the lowered-form checker refuses a call to each one. `Console`'s four were written as `crashes none` on 2026-11-13 because their default provider's public bodies promise that; the rest need the ceiling their providers actually have, which is not a mechanical edit
- Cost: every program that calls a library service other than `Console`'s write and exit operations is refused as not supported, and nothing tells a library author at the declaration that the clause is missing
- Reported: 2026-11-13, Claude Code, while taking the Console program through the route probe
