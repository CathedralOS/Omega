//! Publication rejoins graph results to operation homes or incoming ABI
//! parameters, including scalar results of aggregate-bearing calls. Mandatory source/selection
//! replay checks the layout, every fragment, and every load/store; these checks
//! account for the exact declarations and operations at the object boundary.
use abstract_operations::{AbstractFunction, AbstractOperation};
use selected_instructions::{SelectedFunction, SelectedTerminator};
use target_operations::{TargetControlTerminator, TargetFunction, TargetUnitOperation};

pub(super) fn header(
    source: &AbstractFunction,
    target: &TargetFunction,
    selected: &SelectedFunction,
) -> bool {
    let Some(contract) = &selected.structural else {
        return false;
    };
    let graph = &target.graph;
    let result_matches = match &source.result {
        abstract_operations::AbstractFunctionResult::Structural(result) => {
            contract.result.as_ref() == Some(result)
        }
        abstract_operations::AbstractFunctionResult::Scalar(_) => contract.result.is_none(),
        _ => false,
    };
    result_matches
        && graph.call_plan.result.is_some()
        && target.scalar_abi.is_none()
        && target.mixed_structural_scalar_abi.is_none()
        && graph.scalar_parameters.len() == source.parameters.len()
        && graph.parameters.len() == source.structural_parameters.len()
        && graph
            .scalar_parameters
            .iter()
            .zip(&source.parameters)
            .all(|(actual, expected)| {
                actual.value == expected.value && actual.scalar_type == expected.scalar_type
            })
        && contract
            .parameters
            .iter()
            .map(|row| &row.semantic)
            .eq(source.structural_parameters.iter())
        && contract
            .parameters
            .iter()
            .map(|row| &row.target)
            .eq(graph.parameters.iter())
}

pub(super) fn operation(
    source: &AbstractOperation,
    target: &TargetFunction,
    selected: &SelectedFunction,
) -> bool {
    let graph = &target.graph;
    match source {
        AbstractOperation::StructuralCaseMembership { psi_operation, result, source, case } => {
            result.scalar_type == semantic_vocabulary::ScalarType::Boolean
                && graph.blocks.iter().flat_map(|block| &block.operations).filter(|row| matches!(row,
                    TargetUnitOperation::StructuralCaseMembership { psi_operation: retained, result: retained_result, source: retained_source, case: retained_case, .. }
                    if retained == psi_operation && retained_result == result && retained_source == source && retained_case == case
                )).count() == 1
                && selected.blocks.iter().flat_map(|block| &block.instructions).filter(|row|
                    row.provenance.operations == [*psi_operation] && row.provenance.values == [result.value]
                        && matches!(row.kind, selected_instructions::SelectedInstructionKind::Load32 { byte_offset: 0 }
                            | selected_instructions::SelectedInstructionKind::ZeroExtendU32)
                ).count() == 1
        }
        AbstractOperation::EstablishRecord { psi_operation, result, fields } => {
            graph.blocks.iter().flat_map(|block| &block.operations).filter(|row| matches!(row,
                TargetUnitOperation::EstablishRecord { psi_operation: retained, result_home, fields: retained_fields }
                if psi_operation == retained && result_home.operation_result() == Some((*psi_operation, result)) && fields == retained_fields)).count() == 1
        }
        AbstractOperation::EstablishScalarArray { psi_operation, result, elements } => {
            graph.blocks.iter().flat_map(|block| &block.operations).filter(|row| matches!(row,
                TargetUnitOperation::EstablishScalarArray { psi_operation: retained, result_home, elements: retained_elements }
                    if retained == psi_operation && result_home.operation_result() == Some((*psi_operation, result)) && retained_elements == elements
            )).count() == 1
        }
        AbstractOperation::EstablishScalarCase { psi_operation, result, result_case, fields } => {
            graph.blocks.iter().flat_map(|block| &block.operations).filter(|row| matches!(row,
                TargetUnitOperation::EstablishScalarCase { psi_operation: retained, result_home, result_case: retained_case, fields: retained_fields }
                    if retained == psi_operation && result_home.operation_result() == Some((*psi_operation, result)) && retained_case == result_case && retained_fields == fields
            )).count() == 1
        }
        AbstractOperation::CallStructural { psi_operation, callee, result, .. } => {
            selected.calls.iter().filter(|row| row.operation == *psi_operation && row.call.callee == *callee
                && row.call.structural_result.as_ref() == Some(result)
                && row.call.result_placement == row.call.call_plan.result && row.call.result_placement.is_some()).count() == 1
        }
        AbstractOperation::ReturnStructural { psi_edge, source, returned_claims, trivial_affine_locals, trivial_affine_discards } => {
            returned_claims.is_empty() && trivial_affine_locals.is_empty()
                && graph.blocks.iter().filter(|block| matches!(&block.terminator,
                    TargetControlTerminator::ReturnStructural { psi_edge: retained, source: home, cleanup_actions }
                        if retained == psi_edge && match home {
                            target_operations::TargetStructuralReturnSource::Home(home) => home.place() == *source,
                            target_operations::TargetStructuralReturnSource::Parameter(parameter) => parameter.place == *source,
                        } && super::control_flow::cleanup_matches(cleanup_actions, trivial_affine_discards))).count() == 1
                && selected.blocks.iter().filter(|block| matches!(&block.terminator,
                    SelectedTerminator::Return { psi_return_edge, instruction }
                        if psi_return_edge == psi_edge && (matches!(instruction.kind, selected_instructions::SelectedInstructionKind::ReturnAggregate { .. } | selected_instructions::SelectedInstructionKind::ReturnScalar)
                            || (instruction.kind == selected_instructions::SelectedInstructionKind::ReturnUnit
                                && instruction.operands.is_empty()
                                && selected.structural.as_ref().is_some_and(|contract| contract.result.is_some())
                                && graph.call_plan.result.as_ref().is_some_and(|placement|
                                    (placement.shape == calling_conventions::ValueShape::integer(0, 1)
                                        && placement.locations.is_empty())
                                    // AAPCS returns an indirect aggregate through the
                                    // retained destination, with no result register.
                                    // Mandatory source replay validates the CallPlan
                                    // and every destination write before this join.
                                    || (graph.call_plan.policy == calling_conventions::CallingPolicy::Aapcs64
                                        && placement.shape.class == calling_conventions::ValueClass::Integer
                                        && matches!(placement.locations.as_slice(),
                                            [calling_conventions::ValueLocation::Indirect {
                                                pointer: calling_conventions::IndirectPointerLocation::Register(calling_conventions::MachineRegister::Aarch64X(8)),
                                                copy_stack_byte_offset: None,
                                                byte_size,
                                                alignment,
                                            }] if *byte_size == placement.shape.byte_size
                                                && *alignment == placement.shape.alignment))))))).count() == 1
        }
        _ => false,
    }
}
