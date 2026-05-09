# Chapter 3: Transitions

Within one active function frame, transitions are gotos.

They do not push a stack frame, remember a return address, or resume the source state. The source state path ends, and the target state activates with explicit arguments.

```omega
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    -> min when value < min
    -> max when value > max

    -> value
}
```

This example has no helper states because simple early value completion is allowed.

## State Transitions

State transitions are call-shaped.

```omega
state exploring() {
    turn_result: MoveResult = world.try_move(player.position, ui.ask_direction());

    -> enter_combat() when turn_result.entered_combat
    -> describe_room()
}
```

The parentheses matter:

- `-> describe_room()` means transition to the plain state `describe_room`.
- `-> describe_room` means terminally complete with the value named `describe_room`.
- `->` means terminally complete with unit/no value.

This avoids the ambiguity where a bare name might be either a state label or a return value.

## Terminal Value Transitions

A transition can complete the active function frame directly.

```omega
fn fib(n: i32) -> i32 {
    -> n when n <= 1

    left: i32 = fib(n - 1);
    right: i32 = fib(n - 2);

    -> left + right
}
```

`-> n` does not jump to a state. It returns the value from the current function activation.

For no-value functions, a bare terminal arrow is enough:

```omega
state finished() {
    ->
}
```

Terminal completion from a plain state returns from the currently active function frame. The plain state is not the caller; it is part of the function's internal graph.

## Functions Versus States

`fn` is the current working spelling for a frame boundary.

```omega
fn run() {
    setup();

    -> loop()
}

state loop() {
    tick();

    -> loop()
}
```

Rules:

- Calling a `fn` creates a stack frame and continuation.
- Transitioning to a plain `state` does not create a stack frame.
- Plain states cannot be called with normal call syntax.
- Transitions to functions are illegal for now.
- If Omega later needs tail calls into functions, they should get their own spelling rather than overloading `->`.

This probably replaces the older idea of "static states" secretly creating stack semantics. The frame boundary should be visible in the source.

## Mid-State Transitions

Transitions may appear before the physical end of a source state.

They are still gotos. A guarded transition is an early tail jump:

```omega
state combat_round() {
    survived: bool = combat.fight_rat(&mut player);

    -> game_over() when !survived

    mode = GameMode::Exploring;
    -> describe_room()
}
```

Working interpretation:

```text
survived = call combat.fight_rat(&mut player)
if !survived {
    drop locals not moved
    jump game_over()
}
mode = GameMode::Exploring
jump describe_room()
```

The compiler may lower this into generated semantic sub-states or basic blocks. Diagnostics should point back to the source transition, while graph/debugger views may expose generated nodes when useful.

## Ordered Lazy Branches

Transition rows are ordered and lazy.

```omega
-> enter_combat() when turn_result.entered_combat
-> describe_room(expensive_summary())
```

`expensive_summary()` is evaluated only if the previous guarded transition did not fire.

Each row behaves like:

1. Evaluate the guard, if present.
2. If the guard passes, evaluate that row's arguments or terminal value.
3. Drop locals not moved into the transition.
4. Jump or complete the active function frame.

This rule applies to final transition tables and mid-state transitions alike.

## Local Lifetime Rule

A transition ends the current path, so locals must not leak accidentally.

```omega
state build_inventory() {
    default_inventory: Inventory;

    -> done() when invalid

    -> copy_default_items(move default_inventory)
}
```

Working rules:

- `move default_inventory` transfers ownership into the transition target.
- Copy values may be copied into transition arguments.
- References to stack locals cannot cross a transition unless the compiler can prove the referenced storage outlives the target path.
- Machine-owned storage may be referenced across transitions because it is not owned by the current stack frame.

The IR should eventually make cleanup explicit, either as transition-owned cleanup lists or as lowered `DropLocal` operations.
