//! Independent replay of one direct Boolean entry-parameter condition.

use super::*;

pub(super) fn replay<'a>(
    function: usize,
    architecture: target::Architecture,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &LegalizedCondition,
) -> Result<ReplayedCondition<'a>, LegalizationError> {
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
    let LegalizedCondition::DirectParameter {
        parameter_index,
        register: proposed_register,
        definition_site,
    } = proposed
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    let Some(parameter) = optimized.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(abstract_parameter) = abstracted.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    if register.architecture() != architecture
        || parameter.value != *condition_source
        || parameter.scalar_type != ScalarType::Boolean
        || abstract_parameter.value != *condition_source
        || abstract_parameter.scalar_type != ScalarType::Boolean
    {
        return Err(Error::UnsupportedCondition { function });
    }
    if proposed_source != *condition_source
        || *parameter_index != *condition_parameter_index
        || *proposed_register != *register
        || *definition_site != parameter.site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(ReplayedCondition {
        source: *condition_source,
        shape: ScalarConditionShape::DirectBooleanParameter,
        result_type: *scalar_type,
        when_true,
        when_false,
        conditional_node_index: 0,
        provenance_operations: Vec::new(),
    })
}
