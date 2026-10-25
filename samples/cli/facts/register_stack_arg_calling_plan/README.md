# Seven-Argument Register and Stack Calling Plan Fact Row

Fact: passing seven bounded integer arguments (`a1` through `a7` with `a7 <= 85u64`) across register and stack ABI slots preserves the stack-passed argument's static bound (`a7 <= 85u64`), discharging exact subtraction `85u64 - a7` across all nine places.

Every place performs the same operation: invoke a seven-argument scalar function `calling_plan_seven(a1..a7)` across register and stack ABI slots (on `windows_x86_64`, arguments 1–4 occupy `rcx, rdx, r8, r9` while arguments 5–7 spill to stack; on `linux_x86_64` System V, arguments 1–6 occupy `rdi, rsi, rdx, rcx, r8, r9` while argument 7 spills to stack; on `macos_arm64` and `linux_arm64`, arguments 1–7 occupy `x0..x6`). The callee preserves the static bound `a7 <= 85u64` across the calling boundary, discharging exact subtraction `85u64 - a7` returning 0. Only the transport mechanism of the operands and call invocation changes across places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | seven distinct arguments are held in locals before passing to the call |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | arguments are stored in and read from record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | seven arguments arrive as explicit parameters and are forwarded directly |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper call results supply the arguments |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms execute and join the multi-argument call |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply all seven register and stack operands |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives all seven arguments and invokes the call |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards all seven arguments |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute on each hosted platform |

Pinned fixture twins, one pair per place:

| Place | Pinned Positive | Pinned Reject Twin |
| --- | --- | --- |
| local | [`..._through_local_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_local_exit/main.omg) | [`..._through_local_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_local_rejected/main.omg) |
| field | [`..._through_field_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_field_exit/main.omg) | [`..._through_field_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_field_rejected/main.omg) |
| parameter | [`..._through_parameter_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_parameter_exit/main.omg) | [`..._through_parameter_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_parameter_rejected/main.omg) |
| call result | [`..._through_call_result_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_call_result_exit/main.omg) | [`..._through_call_result_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_call_result_rejected/main.omg) |
| match join | [`..._through_match_join_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_match_join_exit/main.omg) | [`..._through_match_join_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_match_join_rejected/main.omg) |
| const name | [`..._through_const_name_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_const_name_exit/main.omg) | [`..._through_const_name_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_const_name_rejected/main.omg) |
| a nested machine | [`..._through_other_state_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_state_exit/main.omg) | [`..._through_other_state_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_other_state_rejected/main.omg) |
| another machine | [`..._through_other_machine_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_other_machine_exit/main.omg) | [`..._through_other_machine_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_other_machine_rejected/main.omg) |
| another target body | [`..._through_target_body_exit`](../../../../tests/omega/pass/facts/register_stack_arg_calling_plan_through_target_body_exit/main.omg) | [`..._through_target_body_rejected`](../../../../tests/omega/fail/facts/register_stack_arg_calling_plan_through_target_body_rejected/main.omg) |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/register_stack_arg_calling_plan/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/local/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/field/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/parameter/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/call_result/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/match_join/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/const_name/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/other_state/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/other_machine/main.omg
omega run --both samples/cli/facts/register_stack_arg_calling_plan/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. If the caller expectation in `local` is modified to `transition probe() == 1`, execution traps through `machine bad() { crash Trap; }` with `native signal 5` and `interp crash: Trap`, yielding exit code 255. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin whose 7th/stack-passed argument is widened to `86u64` (violating `a7 <= 85u64`), causing exact subtraction `85u64 - a7` to fail closed at compile time with `may overflow \`u64\``. None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/register_stack_arg_calling_plan/<place>/invalid/main.omg
```

| Place | Reject Twin | Authored Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `may overflow `u64`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `may overflow `u64`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `may overflow `u64`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `may overflow `u64`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `may overflow `u64`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `may overflow `u64`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `may overflow `u64`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `may overflow `u64`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `may overflow `u64`` |

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `CALLING-PLAN-STACK-DISCHARGE` (`TASKS.md`)
- **Requirement established:** Passing multi-argument integer values exceeding target register argument ceilings across ABI calling boundaries preserves static interval bounds (`a7 <= 85u64`) on stack-passed slots, discharging exact subtraction (`85u64 - a7`) across all nine language places without arithmetic underflow.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Caller actuals and caller preconditions (`a1 == 10u64 && ... && a6 == 60u64 && a7 <= 85u64`) establish bounded values across all seven argument positions. The upper bound `a7 <= 85u64` on the 7th argument establishes that `85u64 - a7 >= 0`, discharging exact subtraction without requiring post-hoc equality assertions.
- **Identity and lifetime:** The stack-allocated and register-allocated arguments retain their exact positional correspondence and scalar values across call boundaries, record projections, parameter forwarding, match joins, constant names, state transitions, and target-scoped bodies without corruption or stack-displacement aliasing.
- **Composition:** The seven arguments compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), correctly evaluating prefix sums and consuming the stack-passed argument in exact subtraction `85u64 - a7`. Each positive place `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the bound of the 7th argument (`85u64` vs `86u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed with compile-time verification diagnostics (`error: exact arithmetic in machine ... may overflow `u64``) when the 7th argument is widened to `86u64`, proving that unsafe underflows are soundly rejected during proof checking rather than emitted.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Multi-argument calls lower into canonical target-neutral Terminal Psi (`PSITERM\0`), preserving all seven argument lanes and contract assurances while erasing proof terms, and executing with full semantic parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target `CallPlan` partitions the seven arguments across target-specific register lanes and stack spill lanes: on `windows_x86_64`, arguments 1–4 use `rcx, rdx, r8, r9` and arguments 5–7 occupy stack slots; on `linux_x86_64`, arguments 1–6 use `rdi..r9` and argument 7 occupies a stack slot; on ARM64 targets, all seven arguments occupy registers `x0..x6`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation allocates correct register homes and frame displacement offsets for caller argument pushes and callee stack parameter loads. Callee prologues and epilogues allocate sufficient frame storage conforming to worst-case stack usage (WCSU).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Callee frame displacements, stack argument loads, subtraction, and comparison instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Physical Boundary in Emitted Code

The row exists to cross a real ABI boundary, not a larger count of the same
mechanism. Disassembling `local/main.omg` built for `linux_x86_64` shows the
caller place the first six arguments in registers and the seventh on the stack,
and the callee read it back from a stack slot:

```text
; caller, before the call
movabsq $0xa, %rdi      ; a1
movabsq $0x14, %rsi     ; a2
movabsq $0x1e, %rdx     ; a3
movabsq $0x28, %rcx     ; a4
movabsq $0x32, %r8      ; a5
movabsq $0x3c, %r9      ; a6
movabsq $0x55, %rax     ; a7 = 85
leaq (%rsp), %rbx
movq %rax, (%rbx)       ; a7 crosses into a stack slot
callq ...
; callee
leaq 0x10(%rsp), %rax
movq (%rax), %rax       ; a7 read from the stack slot
; six register sums, subtract 210, then add the stack-loaded a7
; caller return: negq %rax; addq $0x55  ->  85 - a7
```

Arguments 1–6 reach the callee in `rdi, rsi, rdx, rcx, r8, r9`; argument 7
travels through memory. On `macos_arm64` all seven occupy `x0..x6`, so this
row's boundary evidence is strongest on the x86_64 catalogued targets.

### Pinned Fixtures

Nine positive places are pinned as
`tests/omega/pass/facts/register_stack_arg_calling_plan_through_<place>_exit`,
each carrying an authored `expected_native_exit.txt` of `0`. Nine reject twins
are pinned as `tests/omega/fail/facts/register_stack_arg_calling_plan_through_<place>_rejected`,
each carrying `expected.txt` with the fragment ``may overflow `u64` ``.

### Verification Evidence

- `python3 tools/corpus_gate.py --native --filter facts/register_stack_arg_calling_plan`
  reports `clean — 9 fixtures ({'built': 9})`, `executed 9 (exit codes {'0': 9})`,
  with 0 fail fixtures that did not reject. This is the golden row that measures
  the fact: nine real `built ... exit:0` rows in
  `tests/omega/corpus_native_macos_arm64.txt`.
- `python3 tools/sample_native_gate.py --filter cli/facts/register_stack_arg_calling_plan`:
  10/10 samples meet their documented oracle on `macos_arm64` (root + 9 places
  run with exit 0 under both engines); 9/9 intended-invalid controls match their
  authored rejection; 0 reported `rejection-failed`.
- `omega run --both` on each of the root and nine places reports `native exit 0`
  and `interp exit: 0`.
- **Discrimination.** The oracle is not an unconstrained default. Changing the
  caller expectation in `local` from `transition probe() == 0` to
  `transition probe() == 1` leaves the seven-argument call, the register/stack
  placement and the arithmetic untouched, and the observed result changes:
  `native signal 5` and `interp crash: Trap (host presentation agrees)`. Both
  engines trap, so the recorded `exit 0` distinguishes correct execution from
  faulty execution.
- **Static enforcement of the stack argument's bound.** Every attempt to change
  the value that crosses in the seventh argument — passing `84` instead of `85`,
  pairing the threshold, nudging the callee result by one, or doing it in
  `Wrapping` — is refused before native execution by the exact-arithmetic prover
  or the `ensures` check. The stack lane's static bound is therefore enforced at
  compile time, and the runtime oracle is what detects a wrong value reaching
  the guard.
- **Splice safety.** Recording compared fixture keys against the base golden and
  lost zero rows: 45 native keys and 18 outcome keys added, none changed. The
  two `lost success` notices the recorder prints are pre-existing pinned
  regressions on main (1798 non-success rows before and after), not movement
  from this record.
- What remains open: floating-point multi-argument register/stack partitions,
  aggregate struct parameter splitting across the register-to-stack boundary,
  and indirect return homes (`sret`).
