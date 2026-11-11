# Chapter 1: Data, values, and literals

Omega programs use explicit data shapes and ordinary machines. This chapter
introduces values; [data and literal rules](../spec/language/data_and_literals.md)
specify the source contract.



## Hello world

A console entry uses its admitted Console provider:

```omega
use omega_language_std::console;

machine start() {
    Console::write_line("Hello, Omega.");
}
```

The project's `build.omg` explicitly declares the application and selects its
entry:

```omega
machine build(builder: &mut Build) {
    Build::application(builder, "hello");
    // Declare the std dependency for this project's source layout.
    Build::bind(builder, windows_x86_64::ProgramEntry, start);
}
```

These are the central source forms, not a complete package setup. The
[CLI sample](../../samples/cli/basics/cli_mvp/main.omg) and its
[build file](../../samples/cli/basics/cli_mvp/build.omg) include dependency
declarations and target bindings.

The build selects the entry and the console provider for the target. A private
machine such as `start` can infer its use of the console from its body.
[Chapter 19](chapter_19_capabilities_effects_boundaries.md) explains providers.

## Data

A record owns its fields:

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

The ordinary `player` parameter supplies access; the machine does not implicitly
own fields. `player.health` identifies stored state, while `amount` is another
explicit input. Call it as `take_damage(&mut player, amount)`. The
preconditions establish that the subtraction is in range.

## Explicit zero defaults

An exhaustive literal asks the compiler to catch any field you forgot. Add
`..` only when you intend omitted fields, including future additions, to use
valid zero values:

```omega
data Options {
    timeout_ms: u32;
    retries: u32;
}

let exact: Options = Options { timeout_ms: 1000, retries: 0 };
let defaults: Options = Options { timeout_ms: 1000, .. };
let forgotten: Options = Options { timeout_ms: 1000 }; // Rejected.
```

If Options gains another runtime field, `exact` requires an update; `defaults`
accepts its zero value only if the complete value remains valid. If a predicate
requires `retries > 0`, the defaulted construction also rejects. The marker
does not invent a nonzero default or call a constructor.

`Options { .. }` requests zero for every runtime field. The marker is last,
occurs at most once and has no operand. It is not record update syntax. An
explicitly written nested literal needs its own `..` if it omits fields.
Erased-field construction retains its independent proof rules.

## Case members (sum shapes)

Alternatives are cases inside ordinary data:

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
```

A value inhabits one case. Its active payload belongs to the value.

A `data` declaration is a record or a sum, never both. To put common fields
beside alternatives, hold the sum in a record's field:

```omega
data RoomEvent {
    consumed: bool;
    found: Found;

    data Found {
        case Nothing;
        case Treasure(gold: u32);
    }
}

let event: RoomEvent = RoomEvent {
    consumed: true,
    found: RoomEvent::Found::Treasure { gold: 5 },
};
let empty: RoomEvent = RoomEvent { found: RoomEvent::Found::Nothing, .. };
```

`event.consumed` needs no case test; `gold` needs the `Treasure` case of
`event.found`. `RoomEvent::Found` is an ordinary sum: build one before there is
an event, pass `&event.found` to a helper, move it out, or store it elsewhere.

Nesting the declaration only scopes its name. A nested `data` adds no field by
itself, inherits its owner's visibility and generic parameters, and can be
declared together with its field:

```omega
data RoomEvent {
    consumed: bool;

    found: data Found {
        case Nothing;
        case Treasure(gold: u32);
    };
}
```

Both spellings mean the same thing. A sum that several records hold is declared
at module scope and held in a field the same way. See
[data and case patterns](chapter_6_pattern_matching_dispatch.md#data-and-case-patterns).

In a case literal, `..` covers omitted fields of the active payload; the case
itself is still selected explicitly. Omitted fields still need valid zero
values. Separately, zeroed storage selects a sum's first case, but observation
requires the record's fields and the active payload to be established. Zero
does not prove an authority qualification or mean “empty.”
[Chapter 20](chapter_20_memory_layout_abi.md#zeroed-storage-and-establishment)
explains that distinction.

Cases are not integer aliases. Foreign numeric codes use their declared integer
carrier and an explicit checked mapping to a sum, including unknown-code
handling. Stable `#N` member numbers are schema identities, not runtime integer
values or byte offsets.

Numbered members put the identity first, including alternatives:

```omega
data Reading {
    #1 sensor_id: u32;
    #2 recorded_at_ms: u64;
    #3 sequence: u32;
    #4 state: State;

    data State {
        #1 case WarmingUp;
        #2 case Sample(#1 temperature_celsius: i32);
        #3 case Fault(#1 code: u32);
    }
}
```

Each declaration and each payload has its own number scope. Use these numbers for stable schema identity, as described
in [Chapter 20](chapter_20_memory_layout_abi.md#relationship-to-serialized-bytes).

## Constraints on individual cases

Use the data's default domain to constrain its active case. `self` names the
whole value in that predicate:

```omega
data Value<T>
    requires ((self in Value::Integer) && T == i32)
       || ((self in Value::Boolean) && T == bool)
{
    case Integer(value: i32);
    case Boolean(value: bool);
}

data Interval
    requires (self in Interval::Empty)
       || ((self in Interval::Range) && self.lo <= self.hi)
{
    case Empty;
    case Range(lo: u64, hi: u64);
}
```

The `Integer` case belongs only to `Value<i32>`; it cannot construct `Value<bool>`.
Matching Integer reveals `T == i32` in that arm, allowing its `i32` payload to be
used as `T` without a cast. This is GADT-style type refinement through a case.

The Range case shows the same mechanism for ordinary values. Constructing it
must prove `lo <= hi`; matching it supplies that fact. The case test on the left
of `&&` justifies reading the payload on its right. Reading an inactive payload
is invalid, even inside a predicate. The data clause may use generic parameters,
fields and guarded projections of `self`. A record tests a sum it holds through
the field, as in `self.found in RoomEvent::Found::Treasure`. It runs no implicit validator.
Changes must re-establish the invariant before observation.

Type-wide constraints still apply. Zeroed `Value<bool>` does not skip Integer to
become Boolean: it is unestablished storage until valid construction. A generic
body can observe zeroed storage only after proving it valid from its own
contract and flow facts, not because one later instantiation happens to work.

Named domains can express similar conditions for a subset of values. The data's
default domain applies to every established value. See the
[case-constraint specification](../spec/language/data_and_literals.md#case-constraints).

## Refine a value by its case

Each case also names the domain of values inhabiting it:

```omega
domain Command::Interactive
    requires self in Command::Move | Command::Say;
```

This is a subset of `Command`, not another enum with copied cases. Case
patterns and domain patterns share `Type::Name` spelling. A payload binding
such as `Command::Move { direction }` additionally exposes the case shape.

Patterns are ordered. A selected domain pattern supplies its facts, but an
executable test must be runtime-checkable and cannot mint routed provenance.
Finite case unions can support exhaustiveness; arbitrary predicate patterns
need a fallback unless coverage is established. See
[dispatch](chapter_6_pattern_matching_dispatch.md) and
[domains](chapter_8_domains.md).

### Equality vs membership

`==` compares values; `in` asks domain membership:

This example assumes `Command` has its required `Equatable` conformance:

```omega
let quit: bool = command == Command::Quit;
let moving: bool = command in Command::Move;
let interactive: bool = command in Command::Move | Command::Say;
```

`Command::Move` alone is not a value because it needs a direction payload.
Construct one to compare whole values. Structural equality compares common
fields, the active case, and its payload—not only a tag.

Primitive and payload-free-sum equality is intrinsic. Records and payload-bearing
sums declare their selected `Equatable` synthesis or conformance. Adding a
payload therefore requires an explicit equality decision. See
[core equality](../spec/language/conformances.md#core-equality-acquisition).

## Locals

Locals are temporary bindings, not stored fields:

```omega
let amount: i32 = 4;
let next_health: i32 = player.health - amount;
player.health = next_health;
```

The subtraction needs the same range proof as any other arithmetic. The local's
scope and ownership determine how long it may be used; introducing it does not
make it part of the data layout.
Values needed by a later nested machine are passed as machine arguments. A temporary
buffer or calculation does not become a field merely because several operations
use it; fields describe state whose lifetime belongs to the record.

## Constants

A `const` names an evaluated value, not one addressable storage occurrence:

```omega
public const PAGE_SIZE: u64 = 4096;
public const IMPORT_NAME: [u8; 9] = "WriteFile";
```

Constants may be package/module-scoped or genuinely attached to a type. Their
complete type must permit copying without cleanup, shared ownership, or interior
mutability. Arrays and records can qualify; constants are not scalar-only.

Initializers use ordinary [semantic evaluation](../spec/language/evaluation.md).
There is no separate `const machine` category. Target-dependent constants retain
their exact target dependencies even after folding. Runtime use additionally
needs determined bytes under the selected layout; proof-only use may erase.

A constant grants no authority and is not static mutable storage. Long-lived
state uses ordinary owned storage, including static placement under its normal
access rules; static placement creates no ambient mutable capability or special
entry instance. See [constants](../spec/language/constants.md) and
[ordinary entry storage](../spec/build/entry_roots.md#entry-shape-and-arrival-bridge).

## Names and whitespace

Source is UTF-8 with ASCII syntax. Identifiers use
`[A-Za-z_][A-Za-z0-9_]*`; whitespace is space, tab, carriage return, or line feed.
Non-ASCII bytes are admitted inside comments and literal bodies, not identifiers.
Invisible Unicode whitespace is not an alternative separator.

The [lexical profile](../spec/language/data_and_literals.md#lexical-profile)
keeps tokenization independent of host Unicode tables.

## String literals and bytes

A quoted literal is a shared byte view, not automatically text:

```omega
let greeting: &[u8] = "Hello, Omega."; // no encoding qualification
public const DLL_NAME: [u8; 12] = "kernel32.dll";
```

The literal view has immutable image backing. An exact-width owned array context
instead copies its bytes; the length must match exactly, without padding or a
hidden live-length field.

Directly authored `"café"` copies the editor's UTF-8 bytes. Byte escapes such as
`\n`, `\0`, and `\xNN` specify bytes without choosing a text encoding. Quotes
and backslashes use `\"` and `\\`. Raw newlines and codepoint escapes are not
accepted inside quotes; encoders and normalizers are library operations.

Establish a text domain explicitly when needed, for example
`"hi" as +Utf8` under the imported domain's checked rules. Source UTF-8
framing alone does not establish that qualification.

## Parameters

Signatures say which inputs are values and which are borrowed:

```omega
machine inspect_and_update(
    source: &Player,
    destination: &mut Player,
    amount: i32
) {
    // Ordinary checked operations use these explicit inputs.
}
```

There is no implicit capture of ambient process state. Shared and exclusive
access, transfers, and cleanup are covered in
[Chapter 2](chapter_2_ownership_borrowing_moves.md).

## Stored values and proof facts

A field's representation and its validity conditions are separate:

```omega
data Player
requires health in 0..=100
{
    health: i32;
}
```

The stored carrier is still `i32`. Construction and consumption must establish
the range; mutation follows the invariant-window rules rather than adding hidden
runtime checks. [Contracts and flow facts](chapter_7_types_constraints_invariants.md)
introduces that model.
