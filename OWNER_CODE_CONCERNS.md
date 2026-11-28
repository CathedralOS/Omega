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

### Locked package review reconstructs a different source set for a supplied Build library
- Category: 4. Re-deriving
- Where: [Build dependency sources](source-rust/subsystems/compiler/build/planning/source.rs#L628), [candidate package review](source-rust/subsystems/package-manager/review/candidate/passes_occurrences.rs#L356) and [library handoff validation](source-rust/subsystems/compiler/build/inputs/source/source_imports/supplied_library.rs#L140).
- Saw: The successor Build for the binary search visualization received the system library with a digest covering its imported sources. Locked package review supplied a handoff covering all reviewed package sources. With frozen inputs, only the source-consumption digest differed. Opening every module experimentally got past this check, then the version 4 lock refused multiple Build entries for the same package.
- Cost: A caller that requests a lock cannot complete this staged package review. The reviewer needs to retain which sources the original Build consumed while it checks the complete package.
- Reported: 2026-11-28, Codex, while running the binary search visualization on macOS ARM64.

### Selected optimization still obtains its input from the preceding implementation crate
- Category: 2. Knowledge crossing a boundary
- Where: [stage 06 entry](source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/src/selected_rewrites/selected_optimization.rs#L232), [stage 05 result](source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/selected.rs#L15), and [stage 06 dependency](source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/Cargo.toml#L12)
- Saw: `optimize_selected_instructions` receives stage 05's `StagedOptimizedSelectedInstructions`. Stage 06 links the instruction-selection implementation to extract the selected plan, body facts, environment, optimization settings and receipt. Those records are the handoff, but their admitted wrapper and unpacking API belong to the preceding transformation crate.
- Cost: the representation directory still does not define this earlier boundary. Publish the admitted stage 05 result in the selected-instructions representation and keep its issuing authority with instruction selection. Stage 06 should consume that product and remain the owner of liveness and range preparation; do not make its completed `SelectedBody` accept missing analyses just to reuse the type at an earlier stage.
- Reported: 2026-11-27, Codex, while following the same ownership defect upstream after separating the stage 06-to-07 and 07-to-08 products; inspected on `b2e3b070ad832`, no performance measurement.

<a id="resuming-a-typing-from-retained-source-costs-more-than-typing-afresh"></a>
### Completing demanded bodies repeats whole-program name resolution
- Category: 4. Re-deriving
- Where: [body demand](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type.rs), [resolution finalization](source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolver.rs), and [source compatibility](source-rust/subsystems/compiler/source-to-psi/driver/resolve_and_type/activation.rs)
- Saw: On Windows at `3f94bb48575f`, each of six checks of `samples/cli/arithmetic/popcount/main.omg` finalized resolution 35 times. Three ordinary checks spent a median 390 ms in those passes (instrumented development compiler, six workers). Three checks that shared source storage and retained the dependency's typed declarations also finalized resolution 35 times. Each declined the retained Product because new files in the same package could change name lookup. None resumed it.
- Cost: Each demand round can revisit contracts and selections outside the bodies it completed. A contract can refer to a different machine's state, which only becomes visible when that body completes. Any narrower update must still resolve those references. The earlier comparison of resumed and fresh typing used a different source graph and does not establish a current reuse penalty.
- Reported: 2026-11-25, Claude Code, while profiling source reuse. Remeasured 2026-11-27, Codex, on the package selected by popcount's current Build.

<a id="every-machine-check-replays-the-summaries-of-all-its-callees-again"></a>
### Inference replays callee summaries while machine contracts are changing
- Category: 4. Re-deriving
- Where: [input inference](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/calls/entry_inputs.rs), [result inference](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/calls/result_summaries.rs) and [callee summary replay](source-rust/subsystems/compiler/psi/semantics/src/descriptors/replay/engine.rs).
- Saw: a caller being checked during inference can rebuild a summary another caller computed. Input inference publishes new entry requirements between callers. Result inference checks batches against the same contracts until it publishes a new guarantee. A summary can be reused only while the machine and all the callees it read still have the same relevant contracts. Checks of the settled module already share summaries.
- Cost: three instrumented Windows checks of `samples/cli/arithmetic/popcount/main.omg` at `3f94bb48575f` rebuilt 453 summaries in input inference and 54 in result inference per check. Crash inference rebuilt none. Outermost uncached replays in these inference walks took a median 14.6 ms summed over workers. A speedup for the complete check has not been demonstrated.
- Reported: 2026-11-25, Claude Code, while profiling source checking. Remeasured 2026-11-27, Codex, with the optimized development profile and six workers.

<a id="stage-03-rebuilds-the-whole-program-operand-owner-table-in-each-pass"></a>
### Stage 03 rebuilds the whole-program operand-owner table in each pass
- Category: 4. Re-deriving
- Where: [cohort typing](source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs) and the [operand-owner walk](source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/expressions/references.rs).
- Saw: each pass that asks which declarations hold an operand needs an answer for the program it reads. Preparing a constructor or cast can add an owner to an existing expression or type. Reusing the constructor pass's table in all later passes returned no owner for 12,784 expression queries per pass on popcount where a fresh walk found one. Such a miss drops the refusal owed by the holding declaration.
- Cost: at `3f94bb48575f`, three optimized dev-profile Windows checks of [popcount](samples/cli/arithmetic/popcount/main.omg) each built and queried 24 whole-program owner tables across four fresh cohorts. The three table-build totals were 62 ms, 73 ms and 7.4 s. The last check spent 186 s overall on the shared host. A continuation visits its selected declarations. To reuse a table across a write, the compiler would have to track which declarations could reach the changed operands.
- Reported: 2026-11-25, Claude Code, while profiling stages 02 to 04 on popcount.
