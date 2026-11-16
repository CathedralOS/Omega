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

### Focused compiler controls spend twenty-two minutes in code generation
- Category: Verifying costs more than writing
- Where: [compiler test targets](omega-rust/omega/compilation/compiler/Cargo.toml#L110), [compiler feature alignment](omega-rust/omega/compilation/compiler/Cargo.toml#L102), and the `coherent-native-controls-24-01` receipt in continuation claim `18e81ab82af520780000000018e96239`
- Saw: On macOS arm64 at `a827cf2c269`, a fixed-source nextest selection of 24 controls across 12 binaries spent 22m17s building before 176.905s of test execution. The command used the retained CLI feature union, optimization level 1, two codegen units and three jobs; 1,219 tests were not selected. Actual compiler processes remained active. The source of this build cost has not been established.
- Cost: The recorded twelve-binary replay spent twenty-two minutes building before exposing the next semantic refusal. A later three-binary replay at c16df641c12 built in 2m51s; that smaller selection does not identify the earlier cost. Measure the actual invalidation and compiled-target work, then repair the responsible build or test ownership while retaining the selected controls and exact-source optimized producer checks.
- Reported: 2026-11-14, Codex, while carrying stored-machine programs through Console and native-binding preparation

### Callback and interrupt records lose the machine that owns their operation
- Category: Re-deriving
- Where: [terminal-psi/src/lib.rs:352](omega-rust/psi/compilation/representations/terminal-psi/src/lib.rs#L352), [executable preparation](omega-rust/omega/compilation/compiler/src/executable.rs), and [codec/src/foundation_validation.rs:2261](omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs#L2261)
- Saw: each lowered machine allocates operation IDs from its own fresh `MachineSources`, but callback and interrupt records retain only `OperationId`. Executable preparation searches every reached machine for that number, while codec interrupt validation collects all machines into a map keyed by the same bare ID. Stage05 currently emits neither roster; the existing constructors are fixtures and decoding, and no failing customer execution was observed.
- Cost: equal operation numbers in different machines can select an unrelated reached use or overwrite an installation site. Preserve the owning machine with the operation through the record, encoding and consumers so each registration names its actual call.
- Reported: 2026-11-14, Codex, while reviewing stored-machine executable preparation

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L568), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1283), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs)
- Saw: A shared typed predecessor still runs the continuation's side-record clone even when no bodies are selected. The [declaration-sharing measurement](https://github.com/CathedralOS/Omega/pull/3681) recorded 46 remaining side-record arena clones during empty activation on both Windows x86-64 Build routes. [Successor symbol sharing](https://github.com/CathedralOS/Omega/pull/3687) subsequently removed the separate symbol-arena copies. Use the [same scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary) to measure current copy work.
- Cost: Empty activation pays to copy unrelated side records. Share immutable side records under their existing owners and keep child-list updates at completion-batch boundaries. Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Measure empty activation and nonempty completion on both routes, accounting separately for copied storage and new output. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Instruction selection can reach earlier program forms
- Category: knowledge crossing a boundary
- Where: [stage 03 manifest](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/Cargo.toml), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs#L309), [boundary rules](tools/xtask/src/boundary.rs#L396)
- Saw: The stage 03 crate imports leaf types from [target-operations](omega-rust/omega/compilation/representations/target-operations/src/lib.rs#L32), which also defines the stage 02 working graph. Its [provider-admission dependency](omega-rust/omega/compilation/realization/provider-admission/Cargo.toml) reaches abstract operations. The driver attaches [handler stacks](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L102) and [entry execution](omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L315) to stage 02's output after preparation. Plan and selection representations use canonical encoders from the [module codec](omega-rust/psi/compilation/representations/terminal-psi/codec/src/lib.rs), which also exposes the sealed Terminal Psi module. Prepared structural declarations still carry checking-form leaf scalars and [`RetainedIntegerTypes`](omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/lib.rs#L166), including an `Unsealed` case for module-free plans.
- Cost: The transitive check guards the representations selection uses. Finish the stage 03 boundary by moving shared leaf types out of the working graph crate, passing bound installations alongside the plan using authority records below provider admission, and separating canonical encoding from module access. Close the leaf scalars during preparation. Keep the surviving-installation roster tied to the prepared operation set and preserve rejection of mismatched installation authority. Require the stage 03 production crate itself to pass the transitive boundary check and forbid its direct dependency on the stage 02 working graph.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Stages 07 and 08 and final assembly are handed several forms at once
- Category: knowledge crossing a boundary
- Where: [driver](omega-rust/omega/compilation/native-compiler/src/pipeline.rs), [assembly](omega-rust/omega/compilation/backend/machine-emission/src/function_realization.rs), [later forms](omega-rust/omega/compilation/representations/machine-code/Cargo.toml)
- Saw: No stage from 04 to 09, no form from the prepared plan onward, and not the target register setup can reach `abstract-operations` any longer; `check-boundary` follows every production edge from them. But the driver hands stage 07 the selected program, the machine plan, the register model and the frame layout, and stage 08 those plus the encoding, instead of one input form each. `assemble_fixed_frame_realization` takes stage 02's output and the products of stages 05 through 09 at once. The later forms also embed records defined in `target-operations` and `legalized-operations` (call records, structural contracts, call-site owners), so `selected-instructions`, `register-homes` and `machine-code` depend on those crates.
- Cost: These stages read across several earlier forms, so a change to one form can silently change another stage's result. Fold what stages 07 and 08 need into the machine plan's form, give assembly one input, and move the embedded records to the form that owns them.
- Reported: 2026-11-16, Claude Code, while removing unused dependencies from stages 04 to 09.

### Image emission still reads the target operations stage 02 keeps as a working form
- Category: knowledge crossing a boundary
- Where: [object evidence](omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs)
- Saw: Stage 02 now publishes one execution plan, and the entry, profile, entry calling plan and surviving installations are read from it. `function_fragments.rs` still reads `target_operations()` for evidence checks alongside the abstract program (reference backing geometry, activation residence, access-reference checks), and borrowed-storage layouts still come from `TargetOperationPlan` because `NativeBorrowedStorageLayout` has no identity encoding to seal into the plan.
- Cost: Stage 02 has to keep exposing two complete graphs. Give borrowed-storage layouts a canonical encoding and move them into the plan, port the evidence checks to plan rows, then stop exposing `target_operations()` outside the stage and its validator.
- Reported: 2026-11-16, Claude Code, while moving the readers of stage 02's output onto the execution plan.
