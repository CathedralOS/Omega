# Record: unit_043 gap_022 (second pass; area 07-psi-semantics, crate proof-admission)

Unit file `units/gap_022.tsv` (read). Root `S/` = `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/`.
Two ranges:
- `S/integer_affine.rs` 1401-7192 (5,792 lines)
- `S/integer_rules.rs` 1-208 (208 lines)

Both ranges were read in full. Lines 1-1400 of `integer_affine.rs` belong to the first reader and are not described here.
Where a claim needs an earlier line (for example the `bound_mapping` module header at `integer_affine.rs:9`), I say so.

## Layout of integer_affine.rs 1401-7192 (read; module openings from a Grep of `mod`/`fn` lines)

| Lines | What it is |
| --- | --- |
| 1401-1409 | tail of `literal_endpoint_image` (starts at 1399, before the range) |
| 1411-1591 | `map_direct_shift_left_bound`, `integer_value_to_u128`, `bitwise_or_image_maximum`, `shift_math_literal` (inside `mod bound_mapping`) |
| 1593-1623 | `IntegerAffineBoundConversionError` (24 variants) |
| 1625-2419 | `mod remainder` production code |
| 2421-2677 | `mod remainder::tests` |
| 2679-2837 | `mod truth_bounds` |
| 2839-4007 | `mod witness_checking` (`correlated`, `definitions`, form types, `check_integer_affine_witness`) |
| 4009-4017 | re-exports |
| 4019-7192 | `#[cfg(test)] mod tests` (about 3,170 lines, 45 `#[test]`, counted from the Grep of `fn` names) |

Production code in this range is about 2,600 lines (1401-4017). The other 3,170 lines are tests.

## 1411-1623: direct shift mapping and the conversion error enum

1. Purpose: maps a bound on a shifted value's operand to a bound on the shifted result, using an exact shift-left term. Also holds the error enum shared by all affine bound mapping.
2. Shape (read):
   - `map_direct_shift_left_bound(form, evidence)` (`:1411`).
   - `integer_value_to_u128` (`:1564`).
   - `bitwise_or_image_maximum(left_upper, right_upper)` (`:1571`).
   - `shift_math_literal(value, count)` (`:1580`).
   - `IntegerAffineBoundConversionError` (`:1594`, `pub`).
3. Inputs and outputs: reads a `CheckedIntegerAffineForm` whose target is `ExactIntegerShiftLeft` (`:1415`), plus `evidence` (a `Proposition`, either one part or a `Conjunction`). It returns a `Proposition::IntegerMathLessOrEqual` against a `ShiftLeft` math term (`:1553-1561`). No state.
4. How it decides, keyed on term identity and literal values:
   - Each conjunct is classified by equality with `form.root()` or with the shift count term (`:1439-1476`).
   - At most one root bound, one exact count, and one count upper bound are allowed; a second of any kind returns `AmbiguousDirectShiftEvidence` (`:1442`, `:1453`, `:1471`).
   - The count range is exact count, or `[0, upper]` when a lower bound is known, or `[0, count_type max]` (`:1484-1510`).
   - Both ends must fit `u32` and be strictly less than the root type's bit width (`:1511-1526`).
   - The count used for the endpoint is the larger one if the (lower endpoint, negative bound) or (upper endpoint, non-negative bound) pair holds, else the smaller (`:1548-1551`). The key is the endpoint side and the literal's sign.
   - The left-shifted magnitude must fit in `u128` and be a valid `IntegerMathLiteral`, else `MappedBoundOverflow` (`:1585-1590`).
5. Cases by shape: the `_` arm at `:1477` rejects any conjunct that is not one of the four forms above (`DirectShiftEvidenceMismatch`). Only `ExactIntegerShiftLeft` targets are handled (`:1415-1423`). Operand and count must pass `direct_math_leaf` (`:1425-1428`).
6. Same job elsewhere:
   - A sibling `map_direct_add_bound` / `subtract` / `multiply` family sits earlier in the same module (`:940-1178`, outside my range; from the Grep list).
   - The unsigned-count lower-bound default `count_lower = count_type.sign() == Unsigned` (`:1435`) is a second place that treats "unsigned" as implying `0 <= count`.
7. Unused or one-caller:
   - `bitwise_or_image_maximum`: `grep -rw` over `source-rust tools` gives 2 lines (definition and one use). The use is outside my range (inferred).
   - `integer_value_to_u128`: not counted.
8. Stated limits: none quoted in this range.
9. Fail-open: none seen.
   - `count_lower` defaults to true for unsigned count types without evidence (`:1435`). That is correct because an unsigned count is `>= 0` by type. Traced: the type is checked at `:1450-1453` and `:1484-1489`.

## 1625-1735: `mod remainder`, unsigned remainder by a value (`check_witness`, `map_bound`)

1. Purpose: lets an unsigned remainder `x % d` (with `d` the root) be bounded by `d`. The result is `x % d < d` given proof that `d > 0`.
2. Shape: `check_witness(context, semantic_axioms, witness) -> Result<Option<CheckedIntegerAffineForm>, _>` (`:1639`), and `map_bound(form, root_bound)` (`:1716`). Both are `pub(super)`.
3. Inputs and outputs: reads the witness target (inline remainder expression) or one cited definition axiom (`:1644-1654`). Produces a form with `coefficient 1`, `offset 0`, and the single step `CorrelatedUnsignedRemainder` (`:1706-1713`). `map_bound` returns `LessThan(target, root)`.
4. How it decides:
   - It returns `Ok(None)` when the target is not a remainder with the root as divisor (`:1655-1661`). It also returns `None` unless the carrier is `Fixed` and `Unsigned` (`:1662-1666`). `Ok(None)` means "not this rule, try the next one".
   - It returns errors for non-value root, non-value or non-literal dividend, wrong definition count, or type drift (`:1667-1694`).
   - `map_bound` accepts `0 < d` (expected value 0) or `1 <= d` (expected value 1) only when the literal is `Unsigned` and of the form's own type (`:1720-1729`). The key is the typed literal value.
5. Cases by shape: the dividend must be `ScalarTerm::Value` or `ScalarTerm::Integer` (`:1670-1675`); exactly zero or one definition axiom (`:1676-1689`).
6. Same job elsewhere: `check_integer_affine_witness` (`:3614`) calls this checker first, then `check_dividend_witness`, then `check_multiple_witness` (`:3619-3631`). The three remainder checkers each accept a different shape of remainder definition. A fourth direct-literal remainder path is in `check_integer_affine_witness` at `:3641-3672`.
7. Unused: n/a (all `pub(super)`, each called from `witness_checking` or `bound_mapping`; inferred from the call site at `:3619`).
8. Stated limits: none.
9. Fail-open: `Ok(None)` for unmatched shapes.
   - Traced at `:3619-3631`: it falls through to later checkers. Later in the function, an unmatched witness reaches the `RootNotValue` or `EmptyDefinitionChain` errors (`:3716-3733`). No accept path found for unmatched shapes.

## 1737-1946: remainder-within-dividend and quotient-at-most-dividend

1. Purpose: for `result = dividend % divisor` or `dividend / divisor` where all three are values, proves `min(dividend,0) <= result <= max(dividend,0)` (remainder) or `result <= dividend` (quotient) given divisor premises.
2. Shape: `check_dividend_witness` (`:1757`), `map_quotient_relation` (`:1850`), `map_dividend_bound` (`:1898`). Endpoint steps `RemainderWithinDividend{divisor}` / `QuotientAtMostDividend{divisor}` are added at `:1816-1824`.
3. Inputs and outputs: reads exactly one definition axiom and a `[None]` landing list (`:1762-1767`). Returns a form with those steps. The bound mappers take a `Conjunction` and return a `LessOrEqual` or `LessThan`.
4. How it decides:
   - Keyed on term identity: `result == witness.target`, `dividend == witness.root`, each a `ScalarTerm::Value` (`:1786-1790`). Duplicate operands are an error (`:1795-1799`). The three terms must share the carrier (`:1801-1806`).
   - `map_dividend_bound` accepts the divisor premise `0 < divisor` or `divisor < -1` only, by literal value against the form's type (`:1916-1920`). It rejects `0 <= d` and `d < 0`.
   - The output endpoint is `max(U, 0)` or `min(L, 0)` (`:1935`, `:1940`). A value outside the carrier gives `MappedBoundOutsideCarrier` (`:1924-1927`).
   - `map_quotient_relation` accepts two exact shapes: `[L <= n, 0 < d]` with literal `L >= 0` giving `<=`, or `[0 < n, 1 < d]` giving `<` (`:1866-1887`).
5. Cases by shape: fixed carrier (any sign, any width) only (`:1791`); a literal divisor is "the endpoint chain's `Remainder` step and is not matched here" (comment `:1752-1753`). Conjunction order is fixed (`:1911`, `:1867-1886`).
6. Same job elsewhere: `check_dividend_witness` handles remainder and quotient in one function (`:1771-1783`). The direct literal remainder is handled by `Remainder(i128)` steps in `definitions::apply_definition` (`:3090-3117`).
7. Unused: `is_remainder_within_dividend` and `is_quotient_at_most_dividend` each have 2 lines in `source-rust tools` (a definition at `:3538`/`:3546` and one caller in another file; grepped, caller not read).
8. Stated limits (quoted):
   - `:1750` "Provisional (`AcceptedProofRule::IntegerRemainderWithinDividend`): the kernel trusts this statement"
   - `:1847` "Provisional (`AcceptedProofRule::IntegerQuotientAtMostDividend`)" with the same sentence "the mathematical core does not derive it".
   These two are statements the kernel accepts without derivation. Which wrong program gets through depends on the statement being true. The doc comment gives the argument (truncating remainder lies between 0 and the dividend); I did not check it independently.
9. Fail-open: none seen at the code level.
   - `Ok(None)` on non-matching shapes (`:1766`, `:1769`, `:1782`, `:1793`) hands off to the next checker.
   - The "provisional" label is the stated gap: the rule's truth is trusted (stated, `:1750-1753`).

## 1948-2419: integer-multiple remainder (`check_multiple_witness`)

1. Purpose: when the dividend of an exact remainder expands into a polynomial whose every coefficient is divisible by the literal divisor, proves the remainder is zero. Example: `(2*x + 4) % 2`.
2. Shape:
   - `VariableKey`/`Polynomial` types (`:1948-1949`).
   - Limits `MAXIMUM_MULTIPLE_DEPTH` 32, `_NODES` 64, `_MONOMIALS` 64 (`:1951-1953`).
   - `normalize_polynomial` (`:1955`), `combine_polynomials` (`:1976`), `multiply_polynomials` (`:1994`), `push_carrier_bounds` (`:2014`).
   - `check_multiple_witness` (`:2043`), with an inner `Walk` struct and `expand` (`:2197-2339`).
   - `check_multiple_premise` (`:2374`), `map_multiple_bound` (`:2390`), `check_multiple_bound` (`:2403`).
3. Inputs and outputs:
   - Reads the witness (root must equal target, `:2048`), definition axioms, the divisor (a literal, or a value defined by an earlier literal-equality axiom, `:2154-2184`).
   - Output is a form with `coefficient 0`, `offset 0`, step `MultipleRemainder{required_bounds}` (`:2364-2371`). `required_bounds` lists carrier-range propositions for each inline partial operation that was not authenticated by a cited definition.
4. How it decides:
   - Divisibility is computed with arbitrary-precision `BigInt` remainders (`:2356-2360`), keyed on polynomial coefficients, not on syntax.
   - Used-ness: every cited definition axiom must be consumed by the walk, else `DefinitionShapeMismatch` (`:2351-2355`).
   - Rejects `signed -1` divisor by returning `Ok(None)` (`:2190-2195`), and zero divisor with an error (`:2187-2189`).
   - `expand` follows a `Value` through its definition axiom only for axioms with index `< upper_axiom` (`:2225-2233`), so a later axiom cannot redefine an earlier value.
   - A conclusion premise must equal `Truth` (no bounds), the single bound, or a `Conjunction` equal to the list (`:2378-2384`). Order matters.
5. Cases by shape (quoted from `:2032-2036`): "remainder by a nonzero literal divisor (excluding signed `-1`), when the dividend expands over exact add, subtract, multiply, widen and exact cast". `expand` accepts those six term kinds and returns `None` for everything else (`:2336`). Fixed, non-address carriers only (`:2205-2207`). Depth 32, 64 nodes, 64 monomials.
6. Same job elsewhere: `check_multiple_bound` is a variant of `map_multiple_bound` that checks a stated conclusion; it accepts `target <= 0` or `0 <= target` (`:2412-2413`).
7. Unused: `check_multiple_bound` has 2 lines in `source-rust tools` (definition and one call site outside my range; grepped, not read). `multiple_remainder_bounds` has 7 lines, 2 files. `is_multiple_remainder` has 2 lines.
8. Stated limits (quoted): `:2038` "Provisional (`AcceptedProofRule::IntegerMultipleRemainder`)" with "the mathematical core does not derive it". `:2040` "inline partial operations inside `dividend` owe their exact destination-carrier bounds in `root_bound`."
9. Fail-open:
   - `normalize_polynomial` and the caps return `None`, which becomes `Ok(None)` (`:2347-2349`), meaning "not this rule" and not accept. Traced.
   - `unreachable!()` at `:2329` is guarded by the enclosing `match` arm (traced).
   - A `Value` with no definition axiom becomes a fresh variable with coefficient 1 (`:2234-2241`), so `x % 2` alone is not divisible and fails. Traced.
   - `used[slot]` marks the last axiom used when `mark_last` (`:2151`); a stale earlier axiom with the same left value would be skipped by the `find`, which takes the first match with index `< upper_axiom` (`:2225-2229`) and not the latest. A duplicate definition of one value therefore resolves to the earliest one. The context validate call at `:2125-2127` does not reject duplicates. Open; I did not trace whether an earlier layer forbids two equalities for one value.

## 2421-2677: remainder tests (read)

Four tests (`:2457`, `:2523`, `:2577`, `:2650`) for the unsigned remainder-by-value rule. They loop over widths 1, 8, 32, 64, 128, over exact, wrapping and saturating remainders, and over inline and named results. The negative cases mutate premises (Truth, `0 <= d`, `d < 1`, wrong value, equality), definition indices, landing lists, other carriers, and signed carriers. All use `accept_certificate` or `check_certificate` or `check_integer_affine_witness` and assert `is_err()` or `Ok`.

## 2679-2837: `mod truth_bounds`, `integer_affine_truth_bounds` (`:2687`)

1. Purpose: for a form with a total image (the whole carrier maps into a known interval), returns the two propositions `min <= target` and `target <= max`, without needing a root bound.
2. Shape: one `pub fn`. Returns `Vec<Proposition>` or `IntegerAffineBoundConversionError`.
3. Inputs and outputs: reads `form.endpoint_steps` and the carrier interval as `i128`. Walks the steps updating `(minimum, maximum)` with checked arithmetic (`:2703-2825`).
4. How it decides, keyed on the step variant (a `match` at `:2704`):
   - Add / Subtract / Multiply / ShiftLeft: arithmetic on the endpoints. Overflow gives `MappedBoundOverflow`.
   - SaturatingAdd, Divide, Remainder, ShiftRight, BitwiseAndMask, `MultipleRemainder` with no required bounds: set `saw_total_image = true`.
   - `Divide(-1)` on signed gives `NonTotalDivisionImage` (`:2745`).
   - `BitwiseOrLiteralImage`, wrapping and correlated steps, `RemainderWithinDividend`, `QuotientAtMostDividend`: `TruthRootWithoutTotalImage` (`:2800-2823`).
   - After the loop: if no step set `saw_total_image`, error (`:2826`).
5. Cases by shape: the `match` lists every variant of `CheckedIntegerEndpointStep` with no wildcard arm (`:2704-2823`), so a new variant breaks compilation (read; no `_ =>`).
6. Same job elsewhere: the per-step arithmetic here repeats the per-step arithmetic in `walk_endpoint_steps` (`:472`, outside my range, suspected) and in the `bound_mapping` direct mappers.
7. Unused: `integer_affine_truth_bounds` has 9 lines across 2 files outside this file (grepped).
8. Stated limits: none quoted.
9. Fail-open: none seen. A form with no steps, or with only non-total steps, is refused at `:2826`. An empty `endpoint_steps` gives no total image, so error.

## 2839-3003: `witness_checking::correlated`

1. Purpose: recognizes "correlated" witnesses where the root equals an expression like `MAX - right`, and the target is `left + right`. It records which carrier endpoint the correlation uses.
2. Shape:
   - `check_correlated_witness(context, axioms, witness, outer, inner, endpoint_step)` (`:2852`).
   - Wrappers `check_correlated_add_witness` (`:2923`), `check_correlated_subtract_witness` (`:2960`), `check_correlated_multiply_witness` (`:2982`).
   - `carrier_step` (`:2941`).
3. Inputs and outputs: `outer`/`inner` are `TermMatcher` function values from `crate::term_matchers` (imported `:2842`). The output is a form with step `CorrelatedAdd{endpoint}`, `CorrelatedSubtractLower/Upper` or `CorrelatedMultiplyMinimum/Maximum`, `coefficient 1`, `offset 0`.
4. How it decides:
   - Target must match the outer operator and both operands must be direct math leaves (`:2860-2869`).
   - Either no definition (root itself is the inner expression) or one definition `root = inner` and a landing slot (`:2870-2890`).
   - The inner right operand must equal the outer right operand by term equality (`:2894`).
   - The inner left term is the endpoint. For add it only needs to be a direct leaf (`:2935`). For subtract and multiply it must be exactly the carrier minimum or maximum literal (`carrier_step`, `:2951-2957`).
5. Cases by shape: add pairs with subtract; subtract pairs with add; multiply pairs with divide (`:2932-2933`, `:2969-2970`, `:2991-2992`). Zero or one definition axiom (`:2874-2889`).
6. Same job elsewhere: `map_correlated_add_bound`, `map_correlated_subtract_bound`, `map_correlated_multiply_bound` (`:818`, `:855`, `:891`, outside my range) consume these steps.
7. Unused: n/a.
8. Stated limits: none.
9. Fail-open: `Ok(None)` for unmatched shapes (`:2861`, `:2868`, `:2878`, `:2885`, `:2889`, `:2892`, `:2894`, `:2899`, `:2903`, `:2911`). Traced: falls to `check_integer_affine_witness`, which tries the next checker or errors. No accept path.

## 3004-3436: `witness_checking::definitions`

1. Purpose: applies one cited definition equality to advance a chain `(current, coefficient, offset)` by one step, yielding a new current term and a `CheckedIntegerEndpointStep`.
2. Shape:
   - `apply_definition` (`:3007`).
   - `apply_shift_definition` (`:3222`).
   - `nonnegative_count` (`:3286`).
   - `apply_add_inverse` (`:3310`).
   - `apply_wrapping_add_inverse` (`:3367`).
   - `apply_exact_add_inverse` (`:3400`).
   - `both_orientations` (`:3429`).
3. Inputs and outputs: each returns `Option<Result<(next, coefficient, offset, used_landing, step), Error>>`. `None` means "this definition is not of my shape".
4. How it decides, keyed on term shape and term identity with `current`:
   - `apply_definition` requires the defined side to be a typed `Value` (`:3021-3025`). It matches the expression against: exact add (`current` as either operand), exact subtract (`current` on the left only), exact multiply, divide or remainder (exact and wrapping; `current` on the left), bitwise and, bitwise or, wrapping/saturating add, wrapping/saturating subtract (`:3026-3207`).
   - The other operand must resolve to a literal through `signed_literal`, which accepts an inline literal of the right type or the single landed literal whose `term` equals the operand (`:3929-3940`).
   - Zero divisor gives `ZeroDivisionLiteral`. Signed wrapping divide by `-1` returns `None` (`:3109-3111`). Negative `&` mask gives `NegativeBitwiseAndMask`. Negative `|` sibling gives `NegativeBitwiseOrSibling`.
   - Overflow in coefficient or offset gives `CoefficientOverflow` (`:3218`).
   - `apply_shift_definition` requires an 8/16/32/64-bit value type (`:3237`) and a non-negative fixed count that is below the value width (`:3270-3277`).
   - `apply_add_inverse` walks backward: `defined == current` and the expression is a wrapping or exact add of an unknown value plus a literal. If both sides could be the operand, it returns `AmbiguousDefinition` (`:3348-3351`).
5. Cases by shape: the fixed `match` list at `:3026-3208`. Exact subtract requires `current` on the left (`:3051`), so `literal - current` is not accepted. Exact divide and remainder likewise need `current` on the left (`:3099`). Signed `-1` exact divide is accepted (`:3105-3106`), because exact (not wrapping) division is assumed not to overflow; the guard is only on the wrapping case (`:3109`). `truth_bounds` rejects `Divide(-1)` on signed (`:2745`), so the two sites differ on a signed `-1` exact divide (read at both lines).
6. Same job elsewhere: `apply_definition`'s wrapping and saturating add arms duplicate `apply_policy_expression_target` (`:3868`), which does the same job for a target spelled inline.
7. Unused: n/a (all `pub(super)` or private; used in `check_integer_affine_witness` at `:3763-3805`).
8. Stated limits: none.
9. Fail-open: `None` returns. In `check_integer_affine_witness`, an empty candidate list gives `DefinitionShapeMismatch` (`:3814`) and two or more give `AmbiguousDefinition` (`:3816`). Traced; no accept path for unmatched definitions. `next.clone()?` at `:3813` propagates the inner `Err`.

## 3440-3568: form types

1. Purpose: the checked result of a witness.
2. Shape:
   - `CheckedIntegerAffineForm` (`:3441`; fields `root, target, integer_type, coefficient, offset, endpoint_steps`, all `pub(crate)`).
   - `CheckedIntegerEndpointStep` (`:3451`, `pub(crate)`, 22 variants: `Add, Subtract, Multiply, Divide, Remainder, ShiftLeft, ShiftRight, SaturatingAdd, WrappingAdd, WrappingAddBackward, BitwiseAndMask, BitwiseOrLiteralImage, CorrelatedAdd, CorrelatedSubtractLower, CorrelatedSubtractUpper, CorrelatedUnsignedSubtract, CorrelatedUnsignedRemainder, RemainderWithinDividend, QuotientAtMostDividend, MultipleRemainder, CorrelatedMultiplyMinimum, CorrelatedMultiplyMaximum`; count by reading `:3452-3493`).
   - `LandedInteger` (`:3509`).
   - Accessors (`:3515-3567`).
3. Notes:
   - `coefficient` and `offset` are kept separately from the steps. They are only trusted for chains of exact add, subtract and multiply: for example `MultipleRemainder` sets `coefficient 0` (`:2368`).
   - `preserves_strict_endpoint` is true only for `Add`, `Subtract`, `WrappingAdd`, `WrappingAddBackward` (`:3497-3505`). It has 2 lines in the repo, so one caller (grepped; the caller is in `bound_mapping`, not read by me).
4. How it decides: nothing; data.
5. Same job elsewhere: the three `is_*` predicates and `multiple_remainder_bounds` each inspect `endpoint_steps` as a one-element slice (`:3538-3566`).
6-9. Fail-open: none seen.

## 3570-3848: `check_integer_affine_witness` (`:3614`, `pub`)

1. Purpose: the entry point that turns an `IntegerAffineWitness` (root, target, list of cited definition axioms and landing axioms) into a `CheckedIntegerAffineForm`, or an error. It is where the affine bound rule's evidence is replayed.
2. Shape: `check_integer_affine_witness(context, semantic_axioms, witness) -> Result<CheckedIntegerAffineForm, IntegerAffineWitnessError>`. Helpers `identity_ring_target` (`:3570`), `trivial_affine_form` (`:3600`), `policy_add_step` (`:3850`), `apply_policy_expression_target` (`:3868`), `signed_literal` (`:3929`), `landed_integer` (`:3942`). `IntegerAffineWitnessError` (`:3977`, 23 variants).
3. Inputs and outputs: reads the witness and the semantic axioms. Output is a form. Callers: 119 lines mention it across 7 other files (grepped); most are tests.
4. How it decides: a fixed ordered ladder (read `:3619-3848`).
   1. `remainder::check_witness` (unsigned `x % d`, d the root).
   2. `check_dividend_witness`.
   3. `check_multiple_witness`.
   4. The three correlated checkers.
   5. A direct literal remainder: no definitions, target `ExactIntegerRemainder` of the root by an integer literal, fixed carrier, unsigned or divisor not `-1` (`:3641-3672`); zero literal gives `ZeroDivisionLiteral`.
   6. Direct unsigned subtract `c - root` (`:3673-3695`), giving `CorrelatedUnsignedSubtract`.
   7. `identity_ring_target`: an exact or wrapping multiply with the root as an operand, or an exact add or subtract with the root on the left, with direct leaves; it returns a form with no steps (`:3570-3612`, `:3696-3701`).
   8. A shift-left target with the root as its value (`:3702-3715`), no steps.
   9. Then the general chain (`:3716-3847`): root must be a `Value` of a `Fixed` integer type; target must have the root's type; lists must be equal length and strictly increasing; each step goes through the four candidate appliers, and exactly one must match. A landing must be used exactly when given (`:3819`).
   10. If the chain's end differs from the target, one final in-place policy add or subtract is applied (`:3827-3839`).
5. Cases by shape: the ladder above is the fixed list. "Direct" shapes require empty definition and literal lists (`:3641-3642`, `:3673-3674`, `:3696-3697`, `:3702-3703`).
6. Same job elsewhere: the same exact-add/subtract/multiply/shift recognition appears in `bound_mapping` where `direct_math_leaf` is used to map the direct forms (e.g. `map_direct_add_bound` at `:940`, outside my range). `identity_ring_target` and those mappers must agree on which operand shapes count as leaves; both call `direct_math_leaf` (`:3595-3596`).
7. Unused: all of the `pub` entries have callers (counts in the table at the end).
8. Stated limits: none quoted.
9. Fail-open:
   - `identity_ring_target` and the shift-left branch return a form with `endpoint_steps: Vec::new()` and no check beyond operand shape (`:3600-3612`, `:3714`). Such a form has no step, so `integer_affine_truth_bounds` refuses it (`:2826`). The bound mappers, which accept a form with no steps for the direct add/subtract/multiply/shift cases, must check the evidence themselves. Traced for the tests (`:4499-4691`, `:4905-5000`): the shape of the premise is checked. Not traced for every caller.
   - For the shift-left branch, `count: _` is ignored (`:3708`). The count is not validated here; `map_direct_shift_left_bound` checks it later (`:1484-1526`). Traced.
   - `context.validate` is called only in some branches. In the general chain, `checked_axiom` validates each cited axiom (`:3754`, definition at `crate::checked_axiom!`, not read). The direct branches at `:3696-3715` do not call `context.validate` on the witness terms. Open: whether the caller validates the witness terms is not traced.

## 3976-4017: error enum and re-exports

`IntegerAffineWitnessError` (`:3977`, 23 variants, `Debug`, `Clone`, `PartialEq`, `Eq`). `debug_display!` and `checked_axiom!` come from the crate (`:4003-4004`). Re-exports at `:4009-4017`.

## 4019-7192: tests (read in full)

45 tests, grouped:

- `bound_mapping::correlated` (3 tests, `:4058`, `:4269`, `:4335`): replay add, subtract and negative-multiply correlated witnesses; landing mutations (swap order, wrong literal, boolean landing, redirected operand).
- `bound_mapping::direct` (5 tests, `:4499`, `:4694`, `:4779`, `:4843`, `:4905`): add (including i128 overflow), subtract, four-corner multiply, zero evidence orientation, shift-left count range.
- Flat tests in `bound_mapping` (7, `:5005`-`:5443`): unsigned and signed remainder, coefficient signs, shape rejections, mixed shifts, bitwise and mask, saturating endpoints. The saturating test loops over every byte input for every bound at 8 bits (`:5510-5537`).
- `witness_checking` (9, `:5609`-`:6156`).
- `wrapping_bounds` (7, `:6235`-`:6709`): the wrapping multiply test brute-forces every interval pair for 8-bit signed and unsigned (`:6284-6347`); divide and remainder tests run all i8 bounds; the add test checks the headroom evidence.
- `wrapping_subtraction` (7, `:6858`-`:7132`): brute-force soundness for subtrahend lists, exact evidence, in-place targets.
- Helpers at `:7179-7191`.

Judgments the tests pin (read):
- A direct signed remainder by `-1` returns `EmptyDefinitionChain` (`:5680`, comment at `:5681`: "a direct signed remainder cannot assume MIN / -1 is representable").
- A strict bound on a non-translation step gives `StrictBoundNotTranslation` (`:5069`, `:5394`, `:5543`, `:6525`).
- Wrapping evidence is demanded only on the side the addend can cross, and extra evidence gives `WrappingEvidenceUnexpected` (`:6769`, `:6978`).

Tests call `accept_certificate`, `check_certificate` from the crate root; no production code is only used by tests, except `value_context`, `affine_witness` (helpers from the crate root, not read).

## integer_rules.rs 1-208 (read in full)

1. Purpose: the module header says (`:1-9`) it is the "independently checked integer rules": closed integer evaluation, open-term ring normalization, fixed native integer readings, and cast, forbidden-root, shift and wrapping-equality witness replay. The first 208 lines hold the closed-integer comparison helper, the fixed-integer helpers, a re-export of `integer_affine`, and most of `integer_cast`.
2. Shape:
   - `closed_integer` module (`:13-33`): `compare_integer_math_terms` (`:23`).
   - `fixed_integer` module (`:35-73`): `fixed_native` (`:38`), `fixed_native_interval` (`:42`), `integer_value_as_i128` (`:52`), `fixed_integer_as_i128` (`:59`), `fixed_integer_term` (`:69`).
   - `pub(crate) use crate::integer_affine` (`:74`).
   - `integer_cast` module starts at `:75`: `CheckedIntegerCastChain` (`:86`), `check_integer_cast_chain_witness` (`:116`), and the private helpers `parameter_width_target` (`:228`), `partial_fixed_native_cast` (`:235`), `strict_fixed_native_widen` (`:246`). Its error enum is `IntegerCastChainWitnessError` (`:254`, 14 variants). The module continues past line 208 (the range ends mid-way through `check_integer_cast_bound_conversion` at `:284`; the first reader owns 209 onward).
   - I read lines 209-1542 by accident (a Read call overran). They are outside this unit; I have not written about them here, other than noting `integer_cast` ends near `:1040` and `integer_forbidden_root` starts at `:1041`. This is not coverage.
3. Inputs and outputs:
   - `compare_integer_math_terms(left, right) -> Result<Option<Ordering>, ClosedIntegerEvaluationError>`: first asks `ClosedIntegerEvaluator::compare`, then falls back to `open_terms::constant_difference_ordering` (`:27-31`).
   - `fixed_native` is true for non-address integer types of 8, 16, 32 or 64 bits (`:38-40`).
   - `fixed_native_interval` returns `(min, max)` as `i128` or `None` (`:42-50`); a `u64` maximum fits, so `u128`-wide types give `None` (inferred from `integer_value_as_i128` using `i128::try_from` at `:55`).
4. How it decides: `fixed_native` is keyed on the type's bit count (`:39`). `fixed_integer_as_i128` requires the term's own integer type to equal the requested one (`:64`).
5. Cases by shape: `fixed_native` lists "8 | 16 | 32 | 64" (`:39`). Cast chain checking (read to `:208`) accepts only fixed native intervals (`:124-125`), with a special case for parameter-width targets (`:192-199`).
6. Same job elsewhere: `fixed_native_interval` and `integer_value_as_i128` are used by `integer_affine.rs` via `fixed_integer_as_i128` (`witness_checking` imports at `integer_affine.rs:2840`), and by the cast code. `apply_shift_definition` checks `matches!(integer_type.bits(), Some(8 | 16 | 32 | 64))` directly (`integer_affine.rs:3237`) instead of calling `fixed_native` (read at both lines). That is a second spelling of the same list.
7. Unused or one-caller (counts from `grep -rw ... source-rust tools --include=*.rs | wc -l`):
   - `fixed_native_interval`: 14 lines, used in 1 file besides the two main ones.
   - `compare_integer_math_terms`: 16 lines across 5 other files.
   - `check_integer_cast_chain_witness`: 25 lines, 4 other files.
   - `check_integer_cast_bound_conversion`: 12 lines, 4 other files.
   - `integer_cast_truth_bounds` (outside my range): 4 lines, 2 other files.
8. Stated limits (quoted): `:189-191` "A parameter width has no interval to narrow by". `:225-227` "An exact conversion from a fixed native carrier into a parameter width".
9. Fail-open:
   - `compare_integer_math_terms` returns `Ok(None)` when neither evaluator can order the terms (`:31`). Callers must treat `None` as "unknown", not "true". Not traced across the 5 caller files.
   - `check_integer_cast_chain_witness` does not accept an empty chain (`:126-128`) and rejects cycles (`:186-188`). A parameter-width edge skips narrowing the interval (`:192-199`), and the stated reason is at `:189-191` (stated).
   - None seen otherwise in 1-208.

## Same-job observations across the whole gap

- Three separate checkers for "remainder by something": `remainder::check_witness` (`:1639`), `check_dividend_witness` (`:1757`), `check_multiple_witness` (`:2043`), plus the direct literal branch at `:3641-3672` and the `Remainder(i128)` arm of `apply_definition` (`:3090-3117`). Each returns `Ok(None)` on mismatch.
- Signed `-1` handling for division and remainder appears at `:2190-2195` (multiple), `:2745` (truth bounds), `:3109` (wrapping divide), `:3653` (direct remainder), and the divisor premise at `:1917`. Each spells the check separately.
- The `8 | 16 | 32 | 64` native-width list is at `integer_rules.rs:39` and `integer_affine.rs:3237`.

## Provisional rules (stated limits, summary)

The text "Provisional ... the kernel trusts this statement; the mathematical core does not derive it" is at `integer_affine.rs:1750`, `:1847` and `:2038` (grepped for "Provisional", 3 doc-comment hits plus 3 mentions at `:3536`, `:3545`, `:3553`). Those three rules (remainder-within-dividend, quotient-at-most-dividend, multiple-remainder) are accepted by the proof checker on trust. Not independently proven here.

## Coverage

- Read in full: `integer_affine.rs` 1401-7192 (5,792 lines); `integer_rules.rs` 1-208 (208 lines). Total 6,000 lines.
- Skimmed: none.
- Not read: none of this unit.
- Extra, not part of the unit and not recorded: `integer_rules.rs` 209-1542 came back from one Read call. I did not use it.
- Commands run: one Grep listing `fn/struct/enum/mod/impl/const/type/macro_rules` and `#[cfg(test)]` lines in `integer_affine.rs`; one bash loop of `grep -rw NAME source-rust tools --include=*.rs | wc -l` and `grep -rlw` for 17 names; one `grep -n Provisional`.

Call-site counts (lines mentioning the name, with the number of files other than `integer_affine.rs`/`integer_rules.rs`):

| Name | Lines | Other files |
| --- | --- | --- |
| `integer_affine_truth_bounds` | 9 | 2 |
| `check_integer_affine_witness` | 119 | 7 |
| `map_integer_affine_bound` | 116 | 6 |
| `check_integer_affine_bound_conversion` | 27 | 6 |
| `integer_affine_wrapping_evidence` | 25 | 2 |
| `direct_math_leaf` | 14 | 0 |
| `is_remainder_within_dividend` | 2 | 1 |
| `is_quotient_at_most_dividend` | 2 | 1 |
| `is_multiple_remainder` | 2 | 1 |
| `multiple_remainder_bounds` | 7 | 2 |
| `check_multiple_bound` | 2 | 0 |
| `preserves_strict_endpoint` | 2 | 0 |

`check_multiple_bound` and `preserves_strict_endpoint` show 0 other files and 2 lines: the definition and exactly one call inside `integer_affine.rs` (the call is outside my range).

## Five things a maintainer most needs to know about this unit

1. `check_integer_affine_witness` (`integer_affine.rs:3614`) is an ordered ladder of shape matchers. Each of the first checkers returns `Ok(None)` on mismatch and the next one tries. A witness that matches none of the direct shapes ends up in the general chain at `:3716-3847`, which demands a typed `Value` root of a fixed integer type.
2. Three of the rules here are accepted on trust: remainder-within-dividend (`:1750`), quotient-at-most-dividend (`:1847`) and multiple-remainder (`:2038`). The comments say the mathematical core does not derive them. The code checks only the evidence shape for them.
3. Whether a bound is accepted is decided by exact `Proposition` shape and conjunct order. Examples: the multiple-remainder premise must equal `Truth`, the single bound or the exact ordered conjunction (`:2378-2384`); the dividend-bound child must be `[bound, divisor premise]` in that order (`:1911`); the quotient child must be one of two exact pairs (`:1866-1887`).
4. `CheckedIntegerEndpointStep` has 22 variants. `integer_affine_truth_bounds` matches all of them without a wildcard (`:2704-2823`), so any new variant must be handled there. The production mappers in `bound_mapping` (before my range) also match steps; I did not read whether they use wildcards.
5. About 3,170 of the 5,792 lines are tests. Several are exhaustive over 8-bit inputs (`:5510`, `:6284`, `:6573`, `:6629`, `:6870`), so they act as soundness checks for the interval arithmetic.

## What you are unsure about

- Whether any caller of the `Ok(None)` and "no steps" forms (`:3600-3615`, `:3696-3715`) can reach acceptance without the bound mapper checking the shape. I traced only the tests.
- Whether a duplicate definition of one value in `semantic_axioms` is rejected upstream. `check_multiple_witness` picks the first matching axiom below `upper_axiom` (`:2225-2229`), while `check_branch` in `integer_rules.rs` (outside my range) picks the latest. I did not verify that distinction at the second site.
- The step count in `CheckedIntegerEndpointStep` (22) is a hand count of `:3452-3493`, not a command.
- Whether `check_multiple_bound` and `preserves_strict_endpoint` are called only from `bound_mapping` (lines 1-1400): grep says one call each; I did not read the call sites.
- The truth of the three provisional statements. I only recorded that the comments call them provisional.
