# unsigned_16bit_exact_subtraction_bound

// Fact: 16-bit unsigned exact subtraction lhs - rhs with 300u16 <= lhs <= 500u16 and 50u16 <= rhs <= 100u16 establishes 200u16 <= diff <= 450u16 so 450u16 - diff and diff - 200u16 are safe.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `field` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `parameter` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `call_result` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `match_join` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `const_name` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `other_state` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `other_machine` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `target_body` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |

## Pinned Fixtures

The row is pinned by 18 fixtures:
`tests/omega/pass/facts/unsigned_16bit_exact_subtraction_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_16bit_exact_subtraction_bound_through_<place>_rejected` for each of the
nine places. Each positive fixture is its place program copied byte-identical, binds `ProgramEntry` for all
four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and carries
`expected_native_exit.txt` = 0; each reject twin is the place's invalid program and `build.omg` copied
byte-identical, with `expected.txt` carrying the row's own `may overflow \`u16\`` fragment (`verify_state`
in state `step` for `other_state`; the four target-scoped bodies of `target_body` report four diagnostics).

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
- **Requirement established:** Target-neutral compile-time 16-bit unsigned exact subtraction bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, exact subtraction (`-`) of bounded 16-bit unsigned `u16` values (`lhs >= 300u16 && lhs <= 500u16`, `rhs >= 50u16 && rhs <= 100u16`) establishes the exact post-subtraction interval (`diff >= 200u16 && diff <= 450u16`), discharging exact 16-bit subtractions `450u16 - diff` and `diff - 200u16` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).
- **Pinned fixtures:** the 18 fixtures named above; nine `checked` and nine `rejected` rows in `tests/omega/corpus_outcomes.txt`, nine `built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`, all stamped `@9b001b8f6afa+wt`.
- **Establishment:** Performing an exact 16-bit unsigned `u16` subtraction of bounded inputs (`lhs >= 300u16 && lhs <= 500u16` and `rhs >= 50u16 && rhs <= 100u16`) statically establishes the synthesized interval bounds `[300 - 100, 500 - 50] = [200, 450]`, fitting safely within the 16-bit unsigned range `[0, 65535]`. This post-subtraction interval directly establishes that exact subtractions `450u16 - diff` (`headroom`) and `diff - 200u16` (`margin`) are provably free from underflow or overflow. Widening operand intervals (e.g. `lhs <= 501u16` or `rhs <= 150u16`, allowing `diff = 451u16`) causes `450u16 - diff` to fail closed at compile time with `may overflow \`u16\``.
- **Identity and lifetime:** The unsigned `u16` difference scalar preserves its 16-bit value and refined interval across local bindings, struct fields (`DiffContainer`), parameter slots, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval `[200, 450]` composes across all nine places, allowing consuming exact subtractions `headroom: u16 = 450u16 - diff` and `margin: u16 = diff - 200u16` to execute safely without post-hoc caller guards.
- **Soundness boundaries:** Exact integer subtraction requires static proof that `lhs >= rhs` (non-underflow) and result representability in `u16`. In invalid twins, widening operands breaks the upper bound requirement, correctly triggering compile-time rejection (`may overflow \`u16\``) rather than runtime modular wrapping or silent overflow.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives exact 16-bit integer subtraction operations (`Sub` on unsigned 16-bit carriers `u16`) with interval arithmetic certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Unsigned 16-bit integers (`u16`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`). Standard calling conventions place parameters in 16-bit register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `AX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 16-bit `sub` instructions on 16-bit register halves (`ax`, `cx`, `dx`) or zero-extended 32-bit registers. On `arm64`, backend lowering selects 32-bit `sub` on `W` registers with 16-bit halfword truncation/masking (`uxth`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
