//! Exhaustive dispatch from a legalization recipe to its replayed value.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_leaf_value<'a>(
    function: usize,
    recipe: LegalizationRecipe,
    arm_edge: EdgeId,
    expression: &TargetIntegerExpression,
    source_value: &psi_core::ValueId,
    nodes: &'a [omega_optimization_unit::OptimizationNode],
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &LegalizedLeaf,
    architecture: omega_target::Architecture,
    temporary_base: u32,
    u64_type: ScalarType,
) -> Result<
    (
        &'a omega_optimization_unit::OptimizationNode,
        Vec<OperationId>,
    ),
    LegalizationError,
> {
    Ok(match (recipe, expression, &proposed.value) {
        (
            LegalizationRecipe::ReturnU64ImmediateConditionalV1
            | LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1
            | LegalizationRecipe::ReturnU64ActiveResidentExactAddBridgeChainConditionalV1
            | LegalizationRecipe::ReturnU64ActiveResidentExactAddOriginalVictimChainConditionalV1,
            TargetIntegerExpression::Immediate {
                source_value: expression_source,
                value: target_value,
            },
            LegalizedLeafValue::Immediate {
                value,
                constant_operation,
                definition_site,
                constant_fuel,
            },
        ) => {
            if nodes.len() != 2 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let operation = replay_constant(
                function,
                arm_edge,
                *source_value,
                *target_value,
                &nodes[0],
                *constant_operation,
                *definition_site,
                constant_fuel,
                u64_type,
            )?;
            if *value != *target_value {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            (&nodes[1], vec![operation])
        }
        (
            LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1,
            expression,
            LegalizedLeafValue::ActiveResidentExactAddChain(chain),
        ) if replay_active_resident_chain_shape(expression) => {
            if nodes.len() != 7 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let TargetIntegerExpression::ExactAdd {
                psi_operation: result_operation,
                obligation: result_obligation,
                left: result_left,
                right: result_right,
            } = expression
            else {
                unreachable!("shape replay admitted exact add")
            };
            let TargetIntegerExpression::ExactAdd {
                psi_operation: middle_operation,
                obligation: middle_obligation,
                left: middle_left,
                right: middle_right,
            } = result_right.as_ref()
            else {
                unreachable!("shape replay admitted middle exact add")
            };
            let TargetIntegerExpression::ExactAdd {
                psi_operation: inner_operation,
                obligation: inner_obligation,
                left: inner_left,
                right: inner_right,
            } = middle_right.as_ref()
            else {
                unreachable!("shape replay admitted inner exact add")
            };
            let resident = &chain.resident;
            let left = &chain.left;
            let right = &chain.right;
            let inner = &chain.inner;
            let middle = &chain.middle;
            let result = &chain.result;
            replay_immediate(
                function,
                arm_edge,
                result_left,
                &nodes[0],
                resident,
                u64_type,
            )?;
            replay_immediate(
                function,
                arm_edge,
                middle_left,
                &nodes[0],
                resident,
                u64_type,
            )?;
            replay_immediate(function, arm_edge, inner_left, &nodes[1], left, u64_type)?;
            replay_immediate(function, arm_edge, inner_right, &nodes[2], right, u64_type)?;
            replay_exact_add_node(
                function,
                optimized,
                accepted_facts,
                &nodes[3],
                *inner_operation,
                *inner_obligation,
                left.source_value,
                right.source_value,
                inner,
            )?;
            replay_exact_add_node(
                function,
                optimized,
                accepted_facts,
                &nodes[4],
                *middle_operation,
                *middle_obligation,
                resident.source_value,
                inner.source_value,
                middle,
            )?;
            replay_exact_add_node(
                function,
                optimized,
                accepted_facts,
                &nodes[5],
                *result_operation,
                *result_obligation,
                resident.source_value,
                middle.source_value,
                result,
            )?;
            if result.source_value != *source_value {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            (
                &nodes[6],
                vec![
                    resident.constant_operation,
                    left.constant_operation,
                    right.constant_operation,
                    inner.operation,
                    middle.operation,
                    result.operation,
                ],
            )
        }
        (
            LegalizationRecipe::ReturnU64ActiveResidentExactAddBridgeChainConditionalV1,
            expression,
            LegalizedLeafValue::ActiveResidentExactAddBridgeChain(chain),
        ) if replay_active_resident_bridge_chain_shape(expression) => {
            replay_active_resident_bridge_chain(
                function,
                arm_edge,
                expression,
                source_value,
                nodes,
                optimized,
                accepted_facts,
                chain,
                u64_type,
            )?
        }
        (
            LegalizationRecipe::ReturnU64ActiveResidentExactAddOriginalVictimChainConditionalV1,
            expression,
            LegalizedLeafValue::ActiveResidentExactAddOriginalVictimChain(chain),
        ) if replay_active_resident_original_victim_chain_shape(expression) => {
            replay_active_resident_original_victim_chain(
                function,
                arm_edge,
                expression,
                source_value,
                nodes,
                optimized,
                accepted_facts,
                chain,
                u64_type,
            )?
        }
        (
            LegalizationRecipe::ReturnU64EntryParameterConditionalV1,
            TargetIntegerExpression::Parameter {
                source_value: expression_source,
                parameter_index,
                location: ScalarParameterLocation::Register(register),
            },
            LegalizedLeafValue::EntryParameter {
                parameter_index: proposed_index,
                register: proposed_register,
                definition_site,
            },
        ) => {
            if nodes.len() != 1
                || source_value != expression_source
                || register.architecture() != architecture
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let Some(parameter) = optimized.parameters.get(*parameter_index) else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(abstract_parameter) = abstracted.parameters.get(*parameter_index) else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if parameter.value != *source_value
                || parameter.scalar_type != u64_type
                || abstract_parameter.value != *source_value
                || abstract_parameter.scalar_type != u64_type
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            if proposed_index != parameter_index
                || proposed_register != register
                || *definition_site != parameter.site
            {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            (&nodes[0], Vec::new())
        }
        (
            LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1,
            TargetIntegerExpression::ExactAdd {
                psi_operation,
                obligation,
                left,
                right,
            },
            LegalizedLeafValue::ExactAdd {
                obligation: proposed_obligation,
                accepted_fact,
                add_operation,
                definition_site,
                add_fuel,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            replay_exact_binary(
                function,
                arm_edge,
                true,
                *source_value,
                *psi_operation,
                *obligation,
                left,
                right,
                nodes,
                optimized,
                accepted_facts,
                *proposed_obligation,
                *accepted_fact,
                *add_operation,
                *definition_site,
                add_fuel,
                proposed_left,
                proposed_right,
                u64_type,
            )?;
            (
                &nodes[3],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *add_operation,
                ],
            )
        }
        (
            LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1,
            TargetIntegerExpression::IntegerWiden {
                psi_operation: widen_operation,
                source_type,
                operand,
            },
            LegalizedLeafValue::WidenedExactAdd {
                source_type: proposed_source_type,
                target_type: proposed_target_type,
                theorem,
                obligation: proposed_obligation,
                accepted_fact,
                add_operation: proposed_add_operation,
                narrow_result,
                add_definition_site,
                add_fuel,
                widen_operation: proposed_widen_operation,
                widen_definition_site,
                widen_fuel,
                left_temporary,
                right_temporary,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            let TargetIntegerExpression::ExactAdd {
                psi_operation: add_operation,
                obligation,
                left,
                right,
            } = operand.as_ref()
            else {
                return Err(Error::NonCanonicalLegalizedPlan);
            };
            let target_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
            replay_widened_exact_binary(
                function,
                arm_edge,
                true,
                *source_value,
                *source_type,
                target_type,
                *add_operation,
                *obligation,
                left,
                right,
                *widen_operation,
                nodes,
                optimized,
                accepted_facts,
                *proposed_source_type,
                *proposed_target_type,
                *theorem,
                *proposed_obligation,
                *accepted_fact,
                *proposed_add_operation,
                *narrow_result,
                *add_definition_site,
                add_fuel,
                *proposed_widen_operation,
                *widen_definition_site,
                widen_fuel,
                *left_temporary,
                *right_temporary,
                proposed_left,
                proposed_right,
                temporary_base,
            )?;
            (
                &nodes[4],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *proposed_add_operation,
                    *proposed_widen_operation,
                ],
            )
        }
        (
            LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1,
            TargetIntegerExpression::IntegerWiden {
                psi_operation: widen_operation,
                source_type,
                operand,
            },
            LegalizedLeafValue::WidenedExactSubtract {
                source_type: proposed_source_type,
                target_type: proposed_target_type,
                theorem,
                obligation: proposed_obligation,
                accepted_fact,
                subtract_operation: proposed_subtract_operation,
                narrow_result,
                subtract_definition_site,
                subtract_fuel,
                widen_operation: proposed_widen_operation,
                widen_definition_site,
                widen_fuel,
                left_temporary,
                right_temporary,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            let TargetIntegerExpression::ExactSubtract {
                psi_operation: subtract_operation,
                obligation,
                left,
                right,
            } = operand.as_ref()
            else {
                return Err(Error::NonCanonicalLegalizedPlan);
            };
            let target_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
            replay_widened_exact_binary(
                function,
                arm_edge,
                false,
                *source_value,
                *source_type,
                target_type,
                *subtract_operation,
                *obligation,
                left,
                right,
                *widen_operation,
                nodes,
                optimized,
                accepted_facts,
                *proposed_source_type,
                *proposed_target_type,
                *theorem,
                *proposed_obligation,
                *accepted_fact,
                *proposed_subtract_operation,
                *narrow_result,
                *subtract_definition_site,
                subtract_fuel,
                *proposed_widen_operation,
                *widen_definition_site,
                widen_fuel,
                *left_temporary,
                *right_temporary,
                proposed_left,
                proposed_right,
                temporary_base,
            )?;
            (
                &nodes[4],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *proposed_subtract_operation,
                    *proposed_widen_operation,
                ],
            )
        }
        (
            LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1,
            TargetIntegerExpression::ExactSubtract {
                psi_operation,
                obligation,
                left,
                right,
            },
            LegalizedLeafValue::ExactSubtract {
                obligation: proposed_obligation,
                accepted_fact,
                subtract_operation,
                definition_site,
                subtract_fuel,
                left: proposed_left,
                right: proposed_right,
            },
        ) => {
            replay_exact_binary(
                function,
                arm_edge,
                false,
                *source_value,
                *psi_operation,
                *obligation,
                left,
                right,
                nodes,
                optimized,
                accepted_facts,
                *proposed_obligation,
                *accepted_fact,
                *subtract_operation,
                *definition_site,
                subtract_fuel,
                proposed_left,
                proposed_right,
                u64_type,
            )?;
            (
                &nodes[3],
                vec![
                    proposed_left.constant_operation,
                    proposed_right.constant_operation,
                    *subtract_operation,
                ],
            )
        }
        _ => return Err(Error::NonCanonicalLegalizedPlan),
    })
}
