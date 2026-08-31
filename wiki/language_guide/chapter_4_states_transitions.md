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
- The machine head and top-level body are its only source entry surface. There
  is no nested `entry` or `pub entry` member; the compiler's internal entry
  block is generated from that ordinary body.
- The machine body may do setup work first, but the entry path must end in one
  explicit tail `transition { ... }` before any `state` declarations.
- `prompt`, `look`, `invalid_command`, and `finished` are internal state labels.
- Ordinary calls target machines, not states.
- Transitions target states in the current machine.
- A transition is a jump, not a call.
- A transition does not push a frame, store a return address, or resume the
  source state later.
- State-to-state control flow is expressed with the `transition` keyword, not
  standalone `-> target when ...;` lines.
- A state that reaches the end without another transition completes the current
  machine invocation.

## State Parameters

State parameters make jump inputs explicit.

```omega
machine Inventory::find_item(
    &self,
    kind: ItemKind,
    out: &mut Optional<u64>
) {
    transition {
        _ -> find_item_at(kind, 0, out)
    }

    state find_item_at(
        &self,
        kind: ItemKind,
        index: u64,
        out: &mut Optional<u64>
    ) {
        let found: bool = self.items[index].kind == kind;
        let next_index: u64 = index + 1;
        let has_next: bool = next_index < self.items.len;

        transition (found, has_next) {
            (true, _) -> found_item(index, out)
            (false, true) -> find_item_at(kind, next_index, out)
            (false, false) -> not_found(out)
        }
    }

    state found_item(
        index: u64,
        out: &mut Optional<u64>
    ) {
        out = Some(index);
    }

    state not_found(out: &mut Optional<u64>) {
        out = None;
    }
}
```

The call-shaped syntax in a transition arm is argument passing for a jump. It is
not method dispatch. A target state's declared `self` parameter is carried as
the machine attachment context and is not repeated in the transition argument
list; only non-`self` state parameters are explicit jump arguments.

Machine parameters are roots owned by the current activation, but they are not
ambient names inside every state. A state may observe, mutate, move, or use only
the values and authority named by its own parameters. Transitions spell how
those places reach the target.

This explicit source frontier does not require runtime copying. The lowered
transfer map may preserve one canonical obligation across renamed state places,
and storage planning may assign the source and target places to the same slot.
Proof and debug artifacts retain the mapping even when its physical realization
uses no instructions.

Keeping state parameters explicit also keeps ranking arguments, invariants,
borrow dependencies, and authority local to the edges that must re-establish
them. Machine-wide implicit capture remains unsupported.

## Data Patterns

A transition over ordinary data may destructure fields and match selected
fields by value:

```omega
transition header {
    Header { ok: 0, version } -> accept(version)
    Header { ok as _, version as _ } -> reject()
}
```

`version` binds to `header.version`; `ok: 0` is a real equality guard over
`header.ok`, so the arm carries the ordinary proof fact `header.ok == 0`.
`field as name` renames a binding and `field as _` explicitly waives it. A
pattern without `..` must mention every field, whether bound, waived, or
matched; adding a field therefore breaks every exhaustive pattern until its
author decides what to do. `..` opts out of that drift check in arm position.

The same spelling applies to case payloads (`Message::Data { kind: 1, body }`).
The subject is evaluated once before dispatch, and extraction reads that saved
value. Field-value patterns are ordinary projection plus equality, not a
second pattern-only fact or comparison system.

## No Silent Fall-Through

A transition dispatch must PROVABLY cover every case: a dispatch that could
reach runtime with no matching arm is a **compile error**, never a behavior.
Coverage the compiler counts: a `_` arm; full case coverage over a sum
subject; full case coverage over an ordinary historical-lineage sum; a `true ->` plus
`false ->` pair over one boolean subject; and a complementary `x == k ->` plus
`x != k ->` pair over one subject and value. Anything else — value matches,
comparison ladders, predicate guards — must close with `_`. An intentional
"stop here" arm is spelled explicitly: `_ -> {}`.

## Terminal Completion

A machine or state can complete by producing the machine's declared result.

```omega
data Controller {
}

machine Controller::run(&mut self) -> i32 {
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
