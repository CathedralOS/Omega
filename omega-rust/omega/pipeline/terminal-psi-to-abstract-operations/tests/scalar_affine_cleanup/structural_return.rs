//! Exact singleton structural-return custody and finite affine cleanup tails.

use abstract_operations::{AbstractFunctionResult, AbstractOperation};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::{
    Block, CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract,
    StructuralAccess, StructuralDomainDeclaration, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, VocabularyMarker,
};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use terminal_verifier::ProofBundle;

use super::support::{
    block_id, claim_id, contract_id, edge_id, machine_id, place_id, structural_domain_id,
    structural_type_id,
};

#[test]
fn omega_preserves_exact_singleton_structural_return_custody() {
    let source = place_id(1);
    let result_place = place_id(2);
    let claim = claim_id(1);
    let structural_type = structural_type_id(1);
    let structural_domain = structural_domain_id(1);
    let edge = edge_id(1);
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_range_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::LinearToken".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: structural_domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain identity"),
            identity: "test::Owned".into(),
            carrier: structural_type,
            content_projection: None,
        }],
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            declared_service_reach: Vec::new(),
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![structural_domain],
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![structural_domain],
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: source,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            entry_claims: vec![EntryClaim {
                claim,
                input: source,
                path: Vec::new(),
            }],
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnStructural {
                    edge,
                    source,
                    returned_claims: vec![claim],
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantics = encode_module(&module).expect("structural return should encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof should encode");

    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("exact structural custody return should enter Omega");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    assert_eq!(
        function.structural_parameters,
        module.machines[0].structural_parameters
    );
    assert_eq!(function.entry_claims, module.machines[0].entry_claims);
    assert_eq!(
        function.result,
        AbstractFunctionResult::Structural(StructuralResultDeclaration {
            place: result_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![structural_domain],
            projected_qualifications: Vec::new(),
        })
    );
    assert_eq!(
        function
            .result
            .structural()
            .expect("structural result")
            .place,
        result_place
    );
    assert!(matches!(
        function.operations.as_slice(),
        [AbstractOperation::ReturnStructural {
            psi_edge,
            source: actual_source,
            returned_claims,
            trivial_affine_discards,
            ..
        }] if *psi_edge == edge
            && *actual_source == source
            && returned_claims.as_slice() == [claim]
            && trivial_affine_discards.is_empty()
    ));

    let mut crash_only = module.clone();
    crash_only.machines[0].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    crash_only.machines[0].blocks[0].terminator = Terminator::Crash {
        edge,
        cause: CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: vec![claim],
    };
    let semantics = encode_module(&crash_only).expect("structural crash-only machine encodes");
    let lowered = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("a structural result does not change the crash operation's semantics");
    assert!(matches!(lowered.functions[0].operations.as_slice(),
        [AbstractOperation::Crash { frontier_lower_bound, .. }] if frontier_lower_bound == &[claim]));

    // Preserve unrelated operations and both control edges, not just the
    // identity-return template. Claim replay still rejects a missing transfer.
    let mut branching = module.clone();
    let original_return = branching.machines[0].blocks[0].terminator.clone();
    let condition = semantic_vocabulary::ValueId::new(1).unwrap();
    branching.machines[0].blocks[0]
        .operations
        .push(terminal_psi::Operation {
            id: semantic_vocabulary::OperationId::new(1).unwrap(),
            result: terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
                id: condition,
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            kind: terminal_psi::OperationKind::BooleanConstant { value: true },
        });
    branching.machines[0].blocks[0].terminator = Terminator::Conditional {
        condition,
        when_true: terminal_psi::SuccessorEdge {
            edge: edge_id(2),
            target: block_id(2),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: terminal_psi::SuccessorEdge {
            edge: edge_id(3),
            target: block_id(2),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    branching.machines[0].blocks.push(Block {
        id: block_id(2),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: original_return,
    });
    let semantics = encode_module(&branching).unwrap();
    let lowered = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified operations compose before a structural return");
    assert_eq!(lowered.functions[0].block_entries.len(), 2);
    assert!(matches!(lowered.functions[0].operations.as_slice(),
        [AbstractOperation::BooleanConstant { .. }, AbstractOperation::Conditional { .. },
         AbstractOperation::ReturnStructural { returned_claims, .. }] if returned_claims == &[claim]));
    if let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut branching.machines[0].blocks[1].terminator
    {
        returned_claims.clear();
    }
    assert!(encode_module(&branching).is_err());
    assert!(
        terminal_verifier::verify_module(
            &branching,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );

    // Establishments remain explicit in the abstract graph. The existing
    // no-code physical return gathers their exact provenance at its own stage.
    let mut with_local = module.clone();
    with_local.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![terminal_psi::StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            identity: "value".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::Scalar(
                semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                ),
            ),
        }],
    };
    let local = StructuralPlaceDeclaration {
        id: place_id(3),
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type: structural_type_id(2),
            construction: None,
        },
    };
    with_local.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "test::Empty".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    with_local.machines[0].structural_places.push(local);
    with_local.machines[0].blocks[0]
        .operations
        .push(terminal_psi::Operation {
            id: semantic_vocabulary::OperationId::new(2).unwrap(),
            result: terminal_psi::OperationResult::Unit,
            kind: terminal_psi::OperationKind::EstablishTrivialAffineLocal {
                destination: local.id,
            },
        });
    if let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut with_local.machines[0].blocks[0].terminator
    {
        trivial_affine_discards.push(local.id);
    }
    let lowered = lower_artifact_sections(
        &encode_module(&with_local).unwrap(),
        &proof,
        &AdmissionProfile::default(),
    )
    .unwrap();
    assert!(matches!(lowered.functions[0].operations.as_slice(),
        [AbstractOperation::EstablishTrivialAffineLocal { place, .. },
         AbstractOperation::ReturnStructural { trivial_affine_locals, .. }]
         if place == &local && trivial_affine_locals.is_empty()));
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let physical =
            abstract_operations_to_target_operations::lower_to_target_operations(&lowered, target)
                .expect("explicit no-code local preserves native return support");
        assert_eq!(
            physical.functions[0].provenance.operations,
            [semantic_vocabulary::OperationId::new(2).unwrap()]
        );
        assert_eq!(physical.functions[0].provenance.edges, [edge]);
    }

    let extra = place_id(3);
    let mut wider_cleanup = module;
    wider_cleanup.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: extra,
            position: 1,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    wider_cleanup.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: extra,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut wider_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.push(extra);
    let semantics = encode_module(&wider_cleanup).expect("wider cleanup return should encode");
    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("one exact affine cleanup should enter Omega abstract operations");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    assert_eq!(function.structural_parameters.len(), 2);
    assert!(matches!(
        function.operations.as_slice(),
        [AbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        }] if trivial_affine_discards == &[extra]
    ));

    let second_extra = place_id(4);
    wider_cleanup.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            access: StructuralAccess::Owned,
            place: second_extra,
            position: 2,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    wider_cleanup.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: second_extra,
            kind: StructuralPlaceKind::Parameter {
                position: 2,
                is_self: false,
            },
        });
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut wider_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![second_extra, extra];
    let semantics = encode_module(&wider_cleanup).expect("two affine cleanups should encode");
    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("a finite exact affine cleanup tail should enter Omega abstract operations");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    assert_eq!(function.structural_parameters.len(), 3);
    assert!(matches!(
        function.operations.as_slice(),
        [AbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        }] if trivial_affine_discards == &[second_extra, extra]
    ));
}
