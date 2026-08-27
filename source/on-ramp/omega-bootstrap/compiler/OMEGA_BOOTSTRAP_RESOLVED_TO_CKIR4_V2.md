# OMGRSW2 resolved-source lowering to unchanged CKIR4

[`OMGRSW2`](OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
[`CKIR4`](OMEGA_BOOTSTRAP_CHECKED_IR_V4.md) |
[`OMGRFN6`](../../../refinement/delta-omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V6.md)

This contract versions the accepted source/resolution relation without
inventing CKIR5. The lowerer consumes `OMGLOW5` and emits the unchanged CKIR
schema major 4.

```text
offset  width  field
0       8      magic: ASCII "OMGLOW5\0"
8       u16    schema major: 5
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 32
16      u32    exact total frame length
20      u32    exact OMGCOMP length
24      u32    exact OMGRSW2 length
28      u32    reserved: zero
32      ...    exact OMGCOMP || exact OMGRSW2 || exact EOF
```

Component and complete-frame ceilings equal OMGLOW4. OMGLOW4 pairs only with
OMGRSW1; OMGLOW5 pairs only with OMGRSW2. Magic/major swaps, cross-pairs,
truncation, and trailing bytes reject 251 without output.

For every direct field-receiver call, source lowering emits the inherited
operations in evaluation order:

```text
SelfPlace -> FieldPlace -> explicit argument values -> Call
```

The exact role-3 row is consumed once at the machine token. The field's nominal
type must equal the callee owner, and a mutable callee requires a mutable field
place. Unit/scalar results, argument materialization, finite acyclic call graph,
private call ABI, receiver address transport, CKIR validation, result meaning,
and ELF reconstruction are unchanged CKIR4 rules.

The lowerer independently reconstructs the admitted syntax shape. OMGRSW1
accepts only direct `self.machine(...)`; OMGRSW2 additionally accepts exactly
one named field between `self` and the machine. A forged role-3 row cannot admit
a parameter, parenthesized, indexed, computed, or deeper receiver. OMGRSW2 must
contain at least one direct field call.

A single version-dispatching implementation may share decoding, checking, and
lowering between OMGLOW4 and OMGLOW5. It must preserve the exact old relation
and reject mismatched pairs. The output remains CKIR4 because opcodes 2
(`SelfPlace`), 3 (`FieldPlace`), and 10 (`Call`) already represent the complete
artifact behavior.

This producer/meaning relation does not widen OMGRFN5. Its distinct OMGRFN6
lower-rooted carrier closes responsibility-local source-to-resolution,
receiver-base source meaning, unchanged CKIR4 result, and exact ELF
reconstruction.
