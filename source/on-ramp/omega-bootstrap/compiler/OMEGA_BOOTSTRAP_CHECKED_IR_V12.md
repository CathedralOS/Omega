# Omega bootstrap checked IR schema major 12

[`CKIR11`](OMEGA_BOOTSTRAP_CHECKED_IR_V11.md) |
[`CKIR3 constant DAG`](OMEGA_BOOTSTRAP_CHECKED_IR_V3.md)

CKIR schema major 12 is the private successor for one bounded shared static-
byte-view relation. It inherits CKIR11's header, table order, row widths,
canonical ordering, resources, statuses, and opcodes 1 through 21. It adds
type kind 7, operation opcodes 22 through 25, and block flag bit 0. Earlier
CKIR identities and bytes remain frozen.

This contract establishes an independently checkable carrier and the control-
flow shape needed to lower a nonempty slice match without speculatively
executing its partial operations. It does not select surface syntax, a source
resolver identity, a Delta lowering frame, a mutable slice, general pointer
arithmetic, or an observable allocation/address identity.

## 1. Exact shared byte-slice type

Type kind 7 denotes exactly shared `&[u8]`. Its inherited 24-byte type row is:

- kind 7;
- flags and reserved zero;
- payload 0 naming the unique canonical full-range `u8` type
  `(kind 1, flags 0, payloads 0, range 0..255)`; and
- payload 1, low, and high zero.

The type is copyable. Its representation remains private, with size 16 and
alignment 8. No CKIR consumer may infer a public pointer/length ABI or compare
the address identity of two views from that private layout.

## 2. Static literal roots

CKIR12 reuses the CKIR3 canonical constant DAG without changing either
constant table. A kind-7 literal node has scalar/reserved zero and between 0
and 32 children. Every child is an earlier scalar node of the exact payload-0
full-range `u8` type. The inherited dense IDs, complete child partition,
backward edges, height-first canonical ordering, interning, reachability, and
table ceilings remain normative.

Only opcode 22 may designate a kind-7 node as a constant root. In particular,
inherited opcode 11 `CopyConstant` continues to accept only record and array
roots; a kind-7 literal cannot be smuggled through an aggregate root. Multiple
opcode-22 rows may designate the same interned literal. Whether identical
literals share backing storage is unobservable.

A literal with 33 children selects resource status 252. Malformed child types,
forward/noncanonical edges, scalar payload drift, unreachable nodes, or a
kind-7 node not rooted by opcode 22 select status 251.

## 3. Operations 22 through 25

All four operations use the inherited 40-byte operation row, have zero flags,
produce the next dense value ID, and require visible operands where listed:

| Opcode | Name | Operands | Immediate 0 | Immediate 1 | Result |
| --- | --- | --- | --- | --- | --- |
| 22 | `StaticByteView` | none | exact kind-7 literal root | zero | that root's exact slice type |
| 23 | `SliceNonEmpty` | one slice | zero | zero | canonical Boolean |
| 24 | `SliceHead` | one slice | zero | zero | the slice's exact full-range `u8` payload type |
| 25 | `SliceTailOne` | one slice | zero | zero | the same exact slice type |

`SliceNonEmpty` is total and is false exactly for an empty view. `SliceHead`
and `SliceTailOne` require a nonempty runtime view and trap without publishing
a result otherwise. On a nonempty view, `SliceHead` returns its first byte and
`SliceTailOne` returns the view after removing exactly that byte. No operation
exposes or compares address identity.

Every CKIR12 carrier contains at least one operation of each opcode 22 through
25. This milestone does not widen inherited load, store, `CopyConstant`, or
arithmetic operations to accept a slice.

## 4. Synthetic nonempty edge

CKIR12 assigns block flag bit 0 as `SYNTHETIC_NONEMPTY_EDGE`; all other block-
flag bits remain zero. Exactly one block in the carrier has this flag. It has
exactly one slice parameter at ordinal 0 and satisfies all of the following:

- it has one predecessor and that predecessor is the true edge (target 0) of
  an inherited `Branch`;
- the branch subject is an opcode-23 `SliceNonEmpty` result;
- opcode 23 consumes the exact slice value passed as synthetic parameter 0;
- the false edge bypasses the synthetic block;
- predecessor, synthetic block, and targets remain in the same inherited
  machine owner;
- the synthetic block contains only opcode 24 and opcode 25, includes both,
  and every such operation consumes parameter 0; and
- it ends in one inherited `Jump` to a non-synthetic authored block and has no
  other predecessor.

This shape is the authority that makes head/tail execution safe. Reversing the
edge, substituting a different condition or slice, adding a predecessor,
moving either partial operation outside the flagged block, introducing another
operation in it, or jumping to another synthetic block selects status 251.
Future producers may carry additional pure pass-through parameters only after
their identity across both edges and the authored target is unambiguous; the
minimal CKIR12 carrier intentionally has only parameter 0.

## 5. Execution, resources, and status

The independent interpreter models a view as an opaque private descriptor over
the validated literal DAG. Its focused one-byte carrier contains byte 70. The
true nonempty edge observes head 70, observes that `SliceTailOne` is empty, and
returns 70. A second carrier uses an empty literal, takes the false bypass, does
not execute either partial operation, and also returns 70. The selected `Fp`
entry therefore returns 70 in both controls.

CKIR11 ceilings remain normative. The existing 8,192 constant-node and 16,384
constant-child ceilings still select 252, as does the new per-literal 32-byte
ceiling. Schema, type, constant, value/visibility, operation, synthetic-edge,
and version failures select 251. Neither failure status publishes a result.

## 6. Independent evidence

- `../gates/checked_ir_v12_reference.py` selects CKIR12 explicitly over the
  inherited decoder and independently interprets the private slice carrier.
- `../gates/delta-checked-ir-v12-fixture.py` emits deterministic one-byte and
  empty carriers plus isolated schema, type, constant, operation, control-flow,
  value, and resource mutations.
- `../gates/delta-checked-ir-v12-reference.sh` checks both runtime paths,
  deterministic bytes, rejection without stdout publication, and status 252
  for a 33-child literal.

Producer/lowerer, backend, persisted-meaning, and same-frame refinement
admission are separate milestones and are not implied by this reference
contract.
