# Index Every Width

Every integer width (`u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`) with a proved
bound `0 <= index <= 3` indexes a four-element array and XORs the selected `u64` values.
No runtime bounds checks or narrowing assertions are inserted. The root witness and nine place
witnesses use canonical `requires`, `ensures`, `where`, and established flow facts.

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`,
and carries an authored `expected_native_exit.txt` containing `0` beside its entry. Both engines
(`omega run --both`) agree: Terminal Psi and native execution report exit 0.

## Places

| Place | Positive Program | Fact-sensitive Reject Twin | Carrier / Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | root witness exercising `combine_fixed` with all 8 widths and `read_slice` view |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | all 8 width indices copied to local variables before indexing |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | width indices packaged into and projected from `Indices` record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | width indices passed as explicit machine parameters |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper call results supply each width index |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | match arms join on bounded width indices before indexing |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant indices bound and index the array |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | state transition passes bounded width indices to successor machine |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine routes bounded width indices to indexing machine |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute indexing across all hosted targets |

## Execution

Every positive place has an authored native exit oracle of zero and exercises `[1, 2, 4, 8]`
indexed at `0, 1, 2, 3, 0, 1, 2, 2` XORing to 12. The match witness also executes its other
arm whose indices are all 3 and whose XOR is 0. A wrong result crashes `Trap`. Run with `omega run --both`:

```sh
omega run --both samples/cli/facts/index_every_width/main.omg
omega run --both samples/cli/facts/index_every_width/local/main.omg
omega run --both samples/cli/facts/index_every_width/field/main.omg
omega run --both samples/cli/facts/index_every_width/parameter/main.omg
omega run --both samples/cli/facts/index_every_width/call_result/main.omg
omega run --both samples/cli/facts/index_every_width/match_join/main.omg
omega run --both samples/cli/facts/index_every_width/const_name/main.omg
omega run --both samples/cli/facts/index_every_width/other_state/main.omg
omega run --both samples/cli/facts/index_every_width/other_machine/main.omg
omega run --both samples/cli/facts/index_every_width/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/` (mirrored in `<place>/controls/widened_i64/`)
that widens only the `i64` index bound to `<= 4` (or constant `4`), admitting `4` which equals the
fixed array length `4` so `0 <= index < 4` cannot be established. All 9 reject twins fail closed at
proof checking before native execution:

```sh
omega --check samples/cli/facts/index_every_width/<place>/invalid/main.omg
```

Every twin rejects with `cannot prove index ... is within length 4` naming that place's widened index.

## Requirements and Evidence

- **Parent outcome:** `ONE-INTEGER-BOUNDS-ENGINE` / `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and fixed-size array/slice indexing across all integer widths (`u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`). The proved in-bounds premise `0 <= index <= 3` satisfies the index demand `0 <= index < 4` without inserting runtime bounds assertions, with exact agreement between Terminal Psi and native execution across hosted targets.
- **Pinned fixtures:** positive places and reject twins under `tests/omega/pass/facts/index_every_width_through_*_exit` and `tests/omega/fail/facts/index_every_width_through_*_rejected`.
- **Authority:** [Counts, indices, and addresses](../../../../wiki/spec/language/counts_and_addresses.md#bounds-and-lowering) (scalar indexing requires `0 <= index < live_length`; signed widths require nonnegativity evidence), [numeric values](../../../../wiki/spec/language/numeric_values.md) (propositions as bounds, same-width and multi-width scalar rules), and [invocation contracts](../../../../wiki/spec/language/state_contracts.md#bindings-and-arrivals) (all-predecessor joins and fact lifetime).

Full 8-layer specification and lowering evidence:

- **Establishment:** Integer range contracts and entry contracts on scalar indices across all integer widths (`u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`) establish non-negative in-bounds indexing (`0 <= idx && idx < 4`) directly on the exact subject without any post-hoc caller equality guard (`transition x == 100`). Both unsigned and signed integer types prove nonnegativity and upper bounds within `[0, 3]` before array index access is admitted.
- **Identity and lifetime:** The width-typed index bound stays attached to the exact scalar subject across local copy (`local`), record field projection (`field`), explicit parameter passing (`parameter`), completed call result (`call_result`), match convergence join (`match_join`), named const bounds (`const_name`), state transition arguments (`other_state`), forwarding machine calls (`other_machine`), and target-scoped body implementations (`target_body`). No mutation, alias clobbering, or storage invalidation intervenes between fact establishment and consumption.
- **Composition:** Multi-width index bounds compose across all 9 places into the consuming array/slice index access (`values[idx]`). Calls substitute exact actuals, match joins retain the bound justified on both incoming arms (arm 0 exercising distinct indices `0, 1, 2, 3, 0, 1, 2, 2` and arm 1 exercising all indices `3`), and state transitions prove arrival obligations on incoming edges. Each `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in the construct's index bound (widening `index_i64 <= 3` to `<= 4` or constant `4`), never in a caller guard.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed with `cannot prove index ... is within length 4` when an out-of-bounds index value `4` is admitted by the contract or constant. The checker strictly refuses unproved index obligations during proof checking, preventing invalid programs from reaching Terminal Psi or native code emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Width-specific integer index bounds erase cleanly after verification while narrow-to-64-bit index conversion and collection indexing lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`). The resulting programs execute identically in the Terminal Psi interpreter (`omega run --both`), observing exact XOR result 12.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Concrete layout geometry for `[u64; 4]` contiguous arrays (32 bytes, 8-byte alignment) and `Indices` records validates field offsets and packing. `CallPlan` and `StatePlan` place multi-width scalar arguments and returns across register and stack spill lanes according to target calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions for narrow-to-wide zero/sign extension (`movzx`/`movsx` on x86-64 / `uxtb`/`uxth`/`sxtw` on ARM64) and scaled indexed element loads (`mov QWORD PTR [rdi + rsi*8]` on x86-64 / `ldr xN, [xBase, xIdx, lsl #3]` on ARM64). Backend assigns register homes and disjoint stack-frame storage, inserting entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, basic blocks and conditional branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across `windows_x86_64` and `macos_arm64`.

## What remains open

Dynamic runtime-length slice indexing bounds unification with the fixed-size array interval engine, and negative signed lower-bound underflow diagnostics across non-linear index expressions remain open under the parent `ONE-INTEGER-BOUNDS-ENGINE` task.
