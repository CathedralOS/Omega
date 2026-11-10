# Unit 90 notes

Files covered (paths relative to `<checkout>/`):

1. `omega-rust/psi/compilation/validation/src/call_cycles.rs` (lines 1-4256)
2. `omega-rust/psi/compilation/validation/src/calls.rs` (lines 1-14739)

Both are files in the `validation` crate and are mounted as modules under `crate::machine_calls` (`crate::machine_calls::call_cycles`, `crate::machine_calls::calls`; paths used by callers at e.g. `contract_entailment.rs:29664`, `calls.rs:5490`). (read; mount inferred from those call paths.)

Citations below are `file:line` with file `cc` = call_cycles.rs and `calls` = calls.rs.

---

# 1. call_cycles.rs (4256 lines)

## Purpose
Builds the machine call graph, finds its strongly connected components (call cycles), and decides which cycles are admitted. Three separate admission regimes live in one file: (a) proof-only machines (structural descent on a data member, or "interval" descent on a computed rank), (b) runtime machines (tail calls only, plus an optional termination ranking over natural-number-like measures), (c) selector/boundary-requirement self-recursion, which is only accepted if the machine carries a termination witness. It also holds the serialization of the runtime ranking plan into / out of the `checked_trees::CheckedRankArrivalPlan` arenas, and the generic "dependency symbols" walkers used by other modules.

## Shape (all `cc:` lines)
- Top-level re-exports and imports `cc:5-13`.
- `mod proof_components` `cc:15-247`: `CompleteProofCallComponent{members, edges}` `cc:25`; `complete` `cc:30`; `complete_for_machine` `cc:39`; `complete_with_demand` `cc:55`; `structural_return_descends` `cc:147`; `StructuralDescentComponent` / `StructuralDescentComponents` `cc:199-208`; memoised `structural_descent_components` `cc:210` (via `frozen_program::frozen_memo`, `cc:213`).
- `mod proof_intervals` `cc:249-392`: `validate_component` `cc:256` returns `Result<Vec<ValidatedProofRankInterval>, &'static str>`.
- `mod proof_ranking` `cc:393-567`: `RankParameter` `cc:403`, `ComponentEdge` `cc:409`, `validate_component` `cc:415` returns `Option<ValidatedProofRecursiveComponent>`, `member_path` `cc:551`.
- `mod rank_components` `cc:568-998`: `RuntimeRankComponents{adjacency, components}` `cc:578` with methods `require_tail_component` `cc:584`, `requires_arrival_witness` `cc:597`, `component_containing` `cc:600`, `unranked_component_reached_from` `cc:606`, `joint_components` `cc:637`, `judge` `cc:656`; free fns `prepare_runtime_rank_components` `cc:679`, `retain_runtime_rank_component` `cc:702` (writes the decision into `CheckedRankArrivalPlan`), `retained_runtime_recursive_components` `cc:830`, `retained_runtime_natural_components` `cc:848` (reads it back), `retained_internal_rank_nonincrease` `cc:955`, `internal_rank_nonincrease` `cc:969`.
- `mod rank_relevance` `cc:999-1184`: `RankRelevance{states: Vec<(SymbolHandle,bool)>}` `cc:1008`, `rank_relevance` `cc:1018`, `boundary_excludes_synchronous_entry` `cc:1136`.
- `mod runtime_ranking` `cc:1190-3369`, with submodules `meaning` `cc:1191` (`binary_is_inert` `cc:1196`), `occurrences` `cc:1240` (`require_classified_tail_occurrences` `cc:1245`), `prefix` `cc:1310` (`preserves_rank` `cc:1320`, `operand_has_admitted_meaning` `cc:1451`), `projection` `cc:1462` (`RankOrder` enum `cc:1475`, `RankProjection` `cc:1497`, `RankProjection::resolve` `cc:1507`, `captured_components` `cc:1767`, `natural_component` `cc:1909`, `retain_internal_transfer` `cc:2001`), `inputs` `cc:2061` (`RuntimeRankInput` `cc:2066`, `RuntimeRankOperand` `cc:2082`), `application_bindings` `cc:2317` (`RuntimeRankApplicationBinding` `cc:2319`). Then `ValidatedRuntimeNaturalState/IntegerRank/Edge/Component` `cc:2387-2420`, `require_tail_component` `cc:2422`, `extend_runtime_adjacency` `cc:2452`, `supplied_requirement_implementations` `cc:2511`, `dispatched_implementations` `cc:2542`, `RuntimeRankBodyWitness` `cc:2574`, `project_component` `cc:2582` (the main runtime ranking judge, ~430 lines), `admit_ranked_edge` `cc:3012`, `weak_edges_are_acyclic` `cc:3181`, `ranged_member_site_invariant` `cc:3189`, `InternalArrival` / `internal_arrival_sites` `cc:3218-3227`, `internal_state_index` `cc:3276`, `activation_target_state_index` `cc:3285`, `target_machine` `cc:3296`, `expression_is_inert[_or_calls]` `cc:3309-3318`.
- Public data types for the validated results `cc:3377-3456` (`ValidatedProofRankingRelation`, `ValidatedProofRecursiveCallSite`, `ValidatedProofRecursiveMember/Edge/Component`, `ValidatedProofCallComponent`, `ValidatedProofRankInterval`, `ExactProofCallEdge` `cc:3451`).
- Dependency-symbol walkers `cc:3458-3726`: `machine_call_dependency_symbols` `cc:3458`, `declaration_call_dependencies` `cc:3475`, `requirement_backing_machines` `cc:3579`, `collect_contract_dependency_symbols` `cc:3602`, `collect_proposition_dependency_symbols` `cc:3625`, `collect_statement_dependency_symbols` `cc:3647`, `collect_expression_dependency_symbols` `cc:3705`.
- Graph build and top-level validation `cc:3728-3923`: `MachineCallGraph{edges, proof_dependencies, selector_self_recursion}` `cc:3728`, `build_machine_call_graph` `cc:3734`, `validate_machine_call_cycles` `cc:3756` (emits the diagnostics), `infer_checked_proof_recursive_machines` `cc:3834`, `build_validated_proof_recursive_components` `cc:3872`.
- Proof-call edge collection `cc:3925-4193` (`proof_call_site_sort_key` `cc:3925`, `collect_proof_call_dependencies` `cc:3970`, `collect_exact_proof_call_edges[_in_machines]` `cc:3999-4006`, statement / expression walkers `cc:4038`, `cc:4134`), `proof_rank_parameter` `cc:4195`, `expression_member_path` `cc:4220`, `exact_member_field` `cc:4247`.

## Inputs and outputs
Reads `&dyn crate::CheckingInput` (the typed program: `machines()`, `machine_states`, `statement_table`, `state_parameters`, `dynamic_slots`, `conformances()`, termination plans on machines) and `ProofOnlyClassification`. Outputs: diagnostics pushed into `&mut Vec<Diagnostic>` (`cc:3790`, `cc:3823`); `Vec<ValidatedProofCallComponent>` (`cc:3831`); `Result<Option<ValidatedRuntimeNaturalComponent>, &'static str>` from the runtime judge (`cc:2582`); retained rows written into `CheckedRankArrivalPlan` arenas (`cc:702-828`). `structural_return_descends` consults a frozen memo (`cc:213`) that is built once per program.

## How it decides
- Cycle membership: Tarjan-style `strongly_connected_components` over `Vec<Vec<usize>>` machine-position adjacency (import `cc:6`; used `cc:63`, `cc:302`, `cc:520`, `cc:684`, `cc:1091`, `cc:3182`, `cc:3780`). Keyed on machine position (index into `program.machines()`), with `SymbolHandle` for states.
- Proof components are kept only when every member is proof-only (`cc:92-94`) and the exact call inventory (`certified_calls`) equals the dependency-derived (`resolved_calls`) set (`cc:120-138`); otherwise the component silently drops out (returns `None` at `cc:137`), and `validate_machine_call_cycles` then reports "the exact call inventory is incomplete" (`cc:3822`) if no more specific reason is recorded.
- Proof ranking by structure (`proof_ranking::validate_component`): finds one rank parameter per member by matching the authored witness `subjects[0]` name against entry parameter **name** (`proof_rank_parameter`, `cc:4203-4211`: `parameter.name.as_str() == caller_subject.as_str()`), and compares carrier types by `package_qualified_type_identity` (`cc:445`, `cc:563`). Candidate sets are narrowed to a fixed point (`cc:474-498`) and must end with exactly one candidate per member (`cc:499`). Each edge's actual argument must be a member path off the caller rank parameter (`expression_member_path`, `cc:4220`), and edges with an empty path (preserving) must form an acyclic graph (`cc:520-530`).
- Proof ranking by interval (`proof_intervals::validate_component`) runs only when structural validation fails (`cc:3896-3910`). Requires all ranked members to share one "well-founded carrier" identity (`cc:289-294`), no helper-only cycle (`cc:302-312`), and every closing call proved smaller by `proof_interval_strict_decrease_judged` (`cc:357`), with a single order relation across intervals (`cc:370`).
- Runtime ranking (`project_component`, `cc:2582-3009`): per-member `RankProjection::resolve` (`cc:1507`) picks one ranking order keyed on `witness.ranking_view` identity AND `view_path` string equal to `ranking_view.canonical_path()` (`cc:1520-1521`, `cc:1548-1549`, `cc:1581-1582`); for an authored, non-builtin view the measure is found by comparing the `witness.view_path` string split on `::` to `measure_path_members` (`cc:1681-1687`) and the owner string compared to `program.symbols.display_path(*data, "::")` (`cc:1695-1697`). All members must share the same `RankOrder` (`cc:2596`). Strictness of each call edge is decided by `prove_captured_rank_call` / `prove_ranking_range_call` (`cc:3057`, `cc:3126`) or, when the caller had an inert write (`entry_relative == false`), by finding a witness arrival edge with `strictly_decreases` (`cc:3101-3115`). A cycle is accepted only if the graph of non-strict edges is acyclic (`cc:3004`, `cc:3181-3187`).
- Tail-call rule (`require_tail_component`, `cc:2422`; `require_classified_tail_occurrences`, `cc:1245`): every call occurrence whose callee is in the component must be classified as completing a transition (`call.completes_transition`, `cc:1302`).
- Inertness of expressions (`expression_is_inert_or_calls`, `cc:3318`): a subtree is inert unless it contains an atomic-once operation, a binary operator without builtin meaning, indexing/ranges without builtin meaning, or a call not admitted by the closure passed in.
- Relevance (`rank_relevance`, `cc:1018`): a state "requires rank" if it is in a cycle containing a ranked machine, can reach one, or has an opaque call. Unknown states default to requires-rank (`cc:1014`, `unwrap_or(true)`).

## Cases by shape
- `RankOrder` (`cc:1475-1495`) is a closed list: `Natural`, `IncreasingTo`, `BoundedDistance`, `SliceLength`, `DeclaredIdentity`, `DeclaredComputation`, `Lexicographic`. `RankProjection::resolve` returns `None` for any other witness (`cc:1620-1625`, `cc:1662-1664`), which `project_component` reports as "a member lacks a supported exact ranking witness" (`cc:2594`).
- `Lexicographic` requires the subject's type to be a named data type whose measure body is a list of single-name paths each naming a distinct fixed-integer field (`cc:1700-1735`).
- `RankOrder::BoundedDistance` requires exactly two subjects (`cc:1523`, `let [lower, upper] = ...`); all other orders exactly one (`cc:1544`).
- `natural_component` (`cc:1909-1999`) builds the retained "natural" component only for `RankOrder::Natural` and `Lexicographic`; any other order returns `None` (`cc:1948`), which `project_component` converts into an empty component (`cc:2668-2671`, `unwrap_or(ValidatedRuntimeNaturalComponent{states: Vec::new(), edges: Vec::new()})`).
- `admit_ranked_edge` handles the range-proof path only for six of the seven orders: Natural, IncreasingTo, BoundedDistance, SliceLength, DeclaredIdentity, DeclaredComputation (`cc:3116-3124`); `Lexicographic` without captured components falls to "ranked invocation has no complete captured value comparison" (`cc:3156`).
- Proof ranking `ranking_relation` has two values `StructuralSubterm` / `StructuralSubtermCycles` (`cc:531-535`).
- `proof_call_site_sort_key` `cc:3925` handles exactly the three `ValidatedProofRecursiveCallSite` variants; `Transition` lanes `Target`/`Continuation` only.
- Statement walkers `collect_statement_dependency_symbols` (`cc:3647`) and `collect_exact_proof_statement_edges` (`cc:4038`) both enumerate the `StatementNode` variants explicitly (no wildcard arm).

## Same job elsewhere
- Two near-identical walks over statements collecting call targets: `collect_statement_dependency_symbols` `cc:3647-3703` (symbol-level, includes selection dependencies) and `collect_exact_proof_statement_edges` `cc:4038-4131` (edge-level with arguments and a site). A third walk, `collect_call_sites_in_statement`, is imported from `calls.rs` (used `cc:1053`, `cc:1254`, and `calls.rs` defines it; see calls.rs section). (read)
- Three separate "is this cyclic SCC" predicates written inline: `cc:88-91`, `cc:302-309`, `cc:520-527`, `cc:686-690`, `cc:1092-1095`, `cc:3185` (same expression `len()>1 || contains self-edge`). (read)
- `internal_state_index` (`cc:3276`) and `activation_target_state_index` (`cc:3285`) are two spellings of "state position of a target symbol in a machine", one excluding the entry, one mapping machine symbol to 0. `calls.rs` also defines `find_state` / `machine_state_by_symbol` (see calls.rs). (read)
- Retain/restore pair `retain_runtime_rank_component` `cc:702` and `retained_runtime_natural_components` `cc:848` mirror each other; `RuntimeRankOperand::retain` `cc:2151` / `from_checked` `cc:2240` and `RuntimeRankApplicationBinding::retain` `cc:2327` / `from_checked` `cc:2361` likewise. (read)

## Unused or one-caller code (grepped: `Grep` over `omega-rust`, content mode)
- `structural_return_descends`: 1 call site outside the file (`calls.rs:5490`).
- `ranged_member_site_invariant` (`cc:3189`): 1 call site (`contract_entailment.rs:29664`).
- `declaration_call_dependencies` (`cc:3475`): 2 call sites (`machine_calls_core.rs:2897`, `:2906`).
- `machine_call_dependency_symbols` (`cc:3458`): 1 external call (`quotients.rs:475`) plus 1 internal (`cc:3496`) plus re-export `lib.rs:1714`.
- `infer_checked_proof_recursive_machines` (`cc:3834`): 1 call site (`checks_termination.rs:7906`) plus re-export `lib.rs:1713`.
- `requirement_backing_machines` (`cc:3579`): 3 callers (`checks.rs:4576`, `checks_contracts.rs:6665`, `cc:3554`) plus re-export `lib.rs:1705`.
- `activation_target_state_index` (`cc:3285`): ~22 uses across the `05_typed-trees-to-checked-trees` crate (e.g. `execution.rs:630,675,2621`, `flow_transfers.rs:2177`, `values_scalar.rs:5096`) plus re-export `lib.rs:1713`.
- The five rank-plan functions (`retain_runtime_rank_component`, `retained_runtime_recursive_components`, `retained_runtime_natural_components`, `retained_internal_rank_nonincrease`, `prepare_runtime_rank_components`) are `pub use`d at `cc:1185-1189` and appear in `lib.rs` and the 05 crate (`checks_termination.rs` 4 hits, `checks_termination_progress.rs` 2 hits, per the combined count); I did not count each name separately. (grepped, combined count only)
- `proof_components::StructuralDescentComponents` is `pub(crate)`; only used as the return type of the private memo accessor (`cc:210`). (read)

## Stated limits
- "termination across implementation selection is not yet ranked" `cc:3791-3794` (diagnostic text for selector/boundary-requirement self re-entry without a witness).
- "runtime calls must be tail; a termination promise additionally requires strict descent on every complete cycle" `cc:718-719`.
- Diagnostic text lists the limits of the proof-only path: "exact arguments must preserve or descend from the ranking subject, and preserving edges must be acyclic" `cc:3825-3826`.
- "a subordinate-state call needs its own arrival evidence" `cc:2952-2953` (calls into a non-entry state of a component machine are refused).
- No `TODO`/`unsupported` comments in the file; the only comments are the file header `cc:1-4` and a few inline (none state migration debt). (read)

## Fail-open risks
- `RankRelevance::requires_rank` `cc:1013-1015`: unknown state defaults to `true` (requires rank), i.e. fail-closed. Not a risk.
- `retain_internal_transfer` `cc:2012-2018`: if the target state's relevance says no rank is needed it returns `true` (accept) without checking the arrival row. Which states are "not relevant" is computed by `rank_relevance` (`cc:1090-1126`), which marks opaque-call states and cyclic-through-ranked states and their predecessors as relevant. A state is not relevant only if it is not in any ranked cycle, cannot reach one, and has no opaque call. Traced: reasoning by reading; an internal arrival to a state outside every ranked cycle cannot form a cycle, so accepting is consistent. Label: traced (reading only).
- `internal_rank_nonincrease` `cc:974-996`: states not requiring rank are skipped (`cc:980-982`); same reasoning as above. Traced (reading only).
- `project_component` `cc:2764-2805`: a non-transition statement that is `Assembly`/`AssemblyFact` is accepted without a rank-preservation check (`cc:2765`), as are inert `LocalData`/`Expression` statements (`cc:2766-2779`), and inert `Assignment` (which only clears `natural.states` and sets `entry_relative=false`, `cc:2781-2799`). Reasoning: assembly statements are not examined for writes to rank coordinates. Whether assembly blocks can write rank parameters is not decided in this file. Label: open for `StatementNode::Assembly`.
- `project_component` `cc:2924-2943`: a tail call to a target that is not a machine in this program (`callee == None`) is accepted if the retained contract or selected requirement signature has `TerminationGuarantee::Terminates { premises }` with empty premises. Otherwise error. Stated by the error text at `cc:2940` ("no unconditional public termination contract"). Traced to the signature check; label stated.
- `project_component` `cc:2956-2960`: a tail call to a machine outside the component is skipped (`continue`) with no check. Reason: callee outside the component cannot form a cycle with it. Label: traced by reading (component is an SCC, so a callee outside it cannot return to it).
- `project_component` `cc:2983-2991`: a tail target that is a `Value(expr)` is accepted when the expression is inert; `Terminal` is accepted. Label: traced.
- `project_component` `cc:2624-2626`: when a body has no witness (`witness` is `None`) the member is skipped entirely in the per-statement loop (`cc:2719-2721`, `continue`). The component then has `natural = None` (`cc:2661-2675`: `complete` false) and the result is `Ok(None)`. So a runtime cycle whose members lack source-body witnesses is accepted without a natural component (the later `adjacency` check at `cc:2996-3001` is also skipped for those members). What this lets through: a component where some member's body was not witnessed is judged only by its other members. Label: open (no comment; I did not trace who supplies `witnesses` — `RuntimeRankComponents::judge` receives them from the 05 crate).
- `unranked_component_reached_from` `cc:606-636`: `seen.get_mut(position)?` returns `None` (= "no unranked component found") on an out-of-range index. Label: open, reachable only with inconsistent adjacency.
- `validate_machine_call_cycles` `cc:3780-3799`: a single-member SCC is only diagnosed if it is in `selector_self_recursion`; plain self-recursion of a single machine is not diagnosed here (`continue` at `cc:3798`). Direct self-recursion is handled by the separate runtime ranking path (`RuntimeRankComponents` includes self-edge components, `cc:687-690`). Label: traced to `prepare_runtime_rank_components` only.
- `proof_components::complete_with_demand` `cc:136-138`: a mismatch between exact and resolved call sets drops the component (fail-closed, then diagnosed at `cc:3822`).
- `exact_member_field` `cc:4254-4255` returns `None` on unknown data; `member_path` then `None`; candidate dies; fail-closed.

---

# 2. calls.rs lines 1-14739 (the file itself is 44215 lines by `wc -l`; this unit is its first 14739; my range ends inside `CallFrameResolver::statement_reference_transfer`, which starts at calls:14733)

(Header comment `calls:1-5`: "Statement- and expression-position machine call validation, argument correspondence, receiver and requirement dispatch, and complete-or-opaque write-frame analysis".) The file is a flat sequence of `pub(crate) mod` blocks. Sections read so far (lines 1-6400) are below; later sections are appended under "calls.rs continued".

## Purpose (whole file, as read)
Validates every machine call site in a checked body: that the callee resolves, that arguments match parameters (types, ranges, borrow access), that results are used, that recursion is only the allowed forms, and that dispatch (named conformance, dynamic `dyn Trait` selector, boundary trait, closed/static requirement dispatch) is exact. Emits `Diagnostic::error` strings into a `&mut Vec<Diagnostic>` and `IntegerRangeObligation`s into `formation_obligations`. Nearly every function returns `()` and reports by pushing a diagnostic, so "accept" means "push nothing".

## Section map, lines 1-6400 (read in full)
| Lines | Module / item | What it does |
|---|---|---|
| calls:24-448 | `closed_dispatch`: `closed_requirement_dispatch_is_valid` :31, `validate_closed_requirement_dispatch` :40, `capture_binding_is_exact` :218, `conformance_binding_is_exact` :256 | Re-derives a `StaticRequirementDispatch` from program tables and checks every row agrees (unique supplying application, template chain acyclic, realization entry matches, public contract scope, trait declares requirement exactly once, exactly one implementation row). All failure paths return `Err(&'static str)`. |
| calls:451-639 | `argument_bounds`: `report_argument_bounds` :461, `report_accessor_scalar_write_predicate` :544 | Range/narrowing check of an argument against a parameter's primitive type; separate check that a write to a placed schema field keeps the source declaration predicate. |
| calls:640-961 | `call_gates`: `validate_named_conformance_call` :650, `receiver_is_declaration_namespace` :737, `receiver_is_operator_namespace` :761, `receiver_is_declared` :786, `validate_asm_statement_call` :830, `self_entry_call_is_banned` :932 | Statement-call front gates. |
| calls:962-1296 | `dynamic_requirement`: `dynamic_selector_call` :977, `validate_dynamic_selector_arguments` :1119, `validate_family_tuple` :1150, `expression_trait_receiver` :1268, `selector_type` :1283 | Resolves a call through a `dyn Trait` selector (first argument is the selector) to a requirement signature and trait application. |
| calls:1297-2868 | `expression_scanning` with sub-modules `result_realization` :1319, `target_resolution` :1864, `traversal` :2090 | Walks every expression in value position, running per-node checks. `ScanContext::scan_at` :2429 is a match over every `ExpressionNode` variant (:2522-2855). |
| calls:2870-3615 | `fence_generic_value_callee` :2870, `receiver_member_chain` :2893, `ValueCallee` :2921, `ValueCallSite` :2949, `CallExpressionUse` :3021, `validate_expression_call_bounds` :3028 (~530 lines), `validate_named_float_to_integer_call` :3561 | Expression-position call validation: a long ordered chain of "is this call kind X? validate and return" tests. |
| calls:3617-3842 | `generic_bounds`: `validate_machine_call_type_parameter_bounds` :3630, `report_bodyless_boundary_symbol_call` :3670, `validate_type_parameter_instantiation_bounds` :3788 | Type-parameter bound satisfaction at a call; error when a bodyless boundary symbol is called with no executable realization. |
| calls:3843-4249 | `generic_requirement`: `selected_requirement_signature` :3853, `requirement_namespace_member` :3896, `requirement_contract_signature` :3908, `resolve_requirement_contract` :3916, `validate_requirement_call_arguments` :3965, `named_conformance_requirement` :4062, `validate_named_conformance_arguments` :4073, `unresolved_implementation_call` :4178, `call_value_arguments_match` :4203 | Conformance-bound (evidence-parameter) requirement calls. |
| calls:4255-4402 | `inline_assembly`: `register_read_mnemonic` :4264, `user_asm_contract` :4276, `validate_asm_operand_constraint` :4290, `validate_asm_value_destination` :4367 | Operand checking for asm intrinsics against the shared `language_core::inline_assembly` catalog. |
| calls:4403-4469 | `machine_parameter_arguments::validate_machine_parameter_arguments` :4408 | Calls through a machine parameter. |
| calls:4470-4800 | `type_arguments`: `call_type_arguments` :4477, `requirement_call_type_arguments` :4500, `callable_type_arguments` :4524, `declaration_call_type_arguments` :4554, `scoped_callable_type_arguments` :4581, `const_selection_matches` :4721, `validate_resolved_call_arguments` :4735 | Builds `(type-parameter symbol, TypeReferenceHandle)` bindings for a call from selected static args plus inference from actuals. |
| calls:4804-5087 | `receiver_calls`: `validate_receiver_call` :4816, `validate_resolved_receiver_call` :4950, `validate_native_callback_selections` :5048 | Statement calls with a receiver (`a.b(..)`). |
| calls:5088-5270 | `receiverless_calls`: `validate_receiverless_call` :5097, `validate_resolved_receiverless_call` :5230 | Statement calls with no receiver. |
| calls:5271-6034 | `recursion` with `proof_machines`: `validate_proof_machine_recursion` :5495, `guarded_integer_predecessor_call` :5382, `slices::guarded_slice_tail_call` :5296, `substate_parameter_descends` :5675, `strict_subterm_of_measure` :5803, `ValidatedSelfCallDescent` :5372; and `validate_self_recursive_call_positions` :5879, `reject_embedded_self_calls` :5993, `recursive_entry_path` :5945, `is_current_entry_call` :5980 | Self-recursion rules: proof machines need a measure with proven descent at each self call; runtime machines may only recurse in tail (transition) position. |
| calls:6035-6077 | `result_use::validate_result_use` :6040 | Error when a non-unit result is discarded. |
| calls:6078-6185 | `signature_queries`: `CallDestinationQueries` :6084, `call_argument_destinations` :6101, `call_argument_signature` :6125, `unique_state_by_symbol` :6158, `call_receiver_member_names` :6174 | Memoised (via `checking_memos().call_destinations`, `calls:6107-6112`) list of parameter type refs for a target. |
| calls:6186-... | `unit_returns`: `unit_statement_call_is_admitted` :6193, `unit_return_call_is_admitted` :6230, `statement_occurrence` :6247, `call_completes_unit_state` :6276, `completion_calls_all` :6290, `declared_call_result` :6358, `call_returns_unit` :6394 | Whether a call whose result is Unit may stand in statement/terminal position. |

## How it decides (calls:1-6400)
- Mostly keyed on symbol/handle identity (`call.target_symbol`, `receiver_symbol`, `machine_state_by_symbol`), with these name-keyed exceptions:
  - `self_entry_call_is_banned` :932-958 compares the last `::` segment of `current_machine.name` to `call.target.as_str()` (string compare) and fires only for receiver `self`.
  - `validate_asm_statement_call` :874-905 matches `call.target.as_str()` against a fixed list of `"asm#..."` strings (`"asm#hlt"`, `"asm#port_out"`, ... :877-898) plus `AsmControlRegister`/`AsmSystemRegister` write-intrinsic names. Anything else: "asm intrinsic `{other}` is not a statement form" :900.
  - `report_unresolved_value_call` :1962-2087: `matches!(target, "encode" | "decode")` with a data definition whose `name.as_str() == receiver` returns without any diagnostic :2062-2069. Resolution of `receiver.target` is by type NAME: `type_name_resolves_value_call` :1939 looks up `symbols.trait_definition(type_name)` / `symbols.machine(type_name)` and compares `signature.name.as_str() == target` / `state.name.as_str() == target`.
  - `declared_receiver_type_reference` :1882-1917 finds the receiver by `parameter.name.as_str() == receiver`, then local name, then owned-data name, then attached-data field name (first hit wins, in that order).
  - `validate_expression_call_bounds` :3500-3507 matches an accessor by `target.operation == call.target.as_str()`; `report_accessor_scalar_write_predicate` :564 by `target.operation == "write"`.
  - `validate_result_use` :6057-6069 exempts discarded results when ANY machine with no attached data whose `name.as_str() == call.target.as_str()` is a proof machine ("is_citation"), looked up by `.find` (first match by name, not by symbol). Keyed on source spelling.
  - `slices::guarded_slice_tail_call` :5310-5315 accepts `witness.view_path` equal to `""` or `"Slice::Length"` (string compare).
  - `ordinary_structural_initializer` etc. use identity.
- Proof self-call descent (`validate_proof_machine_recursion` :5495-5673): a self call is accepted if ANY of five judgments holds on the measure argument (`calls:5597-5638`): `strict_subterm_of_measure` (member path off the measure parameter, following immutable aliases), `substate_parameter_descends` (fixpoint over sub-state parameters), `proof_edge_strict_decrease_judged`, `guarded_integer_predecessor_call`, `slices::guarded_slice_tail_call`.
- `validate_expression_call_bounds` order of early returns (:3028-3559): erased mathematical application (:3048-3056) -> `call.quotient_operation.is_some()` (:3057) -> named conformance requirement (:3083) -> dynamic selector (:3122) -> unit result misuse (:3162) -> admitted build declaration call (:3171) -> exact embed call (:3174) -> asm builtins by `BuiltinFunction` (:3180-3223) -> named operator (:3225) -> conformance-symbol receiver (:3358) -> boundary trait signature (:3384) -> self call / machine parameter / attached / free machine (:3423-3481) -> receiver calls (:3483-3558).

## Cases by shape (calls:1-6400)
- `boundary_return::is_admitted` :1338-1383 accepts only a single-state machine whose body is exactly `[LocalData, Expression(name of that local)]` (:1361-1366), immutable local, attached data present, no const/mutable scalar parameters.
- `unit_result_initializer_call_is_admitted` :1432-1515: a long disjunction of shapes (closed primitive array return; "ordinary structural" initializer; boundary scalar initializer; `statement_index != 0` rule for non-primitive locals :1499-1504).
- `nested_boundary_applications_compose` :1720-1801: only scalar boundary applications; helper `composes` has a wildcard `_ => true` (:1795) for expression kinds not enumerated.
- `guarded_integer_predecessor_call` :5382-5462 recognises exactly `measure - 1` under guard `measure > 0` (optionally wrapped as `(measure > 0) == true`), literals compared via `value_i64() == Some(1)` / `Some(0)` (:5416, :5461).
- `validate_proof_machine_recursion` :5536-5589: requires exactly one witness subject, a bare name, naming an entry parameter. Error text :5560-5562: "the structural measure must be a bare parameter name (rung 1); compound measures over proof data are not proven yet" (stated limit).
- `completion_calls_all` :6290-6337: a Unit completion is any tree of `Match` arms whose leaves are all `Call`; anything else is `false` (`_ => return false` :6333).
- `validate_asm_statement_call`: fixed mnemonic list (see above).
- `report_bodyless_boundary_symbol_call` :3743-3757 lists nine "compiler" exemptions (accessor, access factory, access execution, empty-run release, descriptor realization, static interrupt realization, activation realization, layout snapshot, directly-callable top-level requirement).

## Same job elsewhere
- "Is this call the enclosing machine's own entry": `is_current_entry_call` `calls:5980-5991`, the inline filter in `validate_proof_machine_recursion` `calls:5507-5530`, `recursive_entry_path` `calls:5945-5978`, `self_entry_call_is_banned` `calls:932-960`, and in call_cycles.rs `internal_state_index` `cc:3276` / `target_machine` `cc:3296` / `extend_runtime_adjacency` `cc:2494-2500`. Each re-derives `target == machine.symbol || target == entry.symbol`. (read)
- Three walks of the same call-site shapes: `validate_value_position_calls` `calls:2219`, `receiver_calls`/`receiverless_calls` `calls:4804-5270` for statements, `validate_expression_call_bounds` `calls:3028` for expressions; each repeats "named conformance -> dynamic selector -> machine parameter -> machine state -> boundary trait signature" resolution in a different order (compare `calls:665-734`, `calls:3083-3161`, `calls:5113-5136`). (read)
- Duplicated diagnostic text "value-qualified machine calls are retired; use a declaration namespace and explicit arguments" at `calls:3233`, `:3520`, `:4892`, `:4979`. (read)
- Same unknown-field-read diagnostics built twice: `unknown_field_read_reports` `calls:2327-2364` and inline in `scan_at` `calls:2446-2484`; `scan_target` `calls:2412-2424` filters the first against the second by message equality. (read)
- Cyclic-component "len>1 or self edge" predicate repeated in call_cycles.rs (see section 1).

## Stated limits (calls:1-6400)
- "a value-call argument cannot itself be a machine call yet" `calls:1708-1709`.
- "tail recursion spelled as a call, which Omega does not support (machine call cycles are banned; stack size must be predictable)" `calls:953`.
- "storage address_of for write-only references is not implemented" `calls:2013`.
- "value-qualified machine calls are retired" (migration debt: old spelling still diagnosed) `calls:3233`.
- "compound measures over proof data are not proven yet" `calls:5561-5562`.
- "Runtime recursion is TAIL-ONLY (measured recursion amendment)" `calls:6017-6018`.
- "`zero_value<T>()` is a proof-only representation observation and may appear only in contracts" `calls:2847-2849`.
- "does not return a value but is used in a VALUE position -- it would silently bind 0 (ZII)" `calls:1856-1858`.

## Unused / one-caller (calls:1-6400 so far; greps in the counts block at the end)
(see "Counts" under calls.rs continued)

## Fail-open risks (calls:1-6400)
- `report_unresolved_value_call` `calls:2062-2069`: call `X.encode(..)` / `X.decode(..)` where `X` is spelled like any data definition returns with no diagnostic, regardless of whether the method exists. Keyed on spelling. Label: open (no comment).
- `validate_expression_call_bounds` `calls:3057-3059`: `call.quotient_operation.is_some()` returns before any argument validation. Label: open (no comment at the site; the check may live elsewhere, not traced).
- `calls:3048-3056`: erased mathematical application returns early; comment `calls:3044-3047` says "Its complete dependent application was checked by the mathematical producer; relevance still visits every operand". Label: stated.
- `calls:3171-3176`: `admitted_build_declaration_call` and `is_exact_embed_call` return with no argument validation. Label: open.
- `validate_type_parameter_instantiation_bounds` `calls:3809-3823`: parameter not a type parameter, no bounds, or `declared_place_type` returns `None` for the argument -> `continue` (bound unchecked). A call whose argument's type cannot be resolved by `declared_place_type` skips the bound check. Label: open.
- `validate_named_conformance_arguments` `calls:4152-4155`: an argument with unknown declared type is skipped in the second match pass. Label: open (the earlier `validate_call_arguments` pass still runs).
- `scoped_callable_type_arguments` `calls:4709-4713`: a const selection with no binding is skipped. Label: open.
- `validate_asm_value_destination` `calls:4374-4390`: returns silently unless the assigned value is a builtin asm read; fine by construction (only asm reads are subject), label traced by reading.
- `validate_proof_machine_recursion` `calls:5503-5505` and `:5532-5534`: no entry state or no self calls -> returns `Vec::new()` with no diagnostic. Label: traced (no self call means nothing to validate).
- `validate_result_use` `calls:6059-6069`: name-keyed "citation" exemption, see above. A discarded non-unit result of a machine call without receiver is accepted whenever ANY machine (first by name) with no attached data and the same `name` is proof-only. What gets through: a runtime machine call whose spelling equals a proof machine's name in another scope. Label: open.
- `scan_at` `calls:2701-2702`: `Binary` with `static_type_predicate_operands` is skipped without scanning operands; `calls:2523` `SavedSubject` skipped; `calls:2716-2724` case-membership binary skips scanning the right operand. Label: open (no comments).
- `nested_boundary_applications_compose` `calls:1795` wildcard `true`. Label: open.
- `call_argument_destinations` `calls:6113-6119`: target absent from the memo rows falls back to computing directly (not a fail-open).

---

## calls.rs continued: lines 6400-14739 (read in full)

### Section map
| Lines | Module / item | What it does |
|---|---|---|
| calls:6411-6548 | end of `unit_returns`: `admitted_build_declaration_call` :6411, `machine_has_unit_result` :6477, `has_inferred_value_return` :6486, `unit_type` :6537 | Build-declaration intrinsic recognition and Unit-result queries. |
| calls:6550-9410 | `write_frames` begins; `alias_bindings` :6551, `alias_origins` :6814, `assignment_addresses` :7382, `assignment_targets` :7542, `boundary_calls` :7566 | "Write frame" analysis: which caller places a call may write, expressed as `String` place paths plus a `FrameSourcePlace` (root symbol + segments). `boundary_calls` resolves boundary-trait / requirement signatures and computes `written` paths from argument borrows. |
| calls:9411-9600 | `call_targets`: `call_argument_types` :9420, `free_machine_entry_state` :9528, `machine_state_by_symbol` :9547, `checked_body_alternatives` :9562 | Target lookup helpers. |
| calls:9601-9998 | `call_trees`: `complete_expression_tree` :9757 (worklist), `ExpressionAdmission` :9619, `PendingNode` :9625 | Decides whether an expression tree's every call has a known write set. |
| calls:9999-11348 | `caller_aliases`: `CallerWriteSite` :10088, `LocalWriteSource` :10183, `assignment_storage_origins` :10383, `CallOriginContext` :10696, `caller_statement_owner` :11275, memoised prefix-site lookups :11053-11139 | Per-site "what was aliased before this statement" evidence. |
| calls:11349-11938 | `cycle_frames`: `summarize_state_written_paths_for_cycles` :11396, `build_cycle_frame_equation` :11524, `summarize_transition_target_written_paths` :11842 | Fixed-point write summary for states in a transition cycle. |
| calls:11939-12185 | `cycle_substitutions`: `Substitutions` :11975, `substitutions_are_finite` :12051, inline `#[cfg(test)] mod tests` :12150-12184 | Checks that parameter-substitution growth around a cycle is bounded. |
| calls:12186-14001 | `demand`: `read_backing` :12187 (CallFrameResolver read-backing queries), `call_origins` :12536 (`invocation_write_origins` :12686, `capture_operand_origins` :12895, `saved_reference_operand_origins` :12971, `statement_receiver_origins` :13145), `normal_subjects` :13236 (`InvocationEntrySubject` :13247, `entry_reference_subjects` :13481, `prepare_entry_reference_subject` :13588, `declaration_preserving_views` :13702), `result_transport` :13753, `transfer_operands` :13954 | `impl CallFrameResolver` methods: origins of call operands, reference subjects at call entry, result transport. |
| calls:14003-14739 | `CallFrameResolver` :14060, `CallFrameTables` :14066 (build :14075), `PrivateScalarLocals` :14098, `CallFrameCaches` :14195 (about 22 `Mutex<HashMap<..>>` caches), `operand_call_plans` :14267, stored-reference effect wrappers :14301-14731 | The resolver object and its caches; thin pub wrappers over `stored_calls::*`. |

### Inputs and outputs
Reads `&dyn CheckingInput` plus `TopLevelSymbols` / `MachineSymbols` name tables (built by `CallFrameTables::build` :14075, which returns `None` if building either produced any diagnostic :14078). Produces `Option<Vec<String>>` write-path lists (None = "opaque / unknown"), `Option<Vec<LocalWriteSource>>` origin lists, `NormalizedWriteFrame` (via `facts`), and `Result<_, &'static str>` refusals. Caches live in `CallFrameCaches` and `symbols.caller_sites.*` memo tables (`memoized` calls :11087, :11119, :11262).

### How it decides (calls:6400-14739)
- Core representation: a place is a `String` path such as `"self.a.b"` or `"x[..]"` (`FramePlaceOrigin.path`, `calls:10189`, `:7370` `format!("{}.{}", receiver.path, member...)`, `:8559` `receiver.join(".")`) PLUS a `FrameSourcePlace` with root symbol and `facts::PlaceSegment`s (`LocalWriteSource` has both, `calls:10183-10193`). Many decisions compare the string path: `split_place_root(&origin.path)` and name equality (`calls:9057-9065` `caller_canonical_result_origin`, `:10162` `program.symbols.name(row.local_symbol) == name`, `:10662-10676`, `:12125` `parameter.name.as_str() == name`, `:12128` `locals.iter().any(|local| local == name)`), `place_suffix` / `frame_paths_overlap` on path strings (`calls:11012-11025`, `:13433`).
- Synthetic identity encoded as text: `invocation_result_origin` builds a path `"invocation result @{arena_index}:{generation}"` from an expression handle (`calls:8895-8899`).
- Pointer identity: `caller_site_key` uses `std::ptr::from_ref(call).addr()` for `Call` and `Statement` sites (`calls:11065`, `:11067`) and arena indices for `Expression`/`Transition`; `caller_statement_owner` matches with `std::ptr::eq` (`calls:11286-11288`) and returns `None` if more than one statement matches (`:11301-11303`).
- Receiver-by-name: `requirement_signature_for_receiver_path` walks `self.field.field` by `field.name.as_str() == member.as_str()` starting from `data_definition_by_name_str(attached.as_str())` (`calls:8259-8289`); `receiver_requires_boundary_frame` compares `parameter.name`, `field.name` and `symbols.trait_definition(name)` by spelling (`calls:7611-7642`); `free_machine_entry_state` picks the state whose `name == target`, else the state named `"entry"`, else the first (`calls:9539-9544`).
- Type-flow questions (`owned_storage_may_hold` family :9091-9388) return `Option<bool>` where `None` means opaque; conservative `Some(true)` for duplicated/missing data definitions, dynamic traits, const expressions, recursion (`calls:9169`, `:9172`, `:9220`, `:9343`, `:9360`, `:9381-9383`).
- `boundary_trait_signature_and_receiver_inner` (`calls:7821-8016`) returns `None` on any mismatch among receiver trait, declaring scope, requirement application, unique signature by `signature.name.as_str() == target` (`:7931`), binder admission by `SignatureBinders` (`:7983-7995`), and name equality `program.symbols.name(declaration.symbol) == signature.name.as_str()` (`:8007`).
- `summarize_state_written_paths_for_cycles` runs only when the named-transition subgraph from the entry has a cycle (`calls:11403` returns `None` when acyclic), builds one equation per reachable state, then iterates a delta fixed point (`:11462-11512`); each substitution edge is marked `grows` when the suffix is nonempty, there are recasts, or either side is collection-coarse (`:12137-12140`), and the substitution graph must not have a growing edge inside one SCC (`:12044-12047`).

### Cases by shape (calls:6400-14739)
- `admitted_build_declaration_call` :6419-6457 is a closed table of seven `BuildOperation` kinds with (static-argument count, value-argument count) ranges, e.g. `WireCompatibilityRequest` takes `5..=usize::MAX` static args and exactly one value arg; every other call target returns `false` (`_ => return false` :6456).
- `private_scalar_local_symbol` :14158-14173 admits only twelve builtin atoms: Bool, Address, I8-I64, U8-U64, F32, F64.
- `boundary_written_argument_paths` handles three parameter shapes: shared-only borrowed leaves (skipped :8183-8192), non-reference parameters that may carry writes (aggregate path via `aggregate_arguments::written_paths` :8201), and `&mut` references (origins :8219-8241).
- `array_shape_is_closed` :9227-9274 accepts only literal-length fixed arrays and non-parameter named types (`_ => return false` :9270).
- `prepare_entry_reference_subject` :13588-13699 has separate branches for scalar referent (:13645), logical-view (representation) source (:13663) and plain declaration path (:13670).
- `saved_reference_operand_origins` :12981-13087 handles only `Borrow`, `Member`, `SavedSubject`; "No computed/indexed/cast source is reconstructed from its spelling." `calls:13085`; depth cap `active_captures.len() >= 64` :13036.
- `call_trees::complete_expression_tree` :9757-9997: byte-predicate calls special-cased (:9834-9845); receiver forms allowed are Borrow/Member/Indexed/Match/Call (`_ => return false` :9820).
- `has_transition_cycle`-gated cycle summary only (`calls:11403`).

### Same job elsewhere
- Two copies of the "may a state/local declare write origins" predicate: closure `may_declare_origins` in `machine_has_tracked_origins` `calls:11091-11096` and in `caller_prefix_entry` `calls:11147-11152`, and the incoming-carrier test `calls:11098-11101` vs `:11153-11158`. (read)
- Three near-identical `*_result_origins` functions (`boundary_result_origins` :8654, `requirement_result_origins` :8963, `bodyless_callee_result_origins` :8910) all end in the same `match exclusive_boundary_result_origins(..) { Named / Unrelated / Opaque }` block. (read)
- `owned_storage_may_hold_inner` :9108 and `storage_member_may_hold` :9304 both walk data members with the same visiting/bindings protocol, differing in which case returns what. (read)
- Statement/expression/transition call-site triple repeated as the four-variant `CallerWriteSite` enum with `match site` in `call_site_symbols` :8072, `parameter_boundary_receiver_type` :8018, `call_has_authority_receiver` :7750, `invocation_write_origins` :12694-12733, `entry_reference_subjects` :13488-13536. (read)
- Suspected (grepped names only, not read): `unique_state_by_symbol` defined `calls:6158` and with the same name in `reference_result_custody.rs` (3 hits), `contract_entailment.rs:82892,82904`, `checks_multiplicity.rs:7801`, `checked-trees/src/flow.rs:1348-1427`; I did not read whether these are calls to this function or separate definitions. Also `checked_body_alternatives` has hits at `calls:25097` and `:33412` (outside my range).

### Unused / one-caller code (grep: Grep tool, content mode, `-o`, over `omega-rust`; counts exclude the definition and re-export lines in lib.rs and calls.rs:39737-39748)
- `closed_requirement_dispatch_is_valid` (`calls:31`, `pub`): 4 external call sites (`machine_calls_core.rs:5315`, `contract_entailment.rs:17559`, `:17564`, `execution_scalar_computations.rs:10159`).
- `nested_boundary_applications_compose` (`calls:1720`, `pub`): 1 external call (`execution_scalar_computations.rs:10810`).
- `result_initializer_call_is_admitted` (`calls:1386`, `pub`): 1 internal (`calls:1419`) + 1 external (`execution_scalar_computations.rs:10806`).
- `unit_result_initializer_call_is_admitted` (`calls:1432`, `pub`): 1 internal call (`calls:1407`); 0 external call sites found (only re-exports `lib.rs:1505`, `calls.rs:2864`, `calls.rs:39738`).
- `unit_return_call_is_admitted` (`calls:6230`): 1 call (`calls:2259`); plus re-export `lib.rs:1505`.
- `machine_has_unit_result` (`calls:6477`): 1 internal (`calls:1844`) + 2 external (`float_and_mathematical_contracts.rs:2398`, `proof/src/mathematical_declarations.rs:4968`).
- `named_conformance_target_requirement` (`calls:4025`): about 14 external call sites in 8 files (`machine_parameters_and_bindings.rs` x2, `effect_inference.rs`, `const_generic_expressions.rs`, `checks_borrows.rs` x3, `borrow.rs`, `published.rs`, `contextual_and_requirement_calls.rs`).
- `validate_proof_machine_recursion` (`calls:5495`): 2 call sites (`program_validation.rs:4417`, `contract_entailment.rs:90447`); `validate_self_recursive_call_positions` (`calls:5879`): 1 call site (`program_validation.rs:4426`); `proof_call_has_structural_descent` (`calls:5475`): 1 call site (`contract_entailment.rs:9183`).
- `call_argument_destinations` (`calls:6101`): 1 call site (`literals_and_struct_literals.rs:2488`).
- `declaration_call_type_arguments` (`calls:4554`): 1 call site (`machine_calls_core.rs:7478`).
- `checked_body_alternatives` (`calls:9562`): 0 call sites found within or outside my range apart from other `calls.rs` hits at `:25097`, `:33412` (not read). Label: suspected one-caller/unused in my range; I did not read the later hits.
- `operand_call_plans` (`calls:14267`, `pub`): about 20 call sites (`contract_entailment.rs` x10, `checks_contracts*.rs` x2, `arithmetic_domains.rs:21331`, calls.rs later lines).
- `statement_returns_reference_without_effects` (`calls:6739`): 1 use in my range (`calls:6589`) and one later in calls.rs (`:26965`).
- `unique_state_by_symbol`: see "suspected" above.
- Documented-as-shared claims: none found in comments of this range that call a type or function THE single/shared/canonical way.

### Stated limits (calls:6400-14739)
- "No computed/indexed/cast source is reconstructed from its spelling." `calls:13085`.
- "Replacing the descriptor cell is not writing its captured referent. The existing cell-address route owns that case." `calls:12989-12990` (returns None).
- "None keeps duplicate names and non-private declarations ineligible." `calls:14070` (doc on `private_scalar_locals`).
- "this proves intermediate retention ... it is not a return summary." `calls:14299-14300`.
- Refusal strings that name missing capabilities: "stored call has no complete statement-successor effects: ..." `calls:14048`, `:14051`, `:14054`.
- "Only writable carried borrows can reach caller storage from an owned value." `calls:8180`.

### Fail-open risks (calls:6400-14739)
- `with_caller_origin_context` `calls:10863-10868`: when `collected_prefix_at` returns `None` (no prefix evidence), it still calls `resolve(&mut FrameInference::default(), None)`, i.e. the origin question is answered with no alias/stored-origin context. What gets through: a write-origin query on a site whose prefix could not be collected is answered as if there were no aliases. Label: open (no comment); I did not trace how callers treat the `None` context. Callers inside my range: `reference_operand_write_sources_with_captures` :12573 (treats `prefix.is_none_or(..)` as "may change origins" = conservative) and `invocation_write_origins` :12760 (same, :12805-12813 `prefix.is_none_or`). So for those two callers a missing prefix is treated as an effect: traced to fail-closed.
- `caller_aliases_at_prefix` `calls:11236-11242`: `CallerPrefixSite::Untracked` returns an EMPTY evidence set (no aliases) as `Some`. Untracked is chosen by `caller_prefix_entry` (:11141-11176) when `machine_has_tracked_origins` is false or the state has no incoming carrier and no preceding local that may declare origins (:11160-11171). Reasoning: with no carriers and no origin-declaring locals there is nothing to alias. Label: traced by reading only the predicate, not its soundness against all `type_may_carry_write` cases.
- `owned_storage_may_hold_inner` `calls:9134-9135`: `program.primitive_type_reference(container).is_some()` -> `Some(false)` ("a primitive cannot hold the referent"); `TypeReferenceNode::Unit` -> `Some(false)` (:9221). Used to conclude `ExclusiveResultRoute::Unrelated` (result does not alias argument) at :8883-8885, which then yields a fresh invocation-result origin (:8716-8719). Label: traced by reading; wrong-program risk would require a primitive container that can hold a reference (none by construction).
- `exclusive_boundary_result_origins` `calls:8844-8846`: non-exclusive reference parameters are skipped (`continue`) when deciding which arguments the result may alias. What gets through: a result that aliases a shared-borrow argument is not recorded as aliasing. Label: open (the shared borrow cannot yield an exclusive result in safe code; I did not trace this).
- `boundary_written_argument_paths` `calls:8183-8192`: parameter whose carried-lifetime leaves are all non-exclusive is skipped (`continue`) i.e. assumed not to write caller storage. Comment :8180-8182 gives the reason. Label: stated.
- `substitutions_are_finite` `calls:12128-12129`: a source rooted at a local name is skipped with `continue` (no dependency edge). Label: open (reasoning probably that locals are not caller-visible; no comment).
- `cycle_frames::summarize_transition_target_written_paths` `calls:11870`: a `Value` transition target contributes an empty write set (`Some(Vec::new())`) and `Terminal` likewise (:11869). Label: open for `Value` (a value target is an expression whose calls are summarized elsewhere via `statement_value_expression_roots` at :11582-11621; I saw that path in `build_cycle_frame_equation` but not for this function's other callers).
- `summarize_transition_target_written_paths` `calls:11875-11886`: when the target state is already active and `require_complete` is false it returns an EMPTY write list if `named_transition_preserves_state_namespace` holds. Label: stated by the function name/flag, no comment.
- `private_scalar_local` `calls:14283-14296`: returns `None` unless exactly one local with that name in the exact state is a builtin scalar. Fail-closed.
- `unit_type` `calls:6537-6548` loops on `Constrained` base types; any other node is false.
- `completion_calls_all`/`has_inferred_value_return` fail-closed on unknown shapes (`_ => return false`).

---

## Counts and greps used (all `Grep` tool over `<checkout>/omega-rust`)
- Pattern `structural_return_descends|activation_target_state_index|requirement_backing_machines|infer_checked_proof_recursive_machines|machine_call_dependency_symbols|declaration_call_dependencies|ranged_member_site_invariant`, content mode: results quoted in section 1.
- Pattern `\b(closed_requirement_dispatch_is_valid|nested_boundary_applications_compose|named_conformance_target_requirement|operand_call_plans|machine_has_unit_result|unit_result_initializer_call_is_admitted|result_initializer_call_is_admitted|resolve_requirement_contract|requirement_contract_signature)\b` with `-o -n`, uncapped (`head_limit 0`): results above.
- Pattern `\b(validate_proof_machine_recursion|proof_call_has_structural_descent|validate_self_recursive_call_positions|call_argument_destinations|unique_state_by_symbol|checked_body_alternatives|statement_returns_reference_without_effects|assignment_replaces_untracked_reference|declaration_call_type_arguments|unit_return_call_is_admitted)\b` with `-o -n`, uncapped: results above.
- `wc -l` on both files (Bash, read-only): `calls.rs` 44215, `call_cycles.rs` 4256.

## Coverage
- `omega-rust/psi/compilation/validation/src/call_cycles.rs` lines 1-4256 (4256 lines): read in full.
- `omega-rust/psi/compilation/validation/src/calls.rs` lines 1-14739 (14739 lines): read in full (I also read through line 14745). Lines 14740-44215 of this file are outside the unit and not read.
- Skimmed: none. Not read (within the unit): none. Total read 18995 of 18995.
- Names in the grep results that point outside my range (e.g. `calls.rs:25097`) were seen only as grep hits.

## Five things a maintainer most needs to know about this unit
1. `call_cycles.rs` holds three different cycle regimes in one file (proof-only structural/interval ranking `cc:15-392`/`:393-567`, runtime tail-call plus natural-number ranking `cc:1190-3369`, selector self-recursion diagnostic `cc:3783-3796`), and they are chosen by `ProofOnlyClassification::is_proof_machine` per member; a component mixing proof and runtime machines is rejected with an error string (`cc:593`, `cc:667`).
2. The runtime ranking judge `project_component` (`cc:2582-3009`) accepts a cycle only after one of several evidence routes proves each call edge (captured-component proof `cc:3057`, range proof `cc:3126`, or a witnessed arrival edge `cc:3101-3115`) and the non-strict edges are acyclic (`cc:3181`). A member with no source-body witness is skipped, not rejected (`cc:2624-2626`, `:2719-2721`), so the component result becomes `Ok(None)`.
3. Proof self-recursion is accepted by an OR of five independent judgments in `calls:5597-5638`; the two string/shape-specific ones are `guarded_integer_predecessor_call` (`measure - 1` under `measure > 0`, `calls:5382`) and `guarded_slice_tail_call` (view path `""`/`"Slice::Length"`, `calls:5310`).
4. The write-frame machinery (`calls:6550-14739`) represents places as strings plus symbol/segment pairs and keeps name tables; many decisions are made on string paths and spelled names (`calls:9057-9065`, `:10162`, `:12125-12128`, `:8895`), and caches are `Mutex<HashMap>` keyed by symbols/handles (`calls:14195-14233`) even though `AGENTS.md` asks for dense-id tables; `use symbols::SymbolKeyMap as HashMap` at `calls:14031` is the alias in use, and `cycle_frames` uses `std::collections::HashMap` directly at `calls:11457` (read).
5. Almost every validator in `calls.rs` reports by pushing a `Diagnostic` and returning `()`, so "accepts" means "pushed nothing". Early `return`s that skip all argument validation exist for quotient operations (`calls:3057`), erased mathematical applications (`calls:3048`, commented), build-declaration calls (`calls:3171`) and exact embed calls (`calls:3174`).

## What I am unsure about
- Whether `StatementNode::Assembly`/`AssemblyFact` can write rank coordinates; `project_component` skips them (`cc:2765`) and `internal_*` walks skip `AssemblyFact` (`cc:2435`, `cc:2467`).
- Who supplies `RuntimeRankBodyWitness` slices and what guarantees a witness exists for each member (`cc:2574`, `cc:2600-2614`); I did not read the 05 crate callers.
- Whether `calls.rs` lines beyond 14739 duplicate or extend the modules I read (grep hits at `calls:25097`, `:33412`, `:38451`, `:39055`, `:39737-39748` show repeated names and what look like re-export lists); I did not read them.
- The soundness of the `Untracked` prefix shortcut (`calls:11160-11171`) and of the `Unrelated` result-origin route (`calls:8716-8719`); I read the predicates, not their proofs.
- Whether `unit_result_initializer_call_is_admitted` has any caller outside re-exports; the grep found none, but I did not search for qualified paths other than by bare name.


