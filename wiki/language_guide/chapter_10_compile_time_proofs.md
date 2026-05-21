# Chapter 10: Compile-Time Proofs

Omega's proof language should exist to help the compiler discharge obligations.

Borrow checking, invariant checking, effect checking, concurrency proof
obligations, and resource cleanup all produce obligations. Most should be
solved automatically. When automation is not enough, users should be able to
write compile-time proofs in the language.

The proof language is not a second runtime language. It is compile-time evidence
for facts.

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
proof two_eq_two() -> 2 == 2 {
    trivial
}
```

Axioms should be rare, loud, and restricted to trusted packages or compiler
foundations. Inconsistent axioms can make the proof system prove nonsense.

## Proof Machines

A proof machine is compile-time-only evidence.

```omega
proof distinct_indices(
    i: usize,
    j: usize
)
requires
    i < j
ensures
    i != j
{
    auto
}
```

The proof checker verifies the body. If verified, the `ensures` facts become
available wherever the proof is used.

Proof machines can establish ordinary math facts:

```omega
proof pythagorean_3_4_5() -> 3 * 3 + 4 * 4 == 5 * 5 {
    auto
}
```

They can also establish facts needed by ordinary program checking.

## Predicates

Predicates name proof-level properties.

```omega
predicate sorted(items: &[i32]) {
    forall i, j in items.indices:
        i <= j -> items[i] <= items[j]
}

predicate permutation(before: &[i32], after: &[i32]) {
    forall value:
        count(before, value) == count(after, value)
}
```

Predicates do not store runtime data. They describe facts the proof checker can
use.

## Machine Contracts

Normal machines can require and guarantee predicates.

```omega
machine Sort::sort(items: &mut [i32])
ensures
    sorted(items),
    permutation(old(items), items)
{
}
```

This creates obligations:

- prove `sorted(items)` at machine completion,
- prove `permutation(old(items), items)` at machine completion.

The implementation may discharge those obligations directly, or it may call
helper machines whose contracts compose.

```omega
machine Sort::insert_sorted(
    items: &mut [i32],
    value: i32
)
requires
    sorted(items)
ensures
    sorted(items)
{
}
```

Large proofs should be decomposed through helper machines and helper proofs,
not written as one giant proof blob.

## Proof Steps

The proof vocabulary should stay small.

Candidate steps:

- `trivial`: solve by reflexivity or an obvious built-in rule.
- `auto`: ask the proof engine to solve the current goal.
- `use`: invoke an existing proof.
- `rewrite`: replace equals with equals.
- `cases`: split on a value, domain, or pattern.
- `induction`: prove recursive or structural cases.
- `show`: name the current goal.
- `assume`: introduce a local assumption for implication/case proofs.

The exact syntax can evolve. The goal is not to clone Lean's terminology. The
goal is to give users a way to supply evidence when the compiler needs help.

## Intersection With Checking

Proof machines feed the same obligation system used by the compiler.

Example borrow obligation:

```omega
let a = &mut items[i];
let b = &mut items[j];
```

The borrow checker needs:

```text
i != j
```

If surrounding facts prove `i < j`, a proof such as `distinct_indices(i, j)` can
discharge the obligation.

Example bounds obligation:

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

Example concurrency obligation:

```text
lock_order(lock_a) < lock_order(lock_b)
```

That fact might come from a type-level resource hierarchy, a machine contract,
or a proof about the particular resources being used.

## Automation And Trust

The proof engine should solve common cases automatically:

- arithmetic normalization,
- range implications,
- simple equality rewrites,
- obvious branch facts,
- simple disjoint field facts,
- straightforward generic const facts.

When automation fails, the user can provide proof code. When proof code cannot
honestly establish a fact, the only remaining option is a trusted axiom or
trusted boundary, which must be visible in build artifacts.

## General Math

The same proof language can prove math unrelated to a runtime program.

```omega
proof add_commutes(a: Int, b: Int) -> a + b == b + a {
    use integer_add_commutativity(a, b);
}
```

This is allowed because compile-time proofs are general facts. The practical
reason to include them is that program proofs eventually need ordinary math:
indices, lengths, ordering, resource counts, graph reachability, sortedness,
permutation, and liveness all depend on mathematical facts.
