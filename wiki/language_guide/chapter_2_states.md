# Chapter 2: States

A state executes straight-line work. At the end of the state, transition arrows decide where control goes next.

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
