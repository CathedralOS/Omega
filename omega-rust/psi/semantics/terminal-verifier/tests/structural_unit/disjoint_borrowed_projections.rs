//! Each ordinary borrowed argument retains independent place and contract custody.
use super::*;

fn module() -> TerminalModule {
    let mut module = projected_unit_call_module();
    module.structural_domains.clear();
    module.boundary_machines.clear();
    module.services.clear();
    module.root_service_reach = Default::default();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "Counter".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "Owner".into(),
            shape: StructuralTypeShape::Record {
                fields: ["source", "destination"]
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, identity)| StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(ordinal as u64 + 1)
                            .unwrap(),
                        identity: identity.into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(structural_type_id(1)),
                    })
                    .collect(),
            },
        },
    ];
    module.machines[1].blocks[0].operations.clear();
    for machine in &mut module.machines {
        machine.entry_claims.clear();
        machine.published_service_ceiling.clear();
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
        machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    }
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(2);
    let mut destination = module.machines[1].structural_parameters[0].clone();
    destination.place = place_id(3);
    destination.position = 1;
    module.machines[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    module.machines[1].structural_parameters.push(destination);
    module.machines[1]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(3),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers.clear();
    *structural_arguments = [
        ("source", StructuralAccess::SharedBorrow),
        ("destination", StructuralAccess::MutableBorrow),
    ]
    .into_iter()
    .map(|(field, access)| StructuralArgument {
        place: place_id(1),
        path: vec![StructuralPathSegment::Field(field.into())],
        access,
    })
    .collect();
    module
}

#[test]
fn disjoint_borrowed_projection_arguments_verify_independently() {
    verify_module(
        &module(),
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn disjoint_borrowed_projections_reject_access_overlap_and_path_substitution() {
    for mutation in ["overlap", "access", "root", "path", "callee type"] {
        let mut changed = module();
        if mutation == "callee type" {
            changed.machines[1].structural_parameters[1].structural_type = structural_type_id(2);
        } else if mutation == "root" {
            changed.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
        } else {
            let OperationKind::CallUnit {
                structural_arguments,
                ..
            } = &mut changed.machines[0].blocks[0].operations[0].kind
            else {
                unreachable!()
            };
            match mutation {
                "overlap" => structural_arguments[1].path = structural_arguments[0].path.clone(),
                "access" => structural_arguments[0].access = StructuralAccess::MutableBorrow,
                "path" => {
                    structural_arguments[1].path =
                        vec![StructuralPathSegment::Field("unknown".into())]
                }
                _ => unreachable!(),
            }
        }
        assert!(
            verify_module(
                &changed,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn second_projected_parameter_cannot_import_a_root_content_contract() {
    let mut changed = module();
    changed.machines[1].contract.ensures.push(ContractClause {
        obligation: obligation_id(1),
        proposition: content_predicate(place_id(3)),
    });
    assert_eq!(
        validate_module(&changed).unwrap_err(),
        ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
            operation: operation_id(1),
            callee: machine_id(2),
            place: place_id(3),
        }
    );
}
