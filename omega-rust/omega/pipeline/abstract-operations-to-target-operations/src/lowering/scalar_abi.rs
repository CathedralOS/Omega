use super::shared::*;

pub(super) fn derive_mixed_structural_scalar_function_abi(
    function: &AbstractFunction,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Option<MixedStructuralScalarFunctionAbi>, LoweringError> {
    if function.structural_parameters.is_empty()
        || !function.published_service_ceiling.is_empty()
        || !function.entry_claims.is_empty()
        || function.operations.iter().any(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
    {
        return Ok(None);
    }
    let Some(result) = function.result.scalar() else {
        return Ok(None);
    };
    let result_supported = match result.scalar_type {
        ScalarType::Boolean => true,
        ScalarType::Integer(integer) => fixed_native_integer_shape(integer).is_some(),
        ScalarType::IeeeFloat(_) => false,
    };
    if !result_supported
        || function.parameters.iter().any(|parameter| {
            parameter.scalar_type != ScalarType::Boolean
                && !matches!(parameter.scalar_type, ScalarType::Integer(integer)
                if fixed_native_integer_shape(integer).is_some())
        })
    {
        return Ok(None);
    }
    let Ok(prepared) =
        super::scalar::setup::prepare_scalar_lowering(function, result, target, structural_types)
    else {
        // ABI publication is a derived claim, not a new lowering entrance.
        // The ordinary function lowerer retains authority for diagnostics.
        return Ok(None);
    };
    let scalar_count = function.parameters.len();
    let result_placement = prepared.call_plan.result.clone().ok_or(
        LoweringError::FixedIntegerScalarAbiPlanMissingResult(function.machine),
    )?;
    let scalar_parameters = function
        .parameters
        .iter()
        .zip(&prepared.call_plan.parameters[..scalar_count])
        .map(|(parameter, placement)| ScalarAbiValue {
            value: parameter.value,
            scalar_type: parameter.scalar_type,
            placement: placement.clone(),
        })
        .collect();
    Ok(Some(MixedStructuralScalarFunctionAbi {
        call_plan: prepared.call_plan,
        scalar_parameters,
        structural_parameters: prepared.target_structural_parameters,
        result: ScalarAbiValue {
            value: result.value,
            scalar_type: result.scalar_type,
            placement: result_placement,
        },
    }))
}

pub(super) fn derive_fixed_scalar_function_abi(
    function: &AbstractFunction,
    target: NativeTarget,
) -> Result<Option<ScalarFunctionAbi>, LoweringError> {
    if !function.published_service_ceiling.is_empty()
        || !function.structural_parameters.is_empty()
        || !function.entry_claims.is_empty()
        || function.operations.iter().any(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
    {
        return Ok(None);
    }
    let Some(result) = function.result.scalar() else {
        return Ok(None);
    };
    let Some(result_shape) = fixed_native_scalar_shape(result.scalar_type) else {
        return Ok(None);
    };

    let parameter_types_and_shapes = function
        .parameters
        .iter()
        .map(|parameter| {
            let scalar_type = parameter.scalar_type;
            Some((scalar_type, fixed_native_scalar_shape(scalar_type)?))
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
            Ok(ScalarAbiValue {
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
    Ok(Some(ScalarFunctionAbi {
        call_plan,
        parameters,
        result: ScalarAbiValue {
            value: result.value,
            scalar_type: result.scalar_type,
            placement: result_placement,
        },
    }))
}

pub(super) fn fixed_native_scalar_shape(scalar_type: ScalarType) -> Option<ValueShape> {
    match scalar_type {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) => fixed_native_integer_shape(integer),
        ScalarType::IeeeFloat(_) => None,
    }
}
pub(super) fn fixed_native_integer_shape(scalar_type: IntegerType) -> Option<ValueShape> {
    if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
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

    #[test]
    fn boolean_parameters_retain_semantic_type_and_one_byte_abi_placement() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::windows_x64(),
            NativeTarget::macos_arm64(),
        ] {
            let mut source = function();
            source.parameters[0].scalar_type = ScalarType::Boolean;
            let abi = derive_fixed_scalar_function_abi(&source, target)
                .unwrap()
                .unwrap();
            assert_eq!(abi.parameters[0].value, source.parameters[0].value);
            assert_eq!(abi.parameters[0].scalar_type, ScalarType::Boolean);
            assert_eq!(abi.parameters[0].placement.shape, ValueShape::integer(1, 1));
            assert_eq!(abi.parameters[0].placement, abi.call_plan.parameters[0]);
            assert_eq!(
                abi.result.scalar_type,
                source.result.scalar().unwrap().scalar_type
            );
        }
    }

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
            derive_fixed_scalar_function_abi(&function(), target)
                .unwrap()
                .is_some()
        );

        let mut serviceful = function();
        serviceful
            .published_service_ceiling
            .push(semantic_vocabulary::ServiceId::new(1).unwrap());
        assert_eq!(
            derive_fixed_scalar_function_abi(&serviceful, target).unwrap(),
            None
        );

        let mut structural = function();
        structural
            .structural_parameters
            .push(terminal_psi::StructuralParameterDeclaration {
                place: PlaceId::new(1).unwrap(),
                position: 0,
                is_self: false,
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                access: terminal_psi::StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        assert_eq!(
            derive_fixed_scalar_function_abi(&structural, target).unwrap(),
            None
        );

        let mut address = function();
        address.parameters[0].scalar_type = ScalarType::Integer(IntegerType::address(64).unwrap());
        assert_eq!(
            derive_fixed_scalar_function_abi(&address, target).unwrap(),
            None
        );

        let mut boolean = function();
        boolean.result = AbstractFunctionResult::Scalar(AbstractResult {
            value: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        let abi = derive_fixed_scalar_function_abi(&boolean, target)
            .unwrap()
            .unwrap();
        assert_eq!(abi.result.scalar_type, ScalarType::Boolean);
        assert_eq!(abi.result.placement.shape, ValueShape::integer(1, 1));
    }

    #[test]
    fn mixed_abi_retains_scalar_prefix_and_structural_suffix_from_abstract_authority() {
        let mut mixed = function();
        let structural_type = StructuralTypeId::new(1).unwrap();
        mixed.structural_parameters = vec![terminal_psi::StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        let declaration = StructuralTypeDeclaration {
            id: structural_type,
            identity: "CounterState".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    )),
                }],
            },
        };
        let declarations = BTreeMap::from([(structural_type, &declaration)]);

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let abi = derive_mixed_structural_scalar_function_abi(&mixed, target, &declarations)
                .unwrap()
                .expect("fixed scalar plus structural signature publishes mixed ABI");
            assert_eq!(abi.call_plan.parameters.len(), 2);
            assert_eq!(abi.scalar_parameters.len(), 1);
            assert_eq!(abi.structural_parameters.len(), 1);
            assert_eq!(abi.scalar_parameters[0].value, mixed.parameters[0].value);
            assert_eq!(
                abi.scalar_parameters[0].placement,
                abi.call_plan.parameters[0]
            );
            assert_eq!(
                abi.structural_parameters[0].placement,
                abi.call_plan.parameters[1]
            );
            assert_eq!(abi.result.value, mixed.result.scalar().unwrap().value);
            assert_eq!(abi.call_plan.result.as_ref(), Some(&abi.result.placement));
        }
    }

    #[test]
    fn mixed_abi_retains_boolean_prefix_and_two_independent_shared_views() {
        let mut mixed = function();
        mixed.parameters[0].scalar_type = ScalarType::Boolean;
        mixed.parameters.push(AbstractParameter {
            value: ValueId::new(3).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        });
        let structural_type = StructuralTypeId::new(1).unwrap();
        mixed.structural_parameters = (0..2)
            .map(|position| terminal_psi::StructuralParameterDeclaration {
                place: PlaceId::new(u64::from(position) + 10).unwrap(),
                position,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            })
            .collect();
        let declaration = StructuralTypeDeclaration {
            id: structural_type,
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        };
        let declarations = BTreeMap::from([(structural_type, &declaration)]);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::windows_x64(),
            NativeTarget::macos_arm64(),
        ] {
            let abi = derive_mixed_structural_scalar_function_abi(&mixed, target, &declarations)
                .unwrap()
                .unwrap();
            assert_eq!(abi.call_plan.parameters.len(), 4);
            assert_eq!(abi.scalar_parameters[0].scalar_type, ScalarType::Boolean);
            assert_eq!(
                abi.scalar_parameters[0].placement.shape,
                ValueShape::integer(1, 1)
            );
            for (position, parameter) in abi.structural_parameters.iter().enumerate() {
                assert_eq!(parameter.place, mixed.structural_parameters[position].place);
                assert_eq!(parameter.placement, abi.call_plan.parameters[position + 2]);
                assert_eq!(parameter.shape, ValueShape::borrowed_reference(16, 8));
            }
            let mut address = mixed.clone();
            address.parameters[0].scalar_type =
                ScalarType::Integer(IntegerType::address(64).unwrap());
            assert!(
                derive_mixed_structural_scalar_function_abi(&address, target, &declarations)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn mixed_abi_admits_a_structural_only_parameter_suffix() {
        let mut structural_only = function();
        structural_only.parameters.clear();
        let structural_type = StructuralTypeId::new(1).unwrap();
        structural_only.structural_parameters =
            vec![terminal_psi::StructuralParameterDeclaration {
                place: PlaceId::new(1).unwrap(),
                position: 0,
                is_self: true,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }];
        let declaration = StructuralTypeDeclaration {
            id: structural_type,
            identity: "CounterState".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    )),
                }],
            },
        };
        let declarations = BTreeMap::from([(structural_type, &declaration)]);

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let abi = derive_mixed_structural_scalar_function_abi(
                &structural_only,
                target,
                &declarations,
            )
            .unwrap()
            .expect("structural-only scalar-result signature publishes mixed ABI");
            assert!(abi.scalar_parameters.is_empty());
            assert_eq!(abi.structural_parameters.len(), 1);
            assert_eq!(abi.call_plan.parameters.len(), 1);
            assert_eq!(
                abi.structural_parameters[0].shape.class,
                calling_conventions::ValueClass::BorrowedReference
            );
            assert_eq!(
                abi.structural_parameters[0].placement,
                abi.call_plan.parameters[0]
            );
        }
    }
}
