//! A parked loan of a field of a `[copy]` record
//! (`wiki/spec/terminal-psi/calls_and_outcomes.md#suspension`).
//!
//! `suspend wait(@x.key)` lends a field of `x`. For an affine `Item` the
//! ownership pass publishes an owner row for `x`, and that row backs the loan.
//! A `[copy]` `Item` has no ownership claim, but its storage must still stay in
//! the parked activation, so the site gives `x` a home row with the carry its
//! schema composes. The borrow operand keeps the captured place and access.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{Carry, OwnedCustody, PlaceProjection, ReferenceAccess, ValueOperand};
use psi_handoff_consumer::{CopyableHome, parked_copyable_homes, parked_homes};

const CASE: &str = include_str!("fixtures/package_cases/copyable-record-field-loan-at-suspension/case/case.omg");
const CONTROL: &str = include_str!("fixtures/package_cases/copyable-record-field-loan-at-suspension/control/case.omg");

const UNRESTRICTED: Carry = Carry {
    allows_suspension: true,
    requires_same_cpu: false,
    requires_same_thread: false,
    requires_stable_address: false,
};

fn caller(package: &omega_lowering::LoweredPackage) -> &omega_lowering::LoweredMachine {
    package.machines.iter().find(|m| m.declaration.path == "f").unwrap()
}

/// The operands of the one parked call.
fn parked_arguments(body: &omega_lowering::MachineBody) -> Vec<ValueOperand> {
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    match site.call {
        omega_lowering::CallSite::Operation(id) => match &body.operations[id.index()].kind {
            omega_lowering::OperationKind::Call { arguments, .. } => arguments.clone(),
            _ => panic!("a call"),
        },
        omega_lowering::CallSite::Edge(id) => body
            .blocks
            .iter()
            .find_map(|b| match &b.terminator {
                omega_lowering::BlockTerminator::TerminalCall { id: e, arguments, .. } if *e == id => Some(arguments.clone()),
                _ => None,
            })
            .unwrap(),
        omega_lowering::CallSite::Cleanup(_) => panic!("a cleanup call does not park"),
    }
}

#[test]
fn a_parked_field_loan_keeps_the_copyable_record_in_the_activation() {
    let pkg = source::lower(&[("case.omg", CASE)]).unwrap();
    let body = &caller(&pkg).body;
    let omega_lowering::Parameter::Structural(x) = body.parameters[0] else {
        panic!("x is a structural formal")
    };
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    let arguments = parked_arguments(body);
    let [ValueOperand::Borrow { place, access }] = arguments.as_slice() else {
        panic!("one borrowed operand")
    };
    let (place, access) = (place.clone(), *access);
    spec_corpus::observe_handoff!(psi "psi-output/copyable-record-field-loan-at-suspension", assert_eq!(
        (
            (place.root, place.path.len(), matches!(place.path[0], PlaceProjection::Field(_)), access),
            (site.retained_owners.is_empty(), site.retained_loans.is_empty()),
            parked_copyable_homes(&pkg, body, site),
            parked_homes(body, site),
        ),
        (
            (x, 1, true, ReferenceAccess::Shared),
            (true, true),
            Ok(vec![CopyableHome {
                place: x,
                formal: Some(0),
                carry: UNRESTRICTED,
                needed_by_callee: true,
                needed_after_completion: false,
                loans: vec![],
            }]),
            Ok(vec![]),
        )
    ));

    // The affine control backs the same loan with an owner row and no home.
    let control = source::lower(&[("case.omg", CONTROL)]).unwrap();
    let body = &caller(&control).body;
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    assert!(site.retained_homes.is_empty());
    let [owner] = site.retained_owners.as_slice() else {
        panic!("one owner row")
    };
    assert_eq!((owner.place, owner.custody), (x, OwnedCustody::Caller));
    assert_eq!(parked_copyable_homes(&control, body, site), Ok(vec![]));
}

#[test]
fn a_named_loan_of_the_field_is_retained_with_its_copyable_backing() {
    // The same field lent through a named reborrow: the site retains the
    // descriptor `k`, and `k`'s defining borrow selects the record's storage.
    let text = CASE.replace("suspend wait(@x.key)", "let k = @x.key; suspend wait(k)");
    let pkg = source::lower(&[("named.omg", &text)]).unwrap();
    let body = &caller(&pkg).body;
    let omega_lowering::Parameter::Structural(x) = body.parameters[0] else {
        panic!("x is a structural formal")
    };
    let [site] = body.suspension_sites.as_slice() else {
        panic!("only wait parks")
    };
    let [loan] = site.retained_loans.as_slice() else {
        panic!("one retained loan")
    };
    assert_eq!((loan.access, loan.needed_by_callee), (ReferenceAccess::Shared, true));
    assert_eq!(
        parked_copyable_homes(&pkg, body, site),
        Ok(vec![CopyableHome {
            place: x,
            formal: Some(0),
            carry: UNRESTRICTED,
            needed_by_callee: true,
            needed_after_completion: false,
            loans: vec![loan.descriptor],
        }])
    );
}

#[test]
fn the_reader_rejects_a_missing_or_misstated_copyable_home() {
    let pkg = source::lower(&[("case.omg", CASE)]).unwrap();
    let mut site = caller(&pkg).body.suspension_sites[0].clone();
    let body = &caller(&pkg).body;
    let row = site.retained_homes.pop().unwrap();
    assert_eq!(
        parked_copyable_homes(&pkg, body, &site),
        Err("needed copyable storage has no row, or the rows are out of order")
    );
    let mut restricted = row.clone();
    restricted.carry.requires_same_thread = true;
    site.retained_homes.push(restricted);
    assert_eq!(
        parked_copyable_homes(&pkg, body, &site),
        Err("a copyable home row disagrees with its storage type or composed carry")
    );
    site.retained_homes[0] = row.clone();
    site.retained_homes[0].needed_after_completion = true;
    assert_eq!(
        parked_copyable_homes(&pkg, body, &site),
        Err("a copyable home row's needs disagree with the call and its continuation")
    );
    // A row for storage the call does not need, beside the real one.
    let control = source::lower(&[("case.omg", CONTROL)]).unwrap();
    let body = &caller(&control).body;
    let mut site = body.suspension_sites[0].clone();
    site.retained_homes.push(row);
    assert_eq!(
        parked_copyable_homes(&control, body, &site),
        Err("a home row names storage that is neither a primitive home nor needed copyable storage")
    );
}
