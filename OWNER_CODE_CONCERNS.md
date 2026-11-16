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
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L555), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs)
- Saw: Replacing symbols on a shared typed predecessor can still clone side records, normalized identities and owned containers when no bodies are selected. The [data-child sharing change](https://github.com/CathedralOS/Omega/pull/3720) records a historical baseline on both Windows x86-64 Build routes.
- Cost: Empty activation pays to copy storage unrelated to the selection. Share the remaining immutable storage under its existing owners and keep writes local to the changed records or completion batches. Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Instruction selection's inputs still carry earlier-stage pieces
- Category: knowledge crossing a boundary
- Where: [stage 03 manifest](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/Cargo.toml), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs#L309), [boundary rules](tools/xtask/src/boundary.rs#L396)
- Saw: Stage 03 now takes installation authority as the plain [`BoundInstallations`](omega-rust/omega/compilation/backend/runtime/bound-installations/src/lib.rs) record and sits under both transitive rules in `tools/xtask/src/boundary.rs`. The driver still attaches [handler stacks](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L102) and [entry execution](omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L315) to stage 02's output after preparation, because image emission reads them back from that output (`installed_entry_execution`, `installed_stack_requirements` in `image-emission`). Plan and selection representations use canonical encoders from the [module codec](omega-rust/psi/compilation/representations/terminal-psi/codec/src/lib.rs), which also exposes the sealed Terminal Psi module.
- Cost: Stage 02's output is still mutated after preparation. Once image emission takes the installed entry and handler stacks from the bound record instead of stage 02's output, delete `with_installed_entry_execution` and `with_installed_stack_requirements` and pass the record beside the plan to both consumers. Separate canonical encoding from module access.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Later machine forms embed records from the prepared plan
- Category: knowledge crossing a boundary
- Where: [selected instructions](omega-rust/omega/compilation/representations/selected-instructions/Cargo.toml#L25), [register homes](omega-rust/omega/compilation/representations/register-homes/Cargo.toml#L13)
- Saw: `selected-instructions` and `register-homes` embed call records, structural contracts and call-site owners defined in `legalized-operations`, so both depend on the prepared plan's crate.
- Cost: A change to a legalized-operations record changes the selected and allocated forms without passing through stage 03. Move the embedded records to the form that owns them.
- Reported: 2026-11-16, Claude Code, while removing unused dependencies from stages 04 to 09.

### Image emission still reads the target operations stage 02 keeps as a working form
- Category: knowledge crossing a boundary
- Where: [object evidence](omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs)
- Saw: Stage 02 now publishes one execution plan, and the entry, profile, entry calling plan and surviving installations are read from it. `function_fragments.rs` still reads `target_operations()` for evidence checks alongside the abstract program (reference backing geometry, activation residence, access-reference checks), and borrowed-storage layouts still come from `TargetOperationPlan` because `NativeBorrowedStorageLayout` has no identity encoding to seal into the plan.
- Cost: Stage 02 has to keep exposing two complete graphs. Give borrowed-storage layouts a canonical encoding and move them into the plan, port the evidence checks to plan rows, then stop exposing `target_operations()` outside the stage and its validator.
- Reported: 2026-11-16, Claude Code, while moving the readers of stage 02's output onto the execution plan.
