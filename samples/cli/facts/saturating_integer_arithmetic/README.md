# Saturating Integer Arithmetic

Clamping or bounding an unsigned sum/value at a ceiling (`clamped >= 10u64 && clamped <= 100u64`) establishes both lower and upper interval bounds across all nine places, discharging exact subtraction `100u64 - clamped` and `clamped - 10u64` without overflow.

Every place performs the same operation: establish or pass an unsigned scalar value clamped to `[10u64..100u64]` (`clamped = 60u64`), discharge both exact subtractions `let headroom: u64 = 100u64 - clamped;` and `let above_floor: u64 = clamped - 10u64;`, and verify that the combined interval sum `headroom + above_floor == 90u64` holds, exiting 0 only when both hold. Only the transport of the bounded operand changes across the nine places.

## Places

| Place | Positive | Fact-sensitive reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | root clamped u64 value |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | clamped operand copied into a local before subtraction |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | bounded record field carries the clamped value |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | compute machine reads its bounded parameter directly |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | helper call returns bounded result |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | both match arms preserve the interval bounds at join |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constant supplies the clamped value |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | successor state receives the bounded operand |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | forwarding machine transports the bounded value |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each of four target bodies evaluates the clamped arithmetic |

Match the fact sentence beside every place: each positive place is pinned as `tests/omega/pass/facts/saturating_integer_arithmetic_through_<place>_exit` and each reject twin as `tests/omega/fail/facts/saturating_integer_arithmetic_through_<place>_rejected`, byte-equal to the place and `invalid/` programs beside them.

Each positive place and the root `main.omg` binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`, carries `expected_native_exit.txt` = 0, and passes `omega run --both` — Terminal Psi and native execution agreeing:

```sh
omega run --both samples/cli/facts/saturating_integer_arithmetic/main.omg
omega run --both samples/cli/facts/saturating_integer_arithmetic/<place>/main.omg
```

## Reject twins

Each place has an `invalid/` twin that widens the ceiling bound to `101u64` (`clamped <= 101u64`), so the exact subtraction `100u64 - clamped` is no longer provably non-negative in unsigned arithmetic and fails closed at compile time with `may overflow \`u64\``.

Check a twin with `omega --check samples/cli/facts/saturating_integer_arithmetic/<place>/invalid/main.omg`.

| Place | Reject Twin | Authored diagnostic fragment |
| --- | --- | --- |
| local | [local/invalid/main.omg](local/invalid/main.omg) | `may overflow `u64`` |
| field | [field/invalid/main.omg](field/invalid/main.omg) | `may overflow `u64`` |
| parameter | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `may overflow `u64`` |
| call_result | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `may overflow `u64`` |
| match_join | [match_join/invalid/main.omg](match_join/invalid/main.omg) | `may overflow `u64`` |
| const_name | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `may overflow `u64`` |
| other_state | [other_state/invalid/main.omg](other_state/invalid/main.omg) | `may overflow `u64`` |
| other_machine | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `may overflow `u64`` |
| target_body | [target_body/invalid/main.omg](target_body/invalid/main.omg) | `may overflow `u64`` |

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`)
- **Owning requirement:** `wiki/spec/language/numeric_values.md#integer-equality-and-order`
- **Requirement established:** Clamping an unsigned value at floor and ceiling bounds (`clamped >= 10u64 && clamped <= 100u64`) establishes exact interval bounds across source checking, Terminal Psi representation, backend code generation, and native execution across all nine places, discharging exact arithmetic subtractions without runtime overflow.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** The guarded clamping helper machine `clamp(val: u64)` and explicit interval preconditions (`clamped >= 10u64 && clamped <= 100u64`) establish both lower (`>= 10u64`) and upper (`<= 100u64`) bounds directly on the unsigned scalar prior to subtraction, without requiring any post-hoc caller equality guard (`transition x == 90`). The upper bound proves `100u64 - clamped >= 0` and the lower bound proves `clamped - 10u64 >= 0`, discharging exact arithmetic without underflow.
- **Identity and lifetime:** The clamped scalar value and its bounded interval `[10u64..100u64]` track the exact scalar version across value copies, struct fields, machine parameters, call return results, match join bindings, named constants, state transitions, machine forwarding, and target-scoped bodies without loss or invalidation.
- **Composition:** The bounded clamped value composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into both consuming exact subtractions (`100u64 - clamped` and `clamped - 10u64`) and their sum (`headroom + above_floor == 90u64`). Each positive place `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the ceiling bound (`clamped <= 100u64` vs `clamped <= 101u64`).
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compile-time verification diagnostics (`error: exact arithmetic in machine ... may overflow `u64``) when the ceiling bound is widened to `101u64`, refusing compilation rather than admitting potential underflow in `100u64 - clamped`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Proved interval bounds and non-overflowing exact subtractions lower without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`) exact integer arithmetic, erasing proof obligations while retaining integer certificates, state transitions, and target-scoped bodies, and executing with full agreement in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). `IntegerAt` 64-bit scalar layout geometry and record layout (`data Payload where val >= 10u64 && val <= 100u64 { val: u64; }`) validate, and `CallPlan`/`StatePlan` lower 64-bit scalar and aggregate carriers across registers and stack slots according to target ABIs.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target subtraction and addition instructions (`sub`, `add`), assigns register homes and disjoint stack-frame storage with live-across-call spill slots, and inserts entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Subtraction and comparison instruction forms and symbolic stack-frame displacements encode, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/saturating_integer_arithmetic`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow `u64``).
- Pinned record at `abda3e427bc9+wt` on macOS arm64: 18 fixture families (`..._through_<place>_{exit,rejected}` for the nine places) — 9 `checked` rows with `check-target:macos_arm64` and 9 `rejected ... expected` rows carrying the `may overflow `u64`` diagnostic in `corpus_outcomes.txt`; 9 `built ... exit:0` rows in `corpus_native_macos_arm64.txt`; 9 `exit:0 interpreter-exit:0 interpreter:agrees` rows in `corpus_both_macos_arm64.txt`. 0 keys lost, 0 timeout rows.
- Discrimination: corrupting the sum expectation (`transition total == 90u64` to `transition total == 91u64`) triggers the `bad()` state and traps with `crash Trap`, proving the exit 0 oracle actively discriminates computation errors.
- Remaining open: saturating operations across variable non-constant dynamic bounds, and cross-carrier signed/unsigned saturating reduction proofs under `CONSERVATION-CONTRACT`.

