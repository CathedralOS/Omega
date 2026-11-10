# Unit 56 notes

Files covered by this unit:
- omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_scalar_computations.rs (1-11126, 11126 lines)
- omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit.rs (1-8373, 8373 lines)

STATUS: both files read in full (all 19,499 lines). Sections 1 and 2 below; Section 2 is split into "ETU 1-4600" and "ETU 4601-8373"; the Coverage, Five things and Unsure blocks are at the end.

(Short paths below: `ESC` = execution_scalar_computations.rs; `ETU` = execution_terminal_unit.rs.)

---------------------------------------------------------------------
## 1. ESC: execution_scalar_computations.rs (1-11126), read in full

### Purpose
Builds the "checked scalar computation plans" (`CheckedScalarComputationPlans`), the "structural value plans" (`CheckedStructuralValuePlans`) and "completion plans" (`CheckedCompletionPlans`) for every statement of every state of every machine. These are the evaluation graphs (calls, short-circuit selection, dispatch/match, casts, integer ops, field/tag observations, structural constructions, views, rank observations) that later stages (Psi 06+) consume. The header (ESC:1-11) says one builder owns both plan arenas. The file is a sequence of inline `mod` blocks that each add `impl Builder` methods, plus two `pub(crate)` entry points at the bottom.

### Shape
- Entry points (only two outside the file):
  - `build_checked_value_computation_plans` ESC:10508-11100. Callers (grepped `build_checked_value_computation_plans|record_rank_observations` over omega-rust): 1 call site, published_facts.rs:11186. Loops machines -> states -> statements, makes a fresh `Builder` per statement (ESC:10553-10583), then runs a long fixed sequence of "record" steps per statement kind (local data, assignment, call, expression, transition).
  - `record_rank_observations` ESC:11107-11126. 1 call site, execution.rs:275. Runs after the main build so rank roots are appended after completed effects (comment ESC:11102-11106).
- `Builder` struct ESC:9477-9529 (mod `builder`, 9406-10506). Fields that matter: `program: &dyn CheckingInput`, `shapes: ShapeCollector`, `carriers: CarrierIndex`, `call_result_types: CallResultTypes`, `operators`, `semantic`, `flow`, `borrow`, `proof`, `pure` (the earlier pure scalar plans), `machine`, `state`, `statement_index`, `parameters` (scalar-position params), `authored_parameters`, `parameter_types`, `locals`, `plans`, `structural_values`, `declaration_scope/declaration_values/declaration_bindings/declaration_parameters` (rank/declaration application context), `dispatch_receivers: BTreeMap<(u32,u32), DispatchReceiver>` (ESC:9510), `scalar_occurrences: Vec<(ExpressionHandle, PrimitiveType, handle)>` and `integer_occurrences` (linear-scan memo, ESC:9518, 9524), `root_scope: RootScope::{Statement, RankObservation}` (ESC:9463).
- Core dispatcher: `Builder::expression` ESC:9896 (memo by (expression, carrier) via linear scan, 9901-9907) -> `build_expression` ESC:9932-10390, an ordered ladder of `if let ... return` attempts (declaration input, retained const, bool->int, captured dispatch value, match, primitive reference observation, atomic load, structural field, same-carrier cast, semantic-domain casts, selected comparison, byte literal comparison, storage address, byte predicate, pure `lower_return_expression`, indexed read, enum equality, case membership, integer operand, then Call / And-Or / Not / comparison match at 10145-10389). First success wins; the order is the policy.
- `Builder::structural_value` ESC:8945-9381: one 400-line `if / else if` ladder choosing a `ValueKind` (NativeCodeLayout, captured copy, scalar, anonymous numeric match, reborrow, same-carrier cast, quotient cast, string/byte literal, reference capture, copied place, subslice/slice view, call, owned place, projection, case construction, struct literal case/record, scalar array, array literal, scalar case source, final `Match` -> `Dispatch`).
- Inline mods and line ranges: atomic_loads 73-180; boundary_calls 181-525; byte_comparisons 526-689; call_arguments 690-1503 (+ `declarations` 692-869); call_result_types 1504-1610; cases 1611-1867; completions 1868-2001; coordinate_reads 2002-2166; declaration_inputs 2167-2403; dispatch 2404-3173; dispatch_copies 3174-3608; dynamic_calls 3609-3712; enum_equality 3713-4044; indexed_reads 4045-4150; integers 4151-4793; normal_return 4794-4898; quotient_calls 4899-5063; rank_observations 5064-5787 (+ `operands` 5073-5315); reference_captures 5788-6008; slice_view 6009-6475 (+ `formation` 6055-6118); structural_fields 6476-7132; structural_values 7133-9403 (+ family_values 7175-7555, place_types 7556-7761, place_values 7762-8278, value_predicates 8279-8438); builder 9406-10506.

### Inputs and outputs
Reads: the typed program through `&dyn CheckingInput` (expression table, type references, statements, machines), operator facts, semantic FactPlan, `FlowFacts` (calls, operands, copied payloads, value observations), `BorrowFacts` (loans, call accesses, returned loans, call loan resources, ESC:5810-5949), `ProofFacts` (contract calls/evidence terms, ESC:10404-10445), the earlier pure scalar plans `pure`. Produces: appends to `plans.nodes`, `plans.operands`, `plans.roots` (roots keyed by machine/state/statement ordinal/role), `structural_values.nodes/roots/record_fields/dispatch_arms`, `completions`. Mutates `pure` once: `withdraw_captured_pure_roots` ESC:3564-3607 deletes pure roots whose expression observes a captured dispatch value (retain + rebuild `source_bindings`, reset `state_index`, `seal_expressions`).

### How it decides (judgment and key)
- Nearly all functions return `Option`; `None` means "not planned here" and callers either try the next rung or abandon the root. The statement then simply has no root (`record_root` ESC:9561-9598 silently skips when `expression()` returns None). Keys are mostly handles (expression handle identity; `(arena_index, generation)` pairs, e.g. ESC:1307, 2768, 3935 area, 9151), symbols, and type identities (`normalized_type_identity`, `stored_structural_type_identity`).
- Identity-keyed examples: `call_ordinal` ESC:10484-10504 matches a flow call by `(statement_index, target_symbol, authored_expression)` and requires exactly one match (10498-10500). `call_origin` ESC:10448-10482 same for declaration-scoped calls. `comparison_use` ESC:2445-2458 requires exactly one operator-use match. `entry_index_selector` ESC:2102-2164 requires exactly one bound matching 8 fields.
- Source-spelling / name-string comparisons (read): `structural_carriers_match` ESC:8297-8353 accepts two Generic types when `base_symbol`s are equal OR when `base_name.as_str()` strings are equal and non-empty (8329-8330); the Generic-vs-Named arm likewise accepts `base_name.as_str() == name.as_str()` (8348-8349). `is_sealed_accessor_call` ESC:494-524 compares `target.operation == call.target.as_str()` (519-521) next to a symbol compare. `Builder::boundary_call` uses `target.state_symbol == authored_target`. No `format!`/debug-string keys seen.
- Judgments that refuse (read): boundary call result carrier must equal expected and operand count equal parameter count (ESC:324-328); descriptor-table operation shape (ESC:387-404); atomic load place must be atomic storage with builtin coordinates (97-109); dispatch coverage must be complete or the dispatch is not planned (ESC:2582-2584, 3167-3169); `dynamic_call` refuses evidence arguments or non-nominal route (3641-3643); `retain_structural_value_root` refuses an occupied/ambiguous root coordinate (8721-8731); `insert` dedups `Call` nodes within the current statement by (source_call, called_signature, primitive_type) (9729-9758).
- Short-circuit And/Or (ESC:10300-10338): left constant-folds; skips right if `normal_return::boolean_result` says the left decides, or if `right_operand_never_evaluated` (flow roster says only the left was captured; 9542-9559); otherwise emits `Select` with a constant `skipped` arm.

### Cases by shape
- Descriptor-table calls: `ActivateTask` => U8 result, 1 scalar arg, 3 structural args; `ReloadSegments` => U8, 1, 2; `ReadTask`/`ReadSegment` => U16, 0, 0 (ESC:387-404).
- `Builder::dispatch` scalar subject types only Bool, fixed integers, F32/F64 (ESC:2824-2829).
- `zeroed_scalar_array_parts` ESC:8362-8379: "admitted only for a `[scalar; N]` leaf: a literal-length fixed array whose element resolves to one plain integer primitive" (8356-8360).
- `zeroed_integer_literal` ESC:8385-8411: a closed list I8..U64 only.
- `local_selector` ESC:1796-1833: runtime selector only for integer types U8..U64/I8..I64 local at a dense `Local` position.
- `case_construction` / `enum_equality`: only payload-free sums for the generated comparison (ESC:3770-3779, "Payload-free sums compare their complete case roster", 3717).
- `scalar_observation_type` ESC:6511-6557 and `readable_place` ESC:7644-7686: loops capped at 64 referent crossings (`for referent_crossings in 0..64`, ESC:7670); `descriptor_fields` caps at 8 shells (`if visited > 8`, ESC:7119-7123), then silently proceeds with whatever reference it reached.
- `canonical_place_is_borrowable` ESC:7578: named root plus Field/FixedIndex segments only; `canonical_place_is_copyable` ESC:7594: Field/Case/FixedIndex/Index.
- `entry_index_selector`/`scalar_coordinate`: storage steps limited to Referent/Field/Case/FixedIndex, others `return None` (ESC:2066-2072).
- `completion_node` ESC:1908-1999: only `Call` returning unit and `Match`; anything else is a refusal with a reason enum (`Refusal::ExpressionKind` 1977).
- Boolean coverage tracked in `[bool; 2]` arrays at ESC:2531, 3100, 4415.

### Same job elsewhere
- Two scalar-expression builders for integer expressions: `Builder::build_expression` integer tail (ESC:10133-10144, via `integer_operand`) and `integers::build_integer_operand` (ESC:4253-4604) repeat the same prefix ladder (declaration input, retained constant, storage address, captured dispatch, primitive ref observation, atomic load, structural field, same-carrier cast) at ESC:4258-4360 vs 9937-9981. Read, both in this file.
- Pure lowering (`lower_scalar_expression`, `lower_return_expression`, outside unit) does the same job for pure expressions; this file falls back to it (ESC:4385, 10104). Suspected overlap, not read.
- `observes_captured_dispatch_value` (ESC:3518) and `uses_captured_dispatch_value` (3506) both exist as free functions and Builder wrappers (3286-3310).
- The three-argument "match arms + Boolean coverage" logic is repeated in `dispatch` (2531-2585), `prepare_dispatch` (3099-3169) and `integer_operand` (4415-4446).
- `declaration_dispatch` (ESC:2602-2673) vs `structural_subject_dispatch` (2870-2953): two separate arm-pruning schemes for unreachable arms (read).
- `projection_selector_evaluations`, `retain_projection_selector_evaluations`, `retain_reference_selectors` (ESC:8445-8608) are the shared selector retention; `record_transition_selectors` (ESC:932-954) does a parallel selector pass.
- `exact` same-name judgement `declared_carriers_match` is in `validation` (used ~40 times here); `structural_carriers_match` (5 occurrences, all in ESC, grepped) is the local variant that adds the name compare.

### Unused / one-caller code
- `build_checked_value_computation_plans`: 1 call site (published_facts.rs:11186). `record_rank_observations`: 1 call site (execution.rs:275). Grepped.
- `structural_carriers_match`: grepped count 5 occurrences in total, all in ESC (definition + imports/uses within the structural_values mods).
- Comments that call something THE shared way: "Declaration inputs have one binding route in whole expressions and integer operands" (ESC:2169) - `declaration_input` has 2 callers in ESC (9937, 4258), read. "Call operands have one evaluator" (ESC:1068-1072) - `call_operands` callers in ESC: boundary_application (335), dynamic_call (3684), Body call (10268); read, 3 sites. "Applications share the ordinary computation operand evaluator" (198). I did not count `call_operands` with a command beyond reading.
- `#[allow(clippy::too_many_arguments)]` on `boundary_application` (312) and `rank_observations::record` (5328).

### Stated limits
- "One known boundary: Psi 06's `parameter_declaration` omits the `[erased]` predicate, so a state that mixes an erased structural binding with a later structural formal is the one shape where the two sides disagree." (ESC:8898-8901) - a stated mismatch with a later stage.
- "Match production must publish the exact value/place association before its patterns. Replaying its initializer cannot supply it." (ESC:1777-1779) and "An unproduced Match alias cannot recover a receiver" (6990).
- "A known payload-free constructor remains the existing single-membership optimization." (3729).
- Performance note: "about 45 µs, and the product program asks 466 times about 231 distinct types" (1509-1516).
- "Reinterpreting the entire buffer before slicing is not equivalent" (6023).
- The `index_selector` comment says only an anonymous integer uses the u64 index context (898-902).

### Fail-open risks
- `descriptor_fields` ESC:7119-7129: after 8 reference shells the loop `break`s and continues with the unpeeled reference rather than failing. Status: open. Wrong program: a type with 9+ nested reference shells would look up value-parameter descriptors on the wrong layer (returns whatever `data_application_value_fields` gives, likely empty -> `closed_descriptor_value` returns None -> falls to generic field read). Probably benign (fall-through to the ordinary path) but I did not trace.
- `readable_place` ESC:7670-7685: bounded to 64 crossings then returns None (fail closed).
- `structural_carriers_match` ESC:8329-8330, 8348-8349: accepts two distinct generic or named types whose base NAME strings are equal even when the symbols differ ("or" with the symbol test). Status: open (no comment says why a name compare is right). Wrong program: two different generic data types with the same spelled name (e.g. from different modules/packages) could be treated as matching carriers at a call result (ESC:9274), a case construction (9299) or a struct case (7514). I did not trace whether the type checker upstream makes this unreachable.
- `record_root` ESC:9572-9579: returns without building when a pure root already exists for the role and the call is not a boundary and does not observe a captured value. Status: stated by comment at 3557-3563/9461 (pure/computation split); I did not trace.
- `withdraw_captured_pure_roots` ESC:3564-3607 removes the pure root "even if construction of its replacement later fails: the old live read is never a fallback" (3560-3562): fail-closed by statement.
- `build_checked_value_computation_plans` ESC:10538-10543: `continue`s past a whole state if any scalar parameter lacks a primitive type; `let _ = builder.record_completion(...)` (10590, 10597) discards the Option. Status: open. Effect: a state is silently left with no computation plans (downstream consumers must treat absence as refusal; not traced).
- `retain_structural_root` ESC:9668-9674: returns silently if a root already exists for the expression (idempotent by design, comment 9654-9660).
- `observation_completion_actions` ESC:6697-6733: returns an empty cleanup span on every failed precondition (empty receiver, non-readable shell, `descriptor_retirement` None). Status: open: a missed cleanup action rather than a rejected program. Wrong program: a fresh reference receiver whose retirement lookup fails would skip normal_cleanup actions; I did not trace whether Terminal validates it.
- `dispatch_subject` / `structural_subject_dispatch` use `let _ =` style discards in the caller. None else seen: no `unwrap`/`expect`/`panic!`/`unreachable!` in ESC (Grep count of `unreachable!|panic!\(|\.unwrap\(\)|\.expect\(` = 0).

---------------------------------------------------------------------
## 2. ETU: execution_terminal_unit.rs (1-8373)

### ETU lines 1-4600 (read in full)

Purpose: "Executable Unit plans from checked ownership, control and call facts" (ETU:2-7). Root module of the Unit effect-plan builder: it collects boundary signatures and ordinary single-state candidates (through control.rs), composed (multi-state) candidates, reconciles calls, prunes candidates whose transitive callees are unavailable, and records one gap per dropped machine (ETU:9-59). The long header (1-101) is design commentary: single-state body has one producer `control/statement_sequence.rs`; composed through `composed_control/assembly.rs`; erased parameters are skipped on both sides and "the Terminal consumer rejoins each retained parameter" (72-100). The file is a mod hub: several children are separate files via `#[path = ...]`: `execution_terminal_unit_calls.rs` (ETU:2154-2155, `calls`), `execution_terminal_unit_cleanup.rs` (2689-2690, `cleanup`), `execution_terminal_unit_control.rs` (2747-2748, `control`), `execution_terminal_unit_state_graph.rs` (3944-3945, `state_graph`). Those files are outside this unit and not read by me. `build_checked_unit_effect_plans` is named as the entry (ETU:9-11) but is located after line 4600 (see below if read).

Inline mods read so far and ranges: atomic_operations 151-623; borrowed_windows 624-1951; byte_operands 1952-2153 (pub(crate)); candidate_closure 2156-2688; composed_control 2691-2746 (+ `assembly::finish`); initialized_locals 2749-2853; local_storage 2854-2963; plan_gaps 2964-3110 (pub(crate)); receiver_aliases 3111-3446; `use validation::reference_result_custody as reference_results` at 3447; scalar_locals 3448-3488; scalar_return_expressions 3489-3522 (pub(crate)); scalar_targets 3523-3943; storage_transfers 3946-... (its `completion` submod, `expressions` 3996-4088, `plans` 4089-4527, `successors` 4528-... continues past 4600).

#### Shape / how it decides (read)
- atomic_operations: `writing_event` ETU:220-330 maps `AtomicAccessOperation` to `CheckedAtomicEvent` by exact match (Store, Swap, FetchAdd/Sub/And/Or/Xor, CompareExchange); `Load`, `CompareExchangeOnce`, `TryExchangeOnce` return None here (294-298). Operand count by `<[ScalarArgument; N]>::try_from` (262-286). Result binding: observing event needs a reserved placeholder whose `statement_index + 1 == statement_index` (301-310). `place` ETU:480-622 requires `place_is_atomic_storage`, builtin coordinates, and segments limited to Field/FixedIndex/Case (514-521); rejects RuntimeIndex/ComputedIndex/FixedRange in the final path (598-607). Case lookup: scans ALL data definitions for a variant whose symbol equals the segment and requires exactly one (540-553).
- borrowed_windows: `window_place` ETU:1097-1193 (source classification: local vs parameter; required access vs shell access check at 1122-1133; parameter access must be Owned/MutableBorrow, or WriteOnly when WriteOnly required, not Linear MutableBorrow, 1152-1162). `resolve_storage_place` 1202-1241 rebases through reference-typed locals with a cap of 16 iterations (`for _ in 0..16`, 1212); on exhaustion returns the partially rebased place (no failure). `OpenWindows` 1249-1850 tracks opened holes keyed by `same_storage_place` (712-729, uses `checks::storage_path_compatibility` and requires `CapturedPlaceContainment::Same`). `replace` 1434-1554 handles range-assignment `a[x..y] = array` by expanding into per-element `replace_place` calls and requires `source_length.literal() == Some(length)` plus element identity equality (1476-1482). `replace_place` 1561-1839: an unoccupied, un-opened slot with non-Unrestricted multiplicity is refused unless `establishes_descriptor` or a `storage_establishments` row matches (1679-1701); `Linear && needs_displacement` refused (1758).
- byte_operands::plan 1972-2028 tries byte literal, then place, then subslice (admit_resolved). `place` 2031-2102 compares `normalized_type_identity(projected)` to the target; a reference target peels one shell if not WriteOnly.
- candidate_closure 2156-2688: `CandidateClosure` (2177-2286) is a dense roster indexed by candidate ordinal with reverse-edge `dependents: Vec<(target, caller, site)>`; `close` (2247-2285) propagates unavailability with a work queue over sorted dependents. Keys: machine symbol via `symbol_key`, `binary_search_by_key` (2236). `retain_available` 2400-2672 walks ordinary bodies, composed bodies and partial-affine bodies; per call route: `Boundary` requires the target machine in the sorted boundaries list (2330-2340); `Module` with Unit/Structural/NoNormal result calls `require_entry`; `Module` with Scalar result consults `scalar_targets::available_target`; routes `Dynamic | ActivationStorage | DescriptorTable` are accepted with no check (2359-2364). In composed machines any operation kind not on the explicit list is dropped with `GapReason::Unsupported("composed control vocabulary")` (2615-2621). In ordinary bodies the match at 2452-2517 has no catch-all (exhaustive).
- plan_gaps 2964-3110: maps a gap site (Machine/State/Statement) to a source span and `CoverageForm`; `statement_site` is an exhaustive match over `StatementNode`.
- receiver_aliases 3111-3446: erases nonescaping `&mut`/write-only reference locals: `formation` 3266-3360 requires immutable local, valid symbol, no prior alias, exclusive access equal to borrow access, no `Index` segment in the place, equal `lent_storage_identity`, exactly one loan owned by the local at that statement, and a direct or reborrow loan resource row. `projection::formation_place` 3135-3242 walks Name/Member/Indexed only, resolves the member symbol by NAME: `resolve_member_symbol_from_type_symbol(..., member.member.as_str())` and rejects if `member.member_symbol` differs (3191-3198). `resolve` 3421-3445 loops at most `aliases.len()` times.
- scalar_targets 3523-3943: `available_target` 3652-3899 checks a scalar call's target against exactly one candidate plan (ordinary/composed/partial-affine), contract fingerprints/commitments, parameter shape equality, claim catalogue equality and call-site flow coordinates; returns None on any mismatch (fail-closed). `claim_transfers_match_entry_catalogue` 3909-3942 requires every claim to have `carry == STRICT` and either a `Case` path segment or a transfer.
- storage_transfers::completion (3979-...): walks completion plans (`CompletionPath` with exit/guards/outcome, 4139-4143); errors are `&'static str` messages (e.g. 4254). `direct_paths` 4243, `scalar_paths` 4266 match scalar return destinations to checked source exits and require exactly one match (4310-4315). `ComposedCompletion::paths` 4326-4486 is an exhaustive match over `Terminator` variants.

#### Inputs and outputs (so far)
Reads `CheckFacts` (flow, borrow, proof, values.scalar_expressions, values.scalar_computations, values.structural_values, contextual_cleanup dispositions and storage_establishments, contract_plans, service_reaches). Produces operation plans (`CheckedUnitEffectOperationPlan`), `CheckedAtomicAccessPlan`, `ReceiverAlias` lists, `CheckedUnitPlanGap` rows, availability verdicts.

#### Same job elsewhere
- `same_storage_place` (ETU:712-729) vs flow-side `crate::flow::canonical_places_agree` used in ESC:6884: both decide "same place"; different functions (suspected duplicate relation, not read).
- `resolve_storage_place` (ETU:1202-1241, cap 16) "Mirrors the checker rule that windows key on the resolved storage place" (comment 1198-1200).
- Dispatch-completion path building appears in ETU:4107-4527 (`Guard`/`CompletionPath`) and is a second encoding of the same terminator shapes enumerated in `candidate_closure` (2538-2638) over `CheckedComposedUnitControlTerminatorPlan`; same terminator vocabulary handled in 2 separate matches (read).
- `window_place`'s access rule (1122-1133, 1152-1162) vs `byte_operands::place`'s WriteOnly rule (2078-2084): similar access gating coded twice (read).

#### Unused / one-caller code
- Not yet counted by grep for ETU items (see end section).

#### Stated limits (read)
- "their current coverage differs, so a failed candidate is not permission to omit its unsupported statements." (ETU:26-27)
- "What remains open is a contract that names an erased binding" (ETU:95-100): `pass/relevance/erased_parameter_proof_only` is "a checked-only canary".
- "The builders still return `Option`; the construction trace names the phase and statement where they stopped." (ETU:46-47)
- composed control "composed control vocabulary" unsupported reason string (ETU:2618).

#### Fail-open risks (so far)
- `resolve_storage_place` ETU:1212-1240: after 16 iterations (or on any failed lookup `break`) returns the partially rebased `(root, segments)` instead of failing. Status: open. Wrong program: a chain of 17+ reference-copy aliases would resolve to an intermediate local rather than the owning parameter; `window_place` would then treat the local root path (1120-1143) as storage and `same_storage_place` compare against the wrong root. Not traced.
- `candidate_closure::admit_call_target` ETU:2359-2364: `Dynamic | ActivationStorage | DescriptorTable` routes add no dependency edge and no check. Status: open (no comment at the site). Wrong program: a body that calls through such a route is kept even if the called implementation has no plan; I did not trace whether availability for those routes is established elsewhere.
- `retain_available` ordinary bodies, operation kinds `RegisterAccess | GlobalDescriptorTable | EvaluateStructuralValue | Store | AtomicFence | AtomicAccess ...` (ETU:2476-2502): no dependency check; stated by structure (they have no callee), not by comment.
- `receiver_aliases::aliases` ETU:3374-3380: returns `Some(Vec::new())` if the state has no loans, and `.unwrap_or_default()` at the use in `atomic_operations::place` (500-501) converts a `None` (no flow/borrow state) to an empty alias list. Status: open. Wrong program: an atomic place accessed through an erased `&mut` alias whose flow state lookup failed would be planned against the alias local instead of its root. Not traced.
- `byte_operands::store_operand_type_identities` ETU:2110-2152: returns empty Vec when machine/state lookup fails (a type-registry omission, not an accept).
- `ValueReplacement::into_operations`/`replace_place` use `.ok()?`-style refusal elsewhere (fail-closed).

### ETU lines 4601-8373 (read in full)

#### Shape
- `storage_transfers` mod continues (3946-6474): `Transfers {possible, definite, after_production}` (5438-5448) with `append` (5458), `alternatives` (5472, intersection via `common_paths` 5504-5515: a path is definite only if every alternative covers it by `starts_with`), `Destination` enum (Holder/Backing/Call/Storage/Outgoing/RetainedBySuccessor/Unknown, 5364-5402), `Query` (5518-5529) cloned per scope. Entry points: `transfers` 5531, `from_storage` 5557, `at_completion` 4812, `has_computation_transfer` 5662, `parameter_vacancies` 5711-5783, `extraction_owns_source` 6415. Submods: `completion` (3979-5060 incl. `expressions`, `plans`, `successors`) and `computations` 5061-5352 (`Query::computation` 5226-5350 is an exhaustive match over `ComputationKind`).
- `Query::value_kind` 6130-6406 matches `CheckedStructuralValueKind` with a trailing `_ => {}` (6403): kinds not listed contribute no transfers. Listed: Place, Call, Projection, Record/StructuralCase, FixedArray, Dispatch, ScalarArray, ZeroedScalarArray, Case, Reference, CopiedStructuralPlace, ViewSubslice, BorrowedSliceView/MutableSliceView. Not listed (fall to `_`): from ESC's `ValueKind` list the kinds `Scalar`, `NativeCodeLayout`, `QuotientAdaptation`, `QuotientCall`, `ByteLiteralView`, `ZeroedStructural` (read ESC:9019-9029, 9039, 9082, 9107, 7456). Whether that is intended I did not trace.
- `structural_value_calls` mod 6477-6969: `PreparedCalls` (scratch of per-root call schedules, 6510), `prepare_before` 6597, `prepare_state` 6840, `prepare_root` 6898, `consume_caller_results` 6708-6826, `retain` 6682-6701 (error `"surviving bodies disagree on a structural value call schedule"`).
- `calling_signatures::collect` 6971-7062 builds declaration ABI storage; refused ABIs stay as `Err(String)` rows ("A refused dormant ABI remains a refused row, not a failed source check", 6974).
- `ScalarCalleeClosure` unit struct 7077 (marker).
- `build_checked_unit_effect_plans` ETU:7079-7996 (the entry named in the header). Pipeline inside: collect ABI signatures (7092) -> boundary machines + dynamic slots (7097-7135) -> plan every checked body in parallel against forked `ShapeCollector`s with a `ChangeLog` for stale-attempt detection and re-planning (7141-7228; `fork_from` uses rayon on `checking_thread_pool`) -> collect construction gaps (7231-7237) -> select cleanups per candidate and install them, dropping those that fail (7248-7419) -> `candidate_closure::retain_available` (7428) -> prune `prepared_calls` (7439) -> retain type identities for every surviving shape through a very long chain (7486-7964) -> `shapes.retain_transitive` (7965) -> assemble `EffectPlans` (7976-7995). Returns `(EffectPlans, CheckedPartialAffineUnitCleanupPlans, PreparedCalls)`.
- `plan_checked_body` 8035-8117; `BodyRoute::{Sequence, Graph}` with `body_route` 8140-8186. `build_checked_nominal_affine_unit_cleanup_plans` 8195-8236. Helper shape builders `scalar_operand_shapes` 8247, `reference_backing_shapes` 8262, `scalar_case_value_shapes` 8276; `retain_value_types` 8294-8373 (worklist with `visited: Vec` linear `contains`, 8302-8307).
- `symbol_key` ETU:7998-8000: `(arena_index, generation)`.

#### How it decides
- `body_route` ETU:8140-8186 (keyed on authored shape, comment 8119-8123 "no body has two descriptions"): `Sequence` if not a checked body (8144) or if the declared result plan is None (8150); `Graph` if multi-state (8147-8149); `Graph` if any statement is a `Transition`, or a local/parameter "owes a nominal drop" (non-reference, not Linear, `type_graph_requires_nominal_drop`), or the state has contracts (8164-8175); otherwise `Sequence` only if the result is invalid/Unit/primitive, else `Graph` (8176-8185).
- `select_sequence_cleanup` ETU:7248-7281: a sequence body that has a non-normal-continuing operation must have it last (7267), with an empty normal-exit set; a body without `scalar_control` must have exactly one normal exit and it must be `FinalExpression` (7272-7278). Failure returns None -> the machine is dropped with gap `Unsupported("sequence cleanup actions")` (7384-7419, 7405-7416).
- Parallel plan reconciliation: an attempt is accepted if `log.admits(&edits, base)` (7185); otherwise replan (7190-7211). Keyed on `ChangeLog` row marks, not on source text.
- `consume_caller_results` 6708-6826: matches existing calls by `coordinate` only (6731-6738) and by `call.target.state` + argument vector equality for repeats (6773-6781).
- `transfers` keys: source identity equality via `same_source` (5891-5905): `*source == self.source` or a `StructuralLocal` that is the recorded root of a `StructuralResult` binding.
- `Query::value` guards recursion by `active.contains(&value)` (5995) -> returns None on a cycle.
- `parameter_vacancies` 5711-5783: returns None if a store would write a path strictly below a path with a possible transfer (5752-5756).

#### Cases by shape
- `ComposedCompletion::paths` 4326-4486: exhaustive per-`Terminator` list. `successors` Operand pairing sorts by authored position and errors on a duplicate position (4595-4598). `return_arm` errors on `Crash`/`EvaluateNoNormal` (5038-5040).
- Guarded terminators require `return_values.len() == arms.len() + usize::from(fallback.is_some())` (4447-4451).
- `body_route` list in "How it decides" is the main shape table.
- `scalar_targets::available_target` requires exactly one candidate plan for the target (3736, 3751) and `authored_states` to be a single state unless composed (3680-3687).
- `claim_transfers_match_entry_catalogue` (3909-3942): only strict carry claims accepted (3921).

#### Same job elsewhere
- Two separate walkers of the structural-value DAG for custody: `Query::value_kind` (ETU:6130-6406) and `retain_value_types` (ETU:8294-8373), plus ESC-side `Builder::structural_value` that creates it. The `retain_value_types` match has `_ => {}` (8369); `Query::value_kind` has `_ => {}` (6403). Read both.
- Terminator/exit enumeration: `ComposedCompletion::paths` (4326) vs `candidate_closure::retain_available` composed loop (2538-2638); plus `scalar_targets::of_composed_operation` (3580-3649) which re-enumerates the same terminator variants (Guarded, CompleteExpression, return arms, successors) to find operation scopes (read). Three separate enumerations of `CheckedComposedUnitControlTerminatorPlan`.
- `symbol_key` is defined in ETU:7998 and also as separate private functions in 8 other files (grepped `fn symbol_key\b` over omega-rust *.rs: machine_calls_core.rs:402, machine_calls_core.rs:4348, type_reference.rs:1854, lowerer.rs:1724 (pub(crate)), checks.rs:1706, symbols.rs:2946, declaration_tables_and_index.rs:3514, facts.rs:1447, plus this file = 9 definitions total, 9 counting ETU:7998). Same `(arena_index, generation)` body each time (read only this one; others suspected identical).
- The same `(arena_index, generation)` pair is written inline in ESC (e.g. ESC:1307, 3935 area, 9151, 8935) and ETU `symbol_key`.
- `ESC::Builder::call_ordinal` (ESC:10484-10504) and `ETU::scalar_targets::available_target` (3828-3835) both locate a flow call by (statement_index, call_ordinal / authored expression); the former keys on `authored_expression`+`target_symbol`, the latter on the `coordinate`.

#### Unused / one-caller code (grepped over omega-rust *.rs)
- `build_checked_unit_effect_plans`: 1 external call site (execution.rs:304); plus the header comment in ETU:9-10 and a comment at execution_terminal_unit_cleanup.rs:2799. The header (ETU:10-11) says "`build_checked_unit_effect_plans` is its test-only form without call frames" - this reads as a duplicate sentence; the function at 7079 takes `call_frames: Option<...>` and I found no second function by that name, so the "test-only form" it describes was not found in this file (grepped: only the one definition).
- `build_checked_nominal_affine_unit_cleanup_plans`: 1 call site (execution.rs:314).
- `retain_structural_value_calls`: 1 call site (execution.rs:382).
- `scalar_operand_shapes`, `reference_backing_shapes`, `scalar_case_value_shapes`: each 1 call site (execution.rs:1446, 1538, 1410) plus `use` lines at execution.rs:1389-1391.
- `calling_signatures::collect`, `plan_gaps::plan_gap` (used in this file at 2166, 7111, 7334 etc.), `ScalarCalleeClosure`: not grepped for external count.
- `PreparedCalls::state_schedules` etc.: not grepped.

#### Stated limits
- "an unrelated preceding producer must not erase a later value's call schedule" (6486-6487) (design note).
- "Rejected bodies discard their schedules; final retention publishes surviving schedules" (6490-6491).
- `retain` Err string "surviving bodies disagree on a structural value call schedule" (6689).
- Unsupported reasons recorded as strings: "composed control vocabulary" (2618), "sequence cleanup actions" (7395, 7413), and `trace.phase(...)` strings (e.g. "single-state body: value call source custody" 8076).
- "A refused dormant ABI remains a refused row, not a failed source check." (6974)

#### Fail-open risks
- `candidate_closure::admit_call_target` `Dynamic | ActivationStorage | DescriptorTable => {}` (ETU:2359-2364): see above, open.
- `Query::value_kind` `_ => {}` (ETU:6403): value kinds not listed yield empty transfers. Status: open (no comment). Wrong program: a Scalar/QuotientCall/QuotientAdaptation/ZeroedStructural/ByteLiteralView value whose subtree contains an owned move out of a source (for example a `QuotientCall` whose `arguments` hold a `Place` that moves an owned local) would record no transfer, so a later cleanup/retirement of that source could be judged unmoved. I read that `QuotientCall` arguments are published as structural values (ESC:4978-5030) and that `retain_value_types` does traverse `QuotientCall` arguments (ETU:8344-8346) while `value_kind` does not; I did not trace whether quotient arguments can be affine (ESC:4995-4996 requires `Multiplicity::Unrestricted` for the quotient adaptation input, which limits it for that arm).
- `Query::operation_computations` ETU:5116-5164: `_ => {}` arms for operation/binding kinds not listed (e.g. `EstablishLocal` bindings other than Scalar/Primitive/ScalarArray/ByteSequence, `Store` values other than Scalar computation/Place/Bytes). Status: open. Wrong program: not traced.
- `build_checked_unit_effect_plans` ETU:7384-7419: a machine whose cleanup installation fails is dropped with a gap (fail-closed).
- `retain_value_types` `_ => {}` (8369) only affects which type identities are retained; a missing identity would surface as a missing shape later (not an accept).
- `body_route` ETU:8144: any non-`CheckedBody` machine routes to `Sequence` (stated by comment 8126-8131? the comment describes only checked bodies); boundary machines are handled separately (7098-7121), so this branch is only reached for machines passed from `plan_checked_body`, which is called only on `supplies_checked_body()` machines (7144). Traced: reachable only if `supplies_checked_body()` and `supply_mode == CheckedBody` disagree. Not traced further.
- `value_call` ETU:6021-6128 returns None on ambiguity (fail-closed). `through_prefix` returns `(output, true)` when no later store overwrites (6650).
- No `unwrap`/`expect`/`panic!`/`unreachable!` in ETU (Grep count `unreachable!|panic!\(|\.unwrap\(\)|\.expect\(` = 0). `unwrap_or_default()` at ETU:500-501, 7883-7884, 8312 (default alias/binder list on lookup failure).

---------------------------------------------------------------------
## Coverage
- execution_scalar_computations.rs 1-11126: read in full (11,126 lines).
- execution_terminal_unit.rs 1-8373: read in full (8,373 lines).
- Skimmed: none. Not read: none. Total 19,499 lines read. (The four child files that ETU pulls in with `#[path]` (`execution_terminal_unit_calls.rs`, `_cleanup.rs`, `_control.rs`, `_state_graph.rs`, `_structural_scalar_store.rs`, `_types.rs`) are not part of this unit and I did not read them.)

## Five things a maintainer most needs to know about this unit
1. `ESC` is a failure-silent planner: nearly every builder returns `Option`, `record_root` (ESC:9561-9598) drops a root when planning fails, and the callers use `let _ =`/`continue` (ESC:10538-10543, 10590). Absence of a plan is the only signal; later stages must treat missing rows as refusal (not traced here).
2. `ESC::build_expression` (ESC:9932-10390), `integers::build_integer_operand` (ESC:4253-4604) and `Builder::structural_value` (ESC:8945-9381) are ordered `if let` ladders; the order is the policy. The integer prefix ladder is duplicated between the first two (ESC:4258-4360 vs 9937-9981).
3. Identity matching is mostly by handle and symbol, but `structural_carriers_match` (ESC:8297-8353) also accepts Generic/Named types whose base names are equal strings when symbols differ (ESC:8329-8330, 8348-8349); it is used by call results (ESC:9274), case construction (9299) and struct-case values (7514).
4. `ETU::build_checked_unit_effect_plans` (ETU:7079-7996) is the single entry (1 external caller, execution.rs:304). It plans bodies in parallel against forked shape collectors, drops candidates in three passes (construction gaps, cleanup selection, transitive call-closure), and only then retains type shapes. The call-closure accepts `Dynamic`, `ActivationStorage` and `DescriptorTable` routes without a dependency check (ETU:2359-2364).
5. The storage-transfer queries (`storage_transfers::Query`, ETU:5518-6406) decide cleanup custody by walking the structural-value DAG; `value_kind` and `operation_computations` end in `_ => {}` (ETU:6403, 5163, 5145), so a value or operation kind not explicitly listed contributes no transfers.

## What I am unsure about
- Whether the `_ => {}` fall-throughs in `storage_transfers` (ETU:5130, 5145, 5163, 6403) are reachable with a value/operation kind that can carry an owned move; I read the arms but did not trace which kinds can reach them.
- Whether the name-string comparison in `structural_carriers_match` (ESC:8329-8330, 8348-8349) can see two different types with the same spelled name; I did not trace the callers' upstream type checks.
- Whether the `Dynamic | ActivationStorage | DescriptorTable` routes in `candidate_closure::admit_call_target` (ETU:2359-2364) are validated elsewhere; I did not read the files that build those routes.
- The header sentence (ETU:10-11) about a "test-only form" of `build_checked_unit_effect_plans`: I found only one definition (ETU:7079) with grep; I did not search test directories separately (the grep covered all `*.rs` under omega-rust and found no other definition).
- The behaviour of `unwrap_or_default()` fallbacks (ETU:500-501, 7883, 8312) on lookup failure was read but not traced to a consumer.
- Counts of external callers for `call_operands` (ESC) and for the `pub(crate)` modules `byte_operands`, `plan_gaps`, `scalar_return_expressions` were not grepped; `scalar_return_expressions::checked_boolean_contains_short_circuit` (ETU:3493) has not been checked for callers.


