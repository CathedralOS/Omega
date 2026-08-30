use super::setup::PreparedScalarLowering;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_special_form(
    function: &AbstractFunction,
    function_result: AbstractResult,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    prepared: &PreparedScalarLowering,
) -> Result<Option<TargetFunction>, LoweringError> {
    let mut shape_cache = prepared.shape_cache.clone();
    let mut active = prepared.active_shapes.clone();
    let call_plan = prepared.call_plan.clone();
    let target_structural_parameters = prepared.target_structural_parameters.clone();
    if let Some(lowered) = structural_call::lower_direct_return(
        function,
        function_result,
        target,
        functions,
        structural_types,
        &call_plan,
        &target_structural_parameters,
        &mut shape_cache,
        &mut active,
    )? {
        return Ok(Some(lowered));
    }

    if let [
        AbstractOperation::BoundaryCall {
            psi_operation,
            result: Some(boundary_result),
            boundary,
            arguments: _,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
        && *result == function_result.value
        && *value == boundary_result.value
        && *scalar_type == boundary_result.scalar_type
        && boundary_result.scalar_type
            == ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is a valid integer type"),
            )
        && cleanup_actions.is_empty()
        && structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    {
        let binding = settlements.get(boundary).cloned().ok_or(
            LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                machine: function.machine,
                operation: *psi_operation,
                boundary: *boundary,
            },
        )?;
        let omega_target_operations::BoundarySettlementRealization::Builtin(
            BoundaryRealization::DirectPortReadU8(realization),
        ) = binding.realization
        else {
            return Err(
                LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            );
        };
        if target.architecture != Architecture::X86_64 {
            return Err(
                LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            );
        }
        return Ok(Some(TargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: TerminalPsiProvenance {
                operations: vec![*psi_operation],
                edges: vec![*psi_edge],
            },
            operation: TargetOperation::ReturnBoundaryPortReadU8 {
                psi_edge: *psi_edge,
                psi_operation: *psi_operation,
                source_value: boundary_result.value,
                boundary: *boundary,
                provider_execution: binding.provider_execution,
                realization,
                arguments: structural_arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
                call_plan,
                structural_parameters: target_structural_parameters,
            },
        }));
    }

    Ok(None)
}
