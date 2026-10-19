# Unsigned 8-bit saturating multiplication bound

Fact: 8-bit unsigned saturating multiplication lhs *| rhs with bounded interval 40u8 <= prod <= 150u8 establishes 40u8 <= prod <= 150u8 across all nine places so exact subtractions 150u8 - prod and prod - 40u8 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit unsigned saturating multiplication bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit unsigned saturating multiplication establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | saturating multiplication result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | saturating multiplication result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | saturating multiplication result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | saturating multiplication result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | saturating multiplication result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | saturating inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | saturating multiplication result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | saturating multiplication result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | saturating multiplication result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 8-bit unsigned integer saturating multiplication arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that saturating multiplication `lhs * rhs` (denoted `lhs *| rhs`) on bounded `u8 in Saturating` operands produces a bounded product `40u8 <= prod <= 150u8`, which proves that exact 8-bit unsigned subtractions `150u8 - prod` and `prod - 40u8` are provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit unsigned saturating multiplication `lhs * rhs` on `lhs: u8 in Saturating` and `rhs: u8 in Saturating` operates in the saturating arithmetic domain, clamping results to `[0, 255]` on overflow rather than trapping or wrapping. With bounded operands `lhs = 12` and `rhs = 8`, the evaluated product `96u8` resides strictly in `[40, 150]` without reaching the saturation threshold (255). The consuming helper `verify_prod(prod: u8) -> i32 requires prod >= 40 && prod <= 150` discharges exact unsigned subtractions `150 - prod` (evaluating to 54) and `prod - 40` (evaluating to 56), verifying `headroom + margin == 110` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 8-bit unsigned scalar preserves its invariant `[40, 150]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The saturating multiplication bound composes across all 9 standard places to satisfy the consuming requirement `prod >= 40 && prod <= 150` and discharge `150 - prod` and `prod - 40`:
  - `local`: saturating multiplication result is bound to a local variable `local_prod` before verification.
  - `field`: saturating multiplication result is stored in `ProdCarrier.prod` record field and projected out.
  - `parameter`: saturating multiplication result is passed to machine parameter `check_param(val: u8)`.
  - `call_result`: helper machine `produce_prod()` computes saturating multiplication and returns the result across call boundary with ensures contract.
  - `match_join`: saturating multiplication result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: u8 in Saturating = 12; const RHS: u8 in Saturating = 8;`.
  - `other_state`: saturating multiplication result is passed across a transition edge into successor state `step(forwarded: u8)`.
  - `other_machine`: saturating multiplication result is forwarded through intermediate machine `runner()`.
  - `target_body`: saturating multiplication result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 16, rhs = 10` (where `prod` reaches `160u8 > 150u8`), proving that downstream contract verification `requires prod >= 40 && prod <= 150` strictly fails closed when the post-saturating product exceeds 150.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 150`, confirming that the proof engine soundly detects that when `prod` can be 160, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `SaturatingIntegerMultiply` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `u8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA saturating multiplication instructions: clamped 8-bit multiplication on `x86_64` and `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime saturation trapping certificates, 16-bit to 64-bit multi-precision saturating multiplication, and SIMD packed unsigned saturating multiplication instructions.
