# Machine Instructions To Object File

[Pipeline](../pipeline.md) | Previous: [Assigned Target Operations To Machine Instructions](assigned_target_operations_to_machine_instructions.md) | Next: [Object File To Final Image](object_file_to_final_image.md)

This stage encodes symbolic instructions into relocatable object-file data with sections, symbols, and relocations.

## Stage Contract

Input: symbolic machine instructions.

Output: relocatable object-file payload.

Primary responsibility: encode instructions, sections, symbols, and relocations.

## Semantic Ownership

- Places: final storage references become section/offset/register encodings.
- Values: become encoded operands, data bytes, symbols, or relocations.
- Facts: not active except as debug/proven metadata.
- Loans: not active.
- Moves: already lowered.
- Drops: already lowered.
- Calls: become relocations, imports, or direct encoded targets.
- Transitions: become encoded branches/jumps and relocations.
- Effects: appear through emitted calls/syscalls/traps and metadata.
- Boundary edges: become imports, runtime references, syscall instruction sequences, or compiler-owned lowering artifacts.

## Ownership Rules

Must not own: semantic validation or proof acceptance.

## Known Gaps

Object emission is a compatibility/debug bridge; direct image emission remains a long-term pressure.
