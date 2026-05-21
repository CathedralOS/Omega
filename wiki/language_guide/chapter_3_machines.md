# Chapter 3: Machines

A machine is the callable boundary.

Machines may be attached to data, or free-standing when there is no natural
owning data type.

## Attached Machines

Attached machines operate on a named data type.

```omega
data Player {
    health: i32;
}

machine Player::take_damage(
    &mut self,
    amount: i32
) {
    self.health = self.health - amount;
}
```

`self` is explicit. If the machine mutates the receiver, it takes `&mut self`.

## Free-Standing Machines

Free-standing machines are ordinary machines without a data receiver.

```omega
machine clamp_i32(
    value: i32,
    min: i32,
    max: i32
) -> i32 {
    transition value < min {
        true -> min
        false -> clamp_high(value, max)
    }

    state clamp_high(value: i32, max: i32) {
        transition value > max {
            true -> max
            false -> value
        }
    }
}
```

Use a free-standing machine for math helpers, proof helpers, and operations
that are not naturally owned by one data type.

## Program Entry

Executable programs should use an explicit root data type.

```omega
data Main {
    game: Game;
}

machine Main::main(&mut self) -> i32 {
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

The process entry is the `main` machine on the `Main` root data object. Startup
allocates the root object, then enters `Main::main(&mut root)`.

This keeps process-owned state under one explicit owner.

## Parameters And Returns

Machine parameters are entry data. A machine return type is the value shape its
graph eventually produces.

```omega
machine Parser::resolve(
    &mut self,
    line: &ConsoleLine
) -> Command {
    transition line.text {
        "quit" -> Command::Quit
        "look" -> Command::Look
        _ -> Command::Invalid
    }
}
```

Every reachable terminal path in a typed machine must produce a compatible
return value.

## Calls

Ordinary call syntax enters a machine and creates a call frame.

```omega
let command: Command = self.parser.resolve(&self.line);
```

Calls and transitions are different. A call enters another machine. A transition
jumps to a state inside the current machine.

## Contracts

Machines may declare requirements and guarantees.

```omega
machine Player::enter_combat(&mut self)
requires
    self in Player::Alive
ensures
    self in Player::InCombat
{
}
```

The caller must satisfy `requires`. The machine body must establish `ensures`.

## Machine Graph Compatibility

Internal states participate in the machine's graph, but they are not public
machine entries.

Working rules:

- State-transition arguments must match the target state's parameters.
- Terminal values must satisfy the active machine's return type.
- Every reachable terminal path in a typed machine graph must produce the
  declared return type.
- Transition dispatch arms add proof assumptions for the target edge.
