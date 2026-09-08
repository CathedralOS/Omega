//! Exact scalar telescopes and simultaneous edge inputs for ordinary Unit blocks.
use super::{KnownUnitInteger, LiveDefinitions};
use crate::LoweringError;
use abstract_operations::{AbstractBlockEntry, AbstractFunction, AbstractOperation, ValueBinding};
use semantic_vocabulary::{BlockId, ScalarType, ValueId};
use std::collections::BTreeSet;
use target_operations::TargetScalarBlockValue;

pub(super) fn validate_parameters(
    function: &AbstractFunction,
    definitions: &mut BTreeSet<ValueId>,
) -> Result<(), LoweringError> {
    for block in function
        .block_entries
        .iter()
        .filter(|block| block.block != function.entry)
    {
        for parameter in &block.parameters {
            if !matches!(parameter.scalar_type, ScalarType::Boolean)
                && !matches!(parameter.scalar_type, ScalarType::Integer(integer)
                    if crate::lowering::scalar_abi::fixed_native_integer_shape(integer).is_some())
            {
                return Err(LoweringError::ValueTypeMismatch(parameter.value));
            }
            if !definitions.insert(parameter.value) {
                return Err(LoweringError::DuplicateValue(parameter.value));
            }
        }
    }
    Ok(())
}

pub(super) fn enter(block: &AbstractBlockEntry, live: &mut LiveDefinitions) {
    for parameter in &block.parameters {
        match parameter.scalar_type {
            ScalarType::Integer(scalar_type) => {
                live.integers.insert(
                    parameter.value,
                    KnownUnitInteger::BlockParameter {
                        block: block.block,
                        value: parameter.value,
                        scalar_type,
                    },
                );
            }
            ScalarType::Boolean => {
                live.boolean_parameters.insert(
                    parameter.value,
                    TargetScalarBlockValue {
                        block: block.block,
                        value: parameter.value,
                        scalar_type: ScalarType::Boolean,
                    },
                );
            }
            ScalarType::IeeeFloat(_) => {}
        }
    }
}

pub(super) fn validate_successors(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    live: &LiveDefinitions,
) -> Result<(), LoweringError> {
    let validate =
        |target: BlockId,
         bindings: &[ValueBinding],
         structural: &[abstract_operations::AbstractStructuralBinding]| {
            let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
            let block = function
                .block_entries
                .iter()
                .find(|block| block.block == target)
                .ok_or_else(invalid)?;
            if bindings.len() != block.parameters.len() {
                return Err(invalid());
            }
            if structural.len() != block.structural_parameters.len() {
                return Err(invalid());
            }
            for (binding, parameter) in structural.iter().zip(&block.structural_parameters) {
                let place = binding.argument.place;
                let source_type = function
                    .structural_parameters
                    .iter()
                    .find(|source| source.place == place)
                    .map(|source| source.structural_type)
                    .or_else(|| {
                        live.views
                            .get(&place)
                            .map(|(_, structural_type)| *structural_type)
                    })
                    .or_else(|| {
                        live.block_views
                            .contains(&place)
                            .then(|| {
                                function
                                    .block_entries
                                    .iter()
                                    .flat_map(|entry| &entry.structural_parameters)
                                    .find(|source| source.place == place)
                                    .map(|source| source.structural_type)
                            })
                            .flatten()
                    });
                if binding.parameter != parameter.place
                    || source_type != Some(parameter.structural_type)
                    || !binding.argument.path.is_empty()
                    || binding.argument.access != terminal_psi::StructuralAccess::SharedBorrow
                {
                    return Err(invalid());
                }
            }
            // Every source is read from the predecessor environment. Destination
            // declarations are installed only when that block becomes available.
            for (binding, parameter) in bindings.iter().zip(&block.parameters) {
                let scalar_type = live
                    .integers
                    .get(&binding.argument)
                    .map(|known| ScalarType::Integer(known.scalar_type()))
                    .or_else(|| {
                        (live.booleans.contains_key(&binding.argument)
                            || live.boolean_homes.contains_key(&binding.argument)
                            || live.boolean_parameters.contains_key(&binding.argument))
                        .then_some(ScalarType::Boolean)
                    })
                    .or_else(|| {
                        function
                            .parameters
                            .iter()
                            .find(|parameter| parameter.value == binding.argument)
                            .map(|parameter| parameter.scalar_type)
                    });
                if binding.parameter != parameter.value
                    || binding.scalar_type != parameter.scalar_type
                    || scalar_type != Some(parameter.scalar_type)
                {
                    return Err(invalid());
                }
            }
            Ok(())
        };
    match operation {
        AbstractOperation::Jump {
            target,
            bindings,
            structural_bindings,
            ..
        } => validate(*target, bindings, structural_bindings),
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => {
            validate(
                when_true.target,
                &when_true.bindings,
                &when_true.structural_bindings,
            )?;
            validate(
                when_false.target,
                &when_false.bindings,
                &when_false.structural_bindings,
            )
        }
        AbstractOperation::Return { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::StructuralCase { .. } => Ok(()),
        _ => Err(LoweringError::UnsupportedControlFlow(function.machine)),
    }
}
