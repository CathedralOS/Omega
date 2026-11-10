# slice_0132

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1013 | omega-rust/psi/compilation/representations/symbol-resolved-trees/src/storage_and_indexes.rs:37-69 | REACHABLE | Freshness uses slice address plus three sampled row hashes; a cache hit can leave a stale ordinal for a symbol still queried at symbols_expression_paths.rs:245/253. |
| A1014 | omega-rust/psi/compilation/representations/symbol-resolved-trees/src/type_system_and_names.rs:895-908 | UNDECIDED | `Named::primitive_type` ignores `symbol` and keys only on spelling; whether a non-builtin symbol can remain spelled `u8` was not traced in binding. |
| A1015 | omega-rust/psi/compilation/representations/symbol-resolved-trees/src/values.rs:2510-2538 | REACHABLE | `Destructure` passes an empty field span into the dependency walk, so `Equal` subpatterns in fields never reach the 22 `match_pattern_dependencies` call sites. |
| A1016 | omega-rust/psi/compilation/representations/symbol-resolved-trees/src/values.rs:1309-1338,1410,1427-1430,1575 | REACHABLE | `copy_graph` copies several type handles verbatim across tables while `SymbolCopy` remaps them via `type_reference` (values.rs:695-707). |
| A1017 | omega-rust/psi/compilation/representations/symbol-resolved-trees/src/values.rs:578-588,459-469 | REACHABLE | `SymbolCopy` leaves `Call.requirement_application` and `ProofMembershipFact.authored_domain_selection` unchanged while remapping adjacent handles. |
| A1018 | omega-rust/psi/compilation/representations/symbol-resolved-trees/src/lib.rs:725, type_system_and_names.rs:363-377, storage_and_indexes.rs:63,104-108 | UNDECIDED | `AuthoredNominal` yields no contract view and missing owner rows default out; whether any accepting path treats that as success was not settled end-to-end. |
| A1019 | omega-rust/psi/compilation/representations/syntax-trees/src/lib.rs:1716-1725 | REACHABLE | Calls, casts, and members are not “contains name”; const-generic preparation then skips or evaluates where name-driven handling would apply (preparation_const_evaluation.rs:1504,3239). |
| A1020 | omega-rust/psi/compilation/representations/syntax-trees/src/control_flow_and_names.rs:759-764, declarations.rs:735-741, inspection.rs:406-408 | CLOSED | Duplicate retained-source generations are dropped and invalid snapshot handles render empty; defaults apply only to `Default` materialization, not an acceptance gate read here. |

## A1013

Digest area_12 preamble (read): path prefix `SRT/` = `symbol-resolved-trees/src/`; checkout is after commit `457adf4a11f` but cited paths match at read.

Opened `PrefixPositionMaps::position` in full (`storage_and_indexes.rs:37-69`, read). Cache key is slice pointer `start` (`:44`). `fresh` (`:53-56`) requires stored watermark ≤ current length and three `statement_sample` / parameter arena-index samples at indices `0`, `watermark/2`, `watermark-1` to match. When `fresh`, `by_symbol` is **not** cleared; only rows from `enumerate().skip(entry.watermark)` append (`:61-65`). A stale cached `u32` for a symbol is returned at `:68` when `fresh` is true but symbol order changed without changing those three samples and length.

Caller search: `\blocal_statement_position\b` in `omega-rust` — **2** matches (definition `storage_and_indexes.rs:160`, use `symbols_expression_paths.rs:253`). `parameter_position(parameters,` in pipeline `02_syntax-trees-to-symbol-resolved-trees` — **1** match (`symbols_expression_paths.rs:245`).

Read enclosing `call_target` resolver (`symbols_expression_paths.rs:194-264`, read). For `SymbolKind::Local`, input is `machine.prior_statements` slice and `name.symbol`; wrong `position` indexes the wrong statement and thus wrong `type_reference` (`:251-259`), propagating to receiver typing.

**Input that reaches the branch:** A machine body whose `prior_statements` slice is reused at the same address and length with unchanged sample fingerprints but permuted local/parameter ordering; a name lookup hits the cached ordinal.

**Verdict REACHABLE (read):** Smallest story: two compilations share one in-place statement vector address; locals reorder without changing the three sampled rows; a receiver path still resolves the local via cache and reads another local’s type.

## A1014

Opened `TypeReference::primitive_type` (`type_system_and_names.rs:895-908`, read). `TypeReference::Named { name, .. }` calls `PrimitiveType::from_name(name)` (`:899`) and does not consult `symbol` (`:179-183` shows `Named` carries `symbol`).

`from_name` (`:912-927`, read) maps spelling `"u8"` to `PrimitiveType::U8` regardless of symbol identity.

Downstream use read: `equality_and_exhaustiveness.rs:2336-2340` rejects non-primitive const binders via `type_reference.primitive_type()` (read). A false `Some(U8)` would accept integer literals on a non-scalar binding.

**Not traced:** Whether symbol resolution allows `TypeReference::Named` with spelling `u8` and a non-`BuiltinType` `symbol` (e.g. user alias shadowing). `symbols_type_references.rs` discusses shadowing (`:616` comment, read) but no rule forbidding primitive spellings on user symbols was read.

**Verdict UNDECIDED:** Need binding-stage proof that such a `Named` cannot reach `primitive_type()`, or a concrete program where it can.

## A1015

Opened `match_pattern_dependencies` (`values.rs:2510-2538`, read). For `MatchPattern::Destructure(pattern)` the tuple sets `fields` to `HandleSpan::empty()` (`:2518-2519`), not `pattern.fields`. Iterator chains head, `match_fields(fields)` with `Equal` actions only (`:2531-2536`), and `pattern.condition` mislabeled `guard` (`:2520,2538`) — `pattern.guard` is never included (read `TableDestructurePattern` at `:3298-3305`).

`MatchFieldAction` (`:3317-3321`, read): field subpatterns live under `Equal(ExpressionHandle)`; `Bind` is a name only.

Caller search: `\bmatch_pattern_dependencies\b` in `omega-rust` — **22** call sites (17 files), including `constant.rs:1585`, `symbols.rs:370`, `selection_authored.rs:1146` (read samples).

**Input:** Match arm with `Destructure` and a field `Equal` carrying a call or name expression; consumers that enqueue only `match_pattern_dependencies` omit that expression (e.g. `constant.rs:1585` extends children from dependencies before `arm.value`).

**Verdict REACHABLE (read):** Smallest story: destructure field pattern `Equal(call)`; constant-folding queue never visits the call subtree.

## A1016

Read `copy_graph` (`values.rs:1208-1451`, read) and `SymbolCopy::type_reference` (`:695-707`, read). On `ExpressionNode::Cast`, `target_type: cast.target_type` is copied unchanged (`:1309`). `StructLiteral.application_type`, `TypeExpression`, and `ZeroValue` embed type handles without remap (`:1410-1410`, `:1427-1430`). `copy_destructure_pattern` copies `application_type: pattern.application_type` unchanged (`:1575`).

`ExpressionTable::copy_from` uses `ExpressionSource::Table` (`:1188-1193`, read). Test `table_and_own_copies_append_identical_rows` (`:2283-2297`, read) copies within cloned tables sharing one type arena; cross-arena copy is the hazard described in the entry.

**Input:** `copy_from` from source program table into destination program with separate `child_type_references` arena; expression contains `Cast` or `StructLiteral` with handles valid only in source arena.

**Verdict REACHABLE (read):** Destination table keeps source type handles; later lookup in destination arena can resolve the wrong type or default.

## A1017

Read `SymbolCopy::expression_payloads` `Call` arm (`values.rs:578-588`, read): remaps `target_symbol` and `authored_call_binding.selected` but not `requirement_application` (`TableCallExpression` field at `:3138` inferred from struct use at `:1338`).

Read `proof_fact` `Membership` arm (`:459-469`, read): remaps value, domain paths, symbols, and `domain_arguments`; `authored_domain_selection` on `ProofMembershipFact` (`declarations_and_machines.rs:299-301`, read) is untouched.

Contrast `copy_graph` `Call` (`:1338`, read): also copies `requirement_application` verbatim.

**Input:** Program copy with symbol substitution map; call’s `requirement_application` handle still indexes source type table; membership fact keeps stale occurrence id after rebase/copy.

**Verdict REACHABLE (read):** Smallest story: copied call retains old requirement type handle; later validation reads wrong requirement application type.

## A1018

Read `machine_parameter_contract_view` (`lib.rs:716-741`, read): `AuthoredNominal { .. }` => `None` (`:725`).

Read `supplied_operand_owner` / `operand_lexical_owner` (`type_system_and_names.rs:363-377`, read): missing `supplied_operand_owners` row yields `OperandLifetimeOwnerHandle::default()` and `SymbolHandle::default()` via `unwrap_or_default()` (`:370`, `:377`).

Read `ResolvedIndexes::PartialEq` (`storage_and_indexes.rs:104-108`, read): always `true`. Read `position as u32` (`:63`, read).

Sample callers read: `lowerer.rs:1215-1219` returns `false` when view is `None` (fail-closed for compatibility, read). `proof_contracts_core.rs:1028-1030` `continue`s when view is `None` (skips signature expansion, read). Not all validation paths were read to see whether skip equals accept.

**Verdict UNDECIDED:** Whether `AuthoredNominal` or default owner handles let an ill-typed program compile requires tracing each `machine_parameter_contract_view` and owner consumer through rejection vs silent continue; not completed here.

## A1019

Read `const_expression_contains_name` (`syntax-trees/src/lib.rs:1716-1725`, read): only `Name`, `Binary`, `Unary`; all other nodes including `Call`, `Cast`, `Member` => `false`.

Read callers (`preparation_const_evaluation.rs:1504-1507`, read): if `const_expression_contains_name(expression)` then `continue` (skip const-argument replacement for `TypeReferenceNode::ConstExpression`). (`:3239-3242`, read): same predicate returns `Ok(None)` for index-argument const expressions.

**Input:** Const generic index or const type argument written as `target(0)` or `v as usize` with no bare `Name` at root; predicate false; pipeline evaluates or defers instead of name/selection path.

**Verdict REACHABLE (read):** Smallest story: const index expression is a call; compiler treats it as name-free and attempts evaluation where name-based const rules would reject or require selection.

## A1020

Read `SyntaxTextOwner::retain_from` (`control_flow_and_names.rs:757-765`, read): on duplicate `generation`, `binary_search_by_key` returns `Err` and **no** insert — second page dropped (read).

Read `Default for ExternalBinding` and `Item` (`declarations.rs:735-744`, read): `Syscall { number: 0 }` and `Item::Use`.

Read `snapshot_expression_handle` (`inspection.rs:406-408`, read): invalid handle => empty name snapshot (display JSON path, read).

No caller was read that uses `PartialEq` on `ResolvedIndexes` or `retain_from` duplication during compilation acceptance; inspection snapshot is diagnostic output.

**Verdict CLOSED (read):** Acceptance not shown to pass through silent source drop or `Default` bindings in the paths read; invalid expression snapshot is display-only.

## What I did not read

- Full bodies of all `primitive_type()` consumers in `omega-rust/psi/compilation/validation`.
- Symbol-binding rules forbidding user types spelled like primitives (`symbols_type_references.rs` beyond comments).
- All `machine_parameter_contract_view` call sites beyond the samples above.
- `compiler_storage::Record` decode paths that might materialize `Item::default()` / `ExternalBinding::default()` from bytes.
- Callers of `SyntaxTextOwner::retain_from` / `adopt` beyond the defining module.
- Tests in `storage_and_indexes.rs` and `values.rs` except where cited above.

SLICE COMPLETE
