//! Exact-arithmetic shape, proof, and decomposition replay.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_widened_exact_binary(
    function: usize,
    arm_edge: EdgeId,
    add: bool,
    final_value: semantic_vocabulary::ValueId,
    source_type: semantic_vocabulary::IntegerType,
    target_type: semantic_vocabulary::IntegerType,
    operation: OperationId,
    obligation: semantic_vocabulary::ObligationId,
    target_left: &TargetIntegerExpression,
    target_right: &TargetIntegerExpression,
    widen_operation: OperationId,
    nodes: &[optimization_unit::OptimizationNode],
    optimized: &optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[optimization_unit::AcceptedObligationFact],
    proposed_source_type: semantic_vocabulary::IntegerType,
    proposed_target_type: semantic_vocabulary::IntegerType,
    theorem: LegalizationTheorem,
    proposed_obligation: semantic_vocabulary::ObligationId,
    proposed_fact: optimization_core::AcceptedObligationFactIdentity,
    proposed_operation: OperationId,
    proposed_narrow_result: semantic_vocabulary::ValueId,
    proposed_operation_site: optimization_unit::ValueDefinitionSite,
    proposed_operation_fuel: &[optimization_unit::FuelSettlement],
    proposed_widen_operation: OperationId,
    proposed_widen_site: optimization_unit::ValueDefinitionSite,
    proposed_widen_fuel: &[optimization_unit::FuelSettlement],
    left_temporary: LegalizedTemporaryId,
    right_temporary: LegalizedTemporaryId,
    proposed_left: &LegalizedImmediate,
    proposed_right: &LegalizedImmediate,
    temporary_base: u32,
) -> Result<(), LegalizationError> {
    let u8_type = semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u64_type = semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if source_type != u8_type || target_type != u64_type || nodes.len() != 5 {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_theorem = if add {
        LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1
    } else {
        LegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1
    };
    if proposed_source_type != source_type
        || proposed_target_type != target_type
        || theorem != expected_theorem
        || left_temporary != LegalizedTemporaryId(temporary_base)
        || right_temporary != LegalizedTemporaryId(temporary_base + 1)
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let narrow_scalar = ScalarType::Integer(source_type);
    replay_immediate(
        function,
        arm_edge,
        target_left,
        &nodes[0],
        proposed_left,
        narrow_scalar,
    )?;
    replay_immediate(
        function,
        arm_edge,
        target_right,
        &nodes[1],
        proposed_right,
        narrow_scalar,
    )?;

    let (abstract_operation, abstract_obligation, narrow_result, abstract_source_type, left, right) =
        match (&nodes[2].operation, add) {
            (
                AbstractOperation::ExactIntegerAdd {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                true,
            )
            | (
                AbstractOperation::ExactIntegerSubtract {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                false,
            ) => (psi_operation, obligation, result, scalar_type, left, right),
            _ => return Err(Error::UnsupportedSourceShape { function }),
        };
    if *abstract_operation != operation
        || *abstract_obligation != obligation
        || *abstract_source_type != source_type
        || *left != proposed_left.source_value
        || *right != proposed_right.source_value
        || nodes[2].definitions.len() != 1
        || nodes[2].definitions[0].value != *narrow_result
        || nodes[2].definitions[0].scalar_type != narrow_scalar
        || nodes[2].provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(fact) = accepted_facts.iter().find(|fact| {
        fact.machine == optimized.machine
            && fact.operation == operation
            && fact.obligation == obligation
    }) else {
        return Err(Error::SourceCustodyMismatch);
    };
    if !optimized.facts.iter().any(|fact| {
        matches!(
            fact,
            OptimizationFact::OperationObligationReference {
                obligation: referenced,
                support,
            } if *referenced == obligation && *support == operation
        )
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed_obligation != obligation
        || proposed_fact != fact.identity
        || proposed_operation != operation
        || proposed_narrow_result != *narrow_result
        || proposed_operation_site != nodes[2].definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &nodes[2].fuel, proposed_operation_fuel)?;

    let AbstractOperation::IntegerWiden {
        psi_operation: abstract_widen,
        result: widen_result,
        source_type: abstract_widen_source,
        target_type: abstract_widen_target,
        operand,
    } = &nodes[3].operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *abstract_widen != widen_operation
        || *widen_result != final_value
        || *narrow_result == final_value
        || *abstract_widen_source != source_type
        || *abstract_widen_target != target_type
        || *operand != *narrow_result
        || nodes[3].definitions.len() != 1
        || nodes[3].definitions[0].value != final_value
        || nodes[3].definitions[0].scalar_type != ScalarType::Integer(target_type)
        || nodes[3].provenance != vec![PsiProvenance::Operation(widen_operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    if proposed_widen_operation != widen_operation
        || proposed_widen_site != nodes[3].definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(
        function,
        widen_operation,
        &nodes[3].fuel,
        proposed_widen_fuel,
    )?;

    let narrow_result = if add {
        source_type.exact_add(proposed_left.value, proposed_right.value)
    } else {
        source_type.exact_sub(proposed_left.value, proposed_right.value)
    };
    let Some(narrow_result) = narrow_result else {
        return Err(Error::SourceCustodyMismatch);
    };
    let Some(widened_narrow_result) = source_type.widen_value_to(target_type, narrow_result) else {
        return Err(Error::SourceCustodyMismatch);
    };
    let Some(widened_left) = source_type.widen_value_to(target_type, proposed_left.value) else {
        return Err(Error::SourceCustodyMismatch);
    };
    let Some(widened_right) = source_type.widen_value_to(target_type, proposed_right.value) else {
        return Err(Error::SourceCustodyMismatch);
    };
    let widened_result = if add {
        target_type.exact_add(widened_left, widened_right)
    } else {
        target_type.exact_sub(widened_left, widened_right)
    };
    if widened_result != Some(widened_narrow_result) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_exact_binary(
    function: usize,
    arm_edge: EdgeId,
    add: bool,
    result_value: semantic_vocabulary::ValueId,
    operation: OperationId,
    obligation: semantic_vocabulary::ObligationId,
    target_left: &TargetIntegerExpression,
    target_right: &TargetIntegerExpression,
    nodes: &[optimization_unit::OptimizationNode],
    optimized: &optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[optimization_unit::AcceptedObligationFact],
    proposed_obligation: semantic_vocabulary::ObligationId,
    proposed_fact: optimization_core::AcceptedObligationFactIdentity,
    proposed_operation: OperationId,
    proposed_site: optimization_unit::ValueDefinitionSite,
    proposed_fuel: &[optimization_unit::FuelSettlement],
    proposed_left: &LegalizedImmediate,
    proposed_right: &LegalizedImmediate,
    u64_type: ScalarType,
) -> Result<(), LegalizationError> {
    if nodes.len() != 4 {
        return Err(Error::UnsupportedSourceShape { function });
    }
    replay_immediate(
        function,
        arm_edge,
        target_left,
        &nodes[0],
        proposed_left,
        u64_type,
    )?;
    replay_immediate(
        function,
        arm_edge,
        target_right,
        &nodes[1],
        proposed_right,
        u64_type,
    )?;
    let (abstract_operation, abstract_obligation, result, scalar_type, left, right) =
        match (&nodes[2].operation, add) {
            (
                AbstractOperation::ExactIntegerAdd {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                true,
            )
            | (
                AbstractOperation::ExactIntegerSubtract {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                },
                false,
            ) => (psi_operation, obligation, result, scalar_type, left, right),
            _ => return Err(Error::UnsupportedSourceShape { function }),
        };
    let u64_integer =
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *abstract_operation != operation
        || *abstract_obligation != obligation
        || *result != result_value
        || *scalar_type != u64_integer
        || *left != proposed_left.source_value
        || *right != proposed_right.source_value
        || nodes[2].definitions.len() != 1
        || nodes[2].definitions[0].value != result_value
        || nodes[2].provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(fact) = accepted_facts.iter().find(|fact| {
        fact.machine == optimized.machine
            && fact.operation == operation
            && fact.obligation == obligation
    }) else {
        return Err(Error::SourceCustodyMismatch);
    };
    if !optimized.facts.iter().any(|fact| {
        matches!(
            fact,
            OptimizationFact::OperationObligationReference {
                obligation: referenced,
                support,
            } if *referenced == obligation && *support == operation
        )
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed_obligation != obligation
        || proposed_fact != fact.identity
        || proposed_operation != operation
        || proposed_site != nodes[2].definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &nodes[2].fuel, proposed_fuel)
}
