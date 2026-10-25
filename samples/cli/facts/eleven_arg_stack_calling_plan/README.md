# Eleven-Argument Stack Calling Plan Bound

Fact: an 11-argument calling plan across register and stack slots preserves the 11th stack argument bound 40u64 <= a11 <= 80u64 to justify 80u64 - res and res - 40u64.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 11-argument stack calling plan bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 11-argument call returns stack argument bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | 11th stack argument result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | 11th stack argument result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | 11th stack argument result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | 11th stack argument result returned across intermediate call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | 11th stack argument result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | 11th stack argument supplied by named compile-time constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | 11th stack argument result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | 11th stack argument result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | 11th stack argument result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Pinned Fixtures

The nine positive places are pinned under:
`tests/omega/pass/facts/eleven_arg_stack_calling_plan_through_<place>_exit/`
each containing `build.omg`, `main.omg`, and `expected_native_exit.txt` (`0`), binding `ProgramEntry` for all 4 hosted targets.

The nine reject twins are pinned under:
`tests/omega/fail/facts/eleven_arg_stack_calling_plan_through_<place>_rejected/`
each containing `build.omg`, `main.omg`, and `expected.txt` (`may overflow `u64``), binding `ProgramEntry` for all 4 hosted targets.

## Discrimination and Obligation Class

This fact's obligation belongs to the **calling plan bound / arithmetic interval bound** obligation class. The calling convention partitions the 11 arguments across register lanes and stack slots, ensuring the 11th stack-passed argument retains its static interval bound `40u64 <= a11 <= 80u64` at runtime.

Discrimination was verified through two independent checks:
1. **Static refusal on construct domain breach**: In all nine `<place>/invalid/main.omg` reject twins, mutating the 11th argument input from `60u64` to `39u64` (outside the required lower bound `40u64`) causes static proof verification (`omega --check`) to fail closed with `error: exact arithmetic in machine ... may overflow `u64`` on the exact subtraction `margin = res - 40;`, refusing compilation and preventing any native binary emission.
2. **Runtime trap on oracle discrimination**: A 1-token runtime perturbation of the demanded equality check in `main()` (`let check: i32 = (total as i32) - 40;` -> `let check: i32 = (total as i32) - 41;`) reaches native execution, evaluates `check == 0` to false, takes the `bad()` transition edge, and crashes `Trap`. Both engines trap: native execution exits non-zero (`exit 3221225501 != 0` on Windows STATUS_BREAKPOINT trap) and Terminal Psi interpreter crashes with `Trap`.

| Perturbation | Reaches Native? | Native Engine | Terminal Psi |
| --- | --- | --- | --- |
| 11th argument `a11 = 39u64` (violates bound) | No (checker refuses) | `may overflow `u64`` | `may overflow `u64`` |
| Demanded check `total - 41` (runtime perturbation) | Yes | `exit 3221225501` (trap) | `crash: Trap` |


## Requirements and Evidence

- **Parent outcome:** `CALLING-PLANS` / `ACTIVATION-STORAGE` / `STORAGE` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 11-argument stack calling plan bounds preservation and realization (`Psi 05–07` / `Omega 00–09`), verifying that an 11-argument scalar call across register slots and stack spill slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) preserves the 11th stack-passed argument's static interval bound `40u64 <= a11 <= 80u64` to justify exact subtractions `80u64 - res` and `res - 40u64` across all nine places.

Full 8-layer specification and lowering evidence:

- **Establishment:** An 11-argument scalar call `pick11(a1..a11)` receives 11 `u64` arguments. On all hosted platform ABIs, argument 11 is passed in a stack slot: on `windows_x86_64` (4 register slots RCX/RDX/R8/R9, stack slots 5..11), on `linux_x86_64` (6 register slots RDI/RSI/RDX/RCX/R8/R9, stack slots 7..11), and on `macos_arm64`/`linux_arm64` (8 register slots X0..X7, stack slots 9..11). With concrete caller input `a11 = 60u64`, the static interval bound `40u64 <= a11 <= 80u64` is preserved across the stack calling convention boundary and returned without loss, safely discharging exact subtractions `80u64 - res` (evaluating to 20) and `res - 40u64` (evaluating to 20) with `headroom + margin == 40` without post-hoc caller guards.
- **Identity and lifetime:** The returned 11th stack argument value preserves its static identity and interval bound `[40, 80]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The 11-argument stack calling plan bound composes across all 9 standard places to satisfy downstream exact subtraction safety obligations `80u64 - res` and `res - 40u64`:
  - `local`: 11th stack argument result is bound to a local variable `local_res` before subtraction.
  - `field`: 11th stack argument result is stored in `ResCarrier.res` record field and projected out.
  - `parameter`: 11th stack argument result is passed to machine parameter `check_param(val: u64)`.
  - `call_result`: helper machine `produce_pick()` invokes `pick11` and returns the result across call boundary.
  - `match_join`: 11th stack argument result converges through match arms into a unified value.
  - `const_name`: input `a11` is declared as compile-time named constant `const A11: u64 = 60;`.
  - `other_state`: 11th stack argument result is passed across a transition edge into successor machine `step(forwarded: u64)`.
  - `other_machine`: 11th stack argument result is forwarded through intermediate machine `runner()`.
  - `target_body`: 11th stack argument result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in setting `a11 = 39u64` (where `res` becomes `39u64 < 40u64`), proving that downstream exact subtraction `res - 40u64` strictly fails closed during proof checking due to potential underflow.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine ... may overflow \`u64\``, confirming that the proof engine soundly detects that when `a11` is 39, `res - 40u64` underflows `u64`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source multi-argument calls lower into canonical `PSITERM\0` target-neutral representations with explicit multi-operand call actions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target calling plans partition the 11 arguments into register homes and stack spill slots: Windows assigns args 1..4 to registers and 5..11 to stack slots at `[RSP+32..RSP+80]`; Linux x86-64 assigns args 1..6 to registers and 7..11 to stack slots; ARM64 assigns args 1..8 to X0..X7 and 9..11 to stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering allocates caller outgoing stack frame parameter slots for the 11th argument, and callee loads incoming argument 11 from its positive frame displacement. Activation storage accounts for the caller's stack frame reservation and preserves callee frame alignments.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under CALLING-PLANS: aggregate struct argument splitting across register/stack boundaries, indirect large struct return pointers (`sret`), and floating-point multi-argument register/stack partition rows.
