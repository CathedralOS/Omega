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

### POSIX directory counting assumes that two entries are dot names

- Category: One idea, several implementations
- Where: [POSIX directory counting and indexed lookup](source/library/std/posix_filesystem_adapter.omg) and [Windows directory emptiness](source/library/std/targets/windows_x86_64/filesystem/directory.omg).
- Saw: POSIX counts native records in a wrapping `i32`, then subtracts two. Its indexed lookup checks the actual names. Windows counts ordinary names and stops on the first ordinary name when checking emptiness.
- Cost: POSIX can miscount directories whose records omit a dot entry. Its emptiness query also requests metadata for a child whose name already proves the directory is nonempty. Exercise a stream with missing or interleaved dot entries, a checked count and a failure after the first child.
- Reported: 2026-11-24 while comparing the selected directory providers.

### A resumed Build refuses itself whenever it is typed again, because region owners print as symbol slots

- Category: Knowledge crossing a boundary
- Where: [`encode_signature_region`](source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/type_records/type_identity.rs#L2420), compared by [`types_match`](source-rust/subsystems/compiler/psi/semantics/src/continuation/declarations.rs#L208)
- Saw: With ordinary (not package-qualified) identity, a declaration-owned region prints as the owner's symbol arena slot, for example `ref(slice(named(name(u8))),region(owner(177:1),ordinal(0)))` for `Build<'storage>`'s `identifier: @'storage [u8]`. When a Build waits for a dependency and its source is typed again instead of reused, the dependency's symbols shift that slot to `178:1`, and the continuation check refuses with "successor changed a reached machine's entry contract". Reuse after preparation avoids this today, but any other reuse miss after a wait (a changed import, a new lookup contributor) still refuses an unchanged Build.
- Cost: The refusal is safe, but it reports a changed contract that did not change, and it ties staged Build execution to source reuse. The owner's portable identity (`portable_lifetime_parameter_identity`, which already names data, machine and trait owners) would make the comparison independent of symbol numbering.
- Repair scope: Retire cross-program Build resumption under [fixed Build imports](wiki/spec/build/scoped_execution.md#checked-build-stages). Check other consumers of portable region identities independently before removing shared identity checks.
- Reported: 2026-11-22, Claude Code, while repairing `build_staged_dependency_calls`

<a id="a-build-wide-flag-admits-machine-control-for-every-output"></a>
