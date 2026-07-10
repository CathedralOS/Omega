# Chapter 7: Contracts And Flow Facts

Omega no longer puts invariant syntax directly on types with `Type[...]`.

Proof obligations live in contracts, domains, and local flow facts. Values are
still stored as ordinary machine types; the compiler is responsible for proving
the facts that APIs and mutations require.

> **Invariants are the data's DEFAULT DOMAIN (settled 2026-07-05).** A `data`
> declaration is layout only; a data type's invariants live in its **default
> domain** — the domain that is always in scope for that data and travels with it
> everywhere (nothing to shed or track: every use of the value is already inside
> it). A per-field constraint is sugar for a single-field invariant of the default
> domain, maintained *standing* by the store-check obligations (checked at every
> write). "Top-level" domains like `Player::New` or `Quantity::Additive` are
> **subdomains** that refine the default domain (tighter invariants, operators,
> facts) and are proven at a mint point (`as`). Cross-field invariants
> (`start <= end`) live in the default domain too, declared as a `where`
> clause on the data signature (settled — bare field names; field constraints
> are single-field sugar for it; see
> [Chapter 12](chapter_12_dependent_types.md)); a store the checker cannot
> prove domain-preserving opens an [invariant
> window](chapter_11_invariant_windows.md), re-proven at the next consumption
> point (settled 2026-07-17 — supersedes the earlier store-rejection +
> `relax` model). There are **no default *values*** on data
> (see [Chapter 1](chapter_1_data_values_literals.md)); ZII is the substrate and
> construction forces the overrides where zero is invalid. **A default domain the
> zero value cannot satisfy GATES the type (settled 2026-07-17):** such data is not
> zero-constructible — its zeroed form exists only as storage, inaccessible as the
> type until construction or an `as` mint proves the domain, monotonically
> thereafter (every later store re-proves it). The gate propagates through
> containment and is absorbed by a zero-valid first sum case (`case Empty;`); see
> [Chapter 12](chapter_12_dependent_types.md). *Settled model; not yet
> implemented.*

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
    health: i32 [0..=100];
}

machine Player::take_damage(
    &mut self,
    amount: i32 [0..=100]
) ensures self.health in 0..=100 {
    let next: i32 = self.health - amount;

    transition next < 0 {
        true -> floored()
        false -> settle(next)
    }

    state floored(&mut self) {
        self.health = 0;
    }

    state settle(&mut self, next: i32) {
        self.health = next;
    }
}
```

The temp carries the arithmetic, the arm facts (`next < 0` / `next >= 0`)
discharge each store, and both paths discharge the postcondition. Writes that
transiently break a fact in place are also legal: the compiler carries the
proof debt as an invariant window, re-proven at the next consumption point —
Chapter 11 owns those rules.

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

## Window Facts

A range may also quantify (settled 2026-07-18): a fact stated over a window
of a sequence holds for every element of the window, with no binder and no
new syntax — the subslice spelling is the quantifier:

```omega
data MapTable
where
    loaded <= 8,
    maps[0..loaded] in MemoryMap,    // every element below the count is established
{
    maps: [MemoryMap; 8];
    loaded: u32;
}
```

Working rules:

- A window fact is an element fact over `expr[range]`: membership in a
  domain, a range constraint, any single-element fact.
- Extending the window by one element (append: write at the frontier, then
  widen the count) costs one instance — the fact for the new element, which
  the write just established. This is the same delta rule quantified facts
  use (chapter 10).
- Consuming at an index requires the index provably inside the window
  (`i < loaded` by guard or contract), and yields the element fact at `i`.
- Relational facts between elements (order between neighbors) are not window
  facts; they are predicate machines with extraction lemmas (chapter 10).

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

- COMPUTED: the compiler always knows (`sized`); never written. The
  `unbounded` property (chapter 10) is the proof-only marker: no machine
  layout, no ZII, fact-position use only.
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
[Traits](chapter_14_traits.md) for the behavior side.

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
against the layout report (chapter 20). The language does not impose it --
sizing a sum's cases is the author's call (a fat case can be shrunk with an
out-of-line handle if they choose) -- but the property lets an author pin a
guarantee where it matters, such as bounding an actor's continuation field so
a fat in-flight flow does not inflate every parked instance (chapter 18).

This chapter is intentionally narrow:

- Chapter 5 covers expression-level semantics such as indexing, slices, and
  numeric evaluation.
- Chapter 8 covers named semantic classifications through domains.
- Chapter 9 covers the broader compiler obligation model that uses these facts.
- Chapter 14 covers traits; properties here are their fact-side counterpart.
