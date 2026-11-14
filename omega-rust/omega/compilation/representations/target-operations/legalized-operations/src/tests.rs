// Unit tests and shared fixtures for `legalized_operations` call-source validation, scalar call
// ABI shape checks, and canonical plan identity drift.
//
// `call_aware_plan`, `installed_provider_plan`, and `scalar_call_unit_plan` construct baseline
// `LegalizedOperationPlan` fixtures for testing call-source validation (`validate_source`),
// ABI placement shape checks (`validate_shape`), and identity sensitivity. Origin consistency
// uses structural argument ordinals even when a scalar argument prefix is present, and empty
// aggregate results (`ValueShape::integer(0, 1)`) retain `Some` result placement while rejecting
// fabricated register or stack locations.

use super::{
    LegalizedCallSourceError, LegalizedCallUnitParameter, LegalizedOperationPlan,
    LegalizedScalarArgument, LegalizedScalarBlock, LegalizedScalarCall, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarReturn,
    LegalizedScalarReturnValue, LegalizedScalarTerminator, LegalizedStructuralContract,
    LegalizedValueDefinition, NativeCallOrigin, legalized_operation_plan_identity,
};
pub(super) use abstract_operations::CompletionClaimSource;
pub(super) use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_call_plan,
};
pub(super) use optimization_core::OptimizationUnitIdentity;
pub(super) use optimization_unit::{EffectLink, OwnershipEvent, PsiProvenance, WorkSettlement};
pub(super) use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId,
    ScalarType, StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    ValueId, WorkScheduleIdentity,
};
pub(super) use target::NativeTarget;
pub(super) use target_operations::TerminalPsiProvenance;
pub(super) use terminal_psi::{
    BindingRelevance, ClaimTransfer, CompletionReceipt, EntryClaim, ProviderCandidateConformance,
    SemanticFingerprint, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

mod identity;
mod scalar_operations;

pub(super) fn structural_call_mut(plan: &mut LegalizedOperationPlan) -> &mut LegalizedScalarCall {
    let LegalizedScalarInstructionKind::Call(call) =
        &mut plan.scalar_functions[0].blocks[0].instructions[0].kind
    else {
        panic!("structural call fixture");
    };
    call
}

pub(super) fn scalar_argument_mut(
    argument: &mut LegalizedScalarArgument,
) -> (&mut ValueId, &mut calling_conventions::ValuePlacement) {
    let LegalizedScalarArgument::Scalar { source, placement } = argument else {
        panic!("scalar argument fixture");
    };
    (source, placement)
}

pub(super) fn structural_argument_mut(
    argument: &mut LegalizedScalarArgument,
) -> (
    &mut StructuralArgument,
    &mut target_operations::TargetStructuralArgument,
) {
    let LegalizedScalarArgument::Structural {
        semantic, target, ..
    } = argument
    else {
        panic!("structural argument fixture");
    };
    (semantic, target)
}

pub(super) fn id<T>(raw: u64) -> T
where
    T: semantic_vocabulary::PsiSemanticId,
{
    T::new(raw).expect("nonzero test identity")
}

pub(super) fn call_aware_plan() -> LegalizedOperationPlan {
    let extent_type = id::<StructuralTypeId>(1);
    let granted = id::<StructuralDomainId>(1);
    let image_place = id(1);
    let storage_place = id(2);
    let shape = ValueShape::integer(16, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .expect("two-Extent Microsoft x64 call plan");
    let parameters = [image_place, storage_place]
        .into_iter()
        .enumerate()
        .map(|(position, place)| LegalizedCallUnitParameter {
            semantic: StructuralParameterDeclaration {
                place,
                position: position as u32,
                structural_type: extent_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: vec![granted],
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            },
            target: target_operations::TargetStructuralParameter {
                place,
                structural_type: extent_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                projected_qualifications: Vec::new(),
                shape,
                placement: call_plan.parameters[position].clone(),
            },
        })
        .collect::<Vec<_>>();
    let arguments = parameters
        .iter()
        .enumerate()
        .map(
            |(position, parameter)| LegalizedScalarArgument::Structural {
                indices: Vec::new(),
                semantic: StructuralArgument {
                    place: parameter.semantic.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
                target: target_operations::TargetStructuralArgument {
                    place: parameter.semantic.place,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                    root_structural_type: extent_type,
                    structural_type: extent_type,
                    shape,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    indices: Vec::new(),
                    source: call_plan.parameters[position].clone().into(),
                    destination: call_plan.parameters[position].clone(),
                },
            },
        )
        .collect::<Vec<_>>();
    let call = id::<OperationId>(1);
    let return_edge = id::<EdgeId>(1);
    LegalizedOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
        work_schedule: WorkScheduleIdentity::new(1).expect("work schedule"),
        target: NativeTarget::from_omega_target_name(Some("uefi_x86_64")).expect("UEFI target"),
        profile: None,
        entry: id(1),
        scalar_functions: vec![LegalizedScalarFunction {
            machine: id(1),
            attachment: None,
            nominal_cleanup_subject: None,
            provenance: TerminalPsiProvenance {
                operations: vec![call],
                edges: vec![return_edge],
            },
            call_plan: call_plan.clone(),
            parameters: Vec::new(),
            structural: Some(LegalizedStructuralContract {
                result: None,
                structural_types: vec![StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: extent_type,
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    shape: StructuralTypeShape::Record {
                        storage: terminal_psi::StructuralRecordStorage::Packed,
                        fields: vec![
                            StructuralFieldDeclaration {
                                inline_carrier: None,
                                storage: None,
                                id: id::<StructuralFieldId>(1),
                                identity: terminal_psi::TerminalIdentityIndex(2),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(
                                    semantic_vocabulary::ScalarType::Integer(
                                        IntegerType::address(64).expect("addr"),
                                    ),
                                ),
                            },
                            StructuralFieldDeclaration {
                                inline_carrier: None,
                                storage: None,
                                id: id::<StructuralFieldId>(2),
                                identity: terminal_psi::TerminalIdentityIndex(3),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(
                                    semantic_vocabulary::ScalarType::Integer(
                                        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                                    ),
                                ),
                            },
                        ],
                    },
                }]
                .into(),
                parameters,
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: image_place,
                        kind: StructuralPlaceKind::Parameter { position: 0 },
                    },
                    StructuralPlaceDeclaration {
                        id: storage_place,
                        kind: StructuralPlaceKind::Parameter { position: 1 },
                    },
                ],
                entry_claims: vec![
                    EntryClaim {
                        claim: id(1),
                        input: image_place,
                        path: Vec::new(),
                    },
                    EntryClaim {
                        claim: id(2),
                        input: storage_place,
                        path: Vec::new(),
                    },
                ],
                published_service_ceiling: Vec::new(),
            }),
            entry_block: id(1),
            blocks: vec![LegalizedScalarBlock {
                id: id(1),
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                instructions: vec![LegalizedScalarInstruction {
                    operation: call,
                    result: None,
                    kind: LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                        descriptor_origins: Vec::new(),
                        access_bindings: Vec::new(),
                        structural_result: None,
                        call_plan,
                        result_placement: None,
                        source: NativeCallOrigin::Authored,
                        target: target_operations::NativeCallTarget::Machine(id(2)),
                        arguments,
                        claim_transfers: vec![
                            ClaimTransfer {
                                claim: id(1),
                                argument_index: 0,
                                path: Vec::new(),
                            },
                            ClaimTransfer {
                                claim: id(2),
                                argument_index: 1,
                                path: Vec::new(),
                            },
                        ],
                        requirement_obligations: vec![
                            semantic_vocabulary::ObligationId::new(1).unwrap(),
                        ],
                        crash_continuations: vec![terminal_psi::CrashRouteBucket {
                            cause: terminal_psi::CrashCause::Trap,
                            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
                        }],
                    }),
                    work: vec![WorkSettlement {
                        site: PsiProvenance::Operation(call),
                        units: 2,
                    }],
                    effect: EffectLink {
                        input: 0,
                        output: 1,
                    },
                    ownership: vec![OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])],
                }],
                terminator: LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                    edge: return_edge,
                    value: LegalizedScalarReturnValue::Unit,
                    work: vec![WorkSettlement {
                        site: PsiProvenance::Edge(return_edge),
                        units: 1,
                    }],
                    effect: EffectLink {
                        input: 1,
                        output: 2,
                    },
                    ownership: vec![OwnershipEvent::Cleanup(Vec::new())],
                }),
            }],
            ownership_frontier_facts: Vec::new(),
        }],
    }
}

pub(super) fn installed_provider_plan() -> LegalizedOperationPlan {
    let mut plan = call_aware_plan();
    let function = &mut plan.scalar_functions[0];
    let signature = function.structural.as_ref().expect("structural signature");
    let boundary = id::<BoundaryMachineId>(1);
    let provider = ProviderCandidateConformance {
        boundary,
        requirement_identity: terminal_psi::TerminalIdentityIndex(4),
        source_requirement_identity: terminal_psi::TerminalIdentityIndex(4),
        declaration_application: terminal_psi::TerminalIdentityIndex::default(),
        receiving_application: terminal_psi::TerminalIdentityIndex::default(),
        receiving_region_arguments: Vec::new(),
        provider_identity: terminal_psi::TerminalIdentityIndex(5),
        provider_declaration: terminal_psi::NominalDeclarationIdentity::new(
            terminal_psi::TerminalIdentityIndex(5),
        ),
        candidate_identity: terminal_psi::TerminalIdentityIndex(6),
        source_candidate_identity: terminal_psi::TerminalIdentityIndex(6),
        source_candidate_package_identity: None,
        candidate: id(2),
        signature: terminal_psi::ProviderSignature {
            parameters: signature
                .parameters
                .iter()
                .map(|parameter| terminal_psi::ProviderSignatureParameter {
                    position: parameter.semantic.position,
                    structural_type: parameter.semantic.structural_type,
                    multiplicity: parameter.semantic.multiplicity,
                    access: parameter.semantic.access,
                    qualifications: parameter.semantic.qualifications.clone(),
                    projected_qualifications: parameter.semantic.projected_qualifications.clone(),
                })
                .collect(),
        },
        refinement: terminal_psi::ProviderRefinement {
            positional_parameters: vec![
                terminal_psi::ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                },
                terminal_psi::ProviderParameterRefinement {
                    boundary_index: 1,
                    candidate_index: 1,
                },
            ],
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    let completion_claim_sources = signature
        .entry_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    let completion_receipts = signature
        .entry_claims
        .iter()
        .enumerate()
        .map(|(argument_index, claim)| CompletionReceipt {
            claim: claim.claim,
            argument_index: argument_index as u32,
            path: claim.path.clone(),
        })
        .collect::<Vec<_>>();
    let node = &mut function.blocks[0].instructions[0];
    let LegalizedScalarInstructionKind::Call(call) = &mut node.kind else {
        panic!("call fixture");
    };
    node.ownership = vec![OwnershipEvent::ClaimCompletion(completion_receipts.clone())];
    call.source = NativeCallOrigin::InstalledProvider {
        boundary,
        provider,
        assumption: None,
        completion_claim_sources,
        completion_receipts,
    };
    plan
}

pub(super) fn scalar_call_unit_plan() -> LegalizedOperationPlan {
    let mut plan = call_aware_plan();
    plan.scalar_functions.clear();
    plan.target = NativeTarget::linux_x64();
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(plan.target),
        &CallSignature {
            parameters: vec![shape, shape],
            result: Some(shape),
        },
    )
    .unwrap();
    let machine = id(101);
    let callee = id(102);
    let attachment = id(103);
    let block = id(104);
    let operations = [id(105), id(106), id(107), id(108), id(109)];
    let values = [id(110), id(111), id(112), id(113), id(114)];
    let edge = id(115);
    let definition = |node| optimization_unit::ValueDefinitionSite::Node { block, node };
    let work = |operation| {
        vec![WorkSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        }]
    };
    let effect = |index| EffectLink {
        input: index,
        output: index + 1,
    };
    let instruction = |index, kind| LegalizedScalarInstruction {
        operation: operations[index],
        result: Some(LegalizedValueDefinition {
            value: values[index],
            scalar_type: ScalarType::Integer(scalar_type),
            definition_site: definition(index as u32),
        }),
        kind,
        work: work(operations[index]),
        effect: effect(index as u64),
        ownership: Vec::new(),
    };
    let call = |sources: [ValueId; 2]| {
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            descriptor_origins: Vec::new(),
            access_bindings: Vec::new(),
            structural_result: None,
            source: NativeCallOrigin::Authored,
            claim_transfers: Vec::new(),
            target: target_operations::NativeCallTarget::Machine(callee),
            call_plan: call_plan.clone(),
            result_placement: call_plan.result.clone(),
            arguments: sources
                .into_iter()
                .zip(&call_plan.parameters)
                .map(|(source, placement)| LegalizedScalarArgument::Scalar {
                    source,
                    placement: placement.clone(),
                })
                .collect(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        })
    };
    plan.scalar_functions.push(LegalizedScalarFunction {
        structural: None,
        machine,
        attachment: Some(attachment),
        nominal_cleanup_subject: None,
        provenance: TerminalPsiProvenance {
            operations: operations.to_vec(),
            edges: vec![edge],
        },
        call_plan: evaluate_call_plan(
            CallingPolicy::native_for_target(plan.target),
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .unwrap(),
        parameters: Vec::new(),
        entry_block: block,
        blocks: vec![LegalizedScalarBlock {
            id: block,
            structural_parameters: Vec::new(),
            parameters: vec![],
            instructions: vec![
                instruction(
                    0,
                    LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(7)),
                ),
                instruction(
                    1,
                    LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(9)),
                ),
                instruction(2, call([values[0], values[1]])),
                instruction(3, call([values[0], values[1]])),
                instruction(4, call([values[2], values[3]])),
            ],
            terminator: LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge,
                value: LegalizedScalarReturnValue::Unit,
                work: vec![WorkSettlement {
                    site: PsiProvenance::Edge(edge),
                    units: 1,
                }],
                effect: effect(5),
                ownership: vec![OwnershipEvent::Cleanup(Vec::new())],
            }),
        }],
        ownership_frontier_facts: Vec::new(),
    });
    plan
}

fn completion_receipts() -> Vec<terminal_psi::CompletionReceipt> {
    vec![
        terminal_psi::CompletionReceipt {
            claim: id(1),
            argument_index: 0,
            path: Vec::new(),
        },
        terminal_psi::CompletionReceipt {
            claim: id(2),
            argument_index: 1,
            path: Vec::new(),
        },
    ]
}

#[test]
fn empty_call_ownership_events_remain_bound_to_the_original_call_origin() {
    let mut installed = installed_provider_plan();
    let call = structural_call_mut(&mut installed);
    call.claim_transfers.clear();
    let NativeCallOrigin::InstalledProvider {
        completion_claim_sources,
        completion_receipts,
        ..
    } = &mut call.source
    else {
        panic!("installed fixture origin");
    };
    completion_claim_sources.clear();
    completion_receipts.clear();
    call.validate_source(&[]).unwrap();
    call.validate_source(&[OwnershipEvent::ClaimCompletion(Vec::new())])
        .unwrap();
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimTransfer(Vec::new())]),
        Err(LegalizedCallSourceError::OwnershipMismatch)
    );

    call.source = NativeCallOrigin::Authored;
    call.validate_source(&[]).unwrap();
    call.validate_source(&[OwnershipEvent::ClaimTransfer(Vec::new())])
        .unwrap();
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimCompletion(Vec::new())]),
        Err(LegalizedCallSourceError::OwnershipMismatch)
    );
}

#[test]
fn installed_call_origin_retains_projected_operands_and_structural_receipt_ordinals() {
    let mut installed = installed_provider_plan();
    let call = structural_call_mut(&mut installed);
    let path = vec![terminal_psi::StructuralPathSegment::Field(
        terminal_psi::TerminalIdentityIndex(1),
    )];
    let LegalizedScalarArgument::Structural {
        semantic, target, ..
    } = &mut call.arguments[0]
    else {
        panic!("structural fixture argument");
    };
    semantic.path = path.clone();
    target.path = path.clone();
    let NativeCallOrigin::InstalledProvider {
        completion_claim_sources,
        ..
    } = &mut call.source
    else {
        panic!("installed fixture origin");
    };
    completion_claim_sources[0].entry.as_mut().unwrap().path = path;
    let placement = call.call_plan.parameters[0].clone();
    call.arguments.insert(
        0,
        LegalizedScalarArgument::Scalar {
            source: id(7),
            placement,
        },
    );
    let ownership = [OwnershipEvent::ClaimCompletion(completion_receipts())];
    call.validate_source(&ownership)
        .expect("projected structural receipt survives a scalar prefix");

    let mut wrong_projection = call.clone();
    let LegalizedScalarArgument::Structural { target, .. } = &mut wrong_projection.arguments[1]
    else {
        panic!("structural fixture argument");
    };
    target.path.clear();
    assert_eq!(
        wrong_projection.validate_source(&ownership),
        Err(LegalizedCallSourceError::ArgumentSignatureMismatch)
    );

    let mut wrong_receipt = call.clone();
    let NativeCallOrigin::InstalledProvider {
        completion_receipts,
        ..
    } = &mut wrong_receipt.source
    else {
        panic!("installed fixture origin");
    };
    completion_receipts[0].argument_index = 1;
    assert!(wrong_receipt.validate_source(&ownership).is_err());

    let mut interleaved = call.clone();
    interleaved.arguments.swap(0, 1);
    assert_eq!(
        interleaved.validate_source(&ownership),
        Err(LegalizedCallSourceError::ArgumentSignatureMismatch)
    );
}

#[test]
fn structural_call_source_validation_preserves_installed_completion_custody() {
    let mut authored = call_aware_plan();
    structural_call_mut(&mut authored)
        .validate_source(&[OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])])
        .expect("authored call source");

    let mut installed = installed_provider_plan();
    let call = structural_call_mut(&mut installed);
    call.validate_source(&[OwnershipEvent::ClaimCompletion(completion_receipts())])
        .expect("installed provider source");
    assert_ne!(
        legalized_operation_plan_identity(&authored),
        legalized_operation_plan_identity(&installed)
    );

    let mut with_unrelated_source = installed.clone();
    let call = structural_call_mut(&mut with_unrelated_source);
    let NativeCallOrigin::InstalledProvider {
        completion_claim_sources,
        ..
    } = &mut call.source
    else {
        panic!("installed provider source");
    };
    completion_claim_sources.push(CompletionClaimSource {
        claim: id(3),
        entry: Some(EntryClaim {
            claim: id(3),
            input: id(1),
            path: Vec::new(),
        }),
        content: None,
    });
    call.validate_source(&[OwnershipEvent::ClaimCompletion(completion_receipts())])
        .expect("unrelated retained caller source is permitted");
    assert_ne!(
        legalized_operation_plan_identity(&installed),
        legalized_operation_plan_identity(&with_unrelated_source)
    );

    let mut duplicate_source = installed.clone();
    let call = structural_call_mut(&mut duplicate_source);
    let NativeCallOrigin::InstalledProvider {
        completion_claim_sources,
        ..
    } = &mut call.source
    else {
        panic!("installed provider source");
    };
    completion_claim_sources.push(completion_claim_sources[0].clone());
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimCompletion(completion_receipts())]),
        Err(LegalizedCallSourceError::CompletionEvidenceMismatch)
    );

    let mut wrong_ownership = installed.clone();
    let call = structural_call_mut(&mut wrong_ownership);
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimTransfer(vec![id(1), id(2)])]),
        Err(LegalizedCallSourceError::OwnershipMismatch)
    );

    let mut wrong_provider = installed;
    let call = structural_call_mut(&mut wrong_provider);
    let NativeCallOrigin::InstalledProvider { provider, .. } = &mut call.source else {
        panic!("installed provider source");
    };
    provider.candidate = id(3);
    assert_eq!(
        call.validate_source(&[OwnershipEvent::ClaimCompletion(completion_receipts())]),
        Err(LegalizedCallSourceError::ProviderIdentityMismatch)
    );
}

#[test]
fn empty_aggregate_result_shape_retains_some_and_rejects_fake_storage() {
    for policy in [
        CallingPolicy::SystemVAMD64,
        CallingPolicy::MicrosoftX64,
        CallingPolicy::Aapcs64,
    ] {
        let mut plan = scalar_call_unit_plan();
        let call = call(&mut plan);
        call.arguments.clear();
        call.call_plan = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(0, 1)),
            },
        )
        .unwrap();
        call.result_placement = call.call_plan.result.clone();
        call.structural_result = Some(terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            claims: Vec::new(),
        });
        assert_eq!(call.validate_shape(), Ok(()));
        assert_eq!(
            call.result_placement.as_ref().unwrap().shape,
            ValueShape::integer(0, 1)
        );
        assert!(call.result_placement.as_ref().unwrap().locations.is_empty());
        for mutation in 0..8 {
            let mut changed = call.clone();
            match mutation {
                0 => changed.call_plan.result.as_mut().unwrap().shape.alignment = 2,
                1 => changed.call_plan.result.as_mut().unwrap().shape.alignment = 0,
                2 => changed.call_plan.result.as_mut().unwrap().locations.push(
                    calling_conventions::ValueLocation::Register {
                        register: calling_conventions::MachineRegister::X86Rax,
                        value_byte_offset: 0,
                        byte_size: 0,
                    },
                ),
                3 => {
                    changed.call_plan.result.as_mut().unwrap().shape.class =
                        calling_conventions::ValueClass::Float
                }
                4 => changed.call_plan.result = None,
                5 => changed.structural_result = None,
                6 => {
                    changed.result_placement = None;
                    assert!(changed.validate_shape().is_err());
                    continue;
                }
                _ => changed.call_plan.result.as_mut().unwrap().locations.push(
                    calling_conventions::ValueLocation::Stack {
                        stack_byte_offset: 0,
                        value_byte_offset: 0,
                        byte_size: 0,
                        alignment: 1,
                    },
                ),
            }
            changed.result_placement = changed.call_plan.result.clone();
            assert!(
                changed.validate_shape().is_err(),
                "{policy:?} mutation {mutation}"
            );
        }
        let mut unit = call.clone();
        unit.structural_result = None;
        unit.result_placement = None;
        unit.call_plan.result = None;
        assert_eq!(unit.validate_shape(), Ok(()));
        assert_ne!(unit, *call);
    }
}

fn call(plan: &mut LegalizedOperationPlan) -> &mut LegalizedScalarCall {
    plan.scalar_functions[0].blocks[0]
        .instructions
        .iter_mut()
        .find_map(|operation| {
            if let LegalizedScalarInstructionKind::Call(call) = &mut operation.kind {
                Some(call)
            } else {
                None
            }
        })
        .expect("call fixture")
}

#[test]
fn register_call_shape_admits_actual_arity_and_rejects_roster_corruption() {
    for arity in 0..=6 {
        let mut plan = scalar_call_unit_plan();
        let call = call(&mut plan);
        let argument = call.arguments[0].clone();
        call.call_plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); arity],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        call.arguments = call
            .call_plan
            .parameters
            .iter()
            .map(|placement| LegalizedScalarArgument::Scalar {
                placement: placement.clone(),
                source: argument.scalar_source().unwrap(),
            })
            .collect();
        assert_eq!(call.validate_shape(), Ok(()));
        if arity > 0 {
            let mut corrupt = call.clone();
            corrupt.result_placement.as_mut().unwrap().locations.clear();
            assert!(corrupt.validate_shape().is_err());
            let mut corrupt = call.clone();
            corrupt.arguments.pop();
            assert!(corrupt.validate_shape().is_err());
            let mut corrupt = call.clone();
            scalar_argument_mut(&mut corrupt.arguments[0])
                .1
                .locations
                .clear();
            assert!(corrupt.validate_shape().is_err());
        }
    }
}

#[test]
fn fixed_scalar_argument_widths_require_exact_register_geometry() {
    for policy in [CallingPolicy::SystemVAMD64, CallingPolicy::MicrosoftX64] {
        for width in [1, 2, 4, 8] {
            let mut plan = scalar_call_unit_plan();
            let call = call(&mut plan);
            let source = call.arguments[0].scalar_source().unwrap();
            call.call_plan = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![ValueShape::integer(width, width as u16)],
                    result: None,
                },
            )
            .unwrap();
            call.result_placement = None;
            call.arguments = vec![LegalizedScalarArgument::Scalar {
                source,
                placement: call.call_plan.parameters[0].clone(),
            }];
            assert_eq!(call.validate_shape(), Ok(()));
            for mutation in 0..5 {
                let mut changed = call.clone();
                let placement = &mut changed.call_plan.parameters[0];
                match mutation {
                    0 => placement.shape.alignment = if width == 1 { 2 } else { 1 },
                    1 => placement.shape.byte_size = 3,
                    2 => {
                        let calling_conventions::ValueLocation::Register { byte_size, .. } =
                            &mut placement.locations[0]
                        else {
                            panic!("register");
                        };
                        *byte_size = if width == 1 { 2 } else { 1 };
                    }
                    3 => {
                        let calling_conventions::ValueLocation::Register {
                            value_byte_offset, ..
                        } = &mut placement.locations[0]
                        else {
                            panic!("register");
                        };
                        *value_byte_offset = 1;
                    }
                    _ => placement.locations.push(placement.locations[0]),
                }
                *scalar_argument_mut(&mut changed.arguments[0]).1 = placement.clone();
                assert!(
                    changed.validate_shape().is_err(),
                    "width {width}, mutation {mutation}"
                );
            }
            call.call_plan.result = Some(call.call_plan.parameters[0].clone());
            call.result_placement = call.call_plan.result.clone();
            assert!(
                call.validate_shape().is_ok(),
                "fixed integer result geometry is valid; source replay checks supported scalar semantics"
            );
        }
    }
}

#[test]
fn narrow_scalar_stack_arguments_require_exact_payload_geometry() {
    for policy in [
        CallingPolicy::SystemVAMD64,
        CallingPolicy::MicrosoftX64,
        CallingPolicy::Aapcs64,
    ] {
        for width in [1, 2] {
            let mut plan = scalar_call_unit_plan();
            let call = call(&mut plan);
            let source = call.arguments[0].scalar_source().unwrap();
            call.call_plan = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![ValueShape::integer(width, width as u16); 9],
                    result: None,
                },
            )
            .unwrap();
            call.result_placement = None;
            call.arguments = call
                .call_plan
                .parameters
                .iter()
                .map(|placement| LegalizedScalarArgument::Scalar {
                    source,
                    placement: placement.clone(),
                })
                .collect();
            assert!(matches!(
                call.call_plan.parameters[8].locations.as_slice(),
                [calling_conventions::ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                }] if *byte_size == width
            ));
            assert_eq!(call.validate_shape(), Ok(()));
            for mutation in 0..12 {
                let mut changed = call.clone();
                let placement = &mut changed.call_plan.parameters[8];
                let calling_conventions::ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                } = &mut placement.locations[0]
                else {
                    panic!("ninth scalar argument must be on the stack");
                };
                match mutation {
                    0 => *byte_size = 0,
                    1 => *byte_size = width - 1,
                    2 => *byte_size = 4,
                    3 => *value_byte_offset = 1,
                    4 => *alignment = 0,
                    5 => *alignment = 3,
                    6 => *alignment = (width - 1) as u16,
                    7 => {
                        *alignment = 2;
                        *stack_byte_offset = 1;
                    }
                    8 => placement.shape = ValueShape::float(width as u16),
                    9 => placement.shape.alignment = (width * 2) as u16,
                    10 => placement.locations.clear(),
                    _ => placement.locations.push(placement.locations[0]),
                }
                *scalar_argument_mut(&mut changed.arguments[8]).1 = placement.clone();
                assert!(
                    changed.validate_shape().is_err(),
                    "policy {policy:?}, width {width}, mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn boolean_argument_shape_preserves_width_and_unit_result_absence() {
    for policy in [CallingPolicy::SystemVAMD64, CallingPolicy::MicrosoftX64] {
        let mut plan = scalar_call_unit_plan();
        let call = call(&mut plan);
        let source = call.arguments[0].scalar_source().unwrap();
        call.call_plan = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![ValueShape::integer(1, 1)],
                result: None,
            },
        )
        .unwrap();
        call.result_placement = None;
        call.arguments = vec![LegalizedScalarArgument::Scalar {
            source,
            placement: call.call_plan.parameters[0].clone(),
        }];
        assert_eq!(call.validate_shape(), Ok(()));
        let mut changed = call.clone();
        changed.result_placement = Some(changed.call_plan.parameters[0].clone());
        assert!(changed.validate_shape().is_err());
        let mut changed = call.clone();
        let calling_conventions::ValueLocation::Register { byte_size, .. } =
            &mut changed.call_plan.parameters[0].locations[0]
        else {
            unreachable!()
        };
        *byte_size = 8;
        *scalar_argument_mut(&mut changed.arguments[0]).1 = changed.call_plan.parameters[0].clone();
        assert!(changed.validate_shape().is_err());
        let mut changed = call.clone();
        changed.call_plan.result = Some(changed.call_plan.parameters[0].clone());
        changed.result_placement = changed.call_plan.result.clone();
        assert!(
            changed.validate_shape().is_ok(),
            "one-byte result geometry is valid; semantic replay distinguishes Boolean from u8"
        );
    }
}

#[test]
fn fixed_integer_result_widths_reject_coherent_fragment_corruption() {
    for width in [1, 2, 4, 8] {
        let mut plan = scalar_call_unit_plan();
        let call = call(&mut plan);
        call.arguments.clear();
        call.call_plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(width, width as u16)),
            },
        )
        .unwrap();
        call.result_placement = call.call_plan.result.clone();
        assert_eq!(call.validate_shape(), Ok(()));
        for mutation in 0..3 {
            let mut changed = call.clone();
            let placement = changed.call_plan.result.as_mut().unwrap();
            let calling_conventions::ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } = &mut placement.locations[0]
            else {
                panic!("register result")
            };
            match mutation {
                0 => *value_byte_offset = 1,
                1 => *byte_size = width + 1,
                2 => placement.shape = ValueShape::integer(3, 1),
                _ => unreachable!(),
            }
            changed.result_placement = changed.call_plan.result.clone();
            assert!(
                changed.validate_shape().is_err(),
                "width {width}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn register_call_identity_retains_argument_length_order_and_placement() {
    let plan = scalar_call_unit_plan();
    let identity = legalized_operation_plan_identity(&plan);
    for mutation in 0..4 {
        let mut proposed = plan.clone();
        let call = call(&mut proposed);
        match mutation {
            0 => {
                call.arguments.pop();
            }
            1 => call.arguments.swap(0, 1),
            2 => *scalar_argument_mut(&mut call.arguments[0]).0 = id(999),
            _ => scalar_argument_mut(&mut call.arguments[0])
                .1
                .locations
                .clear(),
        }
        assert_ne!(legalized_operation_plan_identity(&proposed), identity);
    }
}
