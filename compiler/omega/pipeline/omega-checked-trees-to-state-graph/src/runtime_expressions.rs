use omega_state_graph::StateGraph;
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{
    ExpressionHandle, TableStructLiteral, TableStructLiteralField,
};

pub(crate) fn copy_runtime_expression(
    target: &mut StateGraph,
    program: &CheckedTrees,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    target
        .expressions
        .copy_from_filtering_struct_literal_fields(
            &program.expression_table,
            expression,
            &|literal, field| runtime_field_is_retained(program, literal, field),
        )
}

pub(crate) fn copy_runtime_expression_slice(
    target: &mut StateGraph,
    program: &CheckedTrees,
    expressions: &[ExpressionHandle],
) -> HandleSpan<ExpressionHandle> {
    // Copying an expression recursively appends its own child-handle spans to
    // this table. Stage the copied roots so those recursive spans cannot split
    // the contiguous span promised for this sibling list.
    let copied = expressions
        .iter()
        .map(|expression| copy_runtime_expression(target, program, *expression))
        .collect::<Vec<_>>();
    target.expressions.insert_expression_handles(copied)
}

fn runtime_field_is_retained(
    program: &CheckedTrees,
    literal: &TableStructLiteral,
    authored: &TableStructLiteralField,
) -> bool {
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == literal.type_name)
    else {
        return true;
    };
    let common = program.data_members(definition).iter().find_map(|member| {
        let psi_checked_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.name == authored.name).then_some(field.relevance)
    });
    let payload = literal.case_name.as_ref().and_then(|case_name| {
        program.data_members(definition).iter().find_map(|member| {
            let psi_checked_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.name == *case_name).then(|| {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name == authored.name)
                    .map(|field| field.relevance)
            })?
        })
    });
    common
        .or(payload)
        .is_none_or(|relevance| !relevance.is_erased())
}

#[cfg(test)]
mod tests {
    use super::{copy_runtime_expression, copy_runtime_expression_slice};
    use omega_state_graph::StateGraph;
    use psi_checked_trees::{CheckFacts, CheckedTrees};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn runtime_copy_keeps_nested_expression_roots_contiguous() {
        let source = r#"
            data Main {}
            machine Main::run() -> i32 {
                let first: i32 = 1 + 2;
                let second: i32 = 3 + 4;
                first + second
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let roots = typed
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                matches!(
                    expression,
                    psi_checked_trees::expression::ExpressionNode::Binary(_)
                )
                .then_some(handle)
            })
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2);
        let checked = CheckedTrees::with_roots(typed, CheckFacts::default());
        let mut graph = StateGraph::default();

        let copied = copy_runtime_expression_slice(&mut graph, &checked, &roots);

        assert_eq!(graph.expressions.expression_handles(copied).len(), 2);
        assert!(
            graph
                .expressions
                .expression_handles(copied)
                .iter()
                .all(|handle| matches!(
                    graph.expressions.expression(*handle),
                    psi_checked_trees::expression::ExpressionNode::Binary(_)
                ))
        );
    }

    #[test]
    fn runtime_copy_omits_synthesized_nullary_erased_initializer() {
        let source = r#"
            data Evidence { case Only; case WithPayload(value: i32); }
            data Certified { value: i32; proof [erased]: Evidence; }
            data Main {}
            machine Main::run() -> i32 {
                let certified: Certified = Certified {
                    value: 7,
                };
                certified.value
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let literal = typed
            .expression_table
            .iter_expressions()
            .find_map(|(handle, expression)| {
                matches!(expression, psi_checked_trees::expression::ExpressionNode::StructLiteral(literal)
                    if literal.type_name.as_str() == "Certified")
                .then_some(handle)
            })
            .expect("Certified literal");
        let checked = CheckedTrees::with_roots(typed, CheckFacts::default());
        let mut graph = StateGraph::default();

        let copied = copy_runtime_expression(&mut graph, &checked, literal);

        let psi_checked_trees::expression::ExpressionNode::StructLiteral(literal) =
            graph.expressions.expression(copied)
        else {
            panic!("runtime root should remain a struct literal");
        };
        let fields = graph.expressions.struct_fields(literal.fields);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name.as_str(), "value");
        assert_eq!(graph.expressions.expression_count(), 2);
        assert!(graph.expressions.iter_expressions().all(|(_, expression)| {
            !matches!(
                expression,
                psi_checked_trees::expression::ExpressionNode::Name(_)
            )
        }));
    }

    #[test]
    fn runtime_copy_omits_explicit_erased_initializer_subtree() {
        let source = r#"
            data Certified { value: i32; proof [erased]: i32; }
            data Main {}
            machine Main::run() -> i32 {
                let certified: Certified = Certified {
                    value: 7,
                    proof: 11 + 13,
                };
                certified.value
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let literal = typed
            .expression_table
            .iter_expressions()
            .find_map(|(handle, expression)| {
                matches!(expression, psi_checked_trees::expression::ExpressionNode::StructLiteral(literal)
                    if literal.type_name.as_str() == "Certified")
                .then_some(handle)
            })
            .expect("Certified literal");
        let checked = CheckedTrees::with_roots(typed, CheckFacts::default());
        let mut graph = StateGraph::default();

        let copied = copy_runtime_expression(&mut graph, &checked, literal);

        let psi_checked_trees::expression::ExpressionNode::StructLiteral(literal) =
            graph.expressions.expression(copied)
        else {
            panic!("runtime root should remain a struct literal");
        };
        let fields = graph.expressions.struct_fields(literal.fields);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name.as_str(), "value");
        assert_eq!(graph.expressions.expression_count(), 2);
        assert!(graph.expressions.iter_expressions().all(|(_, expression)| {
            !matches!(
                expression,
                psi_checked_trees::expression::ExpressionNode::Binary(_)
            )
        }));
    }

    #[test]
    fn runtime_copy_omits_erased_common_and_exact_case_payload_subtrees() {
        let source = r#"
            data Event {
                sequence: u8;
                common_proof [erased]: u64;
                case Ready(value: u8, payload_proof [erased]: u64);
                case Waiting(other_proof [erased]: u64);
            }
            data Main {}
            machine Main::run() -> i32 {
                let event: Event = Event::Ready {
                    sequence: 1,
                    common_proof: 11 + 13,
                    value: 2,
                    payload_proof: 17 + 19,
                };
                0
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let literal = typed
            .expression_table
            .iter_expressions()
            .find_map(|(handle, expression)| {
                matches!(expression, psi_checked_trees::expression::ExpressionNode::StructLiteral(literal)
                    if literal.type_name.as_str() == "Event"
                        && literal.case_name.as_ref().is_some_and(|name| name.as_str() == "Ready"))
                .then_some(handle)
            })
            .expect("Event::Ready literal");
        let checked = CheckedTrees::with_roots(typed, CheckFacts::default());
        let mut graph = StateGraph::default();

        let copied = copy_runtime_expression(&mut graph, &checked, literal);

        let psi_checked_trees::expression::ExpressionNode::StructLiteral(literal) =
            graph.expressions.expression(copied)
        else {
            panic!("runtime root should remain a case literal");
        };
        assert_eq!(
            literal.case_name.as_ref().map(|name| name.as_str()),
            Some("Ready")
        );
        let fields = graph.expressions.struct_fields(literal.fields);
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["sequence", "value"]
        );
        assert_eq!(graph.expressions.expression_count(), 3);
        assert!(graph.expressions.iter_expressions().all(|(_, expression)| {
            !matches!(
                expression,
                psi_checked_trees::expression::ExpressionNode::Binary(_)
            )
        }));
    }
}
