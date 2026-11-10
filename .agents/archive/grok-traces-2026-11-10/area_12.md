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
| 6 | SRT/lib.rs:725 | UNDECIDED | An authored nominal machine parameter has no contract view, so hoist inference keeps the declared unit type; the later check was not read. |
| 7 | SRT/type_system_and_names.rs:370 | CLOSED | A missing owner row is the invalid owner the only writer refuses to store, and lowering records nothing. |
| 8 | SRT/storage_and_indexes.rs:104-108 | UNDECIDED | Index equality is always true and the trees derive `Eq`; no branch on that equality was read. |
| 9 | SRT/storage_and_indexes.rs:63 and 152 | UNDECIDED | `as u32` wraps only past 2^32 elements; one arena cannot get there, a concatenated statement vector can, and no such program was shown. |
| 10 | ST/lib.rs:1724 | CLOSED | A call, cast, or member is reported nameless, then both callers' evaluator rejects that shape. |

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

## 6. Authored nominal machine parameter has no contract view

Verdict: UNDECIDED.

Search: `machine_parameter_contract_view(` under `omega-rust/psi/compilation/pipeline` — 11 matches. `statement_lowering.rs:290` is `source_trees` (this function). `lowerer.rs:1208-1218` is `&TypedTrees` (read). The other nine are in stages 04–06; their receivers were not opened. Validation also calls a method of this name (seen in an earlier search); those bodies were not opened.

Read in full: `machine_parameter_contract_view` (`SRT/lib.rs:716-742`), `MachineParameterContract` (`declarations_and_machines.rs:154-167`), `infer_hoist_temp_type` (`statement_lowering.rs:211-367`). Read the two uses of its `Ok(None)` at `:595-598` and `:1377-1396`.

`AuthoredNominal` returns `None` (`SRT/lib.rs:725`), as does `RequirementIdentity` (`:721`). Inside `infer_hoist_temp_type` the `?` skips that parameter (`statement_lowering.rs:288-291`). If nothing else supplies a return type, the function returns `Ok(None)` (`:320-321`). For an inferred unit local, `None` keeps the declared type (`:1391-1395`), which this branch only reaches when that declared type is unit (`:1378-1381`). A call to a machine parameter whose contract is still the authored nominal form is therefore lowered as a unit temporary.

I did not read the later check of that temporary against the call. `None` at `:595-598` only skips one substitution and continues.

What would decide it: whether a unit-inferred local initialized by a call to an authored-nominal machine parameter is rejected after lowering.

Digest: the `None` arm matches. It is not only `AuthoredNominal`; `RequirementIdentity` returns `None` too.

## 7. Missing operand-owner row

Verdict: CLOSED.

Search: `.supplied_operand_owner(` — 14 matches. The table method that takes a handle is `SRT/type_system_and_names.rs:363`. Its lowering call is `type_reference.rs:2708` (read). `SRT/type_system_and_names.rs:690` and `:704` are the tree-node method, a different function (read). The syntax-tree hits (`ST/values_and_types.rs:224`, `ST/copies.rs:529`, `preparation.rs:3161`, `:3180`, `:3840`) are the syntax-tree table. The remaining pipeline hits (`type_reference.rs:1588`, `symbols_type_references.rs:1513`, `lowering.rs:4101`, `:4105`, `:4262`, `:4314`, `:4450`) call the no-argument node method; I read `:1513` and `:2704-2710` and inferred the others from the call shape (no extra argument).

Read in full: the table method (`SRT/type_system_and_names.rs:363-371`), the only `supplied_operand_owners.insert` (`:690-698`), `TypeReferenceNode` (`:768-809`), `retain_supplied_operand_owner` (`type_reference.rs:2716-2747`).

The node enum has no owner field. The only insert writes a side row when the tree node’s owner or lexical owner is valid (`:692-698`). A missing row is the invalid handle `unwrap_or_default()` returns (`:370`). Lowering then returns `Ok(())` without recording an owner (`type_reference.rs:2729-2730`). That is the same result as an owner that was never valid.

Digest: “no owner row reads as no owner” matches the lookup. It does not describe a second stored owner being ignored.

## 8. Resolution-index equality

Verdict: UNDECIDED.

Search: `resolution_indexes` — 24 matches, listed while tracing entry 1. None of those lines compare two index values. I did not search every `==` on a `SymbolResolvedTrees`.

Read: `PartialEq for ResolvedIndexes` (`SRT/storage_and_indexes.rs:104-108`) and the `SymbolResolvedTrees` derive (`SRT/lib.rs:91-102`). `eq` ignores both sides and returns true. Because the trees derive `Eq`, two trees that differ only in this cache compare equal.

What would decide it: a production branch on that equality. I did not find one, and I did not look through every `==`.

Digest: the always-true comparison matches the code.

## 9. Position truncated to `u32`

Verdict: UNDECIDED.

The casts are `SRT/storage_and_indexes.rs:63` (`position as u32` while filling the prefix cache) and `:152` (`position as u32` in `data_definition_position`). Search: `data_definition_position(` — 3 matches: the definition (`:140`) and calls at `symbols.rs:4078` and `symbols_expression_paths.rs:288`. Both calls were seen in entry 1’s reader (`:286-288` indexes with `as usize`). I did not read the function around `symbols.rs:4078` past the call.

`Handle::arena_index` is `u32` (`psi/foundation/arena/src/arena.rs:54`, read). `next_arena_index` panics when the next slot does not fit in `u32` (`arena.rs:1412-1416`, read). One arena therefore never yields an index that `as u32` changes. The assembly walker builds `sequence` by appending one span to a copy of another (`symbols_statements.rs:94-95`, read in entry 1). Those two lengths can sum past 2^32 even though each span cannot. No check of that sum was found. I did not show a source program that reaches it.

What would decide it: whether that concatenated vector can be longer than 2^32 before `local_statement_position` runs.

Digest: the cast is real. “Practically unreachable” is not established for the concatenated vector.

## 10. Const expression “contains no name”

Verdict: CLOSED.

Search: `const_expression_contains_name(` — 5 matches: the definition (`ST/lib.rs:1716`) and the two recursive calls (`:1720`, `:1723`), plus `preparation_const_evaluation.rs:1504` and `:3239`. All five were read.

Read in full: `const_expression_contains_name` (`ST/lib.rs:1716-1725`), `requires_const_operator_selection` (`preparation_const_evaluation.rs:174-189`), `evaluate_const_argument_expression` through its wildcard (`:784-930`), `anonymous_numeric_expression` (`:354-397`). Read both caller arms (`:1503-1536` and `:3238-3262`).

Only a name, or a binary or unary that reaches a name, returns true (`ST/lib.rs:1718-1724`). A call, cast, or member returns false. Both callers treat false as “evaluate it”. `requires_const_operator_selection` is also false for those shapes (`preparation_const_evaluation.rs:188`). `anonymous_numeric_expression` returns false for them (`:393`). The evaluator then hits `_ => Err("expression is not a symbolic integer const expression")` (`:929`). Caller one turns that into `Diagnostic::error` (`:1521-1525`). Caller two turns it into `Err` (`:3257-3258`).

Digest: the function does answer false for a call, cast, or member. Both cited callers reject that shape rather than accepting a nameless initializer.

## Entries not reached

11 onward. The open list continues with the second syntax-tree page (`ST/control_flow_and_names.rs:759-764`), the syscall/use defaults, the inspection renderer, then the codec, semantics, terminal psi, and typed trees.

## What I did not read

- The other 15 `.parameter_position(` matches, and the bodies of the two `resolve_expression_table_call_target_symbol` callers past the call.
- The constructor of `TypeReference::Named`, and the duplicate-declaration check for builtin spellings.
- 20 of the 22 `match_pattern_dependencies` call sites, and the constant match reducer.
- Test bodies at `SRT/values.rs:1741`, `:1854`, `:2282`, `:2356`, and the `copy_expression_with_symbols` tests.
- Any checker of the copied target-family contracts.
- The stage 04–06 and validation callers of `machine_parameter_contract_view`, and the check of a unit temporary after `infer_hoist_temp_type`.
- The function body around `symbols.rs:4078`, and every `==` on `SymbolResolvedTrees`.
- No codec, semantics, terminal-psi, or typed-trees open entry yet. Notes under `/tmp/area12/` were not used for these verdicts.
