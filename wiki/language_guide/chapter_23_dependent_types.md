# Chapter 23: Dependent Types

A type, contract, or layout may name in-scope, proof-visible values — fields,
parameters, and locals, not only constants.

> **Status (drafted 2026-07-15): direction for owner review, NOT settled.**
> Motivation, prior-art evidence, and the implementation ladder live in the
> companion design brief
> ([dependent_types.md](../design_briefs/dependent_types.md)). Chapters
> [7](chapter_7_types_constraints_invariants.md),
> [8](chapter_8_domains.md), [11](chapter_11_relax_scopes.md), and
> [12](chapter_12_generics.md) are assumed.

```omega
machine Math::clamp(
    value: i32,
    min: i32,
    max: i32,
    out: &mut i32
) requires min <= max
  ensures out in min..=max
{
    match (value < min, value > max) {
        (true, _) -> { out = min; }
        (false, true) -> { out = max; }
        (false, false) -> { out = value; }
    }
}
```

This is chapter 7's clamp with the `const` qualifiers removed: `min` and `max`
are ordinary runtime parameters, and the result's range names them.

Working interpretation:

- A value named by a type or contract is a **witness**. A witness is always an
  ordinary stored field, parameter, or local the program already carries —
  never hidden metadata. `len` is its own witness; there is no shadow copy.
- A fact tying two or more places together (`payload.length == self.len`,
  `count * stride <= len`) is a **coupling**. Couplings on a data type are
  facts of its default domain, declared with the data declaration
  (chapter 7).
- Every naming is a fact the compiler tracks. Every use is an obligation the
  entailment engine discharges from facts in scope, exactly as constant
  ranges discharge today.
- Facts arrive by the same three routes as every other fact: declaration
  (store-enforced at every write), dominating guards (flow-scoped), and `as`
  mints (chapter 8). Dependency adds no fourth route.

## Dependent Contracts

Contract facts and bracket ranges may name sibling parameters and reachable
fields. A bracket range naming a value is sugar for a `requires` fact, the
same desugar a constant bracket range has today:

```omega
machine Buffer::get(items: &[u8], index: u64 [0..items.len]) -> u8 {
    items[index]
}
```

`index: u64 [0..items.len]` desugars to `requires index < items.len`. The
callee indexes without a guard; the obligation is the caller's, discharged by
a fact in the caller's scope or established with a dominating guard.

Entry values: there is no implicit `old` (chapter 10). A name bound in
`requires` denotes the value at machine entry, because requires is evaluated
at entry; `ensures` may use it:

```omega
machine Counter::bump(&mut self)
requires self.count == c && self.count < self.cap
ensures self.count == c + 1
```

Working rules:

- `requires` and `ensures` facts may name any parameter or reachable field of
  the signature, at any binding time — `const` is no longer required.
- `ensures` states deltas: what changed, and to what. Preservation of
  everything else is the frame's job (see Facts Across Calls), never a list
  of "still equals" clauses.
- A dependent parameter range is an obligation at every call site and a
  standing fact inside the callee.

## Dependent Data

A data type's fields may witness each other. The default domain is declared
with the data declaration: a field constraint is a single-field invariant of
it, and cross-field couplings are facts written in the body alongside the
fields (the exact cross-field spelling is an open pin from chapter 7):

```omega
data MemoryMap {
    buf: [u8; 4096];
    len: u32 [0..=4096];
    stride: u32;
    count: u32;

    self.count * self.stride <= self.len;
    self.stride >= 40;
}
```

> **Gating (settled 2026-07-17).** The zero value either satisfies the
> default domain or it does not, and both are legal:
>
> - **Zero satisfies it** — the type is zero-constructible. A zeroed value
>   is born established; the facts are standing everywhere, with nothing to
>   track. Everything landed today is this tier.
> - **Zero does not** — the type is **gated**. Data can have non-zero
>   requirements; such a type is simply not zero-constructible. Its zeroed
>   form exists only as storage — memory the compiler may still zero-fill —
>   and is inaccessible as the type until construction or an `as` mint
>   proves the default domain. Establishment is monotone as observed: a
>   later write may open an invariant window (chapter 11), but every
>   consumption point closes it, so no observer sees an established place
>   fall back.
>
> `MemoryMap` above is gated (`stride >= 40` fails at zero). The landed rule
> that a declared range must include zero is this model's first tier as an
> implementation restriction, not language law.

Working rules:

- **Construction is the gate.** A gated type's literal must prove the
  default domain, so exactly the fields whose zero violates it are
  mandatory:

  ```omega
  data Player { health: i32 [1..=100]; }    // gated: zero health is not a Player

  self.champion = Player { health = 50 };   // health mandatory; other fields ZII
  ```

- **Gating propagates through containment.** A container of gated data is
  gated: `data Team { roster: [Player; 8]; }` owes eight proven Players at
  construction. A zero-valid first sum case absorbs the gate — emptiness is
  spelled as a case, not as a nonsense zero value:

  ```omega
  data PlayerSlot {
      case Empty;              // tag 0: the zero value IS this case (chapter 19)
      case Filled(p: Player);
  }

  data Team { roster: [PlayerSlot; 8]; }    // zero-constructible: eight Empties
  ```

- **Machine-owned data is access-gated, not construction-gated.** Nobody
  constructs `Main`; it boots zeroed. A gated field inside it is legal as
  storage — the machine simply cannot read it as the type until some state
  establishes it.

- **A write that breaks a coupling opens an invariant window**
  (chapter 11). Write the witness and its dependents in either order; the
  coupling is re-proven from the flow facts at the next consumption point —
  read, borrow, call, transition, or return. Nothing can observe the value
  mid-window. Init-syntax (construct a valid whole) remains the idiomatic
  form when rebuilding is cheap:

  ```omega
  self.map.count = fresh_count;      // window opens: coupling unproven
  self.map.len = fresh_len;          // flow facts accumulate
  // next consumption point proves count * stride <= len, or errors
  // citing both the opening write and the point that needed it closed
  ```

- **A live borrow of a dependent place pins its witnesses.** `&self.map.buf`
  held across statements implies a read loan on `len`, `stride`, and
  `count` — every place the dependent facts name. A write to a pinned witness
  while the loan lives is a borrow error, by the ordinary loan rules.

Facts that describe some values of the type rather than all — facts you do
not want gating every access — remain ordinary subdomains (chapter 8),
minted and shed as usual.

## Static Lowering

When every witness is compile-time-known — a literal, a `const`, a `const`
parameter — the dependent type is chapter 12, unchanged:

```omega
data Matrix<const R: u64, const C: u64> {
    cells: [f64; R * C];
}

machine Matrix::multiply<const R: u64, const K: u64, const C: u64>(
    a: &Matrix<R, K>,
    b: &Matrix<K, C>,
    out: &mut Matrix<R, C>
) { ... }
```

Instances are spelled, monomorphization stamps a concrete layout per
instance, `[f64; R * C]` is a fixed size, dimension agreement (`K` appears in
both operand types) is enforced at instantiation, and every obligation
discharges at build. No witness is stored, because nothing varies.

## Dynamic Lowering

When a witness is a runtime value:

- **The witness is stored as the ordinary field or parameter it already is.**
  Dynamic lowering adds no metadata, no runtime type information, and no fat
  pointers beyond the slices of chapter 19. For a zero-constructible type, a
  zeroed witness means an empty structure; a gated type is never observed
  zeroed.
- **Offsets are ordinary arithmetic.** An access strided by a runtime witness
  lowers to a multiply and an add. The proof work is compile-time only.
- **Obligations discharge against flow facts** — declared couplings,
  dominating guards, and minted subdomain facts — instead of constants.

In v1, runtime-sized regions live behind borrowed views or inside
fixed-capacity buffers (as `MemoryMap` above: static storage, dynamic
validity). Owned values whose total size is a runtime witness are deferred to
the `Region` allocator story.

The memory map, end to end:

```omega
// MemoryMap is gated; the boundary decode is what establishes its default
// domain (chapter 18 owns the boundary ensures). After the success arm,
// self.map's facts are standing: count*stride <= len, stride >= 40.
machine Kernel::walk_map(&self) {
    transition { _ -> at(0) }

    state at(&self, i: u32) {
        transition i < self.map.count {
            true -> visit(i)
            false -> done()
        }
    }

    state visit(&self, i: u32) {
        // Obligation: i*stride + stride <= len.
        // Facts in scope: i < count (arm guard), count*stride <= len and
        // stride >= 40 (default domain, standing since the decode
        // established it).
        let entry: &EfiMemoryDescriptor =
            &self.map.buf[i * self.map.stride] as &EfiMemoryDescriptor;
        ...
        transition { _ -> at(i + 1) }
    }

    state done(&self) { }
}
```

The recast borrow (chapters 19/20) discharges its bounds obligation from the
arm guard plus the coupling. Striding by the compile-time size of
`EfiMemoryDescriptor` instead does not compile: no fact ties that constant to
`len`.

## Products In Obligations

Obligations like `i*stride + stride <= len` and `y*width + x < width*height`
are discharged by one closed rule — bounded products: from `0 <= a <= A` and
`0 <= b`, conclude `a*b <= A*b`, normalized into the polynomial engine. There
is no general nonlinear arithmetic and no solver.

```omega
machine Canvas::plot(&mut self, x: u32, y: u32)
requires x < self.width && y < self.height
{
    self.pixels[y * self.width + x] = 1;
}
```

*(Bounded products are not yet implemented; the linear relational fragment
lands first.)*

## Facts Across Calls

The frame rule is **preserve-unless-written, at borrow granularity**:

- A place the callee cannot reach — no borrow passed, no capability that owns
  it — keeps every fact.
- A place passed by shared borrow is frozen: facts survive.
- A place passed by exclusive borrow loses its flow-scoped extras
  (guard-minted narrowings, minted subdomains), atom by atom. Declared
  ranges, standing couplings, and domain memberships survive every call:
  calls and returns are consumption points (chapter 11), so a callee cannot
  return — or call onward, or hand out a borrow — with an open window.
- Callee `ensures` adds facts back.
- Capability effects havoc the facts minted from that capability's boundary.
  Effects frame capability-reachable state only; they never name program
  places.

At machine boundaries the written set is declared with a `stores` clause —
named for what the store checker already enforces — and checked callee-side
by that same pass (an undeclared store is a compile error). Within a
compilation unit it is inferred; boundary traits must write it:

```omega
machine Table::insert(&mut self, item: Item)
stores self.len, self.slots
requires self.len == n && self.len < self.cap
ensures self.len == n + 1
```

*(Clause spelling provisional.)*

A state's signature is its arrival contract. Parameter refinements —
dependent ones included — plus a state-level `requires` are proven at every
in-edge, entry path and back-edges alike, and assumed at entry; that assumed
set is the induction hypothesis the `decreases` rung consumes. A
self-transitioning state is a loop whose invariant is its own signature:

```omega
state fill(&mut self, i: u64 [0..=self.cap]) {
    transition i < self.cap {
        true -> write_one(i)
        false -> done()
    }
}
```

## When The Checker Says No

An undischarged dependent obligation is a compile error naming the missing
fact:

```text
error: cannot prove `i * self.map.stride + self.map.stride <= self.map.len`
       at the recast borrow; missing fact: `i < self.map.count`
       (establish it with a dominating guard, or carry it in a contract)
```

Omega inserts no silent runtime checks. There are exactly two bridges, both
explicit, both already in the language:

- **A dominating guard.** `transition i < self.map.count { true -> ... }` —
  the runtime check is the guard, written in your code, with a false arm the
  no-silent-fallthrough rule forces you to handle.
- **An `as` mint.** A validated decode whose success establishes the domain
  fact once, at the boundary, after which downstream uses are check-free.

## Scope

This chapter is intentionally narrow:

- **No type-level computation.** A type never runs a machine. Layouts and
  facts are parameterized by values, never computed by arbitrary code.
- **No quantifiers.** Array-wide facts are element ranges and domains carried
  on the type (store-enforced), not `forall` propositions.
- **No proof objects.** Proofs remain engine-internal and erased. The
  small-kernel endgame
  ([proof_engine_north_star.md](../design_briefs/proof_engine_north_star.md))
  layers under this surface without changing it.

## Relationship To Other Chapters

- Chapter 7 owns contracts and the default domain; this chapter widens what
  their facts may name.
- Chapter 8 owns domains and `as` mints; establishing a gated default domain
  at a boundary decode is an ordinary `as` mint.
- Chapter 11 owns the mutation discipline; a coupling update is an
  invariant window closed at the next consumption point.
- Chapter 12 owns the static lowering; const parameters are witnesses the
  compiler evaluates away.
- Chapters 19/20 own layout and the recast borrow; dynamic strides are their
  runtime face.
- The index/count model brief (§8, shape-typed views) is the planned home
  for multidimensional index sugar over the raw `y*width + x` spelling.
