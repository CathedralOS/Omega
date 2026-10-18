# dependent_subtract

The relational subtraction rule: `count - used` under `used <= count` proves
exact and non-negative in exact `u32`, so the difference also fits exact
`as i32`. A `count <= 11` premise keeps the destination conversion standing.

The row is pinned by `tests/omega/{pass,fail}/facts/dependent_subtract_through_*`;
every positive records native exit 0.

## Places

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` |
| field | `field/main.omg` | `field/invalid/main.omg` |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` |
| call_result | `call_result/main.omg` | `call_result/invalid/main.omg` |
| match_join | `match_join/main.omg` | `match_join/invalid/main.omg` |
| const_name | `const_name/main.omg` | `const_name/invalid/main.omg` |
| other_state | `other_state/main.omg` | `other_state/invalid/main.omg` |
| other_machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` |
| target_body | `target_body/main.omg` | `target_body/invalid/main.omg` |

Every positive binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64` and `linux_arm64`, declares `Expected exit: 0`, and carries an
authored `expected_native_exit.txt` of zero. Each place copies the same fact
sentence and the same `subtracted(count, used)` operation; only how the two
operands reach that operation changes.

Run one place directly:

```sh
omega --check samples/cli/facts/dependent_subtract/<place>/main.omg
omega run --both samples/cli/facts/dependent_subtract/<place>/main.omg
```

`omega run --both` reports the native exit and the Terminal Psi exit; agreement
at zero is the place's runtime oracle.

## Reject twins

Each twin keeps the operation, the record and the construction shape identical
and removes only the `used <= count` premise at the subtraction site. Every twin
rejects with exit 1 at the `subtracted` call:

```
error: exact arithmetic in machine `subtracted` ... local `remaining` may
overflow `u32`: the operands are not provably in range
```

The `target_body` twin reports it once for each of its four target bodies.
Check a twin with `omega --check <place>/invalid/main.omg`.

## What this row establishes and what stays open

The row establishes transport of the relational bound fact and its carrier
lowering: a `used <= count` proposition proved at one site justifies the
subtraction at another through a local, a record field, an explicit parameter,
a completed call result, a covered match join, a const name, a successor state,
another machine, and each of the four target bodies. It also establishes
realization, because every place runs natively and under Terminal Psi.

Identity and invalidation stay with the storage: each place reads the operands
once through `Main` and the subtraction consumes those exact values, so a
rewrite of `count` or `j` between the read and the subtraction would invalidate
the fact. Composition is covered by the `call_result` and `other_machine`
places, which require the callee's `ensures result <= count` rather than an
ad-hoc read. Formation obligations are unchanged: `Main` carries its
`count <= 11` premise, and the guard that establishes `used <= count` is the
caller's `requires`.