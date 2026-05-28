# Semantic Spine

The semantic spine is the shared vocabulary every IR stage should preserve,
refine, or deliberately lower away.

These nouns should be recognizable across stages even though their data shape
changes.

## Places

A place is a location-like expression that can be read, written, borrowed,
moved, or invalidated.

Examples: `self.health`, `items[index]`, `room.exits[north]`.

Current status: fairly strong. `omega_facts::Place`, `PlaceRoot`,
`PlaceSegment`, and checked-flow `CanonicalPlace` already exist.

Desired direction: every stage that mutates, borrows, invalidates, or lowers a
location should use a stage-specific place projection instead of reconstructing
paths from raw expressions.

## Values

A value is a produced runtime or compile-time object with type, initialization,
ownership, and storage/lowering consequences.

Current status: weak as a first-class ownership concept. Values mostly appear as
expressions, symbols, typed nodes, or backend operands.

Desired direction: checked/lowered IR should distinguish "expression syntax"
from "value instance" so moves, copies, drops, and storage can be reasoned about
directly.

## Facts

A fact is a proven or accepted assertion at a program point.

Current status: strong. `omega_facts::Fact`, `FactContext`, `FactOrigin`,
`FactPayload`, and `ProgramPoint` are real infrastructure.

Desired direction: facts should remain the proof-facing currency for domains,
bounds, invariants, contracts, and boundary guarantees.

## Loans

A loan records that a place or view is borrowed over a span of program points.

Current status: strong in checked trees. `BorrowLoanFact`, loan activation, loan
weakening, and overlap checks exist.

Desired direction: loans should attach to place projections and be invalidated
by move/drop/write/call/transition events through one flow model.

## Moves

A move transfers ownership of a value or place and may make the source unusable.

Current status: weak. Move behavior is checked in places, but there is no
durable `MoveFact` or `MoveEvent` equivalent.

Desired direction: moves should become explicit checked-flow events with source
place/value, destination, program point, and invalidated facts/loans.

## Drops

A drop ends a value's lifetime and runs cleanup if required.

Current status: weak. Drops are a language concern but not yet first-class in the
ownership flow IR.

Desired direction: drops should become explicit scheduled events before backend
lowering, with proof-visible cleanup obligations and effect/boundary metadata.

## Calls

A call invokes a machine, state, operator, helper, or imported boundary entry.

Current status: strong. Semantic call facts, borrow call facts, contract call
facts, flow call facts, and effect call facts all exist.

Desired direction: calls should converge on a shared call-site identity so
borrow, proof, effects, transitions, and backend lowering do not maintain
parallel ordinals by accident.

## Transitions

A transition transfers control and arguments between states or exits.

Current status: medium. Transitions are first-class in syntax, state graph, and
control flow, but ownership transfer across transitions should be as explicit as
call transfer.

Desired direction: transition facts should record argument flow, carried facts,
invalidated facts, moved values, and scheduled drops.

## Effects

An effect records externally visible capability behavior such as allocation,
IO, process exit, or host interaction.

Current status: strong. `omega-effects::EffectPlan`, `EffectSet`, direct
effects, transitive effects, and effect paths exist.

Desired direction: effects should be attached consistently to calls,
transitions, drops, allocations, and boundary edges.

## Boundary Edges

A boundary edge is where Omega stops proving from Omega source and accepts a
declared contract from compiler/runtime/host/toolchain code.

Current status: medium. Boundary syntax, contracts, operators, target policies,
and reports exist. Boundary edges are not yet one unified checked-flow entity.

Desired direction: checked flow should expose boundary crossings as explicit
events with provider, contract, effects, accepted facts, and unchecked-policy
status.
