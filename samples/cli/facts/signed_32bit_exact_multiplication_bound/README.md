# Signed 32-bit Exact Multiplication Bound Fact Row

Fact: a 32-bit signed i32 exact multiplication of bounded inputs (lhs >= 15i32 && lhs <= 25i32, rhs >= 10i32 && rhs <= 20i32) establishes prod >= 150i32 && prod <= 500i32 so exact 32-bit signed subtraction 500i32 - prod is proved overflow-free across all nine places.

Every place computes or transports a 32-bit signed integer product `prod = lhs * rhs` (`compute_prod(20, 15) == 300i32`) from bounded inputs (`lhs in [15i32..25i32]`, `rhs in [10i32..20i32]`), statically establishing the interval `prod in [150i32..500i32]`. This static bound proves that the consuming exact 32-bit signed subtraction `let headroom: i32 = 500i32 - prod;` (`200i32`) is overflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the bounded product changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded multiplication is computed and held in a local variable before subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bounded product is stored in a struct field before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bounded product is passed to a machine parameter with contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the bounded product with guaranteed contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching evaluate the bounded product and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded inputs for multiplication |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bounded product to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives and verifies the product |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies evaluate the bounded product on each hosted target |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_multiplication_bound/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the expectation in `probe()` is modified to require a non-zero exit or if an unexpected computation result occurs (`headroom != 200i32`), execution transitions to `machine bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose `rhs` upper bound is widened to `21i32` (`lhs = 25i32`, `rhs = 21i32`), allowing the product to reach `525i32 > 500i32`. When passed to `verify_prod`, the required contract `prod <= 500i32` fails closed at compile time with `cannot prove requires contract`. None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/signed_32bit_exact_multiplication_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Authored Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove requires contract` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove requires contract` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove requires contract` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove requires contract` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove requires contract` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove requires contract` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove requires contract` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove requires contract` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove requires contract` |

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_multiplication_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt` = that place's `cannot prove requires contract ... prod <= 500` fragment):**
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_multiplication_bound_through_target_body_rejected/`

Each fixture is a byte-for-byte copy of its place program and oracle from this
sample tree (`main.omg`, `build.omg`, `expected_native_exit.txt` / `expected.txt`),
so the sample and the fixture cannot drift apart. The nine positives record
`checked` with `check-target:macos_arm64` (check leg), `built ... exit:0`
(native leg) and `built ... exit:0 interpreter-exit:0 interpreter:agrees`
(Terminal Psi agreement leg); the nine twins record `rejected` with their
authored diagnostic fragment satisfied.

## Requirements and Evidence

- **Parent outcome:** `EXACT-ARITHMETIC-SIGNED-DISCHARGE` / `SIGNED-32BIT-MULTIPLICATION-BOUND` (`TASKS.md`)
- **Requirement established:** 32-bit signed (`i32`) exact multiplication of bounded integer values (`lhs in [15..25]`, `rhs in [10..20]`) statically establishes product interval bounds `[150..500]`, proving exact signed 32-bit subtraction `500i32 - prod` overflow-free across all nine language places without runtime arithmetic overflow traps.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Bounded inputs (`lhs >= 15i32 && lhs <= 25i32` and `rhs >= 10i32 && rhs <= 20i32`) establish static interval bounds on `compute_prod`. The multiplication `lhs * rhs` establishes `result >= 150i32 && result <= 500i32` by monotonic integer range arithmetic. The upper bound `prod <= 500i32` directly justifies the consuming exact signed subtraction `500i32 - prod`, ensuring non-negative headroom `[0..350]` without requiring post-hoc caller equality guards.
- **Identity and lifetime:** The 32-bit signed product value (`i32`) occupies a 4-byte scalar slot and tracks its exact scalar identity and interval bounds across local variables, record fields, machine parameters, return results, pattern match joins, named constants, state transitions, machine calls, and target-scoped bodies without truncation, sign distortion, or lifetime aliasing.
- **Composition:** The bounded product composes through all 9 places into the consuming operation `500i32 - prod`. Positive places and reject twins differ only in the input operand values (`(20, 15)` vs `(25, 21)`) and callee bound contracts.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`cannot prove requires contract` / `prod <= 500`), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed 32-bit multiplication and subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`). Mathematical proof certificates and range assertions are verified and erased, retaining 32-bit signed scalar arithmetic (`ScalarType::I32`), executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). 32-bit signed integers are laid out with 4-byte size and 4-byte alignment (`size_of = 4`, `align_of = 4`). Parameter passing places `i32` values in 32-bit sub-registers (`ecx, edx, r8d, r9d` on Windows x86_64; `edi, esi, edx, ecx, r8d, r9d` on Linux x86_64; `w0..w7` on ARM64) according to target calling plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects 32-bit signed multiply instructions (`imul r32, r32` on x86_64, `mul w, w, w` on ARM64) and 32-bit subtract instructions (`sub r32, r32` / `sub w, w, w`). Register allocation assigns dedicated 32-bit homes and disjoint stack frame spill slots conforming to WCSU bounds.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** 32-bit arithmetic instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/signed_32bit_exact_multiplication_bound`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract`).
- Discrimination: changing expected return value from 0 to 1 causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
- What remains open: 64-bit signed exact multiplication bounds, negative-factor signed multiplication interval arithmetic, and saturating/wrapping signed multiplication modes.
