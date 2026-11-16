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

### Native compilation stops halfway for Build's constant evaluator
- Category: knowledge crossing a boundary
- Where: [native completion](omega-rust/omega/build-system/orchestration/src/compile/native_completion.rs#L123), [scalar completion](omega-rust/omega/compilation/native-compiler/src/scalar_completion.rs), [Build's completer](omega-rust/omega/build-system/build-settings/src/activation/scalar_completion.rs#L20)
- Saw: `native_compiler::prepare` returns a half-built compile. Each caller then runs `build_settings::scalar_completion::complete`, the Psi constant evaluator, on every `NativeScalarCompletion` and hands the results back before `native_compiler::compile` lowers anything. The demand comes from the native execution scope (`scalar_application_demand`), which reads only the Terminal module and target layouts.
- Cost: The Omega pipeline leaves for another pipeline in the middle and every caller repeats the prepare, evaluate, compile sequence. Complete the demand from the Terminal product and target inputs before the native driver starts, pass the completed scalars in `NativeCompileRequest`, and delete `NativeScalarCompletion` and the split entry.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages.

### Emission libraries keep earlier stage outputs beside the final layout
- Category: knowledge crossing a boundary
- Where: [fragment source](omega-rust/omega/compilation/backend/machine-emission/src/fragment_emission.rs#L463), [driver join](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L202), [runtime-only route](omega-rust/omega/compilation/native-compiler/src/realization.rs#L172), [stage-call exceptions](tools/xtask/src/boundary.rs)
- Saw: `StagedOptimizedFunctionFragmentEmissionSource::new` joins the stage 09 realization with stage 02's whole output, and frame staging between stages 06 and 07 runs in machine-emission on types defined inside stage crates. `admit_terminal_runtime` runs stage 01, then calls stage 02's `capture_runtime_borrowed_storage_layouts` on the side. `native-artifact` reads Omega 00's `VerifiedNativeArtifactInput` for runtime provisions.
- Cost: Emission can read anything an earlier stage knew, so a missing fact never has to be carried forward. Move the stage output types emission names into `compilation/representations`, carry the rows emission needs in each stage's output, and delete each `STAGE_CALL_ALLOWANCES` row as its edge goes.
- Reported: 2026-11-16, pi, while checking that only the native driver calls Omega stages.

### Borrowed field recasts lose the target carrier layout of the enclosing record
- Category: re-deriving
- Where: [reference view geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references/representation_geometry.rs#L147), [sum geometry](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L4450)
- Saw: A recast checks the enclosing record's alignment, but its geometry query has no carrier layout for a sibling sum. The checker supplies the retained target's record resolver without its case-tag layout. An exact `PosixFilesystem` reduction refuses a whole aligned 144-byte field. Changing only the sibling `ErrorKind` field to `u32` yields native and Psi exit 0 on macOS arm64.
- Cost: The standard-library POSIX stat decoders still refuse the whole buffer. Pass the existing target carrier layout to projected storage checks, preserve the short and misaligned refusals, and replay the unchanged [note vault](https://github.com/CathedralOS/Omega/pull/3722).
- Reported: 2026-11-15, Codex, while validating shared recasts of an aligned record field.

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L669), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [owned arena clone](omega-rust/psi/foundation/arena/src/arena.rs#L748)
- Saw: Replacing symbols on a shared typed tree can copy side records, normalized identities and owned containers even with no bodies selected. The [call and machine record repair](https://github.com/CathedralOS/Omega/pull/3747) measured empty activation and two nonempty batches on both Windows x86-64 routes. Empty activation requested 2,182,456 bytes in 12,456 allocations and cloned 36 arenas per route on fixed `570d707` with the repair and observer (opt-level 1). See the [data-child repair](https://github.com/CathedralOS/Omega/pull/3720) for the earlier baseline.
- Cost: Share immutable storage where measurements still show copies. Limit writes to changed records or new batches. Preserve source application preparation, projection, authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Image emission gets installation records from stage 02's output
- Category: knowledge crossing a boundary
- Where: [handler stack setup](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L102), [entry execution setup](omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L315), [selection input](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L112), [physical entry requirements](omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs#L4824)
- Saw: After preparation, the driver stores entry and stack records in stage 02's output and reads them back to build [`BoundInstallations`](omega-rust/omega/compilation/backend/runtime/bound-installations/src/lib.rs) for selection. Image emission also reaches back into stage 02 for stack requirements.
- Cost: Bind the records once after admission and pass them beside the plan to both consumers. Remove both setters and preserve the installation's program, target and entry checks.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Later machine forms embed records from the prepared plan
- Category: knowledge crossing a boundary
- Where: [selected dependencies](omega-rust/omega/compilation/representations/selected-instructions/Cargo.toml#L25), [register-home dependencies](omega-rust/omega/compilation/representations/register-homes/Cargo.toml#L13), [selected call records](omega-rust/omega/compilation/representations/selected-instructions/src/calls.rs#L1397), [register-home recovery](omega-rust/omega/compilation/representations/register-homes/src/recovery/structural_calls.rs#L31), [shared vocabulary owners](omega-rust/omega/compilation/representations/README.md#program-families-and-pipeline-boundaries)
- Saw: `selected-instructions` and `register-homes` use call records, structural contracts, call-site owners and encoders from `legalized-operations`. That crate owns the prepared plan.
- Cost: Changes to the prepared plan can alter later forms and codecs. Give unchanged records one vocabulary owner and keep form-specific records with their forms. Remove both plan dependencies and check call and contract identities through selection, allocation and recovery.
- Reported: 2026-11-16, Claude Code, while removing unused dependencies from stages 04 to 09.

### Image publication still reads the prepared plan after selection
- Category: knowledge crossing a boundary
- Where: [object evidence](omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs)
- Saw: Object production reads function ABI and byte-output provider rows through `optimized_target().execution_plan().plan()`. It still reaches the complete earlier plan after instruction selection, rather than the contract rows of its current input.
- Cost: Later publication still depends on earlier program custody. Share the required immutable ABI and provider rows through selected and current native contracts, and read those chunks when publishing the object.
- Reported: 2026-11-16, Claude Code, while moving the readers of stage 02's output onto the execution plan.
