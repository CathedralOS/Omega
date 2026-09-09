//! Publication retains the ordinary graph's aggregate result, not a fabricated
//! input parameter or legacy singular return record. Mandatory source/selection
//! replay checks the layout, every fragment, and every load/store; these checks
//! account for the exact declarations and operations at the object boundary.
use abstract_operations::{AbstractFunction, AbstractOperation};
use selected_instructions::{SelectedFunction, SelectedTerminator};
use target_operations::{TargetControlTerminator, TargetFunction, TargetOperation, TargetUnitOperation};

pub(super) fn header(source: &AbstractFunction, target: &TargetFunction, selected: &SelectedFunction) -> bool {
    let Some(result) = source.result.structural() else { return false; };
    let (TargetOperation::ControlGraph(graph), Some(contract)) = (&target.operation, &selected.structural) else { return false; };
    contract.result.as_ref() == Some(result)
        && graph.call_plan.result.is_some()
        && target.scalar_abi.is_none() && target.mixed_structural_scalar_abi.is_none()
        && graph.scalar_parameters.len() == source.parameters.len()
        && graph.parameters.len() == source.structural_parameters.len()
        && graph.scalar_parameters.iter().zip(&source.parameters).all(|(actual, expected)| actual.value == expected.value && actual.scalar_type == expected.scalar_type)
        && contract.parameters.iter().map(|row| &row.semantic).eq(source.structural_parameters.iter())
        && contract.parameters.iter().map(|row| &row.target).eq(graph.parameters.iter())
}

pub(super) fn operation(source: &AbstractOperation, target: &TargetFunction, selected: &SelectedFunction) -> bool {
    let TargetOperation::ControlGraph(graph) = &target.operation else { return false; };
    match source {
        AbstractOperation::EstablishScalarCase { psi_operation, result, result_case, fields } => {
            graph.blocks.iter().flat_map(|block| &block.operations).filter(|row| matches!(row,
                TargetUnitOperation::EstablishScalarCase { psi_operation: retained, result_home, result_case: retained_case, fields: retained_fields }
                    if retained == psi_operation && &result_home.result == result && retained_case == result_case && retained_fields == fields
            )).count() == 1
        }
        AbstractOperation::CallStructural { psi_operation, callee, result, .. } => {
            selected.calls.iter().filter(|row| row.operation == *psi_operation && row.call.callee == *callee
                && row.call.structural_result.as_ref() == Some(result)
                && row.call.result_placement == row.call.call_plan.result && row.call.result_placement.is_some()).count() == 1
        }
        AbstractOperation::ReturnStructural { psi_edge, source, returned_claims, trivial_affine_locals, trivial_affine_discards } => {
            returned_claims.is_empty() && trivial_affine_locals.is_empty() && trivial_affine_discards.is_empty()
                && graph.blocks.iter().filter(|block| matches!(&block.terminator,
                    TargetControlTerminator::ReturnStructural { psi_edge: retained, source: home, cleanup_actions }
                        if retained == psi_edge && home.result.place == *source && cleanup_actions.is_empty())).count() == 1
                && selected.blocks.iter().filter(|block| matches!(&block.terminator,
                    SelectedTerminator::Return { psi_return_edge, instruction }
                        if psi_return_edge == psi_edge && matches!(instruction.kind, selected_instructions::SelectedInstructionKind::ReturnAggregate { .. }))).count() == 1
        }
        _ => false,
    }
}
