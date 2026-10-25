# Signed 64-bit wrapping subtraction bound

Fact: 64-bit signed wrapping subtraction lhs -% rhs with bounded interval 40i64 <= diff <= 100i64 establishes 40i64 <= diff <= 100i64 across all nine places so exact subtractions 100i64 - diff and diff - 40i64 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Pinned fixtures

The nine positive places and nine reject twins are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_subtraction_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt` = `cannot prove requires contract`):**
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_subtraction_bound_through_target_body_rejected/`

`tests/omega/corpus_native_windows_x86_64.txt` records nine real `built ... exit:0` rows, one per place, and `tests/omega/corpus_outcomes.txt` records all 18 pass and fail outcomes.

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
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | wrapping subtraction result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | wrapping subtraction result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | wrapping subtraction result verified across target-scoped machine bodies |

Run each place with `omega run --both <place>/main.omg`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating 64-bit signed wrapping subtraction `lhs - rhs` with `lhs = 100` and `rhs = 30` produces `diff = 70i64` within static interval `[40i64, 100i64]`. Passing `diff` to `verify_diff(diff)` satisfies its precondition `requires diff >= 40 && diff <= 100`, evaluates exact signed subtractions `headroom = 100 - 70 == 30i64` and `margin = 70 - 40 == 30i64`, and returns `(headroom + margin - 60) as i32 == 0i32`. In `main`, `transition check == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition check == 0` to `transition check == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the `lhs` and `rhs` inputs to `lhs = 125` and `rhs = 20`, so the wrapping difference `diff` reaches `105i64 > 100i64`. Calling `verify_diff(diff)` (or satisfying the parameter/call/state contract) fails closed at compile time under the **Contract precondition** obligation class with `cannot prove requires contract`. None reaches native execution.

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |

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
  - `other_state`: wrapping subtraction result is passed across a transition edge into successor machine `step(forwarded: i64)`.
  - `other_machine`: wrapping subtraction result is forwarded through intermediate machine `runner()`.
  - `target_body`: wrapping subtraction result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 125, rhs = 20` (where `diff` reaches `105i64 > 100i64`), proving that downstream contract verification `requires diff >= 40 && diff <= 100` strictly fails closed when the post-wrapping difference exceeds 100.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: cannot prove ... contract ...: ... <= 100`, confirming that the proof engine soundly detects that when `diff` can be 105, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `WrappingIntegerSubtract` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 8-byte `i64` scalar storage and alignment, while calling plans assign 64-bit register homes (`rax`/`rdi`/`rsi` on x86_64, `x0`/`x1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA wrapping subtraction instructions: 64-bit `sub r64, r64` on `x86_64` and `sub x, x, x` on `arm64` without overflow trapping certificates. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: multi-word 128-bit modular wrapping subtraction borrow propagation, SIMD packed 64-bit signed wrapping subtraction kernels, and modular ring homomorphisms across arithmetic boundaries.
