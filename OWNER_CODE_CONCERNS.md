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

### Writes to immutable owned bindings are reported as unavailable
- Category: a verdict with nothing behind it
- Where: [assignment reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/statements.rs#L83), [projected store reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/statements.rs#L182)
- Saw: At `c06ddb0f570`, `immutable_scalar_assigned` and `immutable_record_stored` in `lowered_route_subjects` produce unsupported refusals while reading `spare`. Their same-carrier mutable controls accept. The reader has already identified the owned root and determined it is not assignable. The [mutability rule](wiki/spec/language/ownership.md#owned-local-mutability) requires `mut` for either write.
- Cost: The diagnostic reports a feature gap for an invalid write and omits the assignment's source position. Preserve legal writes through an existing mutable reference held in an immutable binding.
- Reported: 2026-11-14, Codex, while checking compiler subject fixture verdicts

### An indexed dispatch discriminator has no canonical seal owner
- Category: knowledge crossing a boundary
- Where: [terminator path ownership](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lower/places.rs#L2384), [native proof provenance](omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/graph.rs#L20)
- Saw: A hosted machine matching a case of `table.choices[index]` in a two-element array with `requires index < 2` reaches `UnownedSeal` on clean `8db72a0c99` and the operation-owner repair `de996b3d9aa`. Its twin changes only the contract to `index < 3` and both versions refuse at `cannot prove storage index bounds` on the match.
- Cost: Stage 06 proves the indexed discriminator at its block terminator, before a case chooses an outgoing edge. Canonical receiving provenance names operations and edges. Preserve the existing pre-dispatch execution site through evidence attachment and realization so a chosen edge cannot stand in for the shared read.
- Reported: 2026-11-14, Codex, while repairing ordinary indexed operation evidence on macOS arm64

### A direct partial view of a fixed byte array never reaches endpoint checking
- Category: tests that pin the bug
- Where: [const-length sample](samples/cli/facts/length/const_name/main.omg#L12), [target-body sample](samples/cli/facts/length/target_body/main.omg#L49), [array range reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/arrays.rs)
- Saw: On `ef7d7a17936` with the shared-view range repair, both unchanged positives and their authored source twins with bound entries still refuse `&items[0..3]` with `partial array views need their endpoint and window correspondence` on macOS arm64. Here `items` is a fixed `[u8; 4]`. Each sample's dynamic range over an existing view proceeds through ordinary range checking.
- Cost: A checked direct array window cannot reach its last-byte check. Use the fixed storage's exact extent and projected window when checking a direct range and establishing its loan.
- Reported: 2026-11-14, Codex, while restoring the post-merge runtime sample

### A match loses a byte window's length when its arms name equivalent values
- Category: tests that pin the bug
- Where: [match length sample](samples/cli/facts/length/match_join/main.omg#L7)
- Saw: On `ef7d7a17936` with the shared-view range repair, the macOS arm64 check for `let length: u64 = match flag { true -> window.len, false -> count };` refuses the last-byte read at `8:11` because it cannot prove operation safety. `window` was formed from `items[0..count]` with positive `count`. The control whose false arm uses the backing length also fails to establish the operation.
- Cost: A match arm that uses the endpoint by which the window was formed cannot carry the same bound as an arm that reads its length. Retain the shared equality at the join and keep the backing-length twin unsafe.
- Reported: 2026-11-14, Codex, while checking length transport through a borrowed byte window

### A shared byte window loses its completion observation across a length call
- Category: a verdict with nothing behind it
- Where: [length result sample](samples/cli/facts/length/call_result/main.omg#L11), [forwarded length sample](samples/cli/facts/length/other_machine/main.omg#L14), [completion observation reader](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places.rs#L3851)
- Saw: On `ef7d7a17936` with the shared-view range repair, both unchanged positives stop on macOS arm64 at `borrowed completion storage has no post-call observation`. Their shared view already passed formation, and each length call's result guarantee names the view given as input. The nested state control keeps the matching length and executes.
- Cost: Code cannot use the result guarantee to index the same existing shared window after an ordinary call. Keep the unchanged actual observation attached to the shared parameter in the ordinary completion mechanism.
- Reported: 2026-11-14, Codex, while checking length transport through a borrowed byte window

### An unimported case can turn a deferred name error into a resolver failure
- Category: knowledge crossing a boundary
- Where: [global bare case scan](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_expressions.rs#L47)
- Saw: At `d75251ea39f` the source forest for `machine main() {} machine unused() -> u64 { Missing }` reaches typing and records `UndeclaredValueName` on `unused`. Add an unimported dependency containing `public data Choice { case Missing; }` and resolution of the same client instead fails immediately with `case value Missing requires its carrier-qualified Type::Missing path` (the diagnostic quotes both names).
- Cost: Whether an unused declaration stops resolution depends on an inaccessible case spelling. Classify the exact authored occurrence through ordinary scoped lookup and preserve the deferred violation on its holder.
- Reported: 2026-11-14, Codex, while tracing scoped unbound-name verdicts on macOS arm64

### Instruction selection reconstructs target operands from earlier executable bodies
- Category: re-deriving
- Where: [ordinary native selection](omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L110), [call operand construction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_source.rs#L171), [runtime-index reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization.rs#L273), [descriptor-origin reconstruction](omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization_scalar_graph.rs#L2219)
- Saw: At `1897804e0dbd`, ordinary Omega 03 construction receives target operations together with abstract operations and the optimized executable unit. It zips optimized blocks and operation nodes with target rows (`legalization_source.rs:3101`) and reconstructs structural-call indices from optimized definitions, path geometry and accepted obligation facts. Target arguments already retain index operands and strides, but omit the extent and proof facts this reconstruction supplies: the target handoff is incomplete. Descriptor-bearing call results can reconstruct an optimized catalog graph and selected boundary operations to derive origins. These are production construction paths, not only explicit independent validators or shared immutable type-catalog queries; no miscompile or runtime cost was measured in this audit.
- Cost: Target operands remain dependent on earlier executable histories and their matching operation roster, so adding a value form or changing target operations can require synchronized recognizers across stages. Fix the producer to complete the needed runtime-index evidence and descriptor-origin records in the target representation, retaining shared immutable catalogs rather than copying them; have ordinary selection consume that product and keep earlier bodies available to independent correspondence validation. Acceptance: selection constructs the same instructions from target operations and their explicit catalogs/evidence without reading earlier executable bodies, while the independent validator still rejects changed operands or evidence.
- Reported: 2026-11-15, Codex, while verifying representation handoffs after the IR cleanup; static source and caller-chain evidence only

### Build-time access fixtures stop at the data-owner check
- Category: tests that pin the bug
- Where: [borrowed view fixtures](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/borrowed_views.rs#L9), [access plan constructors](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/access_plan_construction.omg#L9), [checked callee ownership](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/machine.rs#L898)
- Saw: On macOS arm64 at clean `59b8d53b729`, both closed-borrow controls refuse `Access::view_mut` in `relay`, and the access-plan control refuses `AccessPlan::new` in `construct_restricted_plan`. All three stop at `a data declaration cannot own a machine; declare an ordinary static machine` during Build-time source preparation.
- Cost: These controls cannot check original backing and exclusive-loan behavior, invalid view interpretation, or complete access-plan construction. Trace the exact library call owners and preserve those acceptance and rejection judgments.
- Reported: 2026-11-15, Codex, while running the Build-time evaluator suite on macOS arm64

### The Build-time sequence fixtures stop at reference result correspondence
- Category: tests that pin the bug
- Where: [sequence fixture](omega-rust/psi/compilation/build-time-evaluation/src/source_jobs/sequence_values.rs#L81), [direct result slot lookup](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs#L279)
- Saw: On macOS arm64 at base `69ec9aa5bd8`, both sequence controls fail preparing the ordinary `make` entry with `returned reference callee has no complete actual source correspondence`. The same refusal remains with the universe-level admission repair.
- Cost: The fixtures cannot check nested sequence transport or live bounds. Trace the ordinary callee's result slots through Build-time preparation while preserving both the borrowed-transport and invalid-index cases.
- Reported: 2026-11-14, Codex, while comparing Build-time evaluator tests

### Provider supply ignores named implementations' selection rows
- Category: a fact with no owner
- Where: [provider realization](omega-rust/psi/compilation/representations/typed-trees/src/product_and_trait_declarations.rs#L1618), [provider declaration identity](omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs#L212)
- Saw: At `7fca4c49c98` the Stage 06 supply harness finds one typed reference row for `Native satisfies Service { Service::go = provider_go; }`, and that row retains the exact `provider_go` machine. Nominating `Native` for `Service::go` or for the whole `Service` still refuses with `provider does not give the supplied requirement exactly one machine`. The inline implementation of the same contract passes and publishes.
- Cost: Provider admission looks for a machine owned by the implementation. A selection row keeps the existing machine's original declaration. Resolve the named implementation's exact row and check its body against the nominated requirement.
- Reported: 2026-11-14, Codex, while restoring the Stage 06 provider-supply controls

### The selected reference result control stops before checking its public contract
- Category: tests that pin the bug
- Where: [Stage 06 selected result control](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/references.rs#L694)
- Saw: At `7f12cb9f722` the call of `Chosen::hand` in `relay` refuses with `call selected result has no complete original declared types`. The test expects the refusal for the absent public requirement contract on a selected reference result.
- Cost: The test cannot demonstrate that the selected call refuses to use only the private implementation's result origin. Trace the selected declaration and preserve the missing-contract control.
- Reported: 2026-11-14, Codex, while rerunning the Stage 06 refusal controls on macOS arm64

### The checker ends a reference loan at its last use; the lowering never releases the carrier
- Category: a verdict with nothing behind it
- Where: checker loan closure in [06 places/loans.rs](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/loans.rs); the 05 lowering emits no `ReleaseReference` and places carrier cleanup only at returns (`05_typed-trees-to-terminal-psi/src/lower/places.rs`); interpreter transfer check `owned value cannot transfer while its storage is borrowed` (`terminal-interpreter/src/storage_records_and_arrays.rs`, `stage_owned_value`)
- Saw: `let mut holder: Holder = Holder { engine: &mut engine }; start(&mut holder); transition engine.mode { ... }` with a `Mode` that is not `[copy]` is accepted and published, then the dispatch's `MoveStructuralValue` traps in the Psi interpreter. It is not specific to a reference stored in a record: on `372860cf534` the plain local `let lent: &mut Engine = &mut engine; start(lent); transition engine.mode { ... }` traps the same way. The same programs with `Mode [copy]` run (`06_terminal-psi-to-terminal-psi/src/tests/stored_references.rs`, `a_data_value_is_stored_through_a_stored_reference`)
- Cost: an accepted program fails when run. ownership.md, "Borrows and aliases", closes a local's loan "after the last use of that local", so the checker is right and the lowering must release each reference carrier (a local, or the reference field of a record) where its loan closes, on every edge leaving its live range, with 06 checking that release against the same descriptor lifetime
- Reported: 2026-11-14, Claude Code (worker-8), while storing a data value through a stored reference; widened 2026-11-15 to plain locals

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

### Unchecked arena lookups still conflate stale handles with the zero sentinel
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/foundation/arena/src/arena.rs` (`get`, `get_mut`, `span_or_empty`); `paged.rs` (corresponding paged lookups)
- Saw: unchecked lookups return the shared default or an empty slice for unresolved nonzero handles; mutable lookups can change that default. Handle zero deliberately names absence, including its shared default; generation checks alone cannot establish ownership between arenas using the same generation.
- Cost: consumers without an owning-boundary validation can interpret a stale handle as a missing declaration or mutate the shared default. Those consumers need recoverable validation before using these infallible APIs, without changing legitimate zero-sentinel behavior.
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A whole-program stage refuses every program for a library construct nobody calls
- Category: a verdict with nothing behind it
- Where: `03_symbol-resolved-trees-to-typed-trees` (the remaining `unavailable_judgment` callers, such as "contract membership carrier selection" in `contracts_and_signatures.rs`, "closed const argument declaration custody" in `type_reference.rs`, and the declaration-lowering queries in `typing_queries.rs` and `statement_lowering.rs`); `build-time-evaluation/src/source_frontier.rs` (`source_jobs`: the side tables it still refuses whole, such as constant recipes and wire plans); any by-name refusal added to `04_typed-trees-to-typed-trees`
- Saw: these stages see the program together with all of `core` and `std` and have no notion of what an entry demands, so one such construct in an uncalled library declaration stops a program that never reaches it; the gates the route probe met (unbound calls, assignment literals, constructor field types, layout-policy and computed types) and the construction-destination pass's call gates (a mathematical call taking a built argument, a callee whose parameters typing does not select) now record the refusal on the declaration instead (`typed-trees/src/deferred_refusals.rs`), and the others still refuse the program. A proof fact carries no owning declaration to record a row on, and a refusal inside one declaration's lowering (`type_resolved_cohort`) returns midway through it, so those gates need an owner or a clean per-declaration rollback before they can move
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
