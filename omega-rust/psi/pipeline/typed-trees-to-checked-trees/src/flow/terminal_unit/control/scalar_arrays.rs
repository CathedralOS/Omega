//! Closed scalar construction is one operation in ordinary authored sequencing.
//! Checking retains selected indexing meaning separately from the literal query.

use super::*;
use typed_trees::expression::ExpressionHandle;

pub(super) fn elements(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> Option<Vec<checked_trees::CheckedScalarArrayLiteral>> {
    let leaves =
        validation::closed_constant_array_elements(program, machine, expression, expected)?;
    let mut projection = expression;
    while let ExpressionNode::Indexed(indexed) = program.expression_table.expression(projection) {
        if let Some(selected) = facts.operators.expression_use(projection)
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
        {
            return None;
        }
        projection = indexed.collection;
    }
    leaves
        .into_iter()
        .map(
            |(leaf, primitive)| match program.expression_table.expression(leaf) {
                ExpressionNode::Integer(literal) => {
                    Some(checked_trees::CheckedScalarArrayLiteral::Integer(
                        if literal.landing().is_some() {
                            literal.clone()
                        } else {
                            validation::land_anonymous_integer_expression(
                                program,
                                leaf,
                                primitive,
                                |_| false,
                            )?
                        },
                    ))
                }
                ExpressionNode::Boolean(value) => {
                    Some(checked_trees::CheckedScalarArrayLiteral::Boolean(*value))
                }
                _ => None,
            },
        )
        .collect()
}
