# Signed 8-bit exact subtraction bound

Fact: 8-bit signed exact subtraction lhs - rhs with 60i8 <= lhs <= 100i8 and 10i8 <= rhs <= 20i8 establishes 40i8 <= diff <= 90i8 so 90i8 - diff and diff - 40i8 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit signed exact subtraction bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit signed exact subtraction establishes bound and discharges downstream subtractions directly |
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

## Execution

Each positive place and the root `main.omg` is an application that exits 0 in both Terminal Psi and
native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_exact_subtraction_bound/target_body/main.omg
```

### Acceptance Gate

```sh
omega --check samples/cli/facts/signed_8bit_exact_subtraction_bound/local/invalid/main.omg
python tools/sample_native_gate.py --cli <omega> --filter cli/facts/signed_8bit_exact_subtraction_bound --jobs 4
```

`sample_native_gate.py` runs the 10 samples plus the 18 mirrored invalid controls as one batch.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_8bit_exact_subtraction_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture's `main.omg` is byte-identical to its sample place and
carries `expected_native_exit.txt` = 0; each fail fixture's `main.omg`/`expected.txt` are byte-identical
to its place twin; every fixture `build.omg` binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and differs from its sample `build.omg`
only in a fixture-unique `Build::application` name. Positive versus twin `git diff` widens only the `rhs`
bound (`rhs <= 20` to `rhs <= 25`) and evaluates at the widened extreme (inputs 80,15 to 60,25), which is
the fact's own domain.

Measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af` (revision
`9b001b8f6afa2cdac67fd580033c3560d906a5d5`, omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse; eight report `cannot prove requires contract` naming
  their consumer (`verify_diff` in five places, `check_param`, `runner`, `exercise`), and the
  `call_result` twin reports `cannot prove ensures contract for exit from produce_diff at statement 0:
  result >= 40 && result <= 90`, the callee's own postcondition that the widened bound can no longer
  establish.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `9b001b8f6afa`, so every row is stamped `@9b001b8f6afa+wt`: `tests/omega/corpus_outcomes.txt` gained
  60 lines (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with
  24 diagnostic continuation lines), `tests/omega/corpus_native_macos_arm64.txt` gained 18
  `built ... exit:0` rows and `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0
  interpreter-exit:0 interpreter:agrees` rows. Every golden diff is an addition (0 deletions); all three
  legs exited 0 with 0 timeout rows and 0 fail rows that lost their expected fragment.

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 8-bit signed integer exact subtraction bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that subtracting bounded `i8` inputs (`lhs >= 60i8 && lhs <= 100i8`, `rhs >= 10i8 && rhs <= 20i8`) establishes the exact post-subtraction interval `diff >= 40i8 && diff <= 90i8`, which proves that exact 8-bit signed subtractions `90i8 - diff` and `diff - 40i8` are provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit signed exact subtraction `lhs - rhs` on `lhs: i8` and `rhs: i8` with entry contracts `requires lhs >= 60 && lhs <= 100, rhs >= 10 && rhs <= 20` statically establishes the postcondition interval `[60 - 20, 100 - 10] = [40, 90]`. Because `[40, 90]` resides fully within 8-bit signed integer range `[-128, 127]`, subtraction is certified overflow-free without runtime checks or wrapping modes. The consuming helper `verify_diff(diff: i8) -> i32 requires diff >= 40 && diff <= 90` discharges exact signed subtractions `90 - diff` (evaluating in `[0, 50]`) and `diff - 40` (evaluating in `[0, 50]`), verifying `headroom + margin == 50` without post-hoc caller guards.
- **Identity and lifetime:** The subtracted 8-bit signed scalar preserves its invariant `[40, 90]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact subtraction bound composes across all 9 standard places to satisfy the consuming requirement `diff >= 40 && diff <= 90` and discharge `90 - diff` and `diff - 40`:
  - `local`: subtraction result is bound to a local variable `local_diff` before verification.
  - `field`: subtraction result is stored in `DiffCarrier.diff` record field and projected out.
  - `parameter`: subtraction result is passed to machine parameter `check_param(val: i8)`.
  - `call_result`: helper machine `produce_diff()` computes subtraction and returns the result across call boundary with ensures contract.
  - `match_join`: subtraction result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: i8 = 80; const RHS: i8 = 15;`.
  - `other_state`: subtraction result is passed across a transition edge into successor state `step(forwarded: i8)`.
  - `other_machine`: subtraction result is forwarded through intermediate machine `runner()`.
  - `target_body`: subtraction result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the `rhs` upper bound to `rhs <= 25` and evaluating the subtraction at the widened extreme (inputs `80,15` to `60,25`, where `diff` reaches `60 - 25 = 35i8 < 40i8`), proving that downstream contract verification `requires diff >= 40 && diff <= 90` strictly fails closed when the post-subtraction bound drops below 40.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`). Eight refuse with `cannot prove requires contract` naming their consumer (`verify_diff` in five places, `check_param`, `runner`, `exercise`); the `call_result` twin refuses with `cannot prove ensures contract for exit from produce_diff at statement 0: result >= 40 && result <= 90`. The proof engine soundly detects that when `diff` can be 35, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. 8-bit signed exact subtractions are lowered with verified overflow certificates (`[-128, 127]`), preserving interval bounds and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `i8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA subtraction instructions: 8-bit `sub r8, r8` on `x86_64` and sign-extended 8-bit subtraction `sub w, w, w` on `arm64`. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime overflow trapping certificates for unbounded 8-bit subtraction, signed 8-bit saturating arithmetic intrinsics, and SIMD packed signed byte subtraction kernels.
