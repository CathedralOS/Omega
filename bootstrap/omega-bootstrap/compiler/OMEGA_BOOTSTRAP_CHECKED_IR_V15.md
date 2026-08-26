# Omega bootstrap checked IR schema major 15

[`CKIR14`](OMEGA_BOOTSTRAP_CHECKED_IR_V14.md) |
[`CKIR12 view relation`](OMEGA_BOOTSTRAP_CHECKED_IR_V12.md)

CKIR schema major 15 is the private generalized guarded shared-byte-view
successor. It inherits CKIR14's header, table order, row widths, canonical
ordering, resources, statuses, type kinds, opcodes 1 through 27, and block flag
bit 0. It adds no type kind, opcode, row, flag, public IR, or ABI claim.

This major generalizes CKIR12's single program-static head/tail edge to more
than one guarded occurrence with ordered direct pass-through parameters. It
also permits the guarded view to originate as a runtime machine or block
parameter. The selected relation is a CFG/vector prerequisite for the product
`console_write_bytes` adapter; it does not by itself admit boundary-provider
calls, `reaches`, or termination-ranking clauses.

## 1. Identity and inherited families

The fixed header selects schema major 15, schema minor 0, and flags zero.
CKIR13 remains retired, and changing only the major of any CKIR12 or CKIR14
payload does not create CKIR15.

The exact CKIR12 shared `&[u8]` type and opcodes retain their meanings:

- opcode 22 `StaticByteView` is optional;
- opcode 23 `SliceNonEmpty` is total;
- opcode 24 `SliceHead` and opcode 25 `SliceTailOne` are partial and retain
  their nonempty precondition; and
- every kind-7 literal that is present still satisfies the CKIR12 canonical
  DAG, root, and resource relation.

CKIR14's full-width `u32 in Trapping` arithmetic is optional as a complete
family. If selected Add, Subtract, or Multiply operations occur, their exact
type, recursive custody, authored order, and first-trap relation remains
mandatory. Arithmetic neither selects nor weakens the guarded-view relation.

Every CKIR15 carrier contains opcode 23 and at least two operations each of
opcodes 24 and 25. It need not contain opcode 22 or selected arithmetic.

## 2. Selected source relation

The corresponding source lowering relation has the following parameterized
shape, where `P = (p0, ..., p[n-1])` and `0 <= a <= b <= n`:

```text
transition v.len > 0 {
    true  -> T(P[0:a], v[0], P[a:b], v[1..], P[b:n])
    false -> F(P)
}
```

The selected relation requires:

- `v` is one direct exact shared-`&[u8]` machine or state parameter;
- every `p` is one direct in-scope machine or state parameter binder;
- pass-through binders are pairwise distinct and distinct from `v`;
- removing the one head and one tail expression from the true vector yields
  the exact false vector, with binder identity and order unchanged;
- `v[0]` occurs exactly once and precedes exact `v[1..]`, which also occurs
  exactly once;
- `n <= 5`, preserving the inherited seven-parameter target ceiling; and
- the complete carrier has at least two selected occurrences and at least one
  pass-through position across them.

Scalar or copyable structural parameter types already legal on an inherited
edge may pass through. Another shared immutable view may pass through. A
computed expression, call, mutation, index, trapping expression, mutable view,
duplicate binder, substitution, omission, or reorder is outside this cut.
Other computations in the predecessor remain governed by inherited relations;
they do not become selected pass-through arguments.

## 3. Generalized synthetic nonempty edges

CKIR15 retains block flag bit 0 as `SYNTHETIC_NONEMPTY_EDGE`. At least two
blocks carry it. Each selected source occurrence owns one distinct synthetic
block, allocated after its machine's authored blocks in predecessor/source
order. For every such block `S`:

1. Its parameters are exactly `(v, P...)`, with one to six parameters total,
   leading exact shared `&[u8]`, and exact inherited types thereafter.
2. Its unique predecessor is target 0, the true edge, of one same-owner
   authored `Branch`.
3. The branch condition is the result of `SliceNonEmpty(v)`, and the true
   incoming arguments are exactly `(v, P...)`.
4. Target 1, the false edge, directly targets an authored block with exactly
   `P`; it does not enter any synthetic block.
5. `S` contains exactly two operations in order: `SliceHead(parameter 0)` and
   then `SliceTailOne(parameter 0)`.
6. `S` ends in one `Jump` to a same-owner authored block. Its argument vector
   contains each pass-through parameter once and in order, with the head once
   before the tail once at their authored positions.
7. No synthetic block targets another synthetic block or has another
   predecessor.

Every opcode-24 and opcode-25 operation in the module belongs to exactly one of
these validated synthetic blocks. Additional independently valid total
`SliceNonEmpty` operations may occur outside them.

The false edge therefore executes no partial operation. On a true edge, the
view and pass-through values are copied into `S` once, head and tail are
computed once, and the complete authored vector is copied to the target.
Recurrent execution consumes the newly passed tail; pass-through parameters
are never recomputed.

## 4. Resources, status, and exclusions

CKIR14 ceilings remain normative. Each selected occurrence consumes one
synthetic block, between one and six block parameters, two operations, their
two operand words and values, and one complete predecessor/target edge vector.
Declared extent or component exhaustion selects status 252.

Malformed identity, direct-binder custody, owner, predecessor, condition,
view identity, vector identity/order, target type/arity, synthetic operation,
partial-operation ownership, or cross-version relation selects status 251.
Failure publishes no result, CKIR, ELF, or partial artifact.

This contract does not admit effectful or computed pass-through expressions,
mutable-view operations, dynamic indexing, `u64` collection arithmetic,
provider/boundary calls, general allocation, pointer identity, public slice
representation, `reaches`, or a termination-ranking clause.

## 5. Independent evidence and remaining joins

- `../gates/checked_ir_v15_reference.py` selects CKIR15 explicitly over the
  shared independent decoder.
- `../gates/delta-checked-ir-v15-fixture.py` emits deterministic recurrent,
  one-byte, and empty carriers with two synthetic edges and ordered values
  before, between, and after head/tail.
- `../gates/delta-checked-ir-v15-reference.sh` checks deterministic bytes,
  all three runtime paths, responsibility-local malformed relations, and
  resource status 252.

Producer/lowerer, conservative backend, Rust-free meaning, persisted
lower-rooted reconstruction, and exact product-source admission remain separate
milestones and are not implied by this reference contract.
