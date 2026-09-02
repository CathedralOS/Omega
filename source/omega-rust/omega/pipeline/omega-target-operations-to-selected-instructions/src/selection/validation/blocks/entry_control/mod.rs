//! Optimizer module role: executable entrance. Independently replays the selected entry compare and branch projection.

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

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    match &source.condition {
        LegalizedCondition::DirectParameter { .. } => {
            direct_parameter::validate(function_index, source, function, keys, catalog)
        }
        LegalizedCondition::U64EqualZeroParameterV1 { .. } => {
            equal_zero_parameter::validate(function_index, source, function, keys, catalog)
        }
        LegalizedCondition::U64NotEqualZeroParameterV1 { .. } => {
            not_equal_zero_parameter::validate(function_index, source, function, keys, catalog)
        }
        LegalizedCondition::IntegerEqualParametersV1 { .. } => {
            integer_equal_parameters::validate(function_index, source, function, keys, catalog)
        }
        LegalizedCondition::IntegerLessThanParametersV1 { .. } => {
            integer_less_than_parameters::validate(function_index, source, function, keys, catalog)
        }
        LegalizedCondition::IntegerLessOrEqualParametersV1 { .. } => {
            integer_less_or_equal_parameters::validate(
                function_index,
                source,
                function,
                keys,
                catalog,
            )
        }
        LegalizedCondition::IntegerNotEqualParametersV1 { .. } => {
            integer_not_equal_parameters::validate(function_index, source, function, keys, catalog)
        }
        LegalizedCondition::I64LessThanParametersV1 { .. } => {
            i64_less_than_parameters::validate(function_index, source, function, keys, catalog)
        }
        LegalizedCondition::I64LessOrEqualParametersV1 { .. } => {
            i64_less_or_equal_parameters::validate(function_index, source, function, keys, catalog)
        }
    }
}

fn successors(source: &SourceFunction) -> (SelectedSuccessor, SelectedSuccessor) {
    (
        SelectedSuccessor {
            psi_edge: source.branch_true_edge,
            block: SelectedBlockId(1),
            source_target: source.true_block,
            bindings: source.branch_true_bindings.clone(),
            fuel: source.branch_true_fuel.clone(),
        },
        SelectedSuccessor {
            psi_edge: source.branch_false_edge,
            block: SelectedBlockId(2),
            source_target: source.false_block,
            bindings: source.branch_false_bindings.clone(),
            fuel: source.branch_false_fuel.clone(),
        },
    )
}
