use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use image_emission::{
    InstallationError, ObjectError, build_installation_record, build_object_artifact,
    decode_installation_record, emit_executable_image, encode_installation_record,
    validate_installation_record,
};
use machine_emission::emit_machine_code;
use semantic_vocabulary::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    TargetFunction, TargetIntegerExpression, TargetOperation, TargetOperationPlan,
    TargetScalarImmediate, TargetScalarStructuralFieldStore, TargetStructuralParameter,
    TargetUnitBody, TargetUnitOperation, TargetUnitScalarArgumentSource, TerminalPsiProvenance,
};
use target_operations_to_assigned_target_operations::assign_registers;
use terminal_psi::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity,
    VocabularyMarker,
};

fn field_store_plan(target: NativeTarget) -> TargetOperationPlan {
    let machine = MachineId::new(970).unwrap();
    let root_type = StructuralTypeId::new(970).unwrap();
    let carrier_type = StructuralTypeId::new(971).unwrap();
    let root = PlaceId::new(970).unwrap();
    let carrier_field = StructuralFieldId::new(970).unwrap();
    let scalar_field = StructuralFieldId::new(971).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let root_shape = ValueShape::integer(4, 4);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .unwrap();
    let constant_operation = OperationId::new(970).unwrap();
    let store_operation = OperationId::new(971).unwrap();
    let return_edge = EdgeId::new(970).unwrap();
    let constant = ValueId::new(970).unwrap();
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x97; 32]),
        },
        target,
        entry: machine,
        functions: vec![TargetFunction {
            machine,
            attachment: Some(root_type),
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![constant_operation, store_operation],
                edges: vec![return_edge],
            },
            operation: TargetOperation::UnitBody(TargetUnitBody {
                structural_types: vec![
                    StructuralTypeDeclaration {
                        id: root_type,
                        identity: "DriverState".into(),
                        shape: StructuralTypeShape::Record {
                            fields: vec![StructuralFieldDeclaration {
                                id: carrier_field,
                                identity: "item".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Structural(carrier_type),
                            }],
                        },
                    },
                    StructuralTypeDeclaration {
                        id: carrier_type,
                        identity: "Register".into(),
                        shape: StructuralTypeShape::Record {
                            fields: vec![StructuralFieldDeclaration {
                                id: scalar_field,
                                identity: "value".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                    integer_type,
                                )),
                            }],
                        },
                    },
                ],
                call_plan: call_plan.clone(),
                scalar_parameters: Vec::new(),
                parameters: vec![TargetStructuralParameter {
                    place: root,
                    structural_type: root_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    projected_qualifications: Vec::new(),
                    shape: root_shape,
                    placement: call_plan.parameters[0].clone(),
                }],
                operations: vec![
                    TargetUnitOperation::IntegerConstant {
                        psi_operation: constant_operation,
                        result: constant,
                        scalar_type: integer_type,
                        value: IntegerValue::Signed(17),
                    },
                    TargetUnitOperation::StructuralScalarFieldStore {
                        psi_operation: store_operation,
                        destination: StructuralParameterDeclaration {
                            place: root,
                            position: 0,
                            is_self: true,
                            structural_type: root_type,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::MutableBorrow,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        },
                        path: vec![StructuralPathSegment::Field("item".into())],
                        field: scalar_field,
                        destination_placement: call_plan.parameters[0].clone(),
                        field_byte_offset: 0,
                        source: TargetUnitScalarArgumentSource::IntegerImmediate {
                            defining_operation: constant_operation,
                            source_value: constant,
                            scalar_type: integer_type,
                            value: IntegerValue::Signed(17),
                        },
                    },
                    TargetUnitOperation::Return {
                        psi_edge: return_edge,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }),
        }],
    }
}

fn emitted_field_store(target: NativeTarget) -> machine_code::MachineCodePlan {
    let assigned = assign_registers(&field_store_plan(target)).expect("assign field store");
    emit_machine_code(&assigned).expect("emit field store")
}

fn scalar_field_store_plan(target: NativeTarget) -> TargetOperationPlan {
    let machine = MachineId::new(980).unwrap();
    let structural_type = StructuralTypeId::new(980).unwrap();
    let place = PlaceId::new(980).unwrap();
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let shape = ValueShape::borrowed_reference(16, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .unwrap();
    let destination = StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let parameter = TargetStructuralParameter {
        place,
        structural_type,
        multiplicity: destination.multiplicity,
        access: destination.access,
        projected_qualifications: Vec::new(),
        shape,
        placement: call_plan.parameters[0].clone(),
    };
    let stores = vec![
        TargetScalarStructuralFieldStore {
            psi_operation: OperationId::new(981).unwrap(),
            destination: destination.clone(),
            path: Vec::new(),
            field: StructuralFieldId::new(980).unwrap(),
            destination_placement: parameter.placement.clone(),
            field_byte_offset: 0,
            defining_operation: OperationId::new(980).unwrap(),
            source_value: ValueId::new(980).unwrap(),
            immediate: TargetScalarImmediate::Integer {
                scalar_type: u64_type,
                value: IntegerValue::Unsigned(513),
            },
        },
        TargetScalarStructuralFieldStore {
            psi_operation: OperationId::new(983).unwrap(),
            destination: destination.clone(),
            path: Vec::new(),
            field: StructuralFieldId::new(981).unwrap(),
            destination_placement: parameter.placement.clone(),
            field_byte_offset: 8,
            defining_operation: OperationId::new(982).unwrap(),
            source_value: ValueId::new(981).unwrap(),
            immediate: TargetScalarImmediate::Boolean(true),
        },
        TargetScalarStructuralFieldStore {
            psi_operation: OperationId::new(985).unwrap(),
            destination: destination.clone(),
            path: Vec::new(),
            field: StructuralFieldId::new(982).unwrap(),
            destination_placement: parameter.placement.clone(),
            field_byte_offset: 10,
            defining_operation: OperationId::new(984).unwrap(),
            source_value: ValueId::new(982).unwrap(),
            immediate: TargetScalarImmediate::Integer {
                scalar_type: u16_type,
                value: IntegerValue::Unsigned(257),
            },
        },
    ];
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x98; 32]),
        },
        target,
        entry: machine,
        functions: vec![TargetFunction {
            machine,
            attachment: Some(structural_type),
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    OperationId::new(980).unwrap(),
                    OperationId::new(981).unwrap(),
                    OperationId::new(982).unwrap(),
                    OperationId::new(983).unwrap(),
                    OperationId::new(984).unwrap(),
                    OperationId::new(985).unwrap(),
                    OperationId::new(986).unwrap(),
                ],
                edges: vec![EdgeId::new(980).unwrap()],
            },
            operation: TargetOperation::ScalarReturnAfterStructuralScalarFieldStores {
                stores,
                scalar: Box::new(TargetOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(980).unwrap(),
                    source_value: ValueId::new(983).unwrap(),
                    scalar_type: i32_type,
                    expression: TargetIntegerExpression::StructuralField {
                        psi_operation: OperationId::new(986).unwrap(),
                        source_value: ValueId::new(983).unwrap(),
                        source: place,
                        field: StructuralFieldId::new(983).unwrap(),
                        source_placement: parameter.placement.clone(),
                        field_byte_offset: 12,
                        integer_type: i32_type,
                    },
                }),
                structural_types: vec![StructuralTypeDeclaration {
                    id: structural_type,
                    identity: "ScalarCarrier".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![
                            StructuralFieldDeclaration {
                                id: StructuralFieldId::new(980).unwrap(),
                                identity: "value".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                    u64_type,
                                )),
                            },
                            StructuralFieldDeclaration {
                                id: StructuralFieldId::new(981).unwrap(),
                                identity: "enabled".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                            },
                            StructuralFieldDeclaration {
                                id: StructuralFieldId::new(982).unwrap(),
                                identity: "attempts".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                    u16_type,
                                )),
                            },
                            StructuralFieldDeclaration {
                                id: StructuralFieldId::new(983).unwrap(),
                                identity: "code".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                    i32_type,
                                )),
                            },
                        ],
                    },
                }],
                call_plan,
                structural_parameters: vec![parameter],
            },
        }],
    }
}

fn emitted_scalar_field_stores(target: NativeTarget) -> machine_code::MachineCodePlan {
    let assigned =
        assign_registers(&scalar_field_store_plan(target)).expect("assign scalar stores");
    emit_machine_code(&assigned).expect("emit scalar stores")
}

#[test]
fn ordered_scalar_stores_round_trip_through_object_image_and_installation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let machine = emitted_scalar_field_stores(target);
        let [first, second, third] = machine.functions[0]
            .scalar_structural_scalar_field_stores
            .as_slice()
        else {
            panic!("three machine scalar stores")
        };
        assert_eq!(first.operation_ordinal, 1);
        assert_eq!(second.operation_ordinal, 3);
        assert_eq!(third.operation_ordinal, 5);
        assert_eq!(second.code_offset, first.byte_count);
        assert_eq!(third.code_offset, first.byte_count + second.byte_count);
        let object = build_object_artifact(&machine).expect("replay scalar-store object");
        assert_eq!(
            object.functions()[0].scalar_structural_scalar_field_stores,
            [first.clone(), second.clone(), third.clone()]
        );
        let image = emit_executable_image(&object, 3).expect("emit scalar-store image");
        let record = build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("build scalar-store installation");
        assert_eq!(
            record.functions()[0].scalar_structural_scalar_field_stores,
            [first.clone(), second.clone(), third.clone()]
        );
        let bytes = encode_installation_record(&record).expect("encode scalar-store installation");
        let decoded = decode_installation_record(&bytes).expect("decode scalar-store installation");
        assert_eq!(decoded, record);
        validate_installation_record(&decoded, &image).expect("replay installed scalar stores");
    }
}

#[test]
fn scalar_store_object_rejects_order_and_interval_corruption() {
    let mut reordered = emitted_scalar_field_stores(NativeTarget::linux_x64());
    reordered.functions[0]
        .scalar_structural_scalar_field_stores
        .swap(0, 1);
    assert_eq!(
        build_object_artifact(&reordered),
        Err(
            ObjectError::InvalidScalarStructuralScalarFieldStoreEvidence(
                MachineId::new(980).unwrap()
            )
        )
    );

    let mut changed_interval = emitted_scalar_field_stores(NativeTarget::linux_arm64());
    changed_interval.functions[0].scalar_structural_scalar_field_stores[1].code_offset += 4;
    assert_eq!(
        build_object_artifact(&changed_interval),
        Err(
            ObjectError::InvalidScalarStructuralScalarFieldStoreEvidence(
                MachineId::new(980).unwrap()
            )
        )
    );
}

#[test]
fn field_store_custody_round_trips_through_object_image_and_installation() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let machine = emitted_field_store(target);
        let [machine_store] = machine.functions[0]
            .unit_structural_scalar_field_stores
            .as_slice()
        else {
            panic!("one machine field store")
        };
        let object = build_object_artifact(&machine).expect("replay field-store object");
        assert_eq!(
            object.functions()[0].unit_structural_scalar_field_stores,
            std::slice::from_ref(machine_store)
        );
        let image = emit_executable_image(&object, 3).expect("emit field-store image");
        let record = build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("build field-store installation");
        assert_eq!(
            record.functions()[0].unit_structural_scalar_field_stores,
            std::slice::from_ref(machine_store)
        );
        let bytes = encode_installation_record(&record).expect("encode field-store installation");
        let decoded = decode_installation_record(&bytes).expect("decode field-store installation");
        assert_eq!(decoded, record);
        validate_installation_record(&decoded, &image).expect("replay installed field store");
    }
}

#[test]
fn field_store_object_and_installation_reject_custody_corruption() {
    let mut changed_interval = emitted_field_store(NativeTarget::linux_x64());
    changed_interval.functions[0].unit_structural_scalar_field_stores[0].code_offset += 1;
    assert_eq!(
        build_object_artifact(&changed_interval),
        Err(ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(
            MachineId::new(970).unwrap()
        ))
    );

    let mut changed_bytes = emitted_field_store(NativeTarget::linux_arm64());
    changed_bytes.functions[0].unit_structural_scalar_field_stores[0].bytes[0] ^= 1;
    assert_eq!(
        build_object_artifact(&changed_bytes),
        Err(ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(
            MachineId::new(970).unwrap()
        ))
    );

    let mut changed_source = emitted_field_store(NativeTarget::linux_x64());
    let store = &mut changed_source.functions[0].unit_structural_scalar_field_stores[0];
    let machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate { value, .. } =
        &mut store.source
    else {
        panic!("field store uses an integer immediate")
    };
    *value = IntegerValue::Signed(18);
    assert_eq!(
        build_object_artifact(&changed_source),
        Err(ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(
            MachineId::new(970).unwrap()
        ))
    );

    let mut changed_home = emitted_field_store(NativeTarget::linux_arm64());
    changed_home.functions[0].unit_structural_scalar_field_stores[0].parameter_home_byte_offset +=
        8;
    assert_eq!(
        build_object_artifact(&changed_home),
        Err(ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(
            MachineId::new(970).unwrap()
        ))
    );

    let machine = emitted_field_store(NativeTarget::linux_x64());
    let object = build_object_artifact(&machine).unwrap();
    let image = emit_executable_image(&object, 3).unwrap();
    let record = build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let store_bytes = record.functions()[0].unit_structural_scalar_field_stores[0]
        .bytes
        .clone();
    let mut encoded = encode_installation_record(&record).unwrap();
    let encoded_store = encoded
        .windows(store_bytes.len())
        .rposition(|window| window == store_bytes)
        .expect("installed store bytes");
    encoded[encoded_store] ^= 1;
    assert_eq!(
        decode_installation_record(&encoded),
        Err(InstallationError::InvalidUnitStructuralScalarFieldStore(
            MachineId::new(970).unwrap()
        ))
    );
}
