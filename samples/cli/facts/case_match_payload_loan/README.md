# Case-pattern expression payload loan

A case-pattern expression match retains the selected stored exclusive loan while writing 41 to its original backing and preserving sibling 19.

Owner: **STATE-LOCAL-VALUE-FRONTIER**, ordinary aggregate construction and reads.
**MATCH-SELECTIVE-LOWERING** owns the shared pattern/value dispatch mechanism.
This is value-position extraction; `case_payload_stored_loan` covers transition
dispatch. The operation consumes an actually established exclusive loan, never
authority inferred from a mutable-reference type.

## Witnesses

Each place declares expected exit0 and binds all four hosted targets. Its
`invalid/main.omg` changes only the containing descriptor from exclusive to
shared; the nested payload's requested mutable access stays unchanged.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [shared descriptor](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [shared descriptor](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [shared descriptor](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [shared descriptor](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [shared descriptor](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [shared descriptor](const_name/invalid/main.omg) |
| Another state | [program](other_state/main.omg) | [shared descriptor](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [shared descriptor](other_machine/invalid/main.omg) |
| Another target body | [program](target_body/main.omg) | [shared descriptor](target_body/invalid/main.omg) |

The nine pairs are copied to `tests/omega/{pass,fail}/facts/` as
`case_match_payload_loan_through_<place>_{exit,rejected}`.

## Requirement evidence

| Requirement | What this row establishes |
| --- | --- |
| Establishment | Ordinary construction creates the payload and exact exclusive loan; the store uses its already established value41. The exit guard observes the outcome and grants no authority. |
| Identity and lifetime | Saved subjects execute once and retain the exact descriptor, backing and sibling paths. Snapshot, replacement and lending controls distinguish source observations from later storage. |
| Composition | All nine places carry that same loan to the consuming write. False guards retain their effects; unselected bodies do not execute. Shared access, overlapping parent loans, duplicate exclusive payloads and invalidated case facts reject. |
| Soundness boundaries | Payload extraction transfers existing access and custody. Named residuals retain their loans until disposed; mathematical identities and runtime homes remain separate. |
| Psi05–07 | Structural/case patterns use ordinary short-circuit predicates, retained subject captures and structural value continuations. Native and Terminal execution agree; nominal cleanup consumes the current residual after selected moves and before the custody join. |
| Omega00–02 | Provider settlement, layout and calling plans carry the stored descriptor to its original backing. This row exercises ordinary record/case geometry, not every layout or ABI rule. |
| Omega03–06 | Independent custody admission authenticates original ingress and resident loan correspondence; instruction selection, register/frame homes and calls preserve the two backing values on macOS AArch64. |
| Encoding/emission | Produced Mach-O programs execute with the authored result and agree with their exact Terminal artifacts. Other image formats and hosted native targets are not established by this host. |

[Pattern semantics](../../../../wiki/spec/language/patterns.md),
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md),
[ownership](../../../../wiki/spec/terminal-psi/ownership.md) and
[carried borrows](../../../../wiki/spec/language/lifetimes.md) govern the row.

Additional controls pin copied scalar/descriptor snapshots, custom equality,
nested saved tags, effect order and once-only computed subjects. Observable
cleanup controls cover scalar/structural results, linear child moves, named
versus anonymous residuals, skipped arms, whole moves, two child moves,
[nested selection](controls/nested_observation_cleanup/main.omg), and
[a returned field loan held through its receiving call](controls/retained_observation_cleanup/main.omg).
[Supplying type scopes](controls/projected_operand_scopes/main.omg) distinguish
same-spelled lifetimes from different declarations, projected replacement and
permuted lifetime arguments without inventing a caller binder.

## Current frontier

Baseline producer `9e45362bf0e` refused the unchanged root during parsing:
`record and domain destructure patterns belong to transition dispatch`.
The original root source SHA256 is
`d34ab9f25d660fbd5b412df7ac0d578a18b7a9cb3777acf308704ee8f475b7ea`.

At integrated base `1e7b3a721e8`, the focused corpus observes native0/Terminal0
for all nine positives and 15 additional controls on macOS AArch64
(`build/case-match-1e7-integrated-corpus.json`). These are optimized
dirty-worktree observations, pending pinned qualification, independent
exact-candidate review and verified main landing. Run records retain their
actual producer and source identities; earlier observations are not relabeled
as this producer's executions.

Theorem/quotient correspondence still refuses captured-subject expressions:
its legacy value-substitution key cannot encode lexical capture binders.
The portable scoped normalizer does not authorize comparing diagnostic capture
handles or substituting an unsubstituted identity for that key.

Guard-time descriptor replacement remains an uncompleted backing-retention
obligation; its straight-line attribution control also exposes a baseline
gap. The separate retiring checked-tree interpreter supports the scalar/copy
snapshot and predicate controls but explicitly refuses expression-owned nominal
cleanup; it is not the Terminal acceptance engine. The parent capabilities
remain open for their other requirements.

Windows native execution is explicitly owner-deferred to a Windows agent.
Linux native execution is unverified. Compile bindings for those targets do
not establish runtime acceptance.
