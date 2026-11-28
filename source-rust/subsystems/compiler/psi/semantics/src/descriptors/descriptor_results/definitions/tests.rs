use super::*;
use crate::descriptor_results::{DescriptorGraphError, DescriptorSlotKind, terminal_graph_closure};
use crate::module_analyses::tests::machine;
use terminal_psi::{
    Operation, OperationResult, StructuralAccess, StructuralMultiplicity, TerminalModule,
};

fn body(count: u64) -> TerminalMachine {
    let mut body = machine(1, vec![]);
    let ty = StructuralTypeId::new(1).unwrap();
    for index in 0..count {
        let producer = OperationId::new(2 * index + 1).unwrap();
        let place = PlaceId::new(producer.get()).unwrap();
        let literal = PlaceId::new(2 * index + 2).unwrap();
        body.structural_places.extend([
            StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: ty,
                },
            },
            StructuralPlaceDeclaration {
                id: literal,
                kind: StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: index as u32,
                    structural_type: ty,
                },
            },
        ]);
        body.blocks[0].operations.extend([
            Operation::unsuspended(
                producer,
                OperationResult::Structural(StructuralOperationResult {
                    place,
                    structural_type: ty,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    additional_input_qualifications: vec![],
                    qualification_establishments: vec![],
                    claims: vec![],
                }),
                OperationKind::EstablishRecord { fields: vec![] },
            ),
            Operation::unsuspended(
                OperationId::new(literal.get()).unwrap(),
                OperationResult::Unit,
                OperationKind::EstablishByteSequenceLiteral {
                    destination: literal,
                    bytes: vec![],
                    qualifications: vec![],
                },
            ),
        ]);
    }
    body
}

#[test]
fn doubling_results_and_literals_doubles_the_definition_scan() {
    for count in [32, 64] {
        let machine = body(count);
        OPERATION_VISITS.set(0);
        let definitions = Definitions::new(&machine);
        for row in &machine.structural_places {
            let (definition, ty) = definitions.resolve(row).unwrap();
            assert_eq!(ty, machine.structural_place_type(row.id));
            assert!(matches!(
                (row.kind, definition),
                (
                    StructuralPlaceKind::OperationResult { .. },
                    DescriptorDefinition::Local
                ) | (
                    StructuralPlaceKind::ByteSequenceLiteral { .. },
                    DescriptorDefinition::Literal
                )
            ));
        }
        assert_eq!(OPERATION_VISITS.get(), 2 * count as usize);
    }
}

fn assert_setup_refusal(machine: TerminalMachine) {
    let root = machine.id;
    let mut module = TerminalModule::for_entry(root);
    module.machines.push(machine);
    let error = terminal_graph_closure(&module, [root], DescriptorSlotKind::Views, &[])
        .err()
        .unwrap();
    assert_eq!(
        error,
        DescriptorGraphError::IncompleteMachine {
            machine: root,
            position: None,
            successor: None,
            detail: None
        }
    );
}

#[test]
fn missing_or_ambiguous_definitions_keep_the_machine_setup_error() {
    let mut duplicate = body(1);
    duplicate.blocks[0].operations[1].id = duplicate.blocks[0].operations[0].id;
    assert_setup_refusal(duplicate);
    let mut missing = body(1);
    missing.blocks[0].operations.pop();
    assert_setup_refusal(missing);
    let mut twice = body(1);
    let mut extra = twice.blocks[0].operations[1].clone();
    extra.id = OperationId::new(u64::MAX).unwrap();
    twice.blocks[0].operations.push(extra);
    assert_setup_refusal(twice);
    for wrong_type in [false, true] {
        let mut wrong = body(1);
        let OperationResult::Structural(result) = &mut wrong.blocks[0].operations[0].result else {
            unreachable!()
        };
        if wrong_type {
            result.structural_type = StructuralTypeId::new(2).unwrap();
        } else {
            result.place = PlaceId::new(2).unwrap();
        }
        assert_setup_refusal(wrong);
    }
}

#[test]
fn parameter_types_follow_declaration_order_and_definitions_follow_the_owner() {
    let mut body = machine(1, vec![]);
    let place = PlaceId::new(u64::MAX).unwrap();
    let first = StructuralParameterDeclaration {
        place,
        position: 2,
        structural_type: StructuralTypeId::new(1).unwrap(),
        access: StructuralAccess::Owned,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
        additional_input_qualifications: vec![],
    };
    let second = StructuralParameterDeclaration {
        position: 7,
        structural_type: StructuralTypeId::new(2).unwrap(),
        access: StructuralAccess::SharedBorrow,
        ..first.clone()
    };
    body.structural_parameters
        .extend([first.clone(), second.clone()]);
    let mut row = StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter { position: 7 },
    };
    let (definition, ty) = Definitions::new(&body).resolve(&row).unwrap();
    assert_eq!(ty, Some(first.structural_type));
    assert!(
        matches!(definition, DescriptorDefinition::Parameter { position: 1, access: StructuralAccess::SharedBorrow, structural_type } if structural_type == second.structural_type)
    );
    body.structural_places.push(row);
    body.structural_parameters.push(second.clone());
    assert_setup_refusal(body.clone());

    row.kind = StructuralPlaceKind::BlockParameter {
        block: body.entry,
        position: 0,
    };
    body.structural_places[0] = row;
    body.blocks[0].structural_parameters.push(second);
    let (arrival, ty) = Definitions::new(&body).resolve(&row).unwrap();
    assert!(matches!(
        arrival,
        DescriptorDefinition::Arrival {
            access: StructuralAccess::SharedBorrow
        }
    ));
    assert_eq!(ty, body.structural_place_type(place));
    let mut ambiguous = body.clone();
    ambiguous.blocks.push(ambiguous.blocks[0].clone());
    assert_setup_refusal(ambiguous);
    body.blocks[0].structural_parameters[0].place = PlaceId::new(1).unwrap();
    assert_setup_refusal(body);
}
