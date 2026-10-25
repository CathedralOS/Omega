# Unsigned 64-Bit Wrapping Subtraction Bound

// Fact: 64-bit unsigned wrapping subtraction lhs -% rhs with bounded interval 40u64 <= diff <= 100u64 establishes 40u64 <= diff <= 100u64 across all nine places so exact subtractions 100u64 - diff and diff - 40u64 are safe.

Every place performs the same operation: given bounded unsigned 64-bit inputs `lhs` (`80u64..120u64`) and `rhs` (`20u64..40u64`), cast both into `u64 in Wrapping`, evaluate wrapping subtraction `diff_wrap: u64 in Wrapping = w_lhs - w_rhs`, cast back to exact `diff: u64 = diff_wrap as u64`, transport `diff` through the place, and discharge exact subtractions `headroom: u64 = 100u64 - diff` and `margin: u64 = diff - 40u64` returning `0i32`, which `main` verifies with `transition probe() == 0` exiting `0`. Only the transport mechanism of the wrapping difference changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit unsigned wrapping subtraction evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the wrapping difference is copied into a local binding before discharging subtraction bounds |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the wrapping difference is stored and loaded through a `DiffContainer` record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the wrapping difference is forwarded as an explicit machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the wrapping difference is returned from a callee invocation (`sub_wrapping`) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the wrapping difference so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants `LHS` (`100u64`) and `RHS` (`30u64`) supply operands for wrapping subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the wrapping difference to successor machine `step` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the wrapping difference through an intermediate machine call (`forward`) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 64-bit unsigned wrapping subtraction and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `(lhs as u64 in Wrapping) - (rhs as u64 in Wrapping)` with concrete inputs `100u64` and `30u64` establishes difference `70u64` within static interval `40u64..100u64`, discharging exact subtractions `100u64 - diff` (`30u64`) and `diff - 40u64` (`30u64`) and returning `0i32`. In `main`, `transition probe() == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition probe() == 0` to `transition probe() == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the upper bound to allow `diff = 101u64` (`lhs <= 121u64`, `diff <= 101u64`, or `LHS = 121u64`, `RHS = 20u64`), causing the consuming exact subtraction `100u64 - diff` to fail closed at compile time under the **Arithmetic safety** obligation class with `may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_64bit_wrapping_subtraction_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Arithmetic safety | `may overflow \`u64\`` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/unsigned_64bit_wrapping_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_64bit_wrapping_subtraction_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_64bit_wrapping_subtraction_bound_through_target_body_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 64-bit unsigned (`u64 in Wrapping`) wrapping subtraction interval bounds ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). Specifically, 64-bit unsigned wrapping subtraction `(lhs as u64 in Wrapping) - (rhs as u64 in Wrapping)` on bounded inputs (`lhs >= 80u64 && lhs <= 120u64 && rhs >= 20u64 && rhs <= 40u64`) statically establishes `40u64 <= diff <= 100u64` upon casting back to `u64`, discharging downstream exact subtractions `100u64 - diff` and `diff - 40u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Performing a 64-bit unsigned wrapping subtraction `(lhs as u64 in Wrapping) - (rhs as u64 in Wrapping)` on bounded operands (`lhs >= 80u64 && lhs <= 120u64` and `rhs >= 20u64 && rhs <= 40u64`) statically establishes the bounded difference interval `[80 - 40, 120 - 20] = [40, 100]` upon casting back to `u64`. Because all candidate outputs lie strictly in `[40, 100]`, the operation stays within non-wrapping `u64` values and directly proves that subsequent exact 64-bit unsigned subtractions `let headroom: u64 = 100u64 - diff` and `let margin: u64 = diff - 40u64` are safe and free from unsigned underflow without requiring any post-hoc caller equality guard (`transition x == 70`).
  - **Identity and lifetime:** The unsigned 64-bit difference scalar preserves its 64-bit value and refined interval `[40, 100]` across local variables, struct fields (`DiffContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
  - **Composition:** The verified interval `[40, 100]` composes seamlessly across all nine places, allowing consuming exact subtractions `headroom: u64 = 100u64 - diff` and `margin: u64 = diff - 40u64` to execute safely without post-hoc caller guards or dynamic checks. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the construct's input bound or contract (`120u64`/`100u64` vs. `121u64`/`101u64`).
  - **Soundness boundaries:** Wrapping integer arithmetic operates in the modulo $2^{64}$ domain where arithmetic wrapping is non-trapping; casting back to exact `u64` retains static interval knowledge derived from non-wrapping operand ranges. In all 9 invalid twins, widening bounds allows values up to `101u64`, causing the consuming exact subtraction `100u64 - diff` to fail closed at compile time with `may overflow \`u64\``.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives wrapping subtraction operations (`Sub` on `Wrapping` unsigned 64-bit carrier `u64`) and domain casts with interval arithmetic certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Unsigned 64-bit integers (`u64` / `u64 in Wrapping`) settle into 8-byte layouts (`size_of = 8`, `align_of = 8`). Standard calling conventions place parameters in 64-bit register lanes (`RCX`/`RDX` on Windows `x86_64`, `RDI`/`RSI` on Linux `x86_64`, `X0`/`X1` on `arm64`) and return values in `RAX` / `X0`.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** On `x86_64`, backend lowering selects 64-bit `sub` instructions on 64-bit register lanes (`rax`, `rcx`, `rdx`), relying on standard hardware modular 2's complement arithmetic. On `arm64`, backend lowering selects 64-bit `sub` on `X` registers (`x0`, `x1`).
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all four hosted targets.

- **What remains open:** Wrapping subtraction intervals straddling the modulo $2^{64}$ wraparound boundary cannot establish a single contiguous non-wrapping interval and conservatively widen to the full `u64` domain.
