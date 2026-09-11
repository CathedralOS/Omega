//! Select block-owned graph lowering from represented operations and custody.
use super::*;

pub(in crate::lowering) fn requires_graph(
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<bool, LoweringError> {
    if ordinary_scalar_graph(function) {
        return Ok(true);
    }
    if (function.result.scalar().is_some()
        && function
            .parameters
            .iter()
            .any(|parameter| matches!(parameter.scalar_type, ScalarType::IeeeFloat(_))))
        || function
            .result
            .scalar()
            .is_some_and(|result| matches!(result.scalar_type, ScalarType::IeeeFloat(_)))
        || function
            .operations
            .iter()
            .any(|operation| matches!(operation, AbstractOperation::IeeeFloatCompare { .. }))
    {
        return Ok(true);
    }
    if function
        .structural_parameters
        .iter()
        .any(|parameter| super::scalar_arrays::is_owned_parameter(parameter, types))
    {
        return Ok(true);
    }
    if function.result.structural().is_some_and(|result| {
        result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
    }) {
        return Ok(true);
    }
    if function.operations.iter().any(|operation| {
        matches!(
            operation,
            AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishScalarCase { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::PrimitiveScalarRead { .. }
        )
    }) {
        return Ok(true);
    }
    if function.operations.iter().any(|operation| {
        matches!(operation, AbstractOperation::CallStructuralScalar { structural_arguments, .. }
            if structural_arguments.is_empty())
    }) {
        return Ok(true);
    }
    if function.operations.iter().any(|operation| {
        matches!(operation, AbstractOperation::CallStructural { result, .. }
            if matches!(types.get(&result.structural_type).map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Sum { .. } | StructuralTypeShape::FixedArray { .. })))
    }) {
        return Ok(true);
    }
    Ok(function.result.scalar().is_some()
        && (has_cycle(function)?
            || crate::lowering::unobserved_owned::has_block_arrivals(function, types)
            || function.operations.iter().any(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })))
}

// Scalar operations compose through their source blocks and once-only homes.
// Specialized descriptor, cleanup and boundary custody still has separate owners.
// The legacy flat API has no block entries and retains its expression projection;
// current source production supplies blocks and never uses that compatibility form.
fn ordinary_scalar_graph(function: &AbstractFunction) -> bool {
    function.result.scalar().is_some()
        && !function.block_entries.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function
            .block_entries
            .iter()
            .all(|entry| entry.structural_parameters.is_empty())
        && function.operations.iter().all(|operation| match operation {
            AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::IeeeFloatConstant { .. }
            | AbstractOperation::IeeeFloatCompare { .. }
            | AbstractOperation::IntegerWiden { .. }
            | AbstractOperation::IntegerExactCast { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::BooleanNot { .. }
            | AbstractOperation::BooleanEqual { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::Call { .. }
            | AbstractOperation::WrappingIntegerAdd { .. }
            | AbstractOperation::SaturatingIntegerAdd { .. }
            | AbstractOperation::WrappingIntegerSubtract { .. }
            | AbstractOperation::SaturatingIntegerSubtract { .. }
            | AbstractOperation::WrappingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::SaturatingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. }
            | AbstractOperation::ExactIntegerRemainder { .. }
            | AbstractOperation::WrappingIntegerDivide { .. }
            | AbstractOperation::WrappingIntegerRemainder { .. }
            | AbstractOperation::SaturatingIntegerDivide { .. }
            | AbstractOperation::SaturatingIntegerRemainder { .. }
            | AbstractOperation::IntegerBitwiseAnd { .. }
            | AbstractOperation::IntegerBitwiseOr { .. }
            | AbstractOperation::IntegerBitwiseXor { .. }
            | AbstractOperation::IntegerBitwiseNot { .. }
            | AbstractOperation::WrappingIntegerShiftLeft { .. }
            | AbstractOperation::WrappingIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerShiftLeft { .. }
            | AbstractOperation::ExactIntegerShiftRight { .. } => true,
            AbstractOperation::Return {
                cleanup_actions, ..
            } => cleanup_actions.is_empty(),
            AbstractOperation::Jump {
                structural_bindings,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                structural_bindings.is_empty()
                    && trivial_affine_discards.is_empty()
                    && residual_affine_discards.is_empty()
            }
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                when_true.structural_bindings.is_empty()
                    && when_false.structural_bindings.is_empty()
            }
            _ => false,
        })
}

fn has_cycle(function: &AbstractFunction) -> Result<bool, LoweringError> {
    let invalid = || LoweringError::ConditionalControlFlowRequiresBlockLowering(function.machine);
    let mut outgoing = BTreeMap::new();
    let mut incoming = BTreeMap::new();
    for (position, block) in function.block_entries.iter().enumerate() {
        if incoming.insert(block.block, 0_usize).is_some() {
            return Err(invalid());
        }
        let end = function
            .block_entries
            .get(position + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        let terminator = function
            .operations
            .get(block.operation_offset..end)
            .and_then(|operations| operations.last())
            .ok_or_else(invalid)?;
        let successors = match terminator {
            AbstractOperation::Jump { target, .. } => vec![*target],
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            AbstractOperation::Return { .. }
            | AbstractOperation::ReturnUnit { .. }
            | AbstractOperation::ReturnStructural { .. }
            | AbstractOperation::Crash { .. } => Vec::new(),
            _ => return Err(invalid()),
        };
        outgoing.insert(block.block, successors);
    }
    for successor in outgoing.values().flatten() {
        let count = incoming.get_mut(successor).ok_or_else(invalid)?;
        *count = count.checked_add(1).ok_or_else(invalid)?;
    }
    let mut pending = incoming
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<Vec<_>>();
    let mut visited = 0;
    while let Some(block) = pending.pop() {
        visited += 1;
        for successor in outgoing.get(&block).ok_or_else(invalid)? {
            let count = incoming.get_mut(successor).ok_or_else(invalid)?;
            *count = count.checked_sub(1).ok_or_else(invalid)?;
            if *count == 0 {
                pending.push(*successor);
            }
        }
    }
    Ok(visited != function.block_entries.len())
}
