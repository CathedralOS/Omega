//! Unranked claim-carrying cycles: claims pinned on owned entry parameters
//! stay admitted while every custody disturbance keeps the fence closed.

use super::{id, validate_module, validate_module_representation};
use semantic_vocabulary::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ClaimContentProjection, ContentEntryClaim, EntryClaim, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralContentProjection, StructuralDomainDeclaration, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, reconstruct_structural_ownership_frontiers, validate_module_for_interpretation,
    validate_module_for_optimization,
};

/// The emitted `Boot::launch` shape: an entry block falls into a `retain`
/// self-loop that never leaves, keeping every `Extent`-like claim on its
/// original owned machine parameter for the machine's whole cyclic lifetime.
fn claim_pinned_retain_cycle() -> TerminalModule {
    let machine = id(1, MachineId::new);
    let entry = id(1, BlockId::new);
    let retain = id(2, BlockId::new);
    let image = id(1, PlaceId::new);
    let storage = id(2, PlaceId::new);
    let extent = id(1, StructuralTypeId::new);
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Byte".to_owned(),
    };
    let expression = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(1).expect("content domain"),
        projection_report_fingerprint:
            language_semantics::content::terminal_projection_report_fingerprint(
                &algebra,
                &expression,
            ),
    };
    let parameter = |place: PlaceId, position: u32| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type: extent,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let claims = |claim: u64, root: PlaceId| {
        let claim = ClaimId::new(claim).expect("claim");
        (
            EntryClaim {
                claim,
                input: root,
                path: Vec::new(),
            },
            ContentEntryClaim {
                claim,
                input: ContentStructuralPlace {
                    version: ContentPlaceVersion::Entry,
                    root,
                    segments: Vec::new(),
                },
                projections: vec![ClaimContentProjection {
                    projection,
                    algebra: algebra.clone(),
                }],
            },
        )
    };
    let (image_claim, image_content_claim) = claims(1, image);
    let (storage_claim, storage_content_claim) = claims(2, storage);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: extent,
            identity: "Extent".to_owned(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: semantic_vocabulary::StructuralDomainId::new(1).expect("structural domain"),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain"),
            identity: "Extent::Granted".to_owned(),
            carrier: extent,
            content_projection: Some(StructuralContentProjection {
                identity: projection,
                algebra,
                expression,
            }),
        }],
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![parameter(image, 0), parameter(storage, 1)],
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: image,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: storage,
                    kind: StructuralPlaceKind::Parameter {
                        position: 1,
                        is_self: false,
                    },
                },
            ],
            entry_claims: vec![image_claim, storage_claim],
            published_service_ceiling: Vec::new(),
            content_entry_claims: vec![image_content_claim, storage_content_claim],
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: id(1, EdgeId::new),
                        target: retain,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: retain,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: id(2, EdgeId::new),
                        target: retain,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: id(1, ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

#[test]
fn claim_pinned_retain_cycle_validates() {
    let module = claim_pinned_retain_cycle();
    validate_module(&module)
        .map(|_| ())
        .expect("claims pinned at owned entry parameters retain around the loop");
    validate_module_representation(&module).expect("representation policy agrees");
    validate_module_for_interpretation(&module).expect("interpretation policy agrees");
    validate_module_for_optimization(&module).expect("optimization policy agrees");
    let frontiers =
        reconstruct_structural_ownership_frontiers(&module).expect("frontiers reconstruct");
    let reconstructed = frontiers.machine(id(1, MachineId::new)).unwrap();
    for block in [id(1, BlockId::new), id(2, BlockId::new)] {
        assert!(
            reconstructed.block_entry(block).is_some(),
            "missing block {block:?}"
        );
    }
    module
        .machines
        .iter()
        .for_each(|machine| assert_eq!(machine.entry, id(1, BlockId::new)));
}

#[test]
fn claim_pinned_cycle_with_guard_arms_stays_pinned() {
    let mut module = claim_pinned_retain_cycle();
    let machine = &mut module.machines[0];
    let condition = id(1, ValueId::new);
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: condition,
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
    });
    // Both arms re-enter the same block; no claim root or identity moves.
    machine.blocks[1].terminator = Terminator::Conditional {
        condition,
        when_true: terminal_psi::SuccessorEdge {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: terminal_psi::SuccessorEdge {
            edge: id(3, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    validate_module(&module)
        .map(|_| ())
        .expect("guard arms re-entering the retain block keep custody pinned");
}

#[test]
fn claim_cycle_rejects_operations_naming_a_pinned_root() {
    let mut module = claim_pinned_retain_cycle();
    let retain = &mut module.machines[0].blocks[1];
    // Claim roots are not readable operation sources at all — the module
    // rejects at operation registration, before the cycle fence ever runs.
    // `claims_pinned_at_entry` rechecks the same invariant at the fence.
    retain.operations.push(Operation {
        static_reach_binding: None,
        id: id(1, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(2, ValueId::new),
            scalar_type: semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
            ),
        }),
        kind: OperationKind::PrimitiveScalarRead {
            source: id(1, PlaceId::new),
            path: Vec::new(),
        },
    });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidPrimitiveScalarRead {
            operation: id(1, OperationId::new),
            place: id(1, PlaceId::new),
        })
    );
}

#[test]
fn claim_cycle_rejects_edge_rebinding_of_a_pinned_root() {
    let mut module = claim_pinned_retain_cycle();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(2, BlockId::new),
            position: 0,
        },
    });
    let retain = &mut machine.blocks[1];
    // The loop's parameter declaration is itself admissible — an owned affine
    // block parameter on a fresh place — so only the self-edge binding a
    // pinned claim root keeps the custody fence closed.
    retain
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(3, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: id(1, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut retain.terminator
    else {
        panic!("retain self-loop")
    };
    structural_arguments.push(StructuralArgument {
        place: id(1, PlaceId::new),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ControlCycle(id(2, BlockId::new)))
    );
}

#[test]
fn claim_cycle_rejects_a_pinned_root_as_block_parameter() {
    let mut module = claim_pinned_retain_cycle();
    // A claim root can never be declared as a block parameter in any graph —
    // declaration registration rejects before the cycle fence runs, and the
    // fence's own `!roots.contains` clause rechecks the same invariant.
    module.machines[0].blocks[1]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(1, PlaceId::new),
            position: 0,
            is_self: false,
            structural_type: id(1, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidBlockStructuralParameter {
            block: id(2, BlockId::new),
            place: id(1, PlaceId::new),
        })
    );
}

#[test]
fn claim_cycle_rejects_edge_discard_of_a_pinned_root() {
    let mut module = claim_pinned_retain_cycle();
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[1].terminator
    else {
        panic!("retain self-loop")
    };
    trivial_affine_discards.push(id(1, PlaceId::new));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ControlCycle(id(2, BlockId::new)))
    );
}

#[test]
fn claim_cycle_rejects_transfer_of_a_pinned_claim() {
    let mut module = claim_pinned_retain_cycle();
    // A self-call whose roster hands both pinned claims back into the same
    // signature is registration-valid: argument claims match the callee's
    // entry claims exactly. Only the custody fence refuses, because moving a
    // pinned claim identity or rebinding its root inside the loop leaves
    // custody unpinned.
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        id: id(1, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            callee: id(1, MachineId::new),
            arguments: Vec::new(),
            structural_arguments: vec![
                StructuralArgument {
                    place: id(1, PlaceId::new),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
                StructuralArgument {
                    place: id(2, PlaceId::new),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            ],
            claim_transfers: vec![
                terminal_psi::ClaimTransfer {
                    claim: id(1, ClaimId::new),
                    argument_index: 0,
                },
                terminal_psi::ClaimTransfer {
                    claim: id(2, ClaimId::new),
                    argument_index: 1,
                },
            ],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ControlCycle(id(2, BlockId::new)))
    );
}

#[test]
fn claim_cycle_rejects_return_of_a_pinned_claim() {
    let mut module = claim_pinned_retain_cycle();
    let machine = &mut module.machines[0];
    let condition = id(1, ValueId::new);
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: condition,
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
    });
    // The loop survives; the exit edge leaves custody alone but the exit
    // block's structural return tries to hand back a pinned claim.
    machine.blocks[1].terminator = Terminator::Conditional {
        condition,
        when_true: terminal_psi::SuccessorEdge {
            edge: id(2, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: terminal_psi::SuccessorEdge {
            edge: id(3, EdgeId::new),
            target: id(3, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(3, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnStructural {
            edge: id(4, EdgeId::new),
            source: id(1, PlaceId::new),
            returned_claims: vec![id(1, ClaimId::new)],
            trivial_affine_discards: Vec::new(),
        },
    });
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ControlCycle(id(2, BlockId::new)))
    );
}
