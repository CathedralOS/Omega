use super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_leaf(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerControl,
    abstract_operations: &[AbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
    temporaries: [LegalizedTemporaryId; 2],
) -> Result<SourceLeaf, LegalizationError> {
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
    let u64_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer_type);
    let (return_node, value) = match expression {
        TargetIntegerExpression::Immediate {
            source_value: expression_source,
            value: target_value,
        } => {
            if nodes.len() != 2 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } = &nodes[0].operation
            else {
                return Err(Error::MissingConstantDefinition { function, arm_edge });
            };
            if *result != *source_value
                || *value != *target_value
                || *scalar_type != u64_type
                || nodes[0].definitions.len() != 1
                || nodes[0].definitions[0].value != *source_value
                || nodes[0].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::MissingConstantDefinition { function, arm_edge });
            }
            let constant_fuel = exact_operation_fuel(&nodes[0], *psi_operation, function)?;
            (
                &nodes[1],
                SourceLeafValue::Immediate {
                    value: *value,
                    constant_operation: *psi_operation,
                    definition_site: nodes[0].definitions[0].site,
                    constant_fuel,
                },
            )
        }
        TargetIntegerExpression::Parameter {
            source_value: expression_source,
            parameter_index,
            location: ScalarParameterLocation::Register(register),
        } => {
            if nodes.len() != 1 || source_value != expression_source {
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
            (
                &nodes[0],
                SourceLeafValue::EntryParameter {
                    parameter_index: *parameter_index,
                    register: *register,
                    definition_site: parameter.site,
                },
            )
        }
        TargetIntegerExpression::IntegerWiden {
            psi_operation: widen_operation,
            source_type,
            operand,
        } => {
            let u8_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let u8_type = ScalarType::Integer(u8_integer_type);
            let (is_subtract, arithmetic_operation, obligation, left, right) =
                match operand.as_ref() {
                    TargetIntegerExpression::ExactAdd {
                        psi_operation,
                        obligation,
                        left,
                        right,
                    } => (false, psi_operation, obligation, left, right),
                    TargetIntegerExpression::ExactSubtract {
                        psi_operation,
                        obligation,
                        left,
                        right,
                    } => (true, psi_operation, obligation, left, right),
                    _ => return Err(Error::UnsupportedSourceShape { function }),
                };
            if nodes.len() != 5 || *source_type != u8_integer_type {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u8_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u8_type)?;
            let (
                abstract_arithmetic_operation,
                abstract_obligation,
                narrow_result,
                arithmetic_type,
                abstract_left,
                abstract_right,
            ) = match (&nodes[2].operation, is_subtract) {
                (
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    },
                    false,
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
                    true,
                ) => (psi_operation, obligation, result, scalar_type, left, right),
                _ => return Err(Error::UnsupportedSourceShape { function }),
            };
            if abstract_arithmetic_operation != arithmetic_operation
                || abstract_obligation != obligation
                || *arithmetic_type != u8_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *narrow_result
                || nodes[2].provenance != vec![PsiProvenance::Operation(*arithmetic_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let AbstractOperation::IntegerWiden {
                psi_operation: abstract_widen_operation,
                result: widened_result,
                source_type: abstract_source_type,
                target_type: abstract_target_type,
                operand: abstract_operand,
            } = &nodes[3].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_widen_operation != widen_operation
                || *widened_result != *source_value
                || *narrow_result == *source_value
                || *abstract_source_type != u8_integer_type
                || *abstract_target_type != u64_integer_type
                || *abstract_operand != *narrow_result
                || nodes[3].definitions.len() != 1
                || nodes[3].definitions[0].value != *source_value
                || nodes[3].provenance != vec![PsiProvenance::Operation(*widen_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }

            let narrow_value = if is_subtract {
                u8_integer_type.exact_sub(left.value, right.value)
            } else {
                u8_integer_type.exact_add(left.value, right.value)
            };
            let Some(narrow_value) = narrow_value else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(widened_value) =
                u8_integer_type.widen_value_to(u64_integer_type, narrow_value)
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(widened_left) = u8_integer_type.widen_value_to(u64_integer_type, left.value)
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(widened_right) = u8_integer_type.widen_value_to(u64_integer_type, right.value)
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let recomputed_widened = if is_subtract {
                u64_integer_type.exact_sub(widened_left, widened_right)
            } else {
                u64_integer_type.exact_add(widened_left, widened_right)
            };
            if recomputed_widened != Some(widened_value) {
                return Err(Error::UnsupportedSourceShape { function });
            }

            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *arithmetic_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation
                        && *support == *arithmetic_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            let arithmetic_fuel = exact_operation_fuel(&nodes[2], *arithmetic_operation, function)?;
            let widen_fuel = exact_operation_fuel(&nodes[3], *widen_operation, function)?;
            let value = if is_subtract {
                SourceLeafValue::WidenedExactSubtract {
                    source_type: u8_integer_type,
                    target_type: u64_integer_type,
                    theorem: LegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1,
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    subtract_operation: *arithmetic_operation,
                    narrow_result: *narrow_result,
                    subtract_definition_site: nodes[2].definitions[0].site,
                    subtract_fuel: arithmetic_fuel,
                    widen_operation: *widen_operation,
                    widen_definition_site: nodes[3].definitions[0].site,
                    widen_fuel,
                    left_temporary: temporaries[0],
                    right_temporary: temporaries[1],
                    left,
                    right,
                }
            } else {
                SourceLeafValue::WidenedExactAdd {
                    source_type: u8_integer_type,
                    target_type: u64_integer_type,
                    theorem: LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1,
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    add_operation: *arithmetic_operation,
                    narrow_result: *narrow_result,
                    add_definition_site: nodes[2].definitions[0].site,
                    add_fuel: arithmetic_fuel,
                    widen_operation: *widen_operation,
                    widen_definition_site: nodes[3].definitions[0].site,
                    widen_fuel,
                    left_temporary: temporaries[0],
                    right_temporary: temporaries[1],
                    left,
                    right,
                }
            };
            (&nodes[4], value)
        }
        expression @ TargetIntegerExpression::ExactAdd {
            psi_operation,
            obligation,
            left,
            right,
        } if !is_active_resident_exact_add_chain(expression) => {
            if nodes.len() != 4 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u64_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u64_type)?;
            let AbstractOperation::ExactIntegerAdd {
                psi_operation: abstract_operation,
                obligation: abstract_obligation,
                result,
                scalar_type,
                left: abstract_left,
                right: abstract_right,
            } = &nodes[2].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_operation != psi_operation
                || abstract_obligation != obligation
                || *result != *source_value
                || *scalar_type != u64_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *source_value
                || nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let add_fuel = exact_operation_fuel(&nodes[2], *psi_operation, function)?;
            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation && *support == *psi_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            (
                &nodes[3],
                SourceLeafValue::ExactAdd {
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    add_operation: *psi_operation,
                    definition_site: nodes[2].definitions[0].site,
                    add_fuel,
                    left,
                    right,
                },
            )
        }
        expression if is_active_resident_exact_add_chain(expression) => {
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
            let resident = derive_immediate(function, arm_edge, result_left, &nodes[0], u64_type)?;
            let second_resident =
                derive_immediate(function, arm_edge, middle_left, &nodes[0], u64_type)?;
            if resident != second_resident {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, inner_left, &nodes[1], u64_type)?;
            let right = derive_immediate(function, arm_edge, inner_right, &nodes[2], u64_type)?;
            let inner = derive_exact_add(
                function,
                optimized,
                accepted_obligation_facts,
                &nodes[3],
                *inner_operation,
                *inner_obligation,
                left.source_value,
                right.source_value,
                u64_integer_type,
            )?;
            let middle = derive_exact_add(
                function,
                optimized,
                accepted_obligation_facts,
                &nodes[4],
                *middle_operation,
                *middle_obligation,
                resident.source_value,
                inner.source_value,
                u64_integer_type,
            )?;
            let result = derive_exact_add(
                function,
                optimized,
                accepted_obligation_facts,
                &nodes[5],
                *result_operation,
                *result_obligation,
                resident.source_value,
                middle.source_value,
                u64_integer_type,
            )?;
            if result.source_value != *source_value {
                return Err(Error::UnsupportedSourceShape { function });
            }
            (
                &nodes[6],
                SourceLeafValue::ActiveResidentExactAddChain(Box::new(
                    SourceActiveResidentExactAddChain {
                        resident,
                        left,
                        right,
                        inner,
                        middle,
                        result,
                    },
                )),
            )
        }
        TargetIntegerExpression::ExactSubtract {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            if nodes.len() != 4 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u64_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u64_type)?;
            let AbstractOperation::ExactIntegerSubtract {
                psi_operation: abstract_operation,
                obligation: abstract_obligation,
                result,
                scalar_type,
                left: abstract_left,
                right: abstract_right,
            } = &nodes[2].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_operation != psi_operation
                || abstract_obligation != obligation
                || *result != *source_value
                || *scalar_type != u64_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *source_value
                || nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let subtract_fuel = exact_operation_fuel(&nodes[2], *psi_operation, function)?;
            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation && *support == *psi_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            (
                &nodes[3],
                SourceLeafValue::ExactSubtract {
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    subtract_operation: *psi_operation,
                    definition_site: nodes[2].definitions[0].site,
                    subtract_fuel,
                    left,
                    right,
                },
            )
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let AbstractOperation::Return {
        psi_edge,
        value: returned_value,
        scalar_type: returned_type,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_edge != *psi_return_edge
        || *returned_value != *source_value
        || *returned_type != u64_type
        || !cleanup_actions.is_empty()
        || return_node.provenance != vec![PsiProvenance::Edge(*psi_return_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let return_fuel = exact_edge_fuel(return_node, *psi_return_edge, function)?;
    if return_node.fuel.len() != return_fuel.len() {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(SourceLeaf {
        return_edge: *psi_return_edge,
        source_value: *source_value,
        return_fuel,
        value,
    })
}

pub(super) fn source_operations(value: &SourceLeafValue) -> Vec<OperationId> {
    match value {
        SourceLeafValue::Immediate {
            constant_operation, ..
        } => vec![*constant_operation],
        SourceLeafValue::EntryParameter { .. } => Vec::new(),
        SourceLeafValue::ExactAdd {
            add_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *add_operation,
        ],
        SourceLeafValue::ExactSubtract {
            subtract_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *subtract_operation,
        ],
        SourceLeafValue::WidenedExactAdd {
            add_operation,
            widen_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *add_operation,
            *widen_operation,
        ],
        SourceLeafValue::WidenedExactSubtract {
            subtract_operation,
            widen_operation,
            left,
            right,
            ..
        } => vec![
            left.constant_operation,
            right.constant_operation,
            *subtract_operation,
            *widen_operation,
        ],
        SourceLeafValue::ActiveResidentExactAddChain(chain) => vec![
            chain.resident.constant_operation,
            chain.left.constant_operation,
            chain.right.constant_operation,
            chain.inner.operation,
            chain.middle.operation,
            chain.result.operation,
        ],
    }
}

pub(super) fn is_active_resident_exact_add_chain(expression: &TargetIntegerExpression) -> bool {
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
fn derive_exact_add(
    function: usize,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
    scalar_type: psi_core::IntegerType,
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
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_operation != operation
        || *abstract_obligation != obligation
        || *abstract_type != scalar_type
        || *abstract_left != left
        || *abstract_right != right
        || node.definitions.len() != 1
        || node.definitions[0].value != *result
        || node.provenance != vec![PsiProvenance::Operation(operation)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
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
                obligation: referenced_obligation,
                support,
            } if *referenced_obligation == obligation && *support == operation
        )
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(SourceExactAdd {
        source_value: *result,
        obligation,
        accepted_fact: accepted_fact.identity,
        operation,
        definition_site: node.definitions[0].site,
        fuel: exact_operation_fuel(node, operation, function)?,
    })
}

fn derive_immediate(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    expected_type: ScalarType,
) -> Result<SourceImmediate, LegalizationError> {
    let TargetIntegerExpression::Immediate {
        source_value,
        value: target_value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &node.operation
    else {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    };
    if result != source_value
        || value != target_value
        || *scalar_type != expected_type
        || node.definitions.len() != 1
        || node.definitions[0].value != *source_value
        || node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    }
    Ok(SourceImmediate {
        source_value: *source_value,
        value: *value,
        constant_operation: *psi_operation,
        definition_site: node.definitions[0].site,
        fuel: exact_operation_fuel(node, *psi_operation, function)?,
    })
}

pub(super) fn exact_edge_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    edge: EdgeId,
    function: usize,
) -> Result<Vec<FuelSettlement>, LegalizationError> {
    let custody = node
        .successors
        .iter()
        .find(|successor| successor.psi_edge == edge)
        .map_or(node.fuel.as_slice(), |successor| successor.fuel.as_slice());
    let fuel = custody
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Edge(edge))
        .collect::<Vec<_>>();
    if fuel.is_empty() || fuel.len() != custody.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}

fn exact_operation_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    function: usize,
) -> Result<Vec<FuelSettlement>, LegalizationError> {
    let fuel = node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Operation(operation))
        .collect::<Vec<_>>();
    if fuel.is_empty() || fuel.len() != node.fuel.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}
