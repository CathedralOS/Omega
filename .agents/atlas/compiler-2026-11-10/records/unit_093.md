# Unit 93 notes

Unit number: 93

Files covered (one range):
- `omega-rust/psi/compilation/validation/src/contract_entailment.rs` lines 1-18212 (18212 lines). The file is 91,057 lines long (`wc -l`); this unit is the first 18,212. The unit ends inside `law_conformance::operator_contracts` at the `struct ContractConjuncts` (line 18207-18212); the rest of that module is outside the unit.

Paths below are relative to `omega-rust/psi/compilation/validation/src/`, so `:N` means `contract_entailment.rs:N` unless another file is named. The file is one long hand-nested module tree (`proof_contracts::contract_entailment`, header comment at :1-4). There is no separate file per module; every module below is inline.

How this file is organised, in the order it appears (module, line range):

| Module | Lines |
|---|---|
| `arithmetic_judgment` (the integer/polynomial "Engine" and everything hanging off it) | 8-7563 |
| `call_requirements` | 7565-9212 |
| `citations` | 9213-10008 |
| `collection_bounds` | 10009-10566 |
| `completed_domains` | 10567-10857 |
| `completed_observations` | 10858-11218 |
| `completed_values` | 11219-12049 |
| `construction_predicates` | 12053-12521 |
| `zero_predicates` | 12522-12676 |
| `exit_predicates` | 12677-12848 |
| `statement_predicates` | 12849-13155 |
| `call_projection_identity` | 13158-13668 |
| `content_conservation` | 13669-14847 |
| `contract_refinement` | 14848-15717 |
| `declaration_projections` | 15718-15839 |
| `exit_coverage` | 15840-16011 |
| `fact_mentions` | 16012-16194 |
| `inductive_judgment` | 16195-16668 |
| `law_conformance` (partial) | 16669-18212 |

---------------------------------------------------------------------

## A. `arithmetic_judgment` (:8-7563) - the integer proof Engine

### 1. Purpose
Decides integer facts (`a < b`, `x == y + 1`, "this value fits in u8") by turning expressions into multivariate integer polynomials over named atoms, collecting lower bounds and difference constraints from hypotheses, and asking whether a goal polynomial is provably at least a bound. It also forms "declaration applications" (integer-valued measures, calls to declarations whose contract is applied to arguments) into polynomials and records which casts/calls/match arms were used so later stages can replay them.

### 2. Shape (all `file:line` are in `contract_entailment.rs`)
- `Polynomial { terms: BTreeMap<Monomial, BigInt> }` (:5914), `Monomial = BTreeMap<String,u32>` (:5911). Methods `add/sub/neg/checked_mul/as_atom_difference/as_single_atom/constant_value/excludes_integer_zero` (:5918-6075).
- `OpaqueTerm` (:6079-6124): one variant per operation the polynomial cannot express: `DeclarationApplication`, `Conditional{arms}`, `Call`, `IntegerPolicy`, `Remainder`, `Quotient`, `ShiftLeft`, `ShiftRight`, `Bitwise`. Each is stored in `Engine.opaque_terms: Vec<(String, OpaqueTerm)>` under a minted atom name.
- `Engine<'program>` (:6135-6163): about 26 fields. Important ones: `program: &dyn CheckingInput`, `source_owner`, `strict_symbol_bindings: Option<Vec<(SymbolHandle,Polynomial)>>`, `formal_arguments`, `strict_occurrences: Vec<(ExpressionHandle,Polynomial)>`, `parameter_atoms: Vec<String>`, `substitutions: BTreeMap<String,Polynomial>`, `bounds: Vec<(Polynomial,BigInt)>` (meaning polynomial >= bound), `nonzero_differences`, `arithmetic_intervals: BTreeMap<String,Interval>`, `opaque_terms`, `matrix: DifferenceBounds` (difference-constraint closure), `requires_unsatisfiable: bool`, `denotation_identities: Arc<AtomicU64>` and `conditional_work_remaining: Arc<AtomicUsize>` (both shared across clones, :6158-6160).
- Constructors: `new` (:6169, from a machine), `strict` (:4978), `strict_for_declaration` (:4968), `for_explicit_bindings` (:6490), `for_proof_integer_formation` (:6455), `with_result_atom` (:6494), private `for_owner` (:6207).
- Entry judgments: `Engine::judge(fact) -> Judgment` (:6779), `judge_comparison` (:6826), `integer_comparisons_unsatisfiable` (:6745), `install_hypotheses` (:6664), `add_requires` (:6512), `normalize(expression) -> Option<Polynomial>` (:6938).
- `Judgment { Proven, ConstantFalse, Refuted, Unknown{goal_in_language} }` (:5904).
- Sub-modules (line ranges): `application_formation` (:21-1000; with `calls` :24-253 and `dispatch` :254-587), `application_inputs` (:1001-1241), `denotations` (:1314-1501), `difference_matrix` (:1696-1917), `integer_operations` (:1919-2592), `integer_policy` (:2593-2764), `intervals` (:2765-3212), `lower_bound_queries` (:3213-3775), `conditional_values` (:3778-3918), `call_values` (:3920-4063), `captured_calls` (:4064-4471), `mathematical_terms` (:4472-4873), `strict_bindings` (:4874-5897). `#[cfg(test)]` modules at :1245-1313, :1502-1695, :1899-1916, :2451-2591.
- Formation API: `Engine::form_integer_expression` (:682), `form_declared_integer_application` (:719), private `formed_value` (:847), `formed_type` (:969), `IntegerApplicationFormation` (:601; validated constructor `from_retained` :630), `IntegerApplicationInput { Scalar | Record{type_reference, snapshot, reads} }` (:1006).
- Lower-bound proving: `LowerBoundQueries` (:3289) with `direct`/`prepared`/`from_prepared_index`, `prove_at_least` (:3378), `prove_with_deferred_preparation` (:3388), `prove_base_lower_bound` (:3435), `prove_local_lower_bound` (:3495), `affine_difference_lower_bound` (:3525, 3..128 atoms), `prove_proportional_lower_bound` (:3621), `prove_scaled_lower_bound` (:3662), `horner_interval` (:3703, power cap 128, depth cap 128). `PreparedLowerBoundIndex::new` (:3225) sorts bounds into a strongest-per-polynomial index.

### 3. Inputs and outputs
Reads `CheckingInput` tables (`expression_table`, `type_reference_table`, `symbols`, `proof_facts`, machine/state accessors), `crate::proof_contracts::arithmetic_domains` (intervals, `DifferenceBounds`, `integer_bit_width`, `range_constraint_interval`, `standing_integer_interval`, `call_result_interval`), `specification_calls` (`selected_call_contract`, `owner_*`), `proof_integer` (`anonymous_integer_value`, `proof_integer_expression`, `proof_nat_cast`), `proof_embeddings`. Mutates the `Engine` (bounds, substitutions, opaque terms, intervals, matrix). `form_*` output `FormedIntegerApplication { value, formation }` and push `CheckedDeclarationCall` / `CheckedDeclarationDispatchArm` / `ExactIntegerCastFact` records into `ApplicationDecisions` (:589-594). `form_integer_expression` clones the engine and only writes the clone back on success (:687, :715); `form_declared_integer_application` the same (:778, :842).

### 4. How it decides
- Proof of `goal >= bound`: structural, in this order (:3378-3401, :3435-3493): conditional-term probe (`conditional_lower_bound`, :3867); constant; atom-difference/single-atom in the difference matrix (:3495-3523); exact bound lookup (:3348); proportional/scaled stored bounds (:3446, :3662); interval lower bound (:3473); affine difference greedy (:3478); Horner interval (:3490). Keyed on polynomial structure, atom NAMES (strings) and stored `(Polynomial,BigInt)` pairs.
- Atom identity is a string. Minted names are `format!`-built from `{:?}` of values: `"\0integer-policy:{primitive:?}:{domain:?}:{value:?}"` (:2634), `"\0integer-remainder:{operand:?}%{modulus:?}"` (:2931), `"\0integer-quotient:..."` (:2961), `"\0integer-shift-..."` (:3013), `"\0integer-bitwise:..."` (:3073), `"\0conditional:{identity}"` (:3837, counter), `"\0declared-application:{n}"` (:4002), `"\0applied-call:{n}"` (:4050), `"\0call-result:{expression:?}"` (:7319), `"\0embed:{:?}:{identity}"` (:7457), `"\0address-model-bound:{model:?}:{bound:?}"` (:7433), `"\0proof-symbol:{:?}"` (:6476). Member places are keyed by `integer_source_identity` (:7504-7562), a `format!` of `{:?}` of symbols/literals/operators/cast fields plus recursive child identities; a Call only gets a call identity if `expression_values_have_call_identity` holds (:7539), else `occurrence:{expression:?}` (:7546).
- `Engine::normalize` NAME case without strict bindings (:7056-7064) looks the one-segment path text up in `parameter_atoms: Vec<String>` by string compare (`members[0].as_str()`, `parameter_atoms.contains(&name)`). `Engine::new` fills `parameter_atoms` from every state of the machine by parameter NAME, skipping a name already present (:6171-6179), and `collect_entry_range_hypotheses` uses the entry state's parameter names (:6599-6600). So in this mode the decision is keyed on source spelling, not on the resolved symbol.
- With strict bindings the NAME case is keyed on `SymbolHandle` (:7040-7050).
- Operation meaning: `selected_binary_meaning` (:5314-5375) asks `builtin_arithmetic_for_operand_types` for `+ - * / %`, else `has_builtin_spelled_expression_meaning` using `spelled_binary_operator` (`proof_contracts_core.rs:5965-5987`).
- Integer policy: `integer_operation_meaning` (:2010-2036) accepts shifts only if left policy is Exact/Trapping, bitwise always, `+ - * / %` only if both policies Exact/Trapping; `integer_operation_policy` (:2038-2061) combines policies; Wrapping/Saturating results become `IntegerPolicy` atoms (:2600-2646).
- `integer_value_lands` (:2137-2173) proves `min <= value <= max` for the primitive; for `Addr` it needs the address model bound atom (:2152-2161) and returns false when it is missing.
- `formed_type` (:969-999) accepts only types whose constraints are all `ArithmeticDomain` and whose base is a Named builtin integer atom.

### 5. Cases by shape
- `condition_paths` (:489-585): handles exactly `Boolean`, `Atomic`, unary `LogicalNot`, and `Binary` with selected meaning (`And`/`Or`/boolean `Equal`/`NotEqual`, or an operator with a `complement()`); anything else logs "condition-declined" and returns None (:576-583). Depth cap 128 (:495).
- `dispatch::form` (:322-487): match arms `Wildcard`, `Destructure`, `Value`; value patterns need builtin `==` meaning (:393-401); all arms must be covered or `remaining` non-empty returns None (:483-485).
- `formed_value` (:847-967): `Atomic`, unary `BitwiseNot`, `Cast` with `!is_recast() && semantic_domain.is_empty()` (:881-882), `Match`, `Call`, `Binary`; any other node is accepted only if it has no executable children (:954-965).
- `calls::form` (:39-212): requires `parameters.len()==arguments.len()`, acyclic requires, each `Requires` fact must be `ProofFact::Expression` with `binding.is_none()` (:151-157); records a decision only when the owner is `SelectedCallContract::Concrete` (:196-198), otherwise returns None.
- `normalize` (:6968-7343): `Member`, `Integer`, `Borrow`, `Name`, unary `BitwiseNot`, binary bitwise, three different `Binary` arms for `+ - *` (:7083, :7124, :7137, :7162), `Divide/Modulo` (:7179-7251: Divide on non-"builtin proof integer" returns None at :7243-7245; Modulo only by a constant at :7246-7250), `Call` (mathematical, exact embed, formal-argument call, call-result interval), `Cast` (two guarded forms), `Match` (:7341, `normalize_matched_integer` :7346 requires every arm to normalize to the same polynomial).
- `install_integer_operation_bounds` (:2377-2449) only for `Divide`, `ShiftRight`, `Modulo`, and only when dividend/divisor sign facts are proven.
- Mathematical terms (:4472-4873): comparison forms `IntegerRelation` and `Negation` (:4733-4743); term forms `Integer`, `AddressBound`, `IntegerOperation` of Add/Subtract/Multiply, `IntegerEmbedding` (:4766-4802); judged forms `Squash`, `Id` on `Two`, `Id` on an integer/address/boolean carrier, `Sigma` (:4627-4687).

### 6. Same job elsewhere
- Four separate "turn an expression into a Polynomial" paths in this unit: `formed_value` (:847), `Engine::normalize` (:6938), `normalize_scoped_value` (:5082), `normalize_mathematical_term` (:4747). `formed_value` and `normalize` each re-implement Cast/Binary/Call handling.
- Three "does this value fit the primitive range" checks: `formed_type` (:969), `integer_value_lands` (:2137), `integer_value_fit` (:4921, returns `CarrierFit`). `integer_value_fit` callers outside the unit: 2 (`contract_entailment.rs:37780`, `:53620`, grepped).
- Boolean-condition-to-comparison logic with the same "`x == true/false` wraps a condition" rule appears three times: `dispatch::condition_paths` (:489-585), `collect_condition_comparisons` (:5427-5486, wrapped-Boolean rule at :5452-5473), and `inductive_judgment::guard_arm_comparison` (:16516-16546).
- `Engine::conjuncts` (:6900) flattens `&&` (and `Borrow`); other conjunction flatteners: `contract_refinement::split_conjuncts` (:15650, also looks through `Atomic`), `equality_conjuncts::collect_equality_conjuncts` (:17692), `call_requirements::contains_case_premise` (:8976) and `case_fact_only` (:8988). Outside the unit, grepped `fn conjuncts` in `checks_contracts_exits.rs:5061` and a `conjunctive_proof_facts` helper (14 matches across 7 files for the family regex `fn conjuncts|conjunctive_proof_facts|fn collect_conjuncts|fn flatten_and`).
- `opaque_term_name_if_uncorrelated` (:6284) and `opaque_term_name_if_uncorrelated_with_queries` (:6300) are two near-parallel classifiers of the same `OpaqueTerm` variants; the second adds interval/query checks for Quotient/Remainder/ShiftRight. Callers: one each in the same file (grepped, lines :54253 and :54257, outside the unit).
- Depth-cap literals `depth >= 128` / `depth >= 64` appear 48 times across the whole `contract_entailment.rs` (grep count, whole file, not only this unit).

### 7. Unused or one-caller code (grepped across `omega-rust/psi`)
- `form_integer_expression` (:682): 1 outside call (`contract_entailment.rs:37191`).
- `form_declared_integer_application` (:719): 4 outside calls (:32142, :37164, :52700, :52925).
- `integer_value_fit` (:4921): 2 outside calls (:37780, :53620).
- `strict_for_declaration` (:4968): 8 call sites incl. `for_proof_integer_formation` itself (:6480) and :36686, :37230, :37546, :37652, :37758, :52678, :52779.
- `bind_matched_integer_subject` (:5011): 1 (:32381). `bind_captured_integer_place` (:5719): 1 (:32436). `closed_symbol_bindings` (:5060): 1 (:28719). `bind_strict_integer_division/shift/bitwise` (:5662/:5679/:5699): 1 each (:29277/:29280/:29289). `bind_strict_arguments` (:5820): 1 (:30061). `rebind_saved_expression` (:5854): 1 (:30109). `immutable_local_initializer` (:6184): 1 (:34004). `exact_argument_values` (:5777): 2 (:25831, :43698). `bind_goal_formals` (:5802): 2 (:25927, :43710). `has_opaque_terms` (:6252): 1 (:54041). `truncate_opaque_terms_since` (:6256), `restore_matrix_from` (:6275), `restore_intervals_from` (:6280): 1 each (:53111, :53109, :53116).
- `closed_symbol_bindings`, `strict_symbol_bindings_are_valid` are used (4 and 5 call sites). No function in this module came out at zero call sites.
- `Engine::strict` (:4978) and `Engine::new` (:6169) callers were not individually counted.

### 8. Stated limits
- :1581 (test) "Preparation changes query work, never the visible arithmetic premises."
- :3385-3387 "Short batches need no full premise index when existing conditions and bounds answer every question."
- :3405-3406 "Conditional dependencies own a shared work allowance and may return a definitive false."
- :3496-3497 "Failure stays unresolved: other premises may still prove the goal."
- :1625-1626 (test) cloning an engine "intentionally shares the original counter".
- :1682-1683 (test) "Exhausted conditional work is definitive false".
- Work budgets as literals: `conditional_work_remaining` starts at `1 << 20` (:6237), charge = `1 + bounds.len() + opaque_terms.len()` (:3856-3858); 4096 budget in `complete_formed_call` (:4080) and `capture_expression_calls` (:4275); `SUBSTITUTION_ROUNDS = 8` (:6166); capture depth 64 (:4304).

### 9. Fail-open risks
- `selected_binary_meaning` `return true` at :5365 when `spelled_binary_operator` has no spelling. Which operators: `CaseMembership, And, Or, BitwiseAnd, BitwiseOr, BitwiseXor, ShiftLeft, ShiftRight` (`proof_contracts_core.rs:5978-5985`, read). A bitwise/shift/`&&` expression is therefore treated as built-in meaning without an overload check at this site. Traced partly: `normalize` for bitwise goes to `selected_bitwise_value` (:7066-7082, :1958) which requires a primitive integer result type and bit width; for shifts to `selected_exact_shift_value` (:1978) which requires Exact domains. For `And`/`Or`/`CaseMembership` there is no further check in this function. Classification: open for `And/Or/CaseMembership`, traced for bitwise/shift.
- `normalize` third `Binary` arm (:7083-7123): when `selected_binary_meaning` is false but `ranking_range::unselected_or_builtin_arithmetic` (outside unit) holds, `+ - *` is still folded into polynomial arithmetic, and a Wrapping policy is chosen from `integer_embedding` of an operand (:7107-7122). Open (the predicate is outside the unit).
- `normalize` `Cast` arms (:7331-7340): `proof_nat_cast` casts and `value_preserving_integer_cast` are normalized as the inner value. `value_preserving_integer_cast` (:7393-7408) requires a literal or an embedding with a fixed-integer primitive and a fixed-integer target but does NOT check the target is at least as wide; landing is checked later by consumers, not here. Open.
- `selected_policy_result` (:7369-7391): `ArithmeticDomain::Exact => Some(value)` returns the unbounded exact sum/product with no landing check (:7380); landing is proved in `admit_selected_integer_operation` (:2328) but `normalize` does not call that. Inferred: callers that need fit call `integer_value_lands`.
- `conditional_lower_bound` (:3905-3915): an arm whose conditions are unsatisfiable counts as proving the goal (`return true`, :3911-3913), and `all` over zero arms is true. Zero arms is unreachable: `conditional_value` returns None for empty arms (:3820-3822) and is the only constructor of `Conditional` terms (traced). Vacuous-arm acceptance depends on `integer_comparisons_unsatisfiable` being sound (traced as design, not proved).
- `substituted` (:1853-1896): on multiplication overflow it returns the not-fully-substituted polynomial (:1887-1889). That loses equalities (weaker), not fail-open (traced).
- `from_retained` (:630-673) accepts empty cast/call/dispatch vectors. Intended (empty formation). Stated by absence; not a risk.

---------------------------------------------------------------------

## B. `call_requirements` (:7565-9212)

### 1. Purpose
Decides whether a called declaration's `requires` fact is entailed at a call site by walking the caller's prefix (every way execution reaches the call) and judging the instantiated requirement under each arrival's structural judge. Also holds `inherited_predicate`, which maps an inherited trait requirement onto the realizing machine's parameters.

### 2. Shape
- `structural_call_requirement_entailed` (:7978, pub), `selected_value_call_requirement_entailed` (:8011, pub), `prepare_selected_value` (:8037), `prepare_requirement` (:8239), `run_requirement_walk` (:8378), `PreparedRequirement { Decided(bool) | Walk(Box<RequirementWalk>) }` (:8187), `RequirementWalk` (:8192), `CallRequirement` (:8481) implementing `StructuralPrefixVisitor` (:8504-8798).
- `inherited_predicates::inherited_predicate` (:7611-7973) returns `Result<InheritedPredicate, &'static str>` with about 25 distinct error strings; `InheritedPredicate { owner, expression, parameters, values, selections }` (:7601).
- `runtime_predicate_fact` (:8800), `runtime_fact_for_parameters` (:8861), `contains_case_premise` (:8976), `case_fact_only` (:8988), `establish_citation` (:9001), `establish_citation_from_prefix` (:9041), `citation_guarantees` (:9082), `intake_state_induction` (:9142).

### 3. Inputs and outputs
Reads machine/state/statement tables, `specification_calls` (`selected_call_site_contract`, `call_parameter_scope`, `AppliedProofBindings::for_call_site`), `structural_prefix` observers, `StructuralJudge`. Outputs a `bool` verdict; mutates only the visitor (`seen`, `proven`).

### 4. How it decides
- `prepare_selected_value` returns `Decided(false)` for a `TransitionNamed` site (:8050), when no contract is selected (:8052-8062), when the goal expression is not among the owner's declared `Requires` facts or its `&&` conjuncts (:8063-8092), and when no parameter scope exists (:8093-8103).
- `Decided(true)` when `program.static_type_requirement_entailed(...)` holds for the selected application's type bindings (:8112-8127; function outside the unit).
- The walk (`CallRequirement::observe`, :8552-8797) sets `proven &=` the `Proven` verdict from `instantiated_selected_fact_judgment` for each entry (:8781-8795). A type-requirement shortcut `continue`s an entry as accepted (:8774-8780).
- `runtime_values` is true unless both machine and callee are proof machines (:8272-8273); `acyclic` from `named_state_transition_subgraph_is_acyclic` (:8274); proof-only machines that are cyclic give `Decided(false)` (:8277-8279).
- Parameter matching in `observe` is positional (:8715-8719) and keyed on `SymbolHandle` for the "does the goal mention this parameter" check (`path.symbol == predicate_parameter.symbol || path.head_symbol == ...`, :8754-8761).

### 5. Cases by shape
- `inherited_predicate`: only `TypeParameterKind::Type` pairs, and `Const`/`Value` pairs; any other pair returns the error "needs a complete non-scalar substitution" (:7881-7885). Scalars resolve only to `Integer`, `Boolean`, or a forwarded bound value of the exact carrier (:7909-7941); otherwise Err (:7936-7940).
- `runtime_predicate_fact` (:8806-8858): `Boolean`, case observation, `And/Or` (recursive), `Equal/NotEqual` (identity meaning), everything else via `runtime_fact_for_parameters`.
- `runtime_fact_for_parameters` (:8861-8974): static type predicates, `And/Or`, case observations, identity `Equal/NotEqual`, any expression with a structural term that is a "checked predicate", a Bool-typed call with a closed value contract, `ScalarBinary`, `Constructor`, sealed float comparison operators, or a specification application candidate.

### 6. Same job elsewhere
- Three copies of "if the state graph is acyclic observe the invocation prefix, else the runtime state prefix, then try private arrivals": `run_requirement_walk` (:8378-8478), `collection_bounds::prove_one` (:10405-10445), `statement_predicates::StatementPredicateObservations::proves` (:13047-13119); each separately calls `named_state_transition_subgraph_is_acyclic` (:8274, :10396, :13095; other calls: :23225, :85471, and 12 in `calls.rs`, grepped).
- Nine `StructuralPrefixVisitor` implementations inside this unit, each carrying its own `seen`/`proven`/`unresolved` bookkeeping: `CallRequirement` (:8504), `Bounds` (:10456), `CompletedObservation` (:11158, generic), `CompletedProjectionVisitor` (:11681), `CompletedCaseVisitor` (:11989), `ConstructionPredicates` (:12424), `ExitPredicate` (:12746), `Predicate` (:13129), `ProjectionIdentity` (:13369). Grep of `impl ... StructuralPrefixVisitor for` over `omega-rust/psi` finds 18 single-line impls in this file (more impls in lines beyond this unit: :20396, :21462, :22878, :23276, :28208, :30519, :78702, :78835, :83799, :85875, :86100).

### 7. Unused or one-caller code (grepped)
- `structural_call_requirement_entailed` (:7978): 1 outside use (`checks_contracts.rs:9243`) plus the `lib.rs:1918` re-export.
- `selected_value_call_requirement_entailed` (:8011): 2 outside uses (`checks_contracts.rs:9220`, `checks_operators.rs:1580`) plus `lib.rs:1917`.
- `establish_citation` (:9001), `establish_citation_from_prefix` (:9041), `can_establish_citation` (:9385), `citation_argument_types` (:9466), `is_citation_statement` (:9479): all have callers further down this same file (:44238, :45007, :44981, :44990, :45186 etc.), not read.

### 8. Stated limits
- :8277 area is code, not comment. The `ExitPredicate`-style comment is not here. No "unsupported" comments in this module; limits are the `Err(&'static str)` messages at :7622-7973, e.g. "the inherited clause has no concrete selected invocation" (:7624).

### 9. Fail-open risks
- :8747-8766 `let Some(value) = value else { ... continue; }`: when a call argument has no observed value, the parameter is skipped and the requirement is still judged without it. Guard: if the goal expression mentions the predicate parameter (by symbol or head symbol) the walk sets `proven = false` and returns (:8754-8763). Classification: traced. The guard uses `collect_expression_tree` over the goal, so a goal that reaches the parameter only through something that traversal does not follow would be missed (not checked).
- :8774-8780 and :8120-8126: `static_type_requirement_entailed` accepts without a value judgment. Open (function outside the unit).
- `instantiate_citation` with `judge == None` and no requires (:9880-9888 gate) returns the callee's ensures as predicates with no premises checked. Reachable from `collect_citation_predicates` with `judge: None`; stated by the comment at :9510-9523 that callee ensures is "machine-checked in this same validation batch". Classification: stated.

---------------------------------------------------------------------

## C. `citations` (:9213-10008)

### 1. Purpose
Proof citations: a statement call `lemma(args);` inside a proof body (or a runtime call with a case-premise ensures) delivers the callee's proven `ensures` instantiated at the argument terms into the caller's structural judge.

### 2. Shape
`CitationTarget` (:9238), `CitationFacts { predicates, cases }` (:9260), `CitationFrontier` (:9266), `can_establish_citation` (:9385), `citation_needs_site_judge` (:9436), `citation_argument_types` (:9466), `is_citation_statement` (:9479), `collect_citation_predicates` (:9525), `suggest_missing_citation` (:9625), `citation_call_in_statement` (:9762), `machine_requires_facts` (:9803), `instantiate_citation` (:9815), `instantiated_fact_established` (:9962).

### 3. Inputs and outputs
Reads selected contracts (`selected_contract_by_symbol`, `owner_contracts`, `owner_parameters`), the proof-only classification, `StructuralJudge`. Output: `CitationFacts` or a `Vec<StructuralTerm>`; pushes `Diagnostic::error` when a requires fact cannot be established (:9880-9902).

### 4. How it decides
- `can_establish_citation` (:9385-9429): needs a selected contract that is a `Signature` or the first state of a `Concrete` machine (:9350-9367); disallows self-citation at runtime (:9403) and non-proof owners off runtime (:9404-9407); argument count equality; at least one `Ensures` fact (at runtime only one that `contains_case_premise`) (:9413-9428).
- `instantiate_citation` (:9815): maps parameter NAMES to argument terms (:9876-9879); requires facts must be `ProofFact::Expression`, otherwise "requires_out_of_language" error (:9855-9857, :9880-9888); each requires fact must be established by `instantiated_fact_established` (:9890-9902).
- `suggest_missing_citation` (:9625-9695) scans every machine in the program (`program.machines()`, :9644) that is a proof machine without `requires`, and matches the goal equation by `diagnostic_shape_match` (:9739). It only affects an error message.

### 5. Cases by shape
`suggest_missing_citation` only handles `And` (recursive) and `Equal` goals (:9632-9641). `instantiate_citation` skips `EnsuresForResultCase` and `Crashes` contracts (:9868-9869). `instantiated_fact_established` handles case observations, `And`, and structural predicates, nothing else (:9970-10006).

### 6. Same job elsewhere
`instantiate_citation`'s requires-checking loop and `call_requirements::CallRequirement::observe` both instantiate a callee requirement under substitutions and ask `instantiated_*_judgment` (`instantiated_fact_established` :9962 vs `instantiated_selected_fact_judgment` used at :8782).

### 7. Unused or one-caller code
`suggest_missing_citation` (:9625): 1 outside call (`contract_entailment.rs:90825`, read lines 90820-90827); also imported at :90294. `collect_citation_predicates` (:9525): used at :90410 (not read).

### 8. Stated limits
:9510-9523: "Soundness: the callee's ensures is machine-checked in this same validation batch (a false lemma raises its own error...)". :9880-9886 error text: "whose requires contract cannot be established by this citation judgment".

### 9. Fail-open risks
None seen beyond the stated one above (`judge == None`, no requires). The "callee ensures is machine-checked" claim is not verified at the site: classification stated.

---------------------------------------------------------------------

## D. `collection_bounds` (:10009-10566)

### 1. Purpose
Proves that an index or range expression `a[i]` / `a[s..e]` is within bounds, batching many such demands per machine into one prefix walk.

### 2. Shape
`geometry::proves_observed_bounds` (:10018) builds the required comparisons: for a range `start>=0, end>=0, start<=end, end<=length`; for an index `index>=0, index<length` (:10077-10106). `CollectionBoundsObservations { program, machine, states }` (:10162, `new` :10208, `proves` :10221, `resolve_deferred` :10250), `CollectionBoundsDemands { acyclic, demands: BTreeMap<BoundsCoordinate,BoundsDemand>, resolved }` (:10175), `Bounds` visitor (:10449-10565).

### 3. Inputs and outputs
Two-pass protocol (comment :10154-10161): a provisional pass records demands unproven, `resolve_deferred` proves them all, the second pass reads answers. Reads `StructuralJudge::normal_operation_entries`, array extent (`resolved_array_extent_at`), collection length measure. Output: `bool` per demand.

### 4. How it decides
Keyed on `(state arena index+generation, statement_index, expression handle position)` (`BoundsCoordinate` :10152, `BoundsDemand::coordinate` :10189). `proves` returns the stored verdict if present, else `table.resolved && prove_one(...)` (:10235-10247), so before resolution an unseen demand is recorded unproven. `Bounds::observe` requires `place_has_builtin_coordinates` (:10479), a typed collection, array extent for fixed arrays, non-empty operation entries (:10532), and every operand to be consumed (`position == entry.arguments.len()`, :10562).

### 5. Cases by shape
Fixed-size arrays (`fixed_array_type_extent`) use the resolved extent (Closed or Bound, :10030-10049); everything else uses `observed_collection_measure(.., CollectionMeasure::Length, ..)` (:10051) and a `u64` carrier (:10059-10066). Range with optional ends and inclusive flag (:10077-10097).

### 6. Same job elsewhere
`prove_one` (:10405) duplicates the walk structure of `run_requirement_walk` (:8378) and `StatementPredicateObservations::proves` (:13047) (see B.6).

### 7. Unused or one-caller code
`CollectionBoundsObservations` : users `checks_ranges_facts.rs:4974`, `checks_ranges.rs:11075`. `CollectionBoundsDemands`: `checks_ranges.rs` (4 sites) plus `lib.rs:1920` re-export.

### 8. Stated limits
:10154-10161 block comment describing the two-pass protocol ("The observations never change; the caller owns the demand table").

### 9. Fail-open risks
None seen. `Bounds::observe` failure paths set `proven = false`. `resolve_deferred` returns `proven = any(demand.proven)` (:10352) which is a "did anything get proven" flag, not a verdict, and each demand keeps its own `proven`.

---------------------------------------------------------------------

## E. `completed_domains`, `completed_observations`, `completed_values` (:10567-12049)

### 1. Purpose
"Completed value" proofs: judge facts about the value an expression produces after every way of reaching it has been observed (domain predicates, `< static bound`, integer-interval fit, case selection, projections).

### 2. Shape
- `completed_domains`: `completed_value_refutes_domain` (:10577), `completed_value_proves_domain_predicate` (:10598), `completed_value_proves_bound_domain_predicate` (:10621), `CompletedDomainJudgment { observed, proven, refuted }` (:10651), `completed_domain_member` (:10719).
- `completed_observations`: `completed_value_is_bounded_by_static` (:10870), `completed_value_fits_integer_interval` (:10945), `fold_completed_values` (:11026), `observe_completed_values_batched` (:11075), `CompletedObservation<FoldState,Fold,Succeeds>` (:11088-11217).
- `completed_values`: `CompletedValueSource` (:11236), `CompletedSources` (:11250), `observed_projections` (:11472), `term_field_paths` (:11375), `owned_term_field_paths_in_scope` (:11392), `completed_value_observations` (:11641), `CompletedProjectionVisitor` (:11670), `populate_machine_completed_cases` (:11831), `completed_value_case_at_occurrence` (:11911), `CompletedCaseVisitor` (:11980).

### 3. Inputs and outputs
Walk `observe_runtime_state_prefix`, then (if not proven) `observe_private_arrivals` (:11047-11071). `CompletedValueFold { state, complete }` (:11020). `completed_value_case_at_occurrence` reads/writes memo tables on the frozen program (`memos.completed_case_outcomes`, `completed_case_populated`, :11886-11895, :11926-11960; 6 mentions across 2 files, grepped).

### 4. How it decides
`fold_completed_values` returns the universal (runtime-prefix) fold if it is complete and succeeds, otherwise retries with private arrivals (:11053-11072). A fold with `Option::None` observation means "value unavailable": sets `complete = false` (:11150-11154). Domain predicates: `has_bound_symbol_only_proof(domain)` and `domain.alias.is_none()` required (:10770-10772). Case selection: exactly one variant proven (:12033-12040).

### 5. Cases by shape
`completed_value_is_bounded_by_static`: only a `Const`-kind machine type parameter whose primitive equals the value's fixed-integer primitive (:10879-10905). `completed_value_fits_integer_interval`: fixed integers only (:10958-10962). `observed_projections` builds children only for `Constructor` terms whose identity matches the data declaration (:11481-11528).

### 6. Same job elsewhere
`completed_value_fits_integer_interval` (:10945) and `completed_value_is_bounded_by_static` (:10870) each build a `ScalarBinary` predicate with `[(primitive, Exact); 2]` meaning inline (:10920-10930, :10989-10994); `completed_value_proves_backing_addresses`/`completed_value_preserves_entry_projection` (:14504, :14576) build `MathematicalBinary Equal` over `IntegerEmbedding` the same way.

### 7. Unused or one-caller code (grepped outside this file)
`completed_value_refutes_domain`: 1 call (`checks_contracts_exits.rs:4884`). `completed_value_proves_domain_predicate`: 5 calls (`checks_contracts_exits.rs:2678,3004,3022,3036`, `checks_content.rs:966`). `completed_value_proves_bound_domain_predicate`: 2 (`checks_contracts_writes.rs:1343`, `checks_contracts_exits.rs:4306`). `completed_value_is_bounded_by_static`: 2 (`checks_ranges.rs:10144,10187`). `completed_value_fits_integer_interval`: 3 (`recasts.rs:1383`, `program_validation.rs:2254`, `arithmetic_domains.rs:22240`). `completed_value_case_at_occurrence`: 2 (`domains_and_predicates.rs:7474,13587`). `completed_value_case_definition`: 2 (`domains_and_predicates.rs:10236,10249`). `expression_statement_index`: 1 (`arithmetic_domains.rs:22232`). `completed_value_observations` (:11641): 4 call sites inside this file (:24655, :24658, :27660, :27684), none outside. `observe_completed_values_batched` (:11075): 1 call, :14451 (this unit).

### 8. Stated limits
None found as comments. :11999-12000 is code.

### 9. Fail-open risks
- `CompletedDomainJudgment::initial()` starts `proven: true` (:10782-10787) and the fold only lowers it; the result is set `proven &= complete` and `observed = complete` (:10713-10716). With zero observations `complete` stays false (`complete && observed`, :11051). Traced: an expression never observed cannot be proven.
- `fold_completed_values`' `shared_success` demands `recognized && complete && observed && succeeds` (:11145-11148).
- `completed_value_proves_backing_addresses` fold closure (:14629-14632) returns `Break` on a missing observation without setting `proven = false`; the caller filters on `result.complete` (:14681-14683) so the answer becomes `Err`. Traced.

---------------------------------------------------------------------

## F. `construction_predicates`, `zero_predicates`, `exit_predicates`, `statement_predicates` (:12053-13155)

### 1. Purpose
Judge predicates attached to struct construction (`where`-style data predicates), the all-zero value of data/domain types, normal-exit `ensures` at a return, entry `requires` classification and crash conditions, and ordinary Boolean statements.

### 2. Shape
`construction_predicate_judgments` (:12067), `ConstructionPredicates` visitor (:12261, `judge_operands` :12304); `data_predicates_admit_zero` (:12532, pub), `domain_predicates_admit_zero` (:12618); `normal_exit_proves` (:12698, pub), `ExitPredicate` (:12736), `NormalExitContract` (:12690); `EntryPredicateObservations` (:12862: `has_formed_identity_predicate` :12908, `proves_crash_condition` :12996), `StatementPredicateObservations` (:13026, `proves` :13047), `Predicate` (:13122).

### 3. Inputs and outputs
`StructuralJudgment` verdicts (`Proven`, `Refuted`, `Unknown`); `instantiated_selected_fact_judgment`; prefix observers.

### 4. How it decides
- `construction_predicate_judgments` (:12067-12186): if `prepared_mathematics().source_constructor(expression)` matches (same constructor/machine/state, valid schema/type, identity equals normalized source type, and every predicate is in `constructor.predicate_expressions`) it returns `Proven` for all predicates without judging anything (:12080-12097). Otherwise a closed-literal shortcut, runtime-prefix observation, private arrivals, then acyclic invocation prefix (:12137-12185). Verdicts merge by `retain`: Refuted > Unknown > Proven (:12411-12421).
- `data_predicates_admit_zero` (:12532-12616): false when `definition.type_parameters` non-empty or `where_facts` empty (:12536); each fact is judged with zero values for non-erased fields (:12547-12563); falls back to `collection_predicates::data_fact_holds_at_zero` (:12610, outside) if not proven.
- `normal_exit_proves` (:12698): `returned` valid, `parameters.len()==supplying_parameters.len()`; per parameter the entry state parameter must equal the supplied parameter and have identical const/mutability/relevance/type identity (:12776-12792); a parameter the contract expression does not use is skipped (:12793-12799); used ones must be non-mutable, non-const, non-reference, same type as the current parameter (:12805-12813); every normal return path must judge `Proven` (:12829-12844).
- `has_formed_identity_predicate` (:12908-12994): expression must literally be one of the machine's own `Requires`/`Crashes` facts (:12920-12935).

### 5. Cases by shape
`is_closed_literal` (:12503-12519): `Integer, Float, UndeterminedNaN, Boolean, String`, struct literals and array literals of those. `has_formed_identity_predicate` walks only `And/Or/Not` with a 4096-node budget, depth 64 (:12938-12952), leaves must be a scalar leaf or an `Identity` structural term with a valid carrier.

### 6. Same job elsewhere
`ExitPredicate`, `Predicate`, `ConstructionPredicates` and `Bounds` all re-implement the same `seen`/`proven`/`unresolved` visitor protocol (see B.6).

### 7. Unused or one-caller code (grepped)
`data_predicates_admit_zero` (:12532): 5 outside sites (`value_custody_core.rs:4126,4147`, `domains_and_predicates.rs:12494`, `preparation.rs:344` in `04_typed-trees-to-typed-trees`, plus `lib.rs:1805` re-export). `domain_predicates_admit_zero`: 1 (`value_custody_core.rs:4108`). `normal_exit_proves`: 2 (`checks_contracts_exits.rs:141,161`). `construction_predicate_judgments`: 3 (`literals_and_struct_literals.rs:3595,5080,5123`). `EntryPredicateObservations`: 2 (`checks.rs:9650,9744`). `StatementPredicateObservations`: 1 (`checks_contracts.rs:252`).

### 8. Stated limits
:12859-12861 "A crash condition is a question here; it never becomes one of the entry hypotheses used to answer itself." :12906-12907 "This classifies a premise; it neither proves it nor installs a goal."

### 9. Fail-open risks
- :12080-12097 prepared-mathematics shortcut returns `Proven` for all predicates. What input reaches it: a struct literal whose `source_constructor` record matches and lists the predicates. Classification: open (record built outside the unit).
- :12610 `data_fact_holds_at_zero` accepts a data fact the judge did not prove. Open (outside the unit).

---------------------------------------------------------------------

## G. `call_projection_identity` (:13158-13668)

### 1. Purpose
For a call site, observes what the actual arguments were at completion and produces: a stable identity string for an owned subject path, a scalar (`ScopedArithmeticValue`) for an index argument, or a yes/no for "argument index matches the declared index".

### 2. Shape
`owned_contract_subject` (:13171), `captured_call_subject_identity` (:13200), `captured_call_index_value` (:13222), `captured_call_argument_index_matches` (:13244), `ProjectionRequest { IndexEquality | Identity | Value }` (:13274), `capture_projection` (:13299), `ProjectionIdentity` visitor (:13357), `bound_index` (:13603), `completed_index_value` (:13634).

### 4. How it decides
Identity strings are `format!("state:{:?}:completed-contract:{key}", self.state)` (:13517) and `format!("state:{state:?}:completed-index:{}", ...)` (:13660): keyed on a debug-printed `SymbolHandle` plus `observation_identity_key()`. All entries of all paths must agree (`previous != &value` sets `valid = false`, :13592-13597). `capture_projection` returns Some only if `seen && valid` (:13352).

### 5. Cases by shape
`owned_contract_subject` (:13171-13198): only a chain of `Member` with valid symbol and no `case_variant` ending in a parameter name that is not a reference. `completed_index_value` (:13634-13667): `Integer`, `MathematicalBinary` of Add/Subtract/Multiply, otherwise an atom (needs a known `unsigned`). `CallSite::TransitionNamed` is handled only for a transition whose `target` is the named path and whose `continuation` is not (:13318-13337).

### 7. One-caller code (grepped)
`owned_contract_subject`: `checks_contracts.rs:1426`, `published.rs:4871`. `captured_call_subject_identity`: `checks_contracts.rs:1429`, `published.rs:4873`. `captured_call_index_value`: `checks.rs:1630,2074`. `captured_call_argument_index_matches`: `checks.rs:1630,2901`.

### 9. Fail-open risks
None seen.

---------------------------------------------------------------------

## H. `content_conservation` (:13669-14847)

### 1. Purpose
Judges "conservation" equations over content projections (counted quantities and interval sets): that a callable's result preserves a stated content equation, with dependencies on other laws recorded.

### 2. Shape
`interval_sets::judge` (:13683), `observed_contracts` (`ObservedContentEquation` :13847, `call_content_guarantees` :13883, `proof_dependencies` :13994, union-find at :14012-14031), `proof_dependencies::ContentConservationDependencies` (:14151, `derivation_depth` :14158, `independently_proved` :14190), `judge_content_conservation_with_dependencies` (:14234), `completed_value_content_conservation_dependencies` (:14286), `completed_value_conservation_answers` (:14395), `completed_value_proves_content_conservation` (:14474), `completed_value_preserves_entry_projection` (:14504), `completed_value_proves_backing_addresses` (:14576), `ContentValues` (:14687).

### 4. How it decides
`judge_numeric_content` (:14268-14284) on `ContentAlgebraIdentity::CountedQuantity` (equal magnitudes) or `IntervalSet`. If not `Proven`, builds a dependency graph from `normal_content_guarantees` with the same algebra (:13999-14004); `independently_proved` means the goal class is reachable using only laws with no requirements (`derivation_depth(|_| None)`, :14190-14192). `completed_value_proves_content_conservation` accepts when every dependency chain's requirements are in the `available` list (`available.contains(source)`, :14498).

### 5. Cases by shape
`ContentValues::subject_term` handles only `Field` segments; `Case` and `FixedIndex` segments return None (:14748-14751). Scalar expressions: `SubjectField`, `RuntimeScalarEmbedding`, `Natural`, `Successor`, `Arithmetic` Add/Subtract/Multiply (:14766-14827). `completed_value_content_conservation_dependencies` returns None unless `plan.owner == machine.symbol && plan.callable == state.symbol` (:14296-14298).

### 6. Same job elsewhere
The two-term "embed both as `IntegerEmbedding` and judge `MathematicalBinary Equal`" appears in `completed_value_preserves_entry_projection` (:14545-14558) and `completed_value_proves_backing_addresses` (:14655-14676).

### 7. One-caller code (grepped)
`completed_value_conservation_answers`: `checks_content.rs:782`. `completed_value_content_conservation_dependencies`: `checks_content.rs:1201`. `completed_value_proves_content_conservation`: `checks_contracts.rs:8351`. `completed_value_has_content_conservation_candidate`: `checks_content.rs:987`. `completed_value_preserves_entry_projection`: `checks_multiplicity.rs:400`. `completed_value_proves_backing_addresses`: `checks_multiplicity.rs:767`.

### 8. Stated limits
:14299-14301 "A callable law is established under its declared entry contract. Stronger facts at this program's private callers ... cannot publish the law for arbitrary later callers."

### 9. Fail-open risks
None seen. `interval_sets::judge` returns Unknown when endpoints do not pair off (:13708-13722).

---------------------------------------------------------------------

## I. `contract_refinement` (:14848-15717)

### 1. Purpose
Checks that a machine realizing a trait requirement does not ask for more than the requirement's `requires` and guarantees at least its `ensures` (with float-bound implication). Also `CallableContractRefinement` for generic callable-to-callable comparison.

### 2. Shape
`check_contract_refinement` (:15122, diagnostics), `CallableContractRefinement::prepare/proves` (:15033/:15096), `ContractFacts::collect` (:15341), `Conjunct { fact, identity, law_checked, float }` (:15333), `entailed` (:15590), `float_implication::FloatObservation` (:14858), `selected_operation_correspondence` (:15264), `is_law_conjunct` (:15665).

### 4. How it decides
- `entailed` (:15590-15648): refuses unless the goal fact has "source builtin index meaning" (:15597-15601); accepts on equal `CorrespondenceFact` identity with some premise (:15602-15608), on float-predicate implication (:15609-15611), on literal `true` (:15615-15620), else on `scoped_arithmetic_implication_for_declaration == Proven` (:15639-15647).
- `check_contract_refinement` ensures loop (:15225-15247): a required `ensures` conjunct is accepted (no diagnostic) if the fact has source-builtin-index meaning AND any of: `owes_through_edge`, `boundary_qualification_authorization(..).is_some()`, `conclusion.law_checked`, or `entailed(..)`. `owes_through_edge` (:15213-15224) is true if some conformance edge of the machine has the same requirement symbol and the same applied scope. Requires loop (:15248-15256): each member requires must be entailed by the required requires.
- `law_checked` (:15442-15448): `ProofFact::Proposition` => true; `Expression` => `is_law_conjunct` (:15665-15689: no reserved result and the fact is an identity application or `==` whose both subjects have a structural term); `Membership` => false.

### 5. Cases by shape
Ensures/requires only (`SignatureContractKind::Requires`/`Ensures`; others return false/skip, :15101-15115, :15427-15431). `ContractFacts::bind_integer` only fixed-integer primitives (:15563-15567). Generic binders `Const`/`Value` only (:15364-15366).

### 6. Same job elsewhere
`operator_contracts::check_operator_contract_conformance` (:17999) does the same "provider asks no more, guarantees at least" check for operators, with its own equality-only rung (see L). Two separate implementations of requires-subset / ensures-superset.

### 7. One-caller code (grepped)
`check_contract_refinement`: 1 outside call (`traits.rs:2523`); `CallableContractRefinement`: see `lib`/callers not counted.

### 9. Fail-open risks
- :15229-15239 the `owes_through_edge` / `boundary_qualification_authorization` / `law_checked` disjuncts accept an `ensures` conclusion without running `entailed`. No comment at the site says why. Plausible reason (inferred): laws are checked by `check_law_conformance` (`law_conformance` module, :16669 onward) and edge-owed ensures by the edge. What input reaches it: a machine realizing a requirement whose `ensures` is an equation of two structural terms but that is not actually satisfied. Classification: open (I did not trace `check_law_conformance` callers).
- `entailed` fact-has-meaning gate returns false (fail-closed) for facts without builtin index meaning; combined with the `&&` at :15226-15239 this makes such conclusions produce a diagnostic (fail-closed).

---------------------------------------------------------------------

## J. `declaration_projections`, `exit_coverage`, `fact_mentions` (:15718-16194)

### 1. Purpose
- `declaration_projection_cases_hold` (:15725): when a place projection passes through a data case (`Case` segment), prove the case holds from the declaration's `requires`.
- `entailment_covers_all_exits` (:15847): decides whether every state's transition guards cover all outcomes.
- `fact_mentions_proof_only_data` (:16017) / `fact_mentions_zero_value` (:16141): detect `ensures` facts that mention proof-only data or zero-value representation, so a different (fenced) path handles them.

### 4. How it decides
- `declaration_projection_cases_hold` returns `true` when the coordinate has no `Case` segment (:15732-15738); otherwise requires every case segment to be `Proven` by `judge_case` (:15789-15797) on a single non-const, non-erased parameter or the reserved result.
- `entailment_covers_all_exits` (:15851-15871): per state, covered if there are no transition guards, an `Always` guard, a `When(Boolean(true))`, `complementary_boolean_guards` (:15894-15908: some `x == true` and `x == false` pair with `same_condition`), or `complete_case_guards` (:15910-15947: guards are `==` or exact `CaseMembership` comparisons against case symbols and all variants of ANY data definition in the program (`program.data_definitions().iter().any(..)`, :15929) are selected on the same subject). Keyed on variant `SymbolHandle`s and structural condition equality.
- `fact_mentions_proof_only_data` (:16017-16113) is keyed on NAMES: `data_definition_by_name_str(name)` (:16029-16034) for struct literal `type_name` (:16067) and the FIRST segment of a multi-segment name path (`[first, ..] => proof_only_definition(first.as_str())`, :16062); a call's target machine is found by `target.name.as_str() == call.target.as_str()` over `program.machines()` (:16083-16086); a one-segment name is looked up among the entry state's parameters by name (:16057).

### 5. Cases by shape
`same_condition` (:15949-16010): `Binary` with same operator, `Call` with same valid target and no receiver/evidence/quotient/layout/interrupt payload whose target is a proof-machine state (:15963-15994), otherwise only scalar trees of `Boolean`, `Integer`, `Name` and `Binary` compared with `expressions_structurally_equal` (:15997-16009). `find_in_composite_children` (:16115-16139) visits only `Atomic, Binary, Unary, Cast, Member, Borrow, Indexed, Range, ArrayLiteral`.

### 6. Same job elsewhere
`fact_mentions_zero_value` and `fact_mentions_proof_only_data` walk the same node families with two separate match statements (:16036-16112 vs :16152-16192) sharing `find_in_composite_children`.

### 7. One-caller code (grepped)
`declaration_projection_cases_hold`: `checks_current_scalar_values.rs:3758` (+ `lib.rs:1689`). `entailment_covers_all_exits`: 4 outside sites (`program_validation.rs:4740,4765`, `arithmetic_domains.rs:10002,10095`). `fact_mentions_proof_only_data` and `fact_mentions_zero_value`: no uses outside `contract_entailment.rs`; in this file they are imported at :90296-90298 and called at :90713-90715 and :90863-90864 (read, outside the unit).

### 9. Fail-open risks
- :15732-15738 returns `true` with no case segment. Traced: vacuous by construction (nothing to establish).
- :15861 `guards.is_empty()` makes a state with no transitions count as covered. Stated by shape; intended (no exit to cover). Classification: traced.
- :15929 "any data definition": coverage is accepted if the guards select all variants of any data type on one subject; it does not check that the subject has that type. Traced: the guards compare against variant symbols, which belong to one data definition (`name_path_node(comparison.right).symbol`, :15925-15926), so a type mismatch would already have failed earlier typing (inferred, not traced end to end).
- `fact_mentions_proof_only_data` returning `None` for a name that does not resolve by spelling (qualified path beyond the first segment, alias, shadowing) means the fact is NOT fenced as proof-only. What reaches it: an `ensures` fact that names proof-only data through a path whose first segment is not the data name. Classification: open.

---------------------------------------------------------------------

## K. `inductive_judgment` (:16195-16668)

### 1. Purpose
For a single-state machine whose body is a list of guarded transitions (each either returning a value or tail-calling itself), proves each `ensures` fact arm by arm using the arithmetic Engine, including a strict-decrease check for the recursive call.

### 2. Shape
`inductive_transition_entailment` (:16212), `TransitionArm`/`ArmKind { Value | TailSelfCall }` (:16202-16210), `prepare_arm` (:16420), `JudgedArm` (:16413), `guard_arm_comparison` (:16516), `self_call_arguments` (:16563), `discharges_strict_decrease` (:16581), `apply_argument_map` (:16645, pub(crate)), `pure_dispatch_capture` (:16371), `expression_invokes` (:16382).

### 3. Inputs and outputs
Inputs: machine, `requires`/`ensures` handles, `all_facts_are_expressions`. Outputs: pushes `Diagnostic::error` and `proven: &mut Vec<ExpressionHandle>`. Reads `termination_plan.implementation_witness`.

### 4. How it decides
- A fact joins `proven` only when no non-vacuous arm judged it Unknown (`else if unknown_arm.is_none()`, :16365-16367). Constant-false and refuted arms produce errors (:16341-16354). An Unknown arm produces an error only if `goal_always_in_language && machine_fully_visible` (:16355-16358); otherwise the fact is neither proven nor reported here.
- Vacuous arms (engine `requires_unsatisfiable`, or a preceding unconditional arm) are skipped (:16314-16316, :16441-16447, :16507).
- `discharges_strict_decrease` (:16581-16643) decides by STRING compare on the witness `view_path` split on `::`: `["Nat","IncreasingTo"]`, empty, or `["Nat", "Descending"|"BoundedDistance"]` (:16595-16605); anything else returns false. Measure shapes: `([subject], Some([limit]))`, `([single], None)`, `([lower, upper], None)` (:16617-16629). Requires measure_now - measure_after >= 1 and measure_after >= 0 (:16638-16642).

### 5. Cases by shape
- :16222 `let [root] = states else { return; }`: exactly one state. :16241-16243 any statement that is not a transition (after optional marker/pure-capture statements) returns without verdict. :16244-16246 a transition with a continuation returns. :16252-16267 targets: `Value`, or `Named` whose path symbol is the root state or the machine; anything else returns.
- :16277-16286 every guard must have a complete, empty write frame.
- `pure_dispatch_capture` (:16371-16380): immutable, inferred-type, non-operand-capture local with a call-free initializer.

### 7. One-caller code (grepped)
`inductive_transition_entailment`: only call is `contract_entailment.rs:90893` (read, outside the unit). `apply_argument_map` (:16645): used in-unit at :1248, :1278, :1416, :1436, :2678 (and :7478? not checked).

### 8. Stated limits
The early `return;` statements are the limits (no comment). Trace env var read directly: `std::env::var("OMEGA_ENTAILMENT_TRACE")` (:16288); grep shows 2 uses in the whole file, not routed through `trace_flags` (the other trace flags are: `crate::trace_flags::struct_trace`, `trace_flags.rs:34`, wrapped at :90310).

### 9. Fail-open risks
- :16476-16477 `if engine.requires_unsatisfiable { }` is an empty branch: a self-call arm in an unsatisfiable context adds nothing. Traced (the arm becomes vacuous at :16507).
- When strict decrease is not discharged for a `TailSelfCall` arm, the arm's ensures hypotheses are not installed and `fully_visible = false` (:16494-16496). The arm is judged without them; this only weakens what can be proven (traced).
- A machine whose `requires` are contradictory makes every arm vacuous, so every fact is pushed to `proven` (:16365-16367). This is the ordinary "false premises prove anything" behaviour. Stated by design, not commented. Classification: traced.

---------------------------------------------------------------------

## L. `law_conformance` (:16669-18212, first part)

### 1. Purpose
Law conformance of a machine against a trait requirement, operator contract conformance, and the equality-statement matching machinery both use. This unit covers `declaration_correspondence`, `equality_conjuncts`, `inherited_requirements`, `law_guarantees`, and the start of `operator_contracts`. The header comment (:16670-16679) lists sibling files (`operator_contracts.rs`, `law_guarantees.rs`, `proposition_laws.rs`, `inherited_requirements.rs`, `slot_bindings.rs`) that, in this file, are inline modules.

### 2. Shape
- `DeclarationCorrespondence` (:16707, `new` :16717, `matching_guarantee` :16791, `required_conjunct` :16808, `matching_subject_payloads` :16840, `pair_conjuncts` :16874).
- `EqualityConjunct { owner, parameters, expression, meaning }` (:16942) with `EqualityMeaning { Identity | TypeEquality | BooleanPredicate }` (:16950), `equality_conjunct` (:16973), `SourceCorrespondence` (:17030; `matching_conjuncts` :17049, `canonical_source_value` :17127, `matching_values` :17228, `matching_operation_payloads` :17320), `selected_dispatches_match` (:17536), `neutral_call_payload` (:17572), `neutral_operation_payloads` (:17616), `parameter_roles` (:17654), `collect_equality_conjuncts` (:17692), `MAX_SOURCE_MATCH_WORK = 4096` (:17042).
- `inherited_requirement_proposition_application` (:17729), `inherited_requirement_proposition_label` (:17774), `inherited_satisfier_parameters` (:17829).
- `MatchedLawGuarantee` (:17870), `matched_machine_law_guarantees` (:17877).
- `check_operator_contract_conformance` (:17999), `check_operator_crash_contract_refinement` (:18104), `OperatorCrashBucket` (:18161), `operator_crash_bucket` (:18166), `ContractConjuncts` (:18207, cut by the unit boundary).

### 4. How it decides
- `equality_conjunct` (:16973-17024): case observations are rejected; an `identity_application_at` result is `Identity`; a binary `==` is `TypeEquality` (static type predicate operands) or an `Identity` if the builtin-identity operand references exist, else `BooleanPredicate`; a non-binary Bool-typed call is `BooleanPredicate`. Other shapes return None.
- `matching_conjuncts` (:17049-17121): Identity vs Identity needs equal carrier type identity and matching values in either endpoint order; TypeEquality/BooleanPredicate need matching values; mixed kinds return false (:17119).
- `matching_values` (:17228-17314): first `matching_operation_payloads` must hold; then if both sides have a canonical source key (`normalized_expression_identity` over `binders`, only after every infix has builtin meaning and source-builtin-index meaning, :17127-17225) they are compared by string equality (:17261-17263); otherwise falls back to `structural_term` + role-keyed binding + `diagnostic_shape_match` (:17264-17313). Keyed on identity keys built from positional binder strings `(SymbolHandle, String)` roster.
- `matching_operation_payloads` (:17320-17530) compares call payloads field by field (evidence args empty, static machine parameter, selected dispatch, requirement application type identity, exact coercion type, quotient/private layout operation, installed interrupt payload, layout query, native code layout, build selection invalid, operational acknowledgement flags: :17389-17421), cast payloads (:17431-17456), members (:17457-17463), struct literals (:17464-17481). Any `Match`/`Cast`/`Member`/`StructLiteral`/`Call` node paired with a different kind returns false (:17482-17491).
- `inherited_satisfier_parameters` (:17829-17857): for a boundary trait, if the satisfier has one extra parameter and the LAST path segment of that first parameter's type label equals the last segment of the trait NAME (`rsplit("::").next()` on `type_reference_label` and `trait_definition.name`, :17840-17851), the first parameter is dropped. This is keyed on spelling of a type label.
- `operator_contracts::check_operator_contract_conformance` (:17999-18102): provider requires must each match some operator requirement (:18067-18079), each operator ensures must match some provider ensures (:18080-18092), then crash refinement (:18093-18101).
- Crash refinement (:18104-18159): for each provider crash cause, a requirement bucket must exist; valid if the requirement's bucket is unconditional, or the provider's bucket is not unconditional and each provider route matches some required route via `matching_values`.

### 5. Cases by shape
Stated in the doc comment at :17986-17997: "The first checked-software rung deliberately admits the contract language ... typed identity, type equations, declared Boolean calls, and exact `&&` conjunctions." Error text at :18025: "outside checked operator-contract entailment's equality/`&&` rung". `operator_crash_bucket` skips literal `true` routes and treats an empty route list as unconditional (:18188-18197).

### 6. Same job elsewhere
- `check_operator_contract_conformance` (:17999) vs `contract_refinement::check_contract_refinement` (:15122): same requires-subset / ensures-superset shape; the former matches by `SourceCorrespondence::matching_conjuncts`, the latter by `CorrespondenceFact` identity or scoped arithmetic implication.
- `DeclarationCorrespondence::identity_carriers_match` (:16825) and `SourceCorrespondence::matching_conjuncts`'s inline `carrier` closure (:17062-17067) both compare carrier type identity under two binder rosters.
- `callable_binders`/`generic_binders` (shared helper; many users: `proof_contracts_core.rs:1393,1403,1462`, `machine_calls_core.rs`, `lowerer.rs`, `published_facts.rs`, `review-evidence/.../application.rs`; grepped).
- `normalized_expression_identity` (used at :17224) has callers in `quotients.rs`, `proof_contracts_core.rs` (2), `domains_and_predicates.rs`, `typed-trees/src/type_identity.rs`, `typed-trees/src/declarations.rs` (grepped, 8 matches in 6 files counting this file's 2).

### 7. One-caller code (grepped)
`matched_machine_law_guarantees` (:17877): `checks_contracts.rs:8973,9024` (+ `lib.rs:1931`). `check_operator_contract_conformance` (:17999): `traits.rs:1427`, `program_validation.rs:172`. `inherited_satisfier_parameters` used in-unit at :7766 and :17790.

### 8. Stated limits
:17986-17997 (rung comment above). :18030-18035 error text: "checked providers may not ask more than the operator requirement".

### 9. Fail-open risks
- :17007-17016 any non-binary Bool-typed call becomes a `BooleanPredicate` equality conjunct (comment :17002-17006 says "no Boolean call supplies value identity"). Classification: stated.
- `matching_operation_payloads` returns true when no differing payload is found and both trees have the same child shape (:17529). Both expressions are then compared by keys or by shape matching afterwards (:17261, :17307), so this alone does not accept (traced).
- `canonical_source_value` returns None (falls to structural fallback) rather than false on unsupported infix meaning (:17212, :17222). Not fail-open: the fallback must succeed.
- :18128 `.expect("a provider crash contract contributes its own bucket")` panics if `operator_crash_bucket` returns None; unreachable because the provider loop only visits causes present in `provider_contracts` (:18119-18128). Traced.

---------------------------------------------------------------------

## Cross-cutting facts

- Data structures: `Polynomial.terms` is a `BTreeMap<BTreeMap<String,u32>, BigInt>` (:5911-5915); `substitutions`, `arithmetic_intervals` are `BTreeMap<String, _>` (:6153, :6156); `CollectionBoundsDemands.demands` is a `BTreeMap` (:10177); `opaque_terms`, `strict_occurrences`, `owned_arguments`, `formal_read_arguments`, `bounds` are `Vec`s searched linearly (e.g. :3826, :4038, :5749). The repository rule on dense-id maps (AGENTS.md "Data-oriented storage") is not what these keys are; they are strings or handle pairs.
- Trace output: `struct_trace()` wrapper at :90310 delegates to `trace_flags.rs:34` (env `OMEGA_STRUCT_TRACE`). The whole file has 112 `struct_trace()` calls (grep count of the pattern in `contract_entailment.rs`, whole file). `eprintln!` strings start with `STRUCT`/`CITE`/`ENTAILDBG`.
- Engine cloning is used as a transaction: `form_integer_expression` (:688), `form_declared_integer_application` (:778), `admit_selected_integer_operation` (:2270), `capture_admitted_integer_remainder` (:5155), `Path` in `dispatch` (:265-268). Clones share two `Arc` counters (:6158, :6160).

---------------------------------------------------------------------

## Coverage

- Read in full: `contract_entailment.rs` lines 1-18212 (18212 lines), in Read chunks 1-1500, 1501-3300, 3301-5100, 5101-6900, 6901-8700, 8701-10500, 10501-12300, 12301-14100, 14101-15900, 15901-17200, 17201-18212.
- Skimmed: none.
- Not read: none inside the unit.
- Outside the unit I also read, only to verify greps: `contract_entailment.rs` lines 90236-90305, 90306-90317, 90705-90719, 90820-90899; `proof_contracts_core.rs` 5965-5987; `trace_flags.rs` 25-44.

## Five things a maintainer most needs to know about this unit

1. The file is one 91k-line inline module tree and this unit is its first 18k lines. The integer Engine (`arithmetic_judgment`, :8-7563, about 7,500 lines) is the core; everything after is a set of "visitor" style judges that walk caller prefixes and ask the Engine or `StructuralJudge` for a verdict.
2. Atom and identity names are strings built with `format!("{:?}")` of handles, values and operators (:2634, :2931, :3837, :4002, :7004, :7457, :13517, :13660). Equality of two atoms is string equality; nothing else keeps two minted names consistent.
3. `Engine::normalize`'s NAME case (:7056-7064) and `fact_mentions_proof_only_data` (:16029-16086) decide on source spelling (parameter names, data names, machine names), while strict-binding mode (:7040-7050) decides on `SymbolHandle`. Which mode applies depends on whether `strict_symbol_bindings` is `Some`.
4. Several accept paths delegate to code outside the unit with no local check: `static_type_requirement_entailed` (:8120, :8774), the prepared-mathematics constructor shortcut (:12080-12097), `data_fact_holds_at_zero` (:12610), and in refinement the `owes_through_edge`/`law_checked`/`boundary_qualification_authorization` disjuncts (:15229-15239).
5. Work is bounded by literals rather than by program size: shared conditional-work counter `1 << 20` (:6237), 4096-step budgets (:4080, :4275, :12938, :17042), depth caps 128/64 (48 matches in the whole file), 128-atom cap in the affine bound (:3550). Hitting a cap returns None/false (decline), except exhausted conditional work which is a definitive `false` from `conditional_lower_bound` (:3906-3908).

## What you are unsure about

- Whether `selected_binary_meaning` returning true for `And/Or/CaseMembership` (:5365) can ever let a user-defined overload through; I did not read `has_builtin_spelled_expression_meaning` or where `And/Or` are overloadable.
- Whether the `ensures` accept paths in `check_contract_refinement` (:15229-15239) are always backed by `check_law_conformance`; I did not trace its callers.
- Whether `value_preserving_integer_cast` (:7393-7408) needs a width check that is done by a consumer; I did not trace consumers of `normalize` for casts.
- The callers of `fold`-style helpers and the second half of this file (lines 18213-91057) are unread; counts that point there (`:23225`, `:37191`, `:53101` etc.) are grep hits only.
- I counted `struct_trace()` and depth-cap patterns across the whole file, not only in this unit.
