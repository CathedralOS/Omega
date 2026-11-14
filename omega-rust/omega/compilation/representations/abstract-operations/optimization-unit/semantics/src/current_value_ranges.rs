//! Independently reconstructs constant-backed range claims from current scalar operations.
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use optimization_unit::{PsiOptimizationUnit, ValueRangeFact};
use semantic_vocabulary::{BlockId, MachineId};
mod availability {
    use crate::candidates::global_value_numbering::independent_reachable_dominators;
    use crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition;
    use crate::{OperationSite, OptimizationUnitValidationError};
    use optimization_unit::{
        PsiOptimizationFunction, PsiOptimizationUnit, ValueDefinitionSite, ValueRangeFact,
    };
    use semantic_vocabulary::{BlockId, MachineId, ValueId};

    pub(super) fn validate_current_value_range_fact_at(
        unit: &PsiOptimizationUnit,
        fact: &ValueRangeFact,
        machine: MachineId,
        block: BlockId,
        node: u32,
    ) -> Result<(), OptimizationUnitValidationError> {
        let function = unit.function(machine).ok_or(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable(
                OperationSite::new(machine, block, node),
            ),
        )?;
        let query_block = function.block(block).ok_or(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable(
                OperationSite::new(machine, block, node),
            ),
        )?;
        if usize::try_from(node)
            .ok()
            .is_none_or(|node| node >= query_block.nodes.len())
            || fact.valid_in.machine != machine
            || !value_available_at(function, fact.value, block, node)
        {
            return Err(
                OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable(
                    OperationSite::new(machine, block, node),
                ),
            );
        }
        Ok(())
    }

    pub(super) fn value_available_at(
        function: &PsiOptimizationFunction,
        value: ValueId,
        block: BlockId,
        node: u32,
    ) -> bool {
        let Some(definition) = scalar_value_definition(function, value) else {
            return false;
        };
        match definition.site {
            ValueDefinitionSite::FunctionParameter(_) => true,
            ValueDefinitionSite::BlockParameter {
                block: definition_block,
                ..
            } => {
                definition_block == block
                    || independent_reachable_dominators(function)
                        .get(&block)
                        .is_some_and(|dominators| dominators.contains(&definition_block))
            }
            ValueDefinitionSite::Node {
                block: definition_block,
                node: definition_node,
            } => {
                if definition_block == block {
                    definition_node < node
                } else {
                    independent_reachable_dominators(function)
                        .get(&block)
                        .is_some_and(|dominators| dominators.contains(&definition_block))
                }
            }
        }
    }
}
mod reconstruction {
    use crate::candidates::sparse_conditional_constant_propagation::{
        scalar_value_definition, validator_scalar_constant_facts,
    };
    use optimization_core::ValueRangeFactIdentity;
    use optimization_unit::{
        PsiOptimizationUnit, ScalarConstantValue, ValueRangeFact, ValueRangeRegion,
        ValueRangeSupport, value_range_fact_identity,
    };
    use semantic_vocabulary::{BlockId, MachineId, ValueId};

    fn constant_range(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        value: ValueId,
    ) -> Option<ValueRangeFact> {
        let function = unit.function(machine)?;
        let scalar_type = scalar_value_definition(function, value)?
            .scalar_type
            .as_integer()?;
        let (_, constant, identity) = validator_scalar_constant_facts(unit.identity, function)
            .into_iter()
            .find(|(candidate, constant, _)| {
                *candidate == value && matches!(constant, ScalarConstantValue::Integer(_))
            })?;
        let ScalarConstantValue::Integer(constant) = constant else {
            return None;
        };
        let support = ValueRangeSupport::ScalarConstant(identity);
        let valid_in = ValueRangeRegion {
            revision: unit.identity,
            machine,
            value,
        };
        Some(ValueRangeFact {
            identity: value_range_fact_identity(
                value,
                scalar_type,
                constant,
                constant,
                &support,
                &valid_in,
            )?,
            value,
            scalar_type,
            minimum: constant,
            maximum: constant,
            support,
            valid_in,
        })
    }

    pub(super) fn reconstruct_value_range_fact(
        unit: &PsiOptimizationUnit,
        supplied: &ValueRangeFact,
    ) -> Option<ValueRangeFact> {
        constant_range(unit, supplied.valid_in.machine, supplied.value)
    }

    pub(crate) fn independently_reconstruct_value_range_fact_at(
        unit: &PsiOptimizationUnit,
        identity: ValueRangeFactIdentity,
        machine: MachineId,
        value: ValueId,
        block: BlockId,
        node: u32,
    ) -> Option<ValueRangeFact> {
        let fact = constant_range(unit, machine, value)?;
        (fact.identity == identity
            && super::availability::value_available_at(unit.function(machine)?, value, block, node))
        .then_some(fact)
    }
}
pub(super) use reconstruction::independently_reconstruct_value_range_fact_at;
pub(crate) fn validate_current_value_range_fact(
    unit: &PsiOptimizationUnit,
    fact: &ValueRangeFact,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(unit)?;
    let expected = reconstruction::reconstruct_value_range_fact(unit, fact)
        .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
    if expected != *fact {
        return Err(OptimizationUnitValidationError::CurrentValueRangeFactMismatch);
    }
    Ok(())
}

pub fn validate_current_value_range_fact_at(
    unit: &PsiOptimizationUnit,
    fact: &ValueRangeFact,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> Result<(), OptimizationUnitValidationError> {
    validate_current_value_range_fact(unit, fact)?;
    availability::validate_current_value_range_fact_at(unit, fact, machine, block, node)
}
