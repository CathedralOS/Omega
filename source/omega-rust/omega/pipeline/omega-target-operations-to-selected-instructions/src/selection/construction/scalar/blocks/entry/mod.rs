//! Optimizer module role: executable entrance. Selects the exact entry compare while preserving one shared branch envelope.

mod direct_parameter;
mod equal_zero_parameter;
mod i64_less_or_equal_parameters;
mod i64_less_than_parameters;
mod integer_equal_parameters;
mod integer_less_or_equal_parameters;
mod integer_less_than_parameters;
mod integer_not_equal_parameters;
mod not_equal_zero_parameter;

use crate::selection::shared::*;

use super::super::model::ScalarConstructionContext;

pub(in crate::selection::construction::scalar) fn condition(
    context: &ScalarConstructionContext<'_>,
) -> Result<SelectedBlock, SelectedInstructionError> {
    match &context.source.condition {
        LegalizedCondition::DirectParameter { .. } => direct_parameter::build(context),
        LegalizedCondition::U64EqualZeroParameterV1 { .. } => equal_zero_parameter::build(context),
        LegalizedCondition::U64NotEqualZeroParameterV1 { .. } => {
            not_equal_zero_parameter::build(context)
        }
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
        LegalizedCondition::I64LessThanParametersV1 { .. } => {
            i64_less_than_parameters::build(context)
        }
        LegalizedCondition::I64LessOrEqualParametersV1 { .. } => {
            i64_less_or_equal_parameters::build(context)
        }
    }
}
