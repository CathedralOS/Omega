//! A caller that owns an aggregate actual, invokes a callee returning the
//! same structural type, and returns that result itself.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use target_operations::TargetOperationPlan;
use terminal_psi::{
    CrashRouteBucket, SemanticFingerprint, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralReferenceResultSource,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalPsiIdentity, VocabularyMarker,
};

pub(in crate::tests) fn fixture(
    native: target::NativeTarget,
    crash_routes: Vec<CrashRouteBucket>,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let primitive = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let result = |identity| terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: PlaceId::new(identity).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let declaration = terminal_psi::StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: PlaceId::new(99).unwrap(),
        structural_type: array,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let return_structural = |source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(1).unwrap(),
        source: PlaceId::new(source).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let call = |identity, source| AbstractOperation::CallStructural {
        psi_operation: OperationId::new(identity).unwrap(),
        result: result(identity),
        callee: callee_machine,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(source).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: crash_routes.clone(),
        selected_evidence: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x50; 32]),
        },
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "[u64; 2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: primitive,
                    length: 2,
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Structural(declaration.clone()),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(10).unwrap(),
                        result: ValueId::new(10).unwrap(),
                        scalar_type: unsigned,
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::EstablishScalarArray {
                        psi_operation: OperationId::new(11).unwrap(),
                        result: result(11),
                        elements: vec![ValueId::new(10).unwrap(), ValueId::new(10).unwrap()],
                    },
                    call(12, 11),
                    return_structural(12),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(20).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: array,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: AbstractFunctionResult::Structural(declaration),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(20)],
            },
        ],
    };
    let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    for function in &mut unit.functions {
        function.verified_contract = Some(terminal_psi::MachineContract {
            id: semantic_vocabulary::ContractId::new(1).unwrap(),
            crash_routes: crash_routes.clone(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        });
    }
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    (plan, targeted, unit)
}

/// A caller that owns a record carrier with one reference leaf, moves it into
/// a crash-declaring callee returning the same carrier, and returns the result
/// itself. The callee's declared result maps the `reference` leaf back to the
/// owned ingress parameter, so the call's custody rides the reference-result
/// rule rather than the plain structural families.
pub(in crate::tests) fn reference_fixture(
    native: target::NativeTarget,
    crash_routes: Vec<CrashRouteBucket>,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let primitive = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let carrier = StructuralTypeId::new(3).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let caller_place = PlaceId::new(1).unwrap();
    let callee_place = PlaceId::new(20).unwrap();
    let result_place = PlaceId::new(12).unwrap();
    let leaf_path = || vec![StructuralPathSegment::Field("reference".into())];
    let ingress = |place| StructuralArgument {
        place,
        path: vec![
            StructuralPathSegment::Field("reference".into()),
            StructuralPathSegment::Referent,
        ],
        access: StructuralAccess::MutableBorrow,
    };
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let declaration = |place, source| StructuralResultDeclaration {
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![StructuralReferenceResultSource {
            path: leaf_path(),
            source,
        }],
    };
    let result = |place| terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let return_structural = |edge, source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(edge).unwrap(),
        source,
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x51; 32]),
        },
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: reference,
                identity: "&mut u64".into(),
                shape: StructuralTypeShape::Reference {
                    referent: primitive,
                    access: StructuralAccess::MutableBorrow,
                },
            },
            StructuralTypeDeclaration {
                id: carrier,
                identity: "Carrier".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "payload".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(unsigned),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "reference".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(reference),
                        },
                    ],
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(97).unwrap(),
                    ingress(caller_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::CallStructural {
                        psi_operation: OperationId::new(12).unwrap(),
                        result: result(result_place),
                        callee: callee_machine,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: caller_place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        }],
                        claim_transfers: Vec::new(),
                        returned_claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: crash_routes.clone(),
                        selected_evidence: Vec::new(),
                    },
                    return_structural(1, result_place),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(98).unwrap(),
                    ingress(callee_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(2, callee_place)],
            },
        ],
    };
    let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    for function in &mut unit.functions {
        function.verified_contract = Some(terminal_psi::MachineContract {
            id: semantic_vocabulary::ContractId::new(1).unwrap(),
            crash_routes: crash_routes.clone(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        });
    }
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    (plan, targeted, unit)
}

/// A caller that owns a sum carrier whose `Some` case holds one reference
/// leaf, moves it into a crash-declaring callee returning the same carrier,
/// and returns the result itself. The callee's declared result maps the
/// case-field `reference` leaf back to the owned ingress parameter, so the
/// call's custody rides the reference-result rule through union leaf paths.
pub(in crate::tests) fn sum_reference_fixture(
    native: target::NativeTarget,
    crash_routes: Vec<CrashRouteBucket>,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let primitive = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let carrier = StructuralTypeId::new(3).unwrap();
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let caller_machine = MachineId::new(1).unwrap();
    let callee_machine = MachineId::new(2).unwrap();
    let caller_place = PlaceId::new(1).unwrap();
    let callee_place = PlaceId::new(20).unwrap();
    let result_place = PlaceId::new(12).unwrap();
    let leaf_path = || vec![StructuralPathSegment::Field("reference".into())];
    let ingress = |place| StructuralArgument {
        place,
        path: vec![
            StructuralPathSegment::Field("reference".into()),
            StructuralPathSegment::Referent,
        ],
        access: StructuralAccess::MutableBorrow,
    };
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let declaration = |place, source| StructuralResultDeclaration {
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![StructuralReferenceResultSource {
            path: leaf_path(),
            source,
        }],
    };
    let result = |place| terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let block = |raw| AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: BlockId::new(raw).unwrap(),
        parameters: Vec::new(),
        operation_offset: 0,
    };
    let return_structural = |edge, source| AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(edge).unwrap(),
        source,
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x52; 32]),
        },
        entry: caller_machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "u64".into(),
                shape: StructuralTypeShape::PrimitiveScalar(unsigned),
            },
            StructuralTypeDeclaration {
                id: reference,
                identity: "&mut u64".into(),
                shape: StructuralTypeShape::Reference {
                    referent: primitive,
                    access: StructuralAccess::MutableBorrow,
                },
            },
            StructuralTypeDeclaration {
                id: carrier,
                identity: "MaybeRef".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![
                        terminal_psi::StructuralCaseDeclaration {
                            id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                            identity: "None".into(),
                            fields: Vec::new(),
                        },
                        terminal_psi::StructuralCaseDeclaration {
                            id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                            identity: "Some".into(),
                            fields: vec![StructuralFieldDeclaration {
                                id: StructuralFieldId::new(2).unwrap(),
                                identity: "reference".into(),
                                relevance: terminal_psi::BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Structural(reference),
                            }],
                        },
                    ],
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller_machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(97).unwrap(),
                    ingress(caller_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(1)],
                operations: vec![
                    AbstractOperation::CallStructural {
                        psi_operation: OperationId::new(12).unwrap(),
                        result: result(result_place),
                        callee: callee_machine,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: caller_place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        }],
                        claim_transfers: Vec::new(),
                        returned_claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: crash_routes.clone(),
                        selected_evidence: Vec::new(),
                    },
                    return_structural(1, result_place),
                ],
            },
            AbstractFunction {
                machine: callee_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place)],
                result: AbstractFunctionResult::Structural(declaration(
                    PlaceId::new(98).unwrap(),
                    ingress(callee_place),
                )),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(2)],
                operations: vec![return_structural(2, callee_place)],
            },
        ],
    };
    // The target plan is authored literally rather than lowered: the
    // abstract→target lowering of a sum-typed reference carrier is still a
    // pending admission upstream, while every layer below it replays this
    // exact emission.
    let shape = calling_conventions::ValueShape::integer(4, 4);
    let call_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(native),
        &calling_conventions::CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .unwrap();
    let parameter_placement = call_plan.parameters[0].clone();
    let parameter_row = |place| target_operations::TargetStructuralParameter {
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        projected_qualifications: Vec::new(),
        shape,
        placement: parameter_placement.clone(),
    };
    let sum_layout = calling_conventions::evaluate_conventional_sum_layout(
        &[],
        &[
            Vec::new(),
            vec![calling_conventions::ValueShape::integer(0, 1)],
        ],
    )
    .unwrap();
    let operation_result = |place| terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let caller_home = || target_operations::TargetStructuralHomeRequirement {
        origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
            operation: OperationId::new(12).unwrap(),
            result: operation_result(result_place),
        },
        layout: target_operations::TargetStructuralHomeLayout::Sum(sum_layout.clone()),
    };
    let caller_parameter = parameter_row(caller_place);
    let callee_parameter = parameter_row(callee_place);
    let targeted = target_operations::TargetOperationPlan {
        psi: plan.psi.clone(),
        target: native,
        entry: caller_machine,
        functions: vec![
            target_operations::TargetFunction {
                machine: caller_machine,
                attachment: None,
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: target_operations::TerminalPsiProvenance {
                    operations: vec![OperationId::new(12).unwrap()],
                    edges: vec![EdgeId::new(1).unwrap()],
                },
                graph: target_operations::TargetControlGraph {
                    structural_types: plan.structural_types.clone(),
                    call_plan: call_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![caller_parameter.clone()],
                    dynamic_parameters: Vec::new(),
                    entry: BlockId::new(1).unwrap(),
                    blocks: vec![target_operations::TargetControlBlock {
                        block: BlockId::new(1).unwrap(),
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        operations: vec![target_operations::TargetUnitOperation::Call {
                            origin: target_operations::NativeCallOrigin::Authored,
                            psi_operation: OperationId::new(12).unwrap(),
                            callee: callee_machine,
                            call_plan: call_plan.clone(),
                            result: target_operations::TargetCallResult::Structural {
                                result: operation_result(result_place),
                                callee_result: declaration(
                                    PlaceId::new(98).unwrap(),
                                    ingress(callee_place),
                                ),
                                result_home: Some(caller_home()),
                                reference_results: vec![target_operations::TargetReferenceResult {
                                    path: leaf_path(),
                                    root: caller_place,
                                }],
                                returned_claim_transfers: Vec::new(),
                            },
                            scalar_arguments: Vec::new(),
                            arguments: vec![target_operations::TargetStructuralArgument {
                                place: caller_place,
                                access: StructuralAccess::Owned,
                                path: Vec::new(),
                                root_structural_type: carrier,
                                structural_type: carrier,
                                shape,
                                source_byte_offset: 0,
                                fixed_array_length: None,
                                element_stride: None,
                                source:
                                    target_operations::TargetStructuralArgumentSource::Placement(
                                        parameter_placement.clone(),
                                    ),
                                destination: parameter_placement.clone(),
                            }],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: crash_routes.clone(),
                        }],
                        terminator: target_operations::TargetControlTerminator::ReturnStructural {
                            psi_edge: EdgeId::new(1).unwrap(),
                            source: target_operations::TargetStructuralReturnSource::Home(
                                caller_home(),
                            ),
                            cleanup_actions: Vec::new(),
                        },
                    }],
                },
            },
            target_operations::TargetFunction {
                machine: callee_machine,
                attachment: None,
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                provenance: target_operations::TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![EdgeId::new(2).unwrap()],
                },
                graph: target_operations::TargetControlGraph {
                    structural_types: plan.structural_types.clone(),
                    call_plan: call_plan.clone(),
                    scalar_parameters: Vec::new(),
                    parameters: vec![callee_parameter.clone()],
                    dynamic_parameters: Vec::new(),
                    entry: BlockId::new(2).unwrap(),
                    blocks: vec![target_operations::TargetControlBlock {
                        block: BlockId::new(2).unwrap(),
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: target_operations::TargetControlTerminator::ReturnStructural {
                            psi_edge: EdgeId::new(2).unwrap(),
                            source: target_operations::TargetStructuralReturnSource::Parameter(
                                callee_parameter,
                            ),
                            cleanup_actions: Vec::new(),
                        },
                    }],
                },
            },
        ],
        native_callback_arguments: Vec::new(),
    };
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    for function in &mut unit.functions {
        function.verified_contract = Some(terminal_psi::MachineContract {
            id: semantic_vocabulary::ContractId::new(1).unwrap(),
            crash_routes: crash_routes.clone(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        });
    }
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    (plan, targeted, unit)
}
