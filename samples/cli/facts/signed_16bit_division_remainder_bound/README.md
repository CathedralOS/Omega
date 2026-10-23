# signed_16bit_division_remainder_bound

// Fact: a 16-bit signed i16 remainder operation (dividend % divisor where dividend >= 0i16 && dividend <= 100i16, divisor >= 16i16 && divisor <= 16i16) establishes rem >= 0i16 && rem <= 15i16 so exact 16-bit signed subtraction 15i16 - rem is proved overflow-free across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`), with `ARITHMETIC-POLICY-REALIZATION` owning the numeric-policy realization
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Process exit](wiki/spec/language/process_exit.md)

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from remainder` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_rem` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_16bit_division_remainder_bound_through_<place>_exit` (nine positives: `main.omg`, `build.omg`, `expected_native_exit.txt`) and `tests/omega/fail/facts/signed_16bit_division_remainder_bound_through_<place>_rejected` (nine reject twins: `main.omg`, `build.omg`, `expected.txt`). Every fixture is a byte-identical copy of its place program, so the sample and the fixture cannot drift apart. The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Recorded from the shared opt-level 1 producer on macOS arm64: `corpus_outcomes.txt` gains nine `checked` rows carrying `check-target:macos_arm64` and nine `rejected ... expected` rows each carrying its own `cannot prove` diagnostic line, `corpus_native_macos_arm64.txt` gains nine `built ... exit:0` rows, and `corpus_both_macos_arm64.txt` gains nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`), with `ARITHMETIC-POLICY-REALIZATION` owning the numeric-policy realization.
- **Establishment:** Performing a 16-bit signed remainder operation `dividend % divisor` with bounded non-negative dividend (`dividend >= 0i16 && dividend <= 100i16`) and fixed positive divisor (`divisor >= 16i16 && divisor <= 16i16`) establishes exact remainder bounds `[0, 15]`. This interval proves that the exact signed subtraction `15i16 - rem` produces a non-negative result without risk of underflow or overflow. Widening the divisor bound (e.g. `divisor <= 17i16`, allowing remainder `16i16`) fails closed at compile time against `requires rem <= 15i16`.
- **Identity and lifetime:** The signed 16-bit remainder scalar preserves its exact 16-bit two's complement integer value across local bindings, struct fields (`RemResult`), parameter registers, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies.
- **Composition:** The established interval `[0, 15]` composes across all nine places, safely discharging downstream contracts `requires rem >= 0i16 && rem <= 15i16` and executing `15i16 - rem` without post-hoc caller guards.
- **Soundness boundaries:** Signed remainder operations require static certificates of non-zero divisor (`divisor != 0`) and overflow exclusion (`i16::MIN % -1 == 0`). In invalid twins, widening bounds violates downstream contract requirements, correctly rejecting at compile time rather than accepting undefined runtime remainder behavior.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives exact 16-bit signed remainder operations (`Rem` on signed 16-bit carriers) with divisor non-zero certificates and range bounds intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 16-bit integers (`i16`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`). Standard calling conventions place parameters in 16-bit register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return remainders in `AX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering sign-extends `ax` into `dx:ax` using `cwd` and executes 16-bit signed division `idiv r16`, taking the remainder from `dx`. On `arm64`, backend lowering sign-extends 16-bit operands via `sxth`, executes 32-bit signed division `sdiv w2, w0, w1`, and computes remainder via multiply-subtract `msub w0, w2, w1, w0` (`W` registers).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
