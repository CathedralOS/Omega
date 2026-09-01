use super::*;

#[derive(Debug)]
struct InstalledProviderFixture {
    psi: TerminalPsiIdentity,
    calls: Vec<InstalledProviderUnitCallEvidence>,
}

impl ProviderInstallationEvidence for InstalledProviderFixture {
    fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    fn installed_provider_unit_calls(&self) -> Vec<InstalledProviderUnitCallEvidence> {
        self.calls.clone()
    }
}

fn installed_provider_plan() -> (
    AbstractOperationPlan,
    InstalledProviderFixture,
    BoundaryMachineId,
    OperationId,
) {
    let caller = MachineId::new(950).unwrap();
    let callee = MachineId::new(951).unwrap();
    let boundary = BoundaryMachineId::new(950).unwrap();
    let operation = OperationId::new(950).unwrap();
    let structural_type = StructuralTypeId::new(950).unwrap();
    let caller_place = PlaceId::new(950).unwrap();
    let boundary_place = PlaceId::new(951).unwrap();
    let callee_place = PlaceId::new(952).unwrap();
    let claim = psi_core::ClaimId::new(950).unwrap();
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let argument = StructuralArgument {
        place: caller_place,
        access: StructuralAccess::Owned,
        path: Vec::new(),
    };
    let entry_source = psi_terminal::EntryClaim {
        claim,
        input: caller_place,
        path: Vec::new(),
    };
    let receipt = psi_terminal::CompletionReceipt {
        claim,
        argument_index: 0,
    };
    let provider = psi_terminal::ProviderCandidateConformance {
        boundary,
        requirement_identity: "ProgramEntry::enter".into(),
        provider_identity: "ProgramProvider".into(),
        candidate_identity: "ProgramProvider::enter".into(),
        candidate: callee,
        signature: psi_terminal::ProviderUnitSignature {
            parameters: vec![psi_terminal::ProviderSignatureParameter {
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
        },
        refinement: psi_terminal::ProviderUnitRefinement {
            positional_parameters: vec![psi_terminal::ProviderParameterRefinement {
                boundary_index: 0,
                candidate_index: 0,
            }],
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    let block_entry = |machine: MachineId| AbstractBlockEntry {
        block: BlockId::new(machine.get()).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "Extent".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(950).unwrap(),
                    identity: "length".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                }],
            },
        }],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "ProgramEntry::enter".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![parameter(boundary_place)],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: vec![provider.clone()],
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: BlockId::new(caller.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place)],
                result: AbstractFunctionResult::Unit,
                entry_claims: vec![entry_source.clone()],
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(caller)],
                operations: vec![
                    AbstractOperation::BoundaryCall {
                        psi_operation: operation,
                        result: None,
                        boundary,
                        arguments: Vec::new(),
                        structural_arguments: vec![argument.clone()],
                        completion_claim_sources: vec![
                            omega_abstract_operations::CompletionClaimSource {
                                claim,
                                entry: Some(entry_source.clone()),
                                content: None,
                            },
                        ],
                        completion_receipts: vec![receipt],
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(950).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: Some(structural_type),
                entry: BlockId::new(callee.get()).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place)],
                result: AbstractFunctionResult::Unit,
                entry_claims: vec![psi_terminal::EntryClaim {
                    claim: psi_core::ClaimId::new(951).unwrap(),
                    input: callee_place,
                    path: Vec::new(),
                }],
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(callee)],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(951).unwrap(),
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    };
    let installation = InstalledProviderFixture {
        psi: plan.psi,
        calls: vec![InstalledProviderUnitCallEvidence {
            caller,
            psi_operation: operation,
            boundary,
            provider,
            structural_arguments: vec![argument],
            completion_claim_sources: vec![InstalledProviderCompletionClaimSource {
                claim,
                entry: Some(entry_source),
                content: None,
            }],
            completion_receipts: vec![receipt],
        }],
    };
    (plan, installation, boundary, operation)
}

#[test]
fn admitted_structural_provider_projects_to_distinct_target_call() {
    let (plan, installation, boundary, operation) = installed_provider_plan();
    assert_eq!(
        lower_to_target_operations(&plan, NativeTarget::uefi_x64()),
        Err(LoweringError::MissingBoundarySettlement(boundary))
    );
    let lowered = lower_to_target_operations_with_provider_executions_and_installation(
        &plan,
        NativeTarget::uefi_x64(),
        &[],
        Some(&installation),
    )
    .expect("installed provider call lowers without an external settlement");
    let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
        panic!("caller remains a Unit body")
    };
    assert!(matches!(
        &body.operations[0],
        TargetUnitOperation::InstalledProviderCall {
            psi_operation,
            boundary: actual_boundary,
            provider,
            arguments,
            claim_transfers,
            completion_receipts,
            ..
        } if *psi_operation == operation
            && *actual_boundary == boundary
            && provider == &installation.calls[0].provider
            && arguments.len() == 1
            && arguments[0].access == StructuralAccess::Owned
            && claim_transfers.len() == 1
            && completion_receipts.len() == 1
    ));
}

#[test]
fn linux_exit_group_i32_requires_exact_literal_shape_and_stays_fail_closed_elsewhere() {
    let machine = MachineId::new(901).unwrap();
    let boundary = BoundaryMachineId::new(901).unwrap();
    let constant_operation = OperationId::new(901).unwrap();
    let settlement_operation = OperationId::new(902).unwrap();
    let return_edge = EdgeId::new(901).unwrap();
    let value = ValueId::new(901).unwrap();
    let block = BlockId::new(901).unwrap();
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(i32_type);
    let provider_execution =
        omega_target_operations::ProviderExecutionBinding::from_execution_record(
            omega_target_operations::ProviderPlanReportIdentity::new(901).unwrap(),
            902,
            903,
            904,
            905,
        )
        .unwrap();
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "Console::exit_process(i32)->Unit".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
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
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: constant_operation,
                    result: value,
                    scalar_type,
                    value: IntegerValue::Signed(37),
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: settlement_operation,
                    result: None,
                    boundary,
                    arguments: vec![value],
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
    let binding = omega_target_operations::BoundarySettlementBinding {
        boundary,
        execution: provider_execution.into(),
        realization: omega_target_operations::LinuxExitGroupI32Realization.into(),
    };

    let x86 = lower_to_target_operations_with_settlements(
        &plan,
        NativeTarget::linux_x64(),
        &[binding.clone()],
    )
    .expect("Linux x86-64 exit_group lowering");
    assert_eq!(
        x86,
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::linux_x64(),
            &[binding.clone()],
        )
        .expect("deterministic lowering")
    );
    assert!(matches!(
        &x86.functions[0].operation,
        TargetOperation::ExitProcessI32 { argument, nominal_return_edge, .. }
            if argument.source_value == value
                && argument.scalar_type == scalar_type
                && argument.immediate == IntegerValue::Signed(37)
                && argument.destination == MachineRegister::X86Rdi
                && *nominal_return_edge == return_edge
    ));
    let arm = lower_to_target_operations_with_settlements(
        &plan,
        NativeTarget::linux_arm64(),
        &[binding.clone()],
    )
    .expect("Linux AArch64 exit_group lowering");
    assert!(matches!(
        &arm.functions[0].operation,
        TargetOperation::ExitProcessI32 { argument, .. }
            if argument.destination == MachineRegister::Aarch64X(0)
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::windows_x64(),
            &[binding.clone()],
        ),
        Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));
    assert!(matches!(
        lower_to_target_operations_with_settlements(
            &plan,
            NativeTarget::macos_arm64(),
            &[binding.clone()],
        ),
        Err(LoweringError::LinuxExitGroupUnsupportedTarget { .. })
    ));

    let mut wrong_signature = plan;
    wrong_signature.boundary_machines[0].scalar_parameters[0] = ScalarType::Boolean;
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &wrong_signature,
            NativeTarget::linux_x64(),
            &[binding.clone()],
        ),
        Err(LoweringError::InvalidLinuxExitGroupShape(machine))
    );
}

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
                result: None,
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
                result: None,
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
                    result: None,
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
                    result: None,
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
