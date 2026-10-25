# Multi-Exit Callee-Saved Register Restoration

A multi-return callee with interior call pressure restores callee-saved registers on every exit path to preserve 100u64 when subtracting 100u64.

Every place invokes a multi-exit helper machine `multi_exit_probe(selector: u64, x: u64) -> u64` across three separate return states (`path_zero`, `path_one`, and `path_two`) selected by `selector`.
Each path holds eight live scalar bindings across an interior clobber helper call `touch_registers(1u64)` before returning `40u64` under contract `ensures result >= 40u64 && result <= 40u64`.
To satisfy this call pressure, the backend register allocator allocates callee-saved registers, requiring `frame_application/insertion` to emit an entry prologue saving them and insert a matching restoration epilogue at every retained return site while reflowing block/row coordinates and branch offsets.
In the caller, three scalar values (`k1 = 10u64`, `k2 = 20u64`, `k3 = 30u64`, summing to `60u64`) are held live across calls exercising the early and late return paths (`multi_exit_probe(0u64, 40u64)`, `multi_exit_probe(1u64, 40u64)`, and `multi_exit_probe(2u64, 40u64)`).
The preserved caller live sum (`60u64`) and normalized probe results (`(r0 + r1 + r2) - 80u64 == 40u64`) are combined into `100u64`, transported through each respective place, and exact subtraction `total - 100u64` is performed, returning `0` as `i32`.

## Places

The nine places carry the preserved caller-live sum through each language construct:

| Place | Positive | Reject Twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | original witness: multi-return callee with interior call pressure preserves caller live sum |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | caller live scalar transported via local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | caller live scalar transported via record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | caller live scalar passed as machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | caller live scalar returned by call |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | caller live scalar converged across match branches |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | caller live scalar defined by named constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | caller live scalar transitioned to successor machine |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | caller live scalar forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | caller live scalar handled across target-scoped bodies |

All positive places and root bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and pass `omega run --both` with exit 0 (Terminal Psi and native execution agreement):

```sh
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/local/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/field/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/parameter/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/call_result/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/match_join/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/const_name/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/other_state/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/other_machine/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/target_body/main.omg
```

Each place has an `invalid/` twin whose caller live value is broken (`k3 = 29u64` instead of `30u64`, or `BASE_K1 = 9u64` instead of `10u64`), causing the combined sum to be `99u64` and exact subtraction `total - 100u64` to fail compilation with `exact arithmetic in machine ... may overflow \`u64\`` (`exact subtraction underflow`). Check each twin with:

```sh
omega --check samples/cli/facts/multi_exit_callee_saved_restoration/<place>/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `ef38e603ddd04e6ca8065e8e62083781`.
- **Requirement established:** Callee-saved register restoration across all distinct return exits of a multi-exit function under interior call pressure, preserving caller live scalar values across function calls and exact subtraction obligations through all nine program places.

Full 8-layer specification and lowering evidence:

- **Establishment:** Every exit path of the multi-exit machine (`path_zero`, `path_one`, and `path_two`) independently establishes its ensured contract `ensures result >= 40u64 && result <= 40u64` after executing interior call `touch_registers(1u64)`. The caller pre-call values (`k1 = 10u64`, `k2 = 20u64`, `k3 = 30u64`, summing to `60u64`) are preserved across the call sequence without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** Caller-live scalar values (`k1`, `k2`, `k3`) and multi-exit return bounds retain their exact storage and register identities across early, middle, and late exits without clobbering or memory corruption.
- **Composition:** The multi-exit return bound and callee-saved preservation compose through all nine standard places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the caller's post-return consuming operation (`total - 100u64`). In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in corrupting a caller live value (e.g. `k3 = 29u64` instead of `30u64`), leaving the consuming exact subtraction unproved.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time (`--check`) with `exact arithmetic in machine \`run\` ... may overflow \`u64\``, proving that an unproved arithmetic obligation cannot pass compilation.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Multi-exit control-flow edges and return outcomes lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`) basic blocks with discrete `Return` terminators while erasing static proof-only obligations, executing identically in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates (64-bit unsigned integers, size 8, align 8), and `CallPlan`/`StatePlan` record callee-saved preservation contracts (`RBX`/`RBP`/`RDI`/`RSI`/`R12`–`R15` on Win64/SysV; `X19`–`X28`/`X29`/`X30` on AAPCS64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering assigns callee-saved register homes across multi-exit functions under interior call pressure, inserts an entry prologue saving all used callee-saved registers, and emits matching restoration epilogues on every return exit path honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, multi-exit basic blocks and branches relax with offset reflow after prologue/epilogue insertion, transitive machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

## What remains open

Dynamic stack realignment for over-aligned frame objects across multi-exit unwinding boundaries and exception-edge restoration profiles.
