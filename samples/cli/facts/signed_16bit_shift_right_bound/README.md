# Signed 16-Bit Shift Right Bound

A 16-bit signed `i16` arithmetic right shift of a bounded input (`val >= 480i16 && val <= 480i16`) by `2u32` bits establishes `shifted >= 120i16 && shifted <= 120i16` so exact 16-bit signed subtraction `shifted - 120i16` is proved overflow-free across all nine places.

Every place performs the same operation: define bounded signed 16-bit input `val` (`480..480`), evaluate exact 16-bit arithmetic right shift `shifted: i16 = val >> 2u32`, transport the shifted value through the place, pass `shifted` to `verify_shifted(shifted: i16)` which requires `shifted >= 120 && shifted <= 120`, execute exact subtraction `shifted - 120` returning `0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the shifted value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit signed arithmetic right shift evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded signed i16 operand and its arithmetic right-shifted result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the arithmetic right-shifted result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the arithmetic right-shifted result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the arithmetic right-shifted result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the arithmetic right-shifted result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant operand is arithmetic right-shifted by 2u32 bits |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the arithmetic right-shifted result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the arithmetic right-shifted result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 16-bit signed arithmetic right shift and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_shift_right_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `val >> 2u32` with input `480i16` establishes shifted value `120i16` within static interval `120i16..120i16`. Passing `shifted` to `verify_shifted(shifted)` satisfies its contract `shifted >= 120 && shifted <= 120`. `verify_shifted` evaluates exact subtraction `120 - 120 == 0i16`, which returns `0i32`. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. If right shift or contract satisfaction returns an unexpected value, execution branches to `machine bad() { crash Trap; }`, causing exit 255.
- Each invalid twin weakens the `val` bound (e.g. `val >= 476 && val <= 480` or `const VAL: i16 = 476`), so `val >> 2u32` can be `119i16`. Calling `verify_shifted(shifted)` fails closed at compile time with `cannot prove requires contract for call verify_shifted`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_16bit_shift_right_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_16bit_shift_right_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_shifted`).

## Pinned Fixtures

The row is pinned by eighteen fixtures: nine positives under
`tests/omega/pass/facts/signed_16bit_shift_right_bound_through_*_exit` and nine
reject twins under
`tests/omega/fail/facts/signed_16bit_shift_right_bound_through_*_rejected`. Every
positive is a byte-for-byte copy of its place program and binds `ProgramEntry`
for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`,
`linux_arm64`); every twin is a byte-for-byte copy of its place's
`invalid/main.omg` and pins the compiler's complete rejection diagnostic in
`expected.txt`. Recorded on three legs: `corpus_outcomes.txt` (nine `checked`
positives carrying `check-target:macos_arm64` and nine `rejected ... expected`
twins), `corpus_native_macos_arm64.txt` (nine `built ... exit:0`) and
`corpus_both_macos_arm64.txt` (nine `built ... exit:0 interpreter-exit:0
interpreter:agrees`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 16-bit signed (`i16`) arithmetic right shift bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 16-bit signed arithmetic right shift of bounded inputs (`val >= 480i16 && val <= 480i16`) by `2u32` bits statically establishes `shifted >= 120i16 && shifted <= 120i16`, discharging downstream contract requirement `shifted >= 120 && shifted <= 120` and exact subtraction `shifted - 120i16` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires val >= 480 && val <= 480`) and literal shift count `2u32 < 16` introduce static interval facts for 16-bit signed operands. The shift expression `val >> 2u32` synthesizes the exact arithmetic right-shifted interval `[480 >> 2, 480 >> 2] = [120, 120]` directly upon evaluation, falling completely within the representable signed 16-bit integer domain `[-32768, 32767]`. This discharges downstream precondition `shifted >= 120 && shifted <= 120` without requiring any post-hoc caller equality guard (`transition x == 120`).
  - **Identity and lifetime:** The synthesized 16-bit shifted value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 16-bit signed shifted interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_shifted(shifted)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`val >= 480` vs `val >= 476`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `val >= 476` is specified: the shifted interval expands to `[119, 120]`, which fails the required domain `shifted >= 120 && shifted <= 120`, strictly rejecting with `cannot prove requires contract for call verify_shifted`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 16-bit signed arithmetic right shift operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 16-bit signed arithmetic right shift certificates (`shift count < 16`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 16-bit signed integer layout plan (`IntegerAt`: `i16` size 2, alignment 2) validates, and target `CallPlan`/`StatePlan` assign 16-bit integer arguments and return values across target registers and stack slots per ABI (sign-extended to 32/64-bit register lanes where required by platform conventions).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 16-bit arithmetic shift instructions (`sar r16, imm8` on `x86_64`; sign-extended `asr w, w, #imm` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `sar` with operand-size prefix `66h` and imm8 `2` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-constant shift count interval propagation, 16-bit logical right shift signed conversions, and SIMD vector variable right shifts (`psraw`).
