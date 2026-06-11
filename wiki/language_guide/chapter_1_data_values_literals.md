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

## Case Members (Sum Shapes)

Omega does not have a separate `enum` type. Alternatives are a MEMBER CLASS of
`data`: a `case` member declares one shape of a closed set, and a value
inhabits exactly one case at a time.

```omega
data Direction {
    case None;
    case North;
    case South;
    case East;
    case West;
}
```

Cases may carry named payload fields, owned by the value exactly like ordinary
fields:

```omega
data Command {
    case None;
    case Quit;
    case Move(direction: Direction);
    case Say(text: String);
}
```

A declaration's shape follows from its members: only fields is a RECORD, only
cases is a SUM, fields AND cases together is MIXED -- common fields shared by
every case, plus a case part:

```omega
data RoomEvent {
    consumed: bool;                 // present in every case
    case Nothing;
    case Treasure(gold: u32);
    case Enemy(enemy: Enemy);
}
```

The mixed shape replaces the two-type split other languages force (a struct
holding a separately-named `Kind` enum). The header and the tag belong to one
declaration, so the compiler -- and the wire schema, and the version block --
sees them as one thing. Sequencing: sum-only ships first; the mixed shape is
a severable later step (nesting a sum-shaped field expresses the same data in
the meantime), even though the compiler's trees already model it.

## Cases Are Domains

Declaring a case implicitly declares the same-named domain: `case Move(...)`
on `Command` declares `Command::Move`, the set of values whose tag is `Move`,
with a free constant-time classifier (a tag compare). `case` therefore never
appears at a USE site. Checks, patterns, and compositions all use the one
`Type::Name` spelling and the ordinary domain algebra
([Domains](chapter_8_domains.md)):

```omega
domain Command::Interactive when self in Command::Move | Command::Say;
```

A case-subset domain replaces the shadow-enum pattern (`Direction` vs
`HorizontalDirection`): a narrower set of cases is a union of case-domains
over the same type, not a new type.

Match arms are CLASSIFICATIONS -- case arms and domain arms mix freely in one
match, spelled identically, and the first satisfied arm wins (the same rule
transitions use):

```omega
match entity {
    Entity::Dead -> loot()              // domain (predicate)
    Entity::Monster { ai } -> hunt(ai)  // case, payload bound
    Entity::Hostile -> flee()           // domain (case union)
    _ -> ignore()
}
```

Working rules:

- The FIRST case is the zero case: its tag is `0`, so a zeroed value is the
  first case. Zero validity is unconditional; declaring the empty/none-like
  case first (and payload-free) is the rule of the opt-in `zero_init`
  property, recommended style otherwise (see
  [Memory Layout And ABI](chapter_19_memory_layout_abi.md)).
- The subject's shape decides what an arm can be: a scalar subject takes
  value patterns, a record subject takes domain arms, a case-bearing subject
  takes case arms and domain arms together.
- Payload binding (`Entity::Monster { ai }`) is legal only on a case arm,
  because only a case implies a payload shape. A binding arm is therefore
  visibly a case; an unbound `Type::Name` arm requires the declaration (or
  tooling) to tell case from domain -- accepted, since wanting domains over
  case-bearing types makes that ambiguity intrinsic.
- Exhaustiveness counts DECIDABLE arms: case arms and pure case-union
  domains (those are finite tag sets). A match relying on any predicate
  domain needs a `_` arm.
- Cases, domains, and machines share the type's `Type::member` namespace.
  Names must be unique; any collision -- including a later domain or machine
  declared against an existing case -- is a hard compile error. There is no
  shadowing and no resolution priority: silently rebinding what a match arm
  means is never acceptable.
- The compiler never repurposes invalid payload bit patterns to elide the tag
  (no niche optimization); the zero bit pattern must stay a valid value.

Transitional note: today's compiler spells variant-only sums with an `enum`
keyword and rejects payloads. `enum` is retired by this decision; `case`
members, payloads, and the implicit case-domains are pending parser/lowering
work.[^case-members]

[^case-members]: Open details: payload-binding spelling in `transition` arms
vs `match` arms (expected to reuse the data-destructure guard machinery);
generic payloads (`Option<T>`-style); the layout rule for payload storage
(tag-prefixed overlay with the zero case payload-free); and whether a domain
DECLARED as a pure case union gets recognized for exhaustiveness
syntactically or by classifier analysis.

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
