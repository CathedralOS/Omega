# Scalar Sign and Zero Extension

Sign-extending `-28i8` to `i64` and adding zero-extended `128u8` as `i64` yields `100u64` when subtracting `100u64` across all nine places.

Every place performs the same operation: sign-extend `-28i8` to `i64` through `((-28i8 as i16) as i32) as i64` (verifying hardware sign extension so `-28` remains `-28i64`), zero-extend `128u8` through `(((128u8 as u16) as u32) as u64) as i64` (verifying high-bit zero extension so `128` remains `128i64`), sum them (`-28i64 + 128i64 == 100i64`), convert `sum as u64` (`100u64`), transport the resulting scalar through the place, perform exact subtraction `extended - 100u64`, and exit 0. Only the transport mechanism of the resulting scalar changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root scalar sign and zero extension |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both sign- and zero-extended scalars are combined and transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | struct field `extended: u64` carries the extended sum |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine parameter `param: u64` transports the extended sum |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | return result of callee `produce()` supplies the extended sum |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms yield the extended sum into the joined local binding |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant `SUBTRAHEND: u64 = 100` subtracts from the extended sum |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state `compute(extended: u64)` receives the value and subtracts |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the value through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies computes and subtracts the extended sum |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/scalar_sign_zero_extension/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/local/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/field/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/parameter/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/call_result/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/match_join/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/const_name/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/other_state/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/other_machine/main.omg
omega run --both samples/cli/facts/scalar_sign_zero_extension/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The initial signed value is `-28i8` and unsigned value is `128u8`, producing `-28i64 + 128i64 == 100i64` (`100u64`). Subtracting `100u64` yields `0`. If the extended value is corrupted at runtime or inverting the transition guard, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- Each invalid twin replaces `-28i8` with `-29i8` (so `-29i64 + 128i64 == 99i64`). Exact subtraction `extended - 100u64` (`99u64 - 100u64`) underflows below zero, causing compile-time proof obligation checking to fail closed with `exact arithmetic in machine \`evaluate\` ... may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/scalar_sign_zero_extension/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/scalar_sign_zero_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/scalar_sign_zero_extension_through_*_exit` and
`tests/omega/fail/facts/scalar_sign_zero_extension_through_*_rejected` fixtures.
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `b9274c262b8d47488edd4d70db6b82b2`.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for narrow-to-wide sign-extension and zero-extension scalar casting. Specifically, chaining narrow signed extensions (`i8 -> i16 -> i32 -> i64`) and narrow unsigned extensions (`u8 -> u16 -> u32 -> u64`) preserves exact arithmetic values (`-28i64 + 128i64 == 100i64`, `100u64`) across value transport in all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Narrow-to-wide sign extension (`-28i8 as i16 as i32 as i64`) and zero extension (`128u8 as u16 as u32 as u64 as i64`) preserve the exact narrow domain bounds and signedness intervals directly (`IntegerCastBound`) without requiring any post-hoc caller equality guard (`transition x == 100`) to justify forming the exact subtraction `extended - 100u64`.
  - **Identity and lifetime:** Widened scalar bindings retain their exact extended value (`100u64`) and static interval across moves, copies, and all 9 fact places without precision loss, sign corruption, or bit truncation.
  - **Composition:** The sign/zero-extended scalar bound composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction (`extended - 100u64`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the narrow input value (`-28i8` vs `-29i8`), while preserving all casting sequences, types, and caller structures.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when the narrow signed operand is changed from `-28i8` to `-29i8` (summing to `99u64`): exact subtraction `extended - 100u64` underflows below zero and strictly rejects with `exact arithmetic in machine \`evaluate\` ... may overflow \`u64\``. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Explicit narrow-to-wide sign and zero conversions (`CastSignExtend`, `CastZeroExtend`) and integer certificates lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving the concrete conversion operations. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The narrow and wide scalar layout plans (`IntegerAt`: `i8`/`u8` size 1, `i16`/`u16` size 2, `i32`/`u32` size 4, `i64`/`u64` size 8) validate, and target `CallPlan`/`StatePlan` assign scalar arguments and return homes to target register lanes and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA sign-extension instructions (`movsx`/`movsxd` on `x86_64`, `sxtb`/`sxth`/`sxtw` on `arm64`) and zero-extension instructions (`movzx`/32-bit register write zeroing on `x86_64`, `uxtb`/`uxth` on `arm64`), assigns register homes and disjoint activation stack-frame slots, and inserts entry prologues and multi-return epilogues.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA sign- and zero-extension forms encode into machine bytes (REX-prefixed `movsx`/`movsxd` on `x86_64`), basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic cross-carrier bitcasts, theorem-level signed-to-unsigned conversion preservation under general arithmetic constraints, and SIMD packed sign/zero extension vectors.
