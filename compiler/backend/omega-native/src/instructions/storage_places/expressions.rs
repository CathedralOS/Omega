use omega_typed_program::expression::{Expression, IndexedExpression};
use omega_typed_program::name::ProgramName;

pub(in crate::instructions) fn normalized_storage_expression(
    expression: &Expression,
) -> Option<Expression> {
    match expression {
        Expression::Mutable(target) => normalized_storage_expression(target),
        Expression::Indexed(indexed) => Some(Expression::Name(indexed_expression_path(indexed)?)),
        Expression::Name(_) => Some(expression.clone()),
        _ => None,
    }
}

pub(in crate::instructions) fn indexed_expression_path(
    indexed: &IndexedExpression,
) -> Option<Vec<ProgramName>> {
    let Expression::Integer(index) = &indexed.index else {
        return None;
    };
    let mut path = match &indexed.collection {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
}
