//! Exact direct Boolean entry-parameter condition reconstruction.

use super::*;

pub(super) fn derive<'a>(
    function: usize,
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<DerivedCondition<'a>, LegalizationError> {
    let TargetOperation::ReturnIntegerConditionalControl {
        condition_source,
        condition_parameter_index,
        condition_location: ScalarParameterLocation::Register(register),
        scalar_type,
        when_true,
        when_false,
    } = &target.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let Some(parameter) = optimized.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(abstract_parameter) = abstracted.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    if parameter.value != *condition_source
        || parameter.scalar_type != ScalarType::Boolean
        || abstract_parameter.value != *condition_source
        || abstract_parameter.scalar_type != ScalarType::Boolean
    {
        return Err(Error::UnsupportedCondition { function });
    }
    Ok(DerivedCondition {
        source: *condition_source,
        legalized: LegalizedCondition::DirectParameter {
            parameter_index: *condition_parameter_index,
            register: *register,
            definition_site: parameter.site,
        },
        shape: ScalarConditionShape::DirectBooleanParameter,
        result_type: *scalar_type,
        when_true,
        when_false,
        conditional_node_index: 0,
        provenance_operations: Vec::new(),
    })
}
