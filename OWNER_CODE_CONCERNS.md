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
- Merge overlapping reports into one entry for the defect that remains. If an
  active `TASKS.md` item already owns the repair and its acceptance checks,
  add the structural evidence there.
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

### Borrowed field guards lose their reaching value at a state call
- Category: re-deriving
- Where: [scalar observation](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L5360), [tick_marquee](samples/cli/rendering/tick_marquee/main.omg), [bitset](samples/cli/collections/bitset/main.omg)
- Saw: In the marquee, `draw` stores the position, tests `main.pos < 12` and passes the same mutable record to `put`. Giving `put` that exact entry contract reaches `the clause's observation has no reaching value identity`. A pure record program with the same store, guard and nested state call gives the same refusal. The bitset insertion reaches it after supplying its checked word-index guard as the callee contract.
- Cost: The caller's existing field guard cannot establish the same field observation at the receiving state. Preserve the exact projected contents and loan across the checked call. Keep sibling writes independent and refuse a bound after a write overlapping its field.
- Reported: 2026-11-15, Codex, while restoring the paced marquee and checking the bitset policy witness.


### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L553), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1283), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs)
- Saw: Empty activation still copies declaration and side-record storage before projection. On the [scalar-contract and borrowed-root Build routes](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), continuation construction requested 4,136,141 bytes across 12,802 allocations and 58 arena clones. Matching-symbol preparation added 958,615 bytes across eight allocations and five clones. Both measurements used Windows x86-64 at opt-level 1. The [measurement in #3646](https://github.com/CathedralOS/Omega/pull/3646) also covers nonempty completion. Child-list rewrites still use completion-batch boundaries.
- Cost: A zero-body selection pays to copy unrelated storage. Share the remaining immutable storage under its actual owner using the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract). Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Remeasure empty activation and nonempty completion on both routes.
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.


### The execution plan and selected form still depend on the abstract-program crates
- Category: knowledge crossing a boundary
- Where: [plan manifest](omega-rust/omega/compilation/representations/target-operations/legalized-operations/Cargo.toml), [selected-instructions manifest](omega-rust/omega/compilation/representations/selected-instructions/Cargo.toml), [retained types](omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/lib.rs), [selection boundary rule](tools/xtask/src/boundary.rs)
- Saw: Instruction selection no longer depends on `abstract-operations`, `optimization-unit` or `terminal-codec`, and `check-boundary` enforces that. The representations it reads and writes still do: `legalized-operations` uses `optimization_unit::{access_view_catalog_identity_bytes, scalar_refinement_identity_bytes}`, `OwnershipFrontierFact` and `abstract_operations::CompletionClaimSource`, and both it and `selected-instructions` list those crates. Structural declarations in the plan still state leaf scalars in the checking form, so the plan carries `RetainedIntegerTypes` to close them, with an `Unsealed` case for plans built without a module. Stage 03 also names about forty leaf types from `target-operations`.
- Cost: Selection can still reach the earlier program containers through the crates of its own input. Move the identity encoders and the remaining leaf records to `native-vocabulary`, close the leaf scalars of structural declarations during preparation, then drop the dependencies and extend the boundary rule to the two representation crates.
- Reported: 2026-11-16, Claude Code, while moving preparation of the execution plan from stage 03 to stage 02.

### Image emission still reads the target operations stage 02 keeps as a working form
- Category: knowledge crossing a boundary
- Where: [object evidence](omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs)
- Saw: Stage 02 now publishes one execution plan, and the entry, profile, entry calling plan and surviving installations are read from it. `function_fragments.rs` still reads `target_operations()` in six places beside the abstract program (reference backing geometry, activation residence, access-reference checks), and borrowed-storage layouts still come from `TargetOperationPlan` because `NativeBorrowedStorageLayout` has no identity encoding to seal into the plan.
- Cost: Stage 02 has to keep exposing two complete graphs. Give borrowed-storage layouts a canonical encoding and move them into the plan, port the evidence checks to plan rows, then stop exposing `target_operations()` outside the stage and its validator.
- Reported: 2026-11-16, Claude Code, while moving the readers of stage 02's output onto the execution plan.
