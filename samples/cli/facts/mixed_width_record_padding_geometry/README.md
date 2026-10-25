# Mixed-Width Record Padding Geometry

Fact: querying size_of and align_of on a mixed-width structural record (Padded { tag: u8, value: u64 }) establishes its exact 16-byte alignment-padded layout geometry (size == 16u64 and align == 8u64), discharging exact subtraction size - (align * 2u64) across all nine places.

Every place performs the same operation: query the alignment-padded byte size (`size_of<Padded>() == 16u64`) and alignment (`align_of<Padded>() == 8u64`) of a mixed-width structural record (`Padded { tag: u8, value: u64 }`), discharge exact subtraction `size - (align * 2u64)`, verify `diff == 0`, and exit 0. Only the transport mechanism of the geometry queries and subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root mixed-width record size and alignment queries and exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the size and alignment queries are copied into locals before subtracting |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | an enclosing structural carrier record holds the size and alignment in fields before subtracting |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | helper machines receive the queried size and alignment directly as explicit parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the size and alignment are returned from contracted helper call results before subtracting |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms evaluate the size and alignment queries and join before subtracting |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants evaluate size_of and align_of before subtracting |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the size and alignment queries before subtracting |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine delegates the subtraction to the geometry query machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies evaluates size_of and align_of and subtracts |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/local/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/field/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/parameter/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/call_result/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/match_join/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/const_name/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/other_state/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/other_machine/main.omg
omega run --both samples/cli/facts/mixed_width_record_padding_geometry/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `size_of<Padded>() == 16u64` and `align_of<Padded>() == 8u64`. Exact subtraction `16u64 - (8u64 * 2u64) == 0u64`. The transition `diff == 0 { true -> done() _ -> bad() }` exits 0. If record padding layout geometry queries evaluate incorrectly or arithmetic fails, `diff` is non-zero and execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `Padded` omits `tag: u8` so `Padded` has `size == 8u64` and `align == 8u64`. Exact subtraction `8u64 - (8u64 * 2u64)` (`8u64 - 16u64`) underflows below zero (`bounds [-8, -8] lie outside [0, 18446744073709551615]`), causing compile-time proof obligation checking to fail closed with `exact arithmetic may overflow`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/mixed_width_record_padding_geometry/<place>/invalid/main.omg
```

### Pinned fixtures

The row is pinned under `tests/omega/pass/facts/mixed_width_record_padding_geometry_through_<place>_exit`
(nine positives with `expected_native_exit.txt`) and
`tests/omega/fail/facts/mixed_width_record_padding_geometry_unproved_<place>`
(nine reject twins with `expected.txt`), where `<place>` is `local`, `field`,
`parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, or `target_body`. Each pass/twin pair differs only in the
construct's own input: the twin's `Padded` omits `tag: u8`.

```sh
python3 tools/corpus_gate.py --native --filter facts/mixed_width_record_padding_geometry
python3 tools/corpus_gate.py --filter facts/mixed_width_record_padding_geometry
```

The native leg reports `clean` with 9 `built` rows at `exit:0`; the check leg
reports `clean` with 9 `checked` and 9 `rejected`, 0 rejected without their
expected fragment.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omegauild\omega-latest.exe --filter cli/facts/mixed_width_record_padding_geometry --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`exact arithmetic may overflow`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time layout geometry query proofs (`size_of` and `align_of`) and native realization for mixed-width structural records with alignment padding. Specifically, querying `size_of` and `align_of` on a mixed-width structural record (`Padded { tag: u8, value: u64 }`) establishes its exact 16-byte alignment-padded layout geometry (`size == 16u64` and `align == 8u64`), discharging exact arithmetic `size - (align * 2u64) == 0u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time layout evaluation of the mixed-width structural record `Padded { tag: u8, value: u64 }` statically determines `size_of<Padded>() == 16u64` and `align_of<Padded>() == 8u64` from structural alignment rules: field `tag: u8` occupies offset 0 with alignment 1, field `value: u64` requires alignment 8, inserting 7 bytes of internal alignment padding at offsets [1..8], and `value` occupies offsets [8..16]. The total size rounds up to a multiple of record alignment 8, yielding exact composite size 16 and align 8. The static bounds `[16, 16]` and `[8, 8]` justify forming exact subtraction `size - (align * 2u64)` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The static layout geometry values (16u64 and 8u64) retain their exact scalar identity, precision, and alignment constraints across copies, structural carrier fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The layout geometry queries compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`size - (align * 2u64)`), evaluating to `0u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the record definition (`Padded { tag: u8, value: u64 }` with alignment padding totaling 16 bytes vs `Padded { value: u64 }` without mixed-width padding totaling 8 bytes).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `tag: u8` is omitted: querying `size_of<Padded>()` yields 8u64, while `align_of<Padded>()` remains 8u64, so exact subtraction `8u64 - (8u64 * 2u64)` (`8u64 - 16u64`) underflows `u64` (`bounds [-8, -8] lie outside [0, 18446744073709551615]`). Compilation strictly rejects with `exact arithmetic may overflow`. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral record layout queries `size_of<Padded>()` and `align_of<Padded>()` and exact integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining concrete layout plans and certified geometry constants. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The mixed-width structural record layout validates: `tag: u8` (offset 0), internal padding (offsets [1..8]), and `value: u64` (offset 8), totaling size 16 with alignment 8. Target `CallPlan`/`StatePlan` assign arguments and return homes across register lanes, stack slots, or indirect aggregate return homes (`sret`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA arithmetic instructions (`sub`, `shl`/`imul` or constant-folded subtraction of 16), register moves, and comparison branches. Low-level register allocators assign register homes and disjoint activation stack-frame slots, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

### Measurement: interior padding, not a constant

`size_of`/`align_of` for `Padded { tag, value }` were read back on
`macos_arm64` through assertions carried into a `crashes Trap` transition; a
wrong assertion is a runtime trap, so each row is a measurement rather than a
restated constant.

| record | declared payload | size | align | interior padding |
| --- | --- | --- | --- | --- |
| `{tag: u8,  value: u8 }` | 2 | 2 | 1 | 0 |
| `{tag: u8,  value: u16}` | 3 | 4 | 2 | 1 |
| `{tag: u8,  value: u32}` | 5 | 8 | 4 | 3 |
| `{tag: u8,  value: u64}` | 9 | 16 | 8 | 7 |
| `{tag: u16, value: u32}` | 6 | 8 | 4 | 2 |
| `{tag: u16, value: u64}` | 10 | 16 | 8 | 6 |
| `{tag: u32, value: u64}` | 12 | 16 | 8 | 4 |
| `{tag: u64, value: u64}` | 16 | 16 | 8 | 0 |

The padding is non-monotonic in the declared field widths, which is what
separates interior padding from a size sum:

- Widening the narrow field `tag` from `u8` to `u16`, `u32`, then `u64` while
  `value: u64` is held fixed leaves `size` at exactly 16 and `align` at exactly
  8. The 7 bytes reserved by the `u64` field's alignment-1 absorb the widening
  at no size cost, so an 8x wider narrow field does not move the record.
- The naive sum `tag_width + value_width` is falsified:
  `{tag: u32, value: u64}` asserted at `size == 12` (4 + 8) traps.
- Narrowing the wide field `u64` to `u32` halves the size (16 to 8) while the
  padding only falls from 7 to 3, so `size` is not linear in declared widths.

### Falsification control

The recorded `exit 0` is measured, not assumed. Corrupting each record's
assertion by one byte in either direction traps both engines: for
`{tag: u8, value: u64}`, asserting `size == 15`, `17`, or `24` yields
`native signal 5` together with `interp crash: Trap`; only `size == 16`
exits 0. The same one-byte-off pattern traps for `{u8,u32}` (7 and 9),
`{u8,u16}` (3 and 5), `{u32,u64}` (15 and 17), and `{u16,u32}` (7 and 9).
Separately, corrupting the discharged oracle itself (`diff == 99` in place of
`diff == 0`) traps both engines, so the 0-exit is not a constant.

### Static refusal versus runtime trap

- **Static refusal (no execution).** Each reject twin omits `tag`, so
  `size_of<Padded>()` is 8 while `align_of<Padded>()` stays 8, and the exact
  subtraction `size - (align * 2u64)` underflows. Checking fails closed before
  native realization with
  `exact arithmetic may overflow under target <target>: bounds [-8, -8] lie outside [0, 18446744073709551615]`.
  A `{tag: u8}`-only record similarly refuses at `bounds [-1, -1]`.
- **Runtime trap.** Every wrong geometry assertion above compiles and reaches
  execution, then traps: `native signal 5` with `interp crash: Trap`. These are
  runtime disagreements, not static refusals.

- **Obligation class:** layout geometry (target resolution of `size_of`/`align_of`
  under validated structural padding) carried through checking, Terminal Psi
  interpretation, and native execution.

- **What remains open:** Dynamic variable-length nested DST structures, tail-padding in packed record layouts, and unaligned field access modes.
