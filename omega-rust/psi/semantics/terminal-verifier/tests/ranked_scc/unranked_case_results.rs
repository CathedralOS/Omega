use super::*;
use semantic_vocabulary::{BoundaryMachineId, StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryStructuralResultDeclaration, StructuralCaseDeclaration, StructuralCaseSuccessorEdge,
    StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
};

fn fixture() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let structural_type = id(1, StructuralTypeId::new);
    let place = id(1, PlaceId::new);
    let boundary = id(1, BoundaryMachineId::new);
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "ReadResult".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: id(1, StructuralCaseId::new),
                    identity: "Value".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: id(1, StructuralFieldId::new),
                        identity: "value".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar_type),
                    }],
                },
                StructuralCaseDeclaration {
                    id: id(2, StructuralCaseId::new),
                    identity: "Done".into(),
                    fields: Vec::new(),
                },
            ],
        },
    });
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: "Input::read".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        }),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let machine = &mut module.machines[0];
    machine.parameters.clear();
    machine.entry = id(1, BlockId::new);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id(1, OperationId::new),
            structural_type,
        },
    });
    machine.blocks = vec![
        Block {
            id: id(1, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation {
                id: id(1, OperationId::new),
                result: OperationResult::Structural(StructuralOperationResult {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::BoundaryCall {
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                },
            }],
            terminator: Terminator::StructuralCase {
                source: place,
                cases: vec![
                    StructuralCaseSuccessorEdge {
                        edge: id(1, EdgeId::new),
                        target: id(2, BlockId::new),
                        case: id(1, StructuralCaseId::new),
                        payload_fields: vec![id(1, StructuralFieldId::new)],
                        trivial_affine_discards: vec![place],
                    },
                    StructuralCaseSuccessorEdge {
                        edge: id(2, EdgeId::new),
                        target: id(3, BlockId::new),
                        case: id(2, StructuralCaseId::new),
                        payload_fields: Vec::new(),
                        trivial_affine_discards: vec![place],
                    },
                ],
            },
        },
        Block {
            id: id(2, BlockId::new),
            parameters: vec![ValueDeclaration {
                id: id(1, ValueId::new),
                scalar_type,
            }],
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: id(3, EdgeId::new),
                target: id(1, BlockId::new),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        },
        Block {
            id: id(3, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(4, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    module
}

#[test]
fn unranked_case_result_is_fresh_and_disposed_before_reentry() {
    let module = fixture();
    verify_module_for_interpretation(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("fresh affine result is inspected and disposed on every selected edge");
    let frontiers = terminal_verifier::reconstruct_structural_ownership_frontiers(&module).unwrap();
    let machine = frontiers.machine(module.entry).unwrap();
    assert_eq!(
        machine.block_entry(id(1, BlockId::new)),
        machine.edge_exit(id(3, EdgeId::new))
    );
}

#[test]
fn unranked_case_result_rejects_missing_duplicate_or_substituted_disposal() {
    for mutation in 0..3 {
        let mut module = fixture();
        let Terminator::StructuralCase { cases, .. } = &mut module.machines[0].blocks[0].terminator
        else {
            panic!("case fixture");
        };
        match mutation {
            0 => cases[0].trivial_affine_discards.clear(),
            1 => cases[0].trivial_affine_discards.push(id(1, PlaceId::new)),
            _ => cases[0].trivial_affine_discards[0] = id(99, PlaceId::new),
        }
        assert!(
            validate_module_for_interpretation(&module).is_err(),
            "disposal mutation {mutation}"
        );
    }
}

#[test]
fn unranked_case_result_cannot_reuse_a_preheader_value() {
    let mut module = fixture();
    let producer = module.machines[0].blocks[0].operations.remove(0);
    module.machines[0].entry = id(4, BlockId::new);
    module.machines[0].blocks.push(Block {
        id: id(4, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![producer],
        terminator: Terminator::Jump {
            edge: id(5, EdgeId::new),
            target: id(1, BlockId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    });
    assert!(validate_module_for_interpretation(&module).is_err());
}

#[test]
fn selected_case_payload_facts_do_not_cross_iteration_cuts() {
    let mut module = fixture();
    module.machines[0]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: id(1, ObligationId::new),
            proposition: Proposition::Truth,
        });
    let reconstructed = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let previous_payload = Proposition::Equal(
        ScalarTerm::value(id(1, ValueId::new), ScalarType::Integer(integer)),
        ScalarTerm::integer_field_path(
            id(1, PlaceId::new),
            vec![
                semantic_vocabulary::CanonicalStructuralPathSegment::Case(id(
                    1,
                    StructuralCaseId::new,
                )),
                semantic_vocabulary::CanonicalStructuralPathSegment::Field(id(
                    1,
                    StructuralFieldId::new,
                )),
            ],
            integer,
        ),
    );
    assert!(
        !reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&previous_payload),
        "a previous Value iteration cannot describe the fresh Done result at exit"
    );
}
