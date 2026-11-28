//! The value's witness survives a copy independently of its storage address.
use super::*;

fn insert_copy(machine: &mut TerminalMachine, offset: usize, operation: u64, destination: u64) {
    let producer = operation_id(operation);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place(destination),
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type: ty(7),
        },
    });
    machine.blocks[0].operations.insert(
        offset,
        Operation::unsuspended(
            producer,
            result(destination, ty(7)),
            O::StructuralLeafCopy {
                source: place(1),
                path: vec![],
            },
        ),
    );
}

fn replace_pair(machine: &mut TerminalMachine, offset: usize, operation: u64, destination: u64) {
    machine.blocks[0].operations.insert(
        offset,
        Operation::unsuspended(
            operation_id(operation),
            OperationResult::Unit,
            O::StoreStructuralField {
                disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                destination: place(destination),
                path: vec![],
                field: None,
                value: StructuralArgument::whole(place(2), StructuralAccess::Owned),
            },
        ),
    );
}

fn select_copy_source(operation: &mut Operation, slot: u64) {
    let O::StructuralLeafCopy { source, .. } = &mut operation.kind else {
        panic!("expected a package or field copy")
    };
    *source = place(slot);
}

fn original_and_copy() -> TerminalModule {
    let mut program = repacked_shared_pair();
    let machine = &mut program.machines[0];
    select_copy_source(&mut machine.blocks[0].operations[1], 6);
    insert_copy(machine, 0, 4, 6);
    let row = &mut program.execution.packages.openings[1];
    row.subject = StructuralArgument::whole(place(6), StructuralAccess::Owned);
    row.witnesses = vec![ty(2)];
    program.execution.packages.sites[1].event = TerminalPackageEvent::Project {
        opening: 1,
        field: field(2),
    };
    program
}

#[test]
fn distinct_openings_share_a_witness_only_through_a_live_whole_value_copy() {
    let program = original_and_copy();
    check_package_sites(&program)
        .expect("the original and its copy can supply fields to one result");

    let mut reversed = program.clone();
    reversed.execution.packages.openings.swap(0, 1);
    for site in &mut reversed.execution.packages.sites {
        if let TerminalPackageEvent::Project { opening, .. } = &mut site.event {
            *opening = 1 - *opening;
        }
    }
    check_package_sites(&reversed).expect("the catalog order does not select the witness origin");

    let mut foreign = program.clone();
    select_copy_source(&mut foreign.machines[0].blocks[0].operations[0], 2);
    assert_eq!(
        check_package_sites(&foreign).unwrap_err(),
        "package openings reuse a witness without a live copy of the original"
    );

    let mut replaced = program;
    replace_pair(&mut replaced.machines[0], 1, 5, 6);
    assert_eq!(
        check_package_sites(&replaced).unwrap_err(),
        "package openings reuse a witness without a live copy of the original"
    );
}

#[test]
fn assigning_a_copy_replaces_the_destination_witness() {
    let mut program = original_and_copy();
    let machine = &mut program.machines[0];
    // The destination starts as a copy of the other input.
    select_copy_source(&mut machine.blocks[0].operations[0], 2);
    insert_copy(machine, 1, 5, 7);
    replace_pair(machine, 2, 6, 6);
    let O::StoreStructuralField { value, .. } = &mut machine.blocks[0].operations[2].kind else {
        panic!("the replacement must be a structural store");
    };
    value.place = place(7);
    check_package_sites(&program).expect("the slot now carries the first input's witness");

    select_copy_source(&mut program.machines[0].blocks[0].operations[1], 2);
    assert_eq!(
        check_package_sites(&program).unwrap_err(),
        "package openings reuse a witness without a live copy of the original"
    );
}

#[test]
fn stored_callbacks_accept_shared_state_from_a_live_copy() {
    let mut program = readers();
    // This Reader can copy its shared reference and callable.
    let state_ref = &mut program.types.structural_types[5];
    let StructuralTypeShape::Reference { access, .. } = &mut state_ref.shape else {
        panic!("Reader state reference")
    };
    *access = StructuralAccess::SharedBorrow;
    for callable in &mut program.execution.callables.contracts {
        callable.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        callable.proof.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    }
    let machine = &mut program.machines[0];
    for id in [1, 2] {
        machine
            .parameter_order
            .push(BoundaryParameterKind::Structural);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place(id),
            kind: StructuralPlaceKind::Parameter {
                position: (id - 1) as u32,
            },
        });
    }
    insert_copy(machine, 0, 3, 6);
    let O::CallMachine {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].operations[2].kind
    else {
        panic!("the stored callback call")
    };
    structural_arguments[0].place = place(6);
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    check_package_sites(&program).expect("the copied state is accepted by the original callback");

    select_copy_source(&mut program.machines[0].blocks[0].operations[0], 2);
    assert_eq!(
        check_package_sites(&program).unwrap_err(),
        "package argument comes from another opening"
    );
}

#[test]
fn a_copy_can_supply_a_shared_reference_to_the_original_package() {
    let mut program = original_and_copy();
    let op = &mut program.machines[0].blocks[0].operations[3];
    op.kind = O::StoreStructuralField {
        disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
        destination: place(1),
        path: vec![],
        field: Some(field(1)),
        value: StructuralArgument::whole(place(4), StructuralAccess::Owned),
    };
    op.result = OperationResult::Unit;
    program.machines[0]
        .structural_places
        .retain(|p| p.id != place(5));
    program.execution.packages.sites[2].event = TerminalPackageEvent::Store {
        opening: 0,
        field: field(1),
    };
    check_package_sites(&program).expect("the changed field retains the destination's witness");

    select_copy_source(&mut program.machines[0].blocks[0].operations[0], 2);
    assert!(check_package_sites(&program).is_err());
}

#[test]
fn copies_compare_the_value_at_each_read_even_after_replacing_the_original() {
    let mut program = original_and_copy();
    let machine = &mut program.machines[0];
    select_copy_source(&mut machine.blocks[0].operations[1], 7);
    insert_copy(machine, 1, 5, 7);
    replace_pair(machine, 2, 6, 1);
    program.execution.packages.openings[0].subject =
        StructuralArgument::whole(place(7), StructuralAccess::Owned);
    check_package_sites(&program).expect("both copies retain the value read before replacement");

    program.machines[0].blocks[0].operations.swap(1, 2);
    assert_eq!(
        check_package_sites(&program).unwrap_err(),
        "package openings reuse a witness without a live copy of the original"
    );
}

#[test]
fn the_first_opened_slot_can_be_replaced_while_its_two_copies_survive() {
    let mut program = original_and_copy();
    let machine = &mut program.machines[0];
    select_copy_source(&mut machine.blocks[0].operations[1], 7);
    insert_copy(machine, 1, 5, 7);
    replace_pair(machine, 2, 6, 1);
    // An earlier projection opened the original slot before either copy.
    let mut first_read = machine.blocks[0].operations[3].clone();
    first_read.id = operation_id(8);
    first_read.result = result(9, ty(8));
    select_copy_source(&mut first_read, 1);
    machine.blocks[0].operations.insert(0, first_read);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place(9),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(8),
            structural_type: ty(8),
        },
    });
    let packages = &mut program.execution.packages;
    let mut second_copy = packages.openings[0].clone();
    second_copy.subject = StructuralArgument::whole(place(7), StructuralAccess::Owned);
    packages.openings.push(second_copy);
    packages.sites[0].event = TerminalPackageEvent::Project {
        opening: 2,
        field: field(1),
    };
    packages.sites.push(TerminalPackageSite {
        machine: machine_id(1),
        operation: operation_id(8),
        event: TerminalPackageEvent::Project {
            opening: 0,
            field: field(1),
        },
    });
    check_package_sites(&program).expect("the copied witnesses survive their first opening");

    // Move one of the copies until after the replacement.
    program.machines[0].blocks[0].operations.swap(2, 3);
    assert!(check_package_sites(&program).is_err());
}
