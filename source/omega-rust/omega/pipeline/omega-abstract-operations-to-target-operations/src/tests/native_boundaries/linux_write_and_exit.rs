use super::*;

#[test]
fn linux_write_line_and_exit_compose_in_one_shared_unit_body() {
    let machine = MachineId::new(920).unwrap();
    let block = BlockId::new(920).unwrap();
    let write_boundary = BoundaryMachineId::new(920).unwrap();
    let exit_boundary = BoundaryMachineId::new(921).unwrap();
    let byte_type = StructuralTypeId::new(920).unwrap();
    let literal_place = PlaceId::new(920).unwrap();
    let exit_value = ValueId::new(920).unwrap();
    let literal_operation = OperationId::new(920).unwrap();
    let write_operation = OperationId::new(921).unwrap();
    let constant_operation = OperationId::new(922).unwrap();
    let exit_operation = OperationId::new(923).unwrap();
    let return_edge = EdgeId::new(920).unwrap();
    let bytes = vec![0, 0x80, 0xff];
    let byte_declaration = StructuralTypeDeclaration {
        id: byte_type,
        identity: "test::BorrowedBytes".into(),
        shape: StructuralTypeShape::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView),
    };
    let literal_declaration = psi_terminal::StructuralPlaceDeclaration {
        id: literal_place,
        kind: psi_core::StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: byte_type,
        },
    };
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: vec![byte_declaration.clone()],
        boundary_machines: vec![
            BoundaryMachineDeclaration {
                id: write_boundary,
                identity: "Console::write_line(&[u8])->Unit".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(921).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: byte_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: psi_terminal::BoundaryMachineResult::Unit,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            },
            BoundaryMachineDeclaration {
                id: exit_boundary,
                identity: "Console::exit_process(i32)->Unit".into(),
                attachment: None,
                scalar_parameters: vec![ScalarType::Integer(i32_type)],
                structural_parameters: Vec::new(),
                result: psi_terminal::BoundaryMachineResult::Unit,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
            },
        ],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::EstablishByteSequenceLiteral {
                    psi_operation: literal_operation,
                    place: literal_declaration,
                    structural_type: byte_declaration,
                    bytes: bytes.clone(),
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: write_operation,
                    result: omega_abstract_operations::AbstractBoundaryResult::Unit,
                    boundary: write_boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: literal_place,
                        access: StructuralAccess::SharedBorrow,
                        path: Vec::new(),
                    }],
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: constant_operation,
                    result: exit_value,
                    scalar_type: ScalarType::Integer(i32_type),
                    value: IntegerValue::Signed(37),
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: exit_operation,
                    result: omega_abstract_operations::AbstractBoundaryResult::Unit,
                    boundary: exit_boundary,
                    arguments: vec![exit_value],
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let provider = |seed| {
        omega_target_operations::ProviderExecutionBinding::from_execution_record(
            omega_target_operations::ProviderPlanReportIdentity::new(seed).unwrap(),
            seed + 1,
            seed + 2,
            seed + 3,
            seed + 4,
        )
        .unwrap()
    };
    let settlements = [
        omega_target_operations::BoundarySettlementBinding {
            boundary: write_boundary,
            execution: provider(920).into(),
            realization: omega_target_operations::LinuxWriteLineRealization.into(),
        },
        omega_target_operations::BoundarySettlementBinding {
            boundary: exit_boundary,
            execution: provider(930).into(),
            realization: omega_target_operations::LinuxExitGroupI32Realization.into(),
        },
    ];

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = lower_to_target_operations_with_settlements(&plan, target, &settlements)
            .expect("composed Linux effect body lowers");
        let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
            panic!("write_line -> exit_process remains a shared Unit body")
        };
        assert!(matches!(
            &body.operations[0],
            TargetUnitOperation::EstablishByteSequenceLiteral { bytes: actual, .. }
                if actual == &bytes
        ));
        assert!(matches!(
            &body.operations[1],
            TargetUnitOperation::BoundarySettlement {
                realization: omega_target_operations::BoundaryRealization::LinuxWriteLine(_),
                byte_sequence_arguments,
                ..
            } if byte_sequence_arguments[0].bytes == bytes
        ));
        assert!(matches!(
            &body.operations[3],
            TargetUnitOperation::BoundarySettlement {
                realization: omega_target_operations::BoundaryRealization::LinuxExitGroupI32(_),
                scalar_arguments,
                ..
            } if scalar_arguments[0].immediate == IntegerValue::Signed(37)
        ));
    }
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::windows_x64(),
            &settlements,
        ),
        Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid { .. })
            | Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::macos_arm64(),
            &settlements,
        ),
        Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid { .. })
            | Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));
}
