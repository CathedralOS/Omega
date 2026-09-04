use super::super::shared::*;
use super::fuel::exact_operation_fuel;
use super::immediate::derive_operand;
use super::{DerivedValue, LeafContext};

pub(super) fn derive_add<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    let TargetIntegerExpression::ExactAdd {
        psi_operation,
        obligation,
        left,
        right,
    } = expression
    else {
        unreachable!("exact-add catalog arm supplied the add derivation")
    };
    if context.nodes.len() != 4 {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let left = derive_operand(
        context.function,
        context.arm_edge,
        left,
        &context.nodes[0],
        context.u64_type,
    )?;
    let right = derive_operand(
        context.function,
        context.arm_edge,
        right,
        &context.nodes[1],
        context.u64_type,
    )?;
    let AbstractOperation::ExactIntegerAdd {
        psi_operation: abstract_operation,
        obligation: abstract_obligation,
        result,
        scalar_type,
        left: abstract_left,
        right: abstract_right,
    } = &context.nodes[2].operation
    else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    if abstract_operation != psi_operation
        || abstract_obligation != obligation
        || *result != context.source_value
        || *scalar_type != context.u64_integer_type
        || *abstract_left != left.source_value
        || *abstract_right != right.source_value
        || context.nodes[2].definitions.len() != 1
        || context.nodes[2].definitions[0].value != context.source_value
        || context.nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let add_fuel = exact_operation_fuel(&context.nodes[2], *psi_operation, context.function)?;
    let accepted_fact = accepted_fact(context, *psi_operation, *obligation)?;
    Ok((
        &context.nodes[3],
        SourceLeafValue::ExactAdd {
            obligation: *obligation,
            accepted_fact,
            add_operation: *psi_operation,
            definition_site: context.nodes[2].definitions[0].site,
            add_fuel,
            left,
            right,
        },
    ))
}

pub(super) fn derive_subtract<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    let TargetIntegerExpression::ExactSubtract {
        psi_operation,
        obligation,
        left,
        right,
    } = expression
    else {
        unreachable!("exact-subtract catalog arm supplied the subtract derivation")
    };
    if context.nodes.len() != 4 {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let left = derive_operand(
        context.function,
        context.arm_edge,
        left,
        &context.nodes[0],
        context.u64_type,
    )?;
    let right = derive_operand(
        context.function,
        context.arm_edge,
        right,
        &context.nodes[1],
        context.u64_type,
    )?;
    let AbstractOperation::ExactIntegerSubtract {
        psi_operation: abstract_operation,
        obligation: abstract_obligation,
        result,
        scalar_type,
        left: abstract_left,
        right: abstract_right,
    } = &context.nodes[2].operation
    else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    if abstract_operation != psi_operation
        || abstract_obligation != obligation
        || *result != context.source_value
        || *scalar_type != context.u64_integer_type
        || *abstract_left != left.source_value
        || *abstract_right != right.source_value
        || context.nodes[2].definitions.len() != 1
        || context.nodes[2].definitions[0].value != context.source_value
        || context.nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let subtract_fuel = exact_operation_fuel(&context.nodes[2], *psi_operation, context.function)?;
    let accepted_fact = accepted_fact(context, *psi_operation, *obligation)?;
    Ok((
        &context.nodes[3],
        SourceLeafValue::ExactSubtract {
            obligation: *obligation,
            accepted_fact,
            subtract_operation: *psi_operation,
            definition_site: context.nodes[2].definitions[0].site,
            subtract_fuel,
            left,
            right,
        },
    ))
}

fn accepted_fact(
    context: &LeafContext<'_>,
    operation: OperationId,
    obligation: psi_core::ObligationId,
) -> Result<omega_optimization_core::AcceptedObligationFactIdentity, LegalizationError> {
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
    Ok(accepted_fact.identity)
}
