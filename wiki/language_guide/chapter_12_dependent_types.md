# Chapter 12: Dependent Types

A range, contract, or view can name ordinary in-scope values, not only constants.
The compiler tracks the relationship; the program does not acquire hidden
witness storage or runtime type metadata merely because a fact names a value.

This also applies to [runtime-capable value binders](chapter_13_generics.md#runtime-capable-value-parameters):
`Buffer<count>` may retain the exact runtime extent when Buffer declares a
non-const value binder and supplies a valid representation. `const` remains an
explicit requirement for static knowledge, not a universal property of generics.

The [dependent-value specification](../spec/language/dependent_values.md) defines
this systems fragment. Examples show intended contracts; implementation support
for relational proofs and views remains narrower. General mathematical
foundations have their separate [proof contract](../spec/proofs/contracts.md).

## Dependent Contracts

A parameter's range may name another parameter:

```omega
machine Buffer::get(items: &[u8], index: u64 [0..items.len]) -> u8 {
    items[index]
}
```

For this unsigned index, the range supplies `index < items.len`. The caller
proves it, perhaps using a visible guard, and the callee may rely on it. A
runtime length is a witness already stored in the slice, not a const argument.

Pre-state relationships must name a real binding or use `old(place)`. Equality
in `requires` does not invent a snapshot variable. For example, the caller can
pass a scalar snapshot explicitly:

```omega
machine Counter::bump(&mut self, before: u32)
requires
    self.count == before
    self.count < self.cap
ensures
    self.count == before + 1
{
    self.count = self.count + 1;
}
```

This sketch assumes `count` and `cap` are `u32` fields. The strict bound also
establishes representability of the increment. `old(place)` instead selects an
existing structural place's callable-entry revision without copying an owned
value; see [proof views](chapter_10_compile_time_proofs.md#proof-views).

## Dependent Data

Fields can witness one another. Their coupling belongs to the data's default
domain, expressed with `where`:

```omega
data MemoryMap
where
    embed(count) * embed(stride) <= embed(len),
    stride >= 40,
{
    buf: [u8; 4096];
    len: u32 [0..=4096];
    stride: u32;
    count: u32;
}
```

Every observation must satisfy that domain. `embed` states the product bound in
unbounded proof arithmetic; actual runtime offset computations still prove their
own representability. The coupling does not execute a multiplication or exempt
a runtime intermediate from ordinary arithmetic obligations.

This type is gated because zero does not satisfy `stride >= 40`. Zeroed backing
may exist as storage but is not yet a `MemoryMap` value. Construction or checked
qualification establishes the complete domain. A decoder proves only its checked
conditions, not that firmware told the truth or that the bytes grant authority.

Gating propagates through contained values. An explicit empty case can make an
optional container zero-constructible without inventing an invalid payload:

```omega
data Player { health: i32 [1..=100]; }

data PlayerSlot {
    case Empty;
    case Filled(player: Player);
}

data Team { roster: [PlayerSlot; 8]; }
```

The zero case contains no `Player`; constructing `Filled` owes a valid one.
Machine-owned backing may likewise begin zeroed while gated fields remain
inaccessible until established.

Mutating witnesses and dependents can open an
[invariant window](chapter_11_invariant_windows.md), which must close before
consumption. A live dependent borrow read-loans the witnesses required by its
validity, so those witnesses cannot change while the view remains live. Optional
facts about only some values belong in ordinary domains, not the default domain
of every instance.

## Static Lowering

When dimensions are static, ordinary generics select a fixed shape:

```omega
data Matrix<const R: u64, const C: u64> {
    cells: [f64; R * C];
}
```

A concrete `Matrix<3, 4>` has twelve cells after its layout and arithmetic
obligations are checked. Static arguments enter application identity and need
not be stored as runtime witnesses. [Generics](chapter_13_generics.md) explains
those applications; runtime dimensions do not become const arguments by analogy.

## Dynamic Lowering

A runtime witness remains its ordinary stored field, parameter, or view value.
A runtime-capable generic index can use that same witness. Compile a shared body
over its value where possible; static-only operations require a justified static
selection or another supported realization. No new machine is generated for each
value read from input. Index-only proof facts may erase, but executable size or
offset computations retain their needed values.

A strided access can lower to ordinary offset arithmetic; the proof establishes
that its actual byte range lies in the backing region. Dynamic-sized data lives
behind checked views or provisioned buffers/storage, not variable-sized stack
locals or an implicit global layout descriptor.

For a foreign memory map, the supplied stride determines where the next record
starts. A useful access proof relates `i < count`, `count * stride <= len`, and
the requested record's extent. Bounds are only part of establishing a typed
view: alignment, selected representation, initialized content, and ownership
still matter. Casting a byte's address does not silently prove all of them.

Use a checked view/recast or decoder under its exact
[layout plan](../spec/layouts/plans.md) and [recast contract](../spec/layouts/recasts.md).
The compiled record size cannot replace a foreign stride without the necessary
relation. Runtime witnesses add no implicit boxing, hidden proof tuple, or
arbitrary runtime computation of nominal types.

## Products In Obligations

Row-major indexing needs more than two independent range checks. To justify
`y * width + x`, relate the coordinates, dimensions, and total backing extent,
and prove each arithmetic intermediate representable. Mathematical facts such
as multiplying a nonnegative inequality by a nonnegative value are useful proof
steps; they are not a promise of a complete nonlinear solver.

The checker may discharge a supported instance, consume an explicitly cited
theorem, or reject an unproved obligation. Current automation limits belong
beside [validation](../../omega-rust/psi/semantics/validation/README.md), not in
the definition of dependent values.

## Facts Across Calls

The practical frame rule is preserve-unless-written, at exact storage granularity:

- Unreachable places retain their facts.
- A shared loan excludes ordinary conflicting mutation. Synchronized mutation
  retains its own contract and fact-invalidation rules.
- Mutable or write-only access may invalidate facts about written places.
- Exact outcome guarantees can preserve or establish facts after the call.

A checked implementation may publish a complete narrower mutation frame. An
opaque or unknown dynamic call uses its conservative signature and authority
ceiling; unknown is not an empty write set. Operand evaluation contributes writes
too. Broad mutable receivers therefore lose more precision than narrow parameters.
Default-domain facts must be restored before consumption; that is not a promise
that the value stayed unchanged.

For example, an insertion helper taking `&mut Entries` cannot mutate an unrelated
hasher merely because both are fields of a larger table. A helper receiving the
whole table needs a checked frame or an explicit preservation guarantee to retain
that precision. Entry/current comparisons use `old(place)` or a real pre-state
binding, not a name introduced by an equality expression.

State signatures are arrival contracts. Every incoming transition proves them
under the exact substitution, including backedges after mutation:

```omega
state fill(&mut self, i: u64)
requires
    i <= self.cap
{
    transition i < self.cap {
        true -> fill(i + 1)
        false -> done()
    }
}
```

This sketch assumes a `u64` capacity and the enclosing `done` state. The target
may assume its own arrival facts, not stale machine-entry facts. Proof-only
arguments obey the same substitution and validity rules before erasure; no hidden
witness-transition syntax is needed. See [state contracts](../spec/language/state_contracts.md)
and [mutation frames](../spec/language/dependent_values.md#mutation-frames).

## When The Checker Says No

An unproved dependent use rejects with the missing relationship and its subject.
For a descriptor view, that might be the absence of `i < count`, or a missing
alignment or representation fact rather than a numeric bound.

Supply a contract, cite an appropriate proof, use an explicit guard with a handled
false outcome, or call a validator/decoder whose success establishes the required
fact. An `as` qualification uses established evidence; it is not a hidden
validation call. Omega does not insert residual runtime checks to make an
unproved refinement true.

## Scope

### Proof-static domain indices

Static domain indices and runtime witnesses are different applications of
value-dependent reasoning:

```omega
domain<T, const U: Unit> T::Quantity<U>;
```

This family binds its carrier and static index explicitly. Closed indices use
canonical values; open expressions use licensed normalization or exact local
facts for compatibility without rewriting interface identity. Qualifying the
same carrier adds no representation, but a units library's scaling or rounding
operation can still perform real work. See [indexed domains](../spec/language/domains.md#indexed-families).

Runtime-capable generic value indices are distinct from arbitrary runtime
creation or selection of nominal type declarations. They require an existing
declared family and a valid realization, with no implicit runtime proof objects,
storage allocation, or unlimited specialization. Existing const-indexed families
retain their static requirement. These rules neither prohibit eligible static
evaluation in layout plans nor restrict general proof-side quantification.
The [mathematical foundation](../spec/proofs/foundation.md) supports arbitrary
mathematical functions/predicates and noncomputable values independently of
runtime representation. General source binders and the exact extended profile
remain [open joins](../spec/proofs/contracts.md#open-joins), not limits inferred
from this systems fragment.

## Relationship To Other Chapters

Chapter 7 introduces default domains; Chapter 8 explains qualification;
Chapter 11 explains restoring validity after mutation. Chapter 10 supplies
proofs and explicit pre-state reasoning, Chapter 13 handles staged generic
applications, and Chapter 20 explains the representation and view obligations
that a dependent bound alone cannot establish.
