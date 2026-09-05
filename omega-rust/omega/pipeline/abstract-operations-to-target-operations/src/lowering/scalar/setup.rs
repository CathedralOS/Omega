use super::*;

pub(in crate::lowering) struct PreparedScalarLowering {
    pub(super) values: BTreeMap<ValueId, KnownScalar>,
    pub(in crate::lowering) call_plan: CallPlan,
    pub(in crate::lowering) target_structural_parameters: Vec<TargetStructuralParameter>,
    pub(super) shape_cache: BTreeMap<StructuralTypeId, ValueShape>,
    pub(super) active_shapes: BTreeSet<StructuralTypeId>,
}

pub(in crate::lowering) fn prepare_scalar_lowering(
    function: &AbstractFunction,
    function_result: AbstractResult,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<PreparedScalarLowering, LoweringError> {
    let mut values = BTreeMap::new();
    let scalar_parameter_shapes = function
        .parameters
        .iter()
        .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
        .collect::<Result<Vec<_>, _>>()?;
    let boundary_custody_places = function
        .operations
        .iter()
        .filter_map(|operation| {
            let AbstractOperation::BoundaryCall {
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            } = operation
            else {
                return None;
            };
            Some(structural_arguments.iter().enumerate().filter_map(
                |(argument_index, argument)| {
                    let receipt = completion_receipts.iter().find(|receipt| {
                        usize::try_from(receipt.argument_index) == Ok(argument_index)
                    })?;
                    completion_claim_sources
                        .iter()
                        .any(|source| {
                            source.claim == receipt.claim
                                && source.entry.as_ref().is_some_and(|entry| {
                                    entry.input == argument.place && entry.path == argument.path
                                })
                        })
                        .then_some(argument.place)
                },
            ))
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let structural_parameter_shapes = function
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            // Verified qualifications remain in the exact completion-custody
            // source and do not alter the structural ABI shape. Linear inputs
            // enter this scalar lane only when that same boundary call carries
            // their claim toward provider custody.
            let carries_boundary_custody = boundary_custody_places.contains(&parameter.place);
            let direct_borrowed_self = parameter.is_self
                && matches!(
                    parameter.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                )
                && matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                )
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty();
            let custody_bearing_parameter = !parameter.is_self
                && matches!(
                    parameter.multiplicity,
                    terminal_psi::StructuralMultiplicity::Affine
                        | terminal_psi::StructuralMultiplicity::Linear
                )
                && ((parameter.qualifications.is_empty()
                    && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear)
                    || carries_boundary_custody);
            if usize::try_from(parameter.position) != Ok(position)
                || (!direct_borrowed_self && !custody_bearing_parameter)
            {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            let shape = structural_shape(
                parameter.structural_type,
                structural_types,
                &mut shape_cache,
                &mut active,
            )?;
            Ok(
                if matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::MutableBorrow
                ) {
                    ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
                } else {
                    shape
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: scalar_parameter_shapes
            .iter()
            .copied()
            .chain(structural_parameter_shapes.iter().copied())
            .collect(),
        result: Some(scalar_shape(
            function_result.value,
            function_result.scalar_type,
            false,
        )?),
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len()
        != function.parameters.len() + function.structural_parameters.len()
    {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.parameters.len() + function.structural_parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    for (parameter_index, (parameter, placement)) in function
        .parameters
        .iter()
        .zip(&call_plan.parameters[..function.parameters.len()])
        .enumerate()
    {
        let location = scalar_parameter_location(parameter, placement)?;
        let value = match parameter.scalar_type {
            ScalarType::Boolean => {
                KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location,
                })
            }
            ScalarType::Integer(scalar_type) => KnownScalar::Integer {
                scalar_type,
                value: KnownInteger::Runtime(TargetIntegerExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location,
                }),
            },
            ScalarType::IeeeFloat(_) => {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
        };
        insert_value(&mut values, parameter.value, value)?;
    }
    let target_structural_parameters = function
        .structural_parameters
        .iter()
        .zip(structural_parameter_shapes)
        .zip(&call_plan.parameters[function.parameters.len()..])
        .map(
            |((parameter, shape), placement)| TargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                projected_qualifications: parameter.projected_qualifications.clone(),
                shape,
                placement: placement.clone(),
            },
        )
        .collect::<Vec<_>>();

    Ok(PreparedScalarLowering {
        values,
        call_plan,
        target_structural_parameters,
        shape_cache,
        active_shapes: active,
    })
}
