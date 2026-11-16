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


### Literal declaration validation still judges selected predicates in syntax
- Category: one idea, several implementations
- Where: [literal predicate discharge](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_const_evaluation.rs#L3570), [checked destination handoff](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_generic_data.rs#L3704)
- Saw: Record/case and nested constrained-member predicates still reach `ProveSyntax`, which approximates selected operators from spellings and parameter carriers. Computed initializers use checked evaluation. The duplicate judgment was identified by inspection.
- Cost: Literal and computed initializers use separate judgments for the same declaration-dependent obligations. The [EARLY-TERMINAL-JOBS handoff](TASKS.md#early-terminal-jobs) owns the repair and its acceptance checks.
- Reported: 2026-11-15, Codex, while verifying the owner's upstream source-review handoff.

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L554), [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs)
- Saw: Empty activation still copies declaration and side-record storage before projection. On the scalar-contract and borrowed-root Build routes, each zero-body activation cloned 63 arenas and requested 5,072,323 bytes on Windows x86-64 at opt-level 1. The [measurement in #3634](https://github.com/CathedralOS/Omega/pull/3634) also covers the landed primary-body storage repair. Child-list rewrites still use completion-batch boundaries.
- Cost: Share the remaining immutable storage under its actual owner using the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract). Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Remeasure empty activation and nonempty completion on both routes.
- Reported: 2026-11-15, Codex, while measuring retained type and body storage.

