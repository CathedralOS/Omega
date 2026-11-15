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

### Build invocations lose closed data applications during argument construction
- Category: knowledge crossing a boundary
- Where: [host argument declaration lookup](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/invocation.rs#L45), [closed data declarations](omega-rust/psi/compilation/representations/typed-trees/src/type_identity.rs#L7731)
- Saw: Clean `a225808b7c9` on macOS arm64 passes 124 of 129 Build-time evaluation tests. The fixed and generic Pair policies, C scalar layout policy, unselected applied collection, and equal const-index storage all refuse “invocation value has no exact closed data application” while binding the layout schema, shelf, or storage value.
- Cost: Checked host argument preparation cannot use the closed record that its declaration selects. Find which occurrence loses the retained application and use the existing applied declaration owner while binding the value, keeping the new shared typed-view controls and exact missing or wrong application refusals.
- Reported: 2026-11-15, Codex, while validating copyable record constants against the repaired main tree.

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

### A named-map specialization loses its captured application at Psi publication
- Category: knowledge crossing a boundary
- Where: [named-map fixture](tests/fixtures/run/generics/named_map_own_telescope/main.omg#L4), [captured source application](omega-rust/psi/compilation/representations/typed-trees/src/access_views_and_commitments.rs#L1556), [artifact validation](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/calls.rs#L1858)
- Saw: On macOS arm64, `named_map_own_telescope` runs to exit 0 in native and Psi. Reloading its retained `program.oterm` refuses with `checked specialization has no retained application`. Its two selected implementations bind the same value type and different unused map arguments, which the already-accepted source and in-process execution distinguish.
- Cost: Standalone Psi consumers cannot reload either capture in the same checked module. Carry the exact retained map telescope and any method application through the existing capture/application records. Keep different captured arguments distinct and preserve the codec's missing and mismatched capture refusals.
- Reported: 2026-11-15, Codex, while restoring publication of ordinary generic specializations.

### A stored-length element sequence cannot report its length
- Category: knowledge crossing a boundary
- Where: [source measurement](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/bytes.rs#L533), [measurement emission](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs#L3020), [Psi element length operation](omega-rust/psi/compilation/representations/terminal-psi/src/control_flow.rs#L1445)
- Saw: On macOS arm64 at `146cd87aed0`, an ordinary `observe(tickets: [Ticket]) -> u64` returning `tickets.len` stopped at `collection measurement without its descriptor operation`. Its caller passed the exact single-element result of `make(number)` and expected a length of one. Changing only the owned declarations to `[Ticket; 1]` ran with both engines at exit 0. The stored-length maker and by-value consumer reach native target lowering when the consumer has no measurement.
- Cost: Source admits length from a fixed extent or a byte descriptor while the same stored-length element carrier already has a Psi observation operation. Use that operation for the exact located descriptor and preserve the existing per-place extent and index proof obligations.
- Reported: 2026-11-15, Codex, while tracing native owned sequence backing.

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

### Typed size and alignment queries are refused as unresolved calls
- Category: knowledge crossing a boundary
- Where: [layout query typing](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs#L1999), [deferred call formation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L711), [layout observation consumer](omega-rust/psi/compilation/terminal-installation/src/scalar_applications.rs#L2597), [kept C scalar record](samples/cli/facts/c_layout_scalar_record/local/main.omg#L31)
- Saw: On macOS arm64 at `a80de4a6944`, including #3422, the unchanged C scalar evaluator checks size 24, alignment 8, and offsets `[0, 8, 16]` for `linux_x86_64`. Both the direct Product check and `omega run --both` then refuse `size_of<CLayout<Record>>()` at line 31, column 159 as a call whose name resolves to no declaration. The CLI exits 200. Typing already identifies the layout query, while the checked artifact's layout observation consumer has no source producer for that request.
- Cost: Placement can compute the exact selected layout, but a program cannot observe its extent or alignment through these core queries. Join the retained typed request to the existing checked layout observation path. Preserve target scope, policy identity and the existing refusal for unavailable or invalid geometry.
- Reported: 2026-11-15, Codex, while restoring ranked Build-time invocation

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

### The scalar C-layout control is refused at judgment after its layout succeeds
- Category: a test that fails on `main`
- Where: [C-layout control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs) (`the_library_c_layout_policy_keeps_its_existing_scalar_record`)
- Saw: once build-time invocations form views for unviewed data applications, the library C policy lays out the scalar `Record` with the expected size 24, alignment 8 and offsets 0/8/16. `AcceptedTerminalSource::judge` then refuses the same program: "not supported yet: declaration holding a call whose name resolves to no declaration while reading the signature of `main`". Before that repair, the control stopped earlier, at "constant argument `schema`: invocation value has no exact closed data application".
- Cost: the only control for the library C layout policy cannot reach publication, so a regression in C-layout publication goes unseen. Find which call in `main`'s signature the lowered-form checker cannot resolve and support it there; do not change the control's program.
- Reported: 2026-11-15, Claude Code, linux_x86_64, while repairing default-provider layout demand
