# Assigned Target Operations To Machine Instructions

[Pipeline](../pipeline.md) | Previous: [Target Operations To Assigned Target Operations](target_operations_to_assigned_target_operations.md) | Next: [Machine Instructions To Object File](machine_instructions_to_object_file.md)

This stage converts assigned target operations into symbolic ISA instructions without final object encoding.

## Stage Contract

Input: assigned target operations.

Output: symbolic machine instructions.

Primary responsibility: convert assigned target operations into ISA instruction forms without final object-file encoding.

## Semantic Ownership

- Places: are now encoded as assigned memory/register operands.
- Values: are instruction operands.
- Facts: optional diagnostics/debug metadata.
- Loans: not active.
- Moves: become machine copies, loads, stores, or disappear.
- Drops: become calls or instruction sequences.
- Calls: become symbolic call instructions/sequences.
- Transitions: become symbolic jumps/branches/returns.
- Effects: represented by instruction/call sequences.
- Boundary edges: represented by symbolic imports, calls, syscalls, traps, or runtime sequences.

## Ownership Rules

Must not own: section layout, relocation application, final image policy.

## Known Gaps

Keep instruction selection separate from machine encoding.
