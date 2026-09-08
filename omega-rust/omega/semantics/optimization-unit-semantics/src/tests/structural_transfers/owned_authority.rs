//! Current cyclic ownership and retained frontier corruption controls.

use super::*;
use optimization_unit::{
    OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace, OwnershipFrontierPartialCustody,
    OwnershipFrontierSite, OwnershipFrontierSnapshot,
};
use semantic_vocabulary::ClaimId;
use terminal_psi::{StructuralPathSegment, TerminalAffineCleanupAction};

fn snapshot(places: &[u64]) -> OwnershipFrontierSnapshot {
    OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: places
            .iter()
            .map(|place| OwnershipFrontierOwnedPlace {
                place: id(*place, PlaceId::new),
                multiplicity: StructuralMultiplicity::Affine,
            })
            .collect(),
        partial_custody: Vec::new(),
    }
}

fn retain_edge_snapshots(
    candidate: &mut PsiOptimizationUnit,
    entry: OwnershipFrontierSnapshot,
    exit: OwnershipFrontierSnapshot,
) {
    // Rebuild fact identities as well as the unit identity, so a corruption
    // reaches semantic replay instead of failing a stale fingerprint check.
    let machine = candidate.functions[0].machine;
    let edge = id(20, EdgeId::new);
    candidate.ownership_frontier_facts = vec![
        OwnershipFrontierFact::new(
            candidate.psi,
            machine,
            OwnershipFrontierSite::EdgeEntry(edge),
            entry,
        ),
        OwnershipFrontierFact::new(
            candidate.psi,
            machine,
            OwnershipFrontierSite::EdgeExit(edge),
            exit,
        ),
    ];
    refresh_identity(candidate);
}

#[test]
fn owned_arrivals_replay_exact_retained_snapshots_and_reject_corruption() {
    for mutation in 0..7 {
        let mut candidate = super::owned::owned_unit();
        let mut entry = snapshot(&[10, 30]);
        let mut exit = snapshot(&[11, 31]);
        match mutation {
            0 => {}
            1 => exit.owned_places.insert(0, entry.owned_places[0]),
            2 => {
                exit.owned_places.pop();
            }
            3 => exit.owned_places[0].multiplicity = StructuralMultiplicity::Unrestricted,
            4 => {
                let claim = OwnershipFrontierLiveClaim {
                    claim: id(80, ClaimId::new),
                    input: Some(id(10, PlaceId::new)),
                    path: Vec::new(),
                    multiplicity: Some(StructuralMultiplicity::Affine),
                };
                entry.claims.push(claim.clone());
                exit.claims.push(claim);
            }
            5 => {
                let custody = OwnershipFrontierPartialCustody {
                    place: id(10, PlaceId::new),
                    moved_paths: vec![vec![StructuralPathSegment::Field("moved".into())]],
                };
                entry.partial_custody.push(custody.clone());
                exit.partial_custody.push(custody);
            }
            6 => entry.owned_places[0].multiplicity = StructuralMultiplicity::Unrestricted,
            _ => unreachable!(),
        }
        retain_edge_snapshots(&mut candidate, entry, exit);
        let result = validate_psi_optimization_unit(&candidate);
        if mutation == 0 {
            result.expect("exact retained source and rebound destination frontiers");
        } else {
            assert!(
                matches!(
                    result,
                    Err(
                        OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch { .. }
                    )
                ),
                "mutation {mutation}: {result:?}"
            );
        }
    }
}

#[test]
fn retained_snapshots_cannot_authorize_duplicate_current_transfer() {
    let mut candidate = super::owned::owned_unit();
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut candidate.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[1].argument.place = structural_bindings[0].argument.place;
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    retain_edge_snapshots(&mut candidate, snapshot(&[10, 30]), snapshot(&[11, 31]));
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

fn cyclic_owned_unit(swap: bool) -> PsiOptimizationUnit {
    let mut candidate = super::owned::owned_unit();
    let target = candidate.functions[0].blocks[1].id;
    let sources = if swap { [31, 11] } else { [11, 31] };
    candidate.functions[0].blocks[1].nodes[1].operation = AbstractOperation::Jump {
        psi_edge: id(22, EdgeId::new),
        target,
        bindings: Vec::new(),
        structural_bindings: [11, 31]
            .into_iter()
            .zip(sources)
            .map(|(destination, source)| AbstractStructuralBinding {
                parameter: id(destination, PlaceId::new),
                argument: StructuralArgument {
                    place: id(source, PlaceId::new),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            })
            .collect(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    // Jump fuel belongs to its successor; the replaced return charged the node.
    candidate.functions[0].blocks[1].nodes[1].fuel.clear();
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    candidate
}

fn validate_cycle(candidate: &PsiOptimizationUnit) -> Result<(), OptimizationUnitValidationError> {
    crate::validate_psi_optimization_unit_with_admitted_cycle_machines(
        candidate,
        &[candidate.functions[0].machine],
    )
}

#[test]
fn owned_identity_and_swap_backedges_rejoin_without_duplicate_custody() {
    for swap in [false, true] {
        let mut candidate = cyclic_owned_unit(swap);
        validate_cycle(&candidate).expect("simultaneous owned backedge bindings");
        retain_edge_snapshots(&mut candidate, snapshot(&[10, 30]), snapshot(&[11, 31]));
        for site in [
            OwnershipFrontierSite::EdgeEntry(id(22, EdgeId::new)),
            OwnershipFrontierSite::EdgeExit(id(22, EdgeId::new)),
        ] {
            candidate
                .ownership_frontier_facts
                .push(OwnershipFrontierFact::new(
                    candidate.psi,
                    candidate.functions[0].machine,
                    site,
                    snapshot(&[11, 31]),
                ));
        }
        candidate
            .ownership_frontier_facts
            .sort_by_key(|fact| (fact.machine, fact.site));
        refresh_identity(&mut candidate);
        validate_cycle(&candidate)
            .expect("retained backedge snapshots also use simultaneous binding");
    }
}

#[test]
fn owned_backedge_cannot_reuse_consumed_invocation_input() {
    let mut candidate = cyclic_owned_unit(false);
    validate_cycle(&candidate).unwrap();
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut candidate.functions[0].blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.place = id(10, PlaceId::new);
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    assert!(matches!(
        validate_cycle(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn owned_backedge_cannot_overwrite_an_independently_live_destination() {
    let mut candidate = cyclic_owned_unit(false);
    let function = &mut candidate.functions[0];
    let mut spare = function.structural_parameters[0].clone();
    spare.place = id(40, PlaceId::new);
    spare.position = 2;
    function.structural_parameters.push(spare);
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: id(40, PlaceId::new),
            kind: StructuralPlaceKind::Parameter {
                position: 2,
                is_self: false,
            },
        });
    function.declared_places.insert(id(40, PlaceId::new));
    refresh_identity(&mut candidate);
    validate_cycle(&candidate).expect("the untouched spare remains live across the loop");
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut candidate.functions[0].blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.place = id(40, PlaceId::new);
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    assert!(matches!(
        validate_cycle(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn unrestricted_owned_arrivals_allow_repeated_source_without_affine_cleanup() {
    let mut candidate = super::owned::owned_unit();
    let function = &mut candidate.functions[0];
    for parameter in function.structural_parameters.iter_mut().chain(
        function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.structural_parameters),
    ) {
        parameter.multiplicity = StructuralMultiplicity::Unrestricted;
    }
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut function.blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[1].argument.place = structural_bindings[0].argument.place;
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut function.blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    retain_edge_snapshots(&mut candidate, snapshot(&[]), snapshot(&[]));
    validate_psi_optimization_unit(&candidate)
        .expect("unrestricted arrivals create no affine debt");
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut candidate.functions[0].blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(id(
        11,
        PlaceId::new,
    )));
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn mixed_owned_arrivals_dispose_only_the_affine_destination() {
    let mut candidate = super::owned::owned_unit();
    candidate.functions[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Unrestricted;
    candidate.functions[0].blocks[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Unrestricted;
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut candidate.functions[0].blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    *cleanup_actions = vec![TerminalAffineCleanupAction::DiscardRoot(id(
        31,
        PlaceId::new,
    ))];
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    retain_edge_snapshots(&mut candidate, snapshot(&[30]), snapshot(&[31]));
    validate_psi_optimization_unit(&candidate)
        .expect("only the affine destination requires disposal");
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut candidate.functions[0].blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    refresh_node_derivatives(&mut candidate, 0, 1, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
}

#[test]
fn owned_arrivals_reject_even_matching_source_and_destination_qualifications() {
    for source_qualified in [false, true] {
        for destination_qualified in [false, true] {
            let mut candidate = super::owned::owned_unit();
            let domain = structural_domain(90, 90, candidate.structural_types[0].id);
            if source_qualified {
                candidate.functions[0].structural_parameters[0]
                    .qualifications
                    .push(domain.id);
            }
            if destination_qualified {
                candidate.functions[0].blocks[1].structural_parameters[0]
                    .qualifications
                    .push(domain.id);
            }
            candidate.structural_domains = vec![domain].into();
            refresh_identity(&mut candidate);
            let result = validate_psi_optimization_unit(&candidate);
            if source_qualified || destination_qualified {
                assert!(matches!(result,
                    Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)),
                    "source={source_qualified}, destination={destination_qualified}: {result:?}");
            } else {
                result.expect("an unused valid domain does not affect plain owned admission");
            }
        }
    }
}

#[test]
fn owned_arrival_source_cannot_carry_a_declared_whole_claim() {
    let mut candidate = super::owned::owned_unit();
    validate_psi_optimization_unit(&candidate).unwrap();
    let claim = id(1, ClaimId::new);
    candidate.functions[0].entry_claims.insert(claim);
    candidate.functions[0]
        .entry_claim_declarations
        .push(terminal_psi::EntryClaim {
            claim,
            input: id(10, PlaceId::new),
            path: Vec::new(),
        });
    refresh_identity(&mut candidate);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)
    ));
}
