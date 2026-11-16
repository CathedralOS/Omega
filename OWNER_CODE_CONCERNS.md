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

### Unused place-access summaries discard referent effects
- Category: zombie machinery
- Where: [call propagation](omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/semantic_analyses.rs#L1289), [analysis registration](omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/analyses.rs#L87)
- Saw: On `e17077ce417`, known-call propagation reduces each parameter read/write path to the actual argument prefix, dropping `Referent`. An owned local carrier can therefore classify a callee's write through its contained reference as internal and omit it. Repository reference inspection found no production rule requesting or consuming this analysis; its incomplete flag has no readers. The active descriptor solver used by cleanup does retain those effects.
- Cost: The private catalog advertises incomplete access summaries that cannot support preservation claims. Retire the unused analysis or derive its results from the existing owner-aware descriptor effect solver; appending recursive call paths would introduce an unbounded path fixed point.
- Reported: 2026-11-15, Codex, while reviewing ordinary owned cleanup call composition.

### Literal byte-view validation rescans each function for its producer
- Category: re-deriving
- Where: [literal producer lookup](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/native-preparation/src/legalization_scalar_graph.rs#L5308), [function inputs](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/native-preparation/src/legalization_function_inputs.rs#L14)
- Saw: Each lookup scans the structural declarations and every operation in the function to find one exact literal producer. Call reconstruction, provenance checks and home retirement can repeat the lookup for the same literal, even though the validator already prepares indexed input rows for the function.
- Cost: A function with a growing number of literal views can do quadratic lookup work during independent validation. Index producers with the existing function inputs and preserve rejection of mismatched declarations, duplicate producers and nonliteral homes. Measure lookup work with a growing literal roster.
- Reported: 2026-11-15, Codex, while validating a shared byte view across native edges.

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L555), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs)
- Saw: Replacing symbols on a shared typed predecessor can still clone side records, normalized identities and owned containers when no bodies are selected. The [data-child sharing change](https://github.com/CathedralOS/Omega/pull/3720) records a historical baseline on both Windows x86-64 Build routes.
- Cost: Empty activation pays to copy storage unrelated to the selection. Share the remaining immutable storage under its existing owners and keep writes local to the changed records or completion batches. Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Instruction selection can reach earlier program forms
- Category: knowledge crossing a boundary
- Where: [stage 03 manifest](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/Cargo.toml), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs#L309), [boundary rules](tools/xtask/src/boundary.rs#L396)
- Saw: The stage 03 crate names its target records from [target-vocabulary](omega-rust/omega/compilation/representations/target-operations/target-vocabulary/src/lib.rs) and has no direct dependency on the stage 02 working graph, but its [provider-admission dependency](omega-rust/omega/compilation/realization/provider-admission/Cargo.toml) still reaches both `abstract-operations` and `target-operations`. The driver attaches [handler stacks](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L102) and [entry execution](omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L315) to stage 02's output after preparation. Plan and selection representations use canonical encoders from the [module codec](omega-rust/psi/compilation/representations/terminal-psi/codec/src/lib.rs), which also exposes the sealed Terminal Psi module.
- Cost: The transitive check guards the representations selection uses. Finish the stage 03 boundary by passing bound installations alongside the plan using authority records below provider admission, and separating canonical encoding from module access. Keep the surviving-installation roster tied to the prepared operation set and preserve rejection of mismatched installation authority. Then put the stage 03 production crate under both transitive rules in `tools/xtask/src/boundary.rs` (`EARLIER_PROGRAM_FORMS` and `STAGE_02_WORKING_FORM`); today it has direct rules only.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Later machine forms embed records from the prepared plan
- Category: knowledge crossing a boundary
- Where: [selected instructions](omega-rust/omega/compilation/representations/selected-instructions/Cargo.toml#L25), [register homes](omega-rust/omega/compilation/representations/register-homes/Cargo.toml#L13), [frame step](omega-rust/omega/compilation/backend/machine-emission/src/function_realization.rs#L345)
- Saw: `selected-instructions` and `register-homes` embed call records, structural contracts and call-site owners defined in `legalized-operations`, so both depend on the prepared plan's crate. Stage 06's output omits the allocation it was computed from, so `stage_function_frame` takes the allocation and stage 06's output separately.
- Cost: A change to a legalized-operations record changes the selected and allocated forms without passing through stage 03. Move the embedded records to the form that owns them, and let stage 06's output keep its allocation so the frame step reads one form.
- Reported: 2026-11-16, Claude Code, while removing unused dependencies from stages 04 to 09.

### Image emission still reads the target operations stage 02 keeps as a working form
- Category: knowledge crossing a boundary
- Where: [object evidence](omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs)
- Saw: Stage 02 now publishes one execution plan, and the entry, profile, entry calling plan and surviving installations are read from it. `function_fragments.rs` still reads `target_operations()` for evidence checks alongside the abstract program (reference backing geometry, activation residence, access-reference checks), and borrowed-storage layouts still come from `TargetOperationPlan` because `NativeBorrowedStorageLayout` has no identity encoding to seal into the plan.
- Cost: Stage 02 has to keep exposing two complete graphs. Give borrowed-storage layouts a canonical encoding and move them into the plan, port the evidence checks to plan rows, then stop exposing `target_operations()` outside the stage and its validator.
- Reported: 2026-11-16, Claude Code, while moving the readers of stage 02's output onto the execution plan.
