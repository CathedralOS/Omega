use super::leaves::{replay_active_resident_chain_shape, replay_edge_fuel, replay_leaf};
use super::shared::*;

pub(super) fn replay_unit_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed: &LegalizedUnitFunction,
) -> Result<usize, LegalizationError> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [target_return] = body.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let omega_target_operations::TargetUnitOperation::Return {
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
            omega_abstract_operations::AbstractFunctionResult::Unit
        )
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        // This closed Unit record has no structural ABI or ownership
        // vocabulary. Independent replay must fail rather than validate a
        // proposal that erased those source declarations.
        || !body.parameters.is_empty()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || abstracted.entry != abstract_entry.block
        || optimized.entry != abstract_entry.block
        || optimized_block.id != abstract_entry.block
        || abstract_entry.operation_offset != 0
        || !abstract_entry.parameters.is_empty()
        || !optimized_block.parameters.is_empty()
        || !cleanup_actions.is_empty()
        || abstract_return != &optimized_return.operation
        || !matches!(abstract_return, AbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    if proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.provenance != target.provenance
        || proposed.entry_block != optimized_block.id
        || proposed.return_edge != *psi_edge
        || proposed.return_fuel != optimized_return.fuel
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_function(
    function: usize,
    architecture: omega_target::Architecture,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &LegalizedFunction,
) -> Result<usize, LegalizationError> {
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

    let TargetOperation::ReturnIntegerConditionalControl {
        condition_source,
        condition_parameter_index,
        condition_location: ScalarParameterLocation::Register(condition_register),
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
        LegalizationRecipe::ReturnU64ImmediateConditionalV1 => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm.control.as_ref(),
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::Immediate { .. },
                        ..
                    }
                )
            })
        }
        LegalizationRecipe::ReturnU64EntryParameterConditionalV1 => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm.control.as_ref(),
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::Parameter { .. },
                        ..
                    }
                )
            })
        }
        LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::ExactAdd { left, right, .. },
                    ..
                } if matches!(left.as_ref(), TargetIntegerExpression::Immediate { .. })
                    && matches!(right.as_ref(), TargetIntegerExpression::Immediate { .. })
            )
        }),
        LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::ExactSubtract { left, right, .. },
                    ..
                } if matches!(left.as_ref(), TargetIntegerExpression::Immediate { .. })
                    && matches!(right.as_ref(), TargetIntegerExpression::Immediate { .. })
            )
        }),
        LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::IntegerWiden {
                        source_type,
                        operand,
                        ..
                    },
                    ..
                } if *source_type
                    == psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")
                    && matches!(
                        operand.as_ref(),
                        TargetIntegerExpression::ExactAdd { left, right, .. }
                            if matches!(left.as_ref(), TargetIntegerExpression::Immediate { .. })
                                && matches!(right.as_ref(), TargetIntegerExpression::Immediate { .. })
                    )
            )
        }),
        LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1 => [
            when_true, when_false,
        ]
        .iter()
        .all(|arm| {
            matches!(
                arm.control.as_ref(),
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::IntegerWiden {
                        source_type,
                        operand,
                        ..
                    },
                    ..
                } if *source_type
                    == psi_core::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")
                    && matches!(
                        operand.as_ref(),
                        TargetIntegerExpression::ExactSubtract { left, right, .. }
                            if matches!(left.as_ref(), TargetIntegerExpression::Immediate { .. })
                                && matches!(right.as_ref(), TargetIntegerExpression::Immediate { .. })
                    )
            )
        }),
        LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1 => {
            matches!(
                (when_true.control.as_ref(), when_false.control.as_ref()),
                (
                    TargetIntegerControl::Return { expression, .. },
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::Immediate { .. },
                        ..
                    }
                ) if replay_active_resident_chain_shape(expression)
            )
        }
    };
    if !recipe_matches_target {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let (offsets, operation_count, leaf_node_counts, parameter_count) = match proposed.recipe {
        LegalizationRecipe::ReturnU64ImmediateConditionalV1 => ([0, 1, 3], 5, [2, 2], 1),
        LegalizationRecipe::ReturnU64EntryParameterConditionalV1 => ([0, 1, 2], 3, [1, 1], 2),
        LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1
        | LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1 => {
            ([0, 1, 5], 9, [4, 4], 1)
        }
        LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
        | LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1 => {
            ([0, 1, 6], 11, [5, 5], 1)
        }
        LegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1 => {
            ([0, 1, 8], 10, [7, 2], 1)
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
        || optimized.blocks[1].nodes.len() != leaf_node_counts[0]
        || optimized.blocks[2].nodes.len() != leaf_node_counts[1]
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
    let AbstractOperation::Conditional {
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
        0,
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
        2,
    )?;
    if let (
        LegalizedLeafValue::EntryParameter {
            parameter_index: true_index,
            register: true_register,
            ..
        },
        LegalizedLeafValue::EntryParameter {
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
    Ok(usize::from(matches!(
        proposed.recipe,
        LegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
            | LegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1
    )) * 2)
}
