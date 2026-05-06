# Chapter 1: Variables And Owned Data

Omega programs are state machines that own data explicitly.

The default mental model is not "a function has locals on a stack." The default model is "a machine owns data, and states operate on that data."

```omega
machine main {
    owns health: i32[range<1, 100>] = 100;
    owns mass: i32[range<1, 100>];

    state entry {
        -> running;
    }
}
```

Working interpretation:

- `owns` declares data owned by the current machine.
- Owned data is stateful and proof-visible.
- Owned data may carry invariants.
- State parameters are entry data, not ambient hidden globals.
- Mutation should be explicit through `&mut self`, `mut` parameters, or owned machine data.

Bounded data is intentionally part of chapter one because proofs need something concrete to talk about. If a machine owns `health: i32[range<1, 100>]`, then every state that mutates `health` creates proof work.

This is the foundation for the rest of the language: state machines are not a library pattern, and data ownership is not implied by call stacks.
