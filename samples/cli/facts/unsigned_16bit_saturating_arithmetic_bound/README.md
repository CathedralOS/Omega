# Unsigned 16-bit Saturating Arithmetic Bound Fact Row

Fact: 16-bit unsigned saturating addition lhs +| rhs with bounded interval 350u16 <= sum <= 600u16 establishes 350u16 <= sum <= 600u16 across all nine places so exact subtractions 600u16 - sum and sum - 350u16 are safe.

Every place computes or transports a 16-bit unsigned saturating sum `sum = (lhs as u16 in Saturating) + (rhs as u16 in Saturating)` (`compute_sum(400u16, 80u16) == 480u16`) from bounded inputs (`lhs in [300u16..500u16]`, `rhs in [50u16..100u16]`), statically establishing the interval `sum in [350u16..600u16]`. This static bound proves that the consuming exact 16-bit unsigned subtractions `let headroom: u16 = 600u16 - sum;` (`120u16`) and `let margin: u16 = sum - 350u16;` (`130u16`) are overflow- and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the bounded saturating sum changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded saturating sum is computed and held in a local variable before subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bounded saturating sum is stored in a struct field before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bounded saturating sum is passed to a machine parameter with contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the bounded saturating sum with guaranteed contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching evaluate the bounded saturating sum and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded inputs for saturating addition |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bounded saturating sum to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives and verifies the saturating sum |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies evaluate the bounded saturating sum on each hosted target |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the expectation in `verify_sum()` is modified to require a non-zero exit or if an unexpected computation result occurs (`headroom != 120u16 || margin != 130u16`), execution transitions to `state bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose `rhs` upper bound is widened to `101u16` (`lhs = 500u16, rhs = 101u16`), allowing the sum to reach `601u16 > 600u16`. When passed to `verify_sum` (or forwarding contracts), the required contract `sum <= 600u16` fails closed at compile time with `cannot prove requires contract`. None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/unsigned_16bit_saturating_arithmetic_bound/<place>/invalid/main.omg
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

- **Parent outcome:** `EXACT-ARITHMETIC-UNSIGNED-DISCHARGE` / `UNSIGNED-16BIT-SATURATING-ARITHMETIC-BOUND` (`TASKS.md`)
- **Requirement established:** 16-bit unsigned saturating addition (`u16 in Saturating` addition, `lhs +| rhs`, `[numeric values](wiki/spec/language/numeric_values.md)`) of bounded inputs (`lhs in [300u16..500u16]`, `rhs in [50u16..100u16]`) statically establishes interval bounds `[350u16..600u16]`, proving exact 16-bit unsigned subtractions `600u16 - sum` and `sum - 350u16` overflow/underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Bounded inputs (`lhs >= 300u16 && lhs <= 500u16` and `rhs >= 50u16 && rhs <= 100u16`) establish static interval bounds on `compute_sum`. Saturating addition over `u16 in Saturating` (`(lhs as u16 in Saturating) + (rhs as u16 in Saturating)`) computes `sat_sum`, which when cast back to exact `u16` establishes `result >= 350u16 && result <= 600u16` by monotonic saturating interval arithmetic. The bounds directly justify the consuming exact unsigned subtractions `600u16 - sum` (`120u16`) and `sum - 350u16` (`130u16`), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 16-bit unsigned sum value (`u16`) occupies a 2-byte scalar slot and preserves its exact value identity and interval bounds across local variables, record fields, machine parameters, return results, pattern match joins, named constants, state transitions, machine calls, and target-scoped bodies without truncation, sign distortion, or lifetime aliasing.
- **Composition:** The bounded saturating sum composes through all 9 places into the consuming operations `600u16 - sum` and `sum - 350u16`. Positive places and reject twins differ only in the input operand values (`(400, 80)` vs `(500, 101)`) and callee bound contracts.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`cannot prove requires contract` / `sum <= 600`), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Saturating addition and exact unsigned subtractions lower into canonical target-neutral Terminal Psi (`PSITERM\0`). Mathematical proof certificates and range assertions are verified and erased, retaining 16-bit unsigned scalar arithmetic (`ScalarType::U16`), executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). 16-bit unsigned integers are laid out with 2-byte size and 2-byte alignment (`size_of = 2`, `align_of = 2`). Parameter passing places `u16` values in 16-bit sub-registers (`cx, dx, r8w, r9w` on Windows x86_64; `di, si, dx, cx, r8w, r9w` on Linux x86_64; `w0..w7` on ARM64) according to target calling plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects 16-bit unsigned saturating add instructions (or add with carry-clamp / `uqadd` on ARM64, and conditional mov on x86_64) and exact 16-bit subtract instructions (`sub r16, r16` / `sub w, w, w`). Register allocation assigns dedicated 16-bit homes and disjoint stack frame spill slots conforming to WCSU bounds.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** 16-bit saturating and exact arithmetic instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/unsigned_16bit_saturating_arithmetic_bound`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract`).
- Discrimination: modifying transition conditions in `verify_sum` causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
- What remains open: 8-bit, 32-bit, and 64-bit saturating integer arithmetic bounds, saturating subtraction `lhs -| rhs` bounds, and signed saturating arithmetic interval arithmetic.
