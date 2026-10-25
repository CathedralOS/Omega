# Narrow-to-Wide Signed 8-bit to 16-bit Sign Extension Fact Row

Fact: sign-extending a bounded 8-bit signed i8 value (narrow >= 30i8 && narrow <= 100i8) to i16 (narrow as i16) preserves the exact interval (wide >= 30i16 && wide <= 100i16), discharging exact 16-bit signed subtractions 100i16 - wide and wide - 30i16 across all nine places.

Owning Task: `EXACT-ARITHMETIC-SIGNED-DISCHARGE` / `NARROW-TO-WIDE-SIGNED-EXTENSION` (`TASKS.md`)
Claim Ticket: `c1fd0d91325a41a184352e60b8170fa9`

Every place computes or transports a 16-bit signed integer `wide = narrow as i16` (`compute_wide(60i8) == 60i16`) from bounded 8-bit input (`narrow in [30i8..100i8]`), statically establishing the interval `wide in [30i16..100i16]`. This static bound proves that the consuming exact 16-bit signed subtractions `let h1: i16 = 100i16 - wide;` (`40i16`) and `let h2: i16 = wide - 30i16;` (`30i16`) are overflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the bounded sign-extended value changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit to 16-bit signed sign extension evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded sign extension is computed and held in a local variable before subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bounded sign-extended value is stored in a struct field before subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bounded sign-extended value is passed to a machine parameter with contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the bounded sign-extended value with guaranteed contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching evaluate the bounded sign extension and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded inputs for sign extension |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bounded sign-extended value to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives and verifies the sign-extended value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies evaluate the bounded sign extension on each hosted target |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), carries an authored `expected_native_exit.txt` declaring `0`, and declares `Expected exit: 0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_8to16_extension/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is discriminating:
- In every positive place, `compute_wide(60i8)` produces `60i16` within static interval `30i16..100i16`, discharging exact subtractions `h1 = 100i16 - 60i16 == 40i16` and `h2 = 60i16 - 30i16 == 30i16` in `verify_wide`, returning `0i32`. A 1-token perturbation of the demanded result (`transition probe() == 0` to `transition probe() == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each place has an `invalid/` twin whose `narrow` upper bound is widened to `101i8` (`narrow = 101i8`), allowing the sign-extended value to reach `101i16 > 100i16`. When passed to `verify_wide` (or forwarding contracts), the required contract `wide <= 100i16` fails closed at compile time under the **Contract precondition** obligation class with `cannot prove requires contract`. None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/narrow_to_wide_signed_8to16_extension/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide from probe: wide >= 30 && wide <= 100` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide from probe: held.wide >= 30 && held.wide <= 100` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call check_param from probe: wide >= 30 && wide <= 100` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide from probe: wide >= 30 && wide <= 100` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide from probe: wide >= 30 && wide <= 100` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide from probe: wide >= 30 && wide <= 100` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call step from probe: wide >= 30 && wide <= 100` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call forward from probe: wide >= 30 && wide <= 100` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide from evaluate_wide: wide >= 30 && wide <= 100` |

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_8to16_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_8to16_extension_through_target_body_rejected/`

## Requirements and Evidence

- **Parent outcome:** `EXACT-ARITHMETIC-SIGNED-DISCHARGE` / `NARROW-TO-WIDE-SIGNED-EXTENSION` (`TASKS.md`)
- **Ticket:** `c1fd0d91325a41a184352e60b8170fa9`
- **Requirement established:** 8-bit to 16-bit signed sign extension (`i8 as i16`, [numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)) statically preserves interval bounds (`narrow in [30i8..100i8]` to `wide in [30i16..100i16]`), discharging exact 16-bit signed subtractions `100i16 - wide` and `wide - 30i16` without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Bounded inputs (`narrow >= 30i8 && narrow <= 100i8`) establish static interval bounds on `compute_wide`. The narrowing-to-widening sign extension `narrow as i16` establishes `result >= 30i16 && result <= 100i16` by monotonic sign-preserving interval arithmetic. The bounds directly justify the consuming exact signed subtractions `100i16 - wide` (`40i16`) and `wide - 30i16` (`30i16`), proving absence of signed underflow/overflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 16-bit sign-extended value (`i16`) occupies a 2-byte scalar slot (widened from 1-byte `i8`) and preserves its exact value identity and interval bounds across local variables, record fields, machine parameters, return results, pattern match joins, named constants, state transitions, machine calls, and target-scoped bodies without truncation, high-byte corruption, sign distortion, or lifetime aliasing.
- **Composition:** The bounded sign-extended value composes through all 9 places into the consuming operations `100i16 - wide` and `wide - 30i16`. Positive places and reject twins differ only in the input operand values (`60i8` vs `101i8`) and callee bound contracts.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`cannot prove requires contract` / `wide <= 100`), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed 8-to-16 widening casts and signed 16-bit subtractions lower into canonical target-neutral Terminal Psi (`PSITERM\0`). Mathematical proof certificates and range assertions are verified and erased, retaining 16-bit signed scalar arithmetic (`ScalarType::I16`), executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). 8-bit signed integers are laid out with 1-byte size and 1-byte alignment (`size_of = 1`, `align_of = 1`), while 16-bit signed integers are laid out with 2-byte size and 2-byte alignment (`size_of = 2`, `align_of = 2`). Parameter passing places `i16` values in 16-bit sub-registers (`cx, dx, r8w, r9w` on Windows x86_64; `di, si, dx, cx, r8w, r9w` on Linux x86_64; `w0..w7` on ARM64) according to target calling plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects signed widening `SignExtendI8`, which writes the full 64-bit normalized carrier (`movsx r64, r/m8`, REX.W, AT&T `movsbq`, on x86_64; `sxtb xN, wN` on ARM64) rather than a 16-bit or 32-bit destination. Exact subtraction also runs on the 64-bit carrier (`subq` / `sub xN`), not a 16-bit `sub`. Register allocation assigns disjoint stack frame spill slots conforming to WCSU bounds.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** 16-bit arithmetic and widening instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.
- **What remains open:** Negative range bounds sign extension interval arithmetic, vector SIMD sign-extension instructions, and saturating integer conversions.
