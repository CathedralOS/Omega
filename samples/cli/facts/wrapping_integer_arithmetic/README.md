# Wrapping integer arithmetic

Fact: masking an unsigned scalar with a constant bitmask (`val & 63u64`) establishes the upper bound (`masked <= 63u64`), discharging exact subtraction `63u64 - masked`.

Every place performs the same operation: mask an unsigned `u64` with the constant bitmask `63u64`, establishing `masked >= 0u64 && masked <= 63u64`, which discharges exact subtraction `63u64 - masked` (`63u64 - 36u64 == 27u64`) to exit code 0. Only the transport of the masked value changes.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the bitwise mask range bound through each language construct, accompanied by `<place>/invalid/` reject twins:

| Place | Positive | Reject twin | Carrier / transport | Pinned pass fixture | Pinned reject fixture |
| --- | --- | --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | bitwise mask establishes bound and discharges subtraction directly | — | — |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | masked value bound to a local variable | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_local_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_local_rejected` |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | masked value stored in and projected from record field | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_field_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_field_rejected` |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | masked value passed as an explicit machine parameter | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_parameter_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_parameter_rejected` |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | masked value returned across machine call boundary | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_call_result_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_call_result_rejected` |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | masked value preserved across match convergence join | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_match_join_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_match_join_rejected` |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | masked value constrained against named constant | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_const_name_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_const_name_rejected` |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | masked value passed as an argument across state machine transition | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_other_state_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_other_state_rejected` |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | masked value forwarded through another machine | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_other_machine_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_other_machine_rejected` |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | masked value consumed across target-scoped machine bodies | `tests/omega/pass/facts/wrapping_integer_arithmetic_through_target_body_exit` | `tests/omega/fail/facts/wrapping_integer_arithmetic_through_target_body_rejected` |

Each positive place and root `main.omg` binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`, and passes `omega run --both`.

## Execution

```sh
omega run --both samples/cli/facts/wrapping_integer_arithmetic/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/local/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/field/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/parameter/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/call_result/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/match_join/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/const_name/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/other_state/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/other_machine/main.omg
omega run --both samples/cli/facts/wrapping_integer_arithmetic/target_body/main.omg
```

Measured on macOS arm64 with the factsr1 shared generation producer `gen-f1b0d3326d2` (omega sha256 `4e3950e53eb15fbb3b54868f9c8964e23e7efc5bdced336e07f18eb27e28ac4a`): all ten positive places report `native exit 0` and `interp exit: 0` (20/20 engine legs, exit 0).

### The oracle discriminates

Perturbing the root oracle's single comparison (`transition diff == 27u64` -> `transition diff == 28u64`) sends both engines to `state bad() { crash Trap; }` instead of `done()`: the perturbed program exits 255 reporting `native signal 5` and `interp crash: Trap (host presentation agrees)`. The authored exit `0` therefore strictly discriminates between correct evaluation and faulty execution.

### Reject twins

Each twin changes only the construct's own mask constant (`63u64` -> `64u64`), so the masked value can be `64u64` and `63u64 - masked` underflows. Checking fails closed at compile time; the runner-emitted diagnostic per place is:

- `local`, `field`, `call_result`, `match_join`, `const_name`: `exact arithmetic in machine \`main\` state \`entry\` local \`diff\` may overflow \`u64\``
- `parameter`: `exact arithmetic in machine \`compute\` state \`entry\` terminal expression may overflow \`u64\``
- `other_state`: `exact arithmetic in machine \`main\` state \`evaluate\` local \`diff\` may overflow \`u64\``
- `other_machine`: `exact arithmetic in machine \`forward\` state \`entry\` terminal expression may overflow \`u64\``
- `target_body`: `exact arithmetic in machine \`compute\` state \`entry\` terminal expression may overflow \`u64\`` (four diagnostics, one per target body)

```sh
omega --check samples/cli/facts/wrapping_integer_arithmetic/<place>/invalid/main.omg
```

All nine twins exit 1 at checking with their authored `exact arithmetic` and `may overflow \`u64\`` fragments satisfied; none reaches native execution.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/wrapping_integer_arithmetic_through_<place>_exit` and `tests/omega/fail/facts/wrapping_integer_arithmetic_through_<place>_rejected` for the nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its place's `main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy of its place twin's `main.omg`/`build.omg`/`expected.txt` (fragments `exact arithmetic` and `may overflow \`u64\``). All 108 copied files compare identical to their sample sources.

Golden rows were recorded on macOS arm64 at `jobs=3` with the same generation producer (`corpus_runner` sha256 `c25372101a8cd22676fc23b3b80b2387a58609a988353552f158251c65650b96`), stamped `@5648483032b7+wt`:

- `tests/omega/corpus_outcomes.txt`: 18 `checked` rows carrying `check-target:macos_arm64` and 18 `rejected` rows carrying their diagnostics with the authored fragments satisfied (`+60` lines with the 24 diagnostic continuation lines, `-0`);
- `tests/omega/corpus_native_macos_arm64.txt`: 18 `built` rows with `exit:0` (`+18`/`-0`);
- `tests/omega/corpus_both_macos_arm64.txt`: 18 `built` rows with `exit:0 interpreter-exit:0 interpreter:agrees` (`+18`/`-0`).

## Requirements and Evidence

- **Parent outcome:** **ONE-INTEGER-BOUNDS-ENGINE** ([TASKS.md](../../../../TASKS.md)) — this row is one client group's nine-place transport and execution of the shared carrier-aware bitwise provider's immutable unsigned AND (the item's `arithmetic/immutable_unsigned_mask_false_floor_rejected` guards the same bounds independently of formation and false bounds), with **ARITHMETIC-POLICY-REALIZATION** owning the underlying numeric-policy realization.
- **Requirement established:** End-to-end nine-place bitwise mask range synthesis, modular arithmetic bound propagation, and exact integer arithmetic proof obligation discharge for unsigned scalar bitwise AND (`val & 63u64`).

Full 8-layer specification and lowering evidence:

- **Establishment:** The bitwise AND operation `val & 63u64` on unsigned integer `val: u64` with constant bitmask `63u64` (binary `0b0011_1111`) directly establishes the static range upper bound `[0, 63]`. The static range analysis synthesizer deduces that the bitwise AND of any non-negative integer with mask $M = 2^k - 1$ is bounded above by $M$, establishing `masked <= 63u64` without requiring any post-hoc caller equality guard (`transition diff == 27u64` checks the runtime value against expected subtraction `63 - 36`, rather than manufacturing the proof premise).
- **Identity and lifetime:** The established range invariant `[0, 63]` attaches to the exact value produced by `val & 63u64`. The `u64` scalar value preserves its range bounds across local variable binding, aggregate record field storage and projection, explicit machine parameter passing, machine call return values, match join unification, constant expressions, state machine transition argument handoff, machine forwarding chains, and target-scoped body selection.
- **Composition:** The bitwise mask range bound composes across all 9 standard places to statically discharge exact subtraction `63u64 - masked`:
  - `local`: bitwise-masked value is bound to a local variable `masked` before subtracting from `63u64`.
  - `field`: bitwise-masked value is stored in `Carrier.masked` record field and projected out.
  - `parameter`: bitwise-masked value is passed to `compute(masked: u64)` with `requires masked <= 63u64`.
  - `call_result`: helper machine `mask_value(val)` ensures `result <= 63u64` and is consumed across the call boundary.
  - `match_join`: divergent match branches (masking with `63u64` and `31u64`) converge to upper bound `63u64`.
  - `const_name`: bitmask is provided by compile-time named constant `const MASK: u64 = 63u64`.
  - `other_state`: bitwise-masked value is passed to state `evaluate(masked: u64)` across a transition edge.
  - `other_machine`: bitwise-masked value and subtraction are forwarded through helper machine `forward(val)`.
  - `target_body`: bitwise masking and subtraction are executed across target-scoped bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in breaking the mask/bound from `63u64` to `64u64`, proving that exact subtraction `63u64 - masked` is strictly dependent on the established upper bound.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine ... may overflow \`u64\``, confirming that the arithmetic proof engine soundly detects that when mask `64u64` is used, the value can be 64, making `63u64 - masked` underflow `u64`. Both engines agree on the perturbed oracle (`native signal 5` / `interp crash: Trap`).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower without loss into canonical `PSITERM\0` target-neutral representations. The bitwise AND and exact subtraction lower into canonical operations with verified integer certificates, and erased proof terms are eliminated while runtime execution agrees between Terminal Psi interpretation and native execution (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 64-bit scalar layout (8 bytes, alignment 8) and record layout, while `CallPlan`/`StatePlan` assign register and stack homes for scalar parameters and returns.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for 64-bit bitwise AND (`and`), 64-bit subtraction (`sub`), and equality comparisons. Backend register allocation assigns register homes and disjoint stack activation frames, inserting entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements resolve into concrete offsets. Basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute natively with Terminal Psi agreement (`omega run --both` reports `native exit 0` and `interp exit: 0`) across host platforms.

Remaining open under **ONE-INTEGER-BOUNDS-ENGINE**: non-power-of-two bitwise masks requiring multi-interval SMT reasoning, dynamic variable bitmask range propagation, and non-linear bitwise combinations; the native and combined golden rows are macOS arm64 legs, so Linux and Windows host agreement rests on the four-target `ProgramEntry` bindings and the shared Terminal Psi product rather than a replayed run on those hosts.