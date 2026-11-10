# Area 12 open acceptances

Read-only trace of every entry classed "open" in `.agents/atlas/compiler-2026-11-10/digests/area_12.md`. Digest citations are at `457adf4a11f`. This tree is `8febfc42316` (the traces branch contains that commit). Line numbers below were opened in the current files and still match the digest. "Read" means those lines were opened. "Inferred" means a conclusion from lines that were opened, not from a run. Nothing was built or executed.

Path shorthand matches the digest. `SRT/` is `omega-rust/psi/compilation/representations/symbol-resolved-trees/src/`. `ST/` is `syntax-trees/src/`. `codec/` is `terminal-psi/codec/src/`. `sem/` is `terminal-psi/semantics/src/`. `TP/` is `terminal-psi/src/`. `TT/` is `typed-trees/src/`. `DTI` is `TT/declaration_tables_and_index.rs`.

Numbering follows the open bullets in digest order. A bullet that names several independent sites is split so each site has one verdict. Entries not yet opened are absent from the table.

## Summary

| # | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| 1 | SRT/storage_and_indexes.rs:37-69 | UNDECIDED | The only reader indexes a cached position without checking the symbol; a stale hit was not shown to accept a program. |
| 2 | SRT/type_system_and_names.rs:895-908 | UNDECIDED | Spelling alone classifies a named type as a primitive; no constructor of that named type was read. |
| 3 | SRT/values.rs:2518-2519 | UNDECIDED | Destructure field values are omitted from the dependency list; the match reducer was not read. |
| 4 | SRT/values.rs:1309-1316 | CLOSED | Production copies stay in the same expression table; the cross-table copy is only in tests. |
| 5 | SRT/values.rs:578-588 and 459-470 | UNDECIDED | The target-family contract copy leaves the requirement handle and the selection id untouched; their checker was not read. |

## 1. Prefix-position cache

Verdict: UNDECIDED.

Search: `fn local_statement_position` and `.local_statement_position(` — 2 matches, both read (`SRT/storage_and_indexes.rs:160`, `pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_expression_paths.rs:253`). `.parameter_position(` — 16 matches in `omega-rust`. One is this method (`symbols_expression_paths.rs:245`, a slice plus a symbol). The other 15 are different methods and were not read.

Read in full: `PrefixPositionMaps::position` (`SRT/storage_and_indexes.rs:37-69`), `local_statement_position` (`:160-177`), `parameter_position` (`:179-193`), `projected_receivers::call_target` (`symbols_expression_paths.rs:194-346`), `resolve_expression_table_call_target_symbol` (`:28-100`), `assign_statement_slice_symbols` (`symbols_statements.rs:186-215`), `AssemblyScopeRouting::assign` (`symbols_statements.rs:33-134`). Also read the cache comment at `symbols_expression_paths.rs:156-165` and the `MachineScope` field comment at `symbols.rs:3807-3813`.

`call_target` is the only caller of either cache method. Its only caller is `resolve_expression_table_call_target_symbol` (`symbols_expression_paths.rs:45`). That function has two callers, both read at the call: `symbols_expressions.rs:214` and `symbols.rs:439`. Both pass the scope's `prior_statements` and the caller's parameter slice.

The cache key is the slice's address (`storage_and_indexes.rs:44`). A hit keeps the old index (`:63`, `or_insert`). The caller then indexes that position and, for a local, uses the statement if it is `LocalData` without checking that the local's symbol is the one looked up (`symbols_expression_paths.rs:250-259`). A parameter uses `.get` (`:241-248`). A wrong but in-range `LocalData` would supply that other local's type to method resolution (`:333-343`).

The statement walk that feeds this only grows a prefix (`symbols_statements.rs:197-214`), and the comment at `symbols_expression_paths.rs:164-165` says the prefix strictly appends. Assembly builds a fresh vector per block (`symbols_statements.rs:94-100`) and drops it. A recycled address whose first query is shorter than the old watermark misses (`storage_and_indexes.rs:53-58`) and rescans. I did not find a query that both reuses an address and looks up a symbol stored at a different index. I also did not read every later check that might reject a method symbol chosen from the wrong local.

What would decide it: one call sequence whose two slices share `as_ptr`, whose three samples match, and whose looked-up symbol is stored at an index that is a different `LocalData` in the new slice, followed by whether that method symbol is kept.

Digest: the cited caller lines are right. The description of a stale hit indexing the new slice matches the code that was read.

## 2. Named type spelled as a primitive

Verdict: UNDECIDED.

Search: `.primitive_type()` under `pipeline/02_syntax-trees-to-symbol-resolved-trees` — 0 matches. Under `pipeline/03_symbol-resolved-trees-to-typed-trees` — 2 matches, both read (`equality_and_exhaustiveness.rs:2336` and `:2402`). `resolved::types::PrimitiveType::from_name` — the definition (`SRT/type_system_and_names.rs:912`) and one other use at `type_reference.rs:2880`, read.

Read in full: `TypeReference::primitive_type` (`SRT/type_system_and_names.rs:895-908`), `PrimitiveType::from_name` (`:912-927`), `validate_const_literal_argument` (`equality_and_exhaustiveness.rs:2330-2349`), `proposition_const_types_match` (`:2401-2414`). Read the caller of those two at `:2127-2156`, not the whole proposition-lowering function. Read `retain_type_reference_selection` at `type_reference.rs:2867-2882`, which returns before recording a selection when the spelling is a primitive name. That is a skipped record, not this classification.

`primitive_type` on `Named` calls `from_name` and does not read the symbol (`:899`). `validate_const_literal_argument` then accepts an integer literal when that primitive accepts it (`:2342-2348`). `proposition_const_types_match` returns true from the primitive kinds alone when both spellings classify (`:2402-2403`), so the symbols in the `Named` arm (`:2406-2410`) are not consulted.

I did not read the code that builds a `TypeReference::Named`, so I cannot say a user declaration spelled like `u8` is stored there. Builtin spellings exist (`psi/foundation/symbols/src/builtin.rs:134-154`, read). I did not read whether a second declaration of that spelling is rejected before a named type is built.

What would decide it: the constructor of `TypeReference::Named` for a type annotation, and whether a declaration spelled `u8` can be the symbol (or an unresolved symbol) on a node whose name is `u8`.

Digest: the site does ignore the symbol. "Depends on whether shadowing is allowed" is still the open question.

## 3. Destructure field values left out of pattern dependencies

Verdict: UNDECIDED.

Search: `match_pattern_dependencies(` — 25 matches, including three definitions (`SRT/values.rs:2510`, `ST/values_and_types.rs:1503`, `TT/values.rs:3062`). The other 22 are call sites. I read the SRT definition and two SRT consumers. I did not read the other 20 call sites, and I did not separate their receiver types.

Read in full: `ExpressionTable::match_pattern_dependencies` (`SRT/values.rs:2510-2539`) and `assign_pattern_field_symbols` (`symbols_expression_paths.rs:401-416`). Read the match arm of symbol assignment (`symbols_expressions.rs:933-966`) and the match arm of the constant dependency walk (`constant.rs:2146-2155`).

For a destructure the iterator is given an empty field span (`SRT/values.rs:2518-2519`), so an `Equal` field value is never yielded. The symbol walk then calls `assign_pattern_field_symbols`, which writes `field_symbol` and does not visit `field.action` (`symbols_expression_paths.rs:410-416`). The constant walk queues only what the iterator yields (`constant.rs:2153`). A search of `constant.rs` for `match_fields`, `Destructure`, and `MatchFieldAction::Equal` returned no matches.

A name that appears only as a destructure field's compared value is therefore not queued by that walk and not symbol-assigned by the match arm. I did not read the function that reduces a match after the queue is built, so I cannot say the omitted expression is accepted rather than rejected when the match is reduced.

What would decide it: that reducer, and whether any other walk visits `MatchFieldAction::Equal` before acceptance.

Digest: the omission is real. The "23 call sites" count is 22 calls plus the syntax-trees and typed-trees definitions are separate functions; I did not resolve receiver types.

## 4. `copy_graph` shares type handles

Verdict: CLOSED.

Search: `.copy_from(` inside `symbol-resolved-trees` — 5 matches, all in `SRT/values.rs` (`:1706`, `:1811`, `:1894`, `:2289`, `:2367`). `pub(crate) mod tests` starts at `:1643` and `#[test]` functions start at `:1679`, `:1741`, `:1854`, `:2282`, `:2356`. I read the test at `:1679` through the call at `:1706`. The other four calls sit between those `#[test]` markers (inferred from the markers, those four bodies not opened). `copy_from_self(` — 6 matches: the definition (`SRT/values.rs:1196`), the `SymbolCopy` call (`:493`), three test calls (`:1723`, `:1904`, `:2291`, and `:2362`), and `constant.rs:5274`. `copy_expression_handles: true` as a field — 0 matches. The positional `true` is the test `statement_table_stores_transition_payloads_as_handles` (`control_flow_and_calls.rs:1390-1445`, read from the attribute through the call).

Read in full: `copy_graph` (`SRT/values.rs:1208-1451`), `copy_from` / `copy_from_self` (`:1188-1206`), `ExpressionSource` (`:1159-1176`), `copy_destructure_pattern` (`:1553-1582`), `StatementTransplant` (`control_flow_and_calls.rs:1115-1150`). Read `normalize_statements`, which builds the only non-test transplant I found, with `copy_expression_handles: false` (`storage_and_indexes.rs:454-466`). With that flag false, `copy_transition_pattern` is not called (`control_flow_and_calls.rs:1008-1015`).

`copy_graph` does copy `target_type`, `semantic_domain_arguments`, and `application_type` by handle (`SRT/values.rs:1309-1316` and `:1575`). Production uses `copy_from_self` (`constant.rs:5264-5274`, read at the call; `SymbolCopy` at `values.rs:493`). That copies inside the same expression table of the same program, so the handle still addresses that program's type arena. The cross-table entry `copy_from` is only called from the tests above.

Digest: the shared-handle description matches `copy_graph`. A program does not reach a destination arena through the calls that were found.

## 5. `SymbolCopy` leaves a call requirement and a fact's selection id

Verdict: UNDECIDED.

Search: `copy_proof_fact_with_symbols(` — 3 matches: definition `SRT/values.rs:426`, a test at `:915` (not opened), production `symbols.rs:4625` (read). `copy_expression_with_symbols(` — 4 matches, all in `SRT/values.rs` (`:412` definition, `:998`, `:1102`, `:1130`). Those three calls are under `mod tests` at `:868` (inferred from the module marker; the three test bodies were not opened). No production caller of `copy_expression_with_symbols`.

Read in full: `SymbolCopy::proof_fact` (`SRT/values.rs:449-477`), `symbol` (`:479-487`), the `Call` arm of `expression_payloads` (`:578-588`), `ProofMembershipFact` (`declarations_and_machines.rs:294-302`). Read `prepare_premises` (`symbols.rs:4445-4505`) and `copy_contract` (`:4617-4631`), not `telescope_correspondence` beyond its signature and return (`:4563-4614`, the body was read).

`prepare_premises` copies each member's `Requires` and `Crashes` facts onto the target-family interface (`symbols.rs:4487-4494`). `proof_fact` clones a membership and rewrites `value`, the domain path, `domain_symbol`, and `domain_arguments` (`SRT/values.rs:459-469`). It does not rewrite `authored_domain_selection` (`declarations_and_machines.rs:299-301`). The `Call` arm rewrites `target_symbol`, the binding's `selected`, and machine arguments (`SRT/values.rs:581-587`) and does not rewrite `requirement_application` (`:3138` is the field; the arm has no assignment to it).

I did not read the later check that uses the interface's copied contracts. An untouched requirement handle still names the member's type node; an untouched selection id is shared with the member. Either might make a later check fail closed, or let an interface requirement be treated as satisfied.

What would decide it: the consumer of `machine.contracts` after `target_family_contracts_prepared` is set (`symbols.rs:4502`), for a fact that is a call with a requirement application, or a membership with an authored selection id.

Digest: both omissions match the code that was read. `requirement_application` is a type handle, not a symbol. `authored_domain_selection` is an occurrence id, not a symbol.

## Entries not reached

6 onward. The open list continues at `SRT/lib.rs:725`, `SRT/type_system_and_names.rs:370`, the `PartialEq` / `as u32` sites, then syntax trees, the codec, semantics, terminal psi, and typed trees.

## What I did not read

- The other 15 `.parameter_position(` matches, and the bodies of the two `resolve_expression_table_call_target_symbol` callers past the call.
- The constructor of `TypeReference::Named`, and the duplicate-declaration check for builtin spellings.
- 20 of the 22 `match_pattern_dependencies` call sites, and the constant match reducer.
- Test bodies at `SRT/values.rs:1741`, `:1854`, `:2282`, `:2356`, and the `copy_expression_with_symbols` tests.
- Any checker of the copied target-family contracts.
- No codec, semantics, terminal-psi, or typed-trees open entry yet. Subagent notes under `/tmp/area12/` were not used for the verdicts above.
