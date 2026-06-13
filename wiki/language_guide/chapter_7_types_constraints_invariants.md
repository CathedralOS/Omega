# Chapter 7: Contracts And Flow Facts

Omega no longer puts invariant syntax directly on types with `Type[...]`.

Proof obligations live in contracts, domains, and local flow facts. Values are
still stored as ordinary machine types; the compiler is responsible for proving
the facts that APIs and mutations require.

```omega
data Body {
    mass: i32;
}

machine Body::set_mass(&mut self, mass: i32)
    requires mass in 1..=100
    ensures self.mass in 1..=100
{
    self.mass = mass;
}
```

Working interpretation:

- `mass: i32` stays plain type information.
- Contracts carry the proof surface.
- Rust-style ranges such as `1..10` and `1..=10` are the interval syntax in
  contracts and flow facts.
- Contract facts are compile-time proof facts, not RTTI.
- If the compiler cannot prove a constraint, the normal result is a diagnostic.
- Debug or proof builds may emit validation, but validation is instrumentation,
  not the core semantics.

## Fact Propagation

Contract facts flow through assignments, calls, branches, and transitions as
proof facts.

```omega
data Player {
    health: i32;
}

machine Player::take_damage(
    &mut self,
    amount: i32
) ensures self.health in 0..=100 {
    relax self.health {
        self.health -= amount;
        Player::restore_health_range(&mut relaxed self.health);
    }

    transition self.health <= 25 {
        true -> bloodied()
        false -> still_alive()
    }

    state bloodied(&mut self) {
    }

    state still_alive(&mut self) {
    }
}
```

The useful idea is not that `relax` means "anything goes." It means the
compiler has a proof debt. The normal fact set must be restored before control
can leave the relax scope. Chapter 11 goes deeper on relax-specific rules.

## Generic Contracts

Bounds may refer to compile-time or proof-visible values.

```omega
machine Math::clamp_i32(
    value: i32,
    min: const i32,
    max: const i32,
    out: &mut i32
) requires min <= max
  ensures out in min..=max
{
    match (value < min, value > max) {
        (true, _) -> {
            out = min;
        }
        (false, true) -> {
            out = max;
        }
        (false, false) -> {
            out = value;
        }
    }
}
```

The match partitions create facts:

- In the `(true, _)` arm, the compiler knows `value < min`.
- In the `(false, true)` arm, the compiler knows `value >= min` and
  `value > max`.
- In the `(false, false)` arm, the compiler knows `value >= min` and
  `value <= max`.

Those facts are what let the compiler discharge the postcondition
`out in min..=max`.

## Range Forms

Ranges have two spellings, and they are the same `..` / `..=` syntax used for
subslicing:

- `a..b` is exclusive of the end.
- `a..=b` is inclusive of the end.

An inclusive range normalizes to its exclusive form: `a..=b` becomes
`a..(b+1)`. The two forms therefore carry different validity obligations against
a length `len`:

- an exclusive end requires `b <= len`.
- an inclusive end requires `b < len`, so inclusive-end validity is the same as
  index validity.

A non-empty inclusive range establishes a `non_empty` fact, which downstream
contracts and slice operations can consume.

## Local And Named Facts

Many facts are local and flow-sensitive:

- branch conditions
- match arms
- transition dispatch arms
- prior contracts on calls and returns

Repeated proof conditions may still want names, but not as `Type[...]` sugar.
The likely durable homes are:

- domains for semantic states
- helper machines that establish a fact
- reusable proof or contract aliases once that surface is designed explicitly

## Type Properties

Some facts are about the TYPE itself, not any particular value: "copies are
sound", "the zero value is the canonical empty value", "values may cross a
spawn boundary". These are PROPERTIES -- declared as a lowercase fact list in
brackets on the data declaration, the same bracket syntax invariant parameters
use in type positions (`&[u8, [non_empty]]`):

```omega
data Point [copy, zero_init] {
    x: i32;
    y: i32;
}
```

Properties are facts, not behavior: declaring one generates nothing callable.
They are acquired exactly three ways:

- COMPUTED: the compiler always knows (`sized`); never written.
- DECLARED + VERIFIED: the bracket list requests the property and the compiler
  checks its structural rule at the declaration (`copy`: every field copies;
  `zero_init`: the zero case is payload-free and no field invariant excludes
  zero). Failure is a loud error at the declaration.
- BOUNDARY-ASSERTED: a boundary provider asserts a property for an opaque host
  type, audited like every other boundary guarantee.

There is no silent inference and no negative form: a type that does not
declare a property simply does not carry the fact. Properties cannot be
declared on foreign types (their rules read the fields; boundary providers
are the audited exception).

Casing carries the class split: lowercase bracket facts are properties;
capitalized names in `satisfies` positions are traits (behavior). See
[Traits](chapter_13_traits.md) for the behavior side.

Generic bounds reuse the same spelling (frozen decision 13): brackets attach
to whatever they follow, at every position --

```omega
data Box<T [copy]> [copy] {
    value: T;
}
```

The Rust-style colon bound (`<T: copy>`) and the attribute-prefix form
(`[copy]` on its own line above the declaration) are both rejected: the colon
would split the spelling system in half, and a floating prefix line is
positional metadata -- the attribute magic this surface deliberately avoids.
The spelling leaves room for trait bounds without collision
(`T [copy] satisfies Equatable`).[^property-open]

[^property-open]: Open: the initial core property set beyond
copy/zero_init/send; whether evolution-contract facts join the same surface
(`[open]` was ruled OUT for sums -- unknown-case handling is a wire decode
policy, frozen decision 10; `must_use` was ruled out by strict result use,
frozen decision 9). A `[max_size = N]` property is a candidate for this
surface: an opt-in hard bound on a type's total in-memory size, checked
against the layout report (chapter 19). The language does not impose it --
sizing a sum's cases is the author's call (a fat case can be shrunk with an
out-of-line handle if they choose) -- but the property lets an author pin a
guarantee where it matters, such as bounding an actor's continuation field so
a fat in-flight flow does not inflate every parked instance (chapter 17).

This chapter is intentionally narrow:

- Chapter 5 covers expression-level semantics such as indexing, slices, and
  numeric evaluation.
- Chapter 8 covers named semantic classifications through domains.
- Chapter 9 covers the broader compiler obligation model that uses these facts.
- Chapter 13 covers traits; properties here are their fact-side counterpart.
