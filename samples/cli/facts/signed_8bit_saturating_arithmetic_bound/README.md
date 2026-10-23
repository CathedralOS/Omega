# Signed 8-bit saturating arithmetic bound

Fact: 8-bit signed saturating addition lhs +| rhs with bounded interval 50i8 <= sum <= 100i8 establishes 50i8 <= sum <= 100i8 across all nine places so exact subtractions 100i8 - sum and sum - 50i8 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit signed saturating arithmetic bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit signed saturating addition establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | saturating addition result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | saturating addition result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | saturating addition result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | saturating addition result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | saturating addition result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | saturating inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | saturating addition result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | saturating addition result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | saturating addition result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_saturating_arithmetic_bound_through_<place>_exit/`
and `tests/omega/fail/facts/signed_8bit_saturating_arithmetic_bound_through_<place>_rejected/` for the nine places.
Each pass fixture is a byte copy of its place program and carries the four hosted `ProgramEntry` binds and
`expected_native_exit.txt` 0; each reject twin is a byte copy of its `<place>/invalid/main.omg`, which differs
from the positive only in the saturating operand values (`60 + 15` widened to `85 + 20`) and its comment, and
pins `cannot prove` together with the `<= 100` upper bound the widened input breaks.
The three corpus legs recorded at `9b001b8f6af+wt`: `corpus_outcomes.txt` 9 checked + 9 rejected
(`check-target:macos_arm64`), `corpus_native_macos_arm64.txt` 9 built with `exit:0`, and
`corpus_both_macos_arm64.txt` 9 built with `exit:0`, `interpreter-exit:0` and `interpreter:agrees`.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` ([TASKS.md](../../../../TASKS.md)), the item the `signed_8bit_exact_arithmetic_bound` and `signed_8bit_wrapping_arithmetic_bound` siblings name; the earlier `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` names appear in no revision of the board.
- **Requirement established:** 8-bit signed integer saturating addition arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that saturating addition `lhs + rhs` (denoted `lhs +| rhs`) on bounded `i8 in Saturating` operands produces a bounded sum `50i8 <= sum <= 100i8`, which proves that exact 8-bit signed subtractions `100i8 - sum` and `sum - 50i8` are provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit signed saturating addition `lhs + rhs` on `lhs: i8 in Saturating` and `rhs: i8 in Saturating` operates in the saturating arithmetic domain, clamping results to `[-128, 127]` on overflow rather than trapping or wrapping. With bounded operands `lhs = 60` and `rhs = 15`, the evaluated sum `75i8` resides strictly in `[50, 100]` without reaching the saturation threshold. The consuming helper `verify_sum(sum: i8) -> i32 requires sum >= 50 && sum <= 100` discharges exact signed subtractions `100 - sum` (evaluating to 25) and `sum - 50` (evaluating to 25), verifying `headroom + margin == 50` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 8-bit signed scalar preserves its invariant `[50, 100]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The saturating addition bound composes across all 9 standard places to satisfy the consuming requirement `sum >= 50 && sum <= 100` and discharge `100 - sum` and `sum - 50`:
  - `local`: saturating addition result is bound to a local variable `local_sum` before verification.
  - `field`: saturating addition result is stored in `SumCarrier.sum` record field and projected out.
  - `parameter`: saturating addition result is passed to machine parameter `check_param(val: i8)`.
  - `call_result`: helper machine `produce_sum()` computes saturating addition and returns the result across call boundary with ensures contract.
  - `match_join`: saturating addition result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: i8 in Saturating = 60; const RHS: i8 in Saturating = 15;`.
  - `other_state`: saturating addition result is passed across a transition edge into successor state `step(forwarded: i8)`.
  - `other_machine`: saturating addition result is forwarded through intermediate machine `runner()`.
  - `target_body`: saturating addition result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 85, rhs = 20` (where `sum` reaches `105i8 > 100i8`), proving that downstream contract verification `requires sum >= 50 && sum <= 100` strictly fails closed when the post-saturating sum exceeds 100.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 100`, confirming that the proof engine soundly detects that when `sum` can be 105, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `SaturatingIntegerAdd` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `i8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA saturating addition instructions: clamped 8-bit addition on `x86_64` and signed saturating addition `sqadd` on `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open: dynamic runtime saturation trapping certificates, 16-bit to 64-bit multi-precision saturating arithmetic, and SIMD packed signed saturating arithmetic instructions.
