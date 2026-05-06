# Chapter 4: Typed States

A state may accept explicit entry data and declare the value shape its graph eventually produces.

```omega
state clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.clamp_low(min) when value < min;
    -> self.clamp_high(max) when value > max;
    -> self.clamp_done(value);
}

state clamp_low(&mut self, min: f32) -> f32 {
    -> self.clamp_done(min);
}

state clamp_high(&mut self, max: f32) -> f32 {
    -> self.clamp_done(max);
}

state clamp_done(&mut self, value: f32) -> f32 {
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
