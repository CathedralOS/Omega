# Unsigned 64-Bit Bitwise OR Bound

Fact: 64-bit unsigned bitwise OR base | 32u64 with bounded interval 40u64 <= combined <= 63u64 establishes 40u64 <= combined <= 63u64 across all nine places so exact subtractions 63u64 - combined and combined - 40u64 are safe.

Every place performs the same operation: combine a bounded 64-bit unsigned base operand (`base in [16, 31]`) with constant mask `32u64` via bitwise OR (`|`), establishing the synthesized post-operation interval `combined in [48, 63] ⊆ [40, 63]`, discharge both headroom subtraction `63u64 - combined` and margin subtraction `combined - 40u64`, verify `headroom + margin == 23u64`, and exit 0. Only the transport mechanism of the operands and bitwise OR computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit unsigned bitwise OR, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 64-bit base operand and mask are copied into locals before bitwise OR and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 64-bit base operand and mask are stored in record fields before bitwise OR and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 64-bit operands before bitwise OR and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 64-bit operands before bitwise OR and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 64-bit operands and join before bitwise OR and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 64-bit operands before bitwise OR and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 64-bit operands before bitwise OR and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 64-bit operands to bitwise OR |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies performs 64-bit bitwise OR and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_or_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded `0` exit is strictly discriminative:
- At runtime, with `base == 20u64` and mask `32u64`, `combined == 52u64`, `headroom == 63u64 - 52u64 == 11u64`, `margin == 52u64 - 40u64 == 12u64`, and `headroom + margin == 23u64`. The transition `probe() == 23 { true -> done() _ -> bad() }` exits 0. A 1-token perturbation of the demanded value (changing `transition probe() == 23` to `transition probe() == 24`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` and `interp crash: Trap (host presentation agrees)`).
- Each place has an `invalid/` twin (`sample.json` + `expected.txt`) that breaks only the construct's own contract, input, or domain (never a post-hoc caller guard):
  - **Obligation class (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`):** Call contract precondition (`cannot prove requires contract for call`) when the `base` upper bound is widened from `31u64` to `64u64` (or constant `BASE` is set to `64u64`). Bitwise OR analysis synthesizes `combined in [32, 127]` (or `combined == 96`), which violates the required interval `combined >= 40 && combined <= 63` on `verify_combined`. Proof obligation checking fails closed at compile time and none reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_64bit_bitwise_or_bound/<place>/invalid/main.omg
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

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_or_bound_through_target_body_rejected/`

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega-latest.exe --filter cli/facts/unsigned_64bit_bitwise_or_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `0d451bb12c6742f9be98f4ab23772fe5`.
- **Requirement established:** Target-neutral compile-time 64-bit unsigned bitwise OR bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, bitwise OR (`|`) between a bounded 64-bit unsigned `u64` base operand (`base >= 16u64 && base <= 31u64`) and mask constant `32u64` establishes the synthesized post-operation interval (`combined >= 48u64 && combined <= 63u64 ⊆ [40, 63]`), discharging exact 64-bit subtractions `63u64 - combined` and `combined - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the range `[48, 63]` for `combined = base | 32u64` from input intervals `[16, 31]` and `[32, 32]` using common bit-prefix transfer (`ones` and `zeros` hull). The synthesized bounds prove `combined >= 40u64` and `combined <= 63u64` unconditionally and guarantee the value fits within `u64` (`>= 0 && <= 18446744073709551615`), justifying forming exact subtractions `63u64 - combined` and `combined - 40u64` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 64-bit scalar operand, mask, and bitwise OR result interval bounds retain their precision, 64-bit stored width, and non-negative invariants across copies into local variables, record fields (`OrPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 64-bit unsigned bitwise OR composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations, evaluating to `23u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`base <= 31u64` vs `base <= 64u64` or `BASE = 20u64` vs `BASE = 64u64`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `base <= 64u64`: bitwise OR interval synthesis establishes `combined in [32, 127]`, so the preconditions `combined >= 40 && combined <= 63` on `verify_combined` cannot be proved. Compilation strictly rejects with `cannot prove requires contract for call`. No unproved underflow or overflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 64-bit unsigned bitwise OR and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `IntegerBitwiseOr` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 64-bit stored width (8 bytes, align 8) for `u64` and record layouts (`OrPacket` size 16, align 8) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 64-bit arguments and return values to 64-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit bitwise OR instructions (`or r64, r64` on `x86_64`, `orr x, x, x` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector bitwise OR across packed 64-bit integer lanes, bitwise rotate operations, and dynamic mask synthesis.
