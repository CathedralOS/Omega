# Chapter 10: Compile-Time Proofs

Compile-time proofs are not a second programming language.

They are a way to give evidence to the same proof system that checks borrows,
invariants, effects, concurrency, resources, and host boundaries.

The surface should stay close to the rest of Omega:

- Facts appear in contracts: `requires`, `ensures`, `where`, domains, and
  invariants.
- Machines produce guarantees.
- Proof-only machines produce guarantees at compile time and emit no runtime
  code.
- Axioms are trusted roots, not normal proofs.

## Facts, Proofs, And Axioms

These concepts are distinct:

- A fact is something known in a context.
- A proof establishes a fact.
- An axiom is an unproven truth accepted as a trusted root.
- An invariant is a fact attached to a value or boundary.
- A requirement is a fact needed before an operation.
- A guarantee is a fact produced after an operation.

`2 == 2` should be a proven fact, not an axiom.

```omega
proof MathProofs::two_eq_two()
ensures
    2 == 2
{
}
```

The empty body is acceptable only when the compiler can discharge the goal from
built-in rules. Axioms should be rare, loud, and restricted to trusted packages
or compiler foundations.

## Proof Machines

A proof machine is a compile-time-only machine.

```omega
proof IndexProofs::distinct_indices(
    i: usize,
    j: usize
)
requires
    i < j
ensures
    i != j
{
}
```

The proof checker verifies that the `ensures` facts follow from the inputs,
requirements, and proof body. If verified, callers may use those facts.

Proof machines can establish ordinary math:

```omega
proof MathProofs::pythagorean_3_4_5()
ensures
    3 * 3 + 4 * 4 == 5 * 5
{
}
```

They can also establish facts needed by ordinary program checking.

## Naming Properties

Omega already has a way to name semantic properties: domains.

For runtime-shaped values, prefer domains and invariants over inventing a
separate predicate syntax.

```omega
data SliceFacts {
}

data SlicePairI32 {
    before: &[i32];
    after: &[i32];
}

domain SortedI32 for &[i32] {
    // The exact quantifier surface is not designed yet.
    // This domain names the fact that elements are ordered by index.
}

domain SameElements for SlicePairI32 {
    // Names the fact that both slices contain the same values with same counts.
}
```

The bodies above intentionally do not invent a `forall` syntax. Quantified facts
are necessary eventually, but the language should design them as part of the
proof/fact system, not smuggle them into docs as pseudo-code.

For now, the important point is:

```text
named property + contract use + proof obligation
```

not a particular quantifier spelling.

## Machine Contracts

Normal machines can require and guarantee named facts.

```omega
machine Sort::sort(items: &mut [i32])
ensures
    items in SliceFacts::SortedI32
{
}
```

This creates an obligation:

```text
prove items in SliceFacts::SortedI32 at machine completion
```

The implementation may discharge the obligation directly, or it may call helper
machines whose contracts compose.

```omega
machine Sort::insert_sorted(
    items: &mut [i32],
    value: i32
)
requires
    items in SliceFacts::SortedI32
ensures
    items in SliceFacts::SortedI32
{
}
```

Large proofs should be decomposed through helper machines and helper proofs,
not written as one giant proof blob.

## Proof Bodies

Proof bodies should use the same basic control model as machines: straight-line
work, explicit facts, and transitions when a proof needs cases.

Sketch:

```omega
proof CompareProofs::ordered_pair(
    a: i32,
    b: i32
)
ensures
    a <= b || b < a
{
    transition a <= b {
        true -> left_case()
        false -> right_case()
    }

    state left_case() {
    }

    state right_case() {
    }
}
```

This is not runtime control flow. It is compile-time evidence structured like a
machine graph.

The proof checker may also solve many empty proof bodies automatically when the
goal follows from arithmetic, branch facts, type facts, or existing contracts.

## Intersection With Checking

Proof machines feed the same obligation system used by the compiler.

Borrow obligation:

```omega
let a = &mut items[i];
let b = &mut items[j];
```

The borrow checker needs:

```text
i != j
```

If surrounding facts prove `i < j`, a proof such as
`IndexProofs::distinct_indices(i, j)` can discharge the obligation.

Bounds obligation:

```omega
machine Buffer::first<T, const N: usize>(
    buffer: &FixedBuffer<T, N>,
    out: &mut T
)
where
    N > 0
{
    out = buffer.items[0];
}
```

The bounds checker needs `0 < N`. The `where N > 0` fact discharges it.

Concurrency obligation:

```text
lock_order(lock_a) < lock_order(lock_b)
```

That fact might come from a type-level resource hierarchy, a machine contract,
or a proof about the particular resources being used.

## Quantifiers

Sorting, permutation, graph reachability, and liveness eventually need
quantified facts.

The design is still open. Options include:

- domain bodies with quantified fact expressions,
- proof-only machines over finite ranges,
- library-defined finite set/range facts,
- solver-backed contracts over slices and graphs.

What we should not do is pretend Omega already has loops or functions just to
write proof examples. Quantified proof syntax must fit the machine/state model
or be clearly marked as proof-only fact syntax.

## Automation And Trust

The proof engine should solve common cases automatically:

- arithmetic normalization,
- range implications,
- equality facts,
- branch facts,
- disjoint field facts,
- straightforward generic const facts.

When automation fails, users can provide proof machines. When a fact cannot be
honestly established, the remaining option is a trusted axiom or trusted
boundary, which must be visible in build artifacts.

## General Math

Compile-time proofs can establish math unrelated to a runtime program.

```omega
proof MathProofs::add_commutes(a: Int, b: Int)
ensures
    a + b == b + a
{
    MathFoundations::integer_add_commutativity(a, b);
}
```

This is allowed because compile-time proofs establish facts. Program proofs
eventually need ordinary math: indices, lengths, ordering, resource counts,
graph reachability, sortedness, permutation, and liveness all depend on
mathematical facts.
