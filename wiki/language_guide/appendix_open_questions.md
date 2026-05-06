# Appendix: Open Questions

Open questions to settle later:

- Should the refinement syntax be `i32[range<1, 100>]`, `i32 where range<1, 100>`, or something else?
- Are `const` parameters compile-time values, proof constants, or both?
- Is `&mut self` the right spelling, or should Omega avoid Rust-looking receiver syntax?
- Can typed states be called from commands, or only transitioned into?
- Does a final expression make a state too function-like?
- How does this interact with branch-free states and ordered transitions?
- Can the compiler infer result bounds from ordered transitions without explicit annotations?
- Can relax obligations cross arbitrary transitions, or only transitions to states that opt in?
- How explicit should weakened machine invariants be in target state signatures?
