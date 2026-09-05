use super::conditions;
use super::leaf::{replay_edge_fuel, replay_leaf};
use super::shared::*;
use super::validators::{scalar_validator_accepts, validate_unit_form};
use crate::legalization::catalog::{
    LegalizationFormRecipe, LegalizationShapeConstraints, LegalizationValidatorKind,
    legalization_form_for_recipe,
};

pub(super) fn replay_unit_function(
    function: usize,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    proposed: &LegalizedUnitFunction,
) -> Result<usize, LegalizationError> {
    validate_unit_form(target, abstracted, optimized, proposed.recipe)
        .ok_or(Error::NonCanonicalLegalizedPlan)?;
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [target_return] = body.operations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let target_operations::TargetUnitOperation::Return {
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
            abstract_operations::AbstractFunctionResult::Unit
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
    architecture: target::Architecture,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[optimization_unit::AcceptedObligationFact],
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

    let condition = conditions::replay(
        function,
        architecture,
        target,
        abstracted,
        optimized,
        proposed.condition_source,
        &proposed.condition,
    )?;
    if condition.result_type.is_address()
        || condition.result_type.sign() != IntegerSign::Unsigned
        || condition.result_type.bits() != 64
    {
        return Err(Error::UnsupportedIntegerShape { function });
    }

    let form = legalization_form_for_recipe(LegalizationFormRecipe::Scalar(proposed.recipe))
        .ok_or(Error::NonCanonicalLegalizedPlan)?;
    let LegalizationValidatorKind::Scalar(validator) = form.validator else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    if !scalar_validator_accepts(
        validator,
        condition.when_true.control.as_ref(),
        condition.when_false.control.as_ref(),
    ) {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let LegalizationShapeConstraints::Scalar(constraints) = form.constraints else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    if constraints.condition != condition.shape
        || optimized.blocks[0].nodes.len() != constraints.entry_node_count
        || abstracted.operations.len() != constraints.operation_count
        || abstracted.parameters.len() != constraints.parameter_count
        || optimized.parameters.len() != constraints.parameter_count
        || abstracted
            .block_entries
            .iter()
            .zip(constraints.block_offsets)
            .any(|(entry, offset)| entry.operation_offset != offset)
        || optimized.blocks[1].nodes.len() != constraints.leaf_node_counts[0]
        || optimized.blocks[2].nodes.len() != constraints.leaf_node_counts[1]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let entry_node = &optimized.blocks[0].nodes[condition.conditional_node_index];
    if entry_node.operation != abstracted.operations[condition.conditional_node_index] {
        return Err(Error::SourceCustodyMismatch);
    }
    let AbstractOperation::Conditional {
        condition: branch_condition,
        when_true: abstract_true,
        when_false: abstract_false,
    } = &entry_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *branch_condition != condition.source
        || abstract_true.psi_edge != condition.when_true.psi_edge
        || abstract_false.psi_edge != condition.when_false.psi_edge
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
        condition.when_true.psi_edge,
        condition.when_true.control.as_ref(),
        &abstracted.operations[constraints.block_offsets[1]..constraints.block_offsets[2]],
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
        condition.when_false.psi_edge,
        condition.when_false.control.as_ref(),
        &abstracted.operations[constraints.block_offsets[2]..],
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
            || matches!(
                &proposed.condition,
                LegalizedCondition::DirectParameter { parameter_index, .. }
                    if *true_index == *parameter_index
            ))
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let expected_provenance = TerminalPsiProvenance {
        operations: condition
            .provenance_operations
            .into_iter()
            .chain(true_operations)
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
