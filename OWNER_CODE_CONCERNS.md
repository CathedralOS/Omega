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

### Selected-body activation copies retained typed storage
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L553), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1283), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs)
- Saw: With no selected bodies, continuation construction still clones declaration and side-record storage before projection. On both Build routes it requested 4,136,141 bytes across 12,802 allocations and 58 arena clones. Preparing matching symbols separately requested 958,615 bytes across eight allocations and five clones. These are allocation counts and requested bytes from Windows x86-64 at opt-level 1. The [later measurement](https://github.com/CathedralOS/Omega/pull/3646) also records nonempty completion, whose totals include new output and preparation. See the [two workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary).
- Cost: Empty activation pays to copy unrelated storage. Share the remaining immutable storage under its owner and keep child-list updates at completion-batch boundaries. Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Measure copy work for empty activation and nonempty completion on both routes. The [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract) gives the ownership boundary.
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.


### Instruction selection reaches earlier forms through installation authority and the module codec
- Category: knowledge crossing a boundary
- Where: [provider-admission manifest](omega-rust/omega/compilation/realization/provider-admission/Cargo.toml), [authority types](omega-rust/omega/compilation/realization/provider-admission/src/model.rs), [codec](omega-rust/psi/compilation/representations/terminal-psi/codec/src/lib.rs), [boundary rules](tools/xtask/src/boundary.rs)
- Saw: `check-boundary` now proves that no representation selection reads or writes (`target-operations`, `legalized-operations`, `selected-instructions` and its analyses, `native-vocabulary`) can reach `abstract-operations` through any production dependency. Stage 03 itself still can, through `provider-admission`: the installation authority it is handed (`AdmittedInstalledInterruptCohort`, `AdmittedInstalledEntryExecution` and the scopes, entries and helpers inside them) is defined in the admission crate, which depends on `abstract-operations` throughout. Separately, the plan crates, `selected-instructions` and `native-vocabulary` depend on `terminal-codec` for canonical encoders (propositions, crash routes, access catalogs), and that crate also defines `ClosedTerminalModule`, so stage 03 reaches the sealed module's type one hop away. Structural declarations in the plan still state leaf scalars in the checking form, so the plan carries `RetainedIntegerTypes` to close them, with an `Unsealed` case for plans built without a module.
- Cost: Stage 03 is held to the boundary only by its direct-dependency rule. Move the installed-interrupt and installed-entry authority records to a crate below admission and have admission produce them; split the canonical vocabulary encoders from the sealed-module types in the codec, or have the plan carry the encoded bytes; close structural leaf scalars during preparation. Then put stage 03 under the transitive rule too.
- Reported: 2026-11-16, Claude Code, while making the selection representations unable to reach the abstract program.

### Instruction selection depends on stage 02's working form
- Category: knowledge crossing a boundary
- Where: [stage 03 manifest](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/Cargo.toml), [target-operations](omega-rust/omega/compilation/representations/target-operations/src/lib.rs)
- Saw: Stage 03 names about forty leaf types from `target-operations` (structural argument sources, call-site owners, installation call roles, boundary execution bindings, normalized-foreign-call mechanisms, case pointer slots), because the prepared plan embeds them. `target-operations` also holds `TargetOperationPlan`, stage 02's whole working graph, so stage 03 can name a second complete program form.
- Cost: A future change can make selection read target operations again without any check failing. Move the leaf types the plan embeds into `legalized-operations` or `native-vocabulary`, then forbid stage 03 the `target-operations` crate.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Stages 04 to 09 are not held to their inputs
- Category: knowledge crossing a boundary
- Where: [stage 04 manifest](omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/Cargo.toml), [stage 05 manifest](omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/Cargo.toml), [driver](omega-rust/omega/compilation/native-compiler/src/pipeline.rs)
- Saw: Stages 04 and 05 declare `isa-x86_64`, `isa-aarch64`, `terminal-psi`, `abstract-operations`, `target-operations`, `legalized-operations`, `terminal-codec` and `calling-conventions` and use them little or not at all outside tests. Stages 07 and 08 are each handed the selected program, the machine plan and the register model instead of one input form, and `assemble_fixed_frame_realization` takes the products of stages 02 through 09 at once. Only the adjacent-stage rule applies to these stages.
- Cost: Unused edges make the dependency graph misleading and make the next shortcut free. Remove the unused dependencies, give stages 07 and 08 one input each, and extend the transitive rule stage by stage.
- Reported: 2026-11-16, Claude Code, while auditing the native pipeline after the stage 02 to stage 03 repair.

### Bound installations are attached to stage 02's output
- Category: knowledge crossing a boundary
- Where: [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs), [driver](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs)
- Saw: The driver resolves handler stacks and the installed entry, then calls `with_installed_stack_requirements` and `with_installed_entry_execution` on `ValidatedOptimizedTargetOperations`, stage 02's output. Stage 03 checks the binding, but the authority travels inside stage 02's container instead of beside the plan, and the surviving-installations list is validated against the target operations rather than derived independently.
- Cost: The plan's container changes after the stage that produced it has finished. Make the bound installations a value the driver holds next to the plan and passes to selection.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Image emission still reads the target operations stage 02 keeps as a working form
- Category: knowledge crossing a boundary
- Where: [object evidence](omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs)
- Saw: Stage 02 now publishes one execution plan, and the entry, profile, entry calling plan and surviving installations are read from it. `function_fragments.rs` still reads `target_operations()` in six places beside the abstract program (reference backing geometry, activation residence, access-reference checks), and borrowed-storage layouts still come from `TargetOperationPlan` because `NativeBorrowedStorageLayout` has no identity encoding to seal into the plan.
- Cost: Stage 02 has to keep exposing two complete graphs. Give borrowed-storage layouts a canonical encoding and move them into the plan, port the evidence checks to plan rows, then stop exposing `target_operations()` outside the stage and its validator.
- Reported: 2026-11-16, Claude Code, while moving the readers of stage 02's output onto the execution plan.
