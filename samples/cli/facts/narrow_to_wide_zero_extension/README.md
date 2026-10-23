# Narrow to Wide Zero Extension

Zero-extending narrow unsigned 100u8 through u16 and u32 to u64 preserves the exact interval 100u64, discharging exact subtraction widened - 100u64 with exit code 0.

Every place performs the same operation: zero-extend narrow unsigned integer `100u8` through `(((100u8 as u16) as u32) as u64)` (verifying hardware zero extension so `100` remains `100u64`), transport the resulting scalar through the place, perform exact subtraction `widened - 100u64`, and exit 0. Only the transport mechanism of the resulting scalar changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root narrow to wide zero extension |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | zero-extended scalar is transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | struct field `extended: u64` carries the zero-extended scalar |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine parameter `param: u64` transports the zero-extended scalar |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | return result of callee `produce()` supplies the zero-extended scalar |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms yield the zero-extended scalar into the joined local binding |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant `SUBTRAHEND: u64 = 100` subtracts from the zero-extended scalar |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state `compute(extended: u64)` receives the value and subtracts |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the value through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies computes and subtracts the zero-extended scalar |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_zero_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The initial unsigned value is `100u8`, zero-extended through `u16` and `u32` to `u64`, producing `100u64`. Exact subtraction `extended - 100u64` yields `0`. If the extended value is corrupted at runtime or inverting the transition guard, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- Each invalid twin replaces `100u8` with `99u8`, resulting in `99u64`. Exact subtraction `extended - 100u64` (`99u64 - 100u64`) underflows below zero, causing compile-time proof obligation checking to fail closed with `exact arithmetic in machine \`evaluate\` ... may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_zero_extension/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/narrow_to_wide_zero_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

The row is pinned by 18 fixtures: `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_<place>_exit`
and `tests/omega/fail/facts/narrow_to_wide_zero_extension_through_<place>_rejected` for each of the
nine places. Each positive fixture is its place program copied byte-identical, binds `ProgramEntry`
for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and
carries `expected_native_exit.txt` = 0; each reject twin is the place's invalid program copied
byte-identical with the build application renamed `_invalid` to `_rejected`, and `expected.txt`
carrying that place's own diagnostic fragment (`exact arithmetic in machine \`evaluate\` state
\`entry\` local \`sub\` may overflow \`u64\``; `compute` instead of `entry` in `other_state`).

Every row was recorded with the shared generation producer (gen-317bacd12b0, revision 317bacd12b0,
macOS arm64 host) on 2026-10-22, anchored at that base with the fixture and golden changes staged,
so each row is stamped `@317bacd12b0a+wt`:

- `tests/omega/corpus_outcomes.txt`: 18 `checked` rows carrying `check-target:macos_arm64` and 18
  `rejected` rows carrying each twin's own diagnostic line with its expected fragment satisfied;
- `tests/omega/corpus_native_macos_arm64.txt`: 9 `built` rows with `exit:0`;
- `tests/omega/corpus_both_macos_arm64.txt`: 9 `built` rows with `exit:0`, `interpreter-exit:0` and
  `interpreter:agrees`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for narrow-to-wide zero-extension scalar casting. Specifically, zero-extending a bounded narrow unsigned integer (`u8` / `u16` / `u32` `value >= 100 && value <= 100` widened to `u64`) preserves the exact interval (`widened >= 100u64 && widened <= 100u64`), discharging exact subtraction `widened - 100u64` with exit code `0` across value transport in all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Narrow-to-wide zero extension (`100u8 as u16 as u32 as u64`) preserves the exact narrow domain bounds and unsigned interval directly (`IntegerCastBound`) without requiring any post-hoc caller equality guard (`transition x == 100`) to justify forming the exact subtraction `extended - 100u64`.
  - **Identity and lifetime:** Widened scalar bindings retain their exact extended value (`100u64`) and static interval across moves, copies, and all 9 fact places without precision loss or high-order bit corruption.
  - **Composition:** The zero-extended scalar bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction (`extended - 100u64`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the narrow input value (`100u8` vs `99u8`), while preserving all casting sequences, types, and caller structures.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when the narrow unsigned operand is changed from `100u8` to `99u8`: exact subtraction `extended - 100u64` underflows below zero and strictly rejects with `exact arithmetic in machine \`evaluate\` ... may overflow \`u64\``. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Explicit narrow-to-wide zero conversions (`CastZeroExtend`) and integer certificates lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving the concrete conversion operations. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The narrow and wide scalar layout plans (`IntegerAt`: `u8` size 1, `u16` size 2, `u32` size 4, `u64` size 8) validate, and target `CallPlan`/`StatePlan` assign scalar arguments and return homes to target register lanes and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA zero-extension instructions (`movzx r32, ...` and 32-bit register-write zeroing on `x86_64`; the 64-bit UBFM zero-extend forms on `arm64`, which `otool` prints as `ubfx xN, xN, #0, #8|16|32` for the `uxtb`/`uxth`/`uxtw` aliases with an `X` destination), assigns register homes and disjoint activation stack-frame slots, and inserts entry prologues and multi-return epilogues.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA zero-extension forms encode into machine bytes (REX-prefixed `movzx` on `x86_64`), basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

## What remains open

- Dynamic cross-carrier bitcasts, theorem-level signed-to-unsigned conversion preservation under general arithmetic constraints, and SIMD packed zero extension vectors.
