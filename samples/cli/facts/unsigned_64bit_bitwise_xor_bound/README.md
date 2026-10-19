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
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | bitwise XOR result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | bitwise XOR result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | bitwise XOR result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 64-bit unsigned integer bitwise XOR arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that bitwise XOR `raw ^ 15u64` on a bounded input (`raw = 59u64`) produces a bounded result `40u64 <= xored <= 63u64` (`59 ^ 15 == 52u64`), which proves that exact 64-bit unsigned subtractions `63u64 - xored` and `xored - 40u64` are provably underflow-free across all nine standard places.

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
  - `other_state`: bitwise XOR result is passed across a transition edge into successor state `step(forwarded: u64)`.
  - `other_machine`: bitwise XOR result is forwarded through intermediate machine `runner()`.
  - `target_body`: bitwise XOR result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the XOR operand to `127u64` (where `xored` reaches `68u64 > 63u64`), proving that downstream contract verification `requires xored >= 40 && xored <= 63` strictly fails closed when the post-XOR value exceeds 63.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 63`, confirming that the proof engine soundly detects that when `xored` can be 68, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `IntegerBitwiseXor` operations without loss. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 8-byte `u64` scalar storage and alignment, while calling plans assign 64-bit register homes (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA bitwise XOR instructions: 64-bit `xor r64, 15` on `x86_64` and `eor x0, x0, #15` on `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime multi-word XOR masking, 128-bit wide bitwise XOR intrinsics, and SIMD packed XOR logic vectorization.
