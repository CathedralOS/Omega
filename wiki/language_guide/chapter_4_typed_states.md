# Chapter 4: Typed States

A state may accept explicit entry data and declare the value shape its graph eventually produces.

```omega
state Clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.ClampLow(min) when value < min;
    -> self.ClampHigh(max) when value > max;
    -> self.ClampDone(value);
}

state ClampLow(&mut self, min: f32) -> f32 {
    -> self.ClampDone(min);
}

state ClampHigh(&mut self, max: f32) -> f32 {
    -> self.ClampDone(max);
}

state ClampDone(&mut self, value: f32) -> f32 {
    value
}
```

Working interpretation:

- State parameters are local state-entry data.
- `&mut self` means the state may mutate the current machine.
- The return type is the value shape the state graph must eventually produce.
- A state body may end in transitions or a final expression.
- Transitions can forward values into another state.
- A transition to another state is a typed goto, not a stack return.

This makes a state feel function-shaped, but it is still a graph node with explicit outgoing edges.
