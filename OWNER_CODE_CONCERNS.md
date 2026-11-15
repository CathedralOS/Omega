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
- Category: tests that pin the bug
- Where: [shared typed-view control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/sequence_values.rs#L253), [original borrow lookup](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L239)
- Saw: At `6c485f370e4` `echo(items: &[Item]) -> &[Item] { items }` refuses preparing its entry with `call the view's original incoming borrow cannot be identified`, so the test never reaches the returned-backing consumer or the `&mut [Item]` refusal it exists to check.
- Cost: Nothing checks that a host-supplied shared typed view keeps its backing through a returned view, or that immutable host values cannot supply an exclusive one. Identify the probe caller's actual view argument as the borrow `echo` returns.
- Reported: 2026-11-15, Claude Code, while repairing the Build-time sequence controls

### The local-loan escape fixture no longer reads its rejection evidence
- Category: tests that pin the bug
- Where: [local borrow negative](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/loan_regions.rs#L109)
- Saw: On clean `1a5fd676246` on macOS arm64, `a_loan_of_a_local_is_not_returned_for_a_lifetime_of_the_machine` reaches a rule violation naming the current invocation's local storage and the result field's declared lifetime. The test expects the earlier unavailable verdict and panics before comparing that exact rejection, which currently carries no source span.
- Cost: The original escape witness cannot pin its current rejection or its authored location. Read the precise rule evidence and preserve the escape and legal argument-backed controls.
- Reported: 2026-11-14, Codex, while verifying the shared-window completion repair

### Existing match and reference controls reach obligations with no lowered owner
- Category: knowledge crossing a boundary
- Where: [owned match results](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/owned_match_results.rs), [loan-region negative](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/loan_regions.rs#L237), [obligation roster check](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/minted.rs#L752)
- Saw: On clean `f5ed845ea2f` on macOS arm64, all three owned-match controls and the two-origin reference negative refuse with `an obligation row that nothing in the lowered form owes`. The same tests passed on the earlier `59b8d53b729` Stage 06 run. The two-owned-arm and early-ended-arm positive cases never reach their consuming call. On `1a5fd676246`, the new literal-window match also refuses at this roster check, and its companion stops before examining the selected literal establishment.
- Cost: The compiler stops at obligation ownership before checking either valid value transfer or the specific invalid use. Match each generated obligation to its exact lowered arrival, operation or edge and preserve the original ownership judgments.
- Reported: 2026-11-14, Codex, while attributing Stage 06 failures after the ranked-loop changes

### A literal view loses its descriptor during a crashing transfer
- Category: a verdict with nothing behind it
- Where: [crashing transfer fixture](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/collection_members.rs#L366)
- Saw: On clean `f5ed845ea2f` on macOS arm64, the Stage 06 witness `machine entry() crashes Trap { fail("x"); }` refuses its input loan because `descriptor snapshots cannot be reconstructed` at block 1, operation 2. At `59b8d53b729` this control publishes and its interpreter reaches the asserted `Trap`.
- Cost: A valid shared literal argument loses its known backing before the call reaches its declared crash. Keep the staged descriptor's loan tied to that exact transfer.
- Reported: 2026-11-14, Codex, while attributing Stage 06 failures after the ranked-loop changes

### Storing a shared reborrow falsely conflicts with its parent loan
- Category: a verdict with nothing behind it
- Where: [stored shared-reborrow fixture](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/stored_references.rs#L363)
- Saw: On clean `f5ed845ea2f` on macOS arm64, `let view: View = View { cell: exclusive };` followed by reads of the three cell fields rejects `ChangeWhileBorrowed` instead of completing with Unit. The same source passes at `59b8d53b729`. The source reborrows an exclusive parent as shared and writes only the new record descriptor.
- Cost: The checker treats a valid shared descendant as a conflicting change to its backing. Trace the exact conflict site and retain the sibling and access-strengthening controls.
- Reported: 2026-11-14, Codex, while attributing Stage 06 failures after the ranked-loop changes

### Repeated reads of an untouched formal field disagree about its entry identity
- Category: tests that pin the bug
- Where: [formal leaf observation test](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests.rs#L2046)
- Saw: On clean `f5ed845ea2f` on macOS arm64, the two reads of `p.x` in the shaped `p.x - p.x` control cite `ValueId(5)` and `ValueId(2)` before any intervening write. The fixture previously observed the one entry content value on `59b8d53b729`. This assertion stops before the same-field and distinct-field proof outcomes.
- Cost: The test can no longer verify which stable observation justifies the subtraction. Trace the retained identities and keep the two-same-leaf positive and different-leaf negative.
- Reported: 2026-11-14, Codex, while attributing Stage 06 failures after the ranked-loop changes

### Writes to immutable owned bindings are reported as unavailable
- Category: a verdict with nothing behind it
- Where: [assignment reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/statements.rs#L83), [projected store reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/statements.rs#L182)
- Saw: At `c06ddb0f570`, `immutable_scalar_assigned` and `immutable_record_stored` in `lowered_route_subjects` produce unsupported refusals while reading `spare`. Their same-carrier mutable controls accept. The reader has already identified the owned root and determined it is not assignable. The [mutability rule](wiki/spec/language/ownership.md#owned-local-mutability) requires `mut` for either write.
- Cost: The diagnostic reports a feature gap for an invalid write and omits the assignment's source position. Preserve legal writes through an existing mutable reference held in an immutable binding.
- Reported: 2026-11-14, Codex, while checking compiler subject fixture verdicts

### Instruction selection reconstructs target operands from earlier executable bodies
- Category: re-deriving
- Where: [ordinary native selection](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L110), [call operand construction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs#L171), [runtime-index reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs#L273), [descriptor-origin reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_scalar_graph.rs#L2219)
- Saw: At `1897804e0dbd`, ordinary Omega 03 construction receives target operations together with abstract operations and the optimized executable unit. It zips optimized blocks and operation nodes with target rows (`legalization_source.rs:3101`) and reconstructs structural-call indices from optimized definitions, path geometry and accepted obligation facts. Target arguments already retain index operands and strides, but omit the extent and proof facts this reconstruction supplies: the target handoff is incomplete. Descriptor-bearing call results can reconstruct an optimized catalog graph and selected boundary operations to derive origins. These are production construction paths, not only explicit independent validators or shared immutable type-catalog queries; no miscompile or runtime cost was measured in this audit.
- Cost: Target operands remain dependent on earlier executable histories and their matching operation roster, so adding a value form or changing target operations can require synchronized recognizers across stages. Fix the producer to complete the needed runtime-index evidence and descriptor-origin records in the target representation, retaining shared immutable catalogs rather than copying them; have ordinary selection consume that product and keep earlier bodies available to independent correspondence validation. Acceptance: selection constructs the same instructions from target operations and their explicit catalogs/evidence without reading earlier executable bodies, while the independent validator still rejects changed operands or evidence.
- Reported: 2026-11-15, Codex, while verifying representation handoffs after the IR cleanup; static source and caller-chain evidence only

### The Build-time access-plan control stops before its borrowed plan result
- Category: tests that pin the bug
- Where: [access-plan control](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/access_plan_construction.rs#L191), [direct result slot lookup](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L279)
- Saw: After `core::access` stopped attaching machines to data, the control stopped entering `construct_restricted_plan` with `returned reference callee has no complete actual source correspondence`: its ordinary callee `access::new_plan` returns an `AccessPlan` holding the borrowed `TypeSchema` bytes. At `c6a4fd36813` it panics earlier, in symbol resolution, with `arena span_or_empty received an unresolved span`. The make-based sequence controls that shared this refusal now run: a returned sequence its body built carries its own roster.
- Cost: The control cannot check schema backing or complete zero tails. Repair the stale span, then trace the plan's borrowed result slots to the caller's schema argument.
- Reported: 2026-11-14, Codex, while comparing Build-time evaluator tests; narrowed 2026-11-15 by Claude Code to the access-plan control after the sequence controls were repaired

### Provider body admission requires a second satisfaction edge
- Category: a fact with no owner
- Where: [provider body admission](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/machine.rs#L396), [routed entry contract](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/machine.rs#L1418)
- Saw: With supply lookup reading the exact named row, `Native satisfies Service { Service::go = provider_go; }` retains a Reference row to `provider_go`. Removing only the machine's duplicate `satisfies Service::go` clause leaves that row intact. Both the exact requirement and whole-trait supplies then refuse with `machine provider body needs its exact supplied requirement`.
- Cost: A named implementation explicitly selects the machine for its slot. Provider body admission gets the advertised requirement only from an edge or row the machine owns. Route the selected entry under the named row's requirement and retain the independent check of the machine's own contract and body.
- Reported: 2026-11-14, Codex, while checking named provider selection rows

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

### An explicit exclusive borrow becomes shared before the checker sees its formation
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/read/calls.rs`, `lent` and its `attenuated` argument path
- Saw: with a live `let view: &Build = builder`, an ordinary `observe(&mut builder)` call whose parameter is `&Build` is accepted even when `observe(view)` follows it. Producer SHA256 `4B90BD6AC0F6C1FD459B44388B866CCD5B1B85AA44B96930DCFA204A8737FC41` checked this source with exit 0. This is an ordinary helper call, not Build selection syntax. `lent` checks source access but records the formal's shared access, losing the exclusive formation before the lowered-form checker judges overlapping live loans.
- Cost: an explicit exclusive borrow can coexist with an already-live shared cohort. Preserve the written formation and its attenuation as real loan ancestry; do not reject legal attenuation or relax cohort checks. The Build selection repair uses a real mutable-parameter call instead, so it does not depend on this path. Acceptance pairs the still-live cohort with one whose last use precedes the exclusive formation, and keeps ordinary shared calls legal while the cohort is live.
- Reported: 2026-11-14, Codex, while falsifying Build activation access before landing #3207

### Typing scans declarations after resolving the exact binding or field identity
- Category: one idea, several implementations
- Where: [state binding scan](omega-rust/psi/compilation/representations/typed-trees/src/declared_bindings.rs#L41), [whole-program field scan](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L17), [existing local relationship index](omega-rust/psi/compilation/representations/typed-trees/src/declaration_tables_and_index.rs#L683), [existing field relationship index](omega-rust/psi/compilation/representations/typed-trees/src/declaration_tables_and_index.rs#L779)
- Saw: `declared_binding_type_by_symbol` scans a state's parameters and local statements to find one unique resolved symbol; `exact_declared_field` scans all data definitions and fields for one field symbol. Constructor/place/callable type queries use these scanning routes even though the representation also maintains local and field relationships indexed by symbol. The scans deliberately detect duplicates and enforce owner/type constraints, so replacing them with a first match would weaken the contract.
- Cost: Repeated uses revisit unrelated locals or records. Provide owner-aware, uniqueness-preserving borrowed lookup through the existing relationship owner; during mutable construction, maintain the relevant body's binding index instead of repeatedly rebuilding a global one. Return a row or compact coordinate without allocating candidate vectors on successful queries. Acceptance holds queried identities fixed while growing unrelated records, and grows locals/uses independently while counting visits. Preserve duplicate-binder, foreign-state, stale-generation and case/constructor-owner refusals, with cold and retained-snapshot controls.
- Reported: 2026-11-14, Codex, while reconciling the peer audit against `3b0ad89c1fd47`

### Completing selected bodies copies the retained typed program
- Category: re-deriving
- Where: [selected continuation](omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L935), [activation copy](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L566), [retained-body completion](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L847), [shared typing loop](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1596), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L624)
- Saw: `SeededTypingBase` holds `Arc<TypedTrees>`, but body completion calls `Arc::unwrap_or_clone(retained.typed)` and feeds the candidate to one mutable `Lowerer`. `SourceContinuation::complete_selected` starts from `input.typing.clone()` while its `Arc<SourceCompletionInput>` stays retained for later selections, so `activate_body_completion`'s `Arc::make_mut` deep-copies every `TypedTrees` table once per selected-source continuation, even when one small body set changes; later completions in that loop own the Arc uniquely. Sharing the outer Arc does not make the stored tables independently owned. No local repair fits: per-table copy-on-write still copies the expression and statement arenas every completion appends to, and `Arena` exposes contiguous spans (`span`, `storage_slice`, `storage_view`), so paged sharing changes its contract (verified 2026-11-15 at `171113b06`).
- Cost: A small continuation can copy unrelated typed storage and cannot return independent body output. Separate immutable declaration/type storage from owned body tables; publish selected replacements through stable handles without flattening unrelated bodies. A shared-snapshot continuation must retain untouched storage, exact authority/context identities and refusal behavior. Measure copied bytes and peak memory; keep signature and inferred-summary dependencies explicit. Defer wide Psi 02–04 representation splits while `LOWERED-FORM-CHECKER` coverage is actively churning those stages unless a measured continuation bottleneck requires it earlier. See the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals

### Whole-value structural domains are erased before the lowered checker
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/qualifications.rs` (`Qualifications::declare` skips every `ValueType::Record` carrier in `declared_domains`); `lower.rs`, `lower/places.rs`, `lower/control.rs` (structural parameter, result and operation-result qualifications built as `Vec::new()`); `lower/places.rs` (`ExpressionKind::Requalify` on a record is `located(operand)`, the same place); `terminal-psi/codec/src/foundation_validation.rs` and `psi/interpreter/src/engine_structural.rs`, `storage_*.rs` (every structural operation requires empty `qualification_establishments`)
- Saw: the reader types, admits and refuses record domains (`compiler/tests/domain_qualified_types.rs`, `ACCEPTED_DATA`: `reading as +Checked`, the issuer call `stamp(ticket)`, `sealed.seal` returned as `Seal in Issued`), but Terminal Psi receives none of it: `module.structural_domains` is empty and every structural qualification list is empty, so the sole lowered checker accepts those machines with nothing behind the membership. Three gaps block carrying it: (1) no Terminal operation may establish a whole-value structural membership; `qualification_establishments` (domain + route index, encoding.md "structural operation result") is rejected by every validator and interpreter path, and `MoveStructuralValue` admits additions only to a non-unrestricted value and with no establishment row, so neither `as +D` on a copyable record nor an issuer route has a form; (2) `StructuralFieldDeclaration` carries no domain, so a field's membership (`Sealed.seal in Issued`) must ride every place of the owner as a projected qualification, including literals, field reads, stores and joins; (3) 06 runs `scalar_qualifications::validate` but no structural judgment, and the codec's structural foundation validation runs only on decode, not on the unsealed module the verdict is taken from
- Cost: a missing or forged whole-value or field membership, or one added outside its issuer routes, is contradicted only by the reader. Repair as `psi: retain scalar domains through lowered checking` (#3227) did for scalars: produce the structural catalog with issuer routes, give records' `as +D`/`as -D` an exact establishment/erasure operation whose route the checker matches against the enclosing callable, transport whole and projected qualifications through every place family, and run one shared structural qualification judgment in 06 before inference; the interpreter and codec must accept the same establishment rows
- Reported: 2026-11-13, Claude Code, while stating domain predicates as contracts; premise re-verified and gaps mapped 2026-11-15, Claude Code

### A data field's view written without a lifetime is one region in every declaration
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/read/regions.rs` (`Elision::Members`, `elided_region` returning `None`, then `fixed_region` of the typed trees' `RegionForm::Elided` identity); bundled data such as `core/seq.omg` (`items: &[u64]`), `core/wire.omg` (`bytes_and_ordering: &[u8]`), `std/calling.omg` (`reason: &[u8] in Utf8`)
- Saw: signatures and locals now give each elided position its own lifetime or region variable, and a data field that elides a *data application's* lifetime is refused. A field that elides a *reference's* lifetime, `data D { name: &[u8]; }`, still takes the one fixed region of its ownerless rendered identity, so `a.name = b.name` between any two such values is accepted and a formal's view is refused as a type mismatch. It is sound today only because nothing but literals and other such fields produces a value of that region.
- Cost: the first slice that forms a view of local storage, or lets a variable decide as that region, exchanges views across unrelated records. lifetimes.md says borrow-carrying data "declares the lifetimes of its views"; either refuse the form (and state lifetimes in the bundled data above) or give each field its own region
- Reported: 2026-11-13, Claude Code, while giving a call's lifetimes the regions of the views it passes; narrowed 2026-11-15

### A whole-program stage refuses every program for a library construct nobody calls
- Category: a verdict with nothing behind it
- Where: `03_symbol-resolved-trees-to-typed-trees` (the remaining `unavailable_judgment` callers: the declaration-lowering queries in `typing_queries.rs` and `statement_lowering.rs`, and "closed const argument replay" in `type_reference.rs`); `build-time-evaluation/src/source_frontier.rs` (`source_jobs`: the side tables it still refuses whole, such as constant recipes and wire plans); any by-name refusal added to `04_typed-trees-to-typed-trees`
- Saw: these stages see the program together with all of `core` and `std` and have no notion of what an entry demands, so one such construct in an uncalled library declaration stops a program that never reaches it; the gates the route probe met (unbound calls, assignment literals, constructor field types, layout-policy and computed types) and the construction-destination pass's call gates (a mathematical call taking a built argument, a callee whose parameters typing does not select), a membership fact whose domain selection stays late-bound (found through the owners of the value it constrains), and a const argument typing does not admit now record the refusal on the declaration instead (`typed-trees/src/deferred_refusals.rs`), and the others still refuse the program. A refusal inside one declaration's lowering (`type_resolved_cohort`) returns midway through it, so those gates need a clean per-declaration rollback before they can move
- Cost: each remaining gate must be served for the entire library before any program compiles, or be moved onto the same carrier one by one as a program meets it
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing

### The parser still accepts a boundary requirement with no `crashes` clause
- Category: a verdict with nothing behind it
- Where: `01_tokens-to-syntax-trees/src/declarations.rs` (refuses a boundary requirement without `reaches`, says nothing about `crashes`); `05_typed-trees-to-terminal-psi/src/read/calls.rs` (`boundary`); `samples/` and `tests/fixtures/`
- Saw: effects.md says a boundary requirement "must likewise write `crashes`". Only the lowered-form checker enforces it, and only at a call. The bundled library now writes the clause on all 400 of its requirements, but 1,745 requirements in 1,062 files under `samples/` and 70 in 40 files under `tests/fixtures/` do not, so the parser cannot refuse the omission until those are migrated, each to the ceiling its providers really have
- Cost: a library or program author learns that the clause is missing only when some caller is refused, and 30 library requirements with no provider and no caller carry `crashes Trap crashes Abort` because nothing in the repository says what narrower ceiling their owner intends
- Reported: 2026-11-13, Claude Code, while writing the bundled library's crash ceilings

### Layout evaluation forms every target's library layouts instead of the subject's closure
- Category: a verdict with nothing behind it
- Where: [reusable layout scope](omega-rust/omega/compilation/compiler/src/checked/checking/source_checking.rs#L272) (no root bindings); [layout evaluation](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/fixed_layouts.rs#L64), which then selects all declarations; reached at `source/library/std/targets/macos_arm64/filesystem_abi.omg` (`CLayout<DarwinTimespec>`)
- Saw: at `c06ddb0f570`, the entryless `fractional_integer_landing` fixture stops during preparation at `DarwinTimespec` on `linux_arm64`, where `c_abi_fields::append` requires unsupported clause arithmetic. The [explicitly demanded scalar layout](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L1203) at `59b8d53b729` reaches the same arithmetic refusal on `linux_x86_64` before checking size 24, alignment 8 and offsets `[0, 8, 16]`. (The earlier alias refusal at the same field, "selected type alias has no retained source declaration", was a separate defect in extension resolution and is fixed.)
- Cost: a program that never demands the macOS layout is refused for it, against [demand-driven layout formation](wiki/spec/build/configuration.md#multi-target-compilation); may be the same uncalled-library refusal as "A whole-program stage refuses every program for a library construct nobody calls"
- Reported: 2026-11-15, Claude Code, while removing the product gate's duplicate review pass

### Receiving a selected result loses its missing lifetime evidence reason
- Category: a verdict with nothing behind it
- Where: [public result lifetime lookup](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references/selected_results.rs#L50), [descriptor graph invocation](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptors.rs#L13861)
- Saw: At `1a5fd676246` the published selected `Chosen::hand` control accepts with one public header and complete lifetime groups. Keep that operation header, its invocation row and the selected method tuple. Remove only the method's `result_lifetimes`. The public semantic query reports `selected reference result has an unresolved declared lifetime frontier`. The receiving judgment still refuses, but reports that descriptor storage flow cannot be reconstructed at the call.
- Cost: The receiver drops the exact missing public evidence when the descriptor graph takes the query's optional result. Keep the existing failure reason and the actual call site through the receiving judgment.
- Reported: 2026-11-14, Codex, while auditing the selected reference public contract after #3351

### A newly claimed worktree still appears idle in status
- Category: one idea, several implementations
- Where: [worktree status display](tools/xtask/src/worktree.rs), [idle slot selection](tools/xtask/src/worktree.rs)
- Saw: In a temporary pool, `worktree acquire tools-cache-repro` claims `worker-1` while its HEAD remains at `origin/main`. `worktree status` displays `tools-cache-repro` in the topic column and `idle` in the state column. Replayed with the initial `f5ed845ea2f` command and the rebased cache fix `36f5f7a3d04`. The acquire path excludes that same slot from the idle set.
- Cost: At the start of a task, the status table reports an assigned slot as idle until its HEAD changes. Use the retained topic when reporting the slot state.
- Reported: 2026-11-14, Codex, while exercising worktree acquisition after cache collection
