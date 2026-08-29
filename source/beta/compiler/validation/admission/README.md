# Beta compiler admission diagnostics

This directory contains the retained exact-artifact structural diagnostic for
`beta_compiler.alpha → beta_compiler_bytecode.tape`.

| Retained child/files | Bounded role | Deletion condition |
| --- | --- | --- |
| `bc-artifact-structure.alpha`, `bc-artifact-structure.sh` | Decode reachable instructions and reject malformed framing, invalid targets, root returns, cross-procedure branches, overlapping procedure regions, and tape-hole overflow. | Delete when the checked exact edge derives the same structural facts. |

The structural diagnostic does not admit the compiler edge by itself.

The status-only encoding ledger and its gate were deleted after compiler-scale
measurement proved they could not become the selected checked derivation. They
were a parallel assembly semantics, not an admission premise. OWNER Q12 (the exact
Alpha-to-Beta edge) in `OWNER_QUESTIONS.md` owns the measured conflict between one compiler-scale
reflexive equality and the generic checker's conversion-scratch lifetime.

The eventual encoding surface remains one fixed proof and one mutation gate,
not a subsystem. Artifact-specific assembly definitions, traces, and admission
policy belong here; `source/alpha/checker/` remains a generic derivation service.
