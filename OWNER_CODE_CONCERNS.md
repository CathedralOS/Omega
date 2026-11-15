# Code Repair Queue

Agents repair obvious code-quality and architecture defects without waiting for
owner sign-off. This queue records concrete structural problems outside the
current repair so another agent can take them through validation and landing.
An entry is neither a new language rule nor a reason to stop the current work.

Use [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md) only for genuinely unsettled language
or high-level architecture choices. Apply the existing
[architecture rules](AGENTS.md#conventions-and-workflow) directly; cross-cutting
scope, conflicting edits and engineering difficulty do not require a decision.
Plain missing functionality belongs in the existing outcome in [TASKS.md](TASKS.md),
not here. A fail-closed "not implemented yet" is not an unsound acceptance.

## How to add one

Add an entry when something would mislead the next agent, or when fixing it
properly reaches beyond the crate you are in. An ordinary bug, a style
preference, or anything you can repair inside your own task does not belong
here: just fix it.

The best moment is your first hour in an area, before you have a change of your
own to defend. Name the defective mechanism and its cost, not a speculative
framework or a list of features that remain unimplemented.

- Add the entry at the top of the list under "Entries"; the list is flat, newest first.
- Name one category from the list below on its `Category` line.
- Keep it to the lines of the template. Link the file and line.
- Add it in the pull request you are already landing; never open one for it.
- Reconcile overlapping reports by the remaining defect; do not preserve duplicates.
- Any repairing agent removes the entry when the fix lands. Remove stale reports
  after verifying that the offending code or premise is gone; do not keep a
  completion log or mistake deleting a report for implementing its feature.

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
2. **Knowledge crossing a boundary.** A stage imports something that is not its
   input representation, its output representation or a foundation crate.
3. **Branch per shape.** Logic keyed on where a value sits (local, field,
   parameter, match arm) instead of what it is.
4. **Re-deriving.** A later stage recomputes what an earlier one knew, or a
   stage checks its own output again.
5. **Too big to read.** You had to search the file because you could not read
   it.

**Honesty**

6. **A verdict with nothing behind it.** Something is accepted by default, by
   fallback, or because a lookup came back empty.
7. **Tests that pin the bug.** A fixture that would still pass if the feature
   were broken, or a check that quietly stopped running.
8. **Zombie machinery.** Code only tests call, a retired path still being fed,
   a fallback nobody can remove.

**Direction**

9. **The documents disagree.** You can cite a rule for either answer, or a
   document blesses the thing that is rotting.
10. **Verifying costs more than writing.** You waited longer than you worked.

## Entries

### The record-domain catalog test has no declared predicate subject
- Category: tests that pin the bug
- Where: [catalog fixture](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read.rs#L638), [subject requirement](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/predicates.rs#L408)
- Saw: On clean `cdd086553fa` on macOS arm64, `a_record_domain_catalog_keeps_the_declared_predicate_and_its_presence` fails with `spelling domain predicate without a subject symbol`. Its hand-built false and true predicate declarations leave the subject at its invalid default, so lowering never reaches the catalog assertions.
- Cost: The control cannot distinguish false, true and absent record-domain predicates. Supply the same declaration-owned subject that real source retains, and preserve all three catalog assertions.
- Reported: 2026-11-15, Codex, while verifying named-map invocation contracts.

### A descriptor length premise cannot cross the loop join
- Category: knowledge crossing a boundary
- Where: [descriptor ranking](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/rankings.rs#L720), [arrival substitution](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/joins.rs#L675)
- Saw: On macOS arm64 with stored-length reads on `4d76bebd83e`, a loop using `terminates by (tickets.len - position) -> Nat::Descending` and `requires position <= tickets.len` reaches “join predicate form outside the scalar subset.” Replacing the owned record sequence with an actual borrowed byte descriptor and keeping the same rank and requirement reaches the same refusal. The original non-loop stored length machine executes in Psi.
- Cost: The rank observes the current descriptor, but the arrival substitution only maps scalar values. Rebind the extent to the actual descriptor and path coming into each join, preserving the existing refusal for a rebound or unrelated descriptor.
- Reported: 2026-11-15, Codex, while carrying stored record sequence length through checking and execution.

### Typed whole-array slice probes stop before their length checks
- Category: tests that pin the bug
- Where: [kept typed slice probe](samples/cli/facts/as_slice_views/local/main.omg#L8), [invalid twin](samples/cli/facts/as_slice_views/invalid_local/main.omg#L8), [borrow expression reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/expressions.rs)
- Saw: On macOS arm64 at `0eefa10be23` and after the stored-length read repair on `4d76bebd83e`, both kept local probes stop at `let view: &[Entry] = &entries[..];` with “typing borrow without a reference destination.” The authored assertion and the twin's weaker range never reach judgment.
- Cost: The kept tests cannot distinguish a correct whole-array extent from the wrong one. Join this exact authored fixed-array range to the existing typed-view formation and preserve the shared backing and the exact length premise.
- Reported: 2026-11-15, Codex, while checking element length through the same operation as byte length.

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

### Fixed-layout negative samples still expect retired range wording
- Category: tests that pin the bug
- Where: [local oracle](samples/cli/facts/fixed_layout_storage/invalid_local/expected.txt#L1), [field oracle](samples/cli/facts/fixed_layout_storage/invalid_field/expected.txt#L1), [literal diagnostic check](tools/xtask/src/samples.rs#L510)
- Saw: On macOS arm64 with the closed-application repair at `ca02a870855`, both unchanged negative programs exit 200 at `41u64 as +Delivered` on line 24, column 25. The checker cites `u64::Delivered` and `self >= 42 && self <= 42`, while both expectation files still require the literal `expected 42..=42`.
- Cost: The sample gate calls the correct cast refusal a mismatch. Replay the other pinned placements and update the oracles to identify the refused cast, exact domain and unchanged bound.
- Reported: 2026-11-15, Codex, while validating closed Build-time layout arguments.

### The bundled atomic fence still uses a retired attached declaration
- Category: zombie machinery
- Where: [bundled atomic ordering facade](source/library/core/atomic.omg#L12), [kept Boolean store](samples/cli/facts/atomic_boolean_store/local/main.omg#L12), [attached-machine admission](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/machine.rs#L891)
- Saw: On macOS arm64 with the atomic-store build repair at `0eefa10be23`, the unchanged Boolean-store sample fails at `core/atomic.omg:12:16` because `Atomic::fence` is attached to a data declaration. The existing invalid twin still rejects `Receive` on the store itself. The unchanged projected store/load probe from the atomic-wake test runs in both engines when exposed through its hosted entry.
- Cost: A program importing the bundled fence cannot reach the compiler's exact atomic-fence primitive. Restore the existing public ordering behavior through a supported declaration owner and retain the rejection for a lookalike or illegal ordering. Re-run the kept Boolean store/twin placements.
- Reported: 2026-11-15, Codex, while running the atomic-store compiler-build repair.

### A constant snapshot loses a checked element qualification
- Category: knowledge crossing a boundary
- Where: [observed result records](omega-rust/psi/compilation/build-time-evaluation/src/terminal_values.rs#L356), [constant array recovery](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/constant_jobs.rs#L338), [checked element construction](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/arrays.rs#L324)
- Saw: On macOS arm64 with the copyable record constant repair on `4a720b7a9b5`, the accepted invocation behind `const issued_tickets: [Ticket in Stamped; 2] = make();` returns both field values and passes the constant receipt check. Product then refuses the reconstructed element at “typing array literal element.” Calling the original `make()` directly preserves the qualification through both engines. Ordinary computed and literal `[Ticket; 2]` constants also run in both engines.
- Cost: The snapshot holds a record's field bits and declared type while the invocation checked qualification on each element path. Separate copy-eligible refinement facts from authority that a constant may not grant. Use the invocation's existing checked result to authenticate any recoverable refinement and refuse authority-bearing snapshots at their constant boundary.
- Reported: 2026-11-15, Codex, while carrying evaluated record-array constants to named use.

### A textual merge can pass Admission with a Rust type error
- Category: verifying costs more than writing
- Where: [pre-merge Admission](.github/workflows/admission.yml#L68), [post-merge checks](.github/workflows/post-merge-check.yml#L64), [atomic store reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/atomics.rs#L223)
- Saw: [PR #3326](https://github.com/CathedralOS/Omega/pull/3326) had green Admission, then squash commit `d5d5c987120` failed a clean macOS arm64 Rust build with E0533. The new evaluation statement shape and the existing atomic-store constructor were in different files. Git merged the text without a conflict. Admission runs rules and formatting on the merge, and the compiler checks start after publication.
- Cost: Unrelated compiler builds and two in-flight repairs stopped before tests could run. Compare the latency and detection coverage of checking the affected crates on a warmed merge tree with the current post-merge validation.
- Reported: 2026-11-15, Codex, while validating copyable record constants and the owned-element call path.

### A copied-owner test refuses before checking either package's types
- Category: tests that pin the bug
- Where: [copied-owner control](omega-rust/omega/compilation/compiler/tests/anonymous_module_owner.rs#L422), [fixture entry preparation](omega-rust/omega/compilation/compiler/tests/support/fixture_package_inputs.rs#L190), [bundled Console realization](source/library/std/console.omg#L105)
- Saw: On clean `ce501a3161d4` on macOS arm64, `equal_shaped_types_from_a_changed_anonymous_owner_cannot_substitute` fails at its fixture's separate Linux ProgramEntry preparation. Checking the whole std supplier rejects `ConsoleNativeProvider::read_line` with `a data declaration cannot own a machine`. Even if that check succeeds, the helper always returns an error about an unpublished service schema. The separate ordinary and named artifact reload test passes.
- Cost: The imported-package acceptance and copied-owner rejection are never exercised. Use the existing Build and package admission route for the actual entry, with both captured sources and the original nominal-type controls intact.
- Reported: 2026-11-15, Codex, while verifying named-map artifact reload.

### Native lowering loses a stored byte view's length
- Category: knowledge crossing a boundary
- Where: [native byte-length operation](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs#L2671), [projected descriptor geometry](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_scalar_graph.rs#L3257)
- Saw: On macOS arm64 with the byte-domain predicate repair on `6730ae09229`, a machine stores `"no plan"` in `report.reason: &[u8]` and checks that `report.reason.len` equals 7. Psi exits 0. Native refuses machine 1, operation 5 with `SourceCustodyMismatch` when the byte-length operation asks for its geometry at `legalization_source.rs:2687`. Qualifying the same field with the UTF-8 domain yields the same Psi and native outcomes. Measuring the literal through a direct local byte view exits 0 in both engines.
- Cost: A byte view can be stored in a record and read by Psi, yet that field's length cannot reach native execution. Rejoin the exact projected descriptor source and its access to the existing byte-length geometry query, keeping the ordinary local and invalid backing controls.
- Reported: 2026-11-15, Codex, while checking stored byte domains in both engines.

### An owned record-sequence literal stops at native target lowering
- Category: knowledge crossing a boundary
- Where: [native structural construction](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow_storage.rs#L3397), [Psi immutable backing](omega-rust/psi/interpreter/src/storage_records_and_arrays.rs)
- Saw: On macOS arm64, native target lowering refuses `let tickets: [Ticket] = [Ticket { number: 11u32 }];` with `UnsupportedControlFlow` at `control_flow_storage.rs:3397`. Returning that sequence from a machine and transferring it to a by-value formal reaches the same native refusal. The local literal executes in Psi. The exact local type twin `[Ticket; 1]` exits 0 in both engines.
- Cost: Native target lowering receives the checked element roster and places only inline fixed arrays for this operation. Settle physical backing for the stored-length descriptor so the checked element roster keeps its ownership when it leaves its creating machine.
- Reported: 2026-11-15, Codex, while checking structural domains in arrays.

### Two returned-window probes still use retired callable forms
- Category: tests that pin the bug
- Where: [case-selected typed slice](samples/cli/probes/case_selected_slice_reference/main.omg#L7), [selected byte window](samples/cli/probes/byte_window_dispatch/main.omg#L21), [attached-machine refusal](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/machine.rs#L891), [dynamic type refusal](omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/type_syntax_and_patterns.rs#L623)
- Saw: On clean `30ae6323d4b` on macOS arm64, preparing the unchanged case-selected probe from `main` rejects `Storage::select` with `a data declaration cannot own a machine`. The unchanged byte-window probe refuses `dyn Payload` on line 21 with `dyn types are retired`. The kept `run/views/byte_subslice` is accepted through the same Stage 06 source path. Existing selected and rigid `Handoff` controls still retain their real declared source loan and reject a live overlapping write.
- Cost: The two probes stop before their intended returned-window checks. Migrate the callable forms while preserving the typed slice's selected and rebound cells and both byte decoders' written ranges, values, and exit oracles. Then check both engines and keep the independent projection-bound concern open.
- Reported: 2026-11-15, Codex, while reproducing retained result-window bounds

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
- Saw: On macOS arm64 at `c55c52f59b1`, mbx 1.29.2 collected assigned `worker-8` between commands below the workstation's 45.0 GiB free-space floor. The recovered test build compiled from cold in 3m 12s. In two isolated checkouts, `MBX_TARGET_EVICT_FIRST` changed one collector's choice under a one-target budget. An independent collector reverted to oldest-first. Under a zero budget and simulated low disk, a real `mbx run` lease and an explicit `MBX_TARGET_KEEP` each kept the view. After the command ended, that same scratch view was collectible without an explicit keep.
- Cost: Worktree assignments outlast mbx command leases. A per-command eviction hint cannot protect the next check from an independent collector, and a hard keep can exceed the disk budget. Expose advisory managed-view priority in mbx and feed it from the pool’s existing assignment state. Honor explicit operator rules and running-command locks, and keep assigned views eligible for reclamation under pressure.
- Reported: 2026-11-15, Codex, while validating const-index record arguments

### Instruction selection reconstructs target operands from earlier executable bodies
- Category: re-deriving
- Where: [ordinary native selection](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L110), [call operand construction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs#L171), [runtime-index reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs#L273), [descriptor-origin reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_scalar_graph.rs#L2219)
- Saw: At `1897804e0dbd`, ordinary Omega 03 construction receives target operations together with abstract operations and the optimized executable unit. It zips optimized blocks and operation nodes with target rows (`legalization_source.rs:3101`) and reconstructs structural-call indices from optimized definitions, path geometry and accepted obligation facts. Target arguments already retain index operands and strides, but omit the extent and proof facts this reconstruction supplies: the target handoff is incomplete. Descriptor-bearing call results can reconstruct an optimized catalog graph and selected boundary operations to derive origins. These are production construction paths, not only explicit independent validators or shared immutable type-catalog queries; no miscompile or runtime cost was measured in this audit.
- Cost: Target operands remain dependent on earlier executable histories and their matching operation roster, so adding a value form or changing target operations can require synchronized recognizers across stages. Fix the producer to complete the needed runtime-index evidence and descriptor-origin records in the target representation, retaining shared immutable catalogs rather than copying them; have ordinary selection consume that product and keep earlier bodies available to independent correspondence validation. Acceptance: selection constructs the same instructions from target operations and their explicit catalogs/evidence without reading earlier executable bodies, while the independent validator still rejects changed operands or evidence.
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
- Saw: `SourceContinuation::complete_selected` starts from `input.typing.clone()` while its `Arc<SourceCompletionInput>` stays retained for later selections, so `activate_body_completion`'s `Arc::make_mut` deep-copies every `TypedTrees` table once per selected-source continuation, even when one small body set changes. The second copy, in source projection, is gone: `project_source_declarations` now narrows the owned program in place. Naive per-table copy-on-write still copies the expression and statement arenas every completion appends to, and `Arena` exposes contiguous spans (`span`, `storage_slice`, `storage_view`), so paged sharing changes its contract.
- Cost: A small continuation still copies unrelated typed storage once. First inspect whether empty selected activation and the read-only retained-predecessor consumer need full owned payloads, preserving source authority, context and refusal behavior. If measured nonempty-continuation costs warrant independent body output, separate immutable declaration/type storage from owned body tables; publish selected replacements through stable handles without flattening unrelated bodies. A shared-snapshot continuation must retain untouched storage, exact authority/context identities and refusal behavior. Measure copied bytes and peak memory; keep signature and inferred-summary dependencies explicit. Defer wide Psi 02–04 representation splits while `LOWERED-FORM-CHECKER` coverage is actively churning those stages unless a measured continuation bottleneck requires it earlier. See the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Measured: [two unchanged Build routes at historical `62c864e`](wiki/drafts/measurements/selected_body_copies.md) each copied typed storage seven times (about 392.5 MB requested allocations across measured windows), including an empty selected activation. Both selected batches were empty; authored batches had 1,748/1,746 machines. These measurements predate the removal of the projection copy; current copy counts were not remeasured. They do not establish repeated nonempty-continuation scaling or justify a wide representation split. The ownership repair remains open.
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
- Saw: Known typing and construction gates can retain refusals with the declaration that owns them. The evaluation frontier still rejects program-level rows for explicit wire plans, access plans, and retained constant recipes before it has an entry demand. Ordinary numbered data derives schema metadata with no selected encoding, so checking it does not request wire evaluation. A wire plan row still requires the evaluation route.
- Cost: An evaluation request recorded outside its declaration has no demand owner. For a source producer that emits such rows, use the existing declaration-owned request inventory so a selected reader can demand the work and an orphan still refuses.
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing. Narrowed by Codex after checking numbered data and tracing the remaining program-level guards.

### The wire policy constant cannot call its selected reflection path
- Category: knowledge crossing a boundary
- Where: [wire protocol policy constant](samples/cli/systems/wire_protocol/main.omg#L389), [policy field inspection](samples/cli/systems/wire_protocol/main.omg#L405), [static argument binding](omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/passes/specialization.rs#L737)
- Saw: With ordinary numbered records admitted, `omega --check samples/cli/systems/wire_protocol/main.omg` reaches `POLICY_ACCEPTED`, where it refuses the call held by `selected_policy_is_valid` as “holding a call whose static arguments no pass specializes yet.” A one-field record has the same refusal with either inferred or explicitly named context type. Its exact `reflect::field` query and explicit `visit<u64>` call separately execute together in native and Psi. A direct, non-reflective `apply<picked>(7u64)` with a callable parameter also refuses at the static application. Specialization has no reading of a machine operand, and the field visitor's producer was removed when the earlier checker was retired in #2823.
- Cost: The wire sample cannot check its codec policy against the numbered fields. Use the existing applied callable contract to check the selected visitor and its exact arguments before emitting ordinary per-member calls. The callback must satisfy the published binder, including when the application's concrete inputs would happen to satisfy a narrower callback.
- Reported: 2026-11-15, Codex, macOS arm64, with the precheck repair based on `8f2ba3e8ea6`.

### The scalar C layout program loses field identity at checked publication
- Category: knowledge crossing a boundary
- Where: [C scalar source](samples/cli/facts/c_layout_scalar_record/local/main.omg#L18), [field-symbol check](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/places.rs#L795), [C control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs) (`the_library_c_layout_policy_keeps_its_existing_scalar_record`)
- Saw: On macOS arm64 at `fd26884075d` over `781f2f5cb50`, the real C policy computes size 24, alignment 8 and offsets `[0, 8, 16]`. After #3465, Product judgment refuses `main` with “spelling member without a field symbol.” A typed-source probe of the same program found the three fields unresolved on the owned `record`, its local copy and the mapped reference.
- Cost: The exact C-layout control reaches its geometry assertions and still stops before publication. Carry the selected record's authored field symbols through the existing projection path for the `CLayout<Record>` alias and its reference. Keep the test source and byte oracle.
- Reported: 2026-11-15, Claude Code, while repairing default-provider layout demand. Narrowed 2026-11-15, Codex, after selected layout observations landed.
