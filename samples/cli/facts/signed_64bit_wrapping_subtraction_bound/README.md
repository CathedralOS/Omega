# Signed 64-bit wrapping subtraction bound

Fact: 64-bit signed wrapping subtraction lhs -% rhs with bounded interval 40i64 <= diff <= 100i64 establishes 40i64 <= diff <= 100i64 across all nine places so exact subtractions 100i64 - diff and diff - 40i64 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 64-bit signed wrapping subtraction bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 64-bit signed wrapping subtraction establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | wrapping subtraction result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | wrapping subtraction result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | wrapping subtraction result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | wrapping subtraction result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | wrapping subtraction result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | wrapping inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | wrapping subtraction result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | wrapping subtraction result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | wrapping subtraction result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 64-bit signed integer wrapping subtraction arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that wrapping subtraction `lhs - rhs` (denoted `lhs -% rhs`) on bounded `i64 in Wrapping` operands produces a bounded difference `40i64 <= diff <= 100i64`, which proves that exact 64-bit signed subtractions `100i64 - diff` and `diff - 40i64` are provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 64-bit signed wrapping subtraction `lhs - rhs` on `lhs: i64 in Wrapping` and `rhs: i64 in Wrapping` operates in the wrapping arithmetic domain, allowing residue arithmetic without trap obligations or overflow assertions. With bounded operands `lhs = 100` and `rhs = 30`, the evaluated difference `70i64` resides strictly in `[40, 100]`. The consuming helper `verify_diff(diff: i64) -> i32 requires diff >= 40 && diff <= 100` discharges exact signed subtractions `100 - diff` (evaluating to 30) and `diff - 40` (evaluating to 30), verifying `headroom + margin == 60` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 64-bit signed scalar preserves its invariant `[40, 100]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The wrapping subtraction bound composes across all 9 standard places to satisfy the consuming requirement `diff >= 40 && diff <= 100` and discharge `100 - diff` and `diff - 40`:
  - `local`: wrapping subtraction result is bound to a local variable `local_diff` before verification.
  - `field`: wrapping subtraction result is stored in `DiffCarrier.diff` record field and projected out.
  - `parameter`: wrapping subtraction result is passed to machine parameter `check_param(val: i64)`.
  - `call_result`: helper machine `produce_diff()` computes wrapping subtraction and returns the result across call boundary with ensures contract.
  - `match_join`: wrapping subtraction result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: i64 in Wrapping = 100; const RHS: i64 in Wrapping = 30;`.
  - `other_state`: wrapping subtraction result is passed across a transition edge into successor state `step(forwarded: i64)`.
  - `other_machine`: wrapping subtraction result is forwarded through intermediate machine `runner()`.
  - `target_body`: wrapping subtraction result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 125, rhs = 20` (where `diff` reaches `105i64 > 100i64`), proving that downstream contract verification `requires diff >= 40 && diff <= 100` strictly fails closed when the post-wrapping difference exceeds 100.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 100`, confirming that the proof engine soundly detects that when `diff` can be 105, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `WrappingIntegerSubtract` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 8-byte `i64` scalar storage and alignment, while calling plans assign 64-bit register homes (`rax`/`rdi`/`rsi` on x86_64, `x0`/`x1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA wrapping subtraction instructions: 64-bit `sub r64, r64` on `x86_64` and `sub x, x, x` on `arm64` without overflow trapping certificates. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: multi-word 128-bit modular wrapping subtraction borrow propagation, SIMD packed 64-bit signed wrapping subtraction kernels, and modular ring homomorphisms across arithmetic boundaries.
