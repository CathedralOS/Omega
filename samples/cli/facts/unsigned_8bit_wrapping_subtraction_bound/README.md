# Unsigned 8-Bit Wrapping Subtraction Bound

Fact: 8-bit unsigned wrapping subtraction lhs -% rhs with bounded interval 60u8 <= diff <= 160u8 establishes 60u8 <= diff <= 160u8 across all nine places so exact subtractions 160u8 - diff and diff - 60u8 are safe.

Every place performs the same operation: subtract bounded 8-bit unsigned operands (`lhs in [100, 180]`, `rhs in [20, 40]`) via wrapping subtraction in domain `u8 in Wrapping`, establishing the synthesized post-subtraction interval `diff in [60, 160]`, discharge both headroom subtraction `160u8 - diff` and margin subtraction `diff - 60u8`, verify `headroom + margin == 100u8`, and exit 0. Only the transport mechanism of the operands and wrapping subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit unsigned wrapping subtraction, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 8-bit operands are copied into locals before wrapping subtraction and verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 8-bit operands are stored in record fields before wrapping subtraction and verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine parameter receives the 8-bit unsigned wrapping difference before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | callee function computes wrapping subtraction and returns the 8-bit difference before verification |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 8-bit operands and join before wrapping subtraction and verification |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 8-bit operands before wrapping subtraction and verification |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives bounded 8-bit operands before wrapping subtraction and verification |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 8-bit operands to wrapping subtraction |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies subtracts bounded 8-bit operands with wrapping and verifies |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 140u8` and `rhs == 30u8`, `diff == 110u8`, `headroom == 50u8`, `margin == 50u8`, and `headroom + margin == 100u8`. The transition `probe() == 100 { true -> done() _ -> bad() }` exits 0. If 8-bit unsigned wrapping subtraction or exact subtractions evaluate incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `lhs` upper bound is widened to `181u8` so `diff` can reach `161u8 > 160u8`. Proof obligation checking statically detects that `value <= 160` cannot be proved on the consuming call `verify_diff`, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_8bit_wrapping_subtraction_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_8bit_wrapping_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Pinned Corpus Fixtures

The row is pinned under
`tests/omega/pass/facts/unsigned_8bit_wrapping_subtraction_bound_through_<place>_exit`
and
`tests/omega/fail/facts/unsigned_8bit_wrapping_subtraction_bound_through_<place>_rejected`.
Each positive fixture is a byte-identical copy of its `<place>/main.omg`, the
place's `build.omg` (binding `ProgramEntry` for all four hosted targets) and
`expected_native_exit.txt` (`0`); it records native exit 0 with Terminal Psi
agreement. Each reject twin is a byte-identical copy of its
`<place>/invalid/main.omg` and `build.omg`; the twin widens only the `lhs` upper
bound (`<= 180u8` -> `<= 181u8`, so `diff` can reach `161u8 > 160u8`) and pins
`cannot prove requires contract for call` in `expected.txt`, the common prefix of
the nine corpus-runner-emitted diagnostics (`verify_diff` for local, field,
call result, match join, const name and another target body; `consume_param` for
the parameter place; `step` for another state; `forward_wrap` for another
machine).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 8-bit unsigned wrapping subtraction bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, wrapping subtraction (`-` in `u8 in Wrapping`) on bounded 8-bit unsigned `u8` values (`lhs >= 100u8 && lhs <= 180u8`, `rhs >= 20u8 && rhs <= 40u8`) establishes the exact post-subtraction bound (`diff >= 60u8 && diff <= 160u8`), discharging exact 8-bit unsigned subtractions `160u8 - diff` and `diff - 60u8` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[60, 160]` for `wrap_diff = a - b` in domain `u8 in Wrapping` from input intervals `[100, 180]` and `[20, 40]`. The synthesized bounds prove `diff >= 60u8` and `diff <= 160u8` unconditionally and guarantee difference fits within `u8` (`>= 0 && <= 255`), discharging the precondition of `verify_diff` and forming exact subtractions `160u8 - diff` and `diff - 60u8` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 8-bit unsigned minuend, subtrahend, and wrapping difference interval bounds retain their precision, unsigned representation, domain tag, and interval invariants across copies into local variables, record fields (`WrapPacket`, `DiffBox`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 8-bit unsigned wrapping subtraction composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming verification operation, evaluating to `100u8` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 180u8` vs `lhs <= 181u8`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 181u8`: interval synthesis establishes `diff in [60, 161]`, so precondition `value <= 160` on the consuming call fails proof. The corpus check route records each of the nine as `rejected` with the pinned `cannot prove requires contract for call` fragment satisfied, and `omega run --both` refuses at checking (exit 200). No unproved out-of-contract call reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 8-bit unsigned wrapping subtraction and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `WrappingIntegerSubtract` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 8-bit stored width (1 byte, align 1) for `u8` and record layouts (`WrapPacket` size 2, align 1; `DiffBox` size 1, align 1) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 8-bit arguments and return values to 8-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit wrapping subtraction sequences (`sub r8, r8` / `sub al, cl` with modulo 256 byte truncation on `x86_64`, `sub w, w, w` with 8-bit zero-extension mask `uxtb` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector wrapping subtraction across packed 8-bit integer lanes, wrapping multiplication (`u8 in Wrapping` multiplication), and dynamic wrapping intervals.
