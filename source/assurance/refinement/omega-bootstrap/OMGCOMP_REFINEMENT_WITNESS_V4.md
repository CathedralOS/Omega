# OMGCOMP lower-rooted refinement witness, version 4

This contract versions the private refinement frame for the focused `CKIR3`
constant-aggregate tranche. It inherits the complete `OMGCOMP` and `OMGRSW1`
row schemas, ordering, source custody, and ceilings from
[`OMGCOMP_REFINEMENT_WITNESS.md`](OMGCOMP_REFINEMENT_WITNESS.md), including the
exact role-3 call bindings added for `CKIR2`. `OMGCOMP` and `OMGRSW1` do not
change. The outer frame changes because the claimed checked IR, constant-image,
cyclic-result, and artifact relations change.

The normative source, checked-IR, status, and artifact meaning is defined by
[`OMEGA_BOOTSTRAP_CHECKED_IR_V3.md`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V3.md).
This document only fixes its lower-rooted evidence transport and assigns those
already-defined propositions to independent evidence owners.

## Version-4 refinement frame

Lower-rooted CKIR3 refinement uses the distinct private carrier `OMGRFN4`; no
earlier refinement frame is widened or relabeled. Its little-endian header is:

```text
offset  width  field
0       8      magic: ASCII "OMGRFN4\0"
8       u32    version: 4
12      u32    flags: 0 library, 1 entry-bearing compilation
16      u32    exact OMGCOMP byte length
20      u32    exact OMGRSW1 byte length
24      u32    exact CKIR3 byte length
28      u32    exact ELF byte length
32      u32    claimed full result
36      u32    claimed exit projection
40      ...    OMGCOMP || OMGRSW1 || CKIR3 || ELF || exact EOF
```

The inherited library/entry result rules remain in force. Component ceilings
are 267,280 OMGCOMP bytes, 524,288 OMGRSW1 bytes, 2,522,192 CKIR3 bytes, and
1,183,744 ELF bytes. Therefore the exact simultaneous framed maximum is:

```text
40 + 267,280 + 524,288 + 2,522,192 + 1,183,744 = 4,497,544 bytes
```

Checked addition precedes every offset. Excess is 252; malformed identity,
version, fields, component relations, arithmetic, or EOF is 251. Rejection
publishes nothing. `OMGRFN4` accepts only CKIR schema major 3, minor 0.
`OMGRFN1`, `OMGRFN2`, and `OMGRFN3` remain frozen and reject this carrier.

The frame is bridge-private evidence transport. It is not an Omega ABI,
compiler authority, accepted-lock receipt, or digest commitment.

## Independent responsibility split

Five logical persisted-Beta responsibilities consume the same exact `OMGRFN4`
bytes. A responsibility may use more than one bounded executable; this split
fixes proposition ownership, not executable count. No responsibility imports a
producer conclusion or another responsibility's process-local state.

1. **Frame and source custody.** Validate the complete frame and independently
   reconstruct complete `OMGCOMP` structure, source extents, tokens, and root
   custody.
2. **Resolution.** Independently reconstruct source-to-`OMGRSW1` resolution,
   including every inherited exact role-3 row.
3. **Declarations and intrinsic CKIR structure.** Join `OMGRSW1` to CKIR3
   declarations, layout, types, selected entry-machine root, and table
   structure. For the constant-node and child-vector tables this responsibility
   owns exact counts, offsets, framing, IDs, spans, scalar/type/arity validity,
   child back-edges, DAG shape, height/key ordering, duplicate rejection, and
   compatibility with independently reconstructed type/layout tables. It does
   not own source-body-to-constant correspondence, opcode-11 root selection,
   whole-graph reachability from opcode-11 roots, result execution, constant
   image construction, or ELF bytes.
4. **Source lowering and source meaning.** Reconstruct resolved source bodies
   to exact CKIR3 operations, operands, and terminators, including source
   aggregate checking, interning and canonical constant-graph construction,
   opcode-11 root references, opcode-12 `LessEqual`, guardless `Jump`, and
   cyclic interval-flow lowering. This responsibility joins the source-derived
   graph and roots to the claimed CKIR3. A companion physically artifact-free
   evaluator independently reconstructs source meaning and the claimed result.
5. **CKIR meaning and artifact reconstruction.** Independently validate the
   complete CKIR3 and claimed result. This includes operation-root validity and
   the requirement that every constant node and child-vector word is
   transitively reachable from an opcode-11 structural root of the exact
   destination type. Independently derive constant-root image objects, layout,
   zero padding, segment count and extents, instruction references, reachable
   call closure, ABI, displacements, stack bounds, and every ELF byte.

The composition gate joins all five responsibilities over one unchanged
carrier and retains phase-local opacity, mutation, resource, and
valid-but-mismatched cross-pair controls. The split is a conjunction of
independent claims, not one verifier divided into named functions.

## Non-expansion and authority separation

These clauses only version private refinement transport and assign already-
normative CKIR3 propositions to independent evidence owners. They do not admit
a new source form; change `OMGLOW3`, `OMGRSW1`, CKIR3, ELF, result, or status
semantics; grant compilation authority; or decide any final `Ωself` feature.
Compilation authority still requires the separately accepted lock/closure and
exact `OMGCOMP` commitment join.
