//! Borrow primitive referents from incoming pointers or established local storage.
use super::LiveDefinitions;
use crate::lowering::function_signature::{PreparedFunctionSignature, prepare_function_signature};
use crate::lowering::shared::*;
use target_operations::TargetStructuralArgumentSource;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    types: &StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let (psi_operation, result, callee, values, arguments, claims, requirements, crashes) =
        match operation {
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => (
                *psi_operation,
                Some(*result),
                *callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            ),
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => (
                *psi_operation,
                None,
                *callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            ),
            _ => return Err(invalid()),
        };
    let callee_function = functions
        .get(&callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(callee))?;
    if !claims.is_empty()
        || !requirements.is_empty()
        || !crashes.is_empty()
        || !callee_function.entry_claims.is_empty()
        || !callee_function.published_service_ceiling.is_empty()
        || callee_function.attachment.is_some()
        || values.len() != callee_function.parameters.len()
        || arguments.len() != callee_function.structural_parameters.len()
        || result.map(|result| result.scalar_type)
            != callee_function
                .result
                .scalar()
                .map(|result| result.scalar_type)
        || (result.is_none() && callee_function.result != AbstractFunctionResult::Unit)
    {
        return Err(invalid());
    }
    let signature = prepare_function_signature(callee_function, target, types)?;
    let scalar_arguments = values
        .iter()
        .zip(&signature.scalar_parameters)
        .enumerate()
        .map(|(position, (value, parameter))| {
            let source = super::scalar_sources::source(*value, function, live)?;
            if source.scalar_type() != parameter.scalar_type {
                return Err(LoweringError::ValueTypeMismatch(*value));
            }
            Ok(TargetUnitScalarCallArgument {
                parameter_index: u32::try_from(position).map_err(|_| invalid())?,
                source,
                placement: parameter.placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let target_arguments = arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(&signature.parameters)
        .map(|((argument, declaration), destination)| {
            if super::scalar_arrays::is_owned_parameter(declaration, types) {
                return super::scalar_arrays::argument(
                    argument,
                    declaration,
                    destination,
                    prepared,
                    live,
                    types,
                );
            }
            self::argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(result) = result {
        super::primitive_storage::retain_result(psi_operation, result, live)?;
        operations.push(TargetUnitOperation::StructuralScalarCall {
            psi_operation,
            result,
            callee,
            call_plan: signature.call_plan,
            scalar_arguments,
            arguments: target_arguments,
            claim_transfers: claims.clone(),
            requirement_obligations: requirements.clone(),
            crash_continuations: crashes.clone(),
        });
    } else {
        operations.push(TargetUnitOperation::Call {
            psi_operation,
            callee,
            call_plan: signature.call_plan,
            scalar_arguments,
            arguments: target_arguments,
            claim_transfers: claims.clone(),
            requirement_obligations: requirements.clone(),
            crash_continuations: crashes.clone(),
        });
    }
    provenance.operations.push(psi_operation);
    Ok(())
}

/// Retain the same primitive referent custody independently of the call result.
pub(super) fn argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    if !argument.path.is_empty()
        || argument.access != declaration.access
        || !super::primitive_storage::is_primitive_reference(declaration, types)
    {
        return Err(invalid());
    }
    let (identity, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        let (defining_operation, home_result) = home.operation_result().ok_or_else(invalid)?;
        if !function.operations.iter().any(|operation| {
            matches!(operation,
            AbstractOperation::EstablishPrimitiveLocal { psi_operation, result, .. }
            if *psi_operation == defining_operation && result == home_result)
        }) {
            return Err(invalid());
        }
        (
            home.structural_type(),
            TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: defining_operation,
            },
        )
    } else {
        let source = prepared
            .parameters
            .iter()
            .find(|source| source.place == argument.place)
            .ok_or_else(invalid)?;
        let allowed = match source.access {
            StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow => {
                argument.access == StructuralAccess::WriteOnlyBorrow
            }
            StructuralAccess::Owned => false,
        };
        if !allowed {
            return Err(invalid());
        }
        (source.structural_type, source.placement.clone().into())
    };
    if identity != declaration.structural_type {
        return Err(invalid());
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: identity,
        structural_type: identity,
        shape: destination.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: destination.placement.clone(),
    })
}
