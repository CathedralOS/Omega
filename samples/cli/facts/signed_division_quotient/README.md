# Signed Division Quotient

Fact: a signed i64 quotient from -300i64 divided by -3i64 equals 100i64 when subtracting 100i64.

Every place performs the same operation: compute the signed division of negative
dividend `-300i64` by negative divisor `-3i64`, transport the resulting `100i64` quotient,
verify it by casting to `u64` and performing exact subtraction `verified - 100u64`, and
return `0` as `i32` to exit `0`. Only the transport of the quotient changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | the quotient is copied into a local binding |
| field | `field/` | a record field carries the quotient into the subtraction check |
| parameter | `parameter/` | the quotient is passed as an explicit machine parameter |
| call result | `call_result/` | a call returns the quotient into the subtraction check |
| match join | `match_join/` | both match arms produce the quotient and join into the subtraction check |
| const name | `const_name/` | a named constant carries the quotient into the subtraction check |
| another state | `other_state/` | a successor state receives the quotient and performs the subtraction check |
| another machine | `other_machine/` | a forwarding machine forwards the quotient to the verifying machine |
| another target body | `target_body/` | each of four target bodies performs the division and verifies the quotient |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/signed_division_quotient/local/main.omg
omega run samples/cli/facts/signed_division_quotient/field/main.omg
omega run samples/cli/facts/signed_division_quotient/parameter/main.omg
omega run samples/cli/facts/signed_division_quotient/call_result/main.omg
omega run samples/cli/facts/signed_division_quotient/match_join/main.omg
omega run samples/cli/facts/signed_division_quotient/const_name/main.omg
omega run samples/cli/facts/signed_division_quotient/other_state/main.omg
omega run samples/cli/facts/signed_division_quotient/other_machine/main.omg
omega run samples/cli/facts/signed_division_quotient/target_body/main.omg
```

Each place has an `invalid/` child that breaks the quotient value by dividing
`-297i64` by `-3i64` (yielding `99`), causing exact subtraction `verified - 100u64`
to underflow. The control must reject with `may overflow `u64``. Check a twin with
`omega --check samples/cli/facts/signed_division_quotient/<place>/invalid/main.omg`.

The row is pinned by `tests/omega/pass/facts/signed_division_quotient_through_*_exit` and
`tests/omega/fail/facts/signed_division_quotient_unproved_*` fixtures; every positive
records native exit 0 and Terminal Psi execution agreement.

## Governance and Evidence

- Owning requirement: `wiki/spec/language/numeric_values.md#division-and-remainder` and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`
- Parent task: `CONSERVATION-CONTRACT` (`TASKS.md`)
- Active claim ticket: `fccfcd437ced4b829572fe9a9e015c3e`

### Acceptance Demonstrated

1. **Establishment & Identity**: End-to-end signed negative-by-negative integer division `(-300i64) / (-3i64)` satisfies `ExactDivisionDefined` with negative divisor `<= -2i64`, proving quotient is representable and exact `+100i64` from checking through proof admission, Terminal Psi, and native realization.
2. **Transport across Nine Places**: The signed `i64` quotient fact survives transport across local bindings, struct fields, machine parameters, call return results, match join bindings, named constants, state transitions, machine forwarding, and target-scoped bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`).
3. **Soundness & Reject Twin Enforcement**: In each corresponding `invalid/` twin, breaking the quotient value (yielding 99) makes `verified - 100u64` unproven, causing exact arithmetic verification to reject (`may overflow `u64``).

### What Remains Open

- Full interval-set proof propagation and theorem propagation for non-constant signed divisors.
- Mixed-sign wide division and remainder on dynamic operands across non-trivial control-flow joins.
