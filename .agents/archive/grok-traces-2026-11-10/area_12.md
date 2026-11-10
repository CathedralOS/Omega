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
| 11 | ST/control_flow_and_names.rs:759-764 | CLOSED | A repeated generation is a second copy of a page the counter already issued, and a conflicting text fails record decode. |
| 12 | ST/declarations.rs:735-741 | UNDECIDED | `Default` is syscall 0 and `Use`; no caller of those defaults was found. |
| 13 | ST/inspection.rs:406-408 | CLOSED | An invalid expression becomes an empty snapshot name, and every snapshot caller found is a test or JSON dump. |
| 14 | codec/foundation_validation.rs:11215-11218 | REACHABLE | A wrapping integer add with a scalar result is admitted even when the operands are not integers of that carrier. |
| 15 | codec/foundation_validation.rs:142-166 | CLOSED | Every static call kind is passed through `validate_module_invocation_entry`, which checks structural arguments and claims. |
| 16 | codec/foundation_validation.rs:292-361 | REACHABLE | A dynamic call with no table row keeps a structural argument whose place type is not the slot's. |
| 17 | codec/foundation_validation.rs:10483-10502 | REACHABLE | A boundary-application demand may name an ordinary operation that merely exists. |
| 18 | codec/foundation_validation.rs:4613 | REACHABLE | A machine `requires` atom, case membership, byte or float comparison, or content equation is not term-checked. |
| 19 | codec/foundation_validation.rs:3027 and 3190 | REACHABLE | A proof-scope call whose callee id is 0 is not resolved and is not rejected. |
| 20 | codec/foundation_validation.rs:9401-9415 | REACHABLE | Task-activation carry flags and crossings are not checked against the frames. |

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

## 11. Second source page with the same generation

Verdict: CLOSED.

Search: `retain_from` and `adopt` — definitions at `ST/control_flow_and_names.rs:757` and `:768`; production `copies.rs:1052` (`destination.text.retain_from`) and `ST/lib.rs:1810` plus `:1871-1876` (`adopt`). The other hits are tests (`control_flow_and_names.rs:831`, `:923`, `:942`).

Read in full: `retain`, `retain_from`, `adopt`, `source_text` (`ST/control_flow_and_names.rs:746-798`) and `Record::read` (`:879-908`). Read the conflict test (`:948-962`).

`retain` takes the next process-wide generation and panics if the counter would wrap (`:748-754`). `retain_from` and `adopt` insert only when `binary_search` says the generation is absent (`:759-764`, `:770-775`). A generation that is already present keeps the text already stored. `Record::read` remaps generations (`:889`) and rejects a non-increasing original (`:896-899`). The test builds a second page with the same generation and different text and expects decode to fail (`:959-961`).

Digest: the in-memory skip is real. It does not replace a live page with different text.

## 12. Default syscall zero and default use

Verdict: UNDECIDED.

Search: `ExternalBinding::default` and `Item::default()` — 1 match, the `Item` impl body at `ST/declarations.rs:743`. No call.

Read: `Default for ExternalBinding` (`ST/declarations.rs:735-738`) returns `Syscall { number: 0 }`. `Default for Item` (`:740-744`) returns `Use`. `ExternalBinding` does not derive `Default`; the impl is hand-written. I did not read the `compiler_storage::Record` derive to see whether it fills a missing variant from `Default`.

What would decide it: a reader or constructor that uses these defaults when the source did not write a syscall or a use.

Digest: the defaults match the code. Nothing read shows them becoming an accepted binding.

## 13. Invalid expression snapshots as an empty name

Verdict: CLOSED.

Search: `snapshot_expression_handle(` — 48 matches, all in `ST/inspection.rs` (the definition at `:402` and snapshot builders). `SyntaxTrees::snapshot` callers outside that file: `ST/lib.rs:1086`, `:1141`, `:1217` (read `:1075-1086`, a test that panics on a mismatch) and `pipeline/01_tokens-to-syntax-trees/src/tests.rs` via `snapshot_json` (grep lines `:278`, `:5495`, `:5580`, `:5650`, `:5709`, not opened; the path is `tests.rs`). `psi-compiler/src/lib.rs:446` is `assert_eq!` on two snapshots (grep line only).

Read: the invalid-handle arm (`ST/inspection.rs:406-408`) returns `ExpressionSnapshot::Name { path: Vec::new() }`. `snapshot` (`:2668`) and `snapshot_json` (`:2683-2684`) only build that snapshot.

An invalid handle renders as an empty name in a snapshot. No production acceptance branch was found that reads it.

Digest: “display only” matches the callers that were found.

## 14. Operation catch-all checks only a scalar result

Verdict: REACHABLE.

Search: `validate_operation_foundation(` — 2 matches, definition `codec/foundation_validation.rs:10877` and the call ` :2322`. `WrappingIntegerAdd` in that file — 0 matches. In `terminal-psi` — wire and semantics files only (`block_wire.rs:1626`, `term_wire.rs:2551`, `control_flow.rs:1535`, `sem/scalars.rs`, `sem/lib.rs:399`, `sem/substitution.rs:861`, `TP/types_and_values.rs:1606`). Not in `foundation_validation.rs`.

Read: `validate_operation_foundation` from `:10877` through the start of the match `:10923`, and the wildcard `:11215-11220`. Read `decode_module` (`semantic_module.rs:4093-4119`), the call `validate_machine` for every machine (`foundation_validation.rs:10472-10474`), and the per-operation loop (`:2320-2328`). The match arms between `:10924` and `:11214` were not read one by one; the identifier search is what shows this kind has no arm.

Every decoded module runs this check (`semantic_module.rs:4109`). The wildcard rejects a result that is not `OperationResult::Scalar` (`foundation_validation.rs:11216-11217`) and then returns `Ok(())` (`:11221`). A wrapping integer add whose result is a scalar is therefore accepted here even when the two operands are missing or are booleans, or the scalar carrier is not the integer width the operation uses. A non-scalar result of that operation is rejected by this same arm.

Digest: the catch-all description matches. The kinds that fall through were not listed in the digest; wrapping integer add is one of them.

## 15. Static calls and structural arguments

Verdict: CLOSED.

The digest marks this open item corrected. The correction holds.

Search: `validate_module_invocation_entry(` — definition `codec/foundation_validation.rs:61`, call `:2332`. `fn module_invocation` — `TP/control_flow.rs:387` (read).

Read in full: `validate_call_tail` (`codec/foundation_validation.rs:29-59`) and `validate_module_invocation_entry` (`:61-126`). Read the call from `validate_machine` (`:2331-2336`) and `OperationKind::module_invocation` (`TP/control_flow.rs:387-472`).

`validate_machine` runs `validate_module_invocation_entry` for every `machine.module_invocations()` after the per-operation pass. That function always ends in `validate_call_tail` (`:119-125`), which always calls `validate_structural_arguments` (`:36-42`) and `validate_claim_actions` (`:43-49`). `module_invocation` returns inputs for `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructuralWithScalarArguments`, and `CallStructural` (`TP/control_flow.rs:398-471`). Other kinds return `None` (`:472`). `Call` passes empty structural-argument and claim slices (`:410-411`) because that variant has no such fields. The other four pass the slices they store.

Digest: the old claim that this file never checks those arguments does not match. The correction in the digest does.

## 16. Dynamic call with no table row

Verdict: REACHABLE.

Search: `validate_call_dynamic(` — definition `codec/foundation_validation.rs:168`, call `:11107` inside `validate_operation_foundation`. `validate_dynamic_descriptor_results` — `semantic_module.rs:1254`, called from `decode_module` at `:4114`.

Read in full: `validate_call_dynamic` (`codec/foundation_validation.rs:168-370`). Read the `None` row skip (`semantic_module.rs:1265-1268`) and the `CallDynamic` arm (`foundation_validation.rs:11107`).

Counts and the result shape are checked against the slot (`:226-229`). Structural types are compared only inside the table-row loop (`:354-360`), and `flatten` drops a `None` row (`:297`). A missing table, or a table whose row for the slot is `None`, skips that comparison. `validate_dynamic_descriptor_results` allows the `None` (`semantic_module.rs:1266-1268`) and does not require a table to exist. A dynamic call whose slot has one structural parameter, and which has no row, is accepted when that argument's place has a different type or is not declared, as long as the count matches.

Digest: "count only" matches the structural arguments when no row exists. Claims that are present are still compared with the slot (`codec/foundation_validation.rs:285-290`). A missing claim transfer is not required even when a row exists; that is wider than the digest's "no row" case.

## 17. Boundary application demand

Verdict: REACHABLE.

Search: `validate_boundary_application_operations(` — definition `codec/foundation_validation.rs:10483`, call `:10286` inside `validate_structural_foundation` (`:10256`), which `decode_module` runs (`semantic_module.rs:4109`).

Read in full: `validate_boundary_application_operations` (`codec/foundation_validation.rs:10483-10503`).

The function binary-searches demands by operation id and rejects a missing or repeated id (`:10490-10501`). It never reads `operation.kind`. A demand whose operation id is an ordinary operation that exists in some block, such as an integer constant, is accepted. Nothing in this function requires a `BoundaryCall`.

Digest: matches the function that was read.

## 18. Proposition variants left out of term checking

Verdict: REACHABLE.

Search: the match is `scalar_observations::validate` at `codec/foundation_validation.rs:4551-4614`. `Proposition` variants are `psi/foundation/semantic-vocabulary/src/proposition.rs:1806-1847` (read). `scalar_observations::machine` calls `validate` (`foundation_validation.rs:4392`). `validate_machine` calls `machine` (`:2309`), and `decode_module` runs `validate_machine` for every machine (`:10473-10474`).

Read: the match (`:4551-4614`) and the call loop (`:4361-4392`).

The wildcard is `_ => {}` (`:4613`). The arms that do check terms are integer-math comparisons, `StructuralIdentity`, `Equal`, `LessThan`, `LessOrEqual`, `ScalarIeeeFloatComparison`, `Conjunction`, `Disjunction`, and `Implication`. Not named, so skipped: `Truth`, `Falsehood`, `Atom`, `IeeeFloatComparison`, `ByteSequenceEqual`, `StructuralCaseMembership`, `ContentConservation`. `Truth` and `Falsehood` carry no terms. An `Atom` whose proposition id has no application, a case membership whose place is not in the machine, a byte or float comparison with an unknown root, or a content equation whose places are unknown, is accepted on a machine `requires` list.

Digest: matches. `Truth` and `Falsehood` are also in the wildcard and have nothing to check.

## 19. Zero proof-scope ids

Verdict: REACHABLE.

Search: `owner.0 != 0` — `codec/foundation_validation.rs:3027`. `identity.0 != 0` — `:3190`. `proof_scope::validate(` — definition `:2892`, call `scalar_applications::validate` at `:3536`, which `validate_structural_foundation` runs (`:10257`).

Read: the declaration closure (`:2896-2901`), the pending-owner loop (`:3026-3029`), and the call-identity loop (`:3188-3193`). The rest of `proof_scope::validate` was not read.

`declaration` rejects an id that is not in the scope (`:2897-2900`). Both loops skip that check when the id's number is 0. A proof-scope call whose callee id is 0, or a pending-evaluation condition whose owner id is 0, is accepted.

Digest: matches the two skips that were read.

## 20. Task activation carry and crossings

Verdict: REACHABLE.

Search: `carry_obligations` in `foundation_validation.rs` — 0 matches. The field is `TP/terminal_module.rs:973` (read with the struct at `:903-906`). `.crossings` in `foundation_validation.rs` — 1 match, `:9401`. `task_activation::validate(` — definition `:9208`, call `:10258`.

Read: the empty-bundle return (`:9208-9216`), the frame-call check (`:9381-9384`), and the crossing loop (`:9401-9415`). The body from `:9217` to `:9359` was not read. The identifier search shows `carry_obligations` is not read anywhere in the file, including that unread span.

A demand can set `carry_obligations.preserve_cpu` false while a crossing sets `preserve_cpu` true (`TP/terminal_module.rs:896` and `:904`). The crossing loop checks a non-zero identity and live place ids. It does not look at a frame. The frame loop checks that a frame call names a frame. Neither side compares the two lists. A task-activation demand with one frame and one crossing whose CPU-preservation flag disagrees with `carry_obligations` is accepted.

Digest: both unchecked claims match what was searched and read.

## Entries not reached

21 onward, in digest order: `codec/mathematical_proof_actual.rs:776`; `codec/sections.rs:95-104`; `codec/module_wire.rs:3395`; `codec/semantic_module.rs:323`, `:433`, `:522-531`; `:74-80`; `:4072-4076`; `codec/term_wire.rs:1543-1548` (the digest already checked this one); `codec/structural_wire.rs:1901-1905` and `:1942-1945`; `codec/term_wire.rs:754-760` and `:893`. Then every open bullet under terminal-psi/semantics, terminal-psi, and typed-trees in the digest. Those sites were not opened for this report.

## Commit

Entries 1–20 are commit `b7b57ae4351c7fcdc600e580457eeabd9eaebbb0`.

## Counts

| verdict | count |
| --- | --- |
| REACHABLE | 6 |
| CLOSED | 6 |
| UNDECIDED | 8 |
| not reached | the entries listed above |

## What I did not read

- The other 15 `.parameter_position(` matches, and the bodies of the two `resolve_expression_table_call_target_symbol` callers past the call.
- The constructor of `TypeReference::Named`, and the duplicate-declaration check for builtin spellings.
- 20 of the 22 `match_pattern_dependencies` call sites, and the constant match reducer.
- Test bodies at `SRT/values.rs:1741`, `:1854`, `:2282`, `:2356`, and the `copy_expression_with_symbols` tests.
- Any checker of the copied target-family contracts.
- The stage 04–06 and validation callers of `machine_parameter_contract_view`, and the check of a unit temporary after `infer_hoist_temp_type`.
- The function body around `symbols.rs:4078`, and every `==` on `SymbolResolvedTrees`.
- The `Record` derive for `ExternalBinding` and `Item`.
- The operation-foundation match arms between `codec/foundation_validation.rs:10924` and `:11214`, and `activation_storage::validate_module`.
- Codec entries after 15, and all semantics, terminal-psi, and typed-trees open entries. Notes under `/tmp/area12/` were used as a map for entry 14's kind list and were checked against the grep and the lines cited above; they were not copied as verdicts.
