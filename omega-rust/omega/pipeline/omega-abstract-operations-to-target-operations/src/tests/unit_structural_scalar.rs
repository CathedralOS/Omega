use super::*;

fn direct_projected_integer_plan() -> AbstractOperationPlan {
    let caller = MachineId::new(80).unwrap();
    let realization = MachineId::new(81).unwrap();
    let outer_type = StructuralTypeId::new(80).unwrap();
    let carrier_type = StructuralTypeId::new(81).unwrap();
    let outer_field = StructuralFieldId::new(80).unwrap();
    let value_field = StructuralFieldId::new(81).unwrap();
    let caller_place = PlaceId::new(80).unwrap();
    let realization_place = PlaceId::new(81).unwrap();
    let literal = ValueId::new(80).unwrap();
    let call_result = ValueId::new(81).unwrap();
    let realization_result = ValueId::new(82).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let caller_parameter = StructuralParameterDeclaration {
        place: caller_place,
        position: 0,
        is_self: true,
        structural_type: outer_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let realization_parameter = StructuralParameterDeclaration {
        place: realization_place,
        position: 0,
        is_self: true,
        structural_type: carrier_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let block_entry = |machine: MachineId| AbstractBlockEntry {
        block: BlockId::new(machine.get()).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: outer_type,
                identity: "DirectOwner".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: outer_field,
                        identity: "item".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(carrier_type),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: carrier_type,
                identity: "DirectCarrier".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: value_field,
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer_type)),
                    }],
                },
            },
        ],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: Some(outer_type),
                entry: BlockId::new(caller.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![caller_parameter.clone()],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(caller)],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(80).unwrap(),
                        result: literal,
                        scalar_type: ScalarType::Integer(integer_type),
                        value: IntegerValue::Signed(17),
                    },
                    AbstractOperation::StructuralScalarFieldStore {
                        psi_operation: OperationId::new(81).unwrap(),
                        destination: caller_parameter,
                        path: vec![StructuralPathSegment::Field("item".into())],
                        field: value_field,
                        value: AbstractResult {
                            value: literal,
                            scalar_type: ScalarType::Integer(integer_type),
                        },
                    },
                    AbstractOperation::CallStructuralScalar {
                        psi_operation: OperationId::new(82).unwrap(),
                        result: AbstractResult {
                            value: call_result,
                            scalar_type: ScalarType::Integer(integer_type),
                        },
                        callee: realization,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: caller_place,
                            path: vec![StructuralPathSegment::Field("item".into())],
                            access: StructuralAccess::SharedBorrow,
                        }],
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(80).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: realization,
                attachment: Some(carrier_type),
                entry: BlockId::new(realization.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![realization_parameter.clone()],
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: realization_result,
                    scalar_type: ScalarType::Integer(integer_type),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(realization)],
                operations: vec![
                    AbstractOperation::IntegerStructuralField {
                        psi_operation: OperationId::new(83).unwrap(),
                        result: AbstractResult {
                            value: realization_result,
                            scalar_type: ScalarType::Integer(integer_type),
                        },
                        source: realization_parameter,
                        field: value_field,
                    },
                    AbstractOperation::Return {
                        psi_edge: EdgeId::new(81).unwrap(),
                        result: realization_result,
                        value: realization_result,
                        scalar_type: ScalarType::Integer(integer_type),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
        ],
    }
}

#[test]
fn direct_projected_integer_store_and_call_retain_native_custody() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&direct_projected_integer_plan(), target)
            .expect("direct projected store and call lower");
        let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("direct caller must remain an attached Unit body")
        };
        assert!(matches!(
            body.operations.as_slice(),
            [
                TargetUnitOperation::IntegerConstant { result, .. },
                TargetUnitOperation::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    field_byte_offset: 0,
                    source: TargetUnitScalarArgumentSource::IntegerImmediate {
                        source_value,
                        value: IntegerValue::Signed(17),
                        ..
                    },
                    ..
                },
                TargetUnitOperation::StructuralScalarCall {
                    result: AbstractResult {
                        value: call_result,
                        ..
                    },
                    arguments,
                    ..
                },
                TargetUnitOperation::Return { .. }
            ] if *result == ValueId::new(80).unwrap()
                && destination.place == PlaceId::new(80).unwrap()
                && path == &[StructuralPathSegment::Field("item".into())]
                && *field == StructuralFieldId::new(81).unwrap()
                && *source_value == ValueId::new(80).unwrap()
                && *call_result == ValueId::new(81).unwrap()
                && arguments.len() == 1
                && arguments[0].path == [StructuralPathSegment::Field("item".into())]
                && arguments[0].source_byte_offset == 0
        ));
    }
}

#[test]
fn direct_projected_integer_route_rejects_semantic_location_drift() {
    let target = NativeTarget::linux_x64();

    let mut destination_drift = direct_projected_integer_plan();
    let AbstractOperation::StructuralScalarFieldStore { destination, .. } =
        &mut destination_drift.functions[0].operations[1]
    else {
        unreachable!()
    };
    destination.position = 1;
    assert!(matches!(
        lower_to_target_operations(&destination_drift, target),
        Err(crate::LoweringError::UnsupportedOperationInUnitFunction(machine))
            if machine == MachineId::new(80).unwrap()
    ));

    let mut call_path_drift = direct_projected_integer_plan();
    let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut call_path_drift.functions[0].operations[2]
    else {
        unreachable!()
    };
    structural_arguments[0].path.clear();
    assert!(matches!(
        lower_to_target_operations(&call_path_drift, target),
        Err(crate::LoweringError::StructuralCallArgumentTypeMismatch { callee, .. })
            if callee == MachineId::new(81).unwrap()
    ));
}
