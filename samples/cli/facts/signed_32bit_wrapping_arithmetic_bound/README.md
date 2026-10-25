# Signed 32-bit wrapping arithmetic bound

Fact: 32-bit signed wrapping addition lhs +% rhs with bounded interval 50i32 <= sum <= 100i32 establishes 50i32 <= sum <= 100i32 across all nine places so exact subtractions 100i32 - sum and sum - 50i32 are safe.

Owning Task: `ARITHMETIC-POLICY-REALIZATION` ([TASKS.md](../../../../TASKS.md)) — numeric-policy realization of the wrapping integer mode through stage 04 scalar checking, stage 05 expression preparation and the Terminal integer-policy representation ([numeric values](../../../../wiki/spec/language/numeric_values.md)). `CANONICALIZE-SCALAR-RANGE-CONTRACTS` owns the proposition-based scalar range surface these bounds ride on: machine/state `requires`, results `ensures`, and locals the facts from initializers, guards and calls.

Every positive place and reject twin tests 32-bit signed wrapping addition (`i32 in Wrapping`).
When bounded operands `lhs >= 40i32 && lhs <= 80i32` and `rhs >= 10i32 && rhs <= 20i32` are cast into the Wrapping arithmetic domain (`lhs as i32 in Wrapping` and `rhs as i32 in Wrapping`), their modular sum `wl + wr` produces a bounded result in `[50, 100]`.
Casting back to exact signed `i32` statically preserves the interval `sum >= 50i32 && sum <= 100i32`.
Subsequent exact subtractions `headroom = 100i32 - sum` and `margin = sum - 50i32` are proved underflow-free and non-negative, admitting exact conversion to `u32` without requiring any post-hoc caller equality guard before the operations, evaluating to `25u32 + 25u32 = 50u32` for `lhs == 60i32` and `rhs == 15i32` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded i32 operands wrapping-added and dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | wrapping-added 32-bit signed value is copied into a local binding before exact subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | wrapping-added 32-bit signed value is stored into and read from a record field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | wrapping-added 32-bit signed value is passed as an explicit machine parameter before exact subtractions |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | wrapping-added 32-bit signed value is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the wrapping-added 32-bit signed value |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower ceilings for exact subtractions |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the wrapping-added 32-bit signed value into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the wrapping-added 32-bit signed value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing the wrapping addition and subtractions |

Match the fact sentence beside every place: each positive place is pinned as
`tests/omega/pass/facts/signed_32bit_wrapping_arithmetic_bound_through_<place>_exit` and
each reject twin as
`tests/omega/fail/facts/signed_32bit_wrapping_arithmetic_bound_through_<place>_rejected`,
byte-equal to the place and `invalid/` programs beside them.

## Pinned Fixtures

Nine positive fixtures (`main.omg`, `build.omg`, `expected_native_exit.txt` = `0`) and
nine reject twins (`main.omg`, `build.omg`, `expected.txt`) are pinned in the compiler corpus:

- `tests/omega/pass/facts/signed_32bit_wrapping_arithmetic_bound_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_exit/`
- `tests/omega/fail/facts/signed_32bit_wrapping_arithmetic_bound_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_rejected/`

Each positive fixture records `checked` with `check-target:macos_arm64` and each reject twin
records `rejected expected` carrying its own diagnostic in `tests/omega/corpus_outcomes.txt`;
the positive fixtures additionally record `built exit:0` in
`tests/omega/corpus_native_macos_arm64.txt` and `built exit:0 interpreter-exit:0 interpreter:agrees`
in `tests/omega/corpus_both_macos_arm64.txt`.

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_wrapping_arithmetic_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(60i32, 15i32)` computes `sum = 60i32 +% 15i32 = 75i32`.
Because `lhs in [40, 80]` and `rhs in [10, 20]`, wrapping addition statically establishes `sum in [50, 100]`.
Subsequent exact subtractions `headroom = 100i32 - sum` (`25i32`) and `margin = sum - 50i32` (`25i32`) are proved non-negative and in range `[0, 50]`, admitting exact conversion `as u32` to yield `25u32 + 25u32 = 50u32`.
The transition guard `combined == 50u32` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If wrapping addition or subtraction evaluation is corrupted, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the narrow input precondition bound on `rhs` from `rhs <= 20i32` to `rhs <= 21i32`.
When `rhs == 21i32`, `sum` reaches `80 + 21 = 101i32 > 100i32`, making `100i32 - sum = -1i32`, which violates non-negativity and cannot be representably cast to `u32` (or violates caller contract bounds across place boundaries).
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic (exact, as pinned in `expected.txt`) |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `Exact integer cast in machine \`probe\` state \`entry\` from \`i32\` to \`u32\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `field` | native 0 / interp 0 | `Exact integer cast in machine \`probe\` state \`entry\` from \`i32\` to \`u32\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call consume_sum from probe: sum >= 50 && sum <= 100 (needs fact \`forwarded >= 50 && forwarded <= 100\` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state \`entry\`` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from wrap_add at statement 3: result >= 50 && result <= 100` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: sum >= 50 && sum <= 100 && 75 >= 50 && 75 <= 100 (needs fact \`first >= 50 && first <= 100 && second >= 50 && second <= 100\` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state \`entry\`` |
| `const_name` | native 0 / interp 0 | `Exact integer cast in machine \`probe\` state \`entry\` from \`i32\` to \`u32\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: sum >= 50 && sum <= 100 (needs fact \`forwarded >= 50 && forwarded <= 100\` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state \`entry\`` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_sum from probe: sum >= 50 && sum <= 100 (needs fact \`mid >= 50 && mid <= 100\` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state \`entry\`` |
| `target_body` | native 0 / interp 0 | `Exact integer cast in machine \`target_probe\` state \`entry\` from \`i32\` to \`u32\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract preconditions `lhs >= 40i32 && lhs <= 80i32` and `rhs >= 10i32 && rhs <= 20i32` establish bounded 32-bit signed intervals. Casting operands into `i32 in Wrapping` evaluates wrapping modular addition modulo $2^{32}$, statically proving the absence of overflow/wrap and establishing the exact post-addition interval `[50, 100]` without post-hoc caller equality guards.
- **Identity and lifetime**: The computed interval fact `sum in [50, 100]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `sum in [50, 100]` composes directly with exact subtractions `headroom = 100i32 - sum` and `margin = sum - 50i32` to statically discharge underflow safety and non-negativity, proving `headroom in [0, 50]` and `margin in [0, 50]` to satisfy safe `as u32` representation without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the input bound to `rhs <= 21i32` permits `sum` to reach `101i32`, causing `100i32 - sum` to equal `-1i32` which fails closed under exact integer cast representability (or contract violation on boundary transmission), verifying that negative values cannot masquerade as valid unsigned margins.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Wrapping addition in the Wrapping domain lowers to target-neutral Terminal Psi `WrappingIntegerAdd` (`PSITERM\0`) alongside integer conversion instructions, while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 4-byte `i32` and 4-byte `u32` in their stored widths (`IntegerAt 4-byte`) across all four hosted ABI calling conventions (CL on `windows_x86_64`, DIL on `linux_x86_64`, W0 on `macos_arm64` and `linux_arm64`), returning 32-bit unsigned results in EAX / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA 32-bit modular addition instructions: `add eax, ecx` on `x86_64` and `add w0, w0, w1` on `arm64`, followed by exact subtractions `sub ...`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
