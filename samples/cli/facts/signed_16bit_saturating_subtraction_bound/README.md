# Signed 16-Bit Saturating Subtraction Bound

16-bit signed saturating subtraction `lhs -| rhs` with bounded interval `200i16 <= diff <= 450i16` establishes `200i16 <= diff <= 450i16` across all nine places so exact subtractions `450i16 - diff` and `diff - 200i16` are safe.

Every place performs the same operation: define bounded signed 16-bit inputs `lhs` (`300..500`) and `rhs` (`50..100`), evaluate 16-bit saturating subtraction `(lhs as i16 in Saturating) - (rhs as i16 in Saturating)` producing `sat as i16`, transport the difference value through the place, pass `diff` to `verify_diff(value: i16)` which requires `value >= 200 && value <= 450`, execute exact subtractions `head: i16 = 450 - value` and `tail: i16 = value - 200`, sum them to obtain `250i16`, and verify in `main` that `res == 250` transitioning to `done()` exiting 0. Only the transport mechanism of the difference value changes across places.

Owning task: **ARITHMETIC-POLICY-REALIZATION** (`TASKS.md`), numeric-policy realization and full program acceptance. Exact requirement this row establishes: 16-bit signed saturating subtraction `lhs -| rhs` on bounded operands (`lhs >= 300 && lhs <= 500`, `rhs >= 50 && rhs <= 100`) statically establishes the difference interval `[300 - 100, 500 - 50] = [200, 450]` on the `i16` result, carries that fact through all nine transport places, and proves the consuming exact subtractions `450i16 - diff` and `diff - 200i16` from the carried fact itself — with Terminal Psi and native execution agreeing (`omega run --both`, exit 0) — and every place's reject twin failing closed at compile time.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit signed saturating subtraction evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded signed i16 operands and their saturating difference are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the saturating difference result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the saturating difference result is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the saturating difference result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the saturating difference result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs is subtracted via saturating subtraction from the bounded lhs |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the saturating difference result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the saturating difference result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 16-bit signed saturating subtraction and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, declares
`Expected exit: 0`, and carries `expected_native_exit.txt` `0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating saturating subtraction with inputs `400i16` and `80i16` produces `diff == 320i16` within static interval `200i16..450i16`. Passing `diff` to `verify_diff(diff)` satisfies its contract `value >= 200 && value <= 450`. `verify_diff` evaluates exact subtractions `head = 450 - 320 == 130i16` and `tail = 320 - 200 == 120i16`, sums them to `250i16`, and returns `250i16`. In `main`, `transition res == 250` transitions to `machine done() {}`, terminating with native exit 0. If saturating subtraction or contract satisfaction returns an unexpected value, execution branches to `machine bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `lhs` bound (e.g. `lhs <= 520i16` or `const RHS: i16 = 80; lhs <= 540`), so `diff` reaches `470i16 > 450i16` (or `460i16 > 450i16`). Calling `verify_diff(diff)` fails closed at compile time with `cannot prove requires contract for call verify_diff`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_16bit_saturating_subtraction_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Measured evidence

Measured on macOS arm64 with the factsr1 shared generation producer
`gen-4c23cc3391c` (revision `4c23cc3391c2e539d4065563ac95de7b18f2fc67`, omega
sha256 `83c7ae2a45442b1ceb6f5476a6f8ed4c4af9eabbd85e682a3ed7254c7fec94cf`,
corpus_runner sha256 `a1de848f1baca0837d28285d9da6da9acdd7639081c083743f76862a409bdb3d`):
the ten `omega run --both` invocations above report `native exit 0` and
`interp exit: 0` (20/20 engine observations); each of the nine `omega --check`
twin invocations exits 1 with `cannot prove requires contract for call
verify_diff` (nine twins and their nine byte-identical `invalid/<place>/`
mirrors). The same facts are pinned under the corpus (below) and were measured
fresh there through the check, native and agreement legs. The earlier
Windows-host `sample_native_gate.py` run (10/10 oracles, 18/18 controls) is
retained evidence from the row's author, not re-measured here. No compiler
repair was needed and none landed.

## Pinned Fixtures

The row is pinned under
`tests/omega/pass/facts/signed_16bit_saturating_subtraction_bound_through_<place>_exit`
(9 fixtures: the place's `main.omg`, `build.omg` bound to all four hosted
targets for `ProgramEntry`, and `expected_native_exit.txt` `0`) and
`tests/omega/fail/facts/signed_16bit_saturating_subtraction_bound_through_<place>_rejected`
(9 fixtures: the twin's `main.omg`, `build.omg` and `expected.txt` fragment).
Every pinned file is byte-identical to the sample file it names (36/36 pairs
verified with `cmp`).

## Requirements and Evidence

- **Parent outcome:** `ARITHMETIC-POLICY-REALIZATION` (`TASKS.md`), numeric-policy realization and full program acceptance.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 16-bit signed saturating subtraction (`i16 in Saturating`, `[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, 16-bit signed saturating subtraction `lhs -| rhs` with bounded interval `200i16 <= diff <= 450i16` establishes `200i16 <= diff <= 450i16` across all nine places so exact subtractions `450i16 - diff` and `diff - 200i16` are safe without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 300 && lhs <= 500 && rhs >= 50 && rhs <= 100`) and 16-bit signed saturating subtraction (`(lhs as i16 in Saturating) - (rhs as i16 in Saturating)`) establish the difference interval `[300 - 100, 500 - 50] = [200, 450]`. Because `[200, 450]` does not overflow or underflow `i16` bounds `[-32768, 32767]`, recasting to `i16` preserves the interval fact, discharging downstream precondition `value >= 200 && value <= 450` and exact subtractions `450 - value` and `value - 200` without requiring post-hoc caller equality guards (`transition x == 320`).
  - **Identity and lifetime:** The synthesized 16-bit saturating difference and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 16-bit signed difference interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_diff(diff)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`lhs <= 500` vs `lhs <= 520` or `RHS = 80; lhs <= 500` vs `lhs <= 540`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `lhs <= 520` (or `lhs <= 540`) is specified: the difference interval expands to `[200, 470]` (or `[220, 460]`), which exceeds the required domain `value >= 200 && value <= 450`, strictly rejecting with `cannot prove requires contract for call verify_diff`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 16-bit saturating subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving saturating subtraction semantics and exact arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 16-bit signed integer layout plan (`IntegerAt`: `i16` size 2, alignment 2) validates, and target `CallPlan`/`StatePlan` assign 16-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 16-bit subtraction instructions with overflow/underflow clamping (e.g. `sub ax, dx` with conditional saturation bounds clamping on `x86_64`; `subs w, w, w` with saturation select/clamping on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `66 2B /r` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening signed saturating subtraction conversions (`i16 in Saturating -> i32`), vector SIMD 16-bit packed signed saturating subtractions (`ssub` / `sqsub`), and wrapping signed arithmetic conversions.
