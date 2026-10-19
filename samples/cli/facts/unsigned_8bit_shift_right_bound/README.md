# Unsigned 8-bit logical right shift bound

Fact: an 8-bit unsigned u8 logical right shift of a bounded input (val >= 160u8 && val <= 160u8) by 2u32 bits establishes shifted >= 40u8 && shifted <= 40u8 so exact 8-bit subtraction shifted - 40u8 is proved underflow-free across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit unsigned logical right shift bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit logical right shift establishes bound and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | shifted result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | shifted result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | shifted result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | shifted result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | shifted result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | shifted input provided by named compile-time constant |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | shifted result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | shifted result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | shifted result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 8-bit unsigned integer logical right shift bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that shifting bounded `u8` value (`val >= 160u8 && val <= 160u8`) right by `2u32` bits establishes the interval `shifted >= 40u8 && shifted <= 40u8`, which proves that exact 8-bit subtraction `shifted - 40u8 == 0u8` is provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit unsigned logical right shift `val >> 2u32` on `val: u8` with entry contract `requires val >= 160 && val <= 160` establishes the postcondition interval `[160 >> 2, 160 >> 2] = [40, 40]`. The compiler statically models the zero-extending logical right shift, proving that `shifted >= 40 && shifted <= 40` holds within 8-bit unsigned range without overflow or sign extension. The consuming helper `verify_shifted(shifted: u8) -> u8 requires shifted >= 40 && shifted <= 40` discharges exact unsigned subtraction `shifted - 40` without underflow.
- **Identity and lifetime:** The shifted 8-bit unsigned scalar preserves its invariant `[40, 40]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact logical right shift bound composes across all 9 standard places to satisfy the consuming requirement `shifted >= 40 && shifted <= 40` and discharge `shifted - 40`:
  - `local`: shifted result is bound to a local variable `local_shifted` before verification.
  - `field`: shifted result is stored in `ShiftCarrier.shifted` record field and projected out.
  - `parameter`: shifted result is passed to machine parameter `check_param(val: u8)`.
  - `call_result`: helper machine `produce_shifted()` computes the shift and returns the result across call boundary.
  - `match_join`: shifted result converges through match arms into a unified value.
  - `const_name`: input `VAL` is declared as a compile-time named constant `const VAL: u8 = 160;`.
  - `other_state`: shifted result is passed across a transition edge into successor state `step(val: u8)`.
  - `other_machine`: shifted result is forwarded through intermediate machine `runner()`.
  - `target_body`: shifted result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the input bound to `val >= 120 && val <= 160` (so `val == 120` yields `shifted == 30`), proving that downstream contract verification `requires shifted >= 40 && shifted <= 40` strictly fails closed when the post-shift bound allows values below 40.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... >= 40`, confirming that the proof engine soundly detects that when `shifted` can be 30, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Shift operations are lowered with verified shift count certificates (`count < 8`), preserving interval arithmetic bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `u8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA logical right shift instructions: 8-bit `shr r8, imm8` on `x86_64` and zero-extended `lsr w, w, #imm` on `arm64`. Exact unsigned subtraction lowers to 8-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime shift counts requiring masking certificates, 128-bit wide shifts across register pairs, and vector SIMD packed logical right shifts.
