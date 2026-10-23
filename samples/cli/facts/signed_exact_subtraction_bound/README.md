# Signed Exact Subtraction Bound

Subtracting two bounded signed `i64` values (`minuend >= 80i64 && minuend <= 100i64` and `subtrahend >= 20i64 && subtrahend <= 30i64`) establishes the exact post-subtraction interval (`diff >= 50i64 && diff <= 80i64`), discharging a downstream contract requiring `diff >= 50i64 && diff <= 80i64` across all nine places.

Every place performs the same operation: define bounded signed parameters `minuend` (`80..100`) and `subtrahend` (`20..30`), evaluate exact subtraction `diff: i64 = minuend - subtrahend`, transport the difference through the place, pass `diff` to `consume(diff: i64)` which requires `diff >= 50 && diff <= 80`, execute non-negative conversion and exact subtraction `u - 50u64` returning `15i32`, and verify in `main` that `res == 15` exiting 0. Only the transport mechanism of the difference changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root signed exact subtraction bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded signed i64 operands and their difference are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the difference of the signed subtraction is stored in a struct field before passing to the downstream contract |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the difference of the signed subtraction is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the difference arrives as the return result of a helper machine call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the difference so the joined binding preserves the downstream contract |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant subtrahend subtracts from the bounded minuend |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the difference to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the difference through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the signed exact subtraction and satisfies the downstream contract |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_exact_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_exact_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, `subtract_signed(90, 25)` evaluates `90i64 - 25i64`, establishing difference `65i64` which is within `50i64..80i64`. Passing `diff` to `consume(diff)` satisfies its contract `diff >= 50 && diff <= 80`. `consume` converts `65` to `u64`, performs exact subtraction `65u64 - 50u64 == 15u64`, and returns `15i32`. In `main`, `transition res == 15` transitions to `state done() {}`, terminating with native exit 0. If subtraction or contract satisfaction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin weakens the minuend bound (e.g. `minuend >= 79i64` or `minuend >= 74i64`), so `minuend - subtrahend` can be `49i64`. Calling `consume(diff)` fails closed at compile time with `cannot prove requires contract for call consume`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_exact_subtraction_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --cli <gen>/omega --filter cli/facts/signed_exact --jobs 2
```

Measured on macOS arm64 with the factsr1 shared producer built from base `9d9ba4fb625`:
- 30/30 filtered samples meet their documented oracle on `macos_arm64` — the ten places and
  root of this row, of `signed_exact_addition_bound`, and of `signed_exact_multiplication_bound`.
- 36/36 filtered intended-invalid controls match their authored rejection; this row's nine
  twins all match `cannot prove requires contract for call consume`.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_exact_subtraction_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_exact_subtraction_bound_through_<place>_rejected` for the nine
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
  `cannot prove requires contract for call consume` fragment.
- `python3 tools/corpus_gate.py --runner <gen>/corpus_runner --record --filter
  facts/signed_exact_addition_bound,facts/signed_exact_subtraction_bound` recorded three legs
  stamped `@9d9ba4fb625+wt`: 36 outcome rows (18 `checked check-target:macos_arm64` plus 18
  `rejected expected`), 18 native rows (`built ... exit:0`) and 18 both rows (`exit:0
  interpreter-exit:0 interpreter:agrees`). Every golden diff was a pure addition (0 deletions),
  with 0 unimplemented rows and 0 rows recording a lost success.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state
  parameters `requires`, results `ensures`, locals the facts from initializers, guards and
  calls". This row is one customer group's nine-place transport and execution of that settled
  proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying
  numeric-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for signed `i64` exact subtraction interval synthesis and downstream contract discharge. Specifically, subtracting two bounded signed integers (`80i64 <= minuend <= 100i64` and `20i64 <= subtrahend <= 30i64`) statically establishes the difference interval `50i64 <= diff <= 80i64`, discharging a downstream callee requiring `50i64 <= diff <= 80i64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires minuend >= 80 && minuend <= 100 && subtrahend >= 20 && subtrahend <= 30`) introduce exact interval facts for signed operands. The subtraction expression `minuend - subtrahend` synthesizes the exact difference interval `[80 - 30, 100 - 20] = [50, 80]` directly upon evaluation, discharging downstream precondition `diff >= 50 && diff <= 80` without requiring any post-hoc caller equality guard (`transition x == 65`).
  - **Identity and lifetime:** The synthesized difference value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The signed difference interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`consume(diff)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the minuend bound (`minuend >= 80` vs `minuend >= 79`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `minuend >= 79` is specified: the difference interval expands to `[49, 80]`, which fails the downstream contract `diff >= 50 && diff <= 80`, strictly rejecting with `cannot prove requires contract for call consume`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact signed subtraction operations and contract requirements lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving the concrete 64-bit signed integer subtraction. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 64-bit signed integer layout plan (`IntegerAt`: `i64` size 8, alignment 8) validates, and target `CallPlan`/`StatePlan` assign 64-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit subtraction instructions (`sub rax, rdx` on `x86_64`; `sub x0, x0, x1` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA subtraction instructions encode into machine bytes (REX.W-prefixed `sub` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Non-linear signed division interval synthesis under variable divisors, modular signed arithmetic domain boundaries, and SIMD packed vector subtraction interval propagation.
