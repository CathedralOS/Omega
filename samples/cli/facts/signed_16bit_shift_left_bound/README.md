# Signed 16-bit left shift bound

Fact: a 16-bit signed i16 left shift of a bounded input (val >= 30i16 && val <= 30i16) by 2u32 bits establishes shifted >= 120i16 && shifted <= 120i16 so exact 16-bit signed subtraction shifted - 120i16 is proved overflow-free across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 16-bit signed left shift bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 16-bit left shift establishes bound and discharges subtraction directly |
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

## Pinned Fixtures

The row is pinned by eighteen fixtures: nine positives under
`tests/omega/pass/facts/signed_16bit_shift_left_bound_through_*_exit` and nine
reject twins under
`tests/omega/fail/facts/signed_16bit_shift_left_bound_through_*_rejected`. Every
positive is a byte-for-byte copy of its place program and binds `ProgramEntry`
for all four hosted targets; every twin is a byte-for-byte copy of its place's
`invalid/main.omg` and pins the compiler's complete rejection diagnostic in
`expected.txt`. Recorded on three legs: `corpus_outcomes.txt` (nine `checked`
positives carrying `check-target:macos_arm64` and nine `rejected ... expected`
twins), `corpus_native_macos_arm64.txt` (nine `built ... exit:0`) and
`corpus_both_macos_arm64.txt` (nine `built ... exit:0 interpreter-exit:0
interpreter:agrees`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 16-bit signed integer exact left shift bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that shifting bounded `i16` value (`val >= 30i16 && val <= 30i16`) left by `2u32` bits establishes the interval `shifted >= 120i16 && shifted <= 120i16`, which proves that exact signed subtraction `shifted - 120i16 == 0i16` is provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 16-bit signed exact left shift `val << 2u32` on `val: i16` with entry contract `requires val >= 30 && val <= 30` establishes the postcondition interval `[30 << 2, 30 << 2] = [120, 120]`. The compiler statically models the left shift, proving that `shifted >= 120 && shifted <= 120` holds within 16-bit signed range without overflow or sign inversion. The consuming helper `verify_shifted(shifted: i16) -> i32 requires shifted >= 120 && shifted <= 120` discharges exact signed subtraction `shifted - 120` without overflow or narrowing loss.
- **Identity and lifetime:** The shifted 16-bit signed scalar preserves its invariant `[120, 120]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact left shift bound composes across all 9 standard places to satisfy the consuming requirement `shifted >= 120 && shifted <= 120` and discharge `shifted - 120`:
  - `local`: shifted result is bound to a local variable `local_shifted` before verification.
  - `field`: shifted result is stored in `ShiftCarrier.shifted` record field and projected out.
  - `parameter`: shifted result is passed to machine parameter `check_param(val: i16)`.
  - `call_result`: helper machine `produce_shifted()` computes the shift and returns the result across call boundary.
  - `match_join`: shifted result converges through match arms into a unified value.
  - `const_name`: input `VAL` is declared as a compile-time named constant `const VAL: i16 = 30;`.
  - `other_state`: shifted result is passed across a transition edge into successor state `step(val: i16)`.
  - `other_machine`: shifted result is forwarded through intermediate machine `runner()`.
  - `target_body`: shifted result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the input bound to `val >= 29 && val <= 30` (so `val == 29` yields `shifted == 116`), proving that downstream contract verification `requires shifted >= 120 && shifted <= 120` strictly fails closed when the post-shift bound allows values below 120.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... >= 120`, confirming that the proof engine soundly detects that when `shifted` can be 116, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Shift operations are lowered with verified shift count certificates (`count < 16`), preserving interval arithmetic bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2-byte `i16` scalar storage and alignment, while calling plans assign 16-bit register homes (`ax`/`di`/`si` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA left shift instructions: 16-bit `shl r16, imm8` on `x86_64` and 16-bit masked `lsl w, w, #imm` on `arm64`. Exact signed subtraction lowers to 16-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime shift counts requiring masking certificates, 128-bit wide shifts across register pairs, and vector SIMD packed left shifts.
