# Size Align Layout Geometry

Fact: querying size_of and align_of on a two-field u64 structural record (Pair { lo: u64, hi: u64 }) establishes its exact layout geometry (size == 16u64 and align == 8u64), discharging exact subtraction size - (align * 2u64) across all nine places.

Every place performs the same operation: query the byte size (`size_of<Pair>() == 16u64`) and alignment (`align_of<Pair>() == 8u64`) of a two-field `u64` structural record (`Pair { lo: u64, hi: u64 }`), discharge exact subtraction `size - (align * 2u64)`, verify `diff == 0`, and exit 0. Only the transport mechanism of the geometry queries and subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root size and alignment queries and exact subtraction |
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
omega run --both samples/cli/facts/size_align_layout_geometry/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/local/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/field/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/parameter/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/call_result/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/match_join/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/const_name/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/other_state/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/other_machine/main.omg
omega run --both samples/cli/facts/size_align_layout_geometry/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `size_of<Pair>() == 16u64` and `align_of<Pair>() == 8u64`. Exact subtraction `16u64 - (8u64 * 2u64) == 0u64`. The transition `diff == 0 { true -> done() _ -> bad() }` exits 0. If layout geometry queries evaluate incorrectly or arithmetic fails, `diff` is non-zero and execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `Pair` is narrowed to a single `lo: u64` field (`size == 8u64`, `align == 8u64`). Exact subtraction `8u64 - (8u64 * 2u64)` (`8u64 - 16u64`) underflows below zero (`bounds [-8, -8] lie outside [0, 18446744073709551615]`), causing compile-time proof obligation checking to fail closed with `exact arithmetic may overflow`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/size_align_layout_geometry/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/size_align_layout_geometry --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`exact arithmetic may overflow`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `2fa84d91361c4639908613e5e4fa17b3`.
- **Requirement established:** Target-neutral compile-time layout geometry query proofs (`size_of` and `align_of`) and native realization. Specifically, querying `size_of` and `align_of` on a two-field `u64` structural record (`Pair { lo: u64, hi: u64 }`) establishes its exact layout geometry (`size == 16u64` and `align == 8u64`), discharging exact arithmetic `size - (align * 2u64) == 0u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time layout evaluation of the structural record `Pair { lo: u64, hi: u64 }` statically determines `size_of<Pair>() == 16u64` and `align_of<Pair>() == 8u64` from field widths (two 64-bit scalars) and scalar alignment rules. The static bounds `[16, 16]` and `[8, 8]` justify forming exact subtraction `size - (align * 2u64)` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The static layout geometry values (16u64 and 8u64) retain their exact scalar identity, precision, and alignment constraints across copies, structural fields, explicit parameters, helper call returns, match join arms, named constants, state transitions, machine calls, and target bodies.
  - **Composition:** The layout geometry queries compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`size - (align * 2u64)`), evaluating to `0u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the record definition (`Pair { lo: u64, hi: u64 }` with size 16 vs `Pair { lo: u64 }` with size 8).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `Pair` is narrowed to a single `u64` field: querying `size_of<Pair>()` yields 8u64, while `align_of<Pair>()` remains 8u64, so exact subtraction `8u64 - (8u64 * 2u64)` (`8u64 - 16u64`) underflows `u64` (`bounds [-8, -8] lie outside [0, 18446744073709551615]`). Compilation strictly rejects with `exact arithmetic may overflow`. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral layout queries `size_of<Pair>()` and `align_of<Pair>()` and exact integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining concrete layout plans and certified geometry constants. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The structural record layout validates: `Pair` contains `lo: u64` (offset 0, size 8, align 8) and `hi: u64` (offset 8, size 8, align 8), totaling size 16 with alignment 8. Target `CallPlan`/`StatePlan` assign arguments and return homes across register lanes, stack slots, or indirect aggregate return homes (`sret`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA arithmetic instructions (`sub`, `shl`/`imul` or constant-folded subtraction of 16), register moves, and comparison branches. Low-level register allocators assign register homes and disjoint activation stack-frame slots, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic variable-length slice layout queries (`size_of_val`), alignment padding with heterogeneous composite field types, and unaligned packed layout structures.
