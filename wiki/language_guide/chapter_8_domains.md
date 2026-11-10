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

A value gains a domain only through a cast that names it. `as +Small` adds the
domain, and for a predicate-only domain the checker proves its conditions at
that cast:

```omega
domain u16::Small requires self <= 10;

let small: u16::Small = 7 as +Small;
let rejected: u16::Small = 11 as +Small; // Rejected: 11 exceeds 10.
let bare: u16::Small = 7;                // Rejected: no cast adds `Small`.
```

The annotation on `bare` does not add the domain for you. Declared types say
what a place holds; only a cast changes what a value carries.

For runtime input, a branch can establish the same fact:

```omega
data SmallResult {
    case Accepted(value: u16::Small);
    case TooLarge;
}

machine classify(value: u16) -> SmallResult {
    transition value <= 10 {
        true -> SmallResult::Accepted { value: value as +Small }
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

`as` does two separate jobs. With a carrier type it performs a compiler-derived
exact conversion: it can widen a number or narrow one whose range is proved.
With domain entries it changes which domains the value carries:

```omega
let byte: u8 = 7;
let widened: u16 = byte as u16;
let small: u16::Small = widened as +Small;
```

It never silently rounds, saturates or calls a user conversion. Use a named
machine when the operation needs runtime validation, allocation, scaling or
another policy choice.

A domain cast lists every domain involved, each with one of three marks:

| Entry | Meaning |
| --- | --- |
| `+D` | The value lacks `D`; the result carries it. |
| `-D` | The value carries `D`; the result does not. |
| `=D` | The value carries `D`; the result keeps it. |

A predicate-free domain can attach a meaning without restricting the number:

```omega
domain i32::Km;
domain i32::Positive requires self > 0;

let distance: i32::Km = 5 as +Km;
let raw: i32 = distance as -Km;

let trip: i32 in Km & Positive = 5 as +Km +Positive;
let length: i32::Km = trip as =Km -Positive;
let lost: i32::Km = trip as =Km;   // Rejected: `Positive` is unaccounted for.
```

`distance as -Km` forgets the unit. It does not multiply by 1,000; converting
kilometres to metres needs the unit library's conversion operation. Likewise
`distance as -Km +Miles`, for a similar `Miles` tag, only relabels the number;
no cast converts between units.

Nothing is dropped silently, including a predicate-only domain such as
`Positive`: every domain the value carries appears in the cast as `=` or `-`,
so the line shows both what survives and what is given up. Removing a domain
discards no fact; `length` is still known to exceed zero. Mentioning a domain
the value lacks with `=` or `-`, or adding one it already carries, rejects.

A carrier conversion takes a value with no domains and may add some to its
result. To move a qualified value to another carrier, remove its domains first:
`small as -Small as u32`. Because `+` and `-` after a cast always belong to the
cast, put parentheses around a cast before doing arithmetic on it:
`(distance as -Km) + 1`.

Owned claims need consumption or transfer, not a cast, and access restrictions
remain attached to governed storage even when its displayed type omits the
domain.

The same cast works on a reference, so a helper can take a plainer view of
your value. Through `&mut` you may remove a domain only if you could add it
back yourself, because that is what happens when the loan ends:

```omega
machine reset(n: &mut i32) ensures n == 1 { n = 1; }

let mut trip: i32 in Km & Positive = 5 as +Km +Positive;
reset(&mut trip as -Km -Positive);
```

Afterwards `trip` is `i32 in Km & Positive` again without another cast. `Km`
comes back for free. `Positive` must be proved for the new contents, here by
`reset`'s guarantee; if the helper proves nothing, repair the value yourself
before using it. A domain with an authorized establishment route cannot be
removed this way outside that route, since you could not add it back. See
[domain casts](../spec/language/domains.md#mutable-views).

## Defaults and zero

A domain can exclude zero. Such a field must be established during construction:

```omega
domain u8::Level requires self <= 100;
domain u8::Rank requires self in 1..=9;

data Config {
    level: u8::Level;
    rank: u8::Rank;
}

let ready: Config = Config { rank: 1 as +Rank, .. };  // zero-valid level defaults
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
establishment route, the child keeps it. A value carrying the child carries its
ancestors too, and a cast names each: `player as =Alive -Alive::InCutscene`
keeps the parent and removes the child.

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

The alias expands to the same facts. It creates no new evidence, and naming
it in a cast names both of its domains. Ordered domain
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

The builtin operators and comparisons work on plain values only. A qualified
value either has an operator declared for its exact type or is cast first.
For cyclic degrees:

```omega
domain u32::Degrees requires self < 360;

machine + Degrees::add(left: u32::Degrees, right: u32::Degrees) -> u32::Degrees
ensures (result as -Degrees)
     == ((left as -Degrees) + (right as -Degrees)) % 360
{
    let sum: u32 = (left as -Degrees) + (right as -Degrees);
    (sum % 360) as +Degrees
}
```

The operands bound the intermediate sum at 718, so its Exact addition is safe.
Remainder supplies the angular wrap. The contract states that relation as well
as the result's range.

Operator selection reads declared types and written casts. A `requires` or a
range fact learned from a branch proves something about a plain integer; it
does not switch that integer's `+` to degree addition.

An operator applies only when its parameter types match the operands' complete
types. A value may carry independent meanings, such as a unit and an overflow
policy, but the pair has no `+` unless one is declared for exactly that pair;
nothing combines the unit's `+` with the policy's. Competing meanings for the
same role reject. A domain can also declare an operator against a plain
operand, so common comparisons need no cast:

```omega
machine > Positive::greater(left: i32::Positive, right: i32) -> bool
ensures result == ((left as -Positive) > right)
{
    (left as -Positive) > right
}
```

See [domain operators](../spec/language/domains.md#semantic-roles-and-operators)
and [operator selection](chapter_5_expressions_evaluation.md).

## Domains on strings and encodings

Keep the container and encoding choice separate:

```omega
&[u8] in Utf8             // borrowed bytes
Vec<u8> in Utf8           // owned, growable bytes
FixedVec<u8, 64> in Utf8   // inline capacity with a live length
[u8; 64] in Utf8          // exactly 64 bytes
```

The domain changes none of those storage shapes. A quoted literal supplies
bytes; qualifying it with `as +Utf8` proves the encoding. Known bytes can be checked
at compile time, while incoming bytes need a runtime validator.

### Slice UTF-8 at character boundaries

Reading a byte is ordinary byte access, through the encoding library's index
operator or on a view cast with `as -Utf8`. A slice that retains `Utf8` must start
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
    Cell { value: value } as +Controlled
}

public machine read_cell(cell: &Cell in Controlled) -> u32 {
    cell.value
}

public machine set_cell(cell: &write Cell in Controlled, value: u32) {
    write_cell(cell, value);
}

// Ordinary helper; not separately listed as a domain writer.
machine write_cell(cell: &write Cell in Controlled, value: u32) {
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

`create_cell` is the authorized route, so its `as +Controlled` is accepted; the
same cast anywhere else rejects. `read_cell` observes the cell, and `set_cell`
lends its authorized write-only access to `write_cell`, whose parameter names
the same qualified type. The helper cannot read
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
