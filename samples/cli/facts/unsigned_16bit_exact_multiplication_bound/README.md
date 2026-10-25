# Unsigned 16-Bit Exact Multiplication Bound

Fact: a 16-bit unsigned u16 exact multiplication of bounded inputs (lhs >= 20u16 && lhs <= 30u16, rhs >= 15u16 && rhs <= 25u16) establishes prod >= 300u16 && prod <= 750u16 so exact 16-bit subtractions 750u16 - prod and prod - 300u16 are proved overflow-free across all nine places.

Every place performs the same operation: multiply bounded 16-bit unsigned operands (`lhs in [20, 30]`, `rhs in [15, 25]`) via exact multiplication `lhs * rhs`, establishing the synthesized post-multiplication interval `prod in [300, 750]`, discharge both headroom subtraction `750u16 - prod` and margin subtraction `prod - 300u16`, verify `headroom + margin == 450u16`, and exit 0. Only the transport mechanism of the operands and multiplication computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit unsigned exact multiplication, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 16-bit operands are copied into locals before multiplication and subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 16-bit operands are stored in record fields before multiplication and subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the bounded 16-bit operands before multiplication and subtractions |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 16-bit operands before multiplication and subtractions |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 16-bit operands and join before multiplication and subtractions |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 16-bit operands before multiplication and subtractions |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 16-bit operands before multiplication and subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards bounded 16-bit operands to the multiplication machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies multiplies bounded 16-bit operands and subtracts |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 25u16` and `rhs == 20u16`, `prod == 500u16`, `headroom == 250u16`, `margin == 200u16`, and `headroom + margin == 450u16`. The transition `probe() == 450 { true -> done() _ -> bad() }` exits 0. If 16-bit multiplication or exact subtractions evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `rhs` upper bound is widened to `26u16` so `prod` can reach `780u16 > 750u16`. Proof obligation checking statically detects that `750u16 - prod` can underflow below zero, failing closed with `may overflow \`u16\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_16bit_exact_multiplication_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omegauild\omega-latest.exe --filter cli/facts/unsigned_16bit_exact_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u16\``).

## Pinned Fixtures

The row is pinned by 18 fixtures:
`tests/omega/pass/facts/unsigned_16bit_exact_multiplication_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_16bit_exact_multiplication_bound_through_<place>_rejected` for each of the
nine places. Each positive fixture is its place program copied byte-identical, binds `ProgramEntry` for all
four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and carries
`expected_native_exit.txt` = 0; each reject twin is the place's invalid program and `build.omg` copied
byte-identical, with `expected.txt` carrying the row's own `may overflow \`u16\`` fragment (`mul_state` in
state `step` for `other_state`; the four target-scoped bodies of `target_body` report four diagnostics).

Every row was recorded with the shared generation producer (gen-9b001b8f6af, revision 9b001b8f6af, macOS
arm64 host, jobs 4) on 2026-10-23, anchored at that base with the fixture and golden changes staged, so
each row is stamped `@9b001b8f6afa+wt`:

- `tests/omega/corpus_outcomes.txt`: nine `checked` rows carrying `check-target:macos_arm64` and nine
  `rejected` rows carrying each twin's own diagnostic line with its `may overflow \`u16\`` fragment
  satisfied;
- `tests/omega/corpus_native_macos_arm64.txt`: nine `built` rows with `exit:0`;
- `tests/omega/corpus_both_macos_arm64.txt`: nine `built` rows with `exit:0`, `interpreter-exit:0` and
  `interpreter:agrees`.

Measured with the same base producer before the record: the root and all nine places run `omega run --both`
with `native exit 0` and `interp exit: 0` (10/10), and all nine twins reject with that fragment (9/9).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 16-bit unsigned exact multiplication bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, exact multiplication (`*`) on bounded 16-bit unsigned `u16` values (`lhs >= 20u16 && lhs <= 30u16`, `rhs >= 15u16 && rhs <= 25u16`) establishes the exact post-multiplication bound (`prod >= 300u16 && prod <= 750u16`), discharging exact 16-bit subtractions `750u16 - prod` and `prod - 300u16` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[300, 750]` for `prod = lhs * rhs` from input intervals `[20, 30]` and `[15, 25]`. The synthesized bounds prove `prod >= 300u16` and `prod <= 750u16` unconditionally and guarantee product fits within `u16` (`<= 65535`), justifying forming exact subtractions `750u16 - prod` and `prod - 300u16` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 16-bit scalar multiplicand, multiplier, and product interval bounds retain their precision, 16-bit stored width, and non-negative invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 16-bit unsigned exact multiplication composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations, evaluating to `450u16` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`rhs <= 25u16` vs `rhs <= 26u16`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `rhs <= 26u16`: interval synthesis establishes `prod in [300, 780]`, so exact subtraction `750u16 - prod` can underflow `u16` below zero (`bounds [-30, 450] lie outside [0, 65535]`). Compilation strictly rejects with `may overflow \`u16\``. Multiplication is statically certified against 16-bit overflow (`prod <= 65535`). No unproved underflow or overflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 16-bit unsigned exact multiplication, subtraction, and overflow contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while certifying 16-bit exact arithmetic certificates and preserving exact integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 16-bit stored width (2 bytes, align 2) for `u16` and record layouts (`MulPacket` size 4, align 2) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 16-bit arguments and return values to 16-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 16-bit multiplication instructions (`imul r16, r16` on `x86_64`, 16-bit masked `mul w` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening 16-bit products into 32-bit registers without intermediate overflow checks, saturating multiplication (`u16 in Saturating`), and SIMD packed 16-bit integer multiplication.
