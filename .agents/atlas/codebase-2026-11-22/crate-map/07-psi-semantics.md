# Crate map: 07-psi-semantics

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Root of the area: `source-rust/subsystems/compiler/psi/semantics/`
(below, `S/`). Crate list, files and lines come from `crates.tsv` (read). `terminal-semantics` counts exclude the
six nested crates (its 231 files / 114,411 lines equal `S/src` + `S/tests`, checked by summing `wc -l`).

## What the area does

This area holds the rules that give a Psi program (the portable compiled form, type `TerminalModule` in
`source-rust/subsystems/compiler/psi/psi.rs`) one meaning for every consumer. The source checker (stage 06 of
source-to-psi), the interpreter and the native compiler call these crates for questions such as: what does an
operation mean, which references and loans are live, which storage a call claims, whether a proof certificate
holds, what a closed predicate evaluates to, and how many work units an executed operation costs. Input is a
Psi module or catalog (operations, types, declarations, certificates, predicates) plus concrete operands where a
question is about values. Output is yes/no judgments, derived rows (origins, regions, plans, accepted facts) and
costs. No crate here lowers, emits or executes a program; the interpreter and native compiler own that.

## Crates (in dependency order, leaves first)

### 1. closed-predicates

- Path `S/closed-predicates` (lib `closed_predicates.rs`); 5 files, 936 lines (crates.tsv).
- Purpose: evaluates a published predicate (`Proposition`) on concrete operands under shared resource limits, so
  the compiler and the interpreter reach the same verdict. Decides crash-route guards with short-circuiting, closed
  IEEE float relations and arbitrary-precision signed integer relations.
- Consumes `semantic_vocabulary::Proposition`/`ScalarTerm`/`IntegerMathTerm` (`guard.rs:5`, `ieee.rs:7`); produces
  `Option<bool>`/guard verdicts and `ClosedIntegerEvaluationError` (`integer.rs:15`).
- Entry points: `closed_guard`, `closed_guard_with`, `closed_guard_with_float_meanings` (`guard.rs:23,15,32`);
  `closed_ieee_relation` (`ieee.rs:13`); `ClosedIntegerEvaluator` (`integer.rs:28`), `big_integer_literal`
  (`integer.rs:214`), `check_integer_math_term_size` (`integer.rs:205`); `PredicateBudget` (`budget.rs:8`),
  `check_predicate_evaluation_size` (`budget.rs:182`).
- Production dependencies (Cargo.toml): bignum, numerics, semantic-vocabulary. Dependents (grepped manifests): 3:
  terminal-semantics, proof-admission, interpreter.
- Modules: `budget.rs` (size and budget limits), `guard.rs` (guard decision), `ieee.rs` (float relations),
  `integer.rs` (bounded integer evaluator; limits 8,192 nodes, depth 128, 65,536 result bits, 262,144 limbs,
  1,048,576 limb-work at `integer.rs:8-12`).
- Tests: inline only: 12 `#[test]` (guard 1, ieee 2, integer 9; grepped). No `tests/` directory.
- Open work: none named in TASKS.md or OWNER_CODE_CONCERNS.md (grepped).

### 2. layout-planning

- Path `S/default-integer-layouts` (lib `integer_defaults.rs`); 1 file, 60 lines.
- Purpose: gives a standalone integer or a record-field integer its default size and alignment (1 to 128 bits;
  alignment capped at 8 bytes standalone, 16 as a field). Re-exports `conventional_record_geometry` and
  `conventional_sum_geometry` from layout-plans.
- Consumes a bit width; produces `layout_plans::StorageGeometry`.
- Entry points: `standalone_integer_geometry` (`integer_defaults.rs:8`), `field_integer_geometry` (`:13`).
- Production dependencies: layout-plans only. Dependents: 3: terminal-semantics (one call site,
  `src/references/reference_storage_geometry.rs:80`), calling-conventions (native targets), source-to-psi.
- Tests: one inline unit test (`only_closed_host_widths_have_default_storage`).
- Open work: none found.

### 3. resource-bounds

- Path `S/storage-bounds` (`src/lib.rs`); 1 file, 83 lines.
- Purpose: the static law for composing activation storage claims. Claims on branches of a path tree add along a
  path and take the maximum across mutually exclusive branches. The checker and the live ledger use the same law.
  It holds no ledger, program or host state (doc comment `lib.rs:1-3`).
- Types and entry points: `ActivationClaimBranch` (`lib.rs:6`), `ClaimBoundRow` (`:33`), `aligned_claim_bound`
  (`:40`), `compose_claim_bounds` (`:51`), `validate_committed_within_bound` (`:77`).
- Production dependencies: extent-contracts (`psi/access/extents`). Dependents: 3: terminal-semantics (one call
  site, `src/storage/activation_storage/flow.rs:1128`), interpreter, interpreter/memory/extents.
- Tests: none in the crate (no `#[test]`, grepped).
- Open work: none found.

### 4. mathematical-kernel

- Path `S/proofs/mathematical` (lib `mathematical.rs`); 12 files, 14,447 lines.
- Purpose: the predicative proof kernel: de Bruijn dependent terms, `Type`/`Strict` sorts with closed levels,
  Pi/lambda/application typing, dependent pairs, typed conversion, the `Two` type with `caseTwo`, and the indexed
  W-family and quotient schemes. Verifies a supplied mathematical certificate against a signature and context.
- Consumes `MathematicalCertificate` (`certificate.rs`) with `TermArena`/`Signature`; produces accept or
  `CoreError`, plus `Budget`-bounded conversion results.
- Entry points: `verify_mathematical_certificate` (`certificate.rs:658`), `verify_mathematical_certificates`
  (`certificate.rs:16`), `check_type` (`typing.rs:3837`), `infer_type` (`typing.rs:237`), `convertible`
  (`conversion.rs:218`), `check_signature` (`signature.rs`), `indexed_scheme` (`indexed.rs`), `quotient_scheme`
  (`quotient.rs`).
- Production dependencies: arena, record-storage. Dependents: 2: proof-admission, `psi/artifact/wire` (the Psi wire
  reader).
- Modules: `term.rs` (terms, levels, sorts; 857), `typing.rs` (3,997), `conversion.rs` (reduction and
  conversion, 241), `substitution.rs`, `signature.rs`, `certificate.rs` (1,396), `indexed.rs`, `quotient.rs`
  (1,297), `scheme_dsl.rs` (public module), `theorems.rs`, `tests.rs` (5,282).
- Tests: `tests.rs` with 88 `#[test]` (grepped); no separate test target.
- Open work: `PROOF-KERNEL-CORE` (TASKS.md:289), covering PROOF-CONTRACT-MIGRATION, QUOTIENT-THEOREM-LIFT and
  PSI-CLAIM-ADMISSION.

### 5. proof-admission

- Path `S/proofs/bounded` (lib `bounded.rs`, lib name `proof_admission`); 39 files, 47,437 lines.
- Purpose: admits a supplied certificate against a program obligation, its premises and its authority. It
  re-decides each judgment from the evidence, runs no proof search, and walks certificates with an explicit work list.
  For rule families that have a translation, it denotes the bounded certificate into mathematical-kernel terms and
  lets the kernel judge it; otherwise the acceptance records `Refused` naming the family (README lines 38-44 of
  `proofs/bounded/README.md`). Integer rules (affine, cast, shift, forbidden-root witnesses) replay the witness
  coordinates themselves.
- Consumes `Obligation` (`bounded.rs:35`), `CertificateEnvelope`/`AdmissionEvidence` (from terminal-psi) and
  `Proposition`; produces `AcceptedFact`/`AcceptedFactRoute` (`bounded.rs:49-60`), `CertificateAcceptance`,
  `BoundedDenotation`.
- Entry points: `verify_obligation` (`bounded.rs:65`), `verify_obligation_with_machine_parameters` (`:82`),
  `check_certificate` (`proof.rs:163`), `accept_certificate` (`:173`), `accept_certificate_with_machine_parameters`
  (`:190`), `decide_primitive` (`kernel.rs:18`), `denote_bounded_certificate` (`bounded_denotation.rs:14413`),
  `verify_bounded_certificate` (`:14456`), `check_predicate_denotations` (`predicate_denotation.rs`),
  `check_float_integer_conversion` (`float_integer.rs`), `check_float_meaning_normalization` (`float_meaning.rs`).
  It also re-exports closed-predicates and the mathematical-kernel surface (`bounded.rs:397-452`).
- Production dependencies: closed-predicates, mathematical-kernel, bignum, record-storage, arena, terminal-psi,
  semantic-vocabulary, numerics. Dependents: 3 manifests: terminal-semantics, source-to-psi stage 06
  (`06_lowered-program-to-checked-program`), and source-to-psi root (dev-dependency only).
- Modules: `bounded.rs` (module root, evidence/admission types, `term_matchers`), `proof.rs` (5,117; rule set and
  traversal), `bounded_denotation.rs` (17,334; the largest file in the area), `integer_affine.rs` (7,192),
  `integer_rules.rs` (3,386: closed_integer, integer_cast, forbidden_root, integer_shift), `predicate_denotation.rs`,
  `kernel.rs` (primitive judgments), `classicality.rs`, `float_integer.rs`, `float_meaning.rs`,
  `ieee_float_rules.rs`.
- Tests: one integration binary `suite` (`Cargo.toml` `[[test]]`, `autotests = false`; `tests/suite.rs` lists 21
  topic modules, 69 `#[test]` in them) and 289 inline tests outside `tests/` (grepped; `bounded_denotation.rs` 122,
  `proof.rs` 44, the rest in the float and IEEE modules).
- Open work: `PROOF-KERNEL-CORE` (TASKS.md:289). No OWNER_CODE_CONCERNS entry names it.

### 6. terminal-work

- Path `S/work` (`src/lib.rs`); 2 files, 796 lines.
- Purpose: the target-neutral work schedule. `WorkSchedule` assigns units to each operation kind and terminator;
  `WorkMeter` charges executed operations and taken edges against a local quantum and an optional external
  `WorkPayment`, records per-site `WorkUsage`, and leaves state unchanged when a charge is refused.
- Consumes `terminal_psi::{Operation, OperationKind, Terminator}`; produces `WorkUsage` (`lib.rs:167`),
  `WorkMeterError` (`:409`), `WorkBudgetExhausted` (`:386`).
- Entry points: `WorkSchedule::CURRENT` (`lib.rs:20`), `operation_units` (`:32`), `terminator_units` (`:128`),
  `WorkMeter::{unbounded, with_budget, with_payment, charge_operation, charge_terminator, charge_edge, replenish}`
  (`:219-295`), trait `ExecutionMeter` (`:361`, sealed).
- Production dependencies: record-storage, semantic-vocabulary, terminal-psi. Dependents: 14 manifests
  (13 production, 1 dev-only: provider-admission): build/evaluation (4), psi-to-native stages 01 and 03,
  optimization-unit-semantics, source-to-psi (root, stage 06, evaluation-receipts), interpreter and its host,
  package-manager (grepped). terminal-semantics does not depend on it, although `S/README.md` lists it under the
  semantics overview.
- Tests: `tests/meter.rs`, 5 `#[test]`.
- Open work: none found.

### 7. terminal-semantics

- Path `S` (lib `terminal_semantics`, `src/lib.rs`); 231 files, 114,411 lines. Heaviest crate: `descriptors/` is
  35,387 lines (31% of it); `references/` 9,712; `structural/` 7,714; `storage/` 5,446; `calls/` 4,752.
- Purpose: the shared judgments over Terminal Psi, in one flat crate: operation meaning, call composition and
  refinement, reference origins and completion, descriptor result and lifetime replay, plain-affine ownership
  and claims, scalar and structural qualifications (domains in a value's type), storage regions and extents,
  byte-sequence predicates, content transformations, continuation (parked-entry switching), crash exclusion for
  compiler-evaluation entries, and callable-value tracking.
- Consumes `TerminalModule`, `TerminalMachine`, catalogs and `StructuralTypeDeclaration` from terminal-psi
  (e.g. `plain_affine.rs:987` `plan(&TerminalMachine, &[StructuralTypeDeclaration])`); produces `Plan`
  (`plain_affine.rs:100`), `DescriptorResultContents`/`DescriptorStoredEffect` (`descriptors/replay.rs:30,156`),
  `ReferenceCallOrigin` and related rows, `OperationSemanticRow` (`operations.rs`), `CheckedPrefix`
  (`continuation.rs:30`), and Ok/Err validations.
- Entry points (all re-exported from `src/lib.rs`, which has about 330 lines of `pub use`):
  `operation_semantic_row` (`operations.rs:309`); `validate_reference_result_completion`
  (`references/reference_completion.rs:154`); `validate_access_lifecycle_plans`
  (`access/access_lifecycles.rs:15`); `validate_call_composition_semantic_rows`
  (`calls/call_composition.rs:321`); `validate_proof_bearing_scalar_semantic_rows`
  (`scalars/proof_bearing_scalar.rs:216`); `validate_boundary_normal_memberships`
  (`membership_facts.rs:1142`); `validate_inline_carriers` (`structural/inline_carriers.rs:50`);
  `validate_structural_store_values` (`structural/structural_store.rs:57`); `target_scalar_ranges`
  (`scalar_target_ranges.rs:56`); `closed_owned_sequence_bounds` (`owned_sequence_bounds.rs:4`);
  `plain_affine::{plan, plan_in, plan_reference_ends, verify, verify_in, plan_claims_in, verify_claims_in}`
  (`plain_affine.rs:987-1084`); `reconstruct_source_origins`, `reconstruct_stored_effects`, `descriptor_slots`
  (`descriptors/replay.rs:205,214,772`); `result_origins`, `return_origins`, `call_origins`
  (`descriptors/descriptor_results.rs:306,323,408`); `borrowed_address_liveness` (`plain_affine.rs:877`).
- Production dependencies (22 path deps in Cargo.toml): record-storage, closed-predicates, graphs,
  service-surface, source-provenance, access-vocabulary, schema-reflection, application-identity,
  layout-planning, resource-bounds, numeric-literals, arena, source-text, bignum, wire, language-core,
  layout-plans, access-admission, semantic-vocabulary, numerics, terminal-psi, proof-admission, and
  proof-applications. Dependents: 38 manifests (2 dev-only: artifact/authorization, native execution-scope),
  including the application, build (4), psi artifact/execution-inputs (5), psi-to-native stages 00-05, 09, 15 and
  representations/verification (about 15), source-to-psi (root, stages 05, 06, 07, checked-program) and
  interpreter plus host (grepped `terminal-semantics =` lines across manifests).
- Module layout (line counts from `wc -l`):
  - `descriptors/` (35,387): `replay/` engine (13,501; `engine.rs` 4,548), `descriptor_results/` (15,926:
    terminal_flow, payload_history, contents, scalar_captures), `descriptor_lifetimes/`, selected and
    presented memberships. `descriptors.rs` is a 14-line root.
  - `references/` (9,712): origins, completion, storage geometry, views, declared and boundary results.
  - `structural/` (7,714): `primitive_place.rs`, `structural_effect/`, owned/zeroed structure, inline carriers,
    nominal cleanup, shared mutation.
  - `storage/` (5,446): activation storage flow, borrowed and arrival storage, calling storage, type graph.
  - `calls/` (4,752): call composition, callable refinement, closed conformances, closed reach, evidence calls.
  - `scalars/` (3,956), `bytes/` (3,150), `content_transformations/` (3,342), `access/` (2,559).
  - top-level files: `plain_affine.rs` (4,396 with `plain_affine/` for claims and tests), `structural_qualifications.rs`
    (3,035), `membership_facts.rs` (2,542), `scalar_qualifications.rs` (1,629), `inhabitation.rs` (1,400),
    `callable_values.rs` (1,338), `continuation.rs` (907), `normal_completion.rs`, `substitution.rs`,
    `suspension_frontier.rs`, `reference_ends.rs`, `static_reach.rs`, `control_graph.rs`, `operations.rs`,
    `logical_providers.rs`, `provider_candidates.rs`, `synchronous_calls.rs`, `evaluation_crashes.rs`,
    `evaluation_scope.rs`, `body_services.rs`.
- Tests: 289 inline `#[test]` in `src` (grepped; 61 `#[cfg(test)]` blocks), largest in
  `descriptors/replay/engine/snapshot_tests.rs` (3,176), `descriptor_results/scalar_captures/tests.rs` (2,707)
  and `plain_affine/tests.rs` (1,998); one integration target `tests/checked_continuation.rs` (1,160 lines, 18
  tests) with `tests/checked_continuation/record_fields.rs`.
- Open work: TASKS.md:164 (boundary-parent closure and `invokes`), TASKS.md:194 (invocation-sensitive crash
  admission, which `evaluation_crashes.rs` serves), TASKS.md:221 (scalar reference recasts, uses representation
  views). OWNER_CODE_CONCERNS.md:81 "A resumed Build refuses itself whenever it is typed again, because region
  owners print as symbol slots" names `continuation/declarations.rs:208` `types_match`.

## How data flows through the area

1. A producer (source-to-psi stage 06, or a consumer reading a saved artifact) holds a `TerminalModule` and calls
   judgments in terminal-semantics: `operation_semantic_row` (`operations.rs:309`) fixes each operation's meaning.
2. Call and reference questions go through `calls/` (`validate_call_composition_semantic_rows`) and `references/`
   (`reference_origins`, `validate_reference_result_completion`), which use `descriptors/replay` (the replay engine,
   `reconstruct_source_origins`) for descriptor contents and lifetimes.
3. Ownership and loans: `plain_affine::plan`/`verify` (`plain_affine.rs:987,1030`) produce and re-check the edge
   schedule; `reference_ends.rs` and `descriptors/descriptor_lifetimes` decide disposal.
4. Storage: `storage/` and `references/reference_storage_geometry.rs` compute regions and extents;
   `reference_storage_geometry.rs:80` asks layout-planning for integer geometry, and `storage/activation_storage/flow.rs:1128`
   asks resource-bounds to compose activation claim bounds.
5. Value facts: `scalar_qualifications`, `structural_qualifications`, `membership_facts`, `inhabitation` decide
   domain facts. When a predicate is closed, `closed-predicates` (`closed_guard`) evaluates it.
6. Proofs: where a goal needs a certificate, terminal-semantics calls `proof_admission::verify_obligation`
   (`calls/callable_refinement.rs:757`). `bounded_denotation.rs:14413` translates a bounded certificate into
   mathematical-kernel terms; `mathematical-kernel::verify_mathematical_certificate` (`certificate.rs:658`) judges them.
7. Execution cost: the interpreter and build evaluation call `terminal-work` `WorkMeter::charge_operation`
   and `charge_edge` while running.

## Where this area's map differs from the documents

- `S/README.md` lists "Work accounting" among semantics pieces; the code has no dependency from terminal-semantics
  on terminal-work (Cargo.toml has no `terminal-work`; `terminal_work` appears in 0 files of `S/src` or `S/tests`,
  grepped). It is a sibling crate called by the interpreter, build and native crates.
- `S/README.md` says "[Proof checking](proofs/README.md) checks explicit certificates"; `proofs/README.md` exists and
  matches, but terminal-semantics also depends on `proof-applications` (`../proofs/applications`, outside this
  area; only 2 files in `S/src` use it) which README does not mention.
- `proofs/bounded/README.md` says the checker and interpreter share the integer evaluator "at
  `../../closed-predicates/integer.rs`"; the code matches. The same README names `integer_cast`, `integer_shift`,
  `integer_forbidden_root` as living in `integer_rules.rs`; they do (`integer_rules.rs:75,1041,1964`), but
  `integer_affine.rs` is a separate file and module in `bounded.rs:7`, as the README links it.
- `S/src/lib.rs:5` says the operation catalog is in `operations.rs`; true (386 lines), declared `pub(crate) mod`
  so only its re-exports are public.
- `S/README.md` calls terminal-semantics "shared judgments (src/lib.rs)" without saying it holds about 114 k lines,
  of which descriptors alone are about 35 k; no README describes the `descriptors/` or `references/` subtrees.

## Not read / unsure

- Not read line by line: all of `descriptors/`, `plain_affine.rs`, `structural_*`, `calls/`, `bounded_denotation.rs`
  (17 k lines), `proof.rs`, `integer_affine.rs`, `typing.rs`. Module purposes for these come from file names, first
  doc comments, `pub` signatures and `lib.rs` re-exports (inferred).
- Dependent counts come from grepping `^<crate> =` in Cargo.toml files under `source-rust/` (and `omega-rust-v2/`
  for completeness, none found there); a crate renamed on import or an optional dependency could be missed. I did
  not check which dependents use it in `[build-dependencies]`.
- The figure 289 is the `#[test]` count in two unrelated places (terminal-semantics `src`, and proof-admission
  outside its `tests/` directory, mostly `bounded_denotation.rs` 122 and `proof.rs` 44). I re-ran both greps; the match is a coincidence.
- `proof-applications` (`psi/proofs/applications`) and `terminal-psi` are outside this area and not mapped here.
- I did not verify spec pages under `wiki/spec/proofs/` or `wiki/spec/terminal-psi/` beyond the links READMEs cite.
