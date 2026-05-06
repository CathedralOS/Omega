# Appendix: Open Questions

This page tracks design pressure that is not fully nailed down yet.

## Current Answers

- `const` parameters are compile-time values, proof constants, or both. Omega should use every fact it can soundly know.
- `&mut self` is acceptable if it is the clearest spelling. The goal is not to be different for the sake of being different.
- Typed states are callable in the broad sense: they can be entered with arguments. Inside one machine, that call-like shape still lowers to typed transitions/gotos rather than hidden call-stack behavior.
- A final expression does not automatically make a state too function-like. It is just the terminal value of a typed state graph.
- Relax obligations are compile-time proof obligations. The runtime should not carry hidden invariant state unless a debug/proof artifact explicitly asks for it.
- Target signatures define the invariants they accept. Either the caller can prove the handoff satisfies the signature, or the transition is illegal.
- The working refinement syntax is `i32[range<1, 100>]` and `i32[range<min, max>]`. Rust has range values, range patterns, and const generics, but it does not have native refined primitive types like this. Omega should use the syntax that makes proof obligations easiest to read.
- Typed states remain branch-free semantically. Source-level mid-state transitions may exist for early exits, but the compiler lowers them into generated branch-free sub-states with explicit edges.
- `state entry` is for implicit invocation, such as `machine main`, anonymous machines, and future thread/task machines. Ordinary machines can still be entered through explicit state names.

## Still Open

- Can the compiler infer result bounds from ordered transitions without explicit annotations?
- Can relax obligations cross arbitrary transitions, or only transitions to states that opt in?
- How explicit should weakened machine invariants be in target state signatures?
- Can typed state clusters suspend across ticks, or must they complete in one scheduling turn?
- What syntax should Omega use for float optimization permissions, separate from float invariants?
- Which float properties should be first-class invariants: `finite`, `non_nan`, `normal`, signed-zero policy, or something else?
