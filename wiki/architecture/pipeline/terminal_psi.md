# Terminal Psi Architecture

[Pipeline](pipeline.md)

Status: target architecture settled 2026-08-02. This document records the
implementation cut and migration from the current bootstrap pipeline. The
semantic and evidence contract is owned by
[`canonical_ir_fuel_and_resource_provisioning.md`](../../design_briefs/canonical_ir_fuel_and_resource_provisioning.md).

Implementation status (2026-08-02): `compiler/psi-rs` is the Psi-owned
workspace root. `psi-core` provides nonzero stable semantic identities, the
typed scalar proposition vocabulary, and a module-owned value-typing context.
`psi-proof-kernel` provides total primitive judgments, structural proof
checking (including semantic-axiom citation and typed equality transitivity),
versioned certificate envelopes, and exact profile-authorized admission
validation. Admission cannot replace a primitive derivation, and architecture
tests reject any Psi dependency on Omega.

The first in-memory executable slice is also live. `psi-terminal` defines a
versioned module with stable machines, blocks, values, operations, edges, and
bodyful contracts; its closed vocabulary currently contains representable
integer constants plus unconditional jump and return edges.
`psi-terminal-verifier` rejects malformed identities, types, contract scopes,
cycles, unreachable fact sources, and missing/extra evidence, reconstructs the
exact operation/edge/return axioms, and checks every `ensures` from a separate
proof bundle. `omega-interpreter` executes only a `VerifiedTerminalModule` on
this path.

The first transitional source producer is now live as
`omega-checked-trees-to-terminal-psi`. It accepts one exact free-machine slice:
typed integer constants, one unconditional literal-carrying state jump, one
literal return, and a matching closed `requires`/`ensures` pair. It rejects all
other checked-tree shapes. The source canary discards `CheckedTrees` before it
verifies and executes the produced semantic module, proving the artifact has no
frontend lifetime dependency. This is explicitly a migration adapter, not the
target ownership direction: parsing and checking still need to move under Psi.
The current legacy exit prover also cannot establish an ordinary
`result == literal` contract, so the bootstrap canary carries the closed typed
fact `7i32 == 7i32` and asserts the executed result separately. An Omega
source-independent consumer is also live:
`omega-terminal-psi-to-abstract-operations` accepts only a
`VerifiedTerminalModule` and produces an owned stream of integer
materialization, jump binding, and return requirements with stable Psi
provenance. Neither it nor `omega-terminal-abstract-operations` depends on
checked/typed trees, `ExpressionHandle`, or the legacy source-shaped abstract
operation plan.

The first target/native realization is live on the same clean lane.
`omega-terminal-abstract-operations-to-target-operations` resolves the verified
constant and jump bindings into a target `ReturnIntegerImmediate` while
retaining every contributing Psi operation and edge identity.
`omega-terminal-machine-emission` emits ordinary scalar-return code for
AArch64 and x86-64 and rejects non-native integer widths. The real-source canary
links only those emitted entry bytes into a minimal host harness and proves its
process result equals terminal interpretation after all producer and
intermediate state is dropped. This checkpoint does not claim standalone
object/image emission, general register assignment, or migration of the legacy
backend. Canonical serialization/fingerprints, branching, arithmetic-policy
operations, and fuel remain next.

## Boundary

Psi operates on Omega-branded source files and owns every target-neutral stage
through one canonical terminal representation. Omega consumes terminal Psi; it
does not feed source-shaped data back into Psi.

```text
Psi
    source files
    -> tokens -> syntax -> resolved -> typed -> checked
    -> lowered expressions, predicates, places, blocks, and edges
    -> terminal Psi

Omega
    terminal Psi
    -> abstract operations -> target operations
    -> assigned instructions -> bytes -> installed image
```

Parsing therefore belongs to Psi. “Omega files” is the language and product
branding; Psi is the frontend, semantic verifier input, and portable execution
representation.

## Why no existing stage is the cut

The current implementation has no expression-lowering pass before instruction
selection:

- `CheckedTrees` embeds `TypedTrees` plus checked fact tables;
- `StateGraphCode` copies the typed expression table, and operations and
  transitions retain `ExpressionHandle`;
- `ControlFlowCode` clones the same expression table and mostly remaps the
  graph topology and semantic arenas; and
- abstract-operation construction and instruction selection still inspect and
  substitute tree expressions directly.

`StateGraph` and `ControlFlowPlan` are therefore useful topology and evidence
scaffolds, not self-contained executable representations. Conversely,
`AbstractOperations` already owns runtime storage regions, calling-convention
classes, ABI aggregate distinctions, and other Omega realization concerns.
Removing those fields would not reveal a hidden portable IR.

The missing pass is the boundary: merge the useful state-graph/control-flow
shape and fill it with lowered semantic content. This is not serialization of
today's `StateGraph`, purification of `AbstractOperations`, or a second similar
block IR placed beside `ControlFlowPlan`.

## Terminal requirements

Terminal Psi is immutable and self-contained. It contains no arena handle that
requires `TypedTrees`, source syntax, the producer compiler, or instruction
selection to interpret its meaning. It contains:

- concrete machines and instantiated types;
- explicit typed blocks, block parameters, values, calls, transitions,
  continuations, and terminals;
- lowered predicates over the same stable value/place identities as execution;
- typed structural places, including ordinary and provider-backed roots plus
  field, dynamic-index, dereference, and range/subextent projection;
- explicit cleanup, transfer, conservation, invalidation, suspension, and
  boundary actions on edges;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

Author-declared hardware geometry is semantic and may contain offsets, widths,
and alignment. Omega begins where the target chooses native layout, stack and
register placement, ABI classes, concrete storage regions, instructions, and
relocations.

## Psi operation definition

Every operation enters the vocabulary as one reviewed vertical slice:

```text
operation identity and canonical encoding
execution transition
generated obligations and authorized admissions
proof rule / logical interpretation
soundness proof of that rule against the transition
interpreter realization
Omega lowering requirement
fuel identity
```

Operations are statically distinct when execution semantics or generated
obligations differ. Obligation-affecting policy is a closed instruction variant,
not an ordinary value that requires constant folding before verification.
Additional sound proof lemmas may be published without changing operation or
program identity.

The proof kernel, proposition representation, total primitive judgments,
certificate envelope, and admission taxonomy land before an operation depends
on them. Concrete proposition and operation vocabularies are then co-designed
in vertical slices; the proof language is not speculated in isolation.

## Verification boundary

The verifier derives structural obligations from terminal Psi and checks the
fingerprinted author contracts. Every accepted fact is:

- re-decided by a specified total kernel judgment;
- discharged by a checked certificate, carried or reconstructed by a total
  certifying procedure; or
- admitted at a sealed site and accepted by the consuming profile.

Admission cannot replace a derivable obligation. Search that may time out or
return unknown must carry its certificate for portable verification. Primitive
trusted judgments are minimized and each joins the enumerable language
soundness audit.

The semantic module, proof bundle, installation record, and debug/source maps
remain separate. Proof improvements do not change semantic identity; provider
selection and attached evidence do change their own section and container
identities. One execution verifies and runs one complete Psi semantic version.

## Migration plan

1. Continue the established workspace boundary: move or rename the current
   target-neutral parsing-through-lowering crates under Psi ownership while
   retaining temporary compatibility adapters. No parser or semantic checker
   remains on an Omega-to-Psi path.
2. Extend the live stable Psi value, proposition, proof, and place identities
   into the first terminal semantic module without changing the current backend.
   **Initial scalar subset complete:** in-memory constant/jump/return module,
   verifier, and direct interpreter. Structural places remain a later slice.
3. Lower the live integer/control/contract slice from the transitional checked
   frontend into terminal Psi, add its Omega abstract-operation consumer, and
   compare interpreted/native behavior before broadening the vocabulary.
   **Initial vertical slice complete through native comparison:** the
   fail-closed compatibility adapter and a real source canary now verify and
   execute after checked trees are dropped, then lower the verified module into
   an owned, source-independent Omega requirement stream, a target
   return-immediate, and host machine code whose execution matches
   interpretation. Standalone image integration is not part of this checkpoint.
4. Add calls, continuations, cleanup, conservation, boundary operations,
   suspension, and scoped ordering as reviewed vertical slices.
5. Move binding substitution and concrete instantiation above terminal Psi so
   no Omega pass consumes source expressions.
6. Re-root the reference interpreter, rebuilding differential-oracle evidence
   during the transition.
7. Re-root abstract-operation construction on terminal Psi, then retire the
   redundant state-graph/control-flow representation and adapters.
8. Freeze canonical serialization, fingerprints, proof/install section
   manifests, and version migration only after the in-memory vocabulary has
   passed interpreter and lowering canaries.

The migration may keep old and new paths temporarily for comparison. That is a
testing bridge, not a permanent two-semantics architecture.
