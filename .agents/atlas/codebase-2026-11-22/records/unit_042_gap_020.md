# Record: unit 042, second pass, gap_020 (area 07-psi-semantics)

File: `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/bounded_denotation.rs`, lines 9101-15100 (6,000 lines), read in full. The file is one crate module (`proof_admission`, see crate map). All claims below are "read" unless marked "grepped" or "inferred". The range begins in the middle of the `forbidden_roots` module's `Law` statement builder and ends in the middle of the top-level `tests` module.

## Layout of the range (grepped `^\s*mod` for module boundaries)

- 9101-9187: tail of `forbidden_roots::Law` statement construction (module starts at 8729; `chains` submodule at 8750).
- 9189-10238: rest of `impl Denotation` in `forbidden_roots`; 10240-10654 its tests.
- 10656-11134: `mod integer_operations` (with `fixed_scalar` at 10666).
- 11135-11955: `mod multiplication`.
- 11956-12287: `mod order_translation`.
- 12288-12432: `mod source_content` and its `pub use`.
- 12433-14287: `mod subtraction` (tests: `bound_tests` 13080, `order_tests` 13790, `tests` 14129).
- 14289-14595: top-level types and entry points of the file.
- 14597-14771: `mod order_antisymmetry_tests`.
- 14773-15100: start of the top-level `mod tests`.

## 1. `forbidden_roots` tail (9101-10654)

**Purpose.** Builds kernel evidence for the rule `IntegerCorrelatedForbiddenRoots`: signed division `dividend / divisor` is defined (divisor is not 0, and not -1 with dividend at the minimum) when a root value in an interval `[lower, upper]` determines both dividend and divisor through chains of exact add, subtract and multiply steps. The evidence is a case split on the divisor that refutes the divisor-zero case and the divisor-minus-one-with-minimum-dividend case.

**Shape.**
- End of the `Law` roster statements: `QuotientUpperPositive/LowerPositive/LowerNegative/UpperNegative(IntegerType)`, `LessOrEqualAntisymmetry`, `DivisorCaseSplit` (9107-9181). Each is a scheme (a statement built with `scheme_dsl`) that is registered once and then cited.
- Helpers: `integer_le_term` (9189), `integer_numeral` (9199), `integer_id_elim` (9204), `solve_divisor` (9239), `nondivisible_contradiction` (9478), `eval_dividend` (9561), `empty_elimination` (9668), `bounds_contradiction` (9672), `tagged_sum_elim` (9714), `disjunct` (9756), `negative_one_bound` (9787).
- Entry: `correlated_forbidden_roots_evidence` (9940, `pub(super)`), returns `Result<Option<TermHandle>, BoundedDenotationError>`.

**Inputs/outputs.** Reads the witness `IntegerCorrelatedForbiddenRootWitness`, the checked form `CheckedIntegerCorrelatedForbiddenRoots` (root, dividend, divisor, interval, integer type), cited axiom evidence (definition equalities) and cited bound evidence, and the conclusion `Proposition`. Interns terms into `self.arena` and law constants into the `forbidden_roots.laws` table. Returns a kernel term proving the three-way disjunction (`divisor <= -2` or `1 <= divisor` or `divisor <= -1 and minimum+1 <= dividend`).

**How it decides.**
- Gate on type: only `IntegerCarrier::Fixed` and `IntegerSign::Signed`, else `Ok(None)` (9949-9953).
- It rebuilds the expected disjunction from the checked data and requires structural equality with the denoted conclusion, else `Ok(None)` (9042-10051 region: `expected` built 10038-10047, compared 10049).
- Chain shape is checked by term identity (`structurally_equal`) between consecutive steps and the root and target (9983-10000). Evidence lists must be consumed exactly: `cursor != axiom_evidence.len()` or an empty chain gives `None` (9977-9981).
- Bound evidence must be exactly two items, and each must be `IntegerRelation::LessOrEqual` between the numeral for the interval end and the root term (10002-10031).
- `solve_divisor` walks the divisor chain backwards solving `divisor = c` for the root with `checked_sub/add/div`; overflow, zero multiplier or a failed numeral equation yields `None` (9288, 9320, 9352-9357). A non-divisible quotient yields a `Refuted` outcome via `nondivisible_contradiction` (9444-9460). Final check: end of chain structurally equals root (9468).
- `negative_one_bound` decides by value: refutes if the quotient is outside `[lower, upper]`; otherwise evaluates the dividend and refuses (`None`) when it equals the type minimum (9845-9847); in-range gives `minimum+1 <= dividend` (9849-9873); out of carrier range refutes through `carrier_bound` (9875-9934).
- Judgments are keyed on term identity, numeric value and interval endpoints, not source spelling.

**Cases by shape.** Hard shape: the result must be the exactly three-way nested tagged sum with the order `[d <= -2] | [1 <= d] | [d <= -1 and min+1 <= dividend]` (10038-10047). Fixed three-case split on divisor (zero, minus one, rest) at 10145-10231 via `Law::DivisorCaseSplit` (9145-9181). Signed fixed carrier only (9949).

**Same job elsewhere.** The chain-step machinery (`chain_steps`, `ChainStep`, `chain_step_lift`, `chain_sibling_congruence`) is in the `chains` submodule at 8750 (outside my range; suspected shared with other affine rules, not read in this pass). The numeral helpers `numeral_sum`, `numeral_difference`, `numeral_product`, `numeral_quotient` are the same job as the other `numeral_*` helpers in `addition` (line 40+) and `multiplication`/`subtraction` here; `numeral_binary_operation` (11100) is the common body for sum, difference and product (11312, 13002); `numeral_quotient` (11334) has its own copy of the keyed-assumption logic.

**Unused / one-caller.** `correlated_forbidden_roots_evidence` has one non-test call site, at 8555 (grepped, whole file). `solve_divisor` is called twice in this function (10077, 10115).

**Stated limits.** None in this range beyond the `Option` declines. The caller at 8573 has the comment "Provisional: the core has no power of two to derive it from" for a different rule.

**Fail-open risks.** When `correlated_forbidden_roots_evidence` returns `None`, the caller at 8555-8571 falls back to `self.rule_instance(AcceptedProofRule::IntegerCorrelatedForbiddenRoots, premises, evidence, &proof.conclusion)`, which introduces the rule's statement as an assumed instance (the tests assert `rule_axioms.is_empty()` only on the derived path, e.g. 10470-10474). Any input where this derivation declines (non-fixed or unsigned type, divisor chain overflow, multiplier 0, evaluated dividend equal to minimum, chain shape mismatch) is therefore accepted on the strength of the earlier checker (`crate::proof` check run before elaboration), not re-derived in the kernel. Classification: "traced" for the fallback path (read 8540-8572); the checker that runs before it was not read in this pass.

**Tests (10240-10654).** Positive: safe fixture i8 (positive multiplier), negative multiplier, out-of-interval root refutes case. Negative: mutated conclusion gives `ConclusionMismatch` (10614-10629), mutated witness gives an error (10641-10652). `check` (10454) asserts the derivation uses laws, `rule_axioms` and `decisions` are empty.

## 2. `integer_operations` (10656-11134)

**Purpose.** Gives every fixed-width integer operation (bitwise, widen, cast, shifts, exact and wrapping and saturating arithmetic, comparisons) an uninterpreted kernel constant with a function type, so equalities between such terms must be proved, not assumed definitional.

**Shape.**
- `IntegerOperation` enum, 24 variants, each carrying the `IntegerType`(s) (10893-10937); `arity` is 1 for `BitwiseNot`, `Widen`, `ExactCast`, else 2 (10940-10945).
- `integer_operation_table!` macro and `application(term)` (10948-10995): table from `ScalarTerm` variants to `(IntegerOperation, left, Option<right>)`; unmatched variants return `None` (macro `_ => return None`, 10954).
- `Denotation::integer_operation` (10998) interns the constant once per operation, via `interned_keyed_assumption` on `integer_operations`; result type is `two` for `Equal/LessThan/LessOrEqual` else the integer carrier (11006-11015). `apply_integer_operation` (11029), `integer_relation_term` (11043), `binary_integer_operation` (11072), `binary_terms` (11089), `numeral_binary_operation` (11100).
- `fixed_scalar::denote` (10694): iterative (explicit `Pending` stack, no recursion) denotation of a `ScalarTerm` into a kernel term.

**How it decides.** Keyed on the full `ScalarTerm` variant plus its type parameters. `fixed_scalar::denote` order: integer literal first (10701), then `Value` with integer type (10705), then exact add/sub/mul (10712), then any `application` row (10727), and otherwise an interned opaque assumption of the carrier type keyed by the cloned term (10740-10745). Different policy (wrapping vs saturating), different widening targets and different shift-count carriers each give different constants because the type parameters are in the `IntegerOperation` key; the test at 10789-10888 checks that these pairs are not definitionally equal.
`numeral_binary_operation` computes the claimed result in `BigInt` and returns `Ok(None)` if the claimed result differs (11116-11120); otherwise interns an `Id` assumption `op(left,right) = result` keyed `(left, right, result)` (11121-11130).

**Cases by shape.** 24 listed operations, the macro table at 10966-10989 is the fixed list. A `ScalarTerm` value outside both the literal/value/exact-arithmetic/table cases becomes an opaque constant (10740) rather than an error.

**Same job elsewhere.** `application(...)`-style tables from `ScalarTerm` to operation also appear in other modules (suspected, not read). `numeral_binary_operation` is the common body for sum/difference/product (see 11321, 13011); `numeral_sum` lives in `addition` (outside range).

**Unused / one-caller.** I did not grep individual helpers beyond those named in section 5 below.

**Fail-open risks.**
- `fixed_scalar::denote` 10740-10745: any unrecognised `ScalarTerm` (not a literal, value, exact arithmetic or table row) is denoted as an interned opaque `assumption` constant of the integer carrier type. This accepts such a term as a fresh, unconstrained integer; it cannot prove anything about it, since identical terms share a constant and nothing equates it with other terms. "inferred": a wrong program cannot get a false equality through this path because no axiom is attached, but it also does not get refused.
- `numeral_binary_operation` creates an assumption stating `op(left,right)=result` (11122-11130) after verifying the numeric fact with `BigInt` (11116). The assumption is true by arithmetic, so accepting is by construction; "stated"? no comment at site; classified "traced": the guard is on the line above.

## 3. `multiplication` (11135-11955)

**Purpose.** Supplies the multiplication constant and laws used to prove bounds on an exact product when one factor's sign and the other's bound are known, including the case where the bound root is a quotient (the `x * (e / r)` shape).

**Shape.** `Law` enum: `MonotonePositive`, `AntitoneNegative`, `DivideLowerPositive/LowerNegative/UpperPositive/UpperNegative(IntegerType)` (11151-11158). `Multiplication` struct with `operation`, `laws`, `numeral_products`, `numeral_quotients` (11161-11167). Functions: `multiply_operation` (11170), `multiply_terms` (11174), `multiply_law_application` (11182), `integer_le_numeral` (11269), `numeral_product` (11312), `numeral_quotient` (11334), `correlated_multiply_bound_evidence` (11374).

**How it decides (`correlated_multiply_bound_evidence`).** Returns `Ok(None)` unless all hold: the witness target is an exact multiply on a fixed carrier (11382-11392); the conclusion is a math `<=` with the product on one side and a literal bound on the other (11393-11401); the literal equals the type minimum (lower) or maximum (upper) as big integers (11402-11413); the premise is exactly a two-part conjunction `[sign, bound]` (11414-11419); the sign premise is `1 <= right` or `right <= -2` (11422-11432); the bound premise matches the expected root premise (11433-11440); the root expression (through at most one definition axiom, via `affine_witness_root_expression`, 14343) is `endpoint / right` of the same type and divisor (11441-11456); the endpoint is the carrier endpoint, directly or via a definition (`affine_carrier_endpoint_equality`, 14369); and the denoted product is structurally equal to `left * right` (11474-11478). Comparison is by `Proposition` equality and term structure.

**Cases by shape.** Four sign/endpoint combinations listed by `match (lower, positive)` (11553-11558). Comment at 11420-11421: "`i1` holds only -1 and 0, so neither sign premise can be written in it and the rule declines there". At most one definition axiom (14349) and no literal-axiom chains of other length: `affine_witness_root_expression` returns `None` unless the axiom lists are both empty or `definition_axioms.len() == 1` (14347-14353).

**Same job elsewhere.** The subtraction counterpart `correlated_subtract_bound_evidence` (12855) and add counterpart `correlated_add_bound_evidence` (suspected, in `addition`, not read) follow the same structure: shape gates, `affine_witness_root_expression`, `affine_carrier_endpoint_equality`, `affine_root_equality_step`, then a law application.

**Unused / one-caller.** `correlated_multiply_bound_evidence`: 2 call sites in the file, 8376 (production) and 11932 (test) (grepped).

**Stated limits.** 11420-11421 (i1). Test comment 11901-11904: "The rule used to build that literal with `expect` and crashed the compiler; it must decline instead".

**Fail-open risks.** Same as section 1: every `Ok(None)` from the correlated-bound chain at 8343-8403 falls through to the later generic path at 8404 (not read in this pass); test `direct_subtract_bound_out_of_numeral_range_keeps_instance_fallback` (13468-13498) confirms that on decline a single `rule_axioms` instance is recorded. "traced" for the 8343-8403 chain, "open" for what the generic path at 8404+ does.

**Tests (11639-11954).** Rows via `fixture(...)` for lower/upper, positive/negative, landed endpoint, divide-root; negative twins for mutated premise and goal (11846-11899); i1 declines and i2 proves (11906-11953).

## 4. `order_translation` (11956-12287)

**Purpose.** Proves "a <= b implies a + c <= b + c" for mathematical (unbounded) integer terms, and rejoins the result with closed numeral terms when `a + c` is a literal sum.

**Shape.** `mathematical_order_translation` (11961) and `translated_numeral_equality` (12008). Entry reads `relation` (must be `Proposition::IntegerMathLessOrEqual`, else `RulePremiseMismatch`, 11967-11973), denotes left, right and addend, applies `add_order_translation` (defined elsewhere, outside range; grepped name in this file: called at 11978), then for each side whose raw sum differs structurally from the normalised result, substitutes using a numeral-sum equality (11989-12004).

**How it decides.** Structural equality of kernel terms (11989, 11997). `translated_numeral_equality` needs all three terms to be closed math values that fit `IntegerValue` (`integer_value_of`, 14517); otherwise returns `BoundedDenotationError::Unsupported("translated numeral exceeds existing numeral evidence")` (12022-12025), or `Unsupported("translated numeral lacks exact addition evidence")` when `numeral_sum` returns `None` (12027-12029). These are refusals, not accepts.

**Same job elsewhere.** `numeral_sum` use matches 12956 (subtraction) and 11970 style; `add_order_translation` in `addition` (suspected, not read).

**Unused / one-caller.** `mathematical_order_translation`: 1 production call at 8288 (grepped).

**Fail-open risks.** None seen. Both unsupported branches return `Err`.

**Tests (12033-12286).** Include a deep-addend test (16,384 nested adds, 12259-12285) asserting `IntegerOrderConclusionMismatch` and that the checker does not clone the addend; strictness, unknown value (id 99) and wrong carrier drift all must error; exact-finite-ring test (12191-12256) checks that `WrappingIntegerAdd` and `SaturatingIntegerAdd` conclusions are not lifted (`lift_exact_integer_order(...).is_none()`, 12249).

## 5. `source_content` (12288-12432)

**Purpose.** A small public wrapper that lets other crates ask for an opaque canonical content-algebra carrier (counted quantity, interval set) in a caller-owned arena, using the same carrier declaration as checked content derivations. Comment at 12290-12294: a carrier declaration "grants no conservation theorem or resource authority".

**Shape.** `pub struct SourceContentTheory { state: Denotation }` (12301); `new(arena, declarations)` (12306) takes the caller's arena and declaration list into a private `Denotation` by `mem::take` then swaps back; `carrier(...)` (12313) swaps the caller's arena in, calls `content_carrier_at(kind, parameter, level)` (defined at 4520, outside the range), swaps out.

**How it decides.** No judgment here; delegates to `content_carrier_at`. Handles from earlier calls remain valid only for the same arena/declaration pair (doc comment 12299-12300).

**Unused / one-caller.** `SourceContentTheory`: outside its file it appears once, the re-export at `bounded.rs:428`; I did not find any other use of the name under `source-rust/` (grepped: 5 matches total, 4 in this file, 1 in `bounded.rs`). It may be consumed by crates reached through re-export; "suspected" that external crates use it through the `bounded.rs` export, as the grep covered `source-rust/` only and the re-export chain was not followed. Its only in-file use is the test at 12342.

**Fail-open risks.** None seen. The test (12338-12429) asserts that a `CountedQuantity` family substituted at `Two` is convertible to the direct carrier, and that `CountedQuantity` is not convertible to `IntervalSet`.

## 6. `subtraction` (12433-14287)

**Purpose.** Supplies subtraction laws and the kernel evidence for subtraction-related rules: strict order from a positive subtrahend (`IntegerSubtractOrder`), order from an addition (`IntegerAddOrder`), antitonicity of subtraction (`IntegerSubtractAntitone`), unsigned `0 <= a - b` from `b <= a`, bounds on `a - b` from bounds on a root, and the "exact subtract definition bound".

**Shape.**
- `Law` enum, 7 variants: `Zero`, `Antitone`, `SelfZero`, `LessOrEqualAntitone`, `CancelAddLeft`, `CancelAddRight`, `MonotoneAntitone` (12449-12457); `Subtraction` state with `operation`, `laws`, `irreflexivity`, `numeral_differences` (12460-12465).
- `subtract_operation` (12468), `subtract_terms` (12472), `subtract_law_application` (12480), `irreflexive` (12572), `subtraction_evidence` (12601), `addition_order_evidence` (12673), `subtraction_antitone_evidence` (12716), `unsigned_subtract_bound_evidence` (12772), `correlated_subtract_bound_evidence` (12855), `numeral_difference` (13002), `direct_subtract_bound_evidence` (13024), `exact_subtract_definition_bound_evidence` (13041), `Subtraction::bound_operation` (13065, builds a `BinaryBoundOperation` table of function pointers used by shared `direct_binary_bound_evidence` and `exact_binary_definition_bound_evidence`, defined elsewhere).

**How it decides.**
- `subtraction_evidence` (12601): `unreachable!` if the proposition is not `Equal(_, ExactIntegerSubtract)` (12613) because "bounded subtraction checker ran first". If the subtraction evaluates to a literal and the decrement is non-positive, it builds a contradiction from the positivity premise (12620-12641); with a positive decrement it uses `literal_order(value, original)` (12645). Open terms use the `Antitone` and `Zero` laws (12647-12662). Keyed on the literal value and sign of the decrement.
- `addition_order_evidence` (12673): `Ok(None)` when the addition is a closed literal (12686), so closed sums keep the checked rule instance (test 13927-13933).
- `subtraction_antitone_evidence` (12716): only when both propositions are exact subtraction (12724-12743; comment 12739-12742 "The exact-order theorem below does not apply to clamped subtraction"); `Ok(None)` if either side evaluates to a literal (12744). Saturating subtraction therefore keeps the checked rule instance (tests 13975-14027 and 14030-14086).
- `unsigned_subtract_bound_evidence` (12772): `None` unless target is exact subtract on an unsigned fixed carrier, no definition or literal axioms, root equals the right operand, `left != right`, both are `Value` or `Integer` terms and the premise is exactly `right <= left` (12787-12801). When both are literals and `left < right` it builds a refutation (12818-12832), else a numeral proof (12833-12840); open terms use `LessOrEqualAntitone` plus `SelfZero` (12842-12852). `unreachable!("checked unsigned subtraction bound denotes IntLe")` at 12811.
- `correlated_subtract_bound_evidence` (12855): gates on fixed carrier, `math_less_or_equal_sides` with `is_math_subtract`; evaluates bound and difference with `ClosedIntegerEvaluator` (12880-12889); if both closed uses `closed_order_bound_evidence` (12891-12906); if only the difference is closed it declines (12907-12909); otherwise needs the root expression to be `endpoint + right` of the same type with the expected premise, and the carrier endpoint (12910-12930), then applies `CancelAddLeft` or `CancelAddRight` (12981-12991).

**Cases by shape.** Same style as multiplication; additionally, the unsigned direct path requires both operands to be a bare value or literal (12787-12789).

**Same job elsewhere.** `correlated_subtract_bound_evidence` (12855) and `correlated_multiply_bound_evidence` (11374) share the gate sequence and helper calls. `direct_subtract_bound_evidence` and `exact_subtract_definition_bound_evidence` delegate to the shared binary-bound code (13031, 13051), presumably the same one used by addition (suspected, not read). `affine_endpoint_equality_order` (called 12992) is defined outside this range (grepped within the call site only).

**Unused / one-caller (grepped across the file for each name).** `correlated_subtract_bound_evidence`: production call 8356, definition 12855. `unsigned_subtract_bound_evidence`: 8343. `subtraction_evidence`: 8078. `addition_order_evidence`: 8091. `subtraction_antitone_evidence`: 8122. Each has one production call.

**Stated limits.** 12739-12742 (clamped subtraction not covered). Test message 13496: "an unrepresentable endpoint difference keeps the explicit instance assumption".

**Fail-open risks.** Same fallback as section 1: when a derivation returns `None` (closed addition order, saturating antitone, out-of-range endpoint difference, closed difference with open bound), the rule is accepted via the checked rule instance (`rule_axioms` non-empty). The tests pin this: `closed_addition_order_keeps_the_checked_rule_instance` 13927, `closed_subtraction_antitonicity_keeps_the_checked_rule_instance` 13965, 13468. "stated" by test names and the comment at 12739-12742. `unreachable!` at 12613 and 12683 would panic if the checker invariant were broken; "traced" only to the call at 8078, 8091.

**Tests (13079-14287).** Cover unsigned bounds across literals and values (13177-13199), correlated lower/upper (13201-13350), closed goal (13352), mismatched-certificate rejections (13500-13570, 13717-13786), saturating distance (14030-14086), conditional subtraction at widths up to 128 bits (14210-14260).

## 7. Top-level types and entry points (14289-14595)

**Purpose.** Result type, error type, small shared helpers and the public entry points that turn a bounded certificate into a kernel certificate and optionally verify it.

**Shape.**
- `BoundedDenotation { arena, certificate, rules, assumptions, semantic_axioms }` (14289) with `receipt()` (14298) summing sizes (`declarations`, `assumption_closure`, `context_depth`, `arena_slots`), saturating to `u32::MAX`.
- `BoundedDenotationError { Certificate(ProofError), Unsupported(&'static str), Kernel(CoreError), DepthLimitExceeded }` (14311).
- `propositions_match` (14323) delegates to `propositions_match_under_integer_math_normalization`.
- `affine_equality_for` (14327), `affine_witness_root_expression` (14343), `affine_root_premise` (14357), `affine_carrier_endpoint_equality` (14369), `math_literal_value` (14392), `unbounded_integer` (14406), `integer_value_of` (14517).
- `denote_bounded_certificate` (14413, pub), `denote_bounded_certificate_with_machine_parameters` (14430), `verify_bounded_certificate` (14456, pub), `verify_bounded_certificate_with_machine_parameters` (14475, pub(crate)).
- `IntegerLaw` enum, 10 order laws (14498-14509); `MathTermKey { Closed(BigInt), Open(IntegerMathTerm) }` (14512); `IntegerRelation` (14530); `integer_law_statement` (14537).

**Inputs/outputs.** `denote_...` builds an `Elaboration::new(context, goal, assumptions, semantic_axioms, machine_parameters)`, walks the proof (`elaboration.node(proof)`), checks `proof.conclusion == *goal` (14448-14452) and returns `elaboration.finish(term)`. `verify_...` then calls `verify_mathematical_certificate(&mut arena, &certificate, budget)` and maps failures to `Kernel` (14492-14493). The denotation alone is not a verdict; only `verify_...` runs the kernel.

**How it decides.** The goal check at 14448 is a plain `Proposition` equality, run after the walk. `affine_equality_for` (14327) searches the cited definitions list and takes the first equality with the subject on either side, returning the other side; the match is by `ScalarTerm` equality, in list order. `affine_witness_root_expression` returns `None` unless there are zero axioms (then the root itself), or exactly one definition axiom (14347-14353). `math_literal_value` maps i128::MIN magnitude specially (14394-14396). `integer_value_of` returns `None` for a magnitude that does not fit, via `binary_numerals::fixed_magnitude`.

**Unused / one-caller (grepped, excluding this file).** `denote_bounded_certificate|verify_bounded_certificate`: 16 matches in 5 files: `bounded.rs` 2, `proof.rs` 10, `tests/theorem_certificate_admission.rs` 2, two test files under `semantics/src/descriptors/...` 1 each (command: Grep count, `glob !**/bounded_denotation.rs`). `affine_carrier_endpoint_equality` `.expect("carrier endpoint is representable")` at 14380-14381 panics if `ScalarTerm::integer` rejects the type's own min or max; "inferred": cannot occur for a type with `minimum_value()` or `maximum_value()` returning `Some`.

**Fail-open risks.** `affine_witness_root_expression` returning `None` for more than one definition axiom causes callers to decline (not accept). The conclusion check at 14448 runs after the walk, so a mismatched conclusion is refused. None seen beyond the fallback already described.

**Stated limits.** None.

## 8. `order_antisymmetry_tests` and start of `tests` (14597-15100)

Tests only. 14597-14771: antisymmetry through the existing core law (no rule instance), rejections for drift in strictness, endpoint and scope, no relabelling of carriers or policies (`WrappingIntegerAdd` accepted as an opaque value but rejected if relabelled `ExactIntegerAdd`, 14718-14744), orientation of literal and reversed equalities. 14773-15100: shared-prefix conjunction (14810-14883), contract entailment as a kernel judgment (14912-14952), discharged hypotheses (14955-15015), equality rules become identity eliminations (15018-15073), order rules become named law citations (15076-15100+).

## Coverage

- Read in full: `bounded_denotation.rs` lines 9101-15100 (6,000 lines).
- Skimmed: none.
- Not read: none of the unit's ranges. Outside the range I read only 8336-8405 and 8540-8590 to see the callers and fallback; I did not read the checker before elaboration, `chains` (8750-9100), `add_order_translation`, `correlated_add_bound_evidence`, `direct_binary_bound_evidence` or `closed_order_bound_evidence`.

## Five things a maintainer most needs to know about this unit

1. Every derivation here is optional. When it returns `Ok(None)` the caller falls back to `rule_instance`, which adds the rule's statement as an assumption after the separate rule checker has passed (8555-8571; tests assert `rule_axioms` empty or non-empty to pin which path a case takes, e.g. 13496, 13927, 13965).
2. Declines are keyed on shape and numeric facts: fixed signed carrier for forbidden roots (9949), exactly one definition axiom (14347-14353), closed versus open terms (12686, 12744, 12907), and `i1` (11420).
3. Integer operations are uninterpreted kernel constants keyed by operation plus integer types (10893-10937, 10998), so wrapping and saturating variants, different widen targets and shift-count carriers are never definitionally equal (test 10789).
4. Numeric facts come from `BigInt` checks before an assumption `op(a,b)=c` is interned (11116, 11344-11350); `numeral_binary_operation` is shared for sum, difference and product, but `numeral_quotient` has its own copy.
5. The file ends the module section with entry points: `denote_bounded_certificate` only builds evidence; the kernel verdict comes from `verify_bounded_certificate` (14456-14495), and it checks `proof.conclusion == *goal` after the walk (14448).

## What I am unsure about

- Whether the checker run before elaboration at 8330-8342 really rejects every input that the elaboration fallback then accepts as an assumed instance; I did not read the checker.
- Whether anything outside `source-rust/` consumes `SourceContentTheory` (grep found only the `bounded.rs:428` re-export).
- The `unreachable!` sites at 12613, 12683 and 12811 depend on the checker running first; I confirmed the call order only at the production call sites 8078 and 8091, not for 12811.
- Line 9042-10051 style references in section 1 are approximate in one place: the `expected` term is built at 10038-10047 and compared at 10049; the stray "9042" in the how-it-decides list is a typo for 10038.
