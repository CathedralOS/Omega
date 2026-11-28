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
