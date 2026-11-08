# Chapter 10: Compile-time proofs

A proof machine states what it assumes with `requires` and what follows with
`ensures`. The checker verifies that implication for every permitted argument:

```omega
machine distinct_indices(i: u64, j: u64)
requires i < j
ensures i != j
{
}
```

The body is empty because the ordering fact already proves the conclusion.
Changing the conclusion to `i == j` would reject. An empty body is useful only
when the checker can establish the promise.

## Use a helper theorem

Break a larger argument into small facts. This helper establishes that a valid
index has a representable successor at or below the length:

```omega
machine successor_bound(index: u64, length: u64)
requires index < length
ensures index + 1 <= length
{
}

machine next_index(index: u64, length: u64) -> u64
requires index < length
ensures result <= length
{
    successor_bound(index, length);
    index + 1
}
```

The caller proves the helper's precondition and receives its conclusion.
A fact-only citation has no runtime work. For more involved proofs, helpers
provide intermediate lemmas that automation cannot find on its own.

A proof applies to its stated subjects and their current revisions. If a write
changes a field the fact depended on, establish a new fact before using it.
Failure to prove a statement is not proof that it is false.

## Choose arithmetic for the statement

Operand types select the operations in a contract just as they do in a body.
`3nat * 3nat` uses unbounded natural arithmetic; `3i32 * 3i32` uses the
selected machine arithmetic and its representability obligations.

Use `embed` when a condition needs unbounded mathematics about an integer:

```omega
requires
    embed(left) + embed(right) <= embed(integer::max<i32>())
```

This contract fragment assumes integer parameters `left` and `right`.
Embedding projects their mathematical values. Embed the operands before adding
if that is the sum you intend: embedding an already wrapped sum preserves the
wrapped answer.

All terms in a contract must be defined. Natural subtraction still needs a bound:

```omega
machine remaining(total: Nat, used: Nat) -> Nat
requires used <= total
{
    total - used
}
```

For clamping, use `Nat::saturating_sub(total, used)` explicitly. A recursive
theorem also has to prove its callee's preconditions; a smaller argument alone
does not supply them. See [total arithmetic](../spec/proofs/contracts.md#total-arithmetic).

## Distinguish equality from identity

`==` selects the same comparison in a contract and an executable expression.
The first example below assumes `Thing` has a pure comparison permitted in
proof terms. The second is a law requirement inside an algebra trait:

```omega
machine same(a: Thing, b: Thing) -> bool
requires a == b {
    a == b
}
```

```omega
machine add_comm(a: T, b: T)
ensures core::Identity<T>(add(a, b), add(b, a));
```

`same` returns true under its precondition. The algebra law instead states
mathematical identity without requiring an executable comparison algorithm.
`core::Identity` is a proof relation whose evidence permits substitution.

A comparison licenses that substitution only through a checked law that a true
comparison implies identity. The reverse law, identity implies a true comparison,
is separate. Evidence traits can supply either or both for the selected operation.

Floats show why the distinction matters:

| Relation | Signed zeros | A NaN compared with itself |
| --- | --- | --- |
| IEEE `==` | Compare equal. | Compares false. |
| `core::Identity<f64>` | Distinct bit patterns. | Identical to itself. |
| Identity of `Float::meaning64` projections | Still distinct. | Same NaN meaning; payload is forgotten. |

Use the relation the law actually needs. See [float equality](../spec/terminal-psi/mathematical_values.md#float-equality-and-source-identity)
for the full table.

## Refer to the entry value

`old(place)` names a structural place's value on entry to the machine. It lets
a postcondition relate an update to the original contents:

```omega
machine increment(value: &mut u32)
requires value < 1000
ensures value == old(value) + 1
{
    value = value + 1;
}
```

The example uses the referent through its mutable parameter. `old` is a
fact-position view; it does not copy the owned value into runtime storage.
For a separately obtained snapshot, pass an explicit `before` parameter.

Library proof views can describe a collection's sequence or bag of elements.
A sorting proof can then show both ordered output and preserved multiplicities.
Those views and their supporting theorems must be defined by the library.

## Prove termination of a cycle

A recursive proof needs a well-founded ranking. Every recursive edge proves
that the ranking decreases before it can use the recursive contract.
For a simple descending cycle:

```omega
machine countdown(n: u32) -> u32
terminates by n
ensures result == 0
{
    transition n {
        0 -> 0
        _ -> countdown(n - 1)
    }
}
```

The nonzero arm proves subtraction safe and decreases `n`. The recursive
result then supplies the postcondition. Citing the same theorem with unchanged
arguments would not establish a proof.

Proof computation may use ranked non-tail recursion. Runtime recursion is
tail-only and lowers to constant-stack iteration. [Termination](../spec/language/termination.md)
covers increasing-to-a-limit, subtree and lexicographic rankings.

## Keep evidence without runtime storage

An erased field carries a checked term without occupying runtime bytes:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

Here `Valid<T>` stands for the library's evidence type. Constructing the record
must supply that evidence. It may support proofs but cannot determine runtime
data or control flow.

Erasure and ownership are separate. An erased linear witness still has a
disposition obligation; erasing its storage does not settle it. Strict logical
proofs, by contrast, are proof-only and copyable. See [erased bindings](../spec/proofs/contracts.md#explicit-erased-bindings).

## Advanced mathematical definitions

Most program contracts use the preceding forms. Mathematical libraries also
need predicates and functions that have no executable decision procedure.
A named mathematical definition can be partially applied:

```omega
let greater_than(limit: i32, value: i32): core::Strict<0> =
    value > limit;
```

```text
expression           type of that expression
greater_than         i32 -> i32 -> core::Strict<0>
greater_than(10)      i32 -> core::Strict<0>
greater_than(10,20)   core::Strict<0>
```

`greater_than(10, 20)` denotes the proposition that 20 exceeds 10. Forming that
proposition does not prove it. `greater_than(10)` is a predicate suitable for
another mathematical definition. Ordinary machine calls still require all
their runtime arguments.

`core::Strict<0>` names a logical universe; `core::Type<u>` names a relevant
universe. The level is part of the mathematical type. Definitions are closed:
pass their inputs explicitly. [Mathematical bindings](../spec/proofs/mathematical_bindings.md)
covers levels, logical hypotheses and quantified claims.

### Existence and witnesses

A record containing `value: A` and a proof of `P(value)` gives you a particular
witness. Squashing that witness type gives only the logical fact that one
exists. You can use the fact in a strict conclusion, but cannot extract a
runtime value from it.

An accepted choice assumption may supply a mathematical witness. Producing
executable bits still needs checked computation or a realization. Review both
the theorem and the assumptions used to obtain it.

### Quotients

A quotient treats equivalent representatives as one mathematical value.
Fractions `(1, 2)` and `(2, 4)` are the usual example: addition respects their
equivalence, while returning the raw numerator does not.

For a Real construction, a schematic declaration is:

```omega
data Real = CauchySeq % ConvergesTogether
requires
    ConvergesTogether satisfies
        Equivalence<CauchySeq, ConvergesTogether>
        as CauchyEquivalence;
```

The library supplies the sequence type, relation and selected equivalence proof.
Operations additionally prove that their result respects the relation. A
runtime implementation can retain a representative without reducing it to a
canonical form; its checked laws connect that computation to the quotient.

Hashing, equality, serialization and other observers need their own appropriate
laws. The [quotient reference](../spec/proofs/quotients.md) covers those laws
and the assumptions of the selected mathematical foundation.

## When a proof fails

First check that the statement is true, that the required premises are in scope,
and that no mutation invalidated them. Then isolate a smaller helper theorem.
Automation handles common arithmetic and branch facts, but a search limit
does not decide the truth of an unresolved statement.

Keep these routes distinct:

| Route | What it establishes |
| --- | --- |
| Checked proof | A derivation under its premises. |
| Evaluation | A result of permitted terminating computation. |
| Deferral | A site-specific waiver, with a warning and no reusable fact; prohibited at package release. |
| Boundary admission | A claim the consumer explicitly trusts. |

An admission is not a proof or an automatic runtime validation. To test an
external guarantee, observe the raw result under an honest weaker contract,
then check it. Comparing against a guarantee already assumed by that same test
can let the optimizer remove the comparison.

**Ordinary programs should reject assumed bindings.** Do not use them to work
around a failed proof, borrow error or compiler limitation. Correct the program,
contract or prover. [Boundary trust](chapter_19_capabilities_effects_boundaries.md#root-assumed-binding-contracts)
explains the exceptional use and loss of assurance.

The [proof reference](../spec/proofs/contracts.md) describes proof admission,
recorded assumptions and certified elaboration. The
[foundation](../spec/proofs/foundation.md) states the calculus behind those
judgments. A checked derivation still needs to prove the statement your
application actually requires.
