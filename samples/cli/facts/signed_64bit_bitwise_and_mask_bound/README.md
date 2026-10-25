# Signed 64-Bit Bitwise AND Mask Bound

Fact: 64-bit signed bitwise AND mask raw & 63i64 with bounded interval 40i64 <= masked <= 63i64 establishes 40i64 <= masked <= 63i64 across all nine places so exact subtractions 63i64 - masked and masked - 40i64 are safe.

Every place performs the same operation: combine a bounded 64-bit signed operand (`raw in [112, 127]`) with constant mask `63i64` via bitwise AND (`&`), establishing the synthesized post-operation interval `masked in [48, 63] ⊆ [40, 63]`, discharge both headroom subtraction `63i64 - masked` and margin subtraction `masked - 40i64`, verify `headroom + margin == 23i64`, and exit 0. Only the transport mechanism of the operands and bitwise AND computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit signed bitwise AND, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 64-bit signed operand and mask are copied into locals before bitwise AND and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 64-bit signed operand and mask are stored in record fields before bitwise AND and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 64-bit operands before bitwise AND and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 64-bit operands before bitwise AND and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 64-bit operands and join before bitwise AND and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 64-bit operands before bitwise AND and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 64-bit operands before bitwise AND and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 64-bit operands to bitwise AND |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies performs 64-bit bitwise AND and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/local/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/field/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_and_mask_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `raw == 116i64` and mask `63i64`, `masked == 52i64`, `headroom == 63i64 - 52i64 == 11i64`, `margin == 52i64 - 40i64 == 12i64`, and `headroom + margin == 23i64`. The transition `probe() == 23 { true -> done() _ -> bad() }` exits 0. A 1-token perturbation of the demanded result (`transition probe() == 23` to `transition probe() == 24`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `raw` lower bound is widened from `112i64` to `64i64` (or constant `RAW` set to `64i64`). Bitwise AND analysis synthesizes `masked in [0, 63]`, which fails the required interval `masked >= 40 && masked <= 63` on `verify_masked`. Proof obligation checking statically detects the violated contract under the **Contract precondition** obligation class, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_64bit_bitwise_and_mask_bound/<place>/invalid/main.omg
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
python tools/sample_native_gate.py --filter cli/facts/signed_64bit_bitwise_and_mask_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_and_mask_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `6353c824ab25419db47a93337863ee2b`.
- **Requirement established:** Target-neutral compile-time 64-bit signed bitwise AND mask bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, bitwise AND (`&`) between a bounded 64-bit signed `i64` operand (`raw >= 112i64 && raw <= 127i64`) and mask constant `63i64` establishes the synthesized post-operation interval (`masked >= 48i64 && masked <= 63i64 ⊆ [40, 63]`), discharging exact 64-bit subtractions `63i64 - masked` and `masked - 40i64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the range `[48, 63]` for `masked = raw & 63i64` from input intervals `[112, 127]` and `[63, 63]` using common bit-prefix transfer (`ones` and `zeros` hull). The synthesized bounds prove `masked >= 40i64` and `masked <= 63i64` unconditionally and guarantee the value fits within `i64` (`>= -9223372036854775808 && <= 9223372036854775807`), justifying forming exact subtractions `63i64 - masked` and `masked - 40i64` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 64-bit signed scalar operand, mask, and bitwise AND result interval bounds retain their precision, 64-bit stored width, signedness, and non-negative invariants across copies into local variables, record fields (`AndPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 64-bit signed bitwise AND composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations, evaluating to `23i64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`raw >= 112i64` vs `raw >= 64i64` or `RAW = 116i64` vs `RAW = 64i64`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `raw >= 64i64`: bitwise AND interval synthesis establishes `masked in [0, 63]`, so the preconditions `masked >= 40 && masked <= 63` on `verify_masked` cannot be proved. Compilation strictly rejects with `cannot prove requires contract for call`. No unproved underflow or overflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 64-bit signed bitwise AND and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `IntegerBitwiseAnd` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 64-bit stored width (8 bytes, align 8) for `i64` and record layouts (`AndPacket` size 16, align 8) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 64-bit arguments and return values to 64-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit bitwise AND instructions (`and r64, r64` on `x86_64`, `and x, x, x` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector bitwise AND across packed 64-bit integer lanes, sign-extended bitwise operations, and dynamic mask synthesis.
