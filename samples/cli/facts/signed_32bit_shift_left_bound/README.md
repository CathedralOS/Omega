# Signed 32-Bit Shift Left Bound

A 32-bit signed `i32` left shift of a bounded input (`val >= 15i32 && val <= 15i32`) by `2u32` bits establishes `shifted >= 60i32 && shifted <= 60i32` so exact 32-bit signed subtraction `shifted - 60i32` is proved overflow-free across all nine places.

Every place performs the same operation: define bounded signed 32-bit input `val` (`15..15`), evaluate exact 32-bit left shift `shifted: i32 = val << 2u32`, transport the shifted value through the place, pass `shifted` to `verify_shifted(shifted: i32)` which requires `shifted >= 60 && shifted <= 60`, execute exact subtraction `shifted - 60` returning `0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the shifted value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit signed left shift evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded signed i32 operand and its left-shifted result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the left-shifted result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the left-shifted result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the left-shifted result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the left-shifted result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant operand is left-shifted by 2u32 bits |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the left-shifted result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the left-shifted result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 32-bit signed left shift and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_shift_left_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `val << 2u32` with input `15i32` establishes shifted value `60i32` within static interval `60i32..60i32`. Passing `shifted` to `verify_shifted(shifted)` satisfies its contract `shifted >= 60 && shifted <= 60`. `verify_shifted` evaluates exact subtraction `60 - 60 == 0i32`, which returns `0i32`. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. If left shift or contract satisfaction returns an unexpected value, execution branches to `machine bad() { crash Trap; }`, causing exit 255.
- Each invalid twin weakens the `val` bound (e.g. `val >= 14 && val <= 15` or `const VAL: i32 = 14`), so `val << 2u32` can be `56i32`. Calling `verify_shifted(shifted)` fails closed at compile time with `cannot prove requires contract for call verify_shifted`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_32bit_shift_left_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_32bit_shift_left_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_shifted`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_32bit_shift_left_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_32bit_shift_left_bound_through_<place>_rejected` for the nine
places. Every pinned `main.omg`, `build.omg`, `expected.txt` and `expected_native_exit.txt` is
byte-identical to the sample place it names (108 `cmp` pairs), each positive fixture carries
`expected_native_exit.txt` = `0`, declares `Expected exit: 0`, and binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64`.

Measured on macOS arm64 (Darwin 27.0.1) with the `gen-9b001b8f6af` producer (`omega` sha256
`6d24393125b7…`, `corpus_runner` sha256 `cee00bbdce17…`) at revision `9b001b8f6af`:

- 10/10 applications (`main.omg` and the nine places) pass `omega run --both`
  (`native exit 0`, `interp exit: 0`);
- 9/9 reject twins exit 1 under `omega --check` with
  `cannot prove requires contract for call verify_shifted`, their authored fragment;
- corpus goldens: nine `checked` rows carrying `check-target:macos_arm64`, nine `rejected`
  rows satisfying their `expected.txt`, nine native `built` rows with `exit:0`, and nine
  Terminal-agreement rows carrying `interpreter-exit:0`/`interpreter:agrees`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 32-bit signed (`i32`) exact left shift bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 32-bit signed left shift of bounded inputs (`val >= 15i32 && val <= 15i32`) by `2u32` bits statically establishes `shifted >= 60i32 && shifted <= 60i32`, discharging downstream contract requirement `shifted >= 60 && shifted <= 60` and exact subtraction `shifted - 60i32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires val >= 15 && val <= 15`) and literal shift count `2u32 < 32` introduce static interval facts for 32-bit signed operands. The shift expression `val << 2u32` synthesizes the exact shifted interval `[15 << 2, 15 << 2] = [60, 60]` directly upon evaluation, falling completely within the representable signed 32-bit integer domain `[-2^31, 2^31 - 1]`. This discharges downstream precondition `shifted >= 60 && shifted <= 60` without requiring any post-hoc caller equality guard (`transition x == 60`).
  - **Identity and lifetime:** The synthesized 32-bit shifted value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 32-bit signed shifted interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_shifted(shifted)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`val >= 15` vs `val >= 14`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `val >= 14` is specified: the shifted interval expands to `[56, 60]`, which fails the required domain `shifted >= 60 && shifted <= 60`, strictly rejecting with `cannot prove requires contract for call verify_shifted`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 32-bit signed left shift operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 32-bit signed left shift certificates (`shift count < 32`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 32-bit signed integer layout plan (`IntegerAt`: `i32` size 4, alignment 4) validates, and target `CallPlan`/`StatePlan` assign 32-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit shift instructions (`shl r32, imm8` on `x86_64`; `lsl w, w, #imm` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `shl` with imm8 `2` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-constant shift count interval propagation, arithmetic right shift signed interval propagation, and SIMD vector variable shifts.
