//! Whether an authored two-guard tail needs no fallback because its second
//! guard holds exactly where the first fails. Unit graph edges, scalar graph
//! guards, ordered scalar exits, and ordered scalar completions all re-derive
//! this here. Each guard row is first rejoined to its authored guard, then the
//! pair is judged by the predicate the producers use, with the subject's case
//! roster reconstructed from the authored parameter type.

use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, ScalarType, authored_state,
    unsupported, validate_pure,
};
use checked_trees::data::DataMember;
use checked_trees::{CheckedScalarExpression, CheckedStructuralParameterField};

/// The Guard row at `ordinal + 1` is the exact complement of the row at
/// `ordinal`, both in `state`. A missing or unrejoined row is an error.
pub(crate) fn complementary(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    ordinal: u32,
) -> Result<bool, LoweringError> {
    let guard = |statement| {
        let (source, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, CheckedScalarExpressionRole::Guard)
            .ok_or(LoweringError::Unsupported(
                "complementary fallback lost its pure source guard",
            ))?;
        validate_pure(checked, source, ScalarType::Boolean)?;
        let CheckedScalarExpression::Boolean(expression) = expression else {
            return unsupported("complementary fallback guard is not Boolean");
        };
        Ok(expression.as_ref())
    };
    let next = ordinal
        .checked_add(1)
        .ok_or(LoweringError::Unsupported("fallback ordinal overflow"))?;
    let (first, second) = (guard(ordinal)?, guard(next)?);
    let (_, source) = authored_state(checked, state)?;
    Ok(checked_trees::values::guard_complement::exact_complement(
        first,
        second,
        |subject| declared_cases(checked, source, subject),
    ))
}

/// The declared case keys of the sum the subject path reaches from its
/// authored state parameter, or `None` unless every member of that sum is a
/// case.
fn declared_cases(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    subject: &CheckedStructuralParameterField,
) -> Option<Vec<String>> {
    let reference =
        super::structural_places::declared_parameter_path_type(checked, state, subject)?;
    checked
        .data_members(super::structural_places::declared_nominal(
            checked, reference,
        )?)
        .iter()
        .map(|member| match member {
            DataMember::Variant(case) => Some(case.path_identity()),
            DataMember::Field(_) => None,
        })
        .collect()
}
