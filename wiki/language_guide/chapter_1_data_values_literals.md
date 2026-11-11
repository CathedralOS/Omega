# Chapter 1: Data, values, and literals

An Omega program is data plus machines that work on it. This chapter covers
the data half: records, sums, literals, locals and constants.

## Hello world

```omega
use omega_language_std::console;

machine start() {
    Console::write_line("Hello, Omega.");
}
```

A project's `build.omg` names the application and says which machine is its
entry:

```omega
machine build(builder: &mut Build) {
    Build::application(builder, "hello");
    Build::bind(builder, windows_x86_64::ProgramEntry, start);
}
```

The [CLI sample](../../samples/cli/basics/cli_mvp/main.omg) and its
[build file](../../samples/cli/basics/cli_mvp/build.omg) show a complete
project, including the standard-library dependency.
[Chapter 19](chapter_19_capabilities_effects_boundaries.md) explains where
`Console` comes from.

## Records

A record is a set of named fields:

```omega
data Player {
    name: [u8; 64];
    health: i32;
    armor: i32;
}

machine take_damage(player: &mut Player, amount: i32)
requires amount >= 0
requires player.health >= amount
{
    player.health = player.health - amount;
}
```

Data has no methods. A machine receives the value it works on as an ordinary
parameter and is called as `take_damage(&mut player, 4)`. The two `requires`
lines are what make the subtraction provably in range;
[Chapter 7](chapter_7_types_constraints_invariants.md) covers them.

## Building a record

A literal names every field. Write `..` last to ask for zero in the fields you
leave out:

```omega
data Options {
    timeout_ms: u32;
    retries: u32;
}

let exact: Options = Options { timeout_ms: 1000, retries: 0 };
let defaults: Options = Options { timeout_ms: 1000, .. };
let forgotten: Options = Options { timeout_ms: 1000 }; // Rejected.
```

When `Options` gains a field, `exact` stops compiling until you supply it, and
`defaults` takes the zero. There are no per-field default values: a
constructor machine supplies anything other than zero.

## Sums

A sum is a set of cases. A value is in exactly one of them, and a case may
carry a payload:

```omega
data Direction {
    case None;
    case North;
    case South;
}

data Command {
    case None;
    case Quit;
    case Move(direction: Direction);
    case Say(text: [u8; 256]);
}

let quit: Command = Command::Quit;
let step: Command = Command::Move { direction: Direction::North };
```

Case names are always written with their type, `Command::Quit`, never `Quit`.

## Fields beside cases

A `data` declaration is a record or a sum, never both. To have common fields
next to alternatives, hold the sum in a field:

```omega
data RoomEvent {
    consumed: bool;

    found: data Found {
        case Nothing;
        case Treasure(gold: u32);
    };
}

let event: RoomEvent = RoomEvent {
    consumed: true,
    found: RoomEvent::Found::Treasure { gold: 5 },
};
```

Writing `data Found { ... }` in the field declares the type and the field
together. The type is named `RoomEvent::Found` and is an ordinary sum you can
build, pass and store separately. Declaring it at module scope and writing
`found: Found` works the same way, and is what you do when several records
hold the same sum.

`event.consumed` needs no case test. Reading `gold` requires knowing that
`event.found` is a `Treasure`, which is what
[matching](chapter_6_pattern_matching_dispatch.md#data-and-case-patterns)
establishes.

## Equality and membership

`==` compares values. `in` asks which case, or which
[domain](chapter_8_domains.md), a value is in:

```omega
let quit: bool = command == Command::Quit;
let moving: bool = command in Command::Move;
let interactive: bool = command in Command::Move | Command::Say;
```

`Command::Move` is not a value by itself because it needs a payload, so it
appears with `in`, not `==`. Equality on records and payload-carrying sums is
something the type [declares](chapter_14_traits.md#synthesized-core-traits);
it is built in for scalars and payload-free sums.

## Constraints on individual cases

A `requires` clause on the data states what must hold for every value. `self`
is the whole value, and a case test on the left of `&&` permits reading that
case's payload on the right:

```omega
data Interval
    requires (self in Interval::Empty)
       || ((self in Interval::Range) && self.lo <= self.hi)
{
    case Empty;
    case Range(lo: u64, hi: u64);
}
```

Constructing a `Range` must prove `lo <= hi`, and matching one hands that fact
back. The same clause can tie a case to a type parameter:

```omega
data Value<T>
    requires ((self in Value::Integer) && T == i32)
       || ((self in Value::Boolean) && T == bool)
{
    case Integer(value: i32);
    case Boolean(value: bool);
}
```

Matching `Integer` shows `T == i32` in that arm, so the payload can be used as
a `T` without a cast. A record tests a sum it holds through the field:
`self.found in RoomEvent::Found::Treasure`. See
[case constraints](../spec/language/data_and_literals.md#case-constraints).

## Locals

```omega
let amount: i32 = 4;
let next_health: i32 = player.health - amount;
player.health = next_health;
```

A local is a temporary binding, not stored state. A nested machine that needs
one receives it as an argument.

## Constants

```omega
public const PAGE_SIZE: u64 = 4096;
public const IMPORT_NAME: [u8; 9] = "WriteFile";
```

A constant names a value computed at compile time. It can be an array or a
record, not only a scalar, and it is not mutable storage. See
[constants](../spec/language/constants.md).

## Strings are bytes

```omega
let greeting: &[u8] = "Hello, Omega.";
public const DLL_NAME: [u8; 12] = "kernel32.dll";
```

A quoted literal is a view of bytes. Given a fixed-size array type it is
copied instead, and the length must match exactly. Escapes are `\n`, `\0`,
`\xNN`, `\"` and `\\`. A literal is not text until you say so, for example
`"hi" as +Utf8`; [Chapter 8](chapter_8_domains.md) explains that cast.

## Stable member numbers

Data that is serialized can number its members so that renaming or reordering
does not change the format:

```omega
data Reading {
    #1 sensor_id: u32;
    #2 recorded_at_ms: u64;
    #3 state: State;

    data State {
        #1 case WarmingUp;
        #2 case Sample(#1 temperature_celsius: i32);
        #3 case Fault(#1 code: u32);
    }
}
```

Each declaration and each payload numbers its own members. The numbers
identify members in a schema; they are not tag values or byte offsets.
[Chapter 20](chapter_20_memory_layout_abi.md#relationship-to-serialized-bytes)
covers serialization.

## Where next

[Chapter 2](chapter_2_ownership_borrowing_moves.md) explains `&`, `&mut` and
moving values. The complete rules for this chapter are in
[data and literals](../spec/language/data_and_literals.md).
