# Code Repair Queue

Agents fix obvious code-quality and architecture defects without waiting for
the owner. This queue lists concrete structural problems found outside an
agent's current repair, so another agent can fix, validate and land them. An
entry is neither a new language rule nor a reason to stop the current work.

Use [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md) only for truly unsettled language or
high-level architecture choices. Apply the existing
[architecture rules](AGENTS.md#conventions-and-workflow) directly; wide scope,
conflicting edits and difficulty need no decision. Plain missing functionality belongs in its existing outcome in
[TASKS.md](TASKS.md), not here. Refusing with "not implemented yet" fails
closed; it is not an unsound acceptance.

## How to add one

Add an entry when something would mislead the next agent, or when fixing it
properly reaches beyond the crate you are in. An ordinary bug, a style
preference, or anything you can repair inside your own task does not belong
here: just fix it.

The best time is your first hour in an area, before you have your own change to
defend. Name the broken mechanism and what it costs, not a speculative
framework or a list of unimplemented features.

- Add the entry at the top of the list under "Entries"; the list is flat, newest first.
- Name one category from the list below on its `Category` line.
- Keep it to the lines of the template. Link the file and line.
- Add it in the pull request you are already landing; never open one for it.
- Merge overlapping reports into one entry for the defect that remains; do not keep duplicates.
- Whoever fixes it removes the entry when the fix lands. Remove a stale report
  after checking that its code or premise is gone; keep no log of finished
  work, and do not mistake deleting a report for implementing its feature.

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
2. **Knowledge crossing a boundary.** A stage imports something other than its
   input representation, its output representation or a foundation crate.
3. **Branch per shape.** Logic keyed on where a value sits (local, field,
   parameter, match arm) instead of what it is.
4. **Re-deriving.** A later stage recomputes what an earlier one knew, or a
   stage checks its own output again.
5. **Too big to read.** You had to search the file because you could not read
   it.

**Honesty**

6. **A verdict with nothing behind it.** Something is accepted by default, by
   a fallback, or because a lookup returned nothing.
7. **Tests that pin the bug.** A fixture that would still pass if the feature
   were broken, or a check that quietly stopped running.
8. **Zombie machinery.** Code that only tests call, a retired path that still
   receives input, a fallback nobody can remove.

**Direction**

9. **The documents disagree.** You can cite a rule for either answer, or a
   document approves code that is decaying.
10. **Verifying costs more than writing.** You waited longer than you worked.

## Entries

### Structural equality refuses a field of a closed generic type
- Category: knowledge crossing a boundary
- Where: [field type preparation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs), [applied type references](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs)
- Saw: On clean `eca5c0b9d21` and candidate `c64a4d172f9` on macOS arm64, `Player.point: Point<1>` with `PointEquality satisfies Equatable<Point<1>>` refuses that the field is not Equatable before choosing its comparison row. `Point<const Index: u64> [copy]` has one `i32` field. The same nested comparison over non-generic `Point` runs in both engines.
- Cost: Equality preparation has the resolved application and has no comparison over its instantiated fields. Use the existing applied type and member substitution owner so a comparison for `Point<1>` cannot be reused for a different argument or unresolved field.
- Reported: 2026-11-15, Codex, while verifying exact structural equality through aliases.

### A borrowed generic equality formal stops a closed comparison
- Category: knowledge crossing a boundary
- Where: [borrowed formal reading](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/machine.rs#L2742), [authored equality signature](samples/cli/probes/synthesized_equatable/main.omg#L17)
- Saw: On clean `eca5c0b9d21` on macOS arm64, an exact named `Equatable<Player>` row with the declared formal `left: &T` is refused as “parameter reference to a value of a type parameter” even though its actual comparison uses only concrete `Player` values. A separate control with by-value formals and copyable records runs in native and Psi.
- Cost: The equality's selected concrete row cannot bypass the earlier rigid borrowed formal. Use the checked callable's existing requirement and lifetime ownership so admission closes the reference without assigning an arbitrary referent or waiving its loan.
- Reported: 2026-11-15, Codex, while preserving the original reference-signature comparison control.

### A user trait named Equatable receives compiler synthesis
- Category: the documents disagree
- Where: [synthesis selection](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs#L2601), [core equality acquisition](wiki/spec/language/conformances.md#core-equality-acquisition)
- Saw: On clean `eca5c0b9d21` on macOS arm64, an authored root `trait Equatable<T>` with `equals(left: T, right: T) -> bool` gives an empty named implementation a compiler-generated row. The copied program with only that trait consistently renamed to `UserComparable` refuses the missing `equals` row. The working spelling runs in both engines and reopens. The specification reserves synthesis for a sealed core declaration.
- Cost: A user root path grants synthesis privilege through spelling. Publish and select the existing equality protocol through an exact sealed owner. Migrate the legacy authored witnesses together with that change so a lookalike user declaration cannot acquire synthesis.
- Reported: 2026-11-15, Codex, while auditing remaining equality-name selection.

### Named mathematical terms reject total wrapping operators before formation
- Category: branch per shape
- Where: [policy-specific binary expression lowering](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs#L1155), [named mathematical definitions](wiki/spec/proofs/mathematical_bindings.md#named-mathematical-definitions), [total arithmetic in proof terms](wiki/spec/proofs/contracts.md#total-arithmetic)
- Saw: With compiler source `16c9a2cd0431` on macOS arm64, a Linux x86-64 check refuses an unused `let reflexive(value: u32): core::Strict<0> = value +% 1u32 == 1u32;` in the [existing predicate fixture](tests/fixtures/fail/proofs/unused_partial_mathematical_predicate). Typing says suffixed operators are supported only in executable machines and their contracts. Replacing only that declaration with a machine precondition over the same `value: u32` and the same `+%` comparison compiles 237 files. The named declaration never reaches canonical term formation.
- Cost: Totality depends on where the term is written before the shared checker can look at its operands. Admit the same wrapping expression under its exact mathematical input and retain the shared definedness obligation for a division. A trapping term must still refuse.
- Reported: 2026-11-15, Codex, while comparing overflow controls for the unused mathematical predicate.

### A `fail/` proof fixture compiles, so `lowered_route_governance` fails at its base
- Category: a verdict with nothing behind it
- Where: `tests/fixtures/fail/proofs/unused_partial_mathematical_predicate/main.omg`, `omega-rust/omega/compilation/compiler/tests/lowered_route_governance.rs:44`
- Saw: `omega --check --target linux_x86_64` on the fixture prints `compiled 237 source file(s)` at dc83fcb65264, although its header expects `carrier arithmetic in a clause term`; `an_invalid_bound_entry_reports_the_routes_rejection` panics with "the compilation must be rejected after Psi 04".
- Cost: the governance test is red for every agent that runs the lowered-form checker's area tests, and an unused partial predicate (`value + 1u32 == 1u32`) is accepted without the rejection the fixture pins.
- Reported: 2026-11-15, Claude Code, while testing refusal locations (`work/compiler-refusal-location`)

### Selection recomputes the plan for a call's indirect result
- Category: re-deriving
- Where: [indirect result check](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_inputs.rs#L1492) (`aggregate_result_input::indirect_result`)
- Saw: Selection decides whether a call returns through a caller-supplied result pointer by running `calling_conventions::evaluate_call_plan` on a result-only signature and comparing its result placement with the stored one. Borrowed and owned call arguments now take their placements from the stored plan, and the independent audit compares that plan with the callee's own plan. This check still recomputes the plan.
- Cost: Every call whose result comes back through a result pointer runs the calling convention a second time in default compilation. Read the register from the stored placement's shape and location, and leave the plan comparison to the audit. A live claim (call_contract_repair domain enforcement) lists this file.
- Reported: 2026-11-15, Claude Code, while removing the recomputed plan from borrowed and owned call arguments

### Literal declaration validation still judges selected predicates in syntax
- Category: one idea, several implementations
- Where: [production constant validation](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs#L569), [literal predicate discharge](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs#L3570), [operator applicability](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs#L197), [checked destination handoff](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs#L3704)
- Saw: At `c30b00a0b878`, complete resolution still sends literal record/case and nested constrained-member predicates through `ProveSyntax`. That path interprets named/indexed domain facts and approximates operator applicability from root spellings and parameter carriers. Computed initializers already defer to checked evaluation; top-level qualified destinations require the exact checked judgment or refuse. This is source inspection, with no divergent acceptance reproduced; the older `const_integer` classifier has no production callers.
- Cost: Declaration-dependent meaning has another implementation before ordinary checking. Retain selected declarations, scope, substitutions and exact destination/initializer through the existing forward evaluation handoff; keep literal decoding and anonymous primitive normalization. Under **EARLY-TERMINAL-JOBS**, check matching declaration/static-argument/construction outcomes and independence from unrelated incompatible operators, preserving false unused-declaration, overflow, zero-divisor, carrier, arity and uncertain-predicate refusals. Removing this evaluator must not remove those obligations.
- Reported: 2026-11-15, Codex, while verifying the owner's upstream source-review handoff.

### Constant-index finalization searches argument storage to recover its owner
- Category: re-deriving
- Where: [receiving parameter search](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs#L236), [finalization caller](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/constant.rs#L5029)
- Saw: At `c30b00a0b878`, production `finalize_const_argument_selections` calls `carrier::receiving_parameters` with an argument span. It scans child types, constants, parameters, returns, fields, payloads, locals, lexical inputs, generated instances and domain constraints for that span, then recovers the receiving declaration's parameters. Conflicting owners return `None`. This is source inspection; no incorrect finalization or timing was measured.
- Cost: Each application depends on a roster of possible containing shapes and searches unrelated declarations. Pass its selected receiving application and binder correspondence through the existing finalization path. Preserve ownership through copying, substitution and shared argument storage; reject substituted owners. A focused work-count control should show that adding unrelated declarations does not increase owner recovery for a fixed application.
- Reported: 2026-11-15, Codex, while verifying the owner's upstream source-review handoff.

### A narrow installation bound rejects a valid selected child
- Category: knowledge crossing a boundary
- Where: [staggered installation source](tests/fixtures/pass/providers/staggered_installation_reach_exit/main.omg), [call reach check](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/ceilings.rs), [installation rule](wiki/spec/build/external_roots.md#installation-bound-reach)
- Saw: The original fixture retains B's X and C/D's Y in A's selected row. Narrow A to `reaches <= X` and move its C call to `main` after the call to A, leaving the same two writes and target selections. On macOS arm64, `AProvider::visit` at line 13, column 59 still refuses. Source checking charges B's whole X + Y upper bound even though the exact selected B only reaches X.
- Cost: A caller cannot state the tighter valid bound on a deployment that supplies a narrower child. Carry that containment question to the existing installation closure and preserve the refusal when the selected child actually reaches Y or has no supplier.
- Reported: 2026-11-15, Codex, while restoring the staggered provider review and manifest.


### Package review cannot recover the nominal callback's Count binder
- Category: knowledge crossing a boundary
- Where: [kept nominal callback package](tests/fixtures/pass/facts/static_callback_reach_boundary_schema/main.omg#L62), [policy recovery assertion](omega-rust/omega/compilation/compiler/tests/package_review.rs#L827), [machine supply rules](wiki/spec/language/machines.md#supply)
- Saw: On clean `cfabcb98f21`, the unchanged fixture stops at `TraversalCode::traverse<quiet, unused>(37)` on line 62, column 37. A prototype admitting the exact nominal operands reaches the complete-interface refusal for `Step<2>` and the generic named member without a method application. Direct `quiet<3>(37)` and `boundary_step<3>(37)` hit the same copier refusal without a callback binder. The latter declares a body-bearing free `boundary machine`; the supply rules reserve that free form for bodyless admission.
- Cost: The policy test never recovers `StepContract::step` with its authored `Count` binder and contract. Carry exact method application and nominal refinement through their existing owners and settle the boundary declaration migration. Keep the checked `Step<2>` and `Step<3>` calls, unused effectful selection, and the recovered schema oracle.
- Reported: 2026-11-15, Codex, macOS arm64, while testing static callable receipts.

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs), [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs), [source application preparation](omega-rust/psi/compilation/representations/typed-trees/src/references.rs)
- Saw: Type, expression and statement tables share retained pages and append private rows. Empty selected activation still copies declaration and side-record storage before projection narrows its roots. Later declarations can also give a retained call its source application; that necessary write still detaches the earlier completion batch's whole page.
- Cost: Unrelated declaration storage is copied even for an empty body batch. A real update to one retained call can copy unrelated bodies from its earlier batch. Use the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract) to share the remaining immutable declaration storage and keep body replacements under their actual owner, not the earlier batch. Do not skip source application preparation, projection, source authority checks, or signature and inferred-summary dependencies.
- Measured: On Windows x86-64 at opt-level 1, the two original Build routes each selected 75 files and zero bodies. Activation requested 5,072,931 bytes across 13,043 allocations and cloned 63 arenas. Later full completions of 1,695 / 1,693 machines requested 322,765,350 / 322,664,660 bytes, including new output and required preparation. They still copied earlier pages containing 21,773 / 21,757 expression rows and 1,955 statement rows. These are operation counts, not a runtime-speedup claim. The [historical macOS measurement](wiki/drafts/measurements/selected_body_copies.md) at `d66164cd0f5` recorded 32,333,303 activation bytes, 14,553 allocations and 101 cloned arenas; it did not measure repeated nonempty completion.
- Reported: 2026-11-15, Codex, after measuring retained type and body storage on the original scalar-contract and borrowed-root Build routes

