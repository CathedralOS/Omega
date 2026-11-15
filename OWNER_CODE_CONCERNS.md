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
- Merge overlapping reports into one entry for the defect that remains; do not keep duplicates.
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

### Fixed-array windows reach slice formation only for bytes
- Category: branch per shape
- Where: [fixed-array view elaboration](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/arrays.rs#L309), [element subslice contract](wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views)
- Saw: On clean `67fda671ffc` on macOS arm64, `extent(&numbers[1..2])` refuses with “partial typed array views need their element-view subslice formation” for `numbers: [u32; 2]` and `extent(view: &[u32]) -> u64 { view.len }`. The whole typed-array twin reads length 2, and the same partial range and length check over `u8` reads length 1. Both twins exit 0 in native and Psi and reopen independently. The elaborator rejects the typed range before forming its array loan or window.
- Cost: The existing two-endpoint subslice rule cannot receive this typed array's backing and element extent. Carry the shared array loan into the existing element-window operation, keeping the actual range and distinct ordered bounds.
- Reported: 2026-11-15, Codex, while checking typed slice fields through a runtime-selected array element.

### A target-computed array extent lacks its selected provider during evaluation
- Category: knowledge crossing a boundary
- Where: [authored extent and size query](samples/cli/facts/target_layout_derived_constant/main.omg#L8), [closed computation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/const_argument_jobs.rs), [invocation admission](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs)
- Saw: A direct array field whose length calls a target-independent helper runs in native and Psi and reopens. On `f8e3039998f`, the unchanged target-layout sample advances to `limit()` at line 8, column 26 and refuses with “source invocation requires selected target, scalar or external authority that is not supplied”. Its Build selects different `Answer` implementations for Linux and macOS.
- Cost: The early checked computation has no selected provider. Carry the symbolic extent into the existing checked scalar application and target-closure path so the request retains its exact selected realization, result and proof. Keep the original layout query, bound and invalid twin.
- Reported: 2026-11-15, Codex, while restoring the exact historical layout input inventory. Narrowed after the target-independent field extent ran and reopened.

### Float fixtures stop before target and requirement policy checks
- Category: tests that pin the bug
- Where: [rootless conversion](tests/fixtures/pass/float/rootless_target_realization_scope/main.omg), [selected conversion](tests/fixtures/packages/selected-provider-unsupported-target/main.omg), [Float::add owner check](omega-rust/omega/compilation/compiler/tests/package_review.rs#L598)
- Saw: On `8329cdc221e`, the two conversion fixtures stop at `coordinates::entry::rx` with “call receiver, requirement or coercion call”. The separate kept `Float::add` case stops in `carry` with “numeric primitive has no canonical result contract yet”.
- Cost: The library and selected UEFI twin never distinguish target capability, and the closed addition never reaches the toolchain owner in recovered policy. Retain each authored operation while completing its canonical call contract.
- Reported: 2026-11-15, Codex, while replaying kept package-review numeric fixtures.

### The native callback test spends minutes deriving a fixture entry binding
- Category: verifying costs more than writing
- Where: [standard-library entry check](omega-rust/omega/compilation/compiler/tests/support/fixture_package_inputs.rs#L144), [source-check storage uses](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L3929)
- Saw: With the kept callback fixture bytes on macOS arm64 at `8329cdc221e`, the run was still checking the standard library after 10m21s and was stopped. A process sample shows `derive_entry_from_inputs` inside fixed-layout evaluation, with Psi 06 repeatedly walking `Storage::uses`, `reaching`, and `supplied`. The test already prepared the C provider.
- Cost: Exact-value and case queries still retrace reconverging copies: the twelve-branch storage control scans 475,120 rows for the exact value. Initialization queries now expand each copy equation once, but that does not establish the callback's end-to-end cost. Replay the actual supplier capture on macOS arm64, retain the route-owned entry schema, and reach the original exact callback binding and native saved-input substitution checks; a loose standard-library source check is not that path.
- Reported: 2026-11-15, Codex, while replaying the native callback fixture after repairing its path.


### Static callable specialization needs a portable receipt for open structural reach
- Category: knowledge crossing a boundary
- Where: [static operand reader](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs), [checked application producer](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/declare.rs#L537), [retained reach reader](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/calls.rs#L2324), [open structural requirements](wiki/spec/language/effects.md#deferred-callback-reach)
- Saw: On `f8687f1fcd3`, `apply<picked>(7u64)` refuses at static binding for `machine apply<const Callback: machine(value: u64) -> u64>(value: u64) -> u64 { Callback(value) }`. Wiring the existing binder judgment and specialization receipt in a local prototype let native and Psi exit 0. It rejected a callback requiring `value <= 8u64` and a generic body claiming a result absent from its binder. The saved runtime artifact could not reopen: `checked specialization has no retained application`. The producer only materializes type and scalar telescopes. The retained reader rejects a structural binder in the reach dependency even though omission on that binder means open reach. Changing the body to `transition { true -> Callback(value) }` also exposed a named call that bypasses static-call preparation. The kept [package review witness](tests/fixtures/pass/facts/static_callback_reach_boundary_schema/main.omg#L62) on `8329cdc221e` stops at line 62, column 37 because no pass specializes the call's static arguments. Its policy oracle requires `StepContract::step` to recover the original static `Count` binder.
- Cost: Route ordinary callable arguments and named transition calls through the existing binder check and exact source application. Generalize the retained reach dependency to preserve the originating open or finite bound across production and readback. An empty finite bound cannot stand for omitted reach.
- Reported: 2026-11-15, Codex, macOS arm64, while carrying the static callback witness into a saved Psi artifact.

### A folded layout constant loses the selected provider commitment
- Category: knowledge crossing a boundary
- Where: [historical layout control](samples/cli/facts/target_layout_derived_constant/main.omg#L8), [constant evaluation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/constant_jobs.rs), [folded-constant closure](omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module.rs#L17)
- Saw: `runtime_constant_layout_contract_exit` now evaluates the named byte count and checks the direct and Boolean-composed layout requirements in the selected target. It exits 0 in native and Psi on macOS arm64, and the retained artifact reopens. The unchanged `false_constant_layout_contract` still refuses at the false target predicate. The historical selected provider remains blocked by [the target-computed array extent](#a-target-computed-array-extent-lacks-its-selected-provider-during-evaluation). The folded-constant rows have readers and no source producer.
- Cost: When the historical extent can evaluate, folding must retain the exact source-selected provider in the existing folded-constant closure. Preserve the wrong-target, absent/repeated plan, equal-result substitution and cast refusals.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles. Narrowed after the ground layout contracts ran.

### A textual merge can pass Admission with a Rust type error
- Category: verifying costs more than writing
- Where: [pre-merge Admission](.github/workflows/admission.yml#L68), [post-merge checks](.github/workflows/post-merge-check.yml#L64), [atomic store reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/atomics.rs#L223)
- Saw: [PR #3326](https://github.com/CathedralOS/Omega/pull/3326) had green Admission, then squash commit `d5d5c987120` failed a clean macOS arm64 Rust build with E0533. The new evaluation statement shape and the existing atomic-store constructor were in different files. Git merged the text without a conflict. Admission runs rules and formatting on the merge, and the compiler checks start after publication.
- Cost: Unrelated compiler builds and two in-flight repairs stopped before tests could run. Compare the latency and detection coverage of checking the affected crates on a warmed merge tree with the current post-merge validation.
- Reported: 2026-11-15, Codex, while validating copyable record constants and the owned-element call path.

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [activation copy](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L565), [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L942), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L727)
- Saw: `complete_selected` starts from typing retained for later selections. Source activation still copies that typed program when the body batch is empty, because the following projection rewrites its declaration roots, side tables and selected source symbols. Projection narrows the resulting owned program in place.
- Cost: Even an empty body batch needs a copy of unrelated typed storage with the current projection owner. Moving the copy into projection leaves the cost. Per-table copy-on-write still copies body arenas that completion appends to, and `Arena` exposes contiguous spans. If repeated nonempty completion proves costly, use the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract) to share immutable declaration and type storage and publish body replacements through stable handles. Preserve exact source authority, refusal behavior, and signature and inferred-summary dependencies.
- Measured: The two Build routes from the [earlier measurement](wiki/drafts/measurements/selected_body_copies.md) on macOS arm64 at `d66164cd0f5` each selected 75 source files with zero bodies to finish. Activation requested 32,333,303 bytes across 14,553 allocations and cloned 101 arenas. Projection's typed extraction allocated zero bytes. Neither route measures repeated nonempty continuations.
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals; narrowed 2026-11-15, Claude Code, after removing the projection copy
