# Signed 8-Bit Exact Arithmetic Bound

An 8-bit signed `i8` exact addition of bounded inputs (`lhs >= 20i8 && lhs <= 45i8`, `rhs >= 15i8 && rhs <= 35i8`) establishes `sum >= 35i8 && sum <= 80i8` so exact 8-bit signed subtraction `80i8 - sum` is proved overflow-free across all nine places.

Every place performs the same operation: define bounded signed 8-bit parameters `lhs` (`20..45`) and `rhs` (`15..35`), evaluate exact 8-bit addition `sum: i8 = lhs + rhs`, transport the sum through the place, perform the consuming exact 8-bit subtraction `let headroom: i8 = 80 - sum`, evaluate with concrete inputs `lhs = 30i8` and `rhs = 25i8` (`sum = 55i8`, `headroom = 25i8`), and verify in `main` that `res == 25` exiting 0. Only the transport mechanism of the sum changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit signed exact arithmetic evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded signed i8 operands and their sum are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the sum of the 8-bit signed addition is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the sum of the 8-bit signed addition is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the sum of the 8-bit signed addition is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the sum so the joined binding preserves the addition bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs adds to the bounded lhs |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the sum to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the sum through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 8-bit signed exact addition and subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `lhs + rhs` with inputs `30i8` and `25i8` establishes sum `55i8` within static interval `35i8..80i8`. Because `35i8 <= sum <= 80i8`, the consuming exact subtraction `80i8 - sum` is provably safe within the signed 8-bit range `[-128, 127]` and yields `25i8`. `res == 25` is checked in `main`: `transition res == 25` transitions to `state done() {}`, terminating with native exit 0. If addition or subtraction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `rhs` bound (e.g. `rhs <= 90i8` or `const RHS: i8 = 90`), so `lhs + rhs` can reach `135` which exceeds the maximum representable 8-bit signed value `127`. Evaluating `lhs + rhs` fails closed at compile time with `error: exact arithmetic in machine ... may overflow \`i8\`: the operands are not provably in range`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_8bit_exact_arithmetic_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_8bit_exact_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`may overflow \`i8\``).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_exact_arithmetic_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_8bit_exact_arithmetic_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy
of its place twin's `main.omg`/`build.omg`/`expected.txt` (`may overflow \`i8\``). Positive versus twin
`git diff` changes only the `rhs` operand bound (`rhs <= 35` to `rhs <= 90`, or `const RHS: i8 = 25` to
`90`), which is the fact's own domain.

Measured on macOS arm64 with the factsr1 shared generation producer `gen-317bacd12b0` (omega sha256
`ecbd667e4dbae1b559f1f08f30dc10e3dbc369b8c8630327560227d06058a805`, corpus_runner sha256
`6065879534a8320403e0dd3918e86f77d1f025ff4e233f6740644b44460daa9c`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse with `exact arithmetic ... may overflow \`i8\`: the operands are not provably in range`.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `b9f31a0e74de`, so every row is stamped `@b9f31a0e74de+wt`: `tests/omega/corpus_outcomes.txt` gained
  36 rows (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with 28 diagnostic
  continuation lines), `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ... exit:0` rows and
  `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows. Every golden diff is a pure addition (0 deletions); all three legs exited 0
  with 0 unimplemented rows and 0 rows recording a lost success (the native leg's 2 `regressed-from`
  rows are the pre-existing memory-canary pins).

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state
  parameters `requires`, results `ensures`, locals the facts from initializers, guards and
  calls". This row is one customer group's nine-place transport and execution of that settled
  proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying
  numeric-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 8-bit signed (`i8`) exact arithmetic bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 8-bit signed addition of bounded inputs (`lhs >= 20i8 && lhs <= 45i8`, `rhs >= 15i8 && rhs <= 35i8`) statically establishes `sum >= 35i8 && sum <= 80i8`, discharging consuming 8-bit signed exact subtraction `80i8 - sum` without overflow across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 20 && lhs <= 45 && rhs >= 15 && rhs <= 35`) introduce static interval facts for 8-bit signed operands. The addition expression `lhs + rhs` synthesizes the exact sum interval `[20 + 15, 45 + 35] = [35, 80]` directly upon evaluation, falling completely within the representable signed 8-bit integer domain `[-128, 127]`. This establishes static bounds that discharge the consuming exact subtraction `80 - sum` (with result `[80 - 80, 80 - 35] = [0, 45]`) without requiring any post-hoc caller equality guard (`transition x == 55`).
  - **Identity and lifetime:** The synthesized 8-bit sum value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 8-bit signed interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction (`80 - sum`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the `rhs` bound (`rhs <= 35` vs `rhs <= 90`), leaving the 8-bit addition unproved and overflowing without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `rhs <= 90` is specified: the sum interval expands to `[35, 135]`, which breaches the maximum signed 8-bit bound `127`, strictly rejecting with `exact arithmetic in machine ... may overflow \`i8\`: the operands are not provably in range`. No unproved overflow breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 8-bit signed arithmetic operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 8-bit signed addition and subtraction certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 8-bit signed integer layout plan (`IntegerAt`: `i8` size 1, alignment 1) validates, and target `CallPlan`/`StatePlan` assign 8-bit integer arguments and return values across target registers and stack slots per ABI (sign-extended to 32/64-bit register lanes where required by platform conventions).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit arithmetic instructions (`add al, dl` / `sub al, dl` on `x86_64`; `add w0, w0, w1` / `sub w0, w0, w1` with 8-bit masking/sign-extension on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (byte-sized opcodes `add` and `sub` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** 8-bit signed saturated arithmetic domain conversions, vector SIMD 8-bit packed additions (`paddb`), and modular arithmetic domain conversions.
