# float_range_delivery

The same-format inclusive f64 fact `0.0 <= value && value <= 0.25`
proves the wider requirement `0.0 <= value && value <= 0.5` at `admit(value)`.
No conversion is inserted. The root witness and nine place witnesses use canonical
`where`, `requires`, `ensures`, and established flow facts.

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`,
and carries an authored `expected_native_exit.txt` containing `0` beside its entry. Both engines
(`omega run --both`) agree: Terminal Psi and native execution report exit 0.

## Places

| Place | Positive Program | Fact-sensitive Reject Twin | Carrier / Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | root witness delivering `forward`'s narrower premise to `admit` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded `Reading` record copied to local variable before `admit` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | nested record field projection `holder.reading.value` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded `Reading` parameter unpacked and forwarded directly |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call result delivers bounded float value |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | match arms join on bounded `Reading` before `admit` |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant endpoints bound the checked float range |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | state transition passes bounded `Reading` to successor machine |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | intermediate machine forwards bounded `Reading` |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute on each hosted platform |

## Execution

Every positive has an authored native exit oracle of zero and exercises both
an interior payload 0.125 and the inclusive upper endpoint 0.25. The match also
exercises its arms across both values. Run with `omega run --both`:

```sh
omega run --both samples/cli/facts/float_range_delivery/main.omg
omega run --both samples/cli/facts/float_range_delivery/local/main.omg
omega run --both samples/cli/facts/float_range_delivery/field/main.omg
omega run --both samples/cli/facts/float_range_delivery/parameter/main.omg
omega run --both samples/cli/facts/float_range_delivery/call_result/main.omg
omega run --both samples/cli/facts/float_range_delivery/match_join/main.omg
omega run --both samples/cli/facts/float_range_delivery/const_name/main.omg
omega run --both samples/cli/facts/float_range_delivery/other_state/main.omg
omega run --both samples/cli/facts/float_range_delivery/other_machine/main.omg
omega run --both samples/cli/facts/float_range_delivery/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Every reject twin removes only the narrow upper premise (`value <= 0.25`),
leaving only `value >= 0.0`, and fails closed at proof checking before native execution:

```sh
omega --check samples/cli/facts/float_range_delivery/<place>/invalid/main.omg
```

All 9 reject twins fail closed with `cannot prove requires contract for call admit`
and `value >= 0.0 && value <= 0.5`.

The const-name witness names the scalar endpoints: floats and records containing
floats are not canonical generic atoms.

The compiler transports proved float observations through ordinary value facts,
entry contracts, completed data construction, and covered match-result joins.
It retains NaN behavior, format identity, write invalidation, and exact subject
identity. Regression controls include out-of-range construction, omitted-zero
construction, f32 rounding, unknown NaN, mutable storage, copied scalar snapshots,
asymmetric joins, an unknown arm, write-only storage, and gated native entry.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and IEEE `f64` range delivery. The narrower inclusive IEEE `f64` interval premise (`value >= 0.0 && value <= 0.25`) satisfies the wider `requires` interval (`value >= 0.0 && value <= 0.5`) at `admit(value)` across all nine program places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), with exact agreement between Terminal Psi and native execution across hosted targets.
- **Pinned fixtures:** positive places and reject twins under `tests/omega/{pass,fail}/facts/float_range_delivery_*`.
- **Authority:** `wiki/spec/language/numeric_values.md` (bounds as propositions, same-format delivery, IEEE comparison), `dependent_values.md` (construction and exact contract substitution), `evaluation.md` (canonical atom eligibility), `expressions.md` (once-only evaluation and mutation), and `state_contracts.md` (all-predecessor joins and fact lifetime).

Full 8-layer specification and lowering evidence:

- **Establishment:** The narrower IEEE `f64` interval premise (`value >= 0.0 && value <= 0.25`) establishes finite, non-NaN ordered bounds directly on the exact subject without any post-hoc caller equality guard (`transition x == 100`). In `main.omg`, `forward` requires `value >= 0.0 && value <= 0.25`; in the nine places, `Reading` enforces `where value >= 0.0 && value <= 0.25` on construction. The established static premise guarantees non-NaN ordered values within `[0.0, 0.25]` prior to admitting the call.
- **Identity and lifetime:** The IEEE `f64` interval fact remains bound to the exact scalar subject across local copy (`local`), record field projection (`field`), explicit parameter passing (`parameter`), completed call result (`call_result`), match convergence join (`match_join`), named const bounds (`const_name`), state transition arguments (`other_state`), forwarding machine calls (`other_machine`), and target-scoped body implementations (`target_body`). No mutation, alias clobbering, or storage invalidation intervenes between fact establishment and consumption.
- **Composition:** The narrower `f64` range (`<= 0.25`) entails the wider `requires` interval (`<= 0.5`) at `admit(value)` across all 9 places without inserting runtime conversions. Calls substitute exact actuals, match joins retain the bound justified on both incoming arms (`0.125` and `0.25`), and state transitions prove arrival obligations on incoming edges. Each `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in the float interval ceiling (the narrow upper premise `<= 0.25` is removed), never in a caller guard.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed with `cannot prove requires contract for call admit` and `value >= 0.0 && value <= 0.5` when any incoming path weakens the `f64` upper bound beyond `0.5` or leaves NaN/out-of-range values possible. The checker strictly refuses unproved upper bounds during proof checking, preventing invalid programs from reaching Terminal Psi or native code emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Checked `f64` range contracts (`where`, `requires`) erase cleanly after verification while IEEE `f64` values, comparisons, and calls lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`). The resulting programs execute identically in the Terminal Psi interpreter (`omega run --both`), observing exact float values `0.125` and `0.25`.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Concrete layout geometry for 8-byte IEEE `f64` scalars and `Reading`/`Holder` records resolves with 8-byte alignment and offset plans. `CallPlan` and `StatePlan` place `f64` scalar arguments and returns in target ABI floating-point registers (`xmm0`–`xmm3` on `windows_x86_64`, `xmm0`–`xmm7` on `linux_x86_64`, `d0`–`d7` on `macos_arm64`/`linux_arm64`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions for IEEE `f64` scalar moves (`movsd` on x86-64 / `ldr`/`fmov` on ARM64) and ordered comparisons (`ucomisd` on x86-64 / `fcmp` on ARM64). Backend assigns FP/SIMD register homes and disjoint stack-frame spill slots across nested calls (`admit(bounded)`), inserting entry prologues and multi-return epilogues honoring target calling conventions.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Read-only IEEE `f64` constant-pool relocations (literals `0.0`, `0.125`, `0.25`, `0.5`) and symbolic stack-frame displacements resolve. Basic blocks and parity/ordered float conditional branches relax with offset reflow. Transitive machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across `windows_x86_64` and `macos_arm64`.

## What remains open

Mixed-format float delivery (e.g. `f32` to `f64` widening without explicit conversion), non-orderable float NaN handling across arithmetic domains, and broader interval arithmetic chains beyond direct subset entailment remain governed under the wider `CANONICALIZE-SCALAR-RANGE-CONTRACTS` and `wiki/spec/language/numeric_values.md` roadmap.
