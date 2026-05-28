# Assigned Target Operations To Machine Instructions

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
