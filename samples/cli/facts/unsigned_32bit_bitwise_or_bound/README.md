# Unsigned 32-Bit Bitwise OR Bound

32-bit unsigned bitwise OR `base | 32u32` with bounded interval `40u32 <= combined <= 63u32` establishes `40u32 <= combined <= 63u32` across all nine places so exact subtractions `63u32 - combined` and `combined - 40u32` are safe.

Every place performs the same operation: define bounded unsigned 32-bit input `base` (`20..20`), evaluate exact 32-bit bitwise OR `combined: u32 = base | 32`, transport the combined value through the place, pass `combined` to `verify_combined(res: u32)` which requires `res >= 40 && res <= 63`, execute exact subtractions `63 - res` and `res - 40` returning `23u32`, and verify in `main` that `res == 23` exiting 0. Only the transport mechanism of the combined value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit unsigned bitwise OR evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded unsigned u32 operand and its bitwise OR result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bitwise OR result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bitwise OR result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the bitwise OR result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the bitwise OR result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant operand is bitwise ORed with 32u32 |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bitwise OR result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the bitwise OR result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 32-bit unsigned bitwise OR and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_or_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `base | 32` with input `20u32` establishes combined value `52u32` within static interval `40u32..63u32`. Passing `combined` to `verify_combined(res)` satisfies its contract `res >= 40 && res <= 63`. `verify_combined` evaluates exact subtractions `63 - 52 == 11` and `52 - 40 == 12`, returning `23u32`. In `main`, `transition res == 23` transitions to `machine done() {}`, terminating with native exit 0. Perturbing the transition guard by a single token (`transition res == 23` -> `transition res == 22` in `main.omg`) causes both engines to trap at runtime (`native exit -1073741795`, `interp crash: Trap (host presentation agrees)`).
- Each invalid twin sets `base >= 64 && base <= 64` (or `const BASE: u32 = 64`), so `base | 32` yields `96u32`. Calling `verify_combined(combined)` fails closed at compile time with `cannot prove requires contract for call verify_combined`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_bitwise_or_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\swarm\w26\worktrees\w26-6\target\debug\omega.exe --filter cli/facts/unsigned_32bit_bitwise_or_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_combined`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_32bit_bitwise_or_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_32bit_bitwise_or_bound_through_<place>_rejected` fixtures across all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`).
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Obligation class:** 32-bit unsigned bitwise OR interval bound (`wiki/spec/language/numeric_values.md`, `wiki/spec/terminal-psi/integer_certificates.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 32-bit unsigned (`u32`) bitwise OR bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 32-bit unsigned bitwise OR of bounded inputs (`base >= 20u32 && base <= 20u32`) with `32u32` statically establishes `combined >= 40u32 && combined <= 63u32`, discharging downstream contract requirement `res >= 40 && res <= 63` and exact subtractions `63u32 - res` and `res - 40u32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires base >= 20 && base <= 20`) and literal bitwise operand `32u32` introduce static interval facts for 32-bit unsigned operands. The bitwise OR expression `base | 32` synthesizes the exact combined value and interval `[20 | 32, 20 | 32] = [52, 52]` directly upon evaluation, falling completely within the declared contract domain `[40, 63]`. This discharges downstream precondition `res >= 40 && res <= 63` and underflow-free exact subtractions `63 - res` and `res - 40` without requiring any post-hoc caller equality guard (`transition x == 52`).
  - **Identity and lifetime:** The synthesized 32-bit combined value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 32-bit unsigned combined interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_combined(combined)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`base >= 20` vs `base >= 64`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `base >= 64` is specified: the combined value becomes `96`, which violates the required domain `res >= 40 && res <= 63`, strictly rejecting with `cannot prove requires contract for call verify_combined`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 32-bit unsigned bitwise OR operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 32-bit unsigned bitwise operation semantics. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 32-bit unsigned integer layout plan (`IntegerAt`: `u32` size 4, alignment 4) validates, and target `CallPlan`/`StatePlan` assign 32-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit bitwise OR instructions (`or r32, imm32` on `x86_64`; `orr w, w, #imm` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `or` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-constant bitwise operand interval propagation across non-power-of-two bitmasks, and SIMD vector bitwise OR operations (`por`).
