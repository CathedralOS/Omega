# Unsigned shift right bound

Logical right shift on a bounded u64 value by a bounded shift count establishes the exact post-shift interval, discharging exact subtraction shifted - 100u64.

Every place performs the same operation: logically shift a bounded `u64` value (`val >= 400u64 && val <= 400u64`) right by a bounded shift count (`shift >= 2u64 && shift <= 2u64`), establishing the exact post-shift value `100u64` (`shifted >= 100u64 && shifted <= 100u64`), which discharges exact subtraction `shifted - 100u64` to exit code 0. Only the transport of the operands changes.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root (anchor) | [source](main.omg) | - | Root reference row performing bounded shift right and exact subtraction. |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | Both the bounded u64 value and bounded shift count are copied into locals. |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | Bounded record fields carry the u64 value and shift count into the shift. |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | The shift machine reads its parameters directly. |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | Calls with declared bounds supply both operands to the shift. |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | Both arms produce operands within bounds so the joined operands shift safely. |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | Named constants provide the bounded operand and shift count. |
| other state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | A transition forwards the bounded operands to a successor state. |
| other machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | A forwarding machine transports operands to the shift machine. |
| target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | Each hosted target body executes the bounded shift and exact subtraction. |

Each positive place and root `main.omg` binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`, and passes `omega run --both`.

Each `invalid/` twin weakens the operand bound to `399..399`, so the logical right shift `399 >> 2 == 99` underflows `shifted - 100` and fails compile-time check with `exact arithmetic in machine ... may overflow `u64``.

## Requirements and Evidence

Owning requirement: `wiki/spec/language/numeric_values.md#shift-counts`
Parent task: **CONSERVATION-CONTRACT** ([TASKS.md](../../../../TASKS.md))

- **Establishment:** Bounded operand facts (`value >= 400 && value <= 400`) and shift count bounds (`shift >= 2 && shift <= 2`) establish the exact post-shift interval (`shifted >= 100 && shifted <= 100`) directly upon evaluating `value >> shift`, without requiring any post-hoc caller equality guard.
- **Identity and lifetime:** The bounded unsigned scalar values retain their exact interval facts across local bindings, struct fields, machine parameters, return values, and match arms throughout their lifetimes.
- **Composition:** Unsigned shift right interval facts compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`shifted - 100u64`); `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the value bound to `399..399`, leaving exact subtraction unproved without any caller guard difference.
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time (`exact arithmetic in machine ... may overflow \`u64\``), strictly preventing underflow in exact subtraction when the shifted interval falls below the subtrahend.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact unsigned logical right shift operations (`ExactIntegerShiftRight`) and arithmetic operations lower into canonical target-neutral `PSITERM\0` with erased proof-only obligations, executing with identical semantics in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), 64-bit integer carriers and struct layouts evaluate and validate, and `CallPlan`/`StatePlan` lower `u64` values across target registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target logical shift right instructions (`shr` on x86_64, `lsr` on arm64) and exact integer subtraction, assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Shift and subtraction instructions encode into target machine code, basic blocks and branch offsets relax with offset reflow, machine-state footprints validate, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.
