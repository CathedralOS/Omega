# Bounded Scalar Sums

Bounded `u64` operands `a <= 50` and `b <= 50` make the exact sum
`a + b` fit `result <= 100`; the body stays a plain `a + b`, with no
`Wrapping` domain and no `as` widening.

Every place performs the same operation: add two operands whose declared
bound is `<= 50` and return the sum under `result <= 100`. The oracle runs
both the interior `30 + 40 == 70` and the upper endpoint `50 + 50 == 100`
and exits 0 only when both hold. Only the transport of the two operands changes.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded u64 operands a <= 50 and b <= 50 |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded operands are copied into locals before adding |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded record fields carry both operands into the sum |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the addition reads its two bounded parameters directly |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | both operands arrive from calls with declared result ranges |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms keep the operands inside their bounds, and both arms run |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the named ceiling bounds both operands |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives both bounded operands and performs addition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports both bounded operands into the sum |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | every supported target architecture provides its own target-scoped machine body executing addition |

## Execution

Every place (and the root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`,
declares `Expected exit: 0`, and carries `expected_native_exit.txt` containing `0`.
Each is an application that runs under both engines:

```sh
omega run --both samples/cli/facts/bounded_scalar_sums/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/local/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/field/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/parameter/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/call_result/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/match_join/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/const_name/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/other_state/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/other_machine/main.omg
omega run --both samples/cli/facts/bounded_scalar_sums/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The oracle verifies both an interior point `compute(30, 40) == 70` and the upper endpoint `compute(50, 50) == 100`.
If the addition or bounds preservation produces any deviation (e.g. `compute(30, 40) == 71`), the transition guard
`compute(30, 40) == 70 && compute(50, 50) == 100` evaluates to false, transitioning to `bad() { crash Trap; }`,
causing native signal 5 and interpreter crash Trap (exit 255).

### Invalid controls and reject twins

Each place has an `invalid/` twin that widens or drops an operand bound (e.g., removing `requires a <= 50`, or setting `const HIGH: u64 = u64::MAX`) so the exact sum is no longer provably in range.
The control must reject at compile time with decision 17:

```
may overflow `u64`
```

specifically:

```
error: exact arithmetic in machine `add` state `entry` terminal expression may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).
```

Restoring the operand bounds restores clean compilation and zero exit.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`)
- **Requirement family IDs:** `language-numeric-values-001`, `language-numeric-values-006`
- **Ticket:** `7bbdd20aa82e4150ac40f12c4dd79750`
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/bounded_scalar_sums_through_<place>_exit/`
  - Negative controls: `tests/omega/fail/facts/bounded_scalar_sums_through_<place>_rejected/`

### Specification and Lowering Layers

1. **Establishment:** Individual operand upper/lower bounds (`a <= 50`, `b <= 50` with `50 + 50 <= u64::MAX`) establish overflow-free exact scalar addition and the sum's interval bound (`result <= 100`) directly from contract preconditions without any post-hoc caller equality guard (`transition sum == 100`). The caller actuals (`30, 40` and `50, 50`) discharge the callee preconditions statically.
2. **Identity and lifetime:** The operand bounds and resulting sum interval fact stay bound to the exact scalar SSA bindings and storage versions across copy into locals, storage in record fields, parameter forwarding, call return values, match join unification, constant naming, and state transitions across all 9 places.
3. **Composition:** Bounded scalar sums compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact addition/subtraction or index operation. Each positive `<place>/main.omg` differs from its `<place>/invalid/main.omg` reject twin strictly in an operand bound or contract precondition, isolating the arithmetic interval proof obligation.
4. **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `exact arithmetic ... may overflow \`u64\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation)` when an operand bound is widened or omitted, proving that the compiler refuses unverified scalar arithmetic.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Proved exact integer addition operations lower into canonical `PSITERM\0` integer arithmetic instructions (`IntegerMode::Exact`) accompanied by integer certificates while mathematical proof annotations are erased. The lowered representation executes in the Terminal Psi interpreter (`omega run --both`) agreeing with native execution at exit 0.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar types (`u64`) validate standard 8-byte layout geometry and alignment. ABI calling plans assign integer arguments and returns to standard machine registers (`rcx`/`rdx`/`rax` on Windows x86_64, `rdi`/`rsi`/`rax` on Linux x86_64, `x0`/`x1` on ARM64).
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects native integer addition (`add`) and subtraction (`sub`) instructions matching target width, maps bounded scalar values to registers or activation stack-frame slots, accounts for frame size in worst-case stack usage (WCSU), and inserts standard machine entry prologues and epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to target addressing modes, basic blocks and branch targets relax with offset reflow, machine-state footprint evidence validates transitive resource usage, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets (`windows_x86_64` and `macos_arm64`).
