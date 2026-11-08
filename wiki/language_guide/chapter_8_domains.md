# Chapter 8: Domains

A domain names a useful property or meaning of a value. For a player, you might
name valid health, being alive, and being ready to respawn:

```omega
data Player {
    health: i32;
    in_cutscene: bool;
}

domain Player::Valid
    requires self.health >= 0 && self.health <= 100;

domain Player::Alive
    requires self in Player::Valid && self.health > 0;

domain Player::Dead
    requires self in Player::Valid
          && self.health == 0 && self.in_cutscene == false;
```

`self` denotes the value being classified. A qualified `Player` has the same
fields and storage as a plain `Player`; the domain adds facts the checker can
use. Predicates do not run merely because a type names a domain.

## Domains in contracts

A machine can require one domain and establish another:

```omega
machine respawn(player: &mut Player)
requires player in Player::Dead
ensures player in Player::Alive
{
    player.health = 100;
}
```

The caller proves the player is dead. The write establishes valid positive
health, which proves `Alive` on completion. Domains make those repeated
conditions easier to name and compose.

A data declaration's own field constraints and `requires` form its **default
domain**. That domain holds whenever a valid value is handed to an ordinary
consumer. Named domains let an operation express stronger entry or completion
guarantees. [Invariant windows](chapter_11_invariant_windows.md) shows how those
choices affect a multi-step update.

## Establishing and qualifying domains

For a predicate-only domain, prove its conditions before qualifying the value:

```omega
domain u16::Small requires self <= 10;

let small: u16::Small = 7;
let rejected: u16::Small = 11; // Rejected: 11 exceeds 10.
```

For runtime input, a branch can establish the same fact:

```omega
data SmallResult {
    case Accepted(value: u16::Small);
    case TooLarge;
}

machine classify(value: u16) -> SmallResult {
    transition value <= 10 {
        true -> SmallResult::Accepted { value: value as u16::Small }
        false -> SmallResult::TooLarge
    }
}
```

The check runs in `classify`. The cast uses the resulting proof; it inserts no
second check. A domain with an establishment policy additionally needs an
authorized route, described under [access policies](#read-and-write-authority).

Domains apply to their declared carrier, meaning the underlying data type.
`7u8` must first convert to `u16` before it can belong to `u16::Small`.
Knowing the numbers are equal does not change their carrier.

## Qualification and conversion

`as` performs a compiler-derived exact coercion or explicit erasure. It can
widen a number, narrow one whose range is proved, or add a proved domain:

```omega
let byte: u8 = 7;
let widened: u16 = byte as u16;
let small: u16::Small = widened as u16::Small;
```

It never silently rounds, saturates or calls a user conversion. Use a named
machine when the operation needs runtime validation, allocation, scaling or
another policy choice.

A predicate-free domain can attach a meaning without restricting the number:

```omega
domain i32::Km;

let distance: i32::Km = 5;
let raw: i32 = distance as i32;
```

The second line explicitly forgets the unit. It does not multiply by 1,000;
converting kilometres to metres needs the unit library's conversion operation.
A direct cast between unrelated meanings such as `Km` and `Degrees` rejects.

Predicate-only facts with no access policy can weaken implicitly. Semantic
meaning and non-owning provenance need explicit erasure. Owned claims need
consumption or transfer, and access restrictions remain attached to governed
storage even when its displayed type omits the domain.

## Defaults and zero

A domain can exclude zero. Such a field must be established during construction:

```omega
domain u8::Level requires self <= 100;
domain u8::Rank requires self in 1..=9;

data Config {
    level: u8::Level;
    rank: u8::Rank;
}

let ready: Config = Config { rank: 1, .. };  // explicitly default zero-valid level
```

`..` opts into zero defaults for omitted runtime fields. `rank` still needs a
value because zero fails its domain. Without `..`, list every runtime field.
See [construction](chapter_1_data_values_literals.md#explicit-zero-defaults).

## Domain patterns

A runtime-checkable domain can be used in dispatch. With the `Player` above:

```omega
machine status(player: &Player) -> u32 {
    transition player {
        Player::Dead -> 0
        Player::Alive -> 1
        _ -> 2
    }
}
```

The selected arm receives the domain's facts. The fallback covers players
outside both domains, including a zero-health player in a cutscene. `match` computes a value; `transition` selects the next control-flow edge.
Both use the same patterns, as explained in [pattern matching](chapter_6_pattern_matching_dispatch.md).

Only pure, finite, executable predicates can supply a runtime test. A proof-only
predicate needs evidence instead, and testing fields cannot manufacture an
authorized issuance history.

## Refine and combine domains

A nested domain name includes its parent's conditions:

```omega
domain Player::Alive::InCutscene
    requires self.in_cutscene == true;
```

It requires both `Alive` and the additional fact. If the parent has a restricted
establishment route, the child keeps it.

Use `&` for facts that must hold together, and `|` for a known union:

```omega
requires socket in Socket::Connected & Socket::Authenticated
ensures game in Game::Playing | Game::Finished
```

These contract fragments assume the named domains are declared. A transparent
alias can name an intersection:

```omega
public domain Socket::Usable = Socket::Connected & Socket::Authenticated;
```

The alias expands to the same facts. It creates no new evidence. Ordered domain
matches permit overlap: the first matching arm wins. To reuse one condition
across unrelated domains, name a pure total Boolean machine and use
`predicate(self) == true` in each domain.

## Domains with no members

This declaration is legal:

```omega
domain i32::Impossible requires self > 0 && self < 0;
```

It describes an empty set. A caller cannot provide a value in it without proving
those conditions. Declaring the domain does not assert that such a value exists.
A domain with no predicate is different: it adds no predicate obligation,
though an access policy may still restrict its establishment and use.

## Select operators through domains

Declared qualifications can select an operator meaning. For cyclic degrees:

```omega
domain u32::Degrees requires self < 360;

machine + Degrees::add(left: u32::Degrees, right: u32::Degrees) -> u32::Degrees
ensures (result as u32) == ((left as u32) + (right as u32)) % 360
{
    let sum: u32 = (left as u32) + (right as u32);
    (sum % 360) as u32::Degrees
}
```

The operands bound the intermediate sum at 718, so its Exact addition is safe.
Remainder supplies the angular wrap. The contract states that relation as well
as the result's range.

Operator selection follows binding-site qualifications: declared types,
explicit casts and entry requirements. Learning a range fact from a branch
does not silently switch a plain integer's `+` to degree addition.

Independent meanings can compose, such as a unit and an overflow policy.
Competing meanings for the same role reject. The selected operator must satisfy
the whole combination; adding a domain never runs unrelated overloads in an
arbitrary order. See [domain operators](../spec/language/domains.md) and
[operator selection](chapter_5_expressions_evaluation.md).

## Domains on strings and encodings

Keep the container and encoding choice separate:

```omega
&[u8] in Utf8             // borrowed bytes
Vec<u8> in Utf8           // owned, growable bytes
FixedVec<u8, 64> in Utf8   // inline capacity with a live length
[u8; 64] in Utf8          // exactly 64 bytes
```

The domain changes none of those storage shapes. A quoted literal supplies
bytes; qualifying it as UTF-8 proves the encoding. Known bytes can be checked
at compile time, while incoming bytes need a runtime validator.

### Slice UTF-8 at character boundaries

Reading a byte is ordinary byte access. A slice that retains `Utf8` must start
and end at character boundaries. For example, UTF-8 encodes `é` in two bytes:
taking both preserves the character, while taking only its first byte does not
produce valid UTF-8.

An encoding library's slice operation therefore requires both bounds and
character-boundary facts. A byte-oriented operation may instead return an
unqualified view. Mutation must preserve or re-establish encoding validity.

### Validate once, then preserve the fact

A classifier can return qualified bytes in its successful cases:

```omega
data Decoded {
    case Ascii(text: &[u8]::Ascii);
    case Utf8(text: &[u8]::Utf8);
    case Invalid(error: DecodeError);
}
```

The validator supplies the proof. Callers matching a successful case use it
without rescanning. Concatenation and slicing can preserve the domain through
their checked contracts. Returned views remain borrowed from the input.

Wire decoding establishes only the facts its codec promises. Ask the schema
for encoding validation when needed; receiving bytes from a wire does not
establish `Utf8`. See [encoding domains](../spec/language/domains.md#byte-containers-and-encoding-domains).

## Read and write authority

Access policies are useful when a domain represents a controlled protocol.
This longer example separates who may establish a controlled cell, read it,
and write it:

```omega
public data Cell {
    value: u32;
}

public domain Cell::Controlled
    access CellAccess;

CellAccess satisfies AccessPolicy {
    machine plan(schema: reflect::TypeSchema) -> AccessPlan {
        let mut plan: AccessPlan = AccessPlan::new(&schema);
        AccessPlan::establish(&mut plan, [reflect::callable(create_cell)]);
        AccessPlan::allow_readers(
            &mut plan, TypeSchema::root(&schema), [reflect::callable(read_cell)]
        );
        AccessPlan::allow_writers(
            &mut plan, TypeSchema::root(&schema), [reflect::callable(set_cell)]
        );
        plan
    }
}

public data Counter {
    cell: Cell in Controlled;
}

public machine create_cell(value: u32) -> Cell in Controlled {
    Cell { value: value }
}

public machine read_cell(cell: &Cell in Controlled) -> u32 {
    cell.value
}

public machine set_cell(cell: &write Cell in Controlled, value: u32) {
    write_cell(cell, value);
}

// Ordinary helper; not separately listed as a domain writer.
machine write_cell(cell: &write Cell, value: u32) {
    cell.value = value;
}
```

```omega
let counter: Counter = Counter { cell: create_cell(0) };
set_cell(&write counter.cell, 7);
let observed: u32 = read_cell(&counter.cell);
```

```omega
let leaked: u32 = counter.cell.value; // Missing read authorization.
counter.cell.value = 9;              // Missing write authorization.
write_cell(&write counter.cell, 9);  // No authorized delegation to the helper.
```

`create` establishes the qualification. `read` observes the cell, and `set`
lends its authorized write-only access to `write_cell`. The helper cannot read
the old contents. Merely sharing a file or namespace grants no access.

An omitted list adds no restriction; an empty list permits nobody. Multiple
domain policies and whole-object/field restrictions intersect. Whole-object
replacement checks every affected field, so it cannot bypass a denied write.

Lists may name exact machines or trait requirements. A requirement route
permits its valid selected implementations; a private requirement can keep
that set closed. A public domain can retain private issuer identities without
making those machines callable to consumers.

Casting or aliasing a restricted view preserves its access policy. A temporary
qualified borrow retains restrictions through all its descendants; when that
complete loan ends, its lender recovers its prior access. Existing owner
restrictions remain.

The policy adds no permission bits and promises no secrecy. It governs checked
establishment and access to a publicly described value. See the
[access reference](../spec/language/domains.md#domain-owned-read-and-write-authority)
for field selectors, delegation, provider receipts and external-root issuance.

## Indexed domain families

A domain can take type or static value arguments:

```omega
domain<T, const U: Unit> T::Quantity<U>;

domain<Frame> Point::Coordinates<Frame>;
```

These declaration sketches assume `Unit` and `Point` are defined. A quantity
can select `Quantity<Units::METER>`; coordinates can select `Coordinates<World>`.
The indices distinguish their meanings while the underlying carrier stays
unchanged. `Coordinates<World>` and `Coordinates<Screen>` have no implicit
relationship.

Closed constant indices use canonical value identity. Equality evidence may
establish further compatibility between open expressions. [Generics](chapter_13_generics.md)
explains that distinction and its proof-work budget.

## Domains on foreign types

A package can classify another package's public type with a domain. Import
the domain explicitly, for example `use ui_policy::screen::Point::OnScreen;`.
It keeps the carrier attachment resolved by its declaring package.

Two visible declarations competing for the same qualified name are ambiguous;
neither import order nor carrier ownership chooses a winner. Narrow imports
avoid unrelated additions. Importing the domain enables naming it, while
membership and authority still need their usual evidence.

Changing a public type's invariant or another domain's issuer rules is outside
that independent classification. See [foreign domains](../spec/language/domains.md#visibility-and-foreign-declarations).

For result-domain overload selection and the full identity rules, use the
[domain reference](../spec/language/domains.md). A domain itself adds no runtime
tag; put an explicit phase or case in the data when the program needs one.
