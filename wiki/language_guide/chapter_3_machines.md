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
machine add_i32(
    left: i32,
    right: i32
) -> i32 {
    left + right
}
```

Use a free-standing machine for math helpers, proof helpers, and operations
that are not naturally owned by one data type.

## Program Entry

Executable programs should use an explicit root data type.

```omega
data Main {
    total: i32;
}

machine Main::main(&mut self) -> i32 {
    self.total = add_i32(3, 4);
    self.total
}
```

The process entry is the `main` machine on the `Main` root data object. Startup
allocates the root object, then enters `Main::main(&mut root)`.

This keeps process-owned state under one explicit owner.

## Parameters And Returns

Machine parameters are entry data. A machine return type is the value shape its
body or internal state graph eventually produces.

```omega
machine Parser::resolve(
    &self,
    line: &String
) -> Command {
    Command::Invalid
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
jumps to a state inside the current machine. Chapter 4 introduces states and
transitions directly.

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
