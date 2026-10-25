# unsigned_16bit_bitwise_and_mask_bound

// Fact: 16-bit unsigned bitwise AND mask raw & 63u16 with bounded interval 40u16 <= masked <= 63u16 establishes 40u16 <= masked <= 63u16 across all nine places so exact subtractions 63u16 - masked and masked - 40u16 are safe.

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

The row is pinned under `tests/omega/pass/facts/unsigned_16bit_bitwise_and_mask_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_16bit_bitwise_and_mask_bound_through_<place>_rejected` fixtures across all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`).
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

### The oracle discriminates

- In every positive place, `probe()` evaluates to `0i32` and `transition probe() == 0` enters `machine done() {}`, exiting 0 in both Terminal Psi and native execution. Perturbing the transition guard by a single token (`transition probe() == 0` -> `transition probe() == 1` in `main.omg`) causes both engines to trap at runtime (`native exit -1073741795`, `interp crash: Trap (host presentation agrees)`).
- Every invalid twin widens the mask or contract bound so `res` can exceed `63u16`, causing exact subtraction `63u16 - res` to fail closed at compile time with `may overflow \`u16\``.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Obligation class:** 16-bit unsigned bitwise AND mask interval bound ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md), [domains](../../../../wiki/spec/language/domains.md)).
- **Establishment ([invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md)):** Performing a 16-bit unsigned bitwise AND `raw & 63u16` on bounded input `raw == 116u16` (or within contract places where `40u16 <= masked <= 63u16` is bounded by mask `63u16` whose bit pattern `00000000_00111111_2` guarantees the upper 10 bits are zero and the result cannot exceed $63$, and where $116 \& 63 = 52$ satisfies $40 \le 52 \le 63$) statically establishes the bounded interval `[40, 63]`. This directly proves that subsequent exact 16-bit unsigned subtractions `let headroom: u16 = 63u16 - res` and `let margin: u16 = res - 40u16` are safe and free from underflow. Widening the mask to `127u16` (yielding $116 \& 127 = 116$) or relaxing contract bounds (e.g. allowing `res <= 64u16`) causes `63u16 - res` to fail closed at compile time with `may overflow \`u16\``.
- **Identity and lifetime ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md)):** The unsigned 16-bit masked scalar preserves its 16-bit value and refined interval `[40, 63]` across local variables, struct fields (`MaskContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
- **Composition ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md)):** The verified interval `[40, 63]` composes seamlessly across all nine places, allowing consuming exact subtractions `headroom: u16 = 63u16 - res` and `margin: u16 = res - 40u16` to execute safely without post-hoc caller guards or dynamic checks.
- **Soundness boundaries ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md)):** Bitwise AND operations on unsigned scalars truncate upper bits and compute bitwise conjunction. The compiler derives interval bounds from operand bitmasks ($0 \le x \& M \le M$). When invalid controls widen masks or postcondition bounds to allow values $> 63$, subsequent exact subtraction fails closed at compile time (`may overflow \`u16\``) rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`, [product](../../../../wiki/spec/terminal-psi/product.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)):** Target-neutral Terminal Psi (`PSITERM\0`) receives bitwise AND operations (`BitAnd` on unsigned 16-bit carrier `u16`) with integer bounds certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`, [provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)):** Unsigned 16-bit integers (`u16`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`). Standard calling conventions place parameters in 16-bit register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `AX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`, [storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md)):** On `x86_64`, backend lowering selects 16-bit `and` instructions on 16-bit register halves (`ax`, `cx`, `dx`) with immediate mask values. On `arm64`, backend lowering selects 32-bit `and` with immediate bitmask on `W` registers.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission, [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md)):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all four hosted targets.
