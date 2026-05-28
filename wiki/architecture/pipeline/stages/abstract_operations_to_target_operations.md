# Abstract Operations To Target Operations

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
