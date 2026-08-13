use omega_terminal_abstract_operations::{
    TerminalAbstractFunctionResult, TerminalAbstractOperation,
};
use omega_terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, lower_artifact_sections,
};
use psi_core::{
    BlockId, ClaimId, ContractId, EdgeId, MachineId, PlaceId, ScalarType, StructuralDomainId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, CrashCause, CrashRouteBucket, CrashRouteGuard, EntryClaim, MachineContract,
    StructuralDomainDeclaration, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::ProofBundle;

#[test]
fn omega_consumes_verified_jump_affine_cleanup_without_emitting_an_operation() {
    let place = place_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "test::AffineToken".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: vec![ValueDeclaration {
                id: value_id(1),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: vec![StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            }],
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: vec![StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![
                Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: vec![value_id(1)],
                        trivial_affine_discards: vec![place],
                    },
                },
                Block {
                    id: block_id(2),
                    parameters: vec![ValueDeclaration {
                        id: value_id(3),
                        scalar_type: ScalarType::Boolean,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: edge_id(2),
                        value: value_id(3),
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let semantics = encode_module(&module).expect("exact jump affine cleanup should encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof should encode");

    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified jump affine cleanup should lower through Omega");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    let [
        TerminalAbstractOperation::Jump {
            psi_edge: jump_edge,
            target,
            bindings,
        },
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            ..
        },
    ] = function.operations.as_slice()
    else {
        panic!("no-code cleanup must not add an abstract operation")
    };
    assert_eq!(*jump_edge, edge_id(1));
    assert_eq!(*target, block_id(2));
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].parameter, value_id(3));
    assert_eq!(bindings[0].argument, value_id(1));
    assert_eq!(*psi_edge, edge_id(2));
    assert_eq!(*result, value_id(2));
    assert_eq!(*value, value_id(3));
    assert_eq!(*scalar_type, ScalarType::Boolean);
}

#[test]
fn omega_preserves_exact_singleton_structural_return_custody() {
    let source = place_id(1);
    let result_place = place_id(2);
    let claim = claim_id(1);
    let structural_type = structural_type_id(1);
    let structural_domain = structural_domain_id(1);
    let edge = edge_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::LinearToken".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: structural_domain,
            identity: "test::Owned".into(),
            carrier: structural_type,
        }],
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![structural_domain],
            }],
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![structural_domain],
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
        TerminalAbstractFunctionResult::Structural(StructuralResultDeclaration {
            place: result_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![structural_domain],
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
        [TerminalAbstractOperation::ReturnStructural {
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
    assert!(matches!(
        lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default()),
        Err(ArtifactLoweringError::Lowering(
            LoweringError::UnsupportedStructuralResult(machine)
        )) if machine == machine_id(1)
    ));

    let extra = place_id(3);
    let mut wider_cleanup = module;
    wider_cleanup.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: extra,
            position: 1,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
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
        [TerminalAbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        }] if trivial_affine_discards == &[extra]
    ));

    let second_extra = place_id(4);
    wider_cleanup.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: second_extra,
            position: 2,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
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
        [TerminalAbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        }] if trivial_affine_discards == &[second_extra, extra]
    ));
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).unwrap()
}

fn claim_id(raw: u64) -> ClaimId {
    ClaimId::new(raw).unwrap()
}
