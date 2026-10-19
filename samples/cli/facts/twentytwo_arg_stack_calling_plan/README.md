# 22-Argument Stack Calling Plan Fact Row

Fact: a 22-argument calling plan across register and stack slots preserves the 22nd stack argument bound 40u64 <= a22 <= 80u64 to justify 80u64 - res and res - 40u64.

Every place invokes or transports arguments through a 22-parameter machine call `pick22(a1, ..., a22)` where `a1..a21` are fixed prefix constants (`1..21`, sum 231) and the 22nd argument `a22` is bounded by `40u64 <= a22 <= 80u64`. Across all four hosted target calling conventions:
- **`windows_x86_64`**: arguments 1..4 reside in registers (`RCX, RDX, R8, R9`), and arguments 5..22 are passed on the caller's stack frame (18 stack slots).
- **`linux_x86_64`** (System V AMD64 ABI): arguments 1..6 reside in registers (`RDI, RSI, RDX, RCX, R8, R9`), and arguments 7..22 are passed on the stack (16 stack slots).
- **`macos_arm64`** / **`linux_arm64`** (AAPCS64): arguments 1..8 reside in registers (`X0..X7`), and arguments 9..22 are passed on the stack (14 stack slots).

On every target, the 22nd argument `a22` is guaranteed to be passed via a stack slot. The machine contract guarantees `result >= 40u64 && result <= 80u64 && result == a22`. This static bound proves that the consuming exact 64-bit unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20) are safe and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the 22-argument calling plan and its bounded stack-passed result changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | 22-argument calling plan is invoked directly in main |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | 22 arguments are held in local variables before invocation |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded call result is stored in a struct field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | 22 arguments arrive as machine parameters and are passed to the call |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | call results supply operands to the multi-argument call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both branches of pattern matching invoke the 22-argument calling plan and join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply arguments to the multi-argument call |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the call result to a successor state before subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate forwarding machine receives arguments and invokes the 22-argument calling plan |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies invoke the 22-argument calling plan and consume the bounded result on each hosted target |

## Requirements and Evidence

- **Parent outcome:** `CALLING-PLANS` / `STORAGE` / `ACTIVATION-STORAGE` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** A 22-argument calling plan across register and stack ABI slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 22nd stack-passed argument's static interval bound `40u64 <= a22 <= 80u64`, proving exact unsigned subtractions `80u64 - res` and `res - 40u64` overflow- and underflow-free without runtime checks across all nine language places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Contracts on `pick22` (`a1 == 1u64 && ... && a21 == 21u64 && a22 >= 40u64 && a22 <= 80u64`) establish preconditions for the 22-argument calling sequence. The function computes prefix sum `prefix == 231u64` and returns `(prefix - 231u64) + a22 == a22`, ensuring `result >= 40u64 && result <= 80u64 && result == a22`. The static interval bounds on the returned scalar directly justify the consuming exact unsigned subtractions `let headroom: u64 = 80u64 - res;` (20) and `let margin: u64 = res - 40u64;` (20), proving absence of underflow without post-hoc caller equality guards.
- **Identity and lifetime:** The 22nd argument occupies an 8-byte scalar value slot (`u64`) passed via a caller-allocated stack frame home across all four hosted targets. The argument retains its exact value identity and interval bounds across locals, struct fields, machine parameters, return values, match join points, constant names, state transition parameters, cross-machine calls, and target-scoped bodies without truncation, register spill corruption, or stack aliasing.
- **Composition:** The 22-argument calling plan composes through all 9 places into the consuming operations `80u64 - res` and `res - 40u64`. Positive places and reject twins differ strictly in the 22nd argument value (`60u64` vs `81u64`) and contract bounds (`80u64` vs `81u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`may overflow \`u64\`` / decision 17), proving that unproved upper bounds are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The 22-argument call expression lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) call nodes. Mathematical proof certificates and range assertions are verified and erased, retaining 64-bit unsigned scalar arithmetic (`ScalarType::U64`) and multi-argument call plans, executing with exact parity in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). ABI calling plan lowering resolves register vs. stack placements: arguments 1..4 in registers on Windows x86_64 (args 5..22 on stack), arguments 1..6 in registers on Linux x86_64 (args 7..22 on stack), and arguments 1..8 in registers on ARM64 (args 9..22 on stack). The 22nd argument is consistently placed in a validated stack parameter slot with 8-byte alignment and offset calculation.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend code generation selects target ISA instructions (`mov [rsp + disp], reg` on x86_64, `str reg, [sp, #disp]` on ARM64) to spill stack arguments before call invocation, and loads them within the callee activation frame. Register allocation computes worst-case stack usage (WCSU) ensuring activation frames accommodate outgoing stack argument areas without frame overlap.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Stack displacements and call instructions encode into valid PE/ELF/Mach-O binaries that natively execute and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/twentytwo_arg_stack_calling_plan`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).
- Discrimination: modifying transition conditions causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
