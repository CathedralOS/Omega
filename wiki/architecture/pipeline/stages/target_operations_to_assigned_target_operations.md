# Target Operations To Assigned Target Operations

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
