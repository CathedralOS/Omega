# Chapter 3: Transitions

Within one active function frame, transitions are gotos.

They do not push a stack frame, remember a return address, or resume the source state. The source state path ends, and the target state activates with explicit arguments.

```omega
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    match (value < min, value > max) {
        (true, _) -> min
        (false, true) -> max
        (false, false) -> value
    }
}
```

This example has no helper states because simple early value completion is allowed.

`match` chooses values. `transition` chooses control flow. Omega keeps those
two ideas separate so a reader can tell whether an arm is returning a value or
moving to another state.

## State Transitions

State transitions are call-shaped.

```omega
state exploring() {
    turn_result: MoveResult = world.try_move(player.position, ui.ask_direction());

    transition turn_result.entered_combat {
        true -> enter_combat()
        false -> describe_room()
    }
}
```

The parentheses matter:

- `describe_room()` in a transition arm means transition to the plain state `describe_room`.
- `-> describe_room` means terminally complete with the value named `describe_room`.
- `->` means terminally complete with unit/no value.

This avoids the ambiguity where a bare name might be either a state label or a return value.

Conditional transitions should name the value being inspected:

```omega
transition navigation.choice {
    NavigationChoice::Quit -> finished()
    NavigationChoice::Look -> look()
    NavigationChoice::Invalid -> invalid_command()
}
```

Anonymous transition blocks are reserved for unconditional jumps:

```omega
transition {
    _ -> prompt()
}
```

## Terminal Value Transitions

A transition can complete the active function frame directly.

```omega
fn fib(n: i32) -> i32 {
    match n <= 1 {
        true -> n
        false -> fib(n - 1) + fib(n - 2)
    }
}
```

The match arms produce values. They do not transition to states.

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

Rules:

- Calling a `fn` creates a stack frame and continuation.
- Transitioning to a plain `state` does not create a stack frame.
- Plain states cannot be called with normal call syntax.
- Transitions to functions are illegal for now.
- If Omega later needs tail calls into functions, they should get their own spelling rather than overloading `->`.

This probably replaces the older idea of "static states" secretly creating stack semantics. The frame boundary should be visible in the source.

## Mid-State Transitions

Transitions may appear before the physical end of a source state.

They are still gotos. A transition dispatch can act as an early tail jump:

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

Working interpretation:

```text
survived = call combat.fight_rat(&mut player)
if survived == false jump game_over()
if survived == true jump continue_exploring()
```

The compiler may lower this into generated semantic sub-states or basic blocks. Diagnostics should point back to the source transition, while graph/debugger views may expose generated nodes when useful.

## Transition Dispatch

Transition dispatch should name a scrutinee unless the transition is
unconditional.

```omega
transition (round.player_defeated, round.enemy_defeated) {
    (true, _) -> player_died()
    (false, true) -> enemy_died()
    (false, false) -> exchange_blows()
}
```

Tuple scrutinees make multi-fact dispatch explicit and proof-friendly. Each arm
adds the matched pattern as facts for that edge.

When the facts become hard to read, name them first:

```omega
let found: bool = inventory.items[index].kind == kind;
let has_next: bool = index + 1 < item_count;

match (found, has_next) {
    (true, _) -> index
    (false, true) -> find_item_at(items, kind, next_index)
    (false, false) -> None
}
```

This keeps Omega away from anonymous guard soup while preserving exhaustive
case coverage and the `_` escape hatch.

## Local Lifetime Rule

A transition ends the current path, so locals must not leak accidentally.

```omega
state build_inventory() {
    default_inventory: Inventory;

    transition invalid {
        true -> done()
        false -> copy_default_items(move default_inventory)
    }
}
```

Working rules:

- `move default_inventory` transfers ownership into the transition target.
- Copy values may be copied into transition arguments.
- References to stack locals cannot cross a transition unless the compiler can prove the referenced storage outlives the target path.
- Machine-owned storage may be referenced across transitions because it is not owned by the current stack frame.

The IR should eventually make cleanup explicit, either as transition-owned cleanup lists or as lowered `DropLocal` operations.
