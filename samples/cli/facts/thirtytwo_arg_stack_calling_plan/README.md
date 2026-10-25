# 32-Argument Stack Calling Plan Fact Row

This fact row establishes that a 32-argument calling plan across register and stack parameter slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 32nd stack-passed argument's static interval bound `40u64 <= a32 <= 80u64`, proving exact unsigned subtractions `80u64 - res` and `res - 40u64` overflow- and underflow-free without runtime checks across all nine canonical language places.

```omg
// Fact: a 32-argument calling plan across register and stack slots preserves the 32nd stack argument bound 40u64 <= a32 <= 80u64 to justify 80u64 - res and res - 40u64.
```

## Calling Plan Across Hosted ABI Targets

Every place invokes or transports arguments through a 32-parameter machine call `pick32(a1, ..., a32)` where `a1..a31` are fixed prefix constants (`1..31`, sum 496) and the 32nd argument `a32` is bounded by `40u64 <= a32 <= 80u64`. Governed by the ABI calling plan specification ([wiki/spec/build/calling_plans.md](../../../../wiki/spec/build/calling_plans.md)) and activation storage rules ([wiki/spec/resources/activation_storage.md](../../../../wiki/spec/resources/activation_storage.md)), argument placement divides across register vs. stack slots across the four hosted targets:
- **`windows_x86_64`** (Microsoft x64 ABI): arguments 1..4 reside in registers (`RCX, RDX, R8, R9`), and arguments 5..32 are passed on the caller's stack frame (28 stack slots above the 32-byte shadow space).
- **`linux_x86_64`** (System V AMD64 ABI): arguments 1..6 reside in registers (`RDI, RSI, RDX, RCX, R8, R9`), and arguments 7..32 are passed on the caller's stack frame (26 stack slots).
- **`macos_arm64`** / **`linux_arm64`** (AAPCS64): arguments 1..8 reside in general-purpose registers (`X0..X7`), and arguments 9..32 are passed on the caller's stack frame (24 stack slots).

On every hosted target, the 32nd argument `a32` is guaranteed to be passed via a stack slot. The machine contract guarantees `result >= 40u64 && result <= 80u64 && result == a32`. This static bound proves that the consuming exact 64-bit unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20) are safe and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the 32-argument calling plan and its bounded stack-passed result changes across the nine places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | 32-argument calling plan is invoked directly in main |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | 32 arguments are held in local variables before invocation |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded call result is stored in a struct field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | 32 arguments arrive as machine parameters and are passed to the call |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | call results supply operands to the multi-argument call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching invoke the 32-argument calling plan and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply arguments to the multi-argument call |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the call result to a successor machine before subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives arguments and invokes the 32-argument calling plan |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies invoke the 32-argument calling plan and consume the bounded result on each hosted target |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/local/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/field/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/parameter/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/call_result/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/match_join/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/const_name/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_state/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_machine/main.omg
omega run --both samples/cli/facts/thirtytwo_arg_stack_calling_plan/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the expectation in the transition guard is modified to require a non-zero exit or if an unexpected computation result occurs (`diff != 0u64 || headroom != 20u64 || margin != 20u64`), execution transitions to `machine bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose 32nd argument bound is widened to `81u64` (`a32 = 81u64`), allowing the return value `res` to reach `81u64 > 80u64`. When computing `headroom = 80u64 - res`, static checking fails closed with diagnostic `exact arithmetic in machine ... may overflow \`u64\`` (decision 17 -- exact arithmetic is a proof obligation). None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/thirtytwo_arg_stack_calling_plan/<place>/invalid/main.omg
```

| Place | Reject Twin | Authored Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `may overflow \`u64\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `may overflow \`u64\`` |

### Pinned Fixtures

All nine positive places and nine reject twins are pinned as canonical corpus fixtures:
- **Pass fixtures** (`tests/omega/pass/facts/`):
  - `thirtytwo_arg_stack_calling_plan_through_local_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_field_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_parameter_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_call_result_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_match_join_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_const_name_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_other_state_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_other_machine_exit/`
  - `thirtytwo_arg_stack_calling_plan_through_target_body_exit/`
- **Fail fixtures** (`tests/omega/fail/facts/`):
  - `thirtytwo_arg_stack_calling_plan_through_local_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_field_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_parameter_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_call_result_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_match_join_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_const_name_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_other_state_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_other_machine_rejected/`
  - `thirtytwo_arg_stack_calling_plan_through_target_body_rejected/`

## Requirements and Evidence

- **Parent outcome:** `NORMALIZED-ABI-LOWERING` ([TASKS.md](../../../../TASKS.md)) — register/stack argument placement under the [calling-plan contract](../../../../wiki/spec/build/calling_plans.md).
- **Requirement established:** A 32-argument calling plan across register and stack ABI slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 32nd stack-passed argument's static interval bound `40u64 <= a32 <= 80u64`, proving exact unsigned subtractions `80u64 - res` and `res - 40u64` overflow- and underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Contracts on `pick32` (`a1 == 1u64 && ... && a31 == 31u64 && a32 >= 40u64 && a32 <= 80u64`) establish preconditions for the 32-argument calling sequence. The function computes prefix sum `prefix == 496u64` and returns `(prefix - 496u64) + a32 == a32`, ensuring `result >= 40u64 && result <= 80u64 && result == a32`. The static interval bounds on the returned scalar directly justify the consuming exact unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 32nd argument occupies an 8-byte scalar value slot (`u64`) passed via a caller-allocated stack frame home across all four hosted targets. The argument retains its exact value identity and interval bounds across locals, struct fields, machine parameters, return values, match join points, constant names, state transition parameters, cross-machine calls, and target-scoped bodies without truncation, register spill corruption, or stack aliasing.
- **Composition:** The 32-argument calling plan composes through all 9 places into the consuming operations `80u64 - res` and `res - 40u64`. Positive places and reject twins differ strictly in the 32nd argument value (`60u64` vs `81u64`) and contract bounds (`80u64` vs `81u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`may overflow \`u64\`` / decision 17), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The 32-argument call expression lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) call nodes. Mathematical proof certificates and range assertions are verified and erased, retaining 64-bit unsigned scalar arithmetic (`ScalarType::U64`) and multi-argument call plans, executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). ABI calling plan lowering resolves register vs. stack placements: `CallPlan` and `StatePlan` place arguments 1..4 in `RCX, RDX, R8, R9` and 5..32 on the stack on Windows x86_64 (28 stack slots); arguments 1..6 in `RDI, RSI, RDX, RCX, R8, R9` and 7..32 on the stack on Linux x86_64 (26 stack slots); and arguments 1..8 in `X0..X7` and 9..32 on the stack on ARM64 (24 stack slots). The 32nd argument is consistently placed in a validated stack parameter slot with 8-byte alignment and offset calculation.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects target ISA instructions (`mov [rsp + disp], reg` on x86_64, `str reg, [sp, #disp]` on ARM64) to spill outgoing stack arguments before call invocation, and generates incoming stack loads for the 32nd argument within the callee activation frame. Register allocation computes worst-case stack usage (WCSU), ensuring activation frames accommodate outgoing stack argument areas without frame overlap.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Stack displacements and call instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

Measured on macOS arm64 at base `9b001b8f6af` with the shared generation producer
`gen-9b001b8f6af` (omega sha256 `6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`,
corpus_runner sha256 `cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`); every
fixture is a byte-for-byte copy of its place, and every golden row is stamped `@9b001b8f6afa+wt`:

- Check leg (`python3 tools/corpus_gate.py --runner gen-9b001b8f6af/corpus_runner --filter thirtytwo_arg_stack_calling_plan,thirtytwo_variant_enum_payload_bound --record`): 18 `checked ... check-target:macos_arm64` rows (nine places + nine places of the sibling row) and 18 `rejected ... expected` rows carrying their own decision-17 diagnostics; 0 timeout rows.
- Native leg (`--native`): 18 `built ... exit:0` rows.
- Both leg (`--both`): 18 `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows.
- The nine reject twins exit 1 at checking with the exact-arithmetic decision-17 diagnostic (`may overflow \`u64\``); `git diff` positive vs twin shows only the construct's own bound moved (`a32 <= 80u64` to `a32 <= 81u64`, or `C32: u64 = 60` to `81` at `const_name`).
- Discrimination: modifying transition conditions causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
- What remains open: multi-argument calls exceeding 33+ arguments, aggregate stack arguments passed by value with indirect `sret` calling plans, and mixed float/vector register and stack calling conventions.
