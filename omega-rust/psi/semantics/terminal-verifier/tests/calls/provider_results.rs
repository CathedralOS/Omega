use super::*;
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, ProviderParameterRefinement,
    ProviderSignatureParameter, StructuralAccess, StructuralParameterDeclaration,
};

fn provider_module() -> TerminalModule {
    let mut module = provider_candidate_module();
    module.machines[0].blocks[0].operations.clear();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    module.boundary_machines[0].scalar_parameters[0] = scalar_type;
    module.machines[1].parameters[0].scalar_type = scalar_type;
    let structural_type = StructuralTypeId::new(2).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::OwnedValue".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut boundary_parameter = parameter.clone();
    boundary_parameter.place = PlaceId::new(3).unwrap();
    module.boundary_machines[0].structural_parameters = vec![boundary_parameter];
    module.boundary_machines[0].result =
        BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        });
    module.provider_candidates[0].signature.parameters = vec![ProviderSignatureParameter {
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    module.provider_candidates[0]
        .refinement
        .positional_parameters = vec![ProviderParameterRefinement {
        boundary_index: 0,
        candidate_index: 0,
    }];
    let candidate = &mut module.machines[1];
    candidate.structural_parameters = vec![parameter];
    candidate.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: PlaceId::new(2).unwrap(),
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    candidate.structural_places = vec![
        StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: PlaceId::new(2).unwrap(),
            kind: StructuralPlaceKind::Result,
        },
    ];
    candidate.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(2),
        source: PlaceId::new(1).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

#[test]
fn provider_result_conformance_joins_structural_results_and_scalar_parameters() {
    let module = provider_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let mut missing_scalar = module.clone();
    missing_scalar.machines[1].parameters.clear();
    assert!(matches!(
        validate_module(&missing_scalar),
        Err(ModuleError::InvalidProviderCandidate { .. })
    ));
}

#[test]
fn provider_result_conformance_rejects_signature_and_contract_drift() {
    let baseline = provider_module();
    validate_module(&baseline).unwrap();
    for mutation in 0..7 {
        let mut module = baseline.clone();
        match mutation {
            0 => module.boundary_machines[0].result = BoundaryMachineResult::Unit,
            1 => module.machines[1].result = TerminalMachineResult::Unit,
            2 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.structural_type = StructuralTypeId::new(1).unwrap();
            }
            3 | 4 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.multiplicity = if mutation == 3 {
                    StructuralMultiplicity::Linear
                } else {
                    StructuralMultiplicity::Unrestricted
                };
                let BoundaryMachineResult::Structural(required) =
                    &mut module.boundary_machines[0].result
                else {
                    unreachable!()
                };
                required.multiplicity = result.multiplicity;
            }
            5 => {
                module.machines[1].contract.requires =
                    call_module().machines[1].contract.requires.clone()
            }
            6 => {
                module.machines[1].contract.ensures =
                    call_module().machines[1].contract.ensures.clone()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::InvalidProviderCandidate {
                boundary: boundary_id(1),
                candidate: machine_id(2)
            },
            "mutation {mutation}"
        );
    }
}
