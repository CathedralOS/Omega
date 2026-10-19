# 31-Argument Calling Plan Across Register and Stack Slots

## Fact Sentence
Fact: a machine transition call passing thirty-one u64 arguments across register and stack ABI slots preserves the thirty-first stack-passed argument's static bound 40u64 <= a31 && a31 <= 80u64 across all nine places so a31 - 40u64 and 80u64 - a31 are proved safe without underflow.

## Specification and Lowering Layers

1. **Establishment**:
   The transition machine `pick31(a1..a31)` receives thirty-one `u64` arguments. Arguments `a1..a30` are exact integers `1..30`, and the thirty-first argument is bounded `40u64 <= a31 && a31 <= 80u64`. Across register-to-stack parameter transitions, static bounds are established and verified at entry contracts.

2. **Identity and Lifetime**:
   Each argument value has unique scalar storage identity across register homes and caller-allocated activation stack frame homes. The 31st argument's interval is preserved across value lifetimes without storage aliasing or clobbering.

3. **Composition**:
   The caller passes evaluated expressions into `pick31`, and the resulting value satisfies `40u64 <= res && res <= 80u64`. In all nine places, this bound directly justifies exact subtractions `80u64 - res` and `res - 40u64` without underflow.

4. **Soundness Boundaries**:
   Proof rules ensure that integer range assertions are checked before subtraction operations occur. Widening `a31 <= 81u64` in the invalid twins causes `80u64 - res` to fail closed at compile time with `may overflow \`u64\``.

5. **Checked-to-Terminal Psi Lowering (Psi 05–07)**:
   The high-level Omega calling expression lowers into canonical Terminal Psi (`PSITERM\0`) call signatures carrying 31 typed argument lanes, correctly preserving the static bound intervals into integer range constraints without losing argument indices or types.

6. **Provider Settlement, Layout Geometry, and ABI Calling Plans (Omega 00–02)**:
   Calling plans evaluate target ABI layout geometries for all four supported targets:
   - `windows_x86_64`: args 1..4 in registers (`RCX, RDX, R8, R9`), args 5..31 in stack slots (27 stack arguments).
   - `linux_x86_64`: args 1..6 in registers (`RDI, RSI, RDX, RCX, R8, R9`), args 7..31 in stack slots (25 stack arguments).
   - `macos_arm64` / `linux_arm64`: args 1..8 in registers (`X0..X7`), args 9..31 in stack slots (23 stack arguments).
   In all four hosted targets, the 31st argument `a31` is guaranteed to be homed in a caller-allocated activation frame stack slot.

7. **Instruction Selection, Register Homes, and Activation Frame Realization (Omega 03–06)**:
   The backend emits instructions placing arguments 1..k in target registers and storing arguments (k+1)..31 into stack frame slots at negative/positive offsets relative to the stack frame pointer, updating worst-case stack usage (WCSU) bounds.

8. **Form Encoding, Branch Relaxation, Relocations, and Native Image Emission (Omega 07–09 + emission)**:
   Final image emission lays out stack frame setup/teardown epilogues, branch relaxation, relocations, and machine code bytes matching native target execution with Terminal Psi interpretation (`omega run --both` exit code 0).

## Nine Places Table

| Place | Directory | Purpose | Expected Exit | Rejection Twin |
|---|---|---|---|---|
| local | `local/` | Arguments held in local variables before invocation | 0 | `may overflow \`u64\`` |
| field | `field/` | Result stored in struct field before exact subtraction | 0 | `may overflow \`u64\`` |
| parameter | `parameter/` | Result validated after passing as parameter to consumer | 0 | `may overflow \`u64\`` |
| call_result | `call_result/` | Arguments returned from individual helper calls | 0 | `may overflow \`u64\`` |
| match_join | `match_join/` | Call invoked across boolean match arms and joined | 0 | `may overflow \`u64\`` |
| const_name | `const_name/` | Arguments passed as top-level constants | 0 | `may overflow \`u64\`` |
| other_state | `other_state/` | Subtractions evaluated in another machine state reached via transition | 0 | `may overflow \`u64\`` |
| other_machine | `other_machine/` | Bound validated inside a secondary machine | 0 | `may overflow \`u64\`` |
| target_body | `target_body/` | Explicit target-scoped bodies for all 4 hosted targets | 0 | `may overflow \`u64\`` |
