# Narrow to Wide Signed 16-to-32 Extension

Fact: sign-extending a bounded 16-bit signed i16 value (narrow >= 300i16 && narrow <= 500i16) to i32 (narrow as i32) preserves the exact interval (wide >= 300i32 && wide <= 500i32), discharging exact 32-bit signed subtractions 500i32 - wide and wide - 300i32 across all nine places.

Owning Task: `NARROW-WIDE-SIGNED-16TO32`
Specification Reference: [Numeric values](wiki/spec/language/numeric_values.md), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Instruction selection](wiki/spec/resources/storage.md)

This fact row demonstrates that sign-extending a bounded 16-bit signed `i16` value (`narrow >= 300i16 && narrow <= 500i16`) to `i32` (`narrow as i32`) preserves the exact interval (`wide >= 300i32 && wide <= 500i32`) across all nine standard places. The compiler statically derives that sign-extension preserves interval bounds on signed types, verifying that `wide` falls strictly within `[300, 500]`, which directly discharges exact downstream 32-bit signed subtractions `500i32 - wide` and `wide - 300i32` and downstream contract `requires wide >= 300i32 && wide <= 500i32` of `verify_wide` without underflow, overflow, or runtime guard checks across both Terminal Psi and native targets.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness sign-extending bounded i16 local into i32 and discharging exact 32-bit subtractions | [`main.omg`](main.omg) | - |
| local | `local/` | the bounded i16 is sign-extended to an i32 local variable before discharging subtraction | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the bounded i16 is wrapped in a NarrowCarrier record field before extraction and extension to i32 | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper machine receives the sign-extended i32 scalar as an explicit parameter under contract | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the sign-extended i32 scalar under postcondition ensures result >= 300i32 && result <= 500i32 | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the sign-extended i32 scalar before subtraction | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant BASE_NARROW defines the bounded i16 operand before sign-extension | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the sign-extended i32 scalar to a successor machine with entry contract | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine sign-extends and forwards the i32 scalar to the caller under postcondition | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies sign-extends the i16 scalar and discharges subtraction | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/local/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/field/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/parameter/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/call_result/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/match_join/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/const_name/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/other_state/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/other_machine/main.omg
omega run --both samples/cli/facts/narrow_to_wide_signed_16to32_extension/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, sign-extending `narrow = 400i16` (`300i16 <= narrow <= 500i16`) to `i32` yields `wide = 400i32` within static interval `300i32..500i32`. Passing `wide` to `verify_wide(wide)` satisfies its contract `wide >= 300i32 && wide <= 500i32`, evaluates exact subtractions `500i32 - 400i32 == 100i32` and `400i32 - 300i32 == 100i32`, and returns `0i32`. In `main`, `transition exit_code == 0i32` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition exit_code == 0i32` to `transition exit_code == 1i32`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the narrow bound is widened to `501i16` (`501i32`), failing closed at compile time under the **Contract precondition** obligation class because `wide <= 500i32` cannot be established for `verify_wide`. Check each twin with:

```sh
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/local/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/field/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/parameter/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/call_result/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/match_join/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/const_name/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/other_state/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/other_machine/invalid/main.omg
omega --check samples/cli/facts/narrow_to_wide_signed_16to32_extension/target_body/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_wide` |

Every reject twin fails closed with:
`error: cannot prove requires contract for call verify_wide from ...: wide >= 300 && wide <= 500 (needs fact `wide >= 300 && wide <= 500` here; establish it before the call via a prior contract guarantee, domain membership, or guard)`

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/narrow_to_wide_signed_16to32_extension --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_wide`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_local_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_field_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_parameter_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_call_result_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_match_join_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_const_name_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_other_state_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_other_machine_exit/`
  - `tests/omega/pass/facts/narrow_to_wide_signed_16to32_extension_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_local_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_field_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_parameter_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_call_result_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_match_join_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_const_name_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_other_state_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_other_machine_rejected/`
  - `tests/omega/fail/facts/narrow_to_wide_signed_16to32_extension_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `NARROW-WIDE-SIGNED-16TO32` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for narrow-to-wide signed 16-bit to 32-bit sign extension (`narrow as i32`) preserving the exact interval (`wide >= 300i32 && wide <= 500i32`), discharging exact 32-bit signed subtractions `500i32 - wide` and `wide - 300i32` and downstream contract `requires wide >= 300i32 && wide <= 500i32` across all nine language transport places with differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Numeric values](wiki/spec/language/numeric_values.md#narrow-to-wide-signzero-extension), [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md), and [Control flow](wiki/spec/terminal-psi/control_flow.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Sign-extending bounded 16-bit signed integer `narrow in [300, 500]` to `i32` via `narrow as i32` provably preserves the exact signed interval `[300, 500]` on the 32-bit destination. Exact subtractions `500i32 - wide` and `wide - 300i32` evaluate without underflow or overflow. The Omega arithmetic solver derives this exact range certificate directly from the sign-extension operation, verifying that `wide` satisfies `verify_wide(wide)` requires contract `wide >= 300i32 && wide <= 500i32` without requiring manual runtime assertions or post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The sign-extended scalar transitions from a 2-byte stored width representation (`i16`) to a 4-byte stored width representation (`i32`) with sign-extended high bits, maintaining 32-bit signed integer value identity across expression evaluations, local variable assignments, parameter passing, and state transitions without loss of sign or magnitude.
- **Composition:** Signed extension bound preservation composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value postcondition (`call_result`), match branch convergence join (`match_join`), named constant operand binding (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in widening the upper bound to `501i16` (`501i32`), causing `verify_wide` to fail closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call verify_wide` because `501i32` violates the contract upper bound `wide <= 500i32`. The compiler soundly refuses any program where sign-extended bounds cannot guarantee arrival contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed extension cast expressions (`as i32`) and integer range certificates lower cleanly into canonical Terminal Psi (`PSITERM\0`). The narrow range `[300, 500]` is preserved in the Terminal Psi integer certificate through intermediate optimization passes, and the Terminal Psi interpreter agrees with native execution on exit code 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<i16>() == 2`, `align_of<i16>() == 2`, `size_of<i32>() == 4`, `align_of<i32>() == 4`, and `size_of<NarrowCarrier>() == 2`. In `CallPlan` and `StatePlan`, 16-bit values pass in 16-bit register/stack lanes and widen to 32-bit general-purpose registers (`ecx`/`edi`/`w0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects the destination-width-independent `SignExtendI16`, which writes the full 64-bit normalized carrier: `movsx r64, r/m16` (REX.W; build bytes `48 0F BF`; AT&T `movswq`) on x86_64 and `sxth xN, wN` (word `0x9340_3c00`) on arm64. The destination logical width does not change the selected form; the extension is keyed on the source width, so this 16→32 row selects exactly the form the 16→64 row selects, and `i32`-typed boundaries add only an ordinary `sxtw` carrier normalization. Exact subtraction runs on the same 64-bit carrier (`subq` / `sub xN`), not a 32-bit `sub`. Disjoint activation frames allocate standard stack slots, honoring worst-case stack usage (WCSU) and bounded activation claims (`committed <= bound`). Measured at the tip on the macos_arm64 build with `otool -tvV <build-dir>/omega-program` and on the linux_x86_64 build with `objdump -d <build-dir>/omega-program`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute with exit code 0 across `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Signed 32-to-64 sign extension (`i32 as i64`), signed 8-to-16 extension (`i8 as i16`), narrow-to-wide floating point conversions, and SIMD vector widening remain governed under the wider numeric value roadmap.

## Queue classification

This entry is a redundant count variant, not a new mechanism. Narrow-to-wide
extension is selected by the **source** width and signedness
(`SignExtendI8`/`SignExtendI16`/`SignExtendI32` and the matching zero-extend
kinds), and its realized form is destination-width-independent: the 16→32 and
16→64 rows both select `SignExtendI16` and emit the same instruction. The landed
`narrow_to_wide_signed_16to64_extension` row already exercises `SignExtendI16`
across the nine places, so recording this row would add golden churn without
new coverage. It was declined on 2026-10-20 after measuring both rows; the full
evidence is in `build/swarm/w33/slot-8-narrow16to32-verdict.txt`.

Genuinely new signed entries by the same rule are the source widths whose
selected instruction differs and is not yet covered: the `i8` source
(`SignExtendI8`) and the `i32` source (`SignExtendI32`). Additional destination
widths for an already-covered source width are count variants.
