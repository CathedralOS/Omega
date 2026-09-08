//! Ordered scalar calls forwarding immutable view references through the ordinary graph.
use super::super::{KnownInteger, KnownScalar, scalar_parameter_location, scalar_shape};
use crate::lowering::shared::*;
use crate::lowering::structural_signature::StructuralCallSignature;

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering::scalar) fn lower(
    operation: &AbstractOperation,
    machine: MachineId,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_parameters: &[TargetStructuralParameter],
    values: &BTreeMap<ValueId, KnownScalar>,
) -> Result<KnownScalar, LoweringError> {
    let AbstractOperation::CallStructuralScalar {
        psi_operation,
        result,
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        return Err(LoweringError::UnsupportedOperationInScalarFunction(machine));
    };
    let caller = functions
        .get(&machine)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(machine))?;
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let callee_result = callee_function
        .result
        .scalar()
        .ok_or(LoweringError::FunctionResultKindMismatch(*callee))?;
    let ScalarType::Integer(integer_type) = result.scalar_type else {
        return Err(LoweringError::UnsupportedOperationInScalarFunction(machine));
    };
    if result.scalar_type != callee_result.scalar_type
        || arguments.len() != callee_function.parameters.len()
        || structural_arguments.is_empty()
        || structural_arguments.len() != callee_function.structural_parameters.len()
        || !claim_transfers.is_empty()
    {
        return Err(LoweringError::UnsupportedOperationInScalarFunction(machine));
    }
    let scalar_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
        .collect::<Result<Vec<_>, _>>()?;
    let signature = StructuralCallSignature::derive(
        &scalar_shapes,
        &callee_function.structural_parameters,
        Some(scalar_shape(result.value, result.scalar_type, false)?),
        structural_types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    let call_plan = signature.plan(target)?;
    let scalar_arguments = arguments
        .iter()
        .zip(&callee_function.parameters)
        .zip(&call_plan.parameters)
        .map(|((argument, parameter), placement)| {
            let expression = values
                .get(argument)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*argument))?
                .into_expression(*argument)?;
            if expression.scalar_type() != parameter.scalar_type {
                return Err(LoweringError::CallArgumentTypeMismatch {
                    callee: *callee,
                    argument: *argument,
                });
            }
            Ok(TargetCallArgument {
                scalar_type: parameter.scalar_type,
                location: scalar_parameter_location(parameter, placement)?,
                expression,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let structural_arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(signature.structural_shapes())
        .zip(call_plan.parameters.iter().skip(arguments.len()))
        .map(
            |(((argument, destination_parameter), shape), destination)| {
                let source = structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                    .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                        machine,
                        place: argument.place,
                    })?;
                let source_parameter = caller
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                    .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                        machine,
                        place: argument.place,
                    })?;
                if !argument.path.is_empty()
                    || !shared_view(source_parameter, structural_types)
                    || !shared_view(destination_parameter, structural_types)
                    || source_parameter.structural_type != destination_parameter.structural_type
                    || source.structural_type != source_parameter.structural_type
                    || source.access != source_parameter.access
                    || source.multiplicity != source_parameter.multiplicity
                    || !source.projected_qualifications.is_empty()
                    || source.shape != *shape
                    || argument.access != destination_parameter.access
                {
                    return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                        callee: *callee,
                        place: argument.place,
                    });
                }
                Ok(TargetStructuralArgument {
                    place: argument.place,
                    access: argument.access,
                    path: Vec::new(),
                    root_structural_type: source.structural_type,
                    structural_type: source.structural_type,
                    shape: *shape,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    source: source.placement.clone(),
                    destination: destination.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    Ok(KnownScalar::Integer {
        scalar_type: integer_type,
        value: KnownInteger::Runtime(TargetIntegerExpression::StructuralCall {
            psi_operation: *psi_operation,
            source_value: result.value,
            callee: *callee,
            arguments: scalar_arguments,
            structural_arguments,
            call_plan,
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        }),
    })
}

fn shared_view(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    !parameter.is_self
        && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
        && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && declarations
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                declaration.shape
                    == terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                    )
            })
}
