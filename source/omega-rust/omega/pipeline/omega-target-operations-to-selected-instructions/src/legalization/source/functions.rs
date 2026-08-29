use super::leaves::{derive_leaf, exact_edge_fuel, source_operations};
use super::matchers::match_scalar_form;
use super::shared::*;

pub(super) fn derive_source_unit_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
) -> Result<SourceUnitFunction, LegalizationError> {
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
        // The current Unit vocabulary carries no structural ABI or ownership
        // rows. Reject them here instead of silently projecting them away; a
        // later ProgramStorage wrapper form must retain these fields exactly.
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
    Ok(SourceUnitFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        entry_block: optimized_block.id,
        return_edge: *psi_edge,
        return_fuel: optimized_return.fuel.clone(),
    })
}

pub(super) fn derive_source_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
) -> Result<SourceFunction, LegalizationError> {
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
    let form = match_scalar_form(when_true.control.as_ref(), when_false.control.as_ref())
        .ok_or(Error::UnsupportedSourceShape { function })?;
    let constraints = form.constraints;
    if abstracted.operations.len() != constraints.operation_count
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
        &abstracted.operations[constraints.block_offsets[1]..constraints.block_offsets[2]],
        &optimized.blocks[1].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [LegalizedTemporaryId(0), LegalizedTemporaryId(1)],
    )?;
    let when_false = derive_leaf(
        function,
        when_false.psi_edge,
        when_false.control.as_ref(),
        &abstracted.operations[constraints.block_offsets[2]..],
        &optimized.blocks[2].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [LegalizedTemporaryId(2), LegalizedTemporaryId(3)],
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
        recipe: form.recipe,
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
