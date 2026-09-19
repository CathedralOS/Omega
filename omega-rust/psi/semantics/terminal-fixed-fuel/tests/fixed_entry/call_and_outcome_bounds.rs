use super::{call_fixture, fixture, unit_effect_fixture, write_only_primitive_store_fixture};
use crate::{
    block_id, claim_id, edge_id, machine_id, operation_id, place_id, structural_type_id, value_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerValue, ScalarType};
use terminal_codec::{CodecError, terminal_psi_identity};
use terminal_fixed_fuel::{
    FixedFuelError, derive_fixed_entry_fuel, derive_fixed_safe_point_segments,
    derive_fixed_segment_fuel, derive_validated_fixed_safe_point_segments,
    retain_validated_fixed_safe_point_segments, validate_fixed_entry_fuel,
    validate_fixed_safe_point_segments, validate_fixed_segment_fuel,
    validate_retained_fixed_safe_point_segments,
};
use terminal_psi::{
    Block, ClaimTransfer, CompletionReceipt, CrashCause, CrashRouteBucket, CrashRouteGuard,
    EntryClaim, Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    Terminator, ValueDeclaration,
};
use terminal_verifier::{ProofBundle, verify_module};

#[test]
fn semantic_mutation_invalidates_the_old_certificate_without_changing_cost() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1)).unwrap();

    let mut changed = module.clone();
    changed.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(8),
    };
    let changed_verified = verify_module(&changed, &proof, &AdmissionProfile::default()).unwrap();
    let changed_certificate = derive_fixed_entry_fuel(&changed_verified, machine_id(1)).unwrap();
    assert_ne!(
        changed_certificate.terminal_psi(),
        certificate.terminal_psi()
    );
    assert_eq!(
        changed_certificate.ceiling_units(),
        certificate.ceiling_units()
    );
    assert_eq!(
        validate_fixed_entry_fuel(&changed_verified, &certificate),
        Err(FixedFuelError::CertificateMismatch)
    );
}

#[test]
fn certificate_derivation_requires_canonical_semantic_identity() {
    let (mut module, proof) = fixture();
    module.machines[0].blocks.swap(0, 1);
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1)),
        Err(FixedFuelError::SemanticIdentity(
            CodecError::NonCanonicalOrder("blocks by BlockId")
        ))
    );
    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(999), block_id(1), edge_id(1)),
        Err(FixedFuelError::SemanticIdentity(
            CodecError::NonCanonicalOrder("blocks by BlockId")
        )),
        "single-segment identity validation still precedes machine lookup"
    );
    assert_eq!(
        derive_fixed_safe_point_segments(&verified, machine_id(999)),
        Err(FixedFuelError::UnknownEntry(machine_id(999))),
        "catalog selection still checks its machine before requesting identity"
    );
}

#[test]
fn selected_segments_include_their_exact_terminal_edge() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    let entry_to_jump =
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(1), edge_id(1)).unwrap();
    assert_eq!(
        entry_to_jump.terminal_psi(),
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(entry_to_jump.schedule().marker(), 1);
    assert_eq!(entry_to_jump.machine(), machine_id(1));
    assert_eq!(entry_to_jump.start_block(), block_id(1));
    assert_eq!(entry_to_jump.end_edge(), edge_id(1));
    assert!(entry_to_jump.relevant_preconditions().is_empty());
    assert_eq!(entry_to_jump.ceiling_units(), 2);
    validate_fixed_segment_fuel(&verified, &entry_to_jump).unwrap();

    let jump_target_to_return =
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(2), edge_id(2)).unwrap();
    assert_eq!(jump_target_to_return.ceiling_units(), 1);
    validate_fixed_segment_fuel(&verified, &jump_target_to_return).unwrap();
}

#[test]
fn a_segment_cannot_cross_the_reached_return_to_find_an_unrelated_edge() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(2), edge_id(1)),
        Err(FixedFuelError::SegmentEndNotReached {
            requested: edge_id(1),
            reached_terminal: edge_id(2),
        })
    );
}

#[test]
fn crash_is_an_explicit_fixed_fuel_terminal_edge() {
    let (mut module, _) = fixture();
    module.machines[0].contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
    }];
    module.machines[0].contract.ensures.clear();
    module.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("crash-ending module verifies");

    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1))
        .expect("crash is a terminal path for fixed fuel");
    assert_eq!(certificate.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &certificate).expect("crash certificate recomputes");

    let segments = derive_fixed_safe_point_segments(&verified, machine_id(1))
        .expect("crash edge closes the final segment");
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[1].end_edge(), edge_id(2));
    assert_eq!(segments[1].ceiling_units(), 1);
}

#[test]
fn calls_include_the_complete_callee_bound() {
    let module = call_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("call module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(1))
        .expect("acyclic call graph has a fixed bound");
    assert_eq!(certificate.ceiling_units(), 4);
    validate_fixed_entry_fuel(&verified, &certificate).expect("call bound recomputes");
}

#[test]
fn unit_calls_and_effect_operations_use_the_same_transitive_schedule() {
    let module = unit_effect_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("Unit/effect module verifies");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(700))
        .expect("acyclic Unit call has an exact fixed bound");

    assert_eq!(certificate.ceiling_units(), 5);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    let segments = derive_fixed_safe_point_segments(&verified, machine_id(700)).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].ceiling_units(), 5);
}

#[test]
fn write_only_primitive_store_has_exact_local_and_transitive_call_bounds() {
    let module = write_only_primitive_store_fixture();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("write-only primitive-store module verifies");

    let callee = derive_fixed_entry_fuel(&verified, machine_id(701))
        .expect("constant, store, and return have a fixed bound");
    assert_eq!(callee.ceiling_units(), 3);
    validate_fixed_entry_fuel(&verified, &callee).unwrap();

    let caller = derive_fixed_entry_fuel(&verified, machine_id(700))
        .expect("the Unit call composes the store callee bound");
    assert_eq!(caller.ceiling_units(), 5);
    validate_fixed_entry_fuel(&verified, &caller).unwrap();
    let segments = derive_fixed_safe_point_segments(&verified, machine_id(700)).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].ceiling_units(), 5);
}

#[test]
fn projected_unit_calls_compose_each_callee_bound_in_call_order() {
    let mut module = unit_effect_fixture();
    module.root_service_reach = Default::default();
    let element = structural_type_id(950);
    let array = structural_type_id(951);
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: element,
            identity: "test::Receipt".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "[test::Receipt;2]".into(),
            shape: StructuralTypeShape::FixedArray { element, length: 2 },
        },
    ];
    module.boundary_machines[0].structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: place_id(952),
        position: 0,
        is_self: false,
        structural_type: element,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];

    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: place_id(950),
        position: 0,
        is_self: false,
        structural_type: array,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(950),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    caller.entry_claims = vec![
        EntryClaim {
            claim: claim_id(1),
            input: place_id(950),
            path: vec![StructuralPathSegment::FixedIndex(0)],
        },
        EntryClaim {
            claim: claim_id(2),
            input: place_id(950),
            path: vec![StructuralPathSegment::FixedIndex(1)],
        },
    ];
    caller.blocks[0].operations = (0..2)
        .map(|index| Operation {
            static_reach_binding: None,
            id: operation_id(950 + index),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                arguments: Vec::new(),
                callee: machine_id(701),
                structural_arguments: vec![StructuralArgument {
                    access: StructuralAccess::Owned,
                    place: place_id(950),
                    path: vec![StructuralPathSegment::FixedIndex(index)],
                }],
                claim_transfers: vec![ClaimTransfer {
                    claim: claim_id(1 + index),
                    argument_index: 0,
                }],
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        })
        .collect();

    let callee = &mut module.machines[1];
    callee.structural_parameters = vec![StructuralParameterDeclaration {
        access: StructuralAccess::Owned,
        place: place_id(951),
        position: 0,
        is_self: false,
        structural_type: element,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    callee.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(951),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    callee.entry_claims = vec![EntryClaim {
        claim: claim_id(1),
        input: place_id(951),
        path: Vec::new(),
    }];
    let OperationKind::BoundaryCall {
        structural_arguments,
        completion_receipts,
        ..
    } = &mut callee.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *structural_arguments = vec![StructuralArgument {
        access: StructuralAccess::Owned,
        place: place_id(951),
        path: Vec::new(),
    }];
    *completion_receipts = vec![CompletionReceipt {
        claim: claim_id(1),
        argument_index: 0,
    }];

    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("projected sibling calls verify");
    let certificate = derive_fixed_entry_fuel(&verified, machine_id(700)).unwrap();
    assert_eq!(certificate.ceiling_units(), 7);
    validate_fixed_entry_fuel(&verified, &certificate).unwrap();
}

#[test]
fn an_all_crash_callee_excludes_the_unreachable_caller_tail() {
    let mut module = call_fixture();
    let route = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    module.machines[0].contract.crash_routes = vec![route.clone()];
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![route.clone()];
    module.machines[1].contract.crash_routes = vec![route];
    module.machines[1].blocks[0].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("all-crash call module verifies");

    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1))
            .expect("all-crash call has an exact bound")
            .ceiling_units(),
        3,
        "the caller return edge is unreachable after the callee crash"
    );
    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(1), edge_id(1)),
        Err(FixedFuelError::SegmentEndUnreachableAfterCall {
            block: block_id(1),
            callee: machine_id(2),
        })
    );
    assert!(
        derive_fixed_safe_point_segments(&verified, machine_id(1))
            .expect("the caller has no reachable machine-local edge")
            .is_empty()
    );
    drop(verified);
    module.machines.swap(0, 1);
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("machine ordering does not change verification");
    assert!(
        derive_fixed_safe_point_segments(&verified, machine_id(1))
            .expect("an empty unsealed catalog does not request semantic identity")
            .is_empty()
    );
    validate_fixed_safe_point_segments(&verified, machine_id(1), &[])
        .expect("empty roster replay preserves the same identity behavior");
    assert_eq!(
        retain_validated_fixed_safe_point_segments(&verified, machine_id(1), Vec::new()),
        Err(FixedFuelError::SemanticIdentity(
            CodecError::NonCanonicalOrder("machines by MachineId")
        )),
        "sealing still binds identity even for an empty catalog"
    );
}

#[test]
fn mixed_call_outcomes_do_not_cross_product_crash_and_caller_return_costs() {
    let mut module = call_fixture();
    let route = CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    module.machines[0].contract.crash_routes = vec![route.clone()];
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *crash_continuations = vec![route.clone()];

    let callee = &mut module.machines[1];
    callee.contract.crash_routes = vec![route];
    callee.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(4),
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(2),
                    target: block_id(3),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: edge_id(4),
                value: value_id(4),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(4),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: operation_id(3),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(6),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: false },
                },
                Operation {
                    static_reach_binding: None,
                    id: operation_id(4),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(7),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: false },
                },
            ],
            terminator: Terminator::Crash {
                edge: edge_id(5),
                cause: CrashCause::Abort,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        },
    ];
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("mixed call-outcome module verifies");

    assert_eq!(
        derive_fixed_entry_fuel(&verified, machine_id(1))
            .expect("mixed call outcomes have an exact bound")
            .ceiling_units(),
        6,
        "the four-unit crash path must not be followed by the caller return"
    );
    assert_eq!(
        derive_fixed_segment_fuel(&verified, machine_id(1), block_id(1), edge_id(1))
            .expect("the caller return edge remains reachable")
            .ceiling_units(),
        5,
        "the caller segment composes with the callee's two-unit return path"
    );
}

#[test]
fn safe_point_selection_covers_the_complete_ordered_path() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();

    let segments = derive_fixed_safe_point_segments(&verified, machine_id(1)).unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].start_block(), block_id(1));
    assert_eq!(segments[0].end_edge(), edge_id(1));
    assert_eq!(segments[0].ceiling_units(), 2);
    assert_eq!(segments[1].start_block(), block_id(2));
    assert_eq!(segments[1].end_edge(), edge_id(2));
    assert_eq!(segments[1].ceiling_units(), 1);
    validate_fixed_safe_point_segments(&verified, machine_id(1), &segments).unwrap();

    assert_eq!(
        validate_fixed_safe_point_segments(&verified, machine_id(1), &segments[..1]),
        Err(FixedFuelError::CertificateMismatch),
        "a producer cannot omit the final terminal segment"
    );
    let mut reordered = segments;
    reordered.reverse();
    assert_eq!(
        validate_fixed_safe_point_segments(&verified, machine_id(1), &reordered),
        Err(FixedFuelError::CertificateMismatch),
        "a producer cannot reorder semantic safe-point segments"
    );
}

#[test]
fn retained_safe_point_catalog_is_complete_ordered_and_semantically_exact() {
    let (module, proof) = fixture();
    let verified = verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let segments = derive_fixed_safe_point_segments(&verified, machine_id(1)).unwrap();
    let retained =
        retain_validated_fixed_safe_point_segments(&verified, machine_id(1), segments.clone())
            .expect("the complete canonical partition is retainable");

    assert_eq!(
        retained.terminal_psi(),
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(retained.schedule().marker(), 1);
    assert_eq!(retained.machine(), machine_id(1));
    assert_eq!(retained.certificates(), segments);
    validate_retained_fixed_safe_point_segments(&verified, &retained)
        .expect("the retained partition independently replays");

    assert_eq!(
        retain_validated_fixed_safe_point_segments(
            &verified,
            machine_id(1),
            segments[..1].to_vec(),
        ),
        Err(FixedFuelError::CertificateMismatch),
        "an omitted reachable segment cannot be sealed"
    );
    let mut reordered = segments.clone();
    reordered.reverse();
    assert_eq!(
        retain_validated_fixed_safe_point_segments(&verified, machine_id(1), reordered),
        Err(FixedFuelError::CertificateMismatch),
        "a reordered partition cannot be sealed"
    );
    let mut duplicated = segments.clone();
    duplicated.push(segments[0].clone());
    assert_eq!(
        retain_validated_fixed_safe_point_segments(&verified, machine_id(1), duplicated),
        Err(FixedFuelError::CertificateMismatch),
        "a duplicated segment cannot be sealed"
    );

    let (mut drifted_module, drifted_proof) = fixture();
    let OperationKind::IntegerConstant { value } =
        &mut drifted_module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("fixture begins with an integer constant")
    };
    *value = IntegerValue::Signed(8);
    let drifted_verified = verify_module(
        &drifted_module,
        &drifted_proof,
        &AdmissionProfile::default(),
    )
    .expect("the semantically drifted module remains structurally valid");
    assert_eq!(
        validate_retained_fixed_safe_point_segments(&drifted_verified, &retained),
        Err(FixedFuelError::CertificateMismatch),
        "a different terminal semantic identity cannot replay the catalog"
    );

    let derived = derive_validated_fixed_safe_point_segments(&verified, machine_id(1))
        .expect("direct catalog derivation succeeds");
    assert_eq!(derived.certificates(), segments);
}
