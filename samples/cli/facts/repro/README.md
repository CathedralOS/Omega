# Checked IEEE guard for `I32::from_f64`

Checked IEEE guard `value == value && value >= 0.0 && value < 1000000000.0` proves `I32::from_f64(value)` is finite and in range.

The operation is `I32::from_f64(...)` under the checked IEEE guard `value == value && value >= 0.0 && value < 1000000000.0`. For `123.75` it returns `123`; for `2000000000.0`, `-1.0`, and `0.0 / 0.0` the guard takes the `false` branch and returns `0`. Each place copies that sentence and performs that conversion; only the transport of the guarded value changes.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness reading local copy of `value` under the guard |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | a local copy of `value` read under the guard |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field initialized from `value` and read under the guard |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the parameter `value` read directly under the guard |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call whose `requires` carries the guard and whose `ensures` equates `result == value` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | a covered boolean match whose arms both yield `value` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named `f64` constants `LOWER` and `UPPER` in the guard |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state whose `requires` carries the guard |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a callee machine whose `requires` carries the guard |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | four target-qualified machine bodies whose `requires` carries the guard |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.
`omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/repro/main.omg
omega run --both samples/cli/facts/repro/local/main.omg
omega run --both samples/cli/facts/repro/field/main.omg
omega run --both samples/cli/facts/repro/parameter/main.omg
omega run --both samples/cli/facts/repro/call_result/main.omg
omega run --both samples/cli/facts/repro/match_join/main.omg
omega run --both samples/cli/facts/repro/const_name/main.omg
omega run --both samples/cli/facts/repro/other_state/main.omg
omega run --both samples/cli/facts/repro/other_machine/main.omg
omega run --both samples/cli/facts/repro/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

`main` verifies conversion correctness across multiple edge cases:
`inside == 123` (finite positive fractional `123.75` truncates to `123`),
`above == 0` (out-of-range positive `2000000000.0` fails guard and takes false branch `0`),
`below == 0` (negative `-1.0` fails guard and takes false branch `0`),
and `unordered == 0` (NaN `0.0 / 0.0` fails self-equality `value == value` and takes false branch `0`).
If any conversion or branch evaluates incorrectly, the transition condition `inside == 123 && above == 0 && below == 0 && unordered == 0` evaluates to `false`, transitioning to `state bad() { crash Trap; }` which crashes with a non-zero exit/signal.

### Invalid controls and reject twins

Each place has an `invalid/` twin that omits the upper bound `< 1000000000.0` from the guard,
leaving `value < 2147483648.0` unproven at `I32::from_f64`.

It must reject at `--check` with:

```
cannot prove unqualified `I32::from_f64` operand `copied` is finite and truncates into `i32`
```

Restoring the upper bound makes each twin check and pass, proving that the twin breaks this exact fact and nothing else.

```sh
omega --check samples/cli/facts/repro/local/invalid/main.omg
omega --check samples/cli/facts/repro/field/invalid/main.omg
omega --check samples/cli/facts/repro/parameter/invalid/main.omg
omega --check samples/cli/facts/repro/call_result/invalid/main.omg
omega --check samples/cli/facts/repro/match_join/invalid/main.omg
omega --check samples/cli/facts/repro/const_name/invalid/main.omg
omega --check samples/cli/facts/repro/other_state/invalid/main.omg
omega --check samples/cli/facts/repro/other_machine/invalid/main.omg
omega --check samples/cli/facts/repro/target_body/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** IEEE float conversion and refinement / [numeric values](../../../../wiki/spec/language/numeric_values.md).
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/ieee_guard_i32_from_f64_*`
  - Negative controls: `tests/omega/fail/facts/ieee_guard_i32_from_f64_*`

### Specification and Lowering Layers

1. **Establishment:** The compound guard `value == value && value >= 0.0 && value < 1000000000.0` establishes that the `f64` value is non-NaN (`value == value` proves non-unordered), non-negative (`value >= 0.0`), and strictly below `1000000000.0` (which is well within `i32::MAX = 2147483647`). This discharges the prerequisite contract for exact, non-trapping `I32::from_f64(value)` conversion directly without any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The established IEEE proposition (`FloatPredicate`) and interval bounds stay attached to the exact `f64` scalar value across local variable copies, single-variant record field projections, parameter passing, contracted call postconditions (`ensures result == value`), match arm joins, named constant comparisons, state transitions, machine calls, and target bodies. No mutating write intervenes between guard evaluation and conversion.
3. **Composition:** The guarded IEEE fact composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into `I32::from_f64(value)`. Each positive `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in omitting `< 1000000000.0` from the guard, isolating the upper-bound proof requirement under test.
4. **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compilation error `cannot prove unqualified \`I32::from_f64\` operand ... is finite and truncates into \`i32\`` when the upper bound is omitted. When NaN or negative values are supplied at runtime (`0.0 / 0.0`, `-1.0`), IEEE comparison rules correctly evaluate the guard to `false`, safely taking the `zero()` branch without invoking trapping conversion.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Proved IEEE non-NaN and range certificates lower into canonical target-neutral `PSITERM\0` float-to-integer truncation operations (`I32::from_f64`), erasing proof-only obligations while retaining float comparison operators, boolean transitions, and integer results. Both Terminal Psi interpreter and native execution agree in `omega run --both`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Float (`f64` 8 bytes) and integer (`i32` 4 bytes) layout geometry validates target plans. `CallPlan` and `StatePlan` lower float arguments and integer returns across floating-point/vector registers (`xmm0`/`d0`) and general-purpose registers (`rax`/`x0`) according to target ABI conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA float comparison instructions (`ucomisd` on `x86_64`, `fcmp` on `arm64`), branch conditions honoring unordered status flags (`jp`/`parity` check for NaN), and conversion instructions (`cvttsd2si` on `x86_64`, `fcvtzs` on `arm64`). Backend assigns register homes and disjoint stack-frame storage, inserting standard entry prologues and epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and float constant pool offsets resolve, basic blocks and guard branches relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).
