use super::*;
use terminal_psi::StructuralCaseSuccessorEdge;

pub(super) fn dispatch_module(access: StructuralAccess) -> TerminalModule {
    let mut module = unit_module();
    let structural_type = StructuralTypeId::new(1).expect("sum type");
    let source = PlaceId::new(1).expect("source place");
    let cases = [
        StructuralCaseId::new(1).expect("first case"),
        StructuralCaseId::new(2).expect("second case"),
    ];
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Choice".to_owned(),
        shape: StructuralTypeShape::Sum {
            cases: cases
                .into_iter()
                .enumerate()
                .map(|(position, case)| StructuralCaseDeclaration {
                    id: case,
                    identity: format!("Case{position}"),
                    fields: Vec::new(),
                })
                .collect(),
        },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 0,
            is_self: false,
            structural_type,
            access,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    machine.blocks[0].terminator = Terminator::StructuralCase {
        source,
        cases: cases
            .into_iter()
            .enumerate()
            .map(|(position, case)| StructuralCaseSuccessorEdge {
                edge: EdgeId::new(10 + position as u64).expect("dispatch edge"),
                target: BlockId::new(10 + position as u64).expect("case block"),
                case,
                payload_fields: Vec::new(),
                trivial_affine_discards: Vec::new(),
            })
            .collect(),
    };
    for position in 0..2 {
        machine.blocks.push(Block {
            id: BlockId::new(10 + position).expect("case block"),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(20 + position).expect("return edge"),
                trivial_affine_discards: Vec::new(),
            },
        });
    }
    module
}

#[test]
fn readable_parameters_allow_fresh_structural_case_dispatch() {
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let module = dispatch_module(access);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("readable direct sum dispatch independently verifies");
    }
}

#[test]
fn write_only_parameter_cannot_drive_fresh_structural_case_dispatch() {
    let module = dispatch_module(StructuralAccess::WriteOnlyBorrow);
    assert!(module.machines[0].contract.requires.is_empty());
    assert!(module.machines[0].contract.crash_routes.is_empty());
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::StructuralCaseRequiresReadableAccess {
            machine: module.machines[0].id,
            block: module.machines[0].entry,
            source: module.machines[0].structural_parameters[0].place,
        }),
        "write-only tag dispatch must reject without relying on any contract predicate"
    );
}

#[test]
fn owned_operation_result_keeps_structural_case_dispatch() {
    let mut module = dispatch_module(StructuralAccess::Owned);
    let machine = &mut module.machines[0];
    let source = machine.structural_parameters.remove(0);
    let operation = OperationId::new(1).expect("case producer");
    machine.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: operation,
        structural_type: source.structural_type,
    };
    machine.blocks[0].operations.push(Operation {
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: source.place,
            structural_type: source.structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPayloadlessCase {
            result_case: StructuralCaseId::new(1).expect("first case"),
        },
    });
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("owned constructed value retains tag observation");
}

#[test]
fn supplied_case_refinement_does_not_grant_executable_tag_access() {
    let mut module = dispatch_module(StructuralAccess::WriteOnlyBorrow);
    let machine = &mut module.machines[0];
    let source = machine.structural_parameters[0].place;
    machine
        .contract
        .requires
        .push(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(source, Vec::new()),
            case: StructuralCaseId::new(1).expect("supplied case"),
        });
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::StructuralCaseRequiresReadableAccess {
            machine: module.machines[0].id,
            block: module.machines[0].entry,
            source,
        }),
    );
}
