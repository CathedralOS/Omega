# Chapter 12: Inline Assembly

Inline assembly is not an escape hatch from Omega's proof model.

Omega may eventually allow assembly inside states, but assembly must participate in the same control-flow, ownership, aliasing, and invariant rules as ordinary Omega code. The right mental model is:

- Assembly instructions emit contracts.
- The compiler checks those contracts.
- If the contracts cannot be satisfied, the program fails to compile.
- Trust is explicit when the compiler cannot prove an assembly contract from Omega facts.

This keeps inline assembly useful for low-level work without letting it become a hole in the language.

## Contract-Emitting Assembly

An assembly block should describe what it reads, writes, clobbers, requires, ensures, and how control can leave the block.

Sketch:

```omega
state my_state(&mut self) {
}

state my_state_with_asm(&mut self) {
    -> some_other_state when self.value > 0;

    asm {
        jmp my_state
    }
}
```

This may be valid if the assembly jump satisfies Omega's normal transition criteria.

The compiler must be able to prove that:

- `my_state` is a valid transition target.
- The target state accepts the current machine invariant state.
- Any required return-value or continuation compatibility is satisfied.
- The assembly block does not create an unmodeled branch.
- Any registers, memory, or machine state modified by the block are declared and allowed.

In other words, `jmp my_state` is not magic. It is a low-level spelling of a transition the compiler still understands.

## Control Flow Is Still Omega Control Flow

Omega is intentionally strict about control flow. Inline assembly should not silently create arbitrary hidden loops or labels.

This kind of block is suspect:

```omega
asm {
label:
    // ...
    jmp label
    // ...
}
```

It may be invalid because it creates a control-flow loop that does not correspond to Omega states and transitions.

There are two possible future policies:

- Reject arbitrary assembly labels and jumps unless they map to declared Omega state transitions.
- Allow them only when the assembly block emits a complete proof contract for termination, invariants, clobbers, and reachable exits.

The first policy is simpler and safer. The second policy is more powerful, but it puts heavy proof burden on the author.

## Assembly Obligations

Different instructions emit different obligations.

Examples:

- A direct jump emits a control-flow obligation.
- A memory load emits initialization, bounds, alignment, provenance, and aliasing obligations.
- A memory store emits mutability, ownership, bounds, alignment, and invariant-preservation obligations.
- A SIMD load may require source data alignment, element count, initialized bytes, non-overlap, and target feature availability.
- A special CPU instruction may require target feature flags or host trust contracts.
- A register clobber requires the compiler to know which values are destroyed.

For example, a SIMD block might require facts like:

```omega
requires src.initialized
requires src.aligned<16>
requires src.len >= 16
requires dst.unique_mut
requires target_feature<sse2>
ensures dst[0..16].initialized
```

The exact syntax is not settled. The important rule is that assembly does not get to mutate reality without telling the compiler what reality changed.

## Trust Levels

Inline assembly can produce facts at the same three trust levels used elsewhere:

- Proven: Omega proves the assembly contract from surrounding code and target rules.
- Checked: Omega inserts or requires a runtime check before continuing.
- Trusted: A human or host/runtime contract asserts the fact.

Unchecked assembly should be loud in build artifacts.

Example artifact shape:

```text
unchecked assembly obligations:
  physics.omg:42 requires src.aligned<16>
  crypto.omg:91 trusts target_feature<aes>
```

This matches the broader trust model: "trust me" is allowed only when it is explicit, scoped, and auditable.

## Syntax Direction

Inline assembly likely needs both a compact form and a contract-heavy form.

Compact sketch:

```omega
asm {
    jmp my_state
}
```

Contract sketch:

```omega
asm where
    requires src.initialized
    requires src.aligned<16>
    requires dst.unique_mut
    ensures dst.initialized
{
    // target-specific instructions
}
```

The `where` spelling is provisional. It lines up with the idea that assembly blocks are executable code plus proof conditions.

## Working Rules

- Inline assembly must not bypass the state-transition model.
- Hidden exits, hidden loops, and undeclared clobbers are invalid.
- Assembly memory effects must be described in terms Omega can reason about.
- Target-specific instructions may require target-feature contracts.
- Assembly should be unavailable in safe/proven builds unless all obligations are discharged or explicitly trusted.
- The compiler should prefer structured assembly contracts over parsing arbitrary textual assembly semantics.

Omega can still emit machine bytes directly. Inline assembly is about letting users request specific low-level operations while preserving the compiler's ability to reason about the program.
