# Explicit integer landing after fractional intermediates

The rules live in
[fractional-intermediate diagnostics](../../spec/language/numeric_values.md#fractional-intermediate-diagnostics);
this note keeps only the acceptance controls for **OPERATOR-MACHINE-SUPPLY** in
[TASKS.md](../../../TASKS.md). The kept rejection fixtures are
`tests/fixtures/fail/arithmetic/fractional_integer_landing` and
`tests/fixtures/fail/arithmetic/implicit_fractional_integer_landing`.

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
| Anonymous `(5 / 2) * 2` at an `i` or `u` destination | Reject implicit landing; a complete explicit cast admits `5`. |
| Anonymous `(5 / 2) * 2` at mathematical `Int` or `Nat` | Accept `5` without acknowledgment. |
| Mathematical `/` with anonymous `(1 / 2 * 4)` divisor | Admit rational division without an intermediate integer landing. |
| An integral value already typed `Rat` at an integer-only use | Require explicit exact narrowing; do not make it anonymous again. |

Also retain controls for cancellation to zero, typed generic operations,
floating destinations, qualified targets, exact typed `f64`-to-`i32` casts,
and unbounded integral intermediates landing in a smaller integer. A cast
outside a call must not excuse a fractional history landing at its argument.
Pin accepted explicit versus rejected implicit forms across the existing
transport fixtures; retain final-fraction and range reject twins.
