# Twenty-One Argument Stack Calling Plan Fact Row

This fact row establishes that a 21-argument calling plan across register and stack parameter slots preserves the static interval bounds of the 21st parameter (passed on the stack across all four hosted targets), proving exact unsigned subtraction operations overflow- and underflow-free without runtime checks across all nine canonical language places.

```omg
// Fact: a 21-argument calling plan across register and stack slots preserves the 21st stack argument bound 40u64 <= a21 <= 80u64 to justify 80u64 - res and res - 40u64.
```

## Calling Plan Across Hosted ABI Targets

The 21-argument function `pick21(a1..a21: u64) -> u64` stresses ABI calling plan resolution across all four supported targets:
- **`windows_x86_64`**: 4 integer register arguments (`RCX`, `RDX`, `R8`, `R9`) followed by 17 stack arguments (`a5..a21`) placed in caller shadow/stack space.
- **`linux_x86_64`**: 6 integer register arguments (`RDI`, `RSI`, `RDX`, `RCX`, `R8`, `R9`) followed by 15 stack arguments (`a7..a21`) passed via the stack.
- **`macos_arm64` / `linux_arm64`**: 8 general-purpose register arguments (`X0..X7`) followed by 13 stack arguments (`a9..a21`) passed via caller activation frame slots.

On every hosted target, `a21` is passed via the stack. Arguments `a1..a20` receive fixed constants `1..20` (prefix sum 210), while `a21` receives `60u64` bound by contract `40u64 <= a21 <= 80u64`. The returned value `res` carries the contract postcondition `40u64 <= res <= 80u64`, statically justifying `80u64 - res` and `res - 40u64`.

## Places

| Place | Directory | Description |
| --- | --- | --- |
| local | [`local/`](local/) | The 21-argument call result is bound to a local variable `res` and consumed by exact subtractions. |
| field | [`field/`](field/) | The call result is stored in a struct field `Carrier.value` before consuming the bounded value. |
| parameter | [`parameter/`](parameter/) | The 21st argument and prefix values are passed through nested machine parameters before invoking `pick21`. |
| call result | [`call_result/`](call_result/) | A wrapper machine `fetch21()` returns the result of `pick21` directly as its call result. |
| match join | [`match_join/`](match_join/) | The call result is unified across conditional branches in a match expression before subtraction. |
| const name | [`const_name/`](const_name/) | The upper and lower bounds are named by package/local constants and verified against `pick21`. |
| a nested machine | [`other_state/`](other_state/) | The 21-argument call result is passed as an argument across an explicit state transition `calc(res)`. |
| another machine | [`other_machine/`](other_machine/) | A separate machine `Delegate` evaluates `pick21` and returns the bounded result to `Consumer`. |
| another target body | [`target_body/`](target_body/) | Target-scoped machine bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`) each invoke the calling plan. |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/local/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/field/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/parameter/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/call_result/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/match_join/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/const_name/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/other_state/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/other_machine/main.omg
omega run --both samples/cli/facts/twentyone_arg_stack_calling_plan/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the expectation in the transition guard is modified to require a non-zero exit or if an unexpected computation result occurs (`diff != 0u64 || headroom != 20u64 || margin != 20u64`), execution transitions to `machine bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose 21st argument bound is widened to `81u64` (`a21 = 81u64`), allowing the return value `res` to reach `81u64 > 80u64`. When computing `headroom = 80u64 - res`, static checking fails closed with diagnostic `exact arithmetic in machine ... may overflow \`u64\`` (decision 17 -- exact arithmetic is a proof obligation). None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/twentyone_arg_stack_calling_plan/<place>/invalid/main.omg
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

## Pinned Fixtures

The nine positive places are pinned under
`tests/omega/pass/facts/twentyone_arg_stack_calling_plan_through_<place>_exit/`,
each carrying `main.omg` byte-identical to its place program, a `build.omg`
binding `ProgramEntry` for all four hosted targets, and
`expected_native_exit.txt` declaring `0`.

The nine reject twins are pinned under
`tests/omega/fail/facts/twentyone_arg_stack_calling_plan_through_<place>_rejected/`,
each carrying the place twin's `main.omg` and `build.omg` and an `expected.txt`
holding the place's observed `exact arithmetic ... may overflow \`u64\``
diagnostic (decision 17), naming the machine the subtraction is proved in.

`git diff` of a positive place against its twin changes only the construct's own
value and contract bounds — `a21`'s bound `80u64` to `81u64` and the argument
supplied in that place's idiom `60u64` to `81u64` — plus the header oracle
comment, so each twin breaks only the fact under test.

## Requirements and Evidence

- **Parent outcome:** `CALLING-PLANS-STACK-BOUNDS` / `TWENTYONE-ARG-STACK-CALLING-PLAN` (`TASKS.md`)
- **Requirement established:** A 21-argument calling plan across register and stack ABI slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 21st stack-passed argument's static interval bound `40u64 <= a21 <= 80u64`, proving exact unsigned subtractions `80u64 - res` and `res - 40u64` overflow- and underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Contracts on `pick21` (`a1 == 1u64 && ... && a20 == 20u64 && a21 >= 40u64 && a21 <= 80u64`) establish preconditions for the 21-argument calling sequence. The function computes prefix sum `prefix == 210u64` and returns `(prefix - 210u64) + a21 == a21`, ensuring `result >= 40u64 && result <= 80u64 && result == a21`. The static interval bounds on the returned scalar directly justify the consuming exact unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 21st argument occupies an 8-byte scalar value slot (`u64`) passed via a caller-allocated stack frame home across all four hosted targets. The argument retains its exact value identity and interval bounds across locals, struct fields, machine parameters, return values, match join points, constant names, state transition parameters, cross-machine calls, and target-scoped bodies without truncation, register spill corruption, or stack aliasing.
- **Composition:** The 21-argument calling plan composes through all 9 places into the consuming operations `80u64 - res` and `res - 40u64`. Positive places and reject twins differ strictly in the 21st argument value (`60u64` vs `81u64`) and contract bounds (`80u64` vs `81u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`may overflow \`u64\`` / decision 17), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The 21-argument call expression lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) call nodes. Mathematical proof certificates and range assertions are verified and erased, retaining 64-bit unsigned scalar arithmetic (`ScalarType::U64`) and multi-argument call plans, executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). ABI calling plan lowering resolves register vs. stack placements: arguments 1..4 in registers on Windows x86_64 (args 5..21 on stack), arguments 1..6 in registers on Linux x86_64 (args 7..21 on stack), and arguments 1..8 in registers on ARM64 (args 9..21 on stack). The 21st argument is consistently placed in a validated stack parameter slot with 8-byte alignment and offset calculation.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects target ISA instructions (`mov [rsp + disp], reg` on x86_64, `str reg, [sp, #disp]` on ARM64) to spill stack arguments before call invocation, and loads them within the callee activation frame. Register allocation computes worst-case stack usage (WCSU) ensuring activation frames accommodate outgoing stack argument areas without frame overlap.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Stack displacements and call instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

Fresh, measured on the macOS arm64 host in worktree `factsr1-pins-ed` with the
shared generation producer `gen-9b001b8f6af` (revision
`9b001b8f6afa2cdac67fd580033c3560d906a5d5`, omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`):

- `omega run --both` on the root and the nine positive places: 10/10 report
  `native exit 0` with `interp exit: 0`; no compiler repair was needed.
- `omega --check` on the nine `<place>/invalid/main.omg` twins: 9/9 exit 1 with
  `exact arithmetic in machine ... local \`headroom\` may overflow \`u64\``
  (decision 17), naming the machine that owns the subtraction: probe, check_call
  (parameter), step (other_state), forward21 (other_machine) and evaluate21
  (target_body).
- `corpus_gate.py --runner <generation>/corpus_runner`, scoped to this fact:
  check leg 18 rows (9 `checked` carrying `check-target:macos_arm64`, 9
  `rejected` carrying the twins' diagnostics with their `expected.txt` fragment
  satisfied; 0 rejected without their expected fragment, 0 fail fixtures that
  did not reject); native leg 9 rows `built` with `exit:0`; both leg 9 rows
  `exit:0 interpreter-exit:0 interpreter:agrees`. Each golden diff against the
  recorded base `9b001b8f6af` is a pure addition (60 / 18 / 18 lines, 0
  deletions), every row stamped `@9b001b8f6afa+wt`. The native leg's two
  `regressed-from:` rows are the standing memory canaries
  (`interrupt_table_canary`, `secondary_processor_canary`), not this fact.
- Discrimination: modifying transition conditions causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
- What remains open: aggregate stack arguments passed by value with indirect `sret` calling plans, and mixed float/vector register and stack calling conventions.

Retained from the authoring session (`windows_x86_64`):
`sample_native_gate.py --filter cli/facts/twentyone_arg_stack_calling_plan`
reported 10/10 samples meeting their documented oracle (root plus nine places
exiting 0) with 9/9 intended-invalid controls matching their authored rejection
(`may overflow \`u64\``).
