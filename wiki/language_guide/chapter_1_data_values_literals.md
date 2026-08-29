# Chapter 1: Data, Values, And Literals

Omega starts with explicit data shapes and explicit values.

> **No default *values* on data.** A field may not carry a `= default`
> initializer. Construction may omit a field only when zero satisfies the
> data's [default domain](chapter_7_types_constraints_invariants.md); otherwise
> that field is mandatory. Non-zero convenience defaults belong in explicit
> constructor machines such as `Config::with_defaults()`, never hidden field
> initializers. The parser rejects field initializers for every data shape.

## Hello World

The smallest console program has a free entry machine and no implicit state.

```omega
use omega::language::std::console;

machine start() {
    Console::write_line("Hello, Omega.");
}

machine build(builder: &mut Build) {
    builder.roots.bind(windows_x86_64::ProgramEntry, start);
}
```

The invocation selects one exact target—when omitted, the CLI may resolve its
`Host` convenience to a concrete profile before evaluating this build. The
`build.omg` file binds `start` for the Windows profile; it does not choose the
target, call the machine, supply arguments, or bless storage, and the language
does not discover `main` by name. At launch, the hosted target's generated
bridge performs platform storage and provider setup before calling this
source-level entry, so ordinary applications do not receive raw image or stack
extents. The target-selected Console provider services the call. Programs that
need one program-lifetime receiver attach the selected entry machine to that
receiver's data type; Chapter 3 shows that form.

The receiver's `data` declaration remains an ordinary value shape. Selecting an
attached entry provisions one occurrence. Storage authority, qualification,
and root lineage belong to that provisioned occurrence; other values of the
same type follow the ordinary construction and ownership rules.

## Data

`data` declarations describe stored state. Fields inside `data` are owned by
that value.

```omega
data Player {
    name: [u8; 64];
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
    case Say(text: [u8; 256]);
}
```

**Explicit discriminant values.** A payload-less case may
pin its tag to a specific integer — required when the sum matches a *foreign
ABI* whose tag values are fixed by a spec (firmware, hardware, a protocol):

```omega
data EfiMemoryType {           // UEFI EFI_MEMORY_TYPE — tags are the firmware's
    case ReservedMemory = 0;
    case LoaderCode     = 1;
    case LoaderData     = 2;
    case ConventionalMemory = 7;
    // ...
}
```

Rules: unspecified cases number sequentially from the previous (0-based by
default), as in C; a mix of specified and unspecified is allowed; duplicate
discriminants are a compile error. The discriminant is the on-the-wire /
in-memory tag under a layout policy, so a foreign enum reads back into the right
case. For a purely internal sum, leave them off — tag identity is the compiler's
to assign.

The interaction between explicit discriminants and the zero-initialization
rule below is not yet settled: an explicit first discriminant other than zero
conflicts with the rule that zeroed storage denotes the first case. The current
specification therefore does not determine that combination's meaning.
Bootstrap slices exclude explicit discriminants rather than choosing one side
locally.

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
declaration, so the compiler, reflected schema, and selected layout policy see
them as one thing. Mixed shapes are LIVE with these rules:

- LAYOUT: tag at offset 0 (the universal case-bearing constant), common
  fields packed after the tag, payload overlay after the common fields.
  Common-field offsets are case-independent constants; a zeroed value is the
  first case with zeroed common fields (ZII unchanged).
- CONSTRUCTION: always the case-literal form (`RoomEvent::Treasure { gold:
  5 }` -- no record-form literal for case-bearing types). Common fields may
  be named alongside payload fields (`RoomEvent::Treasure { consumed: true,
  gold: 5 }`); every common field NOT named zero-initializes -- construction
  replaces the whole value, and ZII makes the zero valid. Because of that
  rule, common fields may not declare default initializers (a default would
  silently never apply), and -- first cut -- must be scalar primitives.
- ACCESS: common fields read and write WITHOUT case knowledge
  (`event.consumed`); payload fields stay case-bound (arm bindings).
- EQUALITY: synthesized structural `==` is common fields AND tag AND the
  matching case's payload.
- Wire encoding over case-bearing value types (sums AND mixed) is rejected
  loudly until the case part has a schema spelling.

## Cases Are Domains

Declaring a case implicitly declares the same-named domain: `case Move(...)`
on `Command` declares `Command::Move`, the set of values whose tag is `Move`,
with a free constant-time membership test (a tag compare). `case` therefore never
appears at a USE site. Checks, patterns, and compositions all use the one
`Type::Name` spelling and the ordinary domain algebra
([Domains](chapter_8_domains.md)):

```omega
domain Command::Interactive
    requires self in Command::Move | Command::Say;
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
  first case with a recursively zeroed payload. The compiler derives whether
  that value establishes the type from the default domain and zero-reachable
  fields. A payload-free first case has no special semantic status; emptiness
  is an authored domain or contract (see
  [Memory Layout And ABI](chapter_20_memory_layout_abi.md)).
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

### Equality Vs Membership

`==` is always VALUE equality (resolved through core `Equatable`,
[Traits](chapter_14_traits.md)); `in` is always DOMAIN membership (the tag
test, for case domains). A bare payload-bearing case name denotes no value --
only its domain -- so comparing against it is a category error:

```omega
let q: bool = cmd == Command::Quit;                  // ok: payload-less name IS a value (tag identity)
let m: bool = cmd in Command::Move;                  // ok: membership -- "is this case"
let e: bool = cmd in Command::Quit | Command::None;  // ok: domain unions, value position
let v: bool = cmd == Command::Move { dx: 1, dy: 2 }; // ok: constructed value, STRUCTURAL equality
let x: bool = cmd == Command::Move;                  // ERROR: `Move` is not a value; use `in`
```

Equatable is intrinsic for primitives and payload-less sums (tag identity is
the only thing it could mean); records and payload-bearing sums declare one
named synthesis block, which synthesizes structural `equals` from
the members. Adding a payload case to a payload-less sum flips the type from
implicit to declared, erroring every `==` site until the one-line conformance
is written -- a deliberate re-affirmation after equality's meaning changed.
Guard-level equality never silently degrades to a tag compare; the tag test
is what `in` lowers to.

Still pending: `match`-statement arms and recursive Equatable types. Both reject
loudly at the conformance block. Bounded byte carriers participate in
synthesized equality through their live length and bytes.[^case-members]

[^case-members]: Payload binding in `transition` arms uses the ordinary
data-pattern machinery (`Case { field, fixed: value }`); a future `match`
statement must reuse that spelling rather than inventing another pattern
language. Generic payloads use ordinary cased data (`Optional<T>`-style), while
the layout rule for payload storage uses a tag-prefixed overlay with a
recursively zeroed first-case payload. A domain declared as a
pure case union is recognized for exhaustiveness
  SYNTACTICALLY -- the domain `requires` clause must contain exactly the fact
`self in Type::A | Type::B` over the target type's own cases; recognition by
general fact analysis remains a possible later widening.

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

## Constants

A `const` is a named compile-time value. Its initializer is evaluated at build
time (a build-time-admissible expression in constant position — see
[Build-Time Evaluation](../design_briefs/build_time_evaluation.md)), so a `const`
is a *value*, not runtime storage.

```omega
pub const PAGE_SIZE: u64 = 4096;
pub const EFI_SUCCESS: EfiStatus = EfiStatus { code: 0 };
pub const IMPORT_NAME: [u8; 9] = "WriteFile";
```

- **Free-floating, namespaced by package/module** (the default), resolved by the
  `::` path rule — a `const` is a compile-time name (`memory::PAGE_SIZE`). It may
  instead be **type-scoped** when it genuinely belongs to a type
  (`const EfiStatus::SUCCESS = …`), declared like a machine (`Type::NAME`),
  **outside** the `data` block — so it is never part of a value's shape and never
  counts toward `sizeof`. (Only scope a constant to a type it truly belongs to;
  binding unrelated constants to a `data` symbol is worse design.)
- **Immutable, and a pure value** — the const's type must have **no cleanup
  obligation, no shared ownership, and no interior mutability**. It is copied
  freely at each use, so it is trivially borrowable and thread-safe. A type with
  a drop/cleanup obligation cannot be a `const`; the restriction is checked from
  the cleanup facts ([Drops And Cleanup](chapter_17_drops_and_cleanup.md)), and
  it is what makes a `const` safe to reference from anywhere without analysis.
- **Not scalar-only.** Fixed arrays, records, and copy-eligible sums are
  const-evaluable when their complete types recursively
  satisfy the same pure-value/multiplicity rule. An unrestricted active case
  does not make a structurally linear sum eligible. A constant initializer may
  call any ordinary machine whose concrete invocation passes semantic-
  evaluation admission; there is no `const machine` category.
- **Not authority.** A constant grants nothing, so free-floating constants are
  consistent with the capability model — unlike ambient *mutable* state, which
  does not exist. There is no `static` keyword. A receiver-bound program entry
  gets one target-provisioned receiver, reachable only through its explicit
  `&mut self` parameter; see
  [Constants And Provisioned Entry State](../design_briefs/static_root_and_constants.md).

A constant may depend on a typed observation from the selected target-semantic
capsule. Such an application remains symbolic in a target-neutral package and
closes only when the target is selected. It is still an ordinary canonical
constant: it may appear anywhere an equivalent constant may appear, including
an array length or const-generic argument. Its exact observation and selected-
realization dependencies remain in the public signature, artifact identity,
and diagnostic provenance after folding. This adds no conditional field/case
or declaration-splice facility. See
[Build-Time Evaluation](../design_briefs/build_time_evaluation.md#target-semantic-observations).

`const` names a value, not one addressable image occurrence. Compile-time-only
uses may erase completely. When runtime use requires bytes, the compiler applies
a separate value-sensitive materialization judgment to the evaluated value and
selected layout. The active case and its actual fields must recursively have a
fully determined observable encoding; inactive cases do not participate, and
layout padding is emitted as zero and remains outside program semantics. The
diagnostic names the first offending component and the computation that
produced it.

This is deliberately distinct from immutable static/image storage. Omega has no
source `static` declaration today; a future addressable read-only image object
would promise one storage identity, which a `const` never does.

## String Literals And Bytes

A quoted literal is **raw bytes**, nothing more. The compiler's only string job
is turning quoted text into bytes; it knows nothing about encodings (that is
library code — see [Chapter 8](chapter_8_domains.md)). So a string literal has
type `&[u8]` and carries **no** encoding domain until one is explicitly
established. Its bytes live in immutable image storage: the resulting
shared view can therefore be copied into persistent machine fields without
borrowing a state-local owner.

```omega
let greeting = "Hello, Omega.";   // : &[u8]  -- just bytes, no Utf8 yet
```

In an exact-width owned fixed-array constant or evaluator-result position, a
quoted literal contextually copies its bytes into the array rather than
returning an evaluator reference:

```omega
pub const DLL_NAME: [u8; 12] = "kernel32.dll";
```

The literal byte count must equal the array length; mismatch rejects rather
than truncating or padding. Length is part of the array type and every byte is
ordinary structural value content. Constant-pool interning is an emission
optimization with no semantic identity. Ordinary temporary references and
slices may be used inside an evaluated machine, but only an owned value
snapshot may cross back out of the evaluator. Variable-length ownership uses
ordinary bounded or allocated collection types; literals do not introduce a
special compile-time byte type.

The rule is **copy, never synthesize or interpret**:

- The lexer copies the source bytes between the quotes verbatim. `"café"` typed
  directly copies whatever bytes the editor saved — the compiler does not decode
  anything.
- **Byte-level escapes** produce one specific byte and need no encoding
  knowledge: `\n \r \t \0 \\ \" \xNN`. `\\` and `\"` are required so the lexer
  can find the closing quote.
- **No `\u{...}` codepoint escapes.** Encoding a codepoint to bytes *is* an
  encoding decision, which the front end does not make; a codepoint is produced
  by a library compile-time helper (e.g. `utf8::encode(0x1F600)`) and joined with
  `+`, not smuggled into literal syntax.
- **No raw newlines inside `"..."`.** A program's meaning must not depend on how
  the file was checked out (CRLF vs LF), so a newline is written `\n`; span
  source lines by joining literals, which folds at compile time:

  ```omega
  let banner = "line one\n"
             + "line two\n";
  ```

- **Source must be ASCII-transparent** (UTF-8 in practice). Only ASCII bytes are
  syntactically significant; any non-ASCII byte appears solely as opaque payload
  inside a literal or comment. This is a fact about the *input format*, not about
  value semantics — it is the one and only place UTF-8 has a privileged
  relationship, and even that is minimizable.

To treat a literal as text, establish the encoding domain explicitly
(`"hi" as [u8]::Utf8`, which the compiler discharges by checking the bytes at
compile time — [Chapter 8](chapter_8_domains.md)). The literal itself stays raw
bytes.

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
