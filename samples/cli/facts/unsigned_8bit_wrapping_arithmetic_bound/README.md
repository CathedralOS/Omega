# Unsigned 8-bit wrapping arithmetic bound

Fact: 8-bit unsigned wrapping addition lhs +% rhs with bounded interval 50u8 <= sum <= 100u8 establishes 50u8 <= sum <= 100u8 across all nine places so exact subtractions 100u8 - sum and sum - 50u8 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit unsigned wrapping arithmetic bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit unsigned wrapping addition establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | wrapping addition result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | wrapping addition result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | wrapping addition result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | wrapping addition result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | wrapping addition result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | wrapping inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | wrapping addition result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | wrapping addition result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | wrapping addition result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 8-bit unsigned integer wrapping addition arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that wrapping addition `lhs + rhs` (denoted `lhs +% rhs`) on bounded `u8 in Wrapping` operands produces a bounded sum `50u8 <= sum <= 100u8`, which proves that exact 8-bit unsigned subtractions `100u8 - sum` and `sum - 50u8` are provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit unsigned wrapping addition `lhs + rhs` on `lhs: u8 in Wrapping` and `rhs: u8 in Wrapping` allows modular arithmetic without trap obligations or overflow assertions. With bounded operands `lhs = 60` and `rhs = 15`, the evaluated sum `75u8` resides strictly in `[50, 100]`. The consuming helper `verify_sum(sum: u8) -> i32 requires sum >= 50 && sum <= 100` discharges exact unsigned subtractions `100 - sum` (evaluating to 25) and `sum - 50` (evaluating to 25), verifying `headroom + margin == 50` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 8-bit unsigned scalar preserves its invariant `[50, 100]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The wrapping addition bound composes across all 9 standard places to satisfy the consuming requirement `sum >= 50 && sum <= 100` and discharge `100 - sum` and `sum - 50`:
  - `local`: wrapping addition result is bound to a local variable `local_sum` before verification.
  - `field`: wrapping addition result is stored in `SumCarrier.sum` record field and projected out.
  - `parameter`: wrapping addition result is passed to machine parameter `check_param(val: u8)`.
  - `call_result`: helper machine `produce_sum()` computes wrapping addition and returns the result across call boundary with ensures contract.
  - `match_join`: wrapping addition result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: u8 in Wrapping = 60; const RHS: u8 in Wrapping = 15;`.
  - `other_state`: wrapping addition result is passed across a transition edge into successor state `step(forwarded: u8)`.
  - `other_machine`: wrapping addition result is forwarded through intermediate machine `runner()`.
  - `target_body`: wrapping addition result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 85, rhs = 20` (where `sum` reaches `105u8 > 100u8`), proving that downstream contract verification `requires sum >= 50 && sum <= 100` strictly fails closed when the post-wrapping sum exceeds 100.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 100`, confirming that the proof engine soundly detects that when `sum` can be 105, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `WrappingIntegerAdd` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `u8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA wrapping addition instructions: 8-bit `add r8, r8` on `x86_64` and `add w, w, w` on `arm64` without overflow trapping certificates. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime modular arithmetic overflow trapping certificates, 16-bit to 64-bit multi-precision modular residue arithmetic, and SIMD packed modular arithmetic intrinsics.
