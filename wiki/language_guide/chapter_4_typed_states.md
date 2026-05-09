# Chapter 4: Typed States

A function or plain state may accept explicit entry data and declare the value shape its graph eventually produces.

```omega
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    -> min when value < min
    -> max when value > max

    -> value
}
```

Plain states may also be typed when they are part of a function's internal transition graph:

```omega
fn fight_rat(player: &mut Player) -> bool {
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

Working interpretation:

- Parameters are local entry data.
- `&mut` parameters are mutable borrows; borrow checking is the long-term model even if early compiler passes treat them as mutable aliases.
- A function return type is the value shape its active graph must eventually produce.
- A plain state return type must be compatible with the function activation that can reach it.
- A body may end in terminal value transitions, plain state transitions, or a final expression.
- A transition to another plain state is a typed goto, not a stack return.

This makes the syntax function-shaped where stack behavior exists, while keeping plain states as explicit graph nodes.
