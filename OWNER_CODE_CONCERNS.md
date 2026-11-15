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

### Stage 06 refusal controls omit the declaration context now emitted by Stage 05
- Category: tests that pin the bug
- Where: [example diagnostic assertions](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/address_bounds.rs)
- Saw: On macOS arm64 at production base `af9510104ca63735a02adea551f7e50332e461c9` with the test-constructor compatibility repair, the Stage 06 suite reports 70 failures. In 46 of those, the exact actual text is the old expected reason followed by one or more `while reading …` contexts. For example, the missing-`embed` control now names the signature of `sized`.
- Cost: The red diagnostic assertions obscure independent failures. Pin the original reason and its appropriate declaration context. Attribute the remaining failures separately.
- Reported: 2026-11-14, Codex, while restoring downstream checker test compilation

### Stored-reference field tests stop at field selection before checking custody
- Category: tests that pin the bug
- Where: [Stage 06 stored-reference fixtures](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/stored_references.rs)
- Saw: At production base `af9510104ca` with the test-constructor compatibility repair, `a_store_through_a_stored_reference_changes_the_original_cell`, `a_record_holding_a_reference_is_passed_on_as_an_owned_argument`, and other stored-reference fixtures fail with `typing member is not a field of its receiver's data type`.
- Cost: Positive programs and their rejection controls cannot reach the original-cell, ownership or loan checks. Trace exact field identity through the referent and preserve the programs and their custody oracles.
- Reported: 2026-11-14, Codex, while restoring downstream checker test compilation on macOS arm64

### An indexed read from a lent record cannot retain its scalar qualification
- Category: knowledge crossing a boundary
- Where: [Stage 06 runtime indexed element controls](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/tests/runtime_indexed_elements.rs)
- Saw: At production base `af9510104ca` with the test-constructor compatibility repair, the read/write acceptance test expects `accepted` and receives `MalformedStructuralFoundation("ordinary scalar operation cannot establish a declared domain")` from scalar-qualification validation.
- Cost: The ordinary indexed-element route cannot show that a value taken from lent storage keeps its declared scalar qualification. Trace the selected array element type through the emitted operation and its validator before changing the fixture.
- Reported: 2026-11-14, Codex, while restoring downstream checker test compilation on macOS arm64

### The macOS entry adapter exhausts proof search before running a Unit program
- Category: verifying costs more than writing
- Where: [macOS arm64 entry Calling policy](source/library/std/targets/macos_arm64/entry.omg#L38), `physical_indices`
- Saw: On macOS arm64, the compiler built from clean `a70ae5ad39e56fd0b94f412321050e7b5c891361` exits 200 on `omega run --both` for a plain Unit `main` with the trapping fixture's unchanged hosted Build. The bundled Calling policy reports exhausted bounded disjunction search for operation safety in `physical_indices`, before either engine runs.
- Cost: Hosted native acceptance can stop in the standard library independently of the program being checked. Trace the conjunction of array-index bounds and preserve each index guard and its invalid control when repairing the proof path. Other targets were unmeasured.
- Reported: 2026-11-14, Codex, while validating native execution of suffixed arithmetic

### The target-body reachability policy has no compiler caller
- Category: zombie machinery
- Where: [target_bodies.rs](omega-rust/omega/compilation/realization/provider-selection/src/bodies/target_bodies.rs#L14), [typed direct-call inventory](omega-rust/psi/compilation/representations/typed-trees/src/service_declarations.rs#L90)
- Saw: `DirectTargetBodyAbsences` and `reject_reached_inert_calls` appear only at their definitions and reexports in the workspace. The inventory's direct-call accessors have only crate tests as customers. Current [target supply](omega-rust/omega/compilation/realization/provider-planning/src/target_supply.rs#L639) refuses a demanded target-machine family by name.
- Cost: The exported graph and inventory tests can look like evidence that current target preparation resolves these requests. Trace the remaining direct-call projections before retiring the unused path while preserving the active refusal.
- Reported: 2026-11-14, Codex, while tracing the surviving path from provider choice to publication

### A view returned by a call keeps no loan of the input it borrows from
- Category: a verdict with nothing behind it
- Where: `reference_call_lifetimes` in [terminal-psi semantics references.rs](omega-rust/psi/compilation/representations/terminal-psi/semantics/src/references.rs), consumed by `formed` in [06 places/loans.rs](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/loans.rs)
- Saw: an argument's loan outlives its call only when the result holds a `Reference` (`structural_type_contains_reference_by`); a byte view or element view result retains nothing. Accepted on `6bba6174a2d`: `machine inner(output: &[u8; 4]) -> &[u8] { &output[..] }` with `let view: &[u8] = inner(&name); name[0] = 9; see(view);`, the same with the owner moved (`eat(token); see(view);`), and the same through a boundary requirement (`let path: &[u8] = Fs::required_path(&token); eat(token); see(path);`). The twin with a reference result, `inner(output: &Cell) -> &Cell`, is rejected (`storage is changed while a conflicting reference remains live`). A view passed in as a view is tracked: its own loan follows the result through the flow
- Cost: lifetimes.md, "Returned views", requires the source to "remain live for the result's use", and the checker accepts programs that write or move it. A machine's result has its origins in the callee's summary (`result_slots`). A boundary's result first needs its declared result lifetimes on the boundary declaration (`Program::declared_result_lifetimes`, `05_typed-trees-to-terminal-psi/src/read/regions/result_lifetimes.rs`, published for selected methods only). `result_reference_regions` is not that row: a region states that the result lies inside an input's storage, which [foreign storage](wiki/spec/build/foreign_storage.md#provider-owned-views-and-completion) denies for a provider-owned view, and the interpreter takes every boundary that declares one through its captured-reference protocol (`terminal-interpreter/src/boundary_storage_inputs.rs`, `capture_reference_inputs`). Until then a machine that hands on a boundary's view has that result slot recorded as of unknown origin (`Summary::unknown_results`, `terminal-psi/semantics/src/descriptor_replay.rs`), which keeps every view passed to the call live for the result's use and keeps no loan of storage lent in place
- Reported: 2026-11-14, Claude Code (worker-11), while tracing a Build refused for a missing descriptor flow

### The checker ends a stored reference at its last use; the interpreter keeps it until its record dies
- Category: a verdict with nothing behind it
- Where: checker loan closure in [06 places/loans.rs](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/places/loans.rs); interpreter transfer check `owned value cannot transfer while its storage is borrowed` in `omega-rust/psi/interpreter`
- Saw: `let mut holder: Holder = Holder { engine: &mut engine }; start(&mut holder); transition engine.mode { ... }` with a `Mode` that is not `[copy]` is accepted and published, then the dispatch's `MoveStructuralValue` traps in the Psi interpreter: the record is not used again, but its reference is still registered at run time. The same program with `Mode [copy]` runs (`06_terminal-psi-to-terminal-psi/src/tests/stored_references.rs`, `a_data_value_is_stored_through_a_stored_reference`)
- Cost: an accepted program fails when run; either lowering must release a reference carrier where the checker closes its loan, or the checker must keep the loan to the carrier's end
- Reported: 2026-11-14, Claude Code (worker-8), while storing a data value through a stored reference

### Derived-index freshness keys omit storage and roster identity
- Category: a verdict with nothing behind it
- Where: [call-target index key](omega-rust/psi/compilation/representations/typed-trees/src/declaration_tables_and_index.rs#L1880), [roster marker](omega-rust/psi/compilation/representations/typed-trees/src/declaration_tables_and_index.rs#L1377), [structural fingerprint](omega-rust/psi/compilation/representations/typed-trees/src/declaration_tables_and_index.rs#L1561), [index reuse](omega-rust/psi/foundation/arena/src/owned_index.rs#L60)
- Saw: call-target freshness sums independent arena revisions and roster markers; `roster_marker` includes ordinal/count but omits generation. The broader fingerprint adds revisions and several collection lengths. `OwnedIndex` reuses an answer when the supplied key compares equal. These keys do not distinguish replacement storage with matching revision/counts or every same-length edit. Public typed tables/roots permit replacement; ordinary monotonic mutation of one unchanged arena is not itself evidence of a collision. Callable memoization also depends on the broad fingerprint, invalidating it for unrelated changes.
- Cost: The freshness contract can reuse a derived answer without identifying all of its inputs, while also rebuilding for irrelevant mutations. Give each index exact dependency stamps: relevant storage identities, complete roster handles and mutation epochs, or encapsulated mutation with explicit invalidation. Do not merely hash the existing incomplete inputs. Acceptance warms an index, replaces relevant storage at matching counts/revisions, changes generations and makes same-length edits, then compares against a cold rebuild; unrelated body edits must not rebuild signature-only indexes. Establish the affected production mutation routes before claiming a compiler misjudgment; none was demonstrated by this source audit.
- Reported: 2026-11-14, Codex, while reconciling the peer audit against `3b0ad89c1fd47`

### Proof preparation copies complete derivations before knowing which facts are needed
- Category: re-deriving
- Where: [conjunction expansion](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/known.rs#L57), [fact preparation](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/known.rs#L136), [owned proof trees](omega-rust/psi/compilation/representations/terminal-psi/src/artifacts.rs#L393)
- Saw: `Known::new` constructs proofs for every assumption/axiom. For each conjunct, `flatten` embeds `Box::new(proof.clone())`; `ProofNode` owns its conclusion and recursive children, so this copies their payload. The equation list clones selected facts/proofs again. A wide conjunction can therefore retain its whole parent proposition in every leaf proof.
- Cost: Available but unused facts create potentially quadratic certificate payload. Retain compact proof origins during search (assumption/axiom plus conjunct path, or shared derivation nodes), materializing only selected certificates for kernel checking/publication. A portable proof-format change is not required. Acceptance measures allocations/retained bytes for wide and nested conjunctions with few used facts, checks selected certificates independently under the same assumptions, and preserves specified logical proof accounting; physical sharing must not silently change proof-budget semantics.
- Reported: 2026-11-14, Codex, while reconciling the peer audit against `3b0ad89c1fd47`; payload growth is inferred from owned clones, not benchmarked

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

### Typing and Psi 05 classify unresolved names by global leaf spelling
- Category: knowledge crossing a boundary
- Where: [typing's unbound-value classification](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs#L808), [domain reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/types.rs#L715), [invocation reader](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/services.rs#L149), [global spelling query](omega-rust/psi/compilation/representations/typed-trees/src/deferred_refusals.rs#L260)
- Saw: typing classifies batches of unbound value/call names with `names_nothing_has`; Psi 05 calls it again for individual unresolved domain/invocation rows. The helper walks every symbol and compares only its final name segment, regardless of namespace, visibility or declaration kind. The domain reader first strips its qualified spelling. An unrelated local or library declaration can therefore change unknown-versus-unserved classification; this observation is not evidence that the final checker accepts an invalid program.
- Cost: Later stages repair incomplete resolution information with a weaker global query, and each Psi 05 request revisits the entire symbol table. Have resolution/typing carry the exact resolved target or a structured unresolved reason with its source, scope and authority; let the reader consume that outcome. Preserve the distinction between malformed source and a valid construct deferred until demanded. Acceptance needs same-leaf names in different namespaces and declaration kinds, inaccessible imports, qualified unknown names, and unreachable malformed uses; unrelated declarations must not change the result or trigger another whole-program search. Trace value/call, domain and invocation producers through publication rather than merely indexing the spelling search.
- Reported: 2026-11-14, Codex, during the owner-requested architecture/data-flow audit at `07d22466d48ff`; source inspection, not an execution or timing result

### Specialization receipt queries scan every receipt per dispatched call
- Category: re-deriving
- Where: [receipt lookup](omega-rust/psi/compilation/representations/typed-trees/src/callable_signature_shapes.rs#L127)
- Saw: `selected_implementation_receipt` filters all of `machine_specializations` for the one receipt of an instance, once per dispatched call read by stage 05, proof scope and build-time evaluation. Stage 04 application lookup and the repeated scan in `source_call_requirement_signature` are indexed or removed already.
- Cost: D dispatched calls over S receipts cost D*S symbol comparisons. A carried instance-to-receipt index has to survive `machine_specializations` being a public `Vec` that declaration projection retains and tests mutate, so it needs one stage-owned construction point rather than a cache. Keep the missing/ambiguous/stale refusals.
- Reported: 2026-11-14, Claude Code, while indexing stage 04 generated applications; structural bound, not a benchmark

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

### Interface ownership still changes some imported-service checks
- Category: one idea, several implementations
- Where: `omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider/service_reach.rs:39`; `src/execution/host_imports/filesystem.rs:513`; `omega-rust/psi/compilation/terminal-installation/src/provider_and_target_installation.rs:451`
- Saw: native delegation removes the import's owning service from its declared reach before checking delegation (its unit permits `[owning_service]` with `Unprovided`); filesystem admission skips that same identity; assumed-binding operational compatibility ignores an extra supplied ceiling member when it equals the supplied owner. `wiki/spec/language/effects.md` distinguishes catalogue ownership from the requirement's explicit reach. These permission/compatibility checks must not be reused as proof that a declared ceiling is empty.
- Cost: the source boundary producer can retain a real catalogue owner even for `reaches none`, so downstream code must keep ownership, declared reach and actual delegated host permissions separate. This entry records the wider retained consumers; the current boundary work requires an explicit source ceiling and fixes the actual exclusion walk's implicit owner contribution, without treating these checks as source proof or expanding into an assumed-binding/delegation campaign.
- Reported: 2026-11-13, Codex, while carrying selected entry and ordinary Build boundary calls through the surviving lowered route

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

### Build selection calls are found and published by the spelling of their protocol operation
- Category: one idea built twice
- Where: `03_symbol-resolved-trees-to-typed-trees/src/call_and_product_selections.rs` (finds `BuildBindingOperations` and `bind_native` by name); `terminal-production/src/build_companions.rs` (`binding`, matches the same names again)
- Saw: typing picks the protocol operation of a Build selection by comparing strings, and publication recovers which operation a boundary call is by comparing the requirement's name a second time. The typed call also carries no static application, so `bind_native<Locator>` reaches the checker as an open requirement although typing knows the locator argument it was written with
- Cost: the host row's operation is decided twice from text, and the native binding cannot be closed at its locator type by the pass that closes every other application until typing records that application on the call
- Reported: 2026-11-13, Claude Code, while closing boundary requirements at the types their calls apply

### The scalar contract fixture's inferred call proof is refused by the kernel
- Category: a proof producer and checker disagree
- Where: `tests/fixtures/run/calls/scalar_contract_route_exit/main.omg:26` (the call `settle(product)`)
- Saw: `omega run --both` on the unchanged fixture reports `cannot prove requires contract for call settle from main: inferred input condition`, with `kernel Certificate(IntegerOrderMiddleMismatch)`, before either engine runs. Observed on Windows x86-64 with the scalar receiving-type diagnostic repair, then reproduced at the same line after restoring all changed production files exactly to `44ed2132219fb20261abeab4ef3e61b4ab0f845a` and rebuilding `omega` through `mbx`. The fixture is unchanged, and its contracts and guards were not weakened.
- Cost: a simple scalar contract program cannot provide native/Psi execution evidence, and an invalid proof certificate is surfaced as a caller fact it should establish instead of identifying the proof producer's disagreement with the kernel
- Reported: 2026-11-14, Codex, while checking scalar receiving-type diagnostics

### Every product gate is refused at a macOS layout alias while building for Linux
- Category: a verdict with nothing behind it
- Where: `source/library/std/targets/macos_arm64/filesystem_abi.omg:20` (`atime: CLayout<DarwinTimespec>`); reached by `tests/product/gate.sh` through each `tests/psi/*` gate and the product roots
- Saw: `OMEGA_TARGET=linux_x86_64 sh tests/psi/resolver/test-resolver.sh` at `372860cf534` stops after 54 s with "checked compilation failed for package `psi`: selected type alias has no retained source declaration" at that macOS-only field, before any gate case runs
- Cost: no product compiler gate can build its artifact on main, so source compiler repairs have no executable cases; the field belongs to another target's ABI and may be the same uncalled-library refusal as "A whole-program stage refuses every program for a library construct nobody calls"
- Reported: 2026-11-15, Claude Code, while removing the product gate's duplicate review pass
