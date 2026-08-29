use super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_leaf(
    function: usize,
    recipe: LegalizationRecipe,
    arm_edge: EdgeId,
    target: &TargetIntegerControl,
    abstract_operations: &[AbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &LegalizedLeaf,
    architecture: omega_target::Architecture,
    temporary_base: u32,
) -> Result<Vec<OperationId>, LegalizationError> {
    if nodes.len() != abstract_operations.len()
        || nodes
            .iter()
            .zip(abstract_operations)
            .any(|(node, operation)| node.operation != *operation)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TargetIntegerControl::Return {
        psi_return_edge,
        source_value,
        expression,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if proposed.return_edge != *psi_return_edge || proposed.source_value != *source_value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let u64_integer = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer);
    let (return_node, operations) = match (recipe, expression, &proposed.value) {
        (
            LegalizationRecipe::ReturnU64ImmediateConditionalV1
            | LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1,
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
    };

    let AbstractOperation::Return {
        psi_edge,
        value,
        scalar_type,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_edge != *psi_return_edge
        || *value != *source_value
        || *scalar_type != u64_type
        || !cleanup_actions.is_empty()
        || return_node.provenance != vec![PsiProvenance::Edge(*psi_return_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    replay_edge_fuel(
        function,
        *psi_return_edge,
        &return_node.fuel,
        &proposed.return_fuel,
    )?;
    Ok(operations)
}

pub(super) fn replay_active_resident_chain_shape(expression: &TargetIntegerExpression) -> bool {
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
fn replay_exact_add_node(
    function: usize,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
    proposed: &omega_legalized_operations::LegalizedExactAdd,
) -> Result<(), LegalizationError> {
    let AbstractOperation::ExactIntegerAdd {
        psi_operation,
        obligation: abstract_obligation,
        result,
        scalar_type,
        left: abstract_left,
        right: abstract_right,
    } = &node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let u64_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    if *psi_operation != operation
        || *abstract_obligation != obligation
        || *scalar_type != u64_type
        || *abstract_left != left
        || *abstract_right != right
        || node.definitions.len() != 1
        || node.definitions[0].value != *result
        || node.provenance != vec![PsiProvenance::Operation(operation)]
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
    if proposed.source_value != *result
        || proposed.obligation != obligation
        || proposed.accepted_fact != fact.identity
        || proposed.operation != operation
        || proposed.definition_site != node.definitions[0].site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &node.fuel, &proposed.fuel)
}

#[allow(clippy::too_many_arguments)]
fn replay_widened_exact_binary(
    function: usize,
    arm_edge: EdgeId,
    add: bool,
    final_value: psi_core::ValueId,
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    target_left: &TargetIntegerExpression,
    target_right: &TargetIntegerExpression,
    widen_operation: OperationId,
    nodes: &[omega_optimization_unit::OptimizationNode],
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed_source_type: psi_core::IntegerType,
    proposed_target_type: psi_core::IntegerType,
    theorem: LegalizationTheorem,
    proposed_obligation: psi_core::ObligationId,
    proposed_fact: omega_optimization_core::AcceptedObligationFactIdentity,
    proposed_operation: OperationId,
    proposed_narrow_result: psi_core::ValueId,
    proposed_operation_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_operation_fuel: &[omega_optimization_unit::FuelSettlement],
    proposed_widen_operation: OperationId,
    proposed_widen_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_widen_fuel: &[omega_optimization_unit::FuelSettlement],
    left_temporary: LegalizedTemporaryId,
    right_temporary: LegalizedTemporaryId,
    proposed_left: &LegalizedImmediate,
    proposed_right: &LegalizedImmediate,
    temporary_base: u32,
) -> Result<(), LegalizationError> {
    let u8_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u64_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
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
fn replay_exact_binary(
    function: usize,
    arm_edge: EdgeId,
    add: bool,
    result_value: psi_core::ValueId,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    target_left: &TargetIntegerExpression,
    target_right: &TargetIntegerExpression,
    nodes: &[omega_optimization_unit::OptimizationNode],
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed_obligation: psi_core::ObligationId,
    proposed_fact: omega_optimization_core::AcceptedObligationFactIdentity,
    proposed_operation: OperationId,
    proposed_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_fuel: &[omega_optimization_unit::FuelSettlement],
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
    let u64_integer = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
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

fn replay_immediate(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    proposed: &LegalizedImmediate,
    expected_type: ScalarType,
) -> Result<(), LegalizationError> {
    let TargetIntegerExpression::Immediate {
        source_value,
        value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    replay_constant(
        function,
        arm_edge,
        *source_value,
        *value,
        node,
        proposed.constant_operation,
        proposed.definition_site,
        &proposed.fuel,
        expected_type,
    )?;
    if proposed.source_value != *source_value || proposed.value != *value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_constant(
    function: usize,
    arm_edge: EdgeId,
    source_value: psi_core::ValueId,
    target_value: psi_core::IntegerValue,
    node: &omega_optimization_unit::OptimizationNode,
    proposed_operation: OperationId,
    proposed_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_fuel: &[omega_optimization_unit::FuelSettlement],
    expected_type: ScalarType,
) -> Result<OperationId, LegalizationError> {
    let AbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &node.operation
    else {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    };
    if *result != source_value
        || *value != target_value
        || *scalar_type != expected_type
        || node.definitions.len() != 1
        || node.definitions[0].value != source_value
        || node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    }
    if proposed_operation != *psi_operation || proposed_site != node.definitions[0].site {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, *psi_operation, &node.fuel, proposed_fuel)?;
    Ok(*psi_operation)
}

fn replay_operation_fuel(
    function: usize,
    operation: OperationId,
    source: &[omega_optimization_unit::FuelSettlement],
    proposed: &[omega_optimization_unit::FuelSettlement],
) -> Result<(), LegalizationError> {
    if source.is_empty()
        || source
            .iter()
            .any(|settlement| settlement.site != PsiProvenance::Operation(operation))
    {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed != source {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

pub(super) fn replay_edge_fuel(
    function: usize,
    edge: EdgeId,
    source: &[omega_optimization_unit::FuelSettlement],
    proposed: &[omega_optimization_unit::FuelSettlement],
) -> Result<(), LegalizationError> {
    if source.is_empty()
        || source
            .iter()
            .any(|settlement| settlement.site != PsiProvenance::Edge(edge))
    {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed != source {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}
