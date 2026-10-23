# Unsigned 16-bit wrapping arithmetic bound

Fact: 16-bit unsigned wrapping addition lhs +% rhs with bounded interval 350u16 <= sum <= 600u16 establishes 350u16 <= sum <= 600u16 across all nine places so exact subtractions 600u16 - sum and sum - 350u16 are safe.

Every place computes or transports a 16-bit unsigned wrapping sum `sum = (lhs as u16 in Wrapping) + (rhs as u16 in Wrapping)` (`compute_wrapping(400u16, 80u16) == 480u16`) from bounded inputs (`lhs in [300u16..500u16]`, `rhs in [50u16..100u16]`), statically establishing the interval `sum in [350u16..600u16]`. That static bound proves the consuming exact 16-bit unsigned subtractions `let headroom: u16 = 600u16 - sum;` (`120u16`) and `let margin: u16 = sum - 350u16;` (`130u16`) are overflow- and underflow-free across all nine language places without runtime arithmetic overflow checks or traps.

Only the transport mechanism of the bounded wrapping sum changes across the nine places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism | Pinned Pass Fixture | Pinned Reject Fixture |
| --- | --- | --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the bounded wrapping sum is computed and held in a local before the subtraction | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_local_rejected` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bounded wrapping sum is carried in a record field before the subtraction | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_field_rejected` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bounded wrapping sum is passed as an explicit machine parameter with contract | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_parameter_rejected` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the bounded wrapping sum with a guaranteed contract | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_call_result_rejected` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms produce the bounded wrapping sum and join into the subtraction bounds | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_match_join_rejected` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply the bounded operands for wrapping addition | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_const_name_rejected` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the bounded wrapping sum and discharges the subtraction bounds | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_other_state_rejected` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine sends the bounded wrapping sum to the verifying machine | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_other_machine_rejected` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of the four target bodies performs the bounded wrapping addition and discharges the bounds | `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_target_body_rejected` |

## Execution

Each positive place is an application that exits 0, and every positive place (and the root `main.omg`) carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpretation and native compilation:

```sh
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is not an unconstrained default. Perturbing the root oracle's single comparison (`transition probe() == 0` -> `transition probe() == 1`) makes both engines take `state bad() { crash Trap; }` instead of `done()`: the perturbed program exits 255 reporting `native signal 5` and `interp crash: Trap (host presentation agrees)` instead of exit 0. The oracle therefore strictly discriminates between correct evaluation and faulty execution.

### Reject Twins

Each place has an `invalid/` twin that moves only the fact's own bound: `lhs`'s upper bound from `500u16` to `501u16` (or the named constants from `400u16`/`80u16` to `501u16`/`100u16`), so the sum can reach `601u16 > 600u16` and the exact subtraction `600u16 - sum` is no longer provably non-underflowing. Checking fails closed at compile time with `may overflow \`u16\``; none reaches native execution.

```sh
omega --check samples/cli/facts/unsigned_16bit_wrapping_arithmetic_bound/<place>/invalid/main.omg
```

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_16bit_wrapping_arithmetic_bound_through_<place>_exit`
and `tests/omega/fail/facts/unsigned_16bit_wrapping_arithmetic_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its place's
`main.omg`/`build.omg` plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy
of its place twin's `main.omg`/`build.omg`/`expected.txt` (the twin's own `may overflow \`u16\``
fragment). Positive versus twin `git diff` moves only the fact's own bound (`lhs <= 500u16` to
`lhs <= 501u16`, or the constants `400u16`/`80u16` to `501u16`/`100u16`).

Measured on macOS arm64 with the factsr1 shared generation producer `gen-9b001b8f6af` (omega sha256
`6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`, corpus_runner sha256
`cee00bbdce17fc300b548aca249c2e9e2a44e59c17e97186055da76c3947444e`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 exit 1 with their authored `may overflow \`u16\`` fragment
  (`exact arithmetic in machine <machine> state <state> local `headroom` may overflow \`u16\``).
- Perturbing the root oracle (one token: `transition probe() == 0` -> `== 1`) exits 255 with
  `native signal 5` and `interp crash: Trap (host presentation agrees)`.
- `python3 tools/sample_gate.py --cli <generation omega> --filter unsigned_16bit_saturating_arithmetic_bound,unsigned_16bit_wrapping_arithmetic_bound`
  records 38/38 projects checked on Darwin arm64 with 18 of these matched intended rejection controls.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `9b001b8f6afa`, so every row is stamped `@9b001b8f6afa+wt`: `tests/omega/corpus_outcomes.txt` gained
  36 rows (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with their diagnostics and
  expected fragments satisfied, 24 diagnostic continuation lines), `tests/omega/corpus_native_macos_arm64.txt`
  gained 18 `built ... exit:0` rows and `tests/omega/corpus_both_macos_arm64.txt` gained 18
  `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows. Every golden diff is a pure addition
  (0 deletions); the legs exited 0 with 36/36, 18/18 and 18/18 fixtures done, 0 unimplemented rows and
  0 rows recording a lost success.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls. This row is one customer group's nine-place transport and execution of that settled proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 16-bit unsigned wrapping (`u16 in Wrapping`) addition interval synthesis. Specifically, wrapping addition `lhs +% rhs` of bounded operands (`lhs >= 300u16 && lhs <= 500u16`, `rhs >= 50u16 && rhs <= 100u16`) statically establishes `sum >= 350u16 && sum <= 600u16`, discharging the consuming exact subtractions `600u16 - sum` (`headroom`) and `sum - 350u16` (`margin`) across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Performing a 16-bit unsigned wrapping addition `(lhs as u16 in Wrapping) + (rhs as u16 in Wrapping)` on bounded inputs (`lhs >= 300u16 && lhs <= 500u16` and `rhs >= 50u16 && rhs <= 100u16`) statically establishes the bounded interval `[300 + 50, 500 + 100] = [350, 600]` upon casting back to exact `u16` ([numeric values](../../../../wiki/spec/language/numeric_values.md), [domains](../../../../wiki/spec/language/domains.md)). This interval fits strictly within the 16-bit unsigned range `[0, 65535]`, directly proving the downstream exact subtractions `600u16 - sum` (`headroom`, evaluating in `[0, 250]`) and `sum - 350u16` (`margin`, evaluating in `[0, 250]`) underflow- and overflow-free. In invalid twins, widening the operand bounds (`lhs <= 501u16`, allowing `sum = 601u16`) makes `600u16 - sum` fail closed at compile time with `may overflow \`u16\``.
- **Identity and lifetime:** The unsigned 16-bit sum scalar preserves its 16-bit value and refined interval `[350u16, 600u16]` across local bindings, record fields (`SumContainer`), parameter positions, call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies without truncation, unwanted widening, or storage degradation ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md)).
- **Composition:** The verified interval `[350, 600]` composes across all nine places, allowing the consuming exact subtractions `headroom: u16 = 600u16 - sum` and `margin: u16 = sum - 350u16` to execute safely without post-hoc caller guards. Positive place versus twin `git diff` shows only the construct's own bound moved (operand upper bounds, named constants, or the forwarded `requires`/`ensures` interval).
- **Soundness boundaries:** Wrapping integer arithmetic operates in the modulo 2^16 domain where overflow is non-trapping; casting back to exact `u16` retains interval knowledge bounded by the operand ranges. In invalid twins, widening the bounds allows values exceeding `600u16`, correctly triggering compile-time rejection (`may overflow \`u16\``) on the subsequent exact arithmetic rather than accepting unproven bounds ([proof contracts](../../../../wiki/spec/proofs/contracts.md)).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives the wrapping addition (`Add` on the `Wrapping` unsigned 16-bit carrier `u16`), the domain casts, and the exact unsigned subtractions with their interval certificates erased without loss into executable nodes that run in the Terminal Psi interpreter (`omega run --both` exit 0) ([integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md), [product](../../../../wiki/spec/terminal-psi/product.md)).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`); unsigned 16-bit integers (`u16` / `u16 in Wrapping`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`) with parameters in 16-bit register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `AX` / `W0` ([calling plans](../../../../wiki/spec/build/calling_plans.md), [layout plans](../../../../wiki/spec/layouts/plans.md)).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** On `x86_64`, backend lowering selects 16-bit `add` instructions on 16-bit register halves (`ax`, `cx`, `dx`) or zero-extended 32-bit registers, relying on hardware modular arithmetic; on `arm64`, 32-bit `add` on `W` registers with 16-bit halfword truncation/masking (`uxth`). Register homes and activation frames stay within the `StatePlan` ceiling ([storage](../../../../wiki/spec/resources/storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation (`omega run --both` `native exit 0` and `interp exit: 0`) across all hosted targets ([executable installation](../../../../wiki/spec/build/executable_installation.md)).