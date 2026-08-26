# Omega bootstrap resolved-source lowering, outer version 14

[`OMGRSW5`](OMEGA_BOOTSTRAP_RESOLUTION_V5.md) |
[`CKIR13`](OMEGA_BOOTSTRAP_CHECKED_IR_V13.md)

`OMGLOWE` is the exact bounded producer relation from OMGRSW5 source custody
to CKIR13 full-width same-carrier subtraction.  Its inherited 32-byte outer
frame has magic `OMGLOWE\0`, major 14, minor and flags zero, header size 32,
checked total/component lengths, and resolution selector 5.  It contains one
exact OMGCOMP followed by its exact canonical OMGRSW5.

The source relation admits binary `-` only when the left operand is a direct
field or state-parameter leaf of canonical unconstrained `u32 in Trapping` and
the right operand is either another direct leaf of that exact type or an
anonymous `u32` literal.  Both operands are pure.  The expression may occupy
the inherited assignment, guard, call-argument, or transition context; the
inherited call boundary still permits at most one potentially trapping
argument.  Literal-left, constrained or different carriers, nested arithmetic,
and a subtraction result used as another arithmetic operand reject.

Every admitted subtraction requires OMGRSW5, irrespective of literal size or
static operand bounds.  The producer emits CKIR13 opcode 26 with the exact
full-width result type and conservatively records its result interval as all
`u32`; runtime underflow is not a lowering rejection.

This selected relation directly covers six of the current fifteen checkpoint
subtraction forms: two leaf-minus-leaf length/span relations and four
leaf-minus-literal digit/byte forms.  Nine UTF-8 leaf-minus-literal expressions
nested inside multiplication/addition remain outside it.  CKIR12 static-byte-
view composition is likewise outside this outer version; the frozen OMGLOWD
and CKIR12 carriers remain accepted by their existing consumers.

Malformed source, frame, selector, witness, type, purity, or expression shape
selects 251 without output.  Inherited table/component ceilings select 252.
Publication occurs only after complete validation and requires at least one
selected subtraction.
