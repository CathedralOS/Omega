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

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L669), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [owned arena clone](omega-rust/psi/foundation/arena/src/arena.rs#L748)
- Saw: Replacing symbols on a shared typed predecessor can still clone other side records, normalized identities and owned containers when no bodies are selected. The [data-child sharing change](https://github.com/CathedralOS/Omega/pull/3720) records a historical baseline on both Windows x86-64 Build routes. A matched pair on fixed `570d707` plus the call/machine-record repair and numeric observer recorded 2,182,456 requested bytes, 12,456 allocations and 36 arena clones on both routes (Windows x86-64, opt-level 1); this is historical measurement, not current-main qualification.
- Cost: Empty activation pays to copy storage unrelated to the selection. Share the remaining immutable storage under its existing owners and keep writes local to the changed records or completion batches. Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Image emission gets installation records from stage 02's output
- Category: knowledge crossing a boundary
- Where: [handler stack setup](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L102), [entry execution setup](omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L315), [physical entry requirements](omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs#L4824)
- Saw: The driver attaches entry execution and installed stack requirements to stage 02's output after preparation. Image emission reads those records back from the stage output. Instruction selection already receives them through [`BoundInstallations`](omega-rust/omega/compilation/backend/runtime/bound-installations/src/lib.rs).
- Cost: The driver must modify the prepared stage output to carry installation records to image emission. Pass the bound record beside the plan to image emission and remove the two stage-output setters.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Later machine forms embed records from the prepared plan
- Category: knowledge crossing a boundary
- Where: [selected instructions](omega-rust/omega/compilation/representations/selected-instructions/Cargo.toml#L25), [register homes](omega-rust/omega/compilation/representations/register-homes/Cargo.toml#L13)
- Saw: `selected-instructions` and `register-homes` embed call records, structural contracts and call-site owners defined in `legalized-operations`, so both depend on the prepared plan's crate.
- Cost: A change to a legalized-operations record changes the selected and allocated forms without passing through stage 03. Move the embedded records to the form that owns them.
- Reported: 2026-11-16, Claude Code, while removing unused dependencies from stages 04 to 09.
