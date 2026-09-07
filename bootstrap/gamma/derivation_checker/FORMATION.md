# Conservative theory formation

[Inner format](FORMAT.md) | [Layout](LAYOUT.md) | [Soundness argument](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md#terms-and-conservative-definitions)

`form_derivation_theory()` checks the owner-supplied constructor and function
definitions after physical layout admission. It does not validate the ground
term tables, proof rows, or root equality. A formed theory is not an accepted
proof, nor evidence that the definitions faithfully formalize Beta.

## Admission and private provision

First call `admit_derivation_layout()` and forward any failure unchanged.
Only its tag 3 permits reading theory fields without repeating physical checks.
The sort count `S` at request offset 28 must be positive. Zero selects rejection
code 7 at that field. Counts above 65,536 select resource code 2 `theory_sorts`,
with coordinate 28, limit 65,536, and the actual requested count.

Before allocating indexes, census the constructor and function tables. Let `C`
and `F` be their counts, `A` the sum of constructor arities, and `W` the theory
payload word count after its four magic bytes. Apply this conservative work
estimate:

```text
E = (S + 1) * (C + A) + F * C + 4 * W + S
```

An estimate above 8,388,608 selects resource code 3 `formation_work_bound`,
coordinate 28, limit 8,388,608, and the exact estimate `E`. Equality proceeds.
This check precedes semantic signature errors. It bounds named row/field
processing visits: inhabitation passes, constructor-catalog scans for case
definitions, ordinary indexing/validation passes, and a final sort scan. A
visit may include bounded indexed lookup; `E` is not a count of Gamma or Alpha
instructions, elapsed time, or every internal tree-descent iteration.

The admitted request bounds `W`, `C`, `F`, and `A` below 2^21; the separate sort
check bounds `S` by 2^16. The entire estimate fits signed 64-bit arithmetic,
including requests that exceed the provision. No wrapping estimate or truncated
diagnostic is permitted. These provisions are adjustable implementation choices,
not restrictions on conservative definitions. The whole-Beta certificate still
has to demonstrate suitable end-to-end provisions before proof acceptance.

## Deterministic semantic checks

After the preflight, build immutable indexes for the constructor and function
records. Check all constructor signatures in declaration order, followed by
all function signatures in declaration order. Within a signature, check the
result sort and then argument sorts in field order. Every sort reference must
lie in `1..S`; failure selects its own field coordinate. Signature checking
precedes inhabitation and all clause/body checking.

### Finite constructor inhabitants

Start with no inhabited sorts. In constructor order, mark a constructor's result
sort when all its argument sorts are marked; nullary constructors mark their
sort immediately. A mark is visible to later constructors in the same pass.
Repeat while a pass adds marks and not all sorts are marked. There are at most
`S` successful marks and `S+1` passes. If a pass adds none while a sort is still
unmarked, reject at the sort-count field, offset 28. Recursive constructors
alone cannot establish a finite inhabitant. No equality axiom is introduced.

### Clauses and template scopes

Process functions in declaration order. For mode 0, require selected argument
zero, then exactly one clause, then constructor zero in that clause. Failures
belong to the selected-argument, clause-count, and clause-constructor fields,
respectively. For mode 1, require the selected index below the function arity;
failure belongs to the selected-argument field. Scan the constructor
catalog once in identity order, consuming one clause for every constructor
returning the selected argument sort. Validate that clause before continuing
the catalog scan. Missing clauses reject at the clause-count field; wrong
constructor identities reject at the current clause's constructor field.
After the catalog ends, any extra clause rejects at its constructor field.
Thus an earlier clause error precedes a later missing or extra case.

Each clause requires a nonempty template table, indexed within that clause
only. Reject an empty table at its count field. Visit every template row in
table order, including rows unused by the body. Variable slots follow FORMAT's
implicit environment: other arguments retain their zero-based slots, the
selected parent is unbound, and constructor children occupy the slots starting
at the function arity. An unbound slot rejects at its slot field.

For an application, check the symbol reference first, then the function-order
restriction, then exact signature arity, then children in argument order.
An arity mismatch rejects at the application's argument-count field.
Each child must be a positive reference below the current local template row;
check its inferred sort against that argument's declared sort. A bad reference
or argument sort rejects at that child-reference field. Sorts of already checked
rows come from their bound slot or referenced symbol signature, not an unchecked
annotation or recursively expanded term. Duplicate structural rows are allowed.

A function reference above the current function rejects at its symbol field.
A direct self-call also rejects there in mode 0. In mode 1, after all arguments
pass, require the selected child to be a variable row naming an unchanged
immediate constructor-child slot. Its sort has already been checked against
the selected parameter sort. A computed expression, reconstructed parent,
unselected parameter, or matched-parent slot cannot witness decrease; reject
at the self-call's selected child-reference field. A smaller row identity alone
does not prove termination.

Finally, the body reference must name a row in this local nonempty table and
have the function result sort. Reject either defect at the body-reference
field. No later function, other clause, ground term, or certificate row can
supply a variable binding, premise, or template reference.

All semantic failures use tag 1, code 7 `theory_formation`, their specified
request coordinate, and zero limit/requested. No partial formed context escapes
on failure. Resource outcomes use existing tag 2 with the fields above.

## Indexed representation and containment

A row index is `(pair count tree)`. Its nonempty tree is balanced by splitting
the known count into `floor(count/2)` left rows and the remaining right rows.
A leaf is its scalar record-start offset; an internal node is a pair of child
trees. Count zero has scalar tree zero. Lookup validates the scalar row identity
before descending; Gamma pair references are never tested as integers.

Build from the sequential sealed records once; a builder returns a tree and
the next cursor. A nonempty `n`-row build uses `3n-2` pairs plus its one index
pair; an empty index uses two. Lookup allocates none and has at most 21 tree
edges under the admitted request bound. Template indexes are clause-local and
may become unreachable after checking, but their allocations still count.

Sort marks use a sparse immutable balanced tree over `1..S`: shared
`Empty=(pair 0 0)`, shared `True=(pair 1 0)`, and occupied internal nodes
`(pair 1 (pair left right))`. Inspect scalar tags and the known tree range before
projecting children; never compare a pair with scalar zero. Each sort changes
from absent to present at most once. An update copies at most 16 internal nodes,
two pairs each, and reuses the shared true leaf. Bounded native recursion,
not allocated rebuild frames, carries the update path. The two shared constants
belong to the fixed allocation allowance.

The implementation's cumulative allocation must stay below
`8W + 32S + 128` pairs, including indexes, clause contexts, pass results, and
terminal outcomes. This is below 18,874,496 pairs for the admitted extents,
within the selected Gamma arena of 40,265,318. Scalar field validation must not
allocate a result tuple per visit. Balanced-tree recursion follows logarithmic
index height; row, argument, constructor, and pass scans are tail calls.
The complete proof checker must account for additional stages separately.

For the cumulative ledger, let `T` be all template rows, `K` all clauses, and
`M` the number of newly marked sorts. Index construction uses at most
`3(C+F+T)+4` pairs: only the two global indexes can be empty, since an empty
template table rejects before building. Function/clause contexts use
`2F+2K`; pass carriers use at most `M+1 <= C+1`; mark paths use at most
`32M <= 32S`. Shared mark constants, the retained layout/context, and a terminal
outcome add eleven pairs. Thus the actual bound is
`4C+5F+3T+2K+32S+16`. The wire has at least
`W >= 3C+6F+3T+4K`, so this is at most `2W+32S+16`, below the allowance above.
These counts include allocations that become unreachable: Gamma does not
reclaim them during this evaluation.

## Private formed outcome

Success is `(pair 4 payload)`, where payload is
`(pair frame (pair S (pair constructors functions)))`. `frame` retains the exact
three section ends from layout admission, and the two indexes retain all
constructor/function record starts. The payload accessors are `formed_frame`,
`formed_sort_count`, `formed_constructors`, and `formed_functions`;
`formation_index_count` reads an index count. `formation_index_lookup` returns
scalar zero for an invalid identity before descending.
This custody is private to the selected checker source, not a producer-supplied
claim or an artifact authentication mechanism.

The diagnostic entry publishes tag 4 followed by the three section ends and
`S,C,F`, each as an eight-byte little-endian word. Failures publish their tag
and the four existing failure fields as eight-byte words, preserving large work
estimates. Process status zero means only that this owned diagnostic was
published. There is no production proof-accepting `main`.
