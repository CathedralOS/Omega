# Front-half boundary packet

This packet gives a disposition for each claim in [the source-stage boundary note](../../boundaries/front_half.md). The note found no wrong compiler result, and neither did this review. Five claims stay open, listed at the end. One is new: `omega --check` stops before the publication checks that `omega build` runs, so the two commands can disagree on one source. No program has yet been shown to hit that.

Code was read at `24ad2700685e`, the common baseline. The note was written at `4f3c255d0559`. 368 source-compiler files changed between the two, so each claim was rechecked by symbol, not by line number. Lines below are baseline lines. Nothing was built or run for this packet; the evidence is code reading plus the names of existing fixtures, each confirmed present at the baseline.

Claim IDs are `FH <handoff>.<item>`. The handoff is the note's section (`00-01` to `07-psi`) and the item is its numbered paragraph; a letter picks a bullet or sentence inside it. Paths use the note's shorthands: `S/` is `source-rust/subsystems/compiler/source-to-psi/`, `P00/` to `P07/` are its pipeline stages, `R/` is `S/representations/` and `PSI/` is `source-rust/subsystems/compiler/psi/`.

Outcomes:
- **intended**: the code does what the note says, and a spec page, a stated precondition or a checked guard makes that correct.
- **refuted**: the code does not do what the note says, or the route has no production producer.
- **duplicate**: an earlier ledger entry settles the claim; the entry is named.
- **out of slice**: the claim belongs to another boundary note, which is named.
- **still open**: no evidence settles it in this pass; the missing trace is named.

Earlier dispositions referred to as "front.md" are in the [source-stage ledger](../front.md).

## 00 to 01: lexer to parser

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 00-01.1 | `lex_file` returns a `LexedSource`; the driver lexes then parses one file at a time. | `P00/lexer.rs:48`; `S/driver/parse_source_batch/batch.rs:141-149` | Read in code. | intended (descriptive) |
| FH 00-01.2 | The parser trusts UTF-8 text, in-range token spans and token order. | `P00/lexer.rs:83` ("Source loading has already admitted UTF-8"); `R/tokens/token_stream.rs:163` `spelling` slices without a check | The lexer is the stream's only producer. `P00/tests/byte_inputs.rs::random_texts_are_covered_exactly_or_refused_at_whole_characters` checks that tokens cover the text exactly and re-lexes each spelling. | duplicate of front.md, 00 → 01 (producer-owned) |
| FH 00-01.3 | Only the lexer's own assertions run (`retain_source` pointer identity, `push_source` decoded bytes), and a lexical error is reported before parse errors. | `R/tokens/token_stream.rs:100-104`, `:130-134`; `batch.rs:101`, `:138` | Read in code. These are producer guards, as the note says. | intended |
| FH 00-01.4a | A retained parse skips stages 00 and 01 when the file's text equals the retained unit's text. | `S/driver/parse_source_batch.rs:36-43` filters on `unit.source == file.source` | Driver fixture `retained_parse_of_other_bytes_is_not_adopted` (`S/driver/parse_source_batch/reused_sources.rs`). | duplicate of front.md, 01 → 02 |
| FH 00-01.4b | Relocation leaves a handle of another page generation unchanged (digest 05, open). | `R/syntax-trees/storage/relocation.rs:19-24`; caller `R/syntax-trees/syntax_trees.rs:204-227` | `relocate_unit` refuses unless `page_count() == 1` ("only a standalone unit page can be relocated"), and `at_coordinates` (`R/syntax-trees/files/parsed.rs:59-72`) refuses generation 0. A standalone page has one generation, so every valid handle in it is rewritten. A handle of another generation would already be dangling before relocation. | intended (precondition checked) |
| FH 00-01.4c | The parser reads source text through the tokens (`check_file_aliases`) and copies the shared text into `ParsedSource.source`. | `P01/parser.rs:90`, `:265` | Read in code. | intended (descriptive) |

## 01 to 02: parser to name resolution

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 01-02.1 | The driver wraps the forest in a `ResolutionRequest` and calls `resolve_deferred`. | `P02/resolver.rs:85-99`, `:113`; `S/driver/resolve_and_type.rs:141-146`, `:197-203` | Read in code. | intended (descriptive) |
| FH 01-02.2 | Without a source map, visibility and stratum checks "stay permissive". | `P02/resolver.rs:85-112` | Every production `ResolutionRequest` passes `sources: Some(..)`: `S/driver/resolve_and_type.rs:141-146` and `:197-203`, and `P02/generated_source.rs:38-43`. The source-free constructor `ResolutionRequest::new` and `resolve` are `#[cfg(any(test, feature = "test-support"))]`. The other literal constructions are in test modules. | intended (test-only mode) |
| FH 01-02.3 | Only the driver's source map is checked (`validate_logical_filenames`), not the syntax trees. | `P02/resolver.rs:121-124` | Read in code. The parser leaves some judgments to later stages by design. | duplicate of front.md, 01 → 02 |
| FH 01-02.4a | `with_build_entry` removes a Build entry root found by name-span equality. | `S/driver/resolve_and_type.rs:83-127` | The search covers only the one Product file whose path, bytes, origin, package root, stratum and package identity equal the Build entry's file, and only that file's own root list. Within one file's text, an equal name span names one declaration. | intended |
| FH 01-02.4b | Resolution rewrites syntax before symbols exist and later writes generated syntax back through `machine_equations::complete`. | `P02/resolver.rs:21`; `P02/deferred_bodies.rs:423` | Read in code. The stderr-only warning loops in this code were removed in [PR 5244](https://github.com/CathedralOS/Omega/pull/5244) (front.md, 02:296). | intended (descriptive) |
| FH 01-02.4c | The frontier keeps the whole syntax forest until all bodies are completed. | `P02/deferred_bodies.rs:152` (`DeferredResolvedFrontier.syntax`) | Read in code. Body completion is demand-driven, so it needs the syntax. | intended (descriptive) |
| FH 01-02.4d | `SymbolTable::name` reads a span-only name from the source map and falls back to `""`; which names take the span-only form in production was not traced. | `R/symbol-resolved-trees/symbol-table/source_symbols/table.rs:3328-3339`; `source-rust/subsystems/compiler/foundation/declaration-identity/src/symbol.rs:91-105`, `:173` | Traced. Only `SymbolNameRef::Source` produces `SymbolNameStorage::Source`. All 40 construction sites outside `tests/` directories sit inside `#[cfg(test)]` modules (for example `P05/src/read.rs:965`, in the module starting at `:763`). Production stages 02 and 03 use `Static` (140 sites), `OwnedSource` (12) and `Borrowed` (9), which all store a spelling. | refuted for production (no producer of span-only names) |

## 02 to 03: name resolution to typing

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 02-03.1 | Typing receives shared resolved trees, then alternates with resolution over pending bodies until none is left. | `S/driver/resolve_and_type.rs:209`, `:216-232` | Read in code. | intended (descriptive) |
| FH 02-03.2a | Typing copies the resolver's symbol table, reach rows, domains, bindings, selections and forwardings without checking them. | `P03/src/lowerer.rs:1584-1625` | Read in code. These are the stage's input facts, carried in the representation as the pipeline rules require. A wrong but self-consistent resolver decision belongs to the producer. | duplicate of front.md, 02 → 03 |
| FH 02-03.2b | Resolution defers dependent token-binding shapes to typing, which reads them at `runtime_callables.rs:1350`. | Writer `P02/selections/machine_token_bindings.rs:199`, carried by `R/symbol-resolved-trees/symbol_resolved_trees.rs:81`; reader `P03/src/runtime_callables.rs:1324-1350` | Traced to the call. `P03/src/lowerer.rs:1906` calls `validate_pending_token_shapes` on the cohort lowering path and propagates its error with `?`. | intended (consumed) |
| FH 02-03.2c | Typing keys about 30 decisions on spelling rather than symbol handles (digest 02 §2). | `P03/` | Not opened individually in this pass. | still open (see list) |
| FH 02-03.3a | `validate_resolved_semantics` checks language rules, not resolver well-formedness. | `P03/src/lowerer.rs:1567-1588` | Read in code. | intended (descriptive) |
| FH 02-03.3b | The continuation checks compare snapshots with the representation's own equality, so they are pairing checks, not content checks. | `P02/deferred_bodies.rs:284`; `P03/src/lowerer.rs:664`, `:843`; `S/driver/resolve_and_type.rs:361` | Driver fixture `resolution_handoff_completes_each_batch_without_changing_its_predecessor` (`S/driver/resolve_and_type/tests.rs`). | duplicate of front.md, 02 → 03 |
| FH 02-03.4a | Typing sends a `ResolvedAllocationExtension` back to resolution before each completion. | `P03/src/lowerer.rs:791`; `S/driver/resolve_and_type.rs:223` | Read in code. | intended (descriptive) |
| FH 02-03.4b | The driver writes `partial_build_entry`, `partial_build_can_return` and `parked_build_locals` into typing's product. | `S/driver/resolve_and_type.rs:233-235` | These are the driver's Build planning inputs (set at `S/driver/compile_build.rs:94`), not stage output. They travel in the typed roots (`R/typed-trees/storage/arena_tables.rs:83`) to their readers at `S/driver/compile_build.rs:182` and `P05/src/read/subjects.rs:395`. | intended |
| FH 02-03.4c | 57 `symbols.name(` lines in `P02/` and 15 in `P03/` may read source text through the symbol table. | Same grep at baseline: 57 and 15 | The counts match. FH 01-02.4d shows no production name takes the source-map branch. | refuted for production (follows FH 01-02.4d) |

## 03 to 04: typing to preparation

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 03-04.1 | `SourcePreparationRequest::settle` hands a `SettledSource` to `freeze`, which calls `prepare_typed_trees`. | `S/driver/prepare_typed_source.rs:76-140`; `P04/prepare.rs:42-56` | Read in code. | intended (descriptive) |
| FH 03-04.2a | "Not already specialized" is a comment, not a check. | `S/driver/prepare_typed_source.rs:49-50`; `R/prepared-source/lib.rs:76` (`into_inner`) | front.md prepared a fixture twice through `into_inner` and found the targets and specialization receipts unchanged. The production driver prepares once. | duplicate of front.md, 03 → 04 |
| FH 03-04.2b | A missing deferred-refusal row means "served". | `R/typed-trees/proofs/deferred_refusals.rs:1-30`; reader `P05/src/read/machine.rs:1458-1494` | The module header states the rule: a pass that cannot serve a request records a row, and every machine read raises it. Whether every typing and preparation pass records a row for everything it skips is a question about each producer, not this reader. | still open (see list) |
| FH 03-04.2c | `record_patterns` leaves unresolved subjects in place for the checker to refuse. | `P04/passes/record_patterns.rs` (digest 02, stated) | Not opened in this pass. Stage 05's reader refuses a name with no valid symbol (`P05/src/read.rs:1-22`), which covers an unresolved subject the program reaches. | intended (stated; refusal site named) |
| FH 03-04.3 | The driver refuses unsupported admission inputs, re-derives opaque representation choices with the same function, and joins constant receipts. `prepare_typed_trees` has no entry check. | `S/driver/prepare_typed_source.rs:76-114`, `:34-46`; `P04/prepare.rs:42-56` | Read in code, as stated. | intended |
| FH 03-04.4a | The driver evaluates demanded compile-time values into the typed program before preparation. | `S/driver/prepare_typed_source.rs:95-104` | Read in code. | intended (descriptive) |
| FH 03-04.4b | Each compile-time call runs stages 04 to 07 and the interpreter, so stage 04's input already contains later-stage results. | `S/driver/compile_time_invocations/calls.rs`, `calls/execution.rs` | Read in code. This is the specified compile-time evaluation route (AGENTS.md: compile-time evaluation belongs to the source compiler). | intended |
| FH 03-04.4c | The request type is a stage 07 type, `TerminalSourceCompilation`. | `S/driver/prepare_typed_source.rs:54` | Read in code. | intended (descriptive) |

## 04 to 05: preparation to lowering

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 04-05.1 | `PreparedTypedTrees` crosses; the driver lowers it with a compilation or a roster. | `R/prepared-source/lib.rs:31-36`; `S/driver/judge_lowered_source.rs:111-118` | Read in code. | intended (descriptive) |
| FH 04-05.2a | Lowering trusts the typed trees' contents; the constructor says preparation passes are the producer's responsibility. | `R/prepared-source/lib.rs:48-60` | Read in code, as stated. Fixtures `a_declaration_value_that_contradicts_its_initializer_is_refused` and `a_block_without_the_statement_that_binds_its_value_is_refused` (stage 05 `tests/`) refuse two incorrect tree contents. | duplicate of front.md, 04 → 05 |
| FH 04-05.2b | `served` returns `Ok` when no row is found. Note: `served` also skips `LayoutPolicyStorage` rows, saying a geometry consumer must settle them. | `P05/src/read/machine.rs:1458-1494`, filter at `:1464-1467`; row writer `S/driver/compile_time_invocations/frontier.rs:154-160` | The no-row rule is FH 03-04.2b. For the skipped layout rows, `P05/src/read/source_records.rs:35-54` accepts a storage application without a plan and returns its semantic type, leaving target geometry to a later consumer. That consumer's refusal was not traced. | still open (see list) |
| FH 04-05.3a | `constant_inputs` checks value inputs only. | `R/prepared-source/values.rs:63` | Fixtures `computation_inputs_cannot_be_attached_to_another_source` and `a_static_input_must_keep_its_owner_expression_and_carrier` (`R/prepared-source/lib.rs`). | duplicate of front.md, 04 → 05 |
| FH 04-05.3b | Only stage 04 can mint the issuer, and `compile_fail` doctests keep a bare `FrozenTypedTrees` out of lowering. | `tools/xtask/src/boundary/linear_pipeline.rs:111-131`, `:557`; `P05/src/compilation.rs:30`, `:42`, `:76` | Read in code. | intended |
| FH 04-05.3c | Lowering's entry checks are stage 05 code: `check_bound_callable_shapes`, binding rejoin as `ForeignIdentity`, `served`, and the reader's spelling refusal. | `P05/src/callable_formations.rs:21`, called at `P05/src/compilation.rs:89`; `:105-124`; `P05/src/read.rs:1-22` | Read in code. `check_bound_callable_shapes` moved to `callable_formations.rs` and is re-exported from `P05/src/lib.rs:184`. | intended (descriptive) |
| FH 04-05.4 | Lowering reads the typed trees directly (16 `symbols.name(` lines) and prepares what stage 06 needs. | grep at baseline: 16 | Counts match; FH 01-02.4d applies to the source-map branch. | intended (descriptive) |

## 05 to 06: lowering to the lowered-form checker

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 05-06.1 | `LoweredSourcePublication` crosses, and a stage 05 refusal travels inside it. | `R/lowered-source-publication/lowered_source_publication.rs:50-61`; `P05/src/compilation.rs:89-103` | Read in code. On formation failures stage 05 returns `source: Default::default()`. | intended (descriptive) |
| FH 05-06.2a | `from_lowering` checks nothing; the issuer decides who builds the value. | `R/lowered-source-publication/lowered_source_publication.rs:55-61` | Read in code, as stated. | intended |
| FH 05-06.2b | Stage 05 formation failures and refusals become verdicts as given. | `P06/src/verdict.rs:1548`, `:1566`, `:1576` | Read in code. A stage 05 refusal refuses the program; the checker never turns one into acceptance. | intended |
| FH 05-06.2c | `row()` cannot tell another machine's in-range identity; the rule is structural. | `R/lowered-program/tables/mod.rs:313-331` | The comment at the function states the rule: identities never cross `MachineTables`, and calls go through an `InvocationRow` holding a `MachineId` and positions. | intended (stated) |
| FH 05-06.2d | `judge_publication` returns the source half untouched beside the checked program. | `P06/src/publication.rs:19` | Read in code. See FH 06-07.4. | intended (descriptive) |
| FH 05-06.3a | `minted.rs` re-derives owed rows independently of stage 05. | `P06/src/minted.rs` | Fixture `an_operation_whose_safety_obligation_has_no_row_is_refused` (`P06/src/tests/table_faults.rs`). | duplicate of front.md, 05 → 06 |
| FH 05-06.3b | Static reach, suspension frontiers and claim planning are compared through the same shared helpers stage 05 called, so a helper defect would appear on both sides (inferred). | `P05/src/declare.rs`, `P05/src/suspension.rs`, `P05/src/lib.rs`; `P06/src/static_reach.rs`, `P06/src/operational.rs` | Fixtures `a_missing_or_altered_crossing_is_not_accepted` and `malformed_or_forged_operational_metadata_is_not_a_program_rejection` (`P06/src/tests/operational_effects.rs`) cover a tampered frontier. No source case was found where a helper answers wrongly on both sides. | duplicate of front.md, 05 → 06 (still open; see list) |
| FH 05-06.4 | Typed source rides beside the checker inside `PublicationSource`. | `P06/src/publication.rs` | Read in code. See FH 06-07.4. | intended (descriptive) |

## 06 to 07: checker to publication

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 06-07.1 | `CheckedPublication` crosses, built with the `CHECKER` issuer. | `R/checked-program/checked_program.rs:84-105` | Read in code. | intended (descriptive) |
| FH 06-07.2 | Stage 07 trusts the elaboration, the requirement reviews and the source correspondence that stage 05 wrote. | `P07/src/accepted.rs:225-257` (`accept_judgment`) | Fixture `equal_code_in_another_invocation_cannot_reuse_source_selections` (`P07/src/source_selections.rs`) covers selections across invocations. A correspondence with a wrong identity inside one invocation remains untested. | duplicate of front.md, 06 → 07 (still open; see list) |
| FH 06-07.3a | `accept_judgment` refuses unless the correspondence has `1 + further_forms` forms, none missing. | `P07/src/accepted.rs:231-237` | Read in code; the refusal says the program is refused rather than accepted without a verdict. | intended |
| FH 06-07.3b | `prepare_publication` runs `validate`, ending with `activation_storage::validate_module`; whether stage 05 or 06 runs that earlier was not traced. | `R/checked-program/checked_program.rs:232-238`, `:262-334` | Traced. Outside tests, `validate_module` is called only by `validate` and by the wire decoder (`PSI/artifact/wire/src/foundation_validation.rs:12229`). Stages 05 and 06 never call it. Stage 06's comment at `P06/src/operational.rs:349-350` nevertheless relies on it to refuse a resource claim. Source stage 05 emits no `EstablishActivationClaim`, so a source module has no claim types for that rule to judge. Separately, `source_to_psi::check` (`S/source_to_psi.rs:51-66`, used by `source-rust/application/src/commands/compile.rs:402`) stops after `accept_judgment` and `check_selections`. It never calls `prepare_publication`, so a check skips `validate`, which `compile` runs. | traced; new lead still open (see list) |
| FH 06-07.3c | `validate_companions` is stage 07 checking its own `build_companions::produce` output. | `P07/src/accepted.rs:96`, `:323` | Read in code. | intended (descriptive) |
| FH 06-07.4 | Stage 07 reads the typed source for provenance, companions and selection questions, so it takes facts from stages 03 and 04 that did not pass the checker. | `P07/src/accepted.rs:286-300` | The comment says these are captured "while their typed owner is available". The selection questions are then checked against the published module (FH 07-psi.3c). Provenance and companions are not verdict inputs. | intended (descriptive) |

## What 07 publishes for Psi

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 07-psi.1 | `SourcePublication::publish` produces the artifact once per roster; deployments add `PublishedTargetProgram`. | `P07/src/source_publication.rs:136-190`; `R/published_target_program.rs:9-23` | Read in code. | intended (descriptive) |
| FH 07-psi.2 | The closed module does not show checker acceptance; whether native stage 00 re-checks the artifact was not traced here. | `PSI/artifact/codec/src/closed_module.rs:1-3` | Decoding a `PSITERM` module runs the full foundation validation, including `activation_storage::validate_module` (`PSI/artifact/wire/src/semantic_module.rs:4208-4238`). Native stage 00's own use belongs to the next boundary. | out of slice: [Psi and native note](../../boundaries/psi_and_native.md) |
| FH 07-psi.3a | The encoder validates scalar qualifications and canonical order; whether `encode_raw_closed` runs foundation validation was not traced. | `PSI/artifact/wire/src/semantic_module.rs:4150-4158`; `PSI/artifact/wire/src/module_wire.rs:4192-4215` | Traced. `encode_raw_closed` only writes the bytes. Validation runs at publication (`validate`) and again when bytes are decoded (above). That matches AGENTS.md "Verify at trust boundaries". | intended |
| FH 07-psi.3b | `from_closed_parts` refuses invocation-scoped modules; `with_publication` refuses foreign metadata or seals. | `PSI/artifact/codec/src/canonical_artifact.rs:94-106`, `:132-180` | Read in code. | intended |
| FH 07-psi.3c | `admit_selection_questions` validates the packet against the published module, outside stage 07. | `PSI/artifact/authorization/selection_questions.rs:846-879`, calling `validate_selection_questions` at `:393` | Read in code. | intended (independent) |
| FH 07-psi.3d | Stage 07 refuses a requirement with no supply or several. | `P07/src/provider_supply/mod.rs:242-300`, `choose_supply` at `:375-380` | Partly right. `choose_supply` refuses when nothing supplies the requirement, or when several supplies tie at the highest authority. A higher authority overrides lower ones: Build over target default over native binding. Fixture `a_provider_for_one_receiver_does_not_supply_another_receiver_or_the_template` (`P07/src/provider_supply/tests.rs`). | intended |
| FH 07-psi.4 | Typed trees travel past publication in `RuntimePermissionDeclarations`; `requirement_path` matches a service by `display_path` text. | `R/published_target_program.rs:17-75`; caller `source-rust/subsystems/compiler/runtime_consent.rs:55-94` | The match is limited to boundary services of one package identity and refuses unless exactly one service and one method match. Its only caller uses the result as the path written for the lock comparison. | intended (fails closed) |

## Closing list in the note

The note's last section, "Handoffs where nothing independent checks the value", repeats the items above: 00-01 is FH 00-01.2 and .3; 01-02 is FH 01-02.3; 02-03 is FH 02-03.2a and .3b; 03-04 is FH 03-04.2a and .3; 04-05 is FH 04-05.2a and .3; 05-06 is FH 05-06.3b and .2b; 06-07 is FH 06-07.2, .3c and .4.

## Records joined

The note cites three records directly. They agree with the code at the baseline:
- `unit_050` A2 (the checker rebuilds suspension liveness instead of trusting the producer): FH 05-06.3b.
- `unit_053` (through digest 02): FH 00-01 and 01-02 parser items.
- `unit_057_gap_029` (pending token shapes consumed in typing): FH 02-03.2b, now traced to `P03/src/lowerer.rs:1906`.

## Unresolved claims

1. **FH 06-07.3b, check and build can disagree.** `omega --check` stops before `CheckedProgram::validate`, so a source the check accepts can still fail publication in `omega build`. That function also refuses generic check forms, which a check is meant to accept. Its parts that also apply to closed programs are the `undeclared` roster, the installation-bound refinement and `activation_storage::validate_module`. No such program was found or run. To settle it, find a closed source program that `omega --check` accepts and `omega build` refuses with `accepted Terminal publication failed`. Separately, the stage 06 comment at `P06/src/operational.rs:349-350` names a refusal that does not run before the checker's verdict. It should name the real refusal site, or stage 06 should judge the case itself.
2. **FH 03-04.2b, unserved constructs without a row.** Whether every typing and preparation pass records a deferred-refusal row for each request it skips. To settle it, find a construct a pass skips with no row and no refusal that stage 05 then reads as served.
3. **FH 04-05.2b, unplanned layout storage.** Stage 05 accepts a layout storage application with no target plan (`P05/src/read/source_records.rs:47-49`) and skips its `LayoutPolicyStorage` row. The consumer that must refuse when target geometry is required was not traced.
4. **FH 02-03.2c, spelling-keyed typing decisions.** About 30 places in `P03/` decide on spelling (digest 02 §2). Each needs either an owner check in front of it or a same-spelled user declaration that changes the result, as in the `IntervalSet` and `Rat` repairs in front.md.
5. **FH 05-06.3b and FH 06-07.2, shared helpers and same-invocation correspondence.** Both remain as front.md left them: each needs a concrete source case.

Packet complete.
