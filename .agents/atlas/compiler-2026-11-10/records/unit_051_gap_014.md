Second pass for unit 51

Ranges covered (one file, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks_contracts_prover.rs`, abbreviated "prover.rs"):
- prover.rs 1701-7700 (6000 lines), read in full (I also read 1680-1700 and 7701-7719 for context; those lines do not count).

Where the range starts and ends: line 1701 is inside `call_guarantees::arithmetic`, in the tail of `append_store_hypotheses` (starts 1617 per the earlier notes; the closure and `returned` return are at 1690-1716). The range ends at 7700 inside `instruction_effects::evidence_inner` (7646), in the `for context in contexts` loop, in the atomic-release-store branch. Everything between is read. Module map of the range (all line numbers read):

- 1701-2630: rest of `call_guarantees::arithmetic` (closes 2630).
- 2631-2849: `call_guarantees::callable` (`Callable`).
- 2851-3371: `availability`. 3372-3441: `induction`.
- 3442-4139: `static_operands` (with inner `projections` 3448-3804).
- 4140-5439: items directly in `call_guarantees` (`Invocation`, `proves_current`, `current_predicates`, `proves_captured_predicates`, `scalar_at_place`, `proves`, `capture_preserved`, `guarantee_scalar`, `guarantee_input`, `invocation`, `stable_predicate_inputs`, `predicates_match`, `captured_place`, ...). `call_guarantees` closes at 5440.
- 5441-5745: `current_boolean`. 5746-5854: `field_actuals`.
- 5855-6376: the file's top-level entry functions. 6378-6520: `propositions`. 6523-7700: `instruction_effects` (`atomic` 6540-6821, `operands` 6822-7507, `proves_contract` 7510, `InstructionEffectEvidence` 7568, `evidence` 7626, `evidence_inner` 7646).

Contradictions with the earlier notes: none about content of 1-1700 (not re-read). One correction to their "unread" list: `evaluator` is not defined in this file; it is a sibling module imported at prover.rs:5861 (`use super::evaluator::call_site_proves_boolean_contract_expression`), so it is outside this unit. The three entries named in their header note are at `semantic_contexts_prove_boolean_expression` 5863, `call_entry_contexts_prove_boolean_contract_expression` 6031, `semantic_contexts_prove_contract_fact` 6134.

---------------------------------------------------------------------
## 1. `call_guarantees::arithmetic` (rest), prover.rs 1701-2630

### Purpose
Turns a callee's `ensures` guarantee (and the caller's live facts) into propositions for the strict arithmetic judge (`validation::scoped_arithmetic_implication`), so a caller can prove a call's `requires` or an exit's `ensures` from a normal-return guarantee. Values that cannot be named by a place become opaque "atoms" with a string identity.

### Shape
- `proves_bound` (1719): builds hypotheses from context facts (`append_context_hypotheses` 1768) and asks `scoped_arithmetic_implication(...) == Proven` (1764). Returns `false` if the caller machine is not found (1730).
- `append_context_hypotheses` (1768): skips facts whose origin is `CallRequires`/`CallEnsures` (1795-1800); takes `ContractBooleanExpression{Requires}` with `!instantiated.is_valid()` as `holds=true` and `BooleanValue` as `(expression, value)` (1801-1809); everything else `continue`s.
- `at_invocation` (1840): returns `InvocationArithmetic{proposition, returned}`. Atoms for a callee's own result go through `call_result_atom`; parameters that were written on normal return get a "post-return" atom with identity `"\0call-guarantee:return:..."` (1916-1927) and are recorded in `returned` (1928).
- `call_result_atom` (1973), `value_origin` (2036), `observation_origin` (2055), `atom` (2082), `collection_length_value` (2100), `length_atom` (2117), `collection_length_observation` (2126), `collection_length_subject` (2159), `proposition` (2177), `proposition_with_static_actuals` (2187), `scalar_term` (2211), `admitted_scalar_expression` (2228), `completed_scalar_value` (2250), `bind_completed_remainders` (2262), `bind_occurrences` (2301), `predicate` (2438), `term` (2465), `contents_primitive` (2536), `builtin_unsigned_remainder` (2553), `admitted_unsigned_remainder` (2575), `value_preserving_integer_coercion` (2594), `proof_integer_term` (2609).

### Inputs and outputs
Reads `CheckingInput`, `CheckFacts` (`flow`, `semantic`, `operators`), `FlowStateFact`, `CallFrameResolver`. Produces `ScopedArithmeticExpression`/`ScopedArithmeticValue` (terms or `Atom{identity: String, unsigned}`), `Vec<ReturnedObservation>`, booleans. No diagnostics.

### How it decides
- Atom identity is a `format!` string of debug-printed handles/places: `"\0call-guarantee:{primitive:?}:{place:?}"` (2087), `"\0call-guarantee:length:{place:?}"` (2121), `"\0call-guarantee:return:{caller_state:?}:{statement}:{ordinal}:{primitive:?}:{length}:{storage:?}"` (1918), `"\0call-result:{state:?}:{statement}:{ordinal}:{segments:?}:{primitive:?}"` (2022), `"\0static-parameter:{symbol:?}"` (2374). Two atoms are the same value iff these strings are equal (read; the judge's equality is in `validation`, not read). All `\0call-guarantee|\0static-parameter|\0instruction-` string prefixes: 9 hits, all in prover.rs (grep over `omega-rust`, count mode).
- `call_result_atom` (1973-2034): keyed on call-site coordinates `(statement_index, call_ordinal)` and handle equality of `authored_expression`, `authored_site`, `authored_target`; two cases only: `CallSite::Expression` (atom on `PlaceRoot::Expression`) and `CallSite::TransitionNamed` (string atom). `_ => None` (2032). Also requires `call.authored_source_custody_valid` (1980) and primitive equal to the projected return type's primitive (1983-1989).
- `observation_origin` (2055): if the place root is an expression node of kind `Call`, the place is its own origin (2065-2069); otherwise delegates to `crate::flow::value_origin_before_statement`.
- Length: a `.len`-style member is recognised through `validation::collection_operations::collection_measure_member(...) == Some(CollectionMeasure::Length)` (2166-2170), not by member spelling. Observation is `Binding` when the receiver is `&[T]`-slice-through-reference, else `Contents` (2145-2156).
- Proposition admission: `has_builtin_operators` + `callable.builtin_meaning` + `predicate(...)` (2195-2198). `predicate` allows `And` of comparisons; comparisons need `term` on both sides (2447-2462). `term` (2465): `Integer`; `Name`/`Member` with integer-literal-accepting readable primitive; `Indexed` only when `immutable_const_projection` is Some; `+ - *` only when the operand reference's `arithmetic_domain_for_type_reference == Exact` or `proof_integer_term` (2486-2502); `%` only for builtin unsigned remainder with equal unsigned primitives (2503-2509, 2553-2572); `Cast` only when not recast and fixed-integer to fixed-integer (2510-2513, 2594-2607); `Call` only integer-embedding or integer-typed result (2514-2525).
- `bind_occurrences` (2301): for each occurrence with an integer-accepting primitive, choose a binder: `Member` -> `Occurrence`; `Indexed` with immutable const projection -> `Occurrence`; single-segment `Name` -> `Symbol`; `Name` that is the callable's own `result` with empty segments -> `Result`; anything else returns `None` (2354) (whole proposition refused). Const type parameters take the caller's static actual else a `\0static-parameter` atom (2359-2380).

### Cases by shape
- `term`'s fixed match (2474-2526) with `_ => false`; `predicate` (2447-2462) `_ => false`; `proof_integer_term` (2616-2628) `_ => false`.
- `collection_length_observation` special-cases only `Constrained` and `Reference` wrappers around `Slice` (2137-2155).
- `bind_occurrences` binder choice (2331-2355) as above.

### Same job elsewhere
- `Callable::scalar_reference` (2763) and `contents_primitive` (2536) vs `validation::expression_result_type_reference` family: the former delegates to it (read).
- `atom`/`length_atom`/`call_result_atom` vs the atoms built in `instruction_effects::operands` (`"\0instruction-capture:..."` 6969, `"\0instruction-current:..."` 7104, `"\0instruction-return:..."` 7321) and in `static_operands::projections::atom` (`"\0static-parameter:...:path:..."` 3475): four atom-identity formats in one file, each a `format!` of debug text (read). The exit prover has its own (`"\0{scope}:{:?}"`, exits.rs 2237 per earlier notes).

### Unused or one-caller code (grep, count mode over `omega-rust`)
- `call_result_atom|completed_scalar_value|collection_length_value|length_atom`: 14 occurrences in 3 files: prover.rs 9, `checks_current_scalar_values.rs` 2, `checks_contracts_exits.rs` 3.
- Comment 2225 "One admission owner for scalar denotations and captured call actuals": `admitted_scalar_expression` is private (2228); its two callers are `scalar_term` (2218) and `bind_occurrences` (2416), both in this file (read).

### Stated limits
- "Complete ... Consumers must establish completion/availability before using this identity." (1968-1972, paraphrase of the comment; the atom does not itself check completion.)
- "Unsigned remainder is unchanged by every arithmetic policy; its exact selected source retains that policy" (2244-2248, paraphrase).

### Fail-open risks
- `append_context_hypotheses` (1768): a missing machine/state returns with no hypotheses (1779-1788), which only makes the proof weaker. Closed.
- `call_result_atom` returns `None` on any mismatch; no accept-on-None seen.
- `term` `ExpressionNode::Call` arm (2514-2525): any call whose scalar reference is an integer-accepting primitive is admitted as a term. What reaches it: a call expression whose value is then bound by `bind_occurrences`; calls are not in the `Member/Indexed/Name` binder list, so they reach `_ => return None` (2354) unless handled as an argument at 2415-2430. Traced locally: closed.
- Otherwise none seen.

---------------------------------------------------------------------
## 2. `call_guarantees::callable`, prover.rs 2631-2849

### Purpose
`Callable` (2654) is one view over "something with parameters, a return type, contracts and arithmetic meaning": a machine state, a bodyless requirement signature, or a transparent proposition. All contract arithmetic is written against it.

### Shape
Enum `Callable{Machine{machine,state}, Requirement{signature}, Proposition{declaration}}` (2654-2665). Methods: `resolve` (2668), `owner_symbol` (2699), `target_symbol` (2707), `parameters` (2715), `result_type` (2730), `contracts` (2738), `scalar_reference` (2763), `builtin_meaning` (2793), `decomposed_builtin_meaning` (2824).

### How it decides
- `resolve` (2668-2697): looks the target up as machine head AND as requirement signature (two sources); accepts only when exactly one candidate (`candidates.next().is_none()` 2691) and, if there is a result type, it exists in the type table (2692-2695). Ambiguous target -> `None`. Keyed on symbol handle.
- `contracts` (2738): machine-level contracts are included only when `state` is the machine's first state (2744-2752); state contracts always. Keyed on state symbol equality with the first state.
- `scalar_reference` (2763): a `result` place of this owner returns its type, a `result` place of another owner returns `None` (2768-2771).
- `decomposed_builtin_meaning` (2824-2847): for `Requirement`/`Proposition`, any `And`/`Or` binary is accepted outright (2837-2843) without checking its operands' meaning; for machines it calls `has_builtin_decomposed_guard_meaning`.

### Cases by shape
Three variants only; `Proposition` has no contracts (2758) and an invalid result type (2689, 2734).

### Fail-open risks
- `decomposed_builtin_meaning` Requirement/Proposition arm (2836-2845): returns `true` for any `And`/`Or` expression regardless of operand meaning. What passes: an `And`/`Or` over operands with non-builtin operators in a requirement/proposition contract. Used by `AvailableGuarantee::has_builtin_meaning` (2885). Callers inside this range: grep shows `has_builtin_meaning` is in availability only; leaves are still checked by `builtin_meaning` elsewhere (inferred, not traced). "open".

### Unused / one-caller
`result_type` (2730) `pub(super)`; used in 4259, 4277, 5032 (read). `Callable` is re-imported at 2850.

---------------------------------------------------------------------
## 3. `call_guarantees::availability`, prover.rs 2851-3371

### Purpose
Decides which callee `ensures` clauses are available to a caller at a given statement: the guarantee must be owned by the callee's declaration, in the current flow contexts, the call must be independent (or strictly descending recursion), its inputs stable, and its captured inputs preserved by the call. Memoises admissions per state so borrow checks that ask at every statement do not redo them.

### Shape
- `AvailableGuarantee` (2872: `expression`, `fact` (ProofFact handle), `invocation`, `post_return_writes: Option<Vec<CanonicalPlace>>`) with `coordinates` 2881, `call_expression` 2900, `result_segments` 2912, `actual_projection` 2928, `result_binding` 2951.
- `GuaranteeUse{StatementEntry(usize), EvaluatedContext(usize)}` (2991).
- Free `available` (2996), `any_available` (3036), memoised `StateGuarantees<'a,'program>` (3087; `admitted: Vec<Vec<Admission>>` indexed by statement, 3093; `Admission` 3101; `walk` 3154), `scan` (3215), `admit` (3285), `owns_guarantee` (3355).

### Inputs and outputs
Reads `CheckFactsRef` (`semantic` contexts, `flow` control rows `operand_origin_writes`, `operand_referent_segments`, `operators`, `borrow`), `FlowStateFact`, `CallFrameResolver`. Output: `Vec<AvailableGuarantee>` or a bool.

### How it decides
- `scan` (3215-3278): iterates facts of the given contexts; keeps only `ContractBooleanExpression{Ensures}` facts whose point is `ProgramPoint::CallEnsures` with same `machine_symbol` and `state_symbol` as the caller and `fact.origin == FactOrigin::CallEnsures` (3235-3255); statement filter is `statement_index >= statement` -> drop for `StatementEntry`, `> statement` -> drop for `EvaluatedContext` (3256-3259); de-duplicates on `(source fact handle, (statement, ordinal))` (3263-3267).
- `admit` (3285-3353): all must hold: `invocation(...)` found; `owns_guarantee` (the ProofFact is `Expression(expression)` and is in one of the callable's `Ensures` contracts' fact list, 3361-3369); `builtin_predicate`; `validation::independent_call_site(...)` OR `induction::strict_recursive_call`; `stable_predicate_inputs`; `capture_preserved`. `post_return_writes` is `Some(..)` only when `call.normal_return_writes_complete` (3329-3346).
- `StateGuarantees::walk` memo key is `(ordinal, source fact, expression)` within a statement row (3179-3183) (linear `find` over the row). If `frames` is `None` it returns an empty list (3160-3162).
- `result_binding` (2951): requires the statement to be a local-data statement whose `initial_value` is this call expression and `captured_place` to map the local back to the call expression (2958-2981).

### Cases by shape
`CallSite::Expression` only has a call expression (2900-2904); statement-site calls bind names. `result_binding` only for local declarations (`as_local_data`, 2964).

### Same job elsewhere
- `StateGuarantees::available`/`any_available` duplicate the free `available`/`any_available` by design (comment 3135, 3144 says "The same guarantees, in the same order"); both call `scan`+`admit`. Outside users (grep `StateGuarantees`, count mode): `checks_contracts_calls.rs` 3, `checks_borrows.rs` 5, `checks_borrows_overlap.rs` 2, `checks_borrows_calls.rs` 1, `checks_contracts.rs` 1, prover.rs 4 (16 total, 6 files). Free `available` and `any_available` are used in this file (4205, 4249, 4487); other users are not separated by that grep.
- Memo cache comment: "Each admission is decided at most once and kept here." (3079). The AGENTS.md says "never cached in ad-hoc mutable maps" for derived data; this is a per-state `Vec` memo (read).

### Stated limits
- "Whether a call's guarantee is admitted ... does not depend on where it is used" (3072-3075, paraphrase): the soundness argument for the memo.

### Fail-open risks
- `owns_guarantee` requires membership in the callable's `Ensures` contracts; no accept-on-missing.
- `scan` `continue`s past non-matching facts; only matching admitted guarantees are returned. Closed.
- Memo soundness depends on the program/facts being immutable while borrowed: "stated" (3079-3085).

---------------------------------------------------------------------
## 4. `call_guarantees::induction`, prover.rs 3372-3441

### Purpose
A recursive call may use its own callee's `ensures` only if that exact call edge is a strictly decreasing arrival in the machine's termination plan.

### Shape/decides
`strict_recursive_call` (3381): false unless owner==caller machine, same state, and not an internal state transfer (3387-3393); needs a body judgment with `local_terminates`, empty diagnostic, and (when the implementation witness has `rank_range`) `entry_rank_formed` (3401-3414); then there must be an `arrival_edges` row with `source == state.symbol`, `destination == entry_state`, `statement_ordinal == invocation.statement`, `target == target`, `strictly_decreases` (3428-3439). Keyed on symbol and statement ordinal.

### Fail-open risks
None seen: every lookup failure returns `false`.

---------------------------------------------------------------------
## 5. `call_guarantees::static_operands`, prover.rs 3442-4139

### Purpose
Substitutes the caller's static (const generic / const argument) values into a callee's contract arithmetic: reads the call's admitted static-machine-application tuple and evaluates const arguments, including fields/elements of immutable const aggregates.

### Shape
- `projections` (3448-3804): `atom` (3467), `value` (3486), `schema` (3560), `step_carrier` (3571), `field_step` (3588), `decoded_value` (3608), `expression_value` (3653).
- Outer: `has_template` (3818), `tuple` (3837), `admitted` (3915), `const_symbol` (3932), `immutable` (3944), `projection_atom` (3952), `is_invocation_formal` (3964), `bindings` (4004), `selected_value` (4087).

### Inputs and outputs
Reads the call site's `authored_machine_application`, `machine_arguments`, `static_machine_application_bindings`, const declarations' `evaluated_value` encoding. Produces `Vec<ScopedArithmeticBinding>`.

### How it decides
- `tuple` (3837-3913): accepts only when template chain from the callee up to the binding declaration exists (`has_template`, 3861-3865; loop with `seen` guard 3823-3834), argument counts line up (3868-3873), no duplicate parameter bindings (3874-3881), no `RuntimeValue` bindings and `static_binding_matches_argument` (3886-3897), conformance bounds match (3899-3911).
- `admitted` (3915-3930): `true` when no machine arguments and the callee has no type parameters or conformance bounds (3926-3928); otherwise requires `tuple(..).is_some()`.
- `bindings` (4004-4085): for every contract operand occurrence: an immutable const projection or a bare const type parameter. If the callee's own formal (`is_invocation_formal`) has no binding in the tuple -> `return None` (4043-4045, 4062-4064). Non-formal const parameters with no binding are given a symbolic atom (4046-4048) or skipped (4065). Conflicting values for the same symbol -> `None` (4076-4079).
- `decoded_value` (3608-3651): checks type identity by text (3615), `validate_exact_const_value_encoding` (3620), walks the path with `match (step, decoded)` over `Record`/`Array` only, then requires `Integer` with matching `type_name == primitive.name()` (3645-3650).
- `field_step` (3588): field chosen by symbol equality with exactly one candidate (3599-3604); the field NAME (`selected.name.as_str().to_owned()`) is then used to find the value in the decoded record (3634-3635): symbol to name to record lookup (source spelling used as the key after the symbol match).
- `projections::value` `Named` case with an invalid symbol: `CanonicalConstValue::from_atom(name.as_str())` (3548) — decodes a const from the NAME text of an unresolved type reference.

### Cases by shape
- `expression_value` (3653-3803): `StructLiteral` without case, `ArrayLiteral`, `Name` only; `_ => None` (3799). Struct literal requires every declared field supplied exactly once (3703-3720).
- `projections::value` (3486-3554): `ConstExpression`, `Named`; `_ => None` (3552).
- `selected_value` (4087-4138): literal, `Named` const param, `ConstExpression`; `_ => None`.

### Same job elsewhere
- `static_operands::projections::expression_value` handling `Name` -> const declaration (3755-3797) vs `value`'s `Named` case (3512-3551): same const-declaration decode written twice (read both).
- `selected_value` (4094-4136) vs `bind_occurrences`'s const-type-parameter atoms (arithmetic 2359-2380) both produce `"\0static-parameter:{symbol:?}"` atoms (4105, 2374).
- The "static actuals" idea (tuple of static arguments) is also read in `checks_contracts_exits.rs` `static_calls` (8771, earlier notes). Suspected same job, not compared.

### Unused / one-caller
`static_operands::immutable` is used at 4651 and 4983 only (read). `bindings` is used at 1850 (read).

### Stated limits
- "This never reads runtime storage or executes a const initializer." (3453-3454, paraphrase)
- "Runtime Value subjects keep their ordinary capture/preservation obligation." (3446-3447, paraphrase)

### Fail-open risks
- `admitted` returning `true` for zero static arguments on a non-generic callee (3926): a call with no static arguments to a callee that has no static parameters. Traced locally: correct, there is nothing to bind.
- `bindings` skipping an occurrence that is not an immutable const projection or const parameter (4055-4057): intended (other occurrences are bound by `bind_occurrences`).
- `bindings` for a non-formal const projection root with no tuple binding returns an atom (4046-4048): the occurrence is a const in an enclosing scope rather than the callee's; treated as an opaque constant. Traced locally; "stated" by the `formal` split but no comment.
- Others closed (`None` on mismatch).

---------------------------------------------------------------------
## 6. Items in `call_guarantees` after `static_operands`, prover.rs 4140-5439

### Purpose
The core matching between a caller's requirement and an available callee guarantee: either they are the same predicate over the same captured places, or the arithmetic judge proves the requirement from the guarantee's hypotheses. Also exposes the guarantees as bound predicates and scalar values to other checkers.

### Shape
- `Invocation` (4146: `site: CallSite`, `callable`, `caller_state`, `statement`, `ordinal`), `invocation_fact` (4154), `PredicateScope` (4173; closures `literal`, `place`).
- `proves_current` (4184), `current_predicates` (4241), `proves_captured_predicates` (4350), `scalar_at_place` (4431), `proves` (4462, `pub(in crate::checks::contracts)`), `capture_preserved` (4557), `guarantee_scalar` (4697), `guarantee_input` (4800), `captured_actual_operand` (4826), `invocation` (4865), `ordinary_arguments` (4907), `stable_predicate_inputs` (4942), `logical_formal_reads` (5009), `stable_captured_projection` (5058), `stable_owned_projection` (5092), `stable_value` (5114), `exclusive_borrow_place` (5145), `direct_place` (5162), `builtin_predicate` (5183), `predicates_match` (5193), `same_scalar_type` (5274), `actual_projection` (5301), `formal_place` (5320), `bound_literal` (5336), `bound_place` (5350), `captured_place` (5378).

### Inputs and outputs
Reads flow control rows (`operands`, `calls`, `states`), the call site, semantic fact contexts, `CallFrameResolver`. Outputs booleans, `Vec<BoundPredicate>`, `Option<ScalarValue>`, `Option<&FlowOperandFact>`.

### How it decides
- `proves` (4462-4555): requires `invocation` found, `frames` present, `stable_predicate_inputs` and `builtin_predicate` (4474-4485). Then `exact` = any available guarantee whose predicate structurally matches (`predicates_match`), and falls back to `arithmetic::proves` with hypotheses collected from guarantees having `post_return_writes` (4486-4554).
- `predicates_match` (4193 on... see 5193-5272): structural match of two expression trees: `Binary` with same operator and `same_scalar_type` on both sides; `Unary`; `Cast`; otherwise leaves compare literal values, or place equality after `captured_place`. Also accepts when the SUPPLIED side is an `And` and the required matches either conjunct (5263-5269). Cycle guard: `pending` list of `(left,right)` pairs (5203-5206). Keyed on expression-tree shape, operator equality, literal value and canonical place.
- `same_scalar_type` (5274-5299): both literals -> compare values; else both primitives equal and `arithmetic_domain_for_type_reference` equal.
- `capture_preserved` (4557-4689): the call's `signature_ceiling_places` (possible writes) must not overlap the places the guarantee reads, unless the formal is an exclusive reference (`crate::values::exclusive_reference`, 4639, 4686). Overlap is `canonical_place_segments_may_overlap`. For `logical_formal_reads` routes, a literal actual with empty segments is accepted (4610). Missing borrow state / call / ceiling returns `false` (4566-4596).
- `stable_predicate_inputs` (4942-5002): only the actuals the predicate names are required stable (comment 4935-4941); stable means: operand expression equals actual, projection type resolves, and either `stable_owned_projection` or `stable_value` (a literal, or a place rooted at a `Symbol`, or a struct literal of stable fields) (5088-5089, 5123-5135).
- `captured_place` (5378-5439): looks through facts `AssignedValue`/`AssignedScalarValue`/`AssignedValueCapture` whose destination root is the place root and whose segments are a prefix of the place; the longest prefix wins; if every such fact assigns the SAME call expression the place is rewritten to `PlaceRoot::Expression(call)` with the prefix drained; mixed call/non-call or two different calls -> `None`. If none found the place is returned unchanged (5438).
- `ordinary_arguments` (4907-4933): no evidence arguments, no const parameters, parameter count equals argument count, static operands admitted, `static_requirement_dispatch` only for `Requirement` callables.
- `invocation` (4865-4905): `Requirement` callable when the call has a static requirement dispatch or the target is a requirement signature; `TransitionNamed` first tries `find_state` in the same machine, else `Callable::resolve`.
- `exclusive_borrow_place` (5145): comment says the exclusive-borrow actual resolves "through canonical place spelling"; the code uses `canonical_place_from_expression` and rejects `Index` segments and unresolved identities (5153-5158).

### Cases by shape
- `predicates_match`: Binary, Unary, Cast, else leaf (5214-5262).
- `stable_value`: `StructLiteral` or literal or direct place with `Symbol` root (5123-5135).
- `direct_place` (5162-5181): only `Name`, `Member` chains and `Borrow` targets; any other node -> `None`.
- `guarantee_scalar` (4697): `bool` primitive literal; scalar term; or projection (4771-4791).
- `current_predicates` (4241): scalar predicates lowered with fuel `&mut 4096` (4262).

### Same job elsewhere
- Two predicate matchers: `predicates_match` (5193) over expression trees, and `boolean_predicates::proves`/`BoundPredicate` (called at 4415 and 5982, defined outside the unit in `published::facts::boolean_predicates`). They overlap in purpose (does a stored/promised predicate imply a required one); suspected, not compared.
- `stable_predicate_inputs` (4942) and `capture_preserved` (4557) both walk `append_expression_occurrences` of the guarantee with the same per-occurrence branches (reserved result / bound literal / immutable const) (4647-4653 and 4979-4985): same shape twice, in this range.
- `proves_captured_predicates` has external users (grep with the `src` dir: `checks_contracts.rs` 3929, 5998, 6157, 7750; `checks_contracts_exits.rs` 6085; `published_facts.rs` 1085) and `proves_current` has one (`checks_contracts_exits.rs` 5888). `scalar_at_place` one user outside its definition (prover.rs 727, in the part read by the earlier reader). Combined count for `proves_current|current_predicates|proves_captured_predicates` across `omega-rust`: 15 occurrences in 4 files (published_facts.rs 1, prover.rs 4, exits.rs 3, checks_contracts.rs 7).
- `captured_place`|`direct_place`|`indexed_membership`|`contract_membership_place_is_accessible`: 44 occurrences in 10 files (count mode; prover.rs 15 and 9 other files, including `06_checked-trees-to-lowered-psi/src/retention.rs` 5, `checks_borrows_resources.rs` 7, `execution_terminal_unit*.rs` 10 across three files). These names are widely used outside the file; the pattern also matches a `captured_place` name in other files that I did not confirm is the same function.

### Stated limits
- "Requiring every sibling actual to be a place rejected `cell(grid, y, x + 1)`" (4935-4938): why only named actuals are required stable.
- "Keep storage bindings intact for write retirement; completed-result joins belong to proof scratch" (4238-4239, paraphrase).

### Fail-open risks
- `predicates_match` And-on-supplied-side (5263-5269): a required predicate is accepted if it matches ANY conjunct of the guarantee. Sound (a conjunct implies nothing wrong) provided the supplied `And` is a conjunction; the arm tests `BinaryOperator::And` only. Traced locally: closed.
- `captured_place` (5438): returns the place unchanged when no assignment fact covers it; downstream compares places equal, so an uncaptured place only matches itself. Traced locally.
- `capture_preserved` 4610: literal actual with empty formal segments counts as preserved (a literal cannot be written). Closed.
- `capture_preserved`: `writes` empty -> every occurrence preserved (4667-4676 returns `true` when no write overlaps). Depends on `signature_ceiling_places` being complete (outside unit): "open".
- `proves_captured_predicates` (4350): when two bindings of the same place give different known scalar values it records `None` and returns `false` (4390, 4408-4410). Closed. The final `false` at 4423 after the fixpoint loop. Closed.

---------------------------------------------------------------------
## 7. `current_boolean`, prover.rs 5441-5745

### Purpose
Says whether a stored Boolean place is known true or false at a program point, from current facts only (comment 5442-5446: "an old initializer is not evidence about a value that may have changed"), with a batch query object that shares prepared inputs across fields.

### Shape
`BooleanValueQueries` (5464; fields `predicates: OnceCell<Vec<BoundPredicate>>`, `assignments: RefCell<RootAssignmentObservations>`, `boolean_facts: OnceCell<...>`), `at_place` (5520), `ordinary_at_place` (5507), `boolean_value_at_place` (5592), `boolean_facts` (5609), `ordinary_boolean_value_from` (5647), `postcondition_place` (5707).

### How it decides
- `at_place` (5520): if the place's readable type is a non-Bool primitive -> `None` (5559-5571); else recorded value; else inferred via `inferred_boolean_at_place` over retained predicates.
- `ordinary_boolean_value_from` (5647-5705): starts from the scalar snapshot (`Some(Boolean)` or `None`; any other scalar -> `None`, 5653-5657); for each Boolean fact stored at exactly this place it tests, for each leaf occurrence matching the subject, whether the fact forces the leaf true or false via `direct::guard_value_proves_with`; two facts forcing opposite values -> `None` (5697-5699). For `CallEnsures` facts the subject is mapped through `postcondition_place` (5672-5676) else compared by place equality.
- `place != Some(actual)` is a place-equality filter (5665).

### Fail-open risks
- The type check at 5559-5571 is skipped when the program point has no `(state, statement)` scope (5557 `_ => None`), so for those points a non-Bool place is not rejected here. What would pass: a Boolean value query on a numeric place at a `ProgramPoint` variant not listed (5525-5557). Result would still require a recorded `ScalarValue::Boolean` or a Boolean fact at that place, so a numeric place would normally give `None`. Traced locally: low; "open" for the `_` variants.
- None else seen.

---------------------------------------------------------------------
## 8. `field_actuals`, prover.rs 5746-5854

### Purpose
For a supplied Boolean field fact at call entry (like `grid.flag`), maps the callee's formal-rooted member to the caller's actual place and asks whether that Boolean is true now.

### Shape/decides
`proves` (5762): returns `Option<bool>`; comment (5759-5760): "None leaves unrelated computed-value proof paths unchanged. A recognized place requirement returns Some(false) on mismatch and cannot use labels." Cases: not a `Member` -> `None` (5773-5778); `has_explicit_formal_root` `Some(false)` -> `None`, `None` -> `Some(false)` (5779-5783); place or argument lookups fail -> `Some(false)`; raw actual root not a `Symbol` -> `None` (5798); else `boolean_value_at_place(...) == Some(true)` (5806-5819). `checked_place` (5848) delegates to `crate::flow::projected_leaf_place`.

### Fail-open risks
`None` returns (5787-5788, 5798, 5805) hand the decision to the general route at 6097 (`field_proof.is_none() && ...`). `5787` `required.root.as_symbol()?` and `5788` `state_parameter_position(...)?` return `None` rather than `Some(false)` when the formal cannot be resolved; the fallback is the semantic-context prover (not an accept). Traced locally: closed.

---------------------------------------------------------------------
## 9. Top-level entry functions, prover.rs 5855-6376

### Purpose
The file's public surface (header 1-28 per earlier notes): three entries plus live-state helpers.

### Shape
- `semantic_contexts_prove_boolean_expression` (5863): literal `true` or any entry context proving the expression through `booleans::semantic_context_proves_boolean_expression`.
- `live_contexts_prove_boolean_expression` (5890): `live_scalar_value == Some(Boolean(true))` OR `current_bound_boolean_is_proven` OR `CurrentScalarValues::at(...)...proves` (5898-5934).
- `current_bound_boolean_is_proven` (5942), `live_scalar_value` (5994).
- `call_entry_contexts_prove_boolean_contract_expression` (6031): order of tries: place predicate (case/byte) via `place_predicate` (6057-6081), `field_actuals::proves` (6083-6096), the `semantic_context_proves_instantiated_boolean_expression` over each entry context (only when `field_proof.is_none()`, 6097-6121), then `call_site_proves_boolean_contract_expression` (from sibling `evaluator`, 6122).
- `semantic_contexts_prove_contract_fact` (6134): first `contract_membership_place_is_accessible`; then a match on `fact.payload`: domain membership (indexed families by exact `(symbol, instance)` via `carried_membership_at`, 6151-6161; others via `proves_place_domain_membership_in_program` or `carried_membership_at` with `domain_membership_implies` or `prove_domain`, 6163-6187), carry permission pair (6189), `BooleanExpression`/`ContractBooleanExpression` (6200), proposition applications (6210-6217: label proof or `instruction_effects::proves_contract`), `CarryOrigin` (6218).
- `contract_membership_place_is_accessible` (6251), `place_covers_subject` (6300), `indexed_membership` (6316), `case_test_subject_actual` (6333), `contexts_prove_proposition_label` (6355).

### How it decides
- `indexed_membership` (6316-6327): scans `program.domain_definitions()` for the domain symbol and non-empty index parameters: symbol compare, linear scan per query.
- `contexts_prove_proposition_label` (6355-6376): a proposition fact is proved when a context's `proves_proposition_label(required_label)` holds or, for a label with prefix `"boolean:"`, a retained boolean fact's `boolean_fact_label` equals the suffix (6366-6373). The label comparison is by STRING (labels are the key). Users (grep, content mode): `checks_operators.rs:2207`, `checks_contracts.rs:533`, `checks_contracts.rs:3930` (re-export), prover.rs:6215, i.e. 2 external call sites plus the re-export. Comment 6353 "Call `requires` resolution uses the same admission as the contract prover" matches the two external call sites (`checks_contracts.rs:533` and the prover itself); I did not read `checks_contracts.rs:533`.
- `semantic_contexts_prove_contract_fact` proposition arm (6212): label from `semantic.proposition_fact_label(program, fact)`; missing label -> `false`.

### Cases by shape
`semantic_contexts_prove_contract_fact`'s match is exhaustive without a wildcard: the explicit `false` arm lists `InstructionEffect`, `CallCompletion` (6146) and `BooleanPredicate | CaseValue | AssignedValue | AssignedIntegerBounds | FloatPredicate | AssignedScalarValue | AssignedValueCapture | StorageDependency | BytePredicate | BooleanValue | MatchPattern | TypeConstraint | ProofObligation | Contract` (6229-6242) (read). Those payload kinds can never be proved by this entry.

### Fail-open risks
- `contract_membership_place_is_accessible` (6251): returns `true` for non-membership payloads (6257-6262) and for membership facts whose point is not `CallRequires` (6263-6271), i.e. the gate only restricts call-requires memberships. `true` here means "continue to the real proof", not "accept". Traced locally: closed.
- `semantic_contexts_prove_boolean_expression` accepts the literal `true` expression (5870-5873). Intentional.
- `call_entry_contexts_prove_boolean_contract_expression` returns `false` when no call site or no target parameters (6042-6055). Closed.
- None seen that accepts on `None`.

---------------------------------------------------------------------
## 10. `propositions`, prover.rs 6378-6520

### Purpose
Evaluates a transparent proposition application (a named proposition whose body is a boolean expression or another application) with live scalar values for its arguments.

### Shape
`PropositionProof` (6390; `active: Vec<SymbolHandle>` recursion guard), `PropositionArguments` (6398), `live_contexts_prove_proposition_application` (6402) = `application(app, None) == Some(true)`, `application` (6422), `parameter_value` (6483).

### How it decides
Refuses (`None`) on recursion (`active.contains`, 6427), missing definition, non-`Transparent` body (6433), or argument/parameter count mismatch (6441). Parameter values come only from parameters with a primitive type; others get `None` (6449-6465). Result is `Some(true)` only for `Boolean(true)`.

### Unused / one-caller
`live_contexts_prove_proposition_application`: re-exported at 6521; combined grep with two other names (`live_contexts_prove_boolean_expression|live_contexts_prove_proposition_application|contexts_prove_proposition_label`): 11 occurrences in 3 files (`checks_operators.rs` 1, prover.rs 5, `checks_contracts.rs` 5). `BooleanValueQueries|boolean_value_at_place` (section 7): 17 occurrences in 7 files (prover.rs 7, exits.rs 1, `checks_contracts.rs` 2, `flow_expression.rs` 1, `published_facts_crash_calls.rs` 1, `flow_state_values.rs` 2, `published_facts.rs` 3), so `current_boolean` is used well outside its file.

### Fail-open risks
None seen (`None` is not accepted: `== Some(true)`).

---------------------------------------------------------------------
## 11. `instruction_effects`, prover.rs 6523-7700

### Purpose
Proves a proposition about a sealed instruction effect (an assembly register instruction, or an atomic release store) by matching it to a completed event earlier in the state or to a callee's `ensures` that carries the same effect, comparing the values captured when the instruction ran with the values the query asks about now. Also records receipts so one physical event is not spent twice.

### Shape
- `atomic` (6540-6821): `query_primitive` (6552), `receiver_places` (6574), `captured_places` (6609), `matches_store` (6634), `store_matches` (6650), `guarantee_matches` (6743).
- `operands` (6822-7507): `Capture` (6839), `CaptureFrontier{Operand, NormalReturn}` (6847), `Values` (6853; `captured` 6863, `current` 7005, `equal` 7129), `operand_matches` (7147), `guarantee_matches` (7189), `guarantee_value_matches` (7243), `post_binding_preserved` (7360), `value_place` (7440), `known_integer` (7461).
- `proves_contract` (7510), `proves` (7542), `InstructionEffectEvidence` (7569; `event`, `contract`, `children`, `observation`; `publish` 7576, `observe_at` 7583, `overlaps` 7605), `evidence` (7626), `evidence_inner` (7646, partially in range).

### Inputs and outputs
Reads the fact plan (events are `FactPayload::InstructionEffect` or `CallEnsures` facts), flow rows (`atomic_executions`, operands, calls), the statement execution order. Produces `Option<InstructionEffectEvidence>`; `publish` appends `FlowInstructionEffectReceiptFact` rows to a `PagedArenaSet` (7576-7601).

### How it decides
- Atomic store (`store_matches` 6650-6741): the event point must be a `Statement` in the same machine/state and precede the query statement in the admitted execution order, or the query is the end-of-state index (6672-6680); the authored statement must be a release store (`atomic_effects::release_store`, 6685); the destination operand's referents must equal the queried receiver's places as a set of the same length (6695-6700); the operand expression must equal the carrier's single operand (6719-6727); primitive equality for cell and operand (6728-6735); then value comparison via `operand_matches`.
- Value comparison: both sides are scoped-arithmetic values; "equal" is `scoped_arithmetic_values_relation(..., Equal, captured, current) == Proven` (7137-7144). A captured value is an integer when the live scalar is known, else the origin atom, else a string atom `"\0instruction-capture:{state:?}:{statement}:{expression:?}:{entry_origin_writes:?}:{primitive:?}:{place:?}"` (6952-6977). The current value may reuse a capture's value when the query preserves the place and either same place at the frontier, a copied return, or same origin (7091-7101); otherwise a fresh atom `"\0instruction-current:..."` (7103-7109).
- `guarantee_matches` (atomic 6743, operands 7189): requires `instruction_effect_contract(candidate)` with the matching operation (`AtomicReleaseStore` at 6758, `Register(operation)` at 7205); arity equals the required count (7214).
- `evidence_inner` (7646-): arity must equal the carriers count (`2` for atomic, 7674-7681).
- `overlaps` (7605-7624): receipts overlap only when the two events have the same `point`; then both leaf -> overlap; both with children -> some pair with equal `observation` overlaps; one leaf one with children -> NOT overlapping (7622).

### Cases by shape
- Operation list is closed: `InstructionEffectOperation::Register(op)` and `AtomicReleaseStore` (7670-7673, 7689-7695).
- Atomic release store requires exactly two arguments `[receiver, value]` (6690) and exactly one atomic execution capture row with exactly one operand (6703-6721).
- `known_integer` (7461): with empty projection, only `Name|Member|Integer|Boolean` expressions are read (7488-7496).

### Same job elsewhere
- `operands::Values::current` (7005) and `Values::captured` (6863) build atoms and use `arithmetic::value_origin`, `arithmetic::atom`, `arithmetic::scalar_term` from section 1; same atom-building job as `call_guarantees::arithmetic::at_invocation` (1840) with a different identity prefix.
- `post_binding_preserved` (7360) vs `capture_preserved` (4557): both decide whether a write could have disturbed a captured actual; they use different flow queries (`operand_binding_before_statement_agrees` vs `signature_ceiling_places`). Read both; not compared further.
- Unused/one-caller (grep, count mode, `InstructionEffectEvidence`): 12 occurrences in 2 files (prover.rs 11, `checks_multiplicity.rs` 1). The `pub(in crate::checks)` surface (`evidence`, `publish`, `observe_at`, `overlaps`, `event`, `children`) is therefore used by `checks_multiplicity.rs` (1 occurrence of the type name) outside this file (counted, not read).

### Stated limits
- "no source expression or call initializer is replayed to recover a value" (6527-6529, paraphrase).
- "Generic type arguments retain the exact atomic cell/value pair; a same-sized integer or another cell is not enough." (6543-6544, paraphrase.)

### Fail-open risks
- `matches_store` (6634-6649): `result.unwrap_or(false)` — a `None` inside means "no match", closed.
- `store_matches` (6650): returns `Some(false)`/`None` on every mismatch. Closed.
- `InstructionEffectEvidence.contract: Default::default()` for the atomic-store receipt (7703): the contract handle is the arena's default/invalid handle. Recorded as a placeholder; consumers of `contract` in `publish`/`record` copy it (7599). Whether anything dereferences it was not traced: "open".
- `overlaps` (7615-7623) treats a leaf receipt and a receipt with children over the same event point as non-overlapping. What would pass: one instruction event used to discharge two propositions when one proof routes through a helper's guarantee and the other uses the event directly. Not traced: "open".
- `post_binding_preserved` (7384-7386): `frames.expression_is_fixed_incoming_reference(...)` returns `true` outright. Reason not stated at the site; it is the case of an incoming reference parameter, which this callee cannot rebind (inferred, not traced): "open".

---------------------------------------------------------------------
## Coverage
- Read in full: `checks_contracts_prover.rs` 1701-7700 (6000 lines).
- Skimmed: 0. Not read: 0.
- Also read for context (not counted): 1680-1700 and 7701-7719.
- Grep commands run (Grep tool, count or content mode over `omega-rust` unless noted): `\bStateGuarantees\b` (16 in 6 files); `proves_current|current_predicates|proves_captured_predicates` (15 in 4 files); `scalar_at_place|guarantee_scalar|guarantee_input` (8 in 2 files); `BooleanValueQueries|boolean_value_at_place` (17 in 7 files); `live_contexts_prove_boolean_expression|live_contexts_prove_proposition_application|contexts_prove_proposition_label` (11 in 3 files); `call_result_atom|completed_scalar_value|collection_length_value|length_atom` (14 in 3 files); `captured_place|direct_place|indexed_membership|contract_membership_place_is_accessible` (44 in 10 files); `InstructionEffectEvidence` (12 in 2 files); `\0call-guarantee|\0static-parameter|\0instruction-` (9 in 1 file); `contexts_prove_proposition_label` content mode (5 lines); `proves_current|proves_captured_predicates|scalar_at_place` content mode on `src` (13 lines).

## Five things a maintainer most needs to know about this unit
1. Arithmetic proofs in this range run on string-identified atoms. `format!` of debug-printed handles/places makes the identity (`"\0call-guarantee:..."` 2087/1918, `"\0call-result:..."` 2022, `"\0static-parameter:..."` 2374/3475/4105, `"\0instruction-*"` 6969/7104/7321); equality of those strings is equality of values. All 9 occurrences of those prefixes are in this one file.
2. A callee guarantee reaches a caller only through `availability::admit` (3285): declaration ownership (`owns_guarantee`), independent call site or strict recursive descent (`induction`), stable named inputs, and `capture_preserved` against the call's write ceiling. `StateGuarantees` memoises that decision per statement row in a `Vec` (3087-3207).
3. Requirement matching has two routes: structural predicate equality over the same captured places (`predicates_match` 5193, with an And-on-supplied-side relaxation at 5263) and the strict arithmetic judge (`arithmetic::proves`, 4545), tried in that order in `proves` (4487-4554). `boolean_predicates::proves` (outside the unit) is a third predicate matcher used at 4415/5982.
4. `Callable` (2654) unifies machine, requirement and proposition callees; `Callable::resolve` refuses ambiguity (2691); `decomposed_builtin_meaning` accepts any `And`/`Or` for requirements/propositions (2836-2845).
5. Closed shape lists refuse everything else: `semantic_contexts_prove_contract_fact`'s payload match (6145-6243) can never prove 14 payload kinds; const projections decode only records/arrays/integers (3629-3650); `instruction_effects` supports only `Register(op)` and `AtomicReleaseStore` with exact arities (7670-7681).

## What you are unsure about
- Whether `capture_preserved` (4557) being `true` when the write ceiling is empty is always backed by a complete `signature_ceiling_places` (defined outside the unit).
- Whether `InstructionEffectEvidence::overlaps` (7605-7624) leaving leaf-vs-children receipts non-overlapping can let one event be spent twice; `checks_multiplicity.rs` (outside the unit, 1 occurrence) is the consumer.
- Whether the `contract: Default::default()` placeholder (7703) is ever dereferenced.
- The reasoning behind `post_binding_preserved` accepting fixed incoming references (7384-7386).
- Whether `decomposed_builtin_meaning`'s unconditional And/Or acceptance (2836-2845) is always followed by per-leaf checks; callers were not traced.
- The `captured_place|direct_place|...` count (44 in 10 files) is a name match across files; I did not confirm that every hit outside prover.rs is the same function.
