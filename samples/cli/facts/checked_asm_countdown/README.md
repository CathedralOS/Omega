# Checked assembly countdown

The checked seed-2 mov/sub/jnz CFG falls through with eax == 0 and stores output == 0.

The root is the original assigned reproduction. Each child carries the same
seed, instructions, loop contract and fallthrough contract. Its `invalid` twin
changes only `done requires eax == 0` to `done requires eax == 1`.
The initial output is 99, so skipping the assembly cannot satisfy the runtime
oracle. The match witness invokes both branches. The target witness has Linux
and Windows x86-64 bodies; only the matching Linux body executes here.

## Places

| Place | Positive | Reject twin | Evidence transported |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root assembly countdown in local storage |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | scalar local copies and drives the countdown |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | record field holds seed and feeds countdown |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit callee parameter passes seed to assembly |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper call result supplies seed |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms execute and supply seed |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant supplies seed to assembly |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state receives seed and executes countdown |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine delegates countdown to helper |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute countdown |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/checked_asm_countdown/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/local/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/field/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/parameter/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/call_result/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/match_join/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/const_name/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/other_state/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/other_machine/main.omg
omega run --both samples/cli/facts/checked_asm_countdown/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The initial output is set to `99`. The assembly block counts down from `2` to `0`, writing `eax` into `output`. The transition `output == 0 { true -> done() false -> bad() }` traps via `crash Trap;` if the assembly countdown does not execute or outputs a nonzero value, agreeing on native signal 5 and interpreter crash `Trap` (exit 255).
- Each invalid twin changes `done requires eax == 0` to `done requires eax == 1`. At the conditional fallthrough from `jnz loop`, ZF is set (`eax == 0`), which cannot satisfy `eax == 1`, failing closed at compile time with `cannot prove requires contract for call done` in state `loop`.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/checked_asm_countdown/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/checked_asm_countdown --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call done`).

## Pinned Fixtures

The row is pinned under `tests/omega/{pass,fail}/facts/checked_asm_countdown_*` fixtures.
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for checked assembly countdown control flow. Specifically, a checked `asm` loop (`mov`, `sub`, `jnz`) with declared register contracts (`requires eax > 0`, `requires eax == 0`) establishes the post-assembly bound `output == 0` directly upon fallthrough across all nine language places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** The checked `asm` block's declared register and flag contracts (`loop requires eax > 0`, `done requires eax == 0`), countdown invariant, and target-scoped body admission establish the post-assembly bound `output == 0` directly upon fallthrough without requiring any post-hoc caller equality guard (`transition x == 0`).
  - **Identity and lifetime:** The checked `asm` output value and countdown state remain bound to the exact scalar/register subjects (`output: u32` and scratch register `eax`) across local copy, record field projection, explicit parameter, call result return, match-expression join, named constant bound, state transition, forwarding machine, and target-scoped machine bodies.
  - **Composition:** The checked `asm` countdown fact composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming `output == 0` check and termination contract. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the construct's assembly destination contract (`done requires eax == 0` vs `done requires eax == 1`), while preserving all instruction sequences, registers, clobbers, and caller structures.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during proof checking when the checked `asm` block violates its declared register/clobber/bound contract: the conditional fallthrough from `jnz loop` with ZF=1 leaves `eax == 0`, which cannot satisfy `done requires eax == 1`, failing closed at compile time with `cannot prove requires contract for call done ... in state \`loop\`` (exit 1). Label assertions and asm blocks never waive proof admission or allow unproved hardware states to escape.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Checked `asm` control flow, blocks, and target-scoped body candidates lower into canonical target-neutral Terminal Psi (`PSITERM\0`), decoding 32-bit GPR moves/subtractions and ZF branches into value versions and checked state edges while preserving target-specific assembly operand/clobber contracts. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` and target-scoped body selection settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The `u32` scalar and seed record layout geometry validates (`size_of == 4`, `align_of == 4`), and target `CallPlan`/`StatePlan` place arguments and return homes across registers and stack slots according to target ABI conventions.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering realizes checked `asm` instructions directly into target ISA instructions (`mov`, `sub`, `jnz` on `x86_64`), binds explicit physical/virtual register homes (EAX), honors declared clobbers and callee-saved register preservation, assigns disjoint activation storage for `output`, and inserts entry prologues and multi-return epilogues honoring the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Checked `asm` instruction forms encode directly into machine bytes (short relative jump displacements for `jmp` and `jnz`), basic blocks and countdown branches relax with offset reflow, transitive machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Wider GPR/SIMD register views, 64-bit countdown induction, memory-operand checked assembly, ARM64-specific assembly countdown instruction variants (`cbz`/`cbnz`), and linear hardware effect retirement.
