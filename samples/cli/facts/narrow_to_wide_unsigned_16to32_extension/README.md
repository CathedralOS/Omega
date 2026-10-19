# Narrow-to-wide unsigned 16-to-32 extension bound

Fact: zero-extending a bounded 16-bit unsigned u16 value (narrow >= 300u16 && narrow <= 500u16) to u32 (narrow as u32) preserves the exact interval (wide >= 300u32 && wide <= 500u32), discharging exact 32-bit subtractions 500u32 - wide and wide - 300u32 across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 16-bit to 32-bit unsigned zero extension bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 16-to-32 zero extension establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | widened result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | widened result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | widened result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | widened result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | widened result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | narrow input provided by named compile-time constant |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | widened result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | widened result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | widened result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 16-bit to 32-bit unsigned integer zero-extension bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that zero-extending a bounded `u16` scalar (`narrow >= 300u16 && narrow <= 500u16`) via `narrow as u32` preserves the interval `wide >= 300u32 && wide <= 500u32`, which proves that exact 32-bit unsigned subtractions `500u32 - wide` and `wide - 300u32` are provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 16-to-32 unsigned zero extension `narrow as u32` on `narrow: u16` with entry contract `requires narrow >= 300 && narrow <= 500` statically establishes the postcondition interval `[300, 500]` in `u32`. The compiler statically models the cast as lossless unsigned widening into 32-bit unsigned range `[0, 4294967295]`, proving that `wide >= 300 && wide <= 500` holds. The consuming helper `verify_wide(wide: u32) -> i32 requires wide >= 300 && wide <= 500` discharges exact unsigned subtractions `500 - wide` (evaluating in `[0, 200]`) and `wide - 300` (evaluating in `[0, 200]`) without underflow or truncation.
- **Identity and lifetime:** The widened 32-bit unsigned scalar preserves its invariant `[300, 500]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact widening bound composes across all 9 standard places to satisfy the consuming requirement `wide >= 300 && wide <= 500` and discharge `500 - wide` and `wide - 300`:
  - `local`: widened result is bound to a local variable `local_wide` before verification.
  - `field`: widened result is stored in `ExtCarrier.wide` record field and projected out.
  - `parameter`: widened result is passed to machine parameter `check_param(val: u32)`.
  - `call_result`: helper machine `produce_wide()` widens and returns the result across call boundary with ensures contract.
  - `match_join`: widened result converges through match arms into a unified value.
  - `const_name`: input `NARROW` is declared as a compile-time named constant `const NARROW: u16 = 400;`.
  - `other_state`: widened result is passed across a transition edge into successor state `step(forwarded: u32)`.
  - `other_machine`: widened result is forwarded through intermediate machine `runner()`.
  - `target_body`: widened result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the `narrow` upper bound to `narrow >= 300 && narrow <= 501` (where `wide` reaches `501u32 > 500u32`), proving that downstream contract verification `requires wide >= 300 && wide <= 500` strictly fails closed when the post-widening bound exceeds 500.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 500`, confirming that the proof engine soundly detects that when `wide` can be 501, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Unsigned widening casts are lowered with verified lossless widening certificates, preserving interval bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2-byte `u16` to 4-byte `u32` scalar layout conversion and alignment, while calling plans assign 32-bit register homes (`eax`/`edi`/`esi` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA zero-extension instructions: `movzx r32, r/m16` on `x86_64` and `uxth w, w` on `arm64`. Unsigned subtraction lowers to 32-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime sign/zero extension of arbitrary bitfield slices, 64-bit to 128-bit unsigned widening, and SIMD zero-extension unpack instructions.
