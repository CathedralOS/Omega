# Checked assembly register contract bound

A checked `asm` block (or target-scoped checked assembly helper) with explicit register operand bindings and contract (`result <= 96u64`) establishes the post-assembly bound, discharging exact subtraction `96u64 - result`.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the post-assembly bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | checked assembly helper establishes bound and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | checked assembly result bound to a local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | checked assembly result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | checked assembly result passed as an explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | checked assembly result returned across call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | checked assembly result preserved across match convergence join |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | checked assembly result subtracted against named constant |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | checked assembly result passed as an argument across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | checked assembly result forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | checked assembly result consumed across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `CHECKED-ASM-LABEL-CONTRACTS` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place checked `asm` block register operand binding, label contract verification, and post-assembly range bound establishment, verifying that inline checked assembly with explicit register bindings and destination label contracts (`done requires eax <= 96`) statically establishes the postcondition bound (`result <= 96u64`), discharging exact subtraction `96u64 - result`.

Full 8-layer specification and lowering evidence:

- **Establishment:** The checked `asm` block executes fixed 32-bit GPR move `mov eax, 36;` and branch `jmp done;` targeting label `done requires eax <= 96:`. The compiler verifies the incoming edge against the modeled register state (`eax == 36`), proving destination contract satisfaction. The subsequent move `mov val, eax` stores the register into mutable local `val: u32`, establishing the static upper bound on the cast result `val as u64 <= 96u64`. The bound directly proves non-negativity for exact subtraction `96u64 - val` without requiring any post-hoc caller equality guard (`transition diff == 60u64` checks the runtime value `96 - 36`, rather than manufacturing the proof premise).
- **Identity and lifetime:** The referent identity and modeled register state attach strictly to the activation context of the machine. The register value `eax` is captured, verified, and stored into local storage before the assembly block completes. The resulting scalar `u64` value preserves its upper-bound invariant `result <= 96u64` across local variable binding, struct field projection, parameter passing, machine call returns, match joins, constant bounds, state transitions, machine forwarding, and target-scoped execution bodies.
- **Composition:** The post-assembly bound composes across all 9 standard places to statically discharge exact subtraction `96u64 - result`:
  - `local`: checked assembly result is bound to a local variable `val` before subtracting from `96u64`.
  - `field`: checked assembly result is stored in `Carrier.val` record field and projected out.
  - `parameter`: checked assembly result is passed to machine parameter `compute(val: u64)` with `requires val <= 96u64`.
  - `call_result`: helper machine `forward()` returns the checked assembly result with `ensures result <= 96u64`.
  - `match_join`: checked assembly results from match arms converge to a unified upper bound.
  - `const_name`: subtraction bound is defined by compile-time named constant `const LIMIT: u64 = 96u64`.
  - `other_state`: checked assembly result is passed to state `evaluate(val: u64)` across a transition edge.
  - `other_machine`: checked assembly result and subtraction are forwarded through helper machine `forward()`.
  - `target_body`: checked assembly and exact subtraction are executed across target-scoped bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the assembly label and postcondition bound to `eax <= 97` / `result <= 97u64` (and writing `97`), proving that exact subtraction `96u64 - result` strictly fails closed when the post-assembly bound permits underflow.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine ... may overflow `u64``, confirming that the arithmetic proof engine soundly detects that when `result` can be 97, `96u64 - result` may underflow `u64`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. The checked `asm` CFG decodes into immutable value versions and ordinary checked state edges, validating label contracts before erasing proof formal tags. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 32-bit and 64-bit scalar layout, while `CallPlan`/`StatePlan` assign register and stack homes for passing values across calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for checked inline assembly (`mov`, `jmp`), 64-bit subtraction (`sub`), and equality comparisons. Backend register allocation assigns register homes and disjoint stack activation frames, inserting entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements and jump targets resolve into concrete instruction bytes. Basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute natively with Terminal Psi agreement (`omega run --both` reports `native exit 0` and `interp exit: 0`) across host platforms.

Remaining open under CHECKED-ASM-LABEL-CONTRACTS: dynamic multi-register clobber sets, 64-bit GPR alias bit slices, floating-point/vector register constraints, and arbitrary hardware flag dependencies.
