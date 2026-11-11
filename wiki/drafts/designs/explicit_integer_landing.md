# Explicit integer landing after fractional intermediates

Ratified 2026-11-06. Compiler migration and acceptance belong to
**OPERATOR-MACHINE-SUPPLY** in [TASKS.md](../../../TASKS.md).
Implemented for fixed-integer destinations: body validation, static
evaluation and Psi 02 const arguments reject implicit landing, and `as`
acknowledges its complete operand
(`tests/omega/{pass,fail}/arithmetic/*integer_landing*`,
`fail/arithmetic/implicit_fractional_const_landing`). Proof-level `Int` peers
still warn pending [an owner question](../../../OWNER_QUESTIONS.md#q1---does-explicit-landing-apply-at-a-proof-level-int-peer).

Anonymous arithmetic remains exact mathematical arithmetic. A fractional
intermediate can cancel to an integer, so `(5 / 2) * 2` has value `5` while
`(5u32 / 2) * 2` has value `4`. The old suppressible warning asked the author
to notice this difference. The ratified rule requires an explicit choice:
type an operand for integer arithmetic, or cast the complete anonymous
expression to land its exact final value.

## Decision

[Numeric landing](../../spec/language/numeric_values.md#fractional-intermediate-diagnostics)
rejects implicit integer landing after a fractional intermediate.
`((5 / 2) * 2) as u32` acknowledges that intermediate and lands as `5`.
The complete value must be integral, representable and satisfy the destination's
predicate and authority obligations. `as` cannot truncate, saturate, repair a
zero divisor or bypass an earlier argument/typed-operation landing.

Keep fractional-origin tracking through cancellation. Do not compare against a
hypothetical typed evaluation or distinguish `5` from `5.0`. Existing fractional
literal origins use the same tracking; decimal spelling is not an exemption.
Already-typed arithmetic, exact cross-carrier coercions, floating landing,
remainder formation and proof-level mathematical operations retain their rules.
This adds no syntax or arithmetic mode.

## Implementation route

1. Carry implicit versus explicit landing intent at the exact destination
   occurrence. A cast acknowledges only its complete anonymous operand; it
   does not acknowledge a sibling, an earlier call argument or every reuse of
   that expression. Preserve operation selection and target qualification.
2. Move the existing warning decision in
   [integer landing](../../../omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs)
   and [destination discovery](../../../omega-rust/psi/compilation/validation/src/literals_and_struct_literals.rs)
   into admission. An explicit cast still checks final integrality, range,
   predicates and authority before lowering. Remove warning emission and its
   suppression/reporting path; retain fractional-origin diagnostics.
3. Apply the same rule in
   [static evaluation](../../../omega-rust/psi/compilation/build-time-evaluation/src/const_evaluation/const_generic_expressions/value/match_dispatch.rs),
   including selected result edges and unselected expressions' formation.
   Preserve compositional rational bounds and all-arm checks without enumerating
   independent arm combinations. Do not replace an exact final value with a
   range merely to produce a diagnostic.
4. Sweep locals, fields, parameters, returns, transfers, match joins, constants,
   const arguments, extents, indices and proof integer peers. Anonymous numeric
   comparisons yielding Boolean have no integer landing to acknowledge.
   Cast outputs and named typed values never become anonymous again.
5. Migrate intentional exact calculations in the existing corpus and samples
   using whole-expression casts, including `anonymous_rational_integer_landing`,
   `anonymous_rational_arguments`, `match_anonymous_result_landing`,
   `rational_boolean_indices` and `const_record_computed`. Preserve arithmetic
   and expected values rather than rearranging formulas to avoid the trigger.
   Extend existing focused tests and re-pin affected corpus outcomes on a clean
   compiler commit; qualify executable controls with `omega run --both`.

## Acceptance controls

| Expression and destination | Required result |
| --- | --- |
| `let x: u32 = (5 / 2) * 2;` | Reject; suggest a whole-expression cast or typed operand. |
| `let x: u32 = ((5 / 2) * 2) as u32;` | Accept `5`, with no landing warning. |
| `let x: u32 = (5u32 / 2) * 2;` | Accept `4`. |
| `let x: u32 = (5 / 2) as u32;` | Reject the fractional final value. |
| `let x: u8 = (256 / 2 * 2) as u8;` | Reject range overflow. |
| `let x: u32 = (1 / 0 * 0) as u32;` | Reject division by zero. |
| `let x: u32 = (5 / 2) * 2 as u32;` | Reject: only `2` is cast; `5/2` cannot land at multiplication. |
| `let x: u32 = ((1 / 3 + 1 / 6) * 1000) as u32;` | Accept `500` without rearranging the formula. |
| `let x: u32 = ((5.0 / 2) * 2) as u32;` | Accept `5`; removing the cast rejects just as for `5`. |
| `let x: u32 = (0.5 * 4096) as u32;` | Accept `2048`; preserve the literal's fractional origin at implicit landing. |

Also retain controls for cancellation to zero, typed generic operations,
floating destinations, qualified targets, exact typed `f64`-to-`i32` casts,
and unbounded integral intermediates landing in a smaller integer. A cast
outside a call must not excuse a fractional history landing at its argument.
Pin accepted explicit versus rejected implicit forms across the existing
transport fixtures; retain final-fraction and range reject twins.
