//! Native-boundary provider and Linux settlement fixtures.

use super::super::super::{LoweringError, TargetLoweringRequest, lower_to_target_operations};
use super::super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, BlockId, BoundaryMachineDeclaration,
    BoundaryMachineId, EdgeId, InstalledProviderCallEvidence,
    InstalledProviderCompletionClaimSource, IntegerSign, IntegerType, MachineId, NativeTarget,
    OperationId, PlaceId, ProviderInstallationEvidence, ScalarType, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, TargetUnitOperation, TerminalPsiIdentity, ValueId,
    identity,
};

#[derive(Debug)]
struct InstalledProviderFixture {
    psi: TerminalPsiIdentity,
    calls: Vec<InstalledProviderCallEvidence>,
}

impl ProviderInstallationEvidence for InstalledProviderFixture {
    fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    fn installed_provider_calls(&self) -> Vec<InstalledProviderCallEvidence> {
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
    let claim = semantic_vocabulary::ClaimId::new(950).unwrap();
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
    let entry_source = terminal_psi::EntryClaim {
        claim,
        input: caller_place,
        path: Vec::new(),
    };
    let receipt = terminal_psi::CompletionReceipt {
        claim,
        argument_index: 0,
    };
    let provider = terminal_psi::ProviderCandidateConformance {
        boundary,
        requirement_identity: "ProgramEntry::enter".into(),
        provider_identity: "ProgramProvider".into(),
        candidate_identity: "ProgramProvider::enter".into(),
        candidate: callee,
        signature: terminal_psi::ProviderSignature {
            parameters: vec![terminal_psi::ProviderSignatureParameter {
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
        },
        refinement: terminal_psi::ProviderRefinement {
            positional_parameters: vec![terminal_psi::ProviderParameterRefinement {
                boundary_index: 0,
                candidate_index: 0,
            }],
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    let block_entry = |machine: MachineId| AbstractBlockEntry {
        structural_parameters: Vec::new(),
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
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                }],
            },
        }]
        .into(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary,
            identity: "ProgramEntry::enter".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![parameter(boundary_place)],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
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
                        result: abstract_operations::AbstractBoundaryResult::Unit,
                        boundary,
                        arguments: Vec::new(),
                        structural_arguments: vec![argument.clone()],
                        completion_claim_sources: vec![
                            abstract_operations::CompletionClaimSource {
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
                entry_claims: vec![terminal_psi::EntryClaim {
                    claim: semantic_vocabulary::ClaimId::new(951).unwrap(),
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
        calls: vec![InstalledProviderCallEvidence {
            caller,
            psi_operation: operation,
            boundary,
            provider,
            result: terminal_psi::OperationResult::Unit,
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
fn graph_rejects_unimplemented_claim_bearing_provider_calls() {
    let (plan, installation, _, _) = installed_provider_plan();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        assert!(matches!(
            lower_to_target_operations(
                &plan,
                TargetLoweringRequest {
                    target,
                    settlements: &[],
                    installation: Some(&installation),
                    ieee_float_fma: &[]
                }
            ),
            Err(LoweringError::UnsupportedControlFlow(_))
        ));
    }
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
    let provider = terminal_psi::ProviderCandidateConformance {
        boundary,
        requirement_identity: "Ping::ping_value".into(),
        provider_identity: "PingProvider".into(),
        candidate_identity: "PingProvider::ping_value".into(),
        candidate,
        signature: terminal_psi::ProviderSignature {
            parameters: Vec::new(),
        },
        refinement: terminal_psi::ProviderRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    let scalar_parameter = |value| AbstractParameter { value, scalar_type };
    let block_entry = |machine: MachineId, value: ValueId| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(machine.get()).unwrap(),
        parameters: vec![scalar_parameter(value)],
        operation_offset: 0,
    };
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: caller,
        structural_types: Vec::new().into(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
            id: boundary,
            identity: "Ping::ping_value".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
            scalar_parameters: vec![scalar_type],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
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
                        result: abstract_operations::AbstractBoundaryResult::Unit,
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
        calls: vec![InstalledProviderCallEvidence {
            caller,
            psi_operation: operation,
            boundary,
            provider,
            result: terminal_psi::OperationResult::Unit,
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
fn installed_provider_calls_retain_scalar_operands_and_selection_custody() {
    let (plan, installation, _, _, _, _) = installed_scalar_provider_plan();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(
            &plan,
            TargetLoweringRequest {
                target,
                settlements: &[],
                installation: Some(&installation),
                ieee_float_fma: &[],
            },
        )
        .unwrap();
        crate::validate_abstract_to_target_translation(&plan, target, &lowered).unwrap();
        crate::validation::installed_calls::validate(&lowered, Some(&installation)).unwrap();
        assert!(crate::validation::installed_calls::validate(&lowered, None).is_err());
        for corruption in 0..5 {
            let mut changed = lowered.clone();
            let call = &mut changed.functions[0].graph.blocks[0].operations[0];
            let TargetUnitOperation::Call {
                callee,
                origin,
                scalar_arguments,
                ..
            } = call
            else {
                panic!("ordinary call transport");
            };
            match corruption {
                0 => *origin = target_operations::NativeCallOrigin::Authored,
                1 => {
                    let target_operations::NativeCallOrigin::InstalledProvider { provider, .. } =
                        origin
                    else {
                        unreachable!()
                    };
                    *callee = MachineId::new(9_999).unwrap();
                    provider.candidate = *callee;
                }
                2 => scalar_arguments.clear(),
                3 => {
                    let target_operations::NativeCallOrigin::InstalledProvider {
                        completion_receipts,
                        ..
                    } = origin
                    else {
                        unreachable!()
                    };
                    completion_receipts.push(terminal_psi::CompletionReceipt {
                        claim: semantic_vocabulary::ClaimId::new(9_999).unwrap(),
                        argument_index: 0,
                    });
                }
                _ => {
                    let duplicate = call.clone();
                    changed.functions[0].graph.blocks[0]
                        .operations
                        .push(duplicate);
                }
            }
            assert!(
                crate::validation::installed_calls::validate(&changed, Some(&installation))
                    .is_err(),
                "corruption {corruption}"
            );
        }
    }
}

#[test]
fn installed_i32_provider_rejects_scalar_evidence_substitution() {
    let (plan, mut installation, boundary, operation, _, _) = installed_scalar_provider_plan();
    installation.calls[0].scalar_arguments[0] = ValueId::new(9_600).unwrap();
    assert_eq!(
        lower_to_target_operations(
            &plan,
            TargetLoweringRequest {
                target: NativeTarget::linux_x64(),
                settlements: &[],
                installation: Some(&installation),
                ieee_float_fma: &[]
            }
        ),
        Err(LoweringError::InstalledProviderCallEvidenceMismatch {
            machine: plan.entry,
            operation,
            boundary,
        })
    );
}

#[test]
fn installed_selection_rejects_another_semantically_valid_catalog_candidate() {
    let (mut plan, installation, _, _, _, _) = installed_scalar_provider_plan();
    let mut alternate_function = plan.functions[1].clone();
    let mut alternate_provider = plan.provider_candidates[0].clone();
    alternate_function.machine = MachineId::new(9_999).unwrap();
    alternate_function.entry = BlockId::new(9_999).unwrap();
    alternate_function.block_entries[0].block = alternate_function.entry;
    alternate_provider.candidate = alternate_function.machine;
    alternate_provider.candidate_identity = "OtherPingProvider::ping_value".into();
    alternate_provider.provider_identity = "OtherPingProvider".into();
    plan.functions.push(alternate_function);
    plan.provider_candidates.push(alternate_provider.clone());
    let native = NativeTarget::macos_arm64();
    let mut target = lower_to_target_operations(
        &plan,
        TargetLoweringRequest {
            target: native,
            settlements: &[],
            installation: Some(&installation),
            ieee_float_fma: &[],
        },
    )
    .unwrap();
    crate::validation::installed_calls::validate(&target, Some(&installation)).unwrap();
    let TargetUnitOperation::Call { callee, origin, .. } =
        &mut target.functions[0].graph.blocks[0].operations[0]
    else {
        panic!("installed call");
    };
    *callee = alternate_provider.candidate;
    let target_operations::NativeCallOrigin::InstalledProvider { provider, .. } = origin else {
        panic!("installed origin");
    };
    *provider = alternate_provider;
    // The replacement has the same signature and arguments and is present in
    // this very catalog. Only the separately retained selection distinguishes it.
    crate::validate_abstract_to_target_translation(&plan, native, &target).unwrap();
    assert!(crate::validation::installed_calls::validate(&target, Some(&installation)).is_err());
}

#[test]
fn installed_provider_result_must_match_occurrence_and_boundary_declaration() {
    for replace_call_result in [false, true] {
        let (mut plan, mut installation, boundary, operation) = installed_provider_plan();
        let result =
            terminal_psi::OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: PlaceId::new(9_602).unwrap(),
                structural_type: StructuralTypeId::new(950).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            });
        installation.calls[0].result = result.clone();
        if replace_call_result {
            let AbstractOperation::BoundaryCall { result: actual, .. } =
                &mut plan.functions[0].operations[0]
            else {
                unreachable!()
            };
            *actual = abstract_operations::AbstractBoundaryResult::Structural(
                result.structural().unwrap().clone(),
            );
        }
        assert_eq!(
            lower_to_target_operations(
                &plan,
                TargetLoweringRequest {
                    target: NativeTarget::linux_x64(),
                    settlements: &[],
                    installation: Some(&installation),
                    ieee_float_fma: &[]
                }
            ),
            Err(LoweringError::InstalledProviderCallEvidenceMismatch {
                machine: plan.entry,
                operation,
                boundary,
            })
        );
    }
}
