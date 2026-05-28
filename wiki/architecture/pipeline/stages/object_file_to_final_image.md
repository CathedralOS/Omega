# Object File To Final Image

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
