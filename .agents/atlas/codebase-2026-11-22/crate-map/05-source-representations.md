# Crate map: 05-source-representations

Checkout `4f3c255d0559` (main, 2026-11-22). Paths are relative to the checkout. All crates sit under
`source-rust/subsystems/compiler/source-to-psi/representations/` (written `R/` below). Every crate in the area has an
entry in `crates.tsv` (20 rows; counted with awk on the `area` column), and `units.tsv` has 210 units for the area.

"Depended on by" counts come from `grep -rlE --include=Cargo.toml "^<name> *= *\{.*path"` over the repository
(excluding `_recycling/` and `.agents/`). The count is of manifests, so it includes dev-dependencies and the other
crates of this area. The manifest `source-rust/subsystems/compiler/source-to-psi` (the driver crate) is written `driver`.

## 1. What the area does

These are the data forms that the source half of the compiler passes from stage to stage, with no pipeline logic in
them: source files, tokens, syntax trees, symbol-resolved trees (plus the symbol table), typed trees, prepared typed
trees, the lowered program, the checked program, and the records that sit beside them (lowered-source publication,
provider choices retained from a dependency's Build, provider demand published for one target, evaluation accounting).
`R/README.md` says so and lists producer and next consumer for each form. Input is a batch of captured source bytes
(`LoadedSources`, `R/source-files/source_files.rs:37`); output is `CheckedProgram` (`R/checked-program/checked_program.rs:18`),
which stage 07 turns into a Psi artifact. Three products (prepared typed trees, lowered source publication, checked
program) can be built only through a capability crate ("issuer") that only the producing stage links; the
`SOURCE_PRODUCT_ISSUERS` rule in `tools/xtask/src/boundary/linear_pipeline.rs:112` enforces that (read).

Size note (from `crates.tsv`): `typed-trees` is 92,647 lines of the area's roughly 140,000; `syntax-trees` (12.9k),
`symbol-resolved-trees` (11.4k) and `source-symbol-table` (9.7k) are the next largest. `typed-trees` holds substantial
logic (type identity, callable application, operator and service declarations), not only row types.

## 2. Crates, in pipeline order

### source-files (`R/source-files`)
- Files and lines: 1 file, 62 lines (`crates.tsv`).
- Purpose: the captured source bytes handed to the lexer, with caller-assigned `SourceId`, path, origin and dependency scope.
  No parsing.
- Produces: `LoadedSource`/`LoadedSources` (`source_files.rs:30`, `:37`), `SourceOrigin` (`:11`; user, toolchain, toolchain contract).
- Entry points: none beyond the types; `LoadedSources::from_iter` (`source_files.rs:55`).
- Depends on: `source-provenance`, `arena`, `record-storage`, `source-text`.
- Depended on by (6): driver, pipeline 00, pipeline 01, `source-admission`, `syntax-trees`, `tokens`.
- Modules: `source_files.rs` only; `bundled-declarations/` and `custody/` are separate crates below.
- Tests: none in the crate (grepped `#[test]`).
- Open work: none named in TASKS.md or OWNER_CODE_CONCERNS.md (grepped).

### bundled-declarations (`R/source-files/bundled-declarations`)
- Files and lines: 4 files, 576 lines.
- Purpose: embeds the bytes of the bundled `source/library` files (`include_bytes!` against `../../../../../../../source/library/`, `src/lib.rs:11`)
  so declarations can be recognized as sealed intrinsics or core vocabulary after the source is admitted as toolchain-owned.
- Consumes: `language_core::bundled_files::BundledFile`. Produces: `SealedIntrinsicCatalog::classify` (`src/sealed_intrinsics.rs:19`),
  `CoreVocabularyCatalog::classify`/`source` (`src/core_vocabulary.rs:10`, `:16`), `float_operations_source`, `hosted_process_exit_sources`.
- Depends on: `language-core` (dev: `source-provenance`). Depended on by (5): pipeline 02, 06, 07, `source-symbol-table`, `typed-trees` (dev).
- Modules: `core_vocabulary.rs` (239), `sealed_intrinsics.rs` (275).
- Tests: `tests/revisions.rs` plus inline tests in both modules (11 `#[test]`/`#[cfg(test)]` hits, grepped).
- Open work: none named.

### source-admission (`R/source-files/custody`)
- Files and lines: 2 files, 471 lines. The crate directory is `custody`, the crate name `source-admission`; it has no `[lib] path`, so the root is `src/lib.rs`.
- Purpose: the admitted form of a source (`SourceFile`, `SourceContents`) and inert declaration anchors (`RetainedDeclaration`) that grant
  neither lookup nor acceptance (`src/lib.rs:1-2`), plus the canonical source identity and digest functions (`source_identity` module, `src/lib.rs:184`).
- Produces: `SourceFile` (`:69`), `RetainedDeclaration` (`:139`), `SourceResolutionStratum` (`:44`), `GENERATED_SOURCE_DIRECTORY` (`:52`),
  `source_identity::{consumed_source_unit, canonical_source_entry, toolchain_source_identity_digest, ...}` (`:213`, `:366`, `:355`).
- Depends on: `source-files`, `source-provenance`, `source-text`, `record-storage`, `symbols`.
- Depended on by (19): the widest dependency in the area. Includes build, build/evaluation, build/evaluation/result, build/inputs, `compiler`, driver,
  pipelines 02 to 07, `terminal-check-tables`, `lowered-source-publication`, `retained-product-selections`, `source-declarations`, `source-symbol-table`, `typed-trees`, package-manager.
- Tests: `tests/identity_framing.rs`, 3 inline test hits in `src/lib.rs`.
- Open work: none named.

### tokens (`R/tokens`)
- Files and lines: 9 files, 762 lines (README at `R/tokens/README.md`).
- Purpose: ordered compact token records with their source backing and decoded string bytes, for one file; batches of them per source.
- Consumes: `LoadedSource` origin and scope types from `source-files`. Produces: `TokenStream<'source>` (`token_stream.rs:16`),
  `LexedSource`/`LexedSources` (`files/lexed.rs`), `Token` (`lexemes/token.rs`), `TokenKind`, `KeywordKind`, `PunctuationKind`.
- Entry points (used by the lexer): `TokenStream::with_capacity` (`:58`), `with_scratch` (`:67`), `push_source` (`:100`), `push_decoded` (`:112`),
  `retain_source` (`:130`), `spelling` (`:163`), `literal_bytes` (`:168`), `TokenScratch` (`:24`).
- Depends on: `source-files`, `source-text`, `arena`. Depended on by (10): build/evaluation, driver, pipelines 00, 01 (two crates), 02, 03, 06, 07, package-manager.
- Modules: `lib.rs` (4 lines), `token_stream.rs` (218, pulls the others in), `lexemes/{token,kinds,keywords,punctuation}.rs`, `files/lexed.rs`.
- Tests: `tests/storage.rs`, `tests/punctuation.rs`.
- Open work: none named.

### syntax-trees (`R/syntax-trees`)
- Files and lines: 45 files, 12,861 lines (README at `R/syntax-trees/README.md`).
- Purpose: the unresolved parse result. It keeps authored structure, retained source text and arena identities. It makes no claim that names, types or proofs are valid.
- Consumes: `LexedSources` indirectly (the parser builds it; this crate has no `tokens` dependency, only `source-files`). Produces: `SyntaxTrees`
  (`syntax_trees.rs:24`: `source_id`, `roots: SyntaxTreeRoots`, `tables: SyntaxTreeTables` with `text`, `items`, `expressions`, `statements`, `type_references`),
  `ParsedSource`/`ParsedSources`/`ParsedUnit` (`files/parsed.rs`), `ParsedSyntaxPages` (`files/pages.rs`), `SyntaxTreesSnapshot` (`inspection/snapshot.rs`, JSON via `to_json_pretty` at `:46`).
- Entry points (used by the parser and resolver): `SyntaxTrees::new`/`new_with_generation` (`syntax_trees.rs:90`, `:94`), `push_root_item` (`:106`),
  `append_checkpoint`/`rewind_appended` (`:57`, `:69`), `share_pages`/`adopt_page`/`begin_page` (`:83`, `:183`, `:194`), `extend_from` (`:163`), `root_items` (`:153`).
- Depends on: `source-files`, `syntax-trees-derive`, `arena`, `record-storage`, `source-text`, `diagnostics`, `language-core`, `numerics`, `numeric-literals`,
  `artifacts` (measurements), `serde`, `serde_json`. Depended on by (11): build/evaluation, build/inputs/declarations, driver, pipelines 01 (two crates), 02, 03, 06, 07, `symbol-resolved-trees`, package-manager.
- Modules: `declarations/` (items, machines, data, domains, conformances, capabilities, proofs, constants, parameters), `expressions/` (values, display, float spelling),
  `statements/` (control_flow, assembly), `types/` (references, preparation), `text/` (owner, identifiers), `files/` (parsed, pages), `copying/` (items, bodies, statement paths: copy between forests),
  `storage/` (equality, relocation, paged tables), `inspection/` (snapshot and expansions, about 2,800 lines). Modules are attached with `#[path]` in `syntax_trees.rs:314-367`.
- Tests: `tests/identifier_paths.rs`, `tests/page_roots.rs`, `tests/unit/*` (forest.rs is 1,373 lines; also expression, float spelling, snapshot, statement, text owner, types).
- Open work: TASKS.md **OPERATOR-MACHINE-SUPPLY** (line 89) removes the root `operator` declaration from the parser "and its dependent representation".

### syntax-trees-derive (`R/syntax-trees/derive`)
- Files and lines: 1 file, 136 lines. Proc-macro crate (`syn` 3, `quote`, `proc-macro2`).
- Purpose: derives structural comparison and page relocation for syntax nodes: `SyntaxEquivalent` (`src/lib.rs:21`), `PlainSyntaxEquivalent` (`:75`), `SyntaxRelocate` (`:90`).
- Depended on by (1): `syntax-trees`. Tests: none. Open work: none.

### symbol-resolved-trees (`R/symbol-resolved-trees`)
- Files and lines: 22 files, 11,402 lines (the two sub-crates below are counted separately). No README.
- Purpose: the forest between name binding and typing. It holds the resolved node tables, ordered roots, the symbol table that gives each resolved occurrence its identity, and service-reach and domain tables.
- Consumes: `SyntaxTrees` (via `syntax-trees`). Produces: `SymbolResolvedTrees` (`symbol_resolved_trees.rs:~75`: roots, tables, `symbols: SymbolTable`, service reaches, semantic domains, external bindings, evidence forwardings, generic binding overlaps),
  `SymbolResolvedRoots` (`roots.rs`), `SymbolResolvedTableStorage`, `AuthoredSelectionOccurrenceStore` (`selections.rs`).
- Entry point: `SymbolResolvedTrees::with_roots` (`symbol_resolved_trees.rs:109`); `pub use crate as source_resolved` (`:11`) is the name consumers import.
- Depends on: `syntax-trees`, `source-declarations`, `source-symbol-table`, `semantic-vocabulary`, `symbols`, `language-core`, `numerics`, `numeric-literals`, `source-text`, `wire`, `arena`, `record-storage`, `diagnostics`. Feature `test-support` (forwarded from the driver manifest).
- Depended on by (3): driver, pipelines 02 and 03.
- Modules: `nodes/` (values 2,615; control_flow_and_calls 1,711; type_system_and_names 1,239; declarations_and_machines; callable_values; state_bodies; evidence; `values/expression/{symbol_copies,table_copies}.rs`), `tables/` (storage, views, lookup_indexes), `continuations.rs`, `alias_bodies.rs`, `selections.rs`, `roots.rs`.
- Tests: `tests/authored_declaration_selection_ledger.rs`, `tests/resolved_records/mod.rs`; example `examples/diagnostic_name_allocations.rs`.
- Open work: none named at the crate level (TASKS.md names resolution-stage code in `pipeline/02`, not this crate).

### source-declarations (`R/symbol-resolved-trees/declarations`)
- Files and lines: 5 files, 3,015 lines.
- Purpose: authored declaration selections (which provider, operator or declaration the source chose) carried from resolution through typing and lowering; also domain roles/establishment routes and machine supply, ranking and termination contracts (`declarations.rs:1`, `domain_contracts.rs`, `machine_contracts.rs`).
  The mutable selection tables never cross into a product (`source_selections.rs:1-2`).
- Produces: `declaration_selection` module (`declaration_selection.rs`, 1,367 lines; includes `BuildSelectionOrigin`, `NativeBindingSource`, `product_operands::ProductStaticOperandGraph`), `source_selections` (1,409 lines), `DomainSemanticRoles`, `MachineTerminationPlan`, `RankingWitness`, etc.
- Depends on: `application-identity`, `arena`, `language-core`, `numeric-literals`, `numerics`, `record-storage`, `semantic-vocabulary`, `source-admission`, `source-text`, `symbols`.
- Depended on by (14): build, build/evaluation/result, build/inputs, driver, pipelines 02 to 07, `lowered-source-publication`, `retained-product-selections`, `symbol-resolved-trees`, `typed-trees`.
- Tests: 20 inline test hits (9 + 2 + 9, grepped). Open work: none named.

### source-symbol-table (`R/symbol-resolved-trees/symbol-table`)
- Files and lines: 10 files, 9,727 lines (4,512 of them in `source_symbols/table.rs`, 2,721 in `source_symbols/tests.rs`).
- Purpose: declaration lookup and the captured source catalogue. `SymbolTable` (`source_symbols/table.rs:2463`) indexes declarations against the source map and keeps exact source ownership across lookup and generated extensions;
  `SymbolTableBuilder` (`:2508`) and `SymbolTableExtension` (`:4440`) build and extend it. `source_inputs.rs` owns `SourceMap`/`SourceCatalogue` (activation, visibility, which sources are selected, rejoins, product dependency scope);
  `source_tables.rs` owns `ExternalBindingTable`, `SemanticDomainTable`, `ServiceReachRowTable`, `ServiceReachTable`.
- Entry points: `SourceCatalogue::admit_source_occurrence` (`source_inputs.rs:141`), `admit_checked_instance` (`:71`), `begin_selection` (`:61`), `fork` (`:55`), `from_files` (`:196`); `source_symbols::{retain_declaration, rejoin_declaration, sealed_intrinsic_at, core_vocabulary_at}` (`source_symbols.rs:12`, `:27`, `:42`).
- Depends on: `bundled-declarations`, `source-admission`, `source-provenance`, `source-text`, `symbols`, `language-core`, `numerics`, `arena`, `record-storage`, `semantic-vocabulary`.
- Depended on by (11): build/inputs, driver, pipelines 02 to 07, `retained-product-selections`, `symbol-resolved-trees`, `typed-trees`.
- Tests: `source_symbols/tests.rs`, `source_tables_tests.rs`, inline modules in `table.rs` (`:189`, `:2342`). Open work: none named.

### typed-trees (`R/typed-trees`)
- Files and lines: 84 files, 92,647 lines. No README. Tests are inside the source files (about 230 `#[test]` hits across 31 files, grepped), plus `examples/`.
- Purpose: the checked form of declarations and bodies after typing. One forest, `TypedTrees` (`typed_trees.rs:~15`), holds roots, node tables, the symbol table, retained type and callable identities, layout and access plans, wire plans, machine specializations, trait/data implementation applications, deferred refusals, constant recipes and declaration indexes.
  Also holds the algorithms that define type and callable identity and callable application, not only rows.
- Consumes: `SymbolResolvedTrees` (typing in pipeline 03 builds it; this crate does not depend on `symbol-resolved-trees`, only on `source-declarations`, `source-symbol-table`). Produces: `TypedTrees` and `FrozenTypedTrees` (`storage/frozen.rs`, with `boundary_signatures()` computed once and cached in a `OnceLock`),
  `ProductSourceIdentity` (`storage/frozen.rs:9`, a weak association to a frozen source), `ProgramIdentity` (process-unique counter, ignored by equality and by `Record` writes).
- Entry points: consumers import it as `source_state` (`pub use crate as source_state`, `typed_trees.rs:11`); names come from `exports.rs`. Large namespaces: `expression`, `types`, `data`, `domain`, `constant`, `machine`, `names`, `calls`, `control_flow`, `evidence`, `inspection`, `operator`, `reflection`, `declaration_index`, `type_identity`, `constant_recipes`.
- Depends on (production): `service-surface`, `access-vocabulary`, `access-admission`, `schema-reflection`, `application-identity`, `layout-plans`, `artifacts`, `numeric-literals`, `source-provenance`, `source-admission`, `source-declarations`, `source-symbol-table`, `semantic-vocabulary`, `symbols`, `language-core`, `numerics`, `bignum`, `wire`, `arena`, `record-storage`, `diagnostics`, `source-text`, `sha2`, `serde`, `serde_json`. Dev: `bundled-declarations`.
- Depended on by (10): build/evaluation/result, driver, pipelines 03 to 07, `source-completion-accounting`, `lowered-source-publication`, `prepared-source`.
- Module layout (line counts): `type_records/` (type_identity 11,561; type_system 4,405; data_applications; source_type_applications; opaque_representations), `callables/` (callable_applications 9,432; calls_and_control_flow 2,949; callable_signature_shapes; runtime_callable_identity; machine_instances; static_operand_binding/), `definitions/` (operator_declarations 4,530; service_declarations 3,727; declarations 3,364; declaration_reflection; product_operand/; trait_definition/), `expressions/` (values 6,816; references 5,263; access_views_and_commitments 1,780; canonical_values), `proofs/` (evidence_and_inspection 6,192; deferred_refusals; contract_structure), `storage/` (declaration_tables_and_index 5,972; frozen; forest; symbol_rebinding), `table_macros.rs`, `exports.rs`.
- Tests: inline; `examples/declaration_lookups.rs`, `examples/declaration_owner.rs`.
- Open work: OWNER_CODE_CONCERNS.md entry "A resumed Build refuses itself whenever it is typed again, because region owners print as symbol slots" (`encode_signature_region`, `type_records/type_identity.rs`); entry "A pattern's field reads get their fields in typing and its roster gets them again in preparation" names typing and stage 04, not this crate's files. TASKS.md **PROVIDER-SELECTION-AFTER-TERMINAL** and **EARLY-TERMINAL-JOBS** cite pipeline code that reads these types.

### prepared-source (`R/prepared-source`)
- Files and lines: 2 files, 300 lines. `autotests`, `autoexamples`, `autobenches` are off; the only tests are 5 doctest/inline hits in `lib.rs`.
- Purpose: the stage 04 product. An immutable typed source plus the computation inputs (`ValueInputs`, `TargetInputs`, `PreparedValue`, `PreparedTarget`, `values.rs`) whose source and constant handles the constructor checks. Preparation algorithms stay in stage 04.
- Consumes: `TypedTrees`. Produces: `PreparedTypedTrees` (`lib.rs:~33`) with `trees()` giving `Arc<FrozenTypedTrees>`.
- Entry point: `PreparedTypedTrees::new(PREPARATION, TypedTrees, &[ValueInputs])` (`lib.rs:~45`), callable only with the `PreparationIssuer`.
- Depends on: `prepared-source-issuer`, `typed-trees`, `layout-plans`, `arena`, `diagnostics`, `symbols`. Depended on by (4): pipelines 04, 05, 07, `lowered-source-publication`.
- Open work: none named.

### prepared-source-issuer (`R/prepared-source/issuer`)
- 1 file, 6 lines. `PreparationIssuer` and the constant `PREPARATION` (`issuer.rs`). Depended on by (2): `prepared-source`, pipeline 04. No tests.

### terminal-check-tables (`R/lowered-program`)
- Files and lines: 10 files, 2,911 lines; README at `R/lowered-program/README.md`. Directory name `lowered-program`, crate name `terminal-check-tables`, lib name `terminal_check_tables`.
- Purpose: types only (no elaboration, checking or prover). It holds what the lowered-form checker reads and writes: one machine's unsealed Terminal Psi module as dense id-indexed rows (values, knowledge, places, writes, obligations, edges, invocations), the refusal vocabulary, and the complete stage 05 output.
  The manifest comment says it must never depend on pipeline stages (`Cargo.toml`); it does not.
- Produces: `TerminalCompilation` (`lowered_program.rs:14`: `FormationFailures` | `FormationDefect` | `Prepared(PreparedCompilation)`), `PreparedCompilation` (`:21`: `closure`, `further`, `arrivals`, `forms`, `witnesses`, `specializations`, `selection_witnesses`),
  `Elaboration` (`program.rs:188`), `MachineTables`/`CheckTables` (`tables/mod.rs:333`, `:352`), `Refusal`, `RefusalReason`, `Checked<T> = Result<T, Refusal>` (`tables/mod.rs:278`, `:143`, `:304`), `CompilationForPublication` (`compilation.rs:49`), `MachineNames`/`ClauseText` (`names.rs`).
- Entry points: stage 06 reads `TerminalCompilation`; `Elaboration::call_closure` (`program.rs:406`), `authored_branches`, `unreachable_statements`, `dead_wildcards` (`:472-495`), `authored_machine_sources` (`:510`).
- Depends on: `source-admission`, `source-text`, `arena`, `semantic-vocabulary`, `language-core`, `symbols`, `diagnostics`, `terminal-psi`. Depended on by (6): driver, pipelines 05, 06, 07, `checked-program`, `lowered-source-publication`.
- Modules: `lowered_program.rs`, `program.rs` (Elaboration), `compilation.rs` (formation and witness records), `names.rs` (names and spans for diagnostics only), `tables/{values,bindings,contracts,edges,obligations}.rs`.
- Tests: 13 inline `#[test]` hits in `tables/` (grepped). Open work: TASKS.md **LOWERED-FORM-CHECKER** builds against this contract.

### lowered-source-publication (`R/lowered-source-publication`)
- Files and lines: 6 files, 527 lines.
- Purpose: what stage 05 was asked to lower and what it kept about the source beside the lowered program. It binds the terminal compilation to its `PublicationSource` (prepared trees, publication mode, `SourceCorrespondence`, optional roster).
  The manifest comment says the checker must never link this crate because it names typed trees.
- Consumes: `TerminalCompilation`, `PreparedTypedTrees`. Produces: `LoweredSourcePublication` (`lowered_source_publication.rs:30`), `PublicationSource` (`:35`), `Compilation<'a>` (`compilation.rs:14`), `SourceCorrespondence` (`correspondence.rs:10`), `SourceCompilationRoster` (`source_roster.rs:94`), `RawSelectionQuestions` (`selection_questions.rs:73`), `ProviderSelections` (`provider_selections.rs:19`).
- Entry points: `LoweredSourcePublication::from_lowering(LOWERING, program, source)` (`:56`), `SourceCompilationRoster::preparation_compilation` (`source_roster.rs:126`), `publication_entries` (`:119`).
- Depends on: `lowered-source-publication-issuer`, `prepared-source`, `typed-trees`, `source-declarations`, `terminal-check-tables`, `source-admission`, `source-provenance`, `arena`, `diagnostics`, `numerics`, `semantic-vocabulary`, `symbols`, `terminal-codec`, `terminal-psi`. Depended on by (2): pipelines 05 and 07.
- Tests: none. Open work: none named.

### lowered-source-publication-issuer (`R/lowered-source-publication/issuer`)
- 1 file, 6 lines. `LoweringIssuer`, `LOWERING`. Depended on by (2): `lowered-source-publication`, pipeline 05. No tests.

### checked-program (`R/checked-program`)
- Files and lines: 1 file, 335 lines.
- Purpose: the program the lowered-form checker accepted: the exact `Elaboration`, one evidence seal per discharged operation obligation, and the record of any optimization executed and rechecked. A consumer can read it and cannot change it. The manifest comment says it names no typed tree and no pipeline stage.
- Consumes: `Elaboration` from `terminal-check-tables`. Produces: `CheckedProgram` (`checked_program.rs:18`), `CheckedPublication<Input>` (`:84`), `CheckedPublicationModule` (`:168`), `RoutePublication` (`:118`), `PublicationError` (`:144`).
- Entry points: `CheckedProgram::from_judgment` (`:28`, needs `CheckerIssuer`), `CheckedPublication::from_judgment` (`:94`), `prepare_accepted_publication` (`:71`), `publish`/`publish_invocation` (`:57`, `:61`, `:182`, `:209`), `CheckedPublication::into_source_and_program` (`:109`).
- Depends on: `checked-program-issuer`, `terminal-check-tables`, `optimization`, `psi-publication-records`, `semantic-vocabulary`, `terminal-codec`, `terminal-psi`, `terminal-semantics`. Depended on by (2): pipelines 06 and 07.
- Tests: none in the crate. Open work: none named.

### checked-program-issuer (`R/checked-program/issuer`)
- 1 file, 6 lines. `CheckerIssuer`, `CHECKER`. Depended on by (2): `checked-program`, pipeline 06. No tests.

### source-completion-accounting (`R/evaluation-receipts`)
- Files and lines: 1 file, 422 lines. Directory `evaluation-receipts`, crate `source-completion-accounting`; a different crate named `evaluation-receipts` lives under `build/evaluation/receipts` (listed as a dependency of the provider crate).
- Purpose: keeps the cost of source completion (constant inputs, compile-time invocations) joined to the published invocation and its typed caller. Only real canonical-artifact execution yields a receipt (`PureEvaluationRecord`, `accounting.rs:17`).
- Entry points: `SourceCompletionAccounting` (`:303`) with `constant_input`, `release_constant_inputs`, `has_complete_usage`, `validate_charges`, `append`; `RecordedSourceInvocation::record`/`finish` (`:103`, `:110`, `:171`), `require_site`, `require_program`, `validate_charge`.
- Depends on (14): `evaluation-work`, `build-invocation`, `build-scalar-evaluation`, `build-time-values`, `checked-program-to-psi` (pipeline 07), `evaluation-accounting`, `evaluation-admission`, `terminal-work`, `typed-trees`, and foundation crates. Depended on by (3): build/evaluation/result, driver, package-manager.
- Tests: none. Open work: none named.

### source-provider-publication (`R/published-target-providers`)
- Files and lines: 1 file, 139 lines. Directory `published-target-providers`.
- Purpose: source-issued provider choices and completed Build bindings for one Psi publication and one target; the receiver must present the same program and target.
- Produces: `SourceProviderDemand` (`publication.rs:18`; `Unscoped` or `Target`), `SourcePackageReviewRequirements` (`:13`), `PublishedTargetProviders` (`:31`), `SourceNativeBinding` (`:10`).
- Entry points: `issue_target_provider_demand` (`:37`, called by `07_checked-program-to-psi/src/provider_supply/demand.rs:74`), `PublishedTargetProviders::independent_packages` (`:52`), `receive` (`:74`).
- Depends on: `evaluation-receipts` (build), `psi-provider-selections`, `terminal-codec`, `semantic-vocabulary`, `diagnostics` (dev: `terminal-psi`). Depended on by (2): pipeline 07 and native `provider-demand`.
- Tests: 2 inline hits. Open work: TASKS.md **PROVIDER-SELECTION-AFTER-TERMINAL**.

### retained-product-selections (`R/retained-product-selections`)
- Files and lines: 1 file, 125 lines.
- Purpose: records the providers and native bindings a dependency's Build chose, with exact source declarations and the arguments each choice fixed, so the consuming compile can check them.
- Produces: `RetainedProviderSelection` (`selections.rs:15`), `RetainedProviderSubject` (`:26`), `RetainedOperatorFamily` (`:43`), `RetainedOperatorCoordinate` (`:36`), `CapturedNativeDeclaration` (`:101`), `RetainedProductApplication` (`:112`).
- Depends on: `semantic-vocabulary`, `source-admission`, `source-declarations`, `source-symbol-table`, `source-text`, `symbols`. Depended on by (3): build, build/inputs, driver. Build reads them through `build/inputs/package/provider_selection_codec.rs` and `native_output_codec.rs` (per `R/README.md`).
- Tests: none. Open work: TASKS.md **BUILD-OWNED-TARGET-SELECTION** and **BUILD-OWNED-NATIVE-BINDINGS** concern the selections this crate records (inferred from titles; not read in detail).

## 3. How data flows through the area

1. Caller captures bytes into `LoadedSources` (`source-files`), admitted as `SourceFile` by `source-admission`.
2. Pipeline 00 (lexer) fills `TokenStream`/`LexedSources` (`tokens`).
3. Pipeline 01 (parser) fills `SyntaxTrees`/`ParsedSources` (`syntax-trees`).
4. Pipeline 02 (resolver) builds `SymbolResolvedTrees` with a `SymbolTable` over a `SourceCatalogue` (`symbol-resolved-trees`, `source-symbol-table`) and authored selections (`source-declarations`).
5. Pipeline 03 (typing) builds `TypedTrees` (`typed-trees`).
6. Pipeline 04 runs preparation passes and builds `PreparedTypedTrees` with `PreparedTypedTrees::new(PREPARATION, ...)` (`prepared-source`).
7. Pipeline 05 lowers it to `TerminalCompilation` (`terminal-check-tables`) and binds it to its source with `LoweredSourcePublication::from_lowering(LOWERING, ...)` (`lowered-source-publication`).
8. Pipeline 06 (the lowered-form checker) reads only `TerminalCompilation` and returns `CheckedPublication`/`CheckedProgram` via `from_judgment(CHECKER, ...)` (`checked-program`).
9. Pipeline 07 publishes Psi with `CheckedProgram::publish`/`publish_invocation`, issues provider demand with `issue_target_provider_demand` (`source-provider-publication`), and records evaluation costs in `source-completion-accounting`.
10. Alongside: dependencies' Build choices come back as `retained-product-selections`.

## 4. Where the map differs from the documents

- `R/README.md` lines 7-17 (table) and 45-55: the table names 00-07 producers and consumers and matches the crate dependencies, except that `source-completion-accounting` (a representation crate) depends on pipeline stage 07 (`checked-program-to-psi`) and on the Build evaluation crates. The README does not list it, and `R/lowered-program/Cargo.toml`'s "must never depend on pipeline stages" comment applies only to that crate.
- `R/README.md` says the representation folder holds "data passed between stages", but `typed-trees` (92k lines) contains type-identity, callable-application and operator/service-declaration algorithms (for example `type_records/type_identity.rs` at 11,561 lines, `callables/callable_applications.rs` at 9,432 lines). `R/syntax-trees/README.md` also keeps equality/relocation logic here by design.
- `R/README.md` line 59 says "The compiler still encodes the retired carry policy". Recent commits (`3b2f7aaf922c psi: resolve former carry permission names as ordinary domains`) and `02a8afccc3eb` ("retire data carry") touch it; I did not check whether the sentence is still true.
- `OWNER_CODE_CONCERNS.md:81` links `type_identity.rs#L2420` for `encode_signature_region`; the function is at line 2405 in this checkout (grepped).
- `R/README.md` says `tokens` and `syntax-trees` are separate stages' products; `syntax-trees/Cargo.toml` has no `tokens` dependency (parsed pages come from `source-files` types), so the parser, not the representation, links the two.
- Directory names differ from crate names for four crates (`custody` -> `source-admission`, `lowered-program` -> `terminal-check-tables`, `evaluation-receipts` -> `source-completion-accounting`, `published-target-providers` -> `source-provider-publication`). `R/README.md` links by path, so its links are right; searches by crate name will not find the directory.
- The issuer claim in `R/README.md` ("`mbx xtask check-boundary` refuses the issuer in any other manifest, including as a test dependency") matches `tools/xtask/src/boundary/linear_pipeline.rs:112-139`; the representation crate itself is also allowed (`may_issue`). The README's "Only linked by stage N" column omits that.
- Data-oriented ratchets (AGENTS.md: dense ids, no `HashMap`/`BTreeMap`): grepped `HashMap`/`BTreeMap` in 11 files of this area (accounting, source_selections, source_inputs, lookup_indexes, and seven typed-trees files), plus a `HashSet` field in `TypedTrees` (`typed_trees.rs`, `pending_const_range_endpoints`). I did not check whether each is covered by `tools/ratchets/dense_id_maps.txt`.

## 5. Not read, and unsure

- Not read line by line: `typed-trees` (92k lines; I read the root, exports, frozen storage, and module heads), `source_symbols/table.rs`, `syntax-trees/copying/`, `syntax-trees/inspection/`, `symbol-resolved-trees/nodes/`. Their module descriptions come from file names, module headers and `pub` item greps.
- Not read: `source-to-psi/representations/publication.rs` and `published_target_program.rs`, which sit in the `representations` folder but belong to the driver crate (another area); only their first lines.
- Unsure: whether `typed-trees` imports `symbol-resolved-trees` types (the manifest says no, so typing's input is only reached through pipeline 03); the exact reverse-dependent crate names (I give manifest directories, not package names); whether "Depended on by" counts include test-only edges (they do for dev-dependencies, which I did not separate except for those I read in the manifests).
- Open-work entries for `source-files`, `tokens`, `checked-program`, `lowered-source-publication`, `prepared-source` and others are "none named" because I grepped TASKS.md and OWNER_CODE_CONCERNS.md for the crate names and directory paths only; items that name them indirectly through pipeline paths may exist.
