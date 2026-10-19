# narrow_to_wide_unsigned_8to32_extension

// Fact: zero-extending a bounded 8-bit unsigned u8 value (narrow >= 40u8 && narrow <= 160u8) to u32 (narrow as u32) preserves the exact interval (wide >= 40u32 && wide <= 160u32), discharging exact 32-bit subtractions 160u32 - wide and wide - 40u32 across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `field` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `parameter` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `call_result` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `match_join` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `const_name` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `other_state` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `other_machine` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |
| `target_body` | passes `omega run --both` exit 0 | `may overflow \`u32\`` |

## Requirements and Evidence

- **Establishment:** Zero-extending a bounded 8-bit unsigned `u8` value (`narrow >= 40u8 && narrow <= 160u8`) to 32-bit unsigned `u32` via `narrow as u32` preserves the numeric value and statically establishes the interval `[40, 160]` on the 32-bit destination. This interval directly discharges the exact unsigned 32-bit subtraction proof obligations for both headroom (`160u32 - wide >= 0`) and margin (`wide - 40u32 >= 0`). Widening the source interval (e.g. `narrow <= 161u8`, allowing `wide = 161u32`) causes `160u32 - wide` to potentially underflow, failing closed at compile time with `may overflow \`u32\``.
- **Identity and lifetime:** The zero-extended unsigned 32-bit integer scalar retains its identity and exact numerical interval across all storage carriers and place projections without loss, unwanted sign extension, or storage corruption.
- **Composition:** The verified interval flows through local variables, record fields (`WideContainer`), machine parameters, call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies, discharging both `160u32 - wide` and `wide - 40u32` across all nine places without post-hoc caller guards.
- **Soundness boundaries:** Exact integer arithmetic requires compile-time proof of non-underflow and non-overflow. In invalid twins, widening the source bound allows values outside `[40, 160]`, correctly triggering compile-time rejection (`may overflow \`u32\``) rather than accepting runtime modular wrap-around.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives the unsigned zero-extension operation (`Cast` from `u8` to `u32`) and subsequent subtraction operations with interval bounds and arithmetic modes intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Zero extension converts a 1-byte unsigned scalar (`size_of = 1, align_of = 1`) into a 4-byte unsigned scalar (`size_of = 4, align_of = 4`). Standard calling conventions place 32-bit parameters in 32-bit register lanes (`ECX` on Windows `x86_64`, `EDI` on Linux `x86_64`, `W0` on `arm64`) and return 32-bit scalars in `EAX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects `movzx r32, r/m8` (zero-extending an 8-bit register or byte memory operand into a 32-bit general-purpose register). On `arm64`, backend lowering selects `uxtb w, w` (unsigned extend byte) or loads 8 bits zero-extended into a 32-bit `W` register. Subsequent subtractions select 32-bit `sub` instructions.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
