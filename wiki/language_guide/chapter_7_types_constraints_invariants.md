# Chapter 7: Contracts And Flow Facts

Omega has no authored `invariant` declaration or clause. The retired word does
not name a separate proof surface.

Proof obligations live in contracts, domains, and local flow facts. Values are
still stored as ordinary machine types; the compiler is responsible for proving
the facts that APIs and mutations require.

Invariants are the data's default domain. A `data` declaration supplies layout;
its default domain is always part of the value's static interface. Per-field
constraints are single-field predicates in that domain. Cross-field invariants
such as `start <= end` use a `where` clause on the data signature (see
[Chapter 12](chapter_12_dependent_types.md)).

Domains such as `Player::New` or `Quantity::Additive` refine or semantically
qualify that default theory according to chapter 8. A write that does not
immediately prove the default predicates opens an
[invariant window](chapter_11_invariant_windows.md); the predicates must be
re-established before the next consumption point.

Data has no default *values* beyond zero-initialized storage. When the default
domain excludes zero, the storage is gated: it cannot be observed as the type
until construction or qualification establishes the domain. Gating propagates
through containment and is absorbed by a zero-valid first sum case such as
`case Empty;`. The full model is settled but not yet implemented.

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

Scalar result facts describe the value returned at the call, not a deferred
read of the caller's arguments or the callee's locals. The current checker can
capture selected fixed-integer and Boolean computations through single-state
helpers with local mutable storage and owned mutable scalar parameters. Each
assignment reads the previous value before updating it, and immutable copies
retain their earlier value. Unsupported
calls or nonlocal writes do not produce such a snapshot. Byte-store proofs may
consume a captured byte only while the destination carrier's required per-byte
class remains proved; an ASCII replacement alone cannot preserve an arbitrary
UTF-8 sequence.

An owned scalar argument initializes separate callee storage. Reassigning that
parameter, including through a local borrow, does not invalidate facts about the
caller's original scalar. Real borrowed arguments still expose their caller
storage to mutation. Mutable formal slots cannot be read as immutable incoming
aliases while evaluating a body result; entry contracts retain their separate
invocation-entry meaning.

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

A range may also quantify: a fact stated over a window
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

Some static laws are about the TYPE itself, not any particular value: "copies
are sound", "values impose this carry floor while live", and "established
values must be consumed exactly once". These are
PROPERTIES -- declared as a lowercase list in brackets on the data declaration
or a generic type parameter. These property lists are distinct from value-range
constraints. Named proof constraints such as `T[finite]` or
`&[u8, [non_empty]]` are retired: use a value domain such as `f32 in Finite`, a
declared byte-sequence domain, or a contract on the sequence's length instead.

```omega
data Point [copy] {
    x: i32;
    y: i32;
}

data Task<T> [linear] {
    // representation omitted
}

boundary data PerCpuLease [
    linear,
    carry(
        suspension: allowed,
        cpu: same,
        thread: any,
        address: movable,
    ),
];
```

Properties are static checker laws, not behavior: declaring one generates
nothing callable. Most contribute type facts; `[linear]` instead selects the
linear permission algebra for established values. It must not be stored as a
weakenable flow fact.
They are acquired exactly three ways:

- COMPUTED: the compiler always knows (`sized`); never written. Transparent
  carry policy and whether zero establishes a checked-shape type are also
  derived structurally rather than annotated. The
  `unbounded` property (chapter 10) remains the transitional proof-only
  classifier: no machine layout, no ZII, fact-position use only. Explicit
  relevance replaces that classification as described below.
- DECLARED + VERIFIED: the bracket list requests the property and the compiler
  checks its structural rule at the declaration (`copy`: every field copies;
  `linear`: mutually exclusive with `copy`, and every contained linear
  obligation is structurally preserved). Failure is a loud error at the
  declaration.
- BOUNDARY-ASSERTED: a boundary provider claims a property for an opaque host
  type. The spelling is inert until validation/admission accepts it and records
  a receipt; packages can never self-grant it. Opaque authored carry floors use
  this path as well.

Except for the compiler-owned derived judgments named above, there is no silent
inference and no negative form: a type that does not declare a property simply
does not carry the fact. Properties cannot be
declared on foreign types (their rules read the fields; boundary providers
are the audited exception).

Casing carries the class split: lowercase bracket facts are properties;
capitalized names in `satisfies` positions are traits (behavior). See
[Traits](chapter_14_traits.md) for the behavior side.

Generic bounds reuse the same spelling: brackets attach
to whatever they follow, at every position --

```omega
data Box<T [copy]> [copy] {
    value: T;
}
```

### Binding relevance

`[erased]` reuses bracket placement but applies to one binding occurrence, not
to the bound type globally:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

`Valid<T>` may appear relevant elsewhere. Here only `proof` is erased. The
checker retains it for proof, validity, and provenance analysis while runtime
layout omits it. Proposition terms are copyable. An explicitly erased Type
ghost instead retains its Type multiplicity and conservation obligations.
Erased bindings may not determine runtime data or control and cannot rely on
runtime cleanup; any static Type obligations remain live until discharged. A
structurally zero-layout Type value needs no `[erased]` marker merely to occupy
no bytes, and a representable runtime value cannot use `[erased]` to delete its
storage. See
[Compile-Time Proofs](chapter_10_compile_time_proofs.md#explicit-relevance).

### Carry policy

Carry is one compiler-built-in parameterized property, not four traits or an
open attribute system. It normalizes directly into a compiler semantic record
with four independent axes:

| Axis | Strict/default end | Relaxed end |
|---|---|---|
| suspension | `forbidden` | `allowed` |
| CPU affinity | `same` (the mint/provenance CPU) | `any` |
| host-thread affinity | `same` (the mint/provenance thread) | `any` |
| address stability | `stable` | `movable` |

All four axes are mandatory when the property is authored; order is not
semantic:

```omega
data WorkItem [carry(
    suspension: allowed,
    cpu: any,
    thread: any,
    address: movable,
)] {
    id: u64;
}
```

The axis vocabulary is closed because every member changes compiler liveness,
relocation, or runtime-admission behavior. Axis evolution is a
language/compiler release with composition and validation rules. `CarryPolicy`
is structured normalized compiler IR rather than ordinary `omega::core` data
or a policy-machine result.

Transparent scalars and data derive the most permissive policy their structure
proves. Aggregates share the field traversal used by other structural
properties but combine each carry axis under its own algebra, selecting the
most restrictive live-field demand. A declared `[carry(...)]` policy supplies a
universal floor, validated against that structural result.

Resource claims add a per-value layer. A claim originated by an admitted
provider begins with the strict policy because checked code cannot inspect its
external backing. The provider's result contract may establish four
compiler-owned positive permission facts:

| Permission fact | Granted transition |
|---|---|
| `Carry::AcrossSuspend` | suspension may occur while the value is live |
| `Carry::AnyCpu` | execution may resume on another CPU |
| `Carry::AnyThread` | execution may resume on another host thread |
| `Carry::MovableAddress` | the value's required storage may move |

`Carry` is the compiler-owned subject-polymorphic namespace for these facts:
the same permission may qualify any carried value while retaining that value's
own provenance anchor.

`Carry::Portable` is the standard transparent predicate alias for the
conjunction of all four permissions:

```omega
pub domain Carry::Portable =
    Carry::AcrossSuspend
    & Carry::AnyCpu
    & Carry::AnyThread
    & Carry::MovableAddress;
```

An admitted portable range can therefore publish:

```omega
boundary machine BootMemory::take(entry: FirmwareRange)
    -> Extent::Granted
               & Extent::Physical
               & Carry::Portable;
```

A partially relaxed result names only the transitions it permits:

```omega
pub boundary trait InterruptMaskControl {
    machine save_and_mask(&mut self)
        -> InterruptMaskGuard in Active & Carry::MovableAddress
    ensures
        result in InterruptMaskGuard::Active;
}
```

The missing permissions leave that admitted claim no-suspend, same-CPU, and
same-thread. Permission facts are droppable: forgetting one selects a stricter
policy. The undischarged resource provenance remains attached independently,
so forgetting `Extent::Granted` does not erase its carry demand. A freshly
constructed unqualified `Extent` has no such resource provenance and follows
its structural policy.

Checked-internal claims derive carry from the claims and storage they actually
inherit. Claim transfer preserves permissions; a conserved split gives every
child the parent's permissions; combined origins select the most restrictive
demand per axis. A transformation may establish a more permissive successor
only by discharging the old claim and establishing a new claim with checked or
admitted evidence.

The provenance attached at claim origin supplies relational anchors such as
the meaning of `cpu: same`; no runtime tag is added to ordinary values.
Generic property bounds use the same policy ordering and are checked
parametrically; carry checking is not inherently blocked on backend
monomorphization.

Canonical place liveness, the normalized type/per-claim policy, and the
selected runtime contract decide whether a
suspension, migration, transfer, or relocation is legal. Cross-activation
ownership transfer is checked from ownership plus carry/runtime compatibility;
shared references additionally require a sanctioned shared-access contract.

The Rust-style colon bound (`<T: copy>`) and the attribute-prefix form
(`[copy]` on its own line above the declaration) are both rejected: the colon
would split the spelling system in half, and a floating prefix line is
positional metadata -- the attribute magic this surface deliberately avoids.
The spelling leaves room for trait bounds without collision
(`T [copy] satisfies Equatable`).[^property-open]

[^property-open]: Open: the initial core property set beyond
copy/linear/carry and whether evolution-contract facts join the same surface.
Unknown-case handling remains a wire decode policy, not an `[open]` sum
property, and strict result use needs no `must_use` property. A
`[max_size = N]` property is a candidate for this surface: an opt-in hard bound
on a type's total in-memory size, checked
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
