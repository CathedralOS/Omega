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

### Run record

Measured on 2026-10-21/22 with a fresh optimized producer built from this change's
candidate (`python3 tools/build_slot.py -- mbx build -p omega`, reported
`{"opt_level":"1"}`; the candidate was rebased onto
`5f2beccab8304737319f32377d97240781855d4e` after the first measurements on
`6cd22386cbb51240d17751e61d82b88632bbcd41`, and the row was re-measured on the rebased
candidate). Host: Intel macOS x86_64. Executing target: `macos_x86_64`.

The four authored targets do not include this host's catalogued target, so the bare
`omega run --both <place>/main.omg` refuses here during entry settlement with
`selected target macos_x86_64 has no bound required root slot macos_x86_64::ProgramEntry`.
That is host pinning, not a compiler result, and no fifth binding is added to the authored
sources. The same programs were executed from scratch copies whose `build.omg` additionally
binds `macos_x86_64::ProgramEntry` (and, for `target_body`, whose per-target bodies
additionally carry an identical `macos_x86_64 machine convert(...)`); the scratch copies are
otherwise byte-identical to the authored files. All ten reported `native exit 0` and
`interp exit: 0`. `omega run --target <name>` and `omega run --both --target <name>` are
compile-publication only on this host and were not used as runtime evidence.

The public acceptance command reports the same boundary:

```sh
python3 tools/sample_native_gate.py --cli <candidate> --facts --filter cli/facts/repro
```

It scores `0/0` runnable positives on `macos_x86_64` (10 unscored: not host-runnable)
and `9/9 intended-invalid controls match their authored rejection`.

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

- **Parent outcome:** [CANONICALIZE-SCALAR-RANGE-CONTRACTS](../../../../TASKS.md) — "locals the facts from initializers, guards and calls" (IEEE float conversions under [numeric values](../../../../wiki/spec/language/numeric_values.md#destination-owned-conversion)).
- **Requirement this row establishes:** a checked IEEE guard proves the unqualified exact `I32::from_f64` operand is finite and truncates into `i32`, and that proof survives the local, field, parameter, call-result, match-join, const-name, another-state, another-machine and another-target-body transport places, with each place's omit-the-upper-bound twin still refusing.
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/repro_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_exit`
  - Negative controls: `tests/omega/fail/facts/repro_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_rejected`
  - Earlier-pinned copies of the same programs: `tests/omega/pass/facts/ieee_guard_i32_from_f64_*` and `tests/omega/fail/facts/ieee_guard_i32_from_f64_*_invalid`.
- **Producer:** this change's candidate, optimized `opt_level=1`, based on
  `6cd22386cbb51240d17751e61d82b88632bbcd41`; host `macos_x86_64`.
- **Remaining open:** this row does not close its parent item. The same item's other
  frontiers — field/case propositions through construction, nonliteral and indexed
  writes, mutable recasts and returns, the ten-position recast inventory, and every
  remaining range-suffix customer — are untouched. Only IEEE comparisons whose
  Boolean subject is a captured transition guard were added to the carried
  conditional evidence; a receiving predicate still never supplies an enclosure, and
  qualified (`Trapping`/`Saturating`) overloads keep their own contracts.

### Specification and Lowering Layers

1. **Establishment:** The compound guard `value == value && value >= 0.0 && value < 1000000000.0` establishes that the `f64` value is non-NaN (`value == value` proves non-unordered), non-negative (`value >= 0.0`), and strictly below `1000000000.0` (which is well within `i32::MAX = 2147483647`). This discharges the prerequisite contract for exact, non-trapping `I32::from_f64(value)` conversion directly without any post-hoc caller equality guard (`transition x == 100`). A transition guard is evaluated once into a computed Boolean subject; the established IEEE enclosure and non-NaN identity ride that subject's recorded polarity into the flow-sensitive value environment, which is where the conversion's own finite/truncation obligation is decided. Before this repair the subject carried only integer order relations, so an IEEE comparison published no interval and no non-NaN fact and the conversion refused. The checked-flow observations of a saved guard are owned by the general saved-guard recovery in `flow/guard_predicates.rs`, landed separately with its own evidence.
2. **Identity and lifetime:** The established IEEE proposition (`FloatPredicate`) and interval bounds stay attached to the exact `f64` scalar value across local variable copies, single-variant record field projections, parameter passing, contracted call postconditions (`ensures result == value`), match arm joins, named constant comparisons, state transitions, machine calls, and target bodies. No mutating write intervenes between guard evaluation and conversion.
3. **Composition:** The guarded IEEE fact composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into `I32::from_f64(value)`. Each positive `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in omitting `< 1000000000.0` from the guard, isolating the upper-bound proof requirement under test.
4. **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compilation error `cannot prove unqualified \`I32::from_f64\` operand ... is finite and truncates into \`i32\`` when the upper bound is omitted. When NaN or negative values are supplied at runtime (`0.0 / 0.0`, `-1.0`), IEEE comparison rules correctly evaluate the guard to `false`, safely taking the `zero()` branch without invoking trapping conversion.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Proved IEEE non-NaN and range certificates lower into canonical target-neutral `PSITERM\0` float-to-integer truncation operations (`I32::from_f64`), erasing proof-only obligations while retaining float comparison operators, boolean transitions, and integer results. Both Terminal Psi interpreter and native execution agree in `omega run --both`.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Float (`f64` 8 bytes) and integer (`i32` 4 bytes) layout geometry validates target plans. `CallPlan` and `StatePlan` lower float arguments and integer returns across floating-point/vector registers (`xmm0`/`d0`) and general-purpose registers (`rax`/`x0`) according to target ABI conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA float comparison instructions (`ucomisd` on `x86_64`, `fcmp` on `arm64`), branch conditions honoring unordered status flags (`jp`/`parity` check for NaN), and conversion instructions (`cvttsd2si` on `x86_64`, `fcvtzs` on `arm64`). Backend assigns register homes and disjoint stack-frame storage, inserting standard entry prologues and epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and float constant pool offsets resolve, basic blocks and guard branches relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).
