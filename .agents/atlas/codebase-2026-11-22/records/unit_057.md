# Record: unit_057 (area 02-source-stages-00-04, crate 03 symbol-resolved-trees-to-typed-trees)

Root of paths: `source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/` (written `T/`).
Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Read-only; every claim below was read unless marked "grepped" or "inferred".

**This record is INCOMPLETE.** Three files and the second half of a fourth were not read (see Coverage). They get no entry here.

---

## T/domain_cast_accounting.rs (997 lines, read in full)

1. **Purpose.** Checks a domain cast such as `value as =Keep -Remove +Add` (comment, lines 1-5). It reads one run of cast entries as one simultaneous cast, checks the entries against the domains the operand's declared type carries, and writes a before-type and after-type on each entry node.
2. **Shape.** `PreparedCasts { runs, initializer_owners }` (`:174`), `PreparedCasts::new` (`:180`), `sites` (`:238`), `account` (`:245`). Free functions: `call_operand_destination` (`:121`, pub(crate)), `account_run` (`:751`), `named_domains` (`:658`), `carried` (`:596`), `operand_type` (`:482`), `refuse_qualified_carrier_operand` (`:415`), `spell` (`:966`), `read_run` (`:101`), `flatten` (`:302`), `parent` (`:325`), `domain_label` (`:366`). Types: `Atom { Declared(SymbolHandle, SemanticDomainId), Policy(ArithmeticDomain) }` (`:47`), `Member` (`:53`), `Named` (`:60`), `Unaccounted { Rejected(Diagnostic), Deferred(DeferredRequest) }` (`:71`).
3. **Inputs and outputs.** Reads `TypedTrees` expression table (cast nodes), `domain_definition`, `const_declarations`, `ProgramOperandOwners` (passed in), `cast_destinations` (a slice of receiving type handles indexed by expression arena index, `:492`), and the resolved trees for `select_domain_candidates`. Mutates the cast nodes (`target_type`, `result_type`, `:945-961`), inserts type references, and records deferred refusals on the holder (`:292`).
4. **How it decides.** Domains are identified by `Atom` (symbol handle plus semantic id), not by name (`:785`, `:816`). An entry is refused if it adds a domain already carried (`:787`), keeps or removes a domain not carried (`:798`), leaves a carried attached domain unnamed (`:819-831`), or leaves two arithmetic policies (`:845-860`). A child with no parent in the result is refused (`:861-878`). Two judgments use spelling: `parent` (`:325-340`) decides that one domain is another's parent by `definition.name.strip_prefix(parent.name)` then `strip_prefix("::")` with no further `::` (string compare of declaration names); `domain_label` (`:366-379`) strips the display string of the target type from the domain name. `named_domains` picks a domain by `select_domain_candidates` keyed on the written path string (`:674-687`) and refuses zero or several candidates (`:688-702`).
5. **Cases by shape.** `operand_type` handles an operand in this order: string/array literal received by a fixed array (`:502-514`), operand with a declared type (`:515`), quoted string received by a shared reference (`:526-553`, otherwise deferred), anonymous numeric literal (`:554-567`), then candidate domains whose carrier is not generic, accepted only if exactly one (`:568-584`); otherwise refused with "has no carrier that the named domain fixes" (`:585-592`). `carried` accepts only `Declared` domains with a valid symbol, arithmetic policies, and nothing else (`:610-651`, comment "A compiler-owned domain or a constraint no declaration answered").
6. **Same job elsewhere.** `call_operand_destination` (`:121`) is called from `call_and_product_selections.rs` (grepped: 2 lines total, 1 caller outside the definition; I did not read that caller). `domain_cast_accounting` stage also appears in `expression_meanings.rs:24` (read).
7. **Unused or one-caller.** `call_operand_destination`: 1 call site outside definition (grepped, `call_and_product_selections.rs`). `PreparedCasts`: used in `expression_meanings.rs:24` only (grepped, 3 lines in 2 files).
8. **Stated limits.** "A run typing cannot account for yet ... is recorded on the declaration that holds it" (`:17-19`); "Bundled libraries are typed with every program, so such a run must not stop a program that never reaches it" (`:20-21`). Message at `:285-288`: "no product route yet: an expression outside every declaration is one {construct}".
9. **Fail-open risks.**
   - `:278-296`: an `Unaccounted::Deferred` run returns `Ok(())` after recording a deferred refusal on each holder. Reaches it: a cast whose operand type `declared_type` cannot read (`:554-558`, `:433`, `:647-650`). Stated (`:17-22`); the refusal row is recorded, so the program is refused only if a holder is reached later. Whether the deferred row always becomes a refusal is not traced here (open).
   - `:424-426` `refuse_qualified_carrier_operand` returns `Ok` when the first entry's operand is not itself a cast node; `:430-432` returns `Ok` for an anonymous literal operand with no declared type. Open (no comment at the early return for the non-cast case).
   - `:219-226` / `:267-277`: more than one holder is refused; zero holders at `:284` refuses. None seen as fail-open.

---

## T/domain_cast_accounting_tests.rs (699 lines, read in full)

Unit tests included by `#[path]` (`domain_cast_accounting.rs:42-44`). 13 `#[test]` functions. They type small programs through `crate::resolution::resolve` then `lower_symbol_resolved_trees` (`:8-27`). Pins: carrier cast cannot drop a requirement's result domain (`:72`), borrowed collection cast keeps its field type (`:93`), declared receiving type still constrains a cast initializer (`:169`), policy entries (Add/Keep/Remove for Wrapping, Saturating, Trapping) keep the written domain (`:190`), typed literals keep carriers (`:258`), contracts retain each entry (`:300`), generic domain arguments kept (`:363`), `result`/`old` are exact callable values not storage (`:400`, `:529`), a shared cast with two callable owners is refused with "more than one exact lexical owner" (`:468-490`), a constant and its initializer share one owner (`:492`), captured cast index keeps its type (`:593`), static value types only in its own scope (`:633`). Negative twins present for the first, fifth, tenth and eleventh.

---

## T/expression_meanings.rs (221 lines, read in full)

1. **Purpose.** Runs, before the later declaration walk, the passes that need operand types first: domain casts, binary operators, index ranges, member selections on inferred receivers, inferred local initializers, constant-expression type values, and proof memberships (`:1-3`).
2. **Shape.** `prepare` (`:7`) with a private `enum Meaning { Cast(usize), Operator(..), Member(..), Range, Local(..) }` (`:14-23`); `prepare_member` (`:171`).
3. **Inputs and outputs.** Reads `TypedTrees`, resolved trees, `frontier` / `type_frontier` (arena indexes; only expressions with index greater than the frontier are touched, `:40`, `:54`, `:80`), `cast_destinations`. Mutates expression nodes (member selection, local `type_reference`, `set_constructed_value_type_reference`), then calls `proof_facts::normalize_membership_selections` and `intern_proof_membership_instances` (`:162-165`).
4. **How it decides.** Sites are sorted by expression arena index with locals after other sites at the same index (`:112-114`). A member is completed only when every owner of the member agrees on the receiver type (`:184-192`); disagreement returns `Ok(())` without selecting (`:187-191`). A member resolves to a declared field via `declared_projection_field` by field *name string* `member.member.as_str()` (`:193-200`) or to the collection length via `CollectionMeasure::from_authored_spelling(member.member.as_str())` (`:206`), only for slices and fixed arrays.
5. **Cases by shape.** Length measure only on `Slice`/`FixedArray` receivers and only `Length` (`:203-210`).
6. **Same job elsewhere.** Operator selection is in `operator_calls.rs` (this unit). Member selection by receiver type is also done on the resolved side (`member_binding::selected_member`, called from `statement_lowering.rs:501`); I did not read that.
7. **Unused or one-caller.** `prepare` is called from `domain_cast_accounting_tests.rs:483` and (grepped) elsewhere; not counted.
8. **Stated limits.** None quoted.
9. **Fail-open risks.** `:187-191`, `:212-214`: an unresolved member receiver type, a disagreeing owner, or an unknown field leaves the member `Unresolved` and returns `Ok(())`. Stated by comment at `:169-170` ("Access permission and live-value checks remain with the receiving judgment") only in part. What reaches it: a member access on a receiver whose type is not known at this stage. Whether a later stage refuses a still-unresolved member: open (not traced).

---

## T/lib.rs (134 lines, read in full)

Module list and re-exports of the typing stage (`:33-50`). `unavailable_judgment` (`:29`) builds "typing could not complete the check ({construct})". Public surface: `lower_symbol_resolved_trees` and `access_borrow_factory` only under `test`/`test-support` (`:119-122`); `SeededTypingBase` and `lower_symbol_resolved_trees_to_seeded_base` always (`:128-131`). `tables` (`:80-117`) holds accessor shims over `SymbolResolvedTrees` and `TypedTreeTables`. Header comments (`:1-23`) name `type_resolved_cohort` and `close_typed_cohort` as the two shared cohort operations. `unavailable_judgment`: grepped 2 lines (not counted in detail).

---

## T/ranking_defaults_tests.rs (158 lines, read in full)

Two tests: default ranking view follows typed carriers (unsigned integer or slice length or the selected builtin `.length` gives Nat descending / slice length; signed, tuple, field named `length` give NULL; an explicit view is kept), and the walk through reference/constraint shells is bounded and keyed on parameter identity not spelling (`:71-158`, quote `:132`: "the same spelling cannot replace the parameter identity"). Calls `settle_machine_ranking_default` in `lowerer.rs`.

---

## T/operator_calls.rs (601 lines, read in full)

1. **Purpose.** Turns a binary operator occurrence into either an ordinary call to a user-declared operator machine, or a retained "core integer operator" selection, once operand types are known. Also refuses a builtin operator whose operand carries a declared domain with no matching operator declaration.
2. **Shape.** `SelectedOperator { target, arguments }` (`:14`), `selection` (`:21`), `selection_for_operands` (`:51`), `sites` (`:134`), `qualified_builtin_operand` (`:164`), `declared_domains` (`:225`), `core_application` (`:262`), `retain_core_application` (`:318`), `prepare_site` (`:369`), `retain_application` (`:504`). Private `enum SelectedMeaning { Declaration, Core }` (`:257`).
3. **Inputs and outputs.** Reads `TypedTrees`, `DeclaredExpressionTypes` cache, `ProgramOperandOwners`, authored declaration selections. Rewrites the binary expression node in place into `ExpressionNode::Call(TableCallExpression{..})` (`:471-493`) and sets its result type (`:494-498`), or finalizes an intrinsic selection (`:347-358`).
4. **How it decides.** Only `ArithmeticDomain::Exact` binaries are considered (`:30`, `:147`, `:272`). Candidates come from `resolve_spelling_for_operands` by operator spelling and operand type handles (`:63`). Zero candidates gives `None` (builtin); more than one is an error "ambiguous operator declarations" (`:66-105`). A selected operator is kept only when its machine's `supply_mode` is `CheckedBody` or `TopLevelRequirement` and it has no `conformance_bounds` (`:109-120`). All operands anonymous gives builtin (`:60-62`). If one owner of the expression picks a different meaning than another, error "conflicting declaration contexts" (`:384-392`). The error text for a qualified builtin operand builds a suggested cast from `display_name` strings and a space test `value.contains(' ')` (`:199-205`).
5. **Cases by shape.** `core_application` takes only integer carriers: `core_integer_operand(...)` must return `BuiltinTypeAtom::Integer` (`:282-286`); at least one operand must be typed (`:299`); anonymous numeric operands take the other operand's carrier (`:300-307`).
6. **Same job elsewhere.** Equality is expanded in `equality_preparation.rs:126-311` with its own selection logic for `==`/`!=` (this unit). Both finalize an intrinsic receipt on the authored selection: `operator_calls.rs:347-358` and `equality_preparation.rs:317-369` (same shape, separate code).
7. **Unused or one-caller.** `selection_for_operands`: 3 lines in 2 files (`operator_calls.rs`, `typing_queries.rs`); 1 outside caller (grepped). `selection`/`sites`/`prepare_site` are called from `expression_meanings.rs:30,125` (read).
8. **Stated limits.** `:107-108`: "Explicit evidence and open applications remain unavailable here." `:162-163`: "the lowered-form reader refuses those."
9. **Fail-open risks.**
   - `:109-120`, `:121-127`: a selected operator with conformance bounds or explicit evidence returns `Ok(None)`, so the occurrence stays a builtin binary node. Stated at `:107-108`. What reaches it: a user operator declaration that is generic over a conformance. Traced only to the comment; later refusal in the lowered-form checker is claimed by the comment at `:162-163` but not traced by me.
   - `:398-405`: no selected meaning and no qualified-operand diagnostic returns `Ok(())` leaving the builtin node. Stated at `:162-163`.
   - `:40-44`, `:176-179`: an operand of unknown non-literal type returns `None` and skips the selection. Stated: "Unknown nonliteral types never act as wildcard operands" (`:20`).

---

## T/equality_preparation.rs (1133 lines, read in full; tests `:844-1133`)

1. **Purpose.** Expands `==` and `!=` on data values: either to a plain builtin comparison (scalars, text bytes, field-less enums), to a call to the selected core `Equatable` implementation's `equals`, or to an inline structural comparison of fields and variant tags.
2. **Shape.** `prepare` (`:126`), `prepare_selections` (`:37`) building `PreparedSelection { identity, application, ordinal, is_public, selection }` rows sorted by type identity, `Preparation` (`:371`) with `selection` (`:414`), `is_visible` (`:404`), `validate` (`:509`), `compare` (`:561`), `fields` (`:649`), `field` (`:669`), `tag` (`:705`), `call` (`:755`), `fold` (`:830`), `complete_expanded_equality` (`:317`).
3. **Inputs and outputs.** Reads `program.conformances()` and keeps those whose trait has `SealedIntrinsic::EqualityProtocol` (`:49-52`); reads operand types via `DeclaredExpressionTypes`; rewrites the comparison expression node in place (`:302-308`) to a builtin comparison, a call node, or an `And`/`Or` tree. `!=` is `(== result) == false` (`:296-300`).
4. **How it decides.** Implementation is chosen by `package_qualified_type_identity` of the operand type (binary search, `:419-426`). Visible if public or in the same source package, and `source_reference_may_select_symbol(site, conformance)` (`:404-412`); more than one visible gives "multiple Equatable implementations" (`:440-445`); none visible but some declared gives "not Equatable here" (`:429-434`). Primitive / text / field-less enum fields are compared builtin (`:538-543`, `:570-575`). `is_text` is shape-keyed: shared slice of `u8`, or a domain-qualified fixed `[u8; N]` (`:472-507`). `is_implicit` = enum whose variants have no payload and no quotient (`:460-470`). Operand type mismatch errors "operands are different data types" (`:282-290`). A quotient data type is refused (`:638-646`).
5. **Cases by shape.** `prepare_selections` requires the conformance to have exactly one argument (`:54-61`), exactly one row (`:69-73`), and a signature with exactly two shared-reference parameters (`:80-97`). Structural expansion errors on a record whose every field is erased (`:600-604`). A struct literal operand has its field read directly by name (`:680-696`).
6. **Same job elsewhere.** See operator_calls item 6. Selection-by-identity binary search also appears in `runtime_callables.rs:637` (different key).
7. **Unused or one-caller.** Only `prepare` is crate-visible; called from `lowerer.rs` (grepped by name only in the crate listing; not counted).
8. **Stated limits.** `:313-316`, `:228-230`: unknown types keep "the original comparison for the lowered-form checker".
9. **Fail-open risks.**
   - `:227-231`: if either operand type is unknown the comparison is left unchanged (`continue`). Stated ("Keep the original comparison for the lowered-form checker").
   - `:279-281`: neither operand is data, left alone. Reached by any comparison of non-data types; presumed builtin.
   - `:291-293`: a field-less enum is left as builtin without checking an `Equatable` implementation.
   - `:239-249`: when not inside a synthesized body, a non-reference or non-readable reference operand type is returned unchanged rather than refused. Stated by the comment at `:235-238`.

   Tests `:844-1133` pin: expanded equality keeps core meaning and the selected user call (`:847`), borrowed enum compare (`:909`), quotient missing observer evidence (`:930`), cross-package visibility (`:1021-1037`), distinct reference-argument rows (`:1040`), one comparison owner and ambiguity error (`:1081`), qualified rows do not share (`:1101`).

---

## T/runtime_callables.rs (1701 lines, read in full; tests `:1400-1701`)

1. **Purpose.** Retains the contracts of runtime callable types (`data F = machine(...) -> ...`), binds each call of a callable value to its contract, admits callable types by structural identity, and checks that two operators binding the same fixed token do not have the same operand shape.
2. **Shape.** `lower_contracts` (`:11`), `bind_invocation_lifetimes` (`:84`), `lower_contract_reference` (`:143`), `callee_is_value` (`:170`), `named_implementation_source` (`:189`), `named_value_source` (`:248`), `named_value_needs_destination` (`:267`), `prepare_named_value` (`:282`), `CallOwner` (`:422`: `State`, `Declaration`, `Signature`), `ScopeCoordinate` (`:555`), `CallScopes` (`:611`), `CallIndexes` (`:662`), `CallInventory` (`:830`), `bind_calls` (`:1080`), `RuntimeCallableAdmission` (`:821`), `admit_types` (`:1229`), `validate_pending_token_shapes` (`:1324`).
3. **Inputs and outputs.** `lower_contracts` reads `source_trees.tables.declarations.runtime_callable_contracts` and writes `typed_trees.runtime_callable_contracts` and the `runtime_callable_lowering` index. `bind_calls` walks the whole program with `typed::references::visit_program`, then writes the `contract` of each runtime call binding in expressions and statements (`:1193-1224`). `prepare_named_value` replaces a name expression with `ExpressionNode::MachineValue` (`:405-412`).
4. **How it decides.** Contract rows are keyed by the resolved handle `(arena_index, generation)` and stale ownership errors (`:27-44`, `:147-165`). Scope identity is `ScopeCoordinate` (numeric handles), with a full `CallOwner` equality check on a hit (`:637-648`). `bind_calls` skips an old use whose contract is already valid (`:1129-1135`). A callee type is followed through `Constrained` to a `Callable` node; any other node gives "not a runtime callable" (`:1143-1166`). Admission names binders `$Outer{n}G{ordinal}` / `$Outer{n}V{ordinal}` (`:1277`, `:1296`), i.e. synthesized strings used as identity binders. `validate_pending_token_shapes` compares operator operand signatures plus `(is_mutable, is_const)` mode pairs (`:1363-1382`) and refuses a duplicate with the message "already bound by" (`:1383-1394`).
5. **Cases by shape.** `prepare_named_value` supports a callable destination reached through `Constrained` only (`:291-297`); sources are a conformance row or a plain `Machine`/`State` symbol (`:248-262`); refuses boundary declarations (`:370-373`) and machines with static parameters (`:374-379`).
6. **Same job elsewhere.** `named_implementation_source` path validation resembles named-call owner logic in `named_conformance_member` (comment `:332-336`: "Reuse the ordinary named call owner"); I did not read it.
7. **Unused or one-caller.** Grepped (`grep -rn` over `source-rust`, `--include=*.rs`): `named_value_needs_destination` 2 lines (1 outside caller, `call_and_product_selections.rs`); `prepare_named_value` 3 lines (callers in `call_and_product_selections.rs`); `bind_calls` 3 lines (`lowerer.rs`); `admit_types` 2 lines (`lowerer.rs`); `validate_pending_token_shapes` 2 lines (`lowerer.rs`); `lower_contract_reference` 3 lines (`type_reference.rs`); `callee_is_value` 8 lines in `expression_lowering.rs`, `runtime_callables.rs`, `statement_lowering.rs`.
8. **Stated limits.** "Explicit evidence ..." not here. `:339-336` comment: the public member "may be a closed requirement of this map's applied trait." `:1321-1323`: "Source checking retains pairs whose dependent callable operands have no complete identity yet."
9. **Fail-open risks.**
   - `:358` `prepare_named_value` returns `Ok(false)` for no recognized source (destination not callable, not a name, or the name is not a machine/state/conformance member). Traced: the caller keeps the name as an ordinary value; later checks treat it as a value assignment (comment `:280-281`). Open on whether a bad name is caught downstream.
   - `:244` `named_implementation_source` returns `Ok(None)` for other head kinds; same fall-through.
   - `:418` `let _ = contract;` discards the destination contract; comment `:414-417` says "proof checks refinement against the selected body's effective contract". Stated; not traced.
   - `:1129-1135` old calls with a valid contract are not rebound; keyed on the stored handle only. None seen as unsound.

   Tests (`:1400-1701`): named machine value dependencies, qualified/unqualified `TrimOne::shrink` and stale member refusal.

---

## T/qualified_operator_home_tests.rs (1380 lines, read in full)

Test support plus about 24 tests. Support functions used by other test files in this crate: `checked` (`:9`), `checked_with_equality` (`:20`), `packages_with_equality` (`:37`), `packages` (`:60`); `packages` builds a `SourceMap`, lexes and parses each unit with `frontend_fixture`, adds toolchain units as package 0 and others as user units, registers dependency scope and top-level import bindings, calls `resolution::resolve` then `lower_symbol_resolved_trees` (`:60-159`). Some tests include `source/library/core/equality.omg` via `include_str!` and walk `source/library/core` from `CARGO_MANIFEST_DIR` (`:20-30`, `:167-191`), so they depend on the bundled library on disk. Tests pin: core imports resolve per file (`:193`), core policy casts keep subtraction signature (`:238`), mixed-policy length comparisons keep a `CoreIntegerOperator` selection (`:326`), inferred locals retain one read per expression (`:438`), collection length type queries use the retained selection and not the spelling (`:500`; the test changes `node.member` to "diagnostic_spelling_only" and the type query still answers, `:574-575`), carrier-generic domains retain each exact application (`:616`), generic operator application as an ordinary call (`:690`), anonymous operand cannot select a qualified operator (`:736`), builtin operator refusal text for qualified operands (`:772-850`, the expected string repeated at `:782-786`), undeclared-name deferred refusals keep their own scope (`:892-979`), bare case spelling stays an unknown name (`:985`), operator home/visibility/duplicate-shape errors (`:1102-1233`), body imports belong to the declaring file (`:1258`), generic-bound local name conflict (`:1350`). Negative twins present for most.

---

## T/statement_lowering.rs (1828 lines, read in full)

1. **Purpose.** Lowers each resolved statement in a machine state body to the typed statement: calls, transitions, root bindings, inline-assembly windows, assignments, evaluate/expression statements, and local data with initializer type inference.
2. **Shape.** Everything is inside `pub(crate) mod statement` (`:6`). `LocalTypes` (`:9`, rows `(SymbolHandle, Option<TypeReferenceHandle>)` sorted by handle); submodules `arguments` (`:151`), `calls` (`:199`), `initializer_type` (`:463`), `root_bindings` (`:1044`), `transitions` (`:1095`), `assembly` (`:1264`); `lower_statement` (`:1416`), `complete_blocks` (`:1455`), `lower_local` (`:1490`), `lower_statement_node` (`:1591`).
3. **Inputs and outputs.** Reads resolved statements and expressions, the `Lowerer`, `LocalTypes`. Produces `Vec<typed::statement::StatementNode>`; calls expression lowering for every operand (`lower_statement_expression` -> `lower_executable_expression_handle`, `:160-165`).
4. **How it decides.** `lower_call_statement` (`:210`) tries, in order: callee is a value (`:214`), product description request (`:238-271`), product query operation (`:272-318`), product selections (`:319-363`), else ordinary call. `infer_initializer_type` (`:476`) infers a local's type only when the declared type is `Unit` and the local was marked inferred (`lower_local`, `:1502-1512`). The inference is keyed on expression shape: borrow, member with selected collection length -> `u64` (`:498-514`), struct literal, data-layout query spelling (`is_data_layout_query_spelling(call.target.as_str(), ..)`, a call-name string, `:538-545`), call with declared return, name/member/indexed projection through `declared_projection_type`, and Boolean-valued expressions -> `bool` (`:656-666`). `static_conformance_requirement_return` finds a return type by *name* `requirement.name.as_str() == call.target.as_str()` across a trait and its required traits (`:809-830`). `transition` guards: a `When` guard that is a closed Boolean `true` becomes `Always` only when `closes_run` (`:1185-1191`).
5. **Cases by shape.** `declared_projection_type` handles a single-name root with Indexed/Member/Borrow steps and gives up on ranges (`:687-692`) and multi-member names (`:707`). Machine-parameter contracts are searched across every machine in the program by symbol equality (`:564-610`). `assembly::lower` copies blocks, jumps and zero-flag exits (`:1341-1352`).
6. **Same job elsewhere.** Statement-level product selection duplicates the expression-level one: `calls::lower_call_statement` (`:319-363`) and `build_selections::lower` used by expression lowering (not read; suspected). `static_conformance_requirement_return` (`:775-835`) and the call-return lookup in `declared_call_return` are two ways of finding a call's declared return (the second is in another file, suspected).
7. **Unused or one-caller.** `LocalTypes`: 14 lines in `declarations.rs` and `statement_lowering.rs` (grepped). `infer_initializer_type`: 6 lines in `hoist_temps.rs` and `statement_lowering.rs` (grepped; 1 outside caller).
8. **Stated limits.** `:1604-1606` "assembly typing requires its complete enclosing statement window". `:765-768`: "Applied field views are prepared after declaration lowering. Keep this initializer for ordinary capture inference". `:1689`: `unreachable!` for `ProofOutputBindingStatement` ("evidence packages are classified out before runtime statement lowering").
9. **Fail-open risks.**
   - `:668` `infer_initializer_type` returns `Ok(None)` for any shape it does not know, leaving the local with an invalid type handle (`:1512`). Stated by comment at `:765-768` for one path; later inference is claimed ("ordinary capture inference"). What reaches it: a local whose initializer is a shape not listed. Open on whether every such local gets typed later.
   - `:1185-1191`: a guard that `closed_boolean_guard` decides is `Some(true)` is dropped to `Always` on the last transition of a run. Keyed on a closed-Boolean query (`exhaustiveness`); not traced.
   - `:1542`: `Authored::Waive | Authored::Equal(_) => MatchFieldAction::Waive` turns an `Equal` field pattern into a waive in the typed roster. Open: no comment; the equality value is not carried here (inferred it is handled in the pattern's guard elsewhere, not read).
   - `:1599`: atomic `store` calls short-circuit via `atomic_calls::store`; not read.

   Tests: `boolean_result_tests` (`:1718`) pins Bool types for locals and dispatch subjects; `scoped_field_tests` (`:1787`) pins that a field initializer uses the receiver's own package.

---

## T/lowerer.rs, lines 1-1100 (read); lines 1101-2394 NOT READ

Header comment (`:1-6`): "Typing entrance (`lower_symbol_resolved_trees`), cohort typing and settlement (`type_resolved_cohort` and `close_typed_cohort`), terminating-record progress normalization (`progress`), target-machine family telescopes, and append-only seeded continuation (`seeded_continuation`)."

Read portion:
- `name::lower_name` (`:38`): copies a diagnostic name into `Identifier::generated(name.as_str())`.
- `progress` (`:46-623`): `normalize_trait_progress_premises` (`:55`), `normalize_machine_progress_premises` (`:111`), `settle_machine_ranking_default` (`:179`), `pending_ranking_default` (`:192`), `inherited_guarantee` (`:282`), `authored_premises` (`:343`), `subject_path` (`:383`). It rewrites termination guarantees from `requires` memberships in a domain classified `ProgressProfile` (`:360-364`). `subject_path` finds a progress subject root by parameter symbol, then falls back to a parameter *name* string compare (`:398-406`), and a member by selected field symbol, falling back to a name lookup `field.name.as_str() == member_name` (`:419-425`, `:460-463`). The ranking default (`:192-280`) is keyed on typed carrier: selected builtin collection length gives Nat descending (`:219-227`), a name rooted at a parameter or static value whose type, after peeling `Constrained`/`Reference`, is a slice/array (slice length view) or an unsigned integer atom (Nat descending) (`:262-275`); a tuple subject gives no default (`:211-213`); loop bounded by the type reference count (`:258`). Inherited guarantees come from the first satisfied trait declaration that resolves (`:286-338`) and remap premise roots by parameter position, erroring if the position is missing (`:320-334`).
- `seeded_continuation` (`:624-...`): `SeededContinuationError` (`:642`), `SeededTypingBase { resolved: Arc, typed: Arc }` (`:650`), methods `activate_body_completion` (`:664`), `append_resolved` (`:681`), `activate_sources` (`:703`), `complete_bodies` (`:784`), `prepare_allocations` (`:791`), `into_typed` (`:827`); `lower_symbol_resolved_trees_to_seeded_base` (`:832`), `lower_seeded_extension` (`:843`), `lower_body_completion` (`:908`), `resolved_normalized_prefix_is_retained` (`:983`), `domain_table_retains` (`:1022`), `resolved_completion_retains_declarations` (`:1030-...`). Continuations require pointer/value equality of the retained predecessor (`:669`, `:848`, `:917`), that the authored-selection ledger rows start with the retained rows (`:855-861`, `:927-935`), and that retained statement, transition-target and type-reference rows are unchanged (`:994-1019`). `activate_sources` narrows the typed program to selected source files (`:703-782`) via `project_source_declarations` and filters `resolved.roots.*` by `DeclarationSelection` (`:729-747`).

Fail-open: `lowerer.rs:233-235` `pending_ranking_default` returns `None` (no default) on any non-name subject; the machine then has no ranking view. Reaches it: a ranking subject that is not a parameter name. Traced only to the test `ranking_defaults_tests.rs:45-54` showing signed/tuple/field subjects give NULL; whether a machine with a NULL view and `terminates` is refused later: open.

Not read: `lowerer.rs:1101-2394` (1294 lines), including `Lowerer`, `TypingCohort`, `lower_symbol_resolved_trees` (`:1585` per crate map), `type_resolved_cohort`, `close_typed_cohort`, `join_resolved_semantics`, `validate_resolved_semantics_in`, `append_authored_service_reach_rows`, `lower_evidence_forwarding`.

---

## NOT READ (no entry)

- `T/equality_and_exhaustiveness.rs` (3068 lines): not read.
- `T/expression_lowering.rs` (4221 lines): not read.
- `T/lowerer.rs` lines 1101-2394: not read.

---

## Coverage

- Read in full: `domain_cast_accounting.rs` 997, `domain_cast_accounting_tests.rs` 699, `equality_preparation.rs` 1133, `expression_meanings.rs` 221, `lib.rs` 134, `operator_calls.rs` 601, `qualified_operator_home_tests.rs` 1380, `ranking_defaults_tests.rs` 158, `runtime_callables.rs` 1701, `statement_lowering.rs` 1828, `lowerer.rs` 1-1100 (1100). Total read in full: 9,952 lines. (Test-file bodies were read; entries above summarize them by group.)
- Skimmed: none.
- Not read: `equality_and_exhaustiveness.rs` 1-3068 (3068), `expression_lowering.rs` 1-4221 (4221), `lowerer.rs` 1101-2394 (1294). Total not read: 8,583 lines. Unit total 18,535 (sum of the tsv).
- Commands run: one `grep -rn`/`grep -rln` loop over `source-rust` for the 14 names in item 7 (line counts above are line counts of matches, not call-site counts, and include the definition line).

## Five things a maintainer most needs to know about this unit (read portion only)

1. Binary operators and equality go through two separate selectors that both finalize an intrinsic receipt on the authored selection: `operator_calls.rs:prepare_site` (`:369`) and `equality_preparation.rs:prepare` (`:126`). Both overwrite the binary node in place and both leave the builtin node when an operand type is unknown, relying on the lowered-form checker (`operator_calls.rs:162-163`; `equality_preparation.rs:228-229`).
2. Domain cast accounting records unsupported shapes as deferred refusals on the holder and returns `Ok` (`domain_cast_accounting.rs:278-296`), because bundled library code is typed with every program (`:20-21`). A cast that is never reached is not refused.
3. Parent/child domain relationships in cast accounting are decided by declaration-name string prefix with `::` (`domain_cast_accounting.rs:325-340`); the rest of the accounting is keyed on symbol handles and semantic ids.
4. `runtime_callables.rs` keys contracts and scopes on `(arena_index, generation)` handles and refuses stale ones; it synthesizes binder names `$Outer{n}G{k}` / `$Outer{n}V{k}` for structural identity (`:1277`, `:1296`).
5. `lowerer.rs` `seeded_continuation` extends a retained typed program only if the resolved predecessor is equal, the authored-selection ledger is a prefix, and retained statement/target/type rows are unchanged (`:848-861`, `:917-945`, `:983-1020`); `statement_lowering.rs` `infer_initializer_type` only knows a fixed list of initializer shapes (`:476-669`) and returns `None` otherwise.

## What I am unsure about

- Whether deferred refusals recorded by `domain_cast_accounting.rs:292` always turn into a compile refusal when the holder is reached (not traced).
- Whether `statement_lowering.rs:1542` dropping an `Equal` pattern action to `Waive` is compensated elsewhere.
- Call-site counts in item 7 are line counts from one grep and include the definition; I did not count calls per site.
- The whole content of `expression_lowering.rs`, `equality_and_exhaustiveness.rs` and the second half of `lowerer.rs` is unknown to me.
