# signed_16bit_saturating_multiplication_bound

> Fact: 16-bit signed saturating multiplication `lhs *| rhs` with bounded interval
> 50i16 <= prod <= 200i16 establishes 50i16 <= prod <= 200i16 across all nine
> places so exact subtractions 200i16 - prod and prod - 50i16 are safe.

Owning task: **ARITHMETIC-POLICY-REALIZATION** (`TASKS.md`), numeric-policy
realization and full program acceptance. Exact requirement this row establishes:
16-bit signed saturating multiplication on bounded operands
(`lhs >= 10i16 && lhs <= 20i16`, `rhs >= 5i16 && rhs <= 10i16`) statically
establishes the product interval `[10 * 5, 20 * 10] = [50, 200]` on the `i16`
result, carries that fact through all nine transport places, and proves the
consuming exact subtractions `200i16 - prod` and `prod - 50i16` from the carried
fact itself — with Terminal Psi and native execution agreeing (`omega run
--both`, exit 0) — and every place's reject twin failing closed at compile time.

## Places

| Place | Positive (`main.omg`) | Reject twin (`invalid/main.omg`) | Pinned corpus pass | Pinned corpus fail |
|---|---|---|---|---|
| root | [`main.omg`](main.omg) | — | — | — |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_local_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_local_rejected` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_field_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_field_rejected` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_parameter_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_parameter_rejected` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_call_result_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_call_result_rejected` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_match_join_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_match_join_rejected` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_const_name_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_const_name_rejected` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_other_state_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_other_state_rejected` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_other_machine_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_other_machine_rejected` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_target_body_exit` | `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_target_body_rejected` |

Every positive place and the root `main.omg` binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64`, declares
`Expected exit: 0`, and carries `expected_native_exit.txt` `0`.

## Execution

```sh
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_saturating_multiplication_bound/target_body/main.omg
```

All ten applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:

- The positive places evaluate `compute_saturating(15i16, 8i16)`; the bounded
  operands establish `prod == 120i16` inside `[50i16, 200i16]`, and
  `verify_prod` proves `headroom: i16 = 200i16 - prod` and
  `margin: i16 = prod - 50i16` directly from the carried fact. If the bound or
  the operation returned anything else, the program branches to
  `state bad() { crash Trap; }` instead of exit 0.
- Each invalid twin widens the `lhs` upper bound from `20i16` to `21i16`
  (const/target places widen the literal operands to `21i16`/`10i16`), allowing
  `prod = 210i16 > 200i16`. The twin then rejects at compile time with the
  place's own contract diagnostic: `cannot prove requires contract for call
  verify_prod` (local, field, const_name, target_body), `... call verify_parameter`
  (parameter), `cannot prove ensures contract for exit from mul_saturating`
  (call_result), `... from choose_saturating` (match_join),
  `cannot prove requires contract for call step` (other_state) and
  `cannot prove requires contract for call forward` (other_machine). None
  reaches native execution.

Check all nine twins with:

```sh
omega --check samples/cli/facts/signed_16bit_saturating_multiplication_bound/<place>/invalid/main.omg
```

## Measured evidence

Measured on macOS arm64 with the factsr1 shared generation producer
`gen-4c23cc3391c` (revision `4c23cc3391c2e539d4065563ac95de7b18f2fc67`, omega
sha256 `83c7ae2a45442b1ceb6f5476a6f8ed4c4af9eabbd85e682a3ed7254c7fec94cf`,
corpus_runner sha256 `a1de848f1baca0837d28285d9da6da9acdd7639081c083743f76862a409bdb3d`):
the ten `omega run --both` invocations above report `native exit 0` and
`interp exit: 0` (20/20 engine observations); each of the nine `omega --check`
twin invocations exits 1 with its diagnostic above. No compiler repair was
needed and none landed.

## Pinned Fixtures

All 36 fixtures (108 files) are pinned under the corpus's fact names:

- `tests/omega/pass/facts/signed_16bit_saturating_multiplication_bound_through_<place>_exit/`
  — `main.omg`, `build.omg` (ProgramEntry bound for all four hosted targets) and
  `expected_native_exit.txt` `0`.
- `tests/omega/fail/facts/signed_16bit_saturating_multiplication_bound_through_<place>_rejected/`
  — the twin's `main.omg`, `build.omg` and `expected.txt` fragment.

Every pinned file is byte-identical to the sample file it names (36/36 pairs
verified with `cmp`).

## Requirements and Evidence

- **Owning item:** `ARITHMETIC-POLICY-REALIZATION` (`TASKS.md`), numeric-policy
  realization and full program acceptance.
- **Establishment:** Performing a 16-bit signed saturating multiplication `(lhs as i16 in Saturating) * (rhs as i16 in Saturating)` on bounded operands (`lhs >= 10i16 && lhs <= 20i16` and `rhs >= 5i16 && rhs <= 10i16`) statically establishes the bounded product interval `[10 * 5, 20 * 10] = [50, 200]` upon casting back to `i16`. Because all product values lie strictly in `[50, 200]`, the operation stays within 16-bit signed range `[-32768, 32767]` without clamping and directly proves that subsequent exact 16-bit signed subtractions `let headroom: i16 = 200i16 - prod` and `let margin: i16 = prod - 50i16` in `verify_prod` are safe and contract obligations are satisfied. Widening operand bounds (such as `lhs <= 21i16`, allowing `prod = 210i16`) causes downstream contracts (`requires prod <= 200i16` or `ensures result <= 200i16`) to fail closed at compile time.
- **Identity and lifetime:** The signed 16-bit product scalar preserves its 16-bit two's complement integer value and refined interval `[50, 200]` across local variables, struct fields (`ProdContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval `[50, 200]` composes across all nine places, allowing consuming exact subtractions `headroom: i16 = 200i16 - prod` and `margin: i16 = prod - 50i16` to execute safely from the carried fact, without post-hoc caller guards or dynamic checks. `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the construct's own operand bound (`lhs <= 20i16` vs `lhs <= 21i16`, or the const/target literals `15i16`/`8i16` vs `21i16`/`10i16`).
- **Soundness boundaries:** Saturating integer arithmetic operates in the clamping domain where arithmetic overflow/underflow clamps to integer extrema; casting back to exact `i16` retains static interval knowledge derived from operand ranges. In the invalid twins, widening the bound admits values exceeding `200i16` and the checker rejects the unproven bound instead of accepting it; no unproved bound reaches native execution.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives saturating multiplication operations (`Mul` on the `Saturating` signed 16-bit carrier `i16`) and domain casts with interval certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`, `interp exit: 0`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 16-bit integers (`i16` / `i16 in Saturating`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`). Calling plans place 16-bit parameters and return values in target register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return in `AX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 16-bit `imul` (or sign-extended 32-bit `imul`) on 16-bit register halves (`ax`, `cx`, `dx`) with saturation logic; on `arm64`, 32-bit `mul` on `W` registers with halfword clamping/sign-extension (`sxth`). Register homes and disjoint activation frame storage are assigned, and each entry carries its prologue/epilogue honoring the `StatePlan`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves displacements, emits valid native machine code, and executes with exit 0 matching Terminal Psi on the executing host across all four hosted-target bindings.

## What remains open

Widening signed saturating multiplication conversions (`i16 in Saturating -> i32`),
vector SIMD packed saturating products, and saturating operation results consumed
directly in `u32`-domain comparisons are not covered by this row. This row
closes the 16-bit signed saturating multiplication bound across the nine places
on the facts above; the parent item's remaining acceptance (mixed comparisons and
conversion surface on `source/library/core/numeric_conversion.omg`) is unchanged.