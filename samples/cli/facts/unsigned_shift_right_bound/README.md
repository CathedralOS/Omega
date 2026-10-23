# Unsigned shift right bound

Fact: logical right shift on a bounded `u64` value by a bounded shift count establishes the exact post-shift interval, discharging exact subtraction `shifted - 100u64`.

Every place performs the same operation: logically shift a bounded `u64` value (`val >= 400u64 && val <= 400u64`) right by a bounded shift count (`shift >= 2u64 && shift <= 2u64`), establishing the exact post-shift value `100u64` (`shifted >= 100u64 && shifted <= 100u64`), which discharges exact subtraction `shifted - 100u64` to exit code 0. Only the transport of the operands changes.

## Places

| Place | Positive | Reject twin | Transport | Pinned pass fixture | Pinned reject fixture |
| --- | --- | --- | --- | --- | --- |
| root (anchor) | [source](main.omg) | — | Root reference row performing bounded shift right and exact subtraction. | — | — |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | Both the bounded u64 value and bounded shift count are copied into locals. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_local_rejected` |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | Bounded record fields carry the u64 value and shift count into the shift. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_field_rejected` |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | The shift machine reads its parameters directly. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_parameter_rejected` |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | Calls with declared bounds supply both operands to the shift. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_call_result_rejected` |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | Both arms produce operands within bounds so the joined operands shift safely. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_match_join_rejected` |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | Named constants provide the bounded operand and shift count. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_const_name_rejected` |
| another state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | A transition forwards the bounded operands to a successor state. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_other_state_rejected` |
| another machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | A forwarding machine transports operands to the shift machine. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_other_machine_rejected` |
| another target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | Each hosted target body executes the bounded shift and exact subtraction. | `tests/omega/pass/facts/unsigned_shift_right_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_shift_right_bound_through_target_body_rejected` |

Each positive place and root `main.omg` binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`, and passes `omega run --both`.

## Execution

```sh
omega run --both samples/cli/facts/unsigned_shift_right_bound/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_shift_right_bound/target_body/main.omg
```

Measured on macOS arm64 with the factsr1 shared generation producer `gen-f1b0d3326d2` (omega sha256 `4e3950e53eb15fbb3b54868f9c8964e23e7efc5bdced336e07f18eb27e28ac4a`): all ten positive places report `native exit 0` and `interp exit: 0` (20/20 engine legs, exit 0).

### The oracle discriminates

Perturbing the root oracle's single comparison (`transition res == 0` -> `transition res == 1`) sends both engines to `state bad() { crash Trap; }` instead of `done()`: the perturbed program exits 255 reporting `native signal 5` and `interp crash: Trap (host presentation agrees)`. The authored exit `0` therefore strictly discriminates between correct evaluation and faulty execution.

### Reject twins

Each twin changes only the construct's own value or bound (`400` -> `399`, or its place's own carrier), so `399 >> 2 == 99` underflows `shifted - 100`. Checking fails closed at compile time; the runner-emitted diagnostic per place is:

- `local`: `exact arithmetic in machine \`shift_right_local\` state \`entry\` terminal expression may overflow \`u64\``
- `field`: `exact arithmetic in machine \`shift_right_field\` state \`entry\` terminal expression may overflow \`u64\``
- `parameter`: `exact arithmetic in machine \`shift_right_param\` state \`entry\` terminal expression may overflow \`u64\``
- `call_result`: `exact arithmetic in machine \`shift_right_call_result\` state \`entry\` terminal expression may overflow \`u64\``
- `match_join`: `exact arithmetic in machine \`shift_right_match_join\` state \`entry\` terminal expression may overflow \`u64\``
- `const_name`: `exact arithmetic in machine \`shift_right_const\` state \`entry\` terminal expression may overflow \`u64\``
- `other_state`: `exact arithmetic in machine \`main\` state \`compute\` local \`res\` may overflow \`u64\``
- `other_machine`: `exact arithmetic in machine \`perform_shift\` state \`entry\` terminal expression may overflow \`u64\``
- `target_body`: `exact arithmetic in machine \`shift_right_target\` state \`entry\` terminal expression may overflow \`u64\`` (four diagnostics, one per target body)

```sh
omega --check samples/cli/facts/unsigned_shift_right_bound/<place>/invalid/main.omg
```

All nine twins exit 1 at checking with their authored `exact arithmetic` fragment satisfied; none reaches native execution.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_shift_right_bound_through_<place>_exit` and `tests/omega/fail/facts/unsigned_shift_right_bound_through_<place>_rejected` for the nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its place's `main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy of its place twin's `main.omg`/`build.omg`/`expected.txt` (fragment `exact arithmetic`). All 108 copied files compare identical to their sample sources.

Golden rows were recorded on macOS arm64 at `jobs=3` with the same generation producer (`corpus_runner` sha256 `c25372101a8cd22676fc23b3b80b2387a58609a988353552f158251c65650b96`), stamped `@5648483032b7+wt`:

- `tests/omega/corpus_outcomes.txt`: 18 `checked` rows carrying `check-target:macos_arm64` and 18 `rejected` rows carrying their diagnostics with the authored fragment satisfied (`+60` lines with the 24 diagnostic continuation lines, `-0`);
- `tests/omega/corpus_native_macos_arm64.txt`: 18 `built` rows with `exit:0` (`+18`/`-0`);
- `tests/omega/corpus_both_macos_arm64.txt`: 18 `built` rows with `exit:0 interpreter-exit:0 interpreter:agrees` (`+18`/`-0`).

## Requirements and Evidence

Owning requirement: `wiki/spec/language/numeric_values.md#shift-counts`
Parent task: **ONE-INTEGER-BOUNDS-ENGINE** ([TASKS.md](../../../../TASKS.md)) — this row is one client group's nine-place transport and execution of the shared carrier-aware bitwise provider's selected exact right shift (`arithmetic/runtime_immutable_unsigned_bitwise_bounds_exit` constrains full-width shift bounds independently of formation and false bounds).

- **Establishment:** Bounded operand facts (`value >= 400 && value <= 400`) and shift count bounds (`shift >= 2 && shift <= 2`) establish the exact post-shift interval (`shifted >= 100 && shifted <= 100`) directly upon evaluating `value >> shift`, without requiring any post-hoc caller equality guard.
- **Identity and lifetime:** The bounded unsigned scalar values retain their exact interval facts across local bindings, struct fields, machine parameters, return values, and match arms throughout their lifetimes.
- **Composition:** Unsigned shift right interval facts compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`shifted - 100u64`); `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the value bound to `399..399`, leaving exact subtraction unproved without any caller guard difference.
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time (`exact arithmetic in machine ... may overflow \`u64\``), strictly preventing underflow in exact subtraction when the shifted interval falls below the subtrahend. Both engines agree on the perturbed oracle (`native signal 5` / `interp crash: Trap`).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact unsigned logical right shift operations (`ExactIntegerShiftRight`) and arithmetic operations lower into canonical target-neutral `PSITERM\0` with erased proof-only obligations, executing with identical semantics in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), 64-bit integer carriers and struct layouts evaluate and validate, and `CallPlan`/`StatePlan` lower `u64` values across target registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target logical shift right instructions (`shr` on x86_64, `lsr` on arm64) and exact integer subtraction, assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Shift and subtraction instructions encode into target machine code, basic blocks and branch offsets relax with offset reflow, machine-state footprints validate, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

Remaining: the native and combined golden rows are macOS arm64 legs; `corpus_native_linux_x86_64.txt`, `corpus_native_windows_x86_64.txt` and `corpus_both_linux_x86_64.txt` carry no rows for these fixtures yet, so Linux and Windows host agreement is established by the four-target `ProgramEntry` bindings and the shared Terminal Psi product, not by a replayed run on those hosts.