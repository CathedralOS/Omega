use super::shared::*;

pub(super) fn derive_fixed_integer_scalar_function_abi(
    function: &AbstractFunction,
    target: NativeTarget,
) -> Result<Option<FixedIntegerScalarFunctionAbi>, LoweringError> {
    if !function.published_service_ceiling.is_empty()
        || !function.structural_parameters.is_empty()
        || !function.entry_claims.is_empty()
    {
        return Ok(None);
    }
    let Some(result) = function.result.scalar() else {
        return Ok(None);
    };
    let ScalarType::Integer(result_type) = result.scalar_type else {
        return Ok(None);
    };
    let Some(result_shape) = fixed_native_integer_shape(result_type) else {
        return Ok(None);
    };

    let parameter_types_and_shapes = function
        .parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(scalar_type) = parameter.scalar_type else {
                return None;
            };
            Some((scalar_type, fixed_native_integer_shape(scalar_type)?))
        })
        .collect::<Option<Vec<_>>>();
    let Some(parameter_types_and_shapes) = parameter_types_and_shapes else {
        return Ok(None);
    };
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_types_and_shapes
                .iter()
                .map(|(_, shape)| *shape)
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let result_placement =
        call_plan
            .result
            .clone()
            .ok_or(LoweringError::FixedIntegerScalarAbiPlanMissingResult(
                function.machine,
            ))?;
    if call_plan.parameters.len() != function.parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    let parameters = function
        .parameters
        .iter()
        .zip(parameter_types_and_shapes)
        .zip(&call_plan.parameters)
        .map(|((parameter, (scalar_type, expected_shape)), placement)| {
            if placement.shape != expected_shape {
                return Err(LoweringError::UnsupportedScalarParameterPlacement(
                    parameter.value,
                ));
            }
            Ok(FixedIntegerScalarAbiValue {
                value: parameter.value,
                scalar_type,
                placement: placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if result_placement.shape != result_shape {
        return Err(LoweringError::FixedIntegerScalarAbiPlanMissingResult(
            function.machine,
        ));
    }
    Ok(Some(FixedIntegerScalarFunctionAbi {
        call_plan,
        parameters,
        result: FixedIntegerScalarAbiValue {
            value: result.value,
            scalar_type: result_type,
            placement: result_placement,
        },
    }))
}

pub(super) fn fixed_native_integer_shape(scalar_type: IntegerType) -> Option<ValueShape> {
    if scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
        || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let bytes = scalar_type.bits().div_ceil(8);
    Some(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function() -> AbstractFunction {
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        AbstractFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: vec![AbstractParameter {
                value: ValueId::new(1).unwrap(),
                scalar_type,
            }],
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(2).unwrap(),
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: Vec::new(),
        }
    }

    #[test]
    fn only_complete_service_free_fixed_integer_scalar_signatures_receive_an_abi() {
        let target = NativeTarget::linux_x64();
        assert!(
            derive_fixed_integer_scalar_function_abi(&function(), target)
                .unwrap()
                .is_some()
        );

        let mut serviceful = function();
        serviceful
            .published_service_ceiling
            .push(psi_core::ServiceId::new(1).unwrap());
        assert_eq!(
            derive_fixed_integer_scalar_function_abi(&serviceful, target).unwrap(),
            None
        );

        let mut structural = function();
        structural
            .structural_parameters
            .push(psi_terminal::StructuralParameterDeclaration {
                place: PlaceId::new(1).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                access: psi_terminal::StructuralAccess::Owned,
                qualifications: Vec::new(),
            });
        assert_eq!(
            derive_fixed_integer_scalar_function_abi(&structural, target).unwrap(),
            None
        );

        let mut address = function();
        address.parameters[0].scalar_type = ScalarType::Integer(IntegerType::address(64).unwrap());
        assert_eq!(
            derive_fixed_integer_scalar_function_abi(&address, target).unwrap(),
            None
        );

        let mut boolean = function();
        boolean.result = AbstractFunctionResult::Scalar(AbstractResult {
            value: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        assert_eq!(
            derive_fixed_integer_scalar_function_abi(&boolean, target).unwrap(),
            None
        );
    }
}
