# Gamma compiler admission diagnostics

This directory contains the retained exact-artifact structural diagnostic for
`gamma_compiler.beta → gamma_compiler_bytecode.tape`.

| Retained child/files | Bounded role | Deletion condition |
| --- | --- | --- |
| `gc-artifact-structure.beta`, `gc-artifact-structure.sh` | Decode reachable instructions and reject malformed framing, invalid targets, root returns, cross-procedure branches, overlapping procedure regions, and tape-hole overflow. | Delete when the checked exact edge derives the same structural facts. |

The structural diagnostic does not admit the compiler edge by itself.

The status-only encoding ledger and its gate were deleted after compiler-scale
measurement proved they could not become the selected checked derivation. They
were a parallel assembly semantics, not an admission premise. The settled
**BETA-GAMMA-COMPOSED-CERTIFICATE** task in `TASKS_BOOTSTRAP.md` owns the
measured conflict between one compiler-scale reflexive equality and the generic
checker's conversion-scratch lifetime.

The eventual encoding surface remains one fixed proof and one mutation gate,
not a subsystem. That proof may contain bounded named chunk equalities and one
checked composition theorem while still deriving one root edge judgment.
Artifact-specific assembly definitions, traces, and admission policy belong
here; `source/alpha/checker/` remains a generic derivation service.

The selected implementation shape is a streaming two-pass DFA over
checker-owned raw subtrees. Cuts are power-of-two paths supplied by the
certificate and checked for source/tape adjacency and exhaustion; they are not
fixed semantic authority. Parser-rich pass-one measurement begins at 256-byte
subtrees, while larger comment-only regions may be coarsened after measurement.
The first 1,024-byte comment subtree already checks the exact textual-ASCII and
comment transitions in the authoritative checker in under one second. A
complete retained pass-one checkpoint must include every source region,
cross-cut token/`db` continuation, fixed-width PC accounting, balanced unique
label-map construction, and the exact 104,878-byte / 27,087-byte joint. No
one-chunk demonstration is retained here.

The latest temporary label-map measurement counted 457 definitions and 1,010
symbolic references. Balanced lookup and exact undefined/duplicate/interior
span discriminators all completed in under one second, but the attempted
31-piece aggregate rejected after 16.75 seconds at about 120 MiB RSS because
one 27-row fold did not establish its claimed summary. The other exact
adjacencies in that piece accepted. This is evidence for the compact accessor
and bytewise identity, not for the broken fold; no generated map, ledger,
certificate, or gate was retained. Pass one must establish label-token and
maximal-boundary validity as it emits each record so the final join proves only
exact row coverage, strict order/uniqueness, and frozen lookup ownership.
