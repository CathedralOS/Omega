Second pass for unit 74

Ranges covered (paths under `<checkout>/`):
- omega-rust/psi/compilation/proof/admission/src/integer_affine.rs 3301-6415 (3115 lines)
- omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs 521-1059 (539 lines)

Both ranges are test code only. Neither contains a non-test `pub` item, so sections 7 (unused code), 8 (stated limits) and 9 (fail-open) are mostly "none seen". I did not read the earlier reader's notes; I saw no need to compare.

All paths below are relative to omega-rust/psi/compilation/proof/admission/src/.

---------------------------------------------------------------------
## File 1: integer_affine.rs lines 3301-6415
---------------------------------------------------------------------

### Where the range starts
Line 3269 opens the file's top-level `#[cfg(test)] mod tests` (read via grep of `mod ` / `#[cfg(test)]`, integer_affine.rs:3269-3270). Inside it, `mod bound_mapping` begins at 3271, its `macro_rules! check` at 3289-3293 (forwards to `check_integer_affine_bound_conversion`), and `mod correlated` at 3295. Line 3301 falls inside the first `math_value!` macro body of `mod correlated` (macro spans 3298-3305). So the range starts in the middle of the macro definition in `tests::bound_mapping::correlated`. The range ends at 6415, the closing brace of `mod tests`.

### Purpose
This is the unit-test half of the integer affine proof rule. The code under test (earlier in the file, not in my range) takes a witness (`IntegerAffineWitness`: root, target, definition_axioms, literal_axioms) and checks that a chain of integer definitions is an affine/monotone map from root to target (`check_integer_affine_witness`). It also maps a bound on the root into a bound on the target (`map_integer_affine_bound`, `check_integer_affine_bound_conversion`), reports extra "wrapping evidence" needed when a wrapping operation could cross the carrier limits (`integer_affine_wrapping_evidence`), and produces truth bounds (`integer_affine_truth_bounds`). The tests pin the exact accepted and rejected shapes, including exhaustive 8-bit sweeps.

### Shape (test modules and tests, by line)
Submodule `tests::bound_mapping::correlated` (3295-3702), local macro `math_value!` (3298-3305):
- `correlated_add_bound_replays_exact_complement_identity_and_relation` 3307-3489. Builds an exact add target `left+right`, a "correlated" value defined as `endpoint - right` (axiom), and checks the mapped proposition is an `IntegerMathLessOrEqual` over `IntegerMathTerm::Add` of two `MathValue`s against the literal endpoint (127 upper, -128 lower). Negatives: evidence pointing to a redirected value gives `DirectAddEvidenceMismatch` (3360-3366); redirecting the complement operand or dropping definition custody makes the witness check fail (3367-3396). A second half (3399-3488) lands the endpoint as its own prior `Equal` axiom and tries six mutations (omitted landing, reordered axioms, wrong literal 126, boolean instead of integer, redirected endpoint value, redirected complement operand); all must error.
- `correlated_subtract_replays_complement_definition_and_endpoint_landing` 3491-3555. Same pattern for subtract with MIN landing and complement `endpoint + right`; also checks that reversing the authored guard errors (3551-3554).
- `correlated_negative_multiply_uses_target_endpoint_and_sign_orientation` 3557-3701. Multiply by a negative right factor (`right <= -2`, 3567) with correlated quotient `endpoint / right`. Checks lower uses left<=MIN/right and upper uses MAX/right<=left (comment at 3626). Swapped comparison yields `DirectMultiplyEvidenceMismatch` (3625). Landed endpoint variant with four mutations (3657-3699).

Submodule `tests::bound_mapping::direct` (3703-4224), local macros `math_value!` (3706-3713) and `le!` (3715-3719):
- `direct_add_bound_replays_both_ordered_endpoints_and_rejects_mutations` 3721-3914. Direct (no definition axioms) exact add witness. Accepts `Conjunction([lower-left, upper-right...])` in a fixed order. Also accepts `Equal` for one side and `Truth` placeholders (3770-3799). Error cases: `Truth, Truth` -> `DirectAddEvidenceMismatch` (3800-3806); four malformed conjunctions (3856-3878); i128 overflow of MIN+MIN -> `DirectAddBoundOverflow` (3904-3913). Embedded-literal variants `left+7` and `7+right` (3808-3854).
- `direct_subtract_bound_replays_opposite_ordered_endpoints_and_rejects_mutations` 3916-3999. Lower uses (left lower, right upper) -> -120; upper accepted. Mixed orientation, reordered evidence and redirected operands all error (3955-3976). Exact MAX minuend is accepted (3977-3987); an exact non-carrier-endpoint minuend (100) is rejected (3988-3998).
- `direct_multiply_replays_four_corners_and_rejects_order_mutations` 4001-4063. Four-part conjunction (left lo, left hi, right lo, right hi); (-4..5)x(-3..2) lower = -15. Three malformed shapes error (4042-4062, the third has only three parts).
- `direct_multiply_requires_oriented_zero_evidence_for_exact_zero_bounds` 4065-4125. Zero bound needs right<=0<=right style oriented evidence; an `Equal(right,1)` pair is rejected as "unoriented nonzero" (4117-4124).
- `direct_shift_bound_replays_count_range_and_sign_oriented_endpoint` 4127-4223. Shift-left with a count range [0,3]; four root-bound cases produce -128, 120, 2, -2 endpoints (4172-4201). Missing count lower -> `DirectShiftCountLowerMissing` (4210); count upper 8 (>= value width) -> `DirectShiftCountOutsideValueWidth` (4221).

Tests sitting directly in `tests::bound_mapping` after `mod direct` (4226-4812). Line 4226-4227 has `#[test]` followed by a blank line before the `fn`:
- `unsigned_remainder_intersects_dividend_bounds_and_accepts_legacy_images` 4228-4294. u32 `root % 256` via a definition axiom. Bound table (0->0, 57->57, 255->255, 256->255, u32::MAX->255). Also checks the "previous total-image certificate" `target <= 255` still converts (4266-4270, message at 4269). Wrong conclusions 56 and 58 -> `ConclusionMismatch`, comment "only the tight or legacy mapping is admitted; relaxation needs evidence" (4280). `LessThan` -> `StrictBoundNotTranslation` (4291-4292).
- `signed_remainder_keeps_truncation_bounds_for_both_divisor_signs` 4296-4343. i16, divisors -256 and 256. Table gives e.g. upper 57 -> 255 and lower -57 -> -255.
- `maps_upper_and_lower_bounds_across_every_coefficient_sign` 4345-4396. Builds `CheckedIntegerAffineForm` by hand with fields root, target, integer_type, coefficient, offset, endpoint_steps (4351-4361) and checks coefficient signs 2, -2, 0; zero coefficient produces truth bounds `[5<=target, target<=5]` (4389-4395).
- `affine_bound_mapping_rejects_shape_direction_and_arithmetic_drift` 4398-4479. Hand-built form (coef 2 offset 1). Errors checked: `RootBoundNotLessOrEqual` (4421), `RootBoundMismatch` (4429), `RootBoundNotTypedLiteral` (4437), `ConclusionMismatch` (4445), `MappedBoundOverflow` with i128::MAX steps (4459), `MappedBoundOutsideCarrier` (4477).
- `maps_mixed_exact_shift_endpoints_and_rejects_witness_tamper` 4481-4572. Four-axiom chain over u16: landed i8 count 1, shift-left by that count, shift-right by u16 literal 2, shift-left by i32 literal 3. `root<=63` maps to `target<=248` (4543-4549). Reordered definition axioms -> `NonCanonicalDefinitionOrder` (4559); dropping the count landing -> `ShiftCountNotLanded(1)` (4570).
- `bitwise_and_mask_maps_every_root_endpoint_to_the_mask_image` 4574-4663. i32 `root & 15`: truth bounds [0,15]; `root<=259` -> `target<=15`; `5<=root` -> `0<=target`; strict -> `StrictBoundNotTranslation`; wrong conclusions -> `ConclusionMismatch`. Chain "mask then subtract 3" maps to `widened<=12` and `-3<=widened` (4646-4661).
- `saturating_endpoints_cover_every_byte_input_and_reject_plateau_strictness` 4665-4812. For signed and unsigned 8-bit, amounts {min, 1, max}, three operations (add right, add left, subtract), in two forms (definition axiom vs inline term). For every bound in the full byte range and both orientations, the mapped bound must equal the executed endpoint (a reference `execute` closure uses Rust `saturating_add/sub`, 4719-4732). It also brute-forces all inputs (4751-4758) to confirm monotonicity. Checks `StrictBoundNotTranslation` for `LessThan` (4761-4767) and truth bounds (4768-4780). Swapping root/target (4783-4793) and substituting a different value in the axiom (4794-4806) must error.

Submodule `tests::witness_checking` (4814-5435). Imports at 4815-4829, including `check_certificate`, `ProofNode`, `ProofRule`, `assumption_proof`, i.e. these tests also run the full certificate checker, not just the affine function.
- `direct_literal_remainder_maps_and_certifies_without_invented_definitions` 4831-4868. Direct u32 `root % 256` target with empty definition chain; wraps the witness in `ProofRule::IntegerAffineBound { root_bound, witness }` (4851-4857) and calls `check_certificate` (4858). A false goal (56) with the same proof object must fail (4860-4867).
- `direct_remainder_rejects_zero_divisors_and_unbound_or_mismatched_terms` 4870-4943. Literal divisor 0 -> `ZeroDivisionLiteral` (4895); signed divisor -1 direct -> `EmptyDefinitionChain` with comment "a direct signed remainder cannot assume MIN / -1 is representable" (4903-4904); divisors +-256 accepted; a `literal_axioms: vec![None]` with no definition ("phantom landing") errors (4919-4923); three invalid targets error (4925-4933); empty context -> `MalformedProposition(_)` (4941).
- `wrapping_remainder_retains_landed_divisor_and_carrier_specific_bounds` 4945-5064. u32 `root %wrap divisor` where divisor is a landed 10: bound table (100->9, 7->7, 0->0), also runs `check_certificate` per row and a one-lower false goal. Zero divisor axiom -> `ZeroDivisionLiteral` (5000-5003). Signed i16 %10 with lower -100 gives lower -9, and the test says "a signed remainder cannot inherit the unsigned nonnegative image" (5062).
- `checks_ordered_add_subtract_multiply_normalization` 5066-5118. Chain add 7, then subtract 2 (written with operands flipped: `Equal(subtract(added,2), subtracted)`, 5083-5091), then multiply by -3. Checked coefficient -3 and offset -15 (5116-5117).
- `checks_landed_literal_sibling_and_rejects_stale_or_unused_custody` 5120-5185. Landing `sibling = 7`, definition `target = root + sibling`. Errors: `LiteralAxiomCountMismatch` (5153), `LiteralAxiomNotPrior{definition:0, literal:1}` (5163), `UnusedLiteralAxiom(1)` (5183).
- `exact_add_backward_maps_bounds_and_rejects_ambiguity` 5187-5250. Witness goes from the defined value back to the operand (coefficient 1, offset -1). `defined < 3` -> `operand < 2`; `1 <= defined` -> `0 <= operand`; wrapping evidence is `Ok(vec![])` for exact add. Definition `defined = operand + operand` with the operand landed -> `AmbiguousDefinition(1)` (5248).
- `rejects_reordered_stale_and_non_affine_definitions` 5252-5293. `UnknownSemanticAxiom(1)` (5275), `NonCanonicalDefinitionOrder` (5283), `DefinitionShapeMismatch(0)` for `Equal(target, root)` (5291).
- `rejects_non_value_roots_stale_unsigned_words_and_checked_overflow` 5295-5376. `RootNotValue` for a literal root (5318); empty axioms with u8 witness -> `UnknownSemanticAxiom(0)` (5335); i128 multiply chain by MAX then 2 -> `CoefficientOverflow` (5374).
- `bitwise_and_mask_witness_rejects_negative_and_unlanded_masks` 5378-5434. Negative masks (-1, -256, either operand position) -> `NegativeBitwiseAndMask` (5400); mask given as an unlanded value -> `DefinitionShapeMismatch(0)` (5412); a landed 7 that is never used -> `UnusedLiteralAxiom(1)` (5432).

Submodule `tests::wrapping_bounds` (5436-6006), local macro `map!` (5451-5455), imports 5437-5449:
- `wrapping_product_bounds_require_the_entire_rectangle_to_fit` 5457-5573. For signed and unsigned 8-bit and a list of intervals (arrays at 5459-5479), brute-forces every product in the rectangle; if any mathematical product differs from its stored (wrapped) value, the expected result is `DirectMultiplyBoundOverflow` (5541-5544); otherwise min or max of stored values. Also checks wrong operand and swapped-order mutations error (5555-5568).
- `wrapping_add_backward_maps_strict_bounds_with_headroom_evidence` 5575-5647. u16 `defined = operand +wrap 1`. `defined < 2` requires headroom evidence `operand <= 65534`; without it `WrappingEvidenceMissing` (5610); with it `operand < 1`.
- `wrapping_add_backward_lands_a_sibling_literal_and_rejects_ambiguity` 5649-5702. Headroom is `operand <= 252` for sibling 3 in u8 (5671). `AmbiguousDefinition(1)` again for `operand + operand` (5700).
- `wrapping_divide_maps_both_directions_and_rejects_zero` 5704-5764. Unsigned u8 divide by 3 forward; `StrictBoundNotTranslation`; zero literal divisor -> `ZeroDivisionLiteral`.
- `signed_wrapping_division_maps_truncating_bounds_except_overflow` 5766-5823. Divisors {-128,-3,-1,0,1,3,127}. `-1` and `0` must fail the witness check (comment at 5791: "zero and the MIN/-1 discontinuity are not monotone division"). Others: exhaustive 256-value sweep of bounds with a brute-force monotonicity assertion.
- `signed_wrapping_remainder_bounds_include_minimum_by_negative_one` 5825-5891. Same sweep for wrapping remainder; divisor 0 -> `ZeroDivisionLiteral`; every other divisor including -1 and -128 is accepted. Endpoint formula inline: lower = 0 if bound>=0 else `1-|divisor|`; upper = 0 if bound<=0 else `|divisor|-1` (5859-5865).
- helper `signed_wrapping_witnesses(addend)` 5893-5929 returns operand, target, forward and backward `CheckedIntegerAffineForm`s for `target = operand +wrap addend` over i8.
- `signed_wrapping_add_demands_evidence_on_the_side_its_addend_can_cross` 5931-6005. For addend +1: lower bound needs `operand <= 126`; for addend -1: upper bound needs `-127 <= operand`. Redundant evidence in the wrong place -> `WrappingEvidenceUnexpected` (5992). Backward maps mirror this.

Submodule `tests::wrapping_subtraction` (6007-6401), imports 6008-6018. It defines its own local helpers: `context(integer_type, values)` 6020-6026 (a second implementation of the crate's `value_context`), `typed_literal` 6028-6033, `subtract` 6035-6037, `add` 6039-6041, `subtraction_row` 6043-6064, `holds` 6066-6079 and `assert_subtraction_mapping_is_sound` 6081-6144.
- `assert_subtraction_mapping_is_sound` does a soundness sweep: for every byte bound and four relation kinds (lower/upper x strict/non-strict, 6094-6103), it asks for required wrapping evidence, asserts the evidence is non-empty exactly when `(subtrahend > 0 && !lower) || (subtrahend < 0 && lower)` (6106-6107), maps, then for every operand value satisfying premise and evidence checks that the actual wrapped difference satisfies the mapped conclusion (6128-6139). It also asserts at least one bound is accepted per subtrahend (6142).
- `unsigned_wrapping_subtraction_maps_every_accepted_bound_soundly` 6146-6150 (subtrahends 0,1,5,200,255); `signed_wrapping_subtraction_maps_every_accepted_bound_soundly` 6152-6156 (-128,-3,-1,0,1,5,127).
- `unsigned_wrapping_subtraction_demands_exactly_the_minuend_floor` 6158-6223. u8 minus 1: upper bound needs floor `1 <= operand` (6165); four wrong evidence propositions each give `WrappingEvidenceUnexpected` (6190-6204); `3 <= operand` needs no evidence; `0 <= operand` -> `MappedBoundOutsideCarrier` (6221).
- `a_wrapping_target_spelled_in_place_is_its_own_definition` 6225-6295. Witness with empty definition chain and a target term `operand - 1` or `operand + 1` / `1 + operand` spelled inline.
- `an_in_place_step_follows_the_chain_it_extends` 6297-6352. Chain `defined = operand + 2` then target `defined - 1`; the evidence floor is on `defined` (`1 <= defined`, 6316); a target `operand - 1` with that axiom list -> `TargetMismatch` (6350).
- `in_place_targets_outside_one_literal_step_reject` 6354-6400. Five targets (literal minus operand, operand minus other value, operand plus other value, operand plus operand, other minus literal) -> `EmptyDefinitionChain` (6388); a u16 target over a u8 root -> `TargetTypeMismatch` (6398).

Back at the `tests` level (6402-6414):
- `use` lines 6402-6404, `pub(crate) use crate::integer_rules::value;` (6406), and two helper fns `literal(integer_type, i128)` (6408-6410) and `unsigned_literal(integer_type, u128)` (6412-6414). Both `.expect(...)` on `ScalarTerm::integer`.

### Inputs and outputs
Tests read: the functions under test (`check_integer_affine_witness`, `map_integer_affine_bound`, `check_integer_affine_bound_conversion`, `integer_affine_truth_bounds`, `integer_affine_wrapping_evidence`), types `IntegerAffineWitness`, `CheckedIntegerAffineForm`, `CheckedIntegerEndpointStep`, `IntegerAffineWitnessError`, `IntegerAffineBoundConversionError`, and `semantic_vocabulary` types (`Proposition`, `ScalarTerm`, `IntegerType`, `IntegerValue`, `ValueId`, `PropositionContext`, `ScalarType`, `IntegerMathTerm`). Crate helpers `value_context` (lib.rs:448) and `affine_witness` (lib.rs:459), both `pub(crate)` (grepped). `crate::integer_rules::value` re-exported at 6406. `witness_checking` also reads `check_certificate`, `ProofNode`, `ProofRule`, `assumption_proof`. The tests produce only assertions; no global state.

### How it decides (as pinned by the tests)
- Judgment is keyed on identity (value ids): redirecting an operand to another `ValueId` is a negative in most tests (e.g. 3367-3383, 4052, 5922ish). Also keyed on position: evidence order inside `Conjunction` is significant (reordered conjunctions error: 3874-3878, 4042-4048, 5565-5568). Also keyed on value (the literal endpoint must equal the carrier min/max or the computed endpoint: 3457-3463, 4015-4062).
- Definition axioms are indexed by position in the axiom slice and must be strictly increasing (`NonCanonicalDefinitionOrder`, 4559, 5283); a literal landing must come earlier than the definition that uses it (`LiteralAxiomNotPrior`, 5163) and must be used (`UnusedLiteralAxiom`, 5183, 5432).
- No string comparison or debug-format keyed judgments are in my range. The only `{:?}` uses are in assertion messages (e.g. 6137, 6386).

### Cases by shape
Direct (no axiom) bound mapping handles a fixed list of shapes, as shown by tests only (the implementation lives outside my range, so I did not read the match): add, subtract, multiply (four-corner conjunction), shift-left (count range), and in-place wrapping add/subtract of a single literal. Tests for refusals:
- Four-part conjunction required for multiply; 3-part rejected (4055-4061).
- "mixed orientation, reordered evidence, and redirected operands reject" (message 3974).
- In-place targets outside "one literal step" reject with `EmptyDefinitionChain` (6354-6391, test name at 6355).
- Signed divisor -1 and 0 reject for division (5788-5793); only 0 rejects for remainder (5847).
- Negative bitwise-and masks reject (5387-5402).

### Same job elsewhere
- Building a typed-integer `PropositionContext` is done two ways in the tests: `value_context(...)` called 15 times in my range, and a hand-written `PropositionContext::from_value_types(...)` call 22 times in my range (grep `PropositionContext::from_value_types` and `value_context\(` over integer_affine.rs; counts taken from lines 3301-6415 of the output). `mod wrapping_subtraction` adds a third local `context(...)` function (6020-6026) doing the same job as `value_context`.
- Four copies of a local `math_value!` macro: two in my range (3298 and 3706) and others at earlier lines I did not read (suspected, not read).
- The same test pattern (witness mutation sweep: missing custody, reorder, redirect) is repeated across add (3441-3488), subtract (3534-3550), multiply (3657-3700) in `correlated`, and in `witness_checking` (5143-5184).
- The `check_integer_affine_bound_conversion` vs `map_integer_affine_bound` pair is used interchangeably as the oracle in the tests (e.g. 4258-4265 asserts both agree).

### Unused or one-caller code (greps)
No `pub` items in my range. The `pub(crate) use crate::integer_rules::value;` at 6406 re-exports for the child modules via `super::value` (used at 4815, 5437, 6008 imports). The helper fns `literal` and `unsigned_literal` (6408, 6412) are consumed by child modules via `use super::{literal, unsigned_literal, value}` (4815, 5437, 6008). `signed_wrapping_witnesses` (5893) is called at 5936 and 5965 (two sites, read). Not grepped further.

### Stated limits
- "a direct signed remainder cannot assume MIN / -1 is representable" (4904).
- "zero and the MIN/-1 discontinuity are not monotone division" (5791).
- "only the tight or legacy mapping is admitted; relaxation needs evidence" (4280).
- "a noncarrier exact minuend cannot orient bare subtrahend inclusion" (3997).
- "a signed remainder cannot inherit the unsigned nonnegative image" (5062).
None of these says "unsupported/migration debt" in so many words; they are constraints the tests enforce.

### Fail-open risks
None seen in this range as production code (it is all tests). Two test behaviors that look permissive by design, both "stated" by test names/messages:
- `check_integer_affine_bound_conversion` still accepts the old total-image conclusion (`target <= 255` for `root % 256`) for any root bound, including `root <= 57` (4266-4270, message "the previous total-image certificate remains valid"). Wrong program that gets through: none shown (255 is a sound bound), but the check does not require the tight bound. Stated.
- `assert_subtraction_mapping_is_sound` skips bounds the mapper refuses (`let Ok(conclusion) = ... else { continue; }`, 6124-6126). Only a guard `accepted > 0` (6142) catches a mapper that rejects everything for a subtrahend; it does not catch one that rejects most bounds. Traced in the test only.

---------------------------------------------------------------------
## File 2: mathematical_core.rs lines 521-1059
---------------------------------------------------------------------

### Where the range starts
Line 12 opens `pub(crate) mod certificate`; `mod batch` at 13; `#[cfg(test)] mod tests` at 514-515 (grep of `mod ` lines: 12, 13, 160, 515, 1076). Line 521 is in the middle of the `use crate::mathematical_core::tests::{...}` statement (520-523) at the top of `certificate::tests`. This `tests` module ends at 1074 and `certificate` closes at 1075; `pub(crate) mod conversion` starts at 1076. My range ends at 1059, inside the test `the_certificate_records_its_exact_assumption_closure` (1051-1073). I read through 1073 for context.

Context just above the range (500-512, read): non-test `pub fn certificate_assumption_closure(arena, certificate) -> BTreeSet<u32>` at 500, which builds a `Signature` from `certificate.signature` and calls `judgment_assumption_closure(arena, &signature, &roots)` with roots = context terms plus term plus expected.

### Purpose
Tests for the `MathematicalCertificate` checker, `verify_mathematical_certificate`, in the dependent-type core. A certificate is `{signature, level_arity, context, term, expected}` (fields visible at 951-957). The tests pin that the checker accepts well-formed certificates (identity, case-on-Two, identity elimination, W-induction, strict sorts, universe-polymorphic) and rejects tampered ones with specific `CoreError` variants.

### Shape
Imports (516-524): `CoreError, Declaration, MathematicalCertificate, TermArena, TermHandle, certificate_assumption_closure, verify_mathematical_certificate` from `super`; helpers `default_budget, lambda, pi, polymorphic_identity_declaration, strict_sort, type_sort, variable` from `crate::mathematical_core::tests` (those helpers are defined at mathematical_core.rs:9127-9452, grepped); `Level, Sort, Term`.
- Local macro `term!` (526-539) with two arms: `Apply => f => a` building `Term::Apply`, and `Constant => decl => levels` building `Term::Constant`.
- Helper `polymorphic_identity(arena)` 541-551: returns (identity term, its Pi type).
- `a_complete_certificate_verifies` 553-560: closed certificate verifies.
- `the_context_is_part_of_the_checked_judgment` 562-582: a variable 0 under a one-entry context passes; same term with empty context -> `CoreError::UnboundVariable{index:0, context_depth:0}` (577).
- `dependent_context_bindings_check_formation_under_their_prefix` 584-604: context `[Type0, var0]` with term var0 and expected var1 verifies; a context entry that is a lambda -> `CoreError::NotASort{..}` (602).
- `a_tampered_term_or_claimed_type_rejects` 606-627: wrong expected -> `TypeMismatch`; free variable -> `UnboundVariable`.
- `a_certificate_carries_dependent_two_elimination` 629-679: `Term::CaseTwo{motive, zero_branch, one_branch, scrutinee}` with a dependent motive; the right expected is `Apply(family, scrutinee)`, the wrong one `Apply(family, TwoZero)` gives `TypeMismatch` (675-678).
- `a_certificate_carries_identity_elimination` 681-794: builds `Term::Id{ty,left,right}` and `Term::IdElim{motive,base,endpoint,proof}` with six context entries; wrong expected -> `TypeMismatch`.
- `a_certificate_carries_w_induction` 796-916: builds `Term::W{carrier,children}`, `Term::Sup{carrier,children,label,function}`, `Term::IndW{motive,step,tree}` with a seven-entry context; wrong expected -> `TypeMismatch`.
- `strict_sorts_flow_through_the_certificate` 918-938: a context entry that is a strict sort verifies; a lambda as expected type -> `NotASort`.
- `a_certificate_is_parametric_over_its_level_arity` 940-981: `level_arity: 1` with `Sort::Type(Level::Parameter(0))` verifies; the same term at arity 0 -> `UnboundLevelParameter{index:0, arity:0}` (964); `Level::Parameter(1)` at arity 1 -> `UnboundLevelParameter{index:1, arity:1}` (979).
- helper `instantiated_identity_type(arena, u)` 983-989.
- `a_signed_certificate_verifies_its_declaration_closure` 991-1005: `MathematicalCertificate::with_signature(vec![declaration], context, evidence, expected)` where evidence is `Constant{declaration:0, levels:[Constant(0)]}`.
- `a_forged_declaration_index_never_verifies` 1007-1026: constant index 1 with a one-entry signature -> `UnknownDeclaration{declaration:1, signature_len:1}` (1021).
- `an_unchecked_signature_entry_never_verifies` 1028-1049: a two-entry signature whose second entry is ill-typed (`Declaration::definition(0, Two, Type0)`) causes `TypeMismatch` even though the term only mentions it by constant. I.e. every signature entry is checked, not only those used by the term (read: the evidence constant is index 1, the unsound entry, so this test alone does not prove unused entries are checked; inferred).
- `the_certificate_records_its_exact_assumption_closure` 1051-1073: signature `[assumption axiom, definition that uses the axiom]`; verify succeeds, and `certificate_assumption_closure(&arena, &certificate)` equals `{0}` (1069-1072).

### Inputs and outputs
Tests build terms in a fresh `TermArena` (mutated by `insert`), call `verify_mathematical_certificate(&mut arena, &certificate, &mut default_budget())` and compare against `Ok`/`CoreError` variants. Several tests create a second fresh arena mid-test (571, 595, 929, 967). No outputs other than assertions.

### How it decides (as pinned)
Keyed on de Bruijn indices (variable index values like 0, 1, 5, 4 are chosen explicitly) and on term structure, not on names. Declarations are keyed by their index in the signature (`declaration: 1`, 1021). Level parameters keyed by index against `level_arity` (964, 979).

### Cases by shape
Four elimination forms are exercised, each with a positive and one wrong-`expected` negative: `CaseTwo` (629), `IdElim` (681), `IndW` (796), plus identity/Pi/lambda. I saw no test for other eliminators in this range; whether others exist elsewhere in the file I did not check here (file has `mod identity_and_w_types` at 4850 per grep, suspected overlap with these tests, not read).

### Same job elsewhere
- A test module in `mathematical_core.rs` named `identity_and_w_types` (line 4850, from the `mod` grep) and `type_checking` (8791) likely cover the same Id/W typing rules at the term level; this range tests them at the certificate level. Suspected, not read.
- The expected-type pattern "right expected passes, wrong expected gives `TypeMismatch`" repeats at 607-626, 670-678, 773-793, 900-915 (four copies).

### Unused or one-caller code
No `pub` items in this range. `certificate_assumption_closure` (500, just above the range) is `pub fn`; its use here is 1070. I did not grep outside the file for it.

### Stated limits
None in this range.

### Fail-open risks
None seen (test code). The tests assert fail-closed outcomes: forged index, unchecked signature entry, unbound level parameters all reject.

---------------------------------------------------------------------
## Coverage
- integer_affine.rs 3301-6415: read in full, 3115 lines (I also read 3290-3300 for the header, which do not count).
- mathematical_core.rs 521-1059: read in full, 539 lines (I also read 500-520 and 1060-1075 for context, not counted).
- Skimmed: 0. Not read: 0.
- Total 3654 lines read in full; 0 not read.

## Five things a maintainer most needs to know
1. This whole unit is test code. integer_affine.rs:3269-6415 is the file's `#[cfg(test)] mod tests`; mathematical_core.rs:515-1074 is `certificate::tests`. No production logic sits in these ranges.
2. The integer affine tests pin custody rules, not just arithmetic: definition axioms must be in increasing position (`NonCanonicalDefinitionOrder`, 4559), literal landings must come earlier (`LiteralAxiomNotPrior`, 5163) and be used (`UnusedLiteralAxiom`, 5183/5432), and evidence order inside a `Conjunction` matters (3874-3878, 4042-4062).
3. Wrapping operations demand extra "headroom" evidence only on the side the addend/subtrahend can cross (signed add 5931-6005; subtraction rule `(c>0 && !lower) || (c<0 && lower)` at 6106). `integer_affine_wrapping_evidence` returns that evidence; `map_integer_affine_bound` refuses without it (`WrappingEvidenceMissing`) and refuses extra evidence (`WrappingEvidenceUnexpected`).
4. Several tests are exhaustive over 8-bit (saturating 4665-4812, signed division 5766-5823, signed remainder 5825-5891, subtraction soundness 6081-6144, wrapping multiply 5457-5573), using Rust's own arithmetic as the oracle. These are the strongest checks of the mapper; the rest are example-based.
5. Division and remainder edge handling: direct signed remainder by -1 is refused (4903), wrapping signed division by -1 or 0 is refused (5788-5793), but wrapping signed remainder by -1 is accepted (5836-5851, test name says "include_minimum_by_negative_one").

## What I am unsure about
- I did not read the production code these tests exercise, so statements about "how it decides" come only from what the tests assert, not from the implementation.
- `an_unchecked_signature_entry_never_verifies` (mathematical_core.rs:1028-1049) uses the unsound entry as the evidence constant itself, so I cannot tell from this test whether an unused unsound entry would also be rejected.
- The counts of `from_value_types` (22) and `value_context(` (15) in my range come from line numbers in a whole-file grep that I tallied by hand, not from a range-restricted count command.
- The i128 overflow tests (3880-3913, 5338-5375) depend on exact error variants (`DirectAddBoundOverflow`, `CoefficientOverflow`); I did not check whether those variants are reachable elsewhere.
