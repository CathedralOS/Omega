//! Rejoin constructor fields before their computations enter the ordinary
//! operand-scope walk. A matching final type cannot replace source identity.
use crate::expression_preparation::computation_graph::fields;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedScalarCaseConstruction;
use checked_trees::CheckedTrees;
type Computation = checked_trees::CheckedScalarComputationHandle;
use checked_trees::expression::ExpressionHandle;

pub(crate) fn construction(
    checked: &CheckedTrees,
    subject: &CheckedScalarCaseConstruction,
) -> Result<Vec<(ExpressionHandle, Computation)>, LoweringError> {
    let source = validation::scalar_case_constructor(&checked.typed, subject.expression).ok_or(
        LoweringError::Unsupported("computed case lost its exact authored constructor"),
    )?;
    if source.type_reference != subject.type_reference || source.case != subject.case {
        return unsupported("computed case constructor type or case changed");
    }
    let retained = fields(checked, subject)?;
    if retained.len() != source.fields.len() {
        return unsupported("computed case omitted or duplicated authored fields");
    }
    Ok(source
        .fields
        .into_iter()
        .zip(retained)
        .map(|((_, expression, _), field)| (expression, field.value))
        .collect())
}
