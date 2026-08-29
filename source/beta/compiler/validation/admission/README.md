# Beta compiler admission diagnostics

This directory contains the two retained exact-artifact diagnostics for
`beta_compiler.alpha → beta_compiler_bytecode.tape`.

| Retained child/files | Bounded role | Deletion condition |
| --- | --- | --- |
| `bc-artifact-structure.alpha`, `bc-artifact-structure.sh` | Decode reachable instructions and reject malformed framing, invalid targets, root returns, cross-procedure branches, overlapping procedure regions, and tape-hole overflow. | Delete when the checked exact edge derives the same structural facts. |
| `encoding/` | `beta-compiler-encoding-ledger.alpha` and `test.sh` provide Alpha-written exact two-pass source/tape reconstruction, positive and negative subject mutations, and a compiler-sized fixed-path control for the checker's exact-subject carrier. | Replace atomically with one fixed `beta-compiler-encoding.proof` plus this gate when the rooted certificate absorbs the same relation and mutations. If measured proof construction needs an Alpha producer, transform this ledger into that sole producer; never retain both forms. Delete earlier if adaptation creates a second assembly source of truth. |

Neither diagnostic admits the compiler edge by itself.

The target encoding surface is two files, not a new subsystem: the fixed proof
and this gate. A producer is a measured exception and replaces the current
ledger in place. Artifact-specific assembly definitions, traces, and admission
policy remain here; `source/alpha/checker/` stays a generic derivation service.
