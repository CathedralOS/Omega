//! Whether an authored two-guard tail needs no fallback because its second
//! guard holds exactly where the first fails. The Unit and scalar graph
//! producers both ask this of the selected Guard rows, through the one
//! predicate in `checked_trees::values::guard_complement`, so both graph
//! families admit the same guard pairs.

use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole, CheckedStructuralParameterField,
};
use typed_trees::TypedTrees;

#[cfg(test)]
mod tests;

/// The Guard row at `ordinal + 1` is the exact complement of the row at
/// `ordinal`, both in `state`.
pub(crate) fn complementary(
    program: &TypedTrees,
    expressions: &CheckedScalarExpressionPlans,
    state: &typed_trees::state::State,
    ordinal: u32,
) -> bool {
    let Some((first, second)) = guard_pair(expressions, state.symbol, ordinal) else {
        return false;
    };
    checked_trees::values::guard_complement::exact_complement(first, second, |subject| {
        declared_cases(program, state, subject)
    })
}

/// Exactly one Boolean Guard row at each of two consecutive statements.
fn guard_pair(
    expressions: &CheckedScalarExpressionPlans,
    state: symbols::SymbolHandle,
    ordinal: u32,
) -> Option<(&CheckedBooleanExpression, &CheckedBooleanExpression)> {
    let guard = |ordinal| {
        let mut selected = expressions.expressions.iter().filter(|expression| {
            expression.state == state
                && expression.statement_ordinal == ordinal
                && expression.role == CheckedScalarExpressionRole::Guard
        });
        let expression = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        match &expression.expression {
            CheckedScalarExpression::Boolean(expression) => Some(expression.as_ref()),
            _ => None,
        }
    };
    Some((guard(ordinal)?, guard(ordinal.checked_add(1)?)?))
}

/// The declared case keys of the sum the subject path reaches from its
/// authored state parameter, or `None` unless every member of that sum is a
/// case.
fn declared_cases(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    subject: &CheckedStructuralParameterField,
) -> Option<Vec<String>> {
    let (_, _, sum, _) = crate::values::resolve_structural_parameter_path(
        program,
        program.state_parameters(state),
        subject.parameter_position,
        &subject.path,
    )?;
    let sum = crate::values::structural_data(program, sum)?;
    program
        .data_members(sum)
        .iter()
        .map(|member| match member {
            typed_trees::data::DataMember::Variant(case) => Some(
                case.identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| case.name.as_str().to_owned()),
            ),
            typed_trees::data::DataMember::Field(_) => None,
        })
        .collect()
}
