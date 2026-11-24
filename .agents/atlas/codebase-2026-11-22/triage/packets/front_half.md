# Front-half boundary packet

This packet gives a disposition for each claim in [the source-stage boundary note](../../boundaries/front_half.md). Code was read at `24ad2700685e`, the common baseline. The note was written at `4f3c255d0559`; several cited files changed in between, so each claim was rechecked by symbol, not by line number. Nothing was built for this packet.

Claim IDs are `FH <handoff>.<item>`, where the handoff is the note's section (`00-01` to `07-psi`) and the item is its numbered paragraph. A bullet inside an item gets a letter. Paths use the note's shorthands: `S/` is `source-rust/subsystems/compiler/source-to-psi/`, `P00/` to `P07/` are its pipeline stages, `R/` is `S/representations/`, and `PSI/` is `source-rust/subsystems/compiler/psi/`.

Outcomes:
- **intended**: the code does what the note says, and the spec, a stated precondition or a checked guard elsewhere makes that correct.
- **refuted**: the code does not do what the note says, or the route has no production producer.
- **duplicate**: an earlier ledger entry already settles the claim; the entry is named.
- **still open**: no evidence settles it in this pass; the missing trace is named.

Status: in progress.

## 00 to 01: lexer to parser

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 00-01.1 | `lex_file` returns a `LexedSource`; the driver lexes then parses one file at a time. | `P00/lexer.rs:48`; `S/driver/parse_source_batch/batch.rs:141-149` | Read in code. | intended (descriptive) |
| FH 00-01.2 | The parser trusts UTF-8 text, in-range token spans and token order. | `P00/lexer.rs:83` ("Source loading has already admitted UTF-8"); `R/tokens/token_stream.rs:163` `spelling` slices without a check | The lexer is the only producer of the stream. `P00/tests/byte_inputs.rs::random_texts_are_covered_exactly_or_refused_at_whole_characters` checks that tokens cover the text exactly and re-lexes each spelling. | duplicate of [front.md, 00 → 01](../front.md#front-half-handoffs) (producer-owned) |
| FH 00-01.3 | Only the lexer's own assertions run (`retain_source` pointer identity, `push_source` decoded bytes); the lexical error precedes parse errors. | `R/tokens/token_stream.rs:100-104`, `:130-134`; `batch.rs:101`, `:138` | Read in code. These are producer guards, as the note says. | intended |
| FH 00-01.4a | A retained parse skips stages 00 and 01 when the text equals the retained unit's text. | `S/driver/parse_source_batch.rs:36-43` filters on `unit.source == file.source` | Driver fixture `retained_parse_of_other_bytes_is_not_adopted` (`driver/parse_source_batch/reused_sources.rs`). | duplicate of front.md, 01 → 02 |
| FH 00-01.4b | Relocation leaves a handle of another page generation unchanged (digest 05, open). | `R/syntax-trees/storage/relocation.rs:19-24`; caller `R/syntax-trees/syntax_trees.rs:204-227` `relocate_unit` | `relocate_unit` refuses unless `page_count() == 1` ("only a standalone unit page can be relocated"), and `at_coordinates` (`R/syntax-trees/files/parsed.rs:59-72`) refuses generation 0. A standalone page has one generation, so every valid handle in it is rewritten. A handle of another generation inside a one-page forest would already be dangling before relocation. | intended (precondition checked) |
| FH 00-01.4c | The parser reads source text through the tokens (`check_file_aliases`) and copies the shared text into `ParsedSource.source`. | `P01/parser.rs:90`, `:265` | Read in code. | intended (descriptive) |

## 01 to 02: parser to name resolution

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 01-02.2 | Without a source map, visibility and stratum checks "stay permissive". | `P02/resolver.rs:85-112` | Every production `ResolutionRequest` passes `sources: Some(..)`: `S/driver/resolve_and_type.rs:141-146` and `:197-203`, `P02/generated_source.rs:38-43`. The source-free constructor `ResolutionRequest::new` and `resolve` are `#[cfg(any(test, feature = "test-support"))]`. The remaining literal constructions are in test modules. | intended (test-only mode) |
| FH 01-02.4a | `with_build_entry` removes a Build entry root found by name-span equality. | `S/driver/resolve_and_type.rs:83-127` | The search is limited to the one Product file whose path, bytes, origin, package root, stratum and package identity equal the Build entry's file, and to that file's own root list. Within one file's byte text, an equal name span names one declaration. | intended |
| FH 01-02.4d | `SymbolTable::name` reads a span-only name from the source map and falls back to `""`; which names take the span-only form in production was not traced. | `R/symbol-resolved-trees/symbol-table/source_symbols/table.rs:3328-3339`; `foundation/declaration-identity/src/symbol.rs:91-105`, `:173` | Traced. Only `SymbolNameRef::Source` produces `SymbolNameStorage::Source`. All 40 construction sites outside `tests/` directories are inside `#[cfg(test)]` modules (for example `P05/src/read.rs:965` after `:763`, `R/typed-trees/type_records/type_identity.rs:4590` after `:4468`). Production stages 02 and 03 use `Static` (140), `OwnedSource` (12) and `Borrowed` (9), which all carry a spelling. | refuted for production (no producer of span-only names) |

## 02 to 03: name resolution to typing

| ID | Original claim | Code path at baseline | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| FH 02-03.2b | Resolution defers dependent token-binding shapes to typing, which reads them at `T/runtime_callables.rs:1350`. | Writer `P02/selections/machine_token_bindings.rs:199`, carried by `R/symbol-resolved-trees/symbol_resolved_trees.rs:81`; reader `P03/src/runtime_callables.rs:1324-1350` | Traced to the call. `P03/src/lowerer.rs:1906` calls `validate_pending_token_shapes` and propagates its error with `?` on the cohort lowering path. | intended (consumed) |
