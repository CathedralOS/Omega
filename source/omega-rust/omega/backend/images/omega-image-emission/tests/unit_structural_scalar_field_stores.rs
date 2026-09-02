use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_image_emission::{
    InstallationError, ObjectError, build_installation_record, build_object_artifact,
    decode_installation_record, emit_executable_image, encode_installation_record,
    validate_installation_record,
};
use omega_machine_emission::emit_machine_code;
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetFunction, TargetOperation, TargetOperationPlan, TargetStructuralParameter,
    TargetUnitBody, TargetUnitOperation, TargetUnitScalarArgumentSource, TerminalPsiProvenance,
};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::{
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use psi_terminal::{
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

fn emitted_field_store(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let assigned = assign_registers(&field_store_plan(target)).expect("assign field store");
    emit_machine_code(&assigned).expect("emit field store")
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
            [machine_store.clone()]
        );
        let image = emit_executable_image(&object, 3).expect("emit field-store image");
        let record =
            build_installation_record(&image, psi_core::ProfileDecisionId::new(1).unwrap())
                .expect("build field-store installation");
        assert_eq!(
            record.functions()[0].unit_structural_scalar_field_stores,
            [machine_store.clone()]
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
    let omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
        value, ..
    } = &mut store.source
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
    let record =
        build_installation_record(&image, psi_core::ProfileDecisionId::new(1).unwrap()).unwrap();
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
