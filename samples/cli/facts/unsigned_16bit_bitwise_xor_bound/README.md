# Unsigned 16-bit bitwise XOR bound

Fact: 16-bit unsigned bitwise XOR raw ^ 15u16 with bounded interval 40u16 <= xored <= 63u16 establishes 40u16 <= xored <= 63u16 across all nine places so exact subtractions 63u16 - xored and xored - 40u16 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

- **Claim ticket:** `d03c7cd70e424f6eb423a01e8afb35d6` (`antigravity/w37-2`)
- **Obligation class:** Contract precondition (`requires xored >= 40 && xored <= 63` / `requires val >= 40 && val <= 63`) and return postcondition (`ensures result >= 40 && result <= 63`)
- **Specification references:** [numeric values](wiki/spec/language/numeric_values.md), [integer certificates](wiki/spec/terminal-psi/integer_certificates.md), [storage](wiki/spec/resources/storage.md)

## Places

The nine places carry the 16-bit unsigned bitwise XOR bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Pinned Corpus Pass | Pinned Corpus Fail | Carrier / Transport |
| --- | --- | --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | — | — | 16-bit unsigned bitwise XOR establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_local_rejected` | bitwise XOR result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_field_rejected` | bitwise XOR result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_parameter_rejected` | bitwise XOR result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_call_result_rejected` | bitwise XOR result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_match_join_rejected` | bitwise XOR result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_const_name_rejected` | raw input provided by named compile-time constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_other_state_rejected` | bitwise XOR result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_other_machine_rejected` | bitwise XOR result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_bitwise_xor_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_16bit_bitwise_xor_bound_through_target_body_rejected` | bitwise XOR result verified across target-scoped machine bodies |

Run each place with `omega run --both <place>/main.omg`. Perturbing the demanded oracle check in `main.omg` (`transition check == 0` -> `transition check == 1`) causes both engines to trap (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`), proving that the oracle discriminates.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 16-bit unsigned integer bitwise XOR arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that bitwise XOR `raw ^ 15u16` on a bounded input (`raw = 59u16`) produces a bounded result `40u16 <= xored <= 63u16` (`59 ^ 15 == 52u16`), which proves that exact 16-bit unsigned subtractions `63u16 - xored` and `xored - 40u16` are provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 16-bit unsigned bitwise XOR `raw ^ 15u16` with bounded input `raw = 59u16` toggles the lowest four bits (`0b0011_1011 ^ 0b0000_1111 = 0b0011_0100`), producing `52u16`. Since `52` resides strictly in the interval `[40, 63]`, the consuming helper `verify_xored(xored: u16) -> i32 requires xored >= 40 && xored <= 63` discharges exact unsigned subtractions `63 - xored` (evaluating to 11) and `xored - 40` (evaluating to 12), verifying `(63 - xored) + (xored - 40) - 23 == 0` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 16-bit unsigned scalar preserves its invariant `[40, 63]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The bitwise XOR bound composes across all 9 standard places to satisfy the consuming requirement `xored >= 40 && xored <= 63` and discharge `63 - xored` and `xored - 40`:
  - `local`: bitwise XOR result is bound to a local variable `local_xored` before verification.
  - `field`: bitwise XOR result is stored in `XorCarrier.xored` record field and projected out.
  - `parameter`: bitwise XOR result is passed to machine parameter `check_param(val: u16)`.
  - `call_result`: helper machine `produce_xored()` computes bitwise XOR and returns the result across call boundary with ensures contract.
  - `match_join`: bitwise XOR result converges through match arms into a unified value.
  - `const_name`: input `RAW` is declared as a compile-time named constant `const RAW: u16 = 59;`.
  - `other_state`: bitwise XOR result is passed across a transition edge into successor machine `step(forwarded: u16)`.
  - `other_machine`: bitwise XOR result is forwarded through intermediate machine `runner()`.
  - `target_body`: bitwise XOR result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the XOR operand to `127u16` (where `xored` reaches `68u16 > 63u16`), proving that downstream contract verification `requires xored >= 40 && xored <= 63` strictly fails closed when the post-XOR value exceeds 63.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 63`, confirming that the proof engine soundly detects that when `xored` can be 68, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `IntegerBitwiseXor` operations without loss. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2-byte `u16` scalar storage and alignment, while calling plans assign 16-bit register homes (`cx`/`dx` on Windows x86_64, `di`/`si` on Linux x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA bitwise XOR instructions: 16-bit `xor ax, 15` on `x86_64` and `eor w0, w0, #15` on `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime multi-word XOR masking, 128-bit wide bitwise XOR intrinsics, and SIMD packed XOR logic vectorization.
