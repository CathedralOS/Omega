# Chapter 13: Generics

Generics let one data or machine declaration work over many concrete types or
compile-time values.

The baseline model should stay close to Rust:

- Type parameters are written with angle brackets.
- Const/proof parameters can appear where compile-time values matter.
- Constraints live in `where` clauses.
- Generic code is statically checked once, then instantiated for concrete uses.
- Static dispatch and monomorphization are the default.

## Generic Data

Generic data declarations parameterize stored shape.

```omega
data Optional<T> {
    case #0 None;
    case #1 Some(value: T);
}

data Pair<A, B> {
    first: A;
    second: B;
}
```

Working rules:

- `T`, `A`, and `B` are type parameters.
- Each concrete instantiation has a concrete layout after type checking.
- Generic fields follow the same ownership, move, borrow, and cleanup rules as
  non-generic fields.
- If `T` has cleanup, then `Optional<T>` or `Pair<A, B>` may have structural
  cleanup obligations.

`Optional<T>` is ordinary cased data rather than a distinct language feature.
Packages may declare other generic sums with domain-specific cases. Its home
representation publishes an ordinary machine requirement proving
`zero_value<Optional<T>>() == Optional::None`; the checker discharges that
authored obligation from the normalized home layout.

## Generic Machines

Machines may be generic over types.

```omega
machine Inventory::find<T, Equality: T satisfies Equatable>(
    items: &[T],
    target: &T,
    out: &mut Optional<u64>
)
{
    transition items.len > 0 {
        true -> find_at(items, target, 0, out)
        false -> not_found(out)
    }

    state find_at(
        items: &[T],
        target: &T,
        index: u64,
        out: &mut Optional<u64>
    ) {
        let found: bool = Equality::equals(&items[index], target);
        let next_index: u64 = index + 1;
        let has_next: bool = next_index < items.len;

        transition (found, has_next) {
            (true, _) -> found_at(index, out)
            (false, true) -> find_at(items, target, next_index, out)
            (false, false) -> not_found(out)
        }
    }

    state found_at(index: u64, out: &mut Optional<u64>) {
        out = Some(index);
    }

    state not_found(out: &mut Optional<u64>) {
        out = None;
    }
}
```

The syntax is provisional, but the intended shape is not exotic: generic
machines use type parameters and constraints like Rust does.

## Const And Proof Parameters

Some generic facts are values known at compile time or proof time.

```omega
data FixedBuffer<T, const N: u64> {
    items: [T; N];
}

machine Math::clamp_i32<const MIN: i32, const MAX: i32>(
    value: i32,
    out: &mut i32
) {
    match (value < MIN, value > MAX) {
        (true, _) -> {
            out = MIN;
        }
        (false, true) -> {
            out = MAX;
        }
        (false, false) -> {
            out = value;
        }
    }
}
```

Working rules:

- `const` parameters are compile-time values, proof-visible values, or both.
- Const parameters may appear in array lengths, value constraints, and proof
  obligations.
- The compiler must prove const constraints at each instantiation.
- A canonical target-semantic observation may supply a const argument under the
  same rules as any other constant. Its application remains symbolic before
  target closure and enters the generic application's compatibility identity;
  target dependence is not a separate reason to reject it.

This does not introduce a target-native count or index type. Length APIs retain
their explicit target-independent count carrier, while indexing accepts any
eligible integer that proves `0 <= index < len`. A hypothetical
`UInt<const Bits>` is a separate carrier-family decision: it must define which
widths exist and whether applications coincide with the named primitives before
`UInt<7>` or `UInt<address_bits>` means anything.

### Structured values and indexed domains

Scalar const parameters generalize in three ordered stages. First, structured
proof/static values become eligible when equality is decidable and every value
has one canonical form. Index position erases the value; this does not imply
its value kind lacks an ordinary runtime representation. Current `Rat` is
eligible only after the index site verifies its positive denominator, cancelled
signed coordinates, and gcd-reduced numerator magnitude and denominator.

This first stage is implemented. A named literal `const` over eligible
integers, booleans, fixed arrays, records, or cases may instantiate a `const`
parameter. The compiler recursively checks the declared value kind, orders
fields by their declaration, and records the canonical value rather than the
source initializer. Thus two record literals that differ only in field order
have one generic identity. Floating/text values, references, slices, dynamic
identities, and boundary-opaque data are ineligible. A noncanonical `Rat`
rejects at the generic argument site. Quotient data and records with
default-domain facts also reject until their canonical-representative or
index-site proof path is implemented.

Second, an erased domain family may take a closed static index. The generic
carrier is bound and then used in the ordinary position:

```omega
domain<T, const U: Unit> T::Quantity<U>;
```

The domain itself imposes no carrier constraint. An operator states only the
operation it needs through an ordinary one-off machine bound. This one
declaration therefore supports both `f64::Quantity<KM>` and
`i64::Quantity<KM>`. Named closed combinations such as `KM_PER_SECOND` and a
generic conversion returning its destination index require no symbolic
normalizer.

Third, a generic result-domain constraint may contain an expression over input
indices. A unit divide operation can then produce `Quantity<A / B>` while
requesting only the carrier operation it uses through the existing one-off
machine-bound clause:

```omega
where
    machine T::divide(left: T, right: T) -> T
```

The domain family remains nominal. Closed index values enter semantic identity
in canonical form. Open expressions use only compiler-supported normal forms
licensed by the exact selected, proved algebraic conformance. Compatibility
creates a named verification condition. Closed evaluation, licensed canonical
normalization, or an established local fact may discharge it; otherwise it
rejects. A proof-machine call contributes its checked `ensures` as an ordinary
local fact, so indexed domains add no citation syntax. No ambient theorem search
occurs, and generic code must publish any equality it cannot discharge.
Diagnostics preserve the source-written index expression when available and
name whether compatibility came from closed evaluation, normalization, or an
exact established local fact. Those display and evidence records do not enter
semantic identity.

Closed indexed families and their direct-binder qualification path are
implemented. A concrete constrained argument or destination supplies a
const-generic machine's closed binder during specialization:

```omega
let meters: i64 in Quantity<Units::METER> = retag_i64(70);
```

The binder is erased, distinct canonical destinations receive distinct
machine instances, and an incomplete tuple rejects before code generation. The
shipped `omega::language::std::units` package exercises named closed
combinations, destination-typed conversions, and per-pair operators across
imports in both engines. Computed open result expressions, licensed canonical
normalization, named compatibility conditions, and exact active local-fact
discharge now complete the third stage. Successful judgments retain evidence
without changing identity; unresolved equality rejects.

## Machine Parameters

A generic parameter may name a machine symbol:

```omega
machine Deck::best<machine Key>(&self) -> u64
where machine Key(card: &Card) -> u64
{
    let score: u64 = Key(&self.cards[0]);
    score
}
// spelled at the call site: deck.best<Card::power_key>()
```

`Key` is not a hidden runtime argument. Monomorphization substitutes the
selected symbol at every use; the example above emits a direct
`Card::power_key(&self.cards[0])` call in that specialization.

Rules:

- `<machine M>` binds a machine **symbol** at the spelling site. Its
  `where machine` clause supplies either a structural callable signature or
  one exact nominal requirement, and the selected symbol is checked against
  that contract. Ordinary specialization substitutes it like every generic;
  after substitution, each use of `M` is a direct static call. No runtime
  value exists — the parameter is gone by codegen.
- A static-machine selection in `requires` or `ensures` instantiates a logical
  contract schema, not an executable call site. It is checked against the same
  callable requirement but does not by itself monomorphize the selected generic
  machine; universal quotient relations and their law witnesses therefore stay
  universal.
- Recursive proof-only data may use the same form to index a family:
  `data CauchySeq<machine S> where machine S(index: Nat) -> Rat; { ... }`.
  A concrete `CauchySeq<leibniz_term>` argument is checked against `S`'s full
  callable contract. This is schema identity only: finite-layout data rejects
  machine parameters, and `S` cannot be stored as a field type.
- A machine-parameter signature may itself declare machine parameters. Its
  nested requirements follow it in the same clause stream:

  ```omega
  machine forward<machine Schema, machine Selected>(value: Stream<Selected>) -> Stream<Selected>
  where machine Schema<machine Inner>(value: Stream<Inner>) -> Stream<Inner>
  where machine Inner(index: Nat) -> Rat;
  where machine Selected(index: Nat) -> Rat;
  {
      Schema<Selected>(value)
  }
  ```

  Refinement is binder-positional: a selected generic schema may call its
  nested parameter something other than `Inner`, but its complete nested
  parameter/result shape, service reach, suspension/blocking ceilings, guarded
  crash buckets,
  termination guarantee, and contracts must conservatively refine the authored
  requirement. Forwarding a distinct
  machine parameter uses that same judgment. Specialization first replaces
  `Schema` and `Selected`, then continues to a fixed point until the nested
  call is direct and contains no runtime callable representation.
- Every machine parameter must have an authored contract at its declaration.
  The structural form is `where machine M(...)`; the nominal form is
  `where machine M satisfies Trait::requirement`. The nominal requirement
  supplies its complete parameter/result shape, contracts, operational
  ceilings, and any boundary calling/entry plan, so its signature is not
  repeated. The compiler never infers either abstraction from `M(...)` uses,
  matching signatures, visible conformances, or the machines currently
  supplied by consumers, even in a whole-program build with only one
  instantiation. Missing or ambiguous contracts reject. If exactly one
  implementation is intended, call that concrete machine instead of declaring
  a generic.
- Type and result parameters may be inferred from the selected machine and
  ordinary arguments. For example, `map<Card::power>(cards)` specializes
  `map<T, U, machine F>` with `T = Card`, `U = u64`, and every `F(value)` call
  becomes `Card::power(value)`.
- Generic bodies are checked modularly against the authored `where machine`
  contract: they prove the parameter machine's `requires`, assume its
  `ensures`, and include its published reach and other contract axes. At an
  instantiation, the selected machine must refine that requirement. The
  checker does not infer a stronger generic API from whichever implementation
  happens to be selected.
- The receiver mode in the required signature is the calling discipline:
  `&self` is freely repeatable, `&mut self` is a stateful callback (spell it
  as a type parameter whose machine is required, as below); a consuming
  mode arrives with the cleanup arc.
- There are **no runtime machine values and no capture inference**. A
  stateful callback is a machine *instance* — its fields are its declared
  captures, construction is the capture clause, and borrow modes are field
  types. A type-erased callable is a `dyn` trait (chapter 14). Concurrent task
  start moves the instance. Ownership determines whether the value may be
  transferred; its four-axis carry policy and the selected runtime contract
  determine whether that activation boundary is legal (chapters 7 and 18).
- A static machine parameter does not reify a machine into ordinary data. It
  cannot be stored, converted to an address, placed into a relocation field, or
  returned as a runtime callback reference. Compile-time substitution alone
  supplies only a direct call in the specialized body. Registered callback
  lowering is contextual instead: a foreign binding parameter names one exact
  callback requirement, selects a named static satisfying machine, and emits
  its thunk/relocation privately without producing a general runtime machine
  value.
- When a public package surface contains a static machine parameter, package
  review retains the authored contract rather than only the `machine` kind.
  Structural contracts include the complete recursively alpha-normalized
  signature and operational envelope; nominal contracts include the exact
  public trait and requirement identities. Renaming machine binders is not an
  API change, while changing any nested contract shape or authority is. A
  private nominal requirement or missing checked contract evidence rejects
  package review.
- Accepted generic axioms are granted once at the normalized template
  statement, including its machine-parameter contract. Each instantiation
  records that template receipt and the selected machine-contract identities
  for audit and cache invalidation, but spends no second grant. A project that
  trusts only particular instances must expose and grant non-generic accepted
  facts instead of granting the universal template.

### Proof-family index telescopes

Proof-side proposition and quotient machinery reads a generic proof carrier as
a family with one typed index telescope:

```text
Rat                         ()
CauchySeq<machine S>        (machine S : Nat -> Rat)
```

The telescope is the declaration's complete ordered static-parameter list; it
is not stored metadata. Relation laws quantify a fresh index pack for each
representative, so `CauchySeq<A>` may relate to `CauchySeq<B>` without erasing
either generator identity. A nullary carrier such as `Rat` uses the same rule
with empty packs.

This does not assign a global relational role to a carrier parameter. The
relation declaration chooses whether its subjects use independent packs or one
shared pack:

```omega
proposition equivalent<machine A, machine B>(
    left: Stream<A>,
    right: Stream<B>
);

proposition same_source<machine S>(
    left: Stream<S>,
    right: Stream<S>
);
```

The first formula permits heterogeneous generator indices; the second
requires one generator. Merely declaring either formula proves nothing. Its
evidence and selected relation-law conformances determine where it may be used.
Static arguments otherwise remain nominally exact during structural lifting;
heterogeneity is authored by each relation's own telescope.

This is a proof-stratum interpretation of the machine parameters already
defined above, not a runtime machine value and not a runtime-dependent carrier.
The proposition-family extension that consumes these telescopes is ordered
ahead of quotient implementation and lives in
[chapter 10](chapter_10_compile_time_proofs.md) and the
[law-bearing relation brief](../design_briefs/law_bearing_relations_and_quotients.md).

A generic proof formula uses a proposition parameter and an explicit family
signature:

```omega
trait Symmetric<C, proposition Relation>
where
    proposition Relation(left: C, right: C);
```

Substitution must provide a proposition family with that binder telescope and
representative-value signature. A resultless machine signature remains an
operation constraint:

```omega
where
    machine Visit(item: &T);
```

It requires an executable procedure and does not introduce a proof formula.

## Where Clauses

`where` clauses describe requirements on generic parameters.

```omega
machine Metrics::sample<T, Counters: T satisfies CounterLike>(
    source: &T,
    out: &mut CounterSnapshot
)
{
    Counters::snapshot(source, out);
}
```

Common requirements:

- Whole-trait evidence parameters: `Counters: T satisfies CounterLike`.
- Value/proof requirements: `N > 0`.
- Reach requirements: a generic operation may be callable only when its
  service reach fits the caller's context.

A required member operation belongs to a named trait and is supplied through
an explicit conformance binder. Omega does not infer or carry an anonymous
one-off requirement from a `machine T::member(...)` clause.

`where` is one construct across the language: its facts hold at every
observation of the declared thing. On a compile-time-known operand (a const
parameter) that collapses to a single instantiation-time proof — this
section. On runtime fields of a `data` declaration it is the default
domain, maintained through invariant windows — see
[Dependent Types](chapter_12_dependent_types.md).

Traits are covered in the next chapter. Generics only need to provide a place
for constraints to live.

A whole-trait evidence binder describes an existing nominal conformance; it
does not declare one. The caller always passes its exact package-scoped name:

```omega
machine sort<Element, Order: Element satisfies Ranked>(
    values: &mut [Element]
);

sort<Card, PowerOrder>(&mut cards);
```

When the selected conformance owns a telescope, its application nests inside
that evidence argument:

```omega
machine send_all<
    Element,
    Message,
    Encoding: Vec<Element> satisfies WireEncodable<Message>
>(values: &Vec<Element>);

send_all<
    u8,
    PlayerMessage,
    SequenceEncoding<u8, PlayerMessage>
>(&items);
```

The outer arguments specialize `send_all`; the inner arguments select one
member of the `SequenceEncoding` conformance family. Every type, `const`, and
static-machine argument owned by that conformance is written explicitly. The
expected subject and trait application validate the resulting closed
conformance but never supply those arguments. A bare generic conformance name,
an `_` hole, or an unsupplied non-lifetime argument rejects. An already-closed
evidence binder such as `Encoding` forwards bare.

Lifetime arguments alone follow the ordinary lifetime-elision rules. An elided
lifetime must resolve uniquely from the ordinary borrow constraints; otherwise
the application rejects or writes the lifetime explicitly. Elision removes
only source ceremony: the resolved normalized region remains in checked
semantic identity even though it contributes no runtime generic argument or
code specialization.

The body uses requirements from that one passed conformance. It never searches
visible declarations or mixes machines from several conformances. A generic
and a concrete specialization may overlap freely because neither is selected
without being named.

## Static Dispatch

Generic dispatch should be static by default.

```omega
machine Runner::tick<T>(
    subject: &mut T
)
where
    machine T::increment(&mut self)
{
    subject.increment();
}
```

For a concrete call with `Counter`, the compiler resolves `Counter::increment`
at compile time. This keeps generic code fast, proof-visible, and compatible
with monomorphization.

Dynamic dispatch is a separate feature for runtime-selected interfaces,
plugins, hot-swap boundaries, and language-neutral extension points.

## Monomorphization

The default implementation strategy should be monomorphization:

```text
generic machine + concrete type arguments -> concrete machine instance
```

This gives the compiler concrete layouts, concrete drop obligations, concrete
reach, and concrete machine targets during later pipeline stages.

The language may later support shared generic code generation where profitable,
but that should be an optimization. It should not change generic semantics.

## Generic Invariants And Reach

Generic code emits generic obligations.

```omega
machine Buffer::first<T, const N: u64>(
    buffer: &FixedBuffer<T, N>,
    out: &mut T
)
where
    N > 0
{
    out = buffer.items[0];
}
```

The obligation `N > 0` is proven when the machine is instantiated. If a caller
has `FixedBuffer<Item, 8>`, the obligation is easy. If a caller has an unknown
`N`, that caller must carry a proof fact for `N > 0`.

Generic service and operational ceilings work the same way: a generic
requirement publishes the service reach, `suspends`/`blocks` possibilities, and
guarded crash buckets of calls through it, and a caller must admit every axis. Allocation capacity
and owned-resource cleanup are not service or operational clauses: they travel
through explicit capability contracts and the multiplicity/ownership rules.

A generic call also uses `suspend`, `block`, or both according to that abstract
envelope. This is not unavoidable pessimism: when the algorithm requires a
non-suspending or nonblocking operation, a transparent refinement narrows the
bound and removes the corresponding marker as well as the possibility.

## Associated Types

The first design should avoid associated types unless they become necessary.

Prefer explicit type parameters:

```omega
trait WireReadable<Message, Value> {
    machine Value::from_wire(message: Message, out: &mut Value);
}
```

This is noisier than an associated type slot, but it is clearer while the trait
system is still young. It also keeps the generic surface close to ordinary data
and machine signatures.

Associated types can be added later if explicit parameters become too clumsy.
