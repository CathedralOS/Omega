# OMGCOMP lower-rooted refinement witness, version 5

This contract versions the private refinement carrier for the focused `CKIR4`
runtime named-record-construction tranche. It inherits the complete `OMGCOMP`
and `OMGRSW1` schemas, ordering, source custody, role-3 bindings, and ceilings
from
[`OMGCOMP_REFINEMENT_WITNESS.md`](OMGCOMP_REFINEMENT_WITNESS.md). Neither
component changes. The normative source, checked-IR, status, and artifact
meaning is
[`OMEGA_BOOTSTRAP_CHECKED_IR_V4.md`](../../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V4.md);
this document only fixes the lower-rooted evidence transport and assigns its
already-defined propositions to independent owners.

## Version-5 refinement frame

Lower-rooted CKIR4 refinement uses the distinct little-endian carrier
`OMGRFN5`:

```text
offset  width  field
0       8      magic: ASCII "OMGRFN5\0"
8       u32    version: 5
12      u32    flags: 0 library, 1 entry-bearing compilation
16      u32    exact OMGCOMP byte length
20      u32    exact OMGRSW1 byte length
24      u32    exact CKIR4 byte length
28      u32    exact ELF byte length
32      u32    claimed full result
36      u32    claimed exit projection
40      ...    OMGCOMP || OMGRSW1 || CKIR4 || ELF || exact EOF
```

The inherited library/entry result rules remain in force. Component ceilings
are 267,280 OMGCOMP bytes, 524,288 OMGRSW1 bytes, 2,522,192 CKIR4 bytes, and
1,183,744 ELF bytes, so the simultaneous framed maximum remains:

```text
40 + 267,280 + 524,288 + 2,522,192 + 1,183,744 = 4,497,544 bytes
```

Checked addition precedes every offset. A validated component extent above its
ceiling is 252; malformed identity, version, fields, component relation,
arithmetic, or EOF is 251. Rejection publishes nothing. `OMGRFN5` accepts only
CKIR schema major 4, minor 0. `OMGRFN1` through `OMGRFN4` remain frozen and
reject this carrier. No field offset, constructor-object offset, frame address,
or producer-derived layout appears in the carrier.

The carrier is bridge-private evidence transport. It is not an Omega ABI,
compiler authority, accepted-lock receipt, or digest commitment.

## Independent responsibility split

Five logical persisted-Beta responsibilities consume the same exact `OMGRFN5`
bytes. A responsibility may use multiple bounded executables; this split fixes
proposition ownership, not executable count. No responsibility imports a
producer conclusion or another responsibility's process-local state.

1. **Frame and source custody.** Validate the complete frame and independently
   reconstruct complete `OMGCOMP` structure, source extents, tokens, and root
   custody.
2. **Resolution.** Independently reconstruct source-to-`OMGRSW1` resolution,
   including every inherited role-3 row. Declaration, field, machine,
   parameter, block, call, binding, selected-root/result-type, and witness-
   extent facts are derived from the bounded source and carrier rather than
   fixed to one fixture census. Runtime record construction adds no resolver
   row and cannot be justified by a producer-supplied field binding.
3. **Declarations and intrinsic CKIR structure.** Join `OMGRSW1` to CKIR4
   declarations, nominal identity, copyability, recursive layout, types,
   selected entry, dense tables, and intrinsic constant DAG. This owner checks
   the table envelope and the declaration facts needed to interpret opcode 13;
   it does not claim that source bodies lower to those operations, compute a
   result, assign native frame extents, or reconstruct ELF bytes.
4. **Source lowering and source meaning.** Reconstruct resolved bodies to exact
   CKIR4 operations, operands, and terminators. For runtime constructors this
   includes exact nominal-name selection, complete field-name coverage,
   declaration-order canonicalization, the pure/non-trapping leaf grammar,
   nested construction, scalar interval containment, structural exactness,
   constructor-to-Call/Copy flow, the direct state-edge exclusion, and the
   semantic-before-four/five resource precedence. A companion physically
   artifact-free evaluator reconstructs constructor snapshots, synchronous
   parameter transport, destination copying, and the claimed source result
   without CKIR or ELF access.
5. **CKIR meaning and artifact reconstruction.** Independently validate the
   complete CKIR4 and claimed result. Reconstruct every constructor value as a
   completed immutable object, derive its distinct aligned frame extent between
   ordinary slots and shared scratch, enforce its lifetime/use restrictions,
   evaluate nested construction and structural transport, and rederive frame
   and live-stack bounds. Independently reconstruct every opcode-13 instruction
   byte, inherited call ABI byte, constant image, segment, displacement, and
   complete ELF byte.

The composition gate invokes every persisted executable over each selected
immutable exact carrier and retains phase-local opacity, mutation, resource,
and valid-but-mismatched cross-pair controls. The source-only executable is
physically pruned of CKIR, ELF, and artifact-evaluator access. The CKIR/ELF
executables do not read source bodies beyond the explicit frame/table premises
owned by their responsibilities. This is a conjunction of independently
reconstructed claims, not one verifier divided into named functions.

## Required boundary evidence

Before this carrier can close the tranche, the focused responsibilities and
same-frame composition must cover both exact `source.omg` carriers: the
original same-module runtime-record opener, and a same-module harness composing
the complete current `SourceUnit` API through `clear`, `append`, and
`byte_or_nul`. The latter intentionally varies call count, block-parameter
count, selected-root result type, binding count, and witness extent so a
fixture census cannot stand in for reconstruction. Both must yield exact result
70 through every responsibility. Renamed, count-varied, and authored-reordered
controls; empty and one-through-four field records; nested records; every
admitted scalar and structural leaf; constructor-to-Call and constructor-to-
Copy must also pass. Isolated negatives must cover malformed field coverage/
type/copyability, every excluded field expression, call-order meaning,
source/witness, witness/CKIR4, CKIR4/ELF, and result cross-pairs, direct
constructor-result state-edge use, opcode-13 row/use mutations, and object/
frame/instruction mutations.

Exact/adjacent evidence includes the four/five field tooth, operation/value/
operand aggregates, empty anchors, nested alignment, selected frame, complete
live stack, text, CKIR, ELF, 64/65 active evaluator frames, and 65,536/65,537
dynamic block entries. The five-field result is 252 only after complete source
semantics are valid; malformed five-field forms remain 251. Each executable
reports its procedure, local, tape, and build/run-time use rather than hiding a
bounded-resource failure in a fixture product.

## Non-expansion and authority separation

These clauses only version private refinement transport and assign already-
normative CKIR4 propositions. They do not admit another source form; change
`OMGLOW4`, `OMGRSW1`, CKIR4, ELF, result, or status meaning; decide effectful or
trapping field evaluation order; grant compilation authority; or decide final
`Ωself` retention. Compilation authority still requires the separately
accepted lock/closure and exact `OMGCOMP` commitment join.
