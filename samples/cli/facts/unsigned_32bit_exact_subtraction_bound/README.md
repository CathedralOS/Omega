# Unsigned 32-bit exact subtraction bound

Fact: subtracting two bounded 32-bit unsigned u32 values (lhs >= 80u32 && lhs <= 120u32, rhs >= 20u32 && rhs <= 40u32) establishes the exact post-subtraction interval (diff >= 40u32 && diff <= 100u32), discharging exact 32-bit subtractions 100u32 - diff and diff - 40u32 across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 32-bit unsigned exact subtraction bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 32-bit exact subtraction establishes bound and discharges downstream subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | subtraction result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | subtraction result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | subtraction result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | subtraction result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | subtraction result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | subtraction inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | subtraction result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | subtraction result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | subtraction result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 32-bit unsigned integer exact subtraction bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that subtracting bounded `u32` inputs (`lhs >= 80u32 && lhs <= 120u32`, `rhs >= 20u32 && rhs <= 40u32`) establishes the exact post-subtraction interval `diff >= 40u32 && diff <= 100u32`, which proves that exact 32-bit unsigned subtractions `100u32 - diff` and `diff - 40u32` are provably underflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 32-bit unsigned exact subtraction `lhs - rhs` on `lhs: u32` and `rhs: u32` with entry contracts `requires lhs >= 80 && lhs <= 120, rhs >= 20 && rhs <= 40` statically establishes the postcondition interval `[80 - 40, 120 - 20] = [40, 100]`. Because `lhs >= 80 >= 40 >= rhs`, the domain unconditionally satisfies `lhs >= rhs`, certifying that the subtraction never underflows `u32`. The consuming helper `verify_diff(diff: u32) -> i32 requires diff >= 40 && diff <= 100` discharges exact unsigned subtractions `100 - diff` (evaluating in `[0, 60]`) and `diff - 40` (evaluating in `[0, 60]`), verifying `headroom + margin == 60` without post-hoc caller guards.
- **Identity and lifetime:** The subtracted 32-bit unsigned scalar preserves its invariant `[40, 100]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact subtraction bound composes across all 9 standard places to satisfy the consuming requirement `diff >= 40 && diff <= 100` and discharge `100 - diff` and `diff - 40`:
  - `local`: subtraction result is bound to a local variable `local_diff` before verification.
  - `field`: subtraction result is stored in `DiffCarrier.diff` record field and projected out.
  - `parameter`: subtraction result is passed to machine parameter `check_param(val: u32)`.
  - `call_result`: helper machine `produce_diff()` computes subtraction and returns the result across call boundary with ensures contract.
  - `match_join`: subtraction result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: u32 = 100; const RHS: u32 = 30;`.
  - `other_state`: subtraction result is passed across a transition edge into successor state `step(forwarded: u32)`.
  - `other_machine`: subtraction result is forwarded through intermediate machine `runner()`.
  - `target_body`: subtraction result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the `rhs` upper bound to `rhs >= 20 && rhs <= 41` (where `diff` reaches `80 - 41 = 39u32 < 40u32`), proving that downstream contract verification `requires diff >= 40 && diff <= 100` strictly fails closed when the post-subtraction bound drops below 40.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... >= 40`, confirming that the proof engine soundly detects that when `diff` can be 39, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Unsigned exact subtractions are lowered with verified underflow certificates (`lhs >= rhs`), preserving interval bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 4-byte `u32` scalar storage and alignment, while calling plans assign 32-bit register homes (`eax`/`edi`/`esi` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA subtraction instructions: 32-bit `sub r32, r32` on `x86_64` and `sub w, w, w` on `arm64`. Register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime underflow trapping certificates for unbounded subtraction, 64-bit and 128-bit wide unsigned subtraction borrow propagation, and SIMD saturating unsigned subtraction kernels.
