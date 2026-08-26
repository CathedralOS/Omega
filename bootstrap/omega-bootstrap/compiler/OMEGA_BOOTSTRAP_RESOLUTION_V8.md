# Omega-bootstrap normalized resolution handoff, schema major 8

[`OMGRSW7`](OMEGA_BOOTSTRAP_RESOLUTION_V7.md) |
[`OMGLOWH`](OMEGA_BOOTSTRAP_LOWERING_V17.md)

`OMGRSW8` is the source-family resolution successor for one direct, pure,
same-carrier full-width `u64 < u64` relation. Except where this contract
overrides it, the inherited OMGCOMP1 source-custody, table, ordering, status,
resource, and exact-EOF rules remain normative. OMGRSW8 adds no expression
table: authored body spans remain the lowerer's expression custody.

This is a bridge-private handoff. It is not a public Omega ABI, checked IR,
proof language, or compilation authority.

## 1. Identity and least selection

The exact identity is magic `OMGRSW8\0`, schema major 8, schema minor zero,
flags zero, and the inherited 84-byte header. Table order, row widths,
ceilings, checked offsets, and the 524,288-byte witness ceiling are unchanged.

The shared resolver selects OMGRSW8 only when an admitted body contains an
exact direct `<` whose operands are:

- two direct named parameters or `self` fields with normalized kind 10; or
- one such typed operand and one decimal literal contextualized by that peer.

The complete relation is unqualified `u64`, pure, and same carrier. Two
literals provide no carrier. Mixed carriers, `<=`, a call, indexing, member or
postfix continuation, arithmetic around either operand, chaining, equality
around the relation, or a merely embedded parenthesized prefix do not select
OMGRSW8. Comments and quoted bytes do not select it. Expression boundaries
are checked as tokens, so an admitted prefix cannot relabel a larger excluded
expression.

The resolver publishes the least identity required by the complete source.
A `u64` declaration plus an unrelated inherited `u32 < u32` does not
manufacture OMGRSW8. In this source-family cut, `u64` and a decimal token
requiring a nonzero upper 32-bit word are admitted only as part of the selected
OMGRSW8 relation; without it they select status 251 rather than silently
truncating or relabeling a predecessor. Sources needing only an inherited
relation retain their byte-exact predecessor identity.

## 2. Normalized u64 custody

Kind 10 denotes unqualified `u64`. Its flags and reserved word are zero. The
four words in the inherited 24-byte normalized type row are reinterpreted as:

```text
offset  meaning
8       inclusive lower endpoint, low 32 bits
12      inclusive lower endpoint, high 32 bits
16      inclusive upper endpoint, low 32 bits
20      inclusive upper endpoint, high 32 bits
```

Each word preserves all 32 semantic bits even when held in Delta's signed
storage. Endpoint pairs use unsigned 64-bit lexicographic order. The canonical
full carrier is `(0, 0, 0xffffffff, 0xffffffff)`. An authored
`u64 [lo..=hi]` produces and references its exact canonical constrained row;
range checking and interning include all four words. `u64 in Trapping` and
every other policy qualification are excluded.

This interpretation applies only to kind 10. Inherited kind-2 `u32` rows,
structural IDs, counts, offsets, ordinals, spans, and `NO_ID` keep their prior
meanings. Every legacy integer consumer requires the token's upper word to be
zero; a wide array length or legacy scalar literal cannot be accepted through
low-word truncation.

## 3. Source custody and failure

The witness binds every typed selected operand, field, machine parameter,
block parameter, call signature, and edge parameter to its exact normalized
type. Decimal literals remain contextual body syntax and create no witness
row. Resolution does not evaluate the comparison, publish an operation, or
serialize a control-flow fact.

When the direct relation is used as a transition guard, its true edge may
carry the strict upper-bound fact for the direct left subject. The lowerer,
not OMGRSW8, derives and joins that fact. The false edge carries no such fact.

Malformed source, identity, type, endpoint, canonical ordering, source/witness
pairing, or excluded syntax selects 251 without output. Declared resource
exhaustion selects 252 without output. Decimal overflow above
`18446744073709551615` is malformed. Publication begins only after complete
canonical resolution and exact EOF.

OMGRSW8 does not add u64 arithmetic, equality, casts, coercions, trapping
policy, dependent types, user operators, allocation, or a public integer ABI.
