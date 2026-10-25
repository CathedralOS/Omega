# Signed 32-Bit Saturating Arithmetic Bound

Fact: 32-bit signed saturating addition lhs +| rhs with bounded interval 50i32 <= sum <= 100i32 establishes 50i32 <= sum <= 100i32 across all nine places so exact subtractions 100i32 - sum and sum - 50i32 are safe.

Every place performs the same operation: add bounded 32-bit signed operands (`lhs in [40, 80]`, `rhs in [10, 20]`) via saturating addition in domain `i32 in Saturating`, establishing the synthesized post-addition interval `sum in [50, 100]`, discharge both headroom subtraction `100i32 - sum` and margin subtraction `sum - 50i32`, verify `headroom + margin == 50i32`, and exit 0. Only the transport mechanism of the operands and saturating addition computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit signed saturating addition, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 32-bit signed operands are copied into locals before saturating addition and verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 32-bit signed operands are stored in record fields before saturating addition and verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine parameter receives the 32-bit signed saturating sum before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | callee function computes saturating addition and returns the 32-bit signed sum before verification |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 32-bit operands and join before saturating addition and verification |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 32-bit operands before saturating addition and verification |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 32-bit signed operands before saturating addition and verification |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 32-bit signed operands to saturating addition |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies adds bounded 32-bit signed operands with saturation and verifies |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_saturating_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 60i32` and `rhs == 15i32`, `sum == 75i32`, `headroom == 25i32`, `margin == 25i32`, and `headroom + margin == 50i32`. The check `total - 50i32` evaluates to `0i32`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. If 32-bit signed saturating addition or exact subtractions evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `lhs` upper bound is widened to `81i32` so `sum` can reach `101i32 > 100i32`. Proof obligation checking statically detects that `sum <= 100` cannot be proved on the consuming call `verify_sum`, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_32bit_saturating_arithmetic_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_32bit_saturating_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (each place's own exact `cannot prove requires contract for call <site>: sum >= 50 && sum <= 100` fragment).

## Pinned Fixtures

The row is pinned by 18 fixtures under `tests/omega/pass/facts/signed_32bit_saturating_arithmetic_bound_through_<place>_exit` and
`tests/omega/fail/facts/signed_32bit_saturating_arithmetic_bound_through_<place>_rejected` — nine positives and their nine reject
twins, one per transport place. Each positive fixture is a byte-identical copy of its place program, binds `ProgramEntry` for all
four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and records native exit 0. Each reject
twin is a byte-identical copy of its place's `invalid/main.omg` and pins that place's own exact diagnostic
(`cannot prove requires contract for call <call site>: <sum bound>`). Golden rows live in
`tests/omega/corpus_outcomes.txt` (all 18), `tests/omega/corpus_native_macos_arm64.txt` and `tests/omega/corpus_both_macos_arm64.txt`
(the nine positives each).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 32-bit signed saturating addition bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, saturating addition (`+` in `i32 in Saturating`) on bounded 32-bit signed `i32` values (`lhs >= 40i32 && lhs <= 80i32`, `rhs >= 10i32 && rhs <= 20i32`) establishes the exact post-addition bound (`sum >= 50i32 && sum <= 100i32`), discharging exact 32-bit signed subtractions `100i32 - sum` and `sum - 50i32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[50, 100]` for `sat_sum = a + b` in domain `i32 in Saturating` from input intervals `[40, 80]` and `[10, 20]`. The synthesized bounds prove `sum >= 50i32` and `sum <= 100i32` unconditionally and guarantee sum fits within `i32` (`>= -2147483648 && <= 2147483647`), discharging the precondition of `verify_sum` and forming exact subtractions `100i32 - sum` and `sum - 50i32` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 32-bit signed augend, addend, and saturating sum interval bounds retain their precision, two's complement signed representation, domain tag, and interval invariants across copies into local variables, record fields (`SatPacket`, `SumBox`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 32-bit signed saturating addition composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming verification operation, evaluating to `0i32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 80i32` vs `lhs <= 81i32`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 81i32`: interval synthesis establishes `sum in [50, 101]`, so precondition `sum <= 100` on the consuming call fails proof. Compilation strictly rejects with `cannot prove requires contract for call`. No unproved out-of-contract call reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 32-bit signed saturating addition and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `SaturatingIntegerAdd` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 32-bit stored width (4 bytes, align 4) for `i32` and record layouts (`SatPacket` size 8, align 4; `SumBox` size 4, align 4) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 32-bit arguments and return values to 32-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit signed saturating addition sequences (`add r32, r32` with signed overflow check / `cmovno` or branchless clamping on `x86_64`, `adds w, w, w` with signed saturation or `sqadd` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector signed saturating addition across packed 32-bit integer lanes, signed saturating multiplication (`i32 in Saturating` multiplication), and dynamic saturation bounds.
