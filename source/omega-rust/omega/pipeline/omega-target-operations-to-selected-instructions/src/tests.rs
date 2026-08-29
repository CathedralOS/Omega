use super::*;
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, CompletionClaimSource,
};
use omega_isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, validate_x86_64_register_constraint_catalog,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
use omega_legalized_operations::legalized_operation_plan_identity;
use omega_optimization_unit::PsiOptimizationUnit;
use omega_register_model::{
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    validate_physical_register_model,
};
use omega_selected_instructions::{
    SelectedConstraintKeys, SelectedInstructionId, SelectedSelectionConstraints,
};
use omega_target_operations::TargetOperationPlan;
use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, EdgeId, FuelScheduleIdentity, IntegerSign, MachineId,
    OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use psi_terminal::{
    BindingRelevance, BoundaryMachineDeclaration, CompletionReceipt, EntryClaim,
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderSignatureParameter,
    ProviderUnitRefinement, ProviderUnitSignature, SemanticFingerprint, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalPsiIdentity, VocabularyMarker,
};
use std::sync::Arc;

fn structural_call_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let caller_block = BlockId::new(1).unwrap();
    let callee_block = BlockId::new(2).unwrap();
    let caller_places = [PlaceId::new(1).unwrap(), PlaceId::new(2).unwrap()];
    let callee_places = [PlaceId::new(3).unwrap(), PlaceId::new(4).unwrap()];
    let structural_type = StructuralTypeId::new(1).unwrap();
    let call = OperationId::new(1).unwrap();
    let caller_return = EdgeId::new(1).unwrap();
    let callee_return = EdgeId::new(2).unwrap();
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: vec![psi_core::StructuralDomainId::new(1).unwrap()],
    };
    let abstract_plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x51; 32]),
        },
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "Extent".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "base".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            psi_core::IntegerType::address(64).unwrap(),
                        )),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "length".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            psi_core::IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    },
                ],
            },
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: caller_places
                    .into_iter()
                    .enumerate()
                    .map(|(position, place)| parameter(place, position as u32))
                    .collect(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::CallUnit {
                        psi_operation: call,
                        callee,
                        structural_arguments: caller_places
                            .into_iter()
                            .map(|place| StructuralArgument {
                                place,
                                access: StructuralAccess::Owned,
                                path: Vec::new(),
                            })
                            .collect(),
                        claim_transfers: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: caller_return,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_block,
                parameters: Vec::new(),
                structural_parameters: callee_places
                    .into_iter()
                    .enumerate()
                    .map(|(position, place)| parameter(place, position as u32))
                    .collect(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: callee_return,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    };
    let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        omega_target::NativeTarget::uefi_x64(),
    )
    .unwrap();
    let unit = qualified_fixture_unit(
        omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap(),
        structural_type,
    );
    (abstract_plan, target, unit)
}

fn qualified_fixture_unit(
    mut unit: PsiOptimizationUnit,
    carrier: StructuralTypeId,
) -> PsiOptimizationUnit {
    unit.structural_domains = Arc::from([psi_terminal::StructuralDomainDeclaration {
        id: psi_core::StructuralDomainId::new(1).unwrap(),
        semantic_domain: psi_core::DomainSemanticId::new(1).unwrap(),
        identity: "ExtentDomain".into(),
        carrier,
        content_projection: None,
    }]);
    unit.identity = omega_optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    unit
}

fn installed_provider_legalization_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut abstract_plan, mut target, _) = structural_call_fixture();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let callee = abstract_plan.functions[1].machine;
    let operation = OperationId::new(1).unwrap();
    let caller_parameters = abstract_plan.functions[0].structural_parameters.clone();
    let structural_type = caller_parameters[0].structural_type;
    for function in &mut abstract_plan.functions {
        for parameter in &mut function.structural_parameters {
            parameter.multiplicity = StructuralMultiplicity::Affine;
        }
    }
    let caller_claims = caller_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| EntryClaim {
            claim: ClaimId::new(index as u64 + 1).unwrap(),
            input: parameter.place,
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let callee_claims = abstract_plan.functions[1]
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| EntryClaim {
            claim: ClaimId::new(index as u64 + 1).unwrap(),
            input: parameter.place,
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let arguments = caller_parameters
        .iter()
        .map(|parameter| StructuralArgument {
            place: parameter.place,
            access: parameter.access,
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let completion_sources = caller_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    let receipts = caller_claims
        .iter()
        .enumerate()
        .map(|(index, claim)| CompletionReceipt {
            claim: claim.claim,
            argument_index: index as u32,
        })
        .collect::<Vec<_>>();
    let provider = ProviderCandidateConformance {
        boundary,
        requirement_identity: "ProgramEntry::enter".into(),
        provider_identity: "UefiProgramProvider".into(),
        candidate_identity: "UefiProgramProvider::enter".into(),
        candidate: callee,
        signature: ProviderUnitSignature {
            parameters: caller_parameters
                .iter()
                .map(|parameter| ProviderSignatureParameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                    structural_type: parameter.structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: parameter.access,
                    qualifications: parameter.qualifications.clone(),
                })
                .collect(),
        },
        refinement: ProviderUnitRefinement {
            positional_parameters: vec![
                ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                },
                ProviderParameterRefinement {
                    boundary_index: 1,
                    candidate_index: 1,
                },
            ],
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    };
    abstract_plan.boundary_machines = vec![BoundaryMachineDeclaration {
        id: boundary,
        identity: "ProgramEntry::enter".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: caller_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| StructuralParameterDeclaration {
                place: PlaceId::new(index as u64 + 5).unwrap(),
                position: parameter.position,
                is_self: parameter.is_self,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: parameter.access,
                qualifications: parameter.qualifications.clone(),
            })
            .collect(),
        result: None,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    abstract_plan.provider_candidates = vec![provider.clone()];
    abstract_plan.functions[0].entry_claims = caller_claims.clone();
    abstract_plan.functions[0].operations[0] = AbstractOperation::BoundaryCall {
        psi_operation: operation,
        result: None,
        boundary,
        arguments: Vec::new(),
        structural_arguments: arguments.clone(),
        completion_claim_sources: completion_sources.clone(),
        completion_receipts: receipts.clone(),
    };
    abstract_plan.functions[1].entry_claims = callee_claims;
    for function in &mut target.functions {
        let omega_target_operations::TargetOperation::UnitBody(body) = &mut function.operation
        else {
            continue;
        };
        for parameter in &mut body.parameters {
            parameter.multiplicity = StructuralMultiplicity::Affine;
        }
    }
    let omega_target_operations::TargetOperation::UnitBody(caller_body) =
        &mut target.functions[0].operation
    else {
        panic!("caller Unit body");
    };
    let omega_target_operations::TargetUnitOperation::Call {
        arguments: target_arguments,
        ..
    } = caller_body.operations[0].clone()
    else {
        panic!("authored structural call fixture");
    };
    caller_body.operations[0] =
        omega_target_operations::TargetUnitOperation::InstalledProviderCall {
            psi_operation: operation,
            boundary,
            provider,
            source_arguments: arguments,
            arguments: target_arguments,
            claim_transfers: receipts
                .iter()
                .map(|receipt| psi_terminal::ClaimTransfer {
                    claim: receipt.claim,
                    argument_index: receipt.argument_index,
                })
                .collect(),
            completion_claim_sources: completion_sources,
            completion_receipts: receipts,
        };
    let unit = qualified_fixture_unit(
        omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .expect("installed provider optimization seed"),
        structural_type,
    );
    (abstract_plan, target, unit)
}

fn claim_completion_settlement_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut abstract_plan, mut target, _) = installed_provider_legalization_fixture();
    abstract_plan.provider_candidates.clear();
    abstract_plan.boundary_machines[0]
        .structural_parameters
        .truncate(1);
    abstract_plan.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    for parameter in &mut abstract_plan.functions[0].structural_parameters {
        parameter.multiplicity = StructuralMultiplicity::Linear;
    }
    let AbstractOperation::BoundaryCall {
        boundary,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
        ..
    } = abstract_plan.functions[0].operations[0].clone()
    else {
        panic!("boundary-call fixture");
    };
    let return_operation = abstract_plan.functions[0].operations[1].clone();
    let second_operation = OperationId::new(2).unwrap();
    abstract_plan.functions[0].operations = vec![
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(1).unwrap(),
            result: None,
            boundary,
            arguments: Vec::new(),
            structural_arguments: vec![structural_arguments[0].clone()],
            completion_claim_sources: completion_claim_sources.clone(),
            completion_receipts: vec![completion_receipts[0]],
        },
        AbstractOperation::BoundaryCall {
            psi_operation: second_operation,
            result: None,
            boundary,
            arguments: Vec::new(),
            structural_arguments: vec![structural_arguments[1].clone()],
            completion_claim_sources: completion_claim_sources.clone(),
            completion_receipts: vec![CompletionReceipt {
                claim: completion_receipts[1].claim,
                argument_index: 0,
            }],
        },
        return_operation,
    ];
    let omega_target_operations::TargetOperation::UnitBody(body) =
        &mut target.functions[0].operation
    else {
        panic!("caller Unit body");
    };
    for parameter in &mut body.parameters {
        parameter.multiplicity = StructuralMultiplicity::Linear;
    }
    let return_operation = body.operations[1].clone();
    let settlement = |psi_operation, argument, sources, receipts, seed| {
        omega_target_operations::TargetUnitOperation::BoundarySettlement {
            psi_operation,
            boundary,
            provider_execution:
                omega_target_operations::ProviderExecutionBinding::from_execution_record(
                    omega_target_operations::ProviderPlanIdentity::new(seed).unwrap(),
                    seed + 1,
                    seed + 2,
                    seed + 3,
                    seed + 4,
                )
                .unwrap(),
            realization: omega_target_operations::ClaimCompletionOnlyRealization.into(),
            scalar_arguments: Vec::new(),
            arguments: vec![argument],
            byte_sequence_arguments: Vec::new(),
            completion_claim_sources: sources,
            completion_receipts: receipts,
        }
    };
    body.operations = vec![
        settlement(
            OperationId::new(1).unwrap(),
            structural_arguments[0].clone(),
            completion_claim_sources.clone(),
            vec![completion_receipts[0]],
            7,
        ),
        settlement(
            second_operation,
            structural_arguments[1].clone(),
            completion_claim_sources.clone(),
            vec![CompletionReceipt {
                claim: completion_receipts[1].claim,
                argument_index: 0,
            }],
            17,
        ),
        return_operation,
    ];
    target.functions[0].provenance.operations =
        vec![OperationId::new(1).unwrap(), second_operation];
    let unit = qualified_fixture_unit(
        omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .expect("claim-completion settlement optimization seed"),
        abstract_plan.structural_types[0].id,
    );
    (abstract_plan, target, unit)
}

fn microsoft_selection_environment() -> (
    ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
    SelectedSelectionConstraints,
) {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = validate_x86_64_register_constraint_catalog(
        x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    let constraints = SelectedSelectionConstraints {
        keys: SelectedConstraintKeys {
            structural_unit_call: Some(X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR),
            materialize_i64: X86_64_MATERIALIZE_I64,
            copy_i64: X86_64_COPY_I64,
            add_i64: X86_64_ADD_I64,
            subtract_i64: X86_64_SUBTRACT_I64,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            return_i64: X86_64_MICROSOFT_RETURN,
            return_unit: X86_64_MICROSOFT_RETURN_UNIT,
        },
        fixed_inputs: Vec::new(),
    };
    (physical, catalog, constraints)
}

#[test]
fn structural_call_and_terminal_callee_are_produced_and_replayed() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("one whole-root call and its structural callee legalize");
    assert!(legalized.plan().unit_functions.is_empty());
    assert_eq!(legalized.plan().structural_unit_functions.len(), 2);
    assert!(legalized.plan().structural_unit_functions[0].call.is_some());
    assert!(legalized.plan().structural_unit_functions[1].call.is_none());
    assert_eq!(legalized.receipt().function_count(), 2);

    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("bounded Microsoft structural Unit calls select atomically");
    assert!(selected.plan().functions.is_empty());
    assert_eq!(selected.plan().structural_unit_functions.len(), 2);
    let caller = &selected.plan().structural_unit_functions[0];
    let call = caller.call.as_ref().unwrap();
    assert_eq!(call.id, SelectedInstructionId(0));
    assert_eq!(
        call.source,
        omega_legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit
    );
    assert_eq!(
        call.constraint,
        X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR
    );
    assert!(call.arguments.len() == 2 && !call.implicit_uses.is_empty());
    assert_eq!(caller.terminator.instruction.id, SelectedInstructionId(1));
    assert!(caller.terminator.instruction.operands.is_empty());
    assert!(selected.plan().structural_unit_functions[1].call.is_none());
    assert_eq!(selected.receipt().function_count(), 2);
    assert_eq!(selected.receipt().block_count(), 2);
    assert_eq!(selected.receipt().virtual_register_count(), 0);
    assert_eq!(selected.receipt().instruction_count(), 3);
}

#[test]
fn installed_provider_call_legalization_retains_source_and_completion_custody() {
    let (abstract_plan, target, unit) = installed_provider_legalization_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("installed provider call derives and independently replays");
    let call = legalized.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("installed provider call");
    let omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider {
        boundary,
        provider,
        completion_claim_sources,
        completion_receipts,
    } = &call.source
    else {
        panic!("installed provider source kind");
    };
    assert_eq!(*boundary, BoundaryMachineId::new(1).unwrap());
    assert_eq!(provider.candidate, MachineId::new(2).unwrap());
    assert_eq!(completion_claim_sources.len(), 2);
    assert_eq!(completion_receipts.len(), 2);
    assert_eq!(
        call.ownership,
        [omega_optimization_unit::OwnershipEvent::ClaimCompletion(
            vec![ClaimId::new(1).unwrap(), ClaimId::new(2).unwrap(),]
        )]
    );
    call.validate_source()
        .expect("retained installed source remains internally valid");
}

#[test]
fn installed_provider_call_selection_retains_and_hashes_exact_source_custody() {
    let (abstract_plan, target, unit) = installed_provider_legalization_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("installed provider call legalizes");
    let legalized_source = legalized.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("legalized installed call")
        .source
        .clone();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("installed provider call selects through the shared physical ABI");
    let selected_call = selected.plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("selected installed call");
    assert_eq!(selected_call.source, legalized_source);
    assert_eq!(
        selected_call.ownership,
        [omega_optimization_unit::OwnershipEvent::ClaimCompletion(
            vec![ClaimId::new(1).unwrap(), ClaimId::new(2).unwrap()]
        )]
    );
    assert_eq!(selected_call.claim_transfers.len(), 2);

    let selected_identity = selected.receipt().identity();
    let mut corrupted = selected.plan().clone();
    let call = corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("selected installed call");
    let omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider { provider, .. } =
        &mut call.source
    else {
        panic!("installed provider source")
    };
    provider.candidate_identity.push_str("::substituted");
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(matches!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted,),
        Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
    ));

    let mut wrong_kind = selected.plan().clone();
    wrong_kind.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("selected installed call")
        .source = omega_legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit;
    assert_ne!(
        selected_instruction_plan_identity(&wrong_kind),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, wrong_kind,)
            .is_err()
    );

    let mut receipt_tamper = selected.plan().clone();
    let call = receipt_tamper.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("selected installed call");
    let omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider {
        completion_receipts,
        ..
    } = &mut call.source
    else {
        panic!("installed provider source")
    };
    completion_receipts[0].argument_index = 1;
    assert_ne!(
        selected_instruction_plan_identity(&receipt_tamper),
        selected_identity
    );
    assert!(
        validate_selected_instructions(
            &legalized,
            &constraints,
            &physical,
            &catalog,
            receipt_tamper,
        )
        .is_err()
    );
}

#[test]
fn claim_completion_settlement_is_ordered_metadata_without_instruction_ids() {
    let (abstract_plan, target, unit) = claim_completion_settlement_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("two-Extent claim-completion settlement legalizes and replays");
    let caller = &legalized.plan().structural_unit_functions[0];
    assert!(caller.call.is_none());
    assert_eq!(caller.boundary_settlements.len(), 2);
    assert_eq!(
        caller.boundary_settlements[0]
            .completion_claim_sources
            .len(),
        2
    );
    assert_eq!(caller.boundary_settlements[0].completion_receipts.len(), 1);
    assert_eq!(
        caller.boundary_settlements[0].ownership,
        [omega_optimization_unit::OwnershipEvent::ClaimCompletion(
            vec![ClaimId::new(1).unwrap()]
        )]
    );
    assert_eq!(
        caller.boundary_settlements[1].ownership,
        [omega_optimization_unit::OwnershipEvent::ClaimCompletion(
            vec![ClaimId::new(2).unwrap()]
        )]
    );

    let legalized_identity = legalized.receipt().identity();
    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .boundary_settlements
        .swap(0, 1);
    assert_ne!(
        legalized_operation_plan_identity(&corrupted),
        legalized_identity
    );
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted).is_err());

    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("metadata settlement selects with only the return instruction");
    let selected_caller = &selected.plan().structural_unit_functions[0];
    assert!(selected_caller.call.is_none());
    assert_eq!(
        selected_caller.boundary_settlements,
        caller.boundary_settlements
    );
    assert_eq!(
        selected_caller.terminator.instruction.id,
        SelectedInstructionId(0)
    );

    let selected_identity = selected.receipt().identity();
    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0].boundary_settlements[0].provider_execution =
        omega_target_operations::ProviderExecutionBinding::from_execution_record(
            omega_target_operations::ProviderPlanIdentity::new(23).unwrap(),
            29,
            31,
            37,
            41,
        )
        .unwrap();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted,)
            .is_err()
    );
}

#[test]
fn selected_structural_replay_rejects_abi_constraint_and_semantic_custody_mutations() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog).unwrap();
    let selected_identity = selected.receipt().identity();

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .abi
        .layout
        .outgoing_frame_byte_count -= 8;
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .implicit_uses
        .pop();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0].abi.parameters[0]
        .semantic
        .qualifications[0] = psi_core::StructuralDomainId::new(2).unwrap();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .effect
        .output += 1;
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut missing_key = constraints.clone();
    missing_key.keys.structural_unit_call = None;
    assert!(select_instructions(&legalized, &missing_key, &physical, &catalog).is_err());

    let linux_target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        omega_target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let linux_legalized = legalize_target_operations(&linux_target, &abstract_plan, &unit).unwrap();
    assert!(select_instructions(&linux_legalized, &constraints, &physical, &catalog).is_err());

    let mut wrong_shape = abstract_plan.clone();
    let StructuralTypeShape::Record { fields } = &mut wrong_shape.structural_types[0].shape else {
        unreachable!()
    };
    fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Integer(
        psi_core::IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
    ));
    let wrong_target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
        &wrong_shape,
        omega_target::NativeTarget::uefi_x64(),
    )
    .unwrap();
    let wrong_unit = qualified_fixture_unit(
        omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &wrong_shape,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap(),
        wrong_shape.structural_types[0].id,
    );
    let wrong_legalized =
        legalize_target_operations(&wrong_target, &wrong_shape, &wrong_unit).unwrap();
    assert!(select_instructions(&wrong_legalized, &constraints, &physical, &catalog).is_err());
}

#[test]
fn independent_replay_rejects_placement_effect_and_roster_erasure() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .call_plan
        .shadow_bytes += 8;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut corrupted_target = target.clone();
    let omega_target_operations::TargetOperation::UnitBody(callee) =
        &mut corrupted_target.functions[1].operation
    else {
        panic!("fixture callee is Unit")
    };
    callee.call_plan.shadow_bytes += 8;
    assert!(legalize_target_operations(&corrupted_target, &abstract_plan, &unit).is_err());

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .arguments[0]
        .target
        .source_byte_offset = 1;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .effect
        .output += 1;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut erased = legalized.plan().clone();
    erased.structural_unit_functions.clear();
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, erased,).is_err());
}
