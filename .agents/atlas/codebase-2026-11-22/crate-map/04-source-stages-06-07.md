# Crate map: 04-source-stages-06-07

All paths are relative to `C:/Development/Omega/.agents/worktree/worker-2/` (detached at `4f3c255d0559`).
`P06` = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program`.
`P07` = `source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi`.
Claims are from reading the named files unless marked "grepped" or "inferred". Counts come from `crates.tsv`,
`wc -l`, `grep -c` and `find`, as stated.

## What the area does

Stage 06 is the lowered-form checker. It takes the lowered program that stage 05 wrote (Terminal Psi tables plus
the obligations elaboration minted), proves each obligation with one prover whose every certificate the proof
kernel re-checks, judges initialization and loans, and returns a verdict per compilation. Its verdict is the
compiler's only accept/refuse decision (`P06/README.md:3-9`). Stage 07 takes the accepted result, publishes
the Terminal Psi module and its evidence as a canonical artifact, and binds to it the companions the source owns:
Build requests and bindings, source-record correspondence, application roots, provider selections and demand.
Input: `CompilationForPublication` (stage 05 output) in; output: `ProducedTerminalProgram` /
`CanonicalTerminalArtifact` out, to the CLI/build drivers and to native stage 00.

## Crate 1: lowered-program-to-checked-program (stage 06)

- Path: `P06`; 157 files, 114,553 lines (`crates.tsv`). `find P06 -name '*.rs' | wc -l` gives 157. `src/` is
  about 46.5k lines of non-test and test code mixed; the 77 files under `src/tests/` plus `src/tests.rs` (7,575)
  hold much of the rest.
- Purpose: decides every proof obligation of a lowered program, one verdict per compilation form. It also checks
  that the obligation rows are the ones the lowered form owes, derives what places hold, follows loans and
  invariant windows, admits loop invariants and rankings, and issues the checked program for publication.
  Everything not proved is a refusal (`Unproved`).
- Consumes: `terminal_check_tables::compilation::TerminalCompilation` (`source-rust/subsystems/compiler/source-to-psi/representations/lowered-program/lowered_program.rs:18`)
  and `Elaboration` (`.../representations/lowered-program/program.rs:188`), reached through the trait
  `CompilationForPublication` (`.../representations/lowered-program/compilation.rs:49`).
- Produces: `RouteVerdict` (`P06/src/verdict.rs:82`: `Accepted`, `AcceptedPrefix`, `Rejected{..}`,
  `Incomplete`, `FallBack`), `AcceptedClosure` (`P06/src/publication/accepted_closure.rs:8`: elaboration,
  discharge, seals, optimization record), and `CheckedPublication<Input>`
  (`.../representations/checked-program/checked_program.rs:84`), which carries the source beside the checked program.
- Entry points:
  - `judge_publication` (`P06/src/publication.rs:19`): what the source driver calls
    (`source-rust/subsystems/compiler/source-to-psi/driver/judge_lowered_source.rs:120`). Runs `judge_compilation`,
    and when optimization selections exist, runs the optimizer and judges the transformed program again.
  - `judge_compilation` (`P06/src/verdict.rs:1537`) and `judge` (`P06/src/verdict.rs:892`); the second is
    public only under `test-support`.
  - `discharge` (`P06/src/lib.rs:409`); `discharge_carrying` (`P06/src/lib.rs:427`, test-support).
  - `prepare_replay` (`P06/src/formation.rs:33`) and `ReplayScope`.
  - `finalize_callable_refinements` (`P06/src/refinement.rs:114`); `prove_callable_refinement`
    (`refinement.rs:25`, test-support); `reverify`, `reverify_at_call`, `reverify_specialization`
    (`P06/src/reuse.rs:49,134,818`, test-support).
  - `derive_contract_obligation` (`P06/src/lib.rs:2694`).
- Production dependencies in this repository (`P06/Cargo.toml`, 22 path crates): `checked-program`,
  `checked-program-issuer`, `psi-publication-records`, `service-surface`, `source-provenance`,
  `application-identity`, `numeric-literals`, `source-admission`, `source-text`, `wire`, `arena`, `diagnostics`,
  `worker-budget`, `language-core`, `numerics`, `optimization`, `proof-admission`, `semantic-vocabulary`,
  `symbols`, `terminal-check-tables` (the lowered-program representation crate), `terminal-codec`,
  `terminal-psi`, `terminal-semantics`, `terminal-work`. External: `rayon`. Feature `test-support`.
- Dependents (grepped `Cargo.toml` files): 2. `source-compiler` (`source-to-psi/Cargo.toml:42`, plus a dev entry
  with `test-support` at line 88 and its `[[test]]`/`[[example]]` entries) and `checked-program-to-psi`
  (dev-dependency only, `P07/Cargo.toml`).
- Module layout (`P06/src/lib.rs:50-80` lists 32 modules; line counts from `wc -l`):
  - `lib.rs` (2,750): `Evidence`, `Discharged`, `MachineDischarge`, `Discharge`, `discharge_inner`, goal
    substitution (`substitute_goal`), `hypotheses`, `verify` (the kernel call).
  - `verdict.rs` (2,138): `RouteVerdict`, refusal records (`UnprovedObligation`, `UnestablishedUse`,
    `RuleViolation`, `ExceededReach`, `ConflictingUse`), diagnostics, `judge_compilation`.
  - `prove.rs` (5,318) and `prove/` (`ranges`, `widths`, `reach`, `counterexample`, `float_*`, `mathematical_bounds`):
    the one prover; builds certificates, never decides.
  - `places.rs` (9,440) and `places/` (`loans`, `log`, `byte_views`, `byte_payloads`, `backing`, `contents`,
    `cleanup`, `arrivals`, `goal_observations`): what a place holds at a read, stores derived from operations,
    loans and invariant windows.
  - `minted.rs` (1,461): checks the obligation rows are the ones the lowered form owes.
  - `formation.rs` (1,385): well-formedness of contract clauses before they supply premises; replay.
  - `joins.rs`, `arrivals.rs`, `rankings.rs`, `crashes.rs`: join predicates, loop-head invariants, loop
    rankings, crash-site coverage goals.
  - `known.rs` + `known/normalization.rs`: equality transport at one program point.
  - `graph.rs` + `graph/roots.rs`: dominance between program points.
  - `entry_inputs.rs` + `entry_inputs/`: inferred entry requirements for private helpers.
  - `result_summaries.rs`, `call_contracts.rs`, `nominal_calls.rs`, `callable_arrivals.rs`, `static_contracts.rs`,
    `static_reach.rs`, `external_providers.rs`, `refinement.rs`, `reuse.rs` (2,831): call contracts, body
    summaries, static/provider conformance, proof reuse for generic forms.
  - `ceilings.rs`, `operational.rs`, `owned_cleanup.rs`, `generic_storage.rs`: service-reach and permission
    ceilings, waits, cleanup rows, generic storage checks.
  - `machine_inputs.rs`, `machine_batch.rs`, `branch_warnings.rs`, `describe.rs`: per-machine input borrow,
    parallel per-machine jobs (rayon), advisory impossible-arm reports, refusal text.
  - `publication.rs` + `publication/` (`accepted_closure`, `advisories`, `optimization`): acceptance and the
    optimizer run (`SparseConditionalConstantPropagation` at `publication/optimization.rs:28`).
- Tests: crate unit tests in `src/tests.rs` and 77 files in `src/tests/` (topics such as boundary calls,
  references, type parameters, loans, rankings, termination, table faults, optimization), plus `src/prove/tests.rs`,
  `src/known/tests.rs`, `src/graph/tests.rs`. `grep -rc "#[test]"` over `P06/src` and `P06/tests` totals 1,357.
  Because the crate sets `autotests = false`, its 12 integration files in `P06/tests/` (`block_values`,
  `cast_edges`, `construction_operands`, `dominance`, `lowered_facts`, `nominal_cleanup_selection`,
  `owned_place_transfers`, `rank_edges`, `reference_results`, `runtime_callables`, `subjectless_match`,
  `variant_parent_identity`) and 6 `examples/` (`machine_checks`, `machine_checks_mimalloc`, `proof_preparation`,
  `proof_preparation_timing`, `service_correspondence`, `service_correspondence_allocations`) are registered as
  `[[test]]`/`[[example]]` in `source-to-psi/Cargo.toml:187-303`, not in `P06/Cargo.toml`. Fixtures are under
  `P06/tests/fixtures/` (calls, operators, ownership, stored_reference_end, termination, views); the helper
  `P06/tests/support/source_handoff.rs` runs a program through stages 00-05 and lets a test alter stage 05's output
  before judging.
- Open work: `TASKS.md` items **LOWERED-FORM-CHECKER** (line 13; names `verdict.rs` as the checker entry),
  **REQUIRED-AUTOMATIC-DISCHARGE** (line 135, with sub-bullets on full-range array formation, result-relation
  rows, `scalar_reference_recasts_keep_their_backing_and_presented_fields` in `src/tests/representation_views.rs`,
  and the copyable owned value at a boundary call), **JOIN-PREDICATE-FORMS** (111-112),
  **NEGATIVE-TWIN-REASONS**, **SAME-FAMILY-RECORD-TRANSPORT** (extends the checker's record/callable
  compatibility), **OPERATION-ARITHMETIC-POLICY**, **WIDTH-INDEXED-INTEGERS**, **INTEGER-CARRIER-FAMILY-CONSTRAINTS**
  (all grepped by checker terms; read headlines only). `OWNER_CODE_CONCERNS.md` has three entries
  (lines 78, 86, 94); none names this crate.

## Crate 2: checked-program-to-psi (stage 07)

- Path: `P07`; 19 files, 10,477 lines (`crates.tsv`). `wc -l` of `src/` and `tests/` gives 10,429 plus `lib.rs`.
- Purpose: turns the checker's accepted compilation plus the exact source it came from into a published
  Terminal Psi artifact, and attaches the source-owned records the consumers need (Build bindings, requests,
  filesystem rows, source-record correspondence, application roots, selection questions). It also prepares
  per-target provider demand and bindings for native stage 00.
- Consumes: `checked_program::CheckedPublication<lowered_source_publication::LoweredSourcePublication>`
  (`P07/src/accepted.rs:209`, `AcceptedTerminalSource::from_judgment`).
- Produces: `AcceptedTerminalSource` (`P07/src/accepted.rs:11`), `PreparedTerminalPublication`
  (`accepted.rs:22`), `ProducedTerminalProgram` (`P07/src/custody.rs:65`, holds `TerminalProgramProduct`, evidence
  seals, build bindings/requests/filesystem, applications, provenance, source records, selections), and
  `Arc<CanonicalTerminalArtifact>` (type at `source-rust/subsystems/compiler/psi/artifact/codec/src/canonical_artifact.rs:51`).
- Entry points:
  - `AcceptedTerminalSource::from_judgment` (`accepted.rs:209`), `prepare_publication` (`accepted.rs:280`),
    `produce` (`accepted.rs:327`), `PreparedTerminalPublication::publish` (`accepted.rs:40`).
  - `SourcePublication::new/from_judgment/publish` (`P07/src/source_publication.rs:28,37,136`): publishes a
    roster once, admits selection questions, returns the canonical artifact.
  - `ProducedTerminalProgram::into_portable_publication` (`custody.rs:212`), `artifact` (`custody.rs:257`).
  - `TargetSupplyRequest` / `PreparedTargetSupply::prepare` (`P07/src/provider_supply/mod.rs:139,163`),
    `SourceProviderFacts::prepare_provider_demand`, `prepare_provider_bindings`
    (`provider_supply/demand.rs:95,185`).
  - `CheckedBuildReceiver::build_receiver` (`P07/src/build_receiver.rs:64`), `CheckedSourceCall::check`
    (`P07/src/invocation_site.rs:64`), `TerminalSourceSelections::admit` (`P07/src/source_selections.rs:122`).
  - `lib.rs` (117 lines) re-exports the above through `pub mod production` (`#[path = "src/lib.rs"]`).
- Production dependencies (`P07/Cargo.toml`, 31 path crates): `application-identity`, `artifacts`
  (`foundation/measurements`), `psi-publication-records`, `source-provider-publication`, `psi-authorization`,
  `psi-provider-selections`, `service-surface`, `declaration-scope`, `numeric-literals`,
  `lowered-source-publication`, `checked-program`, `source-declarations`, `source-symbol-table`, `prepared-source`,
  `typed-trees`, `source-provenance`, `evaluation-receipts` (package name `source-completion-accounting`?
  inferred from the path `build/evaluation/receipts`), `source-admission`, `source-text`, `wire`, `arena`,
  `record-storage`, `diagnostics`, `language-core`, `numerics`, `optimization`, `semantic-vocabulary`, `symbols`,
  `terminal-check-tables`, `terminal-codec`, `terminal-psi`, `terminal-semantics`. It does not depend on
  stage 06 in production; `lowered-program-to-checked-program` is a dev-dependency used by `judge_fixture`
  in `P07/lib.rs:42`.
- Dependents (grepped `Cargo.toml` files): 3. `source-compiler` (`source-to-psi/Cargo.toml:41`),
  `build-evaluation-result` (`build/evaluation/result/Cargo.toml:15`), and
  `source-completion-accounting` (`source-to-psi/representations/evaluation-receipts/Cargo.toml:15`).
- Module layout (`wc -l`):
  - `accepted.rs` (802): accepted-source factory; checks source correspondence of every accepted form
    (`accept_judgment`), builds `PreparedTerminalPublication`.
  - `source_publication.rs` (233): `SourcePublication`, one publication per roster.
  - `custody.rs` (267): `ProducedTerminalProgram`, `TerminalProductionCustody`; record encoding of the companions.
  - `source_selection_publication.rs` (3,629) and `source_selections.rs` (510): normalizes every accepted
    form's declaration selections into a question packet; `published_entry.rs` per-declaration outcomes.
  - `build_companions.rs` (857), `build_bindings.rs` (403), `build_receiver.rs` (183): Build requests,
    bindings, host Build parameter; `BuildProtocolUse`.
  - `provider_supply/` (`mod.rs` 670, `demand.rs` 245, `package_review.rs` 75), `provider_choice_inputs.rs`
    (314): per-target provider selection, demand and native binding choices.
  - `application.rs` (80), `invocation_site.rs` (281), `package_review.rs` (108): application roots with the
    entry-receipt check, call-site authentication, numeric review inventory.
- Tests: unit tests inside `src/` (`grep -rc "#[test]"` over `P07` totals 50) and one integration file,
  `P07/tests/build_companions.rs` (1,607 lines), registered in `source-to-psi/Cargo.toml:267` because the crate
  sets `autotests = false`. Covers companion publication, correspondence and refusal cases (read file name and head
  only).
- Open work: `TASKS.md` **PROVIDER-SELECTION-AFTER-TERMINAL** (line 106; names `P07/src/provider_supply`),
  also the "no product route yet" refusals raised in the driver (`source-to-psi/driver/compile_product/*.rs`).
  No `OWNER_CODE_CONCERNS.md` entry names it.

## How data flows through the area

1. Stage 05 hands a `CompilationForPublication` value (lowered program plus source correspondence) to
   `driver/judge_lowered_source.rs:120`.
2. `judge_publication` (`P06/src/publication.rs:19`) splits program and source, calls `judge_compilation`
   (`verdict.rs:1537`) and accepts only if every form is `RouteVerdict::Accepted`.
3. Per machine, in parallel (`machine_batch.rs`): `minted` checks the owed rows; `places`/`known`/`graph`
   build knowledge; `prove` builds certificates; `lib.rs::verify` has the proof kernel (`proof-admission`) decide
   each; `joins`, `arrivals`, `rankings`, `crashes`, `ceilings`, `operational` add their obligations.
4. If optimizations are selected, `optimization::apply` runs and the transformed program goes through step 2
   again; an `PsiOptimizationExecutionRecord` is attached (`publication.rs:24-47`).
5. The result is a `CheckedPublication` (`checked_program.rs:84`), then `AcceptedTerminalSource::from_judgment`
   (`P07/src/accepted.rs:209`) checks every accepted form still has its source correspondence.
6. `prepare_publication` -> `PreparedTerminalPublication::publish` (`accepted.rs:280,40`) emits the module and
   evidence through `terminal-codec` and builds the companions (`build_companions.rs`, `source_selection_publication.rs`).
7. `SourcePublication::publish` (`source_publication.rs:136`) admits the selection questions and yields the
   `CanonicalTerminalArtifact`; `provider_supply` prepares per-target provider demand for native stage 00.

## Where this area's map differs from what the documents say

- `P06/README.md:3-8` says the product driver passes the input to `judge_compilation`. The driver calls
  `judge_publication` (`driver/judge_lowered_source.rs:120`), which calls `judge_compilation`
  (`P06/src/publication.rs:24`).
- `P06/README.md` says "Start at lib.rs (`discharge`)"; `TASKS.md:13` names `verdict.rs` as the checker entry.
  `discharge` is `pub` in `lib.rs:409`, but production callers go through `judge_publication`.
- `P07/README.md:18-23` presents `SparseConditionalConstantPropagation` selection under stage 07. The code that
  runs it is in stage 06 (`P06/src/publication/optimization.rs:28`, called from `publication.rs:34`); grepped
  `P07` finds no occurrence outside the README.
- Relative wiki links in both READMEs (`P06/README.md` uses `../../../../../wiki/...`, `P07/README.md` uses
  `../../../../wiki/...`) resolve to `source-rust/wiki` and `source-rust/subsystems/wiki`; `ls source-rust/wiki`
  fails. The repository wiki is at `wiki/`, which needs six and four-plus-two `../` steps. Inferred from directory
  depth, not opened as links.
- `P06/src/lib.rs` header comments use older names ("Omega 00", "Psi 05", "stage 04" for what the comment
  calls the generic-machine generator) where the pipeline directories are numbered 00-07 and Psi stages are not
  numbered that way (`P06/src/lib.rs:24,416`).
- Neither crate's `Cargo.toml` lists its own tests; they are registered in `source-compiler`'s manifest, which
  `AGENTS.md`'s crate layout does not mention.

## What I did not read

- Bodies of `places.rs` (9,440), `prove.rs` (5,318), `reuse.rs`, `loans.rs`, `byte_views.rs` and most of the 77
  test files: read module headers and signatures only.
- `source_selection_publication.rs` (3,629): read header and `prepare` signature only.
- `wiki/spec` pages cited in the module comments and the design pages `check_on_lowered_form.md` and
  `stage_trust.md`.
- Full text of the `TASKS.md` items other than the headline of each (cut to 400-1,800 characters).

## What I am unsure about

- Whether `source-completion-accounting` is the only name behind `evaluation-receipts` in 07's manifest
  (I read the dependency path and the other manifest's `name`, not the full chain).
- The exact count of `Cargo.toml` dependents outside `source-rust/` and `omega-rust-v2/`: grepped
  `source-rust`, `omega-rust-v2`, root `Cargo.toml` and `_recycling` listing only; root `Cargo.toml:175` lists
  the 06 crate as a workspace member, not a dependent.
- The `#[test]` counts include tests that are only compiled with `test-support`; I did not check which are
  active under default features.
