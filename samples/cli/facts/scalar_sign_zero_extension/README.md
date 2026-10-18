# Scalar Sign and Zero Extension

Sign-extending `-28i8` to `i64` and adding zero-extended `128u8` as `i64` yields `100u64` when subtracting `100u64` across all nine places.

Every place performs the same operation: sign-extend `-28i8` to `i64` through `((-28i8 as i16) as i32) as i64` (verifying hardware sign extension so `-28` remains `-28i64`), zero-extend `128u8` through `(((128u8 as u16) as u32) as u64) as i64` (verifying high-bit zero extension so `128` remains `128i64`), sum them (`-28i64 + 128i64 == 100i64`), convert `sum as u64` (`100u64`), transport the resulting scalar through the place, perform exact subtraction `extended - 100u64`, and exit 0.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | both sign- and zero-extended scalars are combined and transported through local bindings |
| field | `field/` | struct field `extended: u64` carries the extended sum |
| parameter | `parameter/` | machine parameter `param: u64` transports the extended sum |
| call result | `call_result/` | return result of callee `produce()` supplies the extended sum |
| match join | `match_join/` | both arms yield the extended sum into the joined local binding |
| const name | `const_name/` | named constant `SUBTRAHEND: u64 = 100` subtracts from the extended sum |
| another state | `other_state/` | successor state `compute(extended: u64)` receives the value and subtracts |
| another machine | `other_machine/` | forwarding machine passes the value through an intermediate machine call |
| another target body | `target_body/` | each of four target bodies computes and subtracts the extended sum |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/scalar_sign_zero_extension/local/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/field/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/parameter/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/call_result/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/match_join/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/const_name/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/other_state/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/other_machine/main.omg
omega run samples/cli/facts/scalar_sign_zero_extension/target_body/main.omg
```

Each place has an `invalid/` reject twin where the initial signed value is `-29i8` (so `-29i64 + 128i64 == 99i64`), causing exact subtraction `extended - 100u64` to fail compile-time proof checking with exact subtraction underflow (`may overflow `u64``). Check a twin with `omega --check <place>/invalid/main.omg`.

The row is pinned by `tests/omega/pass/facts/scalar_sign_zero_extension_through_*` and `tests/omega/fail/facts/scalar_sign_zero_extension_unproved_*` fixtures.

## Governance and Evidence

- Owning requirement: `wiki/spec/language/numeric_values.md#explicit-casts` and `wiki/spec/terminal-psi/integer_certificates.md#integercastbound`
- Parent task: `CONSERVATION-CONTRACT` (`TASKS.md`)
- Active claim ticket: `b9274c262b8d47488edd4d70db6b82b2`

### Acceptance Demonstrated

1. **Establishment & Identity**: Sign-extension of narrow-to-wide signed integers (`i8 -> i16 -> i32 -> i64`) and zero-extension of narrow-to-wide unsigned integers (`u8 -> u16 -> u32 -> u64`) preserve exact mathematical values across arithmetic evaluation and type conversions (`IntegerCastBound`).
2. **Transport across Nine Places**: The resulting extended value survives transport across all nine places (local bindings, struct fields, machine parameters, call return results, match join bindings, named constants, state transitions, machine forwarding, and target-scoped bodies for `linux_x86_64`, `linux_arm64`, `macos_arm64`, and `windows_x86_64`), enabling exact subtraction `extended - 100u64` to exit 0 under both Terminal Psi and native execution (`omega run --both`).
3. **Soundness & Reject Twin Enforcement**: In each corresponding `invalid/` reject twin, setting the initial signed value to `-29i8` causes the sum `99u64` to fail exact arithmetic subtraction `99u64 - 100u64` under compile-time interval and proof checking (`may overflow `u64``).

### What Remains Open

- Full range/domain proof propagation across dynamic cross-carrier bitcasts.
- Theorem-level signed-to-unsigned conversion preservation under general arithmetic constraints.
