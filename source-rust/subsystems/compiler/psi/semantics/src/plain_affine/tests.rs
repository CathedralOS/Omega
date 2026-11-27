use super::*;
use semantic_vocabulary::{StructuralFieldId, StructuralPlaceKind};

fn id<T: arena::NumericId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn transferring(
    sources: &[StructuralTypeId],
    destinations: &[StructuralTypeId],
    arguments: Vec<StructuralArgument>,
) -> TerminalMachine {
    use terminal_psi::{
        Block, BoundaryParameterKind, MachineContract, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, Terminator,
    };
    let parameter = |place, position, structural_type| StructuralParameterDeclaration {
        place,
        position,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    };
    let mut contract = MachineContract::empty(id(1));
    contract.structural_parameters = sources
        .iter()
        .enumerate()
        .map(|(position, ty)| parameter(id(position as u64 + 1), position as u32, *ty))
        .collect();
    contract.parameter_order = vec![BoundaryParameterKind::Structural; sources.len()];
    let entry = Block::plain(
        id(1),
        Vec::new(),
        Terminator::Jump {
            edge: id(1),
            target: id(2),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: arguments,
            cleanup_actions: Vec::new(),
        },
    );
    let mut target = Block::plain(
        id(2),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id(2),
            cleanup_actions: Vec::new(),
        },
    );
    target.structural_parameters = destinations
        .iter()
        .enumerate()
        .map(|(position, ty)| {
            parameter(
                id((sources.len() + position) as u64 + 1),
                position as u32,
                *ty,
            )
        })
        .collect();
    let structural_places = contract
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        })
        .chain(target.structural_parameters.iter().map(|parameter| {
            StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::BlockParameter {
                    block: id(2),
                    position: parameter.position,
                },
            }
        }))
        .collect();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: id(1),
        attachment: None,
        ranked_scc: None,
        structural_places,
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id(1),
        blocks: vec![entry, target],
        contract,
    }
}

#[test]
fn a_successor_can_reborrow_a_stored_view_into_a_canonical_header() {
    use StructuralAccess::{MutableBorrow, SharedBorrow};
    let base = reconvergence_types()[2].clone();
    let view = |raw, element, access| StructuralTypeDeclaration {
        id: id(raw),
        shape: StructuralTypeShape::ElementView { element: id(element), access },
        ..base.clone()
    };
    let accepts = |source: StructuralTypeDeclaration, target: StructuralTypeDeclaration| {
        let argument = StructuralArgument::whole(id(1), MutableBorrow);
        let mut machine = transferring(&[source.id], &[target.id], vec![argument.clone()]);
        machine.blocks[1].structural_parameters[0].access = MutableBorrow;
        let root = Some((source.id, source.multiplicity, MutableBorrow, None));
        let other_element = StructuralTypeDeclaration { id: id(4), ..base.clone() };
        let types = [source, target, base.clone(), other_element];
        validate_successor(&machine, &types, &[root], id(2), &[argument]).is_ok()
    };
    assert!(accepts(view(1, 3, Some(MutableBorrow)), view(2, 3, None)));
    assert!(!accepts(view(1, 3, Some(SharedBorrow)), view(2, 3, None)));
    assert!(!accepts(view(1, 4, Some(MutableBorrow)), view(2, 3, None)));
    // Borrowing the descriptor's storage needs the exact stored type.
    assert!(!accepts(view(1, 3, None), view(2, 3, Some(MutableBorrow))));
}

fn field_argument(field: u32) -> StructuralArgument {
    StructuralArgument {
        place: id(1),
        path: vec![StructuralPathSegment::Field(
            terminal_psi::TerminalIdentityIndex(field),
        )],
        access: StructuralAccess::Owned,
    }
}

fn transfer_only_types(linear: bool) -> [StructuralTypeDeclaration; 3] {
    let mut types = reconvergence_types();
    for declaration in &mut types {
        declaration.owns_linear_claim = linear && declaration.id == id(3);
        declaration.multiplicity = if linear {
            terminal_psi::StructuralMultiplicity::Linear
        } else {
            terminal_psi::StructuralMultiplicity::Affine
        };
        declaration.no_code_affine_cleanup = false;
        declaration.descriptor_cleanup = false;
    }
    types
}

fn returning_relay(linear: bool) -> TerminalMachine {
    use terminal_psi::{
        EntryClaim, StructuralMultiplicity, StructuralPlaceDeclaration,
        StructuralResultClaimBinding, StructuralResultDeclaration, TerminalMachineResult,
        Terminator,
    };
    let mut machine = transferring(
        &[id(3)],
        &[id(3)],
        vec![StructuralArgument::whole(id(1), StructuralAccess::Owned)],
    );
    let multiplicity = if linear {
        StructuralMultiplicity::Linear
    } else {
        StructuralMultiplicity::Affine
    };
    machine.contract.structural_parameters[0].multiplicity = multiplicity;
    machine.blocks[1].structural_parameters[0].multiplicity = multiplicity;
    machine.contract.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: id(3),
        structural_type: id(3),
        multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3),
        kind: StructuralPlaceKind::Result,
    });
    let returned_claims = if linear {
        machine.contract.entry_claims.push(EntryClaim {
            claim: id(1),
            input: id(1),
            path: Vec::new(),
        });
        machine.block_entry_claims.push(EntryClaim {
            claim: id(2),
            input: id(2),
            path: Vec::new(),
        });
        vec![StructuralResultClaimBinding {
            claim: id(2),
            path: Vec::new(),
        }]
    } else {
        Vec::new()
    };
    machine.blocks[1].terminator = Terminator::ReturnStructural {
        edge: id(2),
        source: id(2),
        returned_claims,
        cleanup_actions: Vec::new(),
    };
    machine
}

#[test]
fn declared_multiplicity_does_not_infer_an_intrinsic_root_claim() {
    use terminal_psi::StructuralMultiplicity as M;
    let mut declaration = reconvergence_types()[2].clone();
    for multiplicity in [M::Unrestricted, M::Affine, M::Linear] {
        declaration.multiplicity = multiplicity;
        declaration.owns_linear_claim = false;
        assert_eq!(
            declared_multiplicity(&declaration, StructuralAccess::Owned),
            multiplicity
        );
        assert_eq!(declaration.is_copyable(), multiplicity == M::Unrestricted);
        assert_eq!(
            declared_multiplicity(&declaration, StructuralAccess::SharedBorrow),
            M::Unrestricted
        );
    }
    declaration.owns_linear_claim = true;
    assert_eq!(
        declared_multiplicity(&declaration, StructuralAccess::Owned),
        M::Linear
    );
}

#[test]
fn affine_parents_retain_exact_linear_siblings_and_active_case_claims() {
    let mut types = transfer_only_types(false);
    types[2].multiplicity = terminal_psi::StructuralMultiplicity::Linear;
    types[2].owns_linear_claim = true;
    let claim_types = crate::owned_structure::ClaimTypes::new(&types).unwrap();
    let siblings = [field_argument(3).path, field_argument(4).path];
    crate::owned_structure::validate_intrinsic_claim_roots(&types).unwrap();
    crate::owned_structure::validate_complete_claim_frontier(
        &claim_types,
        id(2),
        &[],
        siblings.iter().map(Vec::as_slice),
        &|_, _| false,
    )
    .unwrap();
    let bindings = siblings
        .iter()
        .enumerate()
        .map(|(index, path)| terminal_psi::StructuralResultClaimBinding {
            claim: id(index as u64 + 1),
            path: path.clone(),
        })
        .collect::<Vec<_>>();
    crate::validate_result_claim_catalog(types.iter(), id(2), &bindings, [bindings.as_slice()])
        .unwrap();
    crate::validate_result_claim_catalog(
        types.iter().rev(),
        id(2),
        &bindings,
        [bindings.as_slice()],
    )
    .unwrap();
    assert_eq!(
        crate::validate_result_claim_catalog(
            types.iter().chain(std::iter::once(&types[0])),
            id(2),
            &bindings,
            [bindings.as_slice()],
        ),
        Err(crate::ResultClaimCatalogError::NonStaticOwnedPath)
    );
    let collapsed = [terminal_psi::StructuralResultClaimBinding {
        claim: id(9),
        path: Vec::new(),
    }];
    assert_eq!(
        crate::validate_result_claim_catalog(
            types.iter(),
            id(2),
            &collapsed,
            [collapsed.as_slice()]
        ),
        Err(crate::ResultClaimCatalogError::TypedFrontierMismatch)
    );
    assert!(
        crate::owned_structure::validate_complete_claim_frontier(
            &claim_types,
            id(2),
            &[],
            [siblings[0].as_slice()],
            &|_, _| false
        )
        .is_err()
    );
    let fields = types[1].shape.direct_fields().unwrap().to_vec();
    types[1].shape = StructuralTypeShape::Sum {
        cases: vec![
            terminal_psi::StructuralCaseDeclaration {
                declaration_identity: Default::default(),
                id: id(1),
                identity: terminal_psi::TerminalIdentityIndex(8),
                fields: vec![fields[0].clone()],
            },
            terminal_psi::StructuralCaseDeclaration {
                declaration_identity: Default::default(),
                id: id(2),
                identity: terminal_psi::TerminalIdentityIndex(9),
                fields: Vec::new(),
            },
        ],
    };
    let claim_types = crate::owned_structure::ClaimTypes::new(&types).unwrap();
    let active = vec![StructuralPathSegment::Case(id(1)), siblings[0][0].clone()];
    let guarded = [terminal_psi::StructuralResultClaimBinding {
        claim: id(1),
        path: active.clone(),
    }];
    crate::validate_result_claim_catalog(
        types.iter(),
        id(2),
        &guarded,
        [guarded.as_slice(), &[]],
    )
    .unwrap();
    assert_eq!(
        crate::validate_result_claim_catalog(types.iter(), id(2), &[], [&[][..]]),
        Err(crate::ResultClaimCatalogError::TypedFrontierMismatch)
    );
    crate::owned_structure::validate_complete_claim_frontier(
        &claim_types,
        id(2),
        &[],
        [active.as_slice()],
        &|_, case| case == id(1),
    )
    .unwrap();
    crate::owned_structure::validate_complete_claim_frontier(
        &claim_types,
        id(2),
        &[],
        std::iter::empty(),
        &|_, case| case == id(2),
    )
    .unwrap();
    assert!(
        crate::owned_structure::validate_complete_claim_frontier(
            &claim_types,
            id(2),
            &[],
            [active.as_slice()],
            &|_, case| case == id(2)
        )
        .is_err()
    );
    types[1].multiplicity = terminal_psi::StructuralMultiplicity::Linear;
    types[1].owns_linear_claim = true;
    assert!(
        crate::owned_structure::validate_intrinsic_claim_roots(&types).is_err(),
        "an inactive payload cannot justify collapsing possible child claims into a root"
    );
}

#[test]
fn complete_linear_and_nondroppable_values_can_relay_and_return_without_cleanup() {
    for linear in [false, true] {
        let types = transfer_only_types(linear);
        let machine = returning_relay(linear);
        let planned = verify(&machine, &types).unwrap();
        assert!(planned.unavailable.is_empty());
        assert!(planned.cleanup.is_empty());
        if linear {
            // The region planner neither issues nor settles these claims.
            // The ordinary claim owner checks their edge/return transport.
            assert_eq!(machine.entry_claims[0].claim, id(1));
            assert_eq!(machine.block_entry_claims[0].claim, id(2));
            let terminal_psi::Terminator::ReturnStructural {
                returned_claims, ..
            } = &machine.blocks[1].terminator
            else {
                unreachable!()
            };
            assert_eq!(returned_claims[0].claim, id(2));
        }
    }
}

#[test]
fn a_complete_linear_call_transfers_custody_once_without_implicit_disposal() {
    use terminal_psi::{ClaimTransfer, Operation, OperationKind, OperationResult, Terminator};
    let types = transfer_only_types(true);
    let mut machine = returning_relay(true);
    machine.contract.result = terminal_psi::TerminalMachineResult::Unit;
    assert!(matches!(
        machine.structural_places.pop().unwrap().kind,
        StructuralPlaceKind::Result
    ));
    let call = Operation::unsuspended(
        id(1),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: id(2),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument::whole(
                id(2),
                StructuralAccess::Owned,
            )],
            claim_transfers: vec![ClaimTransfer {
                claim: id(2),
                argument_index: 0,
                path: Vec::new(),
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    machine.blocks[1].operations.push(call.clone());
    machine.blocks[1].terminator = Terminator::ReturnUnit {
        edge: id(2),
        cleanup_actions: Vec::new(),
    };
    let planned = verify(&machine, &types).unwrap();
    assert!(planned.unavailable.is_empty());
    assert!(planned.cleanup.is_empty());
    let mut duplicate = call;
    duplicate.id = id(2);
    machine.blocks[1].operations.push(duplicate);
    assert!(!plan(&machine, &types).unwrap().unavailable.is_empty());
}

#[test]
fn an_unread_linear_value_survives_an_internal_edge_to_a_crash() {
    use terminal_psi::{EntryClaim, StructuralMultiplicity, Terminator};
    let types = transfer_only_types(true);
    let mut machine = transferring(&[id(3)], &[], Vec::new());
    machine.contract.structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
    machine.contract.entry_claims.push(EntryClaim {
        claim: id(1),
        input: id(1),
        path: Vec::new(),
    });
    machine.blocks[1].terminator = Terminator::Crash {
        edge: id(2),
        cause: terminal_psi::CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    for mode in [None, Some(claims::Mode::Verify)] {
        let planned = analyze(&machine, &types, true, None, None, mode).unwrap();
        assert!(planned.cleanup.is_empty(), "abandonment is not disposal");
        assert!(planned.unavailable.is_empty());
        assert_eq!(planned.claims.is_some(), mode.is_some());
    }

    // Only the target's outcome changes. A normal return must account
    // for the same declared value and its still-live entry claim.
    machine.blocks[1].terminator = Terminator::ReturnUnit {
        edge: id(2),
        cleanup_actions: Vec::new(),
    };
    for mode in [None, Some(claims::Mode::Verify)] {
        let error = analyze(&machine, &types, true, None, None, mode).unwrap_err();
        assert_eq!(error.site(), Some(Site::Edge(id(2))));
        assert!(matches!(
            error,
            Error::Unconsumed {
                at_join: false,
                ..
            }
        ));
    }
}

#[test]
fn a_returned_value_settles_its_claim_only_when_the_return_transfers_it() {
    let machine = returning_relay(true);
    let types = transfer_only_types(true);
    let storage = verify(&machine, &types).unwrap();
    assert!(storage.cleanup.is_empty() && storage.unavailable.is_empty());

    // Storage can return the value. Follow its claim to the returning block
    // and check what the return owes before and after it hands that claim back.
    let (mut flow, mut claims) = claims::Flow::new(&machine, &types, claims::Mode::Verify).unwrap();
    let terminal_psi::Terminator::Jump {
        target,
        structural_arguments,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("the relay passes its owned argument to the returning block")
    };
    flow.successor(&mut claims, &machine, *target, structural_arguments)
        .unwrap();
    assert!(matches!(
        claims.require_empty_return(),
        Err(Error::Unsupported {
            detail: "a normal return leaves an untransferred logical claim",
            ..
        })
    ));
    flow.terminal(&mut claims, &machine.blocks[1].terminator, true)
        .unwrap();
    claims.require_empty_return().unwrap();
}

#[test]
fn a_normal_return_cannot_forget_a_transfer_only_value() {
    for linear in [false, true] {
        let types = transfer_only_types(linear);
        let mut machine = returning_relay(linear);
        machine.contract.result = terminal_psi::TerminalMachineResult::Unit;
        assert!(matches!(
            machine.structural_places.pop().unwrap().kind,
            StructuralPlaceKind::Result
        ));
        machine.blocks[1].terminator = terminal_psi::Terminator::ReturnUnit {
            edge: id(2),
            cleanup_actions: Vec::new(),
        };
        // A linear value is the program's error; an affine value with no
        // disposition is a limit of this check.
        let planned = plan(&machine, &types);
        if linear {
            assert!(
                matches!(planned, Err(Error::Unconsumed { at_join: false, .. })),
                "{planned:?}"
            );
        } else {
            assert!(matches!(
                planned,
                Err(Error::Unsupported {
                    detail: "normal edge cannot implicitly dispose storage without an authorized disposition",
                    ..
                })
            ));
        }
    }
}

#[test]
fn partial_moves_cannot_retire_an_intrinsic_linear_claim_even_after_all_fields_move() {
    use terminal_psi::{EntryClaim, StructuralMultiplicity};
    let mut types = transfer_only_types(true);
    types[1].owns_linear_claim = true;
    types[2].owns_linear_claim = false;
    types[2].multiplicity = StructuralMultiplicity::Affine;
    for fields in [
        vec![field_argument(3)],
        vec![field_argument(3), field_argument(4)],
    ] {
        let destinations = vec![id(3); fields.len()];
        let mut machine = transferring(&[id(2)], &destinations, fields);
        machine.contract.structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
        machine.contract.entry_claims.push(EntryClaim {
            claim: id(1),
            input: id(1),
            path: Vec::new(),
        });
        for parameter in &mut machine.blocks[1].structural_parameters {
            parameter.multiplicity = StructuralMultiplicity::Affine;
        }
        assert!(
            matches!(
                plan(&machine, &types),
                Err(Error::Unsupported {
                    detail: "normal edge cannot implicitly dispose storage without an authorized disposition",
                    ..
                })
            ),
            "moving fields cannot settle the independent root obligation"
        );
    }
}

#[test]
fn a_join_cannot_discard_a_live_transfer_only_residual() {
    let types = transfer_only_types(true);
    let mut analysis = owned_analysis(&types);
    analysis.dispositions.fill(Some(false));
    let held = Region::new(true);
    let mut joined = held.clone();
    joined.set(&[Segment::Field(id(1))], false);
    assert!(
        analysis
            .equalize(id(1), &held, &held, 0)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        analysis.equalize(id(1), &held, &joined, 1),
        Err(Error::Unconsumed { at_join: true, .. })
    ));
    assert!(analysis.dispose(id(1), &joined, 1).is_err());
}

#[test]
fn only_an_empty_nondroppable_home_can_retire_without_disposing_a_value() {
    let types = transfer_only_types(false);
    let mut analysis = owned_analysis(&types);
    analysis.dispositions.fill(Some(false));
    let mut region = Region::new(true);
    assert!(analysis.dispose(id(1), &region, 0).is_err());
    region.set(&[Segment::Field(id(1))], false);
    assert!(analysis.dispose(id(1), &region, 0).is_err());
    region.set(&[Segment::Field(id(2))], false);
    assert_eq!(
        analysis.dispose(id(1), &region, 0).unwrap(),
        vec![TerminalAffineCleanupAction::RetireHome(id(1))]
    );
    analysis.roots[0].as_mut().unwrap().multiplicity =
        terminal_psi::StructuralMultiplicity::Linear;
    // The value has moved completely. Ending its empty home is not an
    // implicit discard of a linear value or a discharge of a claim.
    assert_eq!(
        analysis.dispose(id(1), &region, 0).unwrap(),
        vec![TerminalAffineCleanupAction::RetireHome(id(1))]
    );
    let mut intrinsic_types = transfer_only_types(false);
    intrinsic_types[0].multiplicity = terminal_psi::StructuralMultiplicity::Linear;
    intrinsic_types[0].owns_linear_claim = true;
    crate::owned_structure::validate_intrinsic_claim_roots(&intrinsic_types)
        .expect("the outer value owns a claim independently of its affine fields");
    let mut intrinsic = owned_analysis(&intrinsic_types);
    intrinsic.roots[0].as_mut().unwrap().multiplicity =
        terminal_psi::StructuralMultiplicity::Linear;
    intrinsic.dispositions.fill(Some(false));
    assert!(intrinsic.dispose(id(1), &region, 0).is_err());
}

#[test]
fn successor_values_are_captured_before_source_cleanup_and_destination_binding() {
    use terminal_psi::Terminator;
    let types = reconvergence_types();
    let mut machine = transferring(
        &[id(2)],
        &[id(3), id(3)],
        vec![field_argument(3), field_argument(4)],
    );
    let planned = plan(&machine, &types).unwrap();
    assert!(planned.unavailable.is_empty());
    assert_eq!(
        planned.cleanup,
        vec![
            (id(1), vec![TerminalAffineCleanupAction::RetireHome(id(1))]),
            (
                id(2),
                vec![
                    TerminalAffineCleanupAction::DiscardRoot(id(3)),
                    TerminalAffineCleanupAction::DiscardRoot(id(2))
                ]
            ),
        ]
    );
    let Terminator::Jump {
        cleanup_actions, ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    *cleanup_actions = planned.cleanup[0].1.clone();
    let Terminator::ReturnUnit {
        cleanup_actions, ..
    } = &mut machine.blocks[1].terminator
    else {
        unreachable!()
    };
    *cleanup_actions = planned.cleanup[1].1.clone();
    verify(&machine, &types).unwrap();
    let Terminator::Jump {
        cleanup_actions, ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    assert!(
        verify(&machine, &types).is_err(),
        "the source's empty home still needs its own retirement"
    );

    let duplicate = transferring(
        &[id(2)],
        &[id(3), id(3)],
        vec![field_argument(3), field_argument(3)],
    );
    assert!(
        !plan(&duplicate, &types).unwrap().unavailable.is_empty(),
        "one projected owner cannot supply both parameters"
    );
    let short = transferring(&[id(2)], &[id(3), id(3)], vec![field_argument(3)]);
    assert!(plan(&short, &types).is_err());
    let mut borrowed = field_argument(3);
    borrowed.access = StructuralAccess::SharedBorrow;
    assert!(plan(&transferring(&[id(2)], &[id(3)], vec![borrowed]), &types).is_err());
    let mut borrowed_source = transferring(&[id(2)], &[id(3)], vec![field_argument(3)]);
    borrowed_source.contract.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    borrowed_source.contract.structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Unrestricted;
    assert!(matches!(
        plan(&borrowed_source, &types),
        Err(Error::Unsupported {
            detail: "owned extraction from borrowed storage",
            ..
        })
    ));
    let mut foreign = types.to_vec();
    let mut same_layout = foreign[2].clone();
    same_layout.id = id(4);
    same_layout.identity = terminal_psi::TerminalIdentityIndex(4);
    foreign.push(same_layout);
    assert!(
        plan(
            &transferring(&[id(2)], &[id(4)], vec![field_argument(3)]),
            &foreign
        )
        .is_err(),
        "matching bytes do not establish nominal type identity"
    );
}

fn borrowed_backedge(access: StructuralAccess) -> TerminalMachine {
    let mut machine = transferring(
        &[id(3)],
        &[id(3)],
        vec![StructuralArgument::whole(id(1), access)],
    );
    for parameter in machine
        .contract
        .structural_parameters
        .iter_mut()
        .chain(&mut machine.blocks[1].structural_parameters)
    {
        parameter.access = access;
        parameter.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
    }
    machine.blocks[1].terminator = terminal_psi::Terminator::Jump {
        edge: id(2),
        target: id(2),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument::whole(id(2), access)],
        cleanup_actions: Vec::new(),
    };
    machine
}

#[test]
fn bodyless_requirements_keep_their_exact_parameter_claims() {
    let types = transfer_only_types(true);
    let mut machine = transferring(
        &[id(3)],
        &[id(3)],
        vec![StructuralArgument::whole(id(1), StructuralAccess::Owned)],
    );
    machine.blocks.clear();
    machine.block_entry_claims.clear();
    machine.structural_places.truncate(1);
    machine.contract.structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Linear;
    let plan = analyze(
        &machine,
        &types,
        false,
        None,
        None,
        Some(claims::Mode::Publish),
    )
    .unwrap()
    .claims
    .expect("a bodyless signature still has a claim plan");
    assert_eq!(plan.entry_claims.len(), 1);
    plan.install(&mut machine).unwrap();
    let verified = analyze(
        &machine,
        &types,
        true,
        None,
        None,
        Some(claims::Mode::Verify),
    )
    .unwrap();
    assert!(verified.claims.is_some());
    assert!(machine.blocks.is_empty());

    machine.contract.entry_claims.clear();
    assert!(
        analyze(
            &machine,
            &types,
            true,
            None,
            None,
            Some(claims::Mode::Verify)
        )
        .is_err()
    );
}

#[test]
fn a_borrowed_successor_preserves_the_sources_exact_logical_claim() {
    let types = transfer_only_types(true);
    let argument = StructuralArgument::whole(id(1), StructuralAccess::Owned);
    let mut machine = transferring(&[id(3)], &[id(3)], vec![argument.clone()]);
    machine.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    machine.blocks[1].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Unrestricted;
    let (mut flow, mut state) =
        claims::Flow::new(&machine, &types, claims::Mode::Publish).unwrap();
    let original = state.clone();
    flow.successor(&mut state, &machine, id(2), std::slice::from_ref(&argument))
        .unwrap();
    assert_eq!(
        state, original,
        "a loan does not transfer its owner's claim"
    );
    assert!(flow.plan.block_entry_claims.is_empty());

    machine.blocks[1].structural_parameters[0].access = StructuralAccess::Owned;
    let (mut flow, mut state) =
        claims::Flow::new(&machine, &types, claims::Mode::Publish).unwrap();
    flow.successor(&mut state, &machine, id(2), &[argument])
        .unwrap();
    assert_eq!(state.live().count(), 1);
    let destination = state.live().next().unwrap();
    assert_eq!(destination.place, id(2));
    assert_ne!(
        destination.identity,
        original.live().next().unwrap().identity
    );
}

#[test]
fn a_borrowed_successor_keeps_its_owned_backing_until_the_last_use() {
    use terminal_psi::{
        Block, Operation, OperationKind, OperationResult, StructuralMultiplicity, Terminator,
    };
    let types = reconvergence_types();
    let mut machine = transferring(
        &[id(3)],
        &[id(3)],
        vec![StructuralArgument::whole(
            id(1),
            StructuralAccess::SharedBorrow,
        )],
    );
    let target = &mut machine.blocks[1];
    target.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    target.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    let observation = Operation::unsuspended(
        id(1),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: id(2),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument::whole(
                id(2),
                StructuralAccess::SharedBorrow,
            )],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    let finish = std::mem::replace(
        &mut target.terminator,
        Terminator::Jump {
            edge: id(3),
            target: id(3),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        },
    );
    let mut later = Block::plain(id(3), Vec::new(), finish);
    later.operations.push(observation);
    machine.blocks.push(later);
    let checked = plan(&machine, &types).unwrap();
    assert!(
        !checked.cleanup.iter().any(|(edge, actions)| *edge == id(3)
            && actions.iter().any(|action| action.place() == id(1))),
        "the borrowed name keeps its backing in the later observation block"
    );
    assert!(checked.unavailable.is_empty());
    assert!(
        !checked.cleanup.iter().any(|(edge, actions)| *edge == id(1)
            && actions.iter().any(|action| action.place() == id(1))),
        "the source home must survive capture and successor use"
    );
    assert!(
        checked.cleanup.iter().any(|(edge, actions)| *edge == id(2)
            && actions.iter().any(|action| action.place() == id(1))),
        "the original owner is disposed after its last borrowed use"
    );
    let terminal_psi::Terminator::Jump {
        cleanup_actions, ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(id(1)));
    assert!(
        verify(&machine, &types).is_err(),
        "cleanup cannot destroy a still-borrowed source home"
    );
}

#[test]
fn a_dead_borrowed_header_can_rejoin_a_path_that_never_entered_it() {
    use terminal_psi::{
        Block, BoundaryParameterKind, Operation, OperationKind, OperationResult, SuccessorEdge,
        Terminator, ValueDeclaration,
    };
    let types = reconvergence_types();
    let mut machine = borrowed_backedge(StructuralAccess::SharedBorrow);
    machine.contract.parameters.push(ValueDeclaration {
        id: id(1),
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
        qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
    });
    machine
        .contract
        .parameter_order
        .insert(0, BoundaryParameterKind::Scalar);
    let edge = |raw, target, structural_arguments| SuccessorEdge {
        edge: id(raw),
        target: id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments,
        cleanup_actions: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(1),
        when_true: edge(
            1,
            2,
            vec![StructuralArgument::whole(
                id(1),
                StructuralAccess::SharedBorrow,
            )],
        ),
        when_false: edge(2, 3, Vec::new()),
    };
    let use_header = Operation::unsuspended(
        id(1),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: id(2),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument::whole(
                id(2),
                StructuralAccess::SharedBorrow,
            )],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    machine.blocks[1].operations.push(use_header.clone());
    machine.blocks[1].terminator = Terminator::Jump {
        edge: id(3),
        target: id(3),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    machine.blocks.push(Block::plain(
        id(3),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id(4),
            cleanup_actions: Vec::new(),
        },
    ));
    let planned = plan(&machine, &types)
        .expect("the dead borrowed name is not an ownership obligation at the join");
    assert!(planned.unavailable.is_empty());
    assert!(
        planned.cleanup.is_empty(),
        "retiring a borrowed name must not discard its backing"
    );
    let mut use_header = use_header;
    use_header.id = id(2);
    machine.blocks[2].operations.push(use_header);
    assert!(
        !plan(&machine, &types)
            .expect("the joined frontier remains available for checking")
            .unavailable
            .is_empty(),
        "a path that never received the header cannot use that borrow after the join"
    );
}

#[test]
fn borrowed_block_storage_survives_a_backedge_without_becoming_owned() {
    let types = reconvergence_types();
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut machine = borrowed_backedge(access);
        let checked = verify(&machine, &types).unwrap();
        assert!(checked.unavailable.is_empty());
        assert!(
            checked.cleanup.is_empty(),
            "borrowed storage is never an owned cleanup obligation"
        );
        let terminal_psi::Terminator::Jump {
            cleanup_actions, ..
        } = &mut machine.blocks[1].terminator
        else {
            unreachable!()
        };
        cleanup_actions.push(TerminalAffineCleanupAction::RetireHome(id(2)));
        assert!(
            verify(&machine, &types).is_err(),
            "a backedge cannot dispose its borrowed referent"
        );
    }
}

#[test]
fn borrowed_successors_cannot_strengthen_actual_access_even_for_copyable_storage() {
    for copyable in [false, true] {
        let mut types = reconvergence_types();
        // Use one empty record so copy permission has no nested premise.
        for declaration in &mut types {
            declaration.multiplicity = if copyable {
                terminal_psi::StructuralMultiplicity::Unrestricted
            } else {
                terminal_psi::StructuralMultiplicity::Affine
            };
        }
        let original = borrowed_backedge(StructuralAccess::SharedBorrow);
        assert!(verify(&original, &types).unwrap().unavailable.is_empty());
        for forged in [StructuralAccess::MutableBorrow, StructuralAccess::Owned] {
            let mut machine = original.clone();
            let terminal_psi::Terminator::Jump {
                structural_arguments,
                ..
            } = &mut machine.blocks[1].terminator
            else {
                unreachable!()
            };
            structural_arguments[0].access = forged;
            assert!(
                matches!(verify(&machine, &types), Err(Error::Malformed { .. })),
                "the destination header cannot grant {forged:?} to a shared source"
            );
        }
    }
}

#[test]
fn borrowed_address_names_and_their_contained_descriptors_have_independent_liveness() {
    use terminal_psi::{Operation, OperationKind, OperationResult, Terminator};
    let mut types = reconvergence_types();
    let mut machine = borrowed_backedge(StructuralAccess::MutableBorrow);
    machine.blocks[1].operations.push(Operation::unsuspended(
        id(1),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: id(2),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument::whole(
                id(2),
                StructuralAccess::MutableBorrow,
            )],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    ));
    let forwarding = machine.blocks[1].terminator.clone();
    machine.blocks[1].terminator = Terminator::ReturnUnit {
        edge: id(2),
        cleanup_actions: Vec::new(),
    };
    let names = borrowed_address_liveness(&machine, &types).unwrap();
    assert!(names.is_address_header(id(2)));
    assert_eq!(names.is_live(id(2), (id(2), 0)), Some(true));
    assert_eq!(names.is_live(id(2), (id(2), 1)), Some(false));
    assert_eq!(names.is_live(id(2), (id(99), 0)), None);
    assert_eq!(names.is_live(id(2), (id(2), 2)), None);
    assert!(borrowed_address_liveness(&machine, &types[..2]).is_err());
    let mut foreign = machine.clone();
    foreign.blocks[1].structural_parameters[0].place = id(u64::MAX);
    assert!(borrowed_address_liveness(&foreign, &types).is_err());
    foreign.blocks[1].structural_parameters[0].place = id(1);
    assert!(borrowed_address_liveness(&foreign, &types).is_err());
    machine.blocks[1].terminator = forwarding;
    let names = borrowed_address_liveness(&machine, &types).unwrap();
    assert_eq!(names.is_live(id(2), (id(2), 1)), Some(true));
    types[2].shape = StructuralTypeShape::Reference {
        referent: id(2),
        access: StructuralAccess::SharedBorrow,
    };
    let cells = borrowed_address_liveness(&machine, &types).unwrap();
    assert!(!cells.is_address_header(id(2)));
    types[2].shape = types[0].shape.clone();
    types[1].shape = StructuralTypeShape::Reference {
        referent: id(1),
        access: StructuralAccess::SharedBorrow,
    };
    types[1].multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
    let contained = borrowed_address_liveness(&machine, &types).unwrap();
    assert!(contained.is_address_header(id(2)));
    let inventory = crate::descriptor_results::DescriptorSlotKind::BorrowedStorage
        .owned_inventory(&types, id(2), types[2].id)
        .unwrap();
    assert!(!inventory.slots.is_empty());
    assert!(inventory.slots.iter().all(|slot| !slot.path.is_empty()));
}

#[test]
fn a_byte_element_successor_keeps_its_actual_scalar_and_access() {
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
    use terminal_psi::{ByteSequenceCarrier, StructuralMultiplicity};

    let mut types = reconvergence_types();
    types[0].shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
        capacity: 2,
        access: Some(StructuralAccess::SharedBorrow),
    });
    for (declaration, bits) in types[1..].iter_mut().zip([8, 16]) {
        declaration.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        ));
        declaration.multiplicity = StructuralMultiplicity::Unrestricted;
    }
    let mut machine = borrowed_backedge(StructuralAccess::SharedBorrow);
    machine.contract.structural_parameters[0].structural_type = id(1);
    machine.blocks[1].structural_parameters[0].structural_type = id(2);
    let arguments = [StructuralArgument {
        place: id(1),
        path: vec![StructuralPathSegment::FixedIndex(0)],
        access: StructuralAccess::SharedBorrow,
    }];
    let contracts = [Some((
        id(1),
        StructuralMultiplicity::Unrestricted,
        StructuralAccess::SharedBorrow,
        None,
    ))];
    validate_successor(&machine, &types, &contracts, id(2), &arguments)
        .expect("the byte projection is the actual unqualified u8 scalar");
    machine.blocks[1].structural_parameters[0].structural_type = id(3);
    assert!(
        validate_successor(&machine, &types, &contracts, id(2), &arguments).is_err(),
        "a byte address cannot become a u16 address from the destination header"
    );
    machine.blocks[1].structural_parameters[0].structural_type = id(2);
    machine.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let mut exclusive = arguments;
    exclusive[0].access = StructuralAccess::MutableBorrow;
    assert!(
        validate_successor(&machine, &types, &contracts, id(2), &exclusive).is_err(),
        "a matching scalar type cannot strengthen the source view's permission"
    );
}

#[test]
fn a_successor_keeps_the_transferred_field_out_of_its_sources_disposal() {
    let types = reconvergence_types();
    let planned = plan(
        &transferring(&[id(2)], &[id(3)], vec![field_argument(3)]),
        &types,
    )
    .unwrap();
    assert!(planned.unavailable.is_empty());
    assert_eq!(
        planned.cleanup,
        vec![
            (
                id(1),
                vec![
                    TerminalAffineCleanupAction::DiscardResidual(
                        terminal_psi::StructuralAffineDiscard {
                            place: id(1),
                            path: field_argument(4).path,
                            structural_type: id(3),
                        }
                    ),
                    TerminalAffineCleanupAction::RetireHome(id(1))
                ]
            ),
            (id(2), vec![TerminalAffineCleanupAction::DiscardRoot(id(2))]),
        ]
    );
}

#[test]
fn a_backedge_captures_both_swapped_owners_before_rebinding_their_names() {
    use terminal_psi::{
        Block, BoundaryParameterKind, SuccessorEdge, Terminator, ValueDeclaration,
    };
    let types = reconvergence_types();
    let mut machine = transferring(
        &[id(3), id(3)],
        &[id(3), id(3)],
        vec![
            StructuralArgument::whole(id(1), StructuralAccess::Owned),
            StructuralArgument::whole(id(2), StructuralAccess::Owned),
        ],
    );
    machine.contract.parameters.push(ValueDeclaration {
        id: id(1),
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
        qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
    });
    machine
        .contract
        .parameter_order
        .insert(0, BoundaryParameterKind::Scalar);
    let edge = |raw, target, structural_arguments| SuccessorEdge {
        edge: id(raw),
        target: id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments,
        cleanup_actions: Vec::new(),
    };
    machine.blocks[1].terminator = Terminator::Conditional {
        condition: id(1),
        when_true: edge(
            2,
            2,
            vec![
                StructuralArgument::whole(id(4), StructuralAccess::Owned),
                StructuralArgument::whole(id(3), StructuralAccess::Owned),
            ],
        ),
        when_false: edge(3, 3, Vec::new()),
    };
    machine.blocks.push(Block::plain(
        id(3),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id(4),
            cleanup_actions: Vec::new(),
        },
    ));
    let planned = plan(&machine, &types).unwrap();
    assert!(planned.unavailable.is_empty());
    assert_eq!(
        planned.cleanup,
        vec![(
            id(3),
            vec![
                TerminalAffineCleanupAction::DiscardRoot(id(4)),
                TerminalAffineCleanupAction::DiscardRoot(id(3))
            ]
        )]
    );
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        unreachable!()
    };
    when_true.structural_arguments[1] = when_true.structural_arguments[0].clone();
    assert!(!plan(&machine, &types).unwrap().unavailable.is_empty());
}

fn field(raw: u64) -> Segment {
    Segment::Field(StructuralFieldId::new(raw).unwrap())
}

#[test]
fn availability_is_owned_by_the_region_not_its_scalar_leaves() {
    let mut value = Region::new(true);
    assert!(value.available(&[]));
    value.set(&[], false);
    assert!(!value.available(&[]), "a moved empty value is still moved");
}

#[test]
fn partial_moves_preserve_siblings_and_exact_replacement_restores_the_parent() {
    let mut value = Region::new(true);
    value.set(&[field(1), field(2)], false);
    assert!(!value.available(&[]));
    assert!(!value.available(&[field(1)]));
    assert!(value.available(&[field(1), field(3)]));
    assert!(value.available(&[field(4)]));
    let mut vacancies = Vec::new();
    value.vacancies(&mut Vec::new(), &mut vacancies);
    assert_eq!(vacancies, [vec![field(1), field(2)]]);
    value.set(&[field(1), field(2)], true);
    assert_eq!(value, Region::new(true));
}

#[test]
fn a_join_owns_only_what_every_arrival_still_owns() {
    let mut moved_a = Region::new(true);
    moved_a.set(&[field(1)], false);
    let mut moved_b = Region::new(true);
    moved_b.set(&[field(2)], false);
    let joined = moved_a.intersection(&moved_b);
    assert!(!joined.available(&[field(1)]));
    assert!(!joined.available(&[field(2)]));
    assert!(joined.available(&[field(3)]));
    assert_eq!(joined, moved_b.intersection(&moved_a));
    assert_eq!(Region::new(true).intersection(&moved_a), moved_a);
    let mut vacancies = Vec::new();
    joined.vacancies(&mut Vec::new(), &mut vacancies);
    assert_eq!(vacancies, [vec![field(1)], vec![field(2)]]);
}

#[test]
fn a_component_partition_keeps_fields_live_without_recreating_the_wrapper() {
    let field = |raw| Segment::Field(id(raw));
    let mut region = Region::new(true);
    region.partition(&[]);
    assert!(!region.available(&[]));
    assert!(region.available(&[field(1)]));
    region.set(&[field(1)], false);
    assert!(!region.available(&[field(1)]));
    assert!(region.available(&[field(2)]));
    let mut vacancies = Vec::new();
    region.vacancies(&mut Vec::new(), &mut vacancies);
    assert_eq!(vacancies, [vec![field(1)]]);
    region.set(&[field(1)], true);
    assert!(!region.available(&[]));
    assert!(region.available(&[field(1)]));
    assert_eq!(region.mode, Mode::Components);
    assert_eq!(region.partition_count, 1);
    assert!(!region.same_partitions(&Region::new(true)));
    let active = vec![id(1)];
    assert!(
        State {
            regions: vec![Some(region)],
            active: active.clone(),
            claims: Default::default(),
        }
        .meet(&State {
            regions: vec![Some(Region::new(true))],
            active,
            claims: Default::default(),
        })
        .is_err()
    );
}

#[test]
fn ended_nominal_wrapper_disposes_only_remaining_components_in_reverse_order() {
    let mut types = reconvergence_types();
    types[0].nominal_cleanup = Some(terminal_psi::NominalCleanupSelection {
        declaration: terminal_psi::NominalDeclarationIdentity::new(types[0].identity),
        cleanup_machine: id(9),
    });
    types[0].no_code_affine_cleanup = false;
    types[0].descriptor_cleanup = false;
    let mut analysis = owned_analysis(&types);
    let mut region = Region::new(true);
    region.partition(&[]);
    assert_eq!(
        analysis.dispose(id(1), &region, 2).unwrap(),
        vec![
            TerminalAffineCleanupAction::DiscardResidual(
                terminal_psi::StructuralAffineDiscard {
                    place: id(1),
                    path: vec![StructuralPathSegment::Field(
                        types[0].shape.direct_fields().unwrap()[1].identity
                    )],
                    structural_type: id(2),
                }
            ),
            TerminalAffineCleanupAction::DiscardResidual(
                terminal_psi::StructuralAffineDiscard {
                    place: id(1),
                    path: vec![StructuralPathSegment::Field(
                        types[0].shape.direct_fields().unwrap()[0].identity
                    )],
                    structural_type: id(2),
                }
            ),
            TerminalAffineCleanupAction::RetireHome(id(1)),
        ]
    );
    assert!(analysis.dispose(id(1), &region, 1).is_err());
    region.set(&[Segment::Field(id(2))], false);
    let disposal = analysis.dispose(id(1), &region, 1).unwrap();
    assert!(
        matches!(disposal.as_slice(), [TerminalAffineCleanupAction::DiscardResidual(discard), TerminalAffineCleanupAction::RetireHome(_)] if discard.path == [StructuralPathSegment::Field(types[0].shape.direct_fields().unwrap()[0].identity)])
    );
    assert!(
        analysis.dispose(id(1), &Region::new(true), 1).is_err(),
        "an intact wrapper requires its actual selected target, never a forged no-code discard"
    );
}

fn reconvergence_types() -> [StructuralTypeDeclaration; 3] {
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralRecordStorage, StructuralTypeShape, TerminalIdentityIndex,
    };

    let member = |raw, child| StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        id: StructuralFieldId::new(raw).unwrap(),
        identity: TerminalIdentityIndex(raw as u32),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(StructuralTypeId::new(child).unwrap()),
        storage: None,
        inline_carrier: None,
    };
    let record = |raw, fields| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        id: StructuralTypeId::new(raw).unwrap(),
        identity: TerminalIdentityIndex(raw as u32),
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        quotient_carrier: None,
        scalar_restriction: None,
        requires: Vec::new(),
        shape: StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        },
    };
    [
        record(1, vec![member(1, 2), member(2, 2)]),
        record(2, vec![member(3, 3), member(4, 3)]),
        record(3, Vec::new()),
    ]
}

fn owned_analysis(types: &[StructuralTypeDeclaration]) -> Analysis<'_> {
    Analysis {
        module: None,
        fences: None,
        types,
        dispositions: vec![Some(true); types.len()],
        borrowed_headers: Vec::new(),
        roots: vec![Some(Root {
            structural_type: StructuralTypeId::new(1).unwrap(),
            owned: true,
            affine: true,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            whole_restoration: true,
            decomposition: Decomposition::Plain,
            parameter: false,
            exclusive: false,
        })],
        unavailable: Vec::new(),
        unavailable_roots: vec![None],
        stores: Vec::new(),
        verify_stores: false,
        references: None,
        body: None,
        residences: Vec::new(),
        position: None,
        successor: None,
        reference_candidates: None,
        reference_ends: Vec::new(),
        claim_flow: None,
    }
}

#[test]
fn an_invalid_last_use_path_keeps_its_scheduled_operation() {
    use terminal_psi::{Operation, OperationKind, OperationResult};

    let types = reconvergence_types();
    let place = id(1);
    let operation = id(1);
    let mut machine = transferring(&[id(1)], &[], Vec::new());
    let block = machine.entry;
    machine.blocks[0].operations.push(Operation::unsuspended(
        operation,
        OperationResult::Unit,
        OperationKind::ReleaseReference {
            source: place,
            path: Vec::new(),
        },
    ));
    let argument = StructuralArgument {
        place,
        path: vec![StructuralPathSegment::FixedIndex(7)],
        access: StructuralAccess::Owned,
    };
    let candidates = crate::reference_ends::Candidates::from([(
        (block, 0),
        vec![crate::reference_ends::Candidate {
            argument,
            unproved_case: None,
        }],
    )]);
    let mut analysis = owned_analysis(&types);
    analysis.reference_candidates = Some(&candidates);
    let state = State {
        regions: vec![Some(Region::new(true))],
        active: vec![place],
        claims: Default::default(),
    };
    let result = analysis.walk(&machine, block, state, &[], false, None);
    assert!(matches!(result, Err(Error::Unsupported {
        detail: "affine projection outside the static structural vocabulary",
        site: Some(Site::Operation(at)),
    }) if at == operation));
}

#[test]
fn a_reference_release_consumes_only_the_owned_descriptor() {
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralTypeShape,
        TerminalIdentityIndex as Identity,
    };
    let mut types = reconvergence_types();
    types[1].shape = StructuralTypeShape::Reference {
        referent: StructuralTypeId::new(3).unwrap(),
        access: StructuralAccess::MutableBorrow,
    };
    let place = PlaceId::new(1).unwrap();
    let mut analysis = owned_analysis(&types);
    let mut state = State {
        regions: vec![Some(Region::new(true))],
        active: vec![place],
        claims: Default::default(),
    };
    let release = Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Unit,
        OperationKind::ReleaseReference {
            source: place,
            path: vec![StructuralPathSegment::Field(Identity(1))],
        },
    );
    analysis.operation(&mut state, &release, true).unwrap();
    let region = state.regions[0].as_ref().unwrap();
    assert!(!region.available(&[field(1)]));
    assert!(region.available(&[field(2)]));
    assert_eq!(
        analysis.dispose(place, region, 1).unwrap(),
        [
            TerminalAffineCleanupAction::DiscardResidual(
                terminal_psi::StructuralAffineDiscard {
                    place,
                    path: vec![StructuralPathSegment::Field(Identity(2))],
                    structural_type: StructuralTypeId::new(2).unwrap(),
                }
            ),
            TerminalAffineCleanupAction::RetireHome(place),
        ]
    );
    let mut repeated = release.clone();
    repeated.id = OperationId::new(2).unwrap();
    analysis.operation(&mut state, &repeated, true).unwrap();
    assert!(
        matches!(analysis.unavailable.as_slice(), [Unavailable {site: Site::Operation(id), place: found, ..}]
        if *id == repeated.id && *found == place)
    );
    let mut borrowed = owned_analysis(&types);
    borrowed.roots[0].as_mut().unwrap().owned = false;
    let mut borrowed_state = State {
        regions: vec![Some(Region::new(true))],
        active: Vec::new(),
        claims: Default::default(),
    };
    let before = borrowed_state.clone();
    assert!(matches!(
        borrowed.operation(&mut borrowed_state, &release, true),
        Err(Error::Unsupported {
            detail: "owned extraction from borrowed storage",
            ..
        })
    ));
    assert_eq!(borrowed_state, before);
}

#[test]
fn partitioned_linear_wrappers_keep_remaining_component_dispositions() {
    let mut declarations = reconvergence_types();
    declarations[0].multiplicity = terminal_psi::StructuralMultiplicity::Linear;
    declarations[0].owns_linear_claim = true;
    declarations[0].no_code_affine_cleanup = false;
    declarations[0].descriptor_cleanup = false;
    let mut analysis = owned_analysis(&declarations);
    analysis.roots[0].as_mut().unwrap().multiplicity =
        terminal_psi::StructuralMultiplicity::Linear;
    analysis.dispositions.fill(None);
    prepare_disposition(
        &declarations,
        None,
        id(1),
        &mut analysis.dispositions,
        &mut vec![false; declarations.len()],
    )
    .unwrap();
    let mut partitioned = Region::new(true);
    partitioned.partition(&[]);
    let actions = analysis.dispose(id(1), &partitioned, 8).unwrap();
    assert!(matches!(
        actions.last(),
        Some(TerminalAffineCleanupAction::RetireHome(_))
    ));
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))),
        "a linear wrapper must still clean its surviving affine fields"
    );
    assert!(
        analysis.dispose(id(1), &Region::new(true), 8).is_err(),
        "an intact intrinsic wrapper still needs its explicit consuming pattern"
    );

    declarations[0].owns_linear_claim = false;
    declarations[1].multiplicity = terminal_psi::StructuralMultiplicity::Linear;
    declarations[1].owns_linear_claim = true;
    declarations[1].no_code_affine_cleanup = false;
    declarations[1].descriptor_cleanup = false;
    let mut analysis = owned_analysis(&declarations);
    analysis.roots[0].as_mut().unwrap().multiplicity =
        terminal_psi::StructuralMultiplicity::Linear;
    analysis.dispositions.fill(None);
    prepare_disposition(
        &declarations,
        None,
        id(1),
        &mut analysis.dispositions,
        &mut vec![false; declarations.len()],
    )
    .unwrap();
    assert!(
        analysis.dispose(id(1), &partitioned, 8).is_err(),
        "partitioning the parent cannot silently consume an intact linear child"
    );
}

#[test]
fn nominal_reference_components_end_only_after_the_wrapper_is_decomposed() {
    use terminal_psi::{Operation, OperationKind, OperationResult, TerminalIdentityIndex};

    let mut types = reconvergence_types();
    types[0].nominal_cleanup = Some(terminal_psi::NominalCleanupSelection {
        declaration: terminal_psi::NominalDeclarationIdentity::new(types[0].identity),
        cleanup_machine: id(9),
    });
    types[1].shape = StructuralTypeShape::Reference {
        referent: id(3),
        access: StructuralAccess::MutableBorrow,
    };
    let place = id(1);
    let at = (id(1), 0);
    let release = Operation::unsuspended(
        id(1),
        OperationResult::Unit,
        OperationKind::ReleaseReference {
            source: place,
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(1))],
        },
    );
    let candidates = crate::reference_ends::Candidates::from([(
        at,
        vec![crate::reference_ends::Candidate {
            argument: StructuralArgument {
                place,
                path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(1))],
                access: StructuralAccess::Owned,
            },
            unproved_case: None,
        }],
    )]);
    let mut analysis = owned_analysis(&types);
    analysis.reference_candidates = Some(&candidates);
    let mut state = State {
        regions: vec![Some(Region::new(true))],
        active: vec![place],
        claims: Default::default(),
    };
    let before = state.clone();
    analysis
        .end_references(&mut state, at, Site::Operation(release.id))
        .unwrap();
    assert_eq!(
        state, before,
        "automatic release must retain the intact wrapper"
    );
    assert!(analysis.reference_ends.is_empty());
    assert!(matches!(
        analysis.operation(&mut state, &release, true),
        Err(Error::Unsupported {
            detail: "reference release crosses an intact nominal wrapper",
            ..
        })
    ));
    assert_eq!(
        state, before,
        "forged explicit release must not vacate a field"
    );

    // The validated decomposition transition leaves the component values
    // at this same home; their ordinary reference closure then applies.
    state.regions[0].as_mut().unwrap().partition(&[]);
    let mut automatic = state.clone();
    analysis
        .end_references(&mut automatic, at, Site::Operation(release.id))
        .unwrap();
    assert_eq!(analysis.reference_ends.len(), 1);
    analysis.operation(&mut state, &release, true).unwrap();
    assert_eq!(state, automatic);
    let region = state.regions[0].as_ref().unwrap();
    assert_eq!(region.mode, Mode::Components);
    assert!(!region.available(&[field(1)]));
    assert!(region.available(&[field(2)]));
    assert!(!region.available(&[]));
}

#[test]
fn a_reference_observation_needs_its_descriptor_without_owning_its_backing() {
    use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex as Identity};

    let mut types = reconvergence_types();
    types[0].shape = StructuralTypeShape::Reference {
        referent: StructuralTypeId::new(2).unwrap(),
        access: StructuralAccess::MutableBorrow,
    };
    let mut analysis = owned_analysis(&types);
    let place = PlaceId::new(1).unwrap();
    let site = Site::Operation(OperationId::new(1).unwrap());
    let mut state = State {
        regions: vec![Some(Region::new(true))],
        active: vec![place],
        claims: Default::default(),
    };
    let mut argument = StructuralArgument {
        place,
        path: vec![
            StructuralPathSegment::Referent,
            StructuralPathSegment::Field(Identity(3)),
        ],
        access: StructuralAccess::SharedBorrow,
    };
    assert_eq!(
        analysis.observed(place, &argument.path).unwrap(),
        (vec![], false)
    );
    let original = state.clone();
    analysis
        .argument(&mut state, &argument, site, true)
        .unwrap();
    assert_eq!(state, original, "a borrow cannot move referenced storage");
    assert!(analysis.unavailable.is_empty());

    argument.access = StructuralAccess::Owned;
    assert!(matches!(
        analysis.argument(&mut state, &argument, site, true),
        Err(Error::Unsupported { .. })
    ));
    assert_eq!(state, original);
    argument.path[1] = StructuralPathSegment::Field(Identity(999));
    assert!(analysis.observed(place, &argument.path).is_err());

    argument.path[1] = StructuralPathSegment::Field(Identity(3));
    argument.access = StructuralAccess::SharedBorrow;
    state.regions[0] = None;
    analysis
        .argument(&mut state, &argument, site, true)
        .unwrap();
    assert!(
        matches!(analysis.unavailable.as_slice(), [Unavailable { site: found, place: found_place, .. }]
        if *found == site && *found_place == place)
    );
    assert!(
        state.regions[0].is_none(),
        "observation cannot revive a moved descriptor"
    );
}

#[test]
fn a_partly_moved_arrival_discards_only_the_nested_remainder_at_a_join() {
    use terminal_psi::{StructuralPathSegment, TerminalIdentityIndex};
    // One arrival transferred selected.first; the other transferred
    // selected whole. Equalizing their retained owner must discard only
    // selected.second, leaving the sibling subtree and moved value alone.
    let types = reconvergence_types();
    let mut analysis = owned_analysis(&types);
    let mut partial = Region::new(true);
    partial.set(&[field(1), field(3)], false);
    let mut whole = Region::new(true);
    whole.set(&[field(1)], false);
    let place = PlaceId::new(1).unwrap();
    let actions = analysis
        .equalize(place, &partial, &whole, 1)
        .expect("the partial arrival has one exact no-code residual");
    assert_eq!(
        actions,
        [TerminalAffineCleanupAction::DiscardResidual(
            terminal_psi::StructuralAffineDiscard {
                place,
                path: vec![
                    StructuralPathSegment::Field(TerminalIdentityIndex(1)),
                    StructuralPathSegment::Field(TerminalIdentityIndex(4)),
                ],
                structural_type: StructuralTypeId::new(3).unwrap(),
            },
        )]
    );
    assert!(analysis.equalize(place, &partial, &whole, 0).is_err());
    assert!(
        analysis
            .equalize(place, &whole, &whole, 0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reconvergence_scopes_keep_reverse_declaration_order_across_regions() {
    use terminal_psi::{
        StructuralPathSegment as Path, StructuralTypeShape, TerminalIdentityIndex as Identity,
    };
    let mut types = reconvergence_types();
    let StructuralTypeShape::Record { fields, .. } = &mut types[0].shape else {
        unreachable!()
    };
    let mut third = fields[1].clone();
    third.id = StructuralFieldId::new(5).unwrap();
    fields[0].identity = Identity(9);
    fields[1].identity = Identity(2);
    third.identity = Identity(5);
    fields.push(third);
    let moved = [vec![Path::Field(Identity(9)), Path::Field(Identity(3))]]
        .into_iter()
        .collect();
    let scopes = [9, 2, 5]
        .map(|identity| vec![Path::Field(Identity(identity))])
        .into_iter()
        .collect();
    let root = StructuralTypeId::new(1).unwrap();
    let residuals =
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &scopes, 3).unwrap();
    assert_eq!(
        residuals
            .iter()
            .map(|(path, ty)| (path.clone(), ty.get()))
            .collect::<Vec<_>>(),
        [
            (vec![Path::Field(Identity(5))], 2),
            (vec![Path::Field(Identity(2))], 2),
            (vec![Path::Field(Identity(9)), Path::Field(Identity(4))], 3),
        ]
    );
    assert!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &scopes, 2).is_none()
    );
    let sibling = [vec![Path::Field(Identity(2))]].into_iter().collect();
    assert_eq!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &sibling, 1).unwrap(),
        [(
            vec![Path::Field(Identity(2))],
            StructuralTypeId::new(2).unwrap()
        ),]
    );
}

#[test]
fn a_scoped_array_partition_does_not_enumerate_untouched_elements() {
    use terminal_psi::{
        ArrayExtent, StructuralPathSegment as Path, StructuralTypeShape,
        TerminalIdentityIndex as Identity,
    };
    let mut types = reconvergence_types();
    types[0].shape = StructuralTypeShape::FixedArray {
        element: StructuralTypeId::new(2).unwrap(),
        length: ArrayExtent::Literal(u64::MAX),
    };
    let root = StructuralTypeId::new(1).unwrap();
    let moved = [vec![Path::FixedIndex(1), Path::Field(Identity(3))]]
        .into_iter()
        .collect();
    let selected = [vec![Path::FixedIndex(1)]].into_iter().collect();
    assert_eq!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &selected, 1).unwrap(),
        [(
            vec![Path::FixedIndex(1), Path::Field(Identity(4))],
            StructuralTypeId::new(3).unwrap()
        ),]
    );
    let whole = [Vec::new()].into_iter().collect();
    assert!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &whole, 1).is_none()
    );
}

#[test]
fn rebuilt_children_cannot_be_erased_as_an_absent_ancestor() {
    let types = reconvergence_types();
    let mut analysis = owned_analysis(&types);
    let mut partial = Region::new(true);
    partial.set(&[field(1)], false);
    partial.set(&[field(1), field(3)], true);
    assert!(matches!(
        analysis.equalize(PlaceId::new(1).unwrap(), &partial, &Region::new(false), 2),
        Err(Error::Unsupported { .. })
    ));
}

#[test]
fn a_common_field_scope_does_not_require_or_invent_payload_membership() {
    use semantic_vocabulary::StructuralCaseId;
    use std::collections::BTreeSet;
    use terminal_psi::{
        StructuralCaseDeclaration, StructuralPathSegment as Path, StructuralTypeShape,
        TerminalIdentityIndex as Identity,
    };
    let mut types = reconvergence_types();
    let fields = types[0].shape.direct_fields().unwrap().to_vec();
    let mut second_payload = fields[1].clone();
    second_payload.id = StructuralFieldId::new(5).unwrap();
    second_payload.identity = Identity(5);
    types[0].shape = StructuralTypeShape::Mixed {
        fields: vec![fields[0].clone()],
        cases: vec![
            StructuralCaseDeclaration {
                declaration_identity: Default::default(),
                id: StructuralCaseId::new(1).unwrap(),
                identity: Identity(10),
                fields: vec![fields[1].clone()],
            },
            StructuralCaseDeclaration {
                declaration_identity: Default::default(),
                id: StructuralCaseId::new(2).unwrap(),
                identity: Identity(11),
                fields: vec![second_payload],
            },
        ],
    };
    let root = StructuralTypeId::new(1).unwrap();
    let common = [vec![Path::Field(Identity(1))]].into_iter().collect();
    let missing = BTreeSet::new();
    assert_eq!(
        crate::partial_affine_residuals_in_regions(&types, root, &missing, &common, 1).unwrap(),
        [(
            vec![Path::Field(Identity(1))],
            StructuralTypeId::new(2).unwrap()
        ),]
    );
    let payload = [vec![
        Path::Case(StructuralCaseId::new(1).unwrap()),
        Path::Field(Identity(2)),
    ]]
    .into_iter()
    .collect();
    assert!(
        crate::partial_affine_residuals_in_regions(&types, root, &missing, &payload, 1)
            .is_none(),
        "a receiving scope does not establish its payload case"
    );
    let moved = [vec![
        Path::Case(StructuralCaseId::new(1).unwrap()),
        Path::Field(Identity(2)),
        Path::Field(Identity(3)),
    ]]
    .into_iter()
    .collect();
    assert_eq!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &payload, 1).unwrap(),
        [(
            vec![
                Path::Case(StructuralCaseId::new(1).unwrap()),
                Path::Field(Identity(2)),
                Path::Field(Identity(4))
            ],
            StructuralTypeId::new(3).unwrap()
        ),]
    );
}

#[test]
fn scoped_cleanup_does_not_inspect_an_unrequested_erased_field() {
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralPathSegment as Path, StructuralTypeShape, TerminalIdentityIndex as Identity,
    };
    let mut types = reconvergence_types();
    let StructuralTypeShape::Record { fields, .. } = &mut types[0].shape else {
        unreachable!()
    };
    fields.push(StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        id: StructuralFieldId::new(5).unwrap(),
        identity: Identity(50),
        relevance: BindingRelevance::Erased,
        field_type: StructuralFieldType::Erased {
            type_identity: Identity(60),
            scalar: None,
        },
        storage: None,
        inline_carrier: None,
    });
    let root = StructuralTypeId::new(1).unwrap();
    let moved = [vec![Path::Field(Identity(1)), Path::Field(Identity(3))]]
        .into_iter()
        .collect();
    let selected = [vec![Path::Field(Identity(1))]].into_iter().collect();
    assert_eq!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &selected, 1)
            .expect("an unrelated erased field is outside the cleanup scope"),
        [(
            vec![Path::Field(Identity(1)), Path::Field(Identity(4))],
            StructuralTypeId::new(3).unwrap()
        ),]
    );
    let erased = [vec![Path::Field(Identity(50))]].into_iter().collect();
    assert!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &erased, 1).is_none(),
        "scoping cannot create runtime custody for an erased field"
    );
    let whole = [Vec::new()].into_iter().collect();
    assert!(
        crate::partial_affine_residuals_in_regions(&types, root, &moved, &whole, 3).is_none()
    );
    assert!(
        crate::partial_affine_residuals(&types, root, &moved, 3).is_none(),
        "the existing whole-partition refusal remains unchanged"
    );
}

/// A bundle is rebuilt after each edit, so it checks the module's type
/// permissions only when a walk first needs them, and every walk over it
/// then gets the answer a walk alone gives.
#[test]
fn a_bundle_checks_type_permissions_on_first_use_and_refuses_as_a_walk_alone() {
    let mut types = reconvergence_types();
    // A copyable parent whose fields hold affine records.
    types[0].multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
    let mut module = terminal_psi::TerminalModule::for_entry(id(1));
    module.types.structural_types = types.to_vec();
    module.machines = vec![transferring(
        &[id(3)],
        &[id(3)],
        vec![StructuralArgument::whole(id(1), StructuralAccess::Owned)],
    )];
    let analyses = crate::ModuleAnalyses::new(&module).unwrap();
    assert!(analyses.type_permissions.get().is_none());
    let alone = MachineAffine::new(&module, id(1))
        .unwrap()
        .plan()
        .unwrap_err();
    assert!(
        matches!(
            alone,
            Error::Malformed {
                detail: "parent type permissions erase a nested ownership obligation",
                site: None,
            }
        ),
        "{alone:?}"
    );
    for _ in 0..2 {
        let shared = analyses.affine(id(1)).unwrap().plan().unwrap_err();
        assert_eq!(format!("{shared:?}"), format!("{alone:?}"));
        assert!(matches!(
            analyses.type_permissions.get(),
            Some(Err(Error::Malformed { .. }))
        ));
    }

    // Valid permissions are checked once and accepted.
    drop(analyses);
    module.types.structural_types = reconvergence_types().to_vec();
    let analyses = crate::ModuleAnalyses::new(&module).unwrap();
    assert!(analyses.type_permissions.get().is_none());
    analyses.affine(id(1)).unwrap().plan().unwrap();
    assert!(matches!(analyses.type_permissions.get(), Some(Ok(()))));
}
