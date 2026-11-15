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

### The bounded concat probe describes an operation its source no longer uses
- Category: the documents disagree
- Where: [probe guide](samples/cli/probes/bounded_concat/README.md#L5), [explicit copies](samples/cli/probes/bounded_concat/main.omg#L45), [fixed-array rule](wiki/spec/language/domains.md#domains-and-qualification)
- Saw: The guide advertises direct bounded concatenation and a green check. Since `c7ae4c207c2`, the source uses two explicit copies into fixed array storage. At `d0e4a8657962` on macOS arm64, checking exits 1 and paired execution exits 200. The retained probe reaches unresolved ASCII declarations or memberships and a retired data-attached `Main::main`.
- Cost: The probe supplies no concatenation evidence and currently cannot check the explicit-copy result. Restore its current explicit storage and live-length oracle through ordinary source owners, and describe the operation the probe actually exercises.
- Reported: 2026-11-15, Codex, while tracing retained byte-sequence target operands.

### An affine-view refusal hides the source sequence bounds test
- Category: tests that pin the bug
- Where: [literal index witness](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/sequence_values.rs#L36), [exact diagnostic assertion](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/sequence_values.rs#L207)
- Saw: The isolated test fails on clean `1b4247dd558` and the computed-extent candidate with “affine element views” on `forbidden_index(value: [Item])`. The expected refusal is the missing live length and bounds fact for `value[0].number`. The test reaches the earlier wrong-element-identity check.
- Cost: The final negative never reaches the operation-safety judgment. Determine how an exact checked view can observe the same scalar field through the original owned sequence before restoring the bound oracle.
- Reported: 2026-11-15, Codex, while validating checked type computations.

### A target-computed array extent lacks its selected provider during evaluation
- Category: knowledge crossing a boundary
- Where: [authored extent and size query](samples/cli/facts/target_layout_derived_constant/main.omg#L8), [closed computation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/const_argument_jobs.rs), [invocation admission](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs)
- Saw: A direct array field whose length calls a target-independent helper runs in native and Psi and reopens. On `f8e3039998f`, the unchanged target-layout sample advances to `limit()` at line 8, column 26 and refuses with “source invocation requires selected target, scalar or external authority that is not supplied”. Its Build selects different `Answer` implementations for Linux and macOS.
- Cost: The early checked computation has no selected provider. Carry the symbolic extent into the existing checked scalar application and target-closure path so the request retains its exact selected realization, result and proof. Keep the original layout query, bound and invalid twin.
- Reported: 2026-11-15, Codex, while restoring the exact historical layout input inventory. Narrowed after the target-independent field extent ran and reopened.

### Two package-review probes use retired source declarations
- Category: zombie machinery
- Where: [qualified recursion](tests/fixtures/pass/calls/machine_self_call_recursion_compile/main.omg#L13), [dynamic slot](tests/fixtures/pass/traits/runtime_dyn_slot_metadata_exit/main.omg#L17)
- Saw: With both kept paths on `8329cdc221e`, the package custody-and-budget witness refuses a data-qualified machine at line 13, column 9. The slot metadata witness rejects `dyn` at line 17, column 19.
- Cost: The first probe cannot test the exact accepted package and byte limit. The second cannot recover its checked returned-borrow origin or `Utf8` precondition. Establish current source forms for the original behaviors before applying the unchanged review assertions.
- Reported: 2026-11-15, Codex, while reconnecting the package-review fixture paths.

### Installation-bound reach stops before the provider union
- Category: knowledge crossing a boundary
- Where: [staggered provider fixture](tests/fixtures/pass/providers/staggered_installation_reach_exit/main.omg), [union and review checks](omega-rust/omega/compilation/compiler/tests/package_review.rs#L83)
- Saw: With the kept path on `8329cdc221e`, Psi checking refuses “machine service reach bound by an installation” for `AProvider::visit`, `BProvider::visit`, `CProvider::visit`, and `DProvider::visit`.
- Cost: The already formulated assertion that A retains both the earlier B and later D contributions cannot run. Carry the original installation-bound declarations into selected reach and the existing review records.
- Reported: 2026-11-15, Codex, while reconnecting the staggered package-review fixture.

### Review refuses the original opaque native binding
- Category: knowledge crossing a boundary
- Where: [authored native binding](tests/fixtures/pass/boundary/native_provider_unoffered_read/build.omg), [inherited ceiling oracle](omega-rust/omega/compilation/compiler/tests/package_review.rs#L627)
- Saw: The unchanged `Probe::read` fixture on `8329cdc221e` now reaches ordinary captured package review, which refuses “the complete selected installation reach or native supply” before policy recovery.
- Cost: The test cannot show that the opaque implementation inherits the original blocking permission from its requirement. Project the exact selected native binding and its checked inherited ceiling into the existing package policy.
- Reported: 2026-11-15, Codex, while restoring the kept native review fixture path.

### Float fixtures stop before target and requirement policy checks
- Category: tests that pin the bug
- Where: [rootless conversion](tests/fixtures/pass/float/rootless_target_realization_scope/main.omg), [selected conversion](tests/fixtures/packages/selected-provider-unsupported-target/main.omg), [Float::add owner check](omega-rust/omega/compilation/compiler/tests/package_review.rs#L598)
- Saw: On `8329cdc221e`, the two conversion fixtures stop at `coordinates::entry::rx` with “call receiver, requirement or coercion call”. The separate kept `Float::add` case stops in `carry` with “numeric primitive has no canonical result contract yet”.
- Cost: The library and selected UEFI twin never distinguish target capability, and the closed addition never reaches the toolchain owner in recovered policy. Retain each authored operation while completing its canonical call contract.
- Reported: 2026-11-15, Codex, while replaying kept package-review numeric fixtures.

### A clause-call refusal hides the nonreturning value denotation test
- Category: tests that pin the bug
- Where: [nonreturning denotation fixture](tests/fixtures/fail/proofs/ordinary_denotational_call_requires_normal_return/main.omg), [UEFI fixture assertions](omega-rust/omega/compilation/compiler/tests/package_review.rs#L23)
- Saw: On `8329cdc221e`, the kept negative fixture refuses an unsupported “contract call in a clause term” at the signature of `harmless`. The test expects the specific rejection that the call has no observation-free, reach-free, crash-free value denotation.
- Cost: The negative never exercises the semantic test for an ordinary call that cannot return normally. Carry the original clause call through its contract reader and preserve the narrow denial.
- Reported: 2026-11-15, Codex, while reconnecting the reusable-package negative fixtures.

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

### A typed slice stored in a field cannot use its own bounds to index
- Category: branch per shape
- Where: [scalar element read lowering](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs) (`read_projected_scalar`), [element view operations](wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views)
- Saw: The [typed slice fixture](tests/fixtures/run/views/typed_slice_index/main.omg) checks a scalar element and a copyable record field using their own descriptor lengths. In a separate macOS arm64 probe, `windows: &Windows` observes `windows.observed.len`, where `observed: &'buffer [u32]`. With `windows.observed.len >= 1`, `windows.observed[0]` still refuses with “slice read without a whole descriptor.” Returning a scalar literal in the guarded arm runs with native and Psi exit 0 using the same record construction, loan and length read.
- Cost: A guarded read loses the descriptor's projected path at the scalar operation. Preserve the same storage and length through its existing exact descriptor access. Keep a sibling descriptor's bound and a non-copy element read refused.
- Reported: 2026-11-15, Codex, while checking native backing for owned record sequences. Narrowed after replaying whole descriptors and projected length.

### A layout-query constant lacks target authority at evaluation
- Category: knowledge crossing a boundary
- Where: [kept layout-query constant](tests/fixtures/pass/layouts/runtime_constant_layout_contract_exit/main.omg#L5), [constant evaluation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/constant_jobs.rs), [folded-constant closure](omega-rust/psi/compilation/representations/terminal-psi/src/terminal_module.rs#L17)
- Saw: A record constant computed through a `LayoutStorage` application now runs once under each deployment's admitted layout and folds the value all of them agree on (`fixed_layout_storage/const_name` exits 0 natively and in Psi). On that same branch the kept `const BYTES: u64 = size_of<Cell>();` still refuses: "source invocation requires selected target, scalar or external authority that is not supplied". Admission in `source_jobs.rs` finds a target, scalar or external demand on the invocation; the per-deployment run supplies only the target's admitted layouts, not its scalar observations (which demand list is non-empty was not isolated). The historical scalar control exposes an [earlier computed extent refusal](#a-computed-array-extent-blocks-a-layout-constant-before-target-evaluation). The existing Psi folded-constant rows have readers and no source producer.
- Cost: Supply each deployment's target observations to the constant's per-deployment run, as its layouts now are, and fold it only when the deployments agree. Publish the exact source-selected provider commitment through the existing folded-constant closure. Preserve the wrong-target, absent/repeated plan, equal-result substitution and cast refusals.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles; narrowed the same day by Claude Code after the record constant was repaired.

### Six Stage06 loan and ranking controls fail on clean main
- Category: tests that pin the bug
- Where: [two-input reference](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/loan_regions.rs#L256), [literal arrivals](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/machine_lifetimes.rs#L428), [slice loop ranks](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/rankings.rs#L357), [short-circuit cleanup](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/successor_loans.rs#L195)
- Saw: On clean linux x86_64 `8bfa8ba2dee`, `mbx test -p terminal-psi-to-terminal-psi --lib` fails six tests that passed on `15145194c7d`: `a_reference_backed_by_either_of_two_inputs_is_refused_by_name` now has a longer refusal than its pinned text; both `machine_lifetimes` literal-arrival tests and `cleanup_must_keep_backing_on_the_edge_that_still_uses_its_view` refuse with "an edge leaves a frontier its successor does not enter"; and both slice-loop `rankings` tests fail.
- Cost: The loan and ranking suites no longer show whether the merges since `15145194c7d` broke edge frontiers for views. Bisect the merges between those commits and repair the regression before touching any oracle; the pinned refusal text may just need the new location suffix.
- Reported: 2026-11-15, Claude Code, while repairing the Stage06 reference controls.

### A textual merge can pass Admission with a Rust type error
- Category: verifying costs more than writing
- Where: [pre-merge Admission](.github/workflows/admission.yml#L68), [post-merge checks](.github/workflows/post-merge-check.yml#L64), [atomic store reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/atomics.rs#L223)
- Saw: [PR #3326](https://github.com/CathedralOS/Omega/pull/3326) had green Admission, then squash commit `d5d5c987120` failed a clean macOS arm64 Rust build with E0533. The new evaluation statement shape and the existing atomic-store constructor were in different files. Git merged the text without a conflict. Admission runs rules and formatting on the merge, and the compiler checks start after publication.
- Cost: Unrelated compiler builds and two in-flight repairs stopped before tests could run. Compare the latency and detection coverage of checking the affected crates on a warmed merge tree with the current post-merge validation.
- Reported: 2026-11-15, Codex, while validating copyable record constants and the owned-element call path.

### An assigned local worktree can lose its warm target between checks
- Category: verifying costs more than writing
- Where: [pool assignments](tools/xtask/src/worktree.rs#L166), [pool cache retention](tools/docs/testing.md#L589), [mbx 1.29.2 collection order](https://github.com/jdx/mr-boxington/blob/v1.29.2/docs/managed-targets.md#keep-or-evict-specific-checkouts)
- Saw: On macOS arm64 at `c55c52f59b1`, below the workstation's 45.0 GiB free-space floor, mbx 1.29.2 deleted assigned `worker-8`'s target between commands. The next test build compiled from cold in 3m 12s. In two isolated checkouts, `MBX_TARGET_EVICT_FIRST` changed one collector's choice under a one-target budget. An independent collector reverted to oldest-first. With a zero budget and simulated low disk, both a running `mbx run` lease and an explicit `MBX_TARGET_KEEP` kept the view. Once the command ended, the same scratch view could be collected unless explicitly kept.
- Cost: Worktree assignments outlast mbx command leases. A per-command eviction hint cannot protect the next check from an independent collector, and a hard keep can exceed the disk budget. Add an advisory priority for managed views to mbx and set it from the pool’s existing assignment state. Respect explicit operator rules and running-command locks, and still let assigned views be reclaimed under disk pressure.
- Reported: 2026-11-15, Codex, while validating const-index record arguments

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [activation copy](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L565), [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L942), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L727)
- Saw: `complete_selected` starts from typing retained for later selections. Source activation still copies that typed program when the body batch is empty, because the following projection rewrites its declaration roots, side tables and selected source symbols. Projection narrows the resulting owned program in place.
- Cost: Even an empty body batch needs a copy of unrelated typed storage with the current projection owner. Moving the copy into projection leaves the cost. Per-table copy-on-write still copies body arenas that completion appends to, and `Arena` exposes contiguous spans. If repeated nonempty completion proves costly, use the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract) to share immutable declaration and type storage and publish body replacements through stable handles. Preserve exact source authority, refusal behavior, and signature and inferred-summary dependencies.
- Measured: The two Build routes from the [earlier measurement](wiki/drafts/measurements/selected_body_copies.md) on macOS arm64 at `d66164cd0f5` each selected 75 source files with zero bodies to finish. Activation requested 32,333,303 bytes across 14,553 allocations and cloned 101 arenas. Projection's typed extraction allocated zero bytes. Neither route measures repeated nonempty continuations.
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals; narrowed 2026-11-15, Claude Code, after removing the projection copy

### Reflection runtime visitation and the wire policy still stop at separate boundaries
- Category: knowledge crossing a boundary
- Where: [reflection elaboration](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization/reflected_fields.rs), [wire protocol sample](samples/cli/systems/wire_protocol/main.omg#L170)
- Saw: `reflect::inspect_record_fields` closes into checked per-field calls. The runtime record visitors, mutable visit and active-case visitors still have no elaboration. The unchanged wire example independently reaches its retired data-owned `machine AuthoredCompactBinary::plan(...)` at line 170. The `policy_kind` specializations for `RoomHeader` and `FixedVec<i32, 4>` also independently refuse `expression type expression` on the baseline and candidate.
- Cost: Runtime visitors need typed field and case projections with ordinary borrow checking. The wire example needs the checked replacement for its retired machine declarations and needs to decide the specialized type comparisons.
- Reported: 2026-11-15, Codex and Claude Code, while checking the wire policy across the portable interpreter and native compiler.
