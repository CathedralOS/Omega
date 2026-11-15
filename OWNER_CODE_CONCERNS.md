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

### Typed size and alignment queries are refused as unresolved calls
- Category: knowledge crossing a boundary
- Where: [layout query typing](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/expression_lowering.rs#L1999), [deferred call formation](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L711), [layout observation consumer](omega-rust/psi/compilation/terminal-installation/src/scalar_applications.rs#L2597), [kept C scalar record](samples/cli/facts/c_layout_scalar_record/local/main.omg#L31)
- Saw: On macOS arm64 at `a80de4a6944`, including #3422, the unchanged C scalar evaluator checks size 24, alignment 8, and offsets `[0, 8, 16]` for `linux_x86_64`. Both the direct Product check and `omega run --both` then refuse `size_of<CLayout<Record>>()` at line 31, column 159 as a call whose name resolves to no declaration. The CLI exits 200. Typing already identifies the layout query, while the checked artifact's layout observation consumer has no source producer for that request.
- Cost: Placement can compute the exact selected layout, but a program cannot observe its extent or alignment through these core queries. Join the retained typed request to the existing checked layout observation path. Preserve target scope, policy identity and the existing refusal for unavailable or invalid geometry.
- Reported: 2026-11-15, Codex, while restoring ranked Build-time invocation

### A reference match loses the incoming loan at its result
- Category: knowledge crossing a boundary
- Where: [incoming descriptor authentication](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L429)
- Saw: On macOS arm64 with the `2654b67f946` CLI, `let joined: &Ticket = match choice { true -> incoming, _ -> incoming };` followed by a call borrowing `joined` refuses because it cannot authenticate the incoming loan. Binding `incoming` to a local reference before the same match instead runs to exit 0 in native and fails in Psi with `AffineFrontierMismatch` at machine 4, block 4. The otherwise equivalent `&Ticket in Stamped` controls reproduce both outcomes with the qualified match change at `1264434c319`.
- Cost: Branches that deliver a shared reference to the same backing cannot carry its checked loan into a caller. Preserve the incoming formation and live permission through the existing arrival and interpreter custody rules.
- Reported: 2026-11-15, Codex, while checking domains through value matches.

### A selected domain access plan never supplies its issuer set
- Category: a verdict with nothing behind it
- Where: [source demand](omega-rust/psi/compilation/build-time-evaluation/src/source_frontier.rs#L143), [cast ownership](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/casts.rs#L529)
- Saw: `domain Ticket::Stamped access StampAccess;` selects an ordinary `AccessPolicy` implementation whose `plan(schema: TypeSchema)` returns `access::new_plan(&schema, Issuance::Only { routes: [reflect::callable(stamp)] })`. Casting `ticket as +Stamped` inside `stamp` is refused before the plan is requested. A twin replacing only the listed callable with `unrelated`, an empty `Only` list, and an explicit `Unrestricted` choice get the same refusal. The existing direct-route positive executes in native and Psi. Its wrong-issuer twin is refused for the actual issuer mismatch.
- Cost: only the policy application is retained. The source frontier has no executable demand for it. Psi 05 treats the selection as unresolved and publishes an empty record issuer set. The legacy Rust access plan models layout, field modes, and reach. The source plan contains a semantic schema, an explicit issuer choice, and independent reader and writer lists. Carrying only a decoded issuer list beside that plan would split the policy's authority. Request the exact applied policy through the existing checked selected-call evaluator once for the demanded domain. Authenticate the whole semantic plan, including schema, regions and callable declaration/application/revision, under the existing access-plan owner before joining it to layout. Supply the canonical issuer judgment and read/write checks from that single result. Reject unresolved plans, preserve explicit empty versus unrestricted issuance, and keep the Build seal's supplied root separate from source issuance.
- Reported: 2026-11-15, Codex, on clean `2ea0edea00f` macOS arm64 while evaluating a record-domain access plan. The bundled `IssuedBy<reflect::callable(stamp)>` variant is refused earlier at the domain signature because the application expression is unevaluated.

### Native lowering refuses to return an incoming record reference
- Category: knowledge crossing a boundary
- Where: [target return lowering](omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/control_flow.rs#L1793)
- Saw: A machine with `machine retained<'a>(ticket: &'a Ticket) -> &'a Ticket { ticket }` passes source checking, then the product refuses with `UnsupportedControlFlow` when its caller uses the result. The same return of a qualified record reference reaches exactly the same target refusal after persistent domain membership is retained. Reproduced on macOS arm64 with the numbered-record CLI `434bc0964dbd948b` and with the qualification candidate `c474455614035e3a`. Both variants reach the structural-home return check, which compares the home type, multiplicity, ABI shape and remaining local cleanup.
- Cost: A caller cannot use a reference returned from an ordinary checked call even when it directly forwards an incoming borrow. Carry that exact accepted origin through the existing target return and ABI correspondence, with the original lifetime and permission.
- Reported: 2026-11-15, Codex, while checking persistent qualified references.

### An assigned local worktree can lose its warm target between checks
- Category: verifying costs more than writing
- Where: [pool assignments](tools/xtask/src/worktree.rs#L166), [pool cache retention](tools/docs/testing.md#L589), [mbx 1.29.2 collection order](https://github.com/jdx/mr-boxington/blob/v1.29.2/docs/managed-targets.md#keep-or-evict-specific-checkouts)
- Saw: On macOS arm64 at `c55c52f59b1`, mbx 1.29.2 collected assigned `worker-8` between commands below the workstation's 45.0 GiB free-space floor. The recovered test build compiled from cold in 3m 12s. In two isolated checkouts, `MBX_TARGET_EVICT_FIRST` changed one collector's choice under a one-target budget. An independent collector reverted to oldest-first. Under a zero budget and simulated low disk, a real `mbx run` lease and an explicit `MBX_TARGET_KEEP` each kept the view. After the command ended, that same scratch view was collectible without an explicit keep.
- Cost: Worktree assignments outlast mbx command leases. A per-command eviction hint cannot protect the next check from an independent collector, and a hard keep can exceed the disk budget. Expose advisory managed-view priority in mbx and feed it from the pool’s existing assignment state. Honor explicit operator rules and running-command locks, and keep assigned views eligible for reclamation under pressure.
- Reported: 2026-11-15, Codex, while validating const-index record arguments

### The shared typed-view Build-time control stops before its mutable twin
- Category: knowledge crossing a boundary
- Where: [shared typed-view control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/sequence_values.rs#L216), [generated caller](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/invocation.rs), [original borrow lookup](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L198)
- Saw: At `6c485f370e4`, returning the original shared typed view stopped at `call the view's original incoming borrow cannot be identified`. At `ffb2464f7b6` on macOS arm64, #3374's stored-lifetime rule also exposed six fixtures that relied on implicit field lifetimes. Naming the data lifetimes and retaining the exact field owner restored the five owned-sequence, byte-record, and numbered-case controls. The original shared nonempty and empty inputs now type with a distinct outer loan and element byte lifetime. Their generated invocation fails at `source invocation lifetime arguments are not materialized`, before the returned-backing consumer and exclusive twin.
- Cost: Both entry points need the original checked lifetime binding. The caller currently constructs shared typed argument literals locally, and the admitted host input path accepts only the issued layout context. Use ordinary call lifetime instantiation and the existing checked structural input and descriptor tables to retain the host view's actual immutable backing, including nested byte fields and an empty sequence. The existing borrow lookup must identify that host formal through `echo` and `consume`, and the same admission must still refuse when the host supplies immutable bytes for the exclusive twin.
- Reported: 2026-11-15, Claude Code, while repairing the Build-time sequence controls. Updated by Codex after the stored-lifetime migration.

### Instruction selection reconstructs target operands from earlier executable bodies
- Category: re-deriving
- Where: [ordinary native selection](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L110), [call operand construction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs#L171), [runtime-index reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs#L273), [descriptor-origin reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_scalar_graph.rs#L2219)
- Saw: At `1897804e0dbd`, ordinary Omega 03 construction receives target operations together with abstract operations and the optimized executable unit. It zips optimized blocks and operation nodes with target rows (`legalization_source.rs:3101`) and reconstructs structural-call indices from optimized definitions, path geometry and accepted obligation facts. Target arguments already retain index operands and strides, but omit the extent and proof facts this reconstruction supplies: the target handoff is incomplete. Descriptor-bearing call results can reconstruct an optimized catalog graph and selected boundary operations to derive origins. These are production construction paths, not only explicit independent validators or shared immutable type-catalog queries; no miscompile or runtime cost was measured in this audit.
- Cost: Target operands remain dependent on earlier executable histories and their matching operation roster, so adding a value form or changing target operations can require synchronized recognizers across stages. Fix the producer to complete the needed runtime-index evidence and descriptor-origin records in the target representation, retaining shared immutable catalogs rather than copying them; have ordinary selection consume that product and keep earlier bodies available to independent correspondence validation. Acceptance: selection constructs the same instructions from target operations and their explicit catalogs/evidence without reading earlier executable bodies, while the independent validator still rejects changed operands or evidence.
- Reported: 2026-11-15, Codex, while verifying representation handoffs after the IR cleanup; static source and caller-chain evidence only

### A stripped runtime window publishes its whole containing region
- Category: re-deriving
- Where: [projection transport](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage.rs) (`forget_address`, `without_runtime_coordinates`), [returned origins](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs) (`reference_result_sources`)
- Saw: result publication turns any inexact, non-indirect, unwindowed projection into `ReferenceOriginExtent::WithinRegion` over its retained prefix. That prefix always contains the actual backing, so the claim is sound, but a forgotten public region and a window stripped of runtime coordinates compare equal as projection values and publish the same region, so the narrower window is lost.
- Cost: callers lend a whole declared region where a window would do. Carry the kind of bound in the projection address so publication can keep the narrower one; the declared lifetime account both call routes read is already one (`MachineContract::result_lifetimes`).
- Reported: 2026-11-15, Claude Code, after letting rigid requirement calls return their declared input region; refines Codex's #3400 note. Narrowed the same day after selected and rigid calls moved to one declared lifetime account.

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
- Saw: the lowered checker forms each declared record-domain predicate with the default facts of its own carrier. Field sites prove the clauses at their storage paths. A cast still refuses domains with predicates because it cannot prove their complete membership theory. A cast through a requirement route is refused because the lowered form carries a conformance edge only as the closed application of a selected call. Domains on array elements and qualified boundary results are refused. A host value issued by the evaluator receives its formal's memberships only at the Build entry.
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

### Layout demand treats default-provider inventory as selected computation
- Category: a fact with no owner
- Where: [layout scope choices](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs), [later default selection](omega-rust/omega/compilation/compiler/src/checked/target_preparation.rs), [layout invocation entrance](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/fixed_layouts.rs)
- Saw: `layout_target_scopes` joins `build.default_provider_selections` with authored supplies and roots each provider's layout closure before `judge_supplied_product` chooses defaults from actual requirement demand. That later loop rejudges the same frozen source without extending its layout preparation; the evaluator rejects preexisting plans without authenticating their producing invocations. These are observed orchestration facts at `7588e7200c1ef`; no unused-default runtime witness has been established.
- Cost: an imported default can request geometry before a target chooses it. Carry newly chosen providers' exact application/target demands and producing-invocation evidence through the source owner instead of eagerly forming the candidate inventory or dropping the existing-plan guard. Preserve declared Build-row demands outside the emission set, real layout-query/constant consumers without an entry, and ordinary checks on unused supplied code, as required by [layout demand and validation](wiki/spec/layouts/plans.md#layout-demand-and-validation).
- Reported: 2026-11-14, Codex, while repairing entryless blanket layout formation
