# Pipeline Architecture

Omega's compiler pipeline should be a sequence of durable representation
boundaries, not a pile of ad hoc helper structs.

The same semantic nouns should be recognizable across stages, but their data
shape changes as they become more resolved. Source-shaped IR can only say "this
syntax looks like a place." Checked IR can say "this place overlaps this loan."
Backend IR can say "this place is stack slot plus offset."

## Normalized Stage Questions

Every stage should answer:

- Input representation.
- Output representation.
- Primary responsibility.
- Places.
- Values.
- Facts.
- Loans.
- Moves.
- Drops.
- Calls.
- Transitions.
- Effects.
- Boundary edges.
- What this stage must not own.
- Known gaps.

## Semantic Spine

### Places

A place is a location-like expression that can be read, written, borrowed,
moved, or invalidated.

Examples: `self.health`, `items[index]`, `room.exits[north]`.

Current status: fairly strong. `omega_facts::Place`, `PlaceRoot`,
`PlaceSegment`, and checked-flow `CanonicalPlace` already exist.

Desired direction: every stage that mutates, borrows, invalidates, or lowers a
location should use a stage-specific place projection instead of reconstructing
paths from raw expressions.

### Values

A value is a produced runtime or compile-time object with type, initialization,
ownership, and storage/lowering consequences.

Current status: weak as a first-class ownership concept. Values mostly appear as
expressions, symbols, typed nodes, or backend operands.

Desired direction: checked/lowered IR should distinguish expression syntax from
value instances so moves, copies, drops, and storage can be reasoned about
directly.

### Facts

A fact is a proven or accepted assertion at a program point.

Current status: strong. `omega_facts::Fact`, `FactContext`, `FactOrigin`,
`FactPayload`, and `ProgramPoint` are real infrastructure.

Desired direction: facts should remain the proof-facing currency for domains,
bounds, invariants, contracts, and boundary guarantees.

### Loans

A loan records that a place or view is borrowed over a span of program points.

Current status: strong in checked trees. `BorrowLoanFact`, loan activation, loan
weakening, and overlap checks exist.

Desired direction: loans should attach to place projections and be invalidated
by move/drop/write/call/transition events through one flow model.

### Moves

A move transfers ownership of a value or place and may make the source unusable.

Current status: weak. Move behavior is checked in places, but there is no
durable `MoveFact` or `MoveEvent` equivalent.

Desired direction: moves should become explicit checked-flow events with source
place/value, destination, program point, and invalidated facts/loans.

### Drops

A drop ends a value's lifetime and runs cleanup if required.

Current status: weak. Drops are a language concern but not yet first-class in the
ownership flow IR.

Desired direction: drops should become explicit scheduled events before backend
lowering, with proof-visible cleanup obligations and effect/boundary metadata.

### Calls

A call invokes a machine, state, operator, helper, or imported boundary entry.

Current status: strong. Semantic call facts, borrow call facts, contract call
facts, flow call facts, and effect call facts all exist.

Desired direction: calls should converge on a shared call-site identity so
borrow, proof, effects, transitions, and backend lowering do not maintain
parallel ordinals by accident.

### Transitions

A transition transfers control and arguments between states or exits.

Current status: medium. Transitions are first-class in syntax, state graph, and
control flow, but ownership transfer across transitions should be as explicit as
call transfer.

Desired direction: transition facts should record argument flow, carried facts,
invalidated facts, moved values, and scheduled drops.

### Effects

An effect records externally visible capability behavior such as allocation, IO,
process exit, or host interaction.

Current status: strong. `omega-effects::EffectPlan`, `EffectSet`, direct
effects, transitive effects, and effect paths exist.

Desired direction: effects should be attached consistently to calls,
transitions, drops, allocations, and boundary edges.

### Boundary Edges

A boundary edge is where Omega stops proving from Omega source and accepts a
declared contract from compiler/runtime/host/toolchain code.

Current status: medium. Boundary syntax, contracts, operators, target policies,
and reports exist. Boundary edges are not yet one unified checked-flow entity.

Desired direction: checked flow should expose boundary crossings as explicit
events with provider, contract, effects, accepted facts, and unchecked-policy
status.

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

Known gaps: none; this stage should stay intentionally boring.

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

Known gaps: parser diagnostics and chapter examples should stay synchronized as
syntax shifts.

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

Known gaps: keep root/operator/domain symbol handling first-class and avoid
string identity leaking into later phases.

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

Known gaps: value identity should start becoming more explicit here so checked
trees are not forced to reverse-engineer it from expressions.

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

Known gaps: add durable value, move, drop, and boundary-edge events to checked
flow.

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

Known gaps: transition ownership transfer should be as explicit as call
ownership transfer.

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

Known gaps: control-flow should not erase move/drop/boundary events before the
backend can lower them.

## Control Flow To Abstract Operations

Input: `ControlFlow`.

Output: target-independent abstract operations.

Primary responsibility: lower checked control flow into explicit operations with
virtual registers and target-independent storage/value actions.

Semantic nouns:

- Places: lower toward abstract storage references.
- Values: become abstract operands, temporaries, constants, or virtual registers.
- Facts: mostly diagnostic/proven metadata.
- Loans: should be already validated; may remain as assertions.
- Moves: become explicit abstract copies/transfers or no-ops.
- Drops: become abstract cleanup/deallocation calls or no-ops.
- Calls: become abstract call operations.
- Transitions: become branches, jumps, returns, exits, and block edges.
- Effects: attach to operations.
- Boundary edges: become abstract runtime/host/compiler calls.

Must not own: target register assignment or machine instruction selection.

Known gaps: currently some runtime lowering decisions are still too tangled with
later backend stages.

## Abstract Operations To Target Operations

Input: abstract operations.

Output: target-aware operations.

Primary responsibility: legalize operations using target, layout, ABI, ISA, and
calling-convention knowledge.

Semantic nouns:

- Places: lower to target-aware memory/register shapes.
- Values: become target-legal operands.
- Facts: should not be re-proved here.
- Loans: should not be rechecked here.
- Moves: become legal target copies, loads, stores, or elisions.
- Drops: become target-callable cleanup sequences.
- Calls: become target-aware call sequences.
- Transitions: become target-aware branch/jump/return operations.
- Effects: map to target/runtime operations.
- Boundary edges: map to ABI-aware host/runtime/compiler operation shapes.

Must not own: language acceptance of unsafe behavior.

Known gaps: this stage needs clean separation between legalization and physical
assignment.

## Target Operations To Assigned Target Operations

Input: target-aware operations.

Output: assigned target operations.

Primary responsibility: decide physical registers, stack slots, spill homes, and
calling-convention homes.

Semantic nouns:

- Places: become concrete homes or memory locations.
- Values: become assigned registers, stack slots, immediates, or symbols.
- Facts: diagnostic metadata only.
- Loans: prior-stage invariant only.
- Moves: become assigned copies or spills.
- Drops: become assigned cleanup operations.
- Calls: receive physical ABI placement.
- Transitions: receive concrete branch/linkage operands where possible.
- Effects: remain operation metadata.
- Boundary edges: receive physical ABI placement.

Must not own: object encoding or final bytes.

Known gaps: register allocation and stack assignment should stay here, not leak
back into target-aware operation construction.

## Assigned Target Operations To Machine Instructions

Input: assigned target operations.

Output: symbolic machine instructions.

Primary responsibility: convert assigned target operations into ISA instruction
forms without final object-file encoding.

Semantic nouns:

- Places: are now encoded as assigned memory/register operands.
- Values: are instruction operands.
- Facts: optional diagnostics/debug metadata.
- Loans: not active.
- Moves: become machine copies, loads, stores, or disappear.
- Drops: become calls or instruction sequences.
- Calls: become symbolic call instructions/sequences.
- Transitions: become symbolic jumps/branches/returns.
- Effects: represented by instruction/call sequences.
- Boundary edges: represented by symbolic imports, calls, syscalls, traps, or
  runtime sequences.

Must not own: section layout, relocation application, final image policy.

Known gaps: keep instruction selection separate from machine encoding.

## Machine Instructions To Object File

Input: symbolic machine instructions.

Output: relocatable object-file payload.

Primary responsibility: encode instructions, sections, symbols, and relocations.

Semantic nouns:

- Places: final storage references become section/offset/register encodings.
- Values: become encoded operands, data bytes, symbols, or relocations.
- Facts: not active except as debug/proven metadata.
- Loans: not active.
- Moves: already lowered.
- Drops: already lowered.
- Calls: become relocations, imports, or direct encoded targets.
- Transitions: become encoded branches/jumps and relocations.
- Effects: appear through emitted calls/syscalls/traps and metadata.
- Boundary edges: become imports, runtime references, syscall instruction
  sequences, or compiler-owned lowering artifacts.

Must not own: semantic validation or proof acceptance.

Known gaps: object emission is a compatibility/debug bridge; direct image
emission remains a long-term pressure.

## Object File To Final Image

Input: object files or object-shaped payloads.

Output: executable/shared image.

Primary responsibility: resolve symbols, lay out final image structures, apply
relocations, build import/export tables, and write platform image bytes.

Semantic nouns:

- Places: no longer semantic; only final addresses/sections remain.
- Values: become bytes, relocations, imports, exports, or debug metadata.
- Facts: not active except artifact/debug metadata.
- Loans: not active.
- Moves: already lowered.
- Drops: already lowered.
- Calls: final direct calls, dynamic imports, or runtime entry references.
- Transitions: final branch targets and entry/exit wiring.
- Effects: visible through imported symbols, syscalls, traps, and runtime calls.
- Boundary edges: final host/runtime/compiler edges should be auditable in image
  metadata and build artifacts.

Must not own: language semantics or borrow/proof checking.

Known gaps: Omega should move toward direct executable image construction from
machine program data where object files are not needed.
