use omega_optimization_unit::{OptimizationFact, PsiOptimizationUnit, PsiProvenance};
use omega_terminal_abstract_operations::{
    TerminalAbstractOperation, TerminalAbstractOperationPlan,
};
use omega_terminal_legalized_operations::{
    TerminalLegalizationRecipe, TerminalLegalizedFunction, TerminalLegalizedImmediate,
    TerminalLegalizedLeaf, TerminalLegalizedLeafValue, TerminalLegalizedOperationPlan,
};
use omega_terminal_target_operations::{
    TerminalPsiProvenance, TerminalScalarParameterLocation, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
};
use psi_core::{EdgeId, IntegerSign, OperationId, ScalarType};

use crate::{TerminalLegalizationError, TerminalLegalizationError as Error};

/// Independently replay a proposed V1 legal projection against all three raw
/// custody inputs. This module deliberately compares fields in place instead
/// of constructing a second plan with the producer's derivation strategy.
pub(crate) fn replay_terminal_legalized_plan(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &TerminalLegalizedOperationPlan,
) -> Result<(), TerminalLegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
        || target.terminal_psi != abstract_plan.terminal_psi
        || target.terminal_psi != unit.terminal_psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
    {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed.terminal_psi != target.terminal_psi
        || proposed.optimization_unit != unit.identity
        || proposed.fuel_schedule != unit.fuel_schedule
        || proposed.target != target.target
        || proposed.entry != target.entry
        || proposed.functions.len() != target.functions.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    for (index, (((target_function, abstracted), optimized), legalized)) in target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .zip(&proposed.functions)
        .enumerate()
    {
        replay_function(
            index,
            target.target.architecture,
            target_function,
            abstracted,
            optimized,
            &unit.accepted_obligation_facts,
            legalized,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_function(
    function: usize,
    architecture: omega_target::Architecture,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &TerminalLegalizedFunction,
) -> Result<(), TerminalLegalizationError> {
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
    if proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.entry_block != optimized.blocks[0].id
        || proposed.true_block != optimized.blocks[1].id
        || proposed.false_block != optimized.blocks[2].id
    {
        return Err(Error::NonCanonicalLegalizedPlan);
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
    if condition_register.architecture() != architecture {
        return Err(Error::SourceCustodyMismatch);
    }

    let recipe_matches_target = match proposed.recipe {
        TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1 => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm.control.as_ref(),
                    TerminalTargetIntegerControl::Return {
                        expression: TerminalTargetIntegerExpression::Immediate { .. },
                        ..
                    }
                )
            })
        }
        TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1 => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm.control.as_ref(),
                    TerminalTargetIntegerControl::Return {
                        expression: TerminalTargetIntegerExpression::Parameter { .. },
                        ..
                    }
                )
            })
        }
        TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TerminalTargetIntegerControl::Return {
                    expression: TerminalTargetIntegerExpression::ExactAdd { left, right, .. },
                    ..
                } if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                    && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
            )
        }),
        TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TerminalTargetIntegerControl::Return {
                    expression: TerminalTargetIntegerExpression::ExactSubtract { left, right, .. },
                    ..
                } if matches!(left.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
                    && matches!(right.as_ref(), TerminalTargetIntegerExpression::Immediate { .. })
            )
        }),
    };
    if !recipe_matches_target {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let (offsets, operation_count, leaf_node_count, parameter_count) = match proposed.recipe {
        TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1 => ([0, 1, 3], 5, 2, 1),
        TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1 => ([0, 1, 2], 3, 1, 2),
        TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1
        | TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => {
            ([0, 1, 5], 9, 4, 1)
        }
    };
    if abstracted.operations.len() != operation_count
        || abstracted.parameters.len() != parameter_count
        || optimized.parameters.len() != parameter_count
        || abstracted
            .block_entries
            .iter()
            .zip(offsets)
            .any(|(entry, offset)| entry.operation_offset != offset)
        || optimized.blocks[1].nodes.len() != leaf_node_count
        || optimized.blocks[2].nodes.len() != leaf_node_count
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
    if proposed.condition_source != *condition_source
        || proposed.condition_parameter_index != *condition_parameter_index
        || proposed.condition_register != *condition_register
        || proposed.condition_definition_site != parameter.site
    {
        return Err(Error::NonCanonicalLegalizedPlan);
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
        || entry_node.successors[0].bindings != abstract_true.bindings
        || entry_node.successors[1].psi_edge != abstract_false.psi_edge
        || entry_node.successors[1].target != abstract_false.target
        || entry_node.successors[1].bindings != abstract_false.bindings
        || !entry_node.provenance.is_empty()
        || !entry_node.fuel.is_empty()
        || entry_node.successors[0].provenance != vec![PsiProvenance::Edge(abstract_true.psi_edge)]
        || entry_node.successors[1].provenance != vec![PsiProvenance::Edge(abstract_false.psi_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    if proposed.branch_true_edge != abstract_true.psi_edge
        || proposed.branch_false_edge != abstract_false.psi_edge
        || proposed.branch_true_bindings != abstract_true.bindings
        || proposed.branch_false_bindings != abstract_false.bindings
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_edge_fuel(
        function,
        abstract_true.psi_edge,
        &entry_node.successors[0].fuel,
        &proposed.branch_true_fuel,
    )?;
    replay_edge_fuel(
        function,
        abstract_false.psi_edge,
        &entry_node.successors[1].fuel,
        &proposed.branch_false_fuel,
    )?;

    let true_operations = replay_leaf(
        function,
        proposed.recipe,
        when_true.psi_edge,
        when_true.control.as_ref(),
        &abstracted.operations[offsets[1]..offsets[2]],
        &optimized.blocks[1].nodes,
        abstracted,
        optimized,
        accepted_facts,
        &proposed.when_true,
        architecture,
    )?;
    let false_operations = replay_leaf(
        function,
        proposed.recipe,
        when_false.psi_edge,
        when_false.control.as_ref(),
        &abstracted.operations[offsets[2]..],
        &optimized.blocks[2].nodes,
        abstracted,
        optimized,
        accepted_facts,
        &proposed.when_false,
        architecture,
    )?;
    if let (
        TerminalLegalizedLeafValue::EntryParameter {
            parameter_index: true_index,
            register: true_register,
            ..
        },
        TerminalLegalizedLeafValue::EntryParameter {
            parameter_index: false_index,
            register: false_register,
            ..
        },
    ) = (&proposed.when_true.value, &proposed.when_false.value)
        && (proposed.when_true.source_value != proposed.when_false.source_value
            || true_index != false_index
            || true_register != false_register
            || *true_index == *condition_parameter_index)
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let expected_provenance = TerminalPsiProvenance {
        operations: true_operations
            .into_iter()
            .chain(false_operations)
            .collect(),
        edges: vec![
            abstract_true.psi_edge,
            abstract_false.psi_edge,
            proposed.when_true.return_edge,
            proposed.when_false.return_edge,
        ],
    };
    if target.provenance != expected_provenance {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed.provenance != expected_provenance {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_leaf(
    function: usize,
    recipe: TerminalLegalizationRecipe,
    arm_edge: EdgeId,
    target: &TerminalTargetIntegerControl,
    abstract_operations: &[TerminalAbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &TerminalLegalizedLeaf,
    architecture: omega_target::Architecture,
) -> Result<Vec<OperationId>, TerminalLegalizationError> {
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
    if proposed.return_edge != *psi_return_edge || proposed.source_value != *source_value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let u64_integer = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer);
    let (return_node, operations) = match (recipe, expression, &proposed.value) {
        (
            TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1,
            TerminalTargetIntegerExpression::Immediate {
                source_value: expression_source,
                value: target_value,
            },
            TerminalLegalizedLeafValue::Immediate {
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
            TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1,
            TerminalTargetIntegerExpression::Parameter {
                source_value: expression_source,
                parameter_index,
                location: TerminalScalarParameterLocation::Register(register),
            },
            TerminalLegalizedLeafValue::EntryParameter {
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
            TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1,
            TerminalTargetIntegerExpression::ExactAdd {
                psi_operation,
                obligation,
                left,
                right,
            },
            TerminalLegalizedLeafValue::ExactAdd {
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
            TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1,
            TerminalTargetIntegerExpression::ExactSubtract {
                psi_operation,
                obligation,
                left,
                right,
            },
            TerminalLegalizedLeafValue::ExactSubtract {
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

    let TerminalAbstractOperation::Return {
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

#[allow(clippy::too_many_arguments)]
fn replay_exact_binary(
    function: usize,
    arm_edge: EdgeId,
    add: bool,
    result_value: psi_core::ValueId,
    operation: OperationId,
    obligation: psi_core::ObligationId,
    target_left: &TerminalTargetIntegerExpression,
    target_right: &TerminalTargetIntegerExpression,
    nodes: &[omega_optimization_unit::OptimizationNode],
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed_obligation: psi_core::ObligationId,
    proposed_fact: omega_optimization_core::AcceptedObligationFactIdentity,
    proposed_operation: OperationId,
    proposed_site: omega_optimization_unit::ValueDefinitionSite,
    proposed_fuel: &[omega_optimization_unit::FuelSettlement],
    proposed_left: &TerminalLegalizedImmediate,
    proposed_right: &TerminalLegalizedImmediate,
    u64_type: ScalarType,
) -> Result<(), TerminalLegalizationError> {
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
                TerminalAbstractOperation::ExactIntegerAdd {
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
                TerminalAbstractOperation::ExactIntegerSubtract {
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
    target: &TerminalTargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    proposed: &TerminalLegalizedImmediate,
    expected_type: ScalarType,
) -> Result<(), TerminalLegalizationError> {
    let TerminalTargetIntegerExpression::Immediate {
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
) -> Result<OperationId, TerminalLegalizationError> {
    let TerminalAbstractOperation::IntegerConstant {
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
) -> Result<(), TerminalLegalizationError> {
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

fn replay_edge_fuel(
    function: usize,
    edge: EdgeId,
    source: &[omega_optimization_unit::FuelSettlement],
    proposed: &[omega_optimization_unit::FuelSettlement],
) -> Result<(), TerminalLegalizationError> {
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
