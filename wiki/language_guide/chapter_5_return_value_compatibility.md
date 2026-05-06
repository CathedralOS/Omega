# Chapter 5: Return Value Compatibility

A typed transition is legal only when the graph handoff lines up.

The key check is return value compatibility: the target graph must be able to produce the value shape the source graph is obligated to produce.

Likely checks:

- The target state exists in the current machine or explicitly addressed machine.
- The provided arguments match the target state's parameters.
- The target state's return value type can satisfy the current state's return value obligation.
- Every reachable terminal expression in a typed state graph produces the declared return value type.
- A guarded transition may add proof assumptions for the target edge, but it does not create a caller frame.

Example:

```omega
state clamp_low(&mut self, min: f32) -> f32 {
    -> self.clamp_done(min);
}
```

`clamp_low` can jump to `clamp_done` because `clamp_done` accepts the forwarded `f32` and produces the same `f32` expected by the `clamp` graph.

This is the key distinction from classic functions: return value compatibility is a graph invariant, not a return path.

Final expressions are constrained by the same idea:

```omega
state clamp_done(&mut self, value: f32) -> f32 {
    value
}
```

The produced value flows to the enclosing graph expectation, not back to an intra-machine caller frame.
