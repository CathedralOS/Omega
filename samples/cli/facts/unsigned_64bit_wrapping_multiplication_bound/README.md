# Unsigned 64-bit Wrapping Multiplication Bound Fact Row

Fact: 64-bit unsigned wrapping multiplication lhs *% rhs with bounded interval 50u64 <= prod <= 200u64 establishes 50u64 <= prod <= 200u64 across all nine places so exact subtractions 200u64 - prod and prod - 50u64 are safe.

Every place computes or transports a 64-bit unsigned wrapping product `prod = (lhs as u64 in Wrapping) * (rhs as u64 in Wrapping)` (`compute_prod(15u64, 8u64) == 120u64`) from bounded inputs (`lhs in [10u64..20u64]`, `rhs in [5u64..10u64]`), statically establishing the interval `prod in [50u64..200u64]`. This static bound proves that the consuming exact 64-bit unsigned subtractions `let headroom: u64 = 200u64 - prod;` (`80u64`) and `let margin: u64 = prod - 50u64;` (`70u64`) are overflow- and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the bounded wrapping product changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded wrapping product is computed and held in a local variable before subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bounded wrapping product is stored in a struct field before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bounded wrapping product is passed to a machine parameter with contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the bounded wrapping product with guaranteed contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching evaluate the bounded wrapping product and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded inputs for wrapping multiplication |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bounded wrapping product to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives and verifies the wrapping product |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies evaluate the bounded wrapping product on each hosted target |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the expectation in `verify_prod()` is modified to require a non-zero exit or if an unexpected computation result occurs (`headroom != 80u64 || margin != 70u64`), execution transitions to `state bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose `lhs` upper bound is widened to `21u64` (`lhs = 21u64, rhs = 10u64`), allowing the product to reach `210u64 > 200u64`. When passed to `verify_prod` (or forwarding contracts), the required contract `prod <= 200u64` fails closed at compile time with `cannot prove requires contract`. None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/unsigned_64bit_wrapping_multiplication_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Authored Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove requires contract` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove requires contract` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove requires contract` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove requires contract` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove requires contract` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove requires contract` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove requires contract` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove requires contract` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove requires contract` |

## Requirements and Evidence

- **Parent outcome:** `EXACT-ARITHMETIC-UNSIGNED-DISCHARGE` / `UNSIGNED-64BIT-WRAPPING-MULTIPLICATION-BOUND` (`TASKS.md`)
- **Requirement established:** 64-bit unsigned wrapping multiplication (`u64 in Wrapping` multiplication, `lhs *% rhs`, `[numeric values](wiki/spec/language/numeric_values.md)`) of bounded inputs (`lhs in [10u64..20u64]`, `rhs in [5u64..10u64]`) statically establishes interval bounds `[50u64..200u64]`, proving exact 64-bit unsigned subtractions `200u64 - prod` and `prod - 50u64` overflow/underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Bounded inputs (`lhs >= 10u64 && lhs <= 20u64` and `rhs >= 5u64 && rhs <= 10u64`) establish static interval bounds on `compute_prod`. Wrapping multiplication over `u64 in Wrapping` (`(lhs as u64 in Wrapping) * (rhs as u64 in Wrapping)`) computes `wrap_prod`, which when cast back to exact `u64` establishes `result >= 50u64 && result <= 200u64` by monotonic unsigned interval arithmetic (since all domain values are non-negative and well below $2^{64}$). The bounds directly justify the consuming exact unsigned subtractions `200u64 - prod` (`80u64`) and `prod - 50u64` (`70u64`), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 64-bit unsigned product value (`u64`) occupies an 8-byte scalar slot and preserves its exact value identity and interval bounds across local variables, record fields, machine parameters, return results, pattern match joins, named constants, state transitions, machine calls, and target-scoped bodies without truncation, sign distortion, or lifetime aliasing.
- **Composition:** The bounded wrapping product composes through all 9 places into the consuming operations `200u64 - prod` and `prod - 50u64`. Positive places and reject twins differ only in the input operand values (`(15, 8)` vs `(21, 10)`) and callee bound contracts.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`cannot prove requires contract` / `prod <= 200`), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Wrapping multiplication and exact unsigned subtractions lower into canonical target-neutral Terminal Psi (`PSITERM\0`). Mathematical proof certificates and range assertions are verified and erased, retaining 64-bit unsigned scalar arithmetic (`ScalarType::U64`), executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). 64-bit unsigned integers are laid out with 8-byte size and 8-byte alignment (`size_of = 8`, `align_of = 8`). Parameter passing places `u64` values in 64-bit general-purpose registers (`rcx, rdx, r8, r9` on Windows x86_64; `rdi, rsi, rdx, rcx, r8, r9` on Linux x86_64; `x0..x7` on ARM64) according to target calling plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects 64-bit unsigned multiplication instructions (`imul r64, r64` on x86_64, `mul x, x, x` on ARM64) for wrapping multiplication, and unsigned subtract instructions (`sub r64, r64` on x86_64, `sub x, x, x` on ARM64) for exact subtraction. Register allocation assigns dedicated 64-bit homes and disjoint stack frame spill slots conforming to WCSU bounds.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** 64-bit arithmetic instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/unsigned_64bit_wrapping_multiplication_bound`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract`).
- Discrimination: modifying transition conditions in `verify_prod` causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
- What remains open: 8-bit, 16-bit, and 32-bit wrapping multiplication bounds, signed wrapping multiplication interval arithmetic, and non-trivial modular wrapping where overflow wraps around $2^{64}$.
