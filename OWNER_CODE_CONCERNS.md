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

<a id="stage-03-rebuilds-the-whole-program-operand-owner-table-in-each-pass"></a>
### Stage 03 rebuilds the whole-program operand-owner table in each pass
- Category: 4. Re-deriving
- Where: `collect_program_operand_owners` callers in `source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/` (`constructor_applications.rs:25`, `:254`, `:446`; `call_and_product_selections.rs:1788`, `:1964`; `equality_preparation.rs:160`; `expression_meanings.rs:115`)
- Saw: each pass of `close_typed_cohort` (`lowerer.rs`) visits the whole typed program six times for expression and type-reference owners; `omega --check` on popcount (259 std files) builds 48 tables, and every one is queried, so building lazily saves nothing. In each pass the tables built by the second `construction_destinations::prepare`, by `equality_preparation.rs:160` and by `constructor_applications.rs:254` came out identical on popcount, but the passes between them do write fields the walk reads (constructed value types, call targets, constructor application types), and `TypedTrees` keeps plain `Vec` fields with no revision, so nothing cheap proves that one table still answers exactly. Each walk now skips the name lookups whose only result is a discarded symbol and finds alias applications through an index; the rest of its cost is spread across about 100k node visits.
- Cost: about 0.8–1.2 s of owner-table builds per popcount `omega --check` (measured in-process against the former walk, which cost 1.2–1.6 s on the same states). The same table sequence is also built twice for some cohorts, once on each of two threads and once more in sequence, which comes from how Build schedules typing rather than from stage 03. A repair needs the passes to state which ownership they change, or a table that extends from an append frontier.
- Reported: 2026-11-25, Claude Code, while profiling stages 02–04 on popcount; walk cost updated 2026-11-25

<a id="a-build-wide-flag-admits-machine-control-for-every-output"></a>
