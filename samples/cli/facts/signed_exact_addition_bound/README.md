# Signed exact addition bound

Fact: Adding two bounded signed i64 values (base >= 20i64 && base <= 40i64 and delta >= 10i64 && delta <= 30i64) establishes the exact post-addition interval (sum >= 30i64 && sum <= 70i64), discharging exact subtraction 70i64 - sum across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state parameters
`requires`, results `ensures`, locals the facts from initializers, guards and calls" — this row
is one customer group's nine-place transport and execution of that settled proposition-based
scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy
realization.
Claim Ticket: `734f3cb98864422f909a5528490239a2`

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness adding bounded signed i64 scalars and discharging 70i64 - sum |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The bounded sum is copied into a local binding before exact subtraction. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | The bounded sum is stored in and retrieved from a record field before exact subtraction. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The bounded sum is passed as an explicit machine parameter before exact subtraction. |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | The bounded sum is returned from a helper machine call before exact subtraction. |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | The bounded sum is unified across conditional branches before exact subtraction. |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant supplies the subtrahend ceiling for the bounded sum. |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | The bounded sum is transported across an explicit state transition before exact subtraction. |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Another machine forwards the bounded sum before exact subtraction. |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies perform exact addition and bound consumption across all hosted targets. |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter
and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_exact_addition_bound/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/local/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/field/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_exact_addition_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, `base` (in `[20, 40]`) and `delta` (in `[10, 30]`) are added via exact signed arithmetic to produce `sum = base + delta` (statically known in `[30, 70]`).
Evaluating `margin = 70i64 - sum` produces a value in `[0, 40]`, satisfying `consume_margin(margin) requires margin >= 0i64 && margin <= 40i64`.
Testing `probe(30i64, 20i64)` evaluates `30 + 20 = 50`, `70 - 50 = 20`.
The caller validates `diff == 20i64`, transitioning to `done()` with exit 0.
Corrupting the operand to `31i64 + 20i64 = 51i64` would evaluate `70 - 51 = 19 != 20`, causing the guard to fail and transition to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` twin that widens `delta` from `delta <= 30i64` to `delta <= 31i64`.
This allows `sum` to reach `71i64`, causing `70i64 - sum` to reach `-1i64` and violating the contract requirements.
The compiler strictly fails closed at compile time with:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `cannot prove requires contract for call consume_margin from probe: margin >= 0 && margin <= 40` |
| `field` | native 0 / interp 0 | `cannot prove requires contract for call consume_margin from probe: margin >= 0 && margin <= 40` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call subtract_and_consume from probe: sum >= 30 && sum <= 70` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from compute_sum at statement 0: result >= 30 && result <= 70` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: sum >= 30 && sum <= 70` |
| `const_name` | native 0 / interp 0 | `cannot prove requires contract for call consume_margin from probe: margin >= 0 && margin <= 40` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: sum >= 30 && sum <= 70` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_sum from probe: sum >= 30 && sum <= 70` |
| `target_body` | native 0 / interp 0 | `cannot prove requires contract for call consume_margin from target_probe: margin >= 0 && margin <= 40` |

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_exact_addition_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_exact_addition_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample
place's `main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a
byte-identical copy of its place twin's `main.omg`/`build.omg`/`expected.txt`.

Measured on macOS arm64 with the factsr1 shared producer built from base `9d9ba4fb625`
(omega sha256 `5a2fde4a3a914444db08f00e9849e06557fe0d6c8cf5fd5591c9b413bdd0e9ab`,
corpus_runner sha256 `98a455599b4d5e13da5f950255736147d1e7fb4508acc76fc6ea5acba18da00a`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and
  `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse with their authored
  `cannot prove requires contract ...` fragment.
- `python3 tools/corpus_gate.py --runner <gen>/corpus_runner --record --filter
  facts/signed_exact_addition_bound,facts/signed_exact_subtraction_bound` recorded three legs
  stamped `@9d9ba4fb625+wt`: 36 outcome rows (18 `checked check-target:macos_arm64` plus 18
  `rejected expected`), 18 native rows (`built ... exit:0`) and 18 both rows (`exit:0
  interpreter-exit:0 interpreter:agrees`). Every golden diff was a pure addition (0 deletions),
  with 0 unimplemented rows and 0 rows recording a lost success.

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract preconditions `base >= 20i64 && base <= 40i64` and `delta >= 10i64 && delta <= 30i64` establish bounded signed intervals for both addends. The arithmetic expression `base + delta` synthesizes the post-addition interval `[30, 70]`.
- **Identity and lifetime**: The computed interval fact `sum in [30, 70]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `sum in [30, 70]` composes with subtraction `margin = 70i64 - sum` to prove `margin in [0, 40]`, discharging `requires margin >= 0i64 && margin <= 40i64` for `consume_margin` without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the operand bound to `delta <= 31i64` makes `70i64 - sum` potentially negative (`-1i64`), causing compile-time rejection due to contract violation and proving that out-of-interval values cannot pass unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Signed interval synthesis and contract verification lower cleanly into canonical target-neutral Terminal Psi (`PSITERM\0`) and execute with identical semantics in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 64-bit signed integers in standard machine words across all four hosted ABI calling conventions (RCX/RDX on `windows_x86_64`, RDI/RSI on `linux_x86_64`, X0/X1 on `macos_arm64` and `linux_arm64`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering emits target ISA 64-bit integer addition and subtraction (`add`, `sub` / `neg`) with correct signed semantics, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Binary emission produces complete PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
