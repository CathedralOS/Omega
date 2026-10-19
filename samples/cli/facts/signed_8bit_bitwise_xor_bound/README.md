# Signed 8-bit Bitwise XOR Bound Fact Row

Fact: 8-bit signed bitwise XOR raw ^ 15i8 with bounded interval 40i8 <= xored <= 63i8 establishes 40i8 <= xored <= 63i8 across all nine places so exact subtractions 63i8 - xored and xored - 40i8 are safe.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded signed i8 operand and its bitwise XOR result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bitwise XOR result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bitwise XOR result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the bitwise XOR result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the bitwise XOR result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant operand is bitwise XORed with 15i8 |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bitwise XOR result to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the bitwise XOR result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 8-bit signed bitwise XOR and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_xor_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `raw ^ 15` with input `59i8` establishes xored value `52i8` within static interval `40i8..63i8`. Passing `xored` to `verify_xored(res)` satisfies its contract `res >= 40 && res <= 63`. `verify_xored` evaluates exact subtractions `63 - 52 == 11` and `52 - 40 == 12`, sums their values (23), and subtracts 23 returning `0i32`. In `main`, `transition res == 0` transitions to `state done() {}`, terminating with native exit 0. If bitwise XOR or contract satisfaction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin sets `raw >= 64 && raw <= 64` (or `const RAW: i8 = 64`), so `raw ^ 15` yields `79i8`. Calling `verify_xored(xored)` fails closed at compile time with `cannot prove requires contract for call verify_xored`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_8bit_bitwise_xor_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_8bit_bitwise_xor_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_xored`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `SIGNED-8BIT-BITWISE-XOR-BOUND` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 8-bit signed (`i8`) bitwise XOR bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 8-bit signed bitwise XOR of bounded inputs (`raw >= 59i8 && raw <= 59i8`) with `15i8` statically establishes `xored >= 40i8 && xored <= 63i8`, discharging downstream contract requirement `res >= 40 && res <= 63` and exact subtractions `63i8 - res` and `res - 40i8` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires raw >= 59 && raw <= 59`) and literal bitwise operand `15i8` introduce static interval facts for 8-bit signed operands. The bitwise XOR expression `raw ^ 15` synthesizes the exact xored value and interval `[59 ^ 15, 59 ^ 15] = [52, 52]` directly upon evaluation, falling completely within the declared contract domain `[40, 63]`. This discharges downstream precondition `res >= 40 && res <= 63` and underflow-free exact subtractions `63 - res` and `res - 40` without requiring any post-hoc caller equality guard (`transition x == 52`).
  - **Identity and lifetime:** The synthesized 8-bit signed xored value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, pattern match joins, named constants, state transitions, machine calls, and target-scoped bodies throughout their lifespans without sign pollution or unexpected promotion.
  - **Composition:** The 8-bit signed xored interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_xored(xored)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`raw >= 59` vs `raw >= 64`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `raw >= 64` is specified: the xored value becomes `79`, which violates the required domain `res >= 40 && res <= 63`, strictly rejecting with `cannot prove requires contract for call verify_xored`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 8-bit signed bitwise XOR operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 8-bit signed bitwise operation semantics. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 8-bit signed integer layout plan (`IntegerAt`: `i8` size 1, alignment 1) validates, and target `CallPlan`/`StatePlan` assign 8-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit bitwise XOR instructions (`xor r8b, imm8` on `x86_64`; `eor w, w, #imm` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `xor` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic signed bitwise XOR across negative operands with sign bit flipping, multi-operand signed XOR chains, and vector SIMD integer XOR optimizations.
