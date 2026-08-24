use super::desugar_generic_data_instances;
use omega_layout::{DataShape, build_layout_plan};
use omega_target::NativeTarget;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::expression::ExpressionNode;
use psi_syntax_trees::item::{DataMember, Item};
use psi_syntax_trees::statement::StatementNode;
use psi_syntax_trees::types::TypeReferenceNode;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax)?;
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)?;
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
}

fn rejected(source: &str, expected: &str) {
    let diagnostics = checked(source).expect_err("program should be rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected diagnostic containing {expected:?}, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
    );
}

fn local_initializer<'syntax>(
    syntax: &'syntax psi_syntax_trees::SyntaxTrees,
    local_name: &str,
) -> &'syntax ExpressionNode {
    let expression = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .flat_map(|machine| syntax.items.state_handles(machine.states))
        .flat_map(|state| {
            syntax
                .items
                .statements(syntax.items.state(*state).statements)
        })
        .find_map(|statement| match syntax.statements.statement(*statement) {
            StatementNode::LocalData(local) if local.name.as_str() == local_name => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("named local initializer");
    syntax.expressions.expression(expression)
}

#[test]
fn closed_generic_record_literal_uses_annotated_local_instance() {
    let source = r#"
        data Box<T> { value: T; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            boxed.value
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let ExpressionNode::StructLiteral(literal) = local_initializer(&syntax, "boxed") else {
        panic!("boxed initializer should remain a record literal");
    };
    assert_eq!(literal.type_name.as_str(), "Box<i32>");
}

#[test]
fn nested_closed_generic_record_literal_uses_concrete_field_instance() {
    let source = r#"
        data Box<T> { value: T; }
        data Holder<T> { boxed: Box<T>; }
        machine run() -> i32 {
            let holder: Holder<i32> = Holder { boxed: Box { value: 7 } };
            holder.boxed.value
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let ExpressionNode::StructLiteral(holder) = local_initializer(&syntax, "holder") else {
        panic!("holder initializer should remain a record literal");
    };
    assert_eq!(holder.type_name.as_str(), "Holder<i32>");
    let boxed = syntax
        .expressions
        .struct_fields(holder.fields)
        .iter()
        .find(|field| field.name.as_str() == "boxed")
        .expect("boxed field");
    let ExpressionNode::StructLiteral(boxed) = syntax.expressions.expression(boxed.value) else {
        panic!("boxed field should remain a record literal");
    };
    assert_eq!(boxed.type_name.as_str(), "Box<i32>");
}

#[test]
fn closed_generic_erased_record_elaborates_and_lays_out_material_fields_only() {
    let checked = checked(
        r#"
        data Evidence { case Only; case WithPayload(value: i32); }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            boxed.value
        }
        "#,
    )
    .expect("closed generic erased record should check");

    let literal = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_checked_trees::expression::ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str() == "Box<i32>" =>
            {
                Some(literal)
            }
            _ => None,
        })
        .expect("closed Box literal");
    let fields = checked.expression_table.struct_fields(literal.fields);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["value", "proof"]
    );
    assert!(matches!(
        checked.expression_table.expression(fields[1].value),
        psi_checked_trees::expression::ExpressionNode::Name(_)
    ));
    let evidence = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Evidence")
        .expect("Evidence definition");
    let only_symbol = checked
        .data_members(evidence)
        .iter()
        .find_map(|member| match member {
            psi_checked_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Only" =>
            {
                Some(variant.symbol)
            }
            _ => None,
        })
        .expect("Only variant");

    let layout = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
    let boxed = layout
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Box<i32>")
        .expect("closed Box layout");
    let DataShape::Record { fields } = boxed.shape else {
        panic!("closed Box should have record layout");
    };
    assert_eq!(layout.fields.span_or_empty(fields).len(), 1);
    assert_eq!(boxed.layout.size, 4);

    let graph = omega_checked_trees_to_state_graph::build_state_graph(&checked)
        .expect("runtime state graph");
    assert!(graph.expressions.iter_expressions().all(|(_, expression)| {
        !matches!(
            expression,
            psi_checked_trees::expression::ExpressionNode::Name(path)
                if path.symbol == only_symbol
        )
    }));
}

#[test]
fn closed_generic_record_literal_checks_substituted_field_type() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: true };
            0
        }
        "#,
        "stores a boolean into a `i32` field",
    );
}

#[test]
fn closed_generic_record_literal_checks_concrete_field_names() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { wrong: 7 };
            0
        }
        "#,
        "data `Box<i32>` has no field `wrong`",
    );
}

#[test]
fn closed_generic_erased_record_rejects_ambiguous_omitted_evidence() {
    rejected(
        r#"
        data Evidence { case First; case Second; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            0
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn closed_generic_erased_record_accepts_explicit_ambiguous_evidence() {
    checked(
        r#"
        data Evidence { case First; case Second; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box {
                value: 7,
                proof: Evidence::Second,
            };
            boxed.value
        }
        "#,
    )
    .expect("explicit evidence should remain legal");
}

#[test]
fn distinct_closed_generic_erased_record_instances_validate_independently() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 {
            let integer: Box<i32> = Box { value: 7 };
            let boolean: Box<bool> = Box { value: true };
            integer.value
        }
        "#,
    )
    .expect("each closed instance should use its own substituted field type");
}

#[test]
fn closed_generic_record_literal_uses_exact_assignment_destination() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Holder { integer: Box<i32>; boolean: Box<bool>; }
        machine Holder::replace(&mut self) {
            self.integer = Box { value: 7 };
            self.boolean = Box { value: true };
        }
        "#,
    )
    .expect("an exact field assignment should select each closed record identity");
}

#[test]
fn closed_generic_erased_record_still_rejects_generic_evidence_omission() {
    rejected(
        r#"
        data Evidence<U> { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence<i32>; }
        machine run() -> i32 {
            let boxed: Box<i32> = Box { value: 7 };
            0
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn closed_generic_data_instance_preserves_public_visibility() {
    let source = r#"
        pub data Box<T> { value: T; }
        machine run(value: Box<i32>) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Box<i32>" => Some(definition),
            _ => None,
        })
        .expect("closed Box definition");
    assert!(instance.is_public);
}

#[test]
fn closed_generic_sum_preserves_payload_relevance_and_identities() {
    let source = r#"
        data Evidence { case Only; }
        data Maybe<T> {
            case #1 None;
            case #2 Some(#1 value: T, #2 proof [erased]: Evidence, retired #3);
            retired #4;
        }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize pure sum");

    let definition = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == "Maybe<i32>" => Some(definition),
            _ => None,
        })
        .expect("closed Maybe definition");
    assert!(definition.type_parameters.is_empty());
    let members = syntax.items.data_members(definition.members);
    assert!(matches!(members[0], DataMember::Variant(ref variant) if variant.identity == Some(1)));
    let DataMember::Variant(some) = &members[1] else {
        panic!("Some variant");
    };
    assert_eq!(some.identity, Some(2));
    assert_eq!(some.retired_payload_identities, [3]);
    let payload = syntax.items.data_payload_fields(some.payload);
    assert_eq!(payload.len(), 2);
    assert_eq!(payload[0].identity, Some(1));
    assert_eq!(payload[1].identity, Some(2));
    assert!(payload[1].relevance.is_erased());
    assert!(matches!(
        syntax.type_references.type_reference(payload[0].type_reference),
        TypeReferenceNode::Named(name) if name.as_str() == "i32"
    ));
    assert!(matches!(members[2], DataMember::Retired(4)));

    let ExpressionNode::StructLiteral(literal) = local_initializer(&syntax, "maybe") else {
        panic!("Maybe::Some literal");
    };
    assert_eq!(literal.type_name.as_str(), "Maybe<i32>");
    assert_eq!(
        literal.case_name.as_ref().map(|name| name.as_str()),
        Some("Some")
    );
}

#[test]
fn closed_generic_erased_sum_elaborates_and_lays_out_material_payload_only() {
    let checked = checked(
        r#"
        data Evidence { case Only; case WithPayload(value: i32); }
        data Maybe<T> {
            case None;
            case Some(value: T, proof [erased]: Evidence);
            case ProvenOnly(proof [erased]: Evidence);
        }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            transition maybe {
                Maybe::Some { value, proof as _ } -> value
                Maybe::None -> 0
                Maybe::ProvenOnly { proof as _ } -> 1
            }
        }
        "#,
    )
    .expect("closed generic erased sum should check");

    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Maybe<i32>")
        .expect("closed Maybe definition");
    assert!(definition.type_parameters.is_empty());
    let some = checked
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            psi_checked_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Some" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Some variant");
    assert_eq!(checked.data_payload_fields(some).len(), 2);

    let layout = build_layout_plan(&checked, NativeTarget::host()).expect("layout");
    let maybe = layout
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Maybe<i32>")
        .expect("closed Maybe layout");
    let DataShape::Enum { variants, .. } = maybe.shape else {
        panic!("closed Maybe should have sum layout");
    };
    let variants = layout.variants.span_or_empty(variants);
    assert_eq!(variants.len(), 3);
    assert_eq!(layout.fields.span_or_empty(variants[1].fields).len(), 1);
    assert!(layout.fields.span_or_empty(variants[2].fields).is_empty());
}

#[test]
fn closed_generic_sum_payload_reaches_nested_record_fixpoint() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; }
        data Maybe<T> {
            case None;
            case Some(boxed: Box<T>, proof [erased]: Evidence);
        }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some {
                boxed: Box { value: 7 },
            };
            transition maybe {
                Maybe::Some { boxed, proof as _ } -> boxed.value
                Maybe::None -> 0
            }
        }
        "#,
    )
    .expect("a nested closed payload should reach the synthesis fixpoint");

    for expected in ["Maybe<i32>", "Box<i32>"] {
        assert!(
            checked
                .data_definitions()
                .iter()
                .any(|definition| definition.name.as_str() == expected
                    && definition.type_parameters.is_empty()),
            "expected closed definition {expected}"
        );
    }
    let maybe = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Maybe<i32>")
        .expect("closed Maybe definition");
    let some = checked
        .data_members(maybe)
        .iter()
        .find_map(|member| match member {
            psi_checked_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Some" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Some variant");
    let boxed = &checked.data_payload_fields(some)[0];
    assert!(matches!(
        checked
            .type_reference_table
            .type_reference(boxed.type_reference),
        psi_checked_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "Box<i32>"
    ));
}

#[test]
fn closed_generic_sum_requires_explicit_generic_evidence() {
    rejected(
        r#"
        data Evidence<U> { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence<i32>); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            0
        }
        "#,
        "no unique accessible nullary constructor",
    );
}

#[test]
fn closed_generic_sum_accepts_explicit_generic_evidence() {
    checked(
        r#"
        data Evidence<U> { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence<i32>); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some {
                value: 7,
                proof: Evidence::Only,
            };
            transition maybe {
                Maybe::Some { value, proof as _ } -> value
                Maybe::None -> 0
            }
        }
        "#,
    )
    .expect("an explicit closed generic evidence term should remain valid");
}

#[test]
fn closed_generic_sum_erased_payload_cannot_drive_runtime_data() {
    rejected(
        r#"
        data Maybe<T> { case None; case Some(value: T, proof [erased]: i32); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7, proof: 9 };
            transition maybe {
                Maybe::Some { value as _, proof } -> proof
                Maybe::None -> 0
            }
        }
        "#,
        "has no runtime value, address, read, write, or cleanup",
    );
}

#[test]
fn closed_generic_sum_retains_erased_linear_payload_obligation() {
    rejected(
        r#"
        data Receipt [linear] { case Issued; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Receipt); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7, proof: Receipt::Issued };
            0
        }
        "#,
        "linear value `maybe",
    );
}

#[test]
fn mixed_generic_sum_preserves_common_and_payload_relevance() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Mixed<T> {
            common: T;
            proof [erased]: Evidence;
            case None;
            case Some(value: T, case_proof [erased]: Evidence);
        }
        machine run() -> i32 {
            let mixed: Mixed<i32> = Mixed::Some { common: 1, value: 7 };
            transition mixed {
                Mixed::Some { value, case_proof as _ } -> value
                Mixed::None -> mixed.common
            }
        }
        "#,
    )
    .expect("closed mixed generic data should check");

    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Mixed<i32>")
        .expect("closed mixed definition");
    assert!(checked.data_members(definition).iter().any(|member| {
        matches!(member, psi_checked_trees::data::DataMember::Field(field)
            if field.name.as_str() == "proof" && field.relevance.is_erased())
    }));
    let layout = build_layout_plan(&checked, NativeTarget::host()).expect("mixed layout");
    let mixed = layout
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "Mixed<i32>")
        .expect("closed mixed layout");
    let DataShape::Enum {
        common_fields,
        variants,
    } = mixed.shape
    else {
        panic!("closed mixed data should have sum layout");
    };
    assert_eq!(layout.fields.span_or_empty(common_fields).len(), 1);
    let variants = layout.variants.span_or_empty(variants);
    assert!(layout.fields.span_or_empty(variants[0].fields).is_empty());
    assert_eq!(layout.fields.span_or_empty(variants[1].fields).len(), 1);
}

#[test]
fn closed_generic_sum_does_not_rewrite_non_case_names() {
    let source = r#"
        data Maybe<T> { case None; case Some(value: T); }
        machine run() -> i32 {
            let maybe: Maybe<i32> = Maybe::Some { value: 7 };
            Maybe::DEFAULT
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax).expect("monomorphize pure sum");

    assert!(
        syntax
            .expressions
            .iter_expressions()
            .any(|(_, expression)| {
                let ExpressionNode::Name(path) = expression else {
                    return false;
                };
                matches!(
                    syntax.expressions.identifier_path_members(*path),
                    [base, member]
                        if base.as_str() == "Maybe" && member.as_str() == "DEFAULT"
                )
            })
    );
}

#[test]
fn closed_generic_sum_rewrites_concrete_call_return_and_assignment_uses() {
    checked(
        r#"
        data Maybe<T> { case None; case Some(value: T); }
        data Holder { value: Maybe<i32>; }
        machine make() -> Maybe<i32> { Maybe::Some { value: 7 } }
        machine inspect(value: Maybe<i32>) -> i32 {
            transition value {
                Maybe::Some { value } -> value
                Maybe::None -> 0
            }
        }
        machine run() -> i32 {
            let holder: Holder = Holder { value: Maybe::None };
            holder.value = make();
            inspect(holder.value)
        }
        "#,
    )
    .expect("the unique closed sum identity should cover concrete executable contexts");
}

#[test]
fn closed_generic_sum_does_not_capture_generic_machine_template_paths() {
    let source = r#"
        data Maybe<T> { case None; case Some(value: T); }
        data Holder { value: Maybe<i32>; }
        machine empty<T>() -> Maybe<T> { Maybe::None }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    desugar_generic_data_instances(&mut syntax).expect("monomorphize pure sum");

    assert!(
        syntax
            .expressions
            .iter_expressions()
            .any(|(_, expression)| {
                let ExpressionNode::Name(path) = expression else {
                    return false;
                };
                matches!(
                    syntax.expressions.identifier_path_members(*path),
                    [base, case] if base.as_str() == "Maybe" && case.as_str() == "None"
                )
            })
    );
}

#[test]
fn distinct_closed_instances_of_one_generic_sum_select_exact_paths() {
    checked(
        r#"
        data Maybe<T> { case None; case Some(value: T); }
        machine inspect_integer(value: Maybe<i32>) -> i32 {
            transition value {
                Maybe::Some { value } -> value
                Maybe::None -> 0
            }
        }
        machine inspect_boolean(value: Maybe<bool>) -> bool {
            transition value {
                Maybe::Some { value } -> value
                Maybe::None -> false
            }
        }
        machine run() -> i32 {
            let integer: Maybe<i32> = Maybe::Some { value: 7 };
            let boolean: Maybe<bool> = Maybe::Some { value: true };
            let checked: bool = inspect_boolean(boolean);
            inspect_integer(integer)
        }
        "#,
    )
    .expect("each generic sum occurrence should select its exact closed identity");
}

#[test]
fn distinct_closed_generic_sums_use_unique_exact_call_parameter() {
    checked(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Holder { boolean: Maybe<bool>; }
        machine take(value: Maybe<i32>) -> i32 { 0 }
        machine run() -> i32 { take(Maybe::Some { value: 7 }) }
        "#,
    )
    .expect("the unique call parameter should select Maybe<i32>");
}

#[test]
fn result_domain_overloads_share_exact_erased_generic_argument_contexts() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Holder { boolean_box: Box<bool>; boolean_maybe: Maybe<bool>; }

        machine take(boxed: Box<i32>, maybe: Maybe<i32>) -> i32 { 1 }
        machine take(boxed: Box<i32>, maybe: Maybe<i32>) -> i32 in Saturating {
            2 as i32 in Saturating
        }

        machine run() -> i32 {
            let selected: i32 in Saturating = take(
                Box { value: 7 },
                Maybe::Some { value: 9 }
            );
            selected as i32
        }
        "#,
    )
    .expect("result-domain overloads should share their exact record/sum parameter context");
}

#[test]
fn overloaded_generic_sum_call_literal_remains_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        machine take(value: Maybe<i32>) -> i32 { 1 }
        machine take(value: Maybe<bool>) -> i32 { 2 }
        machine run() -> i32 { take(Maybe::Some { value: 7 }) }
        "#,
        "construction of erased generic data `Maybe` is unsupported in this context",
    );
}

#[test]
fn closed_generic_sum_preserves_generic_zero_home_lemma() {
    checked(
        r#"
        data Optional<T> { case None; case Some(value: T); }
        machine zero_is_none<T>()
        ensures
            zero_value<Optional<T>>() == Optional::None
        {
        }
        data Holder { value: Optional<u64>; }
        "#,
    )
    .expect("closing one runtime instance must not specialize the generic zero-home lemma");
}

#[test]
fn recursive_generic_sum_stays_in_structural_proof_form() {
    checked(
        r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        machine append(s: Seq<u64>, t: Seq<u64>) -> Seq<u64>
        terminates by s;
        {
            transition s {
                Seq::Empty -> (t)
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: head,
                    tail: append(tail, t)
                }
            }
        }
        machine append_empty_right(s: Seq<u64>) -> Seq<u64>
        terminates by s;
        ensures
            append(s, Seq::Empty) == s
        {
            transition s {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: head,
                    tail: append_empty_right(tail)
                }
            }
        }
        "#,
    )
    .expect("recursive generic proof data must retain its structural entailment form");
}

#[test]
fn bare_erased_generic_literal_uses_exact_return_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine make() -> Box<i32> {
            Box { value: 7 }
        }
        "#,
    )
    .expect("an exact return type should select the closed record identity");
}

#[test]
fn bare_erased_generic_literal_uses_exact_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine consume(boxed: Box<i32>) -> i32 { boxed.value }
        machine run() -> i32 {
            consume(Box { value: 7 })
        }
        "#,
    )
    .expect("a unique exact parameter should select the closed record identity");
}

#[test]
fn bare_erased_generic_literals_use_direct_self_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Main { boolean_box: Box<bool>; boolean_maybe: Maybe<bool>; }

        machine Main::consume(
            &self,
            boxed: Box<i32>,
            maybe: Maybe<i32>
        ) -> i32 {
            transition maybe {
                Maybe::Some { value as _, proof as _ } -> (boxed.value)
                Maybe::None -> 0
            }
        }

        machine Main::run(&self) -> i32 {
            self.consume(Box { value: 7 }, Maybe::Some { value: 9 })
        }
        "#,
    )
    .expect("the enclosing attached data should select a direct self-call context");
}

#[test]
fn bare_erased_generic_literal_uses_direct_self_statement_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Main { stored: i32; boolean_box: Box<bool>; }

        machine Main::store(&mut self, boxed: Box<i32>) {
            self.stored = boxed.value;
        }

        machine Main::run(&mut self) -> i32 {
            self.store(Box { value: 7 });
            self.stored
        }
        "#,
    )
    .expect("the enclosing attached data should select a direct self statement-call context");
}

#[test]
fn bare_erased_generic_literals_use_exact_local_receiver_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, boxed: Box<i32>) -> i32 { boxed.value }
        machine run(consumer: Consumer) -> i32 {
            consumer.take(Box { value: 7 })
        }
        "#,
    )
    .expect("an explicitly typed local receiver should select its exact attached owner");
}

#[test]
fn bare_erased_generic_literals_use_exact_self_field_receiver_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, boxed: Box<i32>) -> i32 { boxed.value }
        data Main { consumer: Consumer; boolean_box: Box<bool>; }
        machine Main::run(&self) -> i32 {
            self.consumer.take(Box { value: 7 })
        }
        "#,
    )
    .expect("a direct self-field receiver should select its exact attached owner");
}

#[test]
fn bare_erased_generic_literals_use_exact_local_receiver_statement_call_context() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { stored: i32; }
        machine Consumer::store(&mut self, boxed: Box<i32>) {
            self.stored = boxed.value;
        }
        machine run(mut consumer: Consumer) -> i32 {
            consumer.store(Box { value: 7 });
            consumer.stored
        }
        "#,
    )
    .expect("an explicitly typed local receiver should contextualize a statement call");
}

#[test]
fn exact_local_receiver_overload_disagreement_remains_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, value: Maybe<i32>) -> i32 { 1 }
        machine Consumer::take(&self, value: Maybe<bool>) -> i32 { 2 }
        machine run(consumer: Consumer) -> i32 {
            consumer.take(Maybe::Some { value: 7 })
        }
        "#,
        "construction of erased generic data `Maybe` is unsupported in this context",
    );
}

#[test]
fn computed_attached_receiver_context_remains_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        data Consumer { marker: i32; }
        machine Consumer::take(&self, boxed: Box<i32>) -> i32 { boxed.value }
        machine choose(left: Consumer, right: Consumer, first: bool) -> Consumer {
            transition first { true -> (left) false -> (right) }
        }
        machine run(left: Consumer, right: Consumer) -> i32 {
            choose(left, right, true).take(Box { value: 7 })
        }
        "#,
        "construction of erased generic data `Box` is unsupported in this context",
    );
}

#[test]
fn closed_generic_erased_records_use_concrete_attached_machine_instances() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine Box::stored<T>(&self) -> T { self.value }

        data Main { integer: Box<i32>; boolean: Box<bool>; }
        machine Main::run(&self) -> i32 {
            let flag: bool = self.boolean.stored();
            self.integer.stored()
        }
        "#,
    )
    .expect("closed generic records should clone checked attached methods over erased layout");
}

#[test]
fn parameter_distinct_direct_self_overloads_remain_fail_closed() {
    rejected(
        r#"
        data Evidence { case Only; }
        data Maybe<T> { case None; case Some(value: T, proof [erased]: Evidence); }
        data Main { boolean: Maybe<bool>; }

        machine Main::take(&self, value: Maybe<i32>) -> i32 { 1 }
        machine Main::take(&self, value: Maybe<bool>) -> i32 { 2 }
        machine Main::run(&self) -> i32 {
            self.take(Maybe::Some { value: 7 })
        }
        "#,
        "construction of erased generic data `Maybe` is unsupported in this context",
    );
}

#[test]
fn unused_erased_generic_schema_is_accepted() {
    checked(
        r#"
        data Evidence { case Only; }
        data Box<T> { value: T; proof [erased]: Evidence; }
        machine run() -> i32 { 0 }
        "#,
    )
    .expect("an unused generic schema has no runtime erased representation");
}

#[test]
fn structured_const_field_order_has_one_canonical_instance_identity() {
    let source = r#"
        data UnitIndex { scale: u64; exponent: i32; }
        data UnitIndices {}
        const UnitIndices::A: UnitIndex = UnitIndex { scale: 1, exponent: -2 };
        const UnitIndices::B: UnitIndex = UnitIndex { exponent: -2, scale: 1 };

        data Indexed<const U: UnitIndex> { marker: u8; }
        data Holder {
            left: Indexed<UnitIndices::A>;
            right: Indexed<UnitIndices::B>;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let holder = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == "Holder" => Some(data),
            _ => None,
        })
        .expect("Holder");
    let fields = syntax.items.data_members(holder.members);
    let [DataMember::Field(left), DataMember::Field(right)] = fields else {
        panic!("Holder fields");
    };
    let TypeReferenceNode::Named(left) = syntax.type_references.type_reference(left.type_reference)
    else {
        panic!("left canonical instance");
    };
    let TypeReferenceNode::Named(right) =
        syntax.type_references.type_reference(right.type_reference)
    else {
        panic!("right canonical instance");
    };
    assert_eq!(left, right);
    assert_eq!(
        syntax
            .root_items()
            .filter(|item| matches!(
                item,
                Item::Data(data) if data.name.as_str() == left.as_str()
            ))
            .count(),
        1
    );
}

#[test]
fn runtime_monomorphization_preserves_erased_lifetime_application() {
    let source = r#"
        data View<'buf, T> {
            body: &'buf i32;
            value: T;
        }

        data Holder<'call> {
            view: View<'call, i32>;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let holder = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == "Holder" => Some(data),
            _ => None,
        })
        .expect("Holder");
    let DataMember::Field(view) = &syntax.items.data_members(holder.members)[0] else {
        panic!("Holder.view");
    };
    let TypeReferenceNode::Generic {
        base_name,
        lifetime_arguments,
        arguments,
    } = syntax.type_references.type_reference(view.type_reference)
    else {
        panic!("lifetime application should survive as an erased generic shell");
    };
    assert!(base_name.as_str().starts_with("View<"));
    assert_eq!(lifetime_arguments[0].as_str(), "call");
    assert!(arguments.is_empty());

    let instance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == base_name.as_str() => Some(data),
            _ => None,
        })
        .expect("synthesized View instance");
    assert_eq!(instance.lifetime_parameters[0].as_str(), "buf");
    assert!(instance.type_parameters.is_empty());
}

#[test]
fn concrete_conformance_arguments_follow_generic_result_rewrites() {
    let source = r#"
        data Unit {}
        data Algebra<T> { value: T; }

        trait Projection<A> {
            machine project(subject: &Self) -> A;
        }

        data Subject {}

        machine Subject::project(subject: &Subject) -> Algebra<Unit>
        satisfies Projection<Algebra<Unit>>::project
        {
            Algebra { value: Unit {} }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");

    desugar_generic_data_instances(&mut syntax).expect("monomorphize");

    let machine = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine) if machine.name.as_str().ends_with("::project") => Some(machine),
            _ => None,
        })
        .expect("project machine");
    let state = syntax.items.state(
        *syntax
            .items
            .state_handles(machine.states)
            .first()
            .expect("entry"),
    );
    let conformance = syntax
        .items
        .satisfies_clauses(machine.satisfies)
        .first()
        .expect("Projection conformance");
    let conformance_argument = *syntax
        .type_references
        .type_reference_handles(conformance.arguments)
        .first()
        .expect("concrete algebra argument");

    let TypeReferenceNode::Named(result) = syntax.type_references.type_reference(state.return_type)
    else {
        panic!("concrete generic result should become one synthesized named instance");
    };
    let TypeReferenceNode::Named(argument) =
        syntax.type_references.type_reference(conformance_argument)
    else {
        panic!("conformance argument should follow the result rewrite");
    };
    assert_eq!(result, argument);
}
