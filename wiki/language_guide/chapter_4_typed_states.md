# Chapter 4: Typed Machine Graphs

A machine may declare the value shape its graph eventually produces.

```omega
machine main(&mut self) -> i32 {
    self.game.initialize(7);

    transition {
        _ -> running()
    }

    state running(&mut self) {
        self.game.run_game_loop();

        transition {
            _ -> shutdown()
        }
    }

    state shutdown(&mut self) {
        0
    }
}
```

`main` promises `i32`. Every reachable terminal path in `main` must produce an
`i32`.

Working interpretation:

- Machine parameters are entry data.
- `&mut` parameters are unique mutable borrows.
- A machine return type is the value shape its internal graph must eventually
  produce.
- Internal states do not create separate public return contracts.
- A state may end by transitioning to another state or by producing the
  machine's terminal value.
- A transition to another state is a typed jump, not a stack return.

## Typed Helper Machines

Helper machines are ordinary callable boundaries.

```omega
machine CommandParser::resolve_command(
    &mut self,
    line: &ConsoleLine
) -> NavigationChoice {
    transition line.text {
        "quit" -> NavigationChoice::Quit
        "look" -> NavigationChoice::Look
        _ -> NavigationChoice::Invalid
    }
}
```

This machine has no internal states. Its dispatch arms produce values directly.

## State Arguments

State arguments are checked against the target state's parameter list.

```omega
machine RoomLookup::find_room(
    &self,
    target: CellId,
    out: &mut Room
) {
    transition {
        _ -> find_room_at(target, 0, out)
    }

    state find_room_at(
        target: CellId,
        index: usize,
        out: &mut Room
    ) {
        let found: bool = self.rooms[index].cell == target;
        let next_index: usize = index + 1;

        transition found {
            true -> apply_room(index, out)
            false -> find_room_at(target, next_index, out)
        }
    }

    state apply_room(
        index: usize,
        out: &mut Room
    ) {
        out = self.rooms[index];
    }
}
```

The compiler checks both the value types and the borrow facts crossing each
transition edge.
