//! Installed-provider admission, ABI projection, and claim-transfer custody.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, BoundaryMachineId, CallSignature,
    CallingPolicy, InstalledProviderCallEvidence, KnownUnitInteger, LoweringError, MachineId,
    NativeTarget, OperationId, PlaceId, ScalarType, StructuralTypeDeclaration, StructuralTypeId,
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarCallArgument, TerminalPsiProvenance, ValueId, ValueShape, evaluate_call_plan,
    fixed_native_integer_shape, structural_shape,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn try_lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<bool, LoweringError> {
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result,
        boundary,
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
    } = operation
    else {
        unreachable!("installed-provider routing admits only boundary calls")
    };
    let Some(installed) = installed_calls.get(&(function.machine, *psi_operation, *boundary))
    else {
        return Ok(false);
    };
    let callee = functions
        .get(&installed.provider.candidate)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(
            installed.provider.candidate,
        ))?;
    let declaration = boundary_machines
        .get(boundary)
        .copied()
        .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
    let has_scalar_argument = !arguments.is_empty();
    if !result.is_unit()
        || !matches!(installed.result, terminal_psi::OperationResult::Unit)
        || callee.result != AbstractFunctionResult::Unit
        || arguments.len() != callee.parameters.len()
        || arguments.len() != declaration.scalar_parameters.len()
        || arguments.len() != installed.scalar_arguments.len()
        || installed.scalar_arguments != *arguments
        || arguments.len() > 1
        || (has_scalar_argument
            && (!callee.structural_parameters.is_empty() || !structural_arguments.is_empty()))
        || structural_arguments.len() != callee.structural_parameters.len()
        || declaration.structural_parameters.len() != callee.structural_parameters.len()
        || installed.provider.signature.parameters.len() != callee.structural_parameters.len()
    {
        return Err(LoweringError::InstalledProviderCallShapeMismatch {
            machine: function.machine,
            operation: *psi_operation,
            boundary: *boundary,
        });
    }
    let scalar_shapes = callee
        .parameters
        .iter()
        .zip(&declaration.scalar_parameters)
        .map(|(callee_parameter, boundary_parameter)| {
            let (ScalarType::Integer(callee_type), ScalarType::Integer(boundary_type)) =
                (callee_parameter.scalar_type, *boundary_parameter)
            else {
                return Err(LoweringError::InstalledProviderCallShapeMismatch {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                });
            };
            if callee_type != boundary_type
                || callee_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || callee_type.sign() != semantic_vocabulary::IntegerSign::Signed
                || callee_type.bits() != 32
            {
                return Err(LoweringError::InstalledProviderCallShapeMismatch {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                });
            }
            fixed_native_integer_shape(callee_type).ok_or(
                LoweringError::InstalledProviderCallShapeMismatch {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callee_shapes = callee
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                shape_cache,
                active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .chain(&callee_shapes)
                .copied()
                .collect(),
            result: None,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let target_scalar_arguments = arguments
        .iter()
        .zip(&callee.parameters)
        .zip(&scalar_shapes)
        .zip(&callee_plan.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), shape), placement))| {
                let known = scalar_values
                    .get(source_value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*source_value))?;
                let ScalarType::Integer(parameter_type) = parameter.scalar_type else {
                    unreachable!("installed provider scalar type was checked above")
                };
                if known.scalar_type() != parameter_type || placement.shape != *shape {
                    return Err(LoweringError::CallArgumentTypeMismatch {
                        callee: callee.machine,
                        argument: *source_value,
                    });
                }
                Ok(TargetUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| {
                        LoweringError::InstalledProviderCallShapeMismatch {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        }
                    })?,
                    source: known.into_target_source(*source_value),
                    placement: placement.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let mut target_arguments = Vec::with_capacity(structural_arguments.len());
    for (index, (((argument, boundary_parameter), callee_parameter), signature)) in
        structural_arguments
            .iter()
            .zip(&declaration.structural_parameters)
            .zip(&callee.structural_parameters)
            .zip(&installed.provider.signature.parameters)
            .enumerate()
    {
        let source = parameters_by_place.get(&argument.place).copied().ok_or(
            LoweringError::UnknownStructuralArgumentPlace {
                machine: function.machine,
                place: argument.place,
            },
        )?;
        let caller_parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                machine: function.machine,
                place: argument.place,
            })?;
        if !argument.path.is_empty()
            || argument.access != terminal_psi::StructuralAccess::Owned
            || source.access != terminal_psi::StructuralAccess::Owned
            || boundary_parameter.access != terminal_psi::StructuralAccess::Owned
            || callee_parameter.access != terminal_psi::StructuralAccess::Owned
            || boundary_parameter.position != index as u32
            || callee_parameter.position != index as u32
            || signature.position != index as u32
            || boundary_parameter.is_self
            || callee_parameter.is_self
            || signature.is_self
            || source.structural_type != boundary_parameter.structural_type
            || source.structural_type != callee_parameter.structural_type
            || source.structural_type != signature.structural_type
            || source.multiplicity != boundary_parameter.multiplicity
            || source.multiplicity != callee_parameter.multiplicity
            || source.multiplicity != signature.multiplicity
            || source.access != signature.access
            || source.structural_type != callee.structural_parameters[index].structural_type
            || source.placement.shape != callee_shapes[index]
            || source.placement.shape != callee_plan.parameters[scalar_shapes.len() + index].shape
            || caller_parameter.qualifications.iter().any(|qualification| {
                !boundary_parameter.qualifications.contains(qualification)
                    || !callee_parameter.qualifications.contains(qualification)
                    || !signature.qualifications.contains(qualification)
            })
            || caller_parameter.qualifications != boundary_parameter.qualifications
            || caller_parameter.qualifications != callee_parameter.qualifications
            || caller_parameter.qualifications != signature.qualifications
        {
            return Err(LoweringError::InstalledProviderCallShapeMismatch {
                machine: function.machine,
                operation: *psi_operation,
                boundary: *boundary,
            });
        }
        target_arguments.push(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: Vec::new(),
            root_structural_type: source.structural_type,
            structural_type: source.structural_type,
            shape: source.shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            source: source.placement.clone(),
            destination: callee_plan.parameters[scalar_shapes.len() + index].clone(),
        });
    }
    let claim_transfers = completion_receipts
        .iter()
        .map(|receipt| terminal_psi::ClaimTransfer {
            claim: receipt.claim,
            argument_index: receipt.argument_index,
        })
        .collect::<Vec<_>>();
    if completion_receipts.len() != callee.entry_claims.len()
        || callee.entry_claims.iter().any(|entry| {
            let Some(index) = callee
                .structural_parameters
                .iter()
                .position(|parameter| parameter.place == entry.input)
            else {
                return true;
            };
            let Some(receipt) = completion_receipts
                .iter()
                .find(|receipt| receipt.argument_index as usize == index)
            else {
                return true;
            };
            let Some(source) = completion_claim_sources
                .iter()
                .find(|source| source.claim == receipt.claim)
            else {
                return true;
            };
            source.entry.as_ref().is_none_or(|source| {
                source.input != structural_arguments[index].place || source.path != entry.path
            })
        })
    {
        return Err(LoweringError::InstalledProviderClaimTransferMismatch {
            machine: function.machine,
            operation: *psi_operation,
            boundary: *boundary,
        });
    }
    operations.push(TargetUnitOperation::InstalledProviderCall {
        psi_operation: *psi_operation,
        boundary: *boundary,
        provider: installed.provider.clone(),
        call_plan: callee_plan,
        scalar_arguments: target_scalar_arguments,
        source_arguments: structural_arguments.clone(),
        arguments: target_arguments,
        claim_transfers,
        completion_claim_sources: completion_claim_sources.clone(),
        completion_receipts: completion_receipts.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(true)
}
