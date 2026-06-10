# Chapter 1: Data, Values, And Literals

Omega starts with explicit data shapes and explicit values.

## Hello World

The smallest console program has one root data object and one entry machine.

```omega
use omega::language::std::console;

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    self.console.write_line("Hello, Omega.");
    self.console.exit_process(0);
}
```

`Main` owns the process state. `Main::main` is the process entry machine.
The string literal is passed to the console capability stored on `Main`.

## Data

`data` declarations describe stored state. Fields inside `data` are owned by
that value.

```omega
data Player {
    name: String;
    health: i32;
    armor: i32;
}
```

Machines describe behavior over data. A machine receives access to data through
its signature.

```omega
machine Player::take_damage(
    &mut self,
    amount: i32
) {
    self.health = self.health - amount;
}
```

Working interpretation:

- `data` owns its fields.
- Machines do not implicitly own fields.
- A machine can mutate receiver state through `&mut self`.
- A machine can read receiver state through `&self`.
- Other inputs arrive as explicit parameters.
- Locals are temporary values inside a machine or state body.
- Mutation should be visible through `&mut` parameters or `self.` field access.

The `self.` prefix is intentionally visible. It lets a reader distinguish stored
state from locals and parameters at a glance.

## Enums

`enum` declarations describe a closed set of alternatives: a value is exactly
one named variant at a time.

```omega
enum Direction {
    None,
    North,
    South,
    East,
    West,
}
```

The direction is Rust-like sum types: variants may carry typed payloads, and
matching a variant binds its payload.

```omega
enum Command {
    None,
    Quit,
    Move(Direction),
    Say(String),
}
```

Working rules:

- The FIRST variant is the zero variant: its tag is `0`, and it should be the
  empty/none-like case. This is what makes a zeroed enum a valid value (see
  [Memory Layout And ABI](chapter_19_memory_layout_abi.md) on zero
  initialization).
- Variant payloads are owned by the value, exactly like `data` fields.
- Matching is exhaustive: every variant is handled or a `_` arm exists.
- The compiler never repurposes invalid payload bit patterns to elide the tag
  (no niche optimization); the zero bit pattern must stay a valid value.

Payload-carrying variants are a committed direction, not yet an implemented
one: today the compiler accepts payload-less enums end-to-end, and rejects
payload declarations at parse time.[^enum-payloads]

[^enum-payloads]: Open details for payload support: pattern-binding syntax in
`transition` arms vs `match` arms, generic payloads (`Option<T>`-style), and
the layout rule for payload storage (tag-prefixed union with the zero variant
payload-free). The no-niche rule above is decided; the rest should be settled
when the parser work starts.

## Locals

Locals are values introduced inside executable machine/state bodies.

```omega
machine Player::heal(
    &mut self,
    amount: i32
) {
    let next_health: i32 = self.health + amount;
    self.health = next_health;
}
```

Locals are not data fields. They do not become part of the data layout and they
do not survive outside the graph paths where their lifetime is valid.

## Parameters

Parameters are explicit entry values.

```omega
machine Combat::strike(
    attacker: &Player,
    defender: &mut Player,
    damage: i32
) {
    defender.health = defender.health - damage + attacker.armor;
}
```

Working interpretation:

- `attacker` is a shared borrow.
- `defender` is a unique mutable borrow.
- `damage` is a value parameter.
- Nothing is implicitly captured from ambient process state.

## Stored Values And Proof Facts

Stored fields may eventually carry proof-visible constraints, but constraints
are not part of Chapter 1's core model.

```omega
data Player {
    health: i32;
}
```

That syntax means `health` is still represented as an `i32`, with additional
proof obligations attached to assignments and transitions that can change it.
The constraint story is covered later in the invariants/proof chapters.

## Foundation

The foundation is:

- Data shape is explicit.
- Behavior is explicit.
- Access is explicit.
- There is no hidden machine-owned field declaration syntax.
- There is no implied stack magic behind state transitions.

Later chapters build on this by adding states, transitions, typed returns,
constraints, domains, invariants, traits, and runtime dispatch.
