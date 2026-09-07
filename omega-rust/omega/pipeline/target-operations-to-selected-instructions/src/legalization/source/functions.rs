use super::leaves::{derive_leaf, exact_edge_fuel, source_operations};
use super::shared::*;
use crate::legalization::catalog::LegalizationFormRecipe;

pub(super) fn derive_source_function(
    function: usize,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
) -> Result<SourceFunction, LegalizationError> {
    let (condition, form) =
        super::conditional_input::match_input(function, target, abstracted, optimized)?;
    let Some(constraints) = form.constraints.scalar([
        optimized.blocks[1].nodes.len(),
        optimized.blocks[2].nodes.len(),
    ]) else {
        unreachable!("matched scalar form");
    };
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
        condition.when_true.psi_edge,
        condition.when_true.control.as_ref(),
        &abstracted.operations[constraints.block_offsets[1]..constraints.block_offsets[2]],
        &optimized.blocks[1].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [LegalizedTemporaryId(0), LegalizedTemporaryId(1)],
        matches!(form.recipe, LegalizationFormRecipe::Scalar(legalized_operations::LegalizationRecipe::ReturnU64ExactIntegerSequenceConditionalV1)),
    )?;
    let when_false = derive_leaf(
        function,
        condition.when_false.psi_edge,
        condition.when_false.control.as_ref(),
        &abstracted.operations[constraints.block_offsets[2]..],
        &optimized.blocks[2].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        [LegalizedTemporaryId(2), LegalizedTemporaryId(3)],
        false,
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
            || matches!(
                &condition.legalized,
                LegalizedCondition::DirectParameter { parameter_index, .. }
                    if *true_index == *parameter_index
            ))
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_provenance = TerminalPsiProvenance {
        operations: condition
            .provenance_operations
            .into_iter()
            .chain(source_operations(&when_true.value))
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
        recipe: match form.recipe {
            LegalizationFormRecipe::Scalar(recipe) => recipe,
            _ => return Err(Error::UnsupportedSourceShape { function }),
        },
        condition_source: condition.source,
        condition: condition.legalized,
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
