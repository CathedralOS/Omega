# Coordinator scope audit

Audit of coordinator and entry files against the two rules they answer to:
"Coordinators stay boring — sequence typed phases and stop"
([AGENTS.md](../../../AGENTS.md)) and the discoverability rule that an entry
file owns "input preparation, phase ordering or dispatch, subordinate work,
and result/error handling" — not the domain computation itself. Audited at
`210ffe3c93` (post `tools: release-record substrate`).

Method: every pipeline crate's named root file beside `lib.rs`, the named
coordination surfaces (`lowering/coordination.rs`,
`native_pipeline`, `pass_manager`), the product route coordinators
(`main.rs`, `compiler.rs`, `terminal_artifact.rs`, both `checking.rs`
drivers), and the swarm-coordinator tooling. For each: does the file contain
domain decisions (representation computation, semantic checks, generated
content, ISA/format detail) or only sequencing? Delete this audit once the F3
watch item moves the product fences to the admission owner, or when a later
sweep supersedes it.

## Findings

### F1 — `source_assembly.rs` owns generated-source content (moderate) — RESOLVED

Resolved at `97de35d903` ("omega: extract build prelude + entry contract
seed from source_assembly"): `source_assembly.rs` dropped to ~507 lines of
load/discover sequence; prelude construction lives in
`source_assembly/build_prelude.rs` and contract-seed derivation in
`source_assembly/entry_contract_seed.rs`. Original finding text retained
below for the audit trail.

`omega-rust/omega/pipeline/source-files-to-assembled-syntax/src/source_assembly.rs`
(1014 lines) sequences discovery → lex → parse → import queue, which is the
crate's transform. But two constructor blocks are domain work inline in the
entry file:

- `construct_build_prelude` + `inject_build_prelude` (~150 lines) assemble
  the generated `build` prelude source text — literal generated content;
  belongs beside `build_vocabulary` or in a `build_prelude` sibling.
- `hosted_entry_contract_seed` (~80 lines) computes the hosted entry contract
  seed from source — semantic derivation, not sequencing.

Repair: move prelude construction and contract-seed derivation into named
siblings; the root keeps `assemble_syntax` and the load/discover sequence.

### F2 — `lowering/coordination.rs` owns the settlement roster contract (moderate) — RESOLVED

Resolved at the commit carrying this note: the fail-closed roster moved to
`coordination/settlement_roster.rs` — `index_settlement_bindings` owns the
Duplicate/Unknown indexing contract and `validate_settlement_roster` owns
the Overlaps/Partial/Missing/Unused rejoin against
`installed_provider_calls` indexes. `coordination.rs` sequences: qualify →
index → validate → per-function `lower_function` dispatch →
post-validation. Original finding text retained below for the audit
trail.

`omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/coordination.rs`
(166 lines) inlines ~100 lines of fail-closed roster validation over
`settlements_by_boundary`, `installed_by_call`, and `boundary_calls`:
Duplicate/Unknown/OverlapsInstalledProvider/PartialInstalledProvider/
Missing/Unused — six of the crate's `LoweringError` variants are emitted
from this one function. The crate already has a `boundary_settlements`
sibling module; the completeness roster is a domain contract that belongs
there, leaving `coordination.rs` to sequence: qualify → index → validate →
per-function `lower_function` dispatch → post-validation.

Repair: extract a `settlement_roster` (or extend `boundary_settlements`)
owner that produces a validated roster; coordination.rs calls it and
dispatches. Not blocking — the file is honest about being the entrance —
but it is the clearest "coordinator absorbed a checker" case in the sweep.

### F3 — `compiler.rs` product fences sit in the route loop (minor)

`omega-rust/omega/compiler/compiler/src/compiler.rs` is otherwise the gold
standard, but the artifact-only/PCC/check-only refusal rules (three
`Diagnostic::error` fences) are product-admission decisions evaluated inside
the target loop rather than in `admit_checked_compilation` or request
validation. Acceptable today since each fence gates the loop's next stage;
flag as a watch item if the fence family grows — a fourth fence should move
the family to the admission owner.

## Clean rows (verified sequencers)

| File | Verdict |
| --- | --- |
| `omega/src/main.rs` | parse → worker spawn → invocation dispatch only |
| `compiler/compiler/src/compiler.rs` | validate → prepare → per-target check/admit/product → report (F3 noted) |
| `pass_manager/{mod,entry,execution,model,accounting}.rs` | entry owns 4 run/replay APIs; execution owns dispatch; accounting owns convergence; model owns carriers — exemplary split |
| `native_pipeline/{mod,report}.rs` + `physical_pipeline/mod.rs` | pure sequence: selections → instruction selection → optimization → allocation → machine → realization |
| `abstract_operation_optimization/mod.rs` | entrance + re-exports only |
| `assembled-syntax-to-checked-compilation/src/checking.rs` | driver: prepare → check → build continuation → child compile; mode gates are the route's own contract |
| `typed-trees-to-checked-trees/src/checking.rs` + `facts.rs` | checking route + fact assembly; extra exported entries share the route (`resolution.rs`-style), documented |
| `checked-trees-to-lowered-psi/src/machine_lowering.rs` | select → dispatch → retained-custody sequence; fail-closed documented |
| `lowered-psi-to-lowered-psi/src/psi_optimization.rs` | executes selected pass list; no pass bodies inline |
| `terminal-psi-to-abstract-operations/src/artifact_admission.rs` | preparation → per-kind admission → retention roster |
| `selected-instructions-to-register-homes/src/register_allocation.rs` | documented decision sequence; each leg owned by `assignment::*` |
| `checked-compilation-to-terminal-artifact/src/terminal_artifact.rs` | custody validators + retained-artifact production under the admission profile; section replay delegated |
| `syntax-trees-to-symbol-resolved-trees/src/resolution.rs` | route doc + driver; per-item translation delegated |
| `source-files-to-tokens/src/lexer.rs`, `symbol-resolved-.../lowerer.rs` | the file IS the stage's mechanism (lexer state machine / per-kind lowering dispatch); not coordinators |
| `tools/coordination.py`, `claims.py`, `landing.py` | claim/landing fence machinery; sequence-only |
| `tools/swarm/launch.py` | wave coordinator: manifest validate → partition → prompt render → spawn; `route_crates` computes crate routing inside the launcher — its assigned job, flagged benign |

## Net

Two repairable overownerships (F1, F2) and one watch item (F3). Both repairs
are mechanical extractions with existing sibling owners; neither changes
behavior. The coordinator estate otherwise holds the "stay boring" line —
the largest files audited (`source_assembly.rs` excepted) are documented
routes whose size is dispatch breadth, not absorbed domain work.

## Candidate board items

- **SOURCE-ASSEMBLY-ENTRY-EXTRACTION** — move build-prelude construction and
  hosted-entry contract seed out of `source_assembly.rs`.
- **SETTLEMENT-ROSTER-EXTRACTION** — move the boundary-settlement completeness
  roster out of `lowering/coordination.rs` into `boundary_settlements`.
