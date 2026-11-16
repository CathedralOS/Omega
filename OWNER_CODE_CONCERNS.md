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

### Callable analysis hides its storage-flow refusal
- Category: Verifying costs more than writing
- Where: [callable projection](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/callable_values/terminal.rs#L124)
- Saw: `terminal_callable_values` replaces the reference graph's error with `callable provenance cannot reconstruct current Terminal storage flow`. The unchanged [indexed-case program](tests/fixtures/run/calls/indexed_case_discriminator/main.omg) produces that refusal before native compilation on both current-form source `70dfe3f` and clean landed base `d3deb345`; the underlying graph error is lost.
- Cost: A caller cannot name the failed operation or storage relationship without another diagnostic build. Preserve the graph's reason through the callable query and its callers; keep the program and its oracle unchanged.
- Reported: 2026-11-16, Codex, while qualifying current-form frame publication

### Changing storage analysis rebuilds the native compiler
- Category: Verifying costs more than writing
- Where: [semantic analysis package](omega-rust/psi/compilation/representations/terminal-psi/semantics/Cargo.toml), [declaration lookup in optimization](omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/structural_places.rs#L619), [target descriptor inputs](omega-rust/omega/compilation/representations/target-operations/target-vocabulary/src/descriptor_input.rs)
- Saw: Changing only the private descriptor replay cache and its error reporting rebuilt 64 packages and took 14m 17s before the original Console test could run. Some native consumers need only declaration or structural-path lookups from the same package; ownership analysis and explicit image validators genuinely need the full analysis. `target-operations` also declares an unused direct dependency, although removing it alone leaves a transitive dependency. Source and build evidence remains in claim `18e85a23de177c780000000018e83683`, under `post-3776-original-binding-control-49/native-terminal-semantics-coupling.json`.
- Cost: Edits to the analysis invalidate native consumers of small lookup functions. Move the declaration and path queries out of the full analysis crate, keep each judgment implemented once, and remove unused direct dependencies. Measure the same Console edit/build/run loop before claiming a build-time improvement.
- Reported: 2026-11-16, Codex, while carrying the original Console program through checking and execution

### Source correspondence repeats signature searches
- Category: Verifying costs more than writing
- Where: [source correspondence](omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs#L497)
- Saw: For each projected method, correspondence validation scans every inherited signature and derives its callable identity before interning it. Finding one match still exhausts the scan. The original Console binding replay43 took 329.112s before its next refusal. A brief sample caught this work, but captured only two samples; its share of total time is unknown. Raw evidence remains in claim `18e85a23de177c780000000018e83683`.
- Cost: Repeated identity normalization grows with both the method and signature counts. Prepare the exact callable/package identities once per inherited declaration set. Preserve source authentication, complete applications, and refusals for missing or ambiguous requirements. Validate with the unchanged Console program and refusal controls; measure the identity derivations before claiming a speedup.
- Reported: 2026-11-16, Codex, while carrying the original Console binding and stored-machine programs through checking and execution

### Every native caller evaluates constants between two driver calls
- Category: one idea, several implementations
- Where: [native completion](omega-rust/omega/build-system/orchestration/src/compile/native_completion.rs#L123), [scalar completion](omega-rust/omega/compilation/native-compiler/src/scalar_completion.rs), [Build's completer](omega-rust/omega/build-system/build-settings/src/activation/scalar_completion.rs#L20)
- Saw: Omega 00 now runs whole inside `native_compiler::compile`, but callers still call `native_compiler::prepare`, run `build_settings::scalar_completion::complete` on every `NativeScalarCompletion`, then call `compile`. Preparation reads only the retained Terminal product and its native proposal (`TerminalNativeRealizationProposal::prepare_native_execution`, `PreparedNativeExecution::scalar_application_demand`). Product checking separately completes the same artifact's conditions with an empty demand (`ProducedTerminalProduct::check_conditions`).
- Cost: Orchestration, the review example and compiler tests each repeat the prepare, evaluate, compile sequence, and conditions are evaluated twice. Select the execution and complete its demand once where the retained native product is formed, pass the completed scalars in `NativeCompileRequest`, and delete `NativeScalarCompletion` and the split entry.
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

### Specialization failures discard the reason they stopped
- Category: Verifying costs more than writing
- Where: [specialization](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs)
- Saw: many specialization checks still return `Stop::Unserved(None)`. The Console binding replay reached an anonymous nested-parameter mismatch; adding the missing reason required another optimized compiler build and replay. The current repair gives that mismatch a reason, but adjacent shape, lookup and argument checks still discard theirs.
- Cost: the next failure names only the unavailable application, hiding the failed check. Carry each check's reason through the existing refusal path so one replay identifies the repair, without changing which programs are accepted.
- Reported: 2026-11-15, Codex, while restoring Console preparation for stored-machine execution

### Focused compiler controls spend twenty-two minutes in code generation
- Category: Verifying costs more than writing
- Where: [compiler test targets](omega-rust/omega/compilation/compiler/Cargo.toml#L110), [compiler feature alignment](omega-rust/omega/compilation/compiler/Cargo.toml#L102), and the `coherent-native-controls-24-01` receipt in continuation claim `18e81ab82af520780000000018e96239`
- Saw: On macOS arm64 at `a827cf2c269`, a fixed-source nextest selection of 24 controls across 12 binaries spent 22m17s building before 176.905s of test execution. The command used the retained CLI feature union, optimization level 1, two codegen units and three jobs; 1,219 tests were not selected. Actual compiler processes remained active. After integrating main `a2684962ce882`, 13 selected unit controls across six crates spent 15m49s building and 0.414s running on the same host and budget (`merge-a268-direct-controls-01`, with exact source patch and input records retained in the active claim). The source of this build cost has not been established.
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
- Where: [body activation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L669), [continuation storage clone](omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1295), [owned arena clone](omega-rust/psi/foundation/arena/src/arena.rs#L748)
- Saw: Replacing symbols on a shared typed tree can copy side records, normalized identities and owned containers even with no bodies selected. The [call and machine record repair](https://github.com/CathedralOS/Omega/pull/3747) measured empty activation and two nonempty batches on both Windows x86-64 routes. Empty activation requested 2,182,456 bytes in 12,456 allocations and cloned 36 arenas per route on fixed `570d707` with the repair and observer (opt-level 1). See the [data-child repair](https://github.com/CathedralOS/Omega/pull/3720) for the earlier baseline.
- Cost: Share immutable storage where measurements still show copies. Limit writes to changed records or new batches. Preserve source application preparation, projection, authority checks, and signature and inferred-summary dependencies. Re-measure empty activation and nonempty completion on both [scalar and borrowed-root workloads](wiki/drafts/measurements/selected_body_copies.md#workloads-and-boundary), counting copied storage and new output separately. See the [body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-15, Codex, while measuring continuation construction and body completion.
