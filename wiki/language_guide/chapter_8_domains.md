# Chapter 8: Domains

A domain is a named set of values of some type. Putting a value "in" a domain
records a fact about it in its type, so the fact travels with the value and
does not have to be proved again at every use.

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

A `Player in Alive` has the same fields and the same bytes as a plain
`Player`. Domains cost nothing at run time.

## Domains in contracts

```omega
machine respawn(player: &mut Player)
requires player in Player::Dead
ensures player in Player::Alive
{
    player.health = 100;
}
```

## Putting a value in a domain

Only a cast adds a domain. For a domain with a predicate, the compiler proves
the predicate at the cast:

```omega
domain u16::Small requires self <= 10;

let small: u16::Small = 7 as +Small;
let rejected: u16::Small = 11 as +Small; // Rejected: 11 exceeds 10.
let bare: u16::Small = 7;                // Rejected: no cast adds Small.
```

For a value that arrives at run time, test it first and cast in the branch
where the test passed:

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

The test runs once, in `classify`. Everyone who later receives the
`u16::Small` uses the fact without testing again.

## The cast lists every domain

A cast accounts for each domain the value carries:

| Entry | Meaning |
| --- | --- |
| `+D` | Add `D`. |
| `-D` | Remove `D`. |
| `=D` | Keep `D`. |

```omega
domain i32::Km;
domain i32::Positive requires self > 0;

let trip: i32 in Km & Positive = 5 as +Km +Positive;
let length: i32::Km = trip as =Km -Positive;
let raw: i32 = length as -Km;
let lost: i32::Km = trip as =Km;   // Rejected: Positive is unaccounted for.
```

`Km` has no predicate: it only labels the number. Removing it does not
convert anything; kilometres to metres is a library operation, not a cast.

A cast followed by arithmetic needs parentheses: `(distance as -Km) + 1`.

## Lending a plainer view

A helper that knows nothing about your domains can still work on your value:

```omega
machine reset(n: &mut i32) ensures n == 1 { n = 1; }

let mut trip: i32 in Km & Positive = 5 as +Km +Positive;
reset(&mut trip as -Km -Positive);
// trip is i32 in Km & Positive again
```

When the loan ends the domains come back. `Positive` has to be true of the new
contents, which `reset`'s `ensures` shows.

## Operators on a domain

Built-in operators work on plain values. A domain gets its own by declaring
them:

```omega
domain u32::Degrees requires self < 360;

machine + Degrees::add(left: u32::Degrees, right: u32::Degrees) -> u32::Degrees {
    let sum: u32 = (left as -Degrees) + (right as -Degrees);
    (sum % 360) as +Degrees
}
```

Now `a + b` on two `u32::Degrees` wraps at 360. The arithmetic policies of
[Chapter 5](chapter_5_expressions_evaluation.md#integer-arithmetic),
`Wrapping`, `Saturating` and `Trapping`, are core domains of this kind.

Domains may be combined even when each defines operators. A distance may
carry both a unit and a `Small` predicate; adding a comparison to `Small`
does not make that type illegal. At an actual operator use, one declaration
must match the operands' complete qualified types. The compiler neither picks
one domain's operator by priority nor combines their implementations.

A named call may also select an overload by its declared expected result's
complete domain set. Named predicate domains count even without operators;
merely proving membership does not select anything. Adding an operator does
not change that selection. See [composition and result selection](../spec/language/domains.md#domain-composition-and-operators).

## Zero and defaults

```omega
domain u8::Level requires self <= 100;
domain u8::Rank requires self in 1..=9;

data Config {
    level: u8::Level;
    rank: u8::Rank;
}

let ready: Config = Config { rank: 1 as +Rank, .. };
```

`..` can default `level`, because zero is a valid `Level`. It cannot default
`rank`, because zero is not a valid `Rank`.

## Matching on a domain

```omega
machine status(player: &Player) -> u32 {
    transition player {
        Player::Dead -> 0
        Player::Alive -> 1
        _ -> 2
    }
}
```

The chosen arm knows the player is in that domain.

## Combining domains

```omega
domain Player::Alive::InCutscene
    requires self.in_cutscene == true;          // a child includes its parent

requires socket in Socket::Connected & Socket::Authenticated   // both
ensures game in Game::Playing | Game::Finished                 // either

public domain Socket::Usable = Socket::Connected & Socket::Authenticated;   // an alias
```

## Text is bytes in a domain

```omega
&[u8] in Utf8             // borrowed
Vec<u8> in Utf8           // owned, growable
[u8; 64] in Utf8          // exactly 64 bytes
```

There is no separate string type. Validate incoming bytes once and return
them in the domain; callers then use the text without rescanning:

```omega
data Decoded {
    case Ascii(text: &[u8]::Ascii);
    case Utf8(text: &[u8]::Utf8);
    case Invalid(error: DecodeError);
}
```

A slice that stays `Utf8` must start and end on character boundaries.

## Read and write authority

A domain can restrict who may create, read and write its values. This is how
a package keeps control of a type whose fields are otherwise public:

```omega
public data Cell {
    value: u32;
}

public domain Cell::Controlled
    access CellAccess;

CellAccess satisfies AccessPolicy {
    machine plan(schema: reflect::TypeSchema) -> AccessPlan {
        let mut plan: AccessPlan = AccessPlan::new(
            &schema,
            Issuance::Only { routes: [reflect::callable(create_cell)] }
        );
        AccessPlan::allow_readers(
            &mut plan, TypeSchema::root(&schema), [reflect::callable(read_cell)]
        );
        AccessPlan::allow_writers(
            &mut plan, TypeSchema::root(&schema), [reflect::callable(set_cell)]
        );
        plan
    }
}

public machine create_cell(value: u32) -> Cell in Controlled {
    Cell { value: value } as +Controlled
}

public machine read_cell(cell: &Cell in Controlled) -> u32 {
    cell.value
}

public machine set_cell(cell: &write Cell in Controlled, value: u32) {
    cell.value = value;
}
```

```omega
let mut cell: Cell in Controlled = create_cell(0);
set_cell(&write cell, 7);
let seen: u32 = read_cell(&cell);

let leaked: u32 = cell.value;                           // Rejected: not a listed reader.
let forged: Cell in Controlled = Cell { value: 1 } as +Controlled;   // Rejected: not the issuer.
```

Only `create_cell` may cast to `Controlled`, and only the listed machines may
read or write. This is a compile-time rule, not secrecy: the layout is still
public.

A plan always says who may create values of the domain; there is no way to
build one without that choice. When that is the only thing you want to
restrict, use the stock policy and skip writing one:

```omega
domain Folder::Writable
    access IssuedBy<reflect::callable(choose_folder)>;
```

See the
[access reference](../spec/language/domains.md#domain-owned-read-and-write-authority).

## Domains with parameters

```omega
domain<T, const U: Unit> T::Quantity<U>;
domain<Frame> Point::Coordinates<Frame>;
```

`Coordinates<World>` and `Coordinates<Screen>` are different domains on the
same `Point`, so mixing them up is a type error.

## Where next

[Chapter 9](chapter_9_proof_obligations.md) shows how the compiler uses these
facts. The complete rules, including domains declared on another package's
types, are in [domains](../spec/language/domains.md).
