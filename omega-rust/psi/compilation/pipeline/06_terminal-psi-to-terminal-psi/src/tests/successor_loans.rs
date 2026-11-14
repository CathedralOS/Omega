//! Passing storage to a successor must keep its original borrow authority.

use super::*;
use language_core::ReferenceAccess;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_check_tables::edges::{Destination, EdgeRow};
use terminal_psi::{
    Block, StructuralAccess, StructuralArgument, StructuralPlaceDeclaration, Terminator,
};

fn duplicated_borrow() -> typed_trees_to_terminal_psi::Elaboration {
    let mut body = super::references::unit(vec![super::references::pair()], Vec::new());
    body.borrowed.push((0, ReferenceAccess::Mutable));
    let mut input = super::references::lower_lending(vec![body]).unwrap();
    let machine = &mut input.module.machines[0];
    let source = machine.structural_parameters[0].clone();
    let argument = StructuralArgument::whole(source.place, StructuralAccess::MutableBorrow);
    machine.blocks[0].terminator = Terminator::Jump {
        edge: id(1),
        target: id(2),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![argument.clone(), argument],
        cleanup_actions: Vec::new(),
    };
    let mut target = Block::plain(
        id(2),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id(2),
            cleanup_actions: Vec::new(),
        },
    );
    for position in 0..2 {
        let mut parameter = source.clone();
        parameter.place = id(2 + u64::from(position));
        parameter.position = position;
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::BlockParameter {
                block: target.id,
                position,
            },
        });
        target.structural_parameters.push(parameter);
    }
    machine.blocks.push(target);
    let tables = input.tables.machines.get_mut(machine.id).unwrap();
    tables.edges.get_mut(id(1)).unwrap().destination = Destination::Block(id(2));
    tables.edges.push(EdgeRow {
        source: id(2),
        destination: Destination::Return,
        dispositions: Vec::new(),
    });
    input
}

#[test]
fn a_successor_cannot_duplicate_one_exclusive_borrow_into_two_headers() {
    let verdict = judge(Ok(duplicated_borrow()));
    assert!(
        matches!(verdict, Err(Refusal { reason: RefusalReason::Conflict, ref detail, .. })
            if detail == "successor arguments duplicate an exclusive borrow"),
        "the successor must reject the overlapping exclusive cohort"
    );
}

#[test]
fn shared_successor_names_may_continue_one_shared_ingress() {
    let mut input = duplicated_borrow();
    let machine = &mut input.module.machines[0];
    machine.contract.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    for parameter in &mut machine.blocks[1].structural_parameters {
        parameter.access = StructuralAccess::SharedBorrow;
    }
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    for actual in structural_arguments {
        actual.access = StructuralAccess::SharedBorrow;
    }
    assert!(
        matches!(judge(Ok(input)), Ok(RouteVerdict::Accepted(_))),
        "shared continuation must retain its ingress"
    );
}

#[test]
fn an_attenuated_successor_still_conflicts_with_an_exclusive_sibling() {
    let mut input = duplicated_borrow();
    let machine = &mut input.module.machines[0];
    machine.blocks[1].structural_parameters[1].access = StructuralAccess::SharedBorrow;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[1].access = StructuralAccess::SharedBorrow;
    let tables = input.tables.machines.get_mut(machine.id).unwrap();
    let captured = tables.roots.get(id(1)).copied().unwrap();
    tables
        .obligations
        .push(terminal_check_tables::obligations::ObligationRow {
            created_at: ProgramPoint::Edge(id(1)),
            kind: ObligationKind::Loan {
                captured,
                access: ReferenceAccess::Shared,
                lender: terminal_check_tables::obligations::Lender::Incoming,
            },
        });
    assert!(
        matches!(judge(Ok(input)), Err(Refusal { reason: RefusalReason::Conflict, ref detail, .. })
        if detail == "successor arguments overlap shared and exclusive borrows"),
        "attenuation cannot hide overlapping siblings"
    );
}

#[test]
fn shared_array_views_keep_their_backing_when_sliced() {
    for (start, accepted) in [(1, true), (3, false)] {
        let source = format!(
            r#"
machine entry() -> u8 {{
    let bytes: [u8; 3] = [17, 42, 99];
    let view: &[u8] = &bytes[..];
    let suffix: &[u8] = view[{start}..];
    suffix[0]
}}
"#
        );
        let results =
            super::boundary_calls::compilation_results_of(&super::boundary_calls::typed(&source));
        assert!(!results.is_empty());
        if accepted {
            assert!(
                results
                    .iter()
                    .all(|result| matches!(result, Ok(RouteVerdict::Accepted(_)))),
                "a shared array view must retain its source through slicing"
            );
        } else {
            assert!(results.iter().any(|result| matches!(result, Ok(RouteVerdict::Rejected { unproved, .. })
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. })))),
                "reading an empty suffix must fail its bounds obligation");
        }
    }
}

#[test]
fn an_empty_shared_payload_keeps_its_owned_permission_at_a_call() {
    let typed = super::boundary_calls::typed(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine inspect<'a>(value: &'a [u8]) -> u64 { value.len }
machine entry() -> u64 {
    let mut holder: Holder;
    inspect(holder.bytes)
}
"#,
    );
    let results = super::boundary_calls::compilation_results_of(&typed);
    assert!(!results.is_empty());
    let mut owned_loan = false;
    for result in results {
        match result {
            Ok(RouteVerdict::Accepted(accepted)) => {
                owned_loan |= accepted
                    .elaboration
                    .tables
                    .machines
                    .iter()
                    .any(|(_, tables)| {
                        tables.obligations.iter().any(|(_, row)| {
                            matches!(
                                row.kind,
                                ObligationKind::Loan {
                                    lender: terminal_check_tables::obligations::Lender::Owner,
                                    ..
                                }
                            )
                        })
                    });
            }
            Ok(RouteVerdict::FallBack(refusal))
            | Err(crate::CompilationRefusal::Elaboration(refusal)) => {
                panic!("empty shared payload was refused: {}", refusal.detail);
            }
            _ => panic!("empty shared payload call was not accepted"),
        }
    }
    assert!(
        owned_loan,
        "the local empty payload must retain an owner-backed loan"
    );
}
