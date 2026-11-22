#[path = "support/source.rs"]
mod source;

use omega_psi::{EmbeddedLoanSource, LoweredPackage, Parameter, ReferenceAccess};
use psi_handoff_consumer::parked_reference_inputs;

const DIRECT: &str = include_str!("fixtures/returned_references/direct.omg");
const RECORD: &str = include_str!("fixtures/returned_references/in_record.omg");
const EXCLUSIVE: &str = include_str!("fixtures/returned_references/exclusive.omg");

fn lower(text: &str) -> LoweredPackage {
    source::lower(&[("case.omg", text)]).unwrap()
}

#[test]
fn a_completed_reference_keeps_the_selected_input_and_its_access_at_the_next_suspension() {
    let mut actual = Vec::new();
    for text in [DIRECT, EXCLUSIVE] {
        let package = lower(text);
        let read = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap();
        let site = read.body.suspension_sites.last().unwrap();
        actual.push(parked_reference_inputs(&package, "read", site));
        let mut wrong_access = site.clone();
        let loan = &mut wrong_access.retained_loans[0];
        loan.access = if loan.access == ReferenceAccess::Shared {
            ReferenceAccess::Mutable
        } else {
            ReferenceAccess::Shared
        };
        assert_eq!(parked_reference_inputs(&package, "read", &wrong_access), None);
    }
    spec_corpus::observe_handoff!(psi "psi-output/parked-returned-reference", assert_eq!(
        actual, [Some((vec![0], ReferenceAccess::Shared)), Some((vec![0], ReferenceAccess::Mutable))]
    ));
    // Changing the selected exclusive actual into a copy loses its authority,
    // even though the retained result, lifetime, and referent type still match.
    let mut damaged = lower(EXCLUSIVE);
    let read = damaged
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "read")
        .unwrap();
    let site = read.body.suspension_sites.last().unwrap().clone();
    let Parameter::Structural(formal) = read.body.parameters[0] else {
        panic!("missing reference formal")
    };
    let omega_psi::OperationKind::Call { arguments, .. } = &mut read.body.operations[0].kind else {
        panic!("missing first call")
    };
    arguments[0] = omega_psi::CallArgument::CopyWhole(formal);
    assert_eq!(parked_reference_inputs(&damaged, "read", &site), None);
    // The returning callee selects one of two references of the same type;
    // the lifetime alone cannot tell which input the parked descriptor reads.
    for (result, index) in [("first", 0), ("second", 1)] {
        let text = format!(
            "machine pick<'r>(first:@'r u64,second:@'r u64)->@'r u64 suspends {{{result}}}
            machine wait() suspends {{}}
            machine read<'r>(left:@'r u64,right:@'r u64)->u64 suspends {{let loan=suspend pick::<'r>(left,right);suspend wait();$loan}}"
        );
        let package = lower(&text);
        let read = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap();
        let site = read.body.suspension_sites.last().unwrap();
        assert_eq!(parked_reference_inputs(&package, "read", site), Some((vec![index], ReferenceAccess::Shared)));
        let mut wrong = site.clone();
        let Parameter::Structural(sibling) = read.body.parameters[1 - index] else {
            panic!("expected reference formal")
        };
        wrong.retained_loans[0].descriptor = sibling;
        assert_eq!(parked_reference_inputs(&package, "read", &wrong), None);
    }
}

#[test]
fn a_reference_stored_after_a_completed_call_keeps_its_result_as_the_field_origin() {
    let package = lower(RECORD);
    let read = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "read")
        .unwrap();
    let site = read.body.suspension_sites.last().unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/parked-stored-returned-reference", assert_eq!(
        parked_reference_inputs(&package,"read",site),Some((vec![0], ReferenceAccess::Shared))
    ));
    // The input has the same type and lifetime as the result, but the
    // constructor installed the result, not the formal, into its field.
    let mut wrong = site.clone();
    let Parameter::Structural(formal) = read.body.parameters[0] else {
        panic!("expected reference formal")
    };
    let EmbeddedLoanSource::CallResult { result, .. } = &mut wrong.retained_owners[0].embedded_loans[0].origins[0].source else {
        panic!("missing call origin")
    };
    *result = formal;
    assert_eq!(parked_reference_inputs(&package, "read", &wrong), None);
}

#[test]
fn mutually_exclusive_returns_park_one_descriptor_for_all_selected_formals() {
    let input = include_str!("fixtures/returned_references/branch.omg");
    for (second, expected) in [("second", vec![1, 2]), ("first", vec![1])] {
        let text = input.replace("false -> second", &format!("false -> {second}"));
        let mut package = lower(&text);
        let read = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap();
        let [site] = read.body.suspension_sites.as_slice() else { panic!("one wait") };
        assert_eq!(parked_reference_inputs(&package, "read", site), Some((expected, ReferenceAccess::Mutable)));
        let mut duplicate = site.clone();
        duplicate
            .retained_loans
            .push(duplicate.retained_loans[0].clone());
        assert_eq!(parked_reference_inputs(&package, "read", &duplicate), None);
        if second == "second" {
            let site = site.clone();
            let read = package
                .machines
                .iter_mut()
                .find(|m| m.declaration.path == "read")
                .unwrap();
            let Parameter::Structural(right) = read.body.parameters[2] else {
                panic!("right reference")
            };
            let omega_psi::OperationKind::Call { arguments, .. } = &mut read.body.operations[0].kind else {
                panic!("pick call")
            };
            arguments[2] = omega_psi::CallArgument::CopyWhole(right);
            assert_eq!(parked_reference_inputs(&package, "read", &site), None);
        }
    }
}

#[test]
fn a_joined_exclusive_descriptor_retains_every_reachable_local_home() {
    use omega_psi::{BlockTerminator, CallArgument, OperationKind, OperationResult, PlaceOrigin, PlaceProjection};
    use std::collections::BTreeSet;

    let input = include_str!("fixtures/returned_references/branch_homes.omg");
    for (arm, count) in [("a", 1), ("b", 2)] {
        let program = input.replace("false -> @mutable b", &format!("false -> @mutable {arm}"));
        for (statement, value) in [
            ("", "$chosen"),
            ("let loan=chosen;", "$loan"),
            ("let loan=identity(chosen);", "$loan"),
        ] {
            let text = program
                .replace("let loan=identity(chosen);", statement)
                .replace("$loan", value);
            let package = lower(&text);
            let body = &package
                .machines
                .iter()
                .find(|m| m.declaration.path == "read")
                .unwrap()
                .body;
            let [site] = body.suspension_sites.as_slice() else { panic!("one wait") };
            let [loan] = site.retained_loans.as_slice() else {
                panic!("one returned authority")
            };
            assert_eq!(
                (loan.access, loan.needed_by_callee, loan.needed_after_completion),
                (ReferenceAccess::Mutable, false, true)
            );
            let join = match body.places[loan.descriptor.index()].origin {
                PlaceOrigin::BlockParameter(_) if statement.is_empty() => loan.descriptor,
                PlaceOrigin::Operation(op) => match &body.operations[op.index()].kind {
                    OperationKind::MoveStructural { source } if statement == "let loan=chosen;" && source.path.is_empty() => source.root,
                    OperationKind::Call { arguments, .. } if statement == "let loan=identity(chosen);" => {
                        let [
                            CallArgument::Borrow {
                                place,
                                access: ReferenceAccess::Mutable,
                            },
                        ] = arguments.as_slice()
                        else {
                            panic!("identity reborrow")
                        };
                        assert_eq!(place.path, [PlaceProjection::Referent]);
                        place.root
                    }
                    _ => panic!("unexpected producer for {statement}"),
                },
                _ => panic!("expected a joined or transferred descriptor"),
            };
            let PlaceOrigin::BlockParameter(block) = body.places[join.index()].origin else {
                panic!("joined exclusive descriptor")
            };
            assert_eq!(body.blocks[block.get() - 1].structural_parameters, [join]);
            let by_arrival: Vec<_> = body
                .blocks
                .iter()
                .filter_map(|b| match &b.terminator {
                    BlockTerminator::Jump(edge) if edge.target == block => Some(edge),
                    _ => None,
                })
                .map(|edge| {
                    let [CallArgument::MoveWhole(source)] = edge.structural_arguments.as_slice() else {
                        panic!("each edge transfers an exclusive descriptor once")
                    };
                    let PlaceOrigin::Operation(op) = body.places[source.index()].origin else {
                        panic!("branch borrow")
                    };
                    let establish = &body.operations[op.index()];
                    assert_eq!(establish.result, OperationResult::Structural(*source));
                    let OperationKind::BorrowPlace {
                        source,
                        access: ReferenceAccess::Mutable,
                    } = &establish.kind
                    else {
                        panic!("expected mutable borrow")
                    };
                    assert!(source.path.is_empty());
                    source.root
                })
                .collect();
            assert_eq!(by_arrival.len(), 2);
            let possible: BTreeSet<_> = by_arrival.into_iter().map(|p| p.index()).collect();
            assert_eq!(possible.len(), count);
            let correct = |site: &omega_psi::SuspensionSite| {
                site.retained_homes.len() == possible.len()
                    && site
                        .retained_homes
                        .iter()
                        .all(|home| !home.needed_by_callee && home.needed_after_completion)
                    && site
                        .retained_homes
                        .iter()
                        .map(|home| home.place.index())
                        .collect::<BTreeSet<_>>()
                        == possible
            };
            assert!(correct(site));
            let mut missing = site.clone();
            missing.retained_homes.pop();
            assert!(!correct(&missing));
        }
    }
}
