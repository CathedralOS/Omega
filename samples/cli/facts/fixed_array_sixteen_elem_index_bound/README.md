# Fact: 16-Element Fixed Array Index Bound

> a static index bound idx < 16u64 carried through the place proves safe fixed-size array indexing arr[idx] on [u64; 16].

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 16-element fixed-size array indexing bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | array and bounded index are copied into locals before indexing |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | record field carries the bounded index alongside the fixed-extent array (`Container`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | reading machine receives the bounded index and array directly as parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | index arrives from a helper call with an `ensures result < 16u64` contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of a match expression produce in-bounds indices |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant bounds the index contract for the fixed-extent array (`LIMIT = 16u64`) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | transition to a successor machine forwards the bounded index and array |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine transports the bounded index and array to the reader |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target bodies reads the array element using the bounded index |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/local/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/field/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/parameter/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/call_result/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/match_join/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/const_name/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/other_state/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/other_machine/main.omg
omega run --both samples/cli/facts/fixed_array_sixteen_elem_index_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The fixed-size array `arr: [u64; 16]` contains 16 distinct `u64` values: `[10u64, 20u64, 30u64, 40u64, 50u64, 60u64, 70u64, 80u64, 90u64, 100u64, 110u64, 120u64, 130u64, 140u64, 150u64, 160u64]`.
- Static index bound `idx < 16u64` proves that index `idx` is in range `[0, 15]`.
- Reading element `arr[11u64]` evaluates to `120u64`.
- At runtime, `elem == 120u64` branches to `done()` (exit 0) or `bad()` (`crash Trap;`, exit 255) on discrepancy.
- Measured discrimination: perturbing `elem == 120u64` to `elem == 121u64` causes native execution to crash with STATUS_ILLEGAL_INSTRUCTION (-1073741795) and the Terminal Psi interpreter to crash with `Trap (host presentation agrees)`. Both engines trap and reject the perturbation.
- Each invalid twin weakens the index bound (e.g. `idx <= 16u64` or `LIMIT = 17u64`), allowing `idx = 16u64` which equals the array length and would exceed bounds `[0, 15]`. Every invalid twin fails closed during Psi proof checking with diagnostic `cannot prove index ... is within length 16`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/fixed_array_sixteen_elem_index_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/fixed_array_sixteen_elem_index_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`index`).

## Pinned Fixtures

The row is pinned under:
- `tests/omega/pass/facts/fixed_array_sixteen_elem_index_bound_through_<place>_exit/`
- `tests/omega/fail/facts/fixed_array_sixteen_elem_index_bound_through_<place>_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS` / `TERMINAL-ARRAY-LAYOUTS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 16-element fixed-size array (`[u64; 16]`) bounded indexing (`idx < 16u64`), in-bounds proof preservation across all 9 places, out-of-bounds rejection, and native realization agreement.
- **Obligation class:** Fixed-size array layout geometry preservation and array index boundary verification across transport places.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Fixed-extent array construction `[10u64, ..., 160u64]` and entry precondition `requires idx < 16u64` (or record constraint `where index < 16u64` / named constant `LIMIT = 16u64`) establish proven in-bounds access before indexing without dynamic bounds checks or post-hoc equality guards.
  - **Identity and lifetime:** The array element values and index bound scalar retain their identity across copies and transports in all nine places (local variables, nominal record fields `Container`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies). Element `arr[11] == 120u64` is preserved intact.
  - **Composition:** Array access composes through calling conventions and control flow. In `call_result`, `supply_index` formally proves postcondition `ensures result < 16u64`; in `match_join`, both arms establish in-bounds index values across the join; in `other_state` and `other_machine`, contract arrivals verify incoming index bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the index bound is weakened to `idx <= 16u64` (or `LIMIT = 17u64`). Because index `16u64` equals the array length and would read out of bounds, compilation fails closed with diagnostic `cannot prove index ... is within length 16`. No out-of-bounds access escapes into runtime execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Fixed-size array indexing lowers into target-neutral Terminal Psi (`PSITERM\0`) array projection operations with static bounds certificates. Proof obligations for in-bounds index checks are satisfied and erased while preserving executable element indexing. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar and composite layout geometry validates 16 contiguous 8-byte elements (`size_of = 128`, `align_of = 8`). Calling plans allocate standard ABI memory/stack references or pointer passing conventions for large fixed-size arrays.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects scaled-index addressing modes: on x86_64, `mov reg, [base + index*8]`; on AArch64, `ldr x_dest, [base, index, lsl #3]`. Register allocation assigns register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, emitting native executable images (PE/ELF/Mach-O). Exact native execution and Terminal Psi interpretation agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- Multi-dimensional fixed-size array indexing (e.g. `[[u64; 16]; 4]`).
- Dynamic slice indexing with runtime-carried lengths.
- Mutable element writes through bounded indices (`arr[idx] = val`).
