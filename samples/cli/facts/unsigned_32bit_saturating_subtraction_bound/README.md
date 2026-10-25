# Unsigned 32-Bit Saturating Subtraction Bound

Fact: 32-bit unsigned saturating subtraction lhs -| rhs with bounded interval 40u32 <= diff <= 100u32 establishes 40u32 <= diff <= 100u32 across all nine places so exact subtractions 100u32 - diff and diff - 40u32 are safe.

Every place performs the same operation: subtract bounded 32-bit unsigned operands (`lhs in [80, 120]`, `rhs in [20, 40]`) via saturating subtraction in domain `u32 in Saturating`, establishing the synthesized post-subtraction interval `diff in [40, 100]`, discharge both headroom subtraction `100u32 - diff` and margin subtraction `diff - 40u32` inside `verify_diff`, verify `headroom + margin == 60u32`, and exit 0. Only the transport mechanism of the operands and saturating subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit unsigned saturating subtraction, interval verification, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 32-bit operands are copied into locals before saturating subtraction and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 32-bit operands are stored in record fields before saturating subtraction and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 32-bit operands before saturating subtraction and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 32-bit operands before saturating subtraction and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 32-bit operands and join before saturating subtraction and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 32-bit operands before saturating subtraction and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 32-bit operands before saturating subtraction and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 32-bit operands to saturating subtraction |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies subtracts bounded 32-bit operands with saturation and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 100u32` and `rhs == 30u32`, `diff == 70u32`, `headroom == 30u32`, `margin == 30u32`, and `headroom + margin == 60u32`. The transition `probe() == 60 { true -> done() _ -> bad() }` exits 0. Perturbing the transition guard by a single token (`transition probe() == 60` -> `transition probe() == 59` in `main.omg`) causes both engines to trap at runtime (`native exit -1073741795`, `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `lhs` upper bound is widened to `125u32` so `diff` can reach `105u32 > 100u32`. Proof obligation checking statically detects that `verify_diff` contract precondition `value <= 100u32` cannot be satisfied, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_saturating_subtraction_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\swarm\w26\worktrees\w26-6\target\debug\omega.exe --filter cli/facts/unsigned_32bit_saturating_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_32bit_saturating_subtraction_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_32bit_saturating_subtraction_bound_through_<place>_rejected` fixtures across all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`).
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Obligation class:** 32-bit unsigned saturating subtraction interval bound ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md), [domains](../../../../wiki/spec/language/domains.md)).
- **Ticket:** `7ac7391b398f4f0f8efb68cffbad9598`.
- **Requirement established:** Target-neutral compile-time 32-bit unsigned saturating subtraction bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, saturating subtraction (`-` in `u32 in Saturating`) on bounded 32-bit unsigned `u32` values (`lhs >= 80u32 && lhs <= 120u32`, `rhs >= 20u32 && rhs <= 40u32`) establishes the exact post-subtraction bound (`diff >= 40u32 && diff <= 100u32`), discharging exact 32-bit subtractions `100u32 - diff` and `diff - 40u32` inside contracted verifier `verify_diff` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment ([invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md)):** Compile-time interval analysis synthesizes the exact range `[40, 100]` for `sat_diff = a - b` in domain `u32 in Saturating` from input intervals `[80, 120]` and `[20, 40]`. The synthesized bounds prove `diff >= 40u32` and `diff <= 100u32` unconditionally and guarantee diff fits within `u32` (`>= 0 && <= 4294967295`), justifying calling `verify_diff(diff)` with precondition `value >= 40u32 && value <= 100u32` which performs exact subtractions `100u32 - value` and `value - 40u32` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md)):** The 32-bit scalar minuend, subtrahend, and saturating difference interval bounds retain their precision, 32-bit stored width, domain tag, and non-negative invariants across copies into local variables, record fields (`SatPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md)):** The 32-bit unsigned saturating subtraction composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming `verify_diff` operation, evaluating to `60u32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 120u32` vs `lhs <= 125u32`).
  - **Soundness boundaries ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md)):** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 125u32`: interval synthesis establishes `diff in [40, 105]`, so `diff <= 100u32` cannot be proved for call `verify_diff(diff)`. Compilation strictly rejects with `cannot prove requires contract for call`. Subtraction is statically certified against 32-bit underflow and bound violation (`diff <= 100`). No unproved boundary violation reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`, [product](../../../../wiki/spec/terminal-psi/product.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)):** Target-neutral 32-bit unsigned saturating subtraction and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `SaturatingIntegerSub` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`, [provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 32-bit stored width (4 bytes, align 4) for `u32` and record layouts (`SatPacket` size 8, align 4) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 32-bit arguments and return values to 32-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`, [storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md)):** Backend lowering selects target ISA 32-bit saturating subtraction sequences (`sub r32, r32` followed by conditional saturation move `cmovc` to zero on `x86_64`, `subs w, w, w` with `csel` or `uqsub` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission, [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md)):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector saturating subtraction across packed 32-bit integer lanes, signed saturating subtraction (`i32 in Saturating`), and dynamic saturation bounds.
