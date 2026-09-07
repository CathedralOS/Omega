# Sorted ground terms and owner root

[Inner format](FORMAT.md) | [Theory formation](FORMATION.md) | [Calculus](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)

`check_derivation_ground()` extends theory formation with sorted owner and
witness term tables and a well-sorted owner root. It does not compare the root
terms for equality, check proof rows, unfold definitions, or authenticate the
theory/proposition. Its `Grounded` result is not proof acceptance.

## Admission and check order

Call `form_derivation_theory()` first and forward any failure unchanged. Only
tag 4 permits the following work. Physical layout has already checked every
word and record in all three sections, including the still-unchecked proof rows.

Build separate immutable row-offset indexes for the owner and witness tables,
using the same balanced representation as formation. Table count fields are at
`theory_end + 4` and `proposition_end + 4`, respectively. These offsets come from
the retained formed frame, never from certificate-provided section pointers.
Then validate in this order:

1. Every owner term, in row order, including unused terms.
2. The owner root's left reference, then its right reference, then their sorts.
3. Every witness term, in row order, including unused terms.

For each application, check the symbol reference in the tag-selected constructor
or function index before its exact arity, then check children in argument order.
An invalid symbol rejects at its symbol field; wrong arity rejects at its
argument-count field. For each child, check the reference before its inferred
sort; either failure belongs to that child-reference field. Variables and
unknown tags are already rejected by physical layout. All formed functions may
occur in ground terms: definition-order and structural-decrease restrictions
apply to definitions, not to closed applications of admitted functions.

Let `R` be the owner count and `V` the witness count. Owner row `i` may reference
only `1..i-1`. Witness row `j` has global identity `R+j` and may reference only
`1..R+j-1`, routing references `1..R` to the immutable owner index and the rest
to witness identity `reference-R`. Zero, self, forward, and cyclic references
reject. Witness identities cannot redefine owner rows or supply a missing owner
child. Both tables share the same formed symbol signatures, but neither can
refer to clause-local template rows as a separate identity space.

Each previously checked child's result sort comes directly from its checked
symbol signature; no recursive expansion or unchecked sort annotation is used.
Duplicate structural rows and repeated children are legal. This stage does not
equate distinct row identities or normalize function applications.

The two root fields are at `proposition_end-8` and `proposition_end-4`. Each must
reference `1..R`, even if a witness would have the requested identity. An invalid
root reference rejects at its own field. A root-sort mismatch rejects at the
right field, after both references pass. An empty owner table therefore rejects
at the left root field. Distinct, same-sorted roots are allowed here: the later
derivation must justify their equality.

All new semantic failures are tag 1, code 8 `ground_terms`, the specified
request-byte coordinate, and zero limit/requested. Earlier physical, theory,
and resource failures retain their original fields. No partial ground context
escapes on failure.

## Work, allocation, and depth

There is no additional arbitrary term-count or logical-depth cutoff in this
stage. The admitted 8 MiB physical request already bounds `N=R+V` below 2^19:
each ground record has at least four words including its length. Thus the global
identity sum fits the administrative u31 word. A physical index is built once;
row and child scans are tail calls. Index descent has at most 19 edges, and
symbol lookup at most the formation index bound of 21. Logical term depth does
not consume native recursion or allocate an expanded tree.

Writing `H` for the total number of ground child-reference fields, validation
visits `N` rows and `H` children, with bounded index lookups at each visit, plus
constant root checks. Index building visits linearly many tree nodes. There is
no linked-list lookup, repeated full-table search, per-child result allocation,
or semantic evaluation. The request extent bounds both `N` and `H` before any
such loop or allocation. These are structural work bounds, not host timeouts.

The two indexes allocate at most `3N+4` pairs, including temporary build
carriers; one shared checking context uses two pairs, and success or rejection
uses at most four. Thus this stage adds at most `3N+10` cumulative pairs. Together
with formation's actual ledger, the request uses at most
`2W+3N+32S+26`, where `W` and `S` retain their formation meanings. This is below
7,864,346 pairs under the independent admitted bounds, within the selected Gamma
arena of 40,265,318. Unreachable pairs still count. The
[combined checking ledger](CHECKING.md#complete-generic-execution-provision)
includes later comparison, substitution, and proof stages.
The provisions remain adjustable implementation choices, not calculus laws.
The [comparison session](COMPARISON.md#work-and-allocation-provision) adds its
completed-pair memo and pending frames to this cumulative allocation ledger.

## Private grounded outcome

Success is `(pair 5 payload)`, with payload
`(pair formed (pair owners (pair witnesses proof_table)))`. `formed` is the
existing formed payload; the two indexes retain separate owner/witness custody.
`proof_table` is the physical proof-count field immediately after the witness
rows, not an index or a claim that those proofs are valid. The payload accessors
are `grounded_theory`, `grounded_owner_terms`, `grounded_witness_terms`, and
`grounded_proof_table`. `grounded_term_lookup(payload, identity)` routes global
term identities through the two retained indexes; invalid identities return
scalar zero before descent.

The diagnostic entry publishes tag 5 followed by eight little-endian u64 words:
the three frame ends, `R`, `V`, left root, right root, and proof-table offset.
Failures publish tag 1/2 and their four existing u64 fields. Process status zero
only means an owned diagnostic was published. No production proof-accepting
`main` is introduced.
