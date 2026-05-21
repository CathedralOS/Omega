# Chapter 2: States

A machine is the callable boundary. A state is an internal control label inside
that machine.

The top of a machine body is the entry path. If that path needs to continue
elsewhere, it transitions to a named state.

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

Working rules:

- Calling `Game::run` enters the machine body at the top.
- `prompt`, `look`, `invalid_command`, and `finished` are internal state labels.
- Ordinary calls target machines, not states.
- Transitions target states in the current machine.
- A transition is a jump, not a call.
- A transition does not push a frame, store a return address, or resume the
  source state later.
- A state can take explicit parameters.
- A state that reaches the end without another transition completes the current
  machine invocation.

## Machines Without States

Many machines do not need internal states.

```omega
machine Player::take_damage(
    &mut self,
    amount: i32
) {
    self.health = self.health - amount;
}
```

This is still a machine. It simply has one entry path and no internal jump
targets.

## States Are Not Methods

States are not alternate public methods on the data type.

```omega
machine Dungeon::enter_room(&mut self) {
    transition {
        _ -> mark_current_room_discovered()
    }

    state mark_current_room_discovered(&mut self) {
        let room: Room;
        self.lookup.find_room(self.level, self.current_cell, &mut room);
        room.discovered = true;
    }
}
```

`mark_current_room_discovered` belongs to the control graph of
`Dungeon::enter_room`. Code outside that machine cannot call it directly. If a
behavior needs to be called from elsewhere, it should be its own machine.

## State Parameters

State parameters make jump inputs explicit.

```omega
machine Inventory::find_item(
    &self,
    kind: ItemKind,
    out: &mut Option<usize>
) {
    let items: &[InventoryItem] = self.items.as_slice();

    transition items.len > 0 {
        true -> find_item_at(items, kind, 0, out)
        false -> not_found(out)
    }

    state find_item_at(
        items: &[InventoryItem],
        kind: ItemKind,
        index: usize,
        out: &mut Option<usize>
    ) {
        let found: bool = items[index].kind == kind;
        let next_index: usize = index + 1;
        let has_next: bool = next_index < items.len;

        transition (found, has_next) {
            (true, _) -> found_item(index, out)
            (false, true) -> find_item_at(items, kind, next_index, out)
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

## Source States Versus Lowered States

The source syntax may not map one-to-one to the final semantic graph.

```omega
machine Loader::load(&mut self) {
    transition {
        _ -> loading()
    }

    state loading(&mut self) {
        self.read_header();

        transition self.header.invalid {
            true -> failed()
            false -> loading_body()
        }
    }

    state loading_body(&mut self) {
        self.read_body();

        transition {
            _ -> loaded()
        }
    }
}
```

Working rules:

- A source state is a readable authoring unit.
- The compiler may lower a source state into smaller semantic states or basic
  blocks.
- A transition ends the current straight-line segment.
- Proofs, optimization, and code generation operate on the lowered graph.
- Tools should be able to show both the source state and the lowered graph.

## Local Lifetime Rule

A transition ends the current path, so locals must be accounted for before the
jump.

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

- Locals are scoped to the source state or generated source segment.
- A transition may copy `Copy` values, move owned values, or pass references
  whose lifetime is valid on the target path.
- Passing a reference to a stack local across a transition is illegal unless the
  compiler can prove the referenced storage outlives the transition target.
- Passing a non-copy local through a transition should be explicit with `move`.
- Values that are live before a transition edge and not moved into the target
  must be cleaned up on that edge.
