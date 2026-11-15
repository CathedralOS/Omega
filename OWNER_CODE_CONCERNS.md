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

### Native application assembly refuses settled provider plans
- Category: a fact with no owner
- Where: [native Product assembly](omega-rust/omega/compilation/compiler/src/terminal/native_product.rs#L56), [boundary settlement](omega-rust/omega/compilation/representations/abstract-operations/verified-optimization-input/src/settlement.rs#L13)
- Saw: On macOS arm64, a Build that selects a named provider for `Service::go` passes source checking. Both the referenced `provider_go` realization and an inline realization with the same contract stop before execution with `executable providers need their canonical row realizations and package permissions`.
- Cost: Native assembly refuses a nonempty settled plan before supplying the existing boundary settlement with canonical row execution provenance and admitted package permissions. Join those existing inputs for the exact selected plan and application before trying to execute it.
- Reported: 2026-11-14, Codex, while comparing the referenced and inline provider controls

### A rigid requirement call cannot return a reference
- Category: a verdict with nothing behind it
- Where: [05 reference result publication](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/reference_results.rs) (`publish_results`), [returned origins](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs) (`reference_result_sources`), [Stage 06 selected result control](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/references.rs) (`a_selected_reference_result_needs_its_public_requirement_contract`)
- Saw: with `trait Handoff { machine hand(source: &mut i64) -> &mut i64 crashes none; }`, the product closure of `relay<Chosen satisfies Handoff>(source: &mut i64) { let child: &mut i64 = Chosen::hand(source); child = 41; }` is accepted and runs, but the generic check form of `relay` falls back with `call reference result has no complete actual return-source correspondence`: the rigid call's callee is the bodiless requirement, whose result origins nothing reconstructs. If `relay` instead returns `Chosen::hand(source)`, the product refuses with `call reference result needs an exact retained input projection`, because the selected call's result is known only within its public region and a published result source admits only exact projections.
- Cost: a generic machine cannot call a reference-returning requirement, and no machine can return a selected reference result. lifetimes.md, "Returned views", keeps every leaf of the selected lifetime as a possible source, so the requirement's declared lifetime groups (`terminal_semantics::selected_reference_result`) must stand for the bodiless callee's origins, and a within-region origin must publish as `ReferenceOriginExtent::WithinRegion` rather than be refused.
- Reported: 2026-11-15, Claude Code (worker-1), after the requirement schema kept its elided signature lifetimes

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L935), [activation copy](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L566), [retained-body completion](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L847), [shared typing loop](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1596), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L624)
- Saw: `SeededTypingBase` holds `Arc<TypedTrees>`, but body completion calls `Arc::unwrap_or_clone(retained.typed)` and feeds the candidate to one mutable `Lowerer`. `SourceContinuation::complete_selected` starts from `input.typing.clone()` while its `Arc<SourceCompletionInput>` stays retained for later selections, so `activate_body_completion`'s `Arc::make_mut` deep-copies every `TypedTrees` table once per selected-source continuation, even when one small body set changes; later completions in that loop own the Arc uniquely. Sharing the outer Arc does not make the stored tables independently owned. No local repair fits: per-table copy-on-write still copies the expression and statement arenas every completion appends to, and `Arena` exposes contiguous spans (`span`, `storage_slice`, `storage_view`), so paged sharing changes its contract (verified 2026-11-15 at `171113b06`).
- Cost: A small continuation can copy unrelated typed storage and cannot return independent body output. Separate immutable declaration/type storage from owned body tables; publish selected replacements through stable handles without flattening unrelated bodies. A shared-snapshot continuation must retain untouched storage, exact authority/context identities and refusal behavior. Measure copied bytes and peak memory; keep signature and inferred-summary dependencies explicit. Defer wide Psi 02–04 representation splits while `LOWERED-FORM-CHECKER` coverage is actively churning those stages unless a measured continuation bottleneck requires it earlier. See the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals

### Some record domains still reach the lowered checker only as a refusal or a placeholder
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/qualifications.rs` (`declare_structural`, `structural_establishments`); `lower/places.rs` (`projected_memberships`); `terminal-psi/semantics/src/structural_qualifications.rs`; `06_terminal-psi-to-terminal-psi/src/lib.rs` (boundary calls with qualified structural formals)
- Saw: record domains now travel through Psi 05 as structural domain declarations, whole-value and projected (field) qualifications and explicit cast rows, and one shared judgment in 06 and artifact ingress holds every transfer to them. What remains carried by something else, or refused: a record domain with predicates is declared with a `Truth` placeholder predicate and an incomplete theory, its clauses riding only on the owning type's `requires`; a domain governed by an access policy is declared with an empty issuer set because nothing evaluates its plan; a cast through a requirement route is refused by name because the lowered form carries a conformance edge only as the closed application of a selected call; a persistent reference to a value in a domain is refused by name (a reference type names its referent's storage type, which carries field domains but not the referent's own); domains on array elements and qualified boundary results are refused; a host value the evaluator issues is presented with its formal's memberships only at the Build entry
- Cost: programs using those forms stay refused or depend on the reader for the part the placeholder stands for; Psi execution of qualified records is unverified while every `omega run` on this host stops at the macOS layout alias below
- Reported: 2026-11-13, Claude Code, while stating domain predicates as contracts; narrowed 2026-11-15, Claude Code, after carrying record domains into the lowered checker

### A whole-program stage refuses every program for a library construct nobody calls
- Category: a verdict with nothing behind it
- Where: `03_symbol-resolved-trees-to-typed-trees` (the remaining `unavailable_judgment` callers: the declaration-lowering queries in `typing_queries.rs`, and "closed const argument replay" in `type_reference.rs`); `build-time-evaluation/src/source_frontier.rs` (`source_jobs`: the side tables it still refuses whole, such as constant recipes and wire plans); any by-name refusal added to `04_typed-trees-to-typed-trees`
- Saw: these stages see the program together with all of `core` and `std` and have no notion of what an entry demands, so one such construct in an uncalled library declaration stops a program that never reaches it; the gates the route probe met (unbound calls, assignment literals, constructor field types, layout-policy and computed types) and the construction-destination pass's call gates (a mathematical call taking a built argument, a callee whose parameters typing does not select), a membership fact whose domain selection stays late-bound (found through the owners of the value it constrains), and a const argument typing does not admit now record the refusal on the declaration instead (`typed-trees/src/deferred_refusals.rs`), and the others still refuse the program. A refusal inside one declaration's lowering (`type_resolved_cohort`) returns midway through it, so those gates need a clean per-declaration rollback before they can move
- Cost: each remaining gate must be served for the entire library before any program compiles, or be moved onto the same carrier one by one as a program meets it
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing

### The parser still accepts a boundary requirement with no `crashes` clause
- Category: a verdict with nothing behind it
- Where: `01_tokens-to-syntax-trees/src/declarations.rs` (refuses a boundary requirement without `reaches`, says nothing about `crashes`); `05_typed-trees-to-terminal-psi/src/read/calls.rs` (`boundary`); `samples/` and `tests/fixtures/`
- Saw: [effects](wiki/spec/language/effects.md#open-callable-contracts) requires a written crash clause. The lowered-form checker only enforces it at a call. At `eea1528bbeb`, the lexer and parser found 1,734 omissions across 1,053 sample files, 69 across 39 test-fixture files, and one of 401 bundled-library requirements. The counts include boundary traits parsed in isolation when another part of the file failed. Twelve sample boundary trait declarations still failed to parse in isolation and are outside those counts. The bundled omission is [the interrupt-table validator](OWNER_QUESTIONS.md#1-may-the-interrupt-table-validator-trap-before-publication). Migrate the missing clauses to their intended ceilings before making the parser reject them
- Cost: a library or program author learns that the clause is missing only when some caller is refused. Thirty other bundled requirements with no known provider or caller already carry `crashes Trap crashes Abort`
- Reported: 2026-11-13, Claude Code, while writing the bundled library's crash ceilings

### Layout evaluation forms every target's library layouts instead of the subject's closure
- Category: a verdict with nothing behind it
- Where: [reusable layout scope](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs#L272) (no root bindings); [layout evaluation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/fixed_layouts.rs#L64), which then selects all declarations; reached at `source/library/std/targets/macos_arm64/filesystem_abi.omg` (`CLayout<DarwinTimespec>`)
- Saw: at `c06ddb0f570`, the entryless `fractional_integer_landing` fixture stops during preparation at `DarwinTimespec` on `linux_arm64`, where `c_abi_fields::append` requires unsupported clause arithmetic. The [explicitly demanded scalar layout](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L1203) at `59b8d53b729` reaches the same arithmetic refusal on `linux_x86_64` before checking size 24, alignment 8 and offsets `[0, 8, 16]`. (The earlier alias refusal at the same field, "selected type alias has no retained source declaration", was a separate defect in extension resolution and is fixed.)
- Cost: a program that never demands the macOS layout is refused for it, against [demand-driven layout formation](wiki/spec/build/configuration.md#multi-target-compilation); may be the same uncalled-library refusal as "A whole-program stage refuses every program for a library construct nobody calls"
- Reported: 2026-11-15, Claude Code, while removing the product gate's duplicate review pass
