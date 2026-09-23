# Backend vocabulary rejection audit

Status: audit record. Measured at `6ff9db37b9` in
`omega-rust/omega/pipeline/target-operations-to-selected-instructions`.
BACKEND-VOCABULARY-REJECTION-AUDIT asks whether every admitted operation
family "either selects on each applicable ISA or returns a structured refusal
through the public compiler", with "exact operation/target diagnostics without
panics or silent omission".

## Admission ordering: three routes, not two

`AbstractOperation` has **83** families. They do not divide into
admitted/rejected — `legalization/scalar_graph_input/nodes.rs::validate` does
`block.nodes.split_last()` first, so the terminator never reaches `admit` at
all:

| route | count | where |
| --- | --- | --- |
| body-admitted | 63 | `admit` / `scalar_instruction` yield an operation identity |
| terminator | 7 | split off by `validate`, checked by `control::validate` |
| named rejection | 13 | `scalar_instruction`'s catch-all -> `LegalizationError::UnsupportedScalarOperation` |

Terminator families: `Conditional`, `Crash`, `Jump`, `Return`,
`ReturnStructural`, `ReturnUnit`, `StructuralCase`.

Named-rejection families: `AtomicEvent`, `CallDynamicScalar`,
`CallDynamicUnit`, `CallStoredDynamicScalar`,
`CallStructuralScalarWithDynamicArguments`, `CallUnitWithDynamicArguments`,
`EstablishTrivialAffineLocal`, `MoveStructuralField`,
`NearestIeeeFloatFusedMultiplyAdd`, `PortWrite`, `SaturatingIntegerMultiply`,
`StoreDynamicDescriptor`, `StoreStructuralField`.

Reading the 20 non-body families as one "rejected" bucket is the mistake this
split exists to prevent: 7 of them are ordinary control flow that legalizes
through a different door.

## The finding: selection refusals cannot name an operation or a target

Legalization's refusal retains identity —
`UnsupportedScalarOperation { machine, operation }`, and its `Display` prints
both. Selection's does not. **No variant of `SelectedInstructionError` carries
an operation, an `OperationId`, or a target**; its refusals are
`UnsupportedSourceShape { function }`, `AmbiguousSourceShape { function, .. }`,
`MissingConstraint(RegisterConstraintKey)`,
`TargetRegisterArchitectureMismatch`, and projection/canonicality variants
keyed by function or register index.

### The first clause holds structurally, not by testing

`selection/construction/scalar_graph.rs`'s per-instruction dispatch matches
`&operation.kind` with **no wildcard arm**. A match on an enum without one
must be exhaustive to compile, so the compiler already guarantees every
`LegalizedScalarInstructionKind` has a selection arm. An admitted family
cannot fall off the end of selection; that is enforced at build time, not
merely untested.

This narrows the audit's question sharply. Selection's `Err` returns are not
family-level "no rule for this operation" refusals — they are operand-shape
and constraint conditions INSIDE those exhaustive arms (`result.ok_or_else`,
a branch suffix that must follow a comparison, a missing register constraint
row). The failure is positional, which is why the diagnostic is positional.

### What remains: attribution

The refusals therefore carry no operation or target identity, and both are in
scope where they are raised: `LegalizedScalarInstruction` carries
`operation: OperationId`, and `LegalizedScalarFunction` carries
`machine: MachineId`. Attributing a selection failure to a capability owner
today means reading the legalized function back by function index.

Scale: `UnsupportedSourceShape` appears at only 12 sites, all construction, so
a variant carrying `{ function, machine, operation }` is a bounded change
rather than a refactor.

This is a diagnostic-identity gap, not a missing rejection. It is engineering,
not language design: the refusal vocabulary is an implementation choice with
no spec sentence to settle.

## Attribution

`NearestIeeeFloatFusedMultiplyAdd` classifies as a named rejection here while
its target-unit ingest arm exists elsewhere
(`control_flow/sources.rs`, `unit/ieee_float.rs`) — the scalar graph admits
only settled target-unit forms, not abstract FMA. That family belongs to
**X86-FMA-PROVIDER-TRANSPORT**, as the row directs; nothing in this audit
expands the accepted vocabulary to cover it.

The other twelve named rejections are dynamic-dispatch, atomics, port and
structural-move families whose implementations belong to their existing
capability owners. This audit does not open work on them.

## What is pinned, and what is not

`admission_vocabulary_tests.rs` gains an exhaustive `admission_route` match
with no wildcard arm, so adding a family to `AbstractOperation` stops that
file compiling until someone classifies it. That closes "silent omission" for
admission ordering.

Not pinned by test: the per-ISA selection outcome for each of the 63 admitted
families. Establishing that by construction needs a legalized function per
family per ISA, which is the "generic audit framework" the row forbids
building — and the exhaustiveness result above makes it largely redundant,
since the dispatch cannot omit a kind.

The tractable remaining step is the attribution one: give
`SelectedInstructionError` a variant carrying `{ function, machine,
operation }` and raise it at the per-instruction refusal sites, where both
identities are already in scope. That closes "exact operation/target
diagnostics" without expanding the accepted vocabulary.
