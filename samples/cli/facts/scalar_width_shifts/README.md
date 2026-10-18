# Scalar Width Shifts

A bounded shift count `shift < 4` on a `Wrapping` `u32` value preserves
in-width shift validity and recovers the original scalar across all nine places.

Every place performs the same operation: shift a `u32 in Wrapping` value (`21`) left
by a bounded shift count (`1`), verify the shifted scalar is `42`, shift right by the
same count to recover the original scalar `21`, and exit 0 only when both hold.
Only the transport of the operands changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | both the Wrapping u32 value and bounded shift count are copied into locals |
| field | `field/` | bounded record fields carry the Wrapping u32 value and shift count |
| parameter | `parameter/` | the shift machine reads its parameters directly |
| call result | `call_result/` | calls with declared types and bounds supply both operands |
| match join | `match_join/` | both arms keep the operands inside Wrapping and bounds, so the joined operands shift safely |
| const name | `const_name/` | `const SHIFT: u32 = 1` provides the named shift count |
| another state | `other_state/` | a successor state receives the operands and shifts there |
| another machine | `other_machine/` | a forwarding machine transports operands to the shift machine |
| another target body | `target_body/` | each of four target bodies shifts the Wrapping u32 value and shift count |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/scalar_width_shifts/local/main.omg
omega run samples/cli/facts/scalar_width_shifts/field/main.omg
omega run samples/cli/facts/scalar_width_shifts/parameter/main.omg
omega run samples/cli/facts/scalar_width_shifts/call_result/main.omg
omega run samples/cli/facts/scalar_width_shifts/match_join/main.omg
omega run samples/cli/facts/scalar_width_shifts/const_name/main.omg
omega run samples/cli/facts/scalar_width_shifts/other_state/main.omg
omega run samples/cli/facts/scalar_width_shifts/other_machine/main.omg
omega run samples/cli/facts/scalar_width_shifts/target_body/main.omg
```

Each place has an `invalid/` child that breaks the shift/width contract by using an
unqualified `u32` value without `in Wrapping`. The control must reject with
`may overflow `u32``. Check a twin with `omega --check <place>/invalid/main.omg`.

The row is pinned by `tests/omega/{pass,fail}/facts/scalar_width_shifts_through_*`
fixtures; every positive records native exit 0.

## Governance and Evidence

- Owning requirement: `wiki/spec/language/numeric_values.md#shift-counts`
- Parent task: `CONSERVATION-CONTRACT` (`TASKS.md`)
- Reserved ticket: `8bfead69f1114e37831c300710f126ca`

### Acceptance Demonstrated

1. **Establishment & Identity**: A `u32 in Wrapping` operand preserves in-width shift validity across arithmetic and bit-shift expressions, enabling left and right shifts without triggering exact arithmetic overflow obligations.
2. **Transport across Nine Places**: The Wrapping u32 scalar value and shift count survive transport across local bindings, struct fields, machine parameters, call return results, match join bindings, named constants, state transitions, machine forwarding, and target-scoped bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`).
3. **Soundness & Reject Twin Enforcement**: In each corresponding `invalid/` twin, omitting `in Wrapping` causes exact arithmetic verification to reject the unproven representability of the left shift (`may overflow `u32``).

### What Remains Open

- Full interval-set proof propagation and theorem propagation for non-wrapping arithmetic on dynamic counts.
- Cross-carrier shift count reduction proofs on composite structures.
