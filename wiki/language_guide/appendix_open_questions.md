# Appendix: Open Questions

This page tracks design pressure that is not fully nailed down yet.

## Current Answers

- `const` parameters are compile-time values, proof constants, or both. Omega should use every fact it can soundly know.
- `&mut self` is acceptable if it is the clearest spelling. The goal is not to be different for the sake of being different.
- Typed states are callable in the broad sense: they can be entered with arguments. Inside one machine, that call-like shape still lowers to typed transitions/gotos rather than hidden call-stack behavior.
- A final expression does not automatically make a state too function-like. It is just the terminal value of a typed state graph.
- Relax obligations are compile-time proof obligations. The runtime should not carry hidden invariant state unless a debug/proof artifact explicitly asks for it.
- Target signatures define the invariants they accept. Either the caller can prove the handoff satisfies the signature, or the transition is illegal.

## Still Open

- Should the refinement syntax be `i32[range<1, 100>]`, `i32 where range<1, 100>`, or something else?
- How do typed states interact with branch-free states and ordered transitions?
- Can the compiler infer result bounds from ordered transitions without explicit annotations?
- Can relax obligations cross arbitrary transitions, or only transitions to states that opt in?
- How explicit should weakened machine invariants be in target state signatures?
- Can typed state clusters suspend across ticks, or must they complete in one scheduling turn?
