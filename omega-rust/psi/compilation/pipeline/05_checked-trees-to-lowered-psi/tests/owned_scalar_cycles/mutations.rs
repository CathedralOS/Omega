use semantic_vocabulary::{EdgeId, PlaceId};
use terminal_psi::{
    OperationKind, StructuralArgument, TerminalMachine, TerminalNaturalRankComparison,
    TerminalRankedScc, Terminator,
};

use super::{CUSTOMER, support};

#[test]
fn canonical_loop_field_reads_reject_block_declaration_and_custody_drift() {
    let (original, proof, _, _) = support::publish(&super::field_read_customer());
    let read = support::operation(support::walk(&original), |kind| {
        matches!(kind, OperationKind::IntegerStructuralField { .. })
    });
    let OperationKind::IntegerStructuralField { source, .. } = read.kind else {
        unreachable!();
    };
    for mutation in [
        "block",
        "position",
        "write-only",
        "claim",
        "field",
        "stale entry",
    ] {
        let mut module = original.clone();
        let walk = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        match mutation {
            "block" => {
                walk.structural_places
                    .iter_mut()
                    .find(|place| place.id == source)
                    .unwrap()
                    .kind = semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                    block: walk.entry,
                    position: 0,
                }
            }
            "position" | "write-only" => {
                let parameter = walk
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.structural_parameters)
                    .find(|parameter| parameter.place == source)
                    .unwrap();
                if mutation == "position" {
                    parameter.position += 1;
                } else {
                    parameter.access = terminal_psi::StructuralAccess::WriteOnlyBorrow;
                }
            }
            "claim" => walk.entry_claims.push(terminal_psi::EntryClaim {
                claim: semantic_vocabulary::ClaimId::new(1).unwrap(),
                input: source,
                path: Vec::new(),
            }),
            "field" | "stale entry" => {
                let operation = walk
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| operation.id == read.id)
                    .unwrap();
                let OperationKind::IntegerStructuralField { source, field, .. } =
                    &mut operation.kind
                else {
                    unreachable!();
                };
                if mutation == "field" {
                    *field = semantic_vocabulary::StructuralFieldId::new(u64::MAX).unwrap();
                } else {
                    *source = walk.structural_parameters[0].place;
                }
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "accepted field {mutation}"
        );
        if mutation != "stale entry" {
            assert!(
                terminal_codec::encode_module(&module).is_err(),
                "codec accepted malformed field {mutation}"
            );
        }
    }
}

fn successor_arguments(
    machine: &mut TerminalMachine,
    edge: EdgeId,
) -> &mut Vec<StructuralArgument> {
    for block in &mut machine.blocks {
        match &mut block.terminator {
            Terminator::Jump {
                edge: candidate,
                structural_arguments,
                ..
            } if *candidate == edge => return structural_arguments,
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if when_true.edge == edge {
                    return &mut when_true.structural_arguments;
                }
                if when_false.edge == edge {
                    return &mut when_false.structural_arguments;
                }
            }
            _ => {}
        }
    }
    panic!("actual cyclic successor");
}

#[test]
fn published_cycle_rejects_owned_descriptor_and_transfer_drift() {
    let (original, proof, _, _) = support::publish(CUSTOMER);
    let walk = support::walk(&original);
    let Some(TerminalRankedScc::Natural(components)) = &walk.ranked_scc else {
        panic!("Natural cycle");
    };
    let edge = components[0]
        .edges
        .iter()
        .find(|edge| !support::successor(walk, edge.edge).2.is_empty())
        .expect("owned descriptor crosses a cyclic successor")
        .edge;
    let entry_place = walk.structural_parameters[0].place;
    assert_ne!(
        support::successor(walk, edge).2[0].place,
        entry_place,
        "the loop consumes a rebound descriptor, not the moved invocation input"
    );
    for mutation in [
        "unknown identity",
        "moved entry identity",
        "missing transfer",
        "duplicate transfer",
        "borrow rewrite",
        "field projection",
    ] {
        let mut module = original.clone();
        let walk = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let arguments = successor_arguments(walk, edge);
        assert_eq!(arguments.len(), 1);
        match mutation {
            "unknown identity" => arguments[0].place = PlaceId::new(u64::MAX).unwrap(),
            "moved entry identity" => arguments[0].place = entry_place,
            "missing transfer" => arguments.clear(),
            "duplicate transfer" => arguments.push(arguments[0].clone()),
            "borrow rewrite" => arguments[0].access = terminal_psi::StructuralAccess::SharedBorrow,
            "field projection" => arguments[0]
                .path
                .push(terminal_psi::StructuralPathSegment::Field("limit".into())),
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn published_cycle_rejects_missing_duplicate_or_substituted_natural_comparisons() {
    let (original, original_proof, _, _) = support::publish(CUSTOMER);
    for mutation in [
        "missing rank edge",
        "duplicate rank edge",
        "no strict edge",
        "missing comparison",
        "duplicate comparison",
        "missing group",
        "duplicate group",
    ] {
        let mut module = original.clone();
        let mut proof = original_proof.clone();
        let walk = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let Some(TerminalRankedScc::Natural(components)) = &mut walk.ranked_scc else {
            panic!("Natural cycle");
        };
        match mutation {
            "missing rank edge" => {
                components[0].edges.pop();
            }
            "duplicate rank edge" => {
                let edge = components[0].edges[0];
                components[0].edges.insert(0, edge);
            }
            "no strict edge" => {
                for edge in &mut components[0].edges {
                    edge.comparison = TerminalNaturalRankComparison::Preserving;
                }
            }
            "missing comparison" => {
                proof.control_cycles[0].certificate.edges.pop();
            }
            "duplicate comparison" => {
                let edge = proof.control_cycles[0].certificate.edges[0].clone();
                proof.control_cycles[0].certificate.edges.insert(0, edge);
            }
            "missing group" => proof.control_cycles.clear(),
            "duplicate group" => proof.control_cycles.push(proof.control_cycles[0].clone()),
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn published_cycle_rejects_erased_guard_or_non_decreasing_successor() {
    let (original, proof, _, _) = support::publish(CUSTOMER);
    for mutation in [
        "guard erased",
        "guard reversed",
        "decrement erased",
        "decrement changed to zero",
    ] {
        let mut module = original.clone();
        let walk = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let operation = walk
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                if mutation.starts_with("guard") {
                    matches!(operation.kind, OperationKind::IntegerLessThan { .. })
                } else {
                    matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. })
                }
            })
            .unwrap();
        match (&mut operation.kind, mutation) {
            (kind, "guard erased") => *kind = OperationKind::BooleanConstant { value: true },
            (OperationKind::IntegerLessThan { left, right }, "guard reversed") => {
                std::mem::swap(left, right)
            }
            (OperationKind::ExactIntegerSubtract { right, .. }, "decrement changed to zero") => {
                // A typed zero in place of the required unit decrement leaves
                // the scalar safe but cannot discharge a strict comparison.
                let right = *right;
                let origin = *support::scalar_origins(support::walk(&original), right)
                    .first()
                    .unwrap();
                let constant = walk
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| {
                        operation
                            .result
                            .scalar()
                            .is_some_and(|value| value.id == origin)
                    })
                    .unwrap();
                constant.kind = OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Unsigned(0),
                };
            }
            (kind, "decrement erased") => {
                *kind = OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Unsigned(5),
                }
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &proof,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "accepted {mutation}"
        );
    }
}
