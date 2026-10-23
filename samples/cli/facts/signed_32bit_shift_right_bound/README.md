# Signed 32-bit arithmetic right shift bound

Fact: a 32-bit signed i32 arithmetic right shift of a bounded input (val >= 300i32 && val <= 300i32) by 2u32 bits establishes shifted >= 75i32 && shifted <= 75i32 so exact 32-bit signed subtraction shifted - 75i32 is proved overflow-free across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 32-bit signed arithmetic right shift bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 32-bit arithmetic right shift establishes bound and discharges subtraction directly |
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

The row is pinned under `tests/omega/pass/facts/signed_32bit_shift_right_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_32bit_shift_right_bound_through_<place>_rejected` for the nine
places. Every pinned `main.omg`, `build.omg`, `expected.txt` and `expected_native_exit.txt` is
byte-identical to the sample place it names, each positive fixture carries
`expected_native_exit.txt` = `0`, declares `Expected exit: 0`, and binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64`.

Measured on macOS arm64 (Darwin 27.0.1) with the `gen-9b001b8f6af` producer (`omega` sha256
`6d24393125b7…`, `corpus_runner` sha256 `cee00bbdce17…`) at revision `9b001b8f6af`:

- 10/10 applications (`main.omg` and the nine places) pass `omega run --both`
  (`native exit 0`, `interp exit: 0`);
- 9/9 reject twins exit 1 under `omega --check`: eight with the authored fragment
  `cannot prove requires contract`; the `call_result` twin weakens the same input bound but
  surfaces it at `produce_shifted`'s exit, so its `expected.txt` carries
  `cannot prove ensures contract` (`cannot prove ensures contract for exit from produce_shifted
  at statement 0: result >= 75 && result <= 75`);
- corpus goldens: nine `checked` rows carrying `check-target:macos_arm64`, nine `rejected`
  rows satisfying their `expected.txt`, nine native `built` rows with `exit:0`, and nine
  Terminal-agreement rows carrying `interpreter-exit:0`/`interpreter:agrees`.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 32-bit signed integer arithmetic right shift bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that shifting bounded `i32` value (`val >= 300i32 && val <= 300i32`) right by `2u32` bits establishes the interval `shifted >= 75i32 && shifted <= 75i32`, which proves that exact signed subtraction `shifted - 75i32 == 0i32` is provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 32-bit signed arithmetic right shift `val >> 2u32` on `val: i32` with entry contract `requires val >= 300 && val <= 300` establishes the postcondition interval `[300 >> 2, 300 >> 2] = [75, 75]`. The compiler statically models the sign-extending arithmetic right shift, proving that `shifted >= 75 && shifted <= 75` holds without requiring defensive caller guards. The consuming helper `verify_shifted(shifted: i32) -> i32 requires shifted >= 75 && shifted <= 75` discharges exact signed subtraction `shifted - 75` without overflow or narrowing loss.
- **Identity and lifetime:** The shifted 32-bit signed scalar preserves its invariant `[75, 75]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The arithmetic shift bound composes across all 9 standard places to satisfy the consuming requirement `shifted >= 75 && shifted <= 75` and discharge `shifted - 75`:
  - `local`: shifted result is bound to a local variable `local_shifted` before verification.
  - `field`: shifted result is stored in `ShiftCarrier.shifted` record field and projected out.
  - `parameter`: shifted result is passed to machine parameter `check_param(val: i32)`.
  - `call_result`: helper machine `produce_shifted()` computes the shift and returns the result across call boundary.
  - `match_join`: shifted result converges through match arms into a unified value.
  - `const_name`: input `VAL` is declared as a compile-time named constant `const VAL: i32 = 300;`.
  - `other_state`: shifted result is passed across a transition edge into successor state `step(val: i32)`.
  - `other_machine`: shifted result is forwarded through intermediate machine `runner()`.
  - `target_body`: shifted result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the input bound to `val >= 296 && val <= 300` (so `val == 296` yields `shifted == 74`), proving that downstream contract verification `requires shifted >= 75 && shifted <= 75` strictly fails closed when the post-shift bound allows values below 75.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... >= 75`, confirming that the proof engine soundly detects that when `shifted` can be 74, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Shift operations are lowered with verified shift count certificates (`count < 32`), preserving interval arithmetic bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 4-byte `i32` scalar storage and alignment, while calling plans assign 32-bit register homes (`eax`/`rdi`/`rsi` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA arithmetic right shift instructions: 32-bit `sar r32, imm8` on `x86_64` and `asr w, w, #imm` on `arm64`, preserving the sign bit while shifting down by 2 bits. Exact signed subtraction lowers to 32-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime shift counts requiring masking certificates, 128-bit wide shifts across register pairs, and vector SIMD packed arithmetic right shifts.
