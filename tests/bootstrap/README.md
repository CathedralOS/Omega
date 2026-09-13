# Bootstrap-chain tests

Only tests whose subject spans more than one language rung live here. Tests of
one accepted language or compiler live under `tests/<language>/` instead.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `chain-hygiene.sh` | Checks cross-rung owner inventory in source archives and Git checkouts, including empty relocation leftovers and actual alternate owners. | Delete only when another topology regression test covers both forms of source custody. |
| `alpha-beta-edge.sh` | Checks the selected Alpha seed, Alpha conformance, exact trusted Beta compiler reconstruction, and finite root audit. | Delete only when a stronger Alpha-to-Beta edge gate subsumes all observations. |
| `source-closure.sh`, `source-closure.py` | Checks ordered Delta/Epsilon source materialization, exact evaluator and D reproduction, malformed or noncanonical manifests, closed source inventories, and output preservation on rejection. | Delete only when the selected lower compilers consume and verify the same member manifests directly. |
| `alpha-identity.sh` | Checks stamping uses exactly the audited Alpha seed container, refuses corrupted or truncated containers before writing, and that the bound identities match the retention inventory. | Delete only when a manifest-driven identity check covers the same subject. |
| `beta-identity.sh` | Checks the shared Beta materializer stamps exactly the audited source/tape pair, refuses corrupted or truncated tapes before writing, and that the bound identities match the audit and root-audit records. | Delete only when a manifest-driven identity check covers the same edge. |
| `gamma-identity.sh` | Checks the shared Gamma materializer stamps exactly the selected evaluator source/tape pair, refuses corrupted or truncated tapes before writing, and that the bound identities match the profile, heap-boundary, and composed-manifest records. | Delete only when a manifest-driven identity check covers the same edge. |
| `delta-identity.sh` | Checks the shared Delta materializer repacks exactly the bound entry-plus-manifest closure, refuses corrupted entry, manifest, member, and composed record before writing, and that the bound identities match the composed record, rung README, and staged compiler pins. | Delete only when a stronger identity check covers the same edge. |
| `epsilon-identity.sh` | Checks the shared Epsilon materializer repacks exactly the bound evaluator closure, refuses corrupted manifest and member before writing, and that the bound identities match the rung README and checking-gate pins. | Delete only when a stronger identity check covers the same edge. |
| `omega-identity.sh` | Checks the shared Omega D materializer repacks exactly the bound compiler closure, refuses corrupted manifest and member before writing, and that the bound identities match the rung README and omega-parser and source-closure gate pins. | Delete only when a stronger identity check covers the same edge. |
| `proofs-identity.sh` | Checks the shared proof materializers repack exactly the bound checker and Beta-encoding member closures, refuse corrupted manifests and members before writing, and that the bound identities match the proof README records. | Delete only when a stronger identity check covers the same subjects. |
| `omega-parser/` | Checks and executes a complete-D parser customer through the selected lower chain, including guard failures and parser reuse. Explicit slow gate; see its README. | Delete only when a stronger full-D gate subsumes these observations. |
