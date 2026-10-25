# Narrow-to-Wide Sign Extension

Narrow-to-wide sign-extending `-25i8`, `-35i16`, and `160i32` to `i64` yields `100u64` when subtracting `100u64` across all nine places.

Every place performs the same operation: sign-extend narrow signed scalars `-25i8 as i64` (verifying byte sign extension preserving bit 7 to bit 63), `-35i16 as i64` (verifying word sign extension preserving bit 15 to bit 63), and `160i32 as i64` (verifying dword sign extension preserving positive bit 31), sum them (`(-25i64) + (-35i64) + 160i64 == 100i64`), convert `sum as u64` (`100u64`), transport the resulting scalar through the place, perform exact subtraction `extended - 100u64`, and exit 0. Only the transport mechanism of the resulting scalar changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root narrow-to-wide sign extension |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the narrow-to-wide sign-extended signed scalars and their sum are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | struct field `extended: u64` carries the sign-extended sum |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine parameter `param: u64` transports the sign-extended sum |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | return result of callee `produce()` supplies the sign-extended sum |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms yield the sign-extended sum into the joined local binding |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant `SUBTRAHEND: u64 = 100` subtracts from the sign-extended sum |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `compute(extended: u64)` receives the value and subtracts |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the value through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies computes and subtracts the sign-extended sum |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_sign_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The narrow signed values `-25i8`, `-35i16`, and `160i32` sign-extend to `i64`, producing `(-25i64) + (-35i64) + 160i64 == 100i64` (`100u64`). Subtracting `100u64` yields `0`. If any sign extension fails (e.g., zero extension instead of sign extension, or wrong sign bit propagation) or if the transition guard is inverted, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- Each invalid twin replaces `-25i8` with `-26i8` (so `(-26i64) + (-35i64) + 160i64 == 99i64`). Exact subtraction `extended - 100u64` (`99u64 - 100u64`) underflows below zero, causing compile-time proof obligation checking to fail closed with `exact arithmetic in machine \`evaluate\` ... may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/narrow_to_wide_sign_extension/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/narrow_to_wide_sign_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

The row is pinned by 18 fixtures: `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_<place>_exit`
and `tests/omega/fail/facts/narrow_to_wide_sign_extension_through_<place>_rejected` for each of the
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

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for narrow-to-wide sign-extension scalar casting. Specifically, widening narrow signed integers (`i8 -> i64`, `i16 -> i64`, `i32 -> i64`) preserves exact signed arithmetic values (`(-25i64) + (-35i64) + 160i64 == 100i64`, `100u64`) across value transport in all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Narrow-to-wide sign extension (`-25i8 as i64`, `-35i16 as i64`, `160i32 as i64`) directly preserves narrow signed intervals and sign-bit semantics (`IntegerCastBound`) without requiring any post-hoc caller equality guard (`transition x == 100`) to justify forming the exact subtraction `extended - 100u64`.
  - **Identity and lifetime:** Widened scalar bindings retain their exact extended value (`100u64`) and static interval across moves, copies, struct fields, and all 9 fact places without precision loss, high-bit corruption, or bit truncation.
  - **Composition:** The sign-extended scalar bounds compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction (`extended - 100u64`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the narrow signed input operand (`-25i8` vs `-26i8`), while preserving all casting sequences, types, and caller structures.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when the narrow signed operand is changed from `-25i8` to `-26i8` (summing to `99u64`): exact subtraction `extended - 100u64` underflows below zero and strictly rejects with `exact arithmetic in machine \`evaluate\` ... may overflow \`u64\``. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Explicit narrow-to-wide sign conversions (`CastSignExtend`) and integer arithmetic modes lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving the concrete conversion operations. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The narrow and wide scalar layout plans (`IntegerAt`: `i8` size 1, `i16` size 2, `i32` size 4, `i64`/`u64` size 8) validate, and target `CallPlan`/`StatePlan` assign scalar arguments and return homes to target register lanes and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA sign-extension instructions (`movsx`/`movsxd` on `x86_64` for byte, word, and dword sources; `sxtb`/`sxth`/`sxtw` on `arm64`), assigns register homes and disjoint activation stack-frame slots, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA sign-extension forms encode into machine bytes (REX-prefixed `movsx`/`movsxd` on `x86_64`), basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic cross-carrier bitcasts, negative floating-point sign extension, and SIMD packed sign-extension vector lanes.
