# Machine Instructions To Object File

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
