# Chapter 2: States

A state is a graph node inside an active function frame.

States execute straight-line work until a transition fires or the state completes. A state does not create a call frame by itself, and ordinary calls cannot target plain states.

```omega
state running() {
    tick();

    transition {
        _ -> running()
    }
}
```

The important rule:

- State bodies do work.
- Transitions move control.
- Transitions do not have bodies.
- Transitions are not function calls.
- Plain states are not callable by normal call syntax.
- A transition target is written with call-shaped arguments, even when there are none: `running()`.
- `transition value { pattern -> target }` replaces local `if` / `else` branching.
- `transition { _ -> target }` is reserved for unconditional jumps.

States are the explicit stateful parts of the program. They are where debugger UX, proof boundaries, and code generation should line up.

Functions add stack-frame boundaries to this model, but they do not erase the graph model.

## Function Frame Boundaries

`fn` is the spelling for a body that may be called with normal call syntax.

```omega
fn run() {
    setup();

    transition {
        _ -> loop()
    }
}

state loop() {
    tick();

    transition {
        _ -> loop()
    }
}
```

Working interpretation:

- Calling a `fn` creates a frame and a continuation.
- Transitioning to a plain state never creates a frame.
- Plain `state`s can only be reached by transition.
- Terminal completion from any plain state returns from the active function frame.
- A function may transition into plain states that form its internal graph.

This means a machine can have many function entry points. They are not necessarily public or runtime entry states; they are just call boundaries.

## Source States Versus Semantic States

Omega may allow transitions to appear before the physical end of a source state.

This is useful for early exits:

```omega
state loading() {
    read_header();

    transition header.invalid {
        true -> failed()
        false -> loading_body()
    }
}

state loading_body() {
    read_body();

    transition {
        _ -> loaded()
    }
}
```

This does not introduce true branching inside a state. It is graph-authoring sugar.

The compiler should decompose the source state into semantic sub-states:

```omega
state loading_0() {
    read_header();

    transition header.invalid {
        true -> failed()
        false -> loading_1()
    }
}

state loading_1() {
    read_body();

    transition {
        _ -> loaded()
    }
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
state combat_round() {
    survived: bool = combat.fight_rat(&mut player);

    transition survived {
        false -> game_over()
        true -> continue_exploring()
    }
}

state continue_exploring() {
    mode = GameMode::Exploring;

    transition {
        _ -> describe_room()
    }
}
```

The lowered graph records which locals die on each outgoing edge.

For trivial locals, cleanup may emit no code. For owned values with cleanup, the transition edge must run that cleanup unless the value is explicitly moved into the transition arguments.

Working rules:

- Locals are scoped to the source state or generated source segment.
- A transition may copy `Copy` values, move owned values, or pass machine-owned references.
- Passing a reference to a stack local across a transition is illegal unless the lifetime is proven to outlive the transition target.
- Passing a non-copy local through a transition should be explicit with `move`.
