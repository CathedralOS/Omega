# ieee_float_32bit_ordered_bound

// Fact: a 32-bit IEEE f32 ordered comparison guard (reading >= 1.25f32 && reading <= 2.75f32) establishes a finite, non-NaN sub-interval that satisfies a downstream contract requiring reading >= 1.0f32 && reading <= 3.0f32 across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_f32_window` |

## Requirements and Evidence

Owning requirement: `wiki/spec/language/numeric_values.md#bounds-as-propositions`, `wiki/spec/language/numeric_values.md#ieee-comparisons`, and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`.
Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).
This row establishes that a 32-bit IEEE `f32` ordered comparison guard establishing `reading >= 1.25f32 && reading <= 2.75f32` refines a finite, non-NaN sub-interval that strictly satisfies a downstream contract requiring `reading >= 1.0f32 && reading <= 3.0f32` across all nine places.

- **Establishment:** Constructing a bounded `Reading` (`data Reading where value >= 1.25f32 && value <= 2.75f32`) or declaring a contract `requires reading >= 1.25f32 && reading <= 2.75f32` establishes a closed, finite 32-bit IEEE `f32` interval. Because `1.25f32 >= 1.0f32` and `2.75f32 <= 3.0f32`, this interval strictly entails the downstream precondition `reading >= 1.0f32 && reading <= 3.0f32` at `verify_f32_window(reading)`. No post-hoc equality guard is required.
- **Identity and lifetime:** The IEEE `f32` scalar value preserves its exact 32-bit IEEE 754 single-precision bit pattern (1 sign bit, 8-bit exponent, 23-bit fraction) across register passing and memory storage. It retains non-NaN status, sub-interval validity, and value identity across local bindings, struct fields (`Holder`), parameter passing, machine calls, match arms, and target-scoped bodies without precision drift or mutation.
- **Composition:** The narrow 32-bit float interval `[1.25f32, 2.75f32]` composes directly through all nine program places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), satisfying the consuming contract `[1.0f32, 3.0f32]` and returning exit code 0. In each place, the positive and reject twin differ solely in the upper bound (`2.75f32` vs. `3.5f32`), keeping all other operations identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: widening the upper bound to `3.5f32` violates the precondition `reading <= 3.0f32`, causing the compiler to reject strictly with `error: cannot prove requires contract for call verify_f32_window ... (needs fact \`reading >= 1.0 && reading <= 3.0\`)`. Rejecting unproved wider float intervals prevents out-of-range or NaN values from bypassing downstream contracts.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** 32-bit float interval bounds and contract propositions lower into target-neutral Terminal Psi (`PSITERM\0`), maintaining ordered IEEE comparison semantics (`OrderedFloatLessThanOrEqual`, `OrderedFloatGreaterThanOrEqual`) and erasing proof-only obligation tags. The Terminal Psi interpreter evaluates the float parameter passing and verification call in exact agreement with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Companion build bindings join `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Standard calling conventions (`CallPlan`) assign 32-bit floating-point registers: `XMM0` on `windows_x86_64` and `linux_x86_64`, `S0` (the lower 32 bits of `V0`) on `macos_arm64` and `linux_arm64`. Value layout geometry for primitive `f32` (`size_of = 4`, `align_of = 4`) and `Reading` validates.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** The backend selects IEEE 754 32-bit ordered comparison instructions: on `x86_64`, `ucomiss` with parity flag check (`jp`/`jbe`) ensuring unordered/NaN results branch to rejection, and scalar move instructions (`movss`); on `arm64`, `fcmp s` setting condition codes (checked via `b.ls`/`b.hi` respecting unordered/NaN) and `fmov` (single-precision). Register homes in 32-bit floating-point registers and stack frames with 4-byte/8-byte alignment are assigned.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Relocation fixups resolve symbolic displacements and float literal byte-pool references, basic blocks and branch offsets reflow across entry and return epilogues, transitive machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and native executable binaries (PE/ELF/Mach-O) are emitted, passing native execution and Terminal Psi agreement (`omega run --both`, exit 0) across all hosted targets.
