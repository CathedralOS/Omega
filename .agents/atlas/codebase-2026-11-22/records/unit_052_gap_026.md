# Record: unit 052, second pass (gap_026) - parser unit tests

Area `02-source-stages-00-04`, crate `source-parser` (stage 01). Root `S/` = `source-rust/subsystems/compiler/source-to-psi/`. All eleven files are in `S/pipeline/01_tokens-to-syntax-trees/tests/unit/` (written `U/`). They are in-crate unit tests, compiled with `#[cfg(test)]` into the parser crate. Eight are declared in `U/mod.rs` (`mod.rs:127-138`, read, via grep); `generic_parameters.rs`, `qualified_types.rs` and `static_argument_delimiters.rs` are pulled in by `#[path]` from `parser.rs:289`, `:346` (as `mod qualified_names`) and `:353` (read, grepped). Shared helpers live in `U/mod.rs` (not in my ranges): `parse_source` (`mod.rs:13`), `parse_source_verbose` (`:17`), `parse_source_unwrap` (`:22`), `dispatch` (`:27`), `argument_path` (`:39`), `parse_source_error` (`:109`), `parse_tokenized_error` (`:114`), `expect_error` (`:118`, a substring check `error.message.contains(needle)`).

Test counts come from Grep count mode of `^#\[test\]` per file: bindings 42, domains 25, expression_stack 12, generic_parameters 5, inline_assembly 2, items_and_selections 38, mathematical_definitions 10, properties_and_requirements 51, qualified_types 5, source_parsing 9, static_argument_delimiters 3; 202 in all.

Common points for items 4 to 9 (apply to every file; not repeated):
- Item 4 (what judgments are keyed on): the tests pin parser outputs by tree shape (`matches!` on `ExpressionNode`/`TypeReferenceNode`/`Item` variants), by source span (`&source[span.start..span.end]`), and, for every rejection, by a substring of the error message through `expect_error` or `error.message.contains(..)`. So the negative tests are keyed on source spelling of the diagnostic text, not on an error code. A few use exact message equality (`bindings_and_conformances.rs:186-190`, `:209-214`, `source_parsing.rs:366-369`, `properties_and_requirements.rs:1363`).
- Item 5 (cases by shape): these are fixtures for fixed program shapes; I quote them where they show a closed list the parser enforces.
- Item 7: no production `pub` code here. Helpers in `U/mod.rs` are used by these files (counts below not needed).
- Item 8: stated limits appear as quoted diagnostics, listed per file.
- Item 9: test code only; "none seen" unless noted. A test that accepts a program is a claim about the parser, not an accepting code path.

## 1. `U/bindings_and_conformances.rs` (1,046 lines, 42 tests)

**Purpose.** Pins how the parser reads external bindings (`via`), stable identities (`#N`, `retired #N`), binding relevance (`[erased]`), lifetime parameters, standalone conformances (`Name satisfies Trait`), evidence binders, retired surface forms (`dyn`, `where`, `spawn`, `provides`, `library`, `abi`), and the `stack N bytes` demand on external machines.

**Shape.** Free `#[test]` functions only; imports `Lexer` (`:1`), `parse_syntax_trees` (`:2`). Examples: `parses_compiler_intrinsic_external_binding_as_a_closed_binding_case` (`:11`), `exact_requirement_application_parses_lifetimes_before_type_arguments` (`:49`), `enforces_all_or_nothing_numbering_per_identity_scope` (`:424`), `parses_name_owned_generic_conformance_telescope` (`:591`), `selection_row_names_its_slot_and_the_existing_machine` (`:745`), `external_stack_demand_reads_three_semantic_tokens_across_trivia` (`:1028`).

**Inputs/outputs.** Reads parser entry points `parse_syntax_trees`, `crate::tests::*` helpers, syntax-tree accessors (`parsed.items.satisfies_clauses`, `state_handles`, `data_members`, `type_parameters`, `conformance_members`). Produces assertions only; one test also calls `parsed.snapshot_json()` and checks substrings (`:268-270`).

**How it decides.** Rejections are keyed on message substrings: "found punctuation `(`" (`:167`), "nonnegative u64" (`:221`), "data does not satisfy traits" (`:681`), "`dyn` types are retired" (`:872`), "`provides` declarations are retired" (`:979-985`), "positive byte count" (`:1045`). Stable identity rule keyed on position/scope: `numbered_mixed_data_is_independent_of_member_order` (`:411`) vs `enforces_all_or_nothing_numbering_per_identity_scope` (`:424`, four sources, all `expect_err`).

**Cases by shape.**
- ForeignBinding forms: `CompilerIntrinsic` parses with no payload (`:11`); `CompilerIntrinsic("..")` rejected (`:153`); `VtableSlot(..)` rejected with an exact retirement message naming `VtableField(field)` (`:171-190`); string `DllImport("legacy.dll","call")` rejected with exact message (`:194-214`); ordinary `via f()` parses as a call expression (`:107`).
- Retired `dyn` spellings, a list of 8 sources (`:861-870`), all expected to give the same message pair.
- `satisfies` clause forms: data subject `PowerOrder: Card satisfies ..` rejected (`:674-683`); retired `as` alias order (`:696-705`); bodyless `ConcreteEvidence satisfies Evidence;` rejected (`:658-671`).
- Reference access: shared / `@mutable` / `@write` (`:878`).
- Lifetime parameters must precede type/const/machine parameters, both in declarations and applications (`:517-535`, message "precede type, const, and machine").

**Same job elsewhere.** Retired-form-with-guidance tests recur in `properties_and_requirements.rs` (`:16`, `:84`, `:110`, `:760`, `:781`, `:872`, `:902`) and `items_and_selections.rs` (`:455`, `:1025`, `:1067`). Lifetime-before-type ordering is also tested in `generic_parameters.rs:178-209` (bound `Buffered<Value, 'held>` rejected, `:204-207`). Binding-relevance parsing is also in `items_and_selections.rs:86-122` and `:857-882`.

**Unused/one-caller.** N/A (tests).

**Stated limits.** Diagnostics quoted: "`ForeignBinding::VtableSlot` is retired" (`:188`), "`ForeignBinding::DllImport(\"module\", \"symbol\")` is retired" (`:211`), "`dyn` types are retired" (`:872`), "spawn { ... }" and "Task<T>" (`:964`), "legacy `library" ... "is retired" (`:1004-1008`), abi: "wiki/spec/build/calling_plans.md" (`:1022`).

**Fail-open risks.** None seen. Two tests assert acceptance only (`empty_binding_brackets_stay_relevant...` `:357-367`, `numbered_cases...` `:225-238`) without inspecting the tree; they would not catch a wrong tree. Test-quality note, "stated" by test names.

## 2. `U/domains_and_transitions.rs` (885 lines, 25 tests)

**Purpose.** Pins parsing of domain declarations, machine termination (`terminates by`), data default `requires`, constrained types (`T in A & B`), borrows/casts/ranges, trait contract clauses, and `transition` dispatch arms (data patterns, outcome proof selectors, field-value patterns).

**Shape.** Free tests plus two local helpers: `assert_expression_count` (`:12`), `assert_path_member_count` (`:23`). Key tests: `rejects_every_authored_domain_subject_shape` (`:71`), `parses_machine_termination_*` (`:103-150`), `retains_complete_default_domain_predicate` (`:217`), `borrows_bind_before_casts_and_after_field_and_index_access` (`:484`), `parses_outcome_arm_payload_and_erased_proof_selectors` (`:679`), `rejects_duplicate_and_defers_noncase_outcome_proof_selectors` (`:802`).

**Inputs/outputs.** Reads syntax-tree tables (`parsed.expressions`, `parsed.statements`, `parsed.items`, `parsed.type_references`); one test copies trees with `SyntaxTrees::extend_from` and re-counts (`:43-53`).

**How it decides.** Keyed on tree shape and spans: the indexed-expression test asserts the operator token span is `[` (`:475-480`); the logical `self` subject is counted by identifier text `"self"`/`"player"` (`:47-52`, a string compare of name text in the test). Rejections by substring: "does not belong on a bodyless requirement" (`:187`), "expected `{`, found identifier `where`" (`:196`), "case-local predicate clauses are retired" (`:261`), "outcome proof selector `selected` is bound more than once" (`:811`).

**Cases by shape.** Six authored domain-subject spellings, all required to fail (`:72-79`). Termination: single subject, tuple subject `(index, limit)`, argumented view `Nat::IncreasingTo(limit)` (`:103-150`). `rejects_duplicate_and_defers_noncase_outcome_proof_selectors` (`:802`) pins that a non-case head `Record { ; selected: local }` parses and "head classification belongs to declaration selection" (`:819`), i.e. the parser does not decide whether the head is a case.

**Same job elsewhere.** Domain-argument parsing tests overlap `static_argument_delimiters.rs:19-198` (domain owner) and `qualified_types.rs:5-57`. Transition target/arm shape tests are also in `items_and_selections.rs:183-452`.

**Unused/one-caller.** N/A.

**Stated limits.** Test comment "head classification belongs to declaration selection" (`:819`); diagnostic "retired" for `where` (`:196`) and case-local `requires` (`:261`).

**Fail-open risks.** None seen. Note `:802-831`: the parser accepts `Record { ; selected: local }` with no check that `Record` is a case; the test says later stage owns it ("stated" by the test's own message at `:819`; downstream not traced by me).

## 3. `U/expression_stack.rs` (445 lines, 12 tests)

**Purpose.** Pins the expression parser's frame loop: depth does not use host stack, operator precedence/associativity, `>>` shift detection, arithmetic policy suffixes, prefix operators, membership grammar, and constant-initializer rejection of suffixes.

**Shape.** Local helper `parse_expression(source) -> (SyntaxTrees, ExpressionHandle)` (`:53`), calling `crate::expressions::parse::parse_expression_handle` (`:1`). Tests: `nested_aggregates_and_grouped_unary_prefixes_use_the_default_stack` (`:7`, 64 nested `~(..)` and 9 struct literals, counts 64/9/8/10 asserted `:35-50`), `operator_stack_retains_precedence_...` (`:84`), `two_greater_tokens_shift_only_when_they_touch` (`:132`), `policy_suffixes_keep_the_base_operator_and_record_its_policy` (`:170`), `a_constant_initializer_rejects_a_nested_policy_suffix` (`:238`), `long_operator_chains_keep_bounded_parser_stack_and_exact_prefix_order` (`:349`), `nested_expressions_parse_on_a_small_thread_stack` (`:381`).

**Inputs/outputs.** Builds an `Input` cursor (`crate::token_input::cursor::Input::new`, `:20`, `:58`) and calls the frame-loop entry directly; also `crate::parse_syntax_trees` for whole-file cases.

**How it decides.** `>>` is a shift only when the two `>` tokens touch: `a > > b` and `a >/**/> b` fail with exact message "expected expression" at the second `>` span (`:145-161`); the doc comment cites `wiki/spec/language/expressions.md` (`:128-130`). Policy suffix is keyed on the lexed operator token text (`+%`, `*|`, `-!`, `>>%`); `>>` takes its suffix from the adjacent token (`:165-168`, `:207-209` comments). Constant rejection keyed on message "is not available in a constant expression yet" and the error span being one of `+%`, `*|`, `>>` (`:250-258`).

**Cases by shape.** Stack test enumerates six nesting shapes at 1000 levels on a 256 KiB thread: parens, calls, index, struct literal, `match`, mixed unary/index (`:383-402`); comment: "nesting costs heap frames rather than host stack" (`:377-379`). Constant-initializer rejects five shapes (`:239-245`). Invalid-prefix list of five (`:329-335`): duplicate `suspend`, duplicate `block`, `block suspend`, `suspend ~perform()`, `block 7`.

**Same job elsewhere.** Prefix `@write -7` literal folding (`:308-316`) and `-1u8 as i32` (`:424-445`) both pin that negation of a literal is folded into the integer node; similar custody tests for floats in `source_parsing.rs:260-315`.

**Unused/one-caller.** N/A.

**Stated limits.** "is not available in a constant expression yet" (`:250`, message quoted in test; parser-side wording). Comment at `:235-236`: "A constant is folded before any operand is typed" (rationale for rejecting suffixes at parse).

**Fail-open risks.** None seen. Note `:424-445` pins that `-1u8 as i32` parses (comment `:428`: "typing must reject that negation"), so the parser deliberately accepts it and defers (stated, not traced into stage 03).

## 4. `U/generic_parameters.rs` (209 lines, 5 tests)

**Purpose.** Pins which generic-parameter binder forms each syntax context admits, via `GenericParameterSyntax` choices.

**Shape.** `SYNTAX_CHOICES: [GenericParameterSyntax; 6]` (`:11-18`): `TypeAndConst`, `StaticBinders`, `DataDeclaration`, `TraitRequirements`, `RequirementSignature`, `MachineDeclaration`; `MathematicalDefinition` is used separately (`:42`). Helper `parse_parameters(source, syntax)` (`:20`) calls `parse_generic_parameters` (`:29`). Tests `:35`, `:56`, `:67`, `:117`, `:178`.

**Inputs/outputs.** Calls `crate::parameters::generics::parse_generic_parameters` with a fresh `SyntaxTrees` (source id 7); checks `ParsedGenericParameters.conformance_bounds`, `type_parameters`, `lifetime_parameters`.

**How it decides.** A matrix: for each syntax context the test computes the expected admission with `matches!(syntax, ..)`. `<proposition Relation>` is admitted only for `TraitRequirements` (`:56-64`). `<Evidence satisfies Trait<Value>>` admitted for TraitRequirements, RequirementSignature, MachineDeclaration, DataDeclaration (`:70-79`); value binder `<Count: usize>` admitted for DataDeclaration, RequirementSignature, MachineDeclaration (`:97-106`). `<machine Selected>` yields `RequirementIdentity` contract only in `TraitRequirements`, `None` in `DataDeclaration` (`:128-137`).

**Cases by shape.** Inline callable bound `<const Callback: machine()>` rejected in all seven contexts (`:36-45`), while `<const Callback: machine>` is accepted only for `MachineDeclaration` (`:46-52`; test name says "inferred binders remain admitted"). Data/alias/trait with machine binder need exactly one authored `requires machine Selected(..)`: missing or duplicated rejected (`:139-175`; trait with empty clause list skipped, `:165-167`).

**Same job elsewhere.** Evidence-binder parsing also tested via full-file route in `bindings_and_conformances.rs:821-857` and `:912-935`.

**Unused/one-caller.** N/A.

**Stated limits.** None quoted. Inline callable bounds are a refused shape (`:37`).

**Fail-open risks.** None seen. In the matrix, the `Ok` branch for conformance asserts `type_parameters` empty for MachineDeclaration (`:92-94`) but does not constrain the `StaticBinders`/`TypeAndConst` Ok case since those are expected Err.

## 5. `U/inline_assembly.rs` (101 lines, 2 tests)

**Purpose.** Pins authored `asm` parsing: mnemonics, operands, clobbers, `requires`/`ensures` facts, jump to transition, and malformed-asm rejections.

**Shape.** `preserves_authored_mnemonics_operands_clobbers_and_facts` (`:6`), `rejects_malformed_asm_syntax_at_the_parser_boundary` (`:81`). Reads `StatementNode::AuthoredAssembly`, `authored_assembly(handle)`, `AssemblyOperands::{Expressions, Transition}` (`:30-77`).

**How it decides.** Mnemonic text kept verbatim, e.g. `vendor.op` with exact span (`:63-65`): the parser does not check mnemonics against an instruction set (inferred from the test accepting `vendor.op`). Clobbers kept as names `rdx, rax` (`:59`). Rejections by substring (`:82-93`): "multiple asm instructions must be separated by `;`", "an asm block must contain at least one", "spell `clobbers none`", "`clobbers <registers>`".

**Cases by shape.** Five malformed shapes listed (`:82-94`): missing `;`, empty block, `requires true {}`, `clobbers { hlt }`, `where clobbers none`.

**Same job elsewhere.** None in unit. Authored-assembly lowering/preparation is in stage 02 (`declarations/assembly/`, per the crate map); not read.

**Unused/one-caller.** N/A.

**Stated limits.** None.

**Fail-open risks.** One observation (inferred): the parser accepts any identifier as mnemonic and register name (`:63`, `:59`); validation is not at this stage. No code read to confirm where it is validated. Classification: open.

## 6. `U/items_and_selections.rs` (1,101 lines, 38 tests)

**Purpose.** Pins source-span custody of names/literals, local `let` parsing (including atomic member calls), trait defaults, terminal acknowledgement (`suspend`/`block`) on transition targets, tail-call target forms, Boolean transition arms and copying, build-selection calls (`Build::select_provider`, `exclude_service`, `bind`, `select_representation`), propositions, `pub` handling, stable identities up to `u64::MAX`.

**Shape.** One local helper `first_machine_statements` (`:175`). Representative tests: `name_expression_source_custody_retains_complete_authored_paths` (`:12`), `member_calls_preserve_their_written_initializer_and_binding_properties` (`:86`), `unmatched_atomic_shapes_publish_only_the_ordinary_binding` (`:125`), `terminal_acknowledgements_preserve_named_and_parenthesized_call_forms` (`:184`), `tail_targets_preserve_other_receiver_calls_without_reclassifying_state_coordinates` (`:246`), `copying_a_dispatch_preserves_wildcard_positions_and_structural_bindings` (`:343`), `root_binding_statement_*` (`:708`, `:748`, `:770`), `parses_primitive_witness_and_transparent_proposition_declarations` (`:885`).

**Inputs/outputs.** Reads `StatementNode`, `TransitionTargetNode::{Named, Value}`, `MatchPattern`, `PropositionBody`, `UseItem::File` (`:319-356`); round-trips via `SyntaxTrees::extend_from` / `new_with_generation(.., 90)` and compares `snapshot()` (`:313-315`).

**How it decides.**
- Named vs value transition target is keyed on the written spelling: `next`, `Main::next` give `Named`; `other.next`, `main.child.next`, `other.child.next` give `Value(Call)` with a receiver (`:247-252`, `:265-300`).
- `block(...)` is "an ordinary call of the contextual name" (comment `:189`); `block (next)(input; proof)` has `acknowledges_block == false` (`:190`).
- Atomic member shapes: seven operations x three binding forms accepted as ordinary `Call` with no `Atomic` node (`:87-122`); the test name `unmatched_atomic_shapes_publish_only_the_ordinary_binding` (`:125`) and `atomic_spellings_are_ordinary_member_calls_through_parsing` (`:464`) indicate no atomic recognition remains in the parser (the list `load, store, fetch_add, swap, compare_exchange, compare_exchange_once` at `:465-472`). Whether `ExpressionNode::Atomic` is still produced anywhere I did not grep; the tests only show it is not produced for these inputs.
- Boolean pairs: `true ->.., false ->..` keep their authored patterns and shared subject (`:306-340`); non-pair shapes keep their tests (`:390-425`).

**Cases by shape.** `Build::bind(builder, Slot, Entry)` operand check: six malformed operand lists all rejected (`:749-756`); computed operands `setup::retain(entry)` and `12` produce an empty `implementation` path and a valid `implementation_operand` (`:771-792`). `Build::exclude_service` receiver-style rejected with "receiver-style service exclusion is retired" (`:633`); `exclude_service::<>()` rejected (`:640`). File imports: nine malformed `use` paths all refused (`:379-389` is in `source_parsing.rs`, not here).

**Same job elsewhere.** Build-selection call parsing (`select_provider`, `exclude_service`, `bind`, `select_representation`) is only here in my unit; stage 02 has `machine_token_bindings.rs` / build-time pre-resolution (crate map, suspected, not read). Copy-then-compare of trees repeats at `domains_and_transitions.rs:43-53` and `source_parsing.rs:298-314`.

**Unused/one-caller.** N/A.

**Stated limits.** "`target` declarations are retired" (`:460`), "silently private API" (`:837`), "`{ Evidence; }` proposition evidence is retired" (`:1034`), "write `requires proposition Relation(...);`" (`:1071`), "requires an authored declaration-site signature" (`:1084`).

**Fail-open risks.** None seen as code. Inferred: `provider_selection_*` tests assert that `machine_arguments` keep paths as written (`:580-588`) and `!call.target.as_str().contains('#')` (`:589`); the parser takes any two paths, not checking that they name declarations (tests give no sign of resolution). Classification: open.

## 7. `U/mathematical_definitions.rs` (259 lines, 10 tests)

**Purpose.** Pins parsing of top-level mathematical `let` definitions and `boundary let` assumptions: parameters with relevance, universe/type binders, arrow types, application types, nullary definitions, and the snapshot visibility of these roots.

**Shape.** Local helpers `parse` (`:12`), `single_definition` (`:17`), `render_type` (`:23`), `render_expression` (`:52`), `render_type_reference` (`:68`). Tests: `parses_mathematical_let_with_relevant_and_erased_parameters` (`:97`), `parses_universe_and_type_binder_telescope` (`:125`), `parses_boundary_let_as_bodyless_assumption` (`:158`), `arrow_types_associate_right_and_bind_domains` (`:176`), `empty_parameter_list_names_a_nullary_definition` (`:202`), rejection tests `:211-248`, snapshot test `:251`.

**Inputs/outputs.** Reads `root_mathematical_definition_handles`, `mathematical_parameters`, `mathematical_type` (`MathematicalTypeNode::{Ordinary, Arrow, Application}`, `:26-48`), `MathematicalDefinitionBody::{Definition, Assumption}`. The renderers panic on any shape they do not list (`:64`, `:89`, `:92`): `other => panic!("unexpected ... in test")`.

**How it decides.** Keyed on rendered text compared as strings (e.g. `"A -> B -> C"`, `"(value: A) -> F(value)"`, `:179`, `:184`). Test `higher` shows `(A -> B) -> C` renders as `"A -> B -> C"` (`:188-190`), so the renderer drops parentheses; the test then checks the tree to separate the two (`:191-198`). Rejections: "boundary let" (`:214`), "`;`" (`:223`), "lifetime" / "machine" for `<'a>` / `<machine M>` (`:232`, `:238`), "->" for `(value: A)` with no arrow (`:247`).

**Cases by shape.** Binders accepted as `TypeParameterKind::Value` including `A: core::Type<u>` (`:138-141`); lifetime and machine binders refused (`:227-239`). Root counts: a definition adds no root item (`root_item_handles().len() == 0`, `:101`).

**Same job elsewhere.** Generic-parameter admission for `MathematicalDefinition` also in `generic_parameters.rs:42` (callable bound refused).

**Unused/one-caller.** N/A.

**Stated limits.** None quoted. Body is "term", `zero` etc.; the parser keeps an expression only (`:99`, `:253-254` use `term`/`zero` as names).

**Fail-open risks.** None seen.

## 8. `U/properties_and_requirements.rs` (1,366 lines, 51 tests)

**Purpose.** Pins capability/trait/machine clause parsing: retired surfaces (`capability entry`, `boundary host`, `effects`, `invariant`, `provider`, `boundary requirement`, `trap`), data property lists (`[copy]`, `[linear]`, `carry(..)`, `[send]`), `reaches`/`suspends`/`blocks`/`invokes` clauses and their source spans, `requires`/`ensures`/`crashes` contracts (named evidence bindings, outcome-specific ensures), fixed operator tokens in declaration heads, and installation-bound `reaches <=`.

**Shape.** Free tests; imports `CapabilityContractKind`, `CrashCause`, `Item`, `Machine`, `StatementNode`, `TransitionExit` (`:7-13`). Examples: `carry_is_refused_on_data_aliases_and_binders` (`:210`), `parses_independent_operational_clauses_on_machines_and_requirements` (`:325`), `parses_contract_clause_orderings_and_predicate_boundaries` (`:461`), `parses_guarded_crash_buckets_on_machines_and_requirements` (`:579`), `rejects_malformed_operator_tokens_on_machine_declarations` (`:719`), `parses_installation_bound_reach_on_bodyless_boundary_requirement` (`:880`), `rejects_ambiguous_and_duplicate_outcome_specific_ensures_surfaces` (`:1087`), `whitespace_and_comments_cannot_separate_two_comparisons` (`:1359`).

**Inputs/outputs.** Reads `capability_contracts`, `proof_facts`, `state_signatures`, `service_reaches`, `invokes`, `suspends_keyword_source_spans`, `spelling: Option<OperatorSpelling>`. One test cross-checks three keyword span lists against `source.match_indices("suspends")` positions (`:393-431`).

**How it decides.** Keyed on spelling and position:
- `carry(...)` refused "at the word `carry`": error `source_span.span.start` equals fixed columns 12/18/17/12 (`:213-227`); doc comment cites `wiki/spec/resources/suspension.md` (`:207-209`).
- Operator tokens: `Option<OperatorSpelling>` from the token spelling (`+` Add, `[]` Index, `[..]` Range, `==` Equal, `-` Subtract), `None` for a plain name (`:663-666`, `:699-715`). Unknown tokens `&&`, `()`, `+=`, `!` rejected "unknown operator spelling" (`:722-736`).
- `reaches` rows: bare `reaches` rejected "write `reaches none`" (`:862-868`); `Suspend`, `Block`, `thread_block` inside a reach row rejected "boundary-service identities only" with replacement names `suspends`/`blocks` (`:970-979`).
- Contextual keywords: `blocks` and `suspends` as parameter names still parse as contract subjects (`:1266-1282`, `blocks` flag set at `:1281`).
- Contract fact lists need a separator: `a > 0 b > 0` fails with exact message "expected `;`, `,`, or end of proof facts" at the second `>` for five whitespace/comment gaps (`:1360-1365`); `a positive, b > 0` keeps `positive` as a call on receiver `a` for the same five gaps (`:1326-1355`).

**Cases by shape.**
- Seven clause-order variants are enumerated with expected (reaches, contracts) counts (`:506-514`).
- Installation-bound `reaches <=` is allowed only on a fresh bodyless boundary requirement; refused on `trait` (non-boundary), on boundary traits with a body, on `requires machine Op()`, on top-level machines (plain, `boundary`, `x86_64`, with `satisfies`) and on named conformance members (`:920-951`); empty or mixed rows refused (`:954-966`).
- Retired standalone `boundary requirement ..` four forms (`:903-908`).
- Outcome-specific ensures: four rejections (Boolean guard, case-literal shape, duplicate group, duplicate machine-wide selector) (`:1088-1105`).
- `machine ... satisfies X` without `via` and without a body fails with "a machine without a body is the ACCEPTED boundary form" (`:1258-1262`).

**Same job elsewhere.** Retired-with-guidance pattern: `bindings_and_conformances.rs` (see file 1). `reaches`/`invokes` rows with static arguments (`Sensor<17u64>`, `:1285-1320`) overlap static-argument parsing in `static_argument_delimiters.rs`. Operator declaration heads: `rejects_retired_operator_introducer_on_trait_requirements` (`:314`), plus a root `operator` declaration is a TASKS.md item (crate map: OPERATOR-MACHINE-SUPPLY); tests at `:629-667` still parse root `operator` declarations (`operator + add(..)`, `operator named(..)`, `:652-655`) and `crate::tests::root_operator(s)`.

**Unused/one-caller.** N/A. Count for `root_operator`: not asked; skipped.

**Stated limits.** Quoted: "`[send]` is retired" (`:235`), "trailing `boundary host` and `boundary Name` contract clauses are retired" (`:124`), "explicit nested `entry` / `public entry` machine members are retired" (`:99`), "`effects` reach clause is retired" (`:875`), "the `invariant` declaration is retired" (`:764`), "write `crash Trap;`" (`:849`), "unknown crash cause `Panic`; expected `Trap` or `Abort`" (`:819`), "`Join<T>`" retired but `Join`/`join` ordinary names (`:138-154`).

**Fail-open risks.** None seen. Test `retired_capability_entry_names_...` (`:16`) pins that `capability Current { entry: bool; state inspect() {...} }` still parses as an ordinary field and state (`:43-65`): the `capability` item remains parseable (not retired as a whole). Test-level observation only.

## 9. `U/qualified_types.rs` (124 lines, 5 tests)

**Purpose.** Pins qualified-name parsing in types, casts and operands: Boolean domain indices keep literal identity and span; path separators need members; module path ending in a static member/generic call; value-cast grammar; file qualifiers `|wire|::`.

**Shape.** `boolean_domain_indices_retain_literal_identity_and_span_in_qualifications_and_casts` (`:5`), `qualified_type_names_reject_missing_path_members` (`:59`), `a_module_path_can_end_in_a_static_member_or_generic_call` (`:77`), `value_casts_require_explicit_domain_entries` (`:86`), `file_qualifiers_can_start_types_and_operands_after_an_operator` (`:104`). Declared as `mod qualified_names` via `#[path]` at `parser.rs:346-347` (grepped).

**Inputs/outputs.** The first test calls `crate::parse_into(&mut syntax, SourceId(17), &tokens)` into trees created with a different id (`:15-20`) and asserts the literal's `source_span.source_id == 17` (`:52`).

**How it decides.** Six malformed qualified names (`dungeon::`, `dungeon::::Damage`, `dungeon::<u64>`, `dungeon\`, `dungeon\\Damage`, `dungeon::Packet\Header`) all rejected (`:61-67`). `x as u64 in Wrapping` / `in Marker` / `in Family<true>` rejected with "`as T in D` is retired" (`:88-92`); the accepted replacements `as +Marker`, `as u64 +Wrapping`, `as @u8 in Marker` parse (`:94-101`). The `|wire|::MASK` operand is checked by identifier text `"|wire|::MASK"` (`:117`), a string compare on a name that contains the pipes; `flags||wire|` is rejected (`:121-123`).

**Cases by shape.** `true`/`false` x type-qualification/cast (4 sources, `:6-14`).

**Same job elsewhere.** `Boolean` index handling also appears in `domains_and_transitions.rs:303-356` for open const expressions; both read `TypeReferenceNode::ConstExpression`.

**Unused/one-caller.** N/A.

**Stated limits.** "`as T in D` is retired" (`:92`).

**Fail-open risks.** None seen.

## 10. `U/source_parsing.rs` (392 lines, 9 tests)

**Purpose.** Pins parse-batch behaviour: appending sources to one tree keeps handles and source identity, worker pages carry one generation, parallel unit parsing preserves order, a failed parse does not corrupt kept trees, import root ranges are exact on both routes, syntax text outlives token/source rows, float spellings share one custody page, and file imports parse their roots/aliases/refusals.

**Shape.** Helper `lexed_source(source_id, path, text) -> tokens::LexedSources` (`:10`) built from `LoadedSources` / `LoadedSource` with `SourceOrigin::User` and `DependencyScope::Build` (`:12-18`). Imports `parse_into as parse`, `parse_source_units`, `parse_sources` (`:2`). Tests: `appending_sources_preserves_root_handles_and_source_identity` (`:23`), `parser_retains_one_worker_page_identity_through_nested_handles` (`:78`, `PAGE_IDENTITY = 7`), `parallel_parser_pages_preserve_source_order_and_direct_handle_lookup` (`:128`, 16 sources, first padded 50,000 comments, `ExecutionBudget::with_workers(4)`), `private_construction_keeps_its_prefix_but_failed_stage_output_is_not_adopted` (`:168`), `import_bounds_preserve_interleaved_root_ordinals_on_both_parser_routes` (`:196`), `syntax_names_remain_valid_after_all_token_and_source_rows_are_released` (`:243`), `float_spellings_share_forest_custody_and_survive_cross_forest_copy` (`:261`), `file_imports_keep_their_roots_filenames_and_local_aliases` (`:318`), `invalid_file_paths_and_aliases_are_refused_at_the_import` (`:359`).

**Inputs/outputs.** Calls `parse_sources(lexed, &mut trees)` (shared page route) and `parse_source_units(lexed)` (independent pages route); reads `parsed.sources`, `parsed.batch`, `ParsedSource.import_root_range`, `root_items`, `parsed.syntax_for(root)`, `page_count()`.

**How it decides.** Identity keyed on `SourceId` and handle `generation()` (`:89-95`, `:158`). The failure test pins two policies: (a) with `parse_into`, a failing parse leaves the prefix already built (`trees.root_item_count() == 1` after `data Retained {} ;`, error at offset 17, `:170-178`); (b) with `parse_sources`, a failing batch leaves existing trees as they were and prepends `appended.omg:2:3: ` to the message (`:180-187`). Import range test: ranges 0..0, 0..0, 0..1, 1..2, 1..4 for five sources, with the Use ordinals `[], [], [0], [1], [1, 3]` (`:197-207`), on both routes (`:210-238`); the range covers from the first to one past the last `use`, including interleaved non-use roots (`:203-206`). File-import refusals: exact "file alias `u8` names a builtin type" (`:366-368`); duplicate alias message with first import line/column (`:374`); nine path shapes refused (`:380-389`) including `..`, `.`, leading `/`, no `.omg`, `.txt`, `{dep}//x.omg`.

**Cases by shape.** `Arc::strong_count(&source)` equals 2 after parsing floats and 3 after a cross-forest copy (`:269-273`, `:303-307`): the test pins that spelling storage is one page owner, not one per literal. Float count 3 ("positive, unsigned operand, folded negative", `:280-283`).

**Same job elsewhere.** Batch parsing is also driven by `S/driver/parse_source_batch/batch.rs:149` (`parse_file`, from the crate map); these tests use `parse_sources` / `parse_source_units` instead (grepped names only, not the driver). Import-range computation is tested here only; code that computes it is in the parser (not in my unit).

**Unused/one-caller.** N/A.

**Stated limits.** None quoted. Test comment on `private_construction_keeps_its_prefix...` name: "failed stage output is not adopted" (`:168`).

**Fail-open risks.** None seen. Observation: policy (a) at `:170-178` leaves a partial prefix after a failed `parse_into`; the test names it "private construction"; stated by the test name, callers not traced.

## 11. `U/static_argument_delimiters.rs` (72 lines, 3 tests)

**Purpose.** Pins how `>` closes static-argument lists in nested type applications, nested domain applications and conformance heads, and when a right shift needs parentheses.

**Shape.** Helper `static_argument_source(owner, operand)` (`:8`) builds `data Packet { value: u64 in Quantity<Inner<..>>; }` for `"domain"`, `Selected satisfies Outer<..> {}` for `"implementation"`, otherwise `data Packet { value: Outer<Inner<..>>; }`. Tests: `a_right_shift_in_a_static_argument_needs_parentheses` (`:20`), `touching_closers_end_a_nested_type_application` (`:54`), `nested_static_values_keep_closers_shifts_and_named_argument_transactions` (`:92`). Declared with `#[path]` at `parser.rs:353` (grepped). Note: the file is 198 lines in the tool output; the unit row says 72 lines. See "Unsure".

**Inputs/outputs.** Reads `TypeReferenceNode::{Generic, Named, ConstExpression, Constrained}`, `TypeConstraintNode::Domain`, `Item::Conformance.trait_arguments`.

**How it decides.** Between `<` and `>`, every `>` closes a list (doc comment `:16-18`): `8u64 >> 1u64` unparenthesized is refused, with the error span at the stray token (`1u64` three bytes after the first `>` for the type and domain owners, `>` one byte after for the conformance head, `:24-47`); `(8u64 >> 1u64)` is one argument. A bare name `N` stays `TypeReferenceNode::Named` and creates no expression rows: "rejected expression alternatives must not publish detached rows" (`:165-169`, expression count asserted 0).

**Cases by shape.** 5 operands x 3 owners = 15 cases (`:93-100`): `1u64`, `N + 1u64`, `(8u64 >> 1u64)`, `((N + 8u64) >> 1u64)`, `N`; the expected binary operator is `ShiftRight` if the operand contains `">>"` else `Add` (`:181-188`) (string search in the test).

**Same job elsewhere.** `>>` handling in expressions: `expression_stack.rs:132-163`; `>>` closing nested generics in `bindings_and_conformances.rs:370-408` (`zero_value::<Optional<T>>()`).

**Unused/one-caller.** N/A.

**Stated limits.** Doc comment `:16-18` is the rule; no "unsupported" wording.

**Fail-open risks.** None seen.

## Cross-file observations

- Retired-syntax tests: 40+ rejection cases assert a message substring naming the replacement (grep not run; counted by reading). They would break on any rewording of parser diagnostics. Classification of "keyed on spelling": yes.
- `U/properties_and_requirements.rs:629-667` still exercises root `operator` declarations (`operator + add(..)`) while the crate map lists removal of the root `operator` declaration as an open TASKS.md item (OPERATOR-MACHINE-SUPPLY, from the crate map, not re-read).
- Duplicate-ish coverage: lifetime-before-type ordering appears in `bindings_and_conformances.rs:92-103`, `:517-535` and `generic_parameters.rs:204-207`.

## Coverage

Read in full, every line: 
- `U/bindings_and_conformances.rs` 1-1046 (1,046)
- `U/domains_and_transitions.rs` 1-885 (885)
- `U/expression_stack.rs` 1-445 (445)
- `U/generic_parameters.rs` 1-209 (209)
- `U/inline_assembly.rs` 1-101 (101)
- `U/items_and_selections.rs` 1-1101 (1,101)
- `U/mathematical_definitions.rs` 1-259 (259)
- `U/properties_and_requirements.rs` 1-1366 (1,366)
- `U/qualified_types.rs` 1-124 (124)
- `U/source_parsing.rs` 1-392 (392)
- `U/static_argument_delimiters.rs` 1-72 per the unit row; I read all 198 lines the tool returned (the unit row says 72; the rest of the file was read too).

Total in the unit rows: 5,999 lines; read in full: 5,999 (plus the 126 extra lines of the last file). Skimmed: 0. Not read: none. I also read supporting snippets of `U/mod.rs` (helpers, via grep, lines 13-39, 109-138) and `parser.rs` `#[path]` lines (via grep); not part of the unit.

## Five things a maintainer most needs to know

1. These eleven files are parser unit tests only; they assert 202 behaviours. Negative tests are keyed on error-message substrings through `expect_error` (`U/mod.rs:118`), so rewording a parser diagnostic breaks them; a few use exact equality (`bindings_and_conformances.rs:186`, `:209`, `source_parsing.rs:366`, `properties_and_requirements.rs:1363`).
2. Roughly a fifth of the tests exist to pin retired surfaces with migration guidance (`dyn`, `where`, `spawn`, `provides`, `library`, `abi`, `capability entry`, trailing `boundary`, `effects`, `invariant`, `trap`, `as T in D`, `boundary requirement`). Each names the replacement spelling in the message.
3. The parser defers several decisions to later stages by design and tests pin that: non-case heads with proof selectors parse (`domains_and_transitions.rs:816-831`), `-1u8 as i32` parses (`expression_stack.rs:424-431`), asm mnemonics/registers are free identifiers (`inline_assembly.rs:63`), build-selection paths are kept as written (`items_and_selections.rs:580-588`).
4. `source_parsing.rs` pins two failure semantics: `parse_into` leaves the already-built prefix on error, `parse_sources` leaves existing trees intact and returns errors prefixed `file:line:col:` (`:170-187`); and import root ranges are exact on both parse routes (`:196-240`).
5. Stack-depth guarantees are tested, not just stated: 1000 levels of six nesting shapes on a 256 KiB thread (`expression_stack.rs:381-422`), 4096 prefix operators, and 1,024-fold operator chains (`:349-375`).

## What I am unsure about

- `static_argument_delimiters.rs`: the unit row says lines 1-72 (72 lines) but the tool returned 198 lines. I read all 198; `wc` was not run (no shell use beyond the allowed tools), so I cannot say which count is authoritative. Cause unknown (possibly CRLF/line-ending counting or a stale row).
- I did not read production parser code, so claims such as "the parser does not validate asm mnemonics" are inferred from the tests accepting them.
- `ExpressionNode::Atomic` (`items_and_selections.rs:140`): whether any production path still produces it was not grepped.
- Whether `crate::tests::root_operator(s)` is still needed once root `operator` declarations are removed was not checked.
