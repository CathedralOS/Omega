# Chapter 7: Contracts And Flow Facts

Contracts state what a machine assumes and guarantees. Domains describe static
facts or meaning attached to values. Flow analysis tracks which facts still hold
after a branch, call, or write. None of these requires runtime type tags.

A data declaration's default domain is part of its static interface. Field
constraints describe individual fields; a data-signature `where` clause describes
relationships such as `start <= end`. There is no separate authored `invariant`
clause. See [dependent data](chapter_12_dependent_types.md).

A write may temporarily leave default-domain facts unproved. That opens an
[invariant window](chapter_11_invariant_windows.md): code must restore the facts
before the next operation that consumes the value under them. Zeroed storage is
similarly not an established value when zero fails its default domain.

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

The caller supplies the range fact. The assignment transports it to `self.mass`,
which proves the postcondition. A missing proof normally produces a diagnostic;
an author does not get a runtime check merely by writing `requires`.
[State contracts](../spec/language/state_contracts.md) owns fact transport and
[default domains](../spec/language/dependent_values.md#default-domains-and-zero-initialization)
owns establishment gates.

## Fact Propagation

Branches refine the facts available on their own paths:

```omega
data Player {
    health: i32 [0..=100];
}

machine Player::take_damage(&mut self, amount: i32 [0..=100])
    ensures self.health in 0..=100
{
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

Both operands lie in `0..=100`, so subtraction fits `i32`. The false arm adds
`next >= 0`; the existing upper bound gives `next <= 100`. Each state can
therefore establish the field's range and the machine's postcondition.

Facts describe exact values or storage at particular program points. A copied
scalar keeps its captured value even if the source changes later. A reference
still denotes live storage, so an overlapping write can invalidate its facts.
An owned scalar parameter initializes separate callee storage; mutating it does
not mutate the caller's scalar. A borrowed parameter has no such separation.

A result fact describes the value returned by that invocation, not an expression
to reevaluate against newer caller arguments. Unknown mutation cannot be treated
as no mutation. See [subject identity](../spec/language/state_contracts.md#mutation-and-subject-identity)
and the [current proof implementation](../../omega-rust/psi/semantics/validation/README.md).

## Generic Contracts

A contract may refer to symbolic parameters without enumerating their values:

```omega
machine interval_contains(value: i32, lower: i32, upper: i32) -> bool
    requires lower <= upper
    ensures result == (lower <= value && value <= upper)
{
    lower <= value && value <= upper
}
```

The same statement works for every admitted argument tuple. Compile-time type
and value parameters use this principle too; [generics](chapter_13_generics.md)
explains their binders. A proof does not substitute guessed concrete values for
an abstract parameter.

## Range Forms

`a..b` excludes its end; `a..=b` includes it. For a slice of length `length`,
an exclusive end may equal `length`, whereas an inclusive end must be smaller.
An inclusive range normalizes to an exclusive end only after the required
increment and range-validity obligations are proved; normalization cannot hide
overflow.

Range membership in a contract describes values. A range used for subslicing
also creates a view and needs bounds, alignment where applicable, and a valid
loan. See [numeric bounds](../spec/language/numeric_values.md#collection-bounds).

## Window Facts

A window fact states an element-wise condition over a sequence prefix:

```omega
data MapTable
where
    loaded <= 8,
    maps[0..loaded] in MemoryMap,
{
    maps: [MemoryMap; 8];
    loaded: u32;
}
```

To extend the prefix, establish the next element and preserve the old prefix
before increasing `loaded`. To use an element fact, prove the index is inside
that prefix. A write to an earlier element must preserve or re-establish its
condition too.

This is the required reasoning, not a promise that one particular automation
rule handles every quantified predicate. Relationships between elements, such
as sortedness, need their own contracts and extraction lemmas. See
[window and sequence facts](../spec/language/dependent_values.md) and
[proofs](chapter_10_compile_time_proofs.md).

## Local And Named Facts

Branch conditions, selected transition arms, and prior call guarantees contribute
local facts. A name does not make a fact true or extend its lifetime.

Use a domain for a reusable qualification, a helper machine to establish a
guarantee, and an explicit trait conformance to bundle operations and laws.
General logical-binder syntax remains [foundation work](../spec/proofs/contracts.md#undetermined-foundations);
optional formula naming is a [proposal](../proposals/proof_formula_syntax.md),
not an extra current declaration category.

## Type Properties

Properties describe checker laws for a type, rather than predicates about one
value. They use lowercase bracket lists:

```omega
data Point [copy] {
    x: i32;
    y: i32;
}

data Box<T [copy]> [copy] {
    value: T;
}
```

`copy` permits duplication and requires compatible fields. `linear` instead
requires each established obligation to be consumed or transferred exactly once.
They cannot both apply. Neither declaration generates a callable machine.

Some judgments, such as `sized`, are compiler-derived. Declared structural
properties must validate; opaque boundary properties need accepted evidence.
Ordinary packages cannot attach structural properties to foreign types.
[Property declarations](../spec/language/ownership.md#property-declarations)
owns the exact rules; [traits](chapter_14_traits.md) names behavior instead.

### Binding relevance

`[erased]` applies to one binding, not globally to its type:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

The checker retains `proof` for static reasoning and identity but emits no runtime
field for it. It cannot determine runtime data or control. An erased Type witness
still owes its ordinary multiplicity and conservation; a logical conclusion
does not become consumable authority. A zero-size Type value is not implicitly
erased. See [explicit relevance](chapter_10_compile_time_proofs.md#explicit-relevance).

### Carry policy

Carry describes which execution transitions a live value can survive. It is
separate from ownership and copyability:

| Axis | Strict end | Relaxed end |
| --- | --- | --- |
| Suspension | forbidden | allowed |
| CPU affinity | same originating CPU | any CPU |
| Host-thread affinity | same originating thread | any thread |
| Address stability | stable storage | movable storage |

An authored policy names all four axes:

```omega
boundary data PerCpuLease [linear, carry(
    suspension: allowed,
    cpu: same,
    thread: any,
    address: movable,
)];
```

This admitted lease may survive suspension and thread changes, but the runtime
must preserve its CPU. The policy's truth needs boundary evidence; the spelling
alone does not force the host to behave that way.

Transparent data derives carry from its contents. Admitted resource claims start
strict and may receive exact positive permissions from their provider:
`Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, and
`Carry::MovableAddress`. `Carry::Portable` is the conjunction of all four.

Forgetting a permission makes the policy stricter. Forgetting a domain cannot
erase the underlying live resource's demand. Moves preserve provenance; combined
origins must satisfy every retained restriction. At a suspension or transfer,
checking joins those demands with what the selected runtime actually guarantees.
Copyability alone is not concurrent shareability.

The [carry contract](../spec/resources/carry.md) owns composition, permission
inheritance, liveness and runtime admission. The remaining property vocabulary
is not an open-ended attribute system; additional properties need their own
specified checker meaning.
