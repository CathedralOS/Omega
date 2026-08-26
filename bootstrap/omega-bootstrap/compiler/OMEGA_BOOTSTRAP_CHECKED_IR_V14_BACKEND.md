# Conservative CKIR14 arithmetic backend implementation note

[`CKIR14`](OMEGA_BOOTSTRAP_CHECKED_IR_V14.md) directly inherits CKIR12 and
adds full-width `u32 in Trapping` Add, Subtract, and Multiply. The shared
checked-IR backend accepts schema major 14 without changing any accepted older
identity or output path. A relabeled CKIR12 carrier or retired CKIR13 carrier
is rejected rather than upgraded by its major word.

The backend first validates the complete CKIR14 carrier, including dense
values, visibility, exact full-width type identity, operation arity and
immediates, recursive operand custody, the major-14 exact-`u8` widening override,
resources, and any optional complete CKIR12 view/synthetic-edge relation.
Sizing and emission begin only after that validation. No error path publishes
partial ELF bytes.

## Conservative x86-64 selections

All scalar arithmetic is 32-bit unsigned arithmetic over exact frame values:

- `IntegerWiden` zero-extends the exact `u8` operand, stores it in a selected
  full-width slot, and makes that value available to a later arithmetic node;
- `Add` loads the left operand, performs 32-bit `add` with the right operand,
  branches on carry to the shared `ud2` trap, performs the ordinary unsigned
  declared-range checks, and only then stores the result;
- `Subtract` loads the left operand, performs 32-bit `sub` of the right
  operand, branches on borrow (`jb`) to the shared trap, performs the unsigned
  range checks, and only then stores the result; and
- `Multiply` zero-extends the left operand into `EAX`, performs unsigned
  32-bit `mul` with the right operand, branches to the trap when `EDX` is
  nonzero, performs the unsigned range checks on `EAX`, and only then stores
  the result.

Carry, borrow, and the nonzero unsigned high half are the exact full-width trap
predicates. Signed `jo`, signed `imul` overflow, signed comparisons, or testing
only the low product word are insufficient for this carrier. The destination
slot is not initialized or otherwise made observable before all checks for
that node succeed.

The backend emits recursive trees in validated operation order. It does not
reassociate, commute, fold, fuse, speculate, or hoist a selected node across
another potential trap. If a child traps, the target executes neither its
parent operations nor a dependent store, call, transition, or return.

## Inherited view implementation

When CKIR14 contains the optional CKIR12 view family, the backend retains the
same private 16-byte descriptor, read-only literal storage, `StaticByteView`,
`SliceNonEmpty`, guarded `SliceHead`/`SliceTailOne`, runtime empty checks, and
synthetic nonempty-edge validation. A CKIR14 carrier with no view operations
allocates or emits none of that machinery. Arithmetic does not expose a view
address or length and cannot replace a view safety check.

## Failure and evidence boundary

Schema/type/operation/control/value mutations and retired/old/new cross-pairs
reject with status 251 before artifact publication. Declared CKIR, frame,
text, or output exhaustion selects 252 before publication. A valid emitted
artifact's `ud2` is the runtime trap behavior and does not define status 251 as
a process ABI.

Backend evidence must cover zero, one, `0x7fffffff`, `0x80000000`, and
`0xffffffff`; an exact-`u8` widening consumed by arithmetic; successful and
trapping cases for each operator; mixed recursive trees; result-store ordering;
exact artifact reconstruction; native/self artifact identity; optional CKIR12
view composition; a no-view carrier; and saved CKIR12 regression. A finite
fixture is coverage of this general row relation, not a compiler-file-shaped
whitelist or a source-text permutation table.

This note defines only conservative artifact selection for CKIR14. It grants
no public scalar ABI, optimizer permission, or general arithmetic admission.
