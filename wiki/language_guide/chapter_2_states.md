# Chapter 2: States

A state is a graph node inside an active callable frame.

States execute straight-line work until a transition fires or the state completes. A state does not create a call frame by itself, and ordinary calls cannot target plain states.

```omega
state running {
    tick();

    -> running()
}
```

The important rule:

- State bodies do work.
- Transitions move control.
- Transitions do not have bodies.
- Transitions are not function calls.
- Plain states are not callable by normal call syntax.
- A transition to a state is written with call-shaped arguments, even when there are none: `-> running()`.
- Ordered transitions replace local `if` / `else` branching.

States are the explicit stateful parts of the program. They are where debugger UX, proof boundaries, and code generation should line up.

Callable states add stack-frame boundaries to this model, but they do not erase the graph model.

## Callable Frame Boundaries

`callable` is the current scratch spelling for a state-like body that may be called with normal call syntax.

```omega
callable run() {
    setup();

    -> loop()
}

state loop {
    tick();

    -> loop()
}
```

Working interpretation:

- Calling a `callable` creates a frame and a continuation.
- Transitioning with `->` never creates a frame.
- Plain `state`s can only be reached by transition.
- Terminal completion from any plain state returns from the active callable frame.
- A callable may transition into plain states that form its internal graph.

This means a machine can have many callable entry points. They are not necessarily public or runtime entry states; they are just call boundaries.

## Source States Versus Semantic States

Omega may allow transitions to appear before the physical end of a source state.

This is useful for early exits:

```omega
state loading {
    read_header();

    -> failed() when header.invalid

    read_body();

    -> loaded()
}
```

This does not introduce true branching inside a state. It is graph-authoring sugar.

The compiler should decompose the source state into semantic sub-states:

```omega
state loading_0 {
    read_header();

    -> failed() when header.invalid
    -> loading_1()
}

state loading_1 {
    read_body();

    -> loaded()
}
```

So the real rule is:

- Source states may contain mid-state transitions for readability and early exits.
- A mid-state transition ends the current straight-line segment.
- The compiler lowers each segment into a branch-free semantic state.
- Proofs, optimization, and code generation operate on the lowered graph.
- Tools should be able to show both the source state and the generated semantic sub-states.

This is a smaller violation of the original design than true local branching. The programmer may not write every graph node by hand, but the compiler still owns an explicit graph and there is still no hidden `if` / `else` inside a semantic state.

## Local Lifetime Rule

Because a transition ends the current path, stack locals must be accounted for before the jump.

```omega
state combat_round {
    survived: bool = combat.fight_rat(&mut player);

    -> game_over() when !survived

    mode = GameMode::Exploring;
    -> describe_room()
}
```

The lowered graph records which locals die on each outgoing edge.

For trivial locals, cleanup may emit no code. For owned values with cleanup, the transition edge must run that cleanup unless the value is explicitly moved into the transition arguments.

Working rules:

- Locals are scoped to the source state or generated source segment.
- A transition may copy `Copy` values, move owned values, or pass machine-owned references.
- Passing a reference to a stack local across a transition is illegal unless the lifetime is proven to outlive the transition target.
- Passing a non-copy local through a transition should be explicit with `move`.
