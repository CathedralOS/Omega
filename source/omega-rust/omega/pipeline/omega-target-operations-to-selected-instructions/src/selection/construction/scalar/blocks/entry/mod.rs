//! Optimizer module role: executable entrance. Selects the exact entry compare while preserving one shared branch envelope.

mod direct_parameter;
mod integer_equal_parameters;
mod integer_less_or_equal_parameters;
mod integer_less_than_parameters;
mod integer_not_equal_parameters;

use crate::selection::shared::*;

use super::super::model::ScalarConstructionContext;

pub(in crate::selection::construction::scalar) fn condition(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    match &context.source.condition {
        LegalizedCondition::DirectParameter { .. } => direct_parameter::build(context),
        LegalizedCondition::IntegerEqualParametersV1 { .. } => {
            integer_equal_parameters::build(context)
        }
        LegalizedCondition::IntegerLessThanParametersV1 { .. } => {
            integer_less_than_parameters::build(context)
        }
        LegalizedCondition::IntegerLessOrEqualParametersV1 { .. } => {
            integer_less_or_equal_parameters::build(context)
        }
        LegalizedCondition::IntegerNotEqualParametersV1 { .. } => {
            integer_not_equal_parameters::build(context)
        }
    }
}
