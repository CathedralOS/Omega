# 15-Argument Stack Calling Plan Fact Row

Fact: a 15-argument calling plan across register and stack slots preserves the 15th stack argument bound 40u64 <= a15 <= 80u64 to justify 80u64 - res and res - 40u64.

Every place invokes or transports arguments through a 15-parameter machine call `pick15(a1, ..., a15)` where `a1..a14` are fixed prefix constants (`1..14`, sum 105) and the 15th argument `a15` is bounded by `40u64 <= a15 <= 80u64`. Across all four hosted target calling conventions:
- **`windows_x86_64`**: arguments 1..4 reside in registers (`RCX, RDX, R8, R9`), and arguments 5..15 are passed on the caller's stack frame.
- **`linux_x86_64`** (System V AMD64 ABI): arguments 1..6 reside in registers (`RDI, RSI, RDX, RCX, R8, R9`), and arguments 7..15 are passed on the stack.
- **`macos_arm64`** / **`linux_arm64`** (AAPCS64): arguments 1..8 reside in registers (`X0..X7`), and arguments 9..15 are passed on the stack.

On every target, the 15th argument `a15` is guaranteed to be passed via a stack slot. The machine contract guarantees `result >= 40u64 && result <= 80u64 && result == a15`. This static bound proves that the consuming exact 64-bit unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20) are safe and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the 15-argument calling plan and its bounded stack-passed result changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded call result is held in a local variable before exact subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded call result is stored in a struct field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded call result is passed to a machine parameter with contract before subtractions |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | an intermediate helper machine forwards the 15-argument call result |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching invoke the 15-argument calling plan and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant supplies the 15th stack-passed argument before invocation |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bounded 15-argument call result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives arguments and invokes the 15-argument calling plan |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies invoke the 15-argument calling plan and consume the bounded result on each hosted target |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/local/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/field/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/parameter/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/call_result/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/match_join/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/const_name/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/other_state/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/other_machine/main.omg
omega run --both samples/cli/facts/fifteen_arg_stack_calling_plan/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. The obligation class is unsigned exact arithmetic underflow prevention on 64-bit integers (`ScalarType::U64` under decision 17: exact arithmetic is a proof obligation). If the expectation in the transition guard is modified via a 1-token perturbation (e.g. changing `headroom == 20u64` to `headroom == 21u64`), execution transitions to `machine bad() { crash Trap; }`. Tested against the opt-level 1 compiler on `windows_x86_64`, this perturbation immediately produces:
- Native execution: `native exit -1073741795` (fast-fail trap)
- Terminal Psi interpreter: `interp crash: Trap (host presentation agrees)`

Both engines strictly agree and trap upon perturbation, demonstrating genuine oracle discrimination.

### Reject Twins and Pinned Corpus Fixtures

Each place has an `invalid/` twin whose 15th argument bound is widened to `81u64` (`a15 = 81u64`), allowing the return value `res` to reach `81u64 > 80u64`. When computing `headroom = 80u64 - res`, static checking fails closed with diagnostic `exact arithmetic in machine ... may overflow \`u64\`` (decision 17 -- exact arithmetic is a proof obligation). None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/fifteen_arg_stack_calling_plan/<place>/invalid/main.omg
```

All 9 positive places and 9 invalid twins are pinned in the test corpus:
- Pass fixtures: `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_<place>_exit/`
- Fail fixtures: `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_<place>_rejected/`

| Place | Positive Sample | Pinned Pass Fixture | Reject Twin | Pinned Fail Fixture | Authored Diagnostic Fragment |
| --- | --- | --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_local_exit` | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_local_rejected` | `may overflow \`u64\`` |
| field | [`field/main.omg`](field/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_field_exit` | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_field_rejected` | `may overflow \`u64\`` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_parameter_exit` | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_parameter_rejected` | `may overflow \`u64\`` |
| call result | [`call_result/main.omg`](call_result/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_call_result_exit` | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_call_result_rejected` | `may overflow \`u64\`` |
| match join | [`match_join/main.omg`](match_join/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_match_join_exit` | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_match_join_rejected` | `may overflow \`u64\`` |
| const name | [`const_name/main.omg`](const_name/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_const_name_exit` | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_const_name_rejected` | `may overflow \`u64\`` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_other_state_exit` | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_other_state_rejected` | `may overflow \`u64\`` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_other_machine_exit` | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_other_machine_rejected` | `may overflow \`u64\`` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | `tests/omega/pass/facts/fifteen_arg_stack_calling_plan_through_target_body_exit` | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/fail/facts/fifteen_arg_stack_calling_plan_through_target_body_rejected` | `may overflow \`u64\`` |

## Requirements and Evidence

- **Parent outcome:** `CALLING-PLANS-STACK-BOUNDS` / `FIFTEEN-ARG-STACK-CALLING-PLAN` (`TASKS.md`)
- **Requirement established:** A 15-argument calling plan across register and stack ABI slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 15th stack-passed argument's static interval bound `40u64 <= a15 <= 80u64`, proving exact unsigned subtractions `80u64 - res` and `res - 40u64` overflow- and underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Contracts on `pick15` (`a1 == 1u64 && ... && a14 == 14u64 && a15 >= 40u64 && a15 <= 80u64`) establish preconditions for the 15-argument calling sequence. The function computes prefix sum `prefix == 105u64` and returns `(prefix - 105u64) + a15 == a15`, ensuring `result >= 40u64 && result <= 80u64 && result == a15`. The static interval bounds on the returned scalar directly justify the consuming exact unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 15th argument occupies an 8-byte scalar value slot (`u64`) passed via a caller-allocated stack frame home across all four hosted targets. The argument retains its exact value identity and interval bounds across locals, struct fields, machine parameters, return values, match join points, constant names, state transition parameters, cross-machine calls, and target-scoped bodies without truncation, register spill corruption, or stack aliasing.
- **Composition:** The 15-argument calling plan composes through all 9 places into the consuming operations `80u64 - res` and `res - 40u64`. Positive places and reject twins differ strictly in the 15th argument value (`60u64` vs `81u64`) and contract bounds (`80u64` vs `81u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`may overflow \`u64\`` / decision 17), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The 15-argument call expression lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) call nodes. Mathematical proof certificates and range assertions are verified and erased, retaining 64-bit unsigned scalar arithmetic (`ScalarType::U64`) and multi-argument call plans, executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). ABI calling plan lowering resolves register vs. stack placements: arguments 1..4 in registers on Windows x86_64 (args 5..15 on stack), arguments 1..6 in registers on Linux x86_64 (args 7..15 on stack), and arguments 1..8 in registers on ARM64 (args 9..15 on stack). The 15th argument is consistently placed in a validated stack parameter slot with 8-byte alignment and offset calculation.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects target ISA instructions (`mov [rsp + disp], reg` on x86_64, `str reg, [sp, #disp]` on ARM64) to spill stack arguments before call invocation, and loads them within the callee activation frame. Register allocation computes worst-case stack usage (WCSU) ensuring activation frames accommodate outgoing stack argument areas without frame overlap.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Stack displacements and call instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/fifteen_arg_stack_calling_plan`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).
- Discrimination: modifying transition conditions causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
- What remains open: multi-argument calls exceeding 16+ arguments, aggregate stack arguments passed by value with indirect `sret` calling plans, and mixed float/vector register and stack calling conventions.
