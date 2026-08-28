use omega_optimization_unit::{
    AcceptedObligationFact, FuelSettlement, OptimizationFact, PsiOptimizationUnit, PsiProvenance,
};
use omega_terminal_abstract_operations::{
    TerminalAbstractOperation, TerminalAbstractOperationPlan,
};
use omega_terminal_legalized_operations::{
    TerminalLegalizationRecipe, TerminalLegalizationTheorem,
    TerminalLegalizedActiveResidentExactAddChain as SourceActiveResidentExactAddChain,
    TerminalLegalizedExactAdd as SourceExactAdd, TerminalLegalizedFunction as SourceFunction,
    TerminalLegalizedImmediate as SourceImmediate, TerminalLegalizedLeaf as SourceLeaf,
    TerminalLegalizedLeafValue as SourceLeafValue, TerminalLegalizedTemporaryId,
    TerminalLegalizedUnitFunction as SourceUnitFunction,
};
use omega_terminal_target_operations::{
    TerminalPsiProvenance, TerminalScalarParameterLocation, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
};
use psi_core::{EdgeId, IntegerSign, OperationId, ScalarType};

use crate::{TerminalLegalizationError, TerminalLegalizationError as Error};

pub(crate) fn derive_source_functions(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceFunction>, TerminalLegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.terminal_psi != abstract_plan.terminal_psi
        || target.terminal_psi != unit.terminal_psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }

    let functions = target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .enumerate()
        .filter_map(|(index, ((target, abstracted), optimized))| {
            (!matches!(target.operation, TerminalTargetOperation::UnitBody(_))).then(|| {
                derive_source_function(
                    index,
                    target,
                    abstracted,
                    optimized,
                    &unit.accepted_obligation_facts,
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if functions.iter().any(|function| {
        function.condition_register.architecture() != target.target.architecture
            || match (&function.when_true.value, &function.when_false.value) {
                (
                    SourceLeafValue::EntryParameter { register: left, .. },
                    SourceLeafValue::EntryParameter {
                        register: right, ..
                    },
                ) => {
                    left.architecture() != target.target.architecture
                        || right.architecture() != target.target.architecture
                }
                _ => false,
            }
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(functions)
}

pub(crate) fn derive_source_unit_functions(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceUnitFunction>, TerminalLegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.terminal_psi != abstract_plan.terminal_psi
        || target.terminal_psi != unit.terminal_psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }
    target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .enumerate()
        .filter_map(|(index, ((target, abstracted), optimized))| {
            matches!(target.operation, TerminalTargetOperation::UnitBody(_))
                .then(|| derive_source_unit_function(index, target, abstracted, optimized))
        })
        .collect()
}

fn derive_source_unit_function(
    function: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<SourceUnitFunction, TerminalLegalizationError> {
    let TerminalTargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [target_return] = body.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let omega_terminal_target_operations::TerminalTargetUnitOperation::Return {
        psi_edge,
        cleanup_actions,
    } = target_return
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_entry] = abstracted.block_entries.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_return] = abstracted.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [optimized_block] = optimized.blocks.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [optimized_return] = optimized_block.nodes.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || !matches!(
            abstracted.result,
            omega_terminal_abstract_operations::TerminalAbstractFunctionResult::Unit
        )
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        || abstracted.entry != abstract_entry.block
        || optimized.entry != abstract_entry.block
        || optimized_block.id != abstract_entry.block
        || abstract_entry.operation_offset != 0
        || !abstract_entry.parameters.is_empty()
        || !optimized_block.parameters.is_empty()
        || !cleanup_actions.is_empty()
        || abstract_return != &optimized_return.operation
        || !matches!(abstract_return, TerminalAbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(SourceUnitFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        entry_block: optimized_block.id,
        return_edge: *psi_edge,
        return_fuel: optimized_return.fuel.clone(),
    })
}

fn derive_source_function(
    function: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
) -> Result<SourceFunction, TerminalLegalizationError> {
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || abstracted.block_entries.len() != 3
        || optimized.blocks.len() != 3
        || optimized.entry != abstracted.entry
        || optimized.blocks[0].id != abstracted.block_entries[0].block
        || optimized.blocks[1].id != abstracted.block_entries[1].block
        || optimized.blocks[2].id != abstracted.block_entries[2].block
        || optimized.blocks[0].nodes.len() != 1
        || abstracted
            .block_entries
            .iter()
            .any(|entry| !entry.parameters.is_empty())
        || optimized
            .blocks
            .iter()
            .any(|block| !block.parameters.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        condition_source,
        condition_parameter_index,
        condition_location: TerminalScalarParameterLocation::Register(condition_register),
        scalar_type,
        when_true,
        when_false,
    } = &target.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if scalar_type.is_address()
        || scalar_type.sign() != IntegerSign::Unsigned
        || scalar_type.bits() != 64
    {
        return Err(Error::UnsupportedIntegerShape { function });
    }
    let constant_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Immediate { .. },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Immediate { .. },
                ..
            }
        )
    );
    let parameter_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Parameter { .. },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Parameter { .. },
                ..
            }
        )
    );
    let exact_add_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactAdd {
                    left,
                    right,
                    ..
                },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactAdd {
                    left: false_left,
                    right: false_right,
                    ..
                },
                ..
            }
        ) if matches!(
            (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
            (
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
            )
        )
    );
    let exact_subtract_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactSubtract {
                    left,
                    right,
                    ..
                },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactSubtract {
                    left: false_left,
                    right: false_right,
                    ..
                },
                ..
            }
        ) if matches!(
            (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
            (
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
            )
        )
    );
    let u8_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let widened_u8_exact_add_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::IntegerWiden {
                    source_type,
                    operand,
                    ..
                },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::IntegerWiden {
                    source_type: false_source_type,
                    operand: false_operand,
                    ..
                },
                ..
            }
        ) if *source_type == u8_integer_type
            && *false_source_type == u8_integer_type
            && matches!(
                (operand.as_ref(), false_operand.as_ref()),
                (
                    TerminalTargetIntegerExpression::ExactAdd { left, right, .. },
                    TerminalTargetIntegerExpression::ExactAdd {
                        left: false_left,
                        right: false_right,
                        ..
                    }
                ) if matches!(
                    (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
                    (
                        TerminalTargetIntegerExpression::Immediate { .. },
                        TerminalTargetIntegerExpression::Immediate { .. },
                        TerminalTargetIntegerExpression::Immediate { .. },
                        TerminalTargetIntegerExpression::Immediate { .. },
                    )
                )
            )
    );
    let widened_u8_exact_subtract_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::IntegerWiden {
                    source_type,
                    operand,
                    ..
                },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::IntegerWiden {
                    source_type: false_source_type,
                    operand: false_operand,
                    ..
                },
                ..
            }
        ) if *source_type == u8_integer_type
            && *false_source_type == u8_integer_type
            && matches!(
                (operand.as_ref(), false_operand.as_ref()),
                (
                    TerminalTargetIntegerExpression::ExactSubtract { left, right, .. },
                    TerminalTargetIntegerExpression::ExactSubtract {
                        left: false_left,
                        right: false_right,
                        ..
                    }
                ) if matches!(
                    (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
                    (
                        TerminalTargetIntegerExpression::Immediate { .. },
                        TerminalTargetIntegerExpression::Immediate { .. },
                        TerminalTargetIntegerExpression::Immediate { .. },
                        TerminalTargetIntegerExpression::Immediate { .. },
                    )
                )
            )
    );
    let active_resident_chain = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return { expression, .. },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Immediate { .. },
                ..
            }
        ) if is_active_resident_exact_add_chain(expression)
    );
    let expected_offsets = if constant_leaves {
        [0, 1, 3]
    } else if parameter_leaves {
        [0, 1, 2]
    } else if exact_add_leaves || exact_subtract_leaves {
        [0, 1, 5]
    } else if widened_u8_exact_add_leaves || widened_u8_exact_subtract_leaves {
        [0, 1, 6]
    } else if active_resident_chain {
        [0, 1, 8]
    } else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let (expected_operation_count, expected_leaf_node_counts) = if constant_leaves {
        (5, [2, 2])
    } else if parameter_leaves {
        (3, [1, 1])
    } else if widened_u8_exact_add_leaves || widened_u8_exact_subtract_leaves {
        (11, [5, 5])
    } else if active_resident_chain {
        (10, [7, 2])
    } else {
        (9, [4, 4])
    };
    let expected_parameter_count = if parameter_leaves { 2 } else { 1 };
    if abstracted.operations.len() != expected_operation_count
        || abstracted.parameters.len() != expected_parameter_count
        || optimized.parameters.len() != expected_parameter_count
        || abstracted
            .block_entries
            .iter()
            .zip(expected_offsets)
            .any(|(entry, offset)| entry.operation_offset != offset)
        || optimized.blocks[1].nodes.len() != expected_leaf_node_counts[0]
        || optimized.blocks[2].nodes.len() != expected_leaf_node_counts[1]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(parameter) = optimized.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(abstract_parameter) = abstracted.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    if parameter.value != *condition_source
        || parameter.scalar_type != ScalarType::Boolean
        || abstract_parameter.value != *condition_source
        || abstract_parameter.scalar_type != ScalarType::Boolean
    {
        return Err(Error::UnsupportedCondition { function });
    }

    let entry_node = &optimized.blocks[0].nodes[0];
    if entry_node.operation != abstracted.operations[0] {
        return Err(Error::SourceCustodyMismatch);
    }
    let TerminalAbstractOperation::Conditional {
        condition,
        when_true: abstract_true,
        when_false: abstract_false,
    } = &entry_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *condition != *condition_source
        || abstract_true.psi_edge != when_true.psi_edge
        || abstract_false.psi_edge != when_false.psi_edge
        || abstract_true.target != optimized.blocks[1].id
        || abstract_false.target != optimized.blocks[2].id
        || !abstract_true.bindings.is_empty()
        || !abstract_false.bindings.is_empty()
        || entry_node.successors.len() != 2
        || entry_node.successors[0].psi_edge != abstract_true.psi_edge
        || entry_node.successors[0].target != abstract_true.target
        || !entry_node.successors[0].bindings.is_empty()
        || entry_node.successors[1].psi_edge != abstract_false.psi_edge
        || entry_node.successors[1].target != abstract_false.target
        || !entry_node.successors[1].bindings.is_empty()
        || !entry_node.provenance.is_empty()
        || !entry_node.fuel.is_empty()
        || entry_node.successors[0].provenance != vec![PsiProvenance::Edge(abstract_true.psi_edge)]
        || entry_node.successors[1].provenance != vec![PsiProvenance::Edge(abstract_false.psi_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let branch_true_fuel = exact_edge_fuel(entry_node, abstract_true.psi_edge, function)?;
    let branch_false_fuel = exact_edge_fuel(entry_node, abstract_false.psi_edge, function)?;
    if entry_node.successors[0].fuel.len() != branch_true_fuel.len()
        || entry_node.successors[1].fuel.len() != branch_false_fuel.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let when_true = derive_leaf(
        function,
        when_true.psi_edge,
        when_true.control.as_ref(),
        &abstracted.operations[expected_offsets[1]..expected_offsets[2]],
        &optimized.blocks[1].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [
            TerminalLegalizedTemporaryId(0),
            TerminalLegalizedTemporaryId(1),
        ],
    )?;
    let when_false = derive_leaf(
        function,
        when_false.psi_edge,
        when_false.control.as_ref(),
        &abstracted.operations[expected_offsets[2]..],
        &optimized.blocks[2].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [
            TerminalLegalizedTemporaryId(2),
            TerminalLegalizedTemporaryId(3),
        ],
    )?;
    if let (
        SourceLeafValue::EntryParameter {
            parameter_index: true_index,
            register: true_register,
            ..
        },
        SourceLeafValue::EntryParameter {
            parameter_index: false_index,
            register: false_register,
            ..
        },
    ) = (&when_true.value, &when_false.value)
        && (when_true.source_value != when_false.source_value
            || true_index != false_index
            || true_register != false_register
            || *true_index == *condition_parameter_index)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_provenance = TerminalPsiProvenance {
        operations: source_operations(&when_true.value)
            .into_iter()
            .chain(source_operations(&when_false.value))
            .collect(),
        edges: vec![
            abstract_true.psi_edge,
            abstract_false.psi_edge,
            when_true.return_edge,
            when_false.return_edge,
        ],
    };
    if target.provenance != expected_provenance {
        return Err(Error::SourceCustodyMismatch);
    }

    Ok(SourceFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        recipe: if constant_leaves {
            TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1
        } else if parameter_leaves {
            TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1
        } else if exact_add_leaves {
            TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1
        } else if widened_u8_exact_add_leaves {
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
        } else if widened_u8_exact_subtract_leaves {
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1
        } else if active_resident_chain {
            TerminalLegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1
        } else {
            TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1
        },
        condition_source: *condition_source,
        condition_parameter_index: *condition_parameter_index,
        condition_register: *condition_register,
        condition_definition_site: parameter.site,
        entry_block: optimized.blocks[0].id,
        true_block: optimized.blocks[1].id,
        false_block: optimized.blocks[2].id,
        branch_true_edge: abstract_true.psi_edge,
        branch_false_edge: abstract_false.psi_edge,
        branch_true_fuel,
        branch_false_fuel,
        branch_true_bindings: abstract_true.bindings.clone(),
        branch_false_bindings: abstract_false.bindings.clone(),
        when_true,
        when_false,
    })
}

#[allow(clippy::too_many_arguments)]
fn derive_leaf(
    function: usize,
    arm_edge: EdgeId,
    target: &TerminalTargetIntegerControl,
    abstract_operations: &[TerminalAbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
    temporaries: [TerminalLegalizedTemporaryId; 2],
) -> Result<SourceLeaf, TerminalLegalizationError> {
    if nodes.len() != abstract_operations.len()
        || nodes
            .iter()
            .zip(abstract_operations)
            .any(|(node, operation)| node.operation != *operation)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TerminalTargetIntegerControl::Return {
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
        TerminalTargetIntegerExpression::Immediate {
            source_value: expression_source,
            value: target_value,
        } => {
            if nodes.len() != 2 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let TerminalAbstractOperation::IntegerConstant {
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
        TerminalTargetIntegerExpression::Parameter {
            source_value: expression_source,
            parameter_index,
            location: TerminalScalarParameterLocation::Register(register),
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
        TerminalTargetIntegerExpression::IntegerWiden {
            psi_operation: widen_operation,
            source_type,
            operand,
        } => {
            let u8_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let u8_type = ScalarType::Integer(u8_integer_type);
            let (is_subtract, arithmetic_operation, obligation, left, right) =
                match operand.as_ref() {
                    TerminalTargetIntegerExpression::ExactAdd {
                        psi_operation,
                        obligation,
                        left,
                        right,
                    } => (false, psi_operation, obligation, left, right),
                    TerminalTargetIntegerExpression::ExactSubtract {
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
                    TerminalAbstractOperation::ExactIntegerAdd {
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
                    TerminalAbstractOperation::ExactIntegerSubtract {
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
            let TerminalAbstractOperation::IntegerWiden {
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
                    theorem: TerminalLegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1,
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
                    theorem: TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1,
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
        expression @ TerminalTargetIntegerExpression::ExactAdd {
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
            let TerminalAbstractOperation::ExactIntegerAdd {
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
            let TerminalTargetIntegerExpression::ExactAdd {
                psi_operation: result_operation,
                obligation: result_obligation,
                left: result_left,
                right: result_right,
            } = expression
            else {
                unreachable!("shape predicate admitted only exact addition")
            };
            let TerminalTargetIntegerExpression::ExactAdd {
                psi_operation: middle_operation,
                obligation: middle_obligation,
                left: middle_left,
                right: middle_right,
            } = result_right.as_ref()
            else {
                unreachable!("shape predicate admitted the middle addition")
            };
            let TerminalTargetIntegerExpression::ExactAdd {
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
        TerminalTargetIntegerExpression::ExactSubtract {
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
            let TerminalAbstractOperation::ExactIntegerSubtract {
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
    let TerminalAbstractOperation::Return {
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

fn source_operations(value: &SourceLeafValue) -> Vec<OperationId> {
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

fn is_active_resident_exact_add_chain(expression: &TerminalTargetIntegerExpression) -> bool {
    let TerminalTargetIntegerExpression::ExactAdd {
        left: result_left,
        right: result_right,
        ..
    } = expression
    else {
        return false;
    };
    let TerminalTargetIntegerExpression::Immediate {
        source_value: result_resident,
        ..
    } = result_left.as_ref()
    else {
        return false;
    };
    let TerminalTargetIntegerExpression::ExactAdd {
        left: middle_left,
        right: middle_right,
        ..
    } = result_right.as_ref()
    else {
        return false;
    };
    let TerminalTargetIntegerExpression::Immediate {
        source_value: middle_resident,
        ..
    } = middle_left.as_ref()
    else {
        return false;
    };
    matches!(
        middle_right.as_ref(),
        TerminalTargetIntegerExpression::ExactAdd { left, right, .. }
            if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
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
) -> Result<SourceExactAdd, TerminalLegalizationError> {
    let TerminalAbstractOperation::ExactIntegerAdd {
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
    target: &TerminalTargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    expected_type: ScalarType,
) -> Result<SourceImmediate, TerminalLegalizationError> {
    let TerminalTargetIntegerExpression::Immediate {
        source_value,
        value: target_value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TerminalAbstractOperation::IntegerConstant {
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

fn exact_edge_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    edge: EdgeId,
    function: usize,
) -> Result<Vec<FuelSettlement>, TerminalLegalizationError> {
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
) -> Result<Vec<FuelSettlement>, TerminalLegalizationError> {
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
