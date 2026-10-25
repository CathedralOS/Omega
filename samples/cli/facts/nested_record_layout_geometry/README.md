# Nested Record Layout Geometry

Fact: querying size_of and align_of on a nested structural record (Outer { inner: Inner, tag: u64 } with Inner { a: u64, b: u64 }) establishes its exact 24-byte composite layout geometry (size == 24u64 and align == 8u64), discharging exact subtraction size - (align * 3u64) across all nine places.

Every place performs the same operation: query the composite byte size (`size_of<Outer>() == 24u64`) and alignment (`align_of<Outer>() == 8u64`) of a nested structural record (`Outer { inner: Inner, tag: u64 }` with `Inner { a: u64, b: u64 }`), discharge exact subtraction `size - (align * 3u64)`, verify `diff == 0`, and exit 0. Only the transport mechanism of the geometry queries and subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root nested record size and alignment queries and exact subtraction |
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
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/nested_record_layout_geometry/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/local/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/field/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/parameter/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/call_result/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/match_join/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/const_name/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/other_state/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/other_machine/main.omg
omega run --both samples/cli/facts/nested_record_layout_geometry/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `size_of<Outer>() == 24u64` and `align_of<Outer>() == 8u64`. Exact subtraction `24u64 - (8u64 * 3u64) == 0u64`. The transition `diff == 0 { true -> done() _ -> bad() }` routes to `machine done() {}`, exiting 0. A 1-token perturbation of the demanded result (`transition diff == 0` to `transition diff == 1` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `Inner` is narrowed to a single `u64` field so `Outer` has `size == 16u64` and `align == 8u64`. Exact subtraction `16u64 - (8u64 * 3u64)` (`16u64 - 24u64`) underflows below zero (`bounds [-8, -8] lie outside [0, 18446744073709551615]`), causing compile-time proof obligation checking to fail closed under the **Exact arithmetic overflow** obligation class with `exact arithmetic may overflow`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/nested_record_layout_geometry/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic overflow | `exact arithmetic may overflow` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/nested_record_layout_geometry --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`exact arithmetic may overflow`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_local_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_field_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_parameter_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_call_result_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_match_join_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_const_name_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_other_state_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_other_machine_exit/`
  - `tests/omega/pass/facts/nested_record_layout_geometry_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_local_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_field_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_parameter_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_call_result_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_match_join_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_const_name_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_other_state_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_other_machine_rejected/`
  - `tests/omega/fail/facts/nested_record_layout_geometry_through_target_body_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `7b7b9ea775954d3cb5ba89d0aba3247d`.
- **Requirement established:** Target-neutral compile-time layout geometry query proofs (`size_of` and `align_of`, [layout plans](../../../../wiki/spec/layouts/plans.md)) and native realization for nested structural records. Specifically, querying `size_of` and `align_of` on a nested structural record (`Outer { inner: Inner, tag: u64 }` with `Inner { a: u64, b: u64 }`) establishes its exact 24-byte composite layout geometry (`size == 24u64` and `align == 8u64`), discharging exact arithmetic `size - (align * 3u64) == 0u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time layout evaluation of the nested structural record `Outer { inner: Inner, tag: u64 }` statically determines `size_of<Outer>() == 24u64` and `align_of<Outer>() == 8u64` from composite field rules (`inner: Inner` occupying offsets [0..16] and `tag: u64` occupying offsets [16..24], total size 24, align 8). The static bounds `[24, 24]` and `[8, 8]` justify forming exact subtraction `size - (align * 3u64)` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The static layout geometry values (24u64 and 8u64) retain their exact scalar identity, precision, and alignment constraints across copies, structural carrier fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The layout geometry queries compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`size - (align * 3u64)`), evaluating to `0u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the inner record definition (`Inner { a: u64, b: u64 }` with composite size 24 vs `Inner { a: u64 }` with composite size 16).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `Inner` is narrowed to a single `u64` field: querying `size_of<Outer>()` yields 16u64, while `align_of<Outer>()` remains 8u64, so exact subtraction `16u64 - (8u64 * 3u64)` (`16u64 - 24u64`) underflows `u64` (`bounds [-8, -8] lie outside [0, 18446744073709551615]`). Compilation strictly rejects under the **Exact arithmetic overflow** obligation class with `exact arithmetic may overflow`. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral nested record layout queries `size_of<Outer>()` and `align_of<Outer>()` and exact integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining concrete layout plans and certified geometry constants. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The composite structural record layout validates: `Inner` contains `a: u64` (offset 0) and `b: u64` (offset 8), and `Outer` embeds `inner: Inner` (offset 0, size 16, align 8) followed by `tag: u64` (offset 16, size 8, align 8), totaling size 24 with alignment 8. Target `CallPlan`/`StatePlan` assign arguments and return homes across register lanes, stack slots, or indirect aggregate return homes (`sret`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA arithmetic instructions (`sub`, `shl`/`imul` or constant-folded subtraction of 24), register moves, and comparison branches. Low-level register allocators assign register homes and disjoint activation stack-frame slots, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic variable-length nested DST structures, alignment padding between heterogeneous inner fields, and recursive record layout references.
