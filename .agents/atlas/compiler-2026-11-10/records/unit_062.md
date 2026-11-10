# Unit 62 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`):

1. `flow_transfers.rs` (1-4593)
2. `immutable_checking_input.rs` (1-130)
3. `lib.rs` (1-86)
4. `products.rs` (1-1414)
5. `published.rs` (1-10658)

Checkout `<checkout>/`. Every range was read in full. Counts below come from the Grep tool (named per item). Paths below are relative to the `src/` directory above unless a longer path is given.

---

## 1. `lib.rs` (86 lines)

**Purpose.** Entry file of the Psi 05 crate. Header comments say that `lower_typed_trees` consumes `FrozenTypedTrees` and publishes `CheckFacts` beside the unchanged typed program (`lib.rs:1-9`). It declares the module tree and re-exports the public surface.

**Shape.**
- Modules: `checking` (`:38`), `immutable_checking_input` as `#[cfg(test)]` (`:39-40`), `borrow checks execution flow values` (`:47-51`), `products published shared` (`:57-59`).
- `pub(crate) use optimization::digest_encoder` (`:35`).
- Re-exports from `checking`: `CheckingOutcome, CheckingRequest, RetainedCheck, lower_typed_trees, lower_typed_trees_reusing, reuse_retained_check` (`:62-65`).
- Re-exports from `products::package_review`: 13 functions (`:66-74`).
- Re-exports from `products::product_pruning`: pruning types and two functions (`:76-80`).
- Re-exports from `::validation`: `AsmAuthorityAdmission, data_requires_establishment` (`:86`).
- Re-exports `::facts::declaration_page_fork_totals` (`:61`).

**Inputs/outputs.** No logic. Read only.

**How it decides.** Nothing.

**Cases by shape.** None.

**Same job elsewhere.** None.

**Unused / one-caller.** Not checked per re-export except the `products` ones (see section 4).

**Stated limits.** The comment says checking "only reads the typed tables" and that preparation belongs to the preceding stage (`:11-22`). Section 2 records how this is enforced.

**Fail-open.** None seen.

---

## 2. `immutable_checking_input.rs` (130 lines)

**Purpose.** One `#[test]` (`:9`, `checking_does_not_reopen_typed_preparation`) that pins the rule "checking may not mutate or re-run preparation". It works by reading source files as text with `include_str!` and `std::fs::read_to_string`, and asserting that forbidden substrings are absent.

**Shape.** Single function.
- Scans `lib.rs` for `mod preparation;`, `mod monomorphization;`, `mod lookup;` (`:11-16`).
- Scans `checking.rs` for 9 forbidden call spellings such as `&mut program`, `prepare_typed_trees(` (`:18-30`).
- Asserts `checking.rs` contains `program: typed_trees::FrozenTypedTrees` (`:31`).
- Scans the stage 04 files `monomorphization.rs` and `preparation.rs` for `resolve_projected_receiver_calls(` and `resolve_specialized_receiver_calls(` (`:32-42`).
- Scans `validation/src/program_validation.rs` for `GenericContracts` (`:51-55`).
- Scans `validation/src/effect_inference.rs` for `structural_fingerprint`, `ProgramKey` (`:56-62`).
- Walks this crate's `src`, `validation/src` and stage 04 `src` recursively, skipping `immutable_checking_input.rs` itself (`:64-120`):
  - Forbids `thread_local!` and `RefCell` in every `.rs` file (`:111-117`).
  - For four named stage-04 files, forbids 9 names such as `validate_closed_const_argument`, `infer_operational_may`, `machine_template_commitment` (`:82-110`).
- Scans `products.rs` for `&mut checked.typed`, `checked.typed =`, `&mut program.typed` (`:122-129`).

**Inputs/outputs.** Reads source text from disk. Produces a pass/fail.

**How it decides.** Keyed purely on source spelling (substring `contains`) at `:13`, `:29`, `:40`, `:44`, `:48`, `:53`, `:59`, `:105`, `:113`, `:128`.

**Cases by shape.** The forbidden-name lists are fixed lists (`:18-28`, `:36-39`, `:93-103`, `:57`, `:111`, `:123-127`). A renamed or differently spelled equivalent would not trip them.

**Same job elsewhere.** None in this unit.

**Unused / one-caller.** Test-only file (`lib.rs:39`).

**Stated limits.** None.

**Fail-open.** The test cannot detect a mutation spelled differently from the listed strings (grep: not run; inferred from the code). It does not accept programs, so this is a guard, not a compiler path.

---

## 3. `flow_transfers.rs` (4593 lines)

**Purpose.** After a `let` or an assignment, copies the facts that held for the written value's source onto the written place. Header (`:1-19`) lists the phases in order: `context_transport`, `owned_qualifications`, `projected`, `constructed`, `byte_sequences`, `scalar_values`, `declared_domains`. Then publishes the new facts as fresh contexts, one per exact place and normalized domain atom.

**Shape (all inline modules in the one file).**

| Module | Lines | Main items |
| --- | --- | --- |
| `byte_sequences` | 43-481 | `append_derived_predicates` `:61`, `value_proves_predicate` `:114`, `append_element_replacement_predicates` `:226`, `replacement_byte` `:376`, `carrier_proves_predicate` `:420`, `facts_about_place` `:447`, `index_free_membership` `:471` |
| `constructed` | 483-689 | `routed_membership_transferable` `:507`, `append_constructed_field_values` `:520` |
| `context_transport` | 690-895 | `transport` `:704` |
| `declared_domains` | 896-1118 | `establish` `:919` |
| `owned_qualifications` | 1119-1339 | `append_owned_qualification_transfer` `:1130`, `explicitly_erased` `:1326` |
| `projected` | 1341-1683 | `append_copied_field_predicates` `:1368`, `append_completed_copy_predicates` `:1397`, `enum CopiedEvidence` `:1421`, `append_value_predicates` `:1427`, `bound_reference_referent_place` `:1621`, `declared_local_type_reference` `:1671` |
| `replacement` | 1685-1847 | `preserve_owned_memberships` `:1704` |
| `scalar_values` | 1848-3683 | nested `calls` `:1867` (with `normal_returns` `:1882`, `capture_call` `:2280`, `exact_call_occurrence` `:2486`), `captured` `:2534`, `conversions` `:2894`, `capture_statement` `:3159`, `capture_bounds` `:3318`, `selected_live_capture` `:3410`, `builtin_bound_meaning_source` `:3439`, `selected_statement` `:3456`, `retains_values_across_unit_call` `:3510`, `selected_scalar_argument` `:3611`, `struct CallValues` `:3651` |
| `write_target` | 3684-3849 | `struct StatementWrite` `:3699`, `statement_write` `:3714`, `narrow_pinned_selectors` `:3804` |

Top level: `propagate_statement_transfers` `:3852` (the entry), `publish_statement_contexts` `:4204`, `retain_qualification_correspondence` `:4280`, `exact_qualification_payload` `:4368`, `exact_evidence_source` `:4389`, `exact_structural_symbol_place` `:4401`, `correspondence_root_type_reference` `:4500`, `exact_statement_owner` `:4534`, `contextual_expression_place` `:4546`, `completed_value_carriers_agree` `:4568`.

Types: `StatementWrite` fields `target_place, source_expression, source_place, candidate_targets, target_is_exact, stable_value_target, destination_is_current` (`:3699-3712`). `trait CapturedValue` (`:2592`) is implemented for `ScalarValue` (`:2652`) and `IntegerRange` (`:2735`). `CallValues<Value>` has `bindings`, `storage`, `fields` (`:3651-3655`).

**Inputs and outputs.**
- Reads: `&dyn CheckingInput` program tables, `BorrowFacts`, the flow build context (`FlowBuildContext`, with operands, contexts, constraints, scalar-expression plan, operators), and the `FactPlan`.
- Writes: appends facts, refs, places and contexts into `FactPlan` (`semantic.append_fact`, `append_transfer_at_point`, `append_statement_transfer`); writes `stored_destination` on the statement fact (`:4004-4010`); `publish_statement_contexts` rewrites `active_contexts` and `active_constraints` (`:4254-4276`).

**How it decides.**
- Whole-pass gates in `propagate_statement_transfers`: no write target, or `destination_is_current` false, returns nothing (`:3867-3872`). If the source place's cases are not selected, returns (`:3886-3906`). Non-exact targets (several candidate referents) take only `declared_domains::establish` and the element-replacement predicates, then return (`:3974-4002`).
- Value-carrying payloads (`CaseValue`, `AssignedValue`, `AssignedScalarValue`, `AssignedIntegerBounds`, `AssignedValueCapture`, `BytePredicate`, `FloatPredicate`) move only if `stable_value_target` and `completed_value_carriers_agree` holds (`:744-787`).
- `completed_value_carriers_agree` compares `normalized_type_identity` of the source type and the target type (target looked up at `statement_index + 1`), peeling any reference shell (`:4568-4593`). Keyed on type identity.
- Domain memberships transfer by place correspondence (`canonical_places_agree`, `:733-741`); dropped when an erasing cast in the source chain removes the domain (`explicitly_erased` `:1326`, used at `:795-803` and `:1223`).
- `routed_membership_transferable` (`:507`) decides on `evidence.origin` being one of five origins (`AuthorizedRouteEstablishment, CheckedValidation, AdmittedReceipt, CheckedTransformation, Propagated`). Keyed on evidence origin.
- Constructor fields: a domain membership re-anchors only if the domain has no alias and (no establishment routes with a predicate present, or routed evidence is transferable) (`:648-658`).
- Owned transfer (`:1130`): proceeds only for an exact expression result or a value with builtin coordinates (`:1159-1169`); drops a candidate when the destination type along the relative path is a `Reference` (`:1254-1273`); requires every segment to be `Field`, `Case` or `FixedIndex` (`:1274-1281`).
- Projection copy (`:1427`): requires static identity on every segment (`:1474-1480`); for `ContainedStorage` it excludes the source place itself (`:1530`); for `CompletedValue` it rejects reference shells and rejects non-stable-copy types unless the payload is `CaseValue` (`:1575-1580`).
- Byte-class rules (`:213-224` comment): per-byte classes (`no_nul`, `ascii_only`) survive a one-byte store when the stored byte is in the class; `valid_utf8` is derived only from the surviving ASCII class (`:340-352`).
- Scalar capture (`capture_statement` `:3159`) tries in order: completed-expression snapshot (`:3174`), selected call capture (`:3186`), selected scalar plan under builtin meaning (`:3198-3220`), a live value at the expression (`:3229`), selected operand under casts (`:3234`). `capture_bounds` repeats the same ladder for integer ranges (`:3318-3403`).
- Normal-return capture: scalar values require both branches to be equal (`join` is `self == other`, `:2703-2705`); integer ranges take the hull (`min`/`max`, `:2798-2803`).
- Selected-scalar lookup is keyed on `(state, ordinal, role, source expression, destination symbol)` and the binding symbol list (`:2247-2272`).
- `publish_statement_contexts` groups new facts by (place, "observes tag", domain identity) using a `DefaultHasher` bucket plus equality check (`:4233-4249`).
- `retain_qualification_correspondence` (`:4280`) records a link between a source fact and its copy only after about 20 identity checks (`:4300-4355`); it requires `QualificationEvidenceOrigin::CheckedTransformation` (`:4300`).

**Cases by shape.**
- `capture_call` (`:2280`): requires no `machine_arguments` and a nominal route (`:2291`); callee machine must be `CheckedBody` with no owned data (`:2338-2342`); every parameter must have its erased-ness resolvable, no const, mutable parameters only if `mutable_scalar_parameter_type` resolves, all primitive (`:2351-2373`); statement must be `LocalData` or `Assignment` (`:2396-2407`); a receiver call returns `None` at `:2347-2349`.
- `NormalReturnCapture::statements` (`:1972`): accepts `Call` (via `retains_values_across_unit_call`), `Transition`, primitive-typed `LocalData`, `Assignment` whose target is a bare name path, and a last-statement `Expression`; any other statement returns `None` (`:2096`). A repeated state (cycle) returns `None` (`:1963-1965`); comment: "A cycle requiring induction ... withholds the whole capture" (`:1890-1892`). A `Terminal` target returns `None` (`:2242`). Named targets with a static machine parameter or evidence arguments return `None` (`:2173-2175`).
- `retains_values_across_unit_call` (`:3510`): callee is a receiverless, no-machine-argument, non-discarding call to state index 0 of a `CheckedBody` machine with no owned data returning `Unit` (`:3521-3548`); every argument is a literal, a name known to the capture, or a selected scalar argument with `call_ordinal: 0` (`:3558-3580`, `:3611-3642`); exactly one borrow row at the statement with `call_ordinal == 0` (`:3584-3598`); passes only if `observation_places` is empty (`:3599-3602`). The comment at `:2275-2278` says "complete storage footprint is empty"; the code checks `observation_places` only (read, `:3599-3602`).
- Conversions (`:2894-3110`): only a same-carrier retag or a total integer widening classifies (`classify` `:3105-3108`); comment: "a partial conversion ... fails the capture, never a widened guess" (`:2899-2901`). Only non-`recast`, non-domain casts are peeled (`:2998`).
- `append_element_replacement_predicates`: only indexed stores whose parent carrier has fixed-selection segments (`:242-265`).
- `replacement::preserve_owned_memberships`: target needs an `Index` segment and every other segment static or a non-range index (`:1740-1748`); replacement type must be `BorrowFree` (`:1755-1760`).
- `write_target::statement_write`: `Assembly`, `AssemblyFact`, `Call`, `Expression`, `Transition` statements return `None` (`:3727, :3782-3786`); `LocalData` without an ordinary initializer returns `None` (`:3728`).
- `exact_structural_symbol_place` (`:4401`): only `Field`, `Case`, literal-length `FixedIndex` on a `Parameter` or earlier `Local` root; `FixedRange`, `OpaqueRange`, `Index` return false (`:4490-4494`).
- `exact_evidence_source`: source symbol must be `Machine` or `Operator` kind (`:4395-4398`).

**Same job elsewhere.**
- Matching and applying a payload to a place is done in at least three separate transfer lanes here: `context_transport::transport` `:704`, `projected::append_value_predicates` `:1427`, and `constructed::append_constructed_field_values` `:520`. Each has its own list of payload kinds: `:744-750`, `:1494-1522`, `:644-647`. The lists differ (for example `AssignedIntegerBounds` is in `:747` but not in `:1516-1520` or `:644-647`; `AssignedValue` is accepted at `:1513` only when `is_assignment_literal`).
- The same "fact place canonicalised, compared by canonical place" pattern appears at `:733-741` and `:465` (`facts_about_place`).
- `captured::CapturedValue` (`:2592`) and the main scalar capture (`capture_statement` `:3159`) both walk "selected plan, builtin meaning, live place" for `ScalarValue` and `IntegerRange` separately: `capture_statement` `:3159-3243` vs `capture_bounds` `:3318-3403`.
- Same-named flow helpers outside the unit (suspected, not read): `source_transfer_cases_selected`, `place_cases_are_selected`, `fixed_selection_segment` in `flow.rs`.
- `completed_value_carriers_agree` (`:4568`) is used at `:767` and `:1582` only (grep `completed_value_carriers_agree|…`, all `.rs` under `omega-rust`: 2 call sites plus the definition).

**Unused / one-caller (grep on `omega-rust`, `*.rs`, pattern `carrier_proves_predicate|append_completed_copy_predicates|narrow_pinned_selectors|publish_statement_contexts|propagate_statement_transfers|completed_value_carriers_agree`).**
- `propagate_statement_transfers`: 1 call site outside this file (`flow.rs:5566`) plus re-export `flow.rs:11830`.
- `publish_statement_contexts`: 1 external call site (`flow_expression.rs:1796`) plus 2 internal (`:3990`, `:4187`).
- `append_completed_copy_predicates`: 1 external call site (`flow_expression.rs:1786`).
- `narrow_pinned_selectors`: 1 external call site (`flow.rs:5151`).
- `carrier_proves_predicate`: 1 external call site (`flow.rs:1220`), 4 internal.
- `CallValues.fields`: only constructed as `Vec::new()` (`:2195`, `:2456`) (grep `fields: Vec::new\(\)` in this file: 2). The comment at `:3649-3650` says it "carries each retained self-field read of a receiver call", but `capture_call` returns `None` for any receiver (`:2347`). So `structural_field` (`:3669-3681`) always searches an empty list. (read and grepped.)
- `append_derived_predicates` and `append_copied_field_predicates` etc. are `pub(super)` inside the file and each has callers in `propagate_statement_transfers`.
- `contextual_expression_place` ignores its `machine_symbol` argument (`let _ = machine_symbol;` `:4554`); it is called about 10 times in this file (read).

**Stated limits.**
- "Runtime-indexed segments still stop the transport" (`:1351-1352`).
- "Previously inactive leaves have no membership to reactivate and are not manufactured" (`:1693-1695`).
- "Mutable bindings..." not here; see `published.rs`.
- "A cycle requiring induction or a missing selected operation withholds the whole capture" (`:1890-1892`).
- "This never proves that a call returns" (`:2278`).
- `#[allow(clippy::too_many_arguments)]` on about 20 functions (read).
- Hard fuel constant is not here (see `published.rs` `:8225`).

**Fail-open risks.**
- `byte_sequences::value_proves_predicate` returns `false` when the point is not a `Statement` (`:167-174`). Fail-closed direction.
- `retains_values_across_unit_call` returns `Some(())` only when the callee call has no observation places (`:3599-3602`). The accepted shape is a Unit call writing nothing observable; "traced": it is called from `NormalReturnCapture::statements` only (`:1983`). What it could let through: a callee that writes caller storage through a path not listed in `observation_places` (depends on the borrow row; not traced) — "open".
- `IntegerRange::join` returns `Some` of the hull always (`:2798-2803`), so a branch whose value is unknown cannot arise there; unknown values come back as `None` from `evaluate_local`. "traced" (the `?` propagation at `:2107-2127` returns `None` for the whole capture).
- `IntegerRange::formal_fallback` returns the declared primitive's full carrier range when an argument has no snapshot (`:2805-2811`); comment at `:2611-2614` says the declared type bounds the incoming value. "stated".
- `declared_domains::establish` with `target_is_exact` false still publishes a membership for candidates that were "live" before (`:1085-1115`). It keeps membership only if every checked source context proves it (`any` over source contexts, `:1092-1105`). It uses `any`, not `all`: if one of several source contexts proves membership, the candidate keeps it. Which wrong program could pass: a write through a reference local with several possible referents where only one source context holds the fact. "open".
- `propagate_statement_transfers` returns early (no facts) on any refusal, which loses facts rather than inventing them. Fail-closed.
- No `None`-to-`true` defaulting seen.

---

## 4. `products.rs` (1414 lines)

**Purpose.** The seams the product coordinator uses after checking (`:1-6`): `package_review` (read-only re-derivation queries), `product_pruning` (drop machines not reachable from chosen roots, with evidence), `quotient_eligibility` (finish a check from the same invocation's crash facts).

**Shape.**
- `package_review` (`:8-209`): 13 `pub fn` wrappers, each a one-line call to `shared::authored_selections`, `flow`, `published::facts`, `checks::contracts` or `checks::termination`. Names: `typed_operator_has_no_authored_selection` `:26`, `late_bound_member_declaration_from_exact_owner` `:39`, `typed_operator_authored_selection_candidates` `:52`, `derive_checked_semantic_dependencies` `:66`, `derive_authored_machine_crash_buckets` `:78`, `derive_authored_signature_crash_buckets` `:89`, `infer_checked_machine_crash_causes` `:101`, `infer_checked_crash_causes` `:114`, `derive_checked_nominal_call_target` `:127`, `derive_checked_contract_expression_evidence_parameters` `:146`, `derive_checked_contract_expression_evidence_instantiation` `:163`, `infer_machine_termination_summary` `:187`, `infer_checked_recursive_machine_terminations` `:202`.
- `product_pruning` (`:210-1255`):
  - `dependencies` `:288-619`: `struct MachineIndex` `:309` (`HashSet`/`HashMap` of machine symbols, state to machine, statement to machine), `interface_retention_set` `:360`, `collect_machine_edges` `:439`, `contract_caller` `:595`.
  - `rewrite` `:620-948`: `apply_pruning` `:639`, `apply_fact_pruning` `:653` (about 270 lines of `retain` calls over about 40 fact tables), `contract_owner_retained` `:928`.
  - Types: `CheckedTreeProductRoots` `:963`, `CheckedTreeProductRootsError` `:995`, `CheckedTreeProductPruning` `:1007`, `CheckedTreeProductSelectionIdentity` `:1027`, `CheckedTreeProductSelection` `:1046`, `CheckedTreeProductPruningOutcome` `:1075`.
  - Entries: `select_checked_tree_product` `:1089`, `prune_checked_tree_product` `:1119`, `retain_live_family_members` `:1216`, `selection_identity` `:1236`.
- `quotient_eligibility` (`:1256-1414`): `check` `:1266`, `check_operation` `:1286`.

**Inputs/outputs.**
- `prune_checked_tree_product` takes `CheckedTrees` by value and returns the pruned tree plus a `CheckedTreeProductSelection` (roots, retained and pruned machine lists in declaration order, domain-separated SHA-256 identity) (`:1119-1207`, `:1236-1254`).
- `quotient_eligibility::check` reads `CheckFacts` and the program; returns diagnostics only (`:1266-1284`).

**How it decides.**
- Pruning decision is by machine symbol identity: retained = interface set + roots + constant-recipe machines (not `ContractExpression` position) + target-family members, then a transitive closure over `collect_machine_edges` (`:1139-1175`). Everything not retained is pruned (`:1177-1182`).
- Interface retention: every machine whose supply mode is not `CheckedBody` is kept (`:367-371`); plus conformance realization rows, nominal uses, contract-expression evidence calls, fact-call projections, access view plans, selected index operations, and every machine referenced by a retained declaration (`retain_declaration_surface_machines` `:549`, `:373-432`).
- `MachineIndex::machine_of` (`:344-352`) resolves a symbol to a machine either directly or through its owning state.
- Row retention by table: `retained.contains(&row.machine)` for most tables; `!pruned.contains(...)` for `boundary_machines` (`:904-905`), `proof` contract owners (`:928-947`) and semantic dependencies' target (`:871-873`).
- Nominal-use and interrupt-registration rows keep `Expression` sites unconditionally (`:809`, `:819`); `Statement` sites are kept only if the owning machine is in `retained` (`:805-808`, `:815-818`).
- `contract_owner_retained` (`:928`): machine owners and call-signature owners keyed on the caller machine not being pruned; all other owner kinds return `true` (`:945`). Comment at `:923-926` says "non-machine owners ... are always retained". "stated".
- `quotient_eligibility::check_operation`: errors (as `&'static str`) when the flow state, the unique call, the representative, the contract plan, or the crash evidence is missing (`:1291-1317`); errors when the representative has surviving crash buckets (`:1329-1333`); errors when the request or theorem bodies retain checked sites, trapping sites, calls with surviving buckets, or operators with surviving causes (`:1346-1360`). Skips `TransitionNamed` call sites inside theorem bodies (`:1377-1379`).

**Cases by shape.**
- `CheckedTreeProductRoots::new` rejects duplicates only (`:978-984`). `select_checked_tree_product` rejects an empty root list when a selection is requested (`:1101-1105`).
- Only `MachineSupplyMode::CheckedBody` machines are candidates (`:235-236` comment, `:367`).
- `retain_live_family_members` drops families whose members all left (`:1230-1232`).
- Hard-coded keyed-by-machine tables list in `apply_fact_pruning`; a new machine-keyed table in `CheckFacts` that is not listed there would be left unpruned (inferred from the explicit list `:659-920`).

**Same job elsewhere.**
- `contract_caller` (`:595-618`) and `contract_owner_retained` (`:928-947`) both map a `ContractProofFactOwner` to "the machine that owns it" with the same `CallSignature` check `call_signatures::owner(..) == Some(owner)` (`:605-608`, `:939-942`).
- `typed_operator_*` wrappers forward to `shared.rs` (`published.rs` not involved).
- Machine-reference walking uses `typed_trees::references::visit_*` (`:293-297`, `:554-583`), outside the unit.

**Unused / one-caller (grep, `*.rs` under `omega-rust`).**
- `select_checked_tree_product`: 1 call site outside the crate (`psi-compiler/src/source_checking.rs:834`).
- `prune_checked_tree_product`: no call site outside `products.rs`; its only caller is `select_checked_tree_product` (`:1107`); re-exported at `lib.rs:79`.
- `infer_machine_termination_summary`: 1 external (`build-time-evaluation/src/machine_execution.rs:321`). `infer_checked_recursive_machine_terminations`: 1 external (`machine_execution.rs:246`).
- `typed_operator_has_no_authored_selection`: external `machine_execution.rs:2914`; the same-named function in `shared.rs` is also called directly from 8 other sites in this crate (`checks_contracts.rs` 5, `flow_exits.rs` 2, `shared.rs:1208`).
- `typed_operator_authored_selection_candidates`: 1 external (`machine_execution.rs:2979`). `late_bound_member_declaration_from_exact_owner`: 1 external (`machine_execution.rs:3135`).
- `derive_authored_signature_crash_buckets`: 1 external (`omega/packages/review-evidence/.../policy_crashes.rs:33`).
- `derive_checked_contract_expression_evidence_parameters` and `..._instantiation`: 1 external each (`review-evidence/.../evidence.rs:54`, `:93`).
- `quotient_eligibility::check`: 1 call site (`checking.rs:1153`).
- I did not grep every one of the other seven `package_review` wrappers (`derive_checked_semantic_dependencies`, `derive_authored_machine_crash_buckets`, `infer_checked_machine_crash_causes`, `infer_checked_crash_causes`, `derive_checked_nominal_call_target`); their external use is "suspected" only.

**Stated limits.**
- "Arena-resident rows keyed by non-machine identities ... remain as retained dead evidence" (`:270-274`).
- "The lower-level pruning transform also supports interface-only products, but that is not an executable product selection." (`:1087-1088`).
- "the phase does not re-extract the edges of its own output to confirm it." (`:276-278`).
- The data-oriented-storage rule in `AGENTS.md` prefers dense vectors over `HashMap`; this file uses `HashSet`/`HashMap` keyed by `SymbolHandle`/`StatementHandle` (`:280`, `:292`, `:311-315`, `:443`). Recorded as an observation only.

**Fail-open risks.**
- `contract_owner_retained` returns `true` for owner kinds it does not name (`:945`) and the machine-keyed `boundary_machines` uses `!pruned` (`:904-905`). What could pass: a row owned by a machine that is in neither set (for example not a machine at all) is kept. Stated for non-machine owners (`:923-926`). "stated".
- `NominalMachineUseSite::Expression(_) => true` (`:809`, `:819`): nominal-use rows with an expression site are kept regardless of the owning machine. No reason given at the site. "open".
- `retain_symbol`/`machine_of` return nothing for a symbol that is neither machine nor state (`:344-352`, `:585-593`); intended ("Anything else ... is not a machine edge" `:339-343`). "stated".
- `interface_retention_set` inserts `call.target_machine_symbol` directly (`:397`) and filters at `:434` to declared machines. "traced".
- `quotient_eligibility` fails closed (errors) on missing evidence (the comment at `:1257-1260` says "Missing invocation evidence is not purity"). None seen.

---

## 5. `published.rs` (10658 lines)

**Purpose.** What checking publishes beside the typed program (`:1-7`): labels (strings used by checks), the place a value expression denotes, the proof facts (contracts, evidence terms, outcome arms, float meaning), and the semantic fact plan (`FactPlan`) that the flow stage later consumes. `published_facts.rs` (not in this unit) is mounted at `:9-10` as `facts`.

**Shape (top-level module and line ranges).**

| Module | Lines | What it holds |
| --- | --- | --- |
| `labels` | 11-611 | `boolean_facts` 37, `kinds` 141, `names` 179, `operators` 266, `places` 472, `propositions` 574 |
| `places` | 612-637 | `place_at` `:629` |
| `proof` | 638-6536 | `contracts` 653, `evidence_forwarding` 2091, `float_meaning` 2786, `obligations` 4254, `outcome_arms` 4417, `proof_output_calls` 5007, `proposition_vocabulary` 6119, entry `build_proof_facts` `:6332` |
| `semantic` | 6537-10658 | `calls` 6558, `facts` 6878, `places` 9956 |

Key functions:
- `labels`: `context_proves_boolean_label` `:52`, `true_expression_proves_label` `:127`, `machine_name` `:182`, `call_target_label` `:192`, `symbol_name` `:228`, `instantiate_operator_contract_expression_label` `:284`, `boolean_expression_has_polarity` `:444`, `borrow_access_label` `:477`, `semantic_fact_requirement_label` `:489`, `joined_place_label` `:560`, `proposition_label_sources` `:585`.
- `proof::contracts`: `call_signatures::{retain 678, owner 764, owns_contract 784, signature 865}`, `calls::{build_contract_call_facts 887, build_contract_exit_facts 1075, append_contract_fact_refs 1198, contract_target_from_state_symbol 1250}`, `inherited::{append_inherited_trait_contract_facts 1361, trait_requirement_state_symbols 1597, machine_selects_exact_requirement 1613}`, `operators::build_contract_operator_use_facts 1653`, `struct ContractEvidenceLanes 1756`, `append_named_contract_evidence_term 1780`, `append_contract_clause_facts 1831`, `machine_parameter_evidence_signatures 1894`, `append_machine/operator/state/parameter/state_signature/signature_contract_facts 1952-2073`.
- `proof::evidence_forwarding`: `bind_evidence_forwarding_facts 2111`, `bind_evidence_projection_facts 2212`, `checked_evidence_producer 2373`, `validate_evidence_forwarding_definite_assignment 2440`, `append_missing_evidence_diagnostics 2686`, `evidence_term_named 2772`.
- `proof::float_meaning`: `enum CheckedFloatProjectionSourceKey 2836`, `projection_source_key 2892`, `ContractCarriers 2982`, `direct_*_source` 3059-3223, `replay_invocation 3258`, `push_float_meaning_projection 3351`, `instantiate_transported_ensures 3630`, `bind_semantic_application_call 3882`, `bind_float_meaning_projection_facts 4117` (the entry).
- `proof::obligations`: `lower_proof_obligation 4258`, `proof_obligation_owner 4364`.
- `proof::outcome_arms`: `bind_outcome_specific_arm_facts 4437`, `saved_result_call 4667`, `exact_outcome_case_test 4758`, `outcome_call_substitutions 4810`, `instantiate_outcome_arm_fact 4838`, `exact_result_case 4913`, `outcome_specific_fact_is_proved 4943`, `outcome_specific_assignment_matches_result 4978`.
- `proof::proof_output_calls`: `bind_proof_output_call_facts 5021` (about 580 lines), `check_open_proof_output_requirement 5602`, `checked_static_requirement_dispatch 5669`, `trait_requires_transitively 5856`, `check_public_named_witness_lanes 5880`, `proof_output_source_term_by_name 5937`, `instantiate_proof_output_proposition 5978`, `intake_checked_proof_output_propositions 6039`, `intake_call_ensures_propositions 6061`.
- `proof::proposition_vocabulary`: `contract_proposition_labels 6140`, `proposition_label_parts 6172`, `proposition_application_label 6201`, `build_checked_proposition_vocabulary 6211`, `contract_fact_kind 6291`.
- `semantic::calls`: `dispatch_subjects::{computed_case_subject 6580, repeats_subject 6595, evaluated_case_subject 6618, StatementDispatchCalls 6645, statement_dispatch_calls 6673}`, `parameter_domains::call_parameter_domains 6740`, `shared_argument_referent 6825`, `view_argument_names_existing_place 6867`.
- `semantic::facts`: `contracts::{case_observations 6896, parameters 7153, payload 7245, places 7300, scopes 7736, append_contract_semantic_facts 7881, seed_current_requirement 8194, instantiate_inherited_contract_payload 8253, instantiate_call_domain_membership_instance 8295, instantiate_call_contract_payload 8387, append_call_semantic_contract_refs 8554, admitted_resource_carry_origin 8659, append_parameter_domain_requirements 8706}`, `field_domains::{zero_cases 8840, declared_field_domain_carrier 9013, append_zero_initialized_field_domain_facts 9059, collect_zero_initialized_field_domains 9098, append_state_parameter_domain_facts 9230, append_local_zii_field_domain_facts 9596}`, `points::{state_point 9692, proof_obligation_point 9702, contract_fact_point 9736, contract_fact_origin 9784}`, `append_observation_context 9842`, `build_semantic_facts 9861` (entry), `append_state_parameter_predicates 9890`, `append_proof_obligation_semantic_facts 9941`.
- `semantic::places`: `expression::{instantiate_call_contract_expression_place 10000, instantiate_outcome_contract_expression_place 10009, instantiate_contract_expression_place 10026, call_contract_argument_projection 10133, instantiate_call_contract_name_path_place 10166}`, `place_builders::{canonical_place_to_fact_place_in_state 10288, append_place_segment 10299, resolve_place_member_symbol 10311, append_place 10367, append_place_from_name_path 10382}`, `receiver::receiver_place_for_call 10418`, `substitution::call_contract_place_substitution 10465`, `instantiate_call_contract_place 10528`, `membership_argument_place 10603`.

**Inputs/outputs.**
- `build_proof_facts(program, proof_plan, borrow, operators) -> ProofFacts` (`:6332`): builds obligations, contract facts for machines, states, inherited traits, parameters, traits, closed requirement applications and operators, then call facts, operator-use facts, exit facts and the proposition vocabulary (`:6345-6534`). Binding passes (`bind_evidence_forwarding_facts`, `bind_evidence_projection_facts`, `bind_float_meaning_projection_facts`, `bind_outcome_specific_arm_facts`, `bind_proof_output_call_facts`) mutate `&mut ProofFacts` after that and return `Result<(), Vec<Diagnostic>>`; they are re-exported at `:6303-6311` and called from elsewhere (not read).
- `build_semantic_facts(program, proof, operators) -> Result<FactPlan, Diagnostic>` (`:9861`): starts from `validation::build_definition_fact_plan`, then `prepare_issuers`, proof obligations, contract facts, state-parameter domain facts, state-parameter predicates (on the checking thread pool with `rayon`, `:9895-9925`), zero-initialised local facts (`:9866-9876`).

**How it decides.** (Judgments and what they are keyed on.)
- Label matching is keyed on source spelling and rendered strings, by design (the header at `:31-35` says labels "are compared"):
  - `context_proves_boolean_label` compares `display_name(expression) == required_label` (`:132`), `place_label(...)` bases (`:109-121`).
  - `instantiate_operator_contract_expression_label` replaces a `Name` by operand label when `first_member == Some(parameter.name)` or `head_symbol == parameter.symbol` or `symbol == parameter.symbol` (`:412-414`); whether operand labels include `self` is decided by length equality `operand_labels.len() == parameters.len()` (`:400`).
  - `call_target_label` treats a state named `"entry"` of a machine with no attached data as the machine name (`:202`).
  - `symbol_name` searches machines, states, data fields in turn (`:236-263`).
  - Evidence terms are found by `(owner, kind, name)` string compare (`:2772-2784`, `:2353-2370`, `:5438-5460`, `:5937-5976`); evidence projections match `program.symbols.name(row.requirement) == projection.member` (`:2282`) and `checked_argument.diagnostic_label == label` where `label` is `format!("{}.{}", term, member)` (`:2301`, `:2313`).
  - `known` sets in definite-assignment and outcome proof are `BTreeSet<String>` of rendered labels, with entries like `format!("boolean:{}", ...)` (`:4962`, `:6153-6159`).
  - Substitution tuples are `(SymbolHandle, String, String)` with a literal `"result"` name (`:4831`, `:4956`).
  - Carry permission recognised by name: `CarryPermission::from_name` at `:6797`, `:7271`, `:9487`.
  - Contract parameter name match fallbacks: `parameter.name.as_str() == head_name` (`:7677`), `first_member == Some(parameter.name.as_str())` (`:10211`), `symbol_name(program, parameter_symbol) == parameter.name.as_str()` (`:10486`), `is_self_receiver(&symbol_name(..))` (`:10508`).
  - Float-meaning direct source `result` is recognised by `name.as_str() != "result"` (`:3129`) and `name.is_reserved_result()` (`:3525`).
- Keyed on identity (symbols, handles, expression structure):
  - `call_signatures::owns_contract` requires equal owner, caller symbols, statement index, call ordinal, source statement, source target, source expression, declaring trait, and signature equality (`:784-863`).
  - `repeats_subject` uses `expressions_structurally_equal` plus equal authored selection occurrences; comment says spans are diagnostic only (`:6590-6612`).
  - `saved_result_call` follows immutable local initialisers in admitted execution order (`:4667-4697`).
  - `applied_inherited_requirement` compares `type_identity` and applied scopes (`:7788-7849`).
  - Place facts are keyed on `(root, segments)` (`push_unique_place` `:7456-7473`).
- Dedup/lookup by linear scans: `applications.iter().find(...)` for `CallContractSignature` equality (`:742-747`); `projection_keys.iter().position` (`:3368`, `:3459`); `append_contract_fact_refs` scans all contract facts per call (`:1209`); `field_domain_walks` cache keyed on `TypeReferenceHandle` (`:9247`); `append_proposition_application_if_missing` uses `Vec::contains` (`:6027-6036`).

**Cases by shape.** (Quoted limits and fixed lists.)
- `proof_output_calls`: error text "is currently limited to a one-state Unit- or scalar-result checked machine or exact generic requirement" (`:5217-5221`); open requirement must be `Unit`, or exactly `I32`/`Bool` with no parameters, no ordinary arguments, and a free caller (`:5629-5647`); static dispatch error prefix "outside the first closed dispatch rung" (`:5687`); dispatch must be one-state `CheckedBody`, `Unit` or `I32`/`Bool` scalar with zero ordinary arguments and no receiver (`:5800-5830`); concreteness list at `:5779-5794`; `check_public_named_witness_lanes` returns "outcome-guarded and crash contract rows remain unsupported" (`:5893`) and demands each lane be one subjectless witness proposition (`:5915-5931`); trait parameters must be plain `Type` (`:5657-5667`, comment `:5651-5656` "which this rung does not instantiate").
- `bind_semantic_application_call` (`:3882`): only the sealed toolchain declaration, only `Meaning` results; `Bool`, `Class`, `Integer` operands return `Ok(None)` (`:3935`); comment: "unrepresentable operands bind no row rather than approximating" (`:3878-3880`).
- Float-meaning direct sources: only `F32`/`F64` (`:3152-3153`, `:3177-3178`, `:3216-3218`); unique contract carrier required (`:3053`); structural leaf requires a non-empty path of `Field`/`Case` segments (`:3083-3103`).
- `exact_outcome_case_test` (`:4758`): accepts `CaseMembership`, or `==` against `true` (recursing), or `==` against a variant name or an empty struct literal with a case symbol (`:4764-4803`).
- `exact_result_case` (`:4913`): only a name with valid `head_symbol` and `symbol`, or a struct literal with a case (`:4918-4927`).
- `selected_cases` (`:7095`): decomposes only `And` with builtin guard meaning and exact case membership; comment: "Authored operators and alternatives grant no case atom." (`:7091-7094`).
- `machine_parameter_evidence_signatures` (`:1894`): `RequirementIdentity` yields none; `Structural` yields a row (and nested parameters); `Nominal` only at top level (`:1918-1945`). Contains `.expect("typed machine-parameter contract must retain a valid requirement identity")` (`:1938`).
- `append_state_parameter_domain_facts`: mutable parameters get facts only when the reference is exclusive and the domain requires provenance, then `continue` (`:9292-9321`).
- `collect` in `zero_cases` handles literal-length `FixedArray` only; zero-length returns nothing (`:8915-8921`).
- `contract_fact_point`/`contract_fact_origin`: `CallSignature` owner returns `None` (`:9779`, `:9821`).
- `computed_case_subject` etc.: only `Call` nodes (`:6603-6604`).
- `obligations::lower_proof_obligation`: `BoundedInitializer` with an owner other than `StateLocal`/`ConstructorField`/`ArrayElement` gets invalid machine/state symbols (`:4300-4304`); `BoundedValue` always gets invalid ones (`:4326-4327`).
- `instantiate_transported_ensures` (`:3630`): operator uses only with `CheckedValueOrigin::StateStatement`, call ordinal set to 0 (`:3726-3740`).
- `bind_evidence_projection_facts`: skips applications that do not normalize (`:2224-2228`), errors if a projection names no term or interface row, or more than one row (`:2259-2299`).

**Same job elsewhere.**
- Contract clause fact building is done twice with its own lane counting: `append_contract_clause_facts` `:1831-1879` and the inherited-trait loop `:1425-1479` (uses `ContractEvidenceLanes` at `:1425`, but only increments the lane when a binding exists and the requirement is exact, `:1437-1441`, whereas `:1845-1851` increments only when `contract.binding` is `Some`). The comment at `:1748-1753` says the lane position "must always agree".
- Cycle guard in field-domain walkers keyed two ways: `zero_cases::collect` uses `(data.symbol, generic_instance)` (`:8966-8970`); `collect_zero_initialized_field_domains` uses `data.name.as_str()` (`:9158-9162`). Two same-named data types in different modules would collide in the second.
- Mapping a contract owner to caller machine appears in `products.rs:595` and `:928` (see section 4) and in `call_signatures::owner` use sites here (`:764`, `:793`, `:872`, `:952`, `:7201`, `:7691`).
- Finding an `ExpressionNode::Name` parameter by name or symbol: `:412-414` (labels), `:7676-7679`, `:10211-10214`, `:10485-10486`.
- Place construction from a contract expression is done in three places with the same structure for `Borrow`/`Name`/`Member`/`Indexed`: `contracts::places::contract_expression_place` `:7475-7592`, `expression::instantiate_contract_expression_place` `:10026-10127`; both repeat the "case variant segment before field segment" step (`:7556-7566`, `:10097-10107`, `:7637-7645`, `:10245-10251`).
- Outcome case detection exists here (`exact_outcome_case_test` `:4758`) and `dispatch_subjects::computed_case_subject` calls it (`:6585`); stage 04 has its own `dispatch_subjects` module (`04_typed-trees-to-typed-trees/src/selection_and_reflection.rs:3094`, grep `dispatch_subjects`); I did not read it ("suspected" overlap only in name and subject).
- `CarryPermission::from_name` appears 14 times outside tests (grep `CarryPermission::from_name` in `omega-rust`: 17 hits, 3 in `language-semantics/src/tests.rs`); 3 are in this file.
- Float-meaning keys deduped through `Vec` search twice (`:3368-3377`, `:3459-3464`).

**Unused / one-caller (grep, `*.rs` under `omega-rust`).**
- `instantiate_operator_contract_expression_label`: comment says it "is shared by requires discharge and by flow introduction of ensures facts" (`:277-283`). Outside `published.rs` it has 14 use lines in `checks_operators.rs` (8), `flow.rs` (3), `published_facts.rs` (3) (grep above; counted from the listing). The claim holds.
- `boolean_expression_has_polarity`: comment says "Both the operator-use discharge and the statement-entry discharge" use it (`:438-443`). External uses: `published_facts.rs:8108`, `checks_operators.rs:1758` (2). Claim holds.
- `context_proves_boolean_label`: comment says it is "shared by selected operator requires and named crash-route discharge" (`:38-41`). External uses: `published_facts.rs:8164`, `checks_operators.rs:2303` (2, plus imports).
- `proposition_label_sources`: comment says every proposition label instantiation "prepares identically" (`:580-584`). External uses: `checks_contracts.rs:1161`, `:1450`, `flow.rs:4477`, `checks_operators.rs:2188`, plus 2 in this file (`:5997`, `:8522`) = 6 call sites.
- `dispatch_subjects`: external users are `published_facts.rs:10251` and `execution_terminal_unit_state_graph.rs:144`, `:689`, `:763`.
- `place_at(`: 274 occurrences in 48 files under this crate's `src` (Grep count mode; includes imports and definitions); 4 in `published.rs`. The doc comment at `:613-619` says checking asks this question at every operand.
- `shared_argument_referent|view_argument_names_existing_place|append_observation_context|exact_outcome_case_test|machine_parameter_evidence_signatures|contract_target_from_state_symbol`: 72 occurrences in 19 files (count mode, includes definitions and re-exports); 21 in `published.rs`.
- `call_target_label|machine_name(` : 65 occurrences across 11 files (count mode); the pattern also matches other `machine_name` identifiers, so it is not a clean call-site count.
- `receiver_place_for_call` ignores a `Statement` receiver path unless `statement_call_receiver_members(...)` is `Some` (`:10426-10440`).
- `intake_call_ensures_propositions` computes `_receiver` and never uses it (`:6080-6083`).
- `labels::names::machine_name` is re-exported at `:602`; its internal callers in this file: `symbol_name` not; I did not count.
- Public `pub fn` names in this file are all `pub(crate)`; none is `pub`.

**Stated limits.**
- "Transitional exact typed-expression custody for source forms whose artifact-reconstructible Terminal coordinate has not landed yet." (`:2873-2876`) on `TypedExpression`.
- "Applications ... projection source stays transitional until lowering rejoins it to the Terminal `SemanticApplication` carrier" (`:2792-2794`).
- "Binding records identity; it does not itself prove equality." (`:2796-2797`).
- "Top-level machine parameters retain the historical behavior" (`:1884`).
- "outcome-guarded and crash contract rows remain unsupported" (`:5893`).
- "Mutable bindings and non-binding value transformations remain fail-closed." (`:4435-4436`).
- "the first closed dispatch rung" (`:5687`).
- Hard-coded budget `&mut 4096` passed to `lower_state_scalar_contract_predicate` (`:8225`); no comment.
- `#[allow(clippy::too_many_arguments)]` on `calls::retain` and many others (read).

**Fail-open risks.**
- `instantiate_call_contract_payload` returns early on many failures (`:8387-8551`), leaving `instantiated` invalid. `append_call_semantic_contract_refs` then drops only `ContractBooleanExpression` payloads with invalid `instantiated` (`:8583-8586`); a `ContractPropositionApplication` whose instantiation failed is still published with `instantiated` invalid (the proposition path at `:8509-8551`). What could pass: a call-site proposition fact carrying the declaration-level (uninstantiated) identity. Whether consumers treat an invalid `instantiated` as "no match" was not traced. "open".
- `call_parameter_domains` `continue`s for a constraint whose `domain_symbol` is invalid and whose name is not a carry permission (`:6796-6806`), so no obligation is added for that constraint. Which program gets through: a parameter qualified by a constraint name that neither resolves to a domain nor to a carry permission. "open".
- `append_place_from_name_path` silently skips member symbols that are invalid (`:10387-10391`), producing a shorter (coarser) place; used by `receiver_place_for_call` (`:10438`). Effect on facts keyed on that place not traced. "open".
- `instantiate_call_domain_membership_instance` leaves `semantic_domain` unchanged when the substituted identity is not already interned (`:8382-8384`); comment: "adopt only an already-interned identity ... identity alone never establishes membership. Missing evidence still fails ordinary entailment." (`:8291-8293`). "stated".
- `append_contract_semantic_facts` skips a contract fact when `contract_fact_point` or `contract_fact_origin` is `None` (`:7890-7895`); `CallSignature` owners return `None` by design (`:9779`, `:9821`) and are published through the call-site path instead (`:7971-8032`). "traced" (loss of a fact, not an invented one).
- `build_contract_exit_facts` includes an exit when completion is unknown: `expression_normal_completion_possible != Some(false)` and `normal_completion_possible != Some(false)` (`:1116-1120`, `:1147-1150`, `:1175`). Unknown completion keeps an exit (more obligations). Fail-closed direction. "traced" at the site (no comment).
- `owns_contract` returns `!matches!(contract.owner, CallSignature)` for a call without a signature (`:790-792`): a call with no retained signature owns every non-signature contract. No comment.
- `ProofObligationOwner::Unknown` maps to `ProgramPoint::Global` (`:9732`) and `FactOwner::Unknown` also to `Global`/`FactOrigin::Unknown` (`:9780`, `:9822`). A malformed owner is published at global scope rather than rejected. "open".
- `bind_outcome_specific_arm_facts`: when the arm has no selectors, every failure to classify the guard, saved call, target state or case silently `continue`s (`:4465-4521`); with selectors an error is pushed. What gets through: an arm that tests an outcome but is not bound to producer guarantees when no selector is written. Whether later checks still demand the guarantee was not traced here. "open".
- `bind_proof_output_call_facts` has a final `else { continue; }` for a call that is neither a machine target nor an open requirement (`:5206-5208`). "traced": unreachable, because `:5055-5074` pushes an error and continues in that case.
- `validate_evidence_forwarding_definite_assignment`: a machine with no entry state, or a transition target that is not found, `continue`s or reports missing evidence (`:2483-2485`, `:2513-2515`, `:2604-2616`). Entry-less machine skipped without a diagnostic. "open".

---

## Cross-file observations

- `published.rs` exports `labels::*`, `places::place_at`, `proof::*` and `semantic::*` up to the rest of the crate; its largest consumers named by grep are `published_facts.rs`, `checks_operators.rs`, `checks_contracts.rs`, `flow.rs` (see counts above).
- Both `flow_transfers.rs` and `published.rs` use string/label equality for some identity decisions and structural/symbol identity for others; the two styles sit in the same module (section 5 "How it decides").
- Three crate-level `use` aliases `crate::flow::...` in `flow_transfers.rs` refer to helpers in `flow.rs` (`place_cases_are_selected`, `source_transfer_cases_selected`, `fixed_selection_segment`, `place_segment_has_static_identity`, `canonical_places_agree`, `literal_expression_projections`, `operand_binds_live_view`, `sole_local_referent_origin`); not read.

---

## Coverage

Read in full (no skimming):
- `flow_transfers.rs` 1-4593 (4593 lines)
- `immutable_checking_input.rs` 1-130 (130 lines)
- `lib.rs` 1-86 (86 lines)
- `products.rs` 1-1414 (1414 lines)
- `published.rs` 1-10658 (10658 lines)

Total read: 16881 lines. Skimmed: 0. Not read: 0.

## Five things a maintainer most needs to know about this unit

1. `flow_transfers.rs` has several separate "move this fact payload to that place" lanes (`context_transport` `:704`, `projected` `:1427`, `constructed` `:520`, `owned_qualifications` `:1130`), each with its own list of accepted payload kinds and its own gate. Adding a payload kind to one lane does not add it to the others (compare `:744-750`, `:1494-1522`, `:644-647`).
2. Scalar capture across calls (`scalar_values::calls`, `:1867-2533`) only works for receiverless, no-machine-argument calls into `CheckedBody` machines with no owned data and only primitive parameters; the receiver path is rejected at `:2347` while `CallValues.fields` (receiver reads) exists but is always empty (`:2195`, `:2456`).
3. `published.rs` matches many things by rendered string or source name: evidence term names, projection member names, `"result"`, `"entry"`, `"boolean:..."` labels, carry permission names, parameter names (`:132`, `:202`, `:2282`, `:2313`, `:4962`, `:6797`, `:7677`, `:10211`, `:10486`). The header at `:31-35` says some labels are compared.
4. `proof_output_calls` and its dispatch/requirement checks are deliberately limited to one-state `Unit`/`I32`/`Bool` shapes with concrete non-generic traits (`:5217-5221`, `:5629-5647`, `:5800-5830`); everything else yields an error diagnostic.
5. `products.rs` pruning is an explicit list of machine-keyed tables (`:659-920`) plus a retention closure keyed on machine symbols (`:1139-1182`); `prune_checked_tree_product` has no caller outside the file other than `select_checked_tree_product`, which has one external caller (`source_checking.rs:834`).

## What you are unsure about

- Whether `observation_places` empty (`:3599-3602`) fully matches the "complete storage footprint is empty" comment (`:2275-2278`); I did not read `call_storage_writes` (`flow_call_phases.rs`).
- Whether downstream consumers treat `instantiated == invalid` on a call-site proposition fact as "never matches" (`:8509-8551`, `:8583`); I did not read the consumers in `checks_contracts*.rs`.
- Whether skipped outcome arms (`:4465-4521`) are caught by another check when no selector is written.
- Whether the `any`-over-contexts rule at `:1092-1105` (`declared_domains::establish`) is intended to mean "at least one source context proves membership".
- I did not grep external callers of seven of the thirteen `package_review` wrappers; their usage is "suspected" only.
- Counts for `place_at(` (274) and the six-name pattern (72) are raw occurrence counts from Grep count mode and include imports, re-exports and definitions.
