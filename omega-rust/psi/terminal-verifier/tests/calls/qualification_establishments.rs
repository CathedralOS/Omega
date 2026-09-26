//! Route-authorized establishment bindings on a boundary call's structural
//! result replay against the boundary declaration's retained requirement
//! identity, and reject every forged or misaligned shape.

use super::{
    AdmissionProfile, OperationKind, OperationResult, ProofBundle, StructuralPlaceKind,
    StructuralTypeDeclaration, TerminalModule, boundary_call_module, operation_id, place_id,
    structural_type_id, validate_module, verify_module,
};
use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, ResultQualificationEstablishment,
    StructuralDomainDeclaration, StructuralEstablishmentRoute, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralTypeShape,
};

fn routed_module() -> TerminalModule {
    let mut module = boundary_call_module();
    let domain = StructuralDomainId::new(90).unwrap();
    let structural_type = structural_type_id(90);
    let place = place_id(90);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Guard".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.structural_domains.push(StructuralDomainDeclaration {
        establishment_routes: vec![StructuralEstablishmentRoute::BoundaryRequirement {
            requirement: "test::observe".to_owned(),
        }],
        id: domain,
        semantic_domain: DomainSemanticId::new(90).unwrap(),
        identity: "Guard::Active".to_owned(),
        carrier: structural_type,
        content_projection: None,
    });
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(2),
                structural_type,
            },
        });
    module.boundary_machines[0].result =
        BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![domain],
        });
    let call = &mut module.machines[0].blocks[0].operations[1];
    assert!(
        matches!(call.kind, OperationKind::BoundaryCall { .. }),
        "fixture call is a boundary call"
    );
    call.result = OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: vec![ResultQualificationEstablishment { domain, route: 0 }],
        place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![domain],
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    });
    module
}

#[test]
fn routed_boundary_call_result_replays_its_authorized_route() {
    let module = routed_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the boundary call's establishment binding replays its authorized route");
}

#[test]
fn establishment_bindings_reject_forged_or_misaligned_routes() {
    for mutation in 0..6 {
        let mut module = routed_module();
        match mutation {
            // A binding on a membership the result does not carry.
            0 => {
                let OperationResult::Structural(result) =
                    &mut module.machines[0].blocks[0].operations[1].result
                else {
                    panic!("structural result");
                };
                result.qualification_establishments[0].domain =
                    StructuralDomainId::new(91).unwrap();
            }
            // A route index past the declaration's catalog.
            1 => {
                let OperationResult::Structural(result) =
                    &mut module.machines[0].blocks[0].operations[1].result
                else {
                    panic!("structural result");
                };
                result.qualification_establishments[0].route = 1;
            }
            // A second route keeps the catalog canonical, but the callee's
            // declared identity is not it.
            2 => {
                module.structural_domains[0].establishment_routes.push(
                    StructuralEstablishmentRoute::BoundaryRequirement {
                        requirement: "zzz::unauthorized".to_owned(),
                    },
                );
                let OperationResult::Structural(result) =
                    &mut module.machines[0].blocks[0].operations[1].result
                else {
                    panic!("structural result");
                };
                result.qualification_establishments[0].route = 1;
            }
            // The same identity under the wrong route kind cannot replay:
            // the operation is a boundary call, not a conformance-selected
            // callee or dynamic dispatch.
            3 => {
                module.structural_domains[0].establishment_routes =
                    vec![StructuralEstablishmentRoute::Requirement {
                        requirement: "test::observe".to_owned(),
                    }];
            }
            // A duplicated route makes the catalog non-canonical.
            4 => {
                let duplicate = module.structural_domains[0].establishment_routes[0].clone();
                module.structural_domains[0]
                    .establishment_routes
                    .push(duplicate);
            }
            // A duplicated binding is not strictly ordered.
            _ => {
                let OperationResult::Structural(result) =
                    &mut module.machines[0].blocks[0].operations[1].result
                else {
                    panic!("structural result");
                };
                result
                    .qualification_establishments
                    .push(ResultQualificationEstablishment {
                        domain: StructuralDomainId::new(90).unwrap(),
                        route: 0,
                    });
            }
        }
        assert!(
            validate_module(&module).is_err(),
            "establishment mutation {mutation}"
        );
    }
}
