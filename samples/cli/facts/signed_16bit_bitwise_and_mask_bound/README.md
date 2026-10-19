# Signed 16-bit bitwise AND mask bound

Fact: 16-bit signed bitwise AND mask raw & 63i16 with bounded interval 40i16 <= masked <= 63i16 establishes 40i16 <= masked <= 63i16 across all nine places so exact subtractions 63i16 - masked and masked - 40i16 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 16-bit signed bitwise AND mask bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 16-bit signed bitwise AND mask establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | bitwise AND result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | bitwise AND result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | bitwise AND result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | bitwise AND result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | bitwise AND result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | raw input provided by named compile-time constant |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | bitwise AND result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | bitwise AND result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | bitwise AND result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 16-bit signed integer bitwise AND mask arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that bitwise AND `raw & 63i16` on a bounded input (`raw = 116i16`) produces a bounded result `40i16 <= masked <= 63i16` (`116 & 63 == 52i16`), which proves that exact 16-bit signed subtractions `63i16 - masked` and `masked - 40i16` are provably underflow/overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 16-bit signed bitwise AND mask `raw & 63i16` with bounded non-negative input `raw = 116i16` clears higher bits, producing `52i16`. Since `52` resides strictly in the interval `[40, 63]`, the consuming helper `verify_masked(masked: i16) -> i32 requires masked >= 40 && masked <= 63` discharges exact signed subtractions `63 - masked` (evaluating to 11) and `masked - 40` (evaluating to 12), verifying `(63 - masked) + (masked - 40) - 23 == 0` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 16-bit signed scalar preserves its invariant `[40, 63]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The bitwise AND bound composes across all 9 standard places to satisfy the consuming requirement `masked >= 40 && masked <= 63` and discharge `63 - masked` and `masked - 40`:
  - `local`: bitwise AND result is bound to a local variable `local_masked` before verification.
  - `field`: bitwise AND result is stored in `MaskCarrier.masked` record field and projected out.
  - `parameter`: bitwise AND result is passed to machine parameter `check_param(val: i16)`.
  - `call_result`: helper machine `produce_masked()` computes bitwise AND and returns the result across call boundary with ensures contract.
  - `match_join`: bitwise AND result converges through match arms into a unified value.
  - `const_name`: input `RAW` is declared as a compile-time named constant `const RAW: i16 = 116;`.
  - `other_state`: bitwise AND result is passed across a transition edge into successor state `step(forwarded: i16)`.
  - `other_machine`: bitwise AND result is forwarded through intermediate machine `runner()`.
  - `target_body`: bitwise AND result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the mask to `127i16` (where `masked` reaches `116i16 > 63i16`), proving that downstream contract verification `requires masked >= 40 && masked <= 63` strictly fails closed when the post-mask value exceeds 63.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 63`, confirming that the proof engine soundly detects that when `masked` can be 116, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `IntegerBitwiseAnd` operations without loss. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2-byte `i16` scalar storage and alignment, while calling plans assign 16-bit register homes (`cx`/`dx` on Windows x86_64, `di`/`si` on Linux x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA bitwise AND instructions: 16-bit `and ax, 63` on `x86_64` and `and w0, w0, #63` on `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime bitwise mask manipulation intrinsics, multi-word bitwise logic vectorization, and SIMD packed bitwise operations.
