//! Native-boundary provider and Linux settlement fixtures.

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
            result: psi_terminal::BoundaryMachineResult::Unit,
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
            scalar_arguments: Vec::new(),
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

fn installed_scalar_provider_plan() -> (
    AbstractOperationPlan,
    InstalledProviderFixture,
    BoundaryMachineId,
    OperationId,
    ValueId,
    ValueId,
) {
    let caller = MachineId::new(960).unwrap();
    let candidate = MachineId::new(961).unwrap();
    let boundary = BoundaryMachineId::new(960).unwrap();
    let operation = OperationId::new(960).unwrap();
    let caller_value = ValueId::new(960).unwrap();
    let candidate_value = ValueId::new(961).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let provider = psi_terminal::ProviderCandidateConformance {
        boundary,
        requirement_identity: "Ping::ping_value".into(),
        provider_identity: "PingProvider".into(),
        candidate_identity: "PingProvider::ping_value".into(),
        candidate,
        signature: psi_terminal::ProviderUnitSignature {
            parameters: Vec::new(),
        },
        refinement: psi_terminal::ProviderUnitRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    let scalar_parameter = |value| AbstractParameter { value, scalar_type };
    let block_entry = |machine: MachineId, value: ValueId| AbstractBlockEntry {
        block: BlockId::new(machine.get()).unwrap(),
        parameters: vec![scalar_parameter(value)],
        operation_offset: 0,
    };
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "Ping::ping_value".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: psi_terminal::BoundaryMachineResult::Unit,
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
                parameters: vec![scalar_parameter(caller_value)],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(caller, caller_value)],
                operations: vec![
                    AbstractOperation::BoundaryCall {
                        psi_operation: operation,
                        result: None,
                        boundary,
                        arguments: vec![caller_value],
                        structural_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(960).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: candidate,
                attachment: None,
                entry: BlockId::new(candidate.get()).unwrap(),
                parameters: vec![scalar_parameter(candidate_value)],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block_entry(candidate, candidate_value)],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(961).unwrap(),
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
            scalar_arguments: vec![caller_value],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        }],
    };
    (
        plan,
        installation,
        boundary,
        operation,
        caller_value,
        candidate_value,
    )
}

#[test]
fn admitted_i32_provider_retains_exact_incoming_and_outgoing_abi() {
    let (plan, installation, boundary, operation, caller_value, candidate_value) =
        installed_scalar_provider_plan();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = lower_to_target_operations_with_provider_executions_and_installation(
            &plan,
            target,
            &[],
            Some(&installation),
        )
        .expect("installed signed-i32 provider call lowers");
        let TargetOperation::UnitBody(caller) = &lowered.functions[0].operation else {
            panic!("caller remains a Unit body")
        };
        assert!(matches!(
            caller.scalar_parameters.as_slice(),
            [parameter] if parameter.value == caller_value
                && parameter.scalar_type
                    == ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 32).unwrap()
                    )
                && parameter.placement == caller.call_plan.parameters[0]
        ));
        assert!(matches!(
            caller.operations.as_slice(),
            [TargetUnitOperation::InstalledProviderCall {
                psi_operation,
                boundary: actual_boundary,
                call_plan,
                scalar_arguments,
                arguments,
                ..
            }, TargetUnitOperation::Return { .. }]
                if *psi_operation == operation
                    && *actual_boundary == boundary
                    && call_plan.result.is_none()
                    && call_plan.parameters.len() == 1
                    && arguments.is_empty()
                    && matches!(scalar_arguments.as_slice(), [argument]
                        if argument.parameter_index == 0
                            && argument.placement == call_plan.parameters[0]
                            && matches!(argument.source,
                                TargetUnitScalarArgumentSource::Parameter {
                                    parameter_index: 0,
                                    source_value,
                                    scalar_type,
                                } if source_value == caller_value
                                    && scalar_type == IntegerType::new(IntegerSign::Signed, 32).unwrap()))
        ));
        let TargetOperation::UnitBody(candidate) = &lowered.functions[1].operation else {
            panic!("provider candidate remains a Unit body")
        };
        assert!(matches!(
            candidate.scalar_parameters.as_slice(),
            [parameter] if parameter.value == candidate_value
                && parameter.placement == candidate.call_plan.parameters[0]
        ));
    }
}

#[test]
fn installed_i32_provider_rejects_scalar_evidence_substitution() {
    let (plan, mut installation, boundary, operation, _, _) = installed_scalar_provider_plan();
    installation.calls[0].scalar_arguments[0] = ValueId::new(9_600).unwrap();
    assert_eq!(
        lower_to_target_operations_with_provider_executions_and_installation(
            &plan,
            NativeTarget::linux_x64(),
            &[],
            Some(&installation),
        ),
        Err(LoweringError::InstalledProviderCallEvidenceMismatch {
            machine: plan.entry,
            operation,
            boundary,
        })
    );
}

#[test]
fn installed_i32_provider_rejects_reusing_the_caller_saved_parameter() {
    let (mut plan, mut installation, boundary, operation, _, _) = installed_scalar_provider_plan();
    let repeated_operation = OperationId::new(9_601).unwrap();
    let mut repeated_call = plan.functions[0].operations[0].clone();
    let AbstractOperation::BoundaryCall { psi_operation, .. } = &mut repeated_call else {
        unreachable!()
    };
    *psi_operation = repeated_operation;
    plan.functions[0].operations.insert(1, repeated_call);
    let mut repeated_evidence = installation.calls[0].clone();
    repeated_evidence.psi_operation = repeated_operation;
    installation.calls.push(repeated_evidence);

    assert_eq!(
        lower_to_target_operations_with_provider_executions_and_installation(
            &plan,
            NativeTarget::linux_x64(),
            &[],
            Some(&installation),
        ),
        Err(LoweringError::InstalledProviderCallShapeMismatch {
            machine: plan.entry,
            operation,
            boundary,
        })
    );
}
