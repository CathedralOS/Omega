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

### Literal byte-view validation rescans each function for its producer
- Category: re-deriving
- Where: [literal producer lookup](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/native-preparation/src/legalization_scalar_graph.rs#L5308), [function inputs](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/native-preparation/src/legalization_function_inputs.rs#L14)
- Saw: Each lookup scans the structural declarations and every operation in the function to find one exact literal producer. Call reconstruction, provenance checks and home retirement can repeat the lookup for the same literal, even though the validator already prepares indexed input rows for the function.
- Cost: A function with a growing number of literal views can do quadratic lookup work during independent validation. Index producers with the existing function inputs and preserve rejection of mismatched declarations, duplicate producers and nonliteral homes. Measure lookup work with a growing literal roster.
- Reported: 2026-11-15, Codex, while validating a shared byte view across native edges.

### Conformance witnesses rebuild the same calling ABI catalog
- Category: re-deriving
- Where: [witness loop](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/compilation.rs#L225), [calling ABI preparation](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/calling_abis.rs#L66), [catalog owner](omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs#L1348)
- Saw: The witness loop passes the same typed program and purpose into each elaboration. Elaborations that need boundary metadata recompute the toolchain source identities and build a full `PreparedServiceCatalog`. A one-second macOS arm64 sampler during [note_vault](samples/cli/systems/note_vault/main.omg) checking at `2d3abb168fe` observed catalog preparation in 16 of 30 samples on the active compiler worker.
- Cost: Each conformance can repeat whole-program service preparation before choosing its boundary rows. Reuse the catalog for matching program ownership, dependency scope, and source authorities. Measure catalog preparations across the same witness roster and preserve applicability and caller-specific ABI rows.
- Reported: 2026-11-15, Codex, while checking the filesystem sample through recursive contracts.

### Selected-body activation copies retained side records
- Category: re-deriving
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L568), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1283), [retained pages](omega-rust/psi/foundation/arena/src/retained.rs)
- Saw: A shared typed predecessor still copies other side records, normalized identities and owned containers even when no bodies are selected. The [declaration-sharing measurement](https://github.com/CathedralOS/Omega/pull/3681) recorded 46 remaining side-record arena clones during empty activation on both Windows x86-64 Build routes. [Successor symbol sharing](https://github.com/CathedralOS/Omega/pull/3687) subsequently removed the separate symbol-arena copies. A historical matched pair on fixed `5bea014` plus the data-child repair and numeric observer recorded 2,756,552 requested bytes, 12,476 allocations and 42 arena clones on both routes (Windows x86-64, opt-level 1); this is not current-main qualification. Use the [same scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary) to measure current copy work.
- Cost: Empty activation pays to copy unrelated side records. Share the remaining immutable side records and owned containers under their existing owners, and keep updates local to the records or completion batches that change. Preserve source application preparation, projection, source authority checks, and signature and inferred-summary dependencies. Measure empty activation and nonempty completion on both routes, accounting separately for copied storage and new output. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.

### Instruction selection's inputs still carry earlier-stage pieces
- Category: knowledge crossing a boundary
- Where: [stage 03 manifest](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/Cargo.toml), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs#L309), [boundary rules](tools/xtask/src/boundary.rs#L396)
- Saw: Stage 03 now takes installation authority as the plain [`BoundInstallations`](omega-rust/omega/compilation/backend/runtime/bound-installations/src/lib.rs) record and sits under both transitive rules in `tools/xtask/src/boundary.rs`. The driver still attaches [handler stacks](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L102) and [entry execution](omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L315) to stage 02's output after preparation, because image emission reads them back from that output (`installed_entry_execution`, `installed_stack_requirements` in `image-emission`). Plan and selection representations use canonical encoders from the [module codec](omega-rust/psi/compilation/representations/terminal-psi/codec/src/lib.rs), which also exposes the sealed Terminal Psi module.
- Cost: Stage 02's output is still mutated after preparation. Once image emission takes the installed entry and handler stacks from the bound record instead of stage 02's output, delete `with_installed_entry_execution` and `with_installed_stack_requirements` and pass the record beside the plan to both consumers. Separate canonical encoding from module access.
- Reported: 2026-11-16, Claude Code, while auditing the stage 02 to stage 03 boundary.

### Later machine forms embed records from the prepared plan
- Category: knowledge crossing a boundary
- Where: [later forms](omega-rust/omega/compilation/representations/selected-instructions/Cargo.toml), [frame step](omega-rust/omega/compilation/backend/machine-emission/src/function_realization.rs), [driver](omega-rust/omega/compilation/native-compiler/src/pipeline.rs)
- Saw: Stages 07, 08 and 09 now each read one form and keep it in their output, and final assembly takes stage 09's output alone; `check-boundary` pins those signatures (`SINGLE_FORM_ENTRIES`). Three joins remain. `selected-instructions` and `register-homes` embed records defined in `legalized-operations` (call records, structural contracts, call-site owners), so they depend on the prepared plan's crate. The frame step (`stage_function_frame`) is handed both the allocation and stage 06's output, because stage 06's output does not keep the allocation it was computed from. Fragment emission is handed stage 02's output beside the realization (see the image-emission entry below); it checks the two share one pre-physical manifest.
- Cost: A change to a legalized-operations record changes the selected and allocated forms without passing through stage 03. Move the embedded records to the form that owns them, and let stage 06's output keep its allocation so the frame step reads one form.
- Reported: 2026-11-16, Claude Code, while removing unused dependencies from stages 04 to 09; narrowed the same day once stages 07 to 09 read one form each.

### Image emission still reads the target operations stage 02 keeps as a working form
- Category: knowledge crossing a boundary
- Where: [object evidence](omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs), [stage 02 output](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lib.rs)
- Saw: Stage 02 now publishes one execution plan, and the entry, profile, entry calling plan and surviving installations are read from it. `function_fragments.rs` still reads `target_operations()` for evidence checks alongside the abstract program (reference backing geometry, activation residence, access-reference checks), and borrowed-storage layouts still come from `TargetOperationPlan` because `NativeBorrowedStorageLayout` has no identity encoding to seal into the plan.
- Cost: Stage 02 has to keep exposing two complete graphs. Give borrowed-storage layouts a canonical encoding and move them into the plan, port the evidence checks to plan rows, then stop exposing `target_operations()` outside the stage and its validator.
- Reported: 2026-11-16, Claude Code, while moving the readers of stage 02's output onto the execution plan.
