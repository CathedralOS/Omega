//! Record result contracts compose with scalar work and loans of owned storage.
use super::*;
use abstract_operations::{AbstractFunctionResult, AbstractResult};
use terminal_psi::{
    StructuralArgument, StructuralParameterDeclaration, StructuralResultDeclaration,
};

fn empty_contract(raw: u64) -> terminal_psi::MachineContract {
    terminal_psi::MachineContract {
        id: id(raw, semantic_vocabulary::ContractId::new),
        requires: Vec::new(),
        ensures: Vec::new(),
        crash_routes: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn record_call_unit(access: StructuralAccess, projected: bool) -> PsiOptimizationUnit {
    let mut candidate = record_unit(true);
    let mut maker = candidate.functions.remove(0);
    maker.machine = id(950, MachineId::new);
    maker.verified_contract = Some(empty_contract(950));
    let scalar = maker.blocks[0].nodes[0].definitions[0].scalar_type;
    let parent = id(902, StructuralTypeId::new);
    let primitive = id(951, StructuralTypeId::new);
    candidate
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: primitive,
            identity: "test::mutable-byte".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        });
    let parameter = |place, structural_type, access, multiplicity| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let input = parameter(
        id(952, PlaceId::new),
        primitive,
        StructuralAccess::MutableBorrow,
        StructuralMultiplicity::Unrestricted,
    );
    maker.structural_parameters.push(input.clone());
    maker.declared_places.insert(input.place);
    maker
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: input.place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    maker.result = AbstractFunctionResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: id(953, PlaceId::new),
        structural_type: parent,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    maker.declared_places.insert(id(953, PlaceId::new));
    maker
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: id(953, PlaceId::new),
            kind: StructuralPlaceKind::Result,
        });
    let mut store = maker.blocks[0].nodes[0].clone();
    store.operation = AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation: id(954, OperationId::new),
        destination: input,
        value: AbstractResult {
            value: id(3, ValueId::new),
            scalar_type: scalar,
        },
    };
    maker.blocks[0].nodes.insert(1, store);
    maker.blocks[0].nodes.last_mut().unwrap().operation = AbstractOperation::ReturnStructural {
        psi_edge: id(955, EdgeId::new),
        source: id(912, PlaceId::new),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };

    let mut caller = unit().functions.remove(0);
    caller.verified_contract = Some(empty_contract(960));
    let input = parameter(
        id(961, PlaceId::new),
        primitive,
        StructuralAccess::MutableBorrow,
        StructuralMultiplicity::Unrestricted,
    );
    caller.structural_parameters.push(input.clone());
    caller.declared_places.insert(input.place);
    caller
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: input.place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    let place = id(962, PlaceId::new);
    let operation = id(963, OperationId::new);
    caller.declared_places.insert(place);
    caller
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: parent,
            },
        });
    let mut call = caller.blocks[0].nodes[0].clone();
    call.operation = AbstractOperation::CallStructural {
        psi_operation: operation,
        result: terminal_psi::StructuralOperationResult {
            place,
            structural_type: parent,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        callee: maker.machine,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: input.place,
            access: input.access,
            path: Vec::new(),
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    caller.blocks[0].nodes.insert(1, call);
    let mut receiver = unit().functions.remove(0);
    receiver.machine = id(970, MachineId::new);
    receiver.verified_contract = Some(empty_contract(970));
    receiver.result = AbstractFunctionResult::Unit;
    let receiver_input = parameter(
        id(971, PlaceId::new),
        if projected {
            id(901, StructuralTypeId::new)
        } else {
            parent
        },
        access,
        StructuralMultiplicity::Unrestricted,
    );
    receiver.structural_parameters.push(receiver_input.clone());
    receiver.declared_places.insert(receiver_input.place);
    receiver
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: receiver_input.place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    receiver.blocks[0].nodes.truncate(1);
    receiver.blocks[0].nodes[0].operation = AbstractOperation::ReturnUnit {
        psi_edge: id(972, EdgeId::new),
        cleanup_actions: Vec::new(),
    };
    let mut loan = caller.blocks[0].nodes[0].clone();
    loan.operation = AbstractOperation::CallUnit {
        psi_operation: id(973, OperationId::new),
        callee: receiver.machine,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place,
            access,
            path: if projected {
                vec![terminal_psi::StructuralPathSegment::Field("first".into())]
            } else {
                Vec::new()
            },
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    caller.blocks[0].nodes.insert(2, loan);
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut caller.blocks[0].nodes.last_mut().unwrap().operation
    else {
        panic!("return");
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
        place,
    ));
    candidate.functions = vec![caller, maker, receiver];
    for index in 0..candidate.functions.len() {
        refresh_function_derivatives(&mut candidate, index);
    }
    candidate
}

#[test]
fn nested_record_call_with_scalar_work_and_mutable_input_retains_result_loans() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        for projected in [false, true] {
            let candidate = record_call_unit(access, projected);
            assert_eq!(
                validate_psi_optimization_unit(&candidate),
                Ok(()),
                "{access:?} projected={projected}"
            );
        }
    }
}

#[test]
fn nested_record_call_contract_and_loan_substitutions_reject() {
    let original = record_call_unit(StructuralAccess::MutableBorrow, false);
    assert_eq!(validate_psi_optimization_unit(&original), Ok(()));
    for mutation in 0..6 {
        let mut changed = original.clone();
        let AbstractOperation::CallStructural {
            result,
            structural_arguments,
            ..
        } = &mut changed.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("call");
        };
        match mutation {
            0 => result.structural_type = id(901, StructuralTypeId::new),
            1 => result.multiplicity = StructuralMultiplicity::Unrestricted,
            2 => result
                .qualifications
                .push(id(980, semantic_vocabulary::StructuralDomainId::new)),
            3 => result
                .claims
                .push(terminal_psi::StructuralResultClaimBinding {
                    claim: id(980, semantic_vocabulary::ClaimId::new),
                    path: Vec::new(),
                }),
            4 => structural_arguments[0].place = id(962, PlaceId::new),
            5 => {
                let AbstractOperation::CallUnit {
                    structural_arguments,
                    ..
                } = &mut changed.functions[0].blocks[0].nodes[2].operation
                else {
                    panic!("loan");
                };
                structural_arguments[0].place = id(961, PlaceId::new);
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}

fn consume_result(candidate: &mut PsiOptimizationUnit, projected: bool, before_loan: bool) {
    let mut consumer = candidate.functions[2].clone();
    consumer.machine = id(990, MachineId::new);
    consumer.verified_contract = Some(empty_contract(990));
    consumer.structural_parameters[0].access = StructuralAccess::Owned;
    consumer.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    consumer.structural_parameters[0].structural_type =
        id(if projected { 901 } else { 902 }, StructuralTypeId::new);
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut consumer.blocks[0].nodes[0].operation
    else {
        panic!("consumer return");
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
        consumer.structural_parameters[0].place,
    ));
    let mut transfer = candidate.functions[0].blocks[0].nodes[2].clone();
    let AbstractOperation::CallUnit {
        psi_operation,
        callee,
        structural_arguments,
        ..
    } = &mut transfer.operation
    else {
        panic!("receiver call");
    };
    *psi_operation = id(991, OperationId::new);
    *callee = consumer.machine;
    structural_arguments[0].access = StructuralAccess::Owned;
    structural_arguments[0].path = if projected {
        vec![terminal_psi::StructuralPathSegment::Field("first".into())]
    } else {
        Vec::new()
    };
    candidate.functions[0].blocks[0]
        .nodes
        .insert(if before_loan { 2 } else { 3 }, transfer);
    let AbstractOperation::Return {
        cleanup_actions, ..
    } = &mut candidate.functions[0].blocks[0]
        .nodes
        .last_mut()
        .unwrap()
        .operation
    else {
        panic!("caller return");
    };
    cleanup_actions.clear();
    if projected {
        cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
            terminal_psi::StructuralAffineDiscard {
                place: id(962, PlaceId::new),
                path: vec![terminal_psi::StructuralPathSegment::Field("second".into())],
                structural_type: id(901, StructuralTypeId::new),
            },
        ));
    }
    let cleanup_actions = cleanup_actions.clone();
    candidate.functions[0].result = AbstractFunctionResult::Unit;
    candidate.functions[0].blocks[0]
        .nodes
        .last_mut()
        .unwrap()
        .operation = AbstractOperation::ReturnUnit {
        psi_edge: id(992, EdgeId::new),
        cleanup_actions,
    };
    candidate.functions.push(consumer);
    for index in 0..candidate.functions.len() {
        refresh_function_derivatives(candidate, index);
    }
}

#[test]
fn completed_record_loans_observe_the_current_owned_frontier() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut loan_then_move = record_call_unit(access, false);
        consume_result(&mut loan_then_move, false, false);
        assert_eq!(
            validate_psi_optimization_unit(&loan_then_move),
            Ok(()),
            "loan before whole move {access:?}"
        );

        let mut move_then_loan = record_call_unit(access, false);
        consume_result(&mut move_then_loan, false, true);
        assert!(
            matches!(validate_psi_optimization_unit(&move_then_loan), Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { place, .. }) if place == id(962, PlaceId::new)),
            "loan after whole move {access:?}"
        );

        let mut partial_then_whole_loan = record_call_unit(access, false);
        consume_result(&mut partial_then_whole_loan, true, true);
        assert!(
            matches!(validate_psi_optimization_unit(&partial_then_whole_loan), Err(OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved { place, .. }) if place == id(962, PlaceId::new)),
            "whole loan after partial move {access:?}"
        );

        let mut partial_then_projected_loan = record_call_unit(access, true);
        consume_result(&mut partial_then_projected_loan, true, true);
        assert!(
            matches!(validate_psi_optimization_unit(&partial_then_projected_loan), Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { place, .. }) if place == id(962, PlaceId::new)),
            "overlapping loan after partial move {access:?}"
        );
        let AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } = &mut partial_then_projected_loan.functions[0].blocks[0].nodes[3].operation
        else {
            panic!("projected receiver call");
        };
        structural_arguments[0].path =
            vec![terminal_psi::StructuralPathSegment::Field("second".into())];
        refresh_function_derivatives(&mut partial_then_projected_loan, 0);
        assert_eq!(
            validate_psi_optimization_unit(&partial_then_projected_loan),
            Ok(()),
            "disjoint loan after partial move {access:?}"
        );
    }
}

#[test]
fn owned_parameter_shared_loans_observe_the_current_frontier() {
    for (partial, before_loan) in [(false, false), (true, true), (false, true)] {
        let mut candidate = record_call_unit(StructuralAccess::SharedBorrow, false);
        consume_result(&mut candidate, partial, before_loan);
        let caller = &mut candidate.functions[0];
        let place = id(962, PlaceId::new);
        caller.blocks[0].nodes.remove(1);
        caller
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position: 1,
                is_self: false,
                structural_type: id(902, StructuralTypeId::new),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        caller
            .structural_places
            .iter_mut()
            .find(|declaration| declaration.id == place)
            .expect("existing owner home")
            .kind = StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        };
        refresh_function_derivatives(&mut candidate, 0);
        let outcome = validate_psi_optimization_unit(&candidate);
        if !before_loan {
            assert_eq!(outcome, Ok(()), "shared loan before parameter transfer");
        } else if partial {
            assert!(
                matches!(outcome, Err(OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved { place: actual, .. }) if actual == place),
                "whole loan after parameter partial move: {outcome:?}"
            );
        } else {
            assert!(
                matches!(outcome, Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { place: actual, .. }) if actual == place),
                "shared loan after parameter transfer: {outcome:?}"
            );
        }
    }
}
