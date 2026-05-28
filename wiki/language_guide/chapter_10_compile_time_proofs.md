# Chapter 10: Compile-Time Proofs

Compile-time proofs are not a second programming language.

They are ordinary machines whose contracts are checked as evidence. If a
machine is used only to establish facts, it emits no runtime code.

The basic shape is:

```text
requires + body facts -> ensures
```

If the checker can prove that implication, the machine is a proof artifact. If
it cannot, the contract is only an unchecked promise and must be rejected or
treated as an explicit boundary.

## Machines As Proofs

This machine proves a simple ordering fact:

```omega
machine distinct_indices(
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

The empty body is valid only if the checker can prove the guarantee from the
requirement and built-in arithmetic/order rules.

This machine proves a closed arithmetic fact:

```omega
machine pythagorean_3_4_5()
ensures
    3nat * 3nat + 4nat * 4nat == 5nat * 5nat
{
}
```

The checker reduces both sides to the same `Nat` value, then closes the equality
by reflexivity. The body does not need to simulate computation.

## Typed Facts

Proof facts must be typed.

```omega
3nat * 3nat
```

is math over `Nat`.

```omega
3i32 * 3i32
```

is machine arithmetic and carries machine obligations such as width and
overflow behavior.

The same operator spelling can exist in both worlds. The operand types decide
which proof rules apply.

## Proof Views

Runtime data often needs a mathematical view before it can be reasoned about.

For slices, useful proof views include:

```text
Seq(items)    ordered finite sequence view
Bag(items)    finite multiset/counting view
Range(len)    finite index space
```

These are proof-only views. They do not allocate at runtime. They let contracts
talk about math without pretending that proof binders are runtime loops.

Sorting is naturally expressed as:

```omega
machine Sort::bubble_sort_preserving(
    before: &[Nat],
    items: &mut [Nat]
)
requires
    Bag(items) == Bag(before)
ensures
    Seq(items) in Sorted
    Bag(items) == Bag(before)
{
}
```

The `before` value is explicit. There is no implicit `old` keyword here. A
caller that wants to prove preservation can make or carry a snapshot itself.

## Helper Machines

Large proofs should be decomposed through helper machines with small contracts.

```omega
machine Sort::compare_swap(
    before: &[Nat],
    items: &mut [Nat],
    index: usize
)
requires
    index + 1 < items.len
    Bag(items) == Bag(before)
ensures
    items[index] <= items[index + 1]
    Bag(items) == Bag(before)
{
}
```

The preservation fact is explicit. If a caller needs a before-state, it passes
one in. Nothing in this chapter relies on an implicit snapshot keyword.

A sorting proof is built from smaller facts:

```text
compare/swap orders one adjacent pair
compare/swap preserves Bag(items)
one pass moves the largest remaining item to the end
repeated passes establish Seq(items) in Sorted
Bag(items) stays equal to the explicit before value
```

## Quantified Facts

Full sorting correctness requires global facts:

```text
every earlier index has a value <= every later index
every value has the same count before and after sorting
```

The language still needs a proof-level way to express those facts. That does
not mean adding runtime loops. It means adding fact syntax, proof views, or
foundation-library definitions that the checker understands.

Possible directions:

- `Seq<T>` and `Bag<T>` are compiler-known finite math objects.
- Domains like `Sorted` are defined over proof views.
- Advanced libraries can define reusable facts about `Seq`, `Bag`, `Range`, and
  state graphs.
- General quantifiers, if added, are proof-level binders in contracts/domains,
  not executable machine control flow.

## Termination Proofs

Termination is another proof shape the checker should eventually understand.

Unlike ordinary pre/postcondition checking, termination is a claim about every
cycle in the reachable machine/state graph.

Working direction:

```omega
machine walk(items: &[Nat])
terminates {
    decreases items -> Slice::Length;
}
{
}
```

The key idea is a ranking argument:

- choose a value to track
- choose a well-founded ranking view for that value
- prove every recursive or cyclic step makes that ranked value strictly smaller

This is a natural fit for proof-oriented helper vocabulary. The language can
provide built-in well-founded measures for common cases such as naturals and
slice lengths. Names such as `Slice::Length` should come from the browsable core
semantic surface for slices, while libraries may later help express richer
rankings such as lexicographic tuples or domain/type-provided orders.

Working direction for the surface:

- put progress clauses under `terminates`
- keep `decreases` / `increases` as the user-facing proof words
- use `->` to select the ranking view/order

Examples:

```omega
terminates {
    decreases items -> Slice::Length;
}
```

```omega
terminates {
    decreases card -> Card::PowerOrder;
}
```

The important design boundary is that `terminates` is not an effect. It is a
proof claim over control flow.

## Automation And Boundary

The checker should automatically solve common cases:

- arithmetic normalization,
- equality reflexivity,
- range implications,
- branch facts,
- disjoint field facts,
- simple generic const facts.

When automation fails, library authors can provide helper machines. When a fact
cannot be proven from machine code, contracts, or boundary foundations, it must
cross an explicit boundary.
