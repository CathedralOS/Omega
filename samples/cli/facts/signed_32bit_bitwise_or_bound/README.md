# Signed 32-Bit Bitwise OR Bound

Fact: 32-bit signed bitwise OR base | 32i32 with bounded interval 40i32 <= combined <= 63i32 establishes 40i32 <= combined <= 63i32 across all nine places so exact subtractions 63i32 - combined and combined - 40i32 are safe.

Every place performs the same operation: combine a bounded 32-bit signed base operand (`base in [16, 31]`) with constant mask `32i32` via bitwise OR (`|`), establishing the synthesized post-operation interval `combined in [48, 63] ⊆ [40, 63]`, discharge both headroom subtraction `63i32 - combined` and margin subtraction `combined - 40i32`, verify `headroom + margin == 23i32`, and exit 0. Only the transport mechanism of the operands and bitwise OR computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit signed bitwise OR, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 32-bit signed base operand and mask are copied into locals before bitwise OR and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 32-bit signed base operand and mask are stored in record fields before bitwise OR and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 32-bit operands before bitwise OR and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 32-bit operands before bitwise OR and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 32-bit operands and join before bitwise OR and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 32-bit operands before bitwise OR and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 32-bit operands before bitwise OR and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 32-bit operands to bitwise OR |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies performs 32-bit bitwise OR and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Pinned fixtures

The nine positive places and nine reject twins are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_or_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt` = `cannot prove requires contract for call`):**
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_or_bound_through_target_body_rejected/`

`tests/omega/corpus_native_windows_x86_64.txt` records nine real `built ... exit:0` rows, one per place, and `tests/omega/corpus_outcomes.txt` records all 18 pass and fail outcomes.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_or_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `base == 20i32` and mask `32i32`, `combined == 52i32`, `headroom == 63i32 - 52i32 == 11i32`, `margin == 52i32 - 40i32 == 12i32`, and `headroom + margin == 23i32`. The transition `probe() == 23 { true -> done() _ -> bad() }` exits 0. A 1-token perturbation of the demanded result (`transition probe() == 23` to `transition probe() == 24`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `base` upper bound is widened from `31i32` to `64i32` (or constant `BASE` set to `64i32`). Bitwise OR analysis synthesizes `combined in [32, 127]`, which fails the required interval `combined >= 40 && combined <= 63` on `verify_combined`. Proof obligation checking statically detects the violated contract under the **Contract precondition** obligation class, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_32bit_bitwise_or_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_32bit_bitwise_or_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 32-bit signed bitwise OR bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, bitwise OR (`|`) between a bounded 32-bit signed `i32` base operand (`base >= 16i32 && base <= 31i32`) and mask constant `32i32` establishes the synthesized post-operation interval (`combined >= 48i32 && combined <= 63i32 ⊆ [40, 63]`), discharging exact 32-bit subtractions `63i32 - combined` and `combined - 40i32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the range `[48, 63]` for `combined = base | 32i32` from input intervals `[16, 31]` and `[32, 32]` using common bit-prefix transfer (`ones` and `zeros` hull). The synthesized bounds prove `combined >= 40i32` and `combined <= 63i32` unconditionally and guarantee the value fits within `i32` (`>= -2147483648 && <= 2147483647`), justifying forming exact subtractions `63i32 - combined` and `combined - 40i32` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 32-bit signed scalar operand, mask, and bitwise OR result interval bounds retain their precision, 32-bit stored width, signedness, and non-negative invariants across copies into local variables, record fields (`OrPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 32-bit signed bitwise OR composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations, evaluating to `23i32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`base <= 31i32` vs `base <= 64i32` or `BASE = 20i32` vs `BASE = 64i32`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `base <= 64i32`: bitwise OR interval synthesis establishes `combined in [32, 127]`, so the preconditions `combined >= 40 && combined <= 63` on `verify_combined` cannot be proved. Compilation strictly rejects with `cannot prove requires contract for call`. No unproved underflow or overflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 32-bit signed bitwise OR and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `IntegerBitwiseOr` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 32-bit stored width (4 bytes, align 4) for `i32` and record layouts (`OrPacket` size 8, align 4) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 32-bit arguments and return values to 32-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit bitwise OR instructions (`or r32, r32` on `x86_64`, `orr w, w, w` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector bitwise OR across packed 32-bit integer lanes, bitwise rotate operations, and dynamic mask synthesis.
