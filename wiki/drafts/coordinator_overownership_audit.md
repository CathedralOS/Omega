# Coordinator over-ownership audit

Sweep of every sequencing owner at `4a6bd936dc` (2026-09-20, Linux x86_64)
against the coordinator rule in
[omega-rust/pipeline.md](../../omega-rust/pipeline.md):

> A coordinator forwards complete typed results rather than owning package
> loading, build evaluation, stage algorithms, artifact formatting, or a
> generic orchestration subsystem.

The coordinators under audit are the named sequencers — `compiler`,
`terminal-production`, `native-realization` — plus the build/product owners
that the same rule names as the things a coordinator may not own:
`package-compilation` (package loading), `build-evaluation` (build
evaluation), `provider-planning` (provider selection), and
`compilation-report` (artifact/report formatting). Pipeline `X-to-Y` crates
are stage owners, not coordinators; the three route crates that mostly
sequence sub-stages (`source-files-to-assembled-syntax`,
`assembled-syntax-to-checked-compilation`,
`checked-compilation-to-terminal-artifact`) are cataloged separately below.

Method: enumerate `src/` top-level files and module directories per crate,
read each entry file and the largest submodules, and classify the resident
code as *forwards* (constructs requests, sequences typed calls, carries
results) or *owns* (contains the algorithm, policy table, or formatting
logic itself).

## Verdicts

| Coordinator | Shape | Verdict |
|---|---|---|
| `compiler` (2.0k src lines) | `compiler.rs` is a 137-line sequencing loop: `validate_for_execution` → `PreparedCheckedSource::prepare` → per-target `check`/`admit_checked_compilation`/`produce_terminal_report`/`prepare_native_product`, failure isolation per target, `CompileReport` assembled by its owner | Forwards — one defect under **F1** |
| `terminal-production` (1.8k) | `terminal_production.rs` sequences `lower_*`/`run_psi_optimization`/`finalize_terminal_artifact`; `receiver_eligibility.rs` is 1,158 lines of eligibility derivation — a semantic algorithm living in the sequencer | Borderline — **F2** |
| `native-realization` (22.3k) | Sequences input preparation → provider admission → emission → replay, but also hosts the terminal-authority policy/review subsystem and the optimized-semantic-wrapper codec | Over-owns — **F3**, **F4** |
| `build-evaluation` (12.8k) | Owns admission/execution of builds | Named owner of build evaluation — correct by the rule |
| `package-compilation` (6.9k) | Owns the package graph, source snapshot and consumption custody | Named owner of package loading — correct by the rule |
| `provider-planning` (31.5k) | Owns provider selection, calling-policy realization, approval | Named owner — correct, though its size is the largest single-owner concentration in the build half |
| `compilation-report` | Owns report/receipt/publication formatting | Named owner of artifact formatting — correct by the rule |

## Findings

- **F1 — dead orchestration residue in `compiler`.**
  `omega-rust/omega/compiler/compiler/src/compiler/native/prepared.rs`
  (171 lines) is unreachable: no `mod native` declaration exists anywhere in
  `compiler/src/`, and the file's `use super::{admission, realization}`
  imports resolve to modules that no longer exist — the same-named types
  (`NativeInputReuseKey`, `PreparedNativeCompilation`) now live in
  `native-realization` (`native_realization::NativeInputReuseKey` is what
  `compiler/tests.rs` exercises). The file cannot compile if linked; it is
  unreferenced source left behind by the input-reuse migration. Delete it.

- **F2 — `terminal-production` hosts a substantive derivation.**
  `psi/compiler/terminal-production/src/terminal_production/receiver_eligibility.rs`
  (1,158 lines — larger than the 673-line coordinator root) derives
  `CheckedProgramEntryReceiverEligibility` from checked trees, replaying
  understood value constraints per record field. That is a semantics-stage
  algorithm resident in the Psi sequencer. It is reachable and pinned today
  (`terminal_production.rs:565-566` calls `receiver_eligibility::derive`), so
  this is placement debt, not dead code: the natural owner is the lowered-Psi
  or terminal-verification family that produces the eligibility evidence
  shape. Not an urgent correctness issue; flag for relocation when the file
  next grows.

- **F3 — `native-realization` owns authority policy, not just sequencing.**
  `terminal_authority_policy/` (construction, classification, inventory,
  commitment, normalized_foreign, model — multi-file subsystem) plus
  `terminal_authority_permission_policy.rs` (246 lines),
  `terminal_authority_permissions.rs`, `terminal_authority_review.rs`, and
  `terminal_authority_review/` are authority-classification and review
  algorithms — stage algorithms carried inside the sequencing crate. The
  crate's own lib.rs already names "settles entry/provider custody" as its
  role, so this is partially sanctioned; the question for a follow-up is
  whether the *policy tables* (the rows themselves, not the review pass)
  belong beside the authority evidence owners in packages/review rather than
  in the realization sequencer. Interim verdict: acceptable load-bearing
  ownership; monitor rather than split blindly.

- **F4 — `native-realization` owns artifact formatting via an orphan codec.**
  `optimized_semantic_wrapper_object/codec.rs` (397 lines, plus
  `optimized_semantic_wrapper_encoding/`) is a durable codec living in the
  coordinator — precisely the "artifact formatting" the rule excludes. This
  overlaps the mined `WRAPPER-OBJECT-OWNERSHIP` item and the
  `DURABLE-CODEC-RELOCATION` remaining-work bullet (move durable codecs to
  their representation owner in `omega/representations/`).

## Cataloged, not flagged

- `run_on_compile_thread`
  (`assembled-syntax-to-checked-compilation/src/checking/compile_thread.rs`):
  a 256 MiB-stack spawn helper whose own docstring admits "host execution
  infrastructure, not a compiler stage". It is shared by `compiler` and
  `compiler::package` and lives inside a stage crate — a generic-orchestration
  primitive in a transform. Small enough that relocation is optional;
  note only.

- `source-files-to-assembled-syntax`: `source/` (SourceStorage/ImportQueue)
  is private working state a stage may own; `source_assembly/build_vocabulary/`
  (~300 lines) is borderline build-vocabulary algorithm in a routing stage —
  cataloged; the crate otherwise forwards to `source-files-to-tokens` and
  `tokens-to-syntax-trees`.

- `assembled-syntax-to-checked-compilation`: `checking/` carries the build
  continuation and `const_evaluation.rs` — these are the stage's own
  transform algorithms (an `X-to-Y` owner), not coordinator over-ownership.

- `checked-compilation-to-terminal-artifact`: `behavior_exclusions.rs`,
  `native_proposal/`, `application_coverage/` are the stage's own production
  checks; `behavior_exclusions` is deliberately shared with
  `build-evaluation` under BUILD-SEMANTIC-EXCLUSIONS' two-owner split.

## Residual risk

The sweep reads module structure and entry files; it does not trace every
`pub` surface for ownership leaks, and it does not measure whether the
policy tables under F3 would change owner under PIPELINE-OWNER-CONSOLIDATION.
F1 is safe to delete at any time (the file cannot compile). F2/F4 have
sibling board items (`WRAPPER-OBJECT-OWNERSHIP`, `DURABLE-CODEC-RELOCATION`)
that should carry the moves; this audit fixes no code.
