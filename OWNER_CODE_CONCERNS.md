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

### Standalone Psi reports its exits as terminal
- Category: the documents disagree
- Where: [public naming](wiki/README.md#public-names), [Psi exit reporting](omega-rust/omega/application/src/commands/run.rs#L177), [execution test](omega-rust/omega/application/tests/psi_runtime_admission.rs#L45)
- Saw: On macOS arm64 at `1bf1f6299d2`, `omega run --engine psi --target macos_arm64` executes the unchanged fixed-layout field sample successfully and prints `terminal exit 0`. The flag and help call the engine Psi, but the success report and existing tests use an internal stage label.
- Cost: The flag and status use different names for the same engine. Bring the public report and its test expectations into the Psi naming contract together.
- Reported: 2026-11-15, Codex, while verifying the target exception in run help.

### A last-use ownership refusal drops its source site
- Category: knowledge crossing a boundary
- Where: [affine walk](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/plain_affine.rs#L1650), [source refusal adapter](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/reference_ends.rs#L47)
- Saw: On macOS arm64 at `a532f60be46`, the retained fixed-layout field sample refused without a source location. A focused trace identified `OperationId(23)` at the proposed reference release. The affine walk attaches an operation identity to errors from the operation itself but drops the known identity from last-use planning errors; the stage 05 adapter also leaves the source span empty.
- Cost: Another last-use planning error can leave a reader unable to locate the failing borrow or use. Keep the existing error site through planning and use its authored source correspondence when available.
- Reported: 2026-11-15, Codex, while tracing the fixed-layout field samples.

### A typed slice refuses indexing after proving its live extent
- Category: branch per shape
- Where: [typed collection reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/arrays.rs#L442), [collection access contract](wiki/spec/language/expressions.md#indexing-and-ranges)
- Saw: On macOS arm64 on clean `a532f60be46`, the same fixed Ticket array supports a native and Psi typed-slice length read and an indexed read through a fixed-array reference. Its borrowed `&[Ticket]` indexed read refuses before discharge when `requires tickets.len >= 1` proves index 0, and when the same fact is established by a local branch. The source reader rejects every extent other than a fixed literal.
- Cost: The checker cannot use ordinary contract or branch evidence to read through a typed slice's own descriptor. Feed its exact path, current length and index into the existing checked access operation. Keep an out-of-bounds index and a bound from another slice refused.
- Reported: 2026-11-15, Codex, while checking native backing for owned record sequences.

### Saved named record sequences lose their storage contracts
- Category: knowledge crossing a boundary
- Where: [constant-element sequence](tests/fixtures/run/ownership/owned_record_sequence/main.omg#L9), [runtime-valued sequence](tests/fixtures/pending/ownership/runtime_record_sequence/main.omg#L7), [payload replay](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptors.rs#L5677)
- Saw: On clean `8bfa8ba2dee` on macOS arm64, the constant-element sequence exits 0 in native and Psi. Its saved artifact fails inspection in `main`, block 5, operation 10 with `established payload contract is absent`. The runtime-valued fixture exits 0 in Psi and its artifact fails in block 3, operation 5 with `structural store backing is unresolved`. Both name the returned sequence in a local before passing it by value. An unchanged local-only literal and a direct returned-value call both reopen.
- Cost: Programs can run with a named returned sequence that the independent reader cannot follow through storage. Retain the exact backing and payload contract at the existing store and join machinery and keep unmatched backing refused.
- Reported: 2026-11-15, Codex, while independently checking the native record-sequence work.

### Console reload treats a mutable byte window as a replaceable descriptor
- Category: knowledge crossing a boundary
- Where: [Console requirement](source/library/std/console.omg#L36), [formal lowering](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower.rs#L529), [writable descriptor slots](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_replay.rs#L7341), [boundary output coverage](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptors.rs#L13050)
- Saw: On macOS arm64 with the borrowed-slice ABI repair based on `dd0a0349985`, the unchanged Console program prints `K` and exits 0 in native and Psi. Independent inspection of its saved artifact reaches `boundary reference parameter contract has incomplete leaf coverage`. Two selected provider programs differing only between `&mut [u8]` and `&mut &mut [u8]` on the retained requirement and its implementation both run and reach that same refusal. Their raw structural formals both name an unrestricted mutable borrow of the same `BorrowedView { access: Some(MutableBorrow) }` shape and both have an empty reference-output roster.
- Cost: The published structural formal loses the distinction between lending bytes and lending a stored descriptor that can be replaced. Carry the source's direct-view role through the shared formal owner using the existing canonical descriptor with access on the parameter. Preserve output coverage for the writable stored descriptor and keep provider and calling signatures aligned.
- Reported: 2026-11-15, Codex, while repairing saved byte-view calling signatures.

### A fixed-layout record constant loses its target selection
- Category: knowledge crossing a boundary
- Where: [kept initializer](samples/cli/facts/fixed_layout_storage/const_name/main.omg#L28), [constant evaluation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/constant_jobs.rs#L720), [shared Product preparation](omega-rust/psi/compilation/source-compiler/src/source_checking.rs#L162)
- Saw: On clean `8bb3a79076b` on macOS arm64, `const_name` refuses at `SAVED` with `TargetLayoutSelectionMissing(LayoutApplicationId(1))`. The unchanged `call_result` exits 0 in both engines and `invalid_const_name` reaches its exact cast refusal. An isolated replay with the existing Linux x86-64 and macOS arm64 layout admissions returns `lo=42, hi=9` at offsets `[8, 0]` on both, then the ordinary constant snapshot decoder refuses the applied `LayoutStorage` carrier.
- Cost: Shared Product preparation substitutes one value before selecting any of the five authored deployments. Keep the recipe until exact target closure, then join the authenticated layout and copy-eligible semantic snapshot through the existing target-scoped fold. Preserve the policy, field qualifications, and wrong-target and cast refusals.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles and tracing the kept named constant.

### Package review refuses the fixed-layout target-body boundary
- Category: knowledge crossing a boundary
- Where: [Delivery boundary](samples/cli/facts/fixed_layout_storage/target_body/main.omg#L28), [review capture](omega-rust/omega/packages/review-evidence/src/capture/policy.rs#L209)
- Saw: On clean `15145194c7d` on macOS arm64, `fixed_layout_storage/target_body` reaches package review projection and refuses with `package review cannot retain trait and boundary declaration contracts from the accepted declaration`. Its invalid twin reaches the authored `Delivered` cast under `--check`.
- Cost: Package review stops execution before the original provider markers and byte offsets can be observed. Retain the accepted `Delivery` contract in the existing review record and keep incomplete contracts refused.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles.

### A textual merge can pass Admission with a Rust type error
- Category: verifying costs more than writing
- Where: [pre-merge Admission](.github/workflows/admission.yml#L68), [post-merge checks](.github/workflows/post-merge-check.yml#L64), [atomic store reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/atomics.rs#L223)
- Saw: [PR #3326](https://github.com/CathedralOS/Omega/pull/3326) had green Admission, then squash commit `d5d5c987120` failed a clean macOS arm64 Rust build with E0533. The new evaluation statement shape and the existing atomic-store constructor were in different files. Git merged the text without a conflict. Admission runs rules and formatting on the merge, and the compiler checks start after publication.
- Cost: Unrelated compiler builds and two in-flight repairs stopped before tests could run. Compare the latency and detection coverage of checking the affected crates on a warmed merge tree with the current post-merge validation.
- Reported: 2026-11-15, Codex, while validating copyable record constants and the owned-element call path.

### Two returned-window probes stop at unbuilt language forms
- Category: tests that pin the bug
- Where: [case-selected typed slice](samples/cli/probes/case_selected_slice_reference/main.omg#L8), [selected byte window](samples/cli/probes/byte_window_dispatch/main.omg#L12), [mutable typed sequence refusal](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/arrays.rs#L62), [callable types](wiki/spec/language/machine_values.md#callable-types-and-construction)
- Saw: Both probes now use current forms. `select` and `select_rebound` are ordinary static machines, and the byte decoders are named machines passed and rebound as a `machine(packet: &[u8]) -> &[u8] requires packet.len >= 4 reaches none crashes none` value. On linux_x86_64 at `15145194c7d8`, `omega run --both` refuses the case-selected probe at `let cells: &mut [Cell] = &mut storage.cells[..];` with `mutable typed sequence requires its actual exclusive backing and loan` (TASKS.md line 19 counts this as the second most common sample refusal). The byte probe stops in the parser at the callable parameter type with expected `)`, found punctuation `(`. The parser accepts no callable type in a field, local or parameter, so runtime machine values are unbuilt (TASKS.md **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**; open PR #3352 works on stored machine selections).
- Cost: The two probes still stop before their returned-window checks. Once mutable typed slices are lent and callable types parse and run, run both unchanged with `omega run --both` and keep the independent projection-bound concern open.
- Reported: 2026-11-15, Codex, while reproducing retained result-window bounds; narrowed 2026-11-15, Claude Code, after migrating both probes off retired forms.

### Eight host access-view controls stop before any domain access view exists
- Category: zombie machinery
- Where: [placed view fixture](tests/fixtures/pass/layouts/placed_view_call_custody/main.omg#L28), [fixture admission](omega-rust/omega/compilation/compiler/tests/access_view_calls.rs#L22), [static call reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/calls.rs#L187), [access-plan evaluation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/domain_access_plans.rs#L175)
- Saw: The fixture uses the current `core::access` surface: `RegisterLayout satisfies FixedLayout<Registers>`, `domain Registers::Placed access RegisterAccess`, `access::new_plan` with `access::observe` for an External read of `status` and an External write of `transmit`, and views typed `RegisterStorage in Registers::Placed`, where `RegisterStorage` is the closed alias `LayoutStorage<Registers, RegisterLayout>`; `alloc::vec` writes `VecCellStorage in VecCell::Controlled` the same way. Typing and Psi 05 now read T's domain on its `LayoutStorage<T, L>` application, and Psi 05 judges a closed subject alias by its expansion. On linux_x86_64 all eight tests are now refused during the fixture's Check, in the body of `observe`, with "call static machine arguments": `read<Accessor<u32>, u32>(&view.status)` is an access-view field operation, and Psi 05 has no route for one. Psi 05 never projects `view.status` to the plan's `Accessor<u32>`, never turns `core::read`/`core::write` on it into the plan's accessor operation, and never writes `module.access_view_plans`.
- Cost: Three pieces remain, and each is needed before the tests reach their checks of exact shared and exclusive backing, alias rejection, field effects and reordered inputs. (1) `domain_access_plans::evaluate` still leaves a plan with field demands unjudged (`field_count` must be 0). It must instead decode the `observe` demands, join them to the selected `PlanLaidLayout` of the view's `LayoutStorage` application, and fill `typed.access_view_plans` (only a `type_identity.rs` unit fixture fills it today). The source frontier must then accept that table instead of refusing it as "source access-policy evaluation". (2) Psi 05 must read a field projection of a laid access view as that field's accessor, and `read`/`write` on it as the plan's operation. Today `callable_formations.rs` and `read/logical.rs` only hide or refuse parameters that have a plan. (3) Psi 05 must publish the joined `TerminalAccessViewPlan` that the tests' `accessor_writes` and the interpreter read. A predicate domain on laid storage stays refused ("a predicate domain on laid layout storage") until field predicates are read through the layout's geometry. Re-run `access_view_calls` unchanged.
- Reported: 2026-11-15, Codex, while restoring Psi source inspection; narrowed 2026-11-15, Claude Code, after migrating the fixture, after entryless Checks began laying out their own storage, and again after Psi 05 read laid-storage domains and closed aliases.

### A reference match loses the incoming loan at its result
- Category: knowledge crossing a boundary
- Where: [descriptor location transport](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptors.rs#L963), [incoming descriptor authentication](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L428)
- Saw: On macOS arm64 at `30ae6323d4b`, `let joined: &Ticket = match choice { true -> incoming, _ -> incoming };` followed by a call borrowing `joined` refuses before publication because the call's reference descriptor has no authenticated formation or incoming loan. The `&Ticket in Stamped` twin gets the same refusal. Each arm forms its own loan. Descriptor transport intersects the authorized locations of each formation across predecessors, leaving neither arm's loan available at the result.
- Cost: A machine cannot pass the match result to a borrowing call even when every arm borrows the same incoming record. Generalize the existing loan and arrival analysis to check each arm's immediate lender at the corresponding arrival. Keep the distinct-lifetime refusal when both arms borrow from another input.
- Reported: 2026-11-15, Codex, while checking domains through value matches.

### An assigned local worktree can lose its warm target between checks
- Category: verifying costs more than writing
- Where: [pool assignments](tools/xtask/src/worktree.rs#L166), [pool cache retention](tools/docs/testing.md#L589), [mbx 1.29.2 collection order](https://github.com/jdx/mr-boxington/blob/v1.29.2/docs/managed-targets.md#keep-or-evict-specific-checkouts)
- Saw: On macOS arm64 at `c55c52f59b1`, below the workstation's 45.0 GiB free-space floor, mbx 1.29.2 deleted assigned `worker-8`'s target between commands. The next test build compiled from cold in 3m 12s. In two isolated checkouts, `MBX_TARGET_EVICT_FIRST` changed one collector's choice under a one-target budget. An independent collector reverted to oldest-first. With a zero budget and simulated low disk, both a running `mbx run` lease and an explicit `MBX_TARGET_KEEP` kept the view. Once the command ended, the same scratch view could be collected unless explicitly kept.
- Cost: Worktree assignments outlast mbx command leases. A per-command eviction hint cannot protect the next check from an independent collector, and a hard keep can exceed the disk budget. Add an advisory priority for managed views to mbx and set it from the pool’s existing assignment state. Respect explicit operator rules and running-command locks, and still let assigned views be reclaimed under disk pressure.
- Reported: 2026-11-15, Codex, while validating const-index record arguments

### Instruction selection reconstructs target operands from earlier executable bodies
- Category: re-deriving
- Where: [ordinary native selection](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L110), [paired instruction projection](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs) (`instruction::project`, `successor`), [body-derived runtime indices](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs) (`runtime_indices::operands`, `runtime_indices::projection`)
- Saw: Ordinary Omega 03 construction still receives the optimized executable unit and pairs each target row with its optimized operation node. Calls (#3388), scalar reads and stores, view formation, references, and now structural leaf copies, structural field stores and case-membership tests take their operands, selector extents and proof keys from the retained target row and the admitted fact list; their reconstruction is left only in the independent audit. Write-only primitive stores, element-view reads and writes, storage addresses, byte-sequence and element-view length operations, accessor scalar access, static-interrupt installation, descriptor-table setup and structural edge bindings (`successor`) still rebuild runtime-index operands from optimized definitions and path geometry. Boundary and normalized foreign calls still read their abstract operation from the optimized node.
- Cost: Those operand families still depend on earlier executable bodies and their matching operation list, so changing one of them can require matching recognizers in several stages. Move each to its retained target row (most rows already carry `indices` with stride, extent and obligation), delete its body-reading branch, and keep the rebuild in the audit. Acceptance: `instruction::project` and `successor` take no optimized node or function, and `TargetSelectionInput` drops `source` and `unit` except for the independent validator. That validator, `verify_target_operation_plan`, has no caller outside the crate's own unit tests (`image-emission`'s `verify_retained_target_operations` is exported but never called), so no validator request or test harness runs it on compiled programs yet.
- Reported: 2026-11-15, Codex, while verifying representation handoffs after the IR cleanup; static source and caller-chain evidence only. Narrowed 2026-11-15, Claude Code, after moving leaf copies, field stores and case membership to retained target rows.

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [activation copy](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L565), [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L942), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L727)
- Saw: `complete_selected` starts from typing retained for later selections. Source activation still copies that typed program when the body batch is empty, because the following projection rewrites its declaration roots, side tables and selected source symbols. Projection narrows the resulting owned program in place.
- Cost: Even an empty body batch needs a copy of unrelated typed storage with the current projection owner. Moving the copy into projection leaves the cost. Per-table copy-on-write still copies body arenas that completion appends to, and `Arena` exposes contiguous spans. If repeated nonempty completion proves costly, use the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract) to share immutable declaration and type storage and publish body replacements through stable handles. Preserve exact source authority, refusal behavior, and signature and inferred-summary dependencies.
- Measured: The two Build routes from the [earlier measurement](wiki/drafts/measurements/selected_body_copies.md) on macOS arm64 at `d66164cd0f5` each selected 75 source files with zero bodies to finish. Activation requested 32,333,303 bytes across 14,553 allocations and cloned 101 arenas. Projection's typed extraction allocated zero bytes. Neither route measures repeated nonempty continuations.
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals; narrowed 2026-11-15, Claude Code, after removing the projection copy

### Some record-domain placements still have no checked representation
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/qualifications.rs` (`declare_structural`, `structural_establishments`); `lower/places.rs` (`value_memberships`); `terminal-psi/semantics/src/structural_qualifications.rs`; `06_terminal-psi-to-terminal-psi/src/lib.rs` (boundary calls with qualified structural formals)
- Saw: the lowered checker forms each declared record-domain predicate with the default facts of its own carrier. Field sites prove the clauses at their storage paths. A cast still refuses domains with predicates because it cannot prove their complete membership theory. A cast through a requirement route is refused because the lowered form carries a conformance edge only as the closed application of a selected call. Qualified boundary results are refused. A host value issued by the evaluator receives its formal's memberships only at the Build entry.
- Cost: programs using those placements are refused or require the Build entry to supply the membership.
- Reported: 2026-11-13, Claude Code, while stating domain predicates as contracts; narrowed 2026-11-15, Claude Code, after carrying record domains into the lowered checker

### Evaluation requests without a declaration owner can refuse the whole program
- Category: a verdict with nothing behind it
- Where: [source evaluation frontier](omega-rust/psi/compilation/build-time-evaluation/src/source_frontier.rs), [declaration refusals](omega-rust/psi/compilation/representations/typed-trees/src/deferred_refusals.rs)
- Saw: Known typing and construction gates can attach refusals to the declaration that owns them. The evaluation frontier still rejects program-level rows for explicit wire plans, access plans, and kept constant recipes before any entry asks for them. Ordinary numbered data derives schema metadata without a selected encoding, so checking it requests no wire evaluation. A wire plan row still needs the evaluation route.
- Cost: An evaluation request recorded outside its declaration has no demand owner. For a source producer that emits such rows, use the existing per-declaration request list so a selected reader can ask for the work and an orphan is still refused.
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing. Narrowed by Codex after checking numbered data and tracing the remaining program-level guards.

### The wire policy constant cannot call its selected reflection path
- Category: knowledge crossing a boundary
- Where: [wire protocol policy constant](samples/cli/systems/wire_protocol/main.omg#L389), [policy field inspection](samples/cli/systems/wire_protocol/main.omg#L405), [static argument binding](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L737)
- Saw: With ordinary numbered records admitted, `omega --check samples/cli/systems/wire_protocol/main.omg` reaches `POLICY_ACCEPTED`, where it refuses the call held by `selected_policy_is_valid` as “holding a call whose static arguments no pass specializes yet.” A one-field record has the same refusal with either inferred or explicitly named context type. Its exact `reflect::field` query and explicit `visit<u64>` call, written separately, run together in native and Psi. A direct, non-reflective `apply<picked>(7u64)` with a callable parameter is also refused at the static application. Specialization cannot read a machine operand, and the field visitor's producer was removed when the earlier checker was retired in #2823.
- Cost: The wire sample cannot check its codec policy against the numbered fields. Use the existing applied callable contract to check the selected visitor and its exact arguments before emitting ordinary per-member calls. The callback must satisfy the published binder, even when the application's concrete inputs would also satisfy a narrower callback.
- Reported: 2026-11-15, Codex, macOS arm64, with the precheck repair based on `8f2ba3e8ea6`.
