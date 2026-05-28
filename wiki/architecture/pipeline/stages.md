# Current Pipeline Stages

This document is deliberately normalized. Each stage answers the same questions
against the semantic spine: places, values, facts, loans, moves, drops, calls,
transitions, effects, and boundary edges.

## Source Files To Tokens

Input: loaded source files.

Output: token streams.

Primary responsibility: preserve source identity and split text into tokens.

Semantic nouns:

- Places: not known.
- Values: not known.
- Facts: not known.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: not known.
- Transitions: not known.
- Effects: not known.
- Boundary edges: not known, except `boundary` as token text.

Must not own: language meaning, import semantics, symbol resolution.

## Tokens To Syntax Trees

Input: token streams.

Output: `SyntaxTrees`.

Primary responsibility: parse source structure without resolving meaning.

Semantic nouns:

- Places: syntactic expressions that may later become places.
- Values: literal/expression syntax only.
- Facts: parsed proof facts and contract clauses.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: syntactic call expressions/statements.
- Transitions: syntactic transition statements and targets.
- Effects: effect clauses as names.
- Boundary edges: parsed `boundary` traits, operators, capability contracts,
  library entries, and target policies.

Must not own: symbol identity, type identity, borrow validity, proof discharge.

## Syntax Trees To Symbol Resolved Trees

Input: `SyntaxTrees`.

Output: `SymbolResolvedTrees`.

Primary responsibility: attach symbol identity to definitions and references.

Semantic nouns:

- Places: names and members begin to resolve to symbols, but place validity is
  not proven.
- Values: expression producers gain resolved names.
- Facts: proof facts can refer to resolved domains, symbols, and members.
- Loans: not known.
- Moves: not known.
- Drops: not known.
- Calls: call targets become symbol-facing.
- Transitions: target states become symbol-facing.
- Effects: effect names become symbol-facing.
- Boundary edges: boundary declarations point at resolved constructs, but
  provider validity is not fully modeled here.

Must not own: type checking, flow invalidation, borrow overlap, backend shape.

## Symbol Resolved Trees To Typed Trees

Input: `SymbolResolvedTrees`.

Output: `TypedTrees`.

Primary responsibility: attach type and signature meaning.

Semantic nouns:

- Places: type-aware member/index candidates.
- Values: typed expression results.
- Facts: typed facts and constraints.
- Loans: not known yet, except through mutable/reference type surfaces.
- Moves: not yet durable events.
- Drops: type information can imply future drop requirements, but scheduling is
  deferred.
- Calls: typed call signatures and argument/return expectations.
- Transitions: typed transition arguments and return/exit expectations.
- Effects: typed effect declarations and call surfaces.
- Boundary edges: typed boundary contracts and operator signatures.

Must not own: final proof discharge, liveness, move/drop scheduling, ABI layout.

## Typed Trees To Checked Trees

Input: `TypedTrees`.

Output: `CheckedTrees`.

Primary responsibility: validate semantic obligations and build checked facts.

Semantic nouns:

- Places: first strongly useful place layer via `omega_facts::Place` and
  checked-flow `CanonicalPlace`.
- Values: still weaker than desired; expressions and symbols stand in for value
  instances.
- Facts: first-class fact contexts, origins, payloads, proof obligations, and
  contract facts.
- Loans: first-class borrow facts, accesses, loans, activations, weakenings, and
  overlap checks.
- Moves: should become first-class here; currently too implicit.
- Drops: should become first-class here; currently too implicit.
- Calls: first-class call facts for contracts, borrows, flow, and effects.
- Transitions: checked for proof/arguments, but ownership transfer needs more
  explicit data.
- Effects: direct/transitive effect plans are available.
- Boundary edges: represented through boundary contracts/operators/policies, but
  should become explicit checked-flow events.

Must not own: machine instruction shape, ABI placement, final storage layout.

## Checked Trees To State Graph

Input: `CheckedTrees`.

Output: `StateGraph`.

Primary responsibility: make machine/state transitions explicit for scheduling,
proof, and later control-flow lowering.

Semantic nouns:

- Places: should be carried only when graph edges need state/data identity.
- Values: transition arguments and state payloads become graph data.
- Facts: relevant checked facts should be attachable to states/edges.
- Loans: should preserve enough information to avoid illegal graph rewrites.
- Moves: should be explicit if a transition consumes a value.
- Drops: should be explicit if a transition exits a lifetime region.
- Calls: state/helper calls become graph actions or edge computations.
- Transitions: first-class graph edges.
- Effects: should be accumulated per node/edge where relevant.
- Boundary edges: should stay visible when graph actions cross host/compiler
  boundaries.

Must not own: proof invention, parser recovery, target instruction lowering.

## State Graph To Control Flow

Input: `StateGraph`.

Output: `ControlFlow`.

Primary responsibility: lower state-machine structure into explicit blocks,
branches, calls, exits, and data flow.

Semantic nouns:

- Places: become control-flow-accessible storage/value references.
- Values: become explicit data-flow operands or temporaries.
- Facts: should be preserved as annotations or diagnostics support where needed.
- Loans: should have already been validated; any remaining data is for
  correctness-preserving lowering.
- Moves: should become control-flow events before backend lowering.
- Drops: should become scheduled control-flow cleanup.
- Calls: explicit control-flow operations.
- Transitions: lowered into branches, calls, exits, and block edges.
- Effects: attached to operations/blocks for later reporting and validation.
- Boundary edges: attached to operations that lower to imported/compiler/runtime
  code.

Must not own: semantic proof discharge or target register assignment.

## Backend Lowering Stages

Input: `ControlFlow`.

Output: abstract operations, target operations, assigned target operations,
machine instructions, object/image data.

Primary responsibility: lower checked program meaning to executable bytes.

Semantic nouns:

- Places: lower to storage homes, offsets, registers, stack slots, globals, or
  ABI locations.
- Values: lower to operands, virtual registers, physical registers, immediates,
  memory values, or relocatable symbols.
- Facts: mostly diagnostic/proven metadata; should not be reinvented as backend
  proof.
- Loans: should not be rechecked here except as assertions/invariants on prior
  lowering.
- Moves: become copies, transfers, invalidated homes, or elided operations.
- Drops: become cleanup calls, deallocations, or no-ops.
- Calls: become calling-convention sequences.
- Transitions: become jumps, branches, calls, returns, or process exits.
- Effects: become host calls, allocation calls, syscalls, traps, or metadata.
- Boundary edges: become compiler/runtime/host lowering sites with ABI and
  image/linkage consequences.

Must not own: language-level acceptance of unsafe behavior. Backend can implement
a boundary, but checked/orchestration layers should say which boundary was
accepted.
