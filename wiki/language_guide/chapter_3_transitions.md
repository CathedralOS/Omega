# Chapter 3: Transitions

Within one machine, transitions are gotos.

They do not push a stack frame, remember a return address, or resume the source state. The source state deactivates, and the target state activates with explicit arguments.

```omega
state clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.clamp_low(min) when value < min;
    -> self.clamp_high(max) when value > max;
    -> self.clamp_done(value);
}
```

`clamp` never resumes after `clamp_low`, `clamp_high`, or `clamp_done`. It hands control away.

Across machines, nested machine flow may be stack-like:

```omega
state running {
    -> dungeon.entry -> running;
}
```

The parent enters a child machine and records or otherwise carries the continuation to use when the child machine terminates. That continuation stack belongs to machine composition, not ordinary intra-machine branching.

Explicit terminal/default completion is written as a trailing bare arrow:

```omega
state done {
    cleanup();

    ->
}
```

There is no `return` keyword. For typed states, a final expression produces the value:

```omega
state done(value: f32) -> f32 {
    value
}
```

If a state has no outgoing transition table, completion can be implicit:

```omega
state cleanup {
    release_temp_buffers();
}
```

This gives Omega two distinct control-flow worlds:

- Inside one machine, transitions are gotos.
- Between machines, composition may use stack-like continuation.

Keeping those separate matters. Otherwise typed states quietly become ordinary functions wearing a state-machine costume, and the graph shape gets muddy.

## Entry States

`state entry` is reserved for machines that need implicit invocation semantics.

Examples:

- `machine main`, because the runtime starts it.
- Anonymous machines, because a caller invokes the machine as a value.
- Future thread, task, or fiber machines, if spawning starts the machine as a unit.

Ordinary named machines do not need `entry` unless they are invoked as a whole. They may still be entered through explicit state names:

```omega
state running {
    -> room_manager.tick_room;
}
```

The rule is:

- If something starts a machine without naming a state, that machine needs `state entry`.
- If code transitions to an explicitly named state, no entry state is required.
- The first state listed is never special.

## Mid-State Transitions

Transitions may eventually be allowed inside source states before the final line.

They are still gotos. They still terminate the current straight-line segment. The compiler lowers the remaining source statements into generated continuation states.

This means an early-exit transition has better source UX without changing the semantic graph model:

- No true branching inside a semantic state.
- No transition bodies.
- No hidden call stack.
- No implicit `if` / `else`.
- Just generated sub-states and explicit edges.

Diagnostics should point back to the source transition, while graph/debugger views may expose the generated sub-states when useful.
