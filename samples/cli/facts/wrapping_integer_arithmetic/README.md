# Wrapping integer arithmetic

Masking an unsigned scalar with a constant bitmask (`val & 63u64`) establishes the upper bound (`masked <= 63u64`), discharging exact subtraction `63u64 - masked`.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the bitwise mask range bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | bitwise mask establishes bound and discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | masked value bound to a local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | masked value stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | masked value passed as an explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | masked value returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | masked value preserved across match convergence join |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | masked value constrained against named constant |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | masked value passed as an argument across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | masked value forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | masked value consumed across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `INTEGER-ARITHMETIC-CERTIFICATES` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place bitwise mask range synthesis, modular arithmetic bound propagation, and exact integer arithmetic proof obligation discharge for unsigned scalar bitwise AND (`val & 63u64`).

Full 8-layer specification and lowering evidence:

- **Establishment:** The bitwise AND operation `val & 63u64` on unsigned integer `val: u64` with constant bitmask `63u64` (binary `0b0011_1111`) directly establishes the static range upper bound `[0, 63]`. The static range analysis synthesizer deduces that the bitwise AND of any non-negative integer with mask $M = 2^k - 1$ is bounded above by $M$, establishing `masked <= 63u64` without requiring any post-hoc caller equality guard (`transition diff == 27u64` checks the runtime value against expected subtraction `63 - 36`, rather than manufacturing the proof premise).
- **Identity and lifetime:** The established range invariant `[0, 63]` attaches to the exact value produced by `val & 63u64`. The `u64` scalar value preserves its range bounds across local variable binding, aggregate record field storage and projection, explicit machine parameter passing, machine call return values, match join unification, constant expressions, state machine transition argument handoff, machine forwarding chains, and target-scoped body selection.
- **Composition:** The bitwise mask range bound composes across all 9 standard places to statically discharge exact subtraction `63u64 - masked`:
  - `local`: bitwise-masked value is bound to a local variable `masked` before subtracting from `63u64`.
  - `field`: bitwise-masked value is stored in `Carrier.masked` record field and projected out.
  - `parameter`: bitwise-masked value is passed to `compute(masked: u64)` with `requires masked <= 63u64`.
  - `call_result`: helper machine `mask_value(val)` ensures `result <= 63u64` and is consumed across the call boundary.
  - `match_join`: divergent match branches (masking with `63u64` and `31u64`) converge to upper bound `63u64`.
  - `const_name`: bitmask is provided by compile-time named constant `const MASK: u64 = 63u64`.
  - `other_state`: bitwise-masked value is passed to state `evaluate(masked: u64)` across a transition edge.
  - `other_machine`: bitwise-masked value and subtraction are forwarded through helper machine `forward(val)`.
  - `target_body`: bitwise masking and subtraction are executed across target-scoped bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in breaking the mask/bound from `63u64` to `64u64`, proving that exact subtraction `63u64 - masked` is strictly dependent on the established upper bound.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine ... may overflow `u64``, confirming that the arithmetic proof engine soundly detects that when mask `64u64` is used, the value can be 64, making `63u64 - masked` underflow `u64`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower without loss into canonical `PSITERM\0` target-neutral representations. The bitwise AND and exact subtraction lower into canonical operations with verified integer certificates, and erased proof terms are eliminated while runtime execution agrees between Terminal Psi interpretation and native execution (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 64-bit scalar layout (8 bytes, alignment 8) and record layout, while `CallPlan`/`StatePlan` assign register and stack homes for scalar parameters and returns.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for 64-bit bitwise AND (`and`), 64-bit subtraction (`sub`), and equality comparisons. Backend register allocation assigns register homes and disjoint stack activation frames, inserting entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack displacements resolve into concrete offsets. Basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute natively with Terminal Psi agreement (`omega run --both` reports `native exit 0` and `interp exit: 0`) across host platforms.

Remaining open under INTEGER-ARITHMETIC-CERTIFICATES: non-power-of-two bitwise masks requiring multi-interval SMT reasoning, dynamic variable bitmask range propagation, and non-linear bitwise combinations.
