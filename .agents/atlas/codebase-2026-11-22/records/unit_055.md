# Record: unit_055 (area 02-source-stages-00-04)

Root `P/` = `source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/`. Files are under `P/names/` unless a different folder is named. All 31 files in the unit were read in full. Counts below are Grep count-mode results over `source-rust/` (line matches, not call sites unless stated); I did not grep `tools/` except where noted. Unit-wide facts first, then one entry per file.

Unit-wide:
- Stage job: after the symbol table exists, `names/` gives every name occurrence in headers, contracts, statements and expressions a `SymbolHandle`; `selections/` records which declaration each authored occurrence chose. Most judgments are keyed on symbol identity (`SymbolKind`, parent, handles) and on source visibility (`SourceSpan` passed to `SymbolTable::*_from_source*` lookups). Spelling keys exist at the sites listed under "string compares" at the end.
- Pattern: most assignment passes walk a declaration's children "by position" in insertion order (`next_child_of_kind`, `top_level.rs:1352`), so `symbol_table.rs` insertion order and `top_level.rs` assignment order must agree (stated at `symbol_table.rs:25-27`).
- A `MachineScope { .. }` literal (13 fields) is written out in full in several places: `mod.rs:408`, `measures.rs:20`, `top_level.rs:446`, `type_references.rs` (6 line matches, one is a test), `statements.rs:800` (plus `..*machine` copies). Grep `MachineScope \{` in `names/`: contracts.rs 1, measures.rs 1, mod.rs 1, scopes.rs 1, statements.rs 9, top_level.rs 1, type_references.rs 6.

## names/expressions.rs (1606 lines)
1. Purpose: gives symbols to every expression node in a body, signature or contract: names, calls, members, struct literals, memberships, match patterns. Also rewrites two kinds of match arm (bare payload-free case, domain head) into destructure patterns. Contains two private modules `references` and `traversal`, then re-exports (`expressions.rs:1458-1467`), then tests.
2. Shape: `references::assign_call_symbol` (`:127`), `retain_unresolved_call` (`:196`), `unresolved_call_name` (`:225`), `assign_member_symbol` (`:239`), `assign_membership_symbol` (`:293`), `lexical_value_head` (`:329`), `assign_name_in_role` (`:428`) with `enum NameRole {Value, PatternHead}` (`:423`), `normalize_qualified_case_value` (`:585`), `assign_struct_literal_symbols` (`:625`). `traversal::ExpressionSymbolContext` (`:887`) with `assign_expression_table_symbols` (`:1048`), `normalize_payload_free_case_pattern` (`:1279`), `normalize_domain_pattern` (`:1340`). Macro `context_wrapped!` (`:756`).
3. In/out: reads `SymbolTable`, `MachineScope`, state parameters; mutates `ExpressionTable` (sets `target_symbol`, `head_symbol`, `member_symbol`, `unresolved_name`, struct field symbols) and child type-reference arena.
4. Decides: lexical head first (`lexical_value_head`: reverse scan of `prior_statements` locals, then parameters, then type-parameters of kind Type/Const/Value, `:344-373`), keyed on spelled name string compared to local/parameter names (`:349,355,363`). Then static path lookup via `resolve_state_scoped_table_path`. Value role with a Data/Module/invalid head tries `normalize_qualified_case_value` (`:475-484`). A `Name` stays unresolved-silent when `symbol.is_valid()`, `reserved_result_of`, or the name is `self` inside a Data/Domain machine (`predicate_subject`, `:560-566`, string compare `"self"`). Payload-free case arm is detected by: head `Name` selected symbol of kind `Variant` with no `Field` child (`:1294-1305`). Domain arm by `membership_selection` returning `Domain` (`:1359-1367`).
5. Cases by shape: nested receiver chains only handled when the chain is `self`-rooted, length >= 3, and every hop is a plain `Named` field type (`nested_receiver_leaf_field_symbol :79-103`, `nested_receiver_call_target_symbol :111-125`, via `MachineScope::nested_self_chain_type`, `scopes.rs:881`). `spelled_receiver_chain` (`:44`) handles `Name`, `Member`, `Borrow` only; anything else returns `None`. `ExpressionNode::Block` is skipped here and left to `statements::blocks` (`:1054-1057`).
6. Same job elsewhere: `propositions.rs:105 assign_expression_symbols_in_role` re-implements the same expression walk for proof surfaces with different lookup rules (owner-binder children, `FixedCoreDeclaration`); `selections/body_calls.rs:288 retarget_expression` and `selections/authored_selections.rs:1201 collect_expression_subtree` are two more full `ExpressionNode` walks; `repeated_locals.rs:216` a fourth. Same bare-case/domain-head rewrite is not repeated in `propositions.rs` (it only sets roles).
7. Unused/one-caller: all `references` items are `pub(super)`/`pub(in names)`. `assign_pattern_head_symbols` used in statements.rs:524 (count via grep of `assign_pattern_head_symbols`: not run separately; read). `assign_call_machine_argument_symbols` (defined `targets.rs:687`): Grep count 6 lines in 3 files (expressions.rs 2, statements.rs 2, targets.rs 2), so used by the expression path (`expressions.rs:175`), statement path (`statements.rs:465`) and transition code. The comment at `targets.rs:680-686` says "Body expression calls and statement calls share this one owner": confirmed 2 callers outside the definition.
8. Stated limits: `nested_self_chain_type` comment: "conservative, so an unsupported nested receiver keeps the existing loud unresolved-call error" (`scopes.rs:875`). Typing refuses blocks in propositions (`propositions.rs:128`).
9. Fail-open: none seen that accept a wrong program; the nearest are: (a) `assign_membership_symbol` maps `Ok(None) | Err(_)` to all-invalid symbols (`:311-315`), dropping the ambiguity message, which later stages must re-derive (traced: `authored_selections.rs:1407-1418` re-runs `membership_selection` and turns `Err` into a diagnostic, so the ambiguity is reported there; stated by that code path). (b) `normalize_qualified_case_value` returns `true` on `Err(_)` (`:605`) without writing anything, so the name stays an unresolved path; `authored_selections.rs:1472-1477` re-runs `qualified_payload_free_case_type` and reports the error (traced).

## names/lookup.rs (529 lines)
1. Purpose: small lookup helpers over `SymbolTable`: child-by-kind, top-level by source, constructor and membership selection (domain vs case), ambiguity messages.
2. Shape: `builtin_type_atom_by_symbol_name` (`:10`), `lexical_declaration_lookup` (`:21`), `diagnostic_path_source_span` (`:30`), `domain_name_reaches` (`:54`, forwards to `SymbolTable::domain_name_reaches`), `top_level_type_symbol_for_source` (`:71`), `top_level_symbol_for_source` (`:81`), `constructor_type` (`:96`), `selection_ambiguity` (`:147`), `qualified_payload_free_case_type` (`:171`), `child_symbol_by_kinds` (`:203`), `child_or_attached_data_child_symbol_by_kinds` (`:218`), `child_indexed_symbol_by_kinds` (`:253`), `indexed_member_name` (`:271`), `call_target_for_attached_data` (`:275`), `enum MembershipSelection` (`:318`), `enum DomainMembership` (`:327`), `membership_domain_lookup` (`:357`), `membership_selection` (`:399`). Test module `:444`.
3. In/out: reads symbol table only; returns handles or `Result<_, String>` messages.
4. Decides: `constructor_type` treats "full data name" and "case-owner prefix" as competing meanings; both found = ambiguity error (`:138-140`), not chosen by segment count (comment `:91-95`). `membership_domain_lookup` unions the path lookup with the attached-domain exposure pool; same symbol via two routes counts once; two different symbols => `Contested` (`:392-396`), which `membership_selection` turns into `Ok(None)` (`:427`). Spelling keys: `name.rsplit_once("::")` (`:107,406`), `format!("{name}[{index}]")` for indexed members (`:271-273`, stated: "Declaration names are identifiers, and the lexer keeps `[` a separate token"), `format!("{attached_data}::{target_name}")` (`:281`), `symbols.name(parent).split_once("::")` to find attached data of a machine (`:232`).
5. Cases by shape: `child_or_attached_data_child_symbol_by_kinds` handles three parents: Variant (falls back to enum parent, `:225`), Machine (falls back to attached data found by splitting the machine's name on `::`, `:232-250`), anything else returns the child as-is.
6. Same job elsewhere: `child_symbol_by_kinds_matching` is the single per-kind query (`:301`, comment "A private walk of the same children skipped source selection"); `symbols.find_child_by_name_and_kind` is its backing call. Domain-reach law is in `SymbolTable` (representation area).
7. Counts: `domain_name_reaches` 27 line matches in 9 files (15 are in `symbol-table/.../tests.rs`; production users outside lookup.rs: declaration_selection.rs, mod.rs (re-export), domain_establishment.rs, domain_operator_homes.rs, machine_token_bindings.rs, `03.../type_reference.rs`, `table.rs`). `membership_domain_lookup` 7 matches in 5 files (users: domain_facts.rs 1, type_references.rs 1, machine_token_bindings.rs 2, mod.rs 1). Comment at `:355` "Expression membership, proof-fact membership, and token homes all read this pool" matches those users. `BuiltinTypeAtom::ALL` 10 matches in 9 files; only 2 in lookup.rs (the function plus its doc), so the comment "Every name-based builtin-type lookup reads the roster here" (`:3-8`) is not literally true: other files scan `ALL` too (e.g. `05.../read/types.rs`, `typed-trees/.../type_identity.rs`, `builtin.rs`). Not opened those.
8. Stated limits: none.
9. Fail-open: `child_or_attached_data_child_symbol_by_kinds` returns invalid (reject direction). `membership_selection` `Contested => Ok(None)` (`:427`) is stated: "downstream validation owns the diagnostic, and no other meaning may claim the contested spelling" (`:339-343`). Status: stated; whether downstream validation does report it is untraced.

## names/mathematical.rs (106 lines)
1. Purpose: resolve names in mathematical `let`/`boundary let` signature types and transparent definition bodies.
2. Shape: `assign_mathematical_expression_symbols` (`:21`).
3. In/out: reads `program.roots.mathematical_definitions`, writes `data_type_parameters` (const/value binder types), `mathematical_types` nodes, expression table via `propositions::assign_expression_symbols`.
4. Decides: per `MathematicalType` variant: `Ordinary` -> type reference with binder-locals (`:60`), `Arrow` -> push children, `Application` -> push callee, resolve each argument expression (`:75-88`). Only `MathematicalBody::Definition` bodies are walked (`:93`).
5. Cases by shape: closed three-variant match; arrow binders are "retained as authored names only" (header comment `:10`).
6. Same job elsewhere: body walk shared with `propositions.rs:85`.
7. Counts: `assign_mathematical_expression_symbols` called once, `mod.rs:206` (read).
8. Limits: none. 9. Fail-open: none seen.

## names/measures.rs (52 lines)
1. Purpose: resolve names in each measure's body expression.
2. Shape: `assign_measure_expression_symbols` (`:8`), builds one `MachineScope` per measure with an invalid `symbol` (`:20`).
3. In/out: reads `program.roots.measures`; calls `expressions::assign_expression_span_symbols` with the measure's single parameter as the parameter list.
4. Decides: skips measures with no `parameter` (`:17-19`).
5. Cases: one-parameter measures only; a measure without a parameter gets no body resolution.
6. Same job elsewhere: another full `MachineScope` literal (see unit-wide).
7. Counts: called once, `mod.rs:207`. 8. none. 9. Fail-open: a measure with `parameter == None` is silently skipped (traced: nothing else resolves its body here; whether the parser permits such a measure is unread). Classification: open.

## names/member_namespaces.rs (130 lines)
1. Purpose: refuses a data declaration whose cases and attached domains reuse one name (one shared member namespace).
2. Shape: `reject_member_collisions` (`:63`), private `Member`, `MemberKind`, `attached_owner` (`:49`).
3. In/out: reads `program.data_definitions`, `domain_definitions`; returns diagnostics.
4. Decides: keyed on (owner symbol arena index, member name string, span start) sort then adjacent compare (`:102-111`). Only selected declarations (`symbol_source_is_selected`, `:66`). Domains counted only when `alias.is_none()` and carrier is `Named`/`Generic` naming a data definition (`:50-60,87`). Domain member name is last `::` segment of the domain name (`:95`).
5. Cases: carrier shapes `Named` or `Generic` only (`:50-54`); others return `None` and are not checked.
6. Same job: none.
7. Counts: `reject_member_collisions` 1 caller (`resolver.rs:287`).
8. Stated limit: "Nested data declarations do not parse yet; when they do, they join these rows." (`:14`).
9. Fail-open: a domain whose carrier is not a plain named/generic data type is not checked against case names (traced in this file: `attached_owner` returns `None`). That is the stated scope of the rule.

## names/mod.rs (609 lines)
1. Purpose: orchestrates symbol assignment: build table, install integer carriers and imports, run passes in order; entry points for initial, seeded-extension, body-completion and lexical-input modes.
2. Shape: `select_machine_entry` (`:60`), `NamespaceDeclarations` (`:72`; `record_import_selections :94`, `register :138`, `install :160`), `assign_program_symbols` (`:184`), `SpelledIntegers` + `spelled_integers` (`:231,245`), `assign_symbols` (`:304`), `assign_symbols_against_resolved_base` (`:348`), `assign_constant_expression_symbols` (`:401`), `assign_declarations` (`:461`), `assign_body_symbols` (`:519`), `assign_lexical_inputs` (`:565`, test/test-support only).
3. In/out: takes `&mut SymbolResolvedTrees` or `&mut Lowerer`; produces symbols in place; returns `Vec<Diagnostic>`.
4. Decides: `select_machine_entry` picks state named like the call leaf, else `"entry"` (`:67-68`; string compare on `rsplit_once("::")`). `spelled_integers` finds integer carriers by scanning all type-reference and expression nodes for decimal spellings (`from_decimal_spelling`) or `"u"|"i"` generic base names (`:265`), and installs only carriers not already in the fixed roster (`:251-253`). Comment says a missed carrier "stays unresolved and is refused where it is used" (`:225-226`).
5. Cases: `assign_symbols` vs `assign_symbols_against_resolved_base` are two near-duplicate sequences (`:304-346` vs `:348-394`), differing in table build/extend and `insert_selected_machine_children` start index (0 vs `roots.machines`).
6. Same job elsewhere: that duplication within this file; `assign_body_symbols` and `assign_lexical_inputs` repeat `append_runtime_callable_symbols` + `bind_program` + `assign_signatures` (`:524-535`, `:583-594`, `:330-338`).
7. Counts: `select_machine_entry` 5 line matches in 5 files (names/mod.rs, target_siblings.rs, targets.rs, call_identity.rs, conformance_blocks.rs). Comment `:58-59` says "Both the symbol table and the provisional conformance catalog use this selection rule"; grep shows conformance_blocks.rs 1 use and targets.rs 1; the "symbol table" use is `targets.rs:359`. `owned_conformance_namespace/realization` (re-exported `:41`): 34 line matches in 18 files (counted for the pattern `owned_conformance_namespace|owned_conformance_realization`), so widely used. `bind_service_application_arguments` and `assign_nominal_machine_requirement_arguments` (re-exports `:51,609`): counted jointly with other names, 16 matches in 6 files; read: used from `selections/mod.rs` and `machine_parameter_requirements.rs`.
8. Stated limits: comment `:6-8` "The seeded variant extends a retained base table with a later stratum in place."
9. Fail-open: `assign_symbols` returns `Ok(())` when `diagnostics.is_empty()` (`:341`); diagnostics from passes that push nothing are not errors by design. `NamespaceDeclarations::register` `continue`s for files with empty directory namespace (`:142-144`): stated by flow, root-directory files have no module.

## names/paths.rs (415 lines)
1. Purpose: path walks: resolve the first member of a name path to a lexical/machine/top-level head, then walk later members as children.
2. Shape: private `base` module: `resolve_base_lookup` (`:18`), `resolve_base_indexed_symbol` (`:83`). `SCOPED_PATH_MEMBER_KINDS` (`:121`), `resolve_state_scoped_table_path` (`:136`), `resolve_state_scoped_table_path_member_symbols` (`:160`), `resolve_path_members_from_head` (`:191`), `resolve_state_scoped_members` (`:220`), `resolve_lexical_members` (`:256`), `resolve_member_suffix` (`:285`), `resolve_path_head(_lookup)` (`:327,338`), `unresolved_scoped_name` (`:369`), `unresolved_scoped_call_name` (`:393`).
3. In/out: reads symbols, machine/state; returns `(head, leaf)` pairs or vectors; `unresolved_*` return `UnresolvedName` carrying NotFound/Ambiguous.
4. Decides: precedence in `resolve_base_lookup`: state parameter/local, then machine field/state/conformance-parameter (or attached data child), then lexical declaration (machine/type-parameter/...), then source-visible top level (`:24-80`). Ambiguity does not fall through to a longer path (comment `:13-17`). Longest-prefix lookup by joining members with `::` (`:350-362`, `prefix.joined_path()`), i.e. keyed on spelling joined into a qualified name.
5. Cases: indexed form `name[i]` only for single-member head or the final member (`:232,297`).
6. Same job: `lexical_value_head` (expressions.rs:329) also performs lexical-head lookup with a different rule set; `resolve_lexical_members` (`:256`) calls it.
7. Counts: not separately grepped; functions are `pub(super)` / `pub(in names)`, used by expressions.rs, statements.rs, propositions.rs, targets.rs, type_references.rs (read).
8. Limits: none. 9. Fail-open: `unresolved_scoped_name` returns `Default` (no error) when the head is a non-Module or the whole path was the prefix (`:383-385`); stated: "A value's unresolved member needs type selection; only a static namespace can establish a missing member here" (`:366-368`). Traced: member of a value is checked later in typing (not read).

## names/private_nested_calls.rs (106 lines)
1. Purpose: refuse `outer::helper(..)` when `helper` is a nested machine private to `outer`.
2. Shape: `reject_private_nested_calls` (`:24`), `report` (`:83`).
3. In/out: reads expression calls, statement calls, transition targets; returns diagnostics.
4. Decides: call with invalid `target_symbol` and a `Name` receiver whose symbol (or head) is a `Machine` having a `Machine` child named like the target (`:89-96`); keyed on symbol kind and child name string equality.
5. Cases: three carriers: expression Call, statement Call, `TransitionTarget::Named` with path `[_, .., leaf]` (`:68`).
6. Same job: none. 7. Called once (`mod.rs:558`; grep joint count 9 lines/6 files for the group of four names).
8. none. 9. Fail-open: receiver that is not a `Machine` symbol is skipped (`:89-91`): a private nested call through any other receiver kind is not reported here (reject occurs elsewhere as unresolved call). Open.

## names/propositions.rs (463 lines)
1. Purpose: resolve names in transparent proposition bodies and (shared) in mathematical terms.
2. Shape: `assign_proposition_expression_symbols` (`:18`), `assign_expression_symbols` (`:85`), `assign_expression_symbols_in_role` (`:105`); tests `:371-462`.
3. In/out: expression table, child type refs; owner symbol's binder/parameter children are lexical scope.
4. Decides: single-member name: child of owner of kind Parameter/TypeParameter/PropositionMachineParameter (`:259-268`). Multi-member: `FixedCoreDeclaration::from_name(&members.joined_path())` (string spelling) else `resolve_state_scoped_table_path` (`:270-283`). Call with no receiver: owner's Parameter/PropositionParameter/MachineParameter child with the call's name, else `resolve_free_call_target_symbol` (`:214-231`).
5. Cases: closed `match` over every `ExpressionNode`; `Block` and `SavedSubject` are skipped (`:129,162`), comment "Typing refuses a block value in a proposition."
6. Same job: duplicates expressions.rs walk with different rules (see expressions.rs item 6).
7. Counts: `assign_expression_symbols` used by `mathematical.rs`; `assign_proposition_expression_symbols` once (`mod.rs:205`).
8. Limit: comment names PROOF-CONTRACT-MIGRATION (`:83`).
9. Fail-open: a call with a receiver gets `target_symbol = invalid` with no unresolved-name record (`:214-216, 239`), i.e. receiver-qualified calls inside propositions are left unresolved without an error here. Open.

## names/public_interfaces.rs (142 lines)
1. Purpose: refuse a public machine whose entry signature names a non-`public` data type of the same package.
2. Shape: `named_symbols` (`:28`), `reject_private_types_in_public_signatures` (`:76`).
3. In/out: reads machines, data definitions, entry state parameters and return type; returns diagnostics.
4. Decides: only the first state ("entry") of each selected public machine (`:98`); private = `!definition.is_public`; same-package test via `symbols.same_source_package(declaration_span, at)`; symbols without source span skipped (`:117-121`).
5. Cases: `named_symbols` handles Named, Generic (+args), Reference, FixedArray, Slice, Constrained, AliasUse; Const-expression/Callable/Unit contribute nothing (`:70-72`).
6. Same job: none. 7. Called once (`resolver.rs:290`).
8. Stated limit: "A domain written as a constraint (`x: u32 in Small`) is named by spelling at this stage and is not judged here yet." (`:16-18`) and spec exceptions "are not read here" (`:14-16`).
9. Fail-open: a private domain used as a constraint, a callable-typed parameter, or a const expression is accepted (stated, `:16-18`; callable/const arms `:70-72` unstated).

## names/recursive_tail_calls.rs (250 lines)
1. Purpose: rewrites a final-position call or transition value that recursively calls the machine's own entry into a named transition target (a constant-stack edge).
2. Shape: `RecursiveEntry`, `EntryNames` (`:40`), `normalize` (`:82`), `unambiguous_canonical_entry` (`:115`), `normalize_state` (`:144`), `normalize_target` (`:183`), `recursive_target` (`:202`).
3. In/out: reads machines and attached roster; mutates statements (replaces `Expression` statement with `Transition`, replaces `Value` target with `Named`).
4. Decides: machine in `work` list (binary search on `(arena_index, generation)`), entry unambiguous among attached machines with same owner and entry spelling (`:115-142`); only states whose `return_type == entry.return_type` (`:108`); call target must equal entry or machine symbol, no machine arguments, receiver (if any) a `Name` equal to the owner data symbol without static application, and exactly one or zero authored-selection occurrences (`:209-225`).
5. Cases: only the final statement of a state as `Expression` (`:152`) and `Transition` target/continuation.
6. Same job: `target_siblings.rs` also rewrites call targets post-resolution. 
7. Counts: `normalize` called once (`resolver.rs:304`).
8. Limit comment: "Legacy statement slices are no longer the consumer's input and must not be rebuilt" (`:7-8`).
9. Fail-open: `unambiguous_canonical_entry` returns `true` when the machine has no attached data (`:120-122`), i.e. free machines are always eligible; other conditions are in `recursive_target`. No wrong program accepted traced (rewrite is semantics-preserving; ranking/admission checked later per header comment `:9-11`).

## names/repeated_locals.rs (340 lines)
1. Purpose: refuses a local/pattern binding that reuses a name still visible; also records overlaps with generic binder names.
2. Shape: `check_bindings` (`:22`) returns `Vec<GenericBindingOverlap>`; `ArmBindings` (`:110`) with `sequence`, `statement`, `expression`, `field_bindings`, `bind`.
3. In/out: reads machines/states/statements/expressions/symbol names; produces diagnostics and overlaps stored in `generic_binding_overlaps` (`mod.rs:559`).
4. Decides: keyed on source-spelled name strings: `self.declared.iter().position(|earlier| earlier == name)` (`:137`), type-parameter overlap `binder.name.as_str() == name.as_str()` (`:54`). Parameter vs local distinguished by position < `parameter_count` (`:140`).
5. Cases: only locals with `role.is_runtime_binding()` and valid symbol (`:132`). Closed matches over `Statement` and `ExpressionNode`.
6. Same job: none. 7. Called once (`mod.rs:560`).
8. none. 9. Fail-open: locals with an invalid symbol or non-runtime role are skipped (`:132`); a state/parameter with invalid symbol is skipped (`:32,65,74`). Open.

## names/requirement_namespaces.rs (269 lines)
1. Purpose: lists the methods an evidence binder (`Model satisfies Trait`) exposes, and maps an evidence child symbol back to its trait requirement signature.
2. Shape: `visible_requirements` (`:17`), `evidence_requirement` (`:37`), `retained_declaration_conformance_bounds` (`:127`), `exact_trait` (`:195`), `collect_requirements` (`:212`), `select_visible_trait` (`:241`).
3. In/out: reads program traits/bounds and `SourceMap`; returns signatures or `Err(String)`.
4. Decides: `select_visible_trait`: candidates with `candidate.name == *reference_name` (spelling compare of `DiagnosticName`, `:251`) and visibility via `reference_can_see_declaration`; picks same-source, else non-separate-strata, else first (`:257-268`). `evidence_requirement` is keyed on symbol: child ordinal in binder's children must equal ordinal in the collected requirement list (`:111-122`).
5. Cases: any mismatch returns an `Err` string (`:111-116,91-96`). Lineage debug message builds up to 8 ancestors/declarations (`:55-88`).
6. Same job: trait-requirement collection also in `scopes.rs:525 collect_trait_requirements`.
7. Counts: `visible_requirements|evidence_requirement(` 6 line matches in 4 files (requirement_namespaces 2, symbol_table.rs 1, authored_selections.rs 2, call_identity.rs 1).
8. Comment: "Missing owner metadata must still refuse." (`:51`).
9. Fail-open: `select_visible_trait` falls back to `candidates.first()` (`:268`) when no candidate is same-source or same-stratum, so with several visible same-named traits one is chosen by roster order. Status: open (visibility filter `:252-254` leaves only visible ones; if `sources` is `None`, all same-named traits pass, `:252`).

## names/runtime_callable_scopes.rs (168 lines)
1. Purpose: records which declaration encloses each anonymous callable contract so its type parameters can be scoped later.
2. Shape: `RuntimeCallableScopes` (`:40`) with `begin/end/bind/retain/bind_program`; `CallableScopeOwner` enum (`:18`); `Lowerer::with_runtime_callable_scope` (`:158`).
3. In/out: lowering pushes scopes; `bind_program` writes `lexical_owner` into `runtime_callable_contracts`.
4. Decides: by scope handle/stack order; asserts (`:71,82`) panic on misuse; returns `Err` if scope has no owner or contract already owned (`:98-111`).
5. Cases: 12 owner kinds, resolved through a closed match (`:122-146`).
6. Same job: none. 7. Not counted.
8. none. 9. Fail-open: none seen (errors on every missing owner).

## names/runtime_callable_types.rs (237 lines)
1. Purpose: gives structural callable contracts their own symbols/types in the lexical scope of their enclosing declaration.
2. Shape: `assign_signatures` (`:12`), `lexical_parameters` (`:89`).
3. In/out: reads `DeclarationTypeScopes`; writes contract signature symbols and parameter types.
4. Decides: `lexical_parameters` walks owner chain (cycle guard `entered`, `:96-101`), accumulating type parameters; stops at machine, data/alias/domain/trait/conformance/operator (macro `declaration_parameters!`, `:142-159`), mathematical definition, proposition, measure; else, for a non-State/non-Parameter owner with no match, `Err("runtime callable lost its supplying type-parameter scope")` (`:221-228`).
5. Cases: fixed list of owner kinds above (macro invocation `:152-159`).
6. Same job: proposition binder -> `TypeParameter` conversion `:175-187` duplicates `propositions.rs:31-40` (kind mapping differs: this one maps `Const` and `Machine` binders, the other maps all to `Type`).
7. Counts: `lexical_parameters` callers: `type_references.rs:1134`, `runtime_callable_types.rs:27`. 
8. none. 9. Fail-open: `measures` owner returns current parameters, accepting any measure owner without adding any (`:190-196`); intended (measures have no generics). Stated by absence.

## names/scopes.rs (910 lines)
1. Purpose: lexical scope data used by all name passes: declaration type scopes, attached-machine roster, binder roles, trait-requirement scopes, `MachineScope`.
2. Shape: `declaration_type_scope` (`:18`), `evidence_type_parameters` (`:40`), `owned_conformance_realization` (`:56`), `record_declaration_owners` (`:92`), `owned_conformance_namespace` (`:138`), `DeclarationTypeScopes` (`:169`), `AttachedMachine(s)` + `attached_machines` (`:253-286`), `BinderRole`/`BinderRoles` (`:329-442`), `TraitLexicalScopes` (`:450`), `LexicalRequirements` (`:561`), `MachineScope` (`:602`), `ContractScopeContext` (`:654`), `impl MachineScope` (`:706-910`: `declaration_scope`, `supplied_scope`, `attached_call_target`, `field_type_reference`, `nested_self_chain_type`).
3. In/out: built from `SymbolResolvedTrees`; consumed by every names/ pass.
4. Decides: `attached_machine_list` excludes machines that are a "closed realization" of a conformance row and requires `attached_data_symbol` valid and `target.is_none()` (`:305-308`). `BinderRoles::of` includes a spelling-and-identity exception: machine named `"X86StaticInterruptScope::run"` whose `sealed_declaration` is `StaticInterrupt(Install)` gets two leading `Declaration` roles (`:369-380`). `attached_call_target` picks the first attached machine whose entry state name equals the target spelling and which is visible (path lookup, or via each import prefix, `:780-822`); second match is ignored (`if selected.is_valid() { continue }`, `:818-820`).
5. Cases by shape: BinderRole of `TypeParameterKind`: Type/Const/Value/else Declaration (`:339-344`).
6. Same job: `collect_trait_requirements` (`:525`) vs `requirement_namespaces.rs:212`; `declaration_type_scope` call patterns repeated in `runtime_callable_types.rs`, `repeated_locals.rs`, `statements.rs`.
7. Counts: `X86StaticInterruptScope::run|"Slice::Length"|$predicate_subject` 8 line matches in 6 files (scopes.rs 1, statements.rs 1, symbol_table.rs 1, machine_supply.rs 2, language-core sealed_intrinsics.rs 1, declaration_contracts.rs 2). Comment `:649-653` says the "four scope literals in `symbols/contracts` differ only in the fields this constructor takes"; `ContractScopeContext::scope` users: `type_references.rs:1142-1163` in this unit, others in `contracts.rs` (not read; suspected).
8. Stated limits: `nested_self_chain_type`: "shell-wrapped intermediates (`@mutable`, constrained, arrays) return `None` -- conservative" (`:875-876`).
9. Fail-open: `attached_call_target` with two visible same-named attached entries returns the first and ignores the second (`:818-821`), no ambiguity error. Status: open (the `selects` closure treats `Ambiguous` as match if either is the attached machine, `:793-795`). `DeclarationTypeScopes::parameters` returns `&[]` when a machine has two owners (`:245-247`) rather than an error. Open.

## names/statements.rs (1078 lines)
1. Purpose: symbol assignment for state bodies: statement routing so each statement sees only its predecessors, assembly scope, block-value scope, machine header ranking views, lexical inputs.
2. Shape: private modules `assembly` (`AssemblyScopeRouting :21`), `blocks` (`BlockScopeRouting :155`), `routing` (`assign_statement_symbols :345`, `assign_statement_slice_symbols :314`, `assign_local_type_symbols :281`); `assign_statement_reference_symbols :701`, `assign_machine_signature_reference_symbols :713`, `assign_lexical_input_reference_symbols :721`, `assign_machine_references :729`, `resolve_ranking_view :968`.
3. In/out: mutates state statements, expression table, state parameter types, `selected_ranking_view`.
4. Decides: transition patterns are rewritten into guards: head symbol kind `Variant|Domain` => `Membership` test (`:556-584`); non-structural pattern with a head => scalar `Equal` binary on subject and head (`:585-612`) and the head is dropped (`:633-639`); patterns with `subject_binding` and no guard are skipped (`:550-554`). Ranking view: lexical declaration lookup, then source top-level; `SymbolLookup::NotFound if path == "Slice::Length"` -> builtin `SLICE_LENGTH` (`:998-1000`; string compare). `RootBinding.implementation_operand` is cleared unless it is a Local/Parameter/Field name or a non-name expression (`:376-389`).
5. Cases: `Statement` closed match (`:370-691`). Assembly blocks walked only for `Statement::Assembly` direct children.
6. Same job: `blocks::block` and `assembly::assign` each rebuild a prefix `Vec` copy of the outer statements per block/instruction (`:222-225`, `:95-96`).
7. Counts: `assign_statement_reference_symbols` called once (`mod.rs:547`); `assign_machine_signature_reference_symbols` once (`mod.rs:212`).
8. none. 9. Fail-open: `if let Some(inputs) = lexical_inputs.iter_mut().find(..)` skips unknown states silently (`:882-885`, test-support only). `RootBinding` with implementation operand that is a `Name` of another kind is dropped rather than refused (`:385-388`). Open.

## names/symbol_table.rs (1456 lines)
1. Purpose: constructs and extends the symbol table from declaration rosters, and appends body-level symbols (states, parameters, locals).
2. Shape: private `children` module (`insert_*_symbol_children`, `insert_generic_parameter_children :549`, `insert_callable_signature_children :599`, `insert_conformance_symbol_children :630`), private `names` module (seed constructors `machine_symbol_seed :768`, `operator_symbol_seed :829`, ...), `extend_symbol_table :877`, `build_symbol_table :1032`, `insert_selected_machine_children :1211`, `append_runtime_callable_symbols :1242`, `append_body_symbols :1297`, `append_local_symbols :1431`.
3. In/out: reads declaration rosters; produces `SymbolTable`; writes handles into each declaration's `.symbol` fields.
4. Decides: child order is the contract (comment `:25-27`). A data with `requires_facts` gets a child `"$predicate_subject"` (`:85-88`, string-named). Machine seed uses `target_symbol_name` when `machine.target.is_some()` (`:772`); generated machines with no source-backed name borrow the carrier's source span (`:779-793`).
5. Cases: root family list is fixed: builtin types, builtin functions, fixed-core declarations, domains, data, aliases, aliased conformances, root machines, propositions, mathematical definitions, operators, measures, traits, wire schemas, consts (`:1056-1131`) and installed in that same order (`:1157-1202`).
6. Same job: `build_symbol_table` and `extend_symbol_table` repeat the per-family list with different mechanisms (macro `extend_root_family` `:905` vs `install_root_family` `:1146`).
7. Counts: `append_body_symbols` 1 caller (`mod.rs:523`, read).
8. Comment: "Homing can append signatures to a retained domain, outside the new-root watermark" (`:885`).
9. Fail-open: `root_children.next().unwrap_or_default()` (`:1177,1195,1201`) yields an invalid handle if the child stream is short instead of failing; `if let Some(symbol) = root_children.next()` skips silently (`:1136,1150,1163,1188`). Traced for one case: later `next_child_of_kind` returns invalid for kind mismatch (`top_level.rs:1352`), so a mis-ordered stream yields invalid symbols that later report unresolved names rather than wrong acceptance. Status: traced, wrong-order inputs produce unresolved symbols.

## names/target_machine_families.rs (327 lines)
1. Purpose: binds each authored target-specific machine (`machine f for linux_x86_64`) to its portable family interface and copies interface premises into the interface's telescope.
2. Shape: module `contracts` with `prepare_premises` (`:30`), `join_property_demands` (`:97`), `telescope_correspondence` (`:134`), `copy_contract` (`:187`); `bind` (`:207`).
3. In/out: reads machines; sets `target_family`; replaces interface `contracts` with copies of members' `Requires`/`Crashes` contracts.
4. Decides: `bind` groups machines by `name.as_str()` string compare (sorted, `:221-235`), per family member finds interface with same name, same module, same attached data and same source package (`:271-281`); two members with the same `authored_target` string => error (`:250-270`); no interface or two => error. Only `DependencyScope::Product` sources (`:217`).
5. Cases: interface premises copied only for `SignatureContractKind::Requires | Crashes` (`:72-77`). Generic multiplicity join supports "affine plus one non-affine"; two different non-affine values give `"unimplemented: target-family generic multiplicity conjunction"` (`:118-122`).
6. Same job: none. 7. `prepare_premises` and `bind` called once each (`resolver.rs:302,308`).
8. Stated: "The existing property carrier cannot encode conflicting demands." (`:95`); "unimplemented" (`:119`).
9. Fail-open: machines whose source is not `Product` scope are not grouped (`:217`) and get no family binding; whether a dependency's target machines are bound elsewhere is untraced. Open.

## names/target_siblings.rs (153 lines)
1. Purpose: re-points calls made inside a target-authored body to that same target's siblings.
2. Shape: `bind_sibling_calls` (`:29`), `bind_sibling_calls_with` (`:38`), `Sibling` (`:18`).
3. In/out: reads machine families; mutates call targets through `retarget_machine_calls`.
4. Decides: per call site: if already resolved, callee machine's family has a sibling with the same `target` string => replacement (`:93-123`); if unresolved, joins `"{spelled}::{target}"` and looks it up, requiring the result to be in `same_target` (`:124-145`). Keyed on target-name strings (`candidate.target == sibling.target`, `:91`) and formatted paths.
5. Cases: callee kinds State or Machine only (`:96-99`).
6. Same job: `retarget_machine_calls`/`retarget_signature_contract_calls` grep: 8 line matches in 3 files (target_siblings.rs 2, body_calls.rs 2, conformance_blocks.rs 4), so the traversal is shared by 2 passes (comment in body_calls.rs says "shared by every pass" — 2 passes).
7. Counts: `bind_sibling_calls` called once (`resolver.rs:303`).
8. Comment: "Runs after every other symbol assignment and selection pass" (`:25`).
9. Fail-open: calls with no same-target replacement are left unchanged (`return None`, `:105-109`): a target body calling a contract that this target does not implement keeps its canonical callee. Open.

## names/targets.rs (2275 lines)
1. Purpose: symbol assignment for call, transition and static-namespace targets, and for compile-time static (machine/value/type) arguments.
2. Shape: module `calls` (`resolve_call_target_symbol :45`, `resolve_free_call_target_symbol :312`, `resolve_free_machine_entry_state_symbol :342`, `machine_entry_symbol :354`, `resolve_static_machine_argument_symbol :378`, `assign_static_argument_symbols :461`, `assign_static_argument_symbol :491`, `assign_call_static_argument_symbol :599`, `assign_call_machine_argument_symbols :687`, `build_operand_route :750`, `assign_provider_selection_argument_symbol :864`, `assign_assumed_binding_argument_symbol :920`, `assign_representation_selection_argument_symbol :945`, `resolve_proposition_binder_argument_symbol :985`); module `static_namespaces` (`static_call_target :1053`, `normalize_expression_module_call :1137`, `normalize_statement_module_call :1200`, `normalize_terminal_namespace :1236`); module `transitions` (`assign_transition_target_symbols :1278`, `terminal_call_target :1391`). Tests from `:1418`. `SOURCE_CALL_KINDS` (`:32`).
3. In/out: reads symbol table, machine scope; writes `target_symbol`, argument `symbol`, normalizes `receiver`/`target` of static calls.
4. Decides: receiver kind dispatch in `resolve_call_target_symbol` (`:54-246`): Local -> its declared type; field/parameter type; Module -> `format!("{module}::{target}")` lookup (`:113-117`); const record binder; BuiltinType -> builtin function child; Data -> attached machine or operator `"{Data}::{target}"` (`:185-192`); Domain -> attached data by domain name or last `::` leaf (`:201-223`); Machine -> only when `receiver == machine.symbol` (`:224-233`); Trait/ConformanceParameter -> State child. Static argument role: type binder reads BuiltinType/Data, value binder reads Const/Conformance, declaration binder tries binders, Conformance, then Const xor type (both valid => invalid, `:422-432`). `static_call_target` (`:1053`): `FixedCoreDeclaration::from_name(name)` first, then module/package-qualifier/declared-owner test, then `BuiltinFunction::from_name(name).is_source_special_form()` for unbound namespaces (`:1103-1111`). `build_operand_route`: `BuildOperation::from_call_target(operation)` on the call spelling after last `::`, qualifier must be a Module named `"Build"` and `is_build_prelude_symbol` (`:759-773`).
5. Cases by shape: Build operations route to one of five resolvers via `match operand_route` (`:697-744`, `ProviderReadiness` index 0 only, `TestGroupService` index 0 only). Static member paths at most the address shapes `Argument(i)` / `Member`.
6. Same job: `selection_symbol` descendant lookup (`:819`) vs `scopes.rs:761 attached_call_target` import-prefix loop; `normalize_expression_module_call` and `normalize_statement_module_call` and `normalize_terminal_namespace` are three carriers calling one `static_call_target` (comment `:1049-1051`: "Every invocation carrier applies this one law").
7. Counts: `machine_entry_symbol` users: `static_namespaces`, `targets.rs` calls, `mod.rs` re-export; `assign_call_machine_argument_symbols` 6 matches in 3 files (see expressions.rs).
8. Stated limits: tests document the behaviors; no "unsupported" comment.
9. Fail-open: `resolve_call_target_symbol` for `receiver_kind == Machine` returns invalid unless the receiver is the current machine (`:224-233`), refusing. `resolve_static_machine_argument_symbol` for `BinderRole::Type|Value` returns the first top-level match of the role's kinds with no ambiguity check beyond the `find_..._from_source` return (`:399-402`), which returns `Option` and treats ambiguity as `None`; refusing. `selection_symbol` returns invalid on ambiguity (`:814`). None seen that accepts a wrong choice.

## names/top_level.rs (1639 lines)
1. Purpose: assigns top-level declaration symbols and child symbols by walking each declaration's children in insertion order, and resolves the type references in headers.
2. Shape: modules `data :36`, `domains :219`, `machines :257`, `mathematical :733`, `measures :773`, `operators :809`, `propositions :922`, `traits :957`, `type_aliases :1119`; `trait_parameter_tables :1227`, `assign_top_level_symbols :1289`, `assign_conformance_parameter_symbols :1308`, `next_child_of_kind :1352`, `SignatureSymbolTables :1373` with `assign_parameter_contract_symbols`, `resolve_type_parameter_kind_symbols`, `assign_callable_signature_symbols :1498`, `assign_proposition_parameter_signature_symbols :1583`, `assign_conformance_bound_symbols :1610`.
3. In/out: mutates `SymbolResolvedTrees` declaration tables; returns diagnostics from machine and trait passes.
4. Decides: machine satisfaction target resolution (`machines :509-586`): joins `"{conformance.name}::{requirement}"` (`:511`), looks up a Machine; `top_level_requirement_symbols` membership decides legal target; otherwise errors "is an ordinary machine, not an explicit top-level `boundary requirement`" (`:540-544`), "names a transparent refinement" (`:565-570`), "does not resolve to an exact trait requirement" (`:572-578`; skipped when the path names an operator, `names_operator`). Generic-data template: unique candidate machine with `template.name.source_span() == origin.template_source.source_span()` (`:701-708`), else error "generic-data method has no unique authored template declaration" (`:712-718`). Predicate subject of generic data instance is shared through a `HashMap<String, SymbolHandle>` keyed on `origin.base_name.as_str()` (`data :59,160,170`).
5. Cases: kinds per `TypeParameterKind` mapped to `SymbolKind` by repeated matches (`:70-74, 311-314, 760-763, 994-1005, 1145-1150, 1323-1327, 1517-1521`).
6. Same job: the `TypeParameterKind -> SymbolKind` mapping is written at least 7 times in this file and again in `symbol_table.rs:750` (`type_parameter_seed`) and `:400-409`.
7. Counts: `assign_top_level_symbols` called once (`mod.rs:191`). `trait_parameter_tables` used in `scopes`-adjacent code (`runtime_callable_types.rs:18`, `top_level.rs`).
8. Comments: "A declaration that already holds a valid symbol keeps it." (`:20`).
9. Fail-open: `next_child_of_kind` returns invalid on empty stream or kind mismatch (`:1357-1365`); callers do not check, so a mis-ordered child stream silently yields invalid symbols (traced to unresolved-name errors, see symbol_table.rs). `machines :316-322` records a diagnostic for a pre-set symbol that differs but proceeds. `assign_data_symbols`: `template_subjects.get(..).unwrap_or_else(next_child_of_kind(..))` (`:160-165`) falls back to a fresh child when the template was not seen first; the order dependency is open.

## names/type_predicates.rs (266 lines)
1. Purpose: recognizes `T == U` / `T != U` where both operands name types and rewrites them into `TypeExpression` operands; refuses mixing a type with a value.
2. Shape: `retain_operands` (`:19`), `mixed_equality_message` (`:81`), `is_type_predicate` (`:106`), `named_type_operand` (`:116`), `has_type_role` (`:165`), `materialize_type_operand` (`:189`), `type_operand_nodes` (`:247`).
3. In/out: mutates expressions; appends type references; transfers authored-selection occurrences.
4. Decides: type role is keyed on resolved symbol kind (`TypeParameter` of kind `Type`, `BuiltinType`, `Data`, `Domain`, `:124-141`), not spelling ("A spelling never supplies this role", `:2-3`). Borrow and one-element array literals are accepted when their sole child has type role (`:177-184`), depth capped at 128 (`:170`). The error message does check spelling: `is_builtin_type_spelling(name)` (`:90`).
5. Cases: structural type operands limited to `Borrow` and single-element `ArrayLiteral` (`:177-184`).
6. Same job: none. 7. Counts for `is_type_predicate|type_operand_nodes|bind_service...|assign_nominal...` 16 matches in 6 files (see mod.rs); `is_type_predicate` users: authored_selections.rs (3 lines), selections/mod.rs.
8. none. 9. Fail-open: `[false,false] => continue` leaves non-type equality alone (intended). Depth 128 returns `false` (`:170`), so a deeper type operand is treated as a value and reported as a mix or as ordinary equality. Status: open.

## names/type_references.rs (2371 lines)
1. Purpose: symbol assignment for type references: names, generics, aliases, constraints, trait-argument slots, plus value expressions embedded in types (array extents, integer widths, domain index values).
2. Shape: module `assignment` (`assign_generic_application_origin_symbols :39`, `assign_nominal_machine_requirement_arguments :98`, `assign_type_reference_symbols :233`, `assign_trait_bound_slot_argument_symbols :593`, `assign_trait_implementation_argument_symbols :641`, `assign_machine_declaration_identity_argument_symbols :689`, `assign_proposition_family_argument_symbols :737`, `assign_type_reference_with_definitions :805`, `bind_service_application_arguments :1092`, `resolve_type_symbol :1201`, `resolve_value_in_type_symbol :1223`); module `queries` (`call_target_for_type_reference :1319`); module `value_expressions` (`assign_declaration_type_value_expression_symbols :1368`, `assign_type_value_expression_symbols :1661`, `reassign_type_reference_symbols_and_values :1912`, `assign_domain_index_values :1965`, `resolve_index_subject :2032`) with a unit test `:2112`.
3. In/out: mutates the child type-reference arena and expression table.
4. Decides: type name resolution: a local binder wins only if it does not introduce a value (`:1206-1212`); value-in-type resolution: local value binder else a top-level `Const` (`:1228-1234`). Integer width `u<N>`/`i<N>` arguments read as values when `symbols.integer_family(base).is_some()` (`:923,1764`). A supplied-operand-owner reference is resolved in the supplying scope or skipped when none (`:1693-1700`). Domain index values only for binder role `Value` (`:1996`). Trait slot classification: slot vectors of bools per trait, joined on trait symbol (`:72-92`, `:609-632`). Machine-identity slot: `"Trait::requirement"` split by `rsplit_once("::")` then State child lookup, else free machine entry (`:718-733`).
5. Cases: closed match over `TypeReference` variants (`:813-983`, `:1730-1904`). Header for generic `Value|Const` slots from Data, Trait, TypeAlias only (`:1780-1799`); alias header requires exactly one alias of that symbol (`:1792-1797`).
6. Same job: `assign_type_reference_with_definitions` (names pass) and `assign_type_value_expressions_with_declarations` (value pass) are two parallel walks of the same type structure.
7. Counts: `bind_service_application_arguments` and `assign_nominal_machine_requirement_arguments`: callers outside this file in `selections/mod.rs` and `machine_parameter_requirements.rs` (read from the grep's file list). 
8. Stated: header comment about "type/domain names retain declaration lookup; index expressions must resolve lexical subjects before a later phase can establish that their value is static" (`:1342-1344`).
9. Fail-open: `resolve_index_subject` (`:2032-2109`) returns `Some(declared)` when the suffix is empty even if invalid (`:2081-2083`), caller writes it; invalid results are written over an existing symbol (`*symbol = selected`, e.g. `:1771,1816`), so an earlier valid selection can be replaced by an invalid handle. Status: open (the comment `:2022-2031` explains order; whether the earlier symbol is rebinding-safe is untraced). `ConstExpression` arm of the names pass is a no-op (`:947`).

## pre_resolution/initializers.rs (470 lines)
1. Purpose: carrier `ConstInitializerSelection` for private constant-initializer preparation, with queries over retained constants, dependencies and pending scalar leaves. Test-only module tree (`resolver.rs:11-12`).
2. Shape: `ConstInitializerSelection` (`:23`, `retained_constant :42`, `retained_constant_custody :64`, `initializer_dependencies :83`, `declaration_dependencies :99`, `trees :131`, `validate_initializer_leaves :139`, `pending_leaves :166`), `ConstInitializerPayload` (`:31`). Tests `:180-469`.
3. In/out: wraps a `DeferredResolvedFrontier`; returns declarations/custody/dependencies or error vectors.
4. Decides: retained constant must exist in the retained base and `symbol_source_is_selected` (`:52`); declaration lookup is by `definition.name.source_span()` (`:105-109`) i.e. keyed on source position.
5. Cases: none.
6. Same job: none. 7. Counts: pre_resolution API: `resolve_deferred_numeric_probe|resolve_const_argument_selection|resolve_deferred_initializer_probe|prepare_const_qualification_selection` 7 matches in 4 files across the whole worktree: `driver/resolve_and_type/tests.rs` 1, `deferred_bodies.rs` 1, `pre_resolution/mod.rs` 4, and 1 in `_recycling/tools/benchmark/selected-bodies/nonempty.rs` (archived). So no non-test production caller seen in `source-rust/` outside this module (module is `#[cfg(any(test, feature="test-support"))]`, `resolver.rs:11`).
8. Comment: "grants no authority to type, execute, or publish it" (`:18-19`). 9. Fail-open: none seen.

## pre_resolution/mod.rs (248 lines)
1. Purpose: build-time pre-resolution entry points that resolve only declarations a constant initializer or probe needs.
2. Shape: `resolve_deferred_numeric_probe :38`, `ConstArgumentSelection :55`, `resolve_const_argument_selection :101`, `resolve_deferred_initializer_probe :126`, `prepare_const_initializer_selection :144`, `prepare_const_qualification_selection :159`, `finish_const_initializer_selection :216`.
3. In/out: `ResolutionRequest` in; frontier, selection or `None` out; `finish_const_initializer_selection` loops `body_demand` / `complete_machine_bodies` until no deferred machines remain (`:236-244`).
4. Decides: qualification selection returns `Ok(None)` unless some destination has a pending qualification or requires proof and has no pending leaves (`:184-203`).
5. Cases: three `ConstResolutionMode` values used (`Complete`, `ArgumentSelection`, `InitializerSelection`).
6. Same job: `begin`+`index_deferred` sequence repeated across 4 functions (`:43-44`, `:104-112`, `:131-132`, `:149-150`).
7. Counts: see initializers.rs item 7.
8. Comment: "This is a one-shot selection carrier, not a continuation" (`:98-99`). 9. Fail-open: `prepare_const_qualification_selection` returns `Ok(None)` (no selection) when no destination qualifies (`:202-204`); the caller then proceeds without pre-resolution (traced by return type only; caller unread).

## resolver.rs (317 lines)
1. Purpose: crate root of stage 02 logic: request type, deferred resolution, the fixpoint loop over machine-equation completion, `finish_resolution` and `settle_resolution` orders.
2. Shape: `ResolutionRequest` (`:59`), `resolve_deferred` (`:91`), `index_deferred` (`:133`), `index_declarations` (`:172`), `begin` (`:184`), `finish_resolution` (`:217`), `settle_resolution` (`:265`), `finish_resolved_tables` (`:296`), test-only `resolve` (`:45`).
3. In/out: `ResolutionRequest` -> `DeferredResolvedFrontier`.
4. Decides: loop `:103-126` repeats a provisional resolution until `complete(..)` adds no root syntax items (count compare `:123`), only if `required(&syntax, base)`. Machines to finish: those not in the predecessor, or in `completed_keys` (`:236-245`).
5. Cases: `finish_resolved_tables` fixed order: closed-conformance routing, named implementation calls, target families bind, sibling calls, recursive tail calls, premises (`:300-310`).
6. Same job: provisional `index_declarations`+`finish_resolution` (`:112-113`) repeats the main sequence.
7. Counts: `reject_private_nested_calls|reject_member_collisions|reject_private_types_in_public_signatures|record_declaration_owners` 9 matches in 6 files; each has one production caller.
8. Stated: none. 9. Fail-open: `retain_operands` and the three `reject_*` calls are in `settle_resolution` (`:282-292`); `sources.is_none()` skips `validate_logical_filenames` (`:94`), and `select_visible_trait` accepts all when `sources` is `None` (requirement_namespaces.rs:252). Stated in `ResolutionRequest` comment: "A source-free forest has none, and those checks stay permissive for it" (`:62-64`).

## selections/authored_selections.rs (2548 lines)
1. Purpose: builds the authored-selection ledger: one record per authored source question (call, operator, member, static argument, case, struct literal, membership, conformance bound), reconciling compiler copies of the same authored expression.
2. Shape: `CandidateTarget` (`:43`), `OperatorQuestion` (`:49`), `Candidate`/`CandidateGroup` (`:77,86`), `reconcile_copy_targets` (`:97`), `original_call_target` (`:164`), `finalize_authored_expression_selections` (`:229`), `normalize_case_membership_facts` (`:333`), statement call candidates (`:395-724`), `finalize_normalized_static_operands` (`:734`), `finalize_conformance_reference_selections` (`:793`), `record_unattached_candidate` (`:834`), `conformance_bound_candidates` (`:886`), `finalize_statement_static_argument_selections` (`:948`), `expression_candidates` (`:1313`), `collect_static_argument_candidates` (`:1587`), `finalize_expression_groups` (`:1730`), `attach_static_argument` (`:1918`). Tests `:1997-2547`.
3. In/out: reads resolved trees; writes ledger rows via `record_*_authored_declaration_selection*` and attaches occurrence ids to expressions, statements and static arguments.
4. Decides: grouping key `(source_span, exposure, kind, compiler_partition, operator question)` (`:1770-1782`). Copy reconciliation: Resolved+Resolved equal => keep; resolved beats late-bound; two different resolved symbols must "share authored declaration" meaning equal `selected_declaration_source` spans (`:208-227`), else error. Call candidates skipped for: unbound contract calls (no top-level declaration of that name, `:1339-1352`), data-layout query spellings (`is_data_layout_query_spelling`, `:1353-1358`), source special forms (`:1363-1366`), non-Source acknowledgement origin (`:1359`). Static argument candidate requires some source-backed path member and symbol selectable (`:1649-1664`); the literal spellings `true`/`false` excluded (`:1653-1659`, string compare via `boolean_literal_spelling`). `static_argument_kind` is `Conformance` if symbol kind is Conformance/ConformanceParameter, else `StaticArgument` (`:1666-1677`). Build operations that "select product declarations" with an invalid target symbol defer operand targets as late-bound (`:1381-1383,1105-1109`).
5. Cases by shape: `expression_candidates` is a closed match: Binary/Indexed/Unary => operator late-bound `CheckedOperator`; Call; Cast with domain; Member; Membership; Name (static application or path segments); StructLiteral; others none (`:1323-1565`). Static-arg addresses `[Argument(i)]` + optional `Member` steps.
6. Same job: three expression-subtree walks in the stage (see expressions.rs item 6); `contract_clause_expression_handles` (`:1182`) + `collect_expression_subtree` (`:1201`).
7. Counts: `same_type_values|callable_inference::pending|normalize_case_membership_facts|original_call_target` 11 matches in 7 files (original_call_target: authored_selections.rs 3, pre_resolution/mod.rs 1 via a different path; `call.authored_call_binding.map(|b| b.original)` there is a different function). `selected_declaration_source|call_declaration_source` 24 matches in 5 files (authored_selections 12, call_identity 3, initializers.rs 3, initializer_dependencies.rs 4, initializer_normalization.rs 2).
8. Limits: `"static input position changed"`, `"original statement static input has ambiguous source custody"` error strings; no unsupported comments.
9. Fail-open: `finalize_authored_expression_selections` skips expressions that already carry occurrences (`:249-253`); `reconcile_copy_targets` `(Resolved, LateBound) => retained` (`:111`) accepts a late-bound candidate against a resolved record without comparing. Status: stated by the header comment "Compiler rewrites may copy an authored expression before selection; every retained copy of the same operator binds to its original occurrence" (`:6-7`). `selection_already_recorded` is a linear scan over all rows per candidate (`:867`), not a safety issue. `record_unattached_candidate` returns `Ok` if an identical row exists (`:838-840`).

## selections/body_calls.rs (434 lines)
1. Purpose: one traversal over every call site in a machine (statements, expressions, transition targets, contract facts, `via` expressions) letting a pass choose a replacement target.
2. Shape: `CallSite` (`:27`), `retarget_machine_calls` (`:52`), `retarget_signature_contract_calls` (`:106`), `append_contract_expressions` (`:127`), `retarget_statement_span` (`:146`), `retarget_transition_target` (`:240`), `retarget_expression` (`:288`), `VisitedExpressions` (`:402`).
3. In/out: `decide: &mut dyn FnMut(&CallSite) -> Option<SymbolHandle>`; mutates `target_symbol`/`path.symbol`.
4. Decides: delegated to the closure; the walk keys on structure. Contract headers are always refreshed; bodies only when `include_body` (`:62`).
5. Cases: closed matches over `StatementNode` and `ExpressionNode`. Assembly blocks only via `Assembly` statements.
6. Same job: see names/target_siblings.rs and `selections/conformance_blocks.rs` (the two users: 8 matches in 3 files).
7. Counts: `retarget_signature_contract_calls`: users in the 3 files (conformance_blocks.rs uses it, grep shows 4 matches there).
8. Comment: bitset `VisitedExpressions` replaces a `Vec` "which made a program-wide contract walk quadratic" (`:394-400`).
9. Fail-open: none seen (a missing machine returns early, `:89-91`; no retarget happens).

## selections/call_identity.rs (81 lines)
1. Purpose: gives each selected call target a declaration source span, including evidence methods and generated machine entries that have no token.
2. Shape: `selected_declaration_source` (`:20`), `call_declaration_source` (`:50`).
3. In/out: reads symbol table and requirement namespaces; returns `SourceSpan` or `Err(String)`.
4. Decides: evidence requirement -> requirement signature's symbol span; Operator/MachineParameter/trait State -> provenance span; other -> `call_declaration_source`: use the symbol's span, else require a State whose parent Machine's `select_machine_entry` result equals the target and use the machine's span (`:57-80`).
5. Cases: three target categories above.
6. Same job: none. 7. Counts: see authored_selections item 7 (callers in 5 files).
8. none. 9. Fail-open: none seen (errors on every miss).

## selections/callable_inference.rs (845 lines)
1. Purpose: infers the callable type of a `machine`-kind generic parameter from the calls that supply it, propagating through forwarding binders until a fixpoint.
2. Shape: `pending` (`:21`), `Use` (`:30`), `StructuralOrigin` (`:39`), `Parameters` (`:45`), `infer` (`:92`), `pending_contract_parameters` (`:272`), `infer_structural` (`:303`), `has_enclosing_lifetime_parameters` (`:451`), `collect_use` (`:499`), `argument_type` (`:599`), `same_type_values` (`:660`), `argument_names` (`:834`).
3. In/out: reads all calls (expression and statement); writes `MachineParameterContract::Nominal` or `Structural` onto the pending parameters; returns diagnostics for any parameter still `Inferred`.
4. Decides: a worklist `ready` indexes uses whose receiving parameter has no pending contract; conflicting nominal routes or arguments => diagnostics (`:221-241`); type equality via `same_type_values` on symbol identity, returning `Err` for forms it cannot compare.
5. Cases by shape (limits stated as errors): inference refuses `Const`/`Value` binder substitutions (`:556-560`), compound type substitution (`:613-616`), applied static selections for structural types (`:309-318`), enclosing lifetime telescope (`:422-427`), mixing nominal and structural (`:364-369`), distinct structural origins (`:373-378`), `Constrained`/`ConstExpression`/`Reference` operands with lifetimes in `same_type_values` (`:811-827`), dependent array extents (`:788-792`).
6. Same job: none.
7. Counts: `callable_inference::pending` and `same_type_values` per the joint grep: `same_type_values` 3 lines in this file; `pending(` used by `selections/mod.rs` and `lowering/data.rs` and `deferred_bodies.rs` (from the file list; unread).
8. Stated limits: "are not implemented" appears 8 times as error text (`:317,367,375,425,558,615,633,825`; e.g. "inference of structural callable types with an enclosing lifetime telescope is not implemented", `:425`).
9. Fail-open: `infer` returns `Ok(())` immediately when `pending_parameters` is empty (`:102-104`), stated: "This lowering-owned inventory gates work, not the declaration catalog" (`:96`) and the later whole-table scan (`:259-264`) only runs when the list is non-empty. Wrong program accepted if a pending parameter exists but was not put in the lowering list: stated at `:98-101` that copied pending contracts "cannot escape the unresolved-contract refusal" only when this function runs. Status: open (traced only within this file).

## Coverage
- Read in full (all 20,840 lines): `names/expressions.rs` 1-1606; `names/lookup.rs` 1-529; `names/mathematical.rs` 1-106; `names/measures.rs` 1-52; `names/member_namespaces.rs` 1-130; `names/mod.rs` 1-609; `names/paths.rs` 1-415; `names/private_nested_calls.rs` 1-106; `names/propositions.rs` 1-463; `names/public_interfaces.rs` 1-142; `names/recursive_tail_calls.rs` 1-250; `names/repeated_locals.rs` 1-340; `names/requirement_namespaces.rs` 1-269; `names/runtime_callable_scopes.rs` 1-168; `names/runtime_callable_types.rs` 1-237; `names/scopes.rs` 1-910; `names/statements.rs` 1-1078; `names/symbol_table.rs` 1-1456; `names/target_machine_families.rs` 1-327; `names/target_siblings.rs` 1-153; `names/targets.rs` 1-2275; `names/top_level.rs` 1-1639; `names/type_predicates.rs` 1-266; `names/type_references.rs` 1-2371; `pre_resolution/initializers.rs` 1-470; `pre_resolution/mod.rs` 1-248; `resolver.rs` 1-317; `selections/authored_selections.rs` 1-2548; `selections/body_calls.rs` 1-434; `selections/call_identity.rs` 1-81; `selections/callable_inference.rs` 1-845.
- Skimmed: none. Not read: none.
- Test modules inside these files (e.g. `expressions.rs:1469-1606`, `targets.rs:1418-2275`, `authored_selections.rs:1997-2548`, `type_references.rs:2111-2349`) were read but I recorded them only as test coverage, not behavior.
- Greps covered `source-rust/` only (and the whole worktree for the pre_resolution API names); `tools/` was not searched.

## Five things a maintainer most needs to know about this unit
1. Symbol assignment is positional: `symbol_table.rs` inserts children in one order and `top_level.rs`/others walk them with `next_child_of_kind`, which silently returns an invalid handle on a kind mismatch or short stream (`top_level.rs:1352-1365`, `symbol_table.rs:1177,1195,1201`). Any reorder breaks symbols without a direct error.
2. The authored-selection ledger (`authored_selections.rs`) reconciles "compiler copies" of one authored expression by source span plus declaration-source equality (`:208-227`); a late-bound candidate is accepted against a resolved row without comparison (`:111`).
3. Callable-type inference refuses a long list of shapes with "not implemented" errors (8 sites in `callable_inference.rs`); value-parameter substitutions and compound types are not supported.
4. Several full-tree `ExpressionNode` walks exist side by side (`expressions.rs`, `propositions.rs`, `repeated_locals.rs`, `body_calls.rs`, `authored_selections.rs`), each with its own closed match; adding a node variant requires touching all.
5. `names/scopes.rs:369-380` special-cases a machine by name string `"X86StaticInterruptScope::run"` plus a sealed-intrinsic check to add two leading declaration roles, and `statements.rs:998` special-cases `"Slice::Length"`.

## What you are unsure about
- Whether `membership_selection` `Contested => Ok(None)` (`lookup.rs:427`) is always reported later; I did not read the downstream validation.
- Whether the first-match behaviors in `attached_call_target` (`scopes.rs:818`) and `select_visible_trait` (`requirement_namespaces.rs:268`) are reachable with real ambiguity.
- Callers of `pre_resolution` functions beyond the grep (`driver/resolve_and_type/tests.rs`, `deferred_bodies.rs`): not read, so whether any production path uses them is unknown.
- Counts are line matches from Grep count mode, not call-site counts; where I said "users" I read the file list rather than each line.
- `contracts.rs`, `domain_facts.rs`, `expression_paths.rs` (siblings in `names/`, not in this unit) were not read; claims about their use of `MachineScope`/`ContractScopeContext` are from grep only.
