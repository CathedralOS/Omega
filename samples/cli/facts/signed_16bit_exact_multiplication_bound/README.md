# Signed 16-bit exact multiplication bound

Fact: a 16-bit signed i16 exact multiplication of bounded inputs (lhs >= 15i16 && lhs <= 25i16, rhs >= 10i16 && rhs <= 20i16) establishes prod >= 150i16 && prod <= 500i16 so exact 16-bit signed subtraction 500i16 - prod is proved overflow-free across all nine places.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 16-bit signed exact multiplication bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 16-bit signed exact multiplication establishes bound and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | multiplication result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | multiplication result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | multiplication result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | multiplication result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | multiplication result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | multiplication inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | multiplication result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | multiplication result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | multiplication result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_16bit_exact_multiplication_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_16bit_exact_multiplication_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy
of its place twin's `main.omg`/`build.omg`/`expected.txt` (eight places pin `cannot prove requires
contract`; `call_result` pins `cannot prove ensures contract for exit from produce_prod`). Positive
versus twin `git diff` changes only the `rhs` bound (`rhs <= 20` to `rhs <= 21`), the concrete inputs
admitted under it (`lhs = 25i16`, `rhs = 21i16`), the named constants in `const_name` (`20`/`15` to
`25`/`21`), and the leading invalid-control comment - all of them the fact's own input or domain. The
concrete inputs matter: this row's places multiply literal-bound locals, whose exact values the checker
folds, so the bound alone leaves `prod = 300i16` in contract; the twin's inputs place `prod` at
`525i16`, which is what `500i16 - prod` cannot survive.

Re-measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af` (omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`) after psi integer-range literal
handling, the scalar-predicate projection and the where-fact-interval proof paths moved past the
previous generation (gen-4c23cc3391c); the same 20 positive places and 18 twins were re-run against it.

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse - eight at the consuming requires contract
  (`cannot prove requires contract for call verify_prod/check_param/runner/exercise from ...`) and
  `call_result` at `cannot prove ensures contract for exit from produce_prod`.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `9b001b8f6afa`, so every row is stamped `@9b001b8f6afa+wt`: `tests/omega/corpus_outcomes.txt` gained
  60 lines (36 rows: 18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with 24
  diagnostic continuation lines), `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ...
  exit:0` rows and `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0
  interpreter-exit:0 interpreter:agrees` rows. Every golden diff is a pure addition (0 deletions); all
  three legs exited 0 with 0 unimplemented rows and 0 rows recording a lost success.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` ([TASKS.md](../../../../TASKS.md)): "machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls". This row is one customer group's nine-place transport and execution of that settled proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization.
- **Requirement established:** 16-bit signed integer exact multiplication bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that multiplying bounded `i16` values (`lhs >= 15i16 && lhs <= 25i16`, `rhs >= 10i16 && rhs <= 20i16`) establishes the bounded interval `prod >= 150i16 && prod <= 500i16`, which proves that exact signed subtraction `let headroom: i16 = 500i16 - prod` is provably overflow-free and within 16-bit signed bounds across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 16-bit signed exact multiplication `lhs * rhs` on `lhs: i16` and `rhs: i16` with entry contracts `requires lhs >= 15 && lhs <= 25, rhs >= 10 && rhs <= 20` statically establishes the postcondition interval `[15 * 10, 25 * 20] = [150, 500]`. The compiler statically checks the multiplication within 16-bit signed range `[-32768, 32767]`, proving that `prod >= 150 && prod <= 500` holds without arithmetic overflow or sign alteration. The consuming helper `verify_prod(prod: i16) -> i32 requires prod >= 150 && prod <= 500` discharges exact signed subtraction `500 - prod` (evaluating in `[0, 350]`) without overflow or narrowing loss.
- **Identity and lifetime:** The multiplied 16-bit signed scalar preserves its invariant `[150, 500]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The exact multiplication bound composes across all 9 standard places to satisfy the consuming requirement `prod >= 150 && prod <= 500` and discharge `500 - prod`:
  - `local`: multiplication result is bound to a local variable `local_prod` before verification.
  - `field`: multiplication result is stored in `ProdCarrier.prod` record field and projected out.
  - `parameter`: multiplication result is passed to machine parameter `check_param(val: i16)`.
  - `call_result`: helper machine `produce_prod()` computes multiplication and returns the result across call boundary with ensures contract.
  - `match_join`: multiplication result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: i16 = 20; const RHS: i16 = 15;`.
  - `other_state`: multiplication result is passed across a transition edge into successor state `step(forwarded: i16)`.
  - `other_machine`: multiplication result is forwarded through intermediate machine `runner()`.
  - `target_body`: multiplication result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening the `rhs` bound to `rhs >= 10 && rhs <= 21` and in the concrete inputs admitted under it (`lhs = 25i16`, `rhs = 21i16`, so `prod` is `525i16 > 500i16`), proving that downstream contract verification `requires prod >= 150 && prod <= 500` strictly fails closed when the post-multiplication bound exceeds 500.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`), confirming that the proof engine soundly detects that when `prod` can reach 525 the contract cannot be established. Eight places reject at the consuming requires contract (`error: cannot prove requires contract for call verify_prod/check_param/runner/exercise from ...: <carried value> >= 150 && <carried value> <= 500`); `call_result` rejects at the producer's own postcondition (`error: cannot prove ensures contract for exit from produce_prod at statement 0: result >= 150 && result <= 500`), and its fixture oracle pins that exact fragment.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Integer multiplications are lowered with verified exact integer arithmetic certificates (`[-32768, 32767]`), proving absence of overflow and erasing proof tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 2-byte `i16` scalar storage and alignment, while calling plans assign 16-bit register homes (`ax`/`di`/`si` on x86_64, `w0`/`w1` on arm64) for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA multiplication instructions: 16-bit signed `imul r16, r16` on `x86_64` and sign-extended 16-bit multiplication `mul w, w, w` on `arm64`. Exact signed subtraction lowers to 16-bit `sub`, and register allocation assigns register homes and disjoint stack activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open: dynamic runtime multiplication overflow trapping certificates, 64-bit to 128-bit widening signed multiplication, and SIMD packed signed multiplication kernels.
