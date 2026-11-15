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

### A suffixed trapping operation is refused as a qualified value
- Category: knowledge crossing a boundary
- Where: [Stage 05 suffixed expression](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/expressions.rs#L1219), [qualification lowering](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/qualifications.rs#L448), [route controls](omega-rust/omega/compilation/compiler/tests/route_probe.rs#L187)
- Saw: `mbx test -p compiler --locked --test route_probe trapping_primitives` at `f208d178cfd` plus the exact #3270 production patch fails both #3204 controls. The published add is `not-supported`, and the runtime witness stops at `type trapping arithmetic policy qualification` before execution. The reader requalifies plain operands to route the suffix through ordinary policy operations, and qualification lowering tries to publish those temporary types as value domains.
- Cost: Plain suffixed programs are blocked by a value-qualification support limit even though the operation and crash predicate have implementations. Carry the operation policy through lowering independently of an authored value qualification and restore both route controls, including the original overflow trap. Full Trapping-domain support is tracked under OPERATION-ARITHMETIC-POLICY.
- Reported: 2026-11-14, Codex, while validating the Stage 05 build repair on macOS arm64

### Build and Product operator-domain resolution controls fail on main
- Category: tests that pin the bug
- Where: [Stage 02 primitive-family tests](omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolution.rs#L2890)
- Saw: On macOS arm64 at clean base `0fc8f51db365ee7b7a3f136794978e1c307ed118`, `mbx test -p syntax-trees-to-symbol-resolved-trees --lib` fails `domain_token_homes_keep_distinct_build_and_product_instances` with no selected domain where it expects `SemanticDomainId(4)`. `inactive_retained_token_bindings_do_not_rejoin_a_product_frontier` refuses the retained `Quantity::subtract` operator because neither operand carries its home domain.
- Cost: The resolution suite cannot show that same-named domain families stay separate across Build and Product or that inactive retained bindings stay out of the Product frontier. Trace domain qualification and selected source custody before changing the tests or their oracles.
- Reported: 2026-11-14, Codex, while attributing resolver test failures during the implicit-entry repair

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

### Parsing and native completion choose their worker budgets independently
- Category: one idea, several implementations
- Where: [parser worker pool](omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs#L113), [native executor queue](omega-rust/omega/build-system/orchestration/src/compile/native_completion_queue.rs#L10), [compiler worker stack](omega-rust/omega/compilation/compiler/src/checked/checking.rs#L51), [prepared machine lowering loop](omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/lib.rs#L882), [settled machine discharge loop](omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/lib.rs#L412)
- Saw: parsing owns a process-global `OnceLock<rayon::ThreadPool>` with 256 MiB stacks (`PARSE_STACK_SIZE`) that defaults to all logical cores; native completion spawns up to four 256 MiB-stack scoped workers (`COMPILE_STACK_SIZE`), including its caller. Neither receives an invocation-wide worker or live-memory budget, and `psi/` cannot import `omega/` orchestration to share one. This establishes separate admission policies, not measured overlap or oversubscription. On the surviving route, Psi 05 lowers machines linearly into owned `Lowered` outputs, while post-inference Psi 06 runs obligation reconstruction and proof discharge over disjoint `MachineTables`.
- Cost: Separate limits cannot bound an invocation's combined active workers, 256 MiB stack reservations or live intermediates, and a process-global parser pool ignores shared-host build budgets. Place the shared execution-admission handle in a target-neutral foundation crate and pass it explicitly per invocation—pairing worker scaling with flat-table recursion (`PIPELINE-ARCHITECTURE-ENFORCEMENT`) so 256 MiB stacks do not multiply. Queue lightweight keys, materialize scratch after admission and distinguish estimated memory reservations from enforced limits. After sequential repeated-work repairs (`outcome.rs`, `graph.rs`, `known.rs`) and a fresh baseline on the surviving route (the ~3,200s Psi 05 measurement in #2788 belonged to the deleted checker), trial post-inference Psi 06 or Psi 05 using dynamic work-stealing over heavy-tailed machines rather than static equal-count chunks, and select the lowest-`MachineId` refusal via a failure watermark rather than completion-order early exit. Closure requires bounded nested execution and a measured elapsed-time benefit without unacceptable small-input or memory regression. See the [draft execution admission design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals; threading placement is a proposed experiment, not a measured speedup

### Logical-module discovery discards a full parse that source admission repeats
- Category: re-deriving
- Where: [logical inventory parsing](omega-rust/omega/packages/package-compilation/src/source/source_imports/logical.rs#L185), [inventory output](omega-rust/omega/packages/package-compilation/src/source/source_imports/logical.rs#L211), [selected-source parsing](omega-rust/omega/packages/package-compilation/src/source/source_assembly.rs#L949)
- Saw: discovery runs the ordinary lexer/parser over the owner's eligible captured files, retains only each path and module name, and drops the syntax. Selected files are then parsed again after admission; source reselection also parses under fresh consumer coordinates. Those fresh coordinates protect source ownership and selection ledgers, but the syntax work is repeated over unchanged bytes.
- Cost: Selected files pay for syntax construction twice. Retain immutable syntax and a module/import index within the invocation, separately from semantic owner, scope, stratum and bindings. Pin parse counts, distinct Build/Product identities, changed-input behavior and diagnostic locations. Release unused discovery syntax when ownership permits and measure retained memory against avoided parsing; do not add a persistent cache. See the [draft syntax retention design](wiki/drafts/designs/compiler_work_ownership.md#actionable-repair-references).
- Reported: 2026-11-14, Codex, while comparing Squalr-inspired pipeline proposals at `c445e89a1eead`

### Completing selected bodies can copy the retained typed program
- Category: re-deriving
- Where: [retained-body completion](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L810), [shared typing loop](omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1596), [arena cloning](omega-rust/psi/foundation/arena/src/arena.rs#L624)
- Saw: `SeededTypingBase` holds `Arc<TypedTrees>`, but body completion calls `Arc::unwrap_or_clone(retained.typed)` and feeds the candidate to one mutable `Lowerer`. When another snapshot retains that Arc, cloning copies owned arena vectors even if only a small selected body set changes. Sharing the outer Arc does not make the stored tables independently owned.
- Cost: A small continuation can copy unrelated typed storage and cannot return independent body output. Separate immutable declaration/type storage from owned body tables; publish selected replacements through stable handles without flattening unrelated bodies. A shared-snapshot continuation must retain untouched storage, exact authority/context identities and refusal behavior. Measure copied bytes and peak memory; keep signature and inferred-summary dependencies explicit. Defer wide Psi 02–04 representation splits while `LOWERED-FORM-CHECKER` coverage is actively churning those stages unless a measured continuation bottleneck requires it earlier. See the [draft body ownership design](wiki/drafts/designs/compiler_work_ownership.md#representation-and-execution-contract).
- Reported: 2026-11-14, Codex (refined by Antigravity at `2e5ca2da570`), while comparing Squalr-inspired pipeline proposals

### The standard library declares two public things named `DarwinClock`
- Category: one idea, several implementations
- Where: [source/library/std/macos_gui.omg:586](source/library/std/macos_gui.omg#L586) (`public boundary trait DarwinClock`) and [source/library/std/targets/macos_arm64/clock_impl.omg:26](source/library/std/targets/macos_arm64/clock_impl.omg#L26) (`public DarwinClock satisfies Clock`)
- Saw: `use omega_language_std::DarwinClock;` is refused as selecting no one declaration (modules.md, "Resolution": "Ambiguous imported declarations reject"). Five GUI samples (`samples/gui/window_app`, `image_viewer`, ...) write `invokes DarwinClock` and cannot name it; std's own Build names both meanings unqualified (`Build::select_provider<Clock, DarwinClock>` and `Build::bind<DarwinClock::clock_ns>`)
- Cost: no consumer can import either declaration, and which one an unqualified `DarwinClock` means inside std depends on the operand's role. One of the two needs another name
- Reported: 2026-11-14, Claude Code, while moving samples to declaration imports


### Whole-value structural domains are erased before the lowered checker
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/lower.rs` (structural parameter/result qualifications); `lower/places.rs` (structural value transport); `terminal-psi/src/types_and_values.rs` (`StructuralDomainDeclaration`, structural qualification fields)
- Saw: whole-value and projected structural qualification fields are still constructed empty, and the source route has no producer for the module's structural domain declarations. Predicates on a qualified record field can join its owner's invariant, but that does not retain the field's nominal structural domains or issuance relationship. Primitive scalar storage and scalar fields are a separate declared scalar type, not a substitute for these structural memberships.
- Cost: the sole lowered checker cannot independently contradict a missing or forged whole-value structural membership or reconstruct its establishment authority; carry the existing structural catalog and exact projected qualifications through their producer, storage and checker owners
- Reported: 2026-11-13, Claude Code, while stating domain predicates as contracts

### Data applied at a lifetime is one record whose stored views name only the data's own binders
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/read/types.rs` (`value_type`, the `Inner<'a>` arm), `read/places.rs` (`lifetime_binders`, `declared_field`), `read/bytes.rs` (`byte_view_type`)
- Saw: `Session<'a>` and bare `Session` are the same `ValueType::Record`, and the field `name: &'storage [u8]` has the type "view of `Session`'s `'storage`" in every value. So `long.name = short.name` between two `Session` values is accepted without anything relating the regions the two were applied at, and `with_alias<'s>(d: &mut Description<'s>, alias: &'s [u8]) { d.alias = alias; }` is refused as a type mismatch because the lifetime argument relates nothing. It is safe today only because a view of a formal's region can never enter a record at all
- Cost: the toolchain's own `dependency::with_alias` cannot be judged, and the first slice that lets a formal's view into a record (or forms a view of local storage) inherits an unchecked exchange between records of different regions. The type of a record value needs its lifetime arguments
- Reported: 2026-11-13, Claude Code, while reading machines with lifetime parameters

### Every view written without a lifetime is one type in every machine
- Category: a verdict with nothing behind it
- Where: `typed-trees/src/type_identity.rs` (`RegionForm::Elided`, no owner and no position); `05_typed-trees-to-terminal-psi/src/read/bytes.rs` (`source_byte_view`), `read/calls.rs` (`view_regions`)
- Saw: `&[u8]` renders one identity wherever it is written, so two formals without a lifetime have the same type and `pick(a: &[u8], b: &[u8]) -> &[u8] { b }` is accepted as a declaration, though lifetimes.md says "Ambiguous elision requires an explicit relation". The call is what refuses it, and only when the two views passed are of different regions
- Cost: the elision rule has no owner at the declaration; a reader of the callee alone cannot tell which input the result is related to, and the diagnostic lands on a caller
- Reported: 2026-11-13, Claude Code, while giving a call's lifetimes the regions of the views it passes


### `Console::exit_process` ends the process without publishing `reaches ProcessExit`
- Category: a verdict with nothing behind it
- Where: `source/library/std/console.omg:66` (the requirement, `reaches Console`) and `:96` (its provider); `omega-rust/psi/compilation/representations/typed-trees/src/declarations.rs` (`exact_process_exit_requirement` with `console_adapter`, `nominate_source_completion`)
- Saw: process_exit.md says "Any retained console-exit convenience adapter must obtain legitimate exit authority and publish its additional reach. Console must not inherit `ProcessExit` wholesale." The standard library's `Console::exit_process` declares `reaches Console` and nothing else, and the typed trees decide whether it completes externally from the trait being named `Console` plus a sealed intrinsic or a completion admission. Where neither is present, as in the route probe (`compiler/tests/route_probe.rs`, the `Console` program), the same source call is lowered as a call that returns
- Cost: a ceiling of `reaches Console` does not mean "cannot end the process", so package review's `ProcessExit` row undercounts; and the checker models one source call as returning or as a terminal transfer depending on an admission the program text does not show, so a body judged in one mode says nothing about the other
- Reported: 2026-11-13, Claude Code, while taking external completion (`ProcessExit::exit_process`) to the lowered-form checker

### Nothing makes a new reader site ask before it looks inside a qualified data value
- Category: a verdict with nothing behind it
- Where: `05_typed-trees-to-terminal-psi/src/read/` (`authority.rs`, and every `ValueType::Record(record, _)` pattern in `places.rs`, `expressions.rs`, `transitions.rs`, `arrays.rs`, `contracts.rs`, `statements.rs`)
- Saw: a type is a carrier and its domains, and about thirty sites match the carrier and discard the domains with `_`. The sites that take a value apart now call `authority::opened`, but the pattern still compiles anywhere, and two sites were typing a literal with whatever qualified type its context expected until this change (`arrays.rs` array literal, `places.rs` record literal). A store's destination is also read through the expression reader, so it asks read authority where only write authority is owed.
- Cost: the next site that projects, compares or constructs a record with `_` silently ignores a domain's reader, writer or issuer rule, and no test fails until someone writes the twin for that exact site
- Reported: 2026-11-13, Claude Code, while reading declared domains on data carriers

### Interface ownership still changes some imported-service checks
- Category: one idea, several implementations
- Where: `omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider/service_reach.rs:39`; `src/execution/host_imports/filesystem.rs:513`; `omega-rust/psi/compilation/terminal-installation/src/provider_and_target_installation.rs:451`
- Saw: native delegation removes the import's owning service from its declared reach before checking delegation (its unit permits `[owning_service]` with `Unprovided`); filesystem admission skips that same identity; assumed-binding operational compatibility ignores an extra supplied ceiling member when it equals the supplied owner. `wiki/spec/language/effects.md` distinguishes catalogue ownership from the requirement's explicit reach. These permission/compatibility checks must not be reused as proof that a declared ceiling is empty.
- Cost: the source boundary producer can retain a real catalogue owner even for `reaches none`, so downstream code must keep ownership, declared reach and actual delegated host permissions separate. This entry records the wider retained consumers; the current boundary work requires an explicit source ceiling and fixes the actual exclusion walk's implicit owner contribution, without treating these checks as source proof or expanding into an assumed-binding/delegation campaign.
- Reported: 2026-11-13, Codex, while carrying selected entry and ordinary Build boundary calls through the surviving lowered route

### "Does every selected external realization have a declaration" is checked two ways, and they disagree on unused ones
- Category: one idea, several implementations
- Where: `omega-rust/omega/compilation/realization/provider-admission/src/compiler_builtins/retained.rs:287` (`validate_external_provider_owners`) and `:295` (`validate_retained_external_provider_owners`); caller `omega-rust/omega/compilation/representations/abstract-operations/verified-optimization-input/src/native_proposal.rs:338`
- Saw: the first demands an `ExternalProviderConformance` row for every selected realization, the second only for requirements the module retains as a boundary; a six-machine scalar program with no float rows builds natively from the default compile, yet the same module is refused ("`NativeFloats` has no retained external implementation for `Float::add`") when a harness hands the proposal the provider declarations prepared for the old program.
- Cost: proposal validation has two different retained-demand contracts. The Check-mode trial requires the 146 float-table rows while the earlier default scalar compile did not read them; treating the former as the default demand can force unrelated metadata into the product path. The all-catalog investigation is parked while the actual default consumer is reconciled.
- Reported: 2026-11-13, Claude Code, while carrying the lowered-form checker's published module into Terminal production

### Constructing the source compiler repeats an hour of checking before its tests
- Category: verifying costs more than writing
- Where: [tests/product/gate.sh:42](tests/product/gate.sh#L42), [tests/product/gate.sh:70](tests/product/gate.sh#L70)
- Saw: frozen Rust producer b520 checked the 269-unit, 3.8 MB compiler in 4,089 seconds during update; native construction then took 3,796 seconds, including 3,227 seconds in Psi 05, with a peak near 19 GiB; all 191 product cases took 80 seconds afterward
- Cost: a source repair spends hours in producer checking before its executable cases run; these measurements belong to the old checker being removed, and cannot stand for seed-chain or replacement-checker performance
- Reported: 2026-11-12, Codex, while finishing bootstrap compiler construction and storage PRs #2782 and #2788

### Stage 05 of the Omega pipeline hands out "Validated" homes and "validation receipts" with no replay behind them
- Category: a verdict with nothing behind it
- Where: `omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes/src/home_assignment.rs:1-6` and `:2628-2640`, `.../src/fixed_view_copy_recovery.rs:1-6` and `:1095-1116`, `.../src/runtime_spill_and_split.rs:530`, `:3224`, `:5460`
- Saw: both file headers say the result is "independently" replayed; `assign_register_homes_reusing` computes the plan and builds `RegisterHomeValidationReceipt` from that same plan's hash and counts; `seal_validation` does the same for fixed-view copies; the three `ReplayMismatch` error variants are declared and never constructed
- Cost: a reader of the headers or the type names believes register homes are checked twice, and a wrong home assignment has nothing between it and encoding
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### Unchecked arena lookups still conflate stale handles with the zero sentinel
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/foundation/arena/src/arena.rs` (`get`, `get_mut`, `span_or_empty`); `paged.rs` (corresponding paged lookups)
- Saw: unchecked lookups return the shared default or an empty slice for unresolved nonzero handles; mutable lookups can change that default. Handle zero deliberately names absence, including its shared default; generation checks alone cannot establish ownership between arenas using the same generation.
- Cost: consumers without an owning-boundary validation can interpret a stale handle as a missing declaration or mutate the shared default. Those consumers need recoverable validation before using these infallible APIs, without changing legitimate zero-sentinel behavior.
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### The Terminal interpreter reports a failed run-time check as a malformed artifact, with one error for 1,329 sites
- Category: a verdict with nothing behind it
- Where: `omega-rust/psi/interpreter/src/boundary_values.rs:696`; example at `engine_execution.rs:3691`
- Saw: `malformed()` returns `VerifiedOperationMalformed` and is called 1,329 times across 15 files; an exact add, multiply or divide whose result does not fit ends in `.ok_or(malformed())?`; a program crash is built on only 7 lines (trapping integer, trapping float, crash terminator, host effect); no crate outside the interpreter names the variant
- Cost: an overflow or zero divisor that native code would trap on shows up as "the verified artifact is malformed" with no reason, so a native and Psi disagreement reads as an interpreter failure and the cause must be found by hand
- Reported: 2026-11-12, Claude Code, while reading the compiler atlas against main

### A whole-program stage refuses every program for a library construct nobody calls
- Category: a verdict with nothing behind it
- Where: `03_symbol-resolved-trees-to-typed-trees` (the remaining `unavailable_judgment` callers, such as "mathematical call receiving types" and "selected call parameter types" in `call_and_product_selections.rs` and `typing_queries.rs`); `build-time-evaluation/src/source_frontier.rs` (`source_jobs`: the side tables it still refuses whole, such as constant recipes and wire plans); any by-name refusal added to `04_typed-trees-to-typed-trees`
- Saw: these stages see the program together with all of `core` and `std` and have no notion of what an entry demands, so one such construct in an uncalled library declaration stops a program that never reaches it; the gates the route probe met (unbound calls, assignment literals, constructor field types, layout-policy and computed types) now record the refusal on the declaration instead (`typed-trees/src/deferred_refusals.rs`), and the others still refuse the program
- Cost: each remaining gate must be served for the entire library before any program compiles, or be moved onto the same carrier one by one as a program meets it
- Reported: 2026-11-13, Claude Code, while getting the route probe through typing

### The parser still accepts a boundary requirement with no `crashes` clause
- Category: a verdict with nothing behind it
- Where: `01_tokens-to-syntax-trees/src/declarations.rs` (refuses a boundary requirement without `reaches`, says nothing about `crashes`); `05_typed-trees-to-terminal-psi/src/read/calls.rs` (`boundary`); `samples/` and `tests/fixtures/`
- Saw: effects.md says a boundary requirement "must likewise write `crashes`". Only the lowered-form checker enforces it, and only at a call. The bundled library now writes the clause on all 400 of its requirements, but 1,745 requirements in 1,062 files under `samples/` and 70 in 40 files under `tests/fixtures/` do not, so the parser cannot refuse the omission until those are migrated, each to the ceiling its providers really have
- Cost: a library or program author learns that the clause is missing only when some caller is refused, and 30 library requirements with no provider and no caller carry `crashes Trap crashes Abort` because nothing in the repository says what narrower ceiling their owner intends
- Reported: 2026-11-13, Claude Code, while writing the bundled library's crash ceilings

### Typing lets a bare literal be stored into a field declared in a domain
- Category: one idea built twice
- Where: `03_symbol-resolved-trees-to-typed-trees` (assignment typing); `05_typed-trees-to-terminal-psi/src/read/rejections.rs` (`mistyped`)
- Saw: `Sealed { seal: Seal { } }` is refused by typing ("record literal field type") because a literal is the bare carrier, but `builder.seal = BuildSeal { };`, a plain argument for a qualified parameter and a plain value for a qualified result all pass typing. The lowered-form reader now rejects each by the rule (one carrier, two domain sets), so nothing is accepted, but typing checks the rule for one form and the reader for all of them
- Cost: "the supplied expression's static type must carry exactly the domains the destination declares" has two owners with different coverage and different wording, so the same mistake reads differently by where it is written
- Reported: 2026-11-13, Claude Code, while declaring the seal that protects the compiler-owned `Build`

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

### Nominal const-domain bounds recover data properties by a name scan
- Category: identity re-derived by a consumer
- Where: Stage 02 `preparation_const_evaluation.rs`, `carrier_satisfies_declared_bounds`
- Saw: the non-scalar branch receives a carrier display string and takes the first root data header with that name; the selected nominal declaration does not accompany this property query
- Cost: generic-domain bound checking cannot rejoin the declaration that owns the value and instead depends on root order when package instances contain same-named data
- Reported: 2026-11-14, Codex worker-22, while preserving integer metadata through constant evaluation
