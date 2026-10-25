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
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | checked assembly result passed as an argument across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | checked assembly result forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | checked assembly result consumed across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Pinned fixtures

The sample folder and its fixture family do not share a name. Each place is
pinned under `tests/omega/` as
`checked_asm_register_contract_bound_through_<place>_{exit,rejected}`, following
the sibling `dyn_dispatch_selector_payload_bound_*` and `length_*` rows:

| Place | Positive fixture | Reject twin fixture |
| --- | --- | --- |
| local | [pass/facts/checked_asm_register_contract_bound_through_local_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_local_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_local_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_local_rejected/main.omg) |
| field | [pass/facts/checked_asm_register_contract_bound_through_field_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_field_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_field_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_field_rejected/main.omg) |
| parameter | [pass/facts/checked_asm_register_contract_bound_through_parameter_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_parameter_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_parameter_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_parameter_rejected/main.omg) |
| call result | [pass/facts/checked_asm_register_contract_bound_through_call_result_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_call_result_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_call_result_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_call_result_rejected/main.omg) |
| match join | [pass/facts/checked_asm_register_contract_bound_through_match_join_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_match_join_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_match_join_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_match_join_rejected/main.omg) |
| const name | [pass/facts/checked_asm_register_contract_bound_through_const_name_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_const_name_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_const_name_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_const_name_rejected/main.omg) |
| a nested machine | [pass/facts/checked_asm_register_contract_bound_through_other_state_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_other_state_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_other_state_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_other_state_rejected/main.omg) |
| another machine | [pass/facts/checked_asm_register_contract_bound_through_other_machine_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_other_machine_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_other_machine_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_other_machine_rejected/main.omg) |
| another target body | [pass/facts/checked_asm_register_contract_bound_through_target_body_exit](../../../../tests/omega/pass/facts/checked_asm_register_contract_bound_through_target_body_exit/main.omg) | [fail/facts/checked_asm_register_contract_bound_through_target_body_rejected](../../../../tests/omega/fail/facts/checked_asm_register_contract_bound_through_target_body_rejected/main.omg) |

Each pass fixture copies its place's `main.omg` byte for byte, declares
`Expected exit: 0`, binds `ProgramEntry` for all four hosted targets, and
carries its own `expected_native_exit.txt`. Each reject twin copies its
place's `invalid/main.omg` and authors its expectation in `invalid/`-style
fragments (`exact arithmetic`, `` may overflow `u64` ``), which the corpus
runner weighs against the refusal it actually produced.

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

## Run record

Producer: `omega` built from this candidate, `--build-profile` reporting
`{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`. Host:
`windows_x86_64`. Command per place: `omega run --both <place>/main.omg`.

| Place | Positive (`--both`) | Reject twin (`--check`) |
| --- | --- | --- |
| root | `native exit 0` / `interp exit: 0` | — |
| local | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| field | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| parameter | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `compute` state `entry` terminal expression may overflow `u64` `` |
| call result | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| match join | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| const name | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `entry` local `diff` may overflow `u64` `` |
| a nested machine | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `main` state `evaluate` local `diff` may overflow `u64` `` |
| another machine | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `forward` state `entry` terminal expression may overflow `u64` `` |
| another target body | `native exit 0` / `interp exit: 0` | rejected: ``exact arithmetic in machine `exercise` state `entry` terminal expression may overflow `u64` `` |

This row established coverage, not a compiler repair: all ten positive
projects already ran green on the base, and the gap was that no host-pinned
golden row had been recorded. No compiler source changed.

## Register-contract discrimination

The obligation's class decides where a violation is caught. For this fact the
bound is a **value/arithmetic bound** (`result <= 96u64`) established from a
checked assembly destination label contract (`done requires eax <= 96`) and
discharged by exact subtraction (`96u64 - result`), so weakening the bound
surfaces at the consuming subtraction while violating the register bound inside
the assembly block surfaces at the destination label gate.

- **Runtime, register operand binding is load-bearing.** Two probes differ in
  exactly one token, the immediate loaded into `eax`. `mov eax, 36;` (so `diff`
  is `96 - 36 == 60`) gives `native exit 0` / `interp exit: 0`. Perturbing to
  `mov eax, 37;` (still satisfying `eax <= 96` and `result <= 96u64`, so `diff`
  is `59`) traps both engines identically: `native exit -1073741795` (`0xC000001D`
  illegal instruction trap on `windows_x86_64`) and `interp crash: Trap (host
  presentation agrees)`. The native and interpreted executions evaluate the
  actual register move into local `val` rather than folding a constant.
- **Static, the post-assembly bound is proof information.** `96u64 - result` is
  provable only because `result <= 96u64` is carried across each transport place.
  Weakening the assembly label contract and postcondition in `<place>/invalid/main.omg`
  to `done requires eax <= 97:` and `ensures result <= 97u64` (loading `97`)
  refuses statically at the consuming subtraction with
  ``exact arithmetic ... may overflow `u64` ``.
- **Static, at the assembly label contract gate.** Perturbing `mov eax, 36;` to
  `mov eax, 97;` while keeping `done requires eax <= 96:` refuses before native
  realization with ``cannot prove asm `requires` fact at block entry in read_asm
  state `entry` (statement 6): asm#scratch#4#eax$version$1 <= 96``, confirming
  that label entry obligations are verified against modeled register values.
