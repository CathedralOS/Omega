# Fixed Array Layout Geometry

Fact: querying size_of and align_of on a four-element u64 fixed-size array ([u64; 4]) establishes its exact layout geometry (size == 32u64 and align == 8u64), discharging exact subtraction size - (align * 4u64) across all nine places.

Every place performs the same operation: query the byte size (`size_of<[u64; 4]>() == 32u64`) and alignment (`align_of<[u64; 4]>() == 8u64`) of a four-element `u64` fixed-size array (`[u64; 4]`), discharge exact subtraction `size - (align * 4u64)`, verify `diff == 0`, and exit 0. Only the transport mechanism of the geometry queries and subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root fixed array size and alignment queries and exact subtraction |
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
`Expected exit: 0` with `expected_native_exit.txt` (`0`).

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/fixed_array_layout_geometry/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/local/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/field/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/parameter/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/call_result/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/match_join/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/const_name/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/other_state/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/other_machine/main.omg
omega run --both samples/cli/facts/fixed_array_layout_geometry/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The recorded 0 exit is strictly discriminative:
- At runtime, `size_of<[u64; 4]>() == 32u64` and `align_of<[u64; 4]>() == 8u64`. Exact subtraction `32u64 - (8u64 * 4u64) == 0u64`. The transition `diff == 0 { true -> done() _ -> bad() }` exits 0.
- A 1-token perturbation of the demanded value (changing `transition diff == 0` to `transition diff == 1` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines with `native exit -1073741795` (Windows trap / `native signal 5` on Unix) and `interp crash: Trap (host presentation agrees)`. Restoring `transition diff == 0` restores `native exit 0` and `interp exit: 0`.

### Reject Twins

In each invalid twin (`<place>/invalid/main.omg`), only the construct's own domain/input is perturbed: the queried array type is narrowed from four elements (`[u64; 4]`) to three elements (`[u64; 3]`, where `size == 24u64` and `align == 8u64`). Exact subtraction `24u64 - (8u64 * 4u64)` (`24u64 - 32u64`) underflows `u64` (`bounds [-8, -8] lie outside [0, 18446744073709551615]`), causing compile-time proof obligation checking to fail closed with `exact arithmetic may overflow`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/fixed_array_layout_geometry/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic range (`u64` underflow `[-8, -8]`) | `exact arithmetic may overflow` / `bounds [-8, -8] lie outside [0, 18446744073709551615]` |

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_local_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_field_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_parameter_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_call_result_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_match_join_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_const_name_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_other_state_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_other_machine_exit/`
  - `tests/omega/pass/facts/fixed_array_layout_geometry_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_local_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_field_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_parameter_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_call_result_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_match_join_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_const_name_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_other_state_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_other_machine_rejected/`
  - `tests/omega/fail/facts/fixed_array_layout_geometry_through_target_body_rejected/`

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\cargo-target\debug\omega-latest.exe --filter cli/facts/fixed_array_layout_geometry --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`exact arithmetic may overflow`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `f781103c8c7c4f75bc10d3b6c232a652` (prior witness ticket `98dfa105c31742ebbc2a93de4f51bc72`).
- **Requirement established:** Target-neutral compile-time layout geometry query proofs (`size_of` and `align_of`) and native realization for fixed-size arrays (`wiki/spec/layouts/plans.md` & `wiki/spec/language/counts_and_addresses.md`). Specifically, querying `size_of` and `align_of` on a four-element `u64` fixed-size array (`[u64; 4]`) establishes its exact layout geometry (`size == 32u64` and `align == 8u64`), discharging exact arithmetic `size - (align * 4u64) == 0u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time layout evaluation of the fixed-size array type `[u64; 4]` statically determines `size_of<[u64; 4]>() == 32u64` and `align_of<[u64; 4]>() == 8u64` from array extent (4 elements) and element scalar layout (64-bit width, 8-byte alignment, contiguous layout without padding). The static bounds `[32, 32]` and `[8, 8]` justify forming exact subtraction `size - (align * 4u64)` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The static layout geometry values (`32u64` and `8u64`) retain their exact scalar identity, precision, and alignment constraints across copies, structural carrier fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The layout geometry queries compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`size - (align * 4u64)`), evaluating to `0u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the queried array extent (`[u64; 4]` with size 32 vs `[u64; 3]` with size 24).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when the array type is narrowed to three elements `[u64; 3]`: querying `size_of<[u64; 3]>()` yields `24u64`, while `align_of<[u64; 3]>()` remains `8u64`, so exact subtraction `24u64 - (8u64 * 4u64)` (`24u64 - 32u64`) underflows `u64` (`bounds [-8, -8] lie outside [0, 18446744073709551615]`). Compilation strictly rejects with `exact arithmetic may overflow`. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral array layout queries `size_of<[u64; 4]>()` and `align_of<[u64; 4]>()` and exact integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining concrete layout plans and certified geometry constants. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The fixed-size array layout validates: `[u64; 4]` contains contiguous 8-byte scalar elements (element size 8, align 8), totaling size 32 with alignment 8. Target `CallPlan`/`StatePlan` assign arguments and return homes across register lanes, stack slots, or indirect aggregate return homes (`sret`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA arithmetic instructions (`sub`, `shl`/`imul` or constant-folded subtraction of 32), register moves, and comparison branches. Low-level register allocators assign register homes and disjoint activation stack-frame slots, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic variable-length slice layout queries (`size_of_val`), multidimensional array stride layout plans, and packed/zero-sized array geometry.
