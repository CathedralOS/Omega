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

### A user trait named Equatable receives compiler synthesis
- Category: the documents disagree
- Where: [synthesis selection](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation.rs#L2601), [core equality acquisition](wiki/spec/language/conformances.md#core-equality-acquisition)
- Saw: On clean `eca5c0b9d21` on macOS arm64, an authored root `trait Equatable<T>` with `equals(left: T, right: T) -> bool` gives an empty named implementation a compiler-generated row. The copied program with only that trait consistently renamed to `UserComparable` refuses the missing `equals` row. The working spelling runs in both engines and reopens. The specification reserves synthesis for a sealed core declaration.
- Cost: A user root path grants synthesis privilege through spelling. Publish and select the existing equality protocol through an exact sealed owner. Migrate the legacy authored witnesses together with that change so a lookalike user declaration cannot acquire synthesis.
- Reported: 2026-11-15, Codex, while auditing remaining equality-name selection.

### Named predicates skip declaration-time arithmetic totality
- Category: a verdict with nothing behind it
- Where: [unused partial predicate](tests/fixtures/fail/proofs/unused_partial_mathematical_predicate/main.omg), [governance assertion](omega-rust/omega/compilation/compiler/tests/lowered_route_governance.rs#L185), [operator lowering](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs#L1155), [named mathematical definitions](wiki/spec/proofs/mathematical_bindings.md#named-mathematical-definitions), [proof arithmetic rules](wiki/spec/proofs/contracts.md#total-arithmetic)
- Saw: At `dc83fcb65264`, `omega --check --target linux_x86_64` accepts the unused predicate `value + 1u32 == 1u32` over arbitrary `u32`, so the governance test panics. At `16c9a2cd0431` on macOS arm64 targeting Linux x86-64, changing only the unused predicate's addition to `+%` is refused during typing. A machine precondition with the same wrapping expression and input type compiles.
- Cost: Check a named predicate under its own formal inputs through the same term formation as an inline contract. Pin the overflow refusal even when the predicate is unused, admit the wrapping control, and preserve the nonzero-divisor obligation. Replace the governance assertion's old `carrier arithmetic in a clause term` expectation with the actual totality refusal.
- Reported: 2026-11-15, Claude Code and Codex, while testing refusal locations and comparing overflow controls.

### Literal declaration validation still judges selected predicates in syntax
- Category: one idea, several implementations
- Where: [literal predicate discharge](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs#L3570), [checked destination handoff](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs#L3704)
- Saw: Record/case and nested constrained-member predicates still reach `ProveSyntax`, which approximates selected operators from spellings and parameter carriers. Computed initializers use checked evaluation. The indexed receiving case gained exact declaration identity in #3616. The record/case const-argument path still refuses before checked evaluation. The duplicate syntax judgment was identified by inspection.
- Cost: The syntax path duplicates part of the ordinary predicate judgment. [EARLY-TERMINAL-JOBS](TASKS.md#early-terminal-jobs) tracks the remaining handoff and its acceptance checks.
- Reported: 2026-11-15, Codex, while verifying the owner's upstream source-review handoff.

### A narrow installation bound rejects a valid selected child
- Category: knowledge crossing a boundary
- Where: [staggered installation source](tests/fixtures/pass/providers/staggered_installation_reach_exit/main.omg), [call reach check](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/ceilings.rs), [installation rule](wiki/spec/build/external_roots.md#installation-bound-reach)
- Saw: The original fixture retains B's X and C/D's Y in A's selected row. Narrow A to `reaches <= X` and move its C call to `main` after the call to A, leaving the same two writes and target selections. On macOS arm64, `AProvider::visit` at line 13, column 59 still refuses. Source checking charges B's whole X + Y upper bound even though the exact selected B only reaches X.
- Cost: A caller cannot state the tighter valid bound on a deployment that supplies a narrower child. Carry that containment question to the existing installation closure and preserve the refusal when the selected child actually reaches Y or has no supplier.
- Reported: 2026-11-15, Codex, while restoring the staggered provider review and manifest.


### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs), [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs), [source application preparation](omega-rust/psi/compilation/representations/typed-trees/src/references.rs)
- Saw: Type, expression and statement tables share retained pages and append private rows. Primary expression and statement rewrites detach their declaration or body owner's rows. Continuation construction installs matching symbols without copying the discarded old symbol table, but still copies declaration and side-record storage before projection narrows its roots. Preparing the matching symbol snapshot also copies symbol storage. Child-list rewrites retain their existing batch boundary.
- Cost: Unrelated declaration storage is copied even for an empty body batch. Use the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract) to share the remaining immutable declaration and side-record storage under its actual owner. Do not skip source application preparation, projection, source authority checks, or signature and inferred-summary dependencies.
- Measured: On Windows x86-64 at opt-level 1, both original Build routes selected 75 files and zero bodies. Construction requested 4,136,141 bytes across 12,802 allocations and cloned 58 arenas; preparing matching symbols separately requested 958,615 bytes across eight allocations and five arena clones. The combined measured work requested 5,094,756 bytes, so the declaration and symbol copies remain substantial. Later full completions of 1,695 / 1,693 machines requested 311,068,557 / 310,879,492 bytes, including new output and required preparation. Primary payload copies remained 205 expression rows and two statement rows per route; directory detachment copied 742,388 / 742,276 bytes. These are operation counts, not a general runtime-speedup claim. The [historical macOS measurement](wiki/drafts/measurements/selected_body_copies.md) at `d66164cd0f5` recorded 32,333,303 activation bytes, 14,553 allocations and 101 cloned arenas; it did not measure repeated nonempty completion.
- Reported: 2026-11-15, Codex, after measuring continuation construction and body completion on the original scalar-contract and borrowed-root Build routes

