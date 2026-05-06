# Chapter 2: States

A semantic state executes straight-line work. At the end of the state, transition arrows decide where control goes next.

```omega
state Running {
    Tick();

    -> self;
}
```

The important rule:

- State bodies do work.
- Transitions move control.
- Transitions do not have bodies.
- Transitions are not function calls.
- Ordered transitions replace local `if` / `else` branching.

States are the explicit stateful parts of the program. They are where debugger UX, proof boundaries, and code generation should line up.

Typed states add signatures to this model, but they do not replace the model.

## Source States Versus Semantic States

Omega may allow transitions to appear before the physical end of a source state.

This is useful for early exits:

```omega
state Loading {
    ReadHeader();

    -> Failed when header.invalid;

    ReadBody();

    -> Loaded;
}
```

This does not introduce true branching inside a state. It is graph-authoring sugar.

The compiler should decompose the source state into semantic sub-states:

```omega
state Loading_0 {
    ReadHeader();

    -> Failed when header.invalid;
    -> Loading_1;
}

state Loading_1 {
    ReadBody();

    -> Loaded;
}
```

So the real rule is:

- Source states may contain mid-state transitions for readability and early exits.
- A mid-state transition ends the current straight-line segment.
- The compiler lowers each segment into a branch-free semantic state.
- Proofs, optimization, and code generation operate on the lowered graph.
- Tools should be able to show both the source state and the generated semantic sub-states.

This is a smaller violation of the original design than true local branching. The programmer may not write every graph node by hand, but the compiler still owns an explicit graph and there is still no hidden `if` / `else` inside a semantic state.
