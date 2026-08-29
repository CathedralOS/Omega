# Beta compiler admission diagnostics

This directory contains the two retained exact-artifact diagnostics for
`beta_compiler.alpha → beta_compiler_bytecode.tape`.

| Retained child/files | Bounded role | Deletion condition |
| --- | --- | --- |
| `bc-artifact-structure.alpha`, `bc-artifact-structure.sh` | Decode reachable instructions and reject malformed framing, invalid targets, root returns, cross-procedure branches, overlapping procedure regions, and tape-hole overflow. | Delete when the checked exact edge derives the same structural facts. |
| `encoding/` | `beta-compiler-encoding-ledger.alpha` and `test.sh` provide Alpha-written exact two-pass source/tape reconstruction, positive and negative subject mutations, and a compiler-sized control for the checker's exact-subject carrier. | Adapt into the artifact-aware checked ledger, then delete this status-only form and carrier control when the rooted certificate absorbs them; delete earlier if adaptation would retain a second assembly source of truth. |

Neither diagnostic admits the compiler edge by itself.
