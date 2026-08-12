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
    let mut copied = HandleSpan::empty();
    for expression in expressions {
        let expression = copy_runtime_expression(target, program, *expression);
        target
            .expressions
            .push_expression_handle(&mut copied, expression);
    }
    copied
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
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            psi_checked_trees::data::DataMember::Field(field) if field.name == authored.name => {
                Some(field.relevance)
            }
            _ => None,
        })
        .is_none_or(|relevance| !relevance.is_erased())
}

#[cfg(test)]
mod tests {
    use super::copy_runtime_expression;
    use omega_state_graph::StateGraph;
    use psi_checked_trees::{CheckFacts, CheckedTrees};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn runtime_copy_omits_erased_field_and_its_initializer_subtree() {
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
}
