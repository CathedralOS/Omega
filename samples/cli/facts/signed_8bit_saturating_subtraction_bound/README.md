# Signed 8-bit saturating subtraction bound

Fact: 8-bit signed saturating subtraction lhs -| rhs with bounded interval 40i8 <= diff <= 90i8 establishes 40i8 <= diff <= 90i8 across all nine places so exact subtractions 90i8 - diff and diff - 40i8 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit signed saturating subtraction bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit signed saturating subtraction establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | saturating subtraction result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | saturating subtraction result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | saturating subtraction result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | saturating subtraction result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | saturating subtraction result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | saturating inputs provided by named compile-time constants |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | saturating subtraction result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | saturating subtraction result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | saturating subtraction result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Pinned Corpus Fixtures

The row is pinned under
`tests/omega/pass/facts/signed_8bit_saturating_subtraction_bound_through_*_exit`
and
`tests/omega/fail/facts/signed_8bit_saturating_subtraction_bound_through_*_rejected`.
Each positive fixture is a byte-identical copy of its `<place>/main.omg`; the
place's `build.omg` carries a fixture-unique `Build::application` name, binds
`ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and
`linux_arm64`, and records native exit 0 with Terminal Psi agreement. Each
reject twin is a byte-identical copy of its `<place>/invalid/main.omg` and
pins the refusal its widened inputs (`lhs = 105`, `rhs = 10` producing `95i8`)
actually produce: `cannot prove requires contract for call verify_diff`
(local, field, match_join, const_name, other_state), `cannot prove requires
contract for call check_param` (parameter), `cannot prove ensures contract for
exit from produce_diff` (call_result), `cannot prove requires contract for
call runner` (other_machine), and `cannot prove requires contract for call
exercise` (target_body).

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 8-bit signed integer saturating subtraction arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that saturating subtraction `lhs - rhs` (denoted `lhs -| rhs`) on bounded `i8 in Saturating` operands produces a bounded difference `40i8 <= diff <= 90i8`, which proves that exact 8-bit signed subtractions `90i8 - diff` and `diff - 40i8` are provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit signed saturating subtraction `lhs - rhs` on `lhs: i8 in Saturating` and `rhs: i8 in Saturating` operates in the saturating arithmetic domain, clamping results to `[-128, 127]` on underflow/overflow rather than trapping or wrapping. With bounded operands `lhs = 80` and `rhs = 15`, the evaluated difference `65i8` resides strictly in `[40, 90]` without reaching the saturation threshold. The consuming helper `verify_diff(diff: i8) -> i32 requires diff >= 40 && diff <= 90` discharges exact signed subtractions `90 - diff` (evaluating to 25) and `diff - 40` (evaluating to 25), verifying `headroom + margin == 50` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 8-bit signed scalar preserves its invariant `[40, 90]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The saturating subtraction bound composes across all 9 standard places to satisfy the consuming requirement `diff >= 40 && diff <= 90` and discharge `90 - diff` and `diff - 40`:
  - `local`: saturating subtraction result is bound to a local variable `local_diff` before verification.
  - `field`: saturating subtraction result is stored in `DiffCarrier.diff` record field and projected out.
  - `parameter`: saturating subtraction result is passed to machine parameter `check_param(val: i8)`.
  - `call_result`: helper machine `produce_diff()` computes saturating subtraction and returns the result across call boundary with ensures contract.
  - `match_join`: saturating subtraction result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: i8 in Saturating = 80; const RHS: i8 in Saturating = 15;`.
  - `other_state`: saturating subtraction result is passed across a transition edge into successor machine `step(forwarded: i8)`.
  - `other_machine`: saturating subtraction result is forwarded through intermediate machine `runner()`.
  - `target_body`: saturating subtraction result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 105, rhs = 10` (where `diff` reaches `95i8 > 90i8`), proving that downstream contract verification `requires diff >= 40 && diff <= 90` strictly fails closed when the post-saturating difference exceeds 90.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 90`, confirming that the proof engine soundly detects that when `diff` can be 95, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `SaturatingIntegerSubtract` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `i8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA saturating subtraction instructions: clamped 8-bit subtraction on `x86_64` and signed saturating subtraction `sqsub` on `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime saturation trapping certificates, 16-bit to 64-bit multi-precision saturating arithmetic, and SIMD packed signed saturating subtraction instructions.
