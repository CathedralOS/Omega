# 27-Argument Stack Calling Plan Fact Row

This fact row proves that a 27-argument calling plan across register and stack parameter slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 27th stack-passed argument's static interval bound `40u64 <= a27 <= 80u64`, proving exact unsigned subtractions `80u64 - res` and `res - 40u64` overflow- and underflow-free without runtime checks across all nine canonical language places.

```omg
// Fact: a 27-argument calling plan across register and stack slots preserves the 27th stack argument bound 40u64 <= a27 <= 80u64 to justify 80u64 - res and res - 40u64.
```

## Calling Plan Across Hosted ABI Targets

Every place invokes or transports arguments through a 27-parameter machine call `pick27(a1, ..., a27)` where `a1..a26` are fixed prefix constants (`1..26`, sum 351) and the 27th argument `a27` is bounded by `40u64 <= a27 <= 80u64`. Governed by the ABI calling plan specification ([wiki/spec/build/calling_plans.md](../../../../wiki/spec/build/calling_plans.md)) and activation storage rules ([wiki/spec/resources/activation_storage.md](../../../../wiki/spec/resources/activation_storage.md)), argument placement divides across register vs. stack slots across the four hosted targets:
- **`windows_x86_64`** (Microsoft x64 ABI): arguments 1..4 reside in registers (`RCX, RDX, R8, R9`), and arguments 5..27 are passed on the caller's stack frame (23 stack slots above the 32-byte shadow space).
- **`linux_x86_64`** (System V AMD64 ABI): arguments 1..6 reside in registers (`RDI, RSI, RDX, RCX, R8, R9`), and arguments 7..27 are passed on the caller's stack frame (21 stack slots).
- **`macos_arm64`** / **`linux_arm64`** (AAPCS64): arguments 1..8 reside in general-purpose registers (`X0..X7`), and arguments 9..27 are passed on the caller's stack frame (19 stack slots).

On every hosted target, the 27th argument `a27` is guaranteed to be passed via a stack slot. The machine contract guarantees `result >= 40u64 && result <= 80u64 && result == a27`. This static bound proves that the consuming exact 64-bit unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20) are safe and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the 27-argument calling plan and its bounded stack-passed result changes across the nine places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | 27-argument calling plan is invoked directly in main |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | 27 arguments are held in local variables before invocation |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded call result is stored in a struct field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | 27 arguments arrive as machine parameters and are passed to the call |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | call results supply operands to the multi-argument call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching invoke the 27-argument calling plan and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply arguments to the multi-argument call |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the call result to a successor state before subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives arguments and invokes the 27-argument calling plan |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies invoke the 27-argument calling plan and consume the bounded result on each hosted target |

Match the fact sentence beside every place: each positive place is pinned as
`tests/omega/pass/facts/twentyseven_arg_stack_calling_plan_through_<place>_exit` and
each reject twin as
`tests/omega/fail/facts/twentyseven_arg_stack_calling_plan_through_<place>_rejected`,
byte-equal to the place and `invalid/` programs beside them.

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/local/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/field/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/parameter/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/call_result/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/match_join/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/const_name/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/other_state/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/other_machine/main.omg
omega run --both samples/cli/facts/twentyseven_arg_stack_calling_plan/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the expectation in the transition guard is modified to require a non-zero exit or if an unexpected computation result occurs (`diff != 0u64 || headroom != 20u64 || margin != 20u64`), execution transitions to `state bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose 27th argument bound is widened to `81u64` (`a27 = 81u64`), allowing the return value `res` to reach `81u64 > 80u64`. When computing `headroom = 80u64 - res`, static checking fails closed with diagnostic `exact arithmetic in machine ... may overflow \`u64\`` (decision 17 -- exact arithmetic is a proof obligation). None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/twentyseven_arg_stack_calling_plan/<place>/invalid/main.omg
```

| Place | Reject Twin | Authored Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `may overflow \`u64\`` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `may overflow \`u64\`` |

## Requirements and Evidence

- **Parent outcome:** `NORMALIZED-ABI-LOWERING` ([TASKS.md](../../../../TASKS.md)) — register/stack argument placement under the [calling-plan contract](../../../../wiki/spec/build/calling_plans.md).
- **Requirement established:** A 27-argument calling plan across register and stack ABI slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 27th stack-passed argument's static interval bound `40u64 <= a27 <= 80u64`, proving exact unsigned subtractions `80u64 - res` and `res - 40u64` overflow- and underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Contracts on `pick27` (`a1 == 1u64 && ... && a26 == 26u64 && a27 >= 40u64 && a27 <= 80u64`) establish preconditions for the 27-argument calling sequence. The function computes prefix sum `prefix == 351u64` and returns `(prefix - 351u64) + a27 == a27`, ensuring `result >= 40u64 && result <= 80u64 && result == a27`. The static interval bounds on the returned scalar directly justify the consuming exact unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 27th argument occupies an 8-byte scalar value slot (`u64`) passed via a caller-allocated stack frame home across all four hosted targets. The argument retains its exact value identity and interval bounds across locals, struct fields, machine parameters, return values, match join points, constant names, state transition parameters, cross-machine calls, and target-scoped bodies without truncation, register spill corruption, or stack aliasing.
- **Composition:** The 27-argument calling plan composes through all 9 places into the consuming operations `80u64 - res` and `res - 40u64`. Positive places and reject twins differ strictly in the 27th argument value (`60u64` vs `81u64`) and contract bounds (`80u64` vs `81u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`may overflow \`u64\`` / decision 17), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The 27-argument call expression lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) call nodes. Mathematical proof certificates and range assertions are verified and erased, retaining 64-bit unsigned scalar arithmetic (`ScalarType::U64`) and multi-argument call plans, executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). ABI calling plan lowering resolves register vs. stack placements: `CallPlan` and `StatePlan` place arguments 1..4 in `RCX, RDX, R8, R9` and 5..27 on the stack on Windows x86_64 (23 stack slots); arguments 1..6 in `RDI, RSI, RDX, RCX, R8, R9` and 7..27 on the stack on Linux x86_64 (21 stack slots); and arguments 1..8 in `X0..X7` and 9..27 on the stack on ARM64 (19 stack slots). The 27th argument is consistently placed in a validated stack parameter slot with 8-byte alignment and offset calculation.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects target ISA instructions (`mov [rsp + disp], reg` on x86_64, `str reg, [sp, #disp]` on ARM64) to spill outgoing stack arguments before call invocation, and generates incoming stack loads for the 27th argument within the callee activation frame. Register allocation computes worst-case stack usage (WCSU), ensuring activation frames accommodate outgoing stack argument areas without frame overlap.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Stack displacements and call instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/twentyseven_arg_stack_calling_plan`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).
- Re-measured on macOS arm64 at producer revision `9b001b8f6afa` (shared generation producer `gen-9b001b8f6af`): `omega run --both` exits 0 with `native exit 0` and `interp exit: 0` for the root and all nine places (10/10), and each of the nine `<place>/invalid` twins exits 1 at checking carrying `may overflow \`u64\`` (decision 17, exact arithmetic is a proof obligation). `git diff` positive vs twin changes only the header oracle lines, the widened bound (`a27 <= 80u64` to `a27 <= 81u64`, matching `result`), and the argument value (`60` to `81`) — 10 to 16 changed lines per place, nothing structural.
- Pinned record at `9b001b8f6afa+wt` with the generation runner at `OMEGA_CORPUS_JOBS=3`: nine `checked` rows with `check-target:macos_arm64` and nine `rejected ... expected` rows carrying the decision-17 `may overflow \`u64\`` continuation line in `corpus_outcomes.txt`; nine `built ... exit:0` rows in `corpus_native_macos_arm64.txt`; nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `corpus_both_macos_arm64.txt`. 0 keys lost, 0 timeout rows, 0 rejected without their expected fragment, 0 fail fixtures that did not reject.
- Discrimination: modifying transition conditions causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
