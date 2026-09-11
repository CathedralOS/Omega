# Chapter 1: Data, Values, And Literals

Omega programs use explicit data shapes and ordinary machines. This chapter
introduces values; [data and literal rules](../spec/language/data_and_literals.md)
specify the source contract.

Fields have no hidden default initializers. A constructor may omit a runtime field only
when zero satisfies the complete data validity requirements. Nonzero defaults
belong in ordinary constructor machines.
Erased proof fields use their [construction rule](../spec/proofs/contracts.md#explicit-erased-bindings),
not zero-filling.

## Hello World

A console entry uses its admitted Console provider:

```omega
use omega_language_std::console;

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    self.console.write_line("Hello, Omega.");
}
```

The project's `build.omg` explicitly declares the application and selects its
entry:

```omega
machine build(builder: &mut Build) {
    builder.application("hello");
    // Declare the std dependency for this project's source layout.
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
```

These are the central source forms, not a complete package setup. The
[CLI sample](../../samples/cli/basics/cli_mvp/main.omg) and its
[build file](../../samples/cli/basics/cli_mvp/build.omg) include dependency
declarations and target bindings.

The invocation selects an exact target; naming a binding does not select it.
The target's entry bridge provisions one receiver and the admitted storage and
providers before calling the entry. There is no special discovery of a machine
named `main`. A free machine may also be selected when no receiver is needed.
Other values of `Main` remain ordinary values: the data type itself grants no
entry authority. See [entry roots](../spec/build/entry_roots.md).

## Data

A record owns its fields:

```omega
data Player {
    name: [u8; 64];
    health: i32;
    armor: i32;
}

machine Player::take_damage(&mut self, amount: i32)
requires amount >= 0
requires self.health >= amount
{
    self.health = self.health - amount;
}
```

The receiver parameter supplies access; the machine does not implicitly own
fields. `self.health` identifies stored state, while `amount` is an explicit
input. The preconditions establish that the subtraction is in range.

## Case Members (Sum Shapes)

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

A value inhabits one case. Its active payload belongs to the value. Common
fields can accompany the cases:

```omega
data RoomEvent {
    consumed: bool;
    case Nothing;
    case Treasure(gold: u32);
}

let event = RoomEvent::Treasure { consumed: true, gold: 5 };
```

This mixed shape has `consumed` in every case, with a case-specific payload.
Common fields can be accessed without a case test; payload fields need the
matching case. Case-bearing types use case construction, not a record literal
that omits the case.

Omitted fields still need valid zero values. Zeroed storage selects the first
case, but observation requires its common fields and active payload to be
established. Zero does not prove an authority qualification or mean “empty.”
[Chapter 20](chapter_20_memory_layout_abi.md#zeroed-storage-and-establishment)
explains that distinction.

Cases are not integer aliases. Foreign numeric codes use their declared integer
carrier and an explicit checked mapping to a sum, including unknown-code
handling. Stable `#N` member numbers are schema identities, not runtime integer
values or byte offsets.

## Constraints On Individual Cases

A case can have its own `where` clause:

```omega
data Value<T> {
    case Integer(value: i32) where T == i32;
    case Boolean(value: bool) where T == bool;
}

data Interval {
    case Empty;
    case Range(lo: u64, hi: u64) where lo <= hi;
}
```

The `Integer` case belongs only to `Value<i32>`; it cannot construct `Value<bool>`.
Matching Integer reveals `T == i32` in that arm, allowing its `i32` payload to be
used as `T` without a cast. This is GADT-style type refinement through a case.

The Range case shows the same mechanism for ordinary values. Constructing it
must prove `lo <= hi`; matching it supplies that fact. The clause may use enclosing
generic parameters, common fields, and its own payload, under the ordinary
contract rules. It is not restricted to type equality and runs no implicit
validator. Changes must re-establish the invariant before observation.

Type-wide constraints still apply. Zeroed `Value<bool>` does not skip Integer to
become Boolean: it is unestablished storage until valid construction. A generic
body can observe zeroed storage only after proving it valid from its own
contract and flow facts, not because one later instantiation happens to work.

These clauses do not introduce hidden case-local generic types or automatic
boxing. See the [case-constraint specification](../spec/language/data_and_literals.md#case-constraints)
and [matching rules](chapter_6_pattern_matching_dispatch.md). The compiler work
is tracked separately as CASE-CONSTRAINTS in [TASKS.md](../../TASKS.md);
the examples describe the language contract, not completed implementation.

## Cases Are Domains

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

### Equality Vs Membership

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
let next_health: i32 = self.health - amount;
self.health = next_health;
```

The subtraction needs the same range proof as any other arithmetic. The local's
scope and ownership determine how long it may be used; introducing it does not
make it part of the data layout.

## Constants

A `const` names an evaluated value, not one addressable storage occurrence:

```omega
pub const PAGE_SIZE: u64 = 4096;
pub const IMPORT_NAME: [u8; 9] = "WriteFile";
```

Constants may be package/module-scoped or genuinely attached to a type. Their
complete type must permit copying without cleanup, shared ownership, or interior
mutability. Arrays and records can qualify; constants are not scalar-only.

Initializers use ordinary [semantic evaluation](../spec/language/evaluation.md).
There is no separate `const machine` category. Target-dependent constants retain
their exact target dependencies even after folding. Runtime use additionally
needs determined bytes under the selected layout; proof-only use may erase.

A constant grants no authority. Omega has no ambient mutable `static`; long-lived
entry state is one explicitly provisioned occurrence. See
[constants](../spec/language/constants.md).

## Lexical Profile V1

Source is UTF-8 with ASCII syntax. Identifiers use
`[A-Za-z_][A-Za-z0-9_]*`; whitespace is space, tab, carriage return, or line feed.
Non-ASCII bytes are admitted inside comments and literal bodies, not identifiers.
Invisible Unicode whitespace is not an alternative separator.

The [lexical profile](../spec/language/data_and_literals.md#lexical-profile)
keeps tokenization independent of host Unicode tables.

## String Literals And Bytes

A quoted literal is a shared byte view, not automatically text:

```omega
let greeting = "Hello, Omega."; // &[u8], no encoding qualification
pub const DLL_NAME: [u8; 12] = "kernel32.dll";
```

The literal view has immutable image backing. An exact-width owned array context
instead copies its bytes; the length must match exactly, without padding or a
hidden live-length field.

Directly authored `"café"` copies the editor's UTF-8 bytes. Byte escapes such as
`\n`, `\0`, and `\xNN` specify bytes without choosing a text encoding. Quotes
and backslashes use `\"` and `\\`. Raw newlines and codepoint escapes are not
accepted inside quotes; encoders and normalizers are library operations.

Establish a text domain explicitly when needed, for example
`"hi" as [u8]::Utf8` under the imported domain's checked rules. Source UTF-8
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

## Stored Values And Proof Facts

A field's representation and its validity conditions are separate:

```omega
data Player {
    health: i32 in 0..=100;
}
```

The stored carrier is still `i32`. Construction and consumption must establish
the range; mutation follows the invariant-window rules rather than adding hidden
runtime checks. [Contracts and flow facts](chapter_7_types_constraints_invariants.md)
introduces that model.

## Foundation

Data owns fields; machines receive access; state transitions transfer explicit
values within an activation. Later chapters build on those rules without adding
hidden storage or ambient authority.
