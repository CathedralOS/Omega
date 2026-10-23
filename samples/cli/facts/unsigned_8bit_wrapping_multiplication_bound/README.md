# Unsigned 8-bit wrapping multiplication bound

Fact: 8-bit unsigned wrapping multiplication lhs *% rhs with bounded interval 40u8 <= prod <= 150u8 establishes 40u8 <= prod <= 150u8 across all nine places so exact subtractions 150u8 - prod and prod - 40u8 are safe.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the 8-bit unsigned wrapping multiplication bound through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | 8-bit unsigned wrapping multiplication establishes bound and discharges subtractions directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | wrapping multiplication result bound to local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | wrapping multiplication result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | wrapping multiplication result passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | wrapping multiplication result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | wrapping multiplication result converged across match join arms |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | wrapping inputs provided by named compile-time constants |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | wrapping multiplication result forwarded across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | wrapping multiplication result forwarded through intermediate machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | wrapping multiplication result verified across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Pinned Corpus Fixtures

The row is pinned under
`tests/omega/pass/facts/unsigned_8bit_wrapping_multiplication_bound_through_<place>_exit`
and
`tests/omega/fail/facts/unsigned_8bit_wrapping_multiplication_bound_through_<place>_rejected`.
Each positive fixture is a byte-identical copy of its `<place>/main.omg`, the
place's `build.omg` (binding `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `macos_arm64` and `linux_arm64`) and `expected_native_exit.txt`
(`0`); it records native exit 0 with Terminal Psi agreement. Each reject twin is
a byte-identical copy of its `<place>/invalid/main.omg` and `build.omg`; the twin
widens only the two wrapping inputs (`lhs = 12, rhs = 8` -> `lhs = 16, rhs = 10`,
so `prod` reaches `160u8 > 150u8`) and pins the corpus-runner-emitted fragment of
its own place in `expected.txt`:

| Place | `expected.txt` reject fragment |
| --- | --- |
| local | `cannot prove requires contract for call verify_prod` |
| field | `cannot prove requires contract for call verify_prod` |
| parameter | `cannot prove requires contract for call check_param` |
| call result | `cannot prove ensures contract for exit from produce_prod` |
| match join | `cannot prove requires contract for call verify_prod` |
| const name | `cannot prove requires contract for call verify_prod` |
| another state | `cannot prove requires contract for call verify_prod` |
| another machine | `cannot prove requires contract for call runner` |
| another target body | `cannot prove requires contract for call exercise` |

## Requirements and Evidence

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `INSTRUCTION-SELECTION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** 8-bit unsigned integer wrapping multiplication arithmetic bounds proof and realization (`Psi 05–07` / `Omega 00–09`), verifying that wrapping multiplication `lhs * rhs` (denoted `lhs *% rhs`) on bounded `u8 in Wrapping` operands produces a bounded product `40u8 <= prod <= 150u8`, which proves that exact 8-bit unsigned subtractions `150u8 - prod` and `prod - 40u8` are provably overflow-free across all nine standard places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The 8-bit unsigned wrapping multiplication `lhs * rhs` on `lhs: u8 in Wrapping` and `rhs: u8 in Wrapping` operates in the wrapping arithmetic domain, allowing modulo-256 wrapping without trapping. With bounded operands `lhs = 12` and `rhs = 8`, the evaluated product `96u8` resides strictly in `[40, 150]` without wrapping around 256. The consuming helper `verify_prod(prod: u8) -> i32 requires prod >= 40 && prod <= 150` discharges exact unsigned subtractions `150 - prod` (evaluating to 54) and `prod - 40` (evaluating to 56), verifying `headroom + margin == 110` without post-hoc caller guards.
- **Identity and lifetime:** The evaluated 8-bit unsigned scalar preserves its invariant `[40, 150]` across all standard value carriers: local bindings, record fields, machine parameters, call return results, match convergence joins, compile-time named constants, state machine transition arguments, intermediate machine forwarding, and target-scoped execution bodies.
- **Composition:** The wrapping multiplication bound composes across all 9 standard places to satisfy the consuming requirement `prod >= 40 && prod <= 150` and discharge `150 - prod` and `prod - 40`:
  - `local`: wrapping multiplication result is bound to a local variable `local_prod` before verification.
  - `field`: wrapping multiplication result is stored in `ProdCarrier.prod` record field and projected out.
  - `parameter`: wrapping multiplication result is passed to machine parameter `check_param(val: u8)`.
  - `call_result`: helper machine `produce_prod()` computes wrapping multiplication and returns the result across call boundary with ensures contract.
  - `match_join`: wrapping multiplication result converges through match arms into a unified value.
  - `const_name`: inputs `LHS` and `RHS` are declared as compile-time named constants `const LHS: u8 in Wrapping = 12; const RHS: u8 in Wrapping = 8;`.
  - `other_state`: wrapping multiplication result is passed across a transition edge into successor state `step(forwarded: u8)`.
  - `other_machine`: wrapping multiplication result is forwarded through intermediate machine `runner()`.
  - `target_body`: wrapping multiplication result is evaluated and verified across target-scoped machine bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in widening inputs to `lhs = 16, rhs = 10` (where `prod` reaches `160u8 > 150u8`), proving that downstream contract verification `requires prod >= 40 && prod <= 150` strictly fails closed when the post-wrapping product exceeds 150.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification: the corpus check route records each as `rejected` with its authored fragment satisfied, and `omega run --both` refuses at checking (exit 200) with `cannot prove requires contract for call verify_prod`/`check_param`/`runner`/`exercise` (the call-result twin: `cannot prove ensures contract for exit from produce_prod`), confirming that the proof engine soundly detects that when `prod` can be 160, contract satisfaction cannot be established.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations with explicit `WrappingIntegerMultiply` operations without overflow trapping assertions. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`) across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 1-byte `u8` scalar storage and alignment, while calling plans assign 8-bit register homes (`al`/`dil`/`sil` on x86_64, `w0`/`w1` on arm64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA unsigned multiplication instructions: 8-bit multiplication on `x86_64` and `mul` on `arm64` truncated to 8 bits. Register allocation assigns register homes and activation frames honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct PE/ELF/Mach-O images are emitted with valid machine-state footprint evidence, executing natively and agreeing with Terminal Psi (`omega run --both` reports `native exit 0` and `interp exit: 0`) across all hosted targets.

Remaining open under NUMERIC-VALUES: dynamic runtime multiplication overflow certificates, 16-bit to 64-bit multi-precision wrapping multiplication, and SIMD packed wrapping multiplication instructions.
