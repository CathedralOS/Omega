# Appendix: Open Questions

This page tracks design pressure that is not fully nailed down yet.

## Current Answers

- `const` parameters are compile-time values, proof constants, or both. Omega should use every fact it can soundly know.
- `&mut self` is the working spelling for machine member functions and states that need machine state. The goal is not to be different for the sake of being different.
- `machine` is the callable boundary. Calling a machine creates the callable activation; transitioning to an internal `state` does not.
- States are graph labels inside the current machine. They may take arguments and participate in return-value compatibility, but they are reached by `transition`, not by normal call syntax.
- Terminal value completion is useful: a final value completes the active machine with a value, while `transition { _ -> state_name(args) }` jumps to an internal state.
- Relax obligations are compile-time proof obligations. The runtime should not carry hidden invariant state unless a debug/proof artifact explicitly asks for it.
- Target signatures define the invariants they accept. Either the caller can prove the handoff satisfies the signature, or the transition is illegal.
- `domain` names a type-scoped proof predicate over an existing value. Domains
  may have explicit `when` classifiers for cheap domain-pattern matching, but
  Omega should not inject hidden runtime tags to make ambiguous domains
  distinguishable. Matching a data value with `Type::Domain` checks that
  domain and adds its facts to the selected arm.
- `match` is for value selection. `transition` is for control movement. Conditional transitions name a scrutinee; anonymous `transition { _ -> target }` is only for unconditional jumps.
- Borrowed slices should use Rust-like `&[T]` / `&mut [T]` surface syntax.
  They are built-in borrowed views with proof-visible facts such as `len` and
  type-scoped invariant parameters such as `&[T, [non_empty]]`; indexing is
  valid when the current facts prove the index is inside the slice bounds.
- The working refinement syntax is `i32[range<1, 100>]` and `i32[range<min, max>]`. Rust has range values, range patterns, and const generics, but it does not have native refined primitive types like this. Omega should use the syntax that makes proof obligations easiest to read.
- Omega should distinguish proof numbers from machine numbers. `UInt`, `Int`, and `Real` are useful as mathematical/spec types, while `i32`, `u64`, `f32`, and similar types are concrete machine representations with explicit proof obligations.
- Machine integer arithmetic should probably default to exact/proven semantics. Weaker behavior such as `wrapping`, `trap`, `saturating`, or `checked` should be explicit because each mode changes proof obligations and runtime behavior.
- Omega's proof vocabulary should distinguish facts, requirements, guarantees, obligations, invariants, contracts, and trust. Values carry facts; operations have contracts; contracts create obligations; trust names the authority for accepting unproved guarantees.
- Inline assembly should be parsed as target assembly under Omega's stricter accepted subset rather than bypassing the language. Assembly jumps are only valid if they satisfy Omega's state-transition rules, and assembly memory/register effects must be declared or inferred from known instruction contracts.
- Semantic states remain branch-free. Source-level mid-state transitions may exist for early exits, but the compiler lowers them into generated branch-free sub-states or basic blocks with explicit edges and cleanup.
- A machine enters at the top of its body. Machines may still need target-specific startup rules, but function callability and runtime startup should not be conflated.
- Omega should avoid reserving keywords aggressively. Prefer contextual keywords when grammar position is enough, especially for words like `entry`, `where`, `trust`, `requires`, and `ensures`. Fully reserved words should be rare and justified by parser clarity, safety, or proof semantics.

## Still Open

- Can the compiler infer result bounds from `match` and `transition` partitions without explicit annotations?
- How much domain classifier/checker inference should Omega attempt beyond
  explicit `when` clauses and executable domain bodies?
- Can relax obligations cross arbitrary transitions, or only transitions to states that opt in?
- How explicit should weakened machine invariants be in target state signatures?
- Can typed state clusters suspend across ticks, or must they complete in one scheduling turn?
- What syntax should Omega use for float optimization permissions, separate from float invariants?
- Which float properties should be first-class invariants: `finite`, `non_nan`, `normal`, signed-zero policy, or something else?
- What exact spelling should arithmetic modes use: `i32[exact]`, `i32[wrapping]`, `i32[trap]`, `i32[saturating]`, and `i32[checked]`, or a separate syntax?
- Does `checked` arithmetic return `Option`, `Result`, a language-specific checked value, or require explicit operator forms?
- How should `Real` contracts lower when called with `f32` or `f64`: explicit `approx<Real, eps=...>`, compiler-inferred error bounds, or named approximation policies?
- Should inline assembly allow local labels and internal jumps, or only structured exits that map to Omega transitions?
- What is the minimum contract syntax for assembly clobbers, memory effects, target features, and emitted invariants?
- Which assembly instructions belong in the first accepted subset for each target, and what exact contracts should each instruction emit?
- When should manual assembly contracts be allowed to supplement known instruction contracts, and when should they be rejected as too opaque?
- Which words must be globally reserved, and which should remain contextual keywords only?
- How should imported library/syscall signatures and target bindings describe native operand lowering, so `Stdout.write` and `Process.exit` are not compiler-special string matches?
- Should the language eventually support explicit tail calls into machines, and if so what spelling avoids confusing them with ordinary `-> state()` transitions?
