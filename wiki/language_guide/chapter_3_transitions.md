# Chapter 3: Transitions

Inside a machine, transitions are jumps.

They do not call a function, push a stack frame, remember a return address, or
resume the source state later. The source path ends, and the target state starts
with explicit arguments.

```omega
machine Game::run(&mut self) {
    transition {
        _ -> prompt()
    }

    state prompt(&mut self) {
        self.view.render_prompt();
        self.console.read_line(&mut self.input);

        transition self.input.text {
            "look" -> look()
            "quit" -> finished()
            _ -> invalid_command()
        }
    }

    state look(&mut self) {
        self.view.render_look();

        transition {
            _ -> prompt()
        }
    }

    state invalid_command(&mut self) {
        self.view.render_invalid_command();

        transition {
            _ -> prompt()
        }
    }

    state finished(&mut self) {
    }
}
```

Working rules:

- `transition { _ -> prompt() }` is an unconditional jump.
- `transition self.input.text { ... }` dispatches on a value.
- Transition targets are written with call-shaped arguments.
- `look()` means jump to state `look`.
- A bare value is terminal completion, not a state target.
- Transitions can only target states in the current machine.

## Dispatch

Conditional transitions name the value being inspected.

```omega
transition navigation.choice {
    NavigationChoice::Quit -> finished()
    NavigationChoice::Look -> look()
    NavigationChoice::Invalid -> invalid_command()
}
```

Tuple scrutinees make multi-fact dispatch explicit.

```omega
transition (round.player_defeated, round.enemy_defeated) {
    (true, _) -> player_died()
    (false, true) -> enemy_died()
    (false, false) -> exchange_blows()
}
```

Each selected arm adds its matched pattern as proof facts for that edge.

When the facts become hard to read, name them first:

```omega
let found: bool = inventory.items[index].kind == kind;
let has_next: bool = index + 1 < item_count;

transition (found, has_next) {
    (true, _) -> found_item(index)
    (false, true) -> find_item_at(next_index)
    (false, false) -> not_found()
}
```

## Guarded Jumps

A state may use a guarded jump when one early edge is clearer than a full
dispatch.

```omega
state read_command(&mut self) {
    self.console.read_line(&mut self.input);

    -> finished() when self.input.text == "";

    transition self.input.text {
        "quit" -> finished()
        "look" -> look()
        _ -> invalid_command()
    }
}
```

The guarded `-> finished()` is still a jump. If the guard is true, the current
path ends and `finished` starts.

## Terminal Completion

A machine or state can complete by producing the machine's declared result.

```omega
machine main(&mut self) -> i32 {
    transition {
        _ -> shutdown()
    }

    state shutdown(&mut self) {
        0
    }
}
```

`shutdown` is a state target. `0` is the terminal value for the active machine.

## Local Lifetime Rule

A transition ends the current path, so locals must not leak accidentally.

```omega
machine InventorySystem::repair(&mut self) {
    transition {
        _ -> build_inventory()
    }

    state build_inventory(&mut self) {
        let default_inventory: Inventory;

        transition self.inventory_valid {
            true -> done()
            false -> copy_default_items(move default_inventory)
        }
    }

    state copy_default_items(default_inventory: Inventory) {
        self.inventory = default_inventory;
    }

    state done(&mut self) {
    }
}
```

Working rules:

- `move default_inventory` transfers ownership into the transition target.
- Copy values may be copied into transition arguments.
- References to stack locals cannot cross a transition unless the compiler can
  prove the referenced storage outlives the target path.
- Machine-owned storage may be referenced across transitions because it is not
  owned by the current stack segment.
- Owned locals that are not moved into the target are cleaned up on the edge
  before the jump.
