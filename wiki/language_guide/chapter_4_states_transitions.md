# Chapter 4: States And Transitions

A state is an internal control label inside a machine. A transition is a jump to
another state in the same machine.

The top of a machine body is the entry path.

```omega
machine Game::run(&mut self) {
    self.view.render_title();

    transition {
        _ -> prompt()
    }

    state prompt(&mut self) {
        self.view.render_prompt();
        self.input.read_line(&mut self.line);

        transition self.parser.resolve_command(&self.line) {
            Command::Look -> look()
            Command::Quit -> finished()
            Command::Invalid -> invalid_command()
        }
    }

    state look(&mut self) {
        self.view.render_room(&self.room);

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

## Working Rules

- Calling `Game::run` enters the machine body at the top.
- `prompt`, `look`, `invalid_command`, and `finished` are internal state labels.
- Ordinary calls target machines, not states.
- Transitions target states in the current machine.
- A transition is a jump, not a call.
- A transition does not push a frame, store a return address, or resume the
  source state later.
- A state that reaches the end without another transition completes the current
  machine invocation.

## State Parameters

State parameters make jump inputs explicit.

```omega
machine Inventory::find_item(
    &self,
    kind: ItemKind,
    out: &mut Option<usize>
) {
    transition {
        _ -> find_item_at(kind, 0, out)
    }

    state find_item_at(
        kind: ItemKind,
        index: usize,
        out: &mut Option<usize>
    ) {
        let found: bool = self.items[index].kind == kind;
        let next_index: usize = index + 1;
        let has_next: bool = next_index < self.items.len;

        transition (found, has_next) {
            (true, _) -> found_item(index, out)
            (false, true) -> find_item_at(kind, next_index, out)
            (false, false) -> not_found(out)
        }
    }

    state found_item(
        index: usize,
        out: &mut Option<usize>
    ) {
        out = Some(index);
    }

    state not_found(out: &mut Option<usize>) {
        out = None;
    }
}
```

The call-shaped syntax in a transition arm is argument passing for a jump. It is
not method dispatch.

## Terminal Completion

A machine or state can complete by producing the machine's declared result.

```omega
data Main {
}

machine Main::main(&mut self) -> i32 {
    transition {
        _ -> shutdown()
    }

    state shutdown(&mut self) {
        0
    }
}
```

`shutdown` is a state target. `0` is the terminal value.

## Lowered Graphs

The source syntax may not map one-to-one to the final semantic graph.

Working rules:

- A source state is a readable authoring unit.
- The compiler may lower a source state into smaller semantic states or basic
  blocks.
- A transition ends the current straight-line segment.
- Proofs, optimization, and code generation operate on the lowered graph.
- Tools should be able to show both the source state and the lowered graph.
