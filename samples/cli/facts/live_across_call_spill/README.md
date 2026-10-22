# Live-Across-Call Register Preservation and Spill/Reload

Eight simultaneously live scalar bindings preserved across an interior call sum to 100u64 when subtracting 100u64.

Every positive place and reject twin defines eight distinct `u64` scalar bindings (`v1 = 5u64`, `v2 = 10u64`, `v3 = 12u64`, `v4 = 13u64`, `v5 = 14u64`, `v6 = 15u64`, `v7 = 16u64`, `v8 = 15u64`, summing to `100u64`) before calling an interior helper machine `clobber_call(seed: u64) -> u64` that executes intermediate arithmetic operations clobbering caller-saved registers and returns `0u64` under contract `requires seed == 1u64 ensures result == 0u64`.
After `clobber_call(1u64)` returns, all eight pre-call live values plus the call result are combined (`v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + call_out == 100u64`), transported through the respective place, and exact subtraction `preserved - 100u64` is performed, returning `0` as `i32`.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with eight live scalar bindings preserved across clobber_call |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | all eight live scalar bindings survive across clobber_call and are combined into a local before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the post-call combined value is stored into and read from a record field before exact subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the post-call combined value is transported as a parameter to an identity helper machine before exact subtraction |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the post-call combined value is returned from a dedicated helper call before exact subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | the post-call combined value is transported across both arms of a conditional match join before exact subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the initial live scalar value held in a local across clobber_call before exact subtraction |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | the post-call combined value is passed across a state transition into another state before exact subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | the post-call combined value is forwarded through an independent helper machine before exact subtraction |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | every supported target architecture provides its own target-scoped machine body executing the exact subtraction |

## Execution

Every positive place and root `main.omg` binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/live_across_call_spill/main.omg
omega run --both samples/cli/facts/live_across_call_spill/local/main.omg
omega run --both samples/cli/facts/live_across_call_spill/field/main.omg
omega run --both samples/cli/facts/live_across_call_spill/parameter/main.omg
omega run --both samples/cli/facts/live_across_call_spill/call_result/main.omg
omega run --both samples/cli/facts/live_across_call_spill/match_join/main.omg
omega run --both samples/cli/facts/live_across_call_spill/const_name/main.omg
omega run --both samples/cli/facts/live_across_call_spill/other_state/main.omg
omega run --both samples/cli/facts/live_across_call_spill/other_machine/main.omg
omega run --both samples/cli/facts/live_across_call_spill/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, the eight live pre-call scalar values `5 + 10 + 12 + 13 + 14 + 15 + 16 + 15` plus `call_out == 0` evaluate exactly to `100u64`.
Exact subtraction `diff: u64 = preserved - 100u64` statically proves no underflow and evaluates to `0u64`, returning `0` as `i32` to the `main` state machine which transitions to `done()` with exit 0.
If any value is corrupted or register preservation fails across `clobber_call`, the sum deviates from `100u64`, failing the `transition outcome == 0` guard and triggering `bad() { crash Trap; }`.

### Invalid controls and reject twins

Each place has an `invalid/` twin (and sibling `invalid/<place>` entry) that weakens the pre-call scalar value `v8` from `15u64` to `14u64` (summing to `99u64`), causing the post-call exact subtraction `preserved - 100u64` to fail proof obligations.

The expected diagnostic is:

```
exact arithmetic in machine `run`
```

specifically:

```
error: exact arithmetic in machine `run` state ... local `diff` may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)
```

Restoring the true pre-call sum (`100u64`) restores clean compilation and zero exit.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`)
- **Ticket:** `7a9b4fb00e034587ad28fd21781dfc63`
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/live_across_call_spill_through_<place>_exit/`
  - Negative controls: `tests/omega/fail/facts/live_across_call_spill_through_<place>_rejected/`

### Specification and Lowering Layers

1. **Establishment:** The eight distinct scalar bindings (`v1 = 5u64`, `v2 = 10u64`, `v3 = 12u64`, `v4 = 13u64`, `v5 = 14u64`, `v6 = 15u64`, `v7 = 16u64`, `v8 = 15u64`) and their constant/contractual bounds are statically established in the caller before the interior invocation. `clobber_call` declares contract `requires seed == 1u64 ensures result == 0u64`. All eight scalar values are preserved across the call without relying on any post-hoc caller equality guard (`transition x == 100`) to manufacture or re-establish value facts.
2. **Identity and lifetime:** The eight live-across-call scalar values retain their exact SSA and storage identities and known values across the interior call to `clobber_call`, despite `clobber_call` executing multi-step arithmetic that clobbers all target caller-saved scratch registers.
3. **Composition:** The preserved pre-call values and the call result compose through all nine canonical places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation `preserved - 100u64`. Each positive `<place>/main.omg` differs from its `<place>/invalid/main.omg` reject twin strictly in the pre-call bound (`v8 = 14u64` instead of `15u64`), leaving all subsequent operations, control flow, and place transport structures identical.
4. **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `exact arithmetic in machine \`run\` ... may overflow \`u64\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` because `99u64 - 100u64` cannot be proved safe in the unsigned exact arithmetic domain. The compiler refuses to emit native code when exact arithmetic bounds cannot be verified.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The eight live values, arithmetic operations, and interior call edges lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`). Erased proof-only obligations are stripped while live SSA values and machine call boundaries are preserved. The resulting Terminal Psi executes in the interpreter (`omega run --both`) agreeing with native execution at exit 0 across all places.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar types (`u64`, `i32`) resolve their standard widths and 8-byte alignments. ABI calling conventions classify caller-saved vs. callee-saved registers: `clobber_call` requires argument passing in standard calling plan registers (`rcx` on Windows x86_64, `rdi` on Linux x86_64, `x0` on ARM64) and clobbers volatile scratch registers, forcing live-across-call values into callee-saved registers or dedicated activation frame spill slots.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering maps the eight live values to register homes or activation stack-frame spill slots. Spilled registers are allocated disjoint 8-byte stack slots within the frame extent, accounted for in worst-case stack usage (WCSU). The backend emits machine entry prologues allocating stack space and epilogues restoring callee-saved registers, with memory stores before the call and reloads after the call.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete addressing modes (`[rsp + disp]` on x86_64, `[sp, #imm]` on arm64). Function basic blocks and call/return sequences relax with offset reflow. Transitive machine-state footprint evidence validates stack and register allocations, and direct PE/ELF/Mach-O emission produces native binaries that agree with Terminal Psi (`omega run --both` exit 0) on `windows_x86_64` and `macos_arm64`.

## Measured evidence

Producer: the reference `omega` CLI at the landing candidate, `macos_arm64` host,
`OMEGA_LIBRARY_ROOT=<checkout>/source/library`.

| Measurement | Result |
| --- | --- |
| `omega run --both` root and nine places | native exit 0, interpreter exit 0, agreeing (10/10) |
| `sample_native_gate --facts --filter live_across_call_spill` | 10/10 meet their oracle; 18/18 intended-invalid controls match their authored rejection; exit 0 |
| `corpus_gate --native --filter facts/live_across_call_spill` | clean; 9 fixtures `built`, executed 9, exit codes `{'0': 9}`; exit 0 |
| `tests/omega/corpus_native_macos_arm64.txt` | nine `built ... exit:0` rows, splice added 9 and removed 0 |

Each reject twin refuses with exit 1 and the diagnostic
`exact arithmetic ... may overflow \`u64\`` (decision 17, exact arithmetic is a
proof obligation). The nine `invalid/<place>/` twins and the nine
`<place>/invalid/` mirrors both carry `expected.txt` and `sample.json` with
`rejection_contains`, so no twin reports `rejection-failed`.

### Oracle discrimination

The oracle reads the post-call observation, so it moves when a value that must
survive the call is corrupted. Measured on this host:

| Program | Native | Interpreter |
| --- | --- | --- |
| live binding `v8 = 15u64` carried across `clobber_call` (baseline) | exit 0 | exit 0 |
| same, `v8 = 14u64` at the call site | signal 5 | crash Trap |
| callee result changed from `0u64` to `1u64` | signal 5 | crash Trap |

Both engines detect the corruption and agree, and restoring the value restores
exit 0. A compiler that clobbered or spilled a live value incorrectly would
change the post-call observation rather than pass.
