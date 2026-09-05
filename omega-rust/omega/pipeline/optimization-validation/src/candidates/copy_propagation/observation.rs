//! Closed-region normalization and outside-region comparison.

use super::*;

/// Construct the validator's normalized pre-rewrite question independently of
/// the output constructor below. Only the exact scalar substitution and the
/// one proved incoming binding slot may change.
pub(crate) fn normalize_redundant_parameter_observation_input(
    input: &PsiOptimizationUnit,
    patch: RedundantBlockParameterRewrite,
    affected_blocks: &[BlockId],
) -> Result<PsiOptimizationUnit, OptimizationUnitValidationError> {
    let affected = affected_blocks.iter().copied().collect::<BTreeSet<_>>();
    let mut normalized = input.clone();
    let function = normalized
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let position = usize::try_from(patch.position).expect("u32 fits usize");
    let removed = target
        .parameters
        .get(position)
        .copied()
        .ok_or(OptimizationUnitValidationError::CandidateBlockParameterMismatch)?;
    if removed.value != patch.parameter
        || removed.scalar_type != patch.scalar_type
        || removed.site
            != (ValueDefinitionSite::BlockParameter {
                block: patch.block,
                position: patch.position,
            })
    {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    target.parameters.remove(position);
    for (new_position, parameter) in target.parameters.iter_mut().enumerate().skip(position) {
        parameter.site = ValueDefinitionSite::BlockParameter {
            block: patch.block,
            position: u32::try_from(new_position).expect("parameter index fits u32"),
        };
    }

    for block in function
        .blocks
        .iter_mut()
        .filter(|block| affected.contains(&block.id))
    {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            node.operation =
                normalize_redundant_parameter_observation_operation(&node.operation, patch)?;
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    normalized.identity = recompute_psi_optimization_unit_identity(&normalized);
    Ok(normalized)
}

pub(crate) fn normalize_redundant_parameter_observation_operation(
    operation: &abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) -> Result<abstract_operations::AbstractOperation, OptimizationUnitValidationError> {
    use abstract_operations::AbstractOperation as O;

    let mut normalized = operation.clone();
    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let normalize_bindings = |target: BlockId,
                              bindings: &mut Vec<abstract_operations::ValueBinding>|
     -> Result<(), OptimizationUnitValidationError> {
        for binding in bindings.iter_mut() {
            replace(&mut binding.argument);
        }
        if target == patch.block {
            let position = usize::try_from(patch.position).expect("u32 fits usize");
            let binding = bindings
                .get(position)
                .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
            if binding.parameter != patch.parameter
                || binding.argument != patch.replacement
                || binding.scalar_type != patch.scalar_type
            {
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            bindings.remove(position);
        }
        Ok(())
    };

    match &mut normalized {
        O::WriteOnlyPrimitiveStore { value, .. } | O::StructuralScalarFieldStore { value, .. } => {
            replace(&mut value.value)
        }
        O::Call { arguments, .. }
        | O::CallUnit { arguments, .. }
        | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => {
            replace(left);
            replace(right);
            replace(addend);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
        O::Jump {
            target, bindings, ..
        } => normalize_bindings(*target, bindings)?,
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            normalize_bindings(when_true.target, &mut when_true.bindings)?;
            normalize_bindings(when_false.target, &mut when_false.bindings)?;
        }
        O::StructuralCase { cases, .. } => {
            for successor in cases {
                if successor.target == patch.block {
                    let position = usize::try_from(patch.position).expect("u32 fits usize");
                    let payload = successor
                        .payloads
                        .get(position)
                        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                    if payload.parameter != patch.parameter
                        || payload.scalar_type != patch.scalar_type
                    {
                        return Err(
                            OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                        );
                    }
                    successor.payloads.remove(position);
                }
            }
        }
        O::Return { value, .. } => replace(value),
        O::DynamicDescriptorParameter { .. }
        | O::StoreDynamicDescriptor { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishAffineScalarRecord { .. }
        | O::CallUnitWithDynamicArguments { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructuralScalarWithDynamicArguments { .. }
        | O::CallDynamicScalar { .. }
        | O::CallStoredDynamicScalar { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallDynamicUnit { .. }
        | O::CallDynamicParameterUnit { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
    Ok(normalized)
}

pub(crate) fn unchanged_outside_redundant_parameter_region(
    input: &PsiOptimizationUnit,
    output: &PsiOptimizationUnit,
    machine: MachineId,
    affected_blocks: &[BlockId],
) -> bool {
    let mut expected = input.clone();
    let Some(expected_function) = expected
        .functions
        .iter_mut()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    let Some(output_function) = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    for block_id in affected_blocks {
        let Some(expected_block) = expected_function
            .blocks
            .iter_mut()
            .find(|block| block.id == *block_id)
        else {
            return false;
        };
        let Some(output_block) = output_function
            .blocks
            .iter()
            .find(|block| block.id == *block_id)
        else {
            return false;
        };
        *expected_block = output_block.clone();
    }
    expected.identity = output.identity;
    expected == *output
}
