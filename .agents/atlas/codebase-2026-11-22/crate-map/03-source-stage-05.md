# Crate map: 03-source-stage-05

Root for every path below: `C:/Development/Omega/.agents/worktree/worker-2/`. The crate root is
`source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/`,
written `<05>/` below.

## 1. What the area does

The area is one crate, stage 05 of the source compiler. It takes the `PreparedTypedTrees` that stage 04 issued
(`source-rust/subsystems/compiler/source-to-psi/representations/prepared-source/lib.rs:32`) plus a
`Compilation` that names the roots to judge (`.../representations/lowered-source-publication/compilation.rs:14`).
It reads each machine's typed trees into a closed body, works out each machine's outcomes and call cycles, and
lowers every body into a Terminal Psi module and its check tables (`MachineTables`, `CheckTables`,
`Elaboration`, all in `.../representations/lowered-program/`). It proves nothing: it records every proof
obligation the lowered form implies and leaves them to stage 06. The product is a `LoweredSourcePublication`
(`.../representations/lowered-source-publication/lowered_source_publication.rs:30`), which binds the lowered
program to its source owner and its source correspondence.

## 2. Crate: `prepared-typed-trees-to-lowered-program`

- Path: `<05>/`. 114 files, 97,047 lines (`.agents/atlas/codebase-2026-11-22/crates.tsv`). A `wc -l` over
  `*.rs` in the crate gives the same 97,047 (ran it). `README.md` alone is 1,681 lines.
- Purpose: reads typed trees, fixes every identity (slot, type, field position, callee `MachineId`), orders
  machines by call cycles, and lowers each body to blocks, values, operations, edges, invocations and
  obligations. It also prepares what stage 06 needs while typed source is still available: generic forms,
  conformance witnesses, provider selections, service reach, calling-policy rows and source correspondence
  (`<05>/src/compilation.rs:1-3`, `<05>/README.md:1-55`).
- Consumes: `PreparedTypedTrees` (`representations/prepared-source/lib.rs:32`) and `Compilation<'_>`
  (`representations/lowered-source-publication/compilation.rs:14`). Internally it reads the frozen
  `TypedTrees` that `PreparedTypedTrees::trees()` lends.
- Produces: `LoweredCompilationWithSource { program: TerminalCompilation, source: SourceCorrespondence }`
  (`<05>/src/compilation.rs:21`), then `LoweredSourcePublication` (via `bind_source`, `<05>/src/compilation.rs:56`).
  `TerminalCompilation` is at `representations/lowered-program/lowered_program.rs:18`; `Elaboration` at
  `representations/lowered-program/program.rs:188`; `MachineTables` at `representations/lowered-program/tables/mod.rs:333`.
  Under tests only, the internal product is `Checked<Elaboration>`.

### Entry points

- `lower_for_publication(owner, compilation, publication)`: `<05>/src/compilation.rs:45`. Called by the driver
  at `source-to-psi/driver/judge_lowered_source.rs:114` (grepped).
- `source_roster::lower_roster(roster, typed)`: `<05>/src/source_roster.rs:14`. Product path; called at
  `driver/judge_lowered_source.rs:116`. Retains the roster in the result.
- `prepare_compilation(typed, compilation, publication)`: `<05>/src/compilation.rs:83`. Does the work both of
  the above wrap; public in all builds.
- `check_bound_callable_shapes`: re-exported from `<05>/src/callable_formations.rs` (`<05>/src/lib.rs`, pub use list);
  called first by `prepare_compilation`.
- `selection_questions::capture`, `capture_with_bodies`, `declaration_custody`, `SelectionCaptureError`:
  `<05>/src/selection_questions.rs:1199, 1220, 1352, 226` (public module). Captures source questions for
  primitive operations without granting permission.
- `provider_selections::publish`: `<05>/src/provider_selections.rs:33` (public module).
- Test-only (gated `cfg(any(test, feature = "test-support"))`): `elaborate` `<05>/src/lib.rs:339`,
  `elaborate_for` `:351`, `elaborate_closure` `:497`, `elaborate_generic` `:533`, `elaborate_alternatives` `:556`,
  `elaborate_witness` `:768`, `elaborate_callable_witness` `:839`, `lower_bodies` `:1422`. `elaborate_generic_forms`
  (`:608`) and `lower_generic` (`:1446`) are `pub` without the gate.
- Refusals: `refuse(machine, Construct, Unserved)` `<05>/src/lib.rs:308` (reason `Unsupported`) and
  `reject(machine, SourceOrigin, rule)` `:322` (reason `Violation`). `Unserved` is the closed family list at `:245`.

### Dependencies

- Production dependencies in the repo (from `<05>/Cargo.toml`, 26 path dependencies, counted with awk): `prepared-source`,
  `lowered-source-publication`, `lowered-source-publication-issuer`, `service-surface`, `proof-applications`,
  `source-declarations`, `source-symbol-table`, `typed-trees`, `source-provenance`, `schema-reflection`,
  `application-identity`, `numeric-literals`, `source-admission`, `source-text`, `bignum`, `wire`, `arena`,
  `diagnostics`, `language-core`, `numerics`, `semantic-vocabulary`, `symbols`, `terminal-check-tables` (the
  `representations/lowered-program` crate), `terminal-codec`, `terminal-psi`, `terminal-semantics`.
- One feature, `test-support` (no dependencies). `autotests = false`.
- Dependents (grepped the three manifests that name it): 3 crates. `source-to-psi` (`source-to-psi/Cargo.toml:43`,
  production, and again in dev-dependencies `:89` with `test-support`); stage 06
  `pipeline/06_lowered-program-to-checked-program/Cargo.toml:54` and stage 07
  `pipeline/07_checked-program-to-psi/Cargo.toml:58`, both dev-dependencies only with `test-support`. So stage 06
  and 07 use it in their tests only (`#[cfg(test)] use ...lowering` at `06/lib.rs:18`, `07/lib.rs:30`). Rust code
  names it as `prepared_typed_trees_to_lowered_program::lowering` because the root `lib.rs` is 10 lines and
  mounts `src/lib.rs` as `pub mod lowering` (`<05>/lib.rs:9-10`).
- `source-to-psi/representations/publication.rs:14` re-exports `RequirementInvocation` and `RequirementService`
  from it.

### Module layout (`<05>/src/`)

Reader (typed trees to `body.rs` closed body):
- `body.rs` (1,471): the closed body: `Body`, `Statement`, `Expression`, `ExpressionKind`, `Signature`, `RecordType`,
  `ValueType`, `DomainSet`, `Ranking`, `Termination`.
- `read.rs` (3,296): shared `Program` (`:156`) and slots; header comment states the identity rules.
- `read/` (about 50 files): `machine.rs` (4,968; admission gate, signatures, body driver), `calls.rs` (4,577),
  `contracts.rs`, `expressions.rs`, `places.rs`, `types.rs`, `source_records.rs`, `services.rs`, `calling.rs`,
  `calling_abis.rs`, `logical.rs`, `bytes.rs`, `arrays.rs`, `traits.rs`, `subjects.rs`, `record_parameters.rs`,
  `references.rs`, `regions.rs` (and `regions/result_lifetimes.rs`), `mathematics.rs`, `predicates.rs`, `casts.rs`,
  `float_meaning.rs`, `atomics.rs`, `statements.rs`, `transitions.rs`, `rejections.rs` (shared receiving-type
  diagnostics), `providers.rs`, `provider_methods.rs`, `provider_selections.rs`, `reflection.rs`, `carriers.rs`,
  `rankings.rs`, `authority.rs`, `review_requirements.rs`, `constant_values.rs`, `nominal_applications.rs`, `applications.rs`.

Whole-compilation preparation:
- `compilation.rs` (740): `prepare_compilation` and `bind_source`.
- `lib.rs` (2,232): `elaborate_source_for` (`:369`), `lower_generic_for` (`:1472`), `Unserved`, public re-exports; its own
  `mod tests`, `mod publication_scope_tests`.
- `declare.rs` (2,983) and `declare_selected.rs`: declaration, boundary, provider and specialization rows
  (`close_specializations` `declare.rs:1519`).
- `callables.rs`, `callable_formations.rs`, `callable_headers.rs`, `boundaries.rs`, `external_providers.rs`,
  `static_contracts.rs`, `provider_selections.rs`, `calling_publication.rs`, `claim_publication.rs`, `publication.rs`
  (`PreparedProgram`, `finalize_source`, `SourceOrigin`, `SourceCorrespondence`).
- `reach.rs`: one service namespace and each call's reach. `qualifications.rs`: domain/qualification rows.
  `proof_scope.rs`: actual declaration coordinates for published rows. `policy.rs`: arithmetic suffix policies.
  `operational.rs`, `suspension.rs`: suspension acknowledgements and crossings. `inhabitation.rs`,
  `default_obligations.rs`, `reference_ends.rs`, `reference_results.rs`: reference lifetime and default obligations.
- `outcome.rs` (903): per-machine "can complete normally" and crash routes, callees first (`outcomes` `:106`).
- `components.rs`, `components_graph.rs`, `components_rankings.rs`: strongly connected call components lowered as
  loops, plus ranking edges (`discover` `components.rs:87`, `components` `:130`).
- `canonical.rs`: the one route to canonical Terminal Psi propositions.

Lowering (closed body to Terminal Psi + tables):
- `lower.rs` (1,526): `Lowering` state; per-machine entry `machine` (`:130`), `signature_header` (`:195`),
  `declaration_contract` (`:400`).
- `lower/`: `places.rs` (5,669, largest file: roots, projections, write log, record/case construction, copies, dispatch
  terminator), `places_contents.rs`, `control.rs`, `operations.rs`, `calls.rs`, `ranking.rs`, `boundaries.rs`.

Tests inside `src/`: `tests.rs` (4,625), `tests/loops.rs`, `tests/array_views.rs`, `selection_layout_query_tests.rs`,
`read/calls_visibility_tests.rs`, `read/source_records/opaque/tests.rs`.

### Tests

- Unit tests: 209 `#[test]` under `src/` (ran `grep -rc '#[test]' src`, summed); 64 in `src/tests.rs`, 10 in
  `src/tests/loops.rs`, 2 in `src/tests/array_views.rs`. They state lowering bodies directly through `lower_bodies`
  and check the tables.
- Integration tests: 9 files under `<05>/tests/` with 35 `#[test]` (grepped): `block_values`, `positional_field_reads`,
  `prepared_array_extents`, `prepared_boolean_patterns`, `prepared_scalar_values`, `selected_calls`,
  `selected_operations`, `subjectless_match`, `variant_parent_identity`; helpers `tests/support/{dispatch_arms,sum_dispatch}.rs`.
  These are NOT declared in this crate's `Cargo.toml` (`autotests = false`, no `[[test]]`). They are declared in the
  parent `source-to-psi/Cargo.toml:142-182` as `lowering_*` targets with `required-features = ["test-support"]`.
- Doc tests: two `compile_fail` examples in `<05>/src/compilation.rs` (a snapshot not issued by stage 04 is
  not accepted; lowering cannot issue its own input).
- Stage 06 and 07 test code also drives this crate through `elaborate_for` etc. (grepped 06 tests).

### Open work

`TASKS.md` items (grepped for this crate's path), by title:
- **LOWERED-FORM-CHECKER** (`TASKS.md:13`), sub-bullets at `:25` (block-valued arm assigning an outer scalar local;
  reader `read/statements.rs`) and `:27` (complementary equality/inequality guards closing a match with no `_`;
  reader `read/expressions.rs`).
- **OWNED-CLEANUP** (`:171`), bullet `:172` (sum dying unmatched; refused in lowering "component disposition needs an established active case").
- **EARLY-TERMINAL-JOBS** (`:190`), bullet `:195` (static callback from a generic machine's own nested declarations;
  `read/calls/callback_bindings.rs`).
- **STATE-LOCAL-VALUE-FRONTIER** (`:220`), bullet `:222` (a `match` joining references with no named lifetime; `read/expressions.rs`).
- **MODULE-NAMESPACE-RESOLUTION** (`:224`), bullets `:225`, `:227` (generic owned-value call retires its temporary; file import example refused in lowering).
- Broader sections "Lowering, checker, and data-flow" (`:111`) and **DOMAIN-ACCESS-AUTHORITY** (`:230`, says lowering already refuses `x in D` on a carrier mismatch).

`OWNER_CODE_CONCERNS.md`: no entry names this crate in its title. One entry, "A pattern's field reads get their
fields in typing and its roster gets them again in preparation" (`:86`), says lowering checks coverage on the roster and
reads through the member, so it touches this crate's reader indirectly (inferred).

## 3. How data flows through the area

1. Driver `source-to-psi/driver/judge_lowered_source.rs:114/116` receives `PreparedTypedTrees` and either a
   `Compilation` (`lower_for_publication`) or a `SourceCompilationRoster` (`lower_roster`).
2. `lower_roster` borrows the roster's roots and calls `prepare_compilation` (`<05>/src/source_roster.rs:14`).
3. `prepare_compilation` (`compilation.rs:83`) first runs `check_bound_callable_shapes`; a formation failure returns
   `TerminalCompilation::FormationFailures` or `FormationDefect` with no further reading.
4. It builds the shared reading (`shared_reading`, `lib.rs`), resolves each bound arrival's requirement, and calls
   `elaborate_source_for` (`lib.rs:369`) with a `read::Program`.
5. The reader (`read.rs`, `read/machine.rs`, family modules) turns each machine's typed trees into `body::Body` and
   `body::Signature` rows, pulling in callees, selected machines, subject declarations and generic forms.
6. `declare.rs` / `reach.rs` / `qualifications.rs` / `proof_scope.rs` publish declaration, reach, domain and provider
   rows; `components.rs` finds call cycles; `outcome.rs::outcomes` computes outcomes callees first.
7. `lower_generic_for` (`lib.rs:1472`) calls `lower::machine` (`lower.rs:130`) per machine, minting blocks, values,
   edges, obligations and places in `MachineTables` while building the `TerminalModule`.
8. `publication::finalize_source` produces `Elaboration` and `SourceCorrespondence` (`publication.rs:104`).
9. `bind_source` (`compilation.rs:56`) wraps program and correspondence into `LoweredSourcePublication`, issued with the
   `LOWERING` capability; stage 06 (`06_.../src/publication.rs`) reads it.

## 4. Where the map differs from the documents

- `<05>/README.md:56-127` ("The subset") says the accepted machines use only builtin integers or `bool` and that
  "a reference outside References, a slice, a generic, a callable value, a suspension, ... a domain, ... or service
  reach is never skipped", i.e. refused. The same README then has sections "Service reach" (`:128`), "Generic
  machines" (`:268`), "References" (`:794`), "Loops" (`:573`), and the code has `elaborate_generic`,
  `elaborate_generic_forms`, `read/services.rs`, `reach.rs`, `read/references.rs`, `read/arrays.rs`, `read/traits.rs`,
  `suspension.rs`. The README opening (`:1-10`, "scalar code and plain data") and the subset list are out of date
  against the code; its own later sections contradict them.
- `<05>/src/lib.rs:2-9` and `:41-46` ("route one's scalar subset and plain records") and `Unserved::Type`'s doc at
  `lib.rs:254` ("not a builtin integer, `bool` or a plain record") describe the same narrow subset; the
  module list shows far more.
- `<05>/README.md` says stage 05 "fills the check tables that stage 06 reads" and the AGENTS.md pipeline text agrees;
  consistent. But the README file list (`:36-54`) names only `machine`, `types`, `contracts`, `statements`,
  `expressions`, `calls`, `transitions`, `places`, `subjects` among the reader families; the directory holds about 50
  reader modules.
- `AGENTS.md` says tests of a crate live under its `tests/`; here the integration targets are registered in the parent
  crate's manifest, not this crate's (`source-to-psi/Cargo.toml:142-182`). Not a contradiction, but a reader of
  `<05>/Cargo.toml` will not find them.
- `AGENTS.md` data-oriented rule discourages `HashMap`/`BTreeMap` keyed by dense ids; the crate uses
  `BTreeMap` at `<05>/src/lower/places.rs:833` and `HashMap` at `<05>/src/read/traits.rs:39` (grepped; I did not check
  their key types against `tools/ratchets/dense_id_maps.txt`).

## 5. Not read, and unsure

- Not read line by line: nearly every file. I read `Cargo.toml`, root and `src/lib.rs` (head, `Unserved`, entry
  points), `compilation.rs` (head), `source_roster.rs`, the header comments of every `src/*.rs`, `read.rs`, `lower.rs`
  and `read/machine.rs` headers, and the README's headings, "The subset", and "Findings" 1-13 (the README's
  Findings run past 13; I did not read the rest, nor sections `:128-1440`).
- Not read: `read/` family modules beyond their names, `lower/` modules, `body.rs` bodies, all tests' contents, the
  spec pages the code cites, `wiki/drafts/designs/check_on_lowered_form.md`.
- Unsure: the exact call order inside `elaborate_source_for` (I saw its start and a few call sites by grep, not the
  whole function); whether `source-to-psi` uses `selection_questions` and `provider_selections` from this crate in
  production (the grep for `selection_questions::` hit `psi_authorization`, not this crate);
  which `representations/` crates own `SourceCorrespondence` (it is re-exported from `publication.rs` here, I did not
  open its definition).
