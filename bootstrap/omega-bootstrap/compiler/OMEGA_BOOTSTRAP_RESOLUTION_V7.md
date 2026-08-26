# Omega-bootstrap normalized resolution handoff, schema major 7

[`OMGRSW4`](OMEGA_BOOTSTRAP_RESOLUTION_V4.md) |
[`OMGLOWF`](OMEGA_BOOTSTRAP_LOWERING_V15.md)

`OMGRSW7` is the source-family resolution successor for the recursive
full-width `u32 in Trapping` arithmetic milestone. Except where this contract
overrides it, every OMGRSW4 source-custody, resolution, ordering, normalized-
type, status, resource, and publication rule remains normative. OMGRSW7 adds
no expression, operator, or body-token table; exact body spans remain the
lowerer's source of expression custody.

This is a bridge-private handoff. It is not a public Omega ABI, a stable wire
format, an arithmetic proof, checked IR, or compilation authority.

## 1. Identity and least selection

The magic is `OMGRSW7\0`, schema major is 7, schema minor is 0, flags are zero,
and the header size is the inherited 84 bytes. The inherited table order, row
widths, ceilings, checked-offset rules, and exact EOF remain unchanged. The
complete witness remains at most 524,288 bytes.

A canonical OMGRSW7 closure contains at least one body expression admitted by
the OMGLOWF recursive arithmetic relation and the unique normalized exact
`u32 in Trapping` type described below. The shared resolver publishes the
least identity required by the complete exact source closure:

- sources needing the recursive full-width arithmetic relation select
  OMGRSW7;
- sources needing only an OMGRSW1, OMGRSW2, OMGRSW3, or OMGRSW4 relation keep
  that byte-exact least identity; and
- OMGRSW6 is an exact OMGCOMP2 compatibility-cost profile, not a candidate in
  this OMGCOMP1 source-family ordering.

An operator-shaped byte inside a comment or quoted literal, unary `-`, `->`,
an excluded expression, or a full-width decimal token outside an admitted
arithmetic expression does not select OMGRSW7. Changing only magic, major,
OMGCOMP version, or a selector never creates another canonical witness.

The OMGRSW5 identity is retired. OMGRSW7 does not reuse its magic, major,
bytes, or former meaning, and no decoder may treat 5 as an alias for 7.

## 2. Full-width normalized scalar custody

The inherited 24-byte normalized scalar row is interpreted positionally. The
unique exact full-width `u32 in Trapping` row has:

- kind 2 (`u32`);
- the inherited exact `Trapping` policy flag and no other flag;
- payload words zero; and
- inclusive low and high semantic words `0x00000000` and `0xffffffff`.

Range endpoints in scalar type rows are unsigned 32-bit semantic words. A
Delta implementation may hold their bits in signed storage, but comparison,
canonicalization, encoding, and reconstruction preserve all 32 bits.
Structural IDs, counts, offsets, ordinals, spans, and `NO_ID` remain structural
words with the inherited bounds; this rule does not turn `0xffffffff` into a
valid structural ID where `NO_ID` is not allowed.

Every direct selected arithmetic leaf that names a field, named state, machine
parameter, or block parameter resolves to this same interned row. The one
inherited exact-widening form may instead name an exact unqualified `u8` leaf;
the witness binds that leaf to its existing `0..=255` row and binds the authored
`u32 in Trapping` target to the selected full-width row. Anonymous decimal
literals are contextual body syntax and add no witness row; their typed meaning
and `0..=4294967295` bound belong to OMGLOWF. The resolver does not evaluate
arithmetic, assign precedence, infer a trap, or publish an operation identity.

## 3. Inherited source custody

OMGRSW7 retains OMGRSW4's exact OMGCOMP1 pairing, units, imports, bindings,
declarations, types, records, sums, machines, parameters, blocks, body spans,
shared-byte-view identity, and bounded plain-literal custody. A selected
closure may contain the inherited exact shared `&[u8]` relation; arithmetic
selection neither requires that relation nor erases it.

The complete witness must bind every named arithmetic leaf to its existing
declaration/parameter identity and exact normalized type. Calls, indexing,
mutation, construction, dispatch, and expression trees remain absent from the
witness even when their surrounding body bytes are retained for later
rejection. No resolver row may be invented merely to make a body admissible.

Malformed identity, framing, source, resolution, type, endpoint, canonical
ordering, or version relations select status 251 without output. A declared
carrier or resource limit selects 252 without output. Publication begins only
after the complete canonical witness and exact EOF have been established.

## 4. Non-expansion

OMGRSW7 does not admit mixed carriers, inferred coercions, user-defined
arithmetic, wrapping or saturating policy, dependent arithmetic, general
constant evaluation, allocation, runtime view construction, or a public
integer ABI. These belong to later source, lowering, or product relations.
