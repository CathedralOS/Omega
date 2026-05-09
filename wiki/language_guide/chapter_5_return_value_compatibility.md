# Chapter 5: Return Value Compatibility

A typed transition is legal only when the graph handoff lines up with the active function's value obligation.

The key check is return value compatibility: every reachable terminal completion in a function's internal graph must be able to produce the value shape the function promised.

```omega
fn fib(n: i32) -> i32 {
    -> n when n <= 1

    left: i32 = fib(n - 1);
    right: i32 = fib(n - 2);

    -> left + right
}
```

`fib` promises `i32`, so both terminal value transitions must produce `i32`.

## State Graph Compatibility

Plain states can participate in the function's return-value graph.

```omega
fn fight_rat(player: &mut Player) -> bool {
    player.health = saturating_sub(player.health, 10);

    -> defeated() when player.health == 0
    -> survived()
}

state defeated() -> bool {
    -> false
}

state survived() -> bool {
    -> true
}
```

`defeated` and `survived` are not called. They are transition targets inside the active `fight_rat` function frame.

The compatibility checks are:

- The transition target state exists in the current machine graph.
- State-transition arguments match the target state's parameters.
- Terminal value transitions satisfy the active function's return type.
- Every reachable terminal path in a typed function graph produces the declared return value type.
- Guarded transitions may add proof assumptions for the target edge, but they do not create caller frames.

This is the key distinction from classic functions: return value compatibility is a graph invariant for the active function frame, not a return path between ordinary states.

## Terminal Value Syntax

Omega does not need a `return` keyword.

```omega
-> value
```

means terminal completion with `value`.

```omega
->
```

means terminal completion with unit/no value.

```omega
-> state_name(args)
```

means transition to a plain state.

State transitions always include parentheses, even when no arguments are passed. That leaves bare values available for terminal completion without requiring `return`.

## Transitioning To Functions

Transitions to functions are rejected in the current model.

```omega
fn helper() -> i32 {
    1
}

state bad() {
    -> helper() // illegal: helper is a function, not a plain transition state
}
```

Normal call syntax is the way to enter a function:

```omega
value: i32 = helper();
```

If Omega later needs tail calls into functions, they should have an explicit spelling. They should not masquerade as ordinary state transitions.
