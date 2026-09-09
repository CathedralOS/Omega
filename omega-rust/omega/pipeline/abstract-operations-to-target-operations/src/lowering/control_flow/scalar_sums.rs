//! Fresh scalar sums use the ordinary graph's values, calls and aggregate homes.
use super::LiveDefinitions;
use crate::lowering::shared::*;
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};

pub(in crate::lowering) fn result_layout(
    result: &terminal_psi::StructuralResultDeclaration,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralSum(result.structural_type);
    let declaration = types.get(&result.structural_type).ok_or_else(invalid)?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(invalid());
    };
    if result.multiplicity == StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || cases.iter().flat_map(|case| &case.fields).any(|field| {
            !matches!(field.field_type.scalar_type(), Some(ScalarType::Integer(integer))
                if crate::lowering::scalar_abi::fixed_native_integer_shape(integer).is_some())
        })
    {
        return Err(invalid());
    }
    crate::lowering::structural_layout::structural_sum_layout(
        result.structural_type,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}

fn home(
    operation: OperationId,
    result: &terminal_psi::StructuralOperationResult,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetStructuralHomeRequirement, LoweringError> {
    if !result.claims.is_empty() {
        return Err(LoweringError::UnsupportedStructuralSum(
            result.structural_type,
        ));
    }
    let declaration = terminal_psi::StructuralResultDeclaration {
        place: result.place,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        qualifications: result.qualifications.clone(),
        projected_qualifications: result.projected_qualifications.clone(),
    };
    Ok(TargetStructuralHomeRequirement {
        defining_operation: operation,
        result: result.clone(),
        layout: TargetStructuralHomeLayout::Sum(result_layout(&declaration, types)?),
    })
}

pub(super) fn establish(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::EstablishScalarCase {
        psi_operation,
        result,
        result_case,
        fields,
    } = operation
    else {
        return Err(invalid());
    };
    let result_home = home(*psi_operation, result, types)?;
    let StructuralTypeShape::Sum { cases } = &types
        .get(&result.structural_type)
        .ok_or_else(invalid)?
        .shape
    else {
        return Err(invalid());
    };
    let case = cases
        .iter()
        .find(|case| case.id == *result_case)
        .ok_or_else(invalid)?;
    if fields.len() != case.fields.len() {
        return Err(invalid());
    }
    for (field, declaration) in fields.iter().zip(&case.fields) {
        let source = super::scalar_sources::source(field.value, function, live)?;
        if field.field != declaration.id
            || declaration.field_type.scalar_type() != Some(source.scalar_type())
            || matches!(
                declaration.field_type,
                StructuralFieldType::BoundedInteger(_)
            ) != field.range_obligation.is_some()
        {
            return Err(invalid());
        }
    }
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::EstablishScalarCase {
        psi_operation: *psi_operation,
        result_home,
        result_case: *result_case,
        fields: fields.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::CallStructural {
        psi_operation,
        result,
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence,
    } = operation
    else {
        return Err(invalid());
    };
    let callee_function = functions.get(callee).copied().ok_or_else(invalid)?;
    let callee_result = callee_function.result.structural().ok_or_else(invalid)?;
    let result_home = home(*psi_operation, result, types)?;
    if callee_result.structural_type != result.structural_type
        || callee_result.multiplicity != result.multiplicity
        || !callee_result.qualifications.is_empty()
        || !callee_result.projected_qualifications.is_empty()
        || !callee_function.entry_claims.is_empty()
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
        || !selected_evidence.is_empty()
        || arguments.len() != callee_function.parameters.len()
        || structural_arguments.len() != callee_function.structural_parameters.len()
    {
        return Err(invalid());
    }
    let signature = crate::lowering::function_signature::prepare_function_signature(
        callee_function,
        target,
        types,
    )?;
    let scalar_arguments = arguments
        .iter()
        .zip(&signature.scalar_parameters)
        .enumerate()
        .map(|(position, (value, parameter))| {
            let source = super::scalar_sources::source(*value, function, live)?;
            if source.scalar_type() != parameter.scalar_type {
                return Err(invalid());
            }
            Ok(TargetUnitScalarCallArgument {
                parameter_index: u32::try_from(position).map_err(|_| invalid())?,
                source,
                placement: parameter.placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let target_arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(&signature.parameters)
        .map(|((argument, declaration), destination)| {
            if !argument.path.is_empty()
                || argument.access != declaration.access
                || !crate::lowering::scalar::byte_views::is_byte_parameter(declaration, types)
            {
                return Err(invalid());
            }
            let (source_type, source_access, source) = if let Some(parameter) = prepared
                .parameters
                .iter()
                .find(|parameter| parameter.place == argument.place)
            {
                (
                    parameter.structural_type,
                    parameter.access,
                    parameter.placement.clone().into(),
                )
            } else if live.block_views.contains(&argument.place) {
                let block = function
                    .block_entries
                    .iter()
                    .find(|block| {
                        block
                            .structural_parameters
                            .iter()
                            .any(|parameter| parameter.place == argument.place)
                    })
                    .ok_or_else(invalid)?;
                let parameter = block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                    .ok_or_else(invalid)?;
                (
                    parameter.structural_type,
                    parameter.access,
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: block.block,
                        place: parameter.place,
                    },
                )
            } else {
                return Err(invalid());
            };
            if source_type != declaration.structural_type || source_access != argument.access {
                return Err(invalid());
            }
            Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: Vec::new(),
                root_structural_type: source_type,
                structural_type: source_type,
                shape: destination.shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                source,
                destination: destination.placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::StructuralResultCall {
        psi_operation: *psi_operation,
        result: result.clone(),
        callee: *callee,
        callee_result: callee_result.clone(),
        result_home: Some(result_home),
        call_plan: signature.call_plan,
        scalar_arguments,
        arguments: target_arguments,
        claim_transfers: claim_transfers.clone(),
        returned_claim_transfers: returned_claim_transfers.clone(),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
