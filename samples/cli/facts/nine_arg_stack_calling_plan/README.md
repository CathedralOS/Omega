# Nine-Argument Stack Calling Plan Fact Row

Fact: passing nine bounded integer arguments (`a1` through `a9` with `a9 <= 92u64`) across register and stack ABI slots on all four hosted targets (including AArch64 stack overflow past `X0–X7`) preserves the 9th argument's static bound (`a9 <= 92u64`), discharging exact subtraction `92u64 - a9` across all nine places.

Every place performs the same operation: invoke a nine-argument scalar function `calling_plan_nine(a1..a9)` across register and stack ABI slots. While x86_64 targets spill earlier (on `windows_x86_64`, arguments 1–4 occupy `rcx, rdx, r8, r9` and arguments 5–9 spill to stack; on `linux_x86_64` System V, arguments 1–6 occupy `rdi, rsi, rdx, rcx, r8, r9` and arguments 7–9 spill to stack), ARM64 targets (`macos_arm64` and `linux_arm64`) provide exactly eight general-purpose argument registers (`x0..x7`), so the 9th argument `a9` spills to the stack across all four hosted targets. The callee preserves the static bound `a9 <= 92u64` across the calling boundary, discharging exact subtraction `92u64 - a9` returning 0. Only the transport mechanism of the operands and call invocation changes across places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | nine distinct arguments are held in locals before passing to the call |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | arguments are stored in and read from record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | nine arguments arrive as explicit parameters and are forwarded directly |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper call results supply the arguments |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms execute and join the multi-argument call |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply all nine register and stack operands |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives all nine arguments and invokes the call |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards all nine arguments |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute on each hosted platform |

Match the fact sentence beside every place: each positive place is pinned as
`tests/omega/pass/facts/nine_arg_stack_calling_plan_through_<place>_exit` and
each reject twin as
`tests/omega/fail/facts/nine_arg_stack_calling_plan_through_<place>_rejected`,
byte-equal to the place and `invalid/` programs beside them.

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/local/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/field/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/parameter/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/call_result/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/match_join/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/const_name/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/other_state/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/other_machine/main.omg
omega run --both samples/cli/facts/nine_arg_stack_calling_plan/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the caller expectation in `local` is modified to `transition probe() == 1`, execution traps through `state bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose 9th stack-passed argument is widened to `93u64` (violating `a9 <= 92u64`), causing exact subtraction `92u64 - a9` to fail closed at compile time with `may overflow \`u64\``. None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/nine_arg_stack_calling_plan/<place>/invalid/main.omg
```

| Place | Reject Twin | Authored Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `may overflow `u64`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `may overflow `u64`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `may overflow `u64`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `may overflow `u64`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `may overflow `u64`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `may overflow `u64`` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `may overflow `u64`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `may overflow `u64`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `may overflow `u64`` |

## Requirements and Evidence

- **Parent outcome:** `NORMALIZED-ABI-LOWERING` ([TASKS.md](../../../../TASKS.md)) — register/stack argument placement under the [calling-plan contract](../../../../wiki/spec/build/calling_plans.md).
- **Requirement established:** Passing nine integer arguments across ABI calling boundaries on all four hosted targets preserves static interval bounds (`a9 <= 92u64`) on stack-passed spill slots (specifically exercising stack parameter passing beyond AAPCS64's `X0–X7` register window on ARM64 as well as Windows x86_64 and Linux x86_64 stack spill slots), discharging exact subtraction (`92u64 - a9`) across all nine language places without runtime arithmetic underflow.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Caller actuals and caller preconditions (`a1 == 10u64 && ... && a8 == 80u64 && a9 <= 92u64`) establish bounded values across all nine argument positions. The upper bound `a9 <= 92u64` on the 9th argument establishes that `92u64 - a9 >= 0`, discharging exact subtraction without requiring post-hoc equality assertions.
- **Identity and lifetime:** The stack-allocated and register-allocated arguments retain their exact positional correspondence and scalar values across call boundaries, record projections, parameter forwarding, match joins, constant names, state transitions, and target-scoped bodies without corruption or stack-displacement aliasing.
- **Composition:** The nine arguments compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), correctly evaluating prefix sums and consuming the stack-passed argument in exact subtraction `92u64 - a9`. Each positive place `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the bound of the 9th argument (`92u64` vs `93u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed with compile-time verification diagnostics (`error: exact arithmetic in machine ... may overflow `u64``) when the 9th argument is widened to `93u64`, proving that unsafe underflows are soundly rejected during proof checking rather than emitted.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Nine-argument calls lower into canonical target-neutral Terminal Psi (`PSITERM\0`), preserving all nine argument lanes and contract assurances while erasing proof terms, and executing with full semantic parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target `CallPlan` partitions the nine arguments across target-specific register lanes and stack spill lanes: on `windows_x86_64`, arguments 1–4 use `rcx, rdx, r8, r9` and arguments 5–9 occupy stack slots; on `linux_x86_64`, arguments 1–6 use `rdi..r9` and arguments 7–9 occupy stack slots; on ARM64 (`macos_arm64`, `linux_arm64`), arguments 1–8 use `x0..x7` and argument 9 overflows to the stack slot.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation allocates correct register homes and frame displacement offsets for caller argument pushes and callee stack parameter loads. Callee prologues and epilogues allocate sufficient frame storage conforming to worst-case stack usage (WCSU).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Callee frame displacements, stack argument loads, subtraction, and comparison instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/nine_arg_stack_calling_plan`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow `u64``).
- Re-measured on macOS arm64 at producer revision `317bacd12b0a` (shared generation producer, `--jobs 2` over both stack-calling-plan rows): 20/20 samples — both roots and all eighteen places — meet their documented oracle with 0 unscored, and 27/27 intended-invalid controls match their authored rejection (nine 9, ten 18 including its `invalid/<place>/` mirrors). `omega run --both` exits 0 with `native exit 0` and `interp exit: 0` for this row's root and nine places (10/10).
- Pinned record at `50abcdce8fe6+wt`: 18 fixture families (`..._through_<place>_{exit,rejected}`), byte-equal to the sample tree — 9 `checked` rows with `check-target:macos_arm64` and 9 `rejected ... expected` rows carrying the decision-17 `may overflow `u64`` diagnostic in `corpus_outcomes.txt`; 9 `built ... exit:0` rows in `corpus_native_macos_arm64.txt`; 9 `exit:0 interpreter-exit:0 interpreter:agrees` rows in `corpus_both_macos_arm64.txt`. 0 keys lost, 0 timeout rows, 0 rejected without their expected fragment, 0 fail fixtures that did not reject.
- Discrimination: modifying `probe()` expectation from 0 to 1 causes runtime traps with `native signal 5` and `interp crash: Trap` (exit 255), proving the exit 0 oracle actively discriminates computation outcomes.
- What remains open: aggregate struct argument splitting across register/stack boundaries, indirect aggregate (`sret`) return homes, and SIMD/float vector register calling spill plans. The ten-argument stack row carries its own folder and pinned family.
