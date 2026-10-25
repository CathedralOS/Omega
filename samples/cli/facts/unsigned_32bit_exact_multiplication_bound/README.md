# Fact: 32-Bit Unsigned Exact Multiplication Bound

> a 32-bit unsigned u32 exact multiplication of bounded inputs (lhs >= 20u32 && lhs <= 30u32, rhs >= 15u32 && rhs <= 25u32) establishes prod >= 300u32 && prod <= 750u32 so exact 32-bit subtractions 750u32 - prod and prod - 300u32 are proved overflow-free across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 32-bit unsigned exact multiplication bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | product is copied into an intermediate local binding before exact interval subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Factors`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns product with postcondition contract `ensures result >= 300u32 && result <= 750u32` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | product is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants define operand intervals and bound limits |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `check_state(p: u32)` receives product and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the product to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform multiplication and interval subtractions |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_exact_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 25u32` (bounded `[20u32, 30u32]`) and `rhs = 20u32` (bounded `[15u32, 25u32]`).
- The product interval bounds are statically proved: `min = 20 * 15 = 300u32`, `max = 30 * 25 = 750u32 <= u32::MAX (4294967295)`.
- Exact multiplication `lhs * rhs` is proved mathematically overflow-free without requiring wrapping mode.
- Consuming exact subtractions `let headroom: u32 = 750u32 - prod` (`250u32`) and `let margin: u32 = prod - 300u32` (`200u32`) are proved non-negative and underflow-free from the interval `[300u32, 750u32]`.
- At runtime, `prod == 500u32 && headroom + margin == 450u32` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin widens the `rhs` bound to `rhs <= 26u32`, allowing `prod` to reach `30 * 26 = 780u32 > 750u32`. The headroom subtraction `750u32 - prod` is no longer provably underflow-free, causing the compiler to fail closed at compile time with `may overflow \`u32\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_exact_multiplication_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_32bit_exact_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).

## Pinned Fixtures

The row is pinned by 18 fixtures:
`tests/omega/pass/facts/unsigned_32bit_exact_multiplication_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_32bit_exact_multiplication_bound_through_<place>_rejected` for each of the
nine places. Each positive fixture is its place program copied byte-identical, binds `ProgramEntry` for all
four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and carries
`expected_native_exit.txt` = 0; each reject twin is the place's invalid program and `build.omg` copied
byte-identical, with `expected.txt` carrying the row's own `may overflow \`u32\`` fragment (`multiply_other_state`
in state `check_state` for `other_state`, `forward_eval` in state `entry` for `other_machine`; the four
target-scoped bodies of `target_body` report four diagnostics).

The rows were first measured on the slot's base generation producer (gen-f1b0d3326d2, omega sha256
4e3950e53eb15fbb3b54868f9c8964e23e7efc5bdced336e07f18eb27e28ac4a) and re-measured on gen-5648483032b
(revision 5648483032b790550a7d772454aa8e20272c0aaf, macOS arm64 host, runner default jobs) after main moved
the contract-entailment checking sources; the diagnostics were byte-identical across the two producers and
every fragment still matched. The three legs were recorded with gen-5648483032b, the tree staged at the
published base f28130ba26ee so each row is stamped `@f28130ba26ee+wt`:

- `tests/omega/corpus_outcomes.txt`: nine `checked` rows carrying `check-target:macos_arm64` and nine
  `rejected` rows carrying each twin's own diagnostic line with its `may overflow \`u32\`` fragment
  satisfied;
- `tests/omega/corpus_native_macos_arm64.txt`: nine `built` rows with `exit:0`;
- `tests/omega/corpus_both_macos_arm64.txt`: nine `built` rows with `exit:0`, `interpreter-exit:0` and
  `interpreter:agrees`.

Measured with these producers before the record: the root and all nine places run `omega run --both`
with `native exit 0` and `interp exit: 0` (10/10), and all nine twins reject with that fragment (9/9).

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` ([TASKS.md](../../../../TASKS.md)): "machine/state parameters `requires`, results `ensures`, locals the facts from initializers, guards and calls". This row is one customer group's nine-place transport and execution of that settled proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying numeric-policy realization.
- **Requirement established:** End-to-end nine-place 32-bit unsigned (`u32`) bounded exact multiplication, interval synthesis (`[300, 750]`), consuming exact subtractions (`750 - prod` and `prod - 300`) overflow/underflow discharge, and native realization agreement.
- **Pinned fixtures:** the 18 fixtures named above; nine `checked` and nine `rejected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`, all stamped `@f1b0d3326d2d+wt`.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires lhs >= 20u32 && lhs <= 30u32` and `requires rhs >= 15u32 && rhs <= 25u32` establish non-negative operand bounds well below `u32::MAX`. Exact unsigned multiplication `lhs * rhs` synthesizes interval `[20 * 15, 30 * 25] = [300, 750]`, directly discharging consuming exact subtractions `750u32 - prod` and `prod - 300u32` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `u32` product scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Factors`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `prod >= 300u32 && prod <= 750u32` is preserved intact.
  - **Composition:** Bounded multiplication composes through calling conventions and control flow. In `call_result`, `compute_prod` formally proves postcondition `ensures result >= 300u32 && result <= 750u32`; in `match_join`, both arms establish identical product intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `rhs` bound is widened to `rhs <= 26u32`. Because the product interval `[300, 780]` admits values above `750u32`, `let headroom: u32 = 750u32 - prod;` fails closed with diagnostic `exact arithmetic in machine ... local headroom may overflow \`u32\``. No underflowing subtraction escapes into code generation.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact unsigned multiplication lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with exact multiplication certificates (`<= u32::MAX`). Proof obligations for bounded multiplication and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 4-byte stored width for `u32` (`size_of = 4`, `align_of = 4`). Calling plans allocate 32-bit registers (`ecx`/`edx` on Windows x86_64, `edi`/`esi` on Linux x86_64, `w0`/`w1` on AArch64) according to target ABIs.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 32-bit unsigned multiplication instructions: on x86_64, two-operand 32-bit multiplication `imul r32, r32` (which computes identical lower 32 bits as unsigned multiplication since high bits are certified zero by static intervals); on AArch64, 32-bit unsigned/modular multiply `mul w_dest, w_lhs, w_rhs`. Register allocation assigns 32-bit register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 64-bit and 128-bit unsigned exact multiplication bounds.
- Signed integer exact multiplication intervals (`i8`, `i16`, `i32`, `i64`).
- Widening multiplication operations producing double-width outputs.
