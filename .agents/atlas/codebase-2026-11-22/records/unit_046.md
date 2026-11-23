# Record: unit_046 (area 07-psi-semantics)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths below are under
`source-rust/subsystems/compiler/psi/semantics/src/descriptors/` (call it `D/`) unless shown otherwise.
Everything is in crate `terminal-semantics`. All 37 ranges in `units/unit_046.tsv` were read in full (22,0xx lines).

Vocabulary used here (defined once): a **descriptor** is a view/slice/reference value that points at storage it
does not own; a **slot** is a location (`DescriptorLocation` = root place + path) that holds a descriptor; a
**flow step** (`DescriptorFlowStep`) is one row of a machine's storage-flow graph; a **cut** is a position
`(block, index)` in that graph; a **payload contract** (`PayloadMembershipContract`) is the set of domain
memberships and scalar facts known about the contents of a place at a cut.

---

## D/descriptor_results.rs (1846 lines)

**Purpose.** Root module of `descriptor_results`. It re-exports the whole descriptor-results surface (`:1-98`),
defines the origin enums, the per-machine storage-flow graph type `DescriptorMachine`, the call-invocation
rows, the result-slot tree, the graph-construction entry points, and the error type for graph failure.

**Shape.**
- `DescriptorResultOrigin` (`:101`), `DescriptorCallOrigin` (`:118`; Invocation / AccessFormation / Empty / Argument / Literal), `DescriptorSourceOrigin` (`:171`; Provider / Backing / IncomingSlot / IncomingContents), `DescriptorProviderOrigin<D>` (`:213`), `DescriptorLifetimeDebt` (`:222`), `DescriptorInvocationSite` (`:228`).
- `DescriptorMachine` (`:447`): ~30 fields, all tables keyed by `DescriptorPosition` (`BTreeMap`). Key ones: `flow`, `places` (sorted by place id), `place_types`, `originations`, `projections`, `contents_queries`, `contents_transfers`, `payload_producers`, `payload_entry_predicates`, `scalar_payload_reads/writes`, `payload_invalidations`, `payload_edge_predicates`, `payload_scalar_bindings`, `unresolved_contents_writes`, `storage_derivations`, `calls` (`Option<Vec<DescriptorInvocation>>`; comment `:484` "Missing is not a call; None is a real invocation whose effects cannot be represented"), `normal_exits`.
- Methods on it: `finish_index_literals` (`:522`), `export_input_projection` (`:533`), `calls_at`/`direct_call`/`invocation`/`insert_call`/`validate_invocation_owners` (`:544-637`), `producer` (`:654`), `sequence_construction` (`:681`), `resolve_location`/`location_capture`/`location_type`/`location_path` (`:737-794`).
- `insert_invocation` (`:490`), `DescriptorInvocation` (`:854`) and `DescriptorInvocationTarget` (`:863`; Machine / Provider / Public), `DescriptorResultSlot` (`:969`) with `canonicalize`, `matching`, `origins_at`, `merge`, `visit`; `DescriptorPlace` (`:1095`), `DescriptorDefinition` (`:1103`).
- Graph builders: `terminal_graph` (`:1214`), `terminal_graph_from` (`:1218`), `terminal_borrowed_storage_graph[_from|_with_nominal_calls]` (`:1225-1253,1666`), `terminal_reference_graph[_from]` (`:1655,1662`), `terminal_access_origin_graph` (`:1677`), `terminal_graph_closure/rows/for` (`:1502,1539,1684`), `terminal_machine_roster` (`:1515`), `terminal_definition` (`:1723`).
- Query wrappers: `result_origins` (`:306`), `return_origins` (`:323`), `call_origins[_in]` (`:408,422`), `source_origins`, `stored_effects`, `call_stored_effects[_in]` (`:1785-1828`), `operation_position` (`:1829`).
- Type predicates: `is_descriptor` (`:235`), `descriptor_value_access` (`:246`), `is_shared_descriptor` (`:279`), `result_access` (`:298`), `external_backing` (`:1144`), `aggregate_descriptor_slots` (`:1194`), `zeroed_case_predicates` (`:803`), `native_code_layout_regions` (`:141`).
- `DescriptorGraphError` (`:1256`; MissingOrAmbiguousMachine / IncompleteMachine{machine, position, successor, detail}) with `at_operation` (`:1272`) and a large `in_module` formatter (`:1343-1469`).

**Inputs/outputs.** Reads `TerminalModule` (machines, `types.structural_types`, `interfaces.boundary_machines`). Builders call `terminal_flow::adapt` (`:1572,1641,1694,1709`) per machine and return `DescriptorMachine` rows; query wrappers build a `DescriptorAnalysis` from those rows and return origins.

**How it decides.**
- Identity: machines are found by `MachineId` through a sorted roster; a duplicate id is `MissingOrAmbiguousMachine` (`:1520`). `descriptor_machine` is a linear `iter().find` (`:844`).
- `is_descriptor`/`is_shared_descriptor` accept only when exactly one declaration has the type id (`:239-243,283-295`); `exactly_one` is used for lookups throughout.
- `result_origins`/`return_origins`/`call_origins_in` apply only to a result that is Unrestricted multiplicity and a shared descriptor (`:311-313,329-331,435-437`).
- `insert_invocation` refuses to mix an `Operation`/`InvokeNoNormal` owner with any other owner at one position (`:499-511`) and refuses a duplicate owner (`:512-514`).
- `native_code_layout_regions` keys on shape: record with exactly 2 fields whose second field is a `Structural` `ElementView` with shared access (`:145-163`); returns the second field's path.
- `terminal_graph_for` fails the whole graph (`.collect::<Option<_>>()?`, `:1697`) if any machine fails to adapt; `terminal_graph_rows` keeps per-machine `Result`s (`:1539`, comment `:1536` "An unsupported body must not erase useful analyses of independent bodies").

**Cases by shape.**
- `native_code_layout_regions` (`:141-168`): exactly the 2-field record shape above.
- `external_backing` (`:1144-1192`): a fixed list of shared-borrow reference / element view / borrowed byte view shapes, at the path tip or at the named record field.
- `terminal_definition` (`:1723-1783`): `StructuralPlaceKind` Parameter / ByteSequenceLiteral / BlockParameter / OperationResult explicit; `_ => Some(Local)` (`:1781`).
- `DescriptorGraphError::in_module` hard-lists terminator kinds (`:1372-1383`) for its message.
- `terminal_graph_rows_with_nominal_calls` runs a second `adapt` pass only when some machine has an `OperationKind::CallDynamic` (`:1611-1623`), passing the first-pass graph as `selector_graph`.

**Same job elsewhere.** `is_descriptor` (`:235`) and `is_shared_descriptor` (`:279`) duplicate a shared "exactly one declaration" scan that `exactly_one` provides (used at `:251,1199`); `D/replay.rs:440` `DescriptorSlotKind::Views.is_slot` calls `is_descriptor`. Three graph-builder families (`terminal_graph*`, `terminal_graph_rows*`, `terminal_graph_closure`) repeat the machine-roster/adapt loop (`:1502-1721`).

**Unused / one-caller code (grepped, `source-rust/`).**
- `native_code_layout_regions`: 3 call sites (`terminal_flow.rs:895`, `optimization-unit-semantics/src/descriptor_results.rs:1878`, its own definition) (command: Grep `native_code_layout_regions` over `source-rust`).
- `reconstruct_return_slots_external`: 1 call site outside its file (`optimization-unit-semantics/src/descriptor_results.rs:1324`).
- `aggregate_descriptor_slots`: 1 call site (`optimization-unit-semantics/src/unit_validation.rs:7530`).
- `terminal_access_origin_graph`: 2 call sites (`provider-admission/src/compiler_builtins.rs:642`, `interpreter/instructions/execution.rs:653`).
- `terminal_residence_rules`: 1 call site (`plain_affine.rs:2151`).
- Several callers are in `psi-to-native/representations/.../optimization-unit-semantics`, i.e. native code reaching into this Psi-side crate (read, grepped).

**Stated limits.** `:1536` comment on unsupported bodies (above); `:484` call rows. No "unsupported" wording otherwise.

**Fail-open risks.**
- `terminal_graph`, `terminal_reference_graph`, `terminal_borrowed_storage_graph` return `Vec::new()` on any adapt failure (`:1215,1663,1667`, `.unwrap_or_default()`). A caller that treats "machine not found in the returned graph" as "nothing to check" would accept a program whose graph failed. Classification: open (not traced); the in-crate wrappers `result_origins`/`return_origins`/`call_origins` return `None` for a missing machine (`:335`), so those are closed.
- `terminal_definition` `_ => Some(Local)` (`:1781`): any place kind not listed is treated as a local with independent storage. Open.
- `is_descriptor` returns `false` for an unknown or duplicated type id (`:240-243`); `has_independent_storage`-style consumers then treat the value as not a descriptor. Open.

---

## D/descriptor_results/boundary_calls.rs (130)

**Purpose.** Decides whether a `BoundaryCall` (a call across a published boundary) leaves the descriptor storage of its arguments intact, so the caller need not treat its loans as escaped.

**Shape.** `BoundaryCompletionResult` (`:14`), `boundary_call_preserves_descriptor_storage` (`:20`, takes module/machine/operation), `boundary_completion_preserves_descriptor_storage` (`:58`, generic over types iterator and a `place_type` closure).

**Inputs/outputs.** Reads the boundary declaration (`exactly_one` on `interfaces.boundary_machines`, `:34`), operation result, argument list; returns `bool`.

**How it decides.** Refuses (false) on: completion receipts (`:66`), argument-count mismatch (`:67`), any `RetainedBorrow` content guarantee (`:68-70`), result kind mismatch, or a structural result that is not `plain_owned_shape` (or an owned shared-byte descriptor shape when `result_lifetimes.is_some()`, `:82-84`). Per argument: access must equal the parameter access (`:95`); SharedBorrow arguments pass (`:98`); an Owned argument passes only for owned shared-byte descriptor shape with declared result lifetimes (`:114-115`); other arguments need a borrowed byte view or an element view whose element is `plain_owned_shape` (`:117-125`), else `plain_owned_shape` of the selected type (`:128`). Keyed on type identity and access, not names.

**Cases by shape.** Hard-coded: `Unit`/`Scalar`/`Structural` result pairs (`:71-87`); path-empty byte view / element view cases (`:117-127`).

**Same job elsewhere.** The conservatism flag `mark_opaque_call_occupancy` in `terminal_flow.rs:2487` consumes this answer (`terminal_flow.rs:1850`).

**Unused/one-caller.** `boundary_call_preserves_descriptor_storage`: caller `terminal_flow.rs:1850`; also re-exported in `descriptor_results.rs:22-25`. `boundary_completion_preserves_descriptor_storage`: re-exported; other call sites not grepped (suspected, `references/boundary_results.rs`).

**Stated limits.** None.

**Fail-open risks.** `place_type(argument.place)` returning `None` gives false (`:101-103`), closed. SharedBorrow arguments always pass (`:98`) regardless of type: traced; shared borrows cannot write, comment-free. Classification: traced.

---

## D/descriptor_results/compatibility.rs (145)

**Purpose.** Computes, bottom-up through calls, which loans a callee requires its callers to keep apart from a demanded argument region, and refuses when a caller's actual backing overlaps the demand.

**Shape.** `DescriptorCompatibilityRequirement {position, loan: DescriptorSourceOrigin, demand: DescriptorCallOrigin}` (`:5`), `DescriptorCompatibilityFailure {machine, position}` (`:12`), `DescriptorAnalysis::compatibility_requirements` (`:18`).

**Inputs/outputs.** Takes a map machine -> requirement rows; calls `compatibility_ingress` (in `replay/analysis.rs:428`), `substitute_call_storage_at`, `substitute_stored_effect_source_at`; returns the closed map of requirements or a failure naming machine+position. Fixpoint loop `:51-143`.

**How it decides.** Per call edge: demand must substitute to an exact `Argument` projection (`:67-76`: `exact`, no `indirect`, no `window`, no `captured_at`); every substituted loan origin is classified: `Provider` -> fail (`:93`), `AccessFormation`/`Invocation` backing -> fail (`:94-97`), `Empty`/`Literal` -> fine (`:98-101`), `Argument` -> fail if `Region::overlaps` demanded (`:102-113`), `IncomingSlot`/`IncomingContents` -> propagate as a new requirement upward (`:114-129`). Keyed on region overlap of (root, projection).

**Cases by shape.** The five-way `match loan` (`:92-130`).

**Same job elsewhere.** None seen in the unit.

**Unused/one-caller.** `compatibility_requirements` caller outside this unit not grepped (suspected: loan checker in `references/`).

**Stated limits.** None.

**Fail-open risks.** `Empty`/`Literal` loans are silently accepted (`:98-101`) with no comment; reasoning is that static or empty origins cannot overlap a demand. Classification: open.

---

## D/descriptor_results/contents.rs (1498)

**Purpose.** Builds and validates *contents queries* (a typed path of address steps and element steps from a root place to stored contents), derives payload membership contracts for such queries, and implements the transfers and membership presentation rules for stores, copies and references.

**Shape.** `DescriptorContentsTransfer` (`:15`), `DescriptorContentsOperand` (`:63`), `descriptor_contents_transfers` (`:70`), `DescriptorFlowStep::record_selected_copy` (`:24`), `DescriptorContentsQuery` impl (`:183-444`: `retains_selector`, `without_selectors`, `is_symbolic_location`, `inline_steps`, `exact_location`, `from_location`, `observation`, `selected_type[_using]`, `validate[_using]`, `projected`, `conservative_projection`), `descriptor_contents_query[_with]` (`:446,590`), `descriptor_storage_use_source` (`:456`), `inline_selected_index` (`:642`), `entry_selector` (`:659`), `descriptor_scalar_arguments` (`:681`), `DescriptorStoreFlow` and `descriptor_leaf_store` (`:703,713`), `DescriptorPayloadTarget` (`:782`), `PayloadMembershipContract` (`:803`; fields `scalar_observations, scalar_requirements, carrier, qualifications, projected_qualifications`) with `presented` (`:812`), `select_payload_memberships` (`:836`), `payload_memberships` (`:878`), `reference_backing_memberships` (`:931`), `reference_payload_memberships` (`:980`), `contents_membership_path` (`:1074`), `close_predicate_memberships` (`:1108`), `admit_declared_scalar_refinement` (`:1157`), `rewrite_subject_as_refinement_value` (`:1200`), `normalize_boolean_comparison` (`:1303`), `implied_predicate_memberships` (`:1345`), `byte_predicate_memberships` (`:1384`), `terminal_payload_contract` (`:1399`), `ElementPayloadMemberships` and `element_payload_memberships[_in]` (`:1447-1498`).

**Inputs/outputs.** Reads structural types, structural domains, `DescriptorAnalysis` (for `payload_targets`), a `source_contract` closure `PlaceId -> Option<PayloadMembershipContract>`. Produces queries, transfers (rows source -> destination), contracts (qualifications lists), or `Err(&'static str)` for reference backing.

**How it decides.**
- Query step kinds: `Address(segment)` or `Element{element, selector}`. `inline_steps` (`:221-273`) judges "stays inside the value's own storage" by segment kind and container shape: `Referent` = not inline (`:243`), `Case` = inline, `Field` = inline unless container is `AccessView`/`Accessor` record storage (`:245-256`), `FixedIndex`/`Element` = inline only in a `FixedArray` (`:257-262`).
- `selected_type_using` (`:353`) steps through `FixedArray` or `ElementView` only when the element type id equals the step's element (`:372-380`); `UnknownContents` only on a `TypeParameter` carrier (`:387-395`).
- `payload_memberships` (`:878`) intersects the contracts at all targets (`retain` on the common set, `:907-923`) and requires identical carrier types (`:908`); targets with `completed: Some(_)` return `None` (`:896-898`).
- `reference_payload_memberships` (`:980`) refuses with specific strings when carrier changes, ancestor memberships are lost, or targets disagree for non-shared access (`:1061-1065`).
- `admit_declared_scalar_refinement` (`:1157`) adds a domain membership when the domain's introduction predicate, after rewriting the subject to `ScalarRefinement::SUBJECT` and normalizing `Equal(true, cmp)` to a direct comparison, is `same_commutative_predicate` as the carrier's declared refinement (`:1188-1196`). This is a structural (term-shape) comparison.
- `entry_selector` (`:659-679`): accepts a scalar term only if every `Value` leaf is a machine parameter; any `Application`, `StructuralRead`, field read or `ViewExtent` returns `None`.
- `descriptor_storage_use_source` (`:552-584`): drops any query whose selected tip is a `ByteSequence` (comment `:548-551` "A byte descriptor's address does not identify the bytes an instruction observes").

**Cases by shape.**
- `incoming_memberships::select`, `contents.rs` fixed arrays: `length <= 4096` (`:54`) (see incoming_memberships entry).
- `descriptor_storage_use_source` handles `Place`, `Projection`, `Canonical`, `ObservedField`, `Element`; `Type(_)` is `unreachable!` (`:546`).
- `Element` use is recognized only for `ElementViewRead`/`ElementViewWrite` operations (`:510-522`).
- `rewrite_subject_as_refinement_value` handles `IntegerField`, `BooleanField`, `IeeeFloatField`, `StructuralRead` only with `Current` version, empty path and matching carrier (`:1211-1249`).
- `normalize_boolean_comparison` handles `IntegerEqual`, `IntegerLessThan`, `IntegerLessOrEqual` only (`:1312-1324`).

**Same job elsewhere.** `select_payload_memberships` (`:836`) vs `incoming_memberships::select` (separate file) both narrow a contract along a path; `PayloadMembershipContract` merge-by-intersection at `:907-923` is the same intersect pattern as `payload_history.rs` `State::meet` (`payload_history.rs:113`) and `payload_predicates.rs:26` `PayloadScalarObservation::intersect`.

**Unused/one-caller.** `terminal_payload_contract` (`:1399`): re-exported; internal callers `payload_history.rs:210`, `contents.rs:1470`. `payload_memberships`: callers `payload_history.rs:388,528,1145`, `contents.rs:1496` (read). Count of external callers not grepped.

**Stated limits.** `:1021-1030` messages are refusals not limits. `:99-102` "an unknown payload cannot supply a descriptor-slot query or justify a transfer through an unresolved address".

**Fail-open risks.**
- `entry_selector` returns `None` for stuff it does not recognize (closed). `rewrite_*`/`normalize_*` leave unrecognized terms alone (`_ => {}`, `:1297,1341`); the result then fails the structural compare, so closed.
- `terminal_payload_contract` (`:1399-1444`) returns the first matching operation result's declared qualifications; no check of uniqueness of the producing operation. Open.
- `record_selected_copy` sets `may_escape = true` when the query is unknown (`:39-42`): closed.
- None seen that accepts on missing data.

---

## D/descriptor_results/contents/incoming_memberships.rs (93)

**Purpose.** Narrows a payload contract through a contents query that includes `Element` steps (array or view elements), returning the contract shared by every element.

**Shape.** `select` (`:3`).

**How it decides.** Address prefix before first `Element` is applied via `select_payload_memberships` (`:13-21`); an `ElementView` carrier recurses with the `AllElements` path segment and clears scalar facts (`:29-50`); a `FixedArray` carrier with literal length `<= 4096` (`:54`) selects each index separately and intersects qualifications (`:56-89`), failing if the carriers differ (`:76`). Other carriers return `None` (`:91`).

**Cases by shape.** Exactly ElementView, FixedArray with literal length at most 4096.

**Stated limits.** `length <= 4096` (`:54`), no comment. Arrays longer than 4096 give `None` (refused).

**Fail-open risks.** None seen; over-limit and unknown carriers return `None`. For `length == 0`, the loop yields `common = None` (`:89`), so `None`: closed.

---

## D/descriptor_results/invocation.rs (427)

**Purpose.** Builds `DescriptorInvocation` rows for the three call kinds (module machine, public callable/dynamic dispatch, provider boundary) and derives writable slots and public result slots.

**Shape.** `DescriptorMachineSignature` (`:6`), `machine_invocation[_for_signature]` (`:12,36`), `DescriptorMachine::direct_invocation` (`:104`), `public_invocation` (`:151`), `public_result_slots` (`:227`), `public_input_origin` (`:311`), `writable_descriptor_slots` (`:362`), `selected_stored_effects_row` (`:384`), `public_realizations` (`:405`).

**How it decides.**
- `machine_invocation_for_signature`: actual and formal access must be equal (`:52`); every actual must have a root type (`actual_root_types.next()??`, `:55`); result must equal the callee's declared result in type, multiplicity, qualifications, and projected qualifications subset (`:79-86`).
- `public_invocation`: result must equal the declared result exactly (`:195-199`; note `projected_qualifications` compared with `!=` here, vs `all(contains)` at `:82-85` for machine calls).
- `public_result_slots` (`:227`): picks leaves by kind and `DescriptorSlotKind` (`:246-260`); every leaf must have at least one origin region or the whole result is `None` (`:300-302`).
- `public_input_origin` (`:311`): scans each prefix of the source path for the last view/descriptor/pointer crossing; origin is `IncomingContents` if one was crossed else `Parameter` (`:320-359`).
- `selected_stored_effects_row` (`:384`): requires the dispatch table, interface and slot to agree, `family_tuple` empty, and row count equal to slot count (`:393-401`).
- `public_realizations` (`:405`): collects distinct machines in the interface's tables at the slot.

**Cases by shape.** `direct_invocation` handles only `InvocationSite::Operation` and `InvokeNoNormal` (`:118-126`; `_ => return None`). `BoundaryMachineResult::Structural` only for public results (`:192-194,203-205`).

**Same job elsewhere.** The result-matching comparison at `:79-86` vs `:195-199` (different strictness for `projected_qualifications`) vs `provider_invocation.rs:138-141`.

**Unused/one-caller.** `selected_stored_effects_row`: caller `terminal_flow.rs:2593`; `machine_invocation` callers inside `terminal_flow.rs:2249`; others not grepped.

**Fail-open risks.** `public_invocation` for a call with a `None` result and a structural declared result returns `None` (`:203`): closed. None seen.

---

## D/descriptor_results/owned_shape.rs (125)

**Purpose.** Decides whether a type is "plain owned": made only of scalars, owned byte buffers and other owned types, so moving it exposes no nested references.

**Shape.** `plain_owned_shape` (`:7`), `owned_shared_byte_descriptor_shape` (`:14`), private `owned_shape` (`:21`) with a recursive `visit` and a `complete` memo and `active` cycle stack.

**How it decides.** Keyed on `StructuralTypeShape` variant and field types: Opaque, PrimitiveScalar, AtomicCell, owned byte sequences = owned (`:85-90`); shared-borrow byte view = owned only if `shared_byte_slots` (`:91-93`); `TypeParameter` = not owned (`:82`); `ElementView`, `Reference`, other byte carriers = not owned (`:94-96`); Record/Sum/Mixed = all non-erased fields owned (`:97-107`); FixedArray = element owned (`:108`). A cycle returns false (`:36-38`); unknown or duplicate type id returns false (`:39-43`). Erased fields must be `Erased` field type (`:46-48`).

**Cases by shape.** Full variant list at `:81-111` (no `_` arm).

**Same job elsewhere.** `external_backing` in `descriptor_results.rs:1144` classifies the same shapes from the other side.

**Unused/one-caller.** Re-exported at `descriptor_results.rs:80`; used by `boundary_calls.rs`, `terminal_flow.rs:2444`, `payload_versions.rs:760`. Counts outside the crate: grepped `plain_owned_shape` was not separately counted.

**Stated limits.** `:83-84` "Moving opaque contents exposes no nested references. Their claim and boundary contracts remain separate." (stated reason for accepting `Opaque`).

**Fail-open risks.** `Opaque => true` (`:85`): stated. `AtomicCell(_) => true` (`:87`): open (no comment).

---

## D/descriptor_results/payload_entry.rs (317)

**Purpose.** At machine entry, derives which structural-domain memberships already hold for each owned parameter by proving each domain's introduction predicate from the machine's `requires` clauses. It contains a small proof *search*: `prove` builds a certificate that is then checked by `proof_admission`.

**Shape.** `close` (`:7`), `prove` (`:117`), `prove_with_budget` (`:126`), `select` (`:203`), `bridge` (`:295`), `mod order` (`:5`), `mod tests` (`:316`).

**Inputs/outputs.** Reads `machine.payload_entry_predicates` (context + requirement propositions), domains, types; writes new entries into `contract.qualifications` / `projected_qualifications`.

**How it decides.**
- `close` walks the carrier type tree (fields of Structural type, and fixed arrays of literal length `<= 4096`) with budgets: 4096 visited nodes (`:18`), depth 64 (`:71`). For each eligible domain (no `content_projection`, no `establishment_routes`, `:24-28`), builds goal by substituting structural places (`:39-42`), calls `check_predicate_denotations_with_value_equalities` (`:43-49`), then `prove`, then `checked.check_certificate` (`:57`). Only a certificate the checker accepts adds the membership (`:60-69`).
- `prove_with_budget` (`:126`): budget 4096 steps (`:123`), depth 64 (`:134`); tries primitive judgments Truth / ReflexiveEquality / ClosedIntegerRelation / IntegerCarrierBound (`:137-150`), the integer-math lowering bridge (`:151-156`), each requirement as an assumption via `select` (`:157-172`), conjunction introduction (`:173-184`), disjunction introduction (`:185-199`). `select` additionally does conjunction elimination, disjunction elimination, and order derivation (`order::derive`).

**Cases by shape.** Fixed proof-rule list above; `select` and `bridge` are tied to `ProofRule` variants.

**Same job elsewhere.** `payload_predicates.rs:229-232` (`close_with_shared_requirements`) calls the same `prove` + `check_certificate` pair. `proof-admission` documents itself as running no proof search (`07-psi-semantics.md` crate map); this file performs search in the semantics crate (read; inferred conflict with that description).

**Unused/one-caller.** `prove` re-exported as `prove_payload_predicate` (`descriptor_results.rs:33`; Grep `prove_payload_predicate|payload_entry::prove|payload_entry::close` gave 4 files: `descriptor_results.rs:1`, `payload_history.rs:1`, `payload_predicates.rs:1`, `scalar_field_requirements.rs:1`).

**Stated limits.** Budget constants `4096`, `64` have no comments.

**Fail-open risks.** A proof that fails the checker is dropped (`:57-59`), so closed. Running out of budget ends the walk early (`:20-23`) leaving fewer memberships: closed. `if pending.len() >= remaining { break; }` (`:79,100`) silently stops queueing children, producing fewer facts: closed. None seen.

---

## D/descriptor_results/payload_entry/order.rs (151) and order/tests.rs (79)

**Purpose.** `derive` (`:4`) builds order-reasoning proof steps: `<=` and `<` conversion through `IntegerOrderDiscreteness`, and translation of a premise `a <= b` to a goal `x <= y` using `IntegerMathOrderTranslation`, `IntegerOrderSubstitution` and `IntegerLessOrEqualTransitivity`. `replace_endpoint` (`:97`) and `primitive` (`:134`) are helpers.

**How it decides.** Each constructed step for discreteness is self-checked with `proof_admission::check_certificate` (`:24-31`); other steps rely on the caller's final check. `primitive` only returns a node when `decide_primitive(ClosedIntegerRelation)` succeeds (`:138-143`). It calls `check_predicate_evaluation_size` (`:44`) as a size guard.

**Cases by shape.** Premise `LessOrEqual`/`IntegerMathLessOrEqual` against goal `IntegerMathLessOrEqual` (`:41,46-53`); anything else `None`.

**Tests (`order/tests.rs`).** One test: `exact_range_rearrangement_retains_subject_policy_and_premise` (`:7`): proves `a + b <= max` from `a <= max - b` with exact subtract; checks the certificate through `check_certificate` and `verify_bounded_certificate`; checks that a wrong subject and a wrapping subtract get `None` (`:62-78`).

**Fail-open risks.** none seen.

---

## D/descriptor_results/payload_entry/tests.rs (122)

Three tests: disjunction transport per alternative scope (`:5`), disjunction with one established alternative (`:48`), the finite-relation kernel bridge in both directions (`:88`). Each asserts both a proof and its `check_certificate` result, and that wrong goals return `None`. No separate purpose beyond covering `prove`.

---

## D/descriptor_results/payload_history.rs (1653)

**Purpose.** Computes, at a cut in a machine, the payload membership contract of a place by forward dataflow over the flow graph: which domain memberships and scalar facts survive each write, store, call, edge predicate and block arrival.

**Shape.** `PayloadContracts` (`:20`), `State` (`:63`; `values`, `scalar_requirements`, `shared_requirements`) with `publish_requirements`, `contract`, `meet` (`:70-170`); `CachedHistory` (`:173`), `HistoryReplay` (`:179`), `PayloadSources` (`:187`), `PayloadMembershipHistory` (`:220`; `new`, `for_current_contracts`, `current_selected_payload_contract[_in]`, `current_payload_contract[_in]`, `scalar_snapshot[_in]`), free functions `current_selected_payload_contract` (`:507`), `current_payload_contract` (`:547`), `completed_payload_contract` (`:580`), `payload_contract_at_cut` (`:635`, the engine), helper functions for premise lists (`:1299-1447`), `retire` (`:1449`), `replace_payload` (`:1480`), `observe_scalar` (`:1499`), `clear`/`clear_except` (`:1538-1558`), `append_scalar_observations` (`:1560`), `retire_scalar_instances` (`:1585`), `survives_payload_retirement` (`:1641`). Three cuts: `BeforeOperation`, `BeforeOperationWithDeclaredBytes`, `NormalCompletion` (`:573`).

**Inputs/outputs.** Reads `DescriptorMachine` fields (flow, `payload_*` tables, `storage_derivations`, `private_observation_inputs`), `DescriptorAnalysis` for `storage_writes`, `storage_origins`, `payload_targets`, `arrival_payload_targets`, structural domains. Output: a `PayloadMembershipContract` for the requested root at the position, or `Err(&'static str)`.

**How it decides.**
- Worklist forward analysis with a budget of 65,536 pops (`:718`). Join is intersection: `meet` (`:113`) drops values missing on one arrival (`:123-125`), drops a value whose carrier differs (`:126-128`), intersects memberships and observations.
- Writes: a known call with resolved writes retires memberships at each written path (`:753-777`); an unresolved call write `None` clears all values except `private_observation_inputs` (`:779-783`); non-call writes come from `payload_invalidations`, `None`/`Incoming`/`completed` targets clear everything (`:785-827`).
- `retire` (`:1449`) keeps a membership only if the write is strictly inside the subject and the domain is in the `content_independent` list (`:1459-1475`).
- A *strong* store (single destination, no overlap with sibling stores, `:855,866-876`) replaces the destination's memberships with the incoming value's presented memberships (`:877-895`).
- Edge arrival: bindings of structural formals select and present the actual's contract to the formal's carrier (`:1109-1158`); formals with a non-empty path abort the whole query (`:1111-1113`); scalar parameters of the target block are retired and rebound through `premises.arrival_requirements` (`:1179-1203`).
- Reachability prunes edges via `payload_reachability` so only edges that can reach the query cut propagate (`:656-666`).
- At `BeforeOperationWithDeclaredBytes`, byte-predicate memberships declared on the source are re-added (`:1260-1293`).
- Keys: place id, position `(block, index)`, structural-domain id, type id; premises compared by `Proposition` ordering (via `SharedPremise` ranks).

**Cases by shape.** Calls vs non-calls (`:752,785`); `DescriptorPayloadTarget` variants (`:794-821`); strong vs weak stores (`:855`); the three cuts.

**Same job elsewhere.** Forward must-hold flow with AND-join also in `payload_versions.rs:551` (`version_flow_from`), `payload_versions.rs:162` (`ScalarCaptureAnalysis::solve`), and `value_identity.rs` via `value_locations` (in `descriptor_lifetimes`, outside unit). Premise-list merge helpers (`extend_canonical_premises` `:1324`, `retain_common_snapshot` `:1359`, etc.) are used by `payload_predicates.rs:163`.

**Unused/one-caller.** `current_selected_payload_contract` / `current_payload_contract` free functions re-exported (`descriptor_results.rs:56-58`); `completed_payload_contract` is `pub(crate)` re-exported at `:35`. `PayloadMembershipHistory::scalar_snapshot` (non-`_in`) caller not grepped (suspected only external).

**Stated limits.** Failure strings with "finite budget" at `:655,720`; `:1111` "edge formal is not a whole payload root" (a refusal).

**Fail-open risks.**
- `retire` keeps `content_independent` memberships across a write inside the subject (`:1459-1465`): traced via test `content_writes_keep_only_existing_content_independent_memberships` (`tests.rs:473`), comment at `:1460` explains; stated.
- A store whose destination has no contract in `after.values` is skipped (`:878-880` `continue`): that destination had no facts, so nothing to retain. Traced by reading: closed.
- `header` with untypeable path removes all facts (`:903-905`): closed.
- `scalar_payload_writes` with `destinations.len() != 1` is skipped (`:1001-1003`): the old observation then remains unretired. Open: this path leaves a previous scalar observation in place when a write's destination resolves to several places. (Other paths at `:785-827` and `payload_invalidations` also fire for stores, which likely retires it; not traced for every operation kind.)

---

## D/descriptor_results/payload_history/entry_snapshots.rs (85)

**Purpose.** Collects, for each Owned parameter, an "entry scalar identity" per scalar leaf, so later reads can be related to the value at entry.

**Shape.** `collect` (`:10`), `for_parameter` (`:27`).

**How it decides.** Only `StructuralAccess::Owned` parameters (`:17`). Walks fields of `Structural` type and literal-length `FixedArray`; scalar leaves become `PayloadScalarObservation{value: None, entry: Some(...)}` (`:44-52`). Budget 4096 (`:34`), lineage depth 64 and cycle check (`:37`); on a cycle/too deep/too many it returns `None` for the whole parameter (the parameter then has no snapshots). `length > remaining` returns `None` (`:74`).

**Fail-open risks.** none seen (None means no snapshot).

---

## D/descriptor_results/payload_history/premise_table.rs (213) and premise_table/tests.rs (83)

**Purpose.** Interns scalar-premise propositions as shared handles ordered by an order-maintenance rank so that sorted merging of premise lists can compare handles by rank instead of structurally.

**Shape.** `SharedPremise(Arc<PremiseRow>)` (`:15`; row holds `proposition`, `values`, `closed`, `substitution_stable`, atomic `rank`), `PremiseTable` (`:80`, a `BTreeSet<SharedPremise>`), `intern`, `intern_ref`, `insert`, `free_rank`, `renumber`, `arrival_requirements` (`:154`). `RANK_SPACING = 1 << 20` (`:12`).

**How it decides.** `Ord for SharedPremise` compares ranks (`:73`); `PartialEq` compares pointer identity (`:62`). `BTreeSet<SharedPremise>` is searched with a `Proposition` through `Borrow` (`:86,129`), which is correct only while rank order equals `Proposition` order (tested at `tests.rs:12`). `arrival_requirements` drops a premise mentioning a block parameter that has no binding (`:168-176`), substitutes bound values, and appends `parameter == actual` equalities (`:192-206`).

**Unused/one-caller.** `PremiseTable` used in `payload_history.rs` only.

**Fail-open risks.** `renumber` uses `expect` (`:105`): panic if no gap remains after renumbering; not a fail-open. None seen. Tests: 2 (`tests.rs:11,51`).

---

## D/descriptor_results/payload_history/tests.rs (1059)

13 tests exercising `State`, `meet`, `retire`, `observe_scalar`, join/fork/retire behavior through a hand-built `DescriptorMachine` (`forked_history`, `:21`), plus `facts_after_an_element_store` (`:824`) building a store-through-slice-element machine. Notable assertions: analysis must be the same graph and inventory (`:102`); join keeps both-path facts but not a retired instance (`:163`); a store through one unknown element credits no element with the stored value (`:1036`); `Arc` sharing of untouched rows (`:359,412`). Fixture style: hand-constructed rows (no compiled `.omg`).

---

## D/descriptor_results/payload_predicates.rs (552)

**Purpose.** Defines scalar-observation types for payload contracts, closes a contract under domain predicates (proves a domain's introduction predicate from the contract's scalar facts), and builds typed scalar expressions for machine values.

**Shape.** `PayloadScalarObservation` (`:12`; `path`, `value: Option<ScalarTerm>`, `entry`), `PayloadEntryScalarIdentity` (`:19`), `intersect` (`:26`), `canonical_path` (`:50`), `observation_type` (`:100`), `close` (`:110`), `closure_may_change` (`:118`), `close_with_shared_requirements` (`:137`), `observed_byte_predicate` (`:259`), `terminal_scalar_literal` (`:321`), `scalar_payload_expression` (`:348`), `TerminalScalarExpressions` (`:385`; `new`, `scalar_types`, `scalar_type`, `unexpanded_operation`, `expression`), `terminal_scalar_snapshots` (`:499`), `introduction_predicate` (`:514`).

**How it decides.**
- `canonical_path` accepts only `Field`, `FixedIndex`, `Case`, `Referent` segments; `RepresentationView`, `RuntimeIndex`, `UnknownContents`, `Extent` give `None` (`:54-71`).
- `close_with_shared_requirements`: for each uninstalled domain (no content projection, no establishment routes), build the introduction predicate, add equations from observed values (Integer and Boolean only; `IeeeFloat` and `Callable` skipped `:211`), add computed byte-predicate truths, run `check_predicate_denotations_with_value_equalities`, then `prove` and `check_certificate` (`:223-233`). A domain is added only after the checker accepts.
- `observed_byte_predicate` (`:259`): requires a `FixedArray` with literal length, a byte-predicate carrier, observations for all `length` elements, each unique, typed, integer and in `u8` range (`:290-318`).
- `intersect` (`:26`): drops an observation unless exactly one matching row exists in `incoming`; differing `value`/`entry` is cleared.
- `scalar_payload_expression` (`:348`): expands value ids through `origin` up to `SCALAR_EXPRESSION_DEPTH`/`SCALAR_EXPRESSION_NODES` (in `substitution`, outside unit) and fails if a type mismatches.
- `TerminalScalarExpressions::expression` (`:464`): literal first, else the first definition's goal-free or proof-bearing leaf semantics `result_equation` of the form `Equal(_, right)`.
- `introduction_predicate` (`:514`): conjunction of each missing required domain's predicate, each domain must be closed (`domain_predicate_is_closed`) and have no content projection or establishment routes; else `None`.

**Cases by shape.** Scalar types handled in `close_with_shared_requirements` (`:204-212`); `unexpanded_operation` handles `IntegerConstant`, `BooleanConstant`, `ReadScalarApplication` directly, others via leaf semantics (`:424-457`).

**Same job elsewhere.** Admission of a declared scalar refinement in `contents.rs:1157` (`admit_declared_scalar_refinement`) and this closure both derive domain memberships from predicates, by structural equality in one case and by proof in the other.

**Unused/one-caller.** `terminal_scalar_snapshots`: `pub(crate)`, re-exported `descriptor_results.rs:59`. `unexpanded_operation`: callers not grepped (suspected external).

**Fail-open risks.**
- `index.definitions(value).next()` takes the first definition only (`:476-478`); if a value had several definitions this uses one. SSA-style single definition is assumed. Open.
- `expression` returns `(scalar_type, None)` (no denotation) for values lacking leaf semantics (`:494`), producing an opaque `Value` term; closed.
- None that accepts on missing data.

---

## D/descriptor_results/payload_predicates/tests.rs (179)

3 tests: byte-predicate observations need complete typed contents (`:10`), closure gate keeps normalization with no eligible domain (`:90`), closure gate for eligible/opaque/unprovable domains (`:111`).

---

## D/descriptor_results/payload_reachability.rs (272) and payload_reachability/tests.rs (284)

**Purpose.** Computes, for a query cut, the set of positions and edges that can reach it, while tracking Boolean conditions required of scalar values, and dropping edges whose predicate contradicts the demanded condition. Used to prune the payload history and entry-storage checks.

**Shape.** `Reachability{positions, edges}` (`:7`; `extension_frontier` test-only `:14`, `compatible_frontier` `:48`, `merge`, `contains`, `includes_edge`), `IncomingFlow` (`:102`), `incoming_flow` (`:105`), `predecessors[_with_incoming]` (`:121,129`), private `QueryFlow` (`:144`; `predecessors_with_incoming`, `before_edge`), `require` (`:244`).

**How it decides.** Backward worklist with budget 65,536 (`:168`). Per edge `before_edge`: rename conditions across block-parameter rebinding (`:208-223`), drop edge if the edge predicate `Equal(Boolean(p), term)` contradicts a demanded condition (`require` returns `None`, `:224-236`), retire demands on values defined at the source position (`:237-239`). `require` follows `BooleanNot` up to 64 levels (`:249`); a non-Boolean-valued term imposes no condition (`_ => return Some(())`, `:265`).

**Cases by shape.** `require` handles `Boolean(value)`, Boolean `Value`, `BooleanNot`; all else adds no demand.

**Fail-open risks.** `_ => return Some(())` (`:265,268`): unknown term shapes impose no demand, so more edges stay relevant. Reachability is over-approximated; at worst more joins reduce facts. Classification: traced (direction is conservative for a must-analysis).

**Tests.** 6 tests (`tests.rs:57,106,142,168,212,240`): distinct edges to one target, pre/post definition, simultaneous rebinding, back-edges, conflicting unknown successors, history extension frontiers.

---

## D/descriptor_results/payload_versions.rs (768) and payload_versions/tests.rs (691)

**Purpose.** Answers "is this stored value still the one produced/entered earlier?" by dataflow: a version bit that is true after the producer and false after any write that may overlap the storage, or arrival that replaces it. Also checks that a captured scalar SSA value was not renewed, and finds unchanged stored sources for a read.

**Shape.** `ScalarCaptureAnalysis` (`:33`, `new`, `require`, `require_on_edges`, `solve`), `ScalarCaptureFailure` (`:25`), `ProducedPayloadVersion` (`:213`), `unchanged_payload_producer` (`:218`), `require_unchanged_produced_payload[_in]` (`:259,281`), `require_version` (`:298`), `require_unchanged_observed_storage` (`:320`), `require_unchanged_between` (`:343`), `require_unchanged_entry_storage[_with_incoming]` (`:360,383`), `require_unchanged_from` (`:413`), `require_completed_scalar_capture` (`:511`), `version_flow_from` (`:551`), `unchanged_stored_scalar_sources` / `unchanged_stored_sources` / `unchanged_stored_projection_sources_in` / `projected_stored_sources_in` (`:620-748`), `direct_owned_storage` (`:750`).

**How it decides.**
- `version_flow_from` (`:551-618`): a boolean "still the same version" per position, joined with AND (`:608`), starting at entry (true for `VersionStart::Entry`, false otherwise) and set true after the completed producer (`:586`). `modifies` is consulted lazily and cached per position (`:590-597`).
- `require_unchanged_from` (`:413`) treats a position as modifying if: `unresolved_contents_writes` contains it (`:450`), a step definition overlaps the home (`:453`), a `payload_invalidations` query is unknown or its targets overlap (or are `Incoming`/`completed`) (`:456-480`), or a derivation destination's resolved storage overlaps (`:481-493`; unresolved destination counts as modifying `:485-488`). An arrival that rebinds the home as a block parameter replaces the version (`:496-500`). Overlap is `contains` in either direction on `(root, path)` (`:441`).
- `unchanged_payload_producer` requires exactly one `Storage{completed: None}` target and exactly one whole-value producer for that root (`:231-251`).
- `ScalarCaptureAnalysis::solve` (`:162`): same AND-join but over SSA value ids: false after a definition of a tracked value or a rebinding parameter.
- Query budget 65,536 cached answers (`:129`).

**Cases by shape.** Target variants `Storage{completed: None}` accepted, `completed: Some` rejected (`:436-438`), `Incoming` uses the root as the home (`:439`).

**Same job elsewhere.** The AND-join dataflow loop appears in `payload_history.rs:719-1239`, here `:175-207` and `:580-616` (three copies of "worklist + join by AND + requeue on change").

**Unused/one-caller.** `ScalarCaptureAnalysis::new`: callers inside this file (`:525`) and `tests.rs`; external callers not grepped. `require_unchanged_entry_storage_with_incoming`: caller `replay/analysis.rs:933`.

**Stated limits.** `:279` doc: "A missing or changed version is an error, including an unresolved write on any arrival."

**Fail-open risks.** None seen: unresolved targets and unknown positions return `Ok(true)` from `modifies` (modified) or `Err`. `source == observation` returns `Err(NotCompleted)` (`:107`).

**Tests.** 13 tests in `tests.rs` over a `copy_graph` fixture (`:21`), including loop completion (`:664`), diamonds (`:584,617`), entry refusals (`:96`).

---

## D/descriptor_results/private_parameters.rs (145)

**Purpose.** Finds owned scalar-record parameters that are never exposed (never named by any storage use except a whole move) so a scalar-only boundary call cannot write them; those roots keep their facts across the call.

**Shape.** `PrivateObservationInputs{observations, roots}` (`:12`, method `at`), `across_closed_observations` (`:27`).

**How it decides.** An observation is a `BoundaryCall` with empty arguments, no receipts, no service reach, no content guarantees, no reference outputs or regions, a unit or scalar result, and a declaration that neither may suspend nor block (`:47-66`). A root is private if it is an Owned parameter of a record whose fields are all `Scalar` (`:83-95`) and no operation or terminator other than a whole-value owned `MoveStructuralValue` has a storage use on it (`:104-131`). Result: sorted observation positions and root list.

**Cases by shape.** Record with all `StructuralFieldType::Scalar` fields only (`:86-94`; comment `:81` "Handles and views may reach mutable state outside this invocation.").

**Stated limits.** Module doc `:1-2`: "Other stores and other callers continue to use the ordinary conservative write frame."

**Fail-open risks.** An observation is accepted only on the long conjunction at `:47-66`; if `module.boundary_machine(*boundary)` is `None` it `continue`s (not observed, closed). The facts retained across such a call are claimed unwritten because the call has no arguments and no reach: traced, stated at `:1-2`. This is a claim about the boundary declaration, not about the program; if a boundary declares no arguments but actually writes global state this would keep facts on private roots, which the callee cannot name. Classification: stated.

---

## D/descriptor_results/provider_invocation.rs (159)

**Purpose.** Builds the invocation row for a provider boundary call: stored effects on its parameters from `reference_outputs`, and result slots, with lifetime dependencies.

**Shape.** `provider_outputs_cover_writable_slots` (`:5`), `provider_invocation` (`:55`).

**How it decides.** `provider_outputs_cover_writable_slots` requires, for each parameter, that the number of writable slots (References + Views kinds) equals the number of declared outputs for that parameter and that every slot is matched by exactly one output (`:34-51`). `provider_invocation` refuses on arity mismatch or `AccessOrigins` slot kind (`:64-67`), on any access mismatch (`:73`), unknown root type (`:76`), unresolved output path (`:103`), failed lifetime-debt derivation (`:110-115`). Outputs whose slot is not a slot of the owned inventory are skipped (`continue`, `:104-109`).

**Cases by shape.** `BoundaryReferenceDestination::Parameter` is the only destination (match has one arm, `:95-102,123-135`).

**Unused/one-caller.** `provider_outputs_cover_writable_slots`: 1 external caller (`references/boundary_results.rs`, from combined grep count `1`); plus `provider-admission/src/compiler_builtins.rs` count `1` for the same combined pattern (combined pattern in one Grep; per-name split not done). `provider_invocation`: caller `terminal_flow.rs:1875`.

**Fail-open risks.** `if !owned_slots(...).contains(&location) { continue; }` (`:104-109`) drops an output that is not a slot of the parameter instead of failing; the output then contributes no stored effect. Open.

---

## D/descriptor_results/scalar_captures.rs (510) and scalar_captures/tests.rs (2707)

**Purpose.** (a) `TerminalScalarCaptures::new` records, per cut, which scalar values were written into which storage and which scalar operands arrive at each block parameter, with validation of dense coordinates and types. (b) `terminal_resident_value_transfers` records which structural operations move or copy resident values and whether the transfer is fully resolved. (c) `scalar_storage_writes` names the scalar writes of one operation.

**Shape.** `CapturedScalarOperand` (`:16`), `TerminalScalarCaptures{writes, arrivals}` (`:33`, `new` `:42`), `ResidentValueTransfer`/`ResidentValueCapture`/`ResidentValueCaptures` (`:211-221`), `terminal_resident_value_transfers` (`:223`), `place_type_index` (`:296`), `capture_inventory_failure` (`:384`), `scalar_storage_writes` (`:399`).

**How it decides.**
- Inventories are dense `IdTable`s with a gap limit of 65,536 (`:61,69,329,381`); gap overflow is an error string (`:384-397`).
- `place_type_index` rejects non-dense parameter positions (`:302,315`), repeated places (`:337`), parameter kind mismatch (`:341-347`), a result place without a matching signature (`:356-362`).
- Arrivals: for jumps/conditionals, actual type must equal formal type (`:130`); for `StructuralCase` edges, uses `case_payload_scalar_types` and descriptor locations (`:145-198`).
- `terminal_resident_value_transfers` (`:223-294`): operation kinds `structural_leaf_copy_pair`, `MoveStructuralValue`, `TransformContent`, `MoveStructuralField`, `StoreStructuralField`, `EstablishStructuralArray/Record/Case` only; unresolved if `step.may_escape` or any derivation is a `storage_derivation`.
- `scalar_storage_writes` (`:399-507`): fixed list: `WriteOnlyPrimitiveStore`, `StructuralScalarFieldStore`, `EstablishPrimitiveLocal`, `EstablishScalarArray`, `EstablishScalarCase`, `EstablishRecord`/`EstablishStructuralCase` scalar fields; everything else `Ok(false)` -> `None`.

**Cases by shape.** The two operation lists above (`:243-253`, `:415-497`).

**Same job elsewhere.** `terminal_flow.rs:360-387,1314-1343,1440-1478` records the same scalar writes into `scalar_payload_writes`/`scalar_value_stores` using the same operation list (read); `scalar_storage_writes` is also called from `selected_effects_machine` (`terminal_flow.rs:2557`).

**Unused/one-caller.** `terminal_resident_value_transfers`, `TerminalScalarCaptures` external callers not grepped (suspected: native optimization-unit crate).

**Fail-open risks.** `terminal_resident_value_transfers` covers only the listed operation kinds; any other operation has no entry in the returned map, so a consumer reading "no entry" must treat it as no transfer. Open (consumer not read). `scalar_storage_writes` returns `None` for any operation not in the list (not a scalar store): closed if the operation does not write scalars.

**Tests (`tests.rs`).** 13 tests over hand-built modules: terminator diagnostics (`:293`), non-returning calls (`:386`), carrier-slot borrow (`:578`), projected release (`:648`), moves vs copies (`:707`), construction custody (`:772`), stored sequence ordinals (`:880`), pointer capture (`:1024`), call inputs (`:1229`; includes ordinals, escape, moved roster, borrowed leaf, shorter alternative returns, nested rosters), cleanup owners (`:2011`), decomposition (`:2076`), moving sequence values (`:2173`), forwarded scalars (`:2572`).

---

## D/descriptor_results/scalar_contracts.rs (319)

**Purpose.** Turns callee `ensures` clauses into caller-side facts: scalar postconditions and stored-scalar writes, and maps case-payload scalar types.

**Shape.** `scalar_conjuncts` (`:7`), `instantiate_payload_scalar_guarantees` (`:20`), `TerminalPostconditions` (`:44`), `terminal_postconditions` (`:49`), `case_payload_scalar_types` (`:289`).

**How it decides.** `scalar_conjuncts` flattens conjunctions and keeps only closed propositions (`visit_value_ids` true, `:13`). `instantiate_payload_scalar_guarantees` keeps a conjunct only if every value id it mentions is bound (`:36-40`). `terminal_postconditions` handles only call operation kinds `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructuralWithScalarArguments`, `CallStructural` (`:59-87`); requires a unique callee machine, matching arities, no erased scalar or proof formals (`:93-99`), exact scalar type match of each actual (`:113-121`), and structural arguments with the formal's type (or an approved nominal or const-array match) (`:137-167`). For stored postconditions it looks at `Equal(field, value)` conjuncts where the subject is an `IntegerField` or `Current` `BooleanField` and the value mentions only bound values (`:236-254`).

**Cases by shape.** Call kinds above; stored postconditions limited to integer and Boolean fields (`:238-244`; floats not handled).

**Stated limits.** Not stated. The `ensures` clauses are trusted as facts: the callee's checker admission is outside this unit (inferred).

**Fail-open risks.** The caller adopts the callee's declared `ensures` without re-deriving them. Open: depends on callee admission elsewhere. A callee with no unique body returns `None` (`:92-93`), closed.

---

## D/descriptor_results/terminal_flow.rs (2632)

**Purpose.** The adapter that reads one `TerminalMachine` and builds its `DescriptorMachine` storage-flow graph: one flow step per operation and per terminator with definitions, inputs, derivations, payload writes, call invocations, scalar reads/writes and successor edges with bindings and cleanup effects.

**Shape.** `adapt` (`:177`), `adapt_flow` (`:207`; ~2200 lines, one function), `residence_rules` (`:142`), `record_owned_consumption` (`:46`), `consume_call` (`:25`), `CapturedBacking` (`:2415`), `captured_storage_source` (`:2423`), `destination_root` (`:2478`), `mark_opaque_call_occupancy` (`:2487`), `selected_effects_machine` (`:2517`).

**Inputs/outputs.** Reads module, sorted machine roster, machine, `slot_kind`, `selected_calls`, optional first-pass graph `selector_graph`, `nominal_calls`. Output `DescriptorMachine` or a `DescriptorGraphError` whose `detail` string records where it failed (`failure_detail` set before each `?`).

**How it decides (judgments).**
- Each operation gets a flow step. For storage uses: a use that has a descriptor location records an input; one that does not sets `may_escape` unless it is an exact primitive read/write projection (`:478-525`).
- Payload writes by operation kind (`:634-799`): `WriteOnlyPrimitiveStore`, `StructuralScalarFieldStore`, byte stores, `StoreStructuralField`, `ElementViewWrite`, `MoveStructural*`, atomics that modify, calls (by `shared_mutation` of each argument's type: `Unbounded` pushes `None`, `:751-752`), etc. The final `kind =>` arm (`:778-798`): any operation that is not a goal-free scalar leaf, a proof-bearing scalar row, or a structural-effect row with no external effect pushes `None` (unknown write).
- Calls (`:1479-1886`): `CallMachine`, `CallDynamic`, direct calls and `BoundaryCall` each build an invocation row; absence of an exact row is recorded as `calls.insert(position, None)` (`:1891-1893`) for any `is_invocation` operation.
- Owned consumption (`:46-140`): a moved/stored/constructor argument is "consumed" (defined and recorded in `step.consumed`) if its source is affine/linear (`structural_argument_source`) or a `TrivialAffineLocal` place (`:64-81`); `ByteSequenceLiteral` is not consumed; any other place kind returns `None` and aborts the adapt (`:79`).
- Terminator edges (`:1925-2067`) check scalar arity/type, structural arity, access equality, build bindings and the `source_access` authority; `Conditional` gets edge predicates `Equal(Boolean(p), term)` (`:2358-2373`); `StructuralCase` gets `case_successors` (`:2321-2357`); `ReturnStructural` requires the source type to equal the declared result (`:2121-2142`).
- `selected_effects_machine` (`:2517`): dynamic dispatch with an unchanged `SelectImplementation` table selects the single effects machine, requiring the target body's signature to match the interface slot exactly (`:2604-2628`).
- Keys: positions, place ids, type ids, operation ids, edge ids.

**Cases by shape (fixed lists).**
- `scalar_store` kinds (`:360-375`); `ScalarStructuralField`/`BooleanStructuralField`/`PrimitiveScalarRead` reads (`:388-428`); `DecomposeStructuralValue`, `ActivateTaskRegister`, `ReloadSegmentDescriptors`, `GlobalDescriptorTable::Store`, etc. in the payload-write match (`:635-777`); `EstablishStructuralArray` accepts only `ElementView` (shared/None access) or `FixedArray` with literal length equal to the element count, else `return None` (`:1346-1359`); `EstablishScalarCase`/`EstablishStructuralCase` require a Sum/Mixed result type (`:840-845`).
- Comment `:1895-1897` "Borrowing a carrier slot makes a child resource, not another spelling of the parent's permission."

**Same job elsewhere.** Scalar write recording duplicates `scalar_captures.rs:399-507` (see above). Retained-call classification `mark_opaque_call_occupancy` mirrors `boundary_calls.rs`.

**Unused/one-caller.** `adapt` callers: `descriptor_results.rs:1572,1641,1694,1709` (4). `captured_storage_source`: callers `replay/analysis.rs:594`, `terminal_flow.rs:155,972`. `residence_rules`: via `terminal_residence_rules` (1 caller, `plain_affine.rs:2151`).

**Stated limits.** `:794-795` "A semantic family not represented by this storage flow cannot publish an empty frame." `:1182-1185` "An absent result never proves that a call writes nothing." `:1536` (in root).

**Fail-open risks.**
- `StructuralLeafCopy | StructuralCaseLeafCopy | EstablishStructuralLocal | EstablishZeroedStructural | AtomicFence | StorageAddress | StorageAlignment | non-modifying AtomicAccess => {}` (`:769-776`): treated as writing nothing. Reasoned correct (reads/creations) but uncommented. Classification: open.
- `kind if kind.declared_invocation().is_some() => {}` (`:777`): calls produce no payload write here; their writes are derived later through `calls`. Traced: `is_invocation` makes a `None` call row if no exact row (`:1891-1893`), so unresolved writes stay unknown. Traced.
- `OperationKind::DecomposeStructuralValue` records no write (`:635-643`, comment "Partitioning ownership does not write the value"). Stated.
- Second match `_ => {}` (`:1887`): operations not listed record nothing besides generic input/definition rows. Open.
- `record_owned_consumption` returns `None` (aborts graph) for an unrecognized place kind (`:79`); closed.

---

## D/descriptor_results/value_identity.rs (341) and value_identity/flow.rs (187)

**Purpose.** Proves that a structural actual at an observation is the same unchanged value as a seed (a parameter at entry, or an earlier operation result), by a restricted flow in which only moves and copies transport a value and any other write kills it.

**Shape.** `ValueIdentitySeed` (`:12`), `ValueIdentityAnalysis` (`:20`; `new`, `require`, `require_at_invocation`, `query`, `normalize_actual`, `prepare_cut`), `require_unchanged_value` (`:328`); `flow::adapt` (`flow.rs:3`), `location_type`, `identity_operation` (`flow.rs:140`), `require_acyclic` (`flow.rs:151`).

**How it decides.** Results are memoized per `(seed, site, actual)` (`:152-158`). Refuses at a `Cleanup` site (`:184-186`), when the seed does not precede the observation (`:221-223`), when seed and actual locations differ in type (`:232-236`), when relevant flow is cyclic (`flow.rs:151-186`), or crosses an unresolved write (`flow.rs:22-24`), an unknown payload write (`:39-41`), or an `Incoming`/`completed` target (`:49-57`). Only the operation kinds in `identity_operation` (`flow.rs:140-149`: `MoveStructuralValue`, `MoveStructuralField`, `StructuralLeafCopy`, `StructuralCaseLeafCopy`, `StoreStructuralField`) carry derivations (`flow.rs:61-80`). The final test is membership of the normalized actual in the set computed by `value_locations` (`:251-261`); error text names "transformations and ordinary call returns supply no identity".

**Cases by shape.** `identity_operation` list; `ValueIdentitySeed::{Parameter, Result}`.

**Unused/one-caller.** `require_unchanged_value`: re-exported `descriptor_results.rs:41`; external caller count not grepped.

**Stated limits.** `:181-183` "Several cleanup actions share one terminator coordinate"; refusal string at `:185`.

**Fail-open risks.** none seen (every unknown yields `Err`).

---

## D/membership_presentations.rs (143)

**Purpose.** Re-states a set of structural-domain memberships when a value is presented as a different carrier type (for example a record viewed through a borrowed referent), by finding the identical domain on the destination carrier.

**Shape.** `present_structural_memberships` (`:6`), `present_structural_membership_contract` (`:23`), `same_presented_theory` (`:74`).

**How it decides.** A domain on the same carrier maps to itself; otherwise the carrier change must be `BorrowedReferentPresentation::Unchanged` (`:39-43`) and exactly one destination domain with the same `semantic_domain` and the same theory (`:44-55`). `same_presented_theory` (`:74-143`) walks the required-membership closure iteratively and requires equal `complete_predicate_theory`, `predicate`, `content_projection`, `byte_predicate`, `establishment_routes`, `access_policy`, equal sorted `required_memberships`, and no cycle. A domain with unknown `required_memberships` (`None`) returns false (`:119-124`). Keys: `semantic_domain` identity plus structural equality of declarations.

**Unused/one-caller.** `present_structural_membership_contract`: only the re-export (`lib.rs:95`) and the in-file call from `present_structural_memberships` found outside `descriptors/` (Grep with glob `!**/descriptors/**`: 1 hit, `lib.rs:95`). So 0 external callers besides the re-export.

**Fail-open risks.** none seen: missing source domain and no/ambiguous destination return `Err`.

---

## D/primitive_store_memberships.rs (390), primitive_store_memberships/scalar.rs (270), primitive_store_memberships/tests.rs (82)

**Purpose.** Decides how a store into a primitive scalar inside a value that holds domain memberships may proceed: which restriction the written scalar must satisfy, whether a store value's scalar memberships match, and which byte predicates an enclosing byte array imposes on one element.

**Shape (`primitive_store_memberships.rs`).** `scalar_projection_entails_membership` (`:14`), `primitive_store_restriction` (`:50`), `enclosing_byte_predicates` (`:105`), `primitive_store_byte_predicates` (`:186`), `exact_domain` (`:206`), `whole_scalar_domains` (`:216`), `primitive_store_preserves_scalar_memberships` (`:255`), `scalar_domain_matches` (`:299`), `primitive_scalar_memberships` (`:320`), `expiring_input_predicate` (`:371`).
**Shape (`scalar.rs`).** `refinement` (`:11`), `primitive_carrier` (`:26`), `bind_subject` (`:37`), `receive` (`:124`), `required_domains` (`:160`), `same_theory` (`:197`).

**How it decides.**
- A domain applies to a scalar write only if its `complete_predicate_theory` is true and it has no required memberships, establishment routes, content projection or byte predicate (`:27-34`).
- `bind_subject` (`scalar.rs:37-122`): rewrite the subject-read terms of the predicate to `ScalarRefinement::SUBJECT`; refuses `Entry` versions, other roots, partial paths, `MathValue` terms and propositions other than truth/falsehood/comparisons/connectives (`:72`); budget 4096 nodes (`:48`).
- `primitive_store_restriction`: combines the type's own restriction with every whole-scalar domain's refinement through `scalar::receive`; then derives an ASCII (0..127) or non-NUL (1..255) `BoundedInteger` for a `u8` element of a byte array carrying `AsciiOnly`/`NoNul` (`:72-101`). Comment `:86-87`: ValidUtf8 gives no octet bound.
- `enclosing_byte_predicates` (`:105-184`): skips memberships that do not overlap the written path; a clause-only domain governs no single write (`:143-155`, comment cites `dependent_values.md` "Invariant windows"); a write inside a byte-predicate domain must be a `FixedIndex`/`RuntimeIndex` step into a `FixedArray` of `u8` (`:156-176`), else `None`.
- `same_theory` (`scalar.rs:197`): structural and scalar domain closures must match node-for-node in `semantic_domain`, required memberships, sorted establishment routes, and the refinement must entail in both directions.
- `expiring_input_predicate` (`:371`): memberships listed in `additional_input` that are pure predicates are ignored for stores (they expire with the call input).

**Cases by shape.** `u8` element in `FixedArray` only for byte predicates (`:156-176`); primitive scalar carriers (`scalar.rs:26-35`).

**Unused/one-caller (grepped).** `present_*` see above. `primitive_store_byte_predicates`: callers `optimization-unit-semantics/src/unit_validation.rs:5724`, `interpreter/memory/primitives.rs:4509` (2). `scalar_projection_entails_membership`: callers `foundation_validation.rs:8739`, `interpreter/embedding/boundary_effects.rs:2398`, `interpreter/memory/references.rs:2831,2844` (4). `primitive_scalar_memberships`: from the first combined grep, in `foundation_validation.rs`, `interpreter/memory/references.rs` and re-exported; per-name counts not isolated.

**Stated limits.** `:86-87` comment; `:143-147` comment.

**Fail-open risks.**
- `if !crate::structural_paths_may_overlap(path, written) { continue; }` (`:131-133`): a membership not overlapping the write is not constrained; correct by definition of the path overlap. Traced.
- Clause-only domain `continue` (`:149-155`): stated (invariant windows; the checker re-proves before use).
- `whole_scalar_domains`: memberships whose path length differs from `written.len()` are skipped (`:241`): only exact-path scalar domains constrain a scalar write. Open: a membership on an enclosing path that is a pure scalar refinement does not exist, so likely fine; not traced.
- `primitive_store_preserves_scalar_memberships`: `required.establishment_routes.is_none() ||` ... (`:287`): a required domain with no establishment routes is treated as satisfied without checking the stored value's own memberships (those domains are refinements the store restriction already enforces through `primitive_store_restriction`). Open.

**Tests.** 1 test (`tests.rs:6`): ValidUtf8 adds no `u8` bound, AsciiOnly does.

---

## D/replay.rs (942)

**Purpose.** Module root of the replay engine: declares submodules (`input_contents`, `location_map`, `analysis`, `case_flow`, `contents_queries`, `dispatch`, `engine`, `provider`, `summary_interface`; `:11-21`), the stored-origin and result-contents types, slot-kind policy (`DescriptorSlotKind`), and slot inventory computation.

**Shape.** `DescriptorResultContents` (`:30`), `ProducedContents<D>` (`:43`), `DescriptorStoredOrigin` (`:65`; 11 variants), `DescriptorStoredEffect` (`:156`; `keeps_incoming_slot`, `keeps_incoming`, `exact_path`), `DescriptorStoredCallEffect` (`:199`), `reconstruct_source_origins`, `reconstruct_stored_effects` (`:205,214`), `result_origin` (`:221`), `result_origins` (`:291`), `return_origins` (`:309`), `return_slots_have_external_backing` (`:325`), `call_origins` (`:353`), `may_contain_descriptor` (`:362`), `DescriptorSlotKind` (`:405`; Views / References / BorrowedStorage / AccessOrigins) and its methods (`tracks_returned_value`, `tracks_views`, `tracks_references`, `is_slot`, `parameter_is_incoming_slot`, `slots`, `owned_slots`, `owned_inventory`, `inventory`, `entry_inventory`, `may_write_indirect_contents`, `overwritten_slots`, `writable_slots`), `indirect_contents_contain_slot` (`:632`), `DescriptorSlotInventory` (`:741`), `UnknownContentsKind` (`:758`), `slot_inventory` (`:797`).

**Inputs/outputs.** Types in, `DescriptorSlotInventory{slots, unknown_contents}` out; `result_origin` maps a stored origin to a public `DescriptorResultOrigin`.

**How it decides.**
- Slot membership by type shape and kind: `Views` = view descriptor, `References` = `Reference` shape, `BorrowedStorage` = borrowed descriptor shape, `AccessOrigins` = `Reference` or access-view/accessor record storage (`:438-458`).
- `slot_inventory` (`:797-942`): walks records, mixed, sum cases, literal fixed arrays, references (only if scope is not Owned) and type parameters (`UnknownContents` packet). Limits: more than 4096 collected rows returns `None` (`:813`); array literal length > 4096 returns `None` (`:890`); a symbolic-length array becomes a `SymbolicArray` unknown-contents packet, except below a `Referent` scope where it returns `None` (`:877-889`); a cycle (type already active) returns `None` (`:813`).
- `fully_enumerated` rejects inventories containing a `SymbolicArray` (`:747-752`).
- `may_write_indirect_contents` (`:561-584`): a SharedBorrow argument never writes (`:567`); an unknown path tip is assumed to write (`:570-573`); otherwise whether the selected type's indirect contents contain a slot of this kind.
- `overwritten_slots` (`:586-629`): path-prefix overlap of a store destination against each slot's resident path, with `RuntimeIndex` matching any `FixedIndex`.
- `writable_slots` (`:677-738`): slots reachable through a mutable/owned permission; any unknown-contents packet that is writable returns `None`.

**Cases by shape.** `slot_inventory` handles `TypeParameter`, `Reference`, Record/Mixed fields (non-erased), `FixedArray`, Sum/Mixed cases; other shapes yield nothing (`_ => {}`, `:899`). `may_contain_descriptor` has the same set (`:383-399`). `indirect_contents_contain_slot` (`:656-671`) `_ => false`.

**Same job elsewhere.** `may_contain_descriptor` (`:362`), `slot_inventory::walk` (`:804`), `indirect_contents_contain_slot` (`:632`), `owned_shape::visit` (`owned_shape.rs:26`) and `descriptor_lifetimes` path walks (outside unit) each recurse over the same type tree with their own cycle handling; `4096` appears at `:813,890` and in `contents`-side files.

**Unused/one-caller.** `descriptor_slots` (`:772`) / `descriptor_slot_inventory` (`:780`): re-exported; external counts not grepped.

**Stated limits.** `:80`-style comments: `:27-28` "A contents packet records custody; it supplies no address or access mode." `:792-794` `Referent` scope comment; `:761-762` SymbolicArray: "The shared input supplies an array whose length is a parameter."

**Fail-open risks.**
- `may_contain_descriptor` and `indirect_contents_contain_slot` return `true` on cycles and unknown types (`:370,373,642,646`): closed (assumes it may contain).
- `_ => false` at `:670` (`indirect_contents_contain_slot`): a scalar, byte sequence, or other leaf cannot contain a slot; stated by shape. A `ByteSequence` borrowed view field in a record is handled by `is_view_field_slot`; a top-level `ByteSequence` borrowed view arrives via `kind.is_slot` check at `:639`. Open for `Opaque`.
- `may_write_indirect_contents` with a SharedBorrow argument returns `false` (`:567`) without checking the argument type: traced; shared borrow cannot write through, except interior mutability, which `terminal_flow.rs:743-752` covers via `shared_mutation`; this function does not.

---

## D/replay/analysis.rs (2268) and D/replay/analysis/call_origins.rs (69)

**Purpose.** `DescriptorAnalysis` is the query object that callers use: it owns a `Replay` (engine, outside unit) over an immutable graph and answers where a descriptor's storage comes from at a cut, which storage an operation writes, what a call's stored effects and results are, and builds the lifetime flow (`flow`, `residence_flow`, `prepare_flow`, `append_invocation_flow`).

**Shape.** `StorageQueries` memo (`:13-109`), `DescriptorAnalysis` (`:111`), constructors `new`, `with_reference_slots`, `with_borrowed_storage_slots`, `with_access_origins` (`:234,393,403,416`); queries `source_observations` (`:154`), `flow_feasibility`, `edge_feasibility`, `case_feasibility`, `case_is_established`, `established_case` (`:243-391`), `location`, `storage_projection`, `captured_storage_family[_at]`, `borrowed_source_storage_family` (`:482-677`), `storage_origins`, `storage_writes`, `write_frame`, `storage_access_frame`, `storage_access` (`:707-813`), `payload_targets`, `arrival_payload_targets` (`:815-841`), `source_origins`, `symbolic_source_origins`, `unchanged_incoming_source_origins`, `try_symbolic_source_origins` (`:843-994`), `substitute_call_source[_at]`, `substitute_stored_effect_source[_at]`, `substitute_input_source`, `export_source_origin` (`:996-1165`), `substitute_call_storage[_at]` (`:1167-1229`), `result_slots`, `result_contents`, `call_result_contents`, `call_preserves_descriptor_storage`, `preserves_descriptor_storage` (`:1230-1370`), `stored_effects`, `call_stored_effects[_at]`, `symbolic_call_stored_effects[_at]` (`:1372-1480`), `invocation_flow_at`, `flow`, `residence_flow`, `prepare_flow` (`:1481-1811`), `append_invocation_flow` (`:1813-2267`). `call_origins.rs`: `call_origins` and `try_call_origins` (`:4,12`).

**Inputs/outputs.** Immutable `types` and `machines` slices (identity checked by pointer equality in `uses_source_graph`, `:126-132`); output rows as above; `Result<_, &'static str>` / `DescriptorGraphError` / `Option`.

**How it decides.**
- Pointer-identity ownership: `uses_source_graph` (`:131`) uses `std::ptr::eq` on the slices (used by `payload_history.rs:236`, `payload_versions.rs:290`).
- Feasibility (`:243-339`): `Unknown` when states cannot be prepared; `ProvedImpossible` when a position has no snapshot; case successors judged by `case_flow::select`.
- `unchanged_incoming_source_origins` (`:875`): accepted only when `slot_kind == BorrowedStorage`, every origin is an incoming slot / contents / a shared byte-view parameter, and `require_unchanged_entry_storage_with_incoming` succeeds for each (`:882-944`); result memoized.
- `substitute_call_storage_at` (`:1176`): only exact `Argument` projections (`:1186-1192`); requires each actual root's value to be `Origin::Storage` only and a single storage location when the path is empty (`:1206-1227`).
- `preserves_descriptor_storage` (`:1301`): true only if every normal exit reads the same value as the entry state and at least one normal exit is reached (`:1359-1369`); reading uses entry state, not widened loop state (comment `:1347-1348`).
- `prepare_flow` (`:1592`): canonicalizes each step's definitions/inputs/derivations to local storage, adds alias definitions (`:1664-1682`), clears `may_escape` for a machine call with unresolved argument address when the callee summary shows no borrowed result, no stored effect and no invalidation (`:1633-1645`), then appends each call's flow.
- `append_invocation_flow` (`:1813-2267`): for each stored effect not equal to its incoming value (`:1891-1898`), resolves the destination targets; `strong` iff exactly one `Storage{completed: None}` target (`:1907-1913`); a result's unknown slot gets derivations from every argument (`:2049-2075`); provider/produced contents add lifetime-only dependencies (`:2096-2117`); `authority_derivations` added only for `Parameter` origins with `original_descriptor_path`, no indirect/window/captured, when slot kind is not References (`:2242-2262`).

**Cases by shape.** `export_source_origin` has exhaustive arms over `DescriptorSourceOrigin` (`:1096-1164`); `append_invocation_flow` origin match (`:2118-2220`).

**Same job elsewhere.** `call_origins` exists as `DescriptorAnalysis::call_origins`/`try_call_origins` (`call_origins.rs`), as `descriptor_results.rs:408` `call_origins` and `:422` `call_origins_in`, and `replay.rs:353` `call_origins` (three layers of wrappers). `substitute_call_source_at` vs `substitute_stored_effect_source_at` differ only by `stored_effects: bool` (`:1032`).

**Unused/one-caller (not grepped individually; list of exported functions).** `flow_feasibility`, `edge_feasibility`, `case_feasibility`, `case_is_established`, `established_case`, `source_observations`, `substitute_input_source`, `write_frame`, `storage_access_frame`, `result_contents`, `call_result_contents`, `invocation_flow_at`: external call counts not grepped (suspected users: `references/`, `storage/`, `plain_affine.rs`, native optimization-unit-semantics). I did not verify any as unused.

**Stated limits.** `:8-11` "Proof and loan consumers revisit the same storage at the same cut." `:696-697` comment "A completed caller snapshot can still consult a callee whose result account has not been prepared." `:720` "None means the storage effect could not be placed; it must use the caller's conservative ceiling, never an empty frame."

**Fail-open risks.**
- `flow_feasibility` returns `ProvedImpossible` when a position has no state in a ready snapshot (`:265-267`): a consumer that skips "impossible" positions would skip any position the replay failed to reach. The snapshot is only used if `prepare_states` succeeded (`:250`), else `Unknown`. Traced.
- `edge_feasibility` returns `ProvedImpossible` when `before` is missing (`:300`): same reason, traced.
- `case_feasibility` returns `Unknown` for a missing `before` (`:336`), closed; the two differ in what they return for the same missing-state condition (`:300` vs `:336`). Open.
- `prepare_flow` clears `may_escape` for the specific unresolved-address machine call case (`:1633-1645`): stated in the comment `:1629-1632`.
- `append_invocation_flow`: effects whose origins are all `keeps_incoming` are skipped as no-write (`:1891-1898`), stated at `:1886-1890`.
- `_ => {}` at `:1966` for non-input origins inside the `Incoming` target branch: other origin kinds add no input. Open.
- `call_origins.rs:46`: `!summary.result.unknown().is_empty()` returns Err (closed).

---

## Coverage

All 37 ranges in `units/unit_046.tsv` were read in full, in order:
`descriptor_results.rs` 1-1846; `boundary_calls.rs` 1-130; `compatibility.rs` 1-145; `contents.rs` 1-1498;
`contents/incoming_memberships.rs` 1-93; `invocation.rs` 1-427; `owned_shape.rs` 1-125; `payload_entry.rs` 1-317;
`payload_entry/order.rs` 1-151; `payload_entry/order/tests.rs` 1-79; `payload_entry/tests.rs` 1-122;
`payload_history.rs` 1-1653; `payload_history/entry_snapshots.rs` 1-85; `payload_history/premise_table.rs` 1-213;
`payload_history/premise_table/tests.rs` 1-83; `payload_history/tests.rs` 1-1059; `payload_predicates.rs` 1-552;
`payload_predicates/tests.rs` 1-179; `payload_reachability.rs` 1-272; `payload_reachability/tests.rs` 1-284;
`payload_versions.rs` 1-768; `payload_versions/tests.rs` 1-691; `private_parameters.rs` 1-145;
`provider_invocation.rs` 1-159; `scalar_captures.rs` 1-510; `scalar_captures/tests.rs` 1-2707;
`scalar_contracts.rs` 1-319; `terminal_flow.rs` 1-2632; `value_identity.rs` 1-341; `value_identity/flow.rs` 1-187;
`membership_presentations.rs` 1-143; `primitive_store_memberships.rs` 1-390; `primitive_store_memberships/scalar.rs` 1-270;
`primitive_store_memberships/tests.rs` 1-82; `replay.rs` 1-942; `replay/analysis.rs` 1-2268; `replay/analysis/call_origins.rs` 1-69.
Read in full: 21,936 lines. Skimmed: 0. Not read: none.
Test-file detail (the four large test files: `payload_history/tests.rs`, `payload_versions/tests.rs`, `scalar_captures/tests.rs`) is summarized by test name and subject, not line-by-line assertions.
Grep commands used: Grep (content/count) for `native_code_layout_regions|aggregate_descriptor_slots|reconstruct_return_slots_external|terminal_access_origin_graph|provider_outputs_cover_writable_slots`, `terminal_residence_rules|...`, `present_structural_membership_contract|primitive_store_byte_predicates|scalar_projection_entails_membership`, `prove_payload_predicate|payload_entry::prove|payload_entry::close`, all over `source-rust/`.

## Five things a maintainer most needs to know about this unit

1. `terminal_flow.rs::adapt_flow` (`:207-2413`) is one ~2,200-line function that is the only place a `TerminalMachine` becomes a `DescriptorMachine`. Which operation kinds write storage is decided by a hard-coded `match` (`:634-799`); an operation kind that is not listed and is not a scalar leaf, a proof-bearing scalar row or a no-external-effect structural row is recorded as an unknown write (`payload_writes.push(None)`, `:796`). Adding an operation kind therefore needs a decision here, in `scalar_captures.rs:415-497` (scalar writes, duplicated list) and in `replay.rs` slot policy.
2. Descriptor graphs fail whole-machine: `terminal_graph`/`terminal_reference_graph`/`terminal_borrowed_storage_graph` return an empty `Vec` when any machine cannot be adapted (`descriptor_results.rs:1215,1663,1667`), while the `*_from` and `terminal_graph_rows` variants keep per-machine errors. Which variant a consumer calls decides whether an unsupported body is an error or silent absence.
3. `payload_entry.rs` and `payload_predicates.rs` contain a small proof search (`prove`/`select`/`order::derive`) that produces `ProofNode` certificates which `proof_admission::check_certificate` then accepts or rejects. A membership is added to a contract only after that check (`payload_entry.rs:57`, `payload_predicates.rs:229-232`).
4. Several analyses are the same forward must-flow with an AND join, written separately: `payload_history.rs:719-1239`, `payload_versions.rs:175-207` and `:580-616`, plus reachability pruning in `payload_reachability.rs`. Budgets of 4096 (types/walk nodes) and 65,536 (worklist steps or cached answers) and depth 64 recur at `payload_entry.rs:18,71,123,134`, `entry_snapshots.rs:34,37`, `replay.rs:813,890`, `scalar.rs:48`, `payload_history.rs:718`, `payload_reachability.rs:168`, `payload_versions.rs:129`, `scalar_captures.rs:61,69`; none carries a comment.
5. Identity of the analysis graph is by slice pointer (`DescriptorAnalysis::uses_source_graph`, `analysis.rs:131`), and callers reuse one `DescriptorAnalysis` across payload history, versions and value identity; passing an analysis built over a different `machines` slice is refused with an `Err` (`payload_history.rs:236`, `payload_versions.rs:290`). Native crates (`optimization-unit-semantics`) call into this crate's `descriptor_results` API (grepped: 3+ call sites).

## What you are unsure about

- Whether callers treat an empty `Vec` from `terminal_graph*` as failure; I did not read those callers.
- Whether `terminal_definition`'s `_ => Some(Local)` (`descriptor_results.rs:1781`) is reachable for kinds that actually hold borrowed storage (e.g. `ProviderAttachment`, `Result`).
- Whether `scalar_payload_writes` with several resolved destinations (`payload_history.rs:1001-1003`) leaves a stale observation for every operation kind, or whether `payload_invalidations` always retires it first. I read both sites and did not trace each operation kind.
- The external callers of many `DescriptorAnalysis` methods (`replay/analysis.rs`); I grepped only the helper functions listed in the "Unused/one-caller" entries, not each method.
- Engine modules referenced from `replay.rs:11-21` (`engine`, `case_flow`, `contents_queries`, `dispatch`, `provider`, `summary_interface`, `input_contents`, `location_map`) and `descriptor_lifetimes` are outside this unit; statements about them are inferred from their call sites.
- `payload_versions.rs:219-221` and similar `with_reference_slots` choices (versus `with_borrowed_storage_slots`) are not explained by comments; I did not determine why each query uses a different slot kind.
