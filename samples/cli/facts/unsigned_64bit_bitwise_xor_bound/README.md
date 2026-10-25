# Unsigned 64-bit bitwise XOR bound

Fact: 64-bit unsigned bitwise XOR raw ^ 15u64 with bounded interval 40u64 <= xored <= 63u64 establishes 40u64 <= xored <= 63u64 across all nine places so exact subtractions 63u64 - xored and xored - 40u64 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 64-bit unsigned bitwise XOR bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 64-bit unsigned bitwise XOR establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | bitwise XOR result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | bitwise XOR result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | bitwise XOR result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | bitwise XOR result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | bitwise XOR result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | raw input provided by named compile-time constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | bitwise XOR result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | bitwise XOR result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | bitwise XOR result verified across target-scoped machine bodies |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_bitwise_xor_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `raw ^ 15u64` with input `59u64` establishes XOR value `52u64` within static interval `40u64..63u64`. Passing `xored` to `verify_xored(xored)` satisfies its contract `xored >= 40 && xored <= 63`. `verify_xored` evaluates exact subtractions `63 - 52 == 11` and `52 - 40 == 12`, sums their values (23), and subtracts 23 returning `0i32`. In `main`, `transition check == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition check == 0` to `transition check == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the XOR operand to `127u64`, so `raw ^ 127u64` yields `68u64 > 63u64`. Downstream contract verification fails closed at compile time under the **Contract precondition** / **Contract postcondition** obligation class with `cannot prove requires contract` (or `cannot prove ensures contract` in `call_result`). None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_64bit_bitwise_xor_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [local/invalid/main.omg](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| field | [field/invalid/main.omg](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| parameter | [parameter/invalid/main.omg](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| call result | [call_result/invalid/main.omg](call_result/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract` |
| match join | [match_join/invalid/main.omg](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| const name | [const_name/invalid/main.omg](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| a nested machine | [other_state/invalid/main.omg](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another machine | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another target body | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/unsigned_64bit_bitwise_xor_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection.

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_bitwise_xor_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 64-bit unsigned integer bitwise XOR arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`, [numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)), verifying that bitwise XOR `raw ^ 15u64` on a bounded input (`raw = 59u64`) produces a bounded result `40u64 <= xored <= 63u64` (`59 ^ 15 == 52u64`), which proves that exact 64-bit unsigned subtractions `63u64 - xored` and `xored - 40u64` are provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 64-bit unsigned bitwise XOR `raw ^ 15u64` with bounded input `raw = 59u64` toggles the lowest four bits (`0b0011_1011 ^ 0b0000_1111 = 0b0011_0100`), producing `52u64`. Since `52` resides strictly in the interval `[40, 63]`, the consuming helper `verify_xored(xored: u64) -> i32 requires xored >= 40 && xored <= 63` discharges exact unsigned subtractions `63 - xored` (evaluating to 11) and `xored - 40` (evaluating to 12), verifying `(63 - xored) + (xored - 40) - 23 == 0` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 64-bit unsigned scalar preserves its invariant `[40, 63]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The bitwise XOR bound composes across all 9 standard places to satisfy the consuming requirement `xored >= 40 && xored <= 63` and discharge `63 - xored` and `xored - 40`:
  - `local`: bitwise XOR result is bound to a local variable `local_xored` before verification.
  - `field`: bitwise XOR result is stored in `XorCarrier.xored` record field and projected out.
  - `parameter`: bitwise XOR result is passed to machine parameter `check_param(val: u64)`.
  - `call_result`: helper machine `produce_xored()` computes bitwise XOR and returns the result across call boundary with ensures contract.
  - `match_join`: bitwise XOR result converges through match arms into a unified value.
  - `const_name`: input `RAW` is declared as a compile-time named constant `const RAW: u64 = 59;`.
  - `other_state`: bitwise XOR result is passed across a transition edge into successor machine `step(forwarded: u64)`.
  - `other_machine`: bitwise XOR result is forwarded through intermediate machine `runner()`.
  - `target_body`: bitwise XOR result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the XOR operand to `127u64` (where `xored` reaches `68u64 > 63u64`), proving that downstream contract verification `requires xored >= 40 && xored <= 63` strictly fails closed when the post-XOR value exceeds 63.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 63`, confirming that the proof engine soundly detects that when `xored` can be 68, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `IntegerBitwiseXor` operations without loss. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 8-byte `u64` scalar storage and alignment, while calling plans assign 64-bit register homes (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA bitwise XOR instructions: 64-bit `xor r64, 15` on `x86_64` and `eor x0, x0, #15` on `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime multi-word XOR masking, 128-bit wide bitwise XOR intrinsics, and SIMD packed XOR logic vectorization.
