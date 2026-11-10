# Unit 76 notes

Files covered (all under `omega-rust/psi/compilation/proof/src/`):
1. `checker.rs` (1-6550)
2. `derivation_store.rs` (1-299)
3. `lemmas.rs` (1-377)
4. `lib.rs` (1-1158)
5. `mathematical_declarations.rs` (1-12715)

All claims are "read" unless marked grepped/inferred. Paths below are relative to the crate `src/` directory.

---------------------------------------------------------------------
## 1. checker.rs (6550 lines)

### Purpose
Walks every obligation in a `ProofPlan` (built in `obligations.rs`, not in this unit) and decides whether each bounded assignment, call argument, initializer, transition argument or state return fits its declared integer/float range or named constraint. For integer ranges it first builds a certificate and hands it to the `proof_admission` kernel (`verify_obligation`); when it cannot build one ("Uncovered") it falls back to older interval reasoning that does not go through the kernel. Output is `Result<ProofDecisions, Vec<Diagnostic>>` (6468).

### Shape (the file is one list of inline `mod` blocks)
| inline mod | lines | what it holds |
|---|---|---|
| `arrival_stability` | 12-127 | `ArrivalPrefix` {written_paths, local_definitions} (18); `prepare_prefix` (51) scans statements 0..=index collecting may-write paths and local names; `preserves_reads` (24) |
| `assignment_stability` | 128-284 | `collect_stable_assignment_conditions` (137); `written_place_path` (218); `collect_read_place_paths` (241); `member_paths_may_alias` (280); `resolved_writes_overlap_reads` (209) |
| `bounded_checks` | 285-846 | the five `check_bounded_*` entry points (332, 514, 591, 656, 729), `assignment_is_overwritten_before_consumption` (420), `discharge_declared_float_range` (824) |
| `certificate` | 847-2855 | `CertificateVerdict {Certified, Rejected, Uncovered}` (884); `BoundedValueCertificate` (891, verify at 899 calls `verify_obligation` with `EvidenceRoute::CertificateDerived`); builders `bounded_integer_value` (1720), `declared_interval_certificate` (1810), `guarded_bounds_certificate` (1268), `guarded_refold_certificate` (1463), `refold_bounds_certificate` (1574), `contract_refined_certificate` (1058), `interval_premise_certificate` (1032), `bounded_conjunction_certificate` (1848); closed math term helpers (1923-1990); `fixed_integer_type` (1992). Large test modules 2013-2854 |
| `derivation_cache` | 2856-2977 | `ProofDerivationCache` (2876) wraps `DerivationStore<BoundedValueCertificate>`; `DerivationConsultation` (2925) with `recheck` (2942, re-verifies every candidate through the kernel) and `retain` (2963) |
| `diagnostics` | 2978-3232 | message constructors `cannot_prove_*` (format! strings with display names) |
| `float_ranges` | 3233-4580 | `float_range_for_*` (macro 3257), `float_range_at` (3347, reaching-store analysis for f32/f64 locals/fields), `guarded_float_range` (3516), `apply_float_condition`/`apply_float_comparison` (3639/3755), `stable_initializer` (3949), `same_float_place` (3982) |
| `guards` | 4581-4957 | `apply_handle_guard` (4593), `apply_assignment_guard` (4607), `apply_handle_condition` (4720), `apply_source_condition` (4756), `expressions_equivalent_for_proof` (4832), `flat_place_segments` (4908), `apply_right/left_literal_guard` (4930/4949) |
| `integer_ranges` | 4958-5560 | `guarded_integer_range_for_transition_argument` (5034), `guarded_integer_range_for_assignment_with_context` (5190), `AssignmentRangeContext` (5375, lazily builds `validation::CallFrameResolver` in a `OnceLock`), `proved_assignment_integer_range_with_context` (5395), `place_refined_binary_range` (5103), `integer_literal_handle` (5531) |
| `measurement` | 5561-5872 | `ProofPlanMeasurements` counters (5570), `render` (5641), env var `OMEGA_PROOF_MEASUREMENTS` (5675), `proof_node_count` (5697) |
| `named_constraints` | 5873-6156 | checks named constraints `finite`, `exact`, `positive`, `non_negative`, `wrapping` |
| `requires_conditions` | 6157-6338 | `surviving_conditions` (6167): entry/state `requires` facts that survive to the statement; `refine` (6229); `stable_expression` (6263); `same_place` (6283) |
| `return_arrival` | 6339-6391 | `integer_range` (6346) for state-return obligations |
| top level | 6393-6550 | `check_obligations!` macro (6413), `check_proof_plan` (6454), `ProofDecisions` (6463), `check_proof_plan_with_decisions` (6468), `check_proof_plan_inner` (6475) |

### Inputs and outputs
Reads `ProofPlan` (`.program` = `dyn validation::CheckingInput`, `.obligations`, `.type_constraints`, `.assignment_value_ranges`), typed expression/statement tables, and `validation::` helpers (`CallFrameResolver`, `arrival_integer_expression_bounds`, `occurrence_integer_expression_fits_range`, `land_anonymous_integer_expression`, `BigIntInterval`, `frame_paths_overlap`, ...; not read, other crate). Produces diagnostics (plain `Diagnostic::error(format!(..))`) and `ProofDecisions.integer_assignments` (obligation handle + proved range). Mutates only a `ProofPlanMeasurements` and an optional derivation cache.

### How it decides (keys)
* Per obligation the loop at 6486-6536 uses `seed = index + 1` as certificate/obligation/value id (6487).
* Integer path, same shape in all five checkers (e.g. 350-400): (a) named constraints, (b) if the target has an integer range: kernel verdict via `certificate_verdict_measured` (934); `Certified` -> accept, `Rejected` -> diagnostic, `Uncovered` -> fall back to non-kernel interval reasoning (`guarded_integer_range_*`, `validation::occurrence_integer_expression_fits_range`, `validation::arrival_integer_expression_bounds`); (c) float range check by interval containment (`discharge_declared_float_range` 824).
* Expression equality for guard matching (`guards::expressions_equivalent_for_proof`, 4832): handle equality, then `flat_place_segments` (4908) which compares the spelling of each path segment as `String`s (4914-4925), then structural cases; Name vs Name compares `name_path_members` (identifier spelling, 4858-4861); Member compares `member` spelling AND `member_symbol` (4891-4892); Call compares target spelling and symbol (4863-4864). So guard matching in the integer path is keyed on spelling for names and on spelling+symbol for members.
* `requires_conditions::same_place` (6283) uses `expressions_structurally_equal` (typed-trees, other crate) plus `stable_expression` (6263) which demands valid symbols.
* Float path is keyed on symbols: `resolved_place_identity` (3451) builds `Vec<SymbolHandle>`; `same_float_place` (3982) compares canonical places (root symbol + segments).
* `ensures_witness_bounds` (field of `BoundedAssignmentObligation`, in obligations.rs) is matched by `display_name` string equality: `place == &value_display` (5200), `place == &operand_display` (5215, 5329).
* `assignment_is_overwritten_before_consumption` (420) decides by `display_name(assignment.target) == target_name` (472) plus `member_paths_may_alias` (280, prefix compare over `Vec<String>`).
* Arrival/stability: write sets and read sets are `Vec<Vec<String>>` joined with `.` and compared by `validation::frame_paths_overlap` (209-216): string paths.
* `named_constraints::constraints_satisfy_named_constraint` (6127): compares the constraint name string against `"exact"`, `"finite"`, `"non_negative"`, `"positive"`, `"wrapping"` (6141-6153); `argument_handle_satisfies_named_constraint` (5996) has a literal-value table (6009-6019).

### Cases by shape (fixed lists that refuse the rest)
* `fixed_integer_type` (1992): only I8..I64/U8..U64; Selector, Bool, F32, F64, Addr return None, i.e. no certificate for `Addr`.
* `closed_math_term` (1923): only Integer literal, Name with a literal value, and Binary Add/Subtract/Multiply with `has_anonymous_operator_meaning` (1934-1951); everything else None.
* `guarded_refold_certificate` (1463): only `place + literal`, `literal + place`, `place - literal` (1471-1485); comment-free. Requires the argument to be exact arithmetic (1489-1497) and the place's primitive to be a fixed integer type.
* `collect_guard_bounds` / `collect_contract_bounds` (1335, 1126): condition shapes: `X == true`, `!X`, `A && B`, comparison of the subject with an integer literal on either side (via `BigIntInterval::from_comparison`).
* `place_refined_binary_range` (5103): only `Add` and `Subtract` with an integer literal on one side (5121-5134).
* `apply_assignment_guard` (4607): only `a - b` value with a guard comparing the same two operands, giving lower bound 1 or 0 (4618-4661).
* `arrival_stability::prepare_prefix` (51): accepts only Assignment, LocalData, Call, Expression statements before the target; any other statement kind returns None (90) i.e. no stability.
* `float_statement_preserves` (3324): Assignment/Call/LocalData/Expression and (optionally) Transition; anything else false.
* `guarded_float_range` (3516) loop: Assignment, Call, Transition-with-refuted-guard, LocalData/Expression; `_ => break` (3599).
* `float_range_at` (3347): only reaching literal stores to f32/f64 places whose identity is a single-segment name or member chain with valid symbols; literal landing format must equal the place format (3415, 3439).
* `operand_declared_primitive` (5446): only `self.<field>` member where the receiver is a single self-receiver name (5453-5457).
* `check_obligations!` (6413) and the call at 6501: `BoundedValue` and `GuardedTransition` are explicit no-ops (6522) ("decided elsewhere" per measurement field 5576, 5615).

### Same job elsewhere
* The five `check_bounded_*` functions repeat one pattern (named constraints, kernel verdict, fall back to interval, float range): 332-418, 514-589, 591-654, 656-727, 729-822. `guarded_float_range_for_*` (3468, 3487) and `float_range_for_*` macro (3257) are the float-side twins of `integer_range_for_*` macros (5002-5027).
* `collect_guard_bounds` (1335) vs `collect_contract_bounds` (1126) vs `guards::apply_handle_condition` (4720) vs `requires_conditions::apply_condition` (6297) vs float `apply_float_condition` (3639): five walkers over the same condition shapes (`== true`, `&&`, literal comparison), one per consumer. `collect_guard_bounds` and `collect_contract_bounds` differ only in which equality test they use (`expressions_equivalent_for_proof` vs `requires_conditions::same_place`) and in `!` handling.
* `push_right_literal_bounds`/`push_left_literal_bounds` (1415, 1434) and `guards::apply_right_literal_guard`/`apply_left_literal_guard` (4930, 4949) both call `validation::BigIntInterval::from_comparison`.
* `guarded_bounds_certificate` lower/upper anchor loop (1275-1303) and the same loop inside `guarded_refold_certificate` (1530-1556) are the same "strongest lower/upper anchor" selection written twice (one over `IntegerMath` props, one over scalar props).
* `integer_ranges::neutral_range` (5501) uses i64::MIN..i64::MAX.
* `AssignmentRangeContext` / `CallFrameResolver::new(program)` is created in several places: `AssignmentRangeContext::call_frames` (5388, lazy once per context), but `float_ranges::float_range_at` (3366), `guarded_float_range` (3526) and `stable_initializer` (3969) each call `CallFrameResolver::new(program)` directly (grepped: `CallFrameResolver::new` appears in those three plus 5390).
* Expression-equality helpers: `expressions_equivalent_for_proof` (checker.rs:4832) vs `expressions_structurally_equal` (typed-trees `values.rs:4006` and a second private fn at pipeline `03_symbol-resolved-trees-to-typed-trees/.../equality_and_exhaustiveness.rs:1726`, not read -> "suspected" third copy of structural equality).

### Unused / one-caller code (greps)
* `check_proof_plan` (6454): grep `check_proof_plan\b` over `omega-rust/**/*.rs` -> 0 callers outside its definition (only mentioned in lib.rs:24 comment for the `_with_decisions` form). `check_proof_plan_with_decisions` has one outside caller: `pipeline/05_typed-trees-to-checked-trees/src/checking.rs:422`.
* `check_proof_plan_inner` takes `Option<&mut ProofDerivationCache>`; its only caller (6472) always passes `None`. grep of `ProofDerivationCache|DerivationStore|DerivationConsultation|derivation_store::` over `omega-rust` *.rs: 53 occurrences, all in `checker.rs` (45) and `derivation_store.rs` (8); none outside the proof crate src. So the cache is exercised only by tests inside `checker.rs` (2392-2555) in production code `derivation` is always `None` at 6494-6500 (`derivation_cache.as_deref_mut()` of a `None`).
* `_recipe_obligations` parameter in all five `check_bounded_*` (338, 519, 597, 661, 734) is never used; `recipe_obligations` is created empty (6482) and `ProofDecisions.constant_recipe_obligations` (6465) therefore always empty (grep `recipe_obligations` in proof/src: only checker.rs, 10 hits, no `push`).
* `proved_assignment_integer_range_with_context` (5395, pub): grep with `AssignmentRangeContext` -> checker.rs 24 occurrences, plus `pipeline/05.../src/values.rs` 2 occurrences (outside).
* `lemmas.rs` types: see section 3.

### Stated limits
* None phrased as "unsupported" in this file; header comment block 1-10 only. Test names such as `affine_refold_gap_stays_uncovered` (2316) document the Uncovered path.

### Fail-open risks
1. `check_bounded_assignment` returns `None` (and skips ALL checks including named constraints) when `assignment_is_overwritten_before_consumption` is true (342-344). Reached by an assignment to a non-indexed place followed in the same state by another assignment whose target has the same `display_name` string, with only Assembly/LocalData/Expression/non-aliasing Assignment statements in between and no calls/transitions (453-509; the `display_name` equality is at 472). Wrong program that could pass: `x = <out-of-range>; x = <in range>` is intended; the string-equal check means a different place that displays identically (e.g. shadowed local with same spelling) would count as the overwrite. Status: "open" for shadowing (I did not trace `display_name`); the later-statement read check is by path prefix so a read of the target aborts (462-467).
2. `Uncovered` fall-through accepts without the kernel when the legacy interval fits: assignment (374-399), initializer (547-577), call argument (689-715), transition argument (762-808), return (623-642). These are "stated" only by structure (the kernel is attempted first); the fallback reasoning (`guarded_integer_range_*`, `validation::occurrence_integer_expression_fits_range`, `validation::arrival_integer_expression_bounds`) is not re-checked by `proof_admission`. Status: "traced" for the control flow, "open" for soundness of the validation helpers (other crate).
3. `operand_is_unsigned` / `operand_declared_primitive` (5427, 5446): loops over ALL machines and takes the first machine whose attached data has a field with the member's name (5459-5473), not the current machine; used to lift `narrowed.minimum` to 0 (5220-5224, 5334-5337) when an `ensures_witness_bounds` entry matches the operand's display name. A signed field with the same spelling in another machine, listed later, would not be seen; an unsigned one listed first would wrongly lift the lower bound. Status: "open" (did not trace the producer of `ensures_witness_bounds`).
4. `integer_range_for_value` (4991): any non-literal operand takes its range from its declared constraints alone (4998); relied on by `integer_range_for_argument`. Status: "stated" implicitly (it is the declared range of the operand's type); not traced.
5. `check_obligations!` no-op for `BoundedValue` and `GuardedTransition` (6522): these produce no diagnostics here. Status: measurement comments say "decided elsewhere" (5576); not traced where.
6. `named_constraints`: `("finite", ExpressionNode::Integer(_)) => true` and `("wrapping", Integer) => true` and `("exact", Integer) => true` (6009-6018): integer literal of any value satisfies these. Intended (an integer literal is exact/finite).
7. `refine` (6229) returns the unrefined range when refinement would be empty (6256-6258), so contradictory `requires` do not make the range vacuous (fail-closed direction). `contract_refined_certificate` drops contract facts when narrowed is empty (1099-1101) - same direction.
8. `arrival_stability::ArrivalPrefix::preserves_reads`: an empty read path counts as overlapping every local (33, `is_none_or`) - fail-closed.
Everything else on the certificate path is decided by the kernel (`verify_obligation`, 900) so a wrong certificate becomes `Rejected` (956).

---------------------------------------------------------------------
## 2. derivation_store.rs (299 lines)

### Purpose
Generic arena-backed store of "derivations" keyed by `ProofObligationKey` (from obligations.rs). Used so a certificate retained for one obligation can be re-verified when the same canonical key is seen again.

### Shape
`DerivationId {arena_index, generation}` (17), `DerivationStoreFull` (32), `DerivationStore<D>` (43): `derivations: Arena<Option<D>>`, `index: BTreeMap<ProofObligationKey, Vec<Handle<Option<D>>>>` (45), `capacity: Option<usize>`. Methods: `store` (82; refuses at capacity 87-92), `store_for` (98; returns `Ok(None)` when `proof_obligation_key` yields None, 104-106), `candidates` (110), `candidates_for` (126), `derivation` (136), `invalidate` (141), `invalidate_where` (152), `clear` (169). Tests 181-299.

### Inputs/outputs
Reads `proof_obligation_key(proof_plan, obligation)` (obligations.rs, not read). Mutates its own arena and index.

### How it decides
Lookups are keyed on `ProofObligationKey` (canonical identity; test `lookup_uses_semantic_identity_not_spelling` 200 says renamed machine still hits). Capacity refusal is explicit, no eviction (test comment 275).

### Cases by shape
None.

### Same job elsewhere
`ProofDerivationCache` (checker.rs:2876) wraps this store; `BTreeMap` used for the index (AGENTS.md says dense-ID maps in flat tables; this key is not a dense id).

### Unused / one-caller
All `DerivationStore` users are in `checker.rs` `derivation_cache` mod (grep `ProofDerivationCache|DerivationStore\b|DerivationConsultation|derivation_store::` over omega-rust *.rs, count mode: 8 in this file, 45 in checker.rs, 0 elsewhere). Grep `store_for|candidates_for|invalidate_where|\.key_count\(` count mode over omega-rust *.rs: derivation_store.rs 8, checker.rs 5, and `pipeline/05.../shared.rs` 2 which are the unrelated name `candidates_for_application` (shared.rs:4557, 4646). So `store_for` and `candidates_for` have no caller in checker.rs production code; I did not separately count `store_for` alone.
Comment at top (1-7) says it "serialize[s]" certificates; no serialization code is in this file (read).

### Stated limits
Header says "Persistent derivation store" (3); the store is in-memory only (read, no I/O).

### Fail-open
`candidates_for` returns an empty iterator when the key is None (131-133) - fail-closed (no reuse). Reuse is always re-verified by the caller (`DerivationConsultation::recheck`, checker.rs:2942). None seen.

---------------------------------------------------------------------
## 3. lemmas.rs (377 lines)

### Purpose
A small table of named bounds/window lemmas (`ProofLemma`, macro-generated 9-50, 5 entries 52-93) and a range-quantifier fact `ForAllInRangeFact` with literal/length bounds.

### Shape
`ProofLemma` enum + `foundation`, `ALL`, `name`, `premises`, `conclusion` (macro 9-50); `apply` (96) and `discharging` (103); `LemmaFact` (111, 7 variants); `ForAllInRangeFact {predicate: String, start, end}` (122) with `proves_element` (144), `contains_index` (151), `is_vacuous` (175); `ElementIndex` (189); `QuantifiedBound {Zero, Literal(i64), Length}` (196).

### How it decides
`proves_element` is keyed on predicate NAME string equality (145) plus bounds. `is_vacuous` (175-185) lists bound-pair shapes.

### Cases by shape
`contains_index` (151-173): `InBounds` index is proved only when range is exactly `(Zero, Length)` (154-157); literal indexes need `Literal` bounds; `Length` as end proves only `LiteralInBounds` (170).

### Unused
grep of `proof::lemmas|ProofLemma|ForAllInRangeFact|ElementIndex|QuantifiedBound|LemmaFact` over omega-rust: matches outside lemmas.rs are only different types in checked-trees (`ProofLemmaFact`, `ProofLemmaKind`, `QuantifiedBoundFact`, `LemmaFacts`) and a terminal-psi `ElementIndexInBounds` variant; no use of this file's types. The module is `pub mod lemmas` (lib.rs:36). So the lemma table has 0 non-test users (inferred from grep; the names could also be used via the library crate name `proof::lemmas::`, which was included in the pattern).

### Stated limits
Tests at 207-234 assert all lemmas are `ConstructiveDecidable`.

### Fail-open
`is_vacuous` true -> `proves_element` false (145), fail-closed. None seen.

---------------------------------------------------------------------
## 4. lib.rs (1158 lines)

### Purpose
Crate root. Declares modules `boundary`, `checker`, `derivation_store`, `lemmas`, `mathematical_declarations`, `obligations` (33-38) and defines two inline modules: `proof_surface` (39-579) and `typed_expressions` (580-848), plus test fixtures `test_program` (864-969) and tests (971-1158).

### proof_surface (39-579)
`build_proof_surface_report(&SyntaxTrees) -> ProofSurfaceReport` (109) walks root items and records `DomainSurface`, `PropositionSurface`, `ContractSurface`, `BoundedTypeSite` rows (strings for owner/base type/constraints, built with `format!` of names, e.g. 121, 133, 149). `collect_bounded_type_site` (431) recurses through Reference/Constrained/FixedArray/Slice/Generic. Reports contracts as Requires/Ensures/Crashes (411-418; `EnsuresForResultCase` folds into Ensures). Items ignored: Capability, Const, Use, Measure, Conformance, Module, Package (114-115, 190-194). Only root items are walked (112); nested machines are not (the `Machine` is collected through `collect_machine`, 250, which does not recurse into `nested_machines`; read).
Unused: grep `build_proof_surface_report|ProofSurfaceReport` over omega-rust: only lib.rs. So 0 callers outside lib.rs/tests.

### typed_expressions (580-848)
Crate-internal helpers: `expression_type_reference` (605), `dehoisted_operand` (667), `dehoisted_condition` (687), `dehoisted_initializer` (695, follows a local's initializer when the initializer has no call), `type_reference_for_symbol` (715; tries state param, machine type params, local, machine-owned data, then ANY data definition field with the symbol), `call_expression_return_type` (777), `callable_return_type_by_symbol` (786), `expression_contains_call_node` (817). `expression_type_reference` falls to `None` for expressions other than Borrow/Name/Member/Indexed/ZeroValue (643). `expression_contains_call_node` only descends Match/Binary/Unary/Cast/Borrow/Indexed/Member; other nodes (`_ => false`, 845) are treated as call-free.
Users: grep `typed_expressions::` in proof/src: checker.rs 7, obligations.rs 4, mathematical_declarations.rs 11.
Fail-open: `expression_contains_call_node` returning false for unlisted expression kinds feeds `dehoisted_initializer` (705-707) and `assignment_is_overwritten_before_consumption` (457, 481, 495): a node kind outside the list that contains a call would be treated as call-free. Which kinds exist beyond the list I did not enumerate -> "open".

### Same job elsewhere
Type-reference walkers: `proof_surface::type_reference_name` (471) / `collect_bounded_type_site` (431) on syntax trees vs `typed_expressions::element_type_reference` (647) on typed trees.

### Stated limits
Header comments (1-31) only.

---------------------------------------------------------------------
## 5. mathematical_declarations.rs (12715 lines), all lines read

### Purpose
Turns authored mathematical definitions, indexed families and source applications into `proof_admission` kernel terms (`TermArena`, `Declaration`, `Signature`) and records the checked result for later stages. The whole file is nested inline modules.

### Module map so far (inline `mod`, file lines)
declaration_records 10-281; fixed_core_vocabulary 282-284 (re-export of `validation::mathematical_applications::{FixedCore, FixedCoreVocabulary}`); mathematical_signature 285-... containing: applications 302-895, carriers 896-1385, content_values 1386-1447, dependencies 1448-1552, identity 1553-2331, level_types 2332-2708, normalization 2709-2805, operand_terms 2806-(continues past 3100).

### declaration_records (10-281)
`build_checked_mathematical_declarations` (25) maps each `program.mathematical_definitions()` entry to a `CheckedMathematicalDeclaration` (name, binders, parameters, result, body). Parameter and result types are stored as STRINGS: `mathematical_type_identity` (191) renders `normalized_type_identity_text`, arrow/application spellings with `format!`; definition bodies stored as `term_spelling: render_proof_expression(..)` (62). Binders: refused when they carry property bounds (82-91), machine/proposition/implementation kinds refused (100-119); a Value binder whose carrier resolves through `vocabulary.classify(symbol, name)` to `Level` or `Type` becomes a Level/Type binder (142-148). `ordinary_type_identity` (239) refuses Reference/Constrained/DynamicTrait carriers (256-265).

### mathematical_signature::applications (302-895)
`Elaborator::source_applications` (330): collects Call expressions whose `target_symbol` is in `authored_index` or `is_identity_call`, drops those already owned (`expression_nodes`, 350-354), and for each call walks operand expressions with a `pending` stack and a linear `seen: Vec` (406-413) to decide which operands become captures. `instantiate_declaration` (499) refuses a callee not in `authored_positions` ("references itself or a later declaration", 509-510). `static_level` (676), `static_term` (712), `static_integer_literal` (783: checks fixed-integer carrier, landing, range, 795-851).
Stated limits: 743-746 and 759-761 "has no kernel denotation yet"; 509-510 "the signature admits strictly earlier ones".

### carriers (896-1385)
`operand_carrier` (912) classifies an expression into `ScalarCarrier` {Integer, Boolean, Address, Float32, Float64, Unit, Carrier(pos)}: literals by landing, names via `name_carrier` (1048; by symbol in scope or declaration; if symbol invalid falls back to the single-segment name spelling compared to scope entries, 1059-1061), calls via `declaration_result_carrier` (1072), arithmetic binaries (934-953), any non-arithmetic binary -> Boolean (954), `member_carrier` (1005; by `member_symbol`, else field found by `field.name == member.member` spelling, 1033 and 1040). `scalar_carrier_of_type_reference` (1099): `_ => Some(ScalarCarrier::Integer)` for every other primitive (1109), including Selector.
Case: arithmetic binary with one known and one unknown operand carrier takes the known one (941: `(Some(carrier), None) | (None, Some(carrier)) => Some(carrier)`); two unknown with a closed integer value and no float leaf -> Integer (942-949). Status for that rule: "open".
Carrier declarations are pushed lazily as assumptions (`push_type_carrier`, 1330; bool is a definition over `Term::Two`, 1208-1217); integer relation/operation constants are kernel assumptions (1247-1310).

### content_values (1386-1447), dependencies (1448-1552)
`elaborate_content_carrier` (1401) builds a canonical-content carrier through `proof_admission::SourceContentTheory` when `validation::content_algebra_application` matches (None otherwise). `free_positions` (1451) walks every `Term` variant with an explicit list and `_ => {}` (1520); `rebind_domain` (1526).

### identity (1553-2331)
Builds the `core::Identity` relation as a universe-polymorphic `Term::Id` declaration (`identity_declaration`, 1638-1684) and transports identity requirements ("emissions" and "applications") through declaration applications (`transport_identity_value_requirements`, 1784-2046). `prepare_identity_values` (2185-2281) normalizes each emission and SKIPS rows whose normal form is not an `Id` application with exactly 3 `Apply` nodes (`continue` at 2211-2215) - such a requirement is silently dropped (status "open": not traced why a row would normalize to a non-Id).

### level_types (2332-2708)
`elaborate_level` (2345): Named reference -> level binder by symbol or (symbol invalid) by NAME (`level_index` 2690-2706, name compare at 2702-2705), else parses the spelling as `u64` (2365); ConstExpression integer literal; everything else refused ("computed universe levels are a later leg", 2389). `elaborate_type_reference` (2441): content carrier, source alias, region nominal carrier, then peel arithmetic policy; Generic bases `core::Type/Strict/Squash/Identity/Level` by `fixed_core.classify`; authored declaration as carrier refused ("a later leg", 2521); unique data definition -> `indexed_application`; FixedArray/Slice/ConstExpression refused ("has no kernel denotation yet", 2544-2549). `named_type` (2610).
Comment 2459-2462: "Other predicates remain unsupported here."

### normalization (2709-2805)
`normalize` (2715): weak-head normalize then recurse into each listed `Term` variant; `leaf => leaf` (2797).

### operand_terms (2806-...)
`scalar_operand_term` (2834): by expression kind; arithmetic binary on Integer carrier goes to `integer_operation_term` (3014), which (a) folds closed integer expressions without float leaves to a literal (3019-3024), (b) for non-Exact fixed-integer arithmetic domains returns an opaque `open_term` (3025-3036), (c) only Add/Subtract/(Multiply when `integer_multiply` is set) get kernel integer operations, and only if `has_builtin_spelled_expression_meaning` (3047), else `open_term`.

`operand_terms` runs 2806-3322 (not 2806-...); its `open_term` (3077-3294) builds an opaque operation: it finds the source type, collects the scope positions the expression depends on (walk over sub-expressions with a linear `seen` Vec, 3126-3167; names matched by symbol or, when the symbol is invalid, by spelling `entry.name == name` at 3152-3158), and either reuses an earlier `SourceOperationDeclaration` if owner, source type, bindings and `source_operations_have_same_meaning` agree (3232-3248) or pushes a new kernel ASSUMPTION constant (`push_assumption`, 3268) with a `PreparedConstantMeaning::SourceOperation` row (3276-3284). `embedding_term` (3296).

### propositions (3323-3714)
`elaborate_proposition_term` (3344): Boolean, `!`, identity call, and Name/Call/Unary/Member/Cast/Indexed go to `subject_proposition` (3398; only Boolean-carrier subjects become `Id Bool x true`, else `Ok(None)` at 3413); Binary: `==`, `!=`, `<,<=,>,>=`, `&&`, `||`; any other binary operator returns `Ok(None)` (3391); fallback `_ => subject_proposition` (3394). `None` means "not a proposition" and callers turn it into a refusal in some places (3628-3632, 11036-11041, 6300-6305) but `continue` past it in `retain_indexed_contract_theorems` (5213-5215).
`equality_proposition` (3441), `order_proposition` (3484): when the `==` / ordering operator does not have the builtin spelled meaning (user-overloaded) or the carrier is not Integer (order) the proposition becomes `Id Bool (open_term) true` over an opaque assumption (3469-3474, 3504-3517). Float comparisons use per-(operator, format, reference) assumption relations (3535-3608). `connective_proposition` (3610) sorts operands by `term_order_key` (3637) so `&&`/`||` operand order does not matter; `equality_proposition` swaps sides by the same key (3477).

### scope (3715-3922)
`named_term` (3724), `scope_term_for_symbol` (3769; also follows `source_index_aliases`), `scope_term_for_name` (3794; spelling match), `constant_for_declaration` (3817; refuses level-arity != 0 and "references itself or a later declaration", 3824-3840), `carrier_for_symbol` (3845): for SymbolKind BuiltinType or Data any symbol that is not a recognised integer/bool/address/float atom gets a FRESH opaque `Type 0` assumption carrier (`_ => self.push_type_carrier()`, 3897), and a Type-kind parameter a per-symbol assumption with level parameter 0 (3849-3874).

### source_domains (3923-4551)
`SourceAlias`, `SourceInput`, `SourceInvocation` (3938-3971). `apply_source_inputs` (4012) records every application to be checked later. `check_source_domains` (4305) is the deferred check that each argument/result domain "corresponds" to the callee Pi domain (messages "does not preserve its declared carrier and policy", 4343, 4389). `source_domains_correspond` (4426) has one branch that skips the kernel: if the expected domain is a source alias whose reference does not need exact qualification and the expression has anonymous numeric results, it returns `validation::checked_argument_matches_type_reference(..)` (4436-4446); otherwise `convertible_retaining` (4453). `source_references_correspond` (4484): primitives compared by carrier and arithmetic domain, others by `type_identity`. `scalar_operand_carrier` (4197): fixed list I8..U64, Addr, F32, F64 only (4222-4234).

### source_evidence (4552-5522)
`retain_source_evidence_carriers` (4642), `retain_checked_source_application` (4860; refuses ambiguous or missing carrier correspondence, 4913-4926), `retain_indexed_contract_theorems` (4942-5306): shape-limited - only a machine whose entry state has exactly one erased indexed-family parameter (4965), a unit result (4968), Type/Value-only binders (4986), at most as many telescope entries as the family has index parameters (5126), and each telescope entry mapping to exactly one index position (5184-5195); every miss is `continue` (skip, no theorem retained). `bind_source_evidence_types` (5310): every retained theorem, constructor schema and evidence row must have assumption closure inside `integer_theory.source_safety_roster()` ("outside the audited integer safety roster", 5325, 5349, 5393) - this is a fail-closed gate. `retain_source_evidence_aliases` (5402): fixed-point propagation of evidence rows to immutable erased locals, at most 128 passes (5418); unresolved names after the last pass are left silently (5515-5519).

### source_families (5523-10534) - inline submodules
* `parameter_closure` (5586-6169): `abstract_parameters` (5616), `excludes_parameters` (5818), `abstract_parameter_closure` (5866), constant-reference collection (5966). Fail-closed with `Err(&'static str)` on dummy terms and unbound variables.
* `construction` (6171-7068): `SourceConstructorContext` (6194); `retain_source_constructor_schema` (6202); `source_family_constructor_term` (6506): depth bound 128 (6512); a Name is followed to its erased immutable local initializer in the current machine (6533-6572, "duplicate declaration identity" refusal 6565), a StructLiteral is built into a `Sup` term through `check_type` at every step; any other expression: "has no constructor derivation yet" (6604-6606). Constructor case selection by `data_constructor_case` symbol identity (6653-6672), not by name. `source_constructor_operand` (7027).
* `induction` (7070-7729): `source_family_induction` (7222) reuses a prior checked induction when the declaration prefix is identical and the invariant is structurally equal (`CheckedSourceInduction::reusable`, 7171-7186; test at 7092); `instantiated_open_induction` (7607); every produced term passes `check_induction_term` -> `proof_admission::check_type` (7706-7727). `term_mentions_variable` (7189) returns true for any variant it does not list (`_ => true`, 7217).
* `terms` (7730-9077): "Untrusted source-family description production" (7731 comment): symbolic term builder `S` (7774), `SourceTerms` (8022), `Builder` (8144), `description` (8481), `intern_description` (8587), `apply_description` (8689), `InductionBuilder` (8714), `intern_invariant` (8882), `constant_invariant_theorem` (8900), `induction` (8918). Tests: `induction_tests` 9079-9841, `indexed_vector_budget_tests` 9846-10328.
* After the submodules, `needs_scheme` (10347), `erased_family_demands` (10433), `declaration_contains_family` (10531) decide whether the indexed-family scheme is needed. These three, plus `source_evidence::known_source_family` (4602) and `construction::known_source_constructor_family` (6453), each walk TypeReferenceNode variants with the same Reference/Constrained/FixedArray/Slice/Generic descent.
* `SourceFamily`/`SourceCase` (10653/10668); `source_family` (10806): refusals "source W descriptions currently require a Type/Value telescope; const/machine telescope denotation remains unsupported" (10836), "quotient, opaque or region-bearing source indexed carriers require their own checked description" (10842), "nested source-family recursion has no checked encoding yet" (10815), "nested or mutual source-family recursion has no checked encoding yet" (11269), "open dependent carrier whose encoding is unsupported" (11346), "no relevant proposition denotation yet" (11017, 11027). The family description is kernel-checked (`check_signature`, 10859) before being cached in `indexed_families` (10865). `build_source_family` (10869-11240) classifies Type parameters as uniform ("external") or index by checking that every recursive child passes the parameter through unchanged (10897-10930); one changing Type child sets all to non-uniform (10924).
* `index_operand` (11421): Value index forms accepted: a Named symbol (kernel-typed and `source_domains_correspond`), a closed-const Named atom decoded from its SPELLING with `BigInt::from_decimal_str(name.as_str())` or `CanonicalConstValue::from_atom` (11489-11499, integer carrier only 11505-11511), a ConstExpression (11515); else "not a bound value or closed scalar" (11529).

### source_levels (11535-11630), term_order (11631-11810)
Per-binder universe scopes (`enter_family_levels` 11558, `family_actual_levels` 11584: a Type argument in the strict layer is refused, 11611-11615). `term_order_key` (11636) serialises every `Term` variant to bytes for canonical ordering.

### Elaborator and entry points (11812-12715)
`Elaborator` struct (11863-11927) holds ~45 tables: `HashMap<SymbolHandle,u32>` and `BTreeMap<(u32,BigInt),u32>` for carriers, positions and literals, `Vec`s of aliases, invocations, emissions. `check_mathematical_signature` (11929): the indexed scheme and `SourceIntegerTheory` are only installed when `needs_scheme` (11933-12018). `elaborate_declaration` (12026), `plan_binder` (12113), `elaborate_body` (12178), `elaborate_expression_term` (12215): Name by symbol else single-segment spelling lookup (12241-12249); Call: identity call, non-authored callee with only the nominal route becomes `open_term` (12260-12279), receiver calls refused ("member call has no mathematical denotation yet", 12281), authored callee via `elaborate_callee`; Integer/Float/Boolean literals; Binary arithmetic on Integer carrier -> kernel operation else `open_term`; other Binary -> proposition else `open_term` (12326-12340); Member via scope else `open_term` (12342-12347); `_ => self.open_term(handle)` (12349). `check` (12386-12515): runs demand/applications/evidence/theorems, `check_signature_with_attribution` over the whole declaration list (12393), `check_source_domains` (12414), per-application `infer_sort` and `infer_type` (12447-12470), normalisation, then `retain_source_evidence_aliases`, `bind_source_evidence_types`, `retain_constant_meanings`. Public entry: `check_mathematical_declarations` (12686): returns `CheckedMathematics::default()` immediately when there are no mathematical definitions and no source evidence demand (12689-12693). `CheckedMathematics::prepared_view` (12704).

### Inputs and outputs
Reads `dyn validation::CheckingInput` (definitions, data/type tables, expression/statement tables, symbols), `validation::mathematical_applications::*` helpers and `proof_admission` term/kernel API (not read). Produces `CheckedMathematics {declarations: Vec<CheckedMathematicalDeclaration> (strings), signature: Option<checked_trees::CheckedMathematicalSignature> (arena + declarations + authored positions), applications: PreparedMathematicalApplications}` (12680).
Users (grep `check_mathematical_declarations|CheckedMathematics\b|prepared_view` over omega-rust *.rs): pipeline `04_typed-trees-to-typed-trees/src/preparation.rs:2045` (and type uses at 1916, 1927), `05_typed-trees-to-checked-trees/src/checking.rs:1016,1038`, and `omega/packages/review-evidence/src/capture/contracts/mathematical_calls.rs:12,31,36`.

### How it decides (keys)
* Symbols are the primary key (`authored_index`, `authored_positions`, `carriers`, `source_index_aliases`: HashMap<SymbolHandle,..>); source SPELLING is the fallback when a symbol is invalid: `level_index` (2702), `scope_term_for_name` (3794), `name_carrier` fallback (1059-1061), `member_carrier` field-by-name (1033, 1040), unresolved Name in `elaborate_expression_term` (12241-12249), const-index atoms (11489), universe literals parsed from spelling (2365), `open_term` dependency scan (3152-3158).
* Type correspondence is decided by `type_identity` / normalised identity (`source_references_correspond` 4484, `retain_checked_source_application` 4913), and then by the kernel (`convertible_retaining`).
* Declaration bodies and parameter types in `CheckedMathematicalDeclaration` are kept as rendered strings (54, 62).

### Cases by shape
* Indexed families: a Type/Value telescope only, no quotient/opaque/region carriers, no mutual or nested recursion (10836, 10842, 10815, 11269).
* `retain_indexed_contract_theorems`: one erased family evidence parameter, unit-result machine, Type/Value binders (4965-4993).
* `static_term` (712-781): only a universe application of fixed-core `Type`/`Strict`, a type reference, a symbol or a scope name; other payloads refused.
* Integer relation kernel support: only `<`, `<=` (and flipped `>`, `>=`) over the Integer carrier with builtin operator meaning (3504-3532); `+`, `-`, and `*` (only when `integer_multiply` is set) (3039-3063).
* `fixed_core.classify` members used: Level, Type, Strict, Squash, Identity (2482-2515).

### Same job elsewhere
* Three TypeReference-descending predicates in this file for "does this mention an indexed family": `needs_scheme::reference` (10348), `erased_family_demands::demand` (10434), `declaration_contains_family` (10531), plus `known_source_family` (4602) and `known_source_constructor_family` (6453).
* Constructing `Context::with_level_arity(self.level_binders.len() as u32 + self.generalized_levels).with_signature(Signature::from_declarations(self.declarations.clone()))` appears many times (e.g. 869-873 `instantiation_context`, 5252-5259, 6355-6358, 6784-6787, 6859-6862, 7303-7306, 7712-7715, 11448-11455); each call clones the whole declaration vector (read). `instantiation_context` (869) is the shared form; grep not run for the exact count.
* Carrier classification: `ScalarCarrier`/`operand_carrier` here (912, 12653). Grep `enum ScalarCarrier|fn scalar_carrier_of_type_reference|fn operand_carrier` over omega-rust *.rs: this file (912, 1099, 12653); `validation/src/arithmetic_domains.rs:22013` and `:22038` are `operand_carrier_bound` and `operand_carrier_within_ceiling` (different names, not read); `pipeline/05.../shared.rs:2492` is a private `operand_carrier(program, OperandType) -> Option<PrimitiveType>` (read 2488-2499 only; "suspected" third carrier classification).
* Level/lowering helpers `infer_sort`, `check_type` with `Budget::new(DEFAULT_CONVERSION_STEPS)` repeated ~30 times (read).

### Unused or one-caller code
* `check_mathematical_declarations`: 3 callers outside the file (listed above). `build_checked_mathematical_declarations` (25) is `pub(crate)`, one caller (12694). `has_source_evidence_demand` (12663): one caller (12690). `SourceFamily::at_levels`/`description_at` used only inside `source_families` (read). `terms::lift_payload` has non-test users in `construction` (6897).
* `checked_trees::CheckedMathematicalDeclaration.body.term_spelling` etc. consumers not traced.

### Stated limits (quotes under 20 words)
* 509: "references itself or a later declaration; the signature admits strictly earlier ones"
* 743-746 / 759: "has no kernel denotation yet"
* 2389: "computed universe levels are a later leg"
* 2521: "applying mathematical declaration ... as a carrier is a later leg"
* 2547: "has no kernel denotation yet" (FixedArray, Slice, ConstExpression)
* 10836: "const/machine telescope denotation remains unsupported"
* 10842: "require their own checked description"
* 6606: "has no constructor derivation yet"
* 6850: "has no checked derivation yet"
* 12282: "has no mathematical denotation yet"
* 12378: "has no kernel denotation yet" (any other expression)
* 5524-5539 comment: "This file does not turn legacy constructor-success verdicts into evidence".

### Fail-open risks
1. `indexed_application` (10775-10779): while a family is under construction (`indexed_family_stack` contains the symbol) an application of that family returns `Sort(Type 0)` rather than the family term. No comment. Which wrong program: a recursive occurrence elaborated during its own construction is typed as a universe. Status "open" (not traced; `source_family` itself refuses recursion at 10813 only when `indexed_families` lacks it).
2. `retain_indexed_contract_theorems` skips with `continue` on every shape miss (4953-5195, 5213): the machine's contract is simply not given a theorem. This drops a derivation, not accept a program; whether a missing theorem later lets a program pass depends on consumers (`validation`, not read). Status "open".
3. `prepare_identity_values` (2211-2215) drops emission rows whose normal form is not a 3-argument application of the identity declaration. Status "open".
4. `retain_source_evidence_aliases` (5418-5518): bounded to 128 iterations; names not resolved by then stay unrecorded without error. Status "open".
5. `source_domains_correspond` branch (4436-4446) accepts via `checked_argument_matches_type_reference` without kernel conversion when the expected domain is a source alias and the argument has anonymous numeric results. Status "traced" for control flow only; soundness is in the validation crate.
6. `operand_carrier` (941): in an arithmetic binary where one operand's carrier is unknown, the known one is taken. A mixed-carrier expression where one side cannot be classified is accepted as the other side's carrier; the later `scalar_operand_term` re-checks each operand against that carrier (2886-2897) and refuses mismatches. Status "traced" (read).
7. `elaborate_expression_term` `_ => open_term` and Member -> `open_term` (12347, 12349); `equality_proposition`/`order_proposition` opaque `Id Bool open true` (3469-3474, 3513-3516): unknown constructs become opaque kernel ASSUMPTIONS, not proofs. They cannot be inhabited by the kernel without a derivation; `bind_source_evidence_types` limits assumption closure to the audited roster (5319-5326) for retained theorems/evidence. Status "stated" (comment 3098-3100: "signature occurrence is not authority"), traced for theorems, not for other consumers.
8. `check_mathematical_declarations` returns an empty `CheckedMathematics` (12689-12693) when no mathematical definitions and no evidence demand exist. Intended; a program with none cannot produce a wrong accept here. Status "traced".
9. `scalar_carrier_of_type_reference` `_ => Integer` for any primitive not Bool/Addr/F32/F64 (1109), including `Selector`: a selector-typed operand is treated as an Integer carrier. Status "open".
10. `term_mentions_variable` `_ => true` (7217) and `free_positions` `_ => {}` (1520, which silently ignores variants it does not list: Dummy, Sort, Two, TwoZero, TwoOne, Empty, Constant, plus Variable below depth) - the latter is by design for leaf terms.
All term-building paths in the family code finish with `check_type`/`check_signature` against the kernel, so an incorrect generated term is refused rather than accepted (e.g. 6863, 6920, 7012, 7706).

---------------------------------------------------------------------
## Coverage
Read in full, in order, nothing skimmed:
- `omega-rust/psi/compilation/proof/src/checker.rs` 1-6550 (6550 lines)
- `omega-rust/psi/compilation/proof/src/derivation_store.rs` 1-299 (299)
- `omega-rust/psi/compilation/proof/src/lemmas.rs` 1-377 (377)
- `omega-rust/psi/compilation/proof/src/lib.rs` 1-1158 (1158)
- `omega-rust/psi/compilation/proof/src/mathematical_declarations.rs` 1-12715 (12715)
Total read: 21099 lines. Skimmed: 0. Not read: 0.
Large parts are `#[cfg(test)]` modules (checker.rs 96-126, 2013-2854, 4138-4579, 5775-5871; mathematical_declarations.rs 6048-6168, 7087-7160, 9079-10328); I read them at test depth and did not record their assertions in detail.

## Five things a maintainer most needs to know about this unit
1. `checker.rs` has two layers for integer bounds: a kernel-checked certificate path (`certificate` mod, verified by `proof_admission::verify_obligation`, checker.rs:899) and an older interval path that runs when the certificate builder returns `Uncovered` (e.g. checker.rs:373-399, 547-577, 689-715, 762-808). Only the first is re-decided by the kernel; the second is plain `BigInt` interval reasoning plus `validation::*` helpers.
2. The derivation cache (`ProofDerivationCache`, `DerivationStore`) is only reachable from tests: `check_proof_plan_inner` is called once with `None` (checker.rs:6472), and no code outside the proof crate names these types. Likewise `ProofDecisions.constant_recipe_obligations` is always empty (checker.rs:6482, 6545) and `check_proof_plan` and `lemmas.rs` have no outside callers.
3. Matching of guard/witness places in the integer path is by spelling: `guards::expressions_equivalent_for_proof` compares path-segment strings (checker.rs:4914-4925), `ensures_witness_bounds` is matched by `display_name` string (5200, 5215, 5329), `assignment_is_overwritten_before_consumption` by `display_name` (472). The float path uses symbols (3451-3466).
4. `mathematical_declarations.rs` is a single 12.7k-line file of nested inline modules that elaborates source constructs into `proof_admission` terms and checks the whole signature once (`check`, 12386). Anything it cannot denote becomes either a refusal (the "no kernel denotation yet" messages) or an opaque assumption constant (`open_term`, 3077).
5. Indexed-family support is shape-limited and gated: Type/Value telescopes only, no quotient/opaque/region carriers, no nested or mutual recursion (10836, 10842, 10815, 11269); theorems/evidence must stay inside the "audited integer safety roster" (5325).

## What I am unsure about
- Whether `validation::occurrence_integer_expression_fits_range`, `arrival_integer_expression_bounds`, `CallFrameResolver` and `checked_argument_matches_type_reference` (all in the validation crate, not read) are sound; the Uncovered and alias branches rely on them.
- Who produces `ensures_witness_bounds` and how `display_name` strings can collide (obligations.rs, not read).
- What downstream consumers do with a skipped theorem/evidence row (items 2-4 in fail-open for section 5).
- Whether `indexed_application` returning `Type 0` during recursion (10775) is ever observable; `source_family` refuses recursion at 10813 only on an in-progress entry that is not yet cached.
- `boundary` and `obligations` modules (declared at lib.rs:33,38) are outside this unit; claims about `ProofPlan`, `ProofObligationKey`, `BoundedAssignmentObligation` fields come only from their use sites here.
