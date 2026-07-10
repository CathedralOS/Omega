# Chapter 23: Dependent Types

A type may name values. That is the whole feature.

> **Status (drafted 2026-07-15): direction for owner review, NOT settled.** This
> chapter specifies the user-facing surface; the companion design brief
> ([dependent_types.md](../design_briefs/dependent_types.md)) carries the deep
> dive, the prior-art evidence, the lifetimes analysis, and the implementation
> ladder. Nothing here is frozen. Reading order: chapters
> [7](chapter_7_types_constraints_invariants.md) (contracts and flow facts),
> [8](chapter_8_domains.md) (domains), [11](chapter_11_relax_scopes.md) (relax),
> and [12](chapter_12_generics.md) (const parameters) are assumed.

## The Problem

Three programs Omega cannot express today, all of them systems code:

```text
1. The UEFI memory map: firmware returns a buffer holding `count` descriptors,
   each `stride` bytes wide — and stride is a RUNTIME value that is larger than
   the descriptor struct you compiled against. Walking the buffer at
   `i * stride` needs the proof `i*stride + stride <= len` where every term is
   a runtime value. (Striding by the compile-time struct size instead is the
   classic firmware bug that corrupts every entry after the first.)

2. pixels[y * width + x]: with `x < width` and `y < height`, the index is in
   bounds — but the proof needs `y*width + x < width*height`, a relation
   BETWEEN runtime values, not a constant range.

3. machine clamp(value, min, max) -> result in min..=max: the result's range
   IS two of the arguments. Today a bound must be a constant.
```

In each case the missing capability is the same: a range, a fact, or a layout
needs to name another value in the program. Constant bounds cannot say it;
dominating guards can establish some of it locally but cannot carry it across
a signature.

## You Already Have One

Dependent types are not a new fact system bolted onto Omega. Three existing
constructs already depend on values:

- **A slice is a length traveling with data.** `items: &[u8]` carries `.len`,
  and indexing obliges `index < items.len` — a fact naming a runtime value.
  The fat descriptor (chapter 19) is the layout half; the length fact is the
  proof half. Every rule in this chapter generalizes what slices already do.
- **A case payload's facts hold under the case fact.** Inside a
  `P::One { v } ->` arm, `v` carries `One`'s declared payload range because the
  arm proves which case is active. A fact valid *conditionally on another
  fact* is dependency, and it is already landed.
- **Const parameters reach ranges and lengths.** Chapter 12's
  `FixedBuffer<T, const N: u64>` puts a value in a layout; chapter 7's clamp
  contract writes `ensures out in min..=max` over `const` parameters. Chapter
  7 already says bounds may name "compile-time or proof-visible values."

This chapter finishes that sentence. The `const` restriction is removed:
**a proof-visible value is any parameter, field, or local the checker can
track — including runtime values.**

## The Rule

```text
A type position or contract may name in-scope, proof-visible values.
Every naming is a FACT the compiler must track.
Every use is an OBLIGATION the entailment engine must discharge from
facts in scope — exactly as constant ranges discharge today.
```

Vocabulary:

- The named value is a **witness**. A witness is always an ordinary stored
  field, parameter, or local the program already carries — never hidden
  metadata. (`len` is its own witness; there is no shadow copy.)
- A fact tying two or more places together (`payload.length == self.len`,
  `count * stride <= len`) is a **coupling**. Couplings on a data type live in
  its default domain (chapter 7) like every other cross-field invariant.
- Facts arrive by the same three routes as every other fact: **declaration**
  (store-enforced at every write), **dominating guards** (flow-scoped), and
  **`as` mints** (validated decode, chapter 8). Dependency adds no fourth
  route.

## Dependent Contracts

Chapter 7's clamp, with `const` deleted — this is the only change:

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

The arm facts (`value >= min`, `value <= max`) discharge the postcondition
exactly as before; `min` and `max` are now runtime values, so the *caller's*
obligation `min <= max` is discharged from the caller's own facts — a guard,
a declared range, or its own contract.

Parameter ranges may name sibling parameters and reachable fields. The
bracket range is sugar for a `requires` fact, exactly as a constant bracket
range is today:

```omega
machine Buffer::get(items: &[u8], index: u64 [0..items.len]) -> u8 {
    items[index]
}
```

`index: u64 [0..items.len]` desugars to `requires index < items.len`. The
callee indexes without a guard; the obligation moved to the call site, where
the caller proves it or establishes it with a dominating guard. This is how a
library exports its obligations instead of re-checking them inside every
call — contract-carried obligations are the composition story.

**Entry values.** There is no implicit `old` (chapter 10). A name bound in
`requires` denotes the value at machine entry, because requires is evaluated
at entry; `ensures` may use it:

```omega
machine Counter::bump(&mut self)
requires self.count == c && self.count < self.cap
ensures self.count == c + 1
```

`ensures` states deltas — what changed, and to what. Preservation of
everything else is the frame's job (see Facts Across Calls), never a list of
"still equals" clauses.

## Dependent Data

A data type's fields may witness each other. The coupling is a cross-field
invariant of the default domain, and everything chapter 7 and chapter 11
settled about cross-field invariants applies unchanged:

```omega
data MemoryMap {
    buf: [u8; 4096];
    len: u32 [0..=4096];
    stride: u32;
    count: u32;
}

domain MemoryMap {
    self.count * self.stride <= self.len;
}
```

(The default-domain declaration surface is an open pin from chapter 7; the
spelling above — a domain block bearing the bare type name — is provisional.)

Working rules:

- **The zero value must satisfy every standing coupling.** This is the
  existing ZII rule — a declared range must include zero — generalized: the
  all-zero value must satisfy the coupling. `count * stride <= len` holds at
  zero (`0 <= 0`); `payload.length == len` holds at zero (`0 == 0`). A
  coupling zero cannot satisfy (`stride >= 40`) is rejected as a standing
  invariant — it belongs in a subdomain established at a mint:

  ```omega
  domain MemoryMap::Loaded {
      self.stride >= 40;
      self.count >= 1;
  }
  ```

  A zeroed `MemoryMap` is a valid value that has established nothing
  (chapter 19); `Loaded` is minted by the boundary decode and shed by
  mutation, like any domain.

- **A lone write to a witness is rejected when it would break a coupling.**
  The store checker enforces couplings at every write, the same pass that
  enforces ranges today. `self.map.count = n` alone is generally unprovable —
  which is correct. The two sanctioned update shapes are chapter 11's:
  construct a valid whole (init-syntax), or open a `relax` scope that updates
  witness and dependents together and re-proves the coupling at exit:

  ```omega
  relax self.map {
      MemoryMap::refill(&mut relaxed self.map, source);
  }
  ```

- **A live borrow of a dependent place pins its witnesses.** `&self.map.buf`
  held across statements implies a read loan on `len`, `stride`, and `count` —
  every place the dependent facts name. A write to a pinned witness while the
  loan lives is a borrow error, by the ordinary loan rules. This one sentence
  is the entire dependent-types/borrow-checking interplay: *a fact about a
  place dies when the place is written; a live dependent borrow therefore
  locks the places its facts name.*

## Static Lowering

When every witness is compile-time-known — a literal, a `const`, a
`const` parameter — the dependent type is chapter 12, unchanged:

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
both operand types) is enforced by instantiation, and every obligation
discharges at build. Zero runtime residue: no witness is stored because
nothing varies.

Static lowering is the degenerate case. It needs this chapter only for the
vocabulary; the machinery is generics.

## Dynamic Lowering

When a witness is a runtime value, three things change — and only three:

1. **The witness is stored, because it already was.** `len`, `stride`, and
   `count` are ordinary fields the program carries anyway. Dynamic lowering
   adds no hidden metadata, no runtime type information, no fat pointers
   beyond the slices that exist today. A zeroed witness means an empty
   structure — ZII holds.

2. **Offsets become ordinary arithmetic.** An access strided by a runtime
   witness lowers to a multiply and an add — the same instructions the
   unverified C version runs. What the proof adds is compile-time only.

3. **Obligations discharge against flow facts** — declared couplings,
   dominating guards, and mint-established subdomain facts — instead of
   constants.

In v1, runtime-sized regions live **behind borrowed views or inside
fixed-capacity buffers** (as `MemoryMap` above: static storage, dynamic
validity). Owned values whose total size is a runtime witness wait for the
`Region` allocator story; nothing in this chapter closes that door.

The memory map, end to end:

```omega
// The boundary contract's success arm mints the facts (chapter 18):
//   ensures self.map.count * self.map.stride <= self.map.len
//   ensures self.map as MemoryMap::Loaded
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
        // Facts in scope: i < count (arm guard), count*stride <= len
        // (standing coupling), stride >= 40 (Loaded).
        let entry: &EfiMemoryDescriptor =
            &self.map.buf[i * self.map.stride] as &EfiMemoryDescriptor;
        ...
        transition { _ -> at(i + 1) }
    }

    state done(&self) { }
}
```

The recast borrow (chapter 19/20's settled design) discharges its bounds
obligation from the arm guard plus the coupling. Note what is *not*
writable: striding by the compile-time size of `EfiMemoryDescriptor` — no
fact ties that constant to `len`, so the classic firmware bug is not a
runtime surprise but a compile error at the recast.

## Multiplication

Both flagship obligations are nonlinear: `i*stride + stride <= len` and
`y*width + x < width*height`. The checker owns these through one closed rule
— bounded products: from `0 <= a <= A` and `0 <= b`, conclude `a*b <= A*b`,
normalized into the polynomial engine. That single rule covers the two
nonlinear shapes systems code produces (index-times-stride, row-major
flattening); there is no general nonlinear arithmetic and no solver.

```omega
machine Canvas::plot(&mut self, x: u32, y: u32)
requires x < self.width && y < self.height
{
    self.pixels[y * self.width + x] = 1;
}
```

*(Bounded products are not yet implemented; the linear relational fragment
lands first. Chapter status note applies.)*

## Facts Across Calls

Dependent facts would be useless if every call destroyed them. The frame
rule is **preserve-unless-written, at borrow granularity**:

- A place the callee cannot reach — no borrow passed, no capability that owns
  it — keeps every fact, verbatim.
- A place passed by shared borrow is frozen: facts survive.
- A place passed by exclusive borrow loses its *flow-scoped extras*
  (guard-minted narrowings, minted subdomains), atom by atom. What survives
  unconditionally is the **floor**: declared ranges, standing couplings, and
  domain memberships survive *any* call — because every store inside the
  callee re-proves them, and a `relax` must close before control returns. A
  `len` sizing its `payload` is usable across an effectful call with zero
  annotations. No surveyed language gets this for free; Omega's
  store-enforcement is why.
- Callee `ensures` adds facts back — the deltas.
- Capability effects havoc the facts minted from that capability's boundary
  (`memory_map` facts die when a `memory_map`-effecting call runs). Effects
  frame *capability-reachable* state only; they never name program places.

At machine boundaries, the written set is declared with a `stores` clause —
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

**Cycles.** A state's signature is its arrival contract. Parameter
refinements — including dependent ones — plus a state-level `requires` are
proven at every in-edge (entry path and back-edges alike) and assumed at
entry; that assumed set is the induction hypothesis the `decreases` rung
already consumes. A self-transitioning state is a loop whose invariant is its
own signature; no separate invariant construct exists or is needed:

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

There are exactly two bridges, both explicit, both already in the language:

- **A dominating guard.** `transition i < self.map.count { true -> ... }` —
  the runtime check *is* the guard, written by you, visible in the state
  graph, with a false arm the no-silent-fallthrough rule forces you to
  handle.
- **An `as` mint.** A validated decode whose success establishes the domain
  fact once, at the boundary, after which every downstream use is check-free.

Omega inserts no silent runtime checks. A failed proof is never a hidden
trap: the language has no ambient failure path for an inserted check to take,
and a check nobody wrote is a check nobody handles. If the engine cannot
prove your index, you will see the guard in your own code — and the false arm
is yours to design.

## What This Is Not (Yet)

- **No type-level computation.** A type never runs a machine; witnesses are
  values, and facts about them live in the entailment engine. Layouts are
  parameterized by values, never computed by arbitrary code.
- **No quantifiers.** Array-wide facts are element ranges and domains carried
  on the type (store-enforced), not `forall` propositions. The quantifier
  fragment is a later rung of the proof-engine north star.
- **No proof objects.** Proofs remain engine-internal and erased by
  construction; the small-kernel endgame
  ([proof_engine_north_star.md](../design_briefs/proof_engine_north_star.md))
  layers under this surface without changing it.

The ladder up — named measures over data, element facts, compile-time proof
machines — is charted in the design brief. Every rung of this chapter
survives those additions unchanged.

## Relationship To Other Chapters

- Chapter 7 owns contracts and the default domain; this chapter widens what
  their facts may name.
- Chapter 8 owns domains and `as` mints; dependent subdomain facts
  (`Loaded`) are ordinary minted domains.
- Chapter 11 owns the mutation discipline; a coupling update is a relax
  scope.
- Chapter 12 owns the static lowering; const parameters are witnesses the
  compiler evaluates away.
- Chapter 19/20 own layout and the recast borrow; dynamic strides are their
  runtime face.
- The index/count model brief (§8, shape-typed views) is the planned home
  for multidimensional index sugar over the raw `y*width + x` spelling.
