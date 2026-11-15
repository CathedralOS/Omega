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

### A fixed-layout record constant loses its target selection
- Category: knowledge crossing a boundary
- Where: [constant initializer](samples/cli/facts/fixed_layout_storage/const_name/main.omg#L28), [constant evaluation request](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/constant_jobs.rs#L662), [interpreter layout admission](omega-rust/psi/interpreter/src/embedding.rs#L2398)
- Saw: On clean `15145194c7d` on macOS arm64, the kept `fixed_layout_storage/const_name` sample fails package review at `SAVED` with `source invocation demands: TargetLayoutSelectionMissing(LayoutApplicationId(1))`. The `call_result` placement of the same selected record layout exits 0 in native and Psi. The invalid constant independently reaches the exact `Delivered` cast refusal.
- Cost: The constant never reaches checking or execution of its original value and layout assertions. Carry the selected layout through the existing constant invocation when that evaluation requires it. Preserve exact target custody and the invalid cast.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles.

### Fixed-layout record fields stop affine checking before the cast verdict
- Category: knowledge crossing a boundary
- Where: [field sample](samples/cli/facts/fixed_layout_storage/field/main.omg#L28), [invalid field sample](samples/cli/facts/fixed_layout_storage/invalid_field/main.omg#L24), [canonical affine region](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/plain_affine.rs#L2456)
- Saw: On clean `15145194c7d` on macOS arm64, `fixed_layout_storage/field` fails package review with `affine projection outside the static structural vocabulary`. `invalid_field` gives the same unjudged refusal during `--check`, before the checker can decide the `Delivered` cast. Both store the selected `LayoutStorage<Pair, PairLayout>` value in `Holder.value`.
- Cost: The field placement cannot check either the authored byte offsets or the invalid 41 cast. Make the existing affine projection check recognize the field's exact selected storage path. Once the negative reaches the cast, verify and refresh its `expected.txt` and `sample.json`, which still use the earlier range wording.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles.

### A failed constant cast loses its original location
- Category: knowledge crossing a boundary
- Where: [failing cast in make](samples/cli/facts/fixed_layout_storage/invalid_const_name/main.omg#L24), [constant error wrapper](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/constant_jobs.rs#L699)
- Saw: On clean `15145194c7d` on macOS arm64, `--check` reports the `Delivered` cast in `make` for `invalid_const_name` but prints `source-unit 241, bytes 1336..1341`, the span of the `SAVED` identifier at 28:7. The `invalid_call_result` control reports the same failure at its authored cast at 24:25.
- Cost: Reading the diagnostic takes the user to the constant name without locating the failing operation. Keep the checked error's original location alongside the constant declaration context.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles.

### Package review refuses the fixed-layout target-body boundary
- Category: knowledge crossing a boundary
- Where: [Delivery boundary](samples/cli/facts/fixed_layout_storage/target_body/main.omg#L28), [review capture](omega-rust/omega/packages/review-evidence/src/capture/policy.rs#L209)
- Saw: On clean `15145194c7d` on macOS arm64, `fixed_layout_storage/target_body` reaches package review projection and refuses with `package review cannot retain trait and boundary declaration contracts from the accepted declaration`. Its invalid twin reaches the authored `Delivered` cast under `--check`.
- Cost: Package review stops execution before the original provider markers and byte offsets can be observed. Retain the accepted `Delivery` contract in the existing review record and keep incomplete contracts refused.
- Reported: 2026-11-15, Codex, while replaying the fixed-layout cast oracles.

### A fence loses the ordering known from a local initializer
- Category: branch per shape
- Where: [fence argument reading](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/atomics.rs#L196), [ordering rules](wiki/spec/language/concurrency.md#orderings)
- Saw: On macOS arm64, the bundled-fence repair runs a direct `FenceOrdering::Receive` in both engines. Binding that same case to an immutable local immediately before the call makes checked compilation refuse at `main.omg:6:5` with “an atomic fence needs a known case of FenceOrdering.” The reader looks for the variant symbol on the argument expression itself.
- Cost: A refactor from an explicit case to a local loses a statically determined order. Use existing proof facts to select the exact normalized fence event when they establish one case. Keep a runtime choice and an unrelated declaration from supplying ordering or proof authority.
- Reported: 2026-11-15, Codex, while restoring the bundled atomic fence.

### A descriptor length premise cannot cross the loop join
- Category: knowledge crossing a boundary
- Where: [descriptor ranking](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/rankings.rs#L720), [arrival substitution](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/joins.rs#L675)
- Saw: On macOS arm64 with stored-length reads on `4d76bebd83e`, a loop using `terminates by (tickets.len - position) -> Nat::Descending` and `requires position <= tickets.len` reaches “join predicate form outside the scalar subset.” Replacing the owned record sequence with an actual borrowed byte descriptor and keeping the same rank and requirement reaches the same refusal. The original non-loop stored length machine executes in Psi.
- Cost: The rank observes the current descriptor, but the arrival substitution only maps scalar values. Rebind the extent to the actual descriptor and path coming into each join, preserving the existing refusal for a rebound or unrelated descriptor.
- Reported: 2026-11-15, Codex, while carrying stored record sequence length through checking and execution.

### Stage06 reference controls disagree with their existing verdicts
- Category: tests that pin the bug
- Where: [byte cohort controls](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/byte_view_elements.rs#L493), [boundary result views](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/returned_views.rs#L62), [owned record with a reference](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/stored_references.rs#L631)
- Saw: On clean macOS arm64 `4d76bebd83e` and `781f2f5cb50`, the same 14 Stage06 tests fail. Two byte-view cohort positives reject, ten returned-view checks diverge from their verdicts, and two owned record transfers refuse incomplete reference disposition. The unknown-origin returned-view control is accepted where it expects one conflicting loan.
- Cost: The suite no longer establishes when a returned descriptor keeps or retires its exact loan. Reproduce the accepted and rejected cases through their actual authored source and distinguish stale artifact fixtures from changes to the loan and disposition checks before changing their oracles.
- Reported: 2026-11-15, Codex, while comparing the stored-length repair with an unchanged Stage06 test baseline.

### Build-time returned views keep their caller loans past last use
- Category: knowledge crossing a boundary
- Where: [shared returned-view control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/borrowed_views.rs#L576), [exclusive returned-view control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/borrowed_views.rs#L631)
- Saw: On clean `781f2f5cb50` on macOS arm64, the shared positive refuses its write after `see(view)` as a conflicting loan. The exclusive positive refuses its array read after `fill(view)` as observation outside a live exclusive reference. The controls call `run()` with no host inputs and both passed on clean `a225808b7c9`.
- Cost: Build-time source judgment cannot reuse the backing after these returned references' last use. Keep the exact call result origin and end its loan at the existing caller frontier. The twins that use the view after accessing the backing must still refuse.
- Reported: 2026-11-15, Codex, while verifying closed Build-time arguments against current main.

### A constant snapshot loses a checked element qualification
- Category: knowledge crossing a boundary
- Where: [constant reconstruction](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/constant_jobs.rs#L674), [Product judgment](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs#L277), [collection membership check](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural_qualifications.rs#L589)
- Saw: On macOS arm64 at clean `637ee209d8a`, the checked `make()` result has the `Stamped` qualification on every element and returns `Ticket.number` values 3 and 5. The constant receipt validates, then Product refuses the reconstructed `issued_tickets` array at `typing array literal element`. An unrestricted `Stamped` tag produces the same refusal. Both direct calls and both plain record-array constants run in native and Psi.
- Cost: A named constant loses the checked facts carried by its initializer's result. Rejoin the existing accepted result to checked constant construction so portable validation can verify the exact value and nested qualifications. Preserve the issuer checks and refuse runtime authority at the constant boundary.
- Reported: 2026-11-15, Codex, while carrying evaluated record-array constants to named use. Reproduced and traced again at `637ee209d8a`.

### A textual merge can pass Admission with a Rust type error
- Category: verifying costs more than writing
- Where: [pre-merge Admission](.github/workflows/admission.yml#L68), [post-merge checks](.github/workflows/post-merge-check.yml#L64), [atomic store reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/atomics.rs#L223)
- Saw: [PR #3326](https://github.com/CathedralOS/Omega/pull/3326) had green Admission, then squash commit `d5d5c987120` failed a clean macOS arm64 Rust build with E0533. The new evaluation statement shape and the existing atomic-store constructor were in different files. Git merged the text without a conflict. Admission runs rules and formatting on the merge, and the compiler checks start after publication.
- Cost: Unrelated compiler builds and two in-flight repairs stopped before tests could run. Compare the latency and detection coverage of checking the affected crates on a warmed merge tree with the current post-merge validation.
- Reported: 2026-11-15, Codex, while validating copyable record constants and the owned-element call path.

### An owned record-sequence literal stops at native target lowering
- Category: knowledge crossing a boundary
- Where: [native structural construction](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow_storage.rs#L3397), [Psi immutable backing](omega-rust/psi/interpreter/src/storage_records_and_arrays.rs)
- Saw: On macOS arm64, native target lowering refuses `let tickets: [Ticket] = [Ticket { number: 11u32 }];` with `UnsupportedControlFlow` at `control_flow_storage.rs:3397`. Returning that sequence from a machine and transferring it to a by-value formal reaches the same native refusal. The local literal executes in Psi. The exact local type twin `[Ticket; 1]` exits 0 in both engines.
- Cost: Native target lowering receives the checked element roster and places only inline fixed arrays for this operation. Settle physical backing for the stored-length descriptor so the checked element roster keeps its ownership when it leaves its creating machine.
- Reported: 2026-11-15, Codex, while checking structural domains in arrays.

### Two returned-window probes stop at unbuilt language forms
- Category: tests that pin the bug
- Where: [case-selected typed slice](samples/cli/probes/case_selected_slice_reference/main.omg#L8), [selected byte window](samples/cli/probes/byte_window_dispatch/main.omg#L12), [mutable typed sequence refusal](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/arrays.rs#L62), [callable types](wiki/spec/language/machine_values.md#callable-types-and-construction)
- Saw: Both probes now use current forms. `select` and `select_rebound` are ordinary static machines, and the byte decoders are named machines passed and rebound as a `machine(packet: &[u8]) -> &[u8] requires packet.len >= 4 reaches none crashes none` value. On linux_x86_64 at `15145194c7d8`, `omega run --both` refuses the case-selected probe at `let cells: &mut [Cell] = &mut storage.cells[..];` with `mutable typed sequence requires its actual exclusive backing and loan` (TASKS.md line 19 counts this as the second most common sample refusal). The byte probe stops in the parser at the callable parameter type with expected `)`, found punctuation `(`. The parser accepts no callable type in a field, local or parameter, so runtime machine values are unbuilt (TASKS.md **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**; open PR #3352 works on stored machine selections).
- Cost: The two probes still stop before their returned-window checks. Once mutable typed slices are lent and callable types parse and run, run both unchanged with `omega run --both` and keep the independent projection-bound concern open.
- Reported: 2026-11-15, Codex, while reproducing retained result-window bounds; narrowed 2026-11-15, Claude Code, after migrating both probes off retired forms.

### Eight host access-view controls stop before any domain access view exists
- Category: zombie machinery
- Where: [placed view fixture](tests/fixtures/pass/layouts/placed_view_call_custody/main.omg#L28), [fixture admission](omega-rust/omega/compilation/compiler/tests/access_view_calls.rs#L22), [layout roots](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs#L287)
- Saw: The fixture now uses the current `core::access` surface: `RegisterLayout satisfies FixedLayout<Registers>`, `domain Registers::Placed access RegisterAccess`, `access::new_plan` with `access::observe` for an External read of `status` and an External write of `transmit`, and views typed `RegisterStorage in Registers::Placed`. On linux_x86_64 at `c75dc2fc5`, all eight tests are refused during the fixture's entryless package Check, before inspection, at three successive points. (1) Layout policies run only for machines reachable from Build root bindings, but the Check judges every machine the package declares, so `observe`'s signature hits `LayoutPolicyStorage`. Adding the subject's machines to each target's layout roots clears this, but one test went from about 2 s to 337 s, so that change was not landed. (2) A bodyless `read(&view.status)` with inferred type arguments is refused because there is no specialization for bodyless static calls, so the fixture now passes `read<Accessor<u32>, u32>` explicitly. (3) `RegisterStorage in Registers::Placed` is refused as a domain application without its prepared theory (`read/types.rs`). `alloc::vec` uses the same three constructs.
- Cost: Even past (3), nothing in production fills `typed.access_view_plans` from a domain's `AccessPolicy` (only a `type_identity.rs` unit fixture does), and the source frontier refuses a non-empty table. So the tests still cannot reach their checks of exact shared and exclusive backing, alias rejection, field effects and reordered inputs. Build that route together with the issuer-set entry below (evaluate the applied policy once, join it to the selected layout, publish the view plan), make subject machines layout roots without the slowdown, then re-run `access_view_calls` unchanged.
- Reported: 2026-11-15, Codex, while restoring Psi source inspection; narrowed 2026-11-15, Claude Code, after migrating the fixture.

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

### Psi cannot run until native lowering succeeds
- Category: knowledge crossing a boundary
- Where: [runtime admission](omega-rust/omega/compilation/native-compiler/src/realization.rs#L152), [borrowed-storage layout extraction](omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L30)
- Saw: At `2669589de8a`, both `omega run --both` and `omega run --engine psi` refuse the unchanged console program with the same native target-lowering error. Neither executes guest code. Runtime admission calls `lower_borrowed_storage_layouts`, which lowers the whole guest program before taking its borrowed-storage descriptions. These runs do not measure that step's time.
- Cost: A backend gap prevents Psi from running checked code. Have the layout stage supply the exact borrowed-storage descriptions directly. Preserve native binding, layout, authority and provider checks, and keep full native lowering for image production.
- Reported: 2026-11-15, Codex, while checking the console's interpreter and native paths

### Test edits change the production compiler identity
- Category: verifying costs more than writing
- Where: [compiler build-input collection](omega-rust/omega/compilation/compiler/build.rs#L32), [recursive input scan](omega-rust/omega/compilation/compiler/build.rs#L98)
- Saw: The build hashes every `.rs`, `.toml` and `.omg` under `omega-rust`, including integration tests. Changing only a control in `compiler/tests/executed_fixture_discovery.rs` rebuilt `compiler`, `package-manager` and `orchestration` in 18.21 seconds before the filtered test ran. Shipped code had not changed.
- Cost: Test edits also change the compiler-version key for accepted packages, forcing compilation and review as the test tree grows. Derive the key from production inputs and configuration. Still invalidate uncommitted implementation changes; ignoring required source or using only Git HEAD would miss changes.
- Reported: 2026-11-14, Codex, while checking actual project publication of native calling metadata

### The service-contract reader hides why it refused a declaration
- Category: verifying costs more than writing
- Where: [full service projection](omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs#L555), [boundary publication](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/declare.rs#L623)
- Saw: At `9101613dfcd`, both `native_boundary_supply` and `default_console_provider_reached` stop during source discovery with `boundary declaration has no complete service contract`. The reader returns `None` for a wrong source scope, an unavailable receiver application, or a method it cannot read. This repair fixes the Build-scope lookup; the API still drops the cause and declaring requirement.
- Cost: Each later failure looks like an unsupported language construct, even when the compiler used the wrong context. Return the reason and declaring requirement from the shared reader so callers can explain the refusal.
- Reported: 2026-11-14, Codex, while carrying Build native bindings through console execution

### Instruction selection reconstructs target operands from earlier executable bodies
- Category: re-deriving
- Where: [ordinary native selection](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L110), [call operand construction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs#L171), [runtime-index reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs#L273), [descriptor-origin reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_scalar_graph.rs#L2219)
- Saw: At `1897804e0dbd`, ordinary Omega 03 construction receives target operations with abstract operations and the optimized executable unit. It pairs optimized blocks and operation nodes with target rows (`legalization_source.rs:3101`) and rebuilds structural-call indices from optimized definitions, path geometry and accepted obligation facts. Target arguments already keep index operands and strides but omit the extent and proof facts this rebuild supplies, so the target handoff is incomplete. Call results that carry descriptors can rebuild an optimized catalog graph and selected boundary operations to find origins. These are production paths, not only explicit independent validators or shared immutable type-catalog queries; this audit measured no miscompile or runtime cost.
- Cost: Target operands still depend on earlier executable bodies and their matching operation list, so adding a value form or changing target operations can require matching recognizers in several stages. Make the producer write the needed runtime-index evidence and descriptor-origin records into the target representation, sharing immutable catalogs rather than copying them; have ordinary selection read that product and keep earlier bodies for independent correspondence validation. Acceptance: selection constructs the same instructions from target operations and their explicit catalogs/evidence without reading earlier executable bodies, while the independent validator still rejects changed operands or evidence.
- Reported: 2026-11-15, Codex, while verifying representation handoffs after the IR cleanup; static source and caller-chain evidence only

### A returned reference keeps its whole call argument borrowed
- Category: a verdict with nothing behind it
- Where: [call lifetimes](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs) (`reference_call_lifetimes`, `retain_call_lifetime`), [retained call loans](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/reference_results.rs), [loan formation](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/loans.rs) (`lent`)
- Saw: `bind_reference_call_results` narrows each returned reference to the argument storage its published origin names, such as `grid.cells` for `pick(&mut grid, i)` returning `&mut grid.cells[i].x`. `reference_call_lifetimes` then retains the argument's own `Loan` row, which captures all of `grid`, for the result's whole life. Stage 05 forms Loan rows only for actual call arguments, so no row exists for the narrower path.
- Cost: while a returned reference is live, the caller cannot write any disjoint field of the argument, such as `grid.tag`, even when the reference's origin is exact. A repair forms a narrower retained Loan row at the call for the bound source path and ends the argument's own loan at the return. It must carry that row through Terminal Psi validation, the Psi interpreter's loan frontier and native lowering.
- Reported: 2026-11-15, Claude Code, after returned runtime-element references started publishing their exact element path. Replaces the stripped-window entry, whose publication half is fixed.

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [activation copy](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L566), [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L940), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L727)
- Saw: `SourceContinuation::complete_selected` starts from `input.typing.clone()` while its `Arc<SourceCompletionInput>` is kept for later selections, so `activate_body_completion`'s `Arc::make_mut` deep-copies every `TypedTrees` table once per selected-source continuation, even when one small body set changes. The second copy, in source projection, is gone: `project_source_declarations` now narrows the owned program in place. Naive per-table copy-on-write still copies the expression and statement arenas every completion appends to, and `Arena` exposes contiguous spans (`span`, `storage_slice`, `storage_view`), so paged sharing changes its contract.
- Cost: A small continuation still copies unrelated typed storage once. First check whether an empty selected activation and the read-only consumer of the kept predecessor need full owned payloads, keeping source authority, context and refusal behavior. If measured nonempty-continuation costs justify separate body output, split immutable declaration/type storage from owned body tables, and publish selected replacements through stable handles without flattening unrelated bodies. A shared-snapshot continuation must retain untouched storage, exact authority/context identities and refusal behavior. Measure copied bytes and peak memory; keep signature and inferred-summary dependencies explicit. Postpone wide Psi 02–04 representation splits while `LOWERED-FORM-CHECKER` work keeps changing those stages, unless a measured continuation bottleneck forces it sooner. See the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Measured: [two unchanged Build routes at historical `62c864e`](wiki/drafts/measurements/selected_body_copies.md) each copied typed storage seven times (about 392.5 MB requested allocations across measured windows), including an empty selected activation. Both selected batches were empty; authored batches had 1,748/1,746 machines. These measurements predate the removal of the projection copy; current copy counts were not remeasured. They show nothing about repeated nonempty continuations and do not justify a wide representation split. The ownership repair is still open.
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

### The scalar C layout program loses field identity at checked publication
- Category: knowledge crossing a boundary
- Where: [C scalar source](samples/cli/facts/c_layout_scalar_record/local/main.omg#L18), [field-symbol check](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/places.rs#L795), [C control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs) (`the_library_c_layout_policy_keeps_its_existing_scalar_record`)
- Saw: On macOS arm64 at `fd26884075d` over `781f2f5cb50`, the real C policy computes size 24, alignment 8 and offsets `[0, 8, 16]`. After #3465, Product judgment refuses `main` with “spelling member without a field symbol.” A typed-source probe of the same program found the three fields unresolved on the owned `record`, its local copy and the mapped reference.
- Cost: The exact C-layout control reaches its geometry assertions and still stops before publication. Carry the selected record's authored field symbols through the existing projection path for the `CLayout<Record>` alias and its reference. Keep the test source and byte oracle.
- Reported: 2026-11-15, Claude Code, while repairing default-provider layout demand. Narrowed 2026-11-15, Codex, after selected layout observations landed.
