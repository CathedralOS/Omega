# Ground equality inner format

[Envelope](REQUEST.md) | [Calculus and full subject](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)

This is the concrete inner encoding, not an accepted checker or certificate.
The [layout admission](LAYOUT.md) traverses physical fields and
[theory formation](FORMATION.md) checks conservative definitions.
[Ground checking](GROUND.md) validates owner/witness term tables and root sorts.
[Structural comparison](COMPARISON.md) compares validated term syntax with
session-scoped memoization. [Checked substitution](SUBSTITUTION.md) compares a
stated clause body under syntax-derived bindings. Explicit proof coordination,
the complete resource profile, and the whole-Beta certificate remain unfinished.

## Common physical representation

Each section starts with its four literal magic bytes. Every subsequent field
is a four-byte little-endian unsigned word with its high bit clear, including
tags, counts, references, and record lengths. No padding, optional fields,
names, strings, digests, or extension records occur. These administrative words
do not provide arithmetic operations or integer constants to the theory.

`record(fields...)` means a word containing the number of following payload
words, followed by those exact words. Nested records count their own length
word as part of the enclosing payload. `table` means a count followed by that
many records. The listed fields consume the exact record or section; unknown
tags, extra fields, incomplete words, and trailing bytes reject.

All row identities are implicit one-based positions. Zero is not a row
reference. Counts and explicitly zero-based argument/variable positions may
be zero. There are no producer-assigned row identities to alias or redefine.
Counts are checked against remaining physical extent before iteration or
allocation; a record payload fits only when its word count is no greater than
the remaining byte count divided by four. Advance its end only after that
subtractive check. Nested record ends cannot escape their containing record.

## Theory section: `GTH1`

The section contains, in order:

```text
magic "GTH1"
sort_count
constructor_table
function_table
exact end
```

Sort identities are `1..sort_count`; at least one is required. Constructors
and functions have separate identity spaces distinguished by application tags.
Their identity is their exact position in this owner-controlled theory, not
a textual name or a certificate claim about a name.

A constructor record contains:

```text
result_sort, argument_count, argument_sorts[argument_count]
```

A function record contains:

```text
result_sort, argument_count, argument_sorts[argument_count],
mode, selected_argument, clause_table
```

Mode 0 is the single nonrecursive variable clause; `selected_argument` must be
zero and there must be exactly one clause. Mode 1 is constructor case analysis;
`selected_argument` is a zero-based argument index strictly below the arity.
Its clauses occur in increasing constructor identity order and cover exactly
the constructors returning that argument's sort. There is no empty case over
an uninhabited sort: formation establishes finite inhabitation of every sort.
Any other mode rejects.

A clause record contains:

```text
constructor, template_table, body
```

For mode 0, `constructor` must be zero. For mode 1, it names the constructor
matched by this clause. `body` names a row in this clause's nonempty template
table. All template rows are checked, whether reachable from `body` or not.

### Clause-local variables and templates

An arity-`a` mode-0 clause binds variable slots `0..a-1` to the corresponding
arguments, with the signature's sorts. In a mode-1 clause selecting argument
`s`, slots `0..a-1` except `s` bind the other arguments; slot `s` is **unbound**.
For a constructor with `k` fields, slots `a..a+k-1` bind its immediate children
in field order, with the constructor's argument sorts. No other slot is bound.
Thus patterns are linear by construction; there is no encoded renaming map,
nested pattern, repeated binder, or way to bind the matched parent as a child.

Each template row is one of:

```text
record(0, variable_slot)
record(1, constructor, argument_count, children[argument_count])
record(2, function, argument_count, children[argument_count])
```

Template children refer strictly backward within this clause's table. They
cannot refer to a different clause, an owner ground term, or a certificate term.
Slots resolve only in this clause's binding environment. Sorts are inferred
from that environment and the theory signatures, never supplied as unchecked
row annotations. Applications have exact declared arity and argument sorts;
the body has the defining function's result sort.

Function applications in function `f` may name a function below `f`. A self-call
may name `f` only in mode 1, and its selected child must be a variable row naming
one of this clause's immediate constructor-child slots of the selected sort.
A smaller template row number is not evidence of structural decrease. Calls
above `f`, self-calls in mode 0, and reconstructed or computed selected values
reject. These checks apply to unused template rows too. The
[formation argument](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md#terms-and-conservative-definitions)
also requires declared sorts and their finite inhabitation; physical decoding
alone does not admit the theory.

## Proposition section: `GPR1`

```text
magic "GPR1"
owner_ground_term_table
left, right
exact end
```

The table contains only constructor or function application records, with the
same tags 1 and 2 and field order as template applications. Variables are
forbidden. Each child refers strictly backward in this table. The two root
references name rows in this table with the same inferred sort. Every row must
be a well-sorted ground term, even if the root does not reach it.

The artifact owner independently fixes this entire section and the theory
section. In the first complete customer, they encode the exact proposition
`encode_Beta(S, source_limit, output_limit) = Success(T)`. Raw source and tape
bytes, word values, and bounds are ordinary constructor terms under that
theory; no byte-string literal, host assembler result, or hash becomes a
trusted checker operation. Exact source/tape custody remains the artifact
owner's responsibility outside this generic format.

## Certificate section: `GCE1`

```text
magic "GCE1"
witness_ground_term_table
proof_table
exact end
```

Let `R` be the owner term count. Witness term row `i` has global term reference
`R+i`; the sum must fit the administrative word. Its application children may
name any owner term or an earlier witness term, hence lie in `1..R+i-1`.
This read-only extension preserves owner term identities and prevents the root
from depending on producer-owned rows. The kind, arity, and sort checks are
identical to owner ground terms. Sharing and duplicate structural terms are
both legal; this format is not a unique serialization of mathematical trees.

Every proof row starts with `rule, left, right`, where both term references
name well-sorted ground terms in the complete owner-plus-witness table and have
the same sort. Rule payloads are:

| Rule | Exact record payload | Required relation |
| --- | --- | --- |
| 1 Reflexivity | `1, left, right` | The two terms agree structurally. |
| 2 Symmetry | `2, left, right, premise` | The earlier premise concludes `right = left`. |
| 3 Transitivity | `3, left, right, first, second` | Earlier premises conclude `left = middle` and `middle = right`, with structurally equal middle terms. |
| 4 Congruence | `4, left, right, count, premises[count]` | Same application tag and symbol on both sides; count equals their arity; each ordered earlier premise relates the corresponding arguments. |
| 5 Unfolding | `5, left, right, clause` | Left applies a formed function; its one-based local clause matches syntactically, and its body under the derived binding equals right. |

Premises are proof-row identities strictly below the current row, not term
identities. Every supplied proof row is checked, including unused rows. The
proof table is nonempty and its **last** row must conclude the owner root in
the stated orientation. There is no producer-selected root index or unchecked
premise. Root comparison is structural, not equality of row numbers.

For unfolding, mode 0 binds all left arguments. Mode 1 first requires the
selected left argument to be an explicit constructor application of the stated
clause; its children supply that clause's child slots. A defined application
is not silently evaluated to choose a clause. Compare the claimed right term
against the template under precisely this environment, without creating a new
variable or normalizing either side. This derives the substitution from checked
syntax rather than trusting a supplied substitution or expanding an entire
intermediate tree.

## Comparison and finite execution

Structural equality compares application tags, symbol identities, and ordered
children, without interpreting definitions. Validated identical scalar term
references in the same request may shortcut that comparison. Gamma pair
addresses and hash equality never establish semantic equality. Completed
comparison memo entries must include both term identities; template comparison
also includes clause and substitution environment identity. An in-progress or
budget-interrupted comparison cannot populate a successful memo entry.

Decode with explicit cursors and table indexes. Do not repeatedly scan a linked
list for arbitrary row lookup or copy every sealed-input byte into a pair.
Comparison and substitution use bounded explicit worklists, preserving DAG
sharing; recursively expanding two separately encoded shared trees can require
exponential work. Ordinary byte-list representations of the selected source
and tape already have 46,484- and 8,355-element spines. Gamma's source syntax
and native call-depth limits must not become the logical term-depth limit.

The implementation must publish a concrete, adjustable profile before accepting
proofs: request and table extents, arity, logical depth, live worklists, memo
entries, cumulative work, cumulative pair allocation, and output. Dropping an
immutable worklist does not reclaim Gamma pairs. Charge allocations across the
whole request, including indexes, helper results, comparisons, and substitutions;
reserve enough evaluator space to publish an owned refusal. Validate counts
before resource-dependent loops, and resource availability before the operation.
An outer trap, timeout, or evaluator refusal is never a checker judgment.

The existing 8 MiB request provision is not evidence that the complete encoding
certificate fits. Measure that certificate, not just this format's small
examples; adjust private provisions or sharing as needed without weakening the
owner root or introducing trusted arithmetic. Rule retention remains conditional
on actual use and mutation controls in that complete certificate.

## Decoder and checker acceptance controls

Physical decoding must reject truncated/high-bit words, record extent escapes,
wrong tags, mismatched counts, surplus fields, and unconsumed section bytes.
Formation controls include uninhabited sorts, wrong arity/sort, cross-clause or
unbound variables, missing/duplicate cases, forward calls, and nondecreasing
self-calls, including errors in unused rows.

Term controls cover zero, forward, cyclic, and cross-owner references; variable
tags in ground tables; deep source spines; and separately encoded shared DAGs.
Proof controls cover every rule's positive and corrupt premises, unmatched
clauses, substituted environments, wrong final roots, and failures after valid
prefixes. Every published resource needs exact and adjacent controls with no
acceptance on exhaustion. These are full-checker implementation requirements,
not recorded semantic test results. The outer and layout gates establish only
their respective physical-input contracts; the formation gate establishes the
theory contract. The ground gate checks term validity and root sorts, not a
ground equality proof. The comparison gate distinguishes structural agreement
from theory equality and checks cumulative session exhaustion; it does not
discharge a supplied proof row.
The substitution gate checks one stated unfolding and its cumulative work;
it does not validate the proof table or enforce the final root.

### Hand-worked field-layout example

The following three sections describe a sort with constructors `zero`, `next`,
and the nonrecursive function `identity(x) = x`. Names are explanatory only.
Each displayed integer is one encoded word; parentheses here expand with
`record(...)` as defined above, and table counts are explicit:

```text
"GTH1", 1, 2,
  record(1, 0),
  record(1, 1, 1),
  1, record(1, 1, 1, 0, 0, 1,
       record(0, 1, record(0, 0), 1))

"GPR1", 3,
  record(1, 1, 0),
  record(1, 2, 1, 1),
  record(2, 1, 1, 2),
  3, 2

"GCE1", 0, 1,
  record(5, 3, 2, 1)
```

The clause and function payload lengths are 6 and 13 words. Section lengths
are 100, 72, and 32 bytes, making a 228-byte outer request. The intended root is
`identity(next(zero)) = next(zero)`; the one unfolding row substitutes owner
term 2 for slot 0. The formation and ground gates check this theory and its
ground terms but do not validate the unfolding row or accept the equality.
The substitution gate checks this unfolding in isolation. These component
checks cannot substitute for the full Beta encoding certificate.
