# Signed 64-bit arithmetic right shift bound

Fact: a 64-bit signed i64 arithmetic right shift of a bounded input (val >= 300i64 && val <= 300i64) by 2u64 bits establishes shifted >= 75i64 && shifted <= 75i64 so exact 64-bit signed subtraction shifted - 75i64 is proved overflow-free across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Pinned fixtures

The nine positive places and nine reject twins are pinned as
`tests/omega/pass/facts/signed_64bit_shift_right_arithmetic_bound_through_<place>`
and `tests/omega/fail/facts/signed_64bit_shift_right_arithmetic_bound_unproved_at_<place>`.
Each twin authors `expected.txt` carrying the contract rejection fragment (`cannot prove requires contract` or `cannot prove ensures contract`).
`tests/omega/corpus_native_windows_x86_64.txt` records nine real `built ... exit:0` rows, one per place.

## Places

The nine places carry the 64-bit signed arithmetic right shift bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 64-bit arithmetic right shift establishes bound and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | shifted result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | shifted result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | shifted result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | shifted result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | shifted result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | shifted input provided by named compile-time constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | shifted result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | shifted result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | shifted result verified across target-scoped machine bodies |

Run each place with `omega run --both <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 64-bit signed integer arithmetic right shift bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that shifting bounded `i64` value (`val >= 300i64 && val <= 300i64`) right by `2u64` bits establishes the interval `shifted >= 75i64 && shifted <= 75i64`, which proves that exact signed subtraction `shifted - 75i64 == 0i64` is provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 64-bit signed arithmetic right shift `val >> 2u64` on `val: i64` with entry contract `requires val >= 300 && val <= 300` establishes the postcondition interval `[300 >> 2, 300 >> 2] = [75, 75]`. The compiler statically models the sign-extending arithmetic right shift, proving that `shifted >= 75 && shifted <= 75` holds without requiring defensive caller guards. The consuming helper `verify_shifted(shifted: i64) -> i64 requires shifted >= 75 && shifted <= 75` discharges exact signed subtraction `shifted - 75` without overflow or narrowing loss.
- **Identity and lifetime:** The shifted 64-bit signed scalar preserves its invariant `[75, 75]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The arithmetic shift bound composes across all 9 standard places to satisfy the consuming requirement `shifted >= 75 && shifted <= 75` and discharge `shifted - 75`:
  - `local`: shifted result is bound to a local variable `local_shifted` before verification.
  - `field`: shifted result is stored in `ShiftCarrier.shifted` record field and projected out.
  - `parameter`: shifted result is passed to machine parameter `check_param(val: i64)`.
  - `call_result`: helper machine `produce_shifted()` computes the shift and returns the result across call boundary.
  - `match_join`: shifted result converges through match arms into a unified value.
  - `const_name`: input `VAL` is declared as a compile-time named constant `const VAL: i64 = 300;`.
  - `other_state`: shifted result is passed across a transition edge into successor machine `step(forwarded: i64)`.
  - `other_machine`: shifted result is forwarded through intermediate machine `runner()`.
  - `target_body`: shifted result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the input bound to `val >= 296 && val <= 300` (so `val == 296` yields `shifted == 74`), proving that downstream contract verification `requires shifted >= 75 && shifted <= 75` (or `ensures result >= 75 && result <= 75`) strictly fails closed when the post-shift bound allows values below 75.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... >= 75`, confirming that the proof engine soundly detects that when `shifted` can be 74, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Shift operations are lowered with verified shift count certificates (`count < 64`), preserving interval arithmetic bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 8-byte `i64` scalar storage and alignment, while calling plans assign 64-bit register homes (`rax`/`rcx`/`rdi`/`rsi` on x86_64, `x0`/`x1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA arithmetic right shift instructions: 64-bit `sar r64, imm8` on `x86_64` and `asr x, x, #imm` on `arm64`, preserving the sign bit while shifting down by 2 bits. Exact signed subtraction lowers to 64-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime shift counts requiring masking certificates, 128-bit wide shifts across register pairs, and vector SIMD packed arithmetic right shifts.
