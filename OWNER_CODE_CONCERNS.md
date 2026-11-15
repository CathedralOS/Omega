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

### Checked ranked control loops are refused at Build-time installation
- Category: a verdict with nothing behind it
- Where: [local termination summary](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/proof_scope.rs#L227), [invocation admission](omega-rust/psi/compilation/terminal-installation/src/evaluation_and_synchronous_calls.rs#L979), [ranked parity witness](tests/fixtures/run/views/ranked_slice_iteration/main.omg#L10)
- Saw: On macOS arm64 at `028760775d4`, the kept `parity` machine is accepted with its original checked rank. Preparing `parity(3, false)` succeeds, then invocation refuses `has no ordinary checked Terminates guarantee`. The acyclic control with the same inputs returns true. Replacing `remaining - 1` with `remaining` still refuses during source checking because removing strict edges leaves a cycle. The scalar C plan reaches the same installation refusal on `c_abi_fields` once diagnosis gets past its source and invocation-provenance blockers.
- Cost: A checked local cycle loses its ordinary termination guarantee when a Build-time consumer runs it. Carry the accepted local ranking evidence to the existing admission check and preserve checks for exact recursive-call edges and nondecreasing ranks.
- Reported: 2026-11-15, Codex, while replaying the generic C layout plan

### An assigned local worktree can lose its warm target between checks
- Category: verifying costs more than writing
- Where: [pool assignments](tools/xtask/src/worktree.rs#L166), [pool cache retention](tools/docs/testing.md#L589), [mbx 1.29.2 collection order](https://github.com/jdx/mr-boxington/blob/v1.29.2/docs/managed-targets.md#keep-or-evict-specific-checkouts)
- Saw: On macOS arm64 at `c55c52f59b1`, mbx 1.29.2 collected assigned `worker-8` between commands below the workstation's 45.0 GiB free-space floor. The recovered test build compiled from cold in 3m 12s. In two isolated checkouts, `MBX_TARGET_EVICT_FIRST` changed one collector's choice under a one-target budget. An independent collector reverted to oldest-first. Under a zero budget and simulated low disk, a real `mbx run` lease and an explicit `MBX_TARGET_KEEP` each kept the view. After the command ended, that same scratch view was collectible without an explicit keep.
- Cost: Worktree assignments outlast mbx command leases. A per-command eviction hint cannot protect the next check from an independent collector, and a hard keep can exceed the disk budget. Expose advisory managed-view priority in mbx and feed it from the pool’s existing assignment state. Honor explicit operator rules and running-command locks, and keep assigned views eligible for reclamation under pressure.
- Reported: 2026-11-15, Codex, while validating const-index record arguments

### The C layout field walk omits ranks on five recursive helpers
- Category: a verdict with nothing behind it
- Where: [C ABI field walk](source/library/core/c_layout.omg#L36), [scalar C-layout control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L1202)
- Saw: On macOS arm64 at `01ee33b7779`, the existing scalar `Record` on `linux_x86_64` fails with `contract termination promise over a call cycle`. `c_abi_fields` decreases `remaining_steps` before calling `place`. The cycle passes through `represented`, `checked`, `extent`, and `append` before returning to `c_abi_fields`. Those five helpers omit that measure. Adding it exposes the value match in `represented`, where the compiler rejects the recursive call as nonterminal. With the ranks and an equivalent terminal test of the two accepted tags, the guard repair leaves the padding value-match guarantee as the only reported proof failure.
- Cost: The compiler refuses the unchanged layout before checking size 24, alignment 8, and offsets `[0, 8, 16]`. The scalar and nested-representation samples use this field walk at all nine placements. Preserve the same rank, field order, bounds, and invalid-layout controls through the existing terminal proof mechanisms.
- Reported: 2026-11-15, Codex, after the Build-time reflection and field lifetime repairs

### The padding value match loses its strict result bound
- Category: knowledge crossing a boundary
- Where: [padding contract](source/library/core/c_layout.omg#L25), [inferred join predicates](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/joins.rs#L226)
- Saw: At `01ee33b7779`, the scalar padding body by itself computes `r = offset % alignment` and returns zero if `r == 0`, or `alignment - r` otherwise. With `1 <= alignment <= 16`, Stage 06 refuses its published `result < alignment` after the value match. Returning the remainder directly proves `result < alignment`, and returning the subtraction directly with `0 < r < alignment` proves the same bound. A terminal branch that selects subtraction for `r > 0` and zero otherwise also proves the original guarantee.
- Cost: A checked arithmetic bound is lost when the padding branches produce a joined value. Keep the result relation through the existing join and arithmetic proofs.
- Reported: 2026-11-15, Codex, while restoring the C ABI field walk

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

### Selected and rigid requirement results keep separate declared-lifetime accounts
- Category: re-deriving
- Where: [requirement header publication](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/reference_results.rs) (`publish_requirements`), [rigid call shape](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references/requirement_results.rs), [selected call shape](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references/selected_results.rs), [projection transport](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage.rs) (`forget_address`, `without_runtime_coordinates`)
- Saw: a rigid call reads the bodiless requirement's header column, which 05 derives from `declared_result_lifetimes`; a selected call reads the closed-conformance method's `result_lifetimes`, checked against the original definition. Both come from the same signature, but two shapes carry them. Result publication turns any inexact, non-indirect, unwindowed projection into `ReferenceOriginExtent::WithinRegion` over its retained prefix. That prefix always contains the actual backing, so the claim is sound, but a forgotten public region and a window stripped of runtime coordinates publish the same region, and the narrower window is lost.
- Cost: callers lend a whole declared region where a window would do, and a change to one lifetime account must be mirrored in the other. Let one declaration-contract owner hold the public lifetime account for both call routes, and carry the kind of bound in the projection address so publication can keep the narrower one.
- Reported: 2026-11-15, Claude Code, after letting rigid requirement calls return their declared input region; refines Codex's #3400 note

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L935), [activation copy](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L566), [retained-body completion](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L847), [shared typing loop](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1596), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L624)
- Saw: `SeededTypingBase` holds `Arc<TypedTrees>`, but body completion calls `Arc::unwrap_or_clone(retained.typed)` and feeds the candidate to one mutable `Lowerer`. `SourceContinuation::complete_selected` starts from `input.typing.clone()` while its `Arc<SourceCompletionInput>` stays retained for later selections, so `activate_body_completion`'s `Arc::make_mut` deep-copies every `TypedTrees` table once per selected-source continuation, even when one small body set changes; later completions in that loop own the Arc uniquely. Sharing the outer Arc does not make the stored tables independently owned. No local repair fits: per-table copy-on-write still copies the expression and statement arenas every completion appends to, and `Arena` exposes contiguous spans (`span`, `storage_slice`, `storage_view`), so paged sharing changes its contract (verified 2026-11-15 at `171113b06`).
- Cost: A small continuation can copy unrelated typed storage and cannot return independent body output. Separate immutable declaration/type storage from owned body tables; publish selected replacements through stable handles without flattening unrelated bodies. A shared-snapshot continuation must retain untouched storage, exact authority/context identities and refusal behavior. Measure copied bytes and peak memory; keep signature and inferred-summary dependencies explicit. Defer wide Psi 02–04 representation splits while `LOWERED-FORM-CHECKER` coverage is actively churning those stages unless a measured continuation bottleneck requires it earlier. See the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals

### Forming a field's domain predicate loses the field carrier's default requirements
- Category: a proof loses its subject's premises
- Where: `05_typed-trees-to-terminal-psi/src/read/places.rs` (field-domain clause instantiation) and `06_terminal-psi-to-terminal-psi/src/formation.rs` (structural type default formation)
- Saw: `data Reading requires number >= 1 { number: u32; }`, `domain Reading::Nonzero requires self.number - 1 <= 4294967295u32;`, and `data Carry { item: Reading in Nonzero; }` produce `data domain: contract arithmetic is not proved total before its condition: the operands' bounds exceed the carrier`. The clause under `Carry.item.number` is checked without the lower bound guaranteed by `Reading` at that same field. The separate declaration check can use `Reading`'s bound. A control with the lower bound in the named domain passes.
- Cost: a field's valid domain predicate needs to repeat a bound its declared carrier already guarantees.
- Reported: 2026-11-14, Codex, macOS arm64. Reproduced with the issuer-guard-only candidate `8c04fa2` over `294241784f4` and with the record-domain catalog repair.

### Some record-domain placements still have no checked representation
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/qualifications.rs` (`declare_structural`, `structural_establishments`); `lower/places.rs` (`projected_memberships`); `terminal-psi/semantics/src/structural_qualifications.rs`; `06_terminal-psi-to-terminal-psi/src/lib.rs` (boundary calls with qualified structural formals)
- Saw: the lowered checker forms each declared record-domain predicate with the default facts of its own carrier. Field sites prove the clauses at their storage paths. A cast still refuses domains with predicates because it cannot prove their complete membership theory. A domain governed by an access policy has an empty issuer set because nothing evaluates its plan. A cast through a requirement route is refused because the lowered form carries a conformance edge only as the closed application of a selected call. A persistent reference to a value in a domain names its referent's storage type, which carries field domains without the referent's own domain. Domains on array elements and qualified boundary results are refused. A host value issued by the evaluator receives its formal's memberships only at the Build entry.
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

### Layout evaluation forms every target's library layouts instead of the subject's closure
- Category: a verdict with nothing behind it
- Where: [reusable layout scope](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs#L272) (no root bindings); [layout evaluation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/fixed_layouts.rs#L64), which then selects all declarations; reached at `source/library/std/targets/macos_arm64/filesystem_abi.omg` (`CLayout<DarwinTimespec>`)
- Saw: at `c06ddb0f570`, the entryless `fractional_integer_landing` fixture stops during preparation at `DarwinTimespec` on `linux_arm64`, where `c_abi_fields::append` requires unsupported clause arithmetic. The [explicitly demanded scalar layout](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L1203) at `59b8d53b729` reaches the same arithmetic refusal on `linux_x86_64` before checking size 24, alignment 8 and offsets `[0, 8, 16]`. (The earlier alias refusal at the same field, "selected type alias has no retained source declaration", was a separate defect in extension resolution and is fixed.)
- Cost: a program that never demands the macOS layout is refused for it, against [demand-driven layout formation](wiki/spec/build/configuration.md#multi-target-compilation); compare "Evaluation requests without a declaration owner can refuse the whole program"
- Reported: 2026-11-15, Claude Code, while removing the product gate's duplicate review pass
