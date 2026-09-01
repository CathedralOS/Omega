use super::super::shared::*;
use super::fuel::exact_operation_fuel;
use super::immediate::derive_operand;
use super::{DerivedValue, LeafContext};

pub(super) fn derive<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    if context.nodes.len() != 7 {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let TargetIntegerExpression::ExactAdd {
        psi_operation: result_operation,
        obligation: result_obligation,
        left: result_left,
        right: result_right,
    } = expression
    else {
        unreachable!("shape predicate admitted only exact addition")
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: middle_operation,
        obligation: middle_obligation,
        left: middle_left,
        right: middle_right,
    } = result_right.as_ref()
    else {
        unreachable!("shape predicate admitted the middle addition")
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: inner_operation,
        obligation: inner_obligation,
        left: inner_left,
        right: inner_right,
    } = middle_right.as_ref()
    else {
        unreachable!("shape predicate admitted the inner addition")
    };
    let resident = derive_operand(
        context.function,
        context.arm_edge,
        result_left,
        &context.nodes[0],
        context.u64_type,
    )?;
    let second_resident = derive_operand(
        context.function,
        context.arm_edge,
        middle_left,
        &context.nodes[0],
        context.u64_type,
    )?;
    if resident != second_resident {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let left = derive_operand(
        context.function,
        context.arm_edge,
        inner_left,
        &context.nodes[1],
        context.u64_type,
    )?;
    let right = derive_operand(
        context.function,
        context.arm_edge,
        inner_right,
        &context.nodes[2],
        context.u64_type,
    )?;
    let inner = derive_exact_add(
        context,
        &context.nodes[3],
        *inner_operation,
        *inner_obligation,
        left.source_value,
        right.source_value,
    )?;
    let middle = derive_exact_add(
        context,
        &context.nodes[4],
        *middle_operation,
        *middle_obligation,
        resident.source_value,
        inner.source_value,
    )?;
    let result = derive_exact_add(
        context,
        &context.nodes[5],
        *result_operation,
        *result_obligation,
        resident.source_value,
        middle.source_value,
    )?;
    if result.source_value != context.source_value {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    Ok((
        &context.nodes[6],
        SourceLeafValue::ActiveResidentExactAddChain(Box::new(SourceActiveResidentExactAddChain {
            resident,
            left,
            right,
            inner,
            middle,
            result,
        })),
    ))
}

pub(in crate::legalization::source) fn is_active_resident_exact_add_chain(
    expression: &TargetIntegerExpression,
) -> bool {
    let TargetIntegerExpression::ExactAdd {
        left: result_left,
        right: result_right,
        ..
    } = expression
    else {
        return false;
    };
    let TargetIntegerExpression::Immediate {
        source_value: result_resident,
        ..
    } = result_left.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::ExactAdd {
        left: middle_left,
        right: middle_right,
        ..
    } = result_right.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::Immediate {
        source_value: middle_resident,
        ..
    } = middle_left.as_ref()
    else {
        return false;
    };
    matches!(
        middle_right.as_ref(),
        TargetIntegerExpression::ExactAdd { left, right, .. }
            if matches!(left.as_ref(), TargetIntegerExpression::Immediate { .. })
                && matches!(right.as_ref(), TargetIntegerExpression::Immediate { .. })
                && result_resident == middle_resident
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_exact_add(
    context: &LeafContext<'_>,
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
) -> Result<SourceExactAdd, LegalizationError> {
    let AbstractOperation::ExactIntegerAdd {
        psi_operation,
        obligation: abstract_obligation,
        result,
        scalar_type: abstract_type,
        left: abstract_left,
        right: abstract_right,
    } = &node.operation
    else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    if *psi_operation != operation
        || *abstract_obligation != obligation
        || *abstract_type != context.u64_integer_type
        || *abstract_left != left
        || *abstract_right != right
        || node.definitions.len() != 1
        || node.definitions[0].value != *result
        || node.provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let Some(accepted_fact) = context.accepted_obligation_facts.iter().find(|fact| {
        fact.machine == context.optimized.machine
            && fact.operation == operation
            && fact.obligation == obligation
    }) else {
        return Err(Error::SourceCustodyMismatch);
    };
    if !context.optimized.facts.iter().any(|fact| {
        matches!(fact, OptimizationFact::OperationObligationReference { obligation: referenced_obligation, support } if *referenced_obligation == obligation && *support == operation)
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(SourceExactAdd {
        source_value: *result,
        obligation,
        accepted_fact: accepted_fact.identity,
        operation,
        definition_site: node.definitions[0].site,
        fuel: exact_operation_fuel(node, operation, context.function)?,
    })
}
