# slice_0128

Digest preamble: area_07 (`digests/area_07.md`) — Psi front (stages 00–04) and build-time evaluation; path shorthand `S02/` = `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/`. Claims: area_07 ACCEPT rows A0984–A0991.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0984 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/preparation_type_equations.rs:1984-1988 | REACHABLE | Non-generic heads and unselected or non-data bases return `Ok(())`, so unsolved generic equations are not checked on those paths. |
| A0985 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolution.rs:1921-1928 | REACHABLE | A valid `owner` bypasses `bind_roots!` and never checks the symbol names a declaration row in the resolved trees. |
| A0986 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection.rs:916-928 | REACHABLE | Hidden default-body calls are errors only when no top-level Machine, Operator, or BuiltinFunction matches the spelled name. |
| A0987 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/selection_authored.rs:371-373 | CLOSED | Skipping authored selection for bare `code_layout` is paired with stage-03 `native_code_layout::lower_request`, which rejects invalid operands. |
| A0988 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols.rs:4003-4006 | REACHABLE | Two visible attachments with the same entry name keep the first match and never report ambiguity. |
| A0989 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_expressions.rs:314-318 | REACHABLE | Membership and qualified-case ambiguity is cleared here; later diagnostics apply only on authored-selection surfaces. |
| A0990 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_statements.rs:257-260 | REACHABLE | Non-local, non-parameter, non-field root-binding implementation operands become invalid with no stage-02 diagnostic. |
| A0991 | omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/symbols_symbol_table.rs:1097 | REACHABLE | Exhausted child stream or kind mismatch yields invalid declaration symbols with no error. |

## A0984

Digest: area_07 (`digests/area_07.md:290`, commit `457adf4a11f`). Read `preparation_type_equations.rs:1974-1997` (`validate_materialized_type_equations`, read).

At `preparation_type_equations.rs:1979-1982` (read), non-`Generic` references return `Ok(())`. At `1984-1985` (read), `selected_data_item` returning `None` returns `Ok(())`. At `1987-1988` (read), a selected item that is not `Data` returns `Ok(())`. Only when base data is found does `template_type_equation_offsets` at `1990-1994` (read) emit the unsolved-equation error.

Search `\bvalidate_materialized_type_equations\(` in `omega-rust`: 2 matches — definition `preparation_type_equations.rs:1974`, one production call `lowering.rs:4127` (read). Re-export `preparation_generic_data.rs:1735` (read line from grep).

Caller `lowering.rs:4118-4131` (`lower_type_reference_contents`, read): invokes validation when `!lowerer.structural_type_equations_pending` (`4126-4131`). When pending, validation is skipped entirely (`4126-4127`, read).

Input reaching the branch: a generic type reference whose base name does not resolve to a data declaration under the current `ConstantSelection` (or resolves to a non-data item), while lowering still materializes the reference.

Smallest program (plain words): a generic application spelling a type constructor name that preparation cannot select as data passes this check and keeps lowering, even if that constructor would retain unsolved type equations once selected.

## A0985

Digest: area_07 (`digests/area_07.md:291`; `records/unit_042.md:406`, open). Read `resolution.rs:1914-1961` (`type_inputs::bind`, read) and `resolution.rs:1897-1910` (`TypeInputScope::finish`, read).

At `resolution.rs:1921-1928` (read), `input.owner.is_valid()` pairs every pending argument with `input.owner` and `continue`s without `bind_roots!`. At `1932-1936` (read), the invalid-owner path requires each new root declaration symbol in the watermark range to be valid or returns `"pending type input has an unbound declaration owner"`.

Search `type_inputs::bind(` in `omega-rust`: 2 calls — `resolution.rs:386` (deferred body completion, read) and `resolution.rs:2182` (`index_deferred`, read).

Valid owner is supplied from `lowering_machine.rs:1659-1660` (read): `complete_machine_body` passes `machines[pending.machine_ordinal].symbol`. Item lowering uses `SymbolHandle::invalid()` at `lowering.rs:1801` (read).

Input: pending type arguments recorded during machine body completion bind to the machine owner handle. If that handle is valid in the encoding but the machine row never received a table symbol (`symbols_symbol_table.rs:1122-1125`, read — `unwrap_or_default()` on missing child), `bind` never walks roots and does not emit the unbound-declaration error.

Smallest program (plain words): a machine whose symbol slot is valid-looking while the root symbol install failed silently; pending generic type arguments from the body still bind to that handle without checking the resolved trees roster.

## A0986

Digest: area_07 (`digests/area_07.md:292`). Read `selection.rs:855-944` (`route_conformance_requirement_calls`, read).

Hidden default-body calls are collected at `892-908` (read) when routing fails and the member is in `hidden_names`. Diagnostics are built at `914-938` (read) only for calls where `find_top_level_declaration_or_operator_family_from_source` returns `None` (`916-928`).

Search `route_conformance_requirement_calls` in `omega-rust`: 2 matches — definition `selection.rs:871`, sole production call `resolution.rs:2641` inside `finish_resolved_tables` (read `26236-2647`).

Input: a trait default body calls a name that is not a declared requirement but matches any visible top-level machine, operator, or builtin with the same spelling (including the wrong one when several exist).

Smallest program (plain words): a default body invokes a helper name absent from the trait’s requirement list while a different top-level machine shares that spelling; resolution finishes without error.

## A0987

Digest: area_07 (`digests/area_07.md:293`). Read `selection_authored.rs:359-375` (statement candidates, read) and `selection_authored.rs:1240-1247` (expression candidates, read). Both exclude `code_layout` when target symbol and receiver are invalid (`371-373`, `1244-1246`, read).

Search `code_layout` in stage-03 lowering: read `expression_lowering.rs:1360-1374` (read) — bare `code_layout` calls take `native_code_layout::lower_request`. Read `expression_lowering.rs:439-465` (`lower_request`, read): requires exactly one machine-application operand with a valid machine/state/parameter symbol; otherwise returns errors at `447-450` or `461-464`.

Closing check: stage 02 may omit authored-selection ledger entries for this spelling by design; stage 03 still gates acceptance on operand shape before the call is typed (`1360-1371` → `439-465`, read).

Verdict: exclusion is harmless for acceptance because the dedicated lowering path enforces the contract.

## A0988

Digest: area_07 (`digests/area_07.md:294`; `records/unit_043.md:212`). Read `symbols.rs:3945-4009` (`attached_call_target`, read).

At `4003-4006` (read), once `selected.is_valid()`, later visible attachments with the same entry name are skipped (`continue`) with no ambiguity diagnostic.

Search `attached_call_target(` in `omega-rust`: 7 matches — 1 definition `symbols.rs:3945`; production uses `symbols_targets.rs` (2), `symbols_type_references.rs:1171`, `symbols_expression_paths.rs:342`, `symbols_expressions.rs:189` and `:501` (grep count; call sites not all re-opened).

Input: a data-owned type references two attached machines that expose the same entry state name, both visible from the use site.

Smallest program (plain words): a qualified call on attached machine entry state where two conforming attachments share the entry name; symbol assignment keeps the first roster attachment only.

## A0989

Digest: area_07 (`digests/area_07.md:295`; `records/unit_043.md:419a`). Read `symbols_expressions.rs:297-326` (`assign_membership_symbol`, read) and `symbols_expressions.rs:538-555` (`normalize_qualified_case_value`, read).

At `314-318` (read), `membership_selection` returning `Ok(None)` or `Err(_)` sets invalid domain/case symbols with no diagnostic. At `551-554` (read), `qualified_payload_free_case_type` returning `Err(_)` makes `normalize_qualified_case_value` return `true` without surfacing the message.

Later re-raise (read): `selection_authored.rs:1328-1329` maps `qualified_payload_free_case_type` errors to diagnostics for `ExpressionNode::Name` during authored candidate collection; `selection_authored.rs:290-292` (`normalize_case_membership_facts`) re-raises `membership_selection` errors for pending proof membership facts.

Search `assign_membership_symbol(` in `omega-rust`: 4 matches — 1 definition `symbols_expressions.rs:297`; calls from `symbols.rs:491`, `symbols.rs:1406`, `symbols_expressions.rs:1054` (grep count).

Input: an ordinary membership expression or qualified case value path with contested domain/case spelling, without an authored-selection occurrence on that expression.

Smallest program (plain words): an expression states membership in a domain name that matches two top-level domains; stage 02 leaves invalid membership symbols and does not error unless later authored-selection passes run on that site.

## A0990

Digest: area_07 (`digests/area_07.md:296`). Read `symbols_statements.rs:244-261` (read, within statement symbol assignment).

For `RootBinding`, when `implementation_operand` is present, the operand must be a `Name` whose symbol kind is `Local`, `Parameter`, or `Field` (`248-255`, read). Otherwise the handle is replaced with invalid (`257-260`, read) and no diagnostic is recorded.

Downstream read `statement_lowering.rs:914-928` (`root_bindings::lower`, read): invalid `implementation_operand` selects `RootBindingOperand::AuthoredPath` from the authored path members (`918-928`), not an error at this site.

Input: a root-binding statement whose implementation operand names a global or other non-local value.

Smallest program (plain words): a build root-binding names a module-level constant as the implementation descriptor; stage 02 silently drops the operand symbol and stage 03 treats the binding as an authored path instead of rejecting the shape.

## A0991

Digest: area_07 (`digests/area_07.md:297`). Read `symbols_symbol_table.rs:1093-1149` (root install, read) and `symbols_top_level.rs:1345-1358` (`next_child_of_kind`, read).

`install_root_family!` at `1097-1099` (read) assigns a symbol only when `root_children.next()` is `Some`; otherwise the declaration keeps its default. `machines` at `1122-1125` (read) uses `unwrap_or_default()` when the stream is short. Similar patterns at `1135-1138`, `1142-1143`, `1145-1148` (read).

`next_child_of_kind` at `1350-1357` (read) returns invalid when the next child’s kind mismatches, with no diagnostic.

Search `next_child_of_kind(` under `omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees`: many production call sites in `symbols_top_level.rs` (grep; not all re-read). Nothing in the read `install_root` loop checks that every roster entry received a symbol or that child streams are fully consumed.

Input: internal symbol-table construction where the preallocated child stream is shorter than the declaration roster or a child handle has the wrong kind.

Smallest program (plain words): a compilation unit whose declaration count exceeds the symbol child budget leaves one top-level machine with an invalid symbol; later passes that only test `is_valid()` may skip work instead of failing closed at install time.

## What I did not read

- Full bodies of every `attached_call_target` caller beyond grep counts (`symbols_targets.rs`, `symbols_type_references.rs`, `symbols_expression_paths.rs`).
- Whether stage 05 / validation always rejects programs that slip through A0984’s early `Ok(())` (only grep hits for structural type equations, not traced).
- `build_selections::lower` and downstream build admission for A0990 root bindings beyond `statement_lowering.rs:914-928`.
- Machine-equation `complete` loops and all consumers of `pending_type_inputs` in build-time evaluation (`const_evaluation.rs`, read not opened).
- `git show 457adf4a11f` line relocation (current checkout lines matched the digest citations on read).

SLICE COMPLETE
