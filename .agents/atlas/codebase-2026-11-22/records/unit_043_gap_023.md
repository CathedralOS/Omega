# Record: unit 043, second pass, gap_023 (area 07-psi-semantics)

Files (both under `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/`, crate `proof-admission`):
- `integer_rules.rs` lines 209-3386 (read in full)
- `proof.rs` lines 1301-4122 (read in full)

Line numbers below are in those files. Tests are inline `#[cfg(test)]` modules. They are listed but not analysed for the 9 items.

---

## integer_rules.rs (209-3386)

The file is a set of `pub(crate) mod` blocks: `closed_integer` (13), `fixed_integer` (35), `integer_cast` (75), `integer_forbidden_root` (1041), `integer_shift` (1964), `open_terms` (2660), `wrapping_equality` (3057), plus a `#[cfg(test)] value` helper (3381). Lines 1-208 are outside this range. I read 100-210 only for context on the cast witness types.

### integer_cast, tail (209-1039)

1. Purpose. Finishes the cast-chain check: the witness is a list of axiom indices, each saying "this value is an exact cast or widen of the previous value". The chain narrows the root's value interval to what survives every cast. Then it checks that a bound on the root carries across to the target.
2. Shape.
   - `check_integer_cast_chain_witness` (116-223) returns `CheckedIntegerCastChain` (carriers, definition_axioms, `surviving_root_interval`; built 216-222).
   - Helpers `parameter_width_target` (228), `partial_fixed_native_cast` (235), `strict_fixed_native_widen` (246).
   - `IntegerCastChainWitnessError` (254, 14 variants), `check_integer_cast_bound_conversion` (284), `integer_cast_truth_bounds` (364), `IntegerCastBoundConversionError` (385).
3. In/out. Reads the `PropositionContext`, the `semantic_axioms` slice, the witness. Produces the checked chain or an error. No mutation.
4. How it decides.
   - Chain step: keyed on axiom identity and term equality. The operand must equal `current` (166), the source type must equal `current_type` (167), and the target value's type must match (168).
   - Axiom order: indices strictly increasing (129-135).
   - Value reuse: `values.contains(next)` or a root value id appearing inside the root (186) gives `CyclicValue`.
   - Cast kind: `exact_cast` edges must be partial narrowing (`partial_fixed_native_cast`, 172). `IntegerWiden` edges must be strict widening (179).
   - Interval: shrunk by intersecting with each fixed-native target interval (204-205).
   - Bound conversion (284-362) has three paths.
     - Path one: both bounds are mathematical orders on the root and target embedded as `MathValue`, with the same strictness and the same other endpoint (302-321).
     - Path two: scalar `LessOrEqual` bounds where the root-side literal and the target literal must be equal i128 values (332-360).
     - Both go by term equality and literal value, not by spelling.
5. Cases by shape.
   - Parameter-width target (comment 225-227): "An exact conversion from a fixed native carrier into a parameter width: how a literal lands there." Handled as a special case at 192-199; it narrows nothing.
   - Bound conversion path two accepts only `LessOrEqual` (322, 337), and only a typed literal on the other side (333, 355).
   - Carriers must be fixed-native (`fixed_native_interval`, 124-125, 201); address and 128-bit carriers are refused by `UnsupportedCarrier` (tests 689-737).
6. Same job elsewhere.
   - The shift chain (2047) and the correlated forbidden-root branch walker (1276) have the same skeleton: ordered axiom indices, a `current` value, a cycle check, and a target-equals-final check.
   - Three copies of "walk an ordered list of definition axioms" (suspected shared shape; I did not diff them).
   - `integer_cast_truth_bounds` (364) is called at proof.rs:753; `check_integer_cast_bound_conversion` at proof.rs:761.
   - Outside the crate, `check_integer_cast_bound_conversion` is called at `source-to-psi/pipeline/06_.../src/prove.rs:2421` and `prove/ranges.rs:727` (grepped).
7. Unused. `integer_cast_truth_bounds` has 1 caller (proof.rs:753; grepped). `check_integer_cast_bound_conversion` has 3 production callers (proof.rs:761, prove.rs:2421, ranges.rs:727).
8. Stated limits. Comment 189-191: "A parameter width has no interval to narrow by." No unsupported-marker comments.
9. Fail-open. None seen. `.expect("a checked cast chain retains its target carrier")` at 354 and 373 panics, not accepts; the chain always has at least two carriers (126). `surviving_root_interval` becomes `None` when empty (221), and `integer_cast_truth_bounds` then returns `EmptySurvivingInterval` (369), which refuses.

### integer_cast tests (400-1039)

Inline modules `tests` (401) and `expression_roots` (767). 6 tests. Covers narrowing, widening, reorder, cyclic, non-native, and expression roots such as a remainder (`RemainderCast`, 799). The tests pin that bounds can't be strengthened (916-926) and that a missing root-bound premise is refused (891-901).

### integer_forbidden_root (1041-1962)

1. Purpose. Proves that a signed division `dividend / divisor` cannot hit a forbidden divisor (0 or -1 with minimum dividend) when both are affine in one machine parameter root. The root is bounded by two tight literal axioms. It either yields the two bounds back, or `False` when every value in the interval is forbidden.
2. Shape.
   - `check_integer_correlated_forbidden_root_witness` (1124-1236) returns `CheckedIntegerCorrelatedForbiddenRoots` (1058: integer_type, root, dividend, divisor, coefficients, offsets, bound axioms, interval, `forbidden_roots: BTreeSet<i128>`, conclusion).
   - `check_integer_correlated_forbidden_root_conversion` (1238).
   - Private: `check_branch` (1276), `known_landed_literal` (1427), `landed_literal` (1446), `selected_signature_bounds` (1510), `affine_equation_root` (1585), `affine_value` (1606).
   - `IntegerCorrelatedForbiddenRootWitnessError` (1628, 27 variants).
3. In/out. Reads context, semantic axioms, `machine_parameter_values: &BTreeSet<ValueId>` (1127), witness. Produces the checked roots.
4. How it decides.
   - Root eligibility: root must be a `Value` whose id is in `machine_parameter_values`, with a fixed-native signed type (1284-1297). That is a value-identity test.
   - Each branch is walked backwards through ExactAdd, ExactSubtract and ExactMultiply definitions. Each step axiom must be the latest `Equal(current, _)` among earlier axioms (1324-1334), so stale definitions are refused. The other operand must be a literal, either inline or via a prior axiom picked by index (1446-1499).
   - Both branches must share the root, type and be nonconstant (1150-1158). Dividend indices all precede divisor indices (1161-1168).
   - Bounds are chosen by scanning axioms after the definition boundary and keeping the tightest literal bound (1520-1548); the witness's two indices must equal the chosen ones (1178-1182).
   - Forbidden roots: divisor = 0 root, and divisor = -1 root where dividend equals carrier minimum (1185-1197).
   - Conclusion must equal the exact expected conjunction of bounds, or `Falsehood` only when forbidden roots cover the whole interval (1204-1216). Otherwise `PartiallyUnsafeInterval`.
   - The conversion (1238-1274) requires zero forbidden roots and the exact disjunction `d <= -2 or 1 <= d or (d <= -1 and MIN+1 <= n)`.
5. Cases by shape.
   - Only signed native carriers (1293, 1576).
   - Only Add, Subtract and Multiply definitions with a literal other operand (1346-1370).
   - Only integer-affine branches in one root: coefficients and offsets are tracked as one `(coefficient, offset)` pair with i128 checked arithmetic (1400-1406).
   - Only two forbidden-root candidates (0 and -1).
6. Same job elsewhere. Affine-branch recognition resembles `integer_affine.rs` (same crate, outside this range; suspected, not read). The type `IntegerAffineWitnessError` appears in proof.rs:3292 (grepped).
7. Unused. `check_integer_correlated_forbidden_root_conversion` 1 production caller (proof.rs:860). The witness checker is called from proof.rs (`correlated_forbidden_roots_relation`, near 839-892; read only the grep line).
8. Stated limits. Panics `unreachable!("checked correlated roots are signed and of a native width")` (1253) and `.expect("nonempty branch")` (1164-1165). Both rely on earlier checks (1298, 1293).
9. Fail-open.
   - `known_landed_literal` returns `Option` (1427-1444). It is used only to refuse a constant branch path (1371-1378); a `None` lets the branch proceed. Traced: the following `landed_literal` (1379) then demands a literal axiom, so a non-literal sibling is refused (1464-1466). Not fail-open.
   - `selected_signature_bounds` skips non-`LessOrEqual` axioms silently (1525-1527) and axioms whose literal does not tighten. Traced: missing tight bounds give `MissingTightBounds` (1553, 1555). Not fail-open.
   - Otherwise none seen.

### integer_shift (1964-2659)

1. Purpose. Same idea as the cast chain for exact shifts: a witness lists shift-definition axioms from a root value to a target; each shift count must be a literal, or a prior `Equal(count, literal)` axiom, and be below the value width.
2. Shape. `IntegerShiftDirection` (1972), `IntegerShiftStepWitness` (1978), `IntegerShiftChainWitness` (1984), `CheckedIntegerShiftStep` (1991), `CheckedIntegerShiftChain` (2022), `check_integer_shift_chain_witness` (2047), `checked_count` (2176), `IntegerShiftChainWitnessError` (2242, 23 variants).
3. In/out. Reads context, axioms, witness. Produces the checked chain.
4. How it decides. Root must be a `Value` of fixed-native integer type (2052-2062). Definition indices strictly increasing (2066-2072). Each step's definition must be `Equal(Value, ExactIntegerShiftLeft|Right)` with operand equal to `current` and matching types (2080-2123). The count comes from an inline literal (no count axiom allowed, 2185-2189) or from the count axiom, which must precede the definition (2200) and be `Equal(count_term, literal)` (2207-2220). Count must be < width (2144). `values.contains(next)` gives a cycle error (2151).
5. Cases by shape. Only exact (not wrapping or saturating) shifts. Only fixed-native count and value carriers (2058, 2125).
6. Same job elsewhere. Cast chain (above) and the forbidden-root walker repeat the order, operand-equals-current, cycle and target-match checks.
7. Unused. `check_integer_shift_chain_witness` has no non-test caller in this range. Grep over `source-rust` for `check_integer_shift_chain_witness` was not run separately (suspected: used by bounded_denotation.rs, unread here).
8. Stated limits. None.
9. Fail-open. None seen. Unknown count shapes error (2217-2220).

### open_terms (2660-3056)

1. Purpose. Normalizes mathematical integer terms into polynomials (sums of monomials with BigInt coefficients) so two terms can be compared up to ring identities. Optionally works modulo `2^bits` for wrapping arithmetic.
2. Shape. `Monomial = BTreeMap<IntegerMathTerm,u32>` (2674), `NormalForm = BTreeMap<Monomial,BigInt>` (2676), `Normalizer { monomials, modulus }` (2696) with `coefficient`, `charge`, `merge`, `product`, `normalize`. Entry points `constant_difference` (2839), `equal_modulo` (2860), `constant_difference_ordering` (2876). Limit `MAXIMUM_MONOMIALS = 4_096` (2672).
3. In/out. Reads two `IntegerMathTerm`s. Returns `Option<BigInt>`, `bool` or `Option<Ordering>`.
4. How it decides. Add, Subtract and Multiply expand algebraically (2802-2820). Literals are constants. `Embed(scalar)` of an exact term is lifted via `integer_math_normalization::lift_exact_integer_term` and re-normalized (2822-2829). Anything else is evaluated by `ClosedIntegerEvaluator` if closed (2830-2833), else becomes an opaque atom keyed on term equality (2834). Difference is a constant only if the result has 0 entries or exactly the empty monomial (2849-2857).
5. Cases by shape. Ring operations only; shifts and wrapping terms stay opaque (test 2991-3014).
6. Same job elsewhere. `wrapping_equality::equal` (3065) calls `equal_modulo`. `closed_integer::compare_integer_math_terms` (23-31, outside range) calls `constant_difference_ordering`.
7. Unused. Callers outside this module (grepped, excluding tests): `constant_difference_ordering` 1 (integer_rules.rs:31), `equal_modulo` 1 (3085). `constant_difference` is used within the module and tests.
8. Stated limits. The monomial cap returns `ResourceLimitExceeded` (2719), which refuses.
9. Fail-open. None seen. `constant_difference` returning `None` means "not provably constant", and callers read that as "establishes nothing" (test 2970-2988). Traced only for the call at integer_rules.rs:31 (not shown).

### wrapping_equality (3057-3378)

1. Purpose. Decides equality of two wrapping-arithmetic scalar terms of one fixed carrier by projecting them to mathematical terms and comparing modulo `2^bits`.
2. Shape. `equal` (3065), `project` (3088), limits `MAXIMUM_NODES = 8_192`, `MAXIMUM_DEPTH = 128` (3062-3063).
3. In/out. Two `ScalarTerm`s in, `Result<bool, ClosedIntegerEvaluationError>` out.
4. How it decides. Returns `false` unless the left is a non-address integer and the right has the same carrier (3069-3074). `project` accepts only `Value`, in-range `Integer` literal, and Wrapping Add, Subtract and Multiply (3101-3115). Any other term gives `None` and `equal` returns `false`. Equality is then `equal_modulo` (3085).
5. Cases by shape. Only wrapping add, subtract and multiply. Address carriers refused (3072, test 3332-3340).
6. Same job elsewhere. Called from `kernel.rs:68` (grepped; the only production caller).
7. Unused. 1 caller (kernel.rs:68).
8. Stated limits. None.
9. Fail-open. None seen. Every unsupported shape returns `Ok(false)` (3071-3081, 3098, 3114), which refuses the claim. Limit overrun returns `Err` (3095).

### value helper (3380-3386)

`#[cfg(test)] pub(crate) fn value` builds a test `ScalarTerm::Value`. Used by the test modules above.

---

## proof.rs (1301-4122)

Starts inside the tests of `integer_math_normalization` (923-1443; its code is before this range). Then the rule modules.

### integer_math_normalization tests, tail (1301-1443)

Tests only: exact left shift lifts to `ShiftLeft`, wrapping does not (1313-1341). `embedded_endpoints_preserve_projection_and_operation_meaning` (1343-1441) round-trips scalar and mathematical orders, and confirms a 65-deep wrapping chain gives `None` from the lift and an error from `decide_primitive` (1422-1440).

### integer_order_rules (1444-1743)

1. Purpose. Checks the integer order proof rules: subtract order, add order, subtract antitone, discreteness, weakening, `<=` transitivity, strict transitivity, endpoint substitution, mathematical order translation, antisymmetry.
2. Shape. Thin `check_*` wrappers take `(&ProofNode, &mut AcceptanceBuilder)`, use `dispatched_rule!` (macro at 3118) to record the rule kind, and call a pure relation checker. Pure checkers: `order_weakening` (1497), `less_or_equal_transitivity` (1526), `endpoint_substitution` (1597), `math_order_translation` (1663), `order_antisymmetry` (1697). Others live in sibling modules.
3. In/out. Reads premise and conclusion propositions. Writes the rule kind into `acceptance.rules` (macro). Returns `Result<(), ProofError>`.
4. How it decides. Each pure checker compares premise and conclusion terms by structural equality.
   - Weakening goes through `propositions_match_under_integer_math_normalization`, which accepts a scalar or lifted-math spelling (1510-1515).
   - Transitivity requires the middle terms to be equal (1536, 1549).
   - Substitution takes an `endpoint` index of 0 or 1; any other gives `UnknownIntegerOrderEndpoint` (1627). It requires the other endpoint unchanged and strictness equal (1611-1624), and the equality to be between old and new endpoint in either direction (1629-1630). Scalar vs mathematical form is chosen by whether the relation is a mathematical order (1635).
   - Antisymmetry requires a non-address integer carrier (1712) and exact `Equal(left,right)` conclusion (1725-1731).
5. Cases by shape. Antisymmetry only for `LessOrEqual` pairs. Translation only `IntegerMathLessOrEqual` with `Add` on both sides of the conclusion (1673-1679). Weakening only from `Equal` or `LessThan` (1501).
6. Same job elsewhere. The same pure functions are called again from `bounded_denotation.rs` at 8144, 8171, 8195, 8262, 8284, 8301 (grepped). That file elaborates the same rules into kernel terms, so each rule is checked by shared code in two traversals. Callers: `order_weakening` 2 (proof.rs:1523, bounded_denotation.rs:8171). The same count of 2 holds for `less_or_equal_transitivity`, `order_antisymmetry`, `math_order_translation` and `endpoint_substitution` (grepped).
7. Unused. None unused; counts above.
8. Stated limits. None.
9. Fail-open. None seen. The `_ => Err(...)` arms all refuse.

### order_discreteness (1744-1969)

1. Purpose. The rule "`a < b` gives `a + 1 <= b`", plus the literal-endpoint forms `a <= k` iff `a < k+1`.
2. Shape. `check` (1751), `same_carrier` (1809), `adjacent` (1817).
3. In/out. Premise, conclusion in; `Result`.
4. How it decides. Open mathematical form: conclusion `IntegerMathLessOrEqual(Add(left, one), right)` must match a strict premise (itself math, or a scalar `LessThan` lifted by `lift_fixed_integer_relation`) and `one` must be literal 1 (1759-1776). Scalar form: moves one literal endpoint by `adjacent` (1778-1799). `adjacent` returns `None` for non-fixed or address carriers and on overflow (1819, 1823-1826).
5. Cases by shape. Scalar form only with a literal endpoint; comment at 1755-1758 explains.
6. Same job elsewhere. Called at proof.rs:1494 and bounded_denotation.rs:8144.
7. Unused. 2 callers.
8. Stated limits. Test comment 1874-1876: "The strict-to-nonstrict form has no derivation from the adjacency laws".
9. Fail-open. None seen. Overflow gives `None`, which refuses (test 1947-1968).

### propositional_rules (1971-2134)

1. Purpose. The checkers for the basic rules: Primitive, SemanticAxiom, Assumption, conjunction, disjunction, implication.
2. Shape. `check_primitive` (1979), `check_semantic_axiom` (1990), `check_assumption` (2009), conjunction intro and elim (2026, 2047), disjunction intro and elim (2063, 2081), implication intro and elim (2100, 2115).
3. In/out. `RuleScope` carries context, assumptions, axioms, machine parameters (3129). Records the used axiom or assumption through `acceptance.record_*` (2005, 2022).
4. How it decides.
   - Citation: keyed on index into the roster, then the conclusion must match under integer-math normalization (2002, 2019), so a cited premise may be spelled in either scalar or lifted-math form.
   - Primitive: delegates to `decide_primitive` (1986).
   - Conjunction intro: arity and each child conclusion equal (2036-2042).
   - Disjunction elim: the number of branches equals the number of disjuncts and every branch has the same conclusion as the proof (2089-2096). The assumption push per branch happens in `traversal` (2931-2939), not here.
   - Implication elim: the premise conclusion equals the required premise and the proof conclusion equals the consequent (2127-2132).
5. Cases by shape. None beyond the rule forms.
6. Same job elsewhere. Rule traversal appears again in `bounded_denotation.rs` (suspected, not read).
7. Unused. Each `check_*` has 1 caller, the `match` at proof.rs:3193-3253 (read).
8. Stated limits. None.
9. Fail-open. None seen. `DisjunctionElimination` does not check each branch's use of the pushed disjunct. The traversal makes the disjunct available as an assumption, and the leaves still check by index (traversal 2931-2939; test 3446-3480 shows swapped branches rejected).

### power_of_two_order (2135-2418)

1. Purpose. The rule "`c <= k` gives `2^c <= 2^k` for a nonnegative literal `c`" with `2^x` as `1 << x`. Needed for parameter-width ranges.
2. Shape. `power` (2155), `check` (2162).
3. In/out. Premise order and conclusion.
4. How it decides. The premise may be math or a scalar lifted to math (2163-2169). The low endpoint must be an `IntegerLiteral` (2170), not negative (2175). The conclusion must be exactly `power(low) <= power(high)` (2180-2183).
5. Cases by shape. Literal low exponent only.
6. Same job elsewhere. None seen.
7. Unused. Callers outside this file: not grepped for `power_of_two_order::check`. In-file: 1 (proof.rs:3258).
8. Stated limits. Block comment 2145-2146: "The mathematical core does not derive this row; it is admitted provisionally (`AcceptedProofRule::is_provisional`), tested against computed powers." That names a provisional rule: no kernel derivation exists. The test `every_small_exponent_pair_agrees_with_computed_powers` (2307-2329) covers exponents 0 to 127.
9. Fail-open. Stated: the rule is accepted without a core derivation, and the comment says so. The wrong program that could pass would depend on `check` being unsound, and it is only tested for exponents < 128 (2310-2311). Classified "stated". Not traced further.

### strict_order_transitivity (2419-2652)

1. Purpose. `a < b`, `b <= c` (or either strict) gives `a < c`.
2. Shape. `check` (2424), `endpoints` (2458).
3. How it decides. Scalar orders must all share the same integer carrier (2434-2439); math orders have no carrier test. At least one edge must be strict (2464); middle terms equal (2467); outer endpoints equal the conclusion (2470).
4. Cases. Scalar integer or mathematical only; booleans are refused (test 2587-2624).
5. Callers: 1 (proof.rs:1577) plus the traversal. Address carriers are allowed here (test 2628-2650), but refused by antisymmetry (1712). Both are order-based checks that treat addresses differently.
6. Fail-open. None seen.

### subtract_antitone (2653-2808)

1. Purpose. Subtraction is antitone in the subtrahend: from `x - large` and `x - small` with `small < large`, get `(x-large) < (x-small)`. Also an unsigned-saturating variant.
2. Shape. `UnsignedDifference<'a>` (2661), `unsigned_difference` (2668), `check` (2716).
3. How it decides.
   - `unsigned_difference` recognises, for an unsigned fixed non-address result, `Equal(result, ExactSubtract|SaturatingSubtract)` with matching types, or the reflexive `Equal(x,x)` treated as `x - 0` (2681-2706).
   - `check`: if either difference is saturating, requires `order` to be a conjunction `[minuend order, subtrahend order, strict premises...]` (2734-2748). The conclusion is `<=`, or `<` only with one extra strict positivity premise (2752-2765).
   - Otherwise, both must be `ExactIntegerSubtract` on a fixed non-address carrier, with equal minuends, and order exactly `small < large` (2767-2803).
4. Same job elsewhere. `unsigned_difference` is called from `prove.rs:2717,2742,2743` in source-to-psi stage 06 (grepped), plus proof.rs:2723. So it is a public function used by both the proof checker and the source-to-psi prover (3 production call sites).
5. Stated limits. Comment 2658-2660: an exact difference "has the same value as a clamped difference wherever it forms".
6. Fail-open. The reflexive arm (2701-2705, `other if other == result`) accepts any `Equal(x, x)` on an unsigned fixed carrier as a difference from zero. Stated: comment 2660, "A value's reflexive equation supplies the difference from zero." Traced: the resulting subtrahend is a literal 0, and later checks still demand the matching order conjunct (2745-2748), so a wrong program cannot get through that way. Classified "stated".

### subtract_order (2809-2850)

`check` (2814): from `r = l - d` (exact, fixed, non-address) and `0 < d`, concludes `r < l`. Decision is by term equality; zero detection is by literal value (2840-2843). Callers: proof.rs:1461 only. No fail-open seen.

### cites_validated_premise (2852-2867)

Comment: "The caller validates every premise before adding it to these rosters." Returns true only if the node is an `Assumption` or `SemanticAxiom` whose index exists and whose stored proposition equals the node's conclusion exactly (2861-2866). It is used by `traversal::check_node` (2898) and `bounded_denotation.rs:7409` (grepped: 2 call sites). A `true` result skips `context.validate` for that node. Traced: skipped validation is sound only if roster entries were validated; the comment states the caller does that (and tests 3726-3806 show malformed rosters are rejected at entry). An unmatched index gives `false` and the node goes through full validation plus the citation rule, which rejects it. Classified "stated" and "traced" for the test path only; the caller's entry validation is outside this range.

### traversal (2869-3111)

1. Purpose. Walks a proof tree with an explicit work list instead of recursion. `Action` (2874) has `Enter, Post, Children, Cases, Branches, PushAssumption, PopAssumption`.
2. Shape. `check_node` (2884) and `schedule_children` (2952).
3. In/out. Reads context, ambient assumptions, axioms, machine parameter values. Mutates `AcceptanceBuilder`. Local assumptions are pushed and popped as a stack (2941-2945). A debug assertion keeps the length balanced (2943, 2948).
4. How it decides.
   - On `Enter`: validates the node conclusion unless it is an exact citation (2898-2902), then pushes `Post` and schedules children. `Post` runs the local check, so children are checked first.
   - `DisjunctionElimination`: pushes `Cases`, then `Enter(disjunction)`. `Cases` requires a `Disjunction` conclusion and arity equality (2923-2929), then each branch is entered with its disjunct pushed as an assumption (2937-2938) and popped afterwards.
   - `ImplicationIntroduction` pushes the premise as an assumption around the body (3004-3013).
5. Cases by shape. `schedule_children` has an exhaustive `match` over `ProofRule` (2956-3108). There is no wildcard arm, so a new rule is a compile error.
6. Same job elsewhere. Same assumptions-stack discipline appears in `bounded_denotation.rs` (suspected; that file's elaboration references `elaboration.roster` at 7409).
7. Unused. `check_node` 1 caller (`use traversal::check_node`, 3116).
8. Stated limits. None.
9. Fail-open. Rules with no children (`Primitive`, `SemanticAxiom`, `Assumption`, `IntegerFloatExact`, `IntegerCorrelatedForbiddenRoots`; 2957-2961) are leaves. Correct, since they carry no sub-proofs.

### Re-exports, `dispatched_rule!`, `RuleScope` (3113-3134)

`dispatched_rule!` (3118) destructures the rule with `let ... else { unreachable!() }` and inserts the `AcceptedProofRule` kind into `acceptance.rules`. The kind is therefore recorded before the check runs, so a failed check still leaves the kind in a builder that is then discarded on error (inferred: errors return early, the builder is dropped by the caller; not traced).

### check_node_locally (3136-3261)

The central dispatch: exhaustive `match &proof.rule` to checkers (no wildcard). Float rules call `crate::check_integer_float_exact`, `check_float_integer_conversion`, `check_float_meaning_normalization`, `ieee_float_rules::check_chain` (3151-3192). Integer bound rules go to `integer_bound_rules` (3244-3253). `IntegerPowerOfTwoOrder` inserts its rule kind directly and calls `power_of_two_order::check` (3254-3259). Decision is keyed on the `ProofRule` variant; each checker then compares by term equality. Fail-open: none seen.

### ProofError and small helpers (3263-3342)

`ProofError` (3264, 36 variants including `RuleConclusionMismatch(&'static str)` and `RulePremiseMismatch(&'static str)`, whose payload is a static label for messages, not a decision key). `scalar_order` (3326) and `mathematical_order` (3334) split a proposition into (strict, left, right). Both are used widely in this file and in integer_rules.rs (`crate::proof::mathematical_order` at integer_rules.rs:310-311). `assumption_proof`, `semantic_axiom_proof` are test-only (3310-3324).

### Test modules (3344-4122, read in full, not analysed)

- `disjunction_elimination` (3345): arity, common conclusion, branch isolation, assumptions not leaking between cases.
- `order_substitution_tests` (3484): strictness cannot change; endpoint 0/1/2, wrong equality, carrier drift.
- `tests` (3628-end of range 4122): exact citations use the current local premise, malformed rosters rejected, default stack (3808-3854), ignored timing test (3879), fixed-to-math normalization of an assumption, disjunction intro, transitivity middle endpoint checks. The range ends inside `integer_order_transitivity_requires_exact_middle_endpoints_and_relations` (starts 4057).

---

## Coverage

- `integer_rules.rs` 209-3386: read in full (3,178 lines). Skimmed: none. Not read in this pass: lines 1-208 (outside the range, other reader's; I read 100-210 for context only).
- `proof.rs` 1301-4122: read in full (2,822 lines). Skimmed: none.
- Total read in full: 6,000 lines. Not read: none within the unit file.

Counts came from Grep content searches over `source-rust` (function names listed in item 7 above); they are lists of lines, not capped.

## Five things a maintainer most needs to know

1. Every integer rule here is a pure checker over proposition terms, and the same pure functions are called from two places: the `proof.rs` traversal (`check_node_locally`, 3136) and the kernel-term elaboration in `bounded_denotation.rs` (8144-8301). A change to one such function affects both paths.
2. Witness-chain checkers (cast 116, shift 2047, forbidden-root branches 1276) all work by listing axiom indices that must be strictly ordered, and by requiring each definition to be the latest `Equal` for its target; there is no search. The witness picks, and the checker re-verifies.
3. `power_of_two_order` is the one rule here the mathematical core cannot derive (2145-2146, "admitted provisionally"). It is tested only for exponents below 128 (2310).
4. `traversal::check_node` skips `context.validate` for nodes that exactly cite a roster entry (2898, `cites_validated_premise` 2856). That relies on the caller having validated the rosters.
5. `check_node_locally` and `schedule_children` have exhaustive matches with no wildcard, so adding a `ProofRule` variant forces edits in both (3150, 2956). The `wrapping_equality` limits (8,192 nodes, depth 128) and `open_terms` limit (4,096 monomials) turn overruns into `ResourceLimitExceeded` errors, never into acceptance.

## What I am unsure about

- Whether `check_integer_shift_chain_witness` has non-test callers: I did not grep it separately.
- Whether `power_of_two_order::check` has callers outside proof.rs:3258 (bounded_denotation.rs probably elaborates it separately; not read).
- Whether `dispatched_rule!` recording the kind before the check could leak a kind into a returned acceptance on failure. I inferred the builder is discarded on `Err` but did not trace the callers.
- The caller-side validation that makes `cites_validated_premise` sound sits outside my range (accept_certificate and friends, earlier in proof.rs).
- The similarity of the three ordered-axiom-walk checkers is by reading, not by diff.
