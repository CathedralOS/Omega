# Signed 16-bit wrapping multiplication bound

Fact: 16-bit signed wrapping multiplication lhs *% rhs with bounded interval 50i16 <= prod <= 200i16 establishes 50i16 <= prod <= 200i16 across all nine places so exact subtractions 200i16 - prod and prod - 50i16 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 16-bit signed wrapping multiplication bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 16-bit signed wrapping multiplication establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | wrapping multiplication result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | wrapping multiplication result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | wrapping multiplication result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | wrapping multiplication result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | wrapping multiplication result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | wrapping inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | wrapping multiplication result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | wrapping multiplication result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | wrapping multiplication result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 16-bit signed integer wrapping multiplication arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that wrapping multiplication `lhs * rhs` (denoted `lhs *% rhs`) on bounded `i16 in Wrapping` operands produces a bounded product `50i16 <= prod <= 200i16`, which proves that exact 16-bit signed subtractions `200i16 - prod` and `prod - 50i16` are provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 16-bit signed wrapping multiplication `lhs * rhs` on `lhs: i16 in Wrapping` and `rhs: i16 in Wrapping` operates in the wrapping arithmetic domain, allowing modulo-65536 two's-complement wrapping without trapping. With bounded operands `lhs = 15` and `rhs = 8`, the evaluated product `120i16` resides strictly in `[50, 200]` without wrapping around 32767. The consuming helper `verify_prod(prod: i16) -> i32 requires prod >= 50 && prod <= 200` discharges exact signed subtractions `200 - prod` (evaluating to 80) and `prod - 50` (evaluating to 70), verifying `headroom + margin == 150` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 16-bit signed scalar preserves its invariant `[50, 200]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The wrapping multiplication bound composes across all 9 standard places to satisfy the consuming requirement `prod >= 50 && prod <= 200` and discharge `200 - prod` and `prod - 50`:
  - `local`: wrapping multiplication result is bound to a local variable `local_prod` before verification.
  - `field`: wrapping multiplication result is stored in `ProdCarrier.prod` record field and projected out.
  - `parameter`: wrapping multiplication result is passed to machine parameter `check_param(val: i16)`.
  - `call_result`: helper machine `produce_prod()` computes wrapping multiplication and returns the result across call boundary with ensures contract.
  - `match_join`: wrapping multiplication result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: i16 in Wrapping = 15; const RHS: i16 in Wrapping = 8;`.
  - `other_state`: wrapping multiplication result is passed across a transition edge into successor state `step(forwarded: i16)`.
  - `other_machine`: wrapping multiplication result is forwarded through intermediate machine `runner()`.
  - `target_body`: wrapping multiplication result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 22, rhs = 10` (where `prod` reaches `220i16 > 200i16`), proving that downstream contract verification `requires prod >= 50 && prod <= 200` strictly fails closed when the post-wrapping product exceeds 200.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 200`, confirming that the proof engine soundly detects that when `prod` can be 220, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `WrappingIntegerMultiply` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2-byte `i16` scalar storage and alignment, while calling plans assign 16-bit register homes (`ax`/`di`/`si` on x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA signed multiplication instructions: 16-bit multiplication `imul` on `x86_64` and `mul` on `arm64` truncated to 16 bits. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime multiplication overflow certificates, 32-bit to 64-bit multi-precision wrapping multiplication, and SIMD packed wrapping multiplication instructions.
