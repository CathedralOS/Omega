# Record: unit 052, gap 027 (area 02-source-stages-00-04)

Root: `source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees/tests/unit/` (written `T/`). All six files are in-crate unit tests of the stage 01 parser (source-parser), included via `#[path]` (crate map says `parser.rs:359`). Every file was read in full. They test parser output (syntax trees), not later stages. Items 5, 7 and 9 do not apply to tests except where noted; "none seen" for fail-open throughout (no test helper accepts on a missing row; `expect`/`panic!` on every miss).

Common helpers (read): `crate::frontend_fixture::Lexer::new(text).tokenize()`, `crate::parse_syntax_trees(&tokens)`, `crate::tests::{parse_source, parse_source_verbose, parse_tokenized_error, expect_error}`. These are defined outside my ranges; I did not read them. Grep not run for them.

## T/static_argument_delimiters.rs lines 73-198 (126 lines; lines 1-72 belong to the other reader)

1. Purpose: one test, `nested_static_values_keep_closers_shifts_and_named_argument_transactions` (`:73`... test fn starts at line 74 region; body 76-198), checks that a static (type-position) argument written as an integer, `N + 1u64`, `(8u64 >> 1u64)`, `((N + 8u64) >> 1u64)` or bare `N` parses inside three owners and lands as the right syntax node. The first lines of the range (73-88) are the tail of a preceding test that walks nested single-argument generics down to `u8`.
2. Shape: tail of previous test asserts innermost is `TypeReferenceNode::Named` with text `u8` and `syntax.expressions.iter_expressions().count() == 0` (`:86-88`). Main test loops operand x owner (`:80-87`) with owners `"type"`, `"domain"`, `"implementation"` and builds text via `static_argument_source(owner, operand)` (`:90`, defined in lines 1-72, not read by me).
3. Reads `syntax_trees` (`Item::Conformance`, `Item::Data`, `DataMember::Field`, `TypeReferenceNode::{Generic,Constrained,Named,ConstExpression}`, `TypeConstraintNode::Domain`, `ExpressionNode::{Integer,Binary}`, `BinaryOperator::{ShiftRight,Add}`). Produces only assertions.
4. Judgment keyed on node shape and variant: implementation owner reads `trait_arguments` (`:99-107`); other owners read the first data field's type reference, taking `Generic.arguments` or the `Domain` constraint's `arguments` (`:117-126`). For operand `"N"` it expects `TypeReferenceNode::Named` with text `"N"` and zero expression rows (`:137-150`): rejected expression alternatives must not leave detached rows. Otherwise it expects `ConstExpression` (`:152-157`) whose node is `Integer` for `1u64`, or `Binary` with operator `ShiftRight` if the operand string contains `">>"` else `Add` (`:160-171`). The operator choice is derived from source spelling of the test's own operand string (`operand.contains(">>")`, `:164`); the right operand is asserted `Integer` (`:171-174`).
5. Cases by shape: fixed list of five operands and three owners (`:76-83`); the Binary branch only checks the operator and that the right side is an integer, not the left. Not a refusal list.
6. Same job elsewhere: similar "one inner application then one argument" walks appear in `T/type_constraints.rs` only in different form; suspected, not compared.
7. Test fns not pub; n/a.
8. Stated limits: none. Assertion message at `:143-145`: "rejected expression alternatives must not publish detached rows".
9. Fail-open: none seen.

## T/static_parameters.rs (1-101, 101 lines)

1. Purpose: pins the parser's handling of parameter receivers and reference/mutability spellings: retired `self` forms refuse, `mut` and `&` spellings refuse, `@`-prefixed forms and `mutable`/`write` parse.
2. Shape: five `#[test]` fns: `retired_receiver_parameters_reject_in_every_signature_owner` (`:5`), `implicit_self_expressions_and_transition_targets_reject` (`:21`), `explicit_owned_shared_mutable_and_write_only_parameters_parse` (`:36`), `only_the_current_mutability_modifier_is_accepted` (`:53`), `an_ordinary_name_can_be_borrowed_after_the_at_sign` (`:71`), `reference_introduction_uses_at_and_leaves_ampersand_as_an_operator` (`:81`).
3. Inputs: source strings built by `format!`; output: assertions on `parse_syntax_trees` result.
4. Judgment: parse error versus success, plus error message substring: "`self` receivers are retired" for parameters (`:15`), "implicit `self` is retired" for expression/transition-target uses (`:30`). The mutability test checks only that the error span is non-empty (`:64-65`), not the message. `&`-forms only check `is_err()` (`:93`).
5. Cases by shape: receivers `self`, `mutable self`, `@self`, `@mutable self`, `@write self` crossed with four owners: data-attached machine, trait requirement, `boundary trait` member with `reaches none crashes none`, nested machine (`:6-13`). Implicit self: `self`, `self.value`, `transition { _ -> self }`, `transition { _ -> self() }` (`:22-27`). Old modifier list `mut value`, `let mut`, `@mut`, `@'cell mut`, `@mut value`, `as @mut` (`:54-61`); `&` list (`:82-87`).
6. Same job elsewhere: error-text pinning pattern `crate::tests::expect_error` also used in `T/value_dispatch.rs:125,169`.
7. n/a.
8. Stated limits: names "retired" in expected messages (`:15`, `:30`).
9. Fail-open: none seen. Note `an_ordinary_name_can_be_borrowed...` (`:71`) pins that a type/value named `mut` still parses (only parse success asserted).

## T/static_targets.rs (1-454, 454 lines)

1. Purpose: pins how the parser records call targets, struct-literal field omission (`..`), static versus value receivers, explicit `::<...>` applications, and type-versus-comparison disambiguation of `<`.
2. Shape: tests `field_omission_is_one_final_standalone_marker_and_survives_copy` (`:5`), `call_roots_retain_distinct_target_occurrences_through_syntax_copy` (`:54`) with helper `check_call_extents` (`:74`), `static_and_value_call_targets_remain_distinct_after_syntax_copy` (`:149`), `structural_static_argument_probe_preserves_indexed_comparisons` (`:194`), `applied_requirement_call_keeps_its_trait_application_as_static_receiver` (`:210`), `boolean_type_operands_preserve_constructor_and_value_contexts` (`:253`), `boolean_type_operands_do_not_turn_qualified_type_arguments_into_indices` (`:298`), `explicit_arguments_belong_to_their_expression_path_component` (`:339`), `legacy_generic_calls_and_chained_comparisons_reject_at_parse_time` (`:407`).
3. Inputs: source strings; copies via `SyntaxTrees::copy_expression_from` (`:32`) and `SyntaxTrees::extend_from` (`:78`, `:161`). Outputs: assertions.
4. Judgment: keyed on node fields: `StructLiteral.zero_omitted_fields` and field count (`:26-30`); call spans (`source_span`) and target spans, with a check that target span sits inside call span and that spans are distinct (`:96-118`); `call.target_is_static` (`:171-189`, `:225`, `:363`); `call.machine_arguments.len()` (`:227`, `:364`, `:392`). `check_call_extents` matches each found call to an expected row by comparing source text slice `text[span]` to the expected string (`:97-100`) and the target name via `call.target.as_str` (`:105`): source-spelling comparison, acceptable in a test. Special target name `"$callable"` for `(factory())(9)` (`:68`) is a name the parser publishes for a non-name callee (read, quoted at `:68`).
5. Cases by shape: omission cases `{ left, right }`, `{ left, .. }`, `{ .. }`, `Item::Ready { left, .., }` accepted; `.. other`, `.., left`, `.., ..`, `...`, `left, .. other` refused (`:6-11`, `:40-46`). `check_call_extents` walks only arguments that are calls and the callee if a call (`:121-141`); anything else asserted via `panic!("call")` (`:92`). Refusal list at `:408-421`: `id<u32>(x)`, `id<u32>`, `consume(id<u32>)`, `library::id<u32>(x)`, `receiver.id<u32>(x)`, `id<T, U>()`, `zero_value<T>()`, `Trait<T>::run()`, `Trait::<T>::run<U>(x)` and chained comparisons `a == b < c`, `a < b == c`, `a != b >= c`, `a <= b > c`. Accept list at `:434-443` includes `T == (Box<u64>)`, `(a < b) > (c)`, `a << b < c >> d`. Only `id<u32>(x)` has its message checked ("use `name::<T>(...)`", `:426-431`); the other refusals assert only `expect_err`. `data Holder { field: Box::<u32>; }` must refuse (`:450-453`).
6. Same job elsewhere: the `(left < values[0]) > (right)` comparison appears in both `:194-207` and `:271-274` (same program tested twice, one asserting no generic nodes, the other via a role table). Other test sites for `target_is_static` not grepped.
7. n/a (tests).
8. Stated limits: none. Assertion text at `:203`: "abandoned static argument probes publish no generic types".
9. Fail-open: none seen. Note `boolean_type_operands_preserve_constructor_and_value_contexts` counts equalities with a `TypeExpression` right operand and requires 2 (`:282-294`); the `"constructor"`, `"call"` and `"comparison"` rows assert something different each, and an unmatched role hits `unreachable!()` (`:288`).

## T/type_constraints.rs (1-358, 358 lines)

1. Purpose: pins how `requires` equations and type positions keep type structure (fixed arrays, references, grouped operands) versus value syntax (array literals, borrows), and which malformed array forms refuse.
2. Shape: tests `unary_const_arguments_retain_their_typed_evaluation_obligation` (`:10`), `fixed_array_equation_operands_retain_type_structure_through_grouping_and_copy` (`:49`), `proof_fact_array_values_remain_value_expressions` (`:110`), `reference_operands_retain_access_and_element_type_role_through_copy` (`:139`), `reference_value_operands_remain_borrows_and_recasts_reject_policy_suffixes` (`:191`), `fixed_array_equation_elements_use_existing_generic_type_grammar` (`:225`), `fixed_array_equation_operands_use_the_type_grammars_rejections` (`:264`), `fixed_array_type_operand_is_not_runtime_value_syntax` (`:325`), `comma_constraint_slots_inside_type_brackets_reject` (`:345`).
3. Inputs: source strings via `crate::tests::parse_source_verbose` or `Lexer`+`parse_syntax_trees`; copies via `SyntaxTrees::copy_item_from` (`:64`, `:152`).
4. Judgment: node variant. Unary static arguments `!false`, `(!false)`, `!!true`, `~1u8`, `(~1u8)`, `!0`, `~false` must stay as `ConstExpression(Unary)` "until its carrier is known" (`:13-45`); the parser does not check operand type (`!0`, `~false` accepted). Equation operands: `Backing == [[Element; Count]; 2]` in three groupings must become `FixedArray` with lengths `Literal(2)` and `ConstParameter(Count)` (`:84-97`) and element `Named("Element")` (`:99-104`); the one-operand-retains-a-type-reference find is by `find_map` on `TypeExpression` (`:77-82`). Array literals in `requires` must stay `ArrayLiteral` with no `TypeExpression` (`:123-134`). Reference operand `@{,mutable ,write }[@Element; Count]` must be `Borrow` with the matching `ReferenceAccess` around a `TypeExpression` of fixed array whose element is `Reference{Shared, lifetime None}` (`:163-185`). Policy suffix `in Wrapping` on `as @u8`, `as @ @u8`, `as @mutable @write u8` must refuse with "takes no arithmetic domain" (`:209-217`); removing the suffix must parse (`:218-220`).
5. Cases by shape: `[Element;]` and `[; 4]` refuse; `[Element; -1]` and `[Element; Count + 1]` are accepted by the parser and kept as `FixedArrayLength::Expression(ConstExpression)` for later evaluation (`:264-321`, comment text "computed extents are retained for semantic evaluation", `:281`). Comma slots refuse: `@[u8, [non_empty]]`, `[u8, x <= 10]`, `[i32, 0..=10]`, `[u8, nonzero]` (`:346-351`).
6. Same job elsewhere: the same "retain until its carrier is known" idea appears in `T/static_argument_delimiters.rs:152-157`.
7. n/a.
8. Stated limits: none quoted as unsupported; deferral wording at `:39` "retain expression until its carrier is known" and `:281`.
9. Fail-open: parser-level acceptance of `!0`, `~false`, `[Element; -1]` is deliberate and tested as retained for later evaluation (stated at `:39`, `:281`); whether stages 02-04 refuse them I did not trace (open).

## T/unannotated_constants.rs (1-102, 102 lines)

1. Purpose: pins what the parser publishes for `const NAME = value;` with no annotation: it supplies the declared type named by a literal suffix, `as` target or `true`/`false`, placed at the initializer, and refuses initializers with no named type. Module comment cites `wiki/spec/language/numeric_values.md#literal-landing-and-destinations` (`:3-8`).
2. Shape: helpers `declared_type` (`:14`) returning `(name, source text under span)` and `refusal` (`:41`) returning `(message, source text under span)`. Tests `a_suffix_a_cast_or_a_boolean_names_the_constant_type_at_its_initializer` (`:49`), `an_annotation_is_kept_as_written` (`:68`), `a_wholly_anonymous_initializer_is_refused_where_it_is_written` (`:76`), `an_initializer_whose_type_only_typing_finds_is_refused_as_not_supported` (`:90`).
3. Reads `Item::Const` and `constant.type_reference` as `TypeReferenceNode::Named` (`:17-37`).
4. Judgment: keyed on source spelling of the initializer. Accepted: `7u8`->`u8`, `-3i32`->`i32`, `0.5f32`->`f32`, `true`->`bool`, `300u16 as u64`->`u64`; the span of the synthesized type name equals the initializer text (`7u8`) or, for `as`, the target `u64` (`:50-57`). Annotated `const LIMIT: u16 = 7u8;` keeps `u16` (`:70`).
5. Cases by shape: refusals fall in two fixed groups. "wholly anonymous": `5`, `2 + 3`, `0.5` (`:77-81`, message contains "wholly anonymous"). "not supported yet:" prefix: `measure()`, `OTHER`, `2u8 + 3`, `5 as +Metres` (`:91-96`), i.e. initializers whose type only typing could find, including a mixed suffixed plus unsuffixed sum. The error span must be exactly the initializer text (`:83`, `:101`).
6. Same job elsewhere: crate map says TASKS.md LOWERED-FORM-CHECKER lists "constants with typing-only initializers" (`declarations/constants.rs`); not re-read here.
7. n/a.
8. Stated limits: the "not supported yet:" message prefix is itself pinned (`:98-99`).
9. Fail-open: none seen at the test level; the test asserts refusal for `2u8 + 3` (a shape that could otherwise take `u8`).

## T/value_dispatch.rs (1-183, 183 lines)

1. Purpose: pins parsing of value `match` and `transition` dispatch: subject kept once, arm order and duplicates kept, trailing commas, guards, and exact error spans for malformed comma use.
2. Shape: tests `value_match_retains_subject_once_ordered_duplicate_arms_and_spans` (`:8`), `machine_tail_match_is_a_value_dispatch_not_source_transitions` (`:57`), `nested_initializer_match_retains_expression_local_dispatch` (`:74`), `value_match_rejects_empty_dispatch_and_duplicate_bindings` (`:113`), `dispatch_accepts_single_and_guarded_arms_with_optional_trailing_commas` (`:137`), `dispatch_rejects_missing_or_doubled_arm_commas_at_the_offending_token` (`:160`). Direct entry used: `parse_expression_handle` (`crate::expressions::parse`, `:1`) with `Input::new` (`crate::token_input::cursor`, `:3`).
3. Inputs: source strings; asserts on `ExpressionNode::Match`, `MatchPattern::{Value,Wildcard}`, `match_arms`.
4. Judgment: arms are kept in order, duplicate `0` patterns are both kept (`:29-31`), exactly 4 `Call` nodes and no `Binary` (`:39-52`). Empty `match value {}` refuses with "at least one arm" and a record pattern binding `same` twice refuses with "introduced more than once" (`:113-132`); the expected message is chosen by `source.contains("Record")` (`:127`, source-spelling key in test). Comma errors: missing comma gives "expected `,` between arms" at the start of the following arm `_`; doubled comma gives "expected an arm or `}` after `,`" at the offending second comma, computed as `following_arm - 1` (`:163-181`). Run for both keywords `match` and `transition`.
5. Cases by shape: accepted arm lists `_ -> 1`, `_ -> 1,`, `Flag { tag } if tag -> 1, _ -> 2` (with/without trailing comma) (`:139-144`); transition with a tuple subject `(a, b)` and guarded tuple pattern (`:151-154`).
6. Same job elsewhere: `crate::tests::expect_error` shared with `T/static_parameters.rs:15,30`.
7. n/a.
8. Stated limits: none.
9. Fail-open: none seen. Note: duplicate arm patterns (`0 -> ..., 0 -> ...`) are accepted by the parser by design here (`:29-30`); rejection, if any, is later (not traced, open).

## Coverage

Read in full: `T/static_argument_delimiters.rs` 73-198 (126 lines); `T/static_parameters.rs` 1-101 (101); `T/static_targets.rs` 1-454 (454); `T/type_constraints.rs` 1-358 (358); `T/unannotated_constants.rs` 1-102 (102); `T/value_dispatch.rs` 1-183 (183). Total 1,324 lines. Skimmed: none. Not read: none (lines 1-72 of `static_argument_delimiters.rs` belong to the first reader). No greps were run for item 7 (tests only).

## Five things a maintainer most needs to know

1. These six files only pin parser output; the parser deliberately keeps unevaluated forms (`!0`, `~false`, `[Element; Count + 1]`, `[Element; -1]`, duplicate match arms) for later stages (`type_constraints.rs:39,281`; `value_dispatch.rs:29`), so a green test here says nothing about whether stages 02-04 refuse them.
2. `unannotated_constants.rs` pins the exact parser boundary for `const X = v;`: suffix, `as` target or Boolean literal gets a synthesized type at a pinned span; anything else is refused with either "wholly anonymous" or a "not supported yet:" message (`:76-101`).
3. Most refusal tests assert only `is_err()`; messages are pinned only for `self` retirement (`static_parameters.rs:15,30`), `id<u32>(x)` (`static_targets.rs:426`), policy suffix (`type_constraints.rs:215`), match/commas (`value_dispatch.rs:125,169`) and the constants file.
4. Call-target tests assert span containment and distinctness across `extend_from` copies, and a callee that is not a name is published with target name `$callable` (`static_targets.rs:68`).
5. The `<` disambiguation (generic argument versus comparison) is pinned in `static_targets.rs:194-207, 253-295, 407-454`, including that abandoned probes publish no generic rows and no detached expression rows (`static_argument_delimiters.rs:143-145`).

## Unsure about

- Contents of `static_argument_source` and the `crate::tests` helpers (defined elsewhere, unread).
- Whether `target_is_static` for `receiver.emit()` being false is relied on by later stages (not traced).
- Which later stage, if any, refuses the parser-retained forms listed in item 1.
