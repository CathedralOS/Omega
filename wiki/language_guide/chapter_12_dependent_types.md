# Chapter 12: Dependent Types

A type, contract, or layout may name in-scope, proof-visible values — fields,
parameters, and locals, not only constants.

The staged systems fragment is settled; implementation remains incomplete.
Motivation, prior-art evidence, and the implementation ladder live in the
companion design brief
([dependent_types.md](../design_briefs/dependent_types.md)). Chapters
[7](chapter_7_types_constraints_invariants.md),
[8](chapter_8_domains.md), [11](chapter_11_invariant_windows.md), and
[13](chapter_13_generics.md) are assumed.

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
  qualifications (chapter 8). Dependency adds no fourth route.

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

Entry values are not selected implicitly. `old(place)` explicitly denotes a
structural place at the callable-entry revision (chapter 10). A name bound in
`requires` denotes the value at machine entry, because `requires` is evaluated
there; `ensures` may use it:

```omega
machine Counter::bump(&mut self)
requires self.count == c && self.count < self.cap
ensures self.count == c + 1
```

Working rules:

- `requires` and `ensures` facts may name any parameter or reachable field of
  the signature, at any binding time — `const` is no longer required.
- `ensures` states results, including exact preservation guarantees where a
  public interface needs them. Internal inferred mutation summaries preserve
  other facts when the callee is known (see Facts Across Calls).
- A dependent parameter range is an obligation at every call site and a
  standing fact inside the callee.

## Dependent Data

A data type's fields may witness each other. The default domain is declared
on the data signature as a `where` clause: bare field
names, any number of facts, holding at every observation of the value. A
field constraint is single-field sugar for a `where` fact; the body stays
pure layout. Semantically and in the implementation these facts ARE the
default domain — the clause is a spelling over that model, so re-skinning
the syntax later is near-trivial by design:

```omega
data MemoryMap
where
    count * stride <= len,
    stride >= 40,
{
    buf: [u8; 4096];
    len: u32 [0..=4096];
    stride: u32;
    count: u32;
}
```

This is the same clause position generics use (chapter 13), deliberately:
`where N > 0` on a const parameter and `where count * stride <= len` on
runtime fields are one construct at two binding times — a compile-time-known
operand collapses the every-observation obligation to a single
instantiation-time proof, which is exactly the static lowering rule.

> **Gating.** The zero value either satisfies the default domain or it does
> not, and both are legal:
>
> - **Zero satisfies it** — the type is zero-constructible. A zeroed value
>   is born established and its facts stand everywhere.
> - **Zero does not** — the type is **gated**. Data can have non-zero
>   requirements; such a type is simply not zero-constructible. Its zeroed
>   form exists only as storage — memory the compiler may still zero-fill —
>   and is inaccessible as the type until construction or an `as` qualification
>   proves the default domain.
>
> `MemoryMap` above is gated (`stride >= 40` fails at zero). The current
> implementation accepts only zero-constructible declared ranges; that is an
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
      case Empty;              // tag 0: the zero value IS this case (chapter 20)
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
  mid-window. An explicit `crash` is the one no-successor exception: it may
  abandon the open window. The checked site records the invariant-bearing
  identity in its abandonment lower bound, but that record does not prove any
  survivor safe. Init-syntax (construct a valid whole) remains the idiomatic
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
established and shed as usual.

## Static Lowering

When every witness is compile-time-known — a literal, a `const`, a `const`
parameter — the dependent type is chapter 13, unchanged:

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
  pointers beyond the slices of chapter 20. For a zero-constructible type, a
  zeroed witness means an empty structure; a gated type is never observed
  zeroed.
- **Offsets are ordinary arithmetic.** An access strided by a runtime witness
  lowers to a multiply and an add. The proof work is compile-time only.
- **Obligations discharge against flow facts** — declared couplings,
  dominating guards, and established subdomain facts — instead of constants.

Runtime-sized data takes exactly three shapes, permanently: borrowed views
(`{ptr, len}` over someone else's bytes), fixed-capacity buffers with
dynamic validity (as `MemoryMap` above: static storage, a runtime valid
prefix carried as facts), and — once the `Arena` allocator lands — owned
allocations (`{handle, len}`, Vec-shaped, the length proof-visible). An
owned value whose INLINE size is a runtime witness (`payload: [u8; len]` as
machine-resident storage) is not part of the language: the facts never
cared where the bytes live, and the one language that shipped inline
value-dependent layout spent forty years paying for it. The same spelling
remains legal in wire schemas (chapter 21), where it describes serialized
bytes; decode establishes it in one of the three shapes.

The memory map, end to end:

```omega
// MemoryMap is gated; the boundary decode is what establishes its default
// domain (chapter 19 owns the boundary ensures). After the success arm,
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

The recast borrow (Chapter 20) discharges its bounds obligation from the
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
- A place passed by write-only borrow is exclusively loaned. The callee may
  use only content-independent projections and writes whose legality follows
  from written inputs, static structure, or explicitly supplied facts. Caller
  facts survive for paths the exact outcome write frame leaves unchanged;
  facts depending on a written path are invalidated and may be re-established
  by the callee's guarantees.
- A place passed by exclusive borrow to an opaque callee or unknown dynamic
  conformance loses its flow-scoped extras (guard-established narrowings,
  established subdomains), atom by atom. A resolved checked callee invalidates
  only the places its inferred mutation summary may overlap. Declared ranges,
  standing couplings, and domain memberships survive every call:
  calls and returns are consumption points (chapter 11), so a callee cannot
  return — or call onward, or hand out a borrow — with an open window.
- Callee `ensures` adds facts back.
- Capability reach havocs the facts established from that capability's boundary.
  Reach frames capability-reachable state only; it never names program
  places.

Checked bodies infer normalized mutation summaries as implementation metadata.
A statically selected checked callee may use that summary to preserve facts
about disjoint places, including across separate compilation when the artifact
publishes the summary. An opaque callee, an unknown dynamic conformance, or an
unresolved or overlapping summary invalidates the flow-scoped facts of every
mutable place reachable from the call's signature.

Public contracts recover any precision the interface deliberately guarantees
with ordinary postconditions:

```omega
boundary trait TableStorage {
    machine reserve(table: &mut Table, additional: u64)
    requires table.capacity == capacity0 && table.hasher == hasher0
    ensures table.capacity >= capacity0
    ensures table.hasher == hasher0;
}
```

Equality to a `requires`-bound entry value transports every fact about that
place; it is not limited to one known predicate. Prefer narrower mutable
parameters when only a subobject changes, so the signature itself provides the
useful frame:

```omega
machine insert(entries: &mut Entries, item: Item) { ... }
```

Broad mutable receivers therefore have broad conservative invalidation under
opaque or abstract dispatch. Interfaces that need structural precision should
accept the narrowest mutable place they require.

Inferred summaries are normalized complete-or-opaque checked plans. Complete
paths sort and deduplicate, state parameters normalize positionally, and each
summary has a deterministic implementation fingerprint. They remain under the
machine-contract artifact's `implementation` section and do not enter authored
contract or specialization identity.

An acyclic state-transition graph participates in the same inference. The
summary unions every conditional arm, memoizes shared downstream states, and
substitutes target-state parameters back through their transition arguments.
Value-position calls nested in a state body contribute their shared inferred
call frames before the containing statement or jump, including calls in local
initializers, assignment operands, statement-call arguments, transition
subjects and arguments, and returned values. Recursive statement- and
value-call graphs share cycle detection. Every reachable state-transition
cycle and every genuinely unresolved frame remain opaque. Callers may
therefore use exact preservation only when the complete control-flow
implementation was summarized; one terminating observed route never licenses
a cyclic machine.

A state's signature is its arrival contract. Parameter refinements —
dependent ones included — plus an explicit state-level `requires` are proven
at every in-edge, including guarded named transitions and back-edges, and are
assumed only inside that state. A state accepts `requires` after its return
type and before its body; exit guarantees and effect/termination clauses stay
on the machine. The assumed arrival set is the induction hypothesis the
ranking-witness rung consumes. A self-transitioning state is a loop whose
invariant is its own signature, so a mutation that invalidates the invariant
must be followed by a guard or other proof that re-establishes it before the
back-edge:

A witness-bearing arrival requirement may bind its evidence term. Each named
transition supplies those state-local terms explicitly, in clause order, after
the same `;` separator used by ordinary calls:

```omega
state ready(value: i32)
requires proof: carries(value);
{
    transition { _ -> next(value; proof) }
}
```

An internal transition does not retransmit the enclosing machine's named
`requires` package; those terms remain in scope throughout that machine
invocation. Missing, extra, unknown, or proposition-mismatched state-arrival
terms reject before execution, and evidence contributes no runtime argument.

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

The contract is part of the state's typed and specialization identity. It is
not a comment or a body-local assertion: an unconditional edge to `fill(n)`
is rejected unless the current proof context establishes `n <= self.cap`.

For the common write-first machine-field loop, the checker also infers one
relational arrival fact without authored syntax. If every entry and back edge
to an increasing-counter head establishes the same
`self.i < self.items.len`, and recursive call frames prove both places stable,
that collection-relative index fact holds at the head. Equivalent guards in
different states are matched semantically rather than by syntax-tree identity.
Reassigning `self.i` immediately invalidates the fact; a collection write or an
opaque/overlapping call prevents the candidate entirely.

A finite chain of stable intermediate bounds may be composed too. Any relation
may supply the strict link: edge/contract chains
`self.i < self.limit <= self.items.len` and
`self.i <= self.outer <= self.limit < self.items.len` both give the head
`self.i < self.items.len`. A fully non-strict chain does not; it permits the
out-of-bounds equality case and is rejected.
Because every bridge premise was established at machine arrival, the checker
requires each intermediate place and `self.items` to remain frame-stable in
every machine state, including the preheader. A preheader assignment or
overlapping call therefore blocks this candidate even when the natural loop
itself is read-only.

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
- **An `as` qualification.** A validated decode whose success establishes the domain
  fact once, at the boundary, after which downstream uses are check-free.

## Scope

### Proof-static domain indices

A staged extension may let an erased domain family take canonical static values
as indices and let a generic result constrain its index using expressions over
input indices. These indices are ordinary first-order proof/static data, not
unique type IDs, predicates, runtime fields, or inhabitants of a type universe.
The generic declaration binds its carrier explicitly before using it in the
ordinary carrier position: `domain<T, const U: Unit> T::Quantity<U>;`.

Closed indices evaluate and compare canonically. An open generic index remains
a constraint fact: use at an expected index produces an equality obligation,
discharged by closed evaluation, licensed canonical normalization, or an
established local fact. Otherwise it rejects. The successful judgment performs no
runtime transport because the domain is erased and the carrier is unchanged.
Normalization determines interface identity; proof strength may accept more
compatible uses but may not rewrite that identity.

This is enough for libraries to build zero-representation-cost units,
coordinate frames, currencies, tensor shapes, fixed-point scales, and protocol
indices. The compiler does not know their meanings or conversions.

The implemented systems fragment in this chapter is intentionally narrow:

- **No type-level computation.** A type never runs a machine. Layouts and
  facts are parameterized by values, never computed by arbitrary code.
- **No runtime-general quantifiers.** Array-wide facts are element ranges and
  domains carried on the type (store-enforced), not hidden runtime loops.
- **No runtime proof objects.** Runtime dependent data acquires no hidden proof
  field, layout, or cleanup.

The proof stratum now has an internal `Prop` universe for formulas, distinct
from runtime `Type` and from effectful machine computation. A dedicated
`proposition R(left: C, right: C);` declaration introduces a nominal family,
and `R(left, right)` applies it in a fact position. Proof inhabitants are
erased, copyable, and cannot contribute runtime storage, be inspected by
runtime code, or participate in machine layout. Explicit `[erased]` bindings
may retain them in typed data for proof, validity, and provenance checking while
lowering omits those bindings. An explicitly erased Type ghost instead retains
ordinary Type multiplicity and conservation. The current source fragment
exposes proposition-valued families over representative values with typed
proof-static index telescopes and
carrierless evidence; it does not yet expose `Prop` itself as an arbitrary
first-class source value or admit value-to-runtime-`Type` computation. It must
land before evidence-bearing quotients and is specified in
[Law-Bearing Relations, Evidence, And Quotients](../design_briefs/law_bearing_relations_and_quotients.md).
The small-kernel endgame
([proof_engine_north_star.md](../design_briefs/proof_engine_north_star.md))
layers under both surfaces.

## Relationship To Other Chapters

- Chapter 7 owns contracts and the default domain; this chapter widens what
  their facts may name.
- Chapter 8 owns domains and `as` qualification; establishing a gated default
  domain at a boundary decode is an ordinary proved qualification.
- Chapter 11 owns the mutation discipline; a coupling update is an
  invariant window closed at the next consumption point.
- Chapter 10 owns proof machines and evidence; a dependent contract may cite
  a theorem — a fact justified by a proof machine, instantiated at the
  operands — including refinement facts equating a runtime place with a pure
  machine's result. It also owns the proof-only proposition-family extension;
  this chapter does not generalize that extension into runtime dependent
  types.
- Chapter 13 owns the static lowering; const parameters are witnesses the
  compiler evaluates away. It also owns the staged structured-static-parameter
  and indexed-domain generic surface.
- Chapter 20 owns layout and the recast borrow; dynamic strides are its
  runtime face.
- The index/count model brief (§8, shape-typed views) is the planned home
  for multidimensional index sugar over the raw `y*width + x` spelling.
