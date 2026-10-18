# Checked IEEE guard for `I32::from_f64`

Checked IEEE guard `value == value && value >= 0.0 && value < 1000000000.0` proves `I32::from_f64(value)` is finite and in range.

The operation is `I32::from_f64(...)` under the checked IEEE guard `value == value && value >= 0.0 && value < 1000000000.0`. For `123.75` it returns `123`; for `2000000000.0`, `-1.0`, and `0.0 / 0.0` the guard takes the `false` branch and returns `0`. Each place copies that sentence and performs that conversion; only the transport of the guarded value changes.

| Place | Project | Value transport |
| --- | --- | --- |
| local | `local/` | a local copy of `value` read under the guard |
| field | `field/` | a record field initialized from `value` and read under the guard |
| parameter | `parameter/` | the parameter `value` read directly under the guard |
| call result | `call_result/` | a call whose `requires` carries the guard and whose `ensures` equates `result == value` |
| match join | `match_join/` | a covered boolean match whose arms both yield `value` |
| const name | `const_name/` | named `f64` constants `LOWER` and `UPPER` in the guard |
| another state | `other_state/` | a successor state whose `requires` carries the guard |
| another machine | `other_machine/` | a callee machine whose `requires` carries the guard |
| another target body | `target_body/` | four target-qualified machine bodies whose `requires` carries the guard |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/repro/local/main.omg
omega run samples/cli/facts/repro/field/main.omg
omega run samples/cli/facts/repro/parameter/main.omg
omega run samples/cli/facts/repro/call_result/main.omg
omega run samples/cli/facts/repro/match_join/main.omg
omega run samples/cli/facts/repro/const_name/main.omg
omega run samples/cli/facts/repro/other_state/main.omg
omega run samples/cli/facts/repro/other_machine/main.omg
omega run samples/cli/facts/repro/target_body/main.omg
```

Each place has an `invalid/` twin that omits the upper bound `< 1000000000.0` from the guard, leaving `value < 2147483648.0` unproven at `I32::from_f64`. Check a twin with `omega --check <place>/invalid/main.omg`.

The `local`, `field`, and `call_result` places drove checking repairs in `validation` (propagating proved IEEE non-NaN and interval bounds across local copies, single-variant struct field literals, and `ensures result == parameter` call postconditions), and `local`, `field`, and `match_join` drove Stage 04 flow repairs (transporting `FloatPredicate` across statement transfers and resolving reaching stores with unchanged source observations in `float_implications`). The row is pinned by `tests/omega/{pass,fail}/facts/ieee_guard_i32_from_f64_*` fixtures.
