# signed_32bit_bitwise_and_mask_bound

// Fact: 32-bit signed bitwise AND mask raw & 63i32 with bounded interval 40i32 <= masked <= 63i32 establishes 40i32 <= masked <= 63i32 across all nine places so exact subtractions 63i32 - masked and masked - 40i32 are safe.

## Places

| Place | Positive Program | Invalid Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | — | — |
| `local` | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_masked` |
| `field` | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_masked` |
| `parameter` | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_parameter` |
| `call_result` | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract for exit from mask_op` |
| `match_join` | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract for exit from choose_mask` |
| `const_name` | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_masked` |
| `other_state` | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call step` |
| `other_machine` | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call forward` |
| `target_body` | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_masked` |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place exits 0 in both Terminal Psi and native execution (`omega run --both`).

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `raw & 63i32` with `raw = 116i32` produces `masked = 52i32` within static interval `40i32..63i32`, discharging exact subtractions `63i32 - masked` (`11i32`) and `masked - 40i32` (`12i32`) and returning `0i32`. In `main`, `transition probe() == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition probe() == 0` to `transition probe() == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the bitwise mask to `127i32` (producing `116 & 127 = 116i32 > 63i32`), failing closed at compile time under the **Contract precondition** / **Contract postcondition** obligation class. None reaches native execution.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/signed_32bit_bitwise_and_mask_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection.

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_32bit_bitwise_and_mask_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_32bit_bitwise_and_mask_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Establishment:** Performing a 32-bit signed bitwise AND `raw & 63i32` on bounded non-negative input `raw == 116i32` (evaluating to $116 \& 63 = 52i32$) statically proves that the masked result satisfies $40i32 \le masked \le 63i32$. The bitwise mask $63i32$ (`0x3F`) guarantees that all higher bits (including the sign bit) are cleared to zero, establishing a non-negative interval $[0, 63]$ where $116 \& 63 = 52$ falls strictly into $[40, 63]$. This interval statically discharges the preconditions of downstream verification and guarantees that exact subtractions `let headroom: i32 = 63i32 - masked` and `let margin: i32 = masked - 40i32` produce non-negative signed integers without arithmetic overflow. Widening the mask to `127i32` produces $116 \& 127 = 116$, which violates the contract and is rejected by proof checking at compile time.
- **Identity and lifetime:** The signed 32-bit masked scalar preserves its 32-bit value and refined interval $[40, 63]$ across local variables, struct fields (`MaskContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without sign corruption, bit truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval $[40, 63]$ composes across all nine places, allowing consuming verification logic and exact subtractions `63i32 - masked` and `masked - 40i32` to execute safely without post-hoc caller guards or dynamic checks.
- **Soundness boundaries:** Signed integer arithmetic and bitwise operations preserve sign invariants. Because the mask $63i32$ is non-negative and clears the sign bit, the result is provably non-negative. When invalid controls widen masks to allow values $> 63i32$, proof checking fails closed with precise contract violation diagnostics rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives signed bitwise AND operations (`BitAnd` on signed 32-bit carrier `i32`) with interval and contract obligations intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 32-bit integers (`i32`) settle into 4-byte layouts (`size_of = 4`, `align_of = 4`). Standard calling conventions place parameters in 32-bit register lanes (`ECX`/`EDX` on Windows `x86_64`, `EDI`/`ESI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `EAX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 32-bit `and` instructions on 32-bit general-purpose registers (`eax`, `ecx`, `edx`) with immediate 32-bit masks. On `arm64`, backend lowering selects 32-bit `and` on `W` registers.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all four hosted targets.
