# Unsigned 64-Bit Shift Left Bound

64-bit unsigned left shift `raw << 2u32` with bounded interval `40u64 <= shifted <= 80u64` establishes `40u64 <= shifted <= 80u64` across all nine places so exact subtractions `80u64 - shifted` and `shifted - 40u64` are safe.

Every place performs the same operation: define bounded unsigned 64-bit input `raw` (`15..15`), evaluate exact 64-bit left shift `shifted: u64 = raw << 2u32`, transport the shifted value through the place, pass `shifted` to `verify_shifted(res: u64)` which requires `res >= 40 && res <= 80`, execute exact subtractions `80 - res` and `res - 40`, sum their values (40), and subtract 40 returning `0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the shifted value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit unsigned left shift evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded unsigned u64 operand and its left-shifted result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the left-shifted result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the left-shifted result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the left-shifted result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the left-shifted result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant operand is left-shifted by 2u32 bits |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the left-shifted result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the left-shifted result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 64-bit unsigned left shift and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_shift_left_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `raw << 2u32` with input `15u64` establishes shifted value `60u64` within static interval `40u64..80u64`. Passing `shifted` to `verify_shifted(res)` satisfies its contract `res >= 40 && res <= 80`. `verify_shifted` evaluates exact subtractions `80 - 60 == 20` and `60 - 40 == 20`, sums their values (40), and subtracts 40 returning `0i32`. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition res == 0` to `transition res == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin sets `raw >= 21 && raw <= 21` (or `const RAW: u64 = 21`), so `raw << 2u32` yields `84u64`. Calling `verify_shifted(shifted)` fails closed at compile time under the **Contract precondition** obligation class with `cannot prove requires contract for call verify_shifted`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_64bit_shift_left_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_shifted` |

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega-latest.exe --filter cli/facts/unsigned_64bit_shift_left_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_shifted`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_shift_left_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_shift_left_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 64-bit unsigned (`u64`) left shift bounds ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). Specifically, exact 64-bit unsigned left shift of bounded inputs (`raw >= 15u64 && raw <= 15u64`) by `2u32` bits statically establishes `shifted >= 40u64 && shifted <= 80u64`, discharging downstream contract requirement `res >= 40 && res <= 80` and exact subtractions `80u64 - res` and `res - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires raw >= 15 && raw <= 15`) and literal shift count `2u32 < 64` introduce static interval facts for 64-bit unsigned operands. The left shift expression `raw << 2u32` synthesizes the exact shifted value and interval `[15 << 2, 15 << 2] = [60, 60]` directly upon evaluation, falling completely within the declared contract domain `[40, 80]`. This discharges downstream precondition `res >= 40 && res <= 80` and underflow-free exact subtractions `80 - res` and `res - 40` without requiring any post-hoc caller equality guard (`transition x == 60`).
  - **Identity and lifetime:** The synthesized 64-bit shifted value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 64-bit unsigned shifted interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_shifted(shifted)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`raw >= 15` vs `raw >= 21`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `raw >= 21` is specified: the shifted value becomes `84`, which violates the required domain `res >= 40 && res <= 80`, strictly rejecting with `cannot prove requires contract for call verify_shifted`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 64-bit unsigned left shift operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 64-bit unsigned left shift semantics (`shift count < 64`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 64-bit unsigned integer layout plan (`IntegerAt`: `u64` size 8, alignment 8) validates, and target `CallPlan`/`StatePlan` assign 64-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit left shift instructions (`shl r64, imm8` on `x86_64`; `lsl x, x, #imm` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (REX.W prefix + opcode `shl` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-constant shift count interval propagation, and SIMD vector variable left shifts (`psllq`).
