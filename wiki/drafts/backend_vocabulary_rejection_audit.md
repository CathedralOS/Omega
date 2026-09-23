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

So an admitted family that selection cannot realize on some ISA does return a
structured refusal — the audit's first clause holds, and nothing panics — but
the diagnostic names a function index or a constraint key. It does not name
the operation or the target, which is the audit's second clause. Attributing
such a failure to a capability owner currently requires reading the legalized
function back by index.

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

Not pinned: the per-ISA selection outcome for each of the 63 admitted
families. Establishing that by construction needs a legalized function per
family per ISA, which is the "generic audit framework" the row forbids
building. The tractable next step is instead to give
`SelectedInstructionError` an operation/target-carrying variant so a
selection refusal attributes itself, and then to let the existing corpus
supply the coverage.
