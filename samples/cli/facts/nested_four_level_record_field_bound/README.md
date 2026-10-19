# nested_four_level_record_field_bound

// Fact: a 4-level nested structural record RootRec { outer: Outer { mid: Middle { inner: Inner { val: u64 } } } } preserves the leaf field bound 40u64 <= root.outer.mid.inner.val <= 80u64 across all nine places so exact subtractions 80u64 - res and res - 40u64 are safe.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `field` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `parameter` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `call_result` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `match_join` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `const_name` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `other_state` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `other_machine` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `target_body` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |

## Requirements and Evidence

- **Establishment:** The 4-level nested structural record `RootRec { outer: Outer { mid: Middle { inner: Inner { val: u64 } } } }` encapsulates an unsigned 64-bit scalar leaf value. In each place, the leaf field is constrained to the interval $40u64 \le root.outer.mid.inner.val \le 80u64$ (instantiated with concrete value $60u64$ in probe). The compiler's path-sensitive typing and structural interval refinement statically prove that projecting through four levels of record access `root.outer.mid.inner.val` retains the bound $[40, 80]$. This statically satisfies the proof obligations for exact subtractions `let headroom: u64 = 80u64 - res;` and `let margin: u64 = res - 40u64;` without arithmetic overflow or underflow. When an invalid twin loosens the upper bound to $81u64$, exact subtraction `80u64 - res` cannot be proven safe and fails closed at compile time with `may overflow \`u64\``.
- **Identity and lifetime:** The structural record values (`Inner`, `Middle`, `Outer`, `RootRec`) are copy-admitted aggregates. Deep projections across four levels of field access maintain exact storage offsets, value identity, and interval refinement through local variables, aggregate fields, machine parameters, return call results, match join bindings, constant definitions, state transition parameters, machine calls, and target-scoped bodies without memory corruption, slicing, or loss of refined bounds.
- **Composition:** The preserved interval $[40, 80]$ composes across caller-callee boundaries, nested state transitions, match arm joins, and forwarding machines. Subtractions `80u64 - res` and `res - 40u64` are proven directly from the propagated field contract, requiring no post-hoc runtime assertions or dynamic caller bounds checks.
- **Soundness boundaries:** Structural access paths adhere strictly to ownership, borrow, and type invariant rules. Relaxing any boundary in the nested path (such as allowing $val \le 81u64$) is caught by proof obligation analysis during exact arithmetic checking, rejecting unsound values before native lowering.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Source-level 4-level projections lower into sequences of target-neutral field projection operations in Terminal Psi (`PSITERM\0`). Erased mathematical contracts and interval certificates validate exact arithmetic operations (`SubExact` on `u64`), executing identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Structural record layout computation aligns each nested record type: `Inner` has `size_of = 8, align_of = 8`; `Middle` has `size_of = 8, align_of = 8`; `Outer` has `size_of = 8, align_of = 8`; `RootRec` has `size_of = 8, align_of = 8`. In calling conventions across all hosted targets, 64-bit aggregate records fit into 64-bit integer registers (`RCX`/`RDX` on Windows `x86_64`, `RDI`/`RSI` on Linux `x86_64`, `X0`/`X1` on `arm64`) or stack slots according to target ABI plans.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering loads the 64-bit leaf scalar from its nested struct offset (offset 0 across all 4 levels) into a 64-bit register (such as `rax`), and performs exact 64-bit subtractions (`sub`, `mov`) with constant immediates. On `arm64`, 64-bit register operations (`sub`, `mov`) execute exact subtraction on `X` registers. Activation frames maintain sufficient stack space for aggregate locals and spills.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves basic blocks and branches, linking entry routines and validating transitive machine-state footprints. Executables run natively and achieve bit-for-bit behavioral agreement with Terminal Psi, returning exit code 0 across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).
