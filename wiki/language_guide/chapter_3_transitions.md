# Chapter 3: Transitions

Within one machine, transitions are gotos.

They do not push a stack frame, remember a return address, or resume the source state. The source state deactivates, and the target state activates with explicit arguments.

```omega
state Clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.ClampLow(min) when value < min;
    -> self.ClampHigh(max) when value > max;
    -> self.ClampDone(value);
}
```

`Clamp` never resumes after `ClampLow`, `ClampHigh`, or `ClampDone`. It hands control away.

Across machines, nested machine flow may be stack-like:

```omega
state Running {
    -> dungeon.Main -> Running;
}
```

The parent enters a child machine and records or otherwise carries the continuation to use when the child machine terminates. That continuation stack belongs to machine composition, not ordinary intra-machine branching.

This gives Omega two distinct control-flow worlds:

- Inside one machine, transitions are gotos.
- Between machines, composition may use stack-like continuation.

Keeping those separate matters. Otherwise typed states quietly become ordinary functions wearing a state-machine costume, and the graph shape gets muddy.

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
