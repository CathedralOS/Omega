//! Produced view descriptors transfer their usable exclusive name across state
//! edges independently of the affine descriptor's eventual cleanup. These
//! modules are reconstructed without checked-source custody or source text.

use super::{
    AdmissionProfile, Block, ModuleError, Operation, OperationKind, OperationResult, ProofBundle,
    ScalarType, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalModule, Terminator, ValueDeclaration, block_id,
    edge_id, operation_id, place_id, structural_type_id, validate_module, value_id, verify_module,
    write_only_primitive_store_module,
};
use semantic_vocabulary::{ClaimId, IntegerSign, IntegerType, StructuralFieldId};
use terminal_psi::{BindingRelevance, StructuralOperationResult, StructuralResultClaimBinding};

const ROOT: u64 = 1;
const PRODUCED: u64 = 2;
const RECEIVED: u64 = 3;
const FORWARDED: u64 = 4;
const ELEMENT: u64 = 1;
const ARRAY: u64 = 2;
const VIEW: u64 = 3;

fn parameter(place: u64, position: u32, structural_type: u64) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place_id(place),
        position,
        is_self: false,
        structural_type: structural_type_id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn argument(place: u64, access: StructuralAccess) -> StructuralArgument {
    StructuralArgument {
        place: place_id(place),
        path: Vec::new(),
        access,
    }
}

fn jump(edge: u64, target: u64, place: u64, access: StructuralAccess) -> Terminator {
    Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![argument(place, access)],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn successor(edge: u64, target: u64, place: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![argument(place, StructuralAccess::MutableBorrow)],
        trivial_affine_discards: Vec::new(),
    }
}

fn block(
    id: u64,
    parameters: Vec<StructuralParameterDeclaration>,
    terminator: Terminator,
) -> Block {
    Block {
        id: block_id(id),
        parameters: Vec::new(),
        structural_parameters: parameters,
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: Vec::new(),
        terminator,
    }
}

fn declare_parameter(module: &mut TerminalModule, place: u64, block: u64, position: u32) {
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(place),
            kind: StructuralPlaceKind::BlockParameter {
                block: block_id(block),
                position,
            },
        });
}

fn length(source: u64) -> Operation {
    Operation {
        id: operation_id(2),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(2),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            qualifications: Default::default(),
        }),
        kind: OperationKind::ElementViewLength {
            source: place_id(source),
        },
    }
}

/// The array belongs to the caller. Its mutable view is produced in block 1,
/// bound as block 2's parameter, and read there before the descriptor retires.
fn view_module() -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(ELEMENT),
            identity: "test::Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "flag".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(ARRAY),
            identity: "test::Cells".into(),
            shape: StructuralTypeShape::FixedArray {
                element: structural_type_id(ELEMENT),
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(VIEW),
            identity: "test::CellView".into(),
            shape: StructuralTypeShape::ElementView {
                element: structural_type_id(ELEMENT),
            },
        },
    ];
    let machine = &mut module.machines[0];
    machine.parameters.clear();
    machine.structural_parameters = vec![parameter(ROOT, 0, ARRAY)];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(PRODUCED),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(1),
                structural_type: structural_type_id(VIEW),
            },
        },
    ];
    let mut entry = block(
        1,
        Vec::new(),
        jump(1, 2, PRODUCED, StructuralAccess::MutableBorrow),
    );
    entry.operations.push(Operation {
        id: operation_id(1),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: place_id(PRODUCED),
            structural_type: structural_type_id(VIEW),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishMutableElementView {
            region: terminal_psi::ElementViewRegion::WholeCollection,
            destination: place_id(PRODUCED),
            source: argument(ROOT, StructuralAccess::MutableBorrow),
            element: structural_type_id(ELEMENT),
        },
    });
    let mut received = block(
        2,
        vec![parameter(RECEIVED, 0, VIEW)],
        Terminator::ReturnUnit {
            edge: edge_id(2),
            trivial_affine_discards: vec![place_id(PRODUCED)],
        },
    );
    received.operations.push(length(RECEIVED));
    machine.blocks = vec![entry, received];
    declare_parameter(&mut module, RECEIVED, 2, 0);
    module
}

fn result(module: &mut TerminalModule) -> &mut StructuralOperationResult {
    let OperationResult::Structural(result) =
        &mut module.machines[0].blocks[0].operations[0].result
    else {
        unreachable!("the establishment produces a descriptor")
    };
    result
}

fn forward(module: &mut TerminalModule, access: StructuralAccess) {
    let machine = &mut module.machines[0];
    let mut final_block = machine.blocks[1].clone();
    final_block.id = block_id(3);
    final_block.structural_parameters[0].place = place_id(FORWARDED);
    final_block.structural_parameters[0].access = access;
    final_block.operations[0].kind = OperationKind::ElementViewLength {
        source: place_id(FORWARDED),
    };
    final_block.terminator = Terminator::ReturnUnit {
        edge: edge_id(3),
        trivial_affine_discards: vec![place_id(PRODUCED)],
    };
    machine.blocks[1].operations.clear();
    machine.blocks[1].terminator = jump(2, 3, RECEIVED, access);
    machine.blocks.push(final_block);
    declare_parameter(module, FORWARDED, 3, 0);
}

#[test]
fn produced_mutable_view_forwards_through_multiple_states() {
    let mut module = view_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a produced mutable descriptor binds a usable block view");
    forward(&mut module, StructuralAccess::MutableBorrow);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the received mutable view can transfer to a further state");
}

#[test]
fn shared_establishment_cannot_supply_an_exclusive_successor() {
    let mut module = view_module();
    result(&mut module).multiplicity = StructuralMultiplicity::Unrestricted;
    module.machines[0].blocks[0].operations[0].kind = OperationKind::EstablishElementView {
        region: terminal_psi::ElementViewRegion::WholeCollection,
        destination: place_id(PRODUCED),
        source: argument(ROOT, StructuralAccess::SharedBorrow),
        element: structural_type_id(ELEMENT),
    };
    module.machines[0].blocks[1].terminator = Terminator::ReturnUnit {
        edge: edge_id(2),
        trivial_affine_discards: Vec::new(),
    };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: edge_id(1),
            place: place_id(PRODUCED),
        })
    );
}

#[test]
fn mutable_view_establishment_cannot_widen_its_backing_access() {
    let baseline = view_module();
    verify_module(
        &baseline,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the mutable backing grants readable exclusive authority");
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = baseline.clone();
        module.machines[0].structural_parameters[0].access = access;
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "an authored mutable source argument cannot widen {access:?} backing authority",
        );
    }
}

fn reference_backed_view_module() -> TerminalModule {
    let mut module = view_module();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(4),
            identity: "test::Room".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "cells".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(ARRAY)),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(5),
            identity: "test::RoomReference".into(),
            shape: StructuralTypeShape::Reference {
                referent: structural_type_id(4),
                access: StructuralAccess::MutableBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(6),
            identity: "test::RoomHolder".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "borrowed".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(5)),
                }],
            },
        },
    ]);
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].structural_type = structural_type_id(6);
    machine.structural_parameters[0].access = StructuralAccess::Owned;
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    let OperationKind::EstablishMutableElementView { source, .. } =
        &mut machine.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    source.path = vec![
        terminal_psi::StructuralPathSegment::Field("borrowed".into()),
        terminal_psi::StructuralPathSegment::Referent,
        terminal_psi::StructuralPathSegment::Field("cells".into()),
    ];
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[1].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.push(place_id(ROOT));
    module
}

#[test]
fn mutable_view_cannot_widen_a_nested_reference_permission() {
    let baseline = reference_backed_view_module();
    verify_module(
        &baseline,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("an owned holder's mutable reference lends its selected array");
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = baseline.clone();
        let StructuralTypeShape::Reference {
            access: granted, ..
        } = &mut module.structural_types[4].shape
        else {
            unreachable!()
        };
        *granted = access;
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "owning a holder cannot widen its nested {access:?} permission"
        );
    }
}

#[test]
fn shared_view_respects_the_nested_reference_access_lattice() {
    let mut baseline = reference_backed_view_module();
    let OperationKind::EstablishMutableElementView {
        region: _,
        destination,
        source,
        element,
    } = baseline.machines[0].blocks[0].operations[0].kind.clone()
    else {
        unreachable!()
    };
    baseline.machines[0].blocks[0].operations[0].kind = OperationKind::EstablishElementView {
        region: terminal_psi::ElementViewRegion::WholeCollection,
        destination,
        source: StructuralArgument {
            access: StructuralAccess::SharedBorrow,
            ..source
        },
        element,
    };
    result(&mut baseline).multiplicity = StructuralMultiplicity::Unrestricted;
    baseline.machines[0].blocks[0].terminator =
        jump(1, 2, PRODUCED, StructuralAccess::SharedBorrow);
    baseline.machines[0].blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    baseline.machines[0].blocks[1].terminator = Terminator::ReturnUnit {
        edge: edge_id(2),
        trivial_affine_discards: vec![place_id(ROOT)],
    };
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = baseline.clone();
        let StructuralTypeShape::Reference {
            access: granted, ..
        } = &mut module.structural_types[4].shape
        else {
            unreachable!()
        };
        *granted = access;
        let outcome = verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        );
        if access == StructuralAccess::WriteOnlyBorrow {
            assert!(
                outcome.is_err(),
                "a shared view cannot read through a write-only reference"
            );
        } else {
            outcome.unwrap_or_else(|error| {
                panic!("a {access:?} reference can lend a shared collection view: {error:?}")
            });
        }
    }
}

#[test]
fn produced_mutable_view_requires_its_exact_unqualified_claim_free_result() {
    let baseline = view_module();
    validate_module(&baseline).expect("the untampered produced view is valid");
    let mut wrong_type = baseline.clone();
    wrong_type.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(4),
            identity: "test::OtherCell".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
        StructuralTypeDeclaration {
            id: structural_type_id(5),
            identity: "test::OtherCellView".into(),
            shape: StructuralTypeShape::ElementView {
                element: structural_type_id(4),
            },
        },
    ]);
    wrong_type.machines[0].blocks[1].structural_parameters[0].structural_type =
        structural_type_id(5);
    assert_eq!(
        validate_module(&wrong_type).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: edge_id(1),
            place: place_id(PRODUCED),
        }),
        "a view of another element type cannot receive this descriptor",
    );
    let mut wrong_multiplicity = baseline.clone();
    result(&mut wrong_multiplicity).multiplicity = StructuralMultiplicity::Unrestricted;
    assert!(
        validate_module(&wrong_multiplicity).is_err(),
        "exclusive establishment remains affine"
    );
    let mut qualified = baseline.clone();
    result(&mut qualified)
        .qualifications
        .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap());
    assert!(
        validate_module(&qualified).is_err(),
        "an edge cannot certify fabricated qualifications"
    );
    let mut claimed = baseline;
    result(&mut claimed)
        .claims
        .push(StructuralResultClaimBinding {
            claim: ClaimId::new(1).unwrap(),
            path: Vec::new(),
        });
    assert!(
        validate_module(&claimed).is_err(),
        "an edge cannot transfer fabricated claim authority"
    );
}

#[test]
fn produced_mutable_view_cannot_fill_two_exclusive_edge_arguments() {
    let mut module = view_module();
    let second = parameter(FORWARDED, 1, VIEW);
    module.machines[0].blocks[1]
        .structural_parameters
        .push(second);
    declare_parameter(&mut module, FORWARDED, 2, 1);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments.push(argument(PRODUCED, StructuralAccess::MutableBorrow));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: edge_id(1),
            place: place_id(PRODUCED),
        })
    );
}

#[test]
fn produced_mutable_name_is_unavailable_after_transfer() {
    let mut module = view_module();
    validate_module(&module).expect("the receiving parameter owns the usable view name");
    module.machines[0].blocks[1].operations[0].kind = OperationKind::ElementViewLength {
        source: place_id(PRODUCED),
    };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ElementViewNotEstablished {
            operation: operation_id(2),
            place: place_id(PRODUCED),
        })
    );
}

#[test]
fn whole_shared_attenuation_retires_the_produced_exclusive_name() {
    let mut module = view_module();
    module.machines[0].blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    module.machines[0].blocks[0].terminator = jump(1, 2, PRODUCED, StructuralAccess::SharedBorrow);
    validate_module(&module).expect("a produced mutable view can attenuate to a shared state view");
    module.machines[0].blocks[1].operations[0].kind = OperationKind::ElementViewLength {
        source: place_id(PRODUCED),
    };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ElementViewNotEstablished {
            operation: operation_id(2),
            place: place_id(PRODUCED),
        })
    );
}

fn condition(module: &mut TerminalModule) {
    module.machines[0].parameters.push(ValueDeclaration {
        id: value_id(1),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
}

#[test]
fn produced_view_must_exist_on_every_incoming_path() {
    let mut module = view_module();
    condition(&mut module);
    let machine = &mut module.machines[0];
    let mut left = block(
        3,
        Vec::new(),
        jump(3, 2, PRODUCED, StructuralAccess::MutableBorrow),
    );
    let right = block(
        4,
        Vec::new(),
        jump(4, 2, PRODUCED, StructuralAccess::MutableBorrow),
    );
    let mut true_edge = successor(1, 3, PRODUCED);
    true_edge.structural_arguments.clear();
    let mut false_edge = successor(5, 4, PRODUCED);
    false_edge.structural_arguments.clear();
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(1),
        when_true: true_edge,
        when_false: false_edge,
    };
    machine.blocks.push(left.clone());
    machine.blocks.push(right);
    validate_module(&module)
        .expect("one dominating descriptor can move on either exclusive branch");
    // Keep the same join and edge rosters but form the descriptor only on the
    // left path. The right path cannot obtain authority from that producer.
    left.operations
        .push(module.machines[0].blocks[0].operations.remove(0));
    module.machines[0].blocks[2] = left;
    assert!(
        validate_module(&module).is_err(),
        "one incoming producer cannot establish the other arrival"
    );
}

#[test]
fn mutable_view_backedge_transfers_the_current_parameter_only() {
    let mut module = view_module();
    forward(&mut module, StructuralAccess::MutableBorrow);
    condition(&mut module);
    module.machines[0].blocks[1].terminator = Terminator::Conditional {
        condition: value_id(1),
        when_true: successor(4, 2, RECEIVED),
        when_false: successor(2, 3, RECEIVED),
    };
    validate_module(&module)
        .expect("the mutable block parameter remains available on its backedge");
    let Terminator::Conditional { when_true, .. } = &mut module.machines[0].blocks[1].terminator
    else {
        unreachable!()
    };
    when_true.structural_arguments[0].place = place_id(PRODUCED);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::InvalidStructuralSuccessorArgument {
            edge: edge_id(4),
            place: place_id(PRODUCED),
        })
    );
}

fn representation_region_module() -> TerminalModule {
    let mut module = view_module();
    let unsigned =
        |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(unsigned(16));
    module.structural_types[1].shape = StructuralTypeShape::FixedArray {
        element: structural_type_id(4),
        length: 8,
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(4),
        identity: "test::octet".into(),
        shape: StructuralTypeShape::PrimitiveScalar(unsigned(8)),
    });
    module.machines[0].parameters = vec![
        ValueDeclaration {
            id: value_id(30),
            scalar_type: unsigned(64),
            qualifications: Default::default(),
        },
        ValueDeclaration {
            id: value_id(31),
            scalar_type: unsigned(64),
            qualifications: Default::default(),
        },
    ];
    let OperationKind::EstablishMutableElementView { region, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("formation")
    };
    *region = terminal_psi::ElementViewRegion::RepresentationBytes {
        start: value_id(30),
        end: value_id(31),
        obligation: semantic_vocabulary::ObligationId::new(40).unwrap(),
    };
    module
}

#[test]
fn representation_region_requires_exact_geometry_operands_and_certificate() {
    let module = representation_region_module();
    validate_module(&module).expect("well-typed region owns an independent obligation");
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "formation must not silently waive its missing tiling/bounds proof"
    );
    for mutation in 0..5 {
        let mut wrong = module.clone();
        match mutation {
            0 => wrong.machines[0].parameters[0].scalar_type = ScalarType::Boolean,
            1 => {
                let OperationKind::EstablishMutableElementView { region, .. } =
                    &mut wrong.machines[0].blocks[0].operations[0].kind
                else {
                    panic!()
                };
                *region = terminal_psi::ElementViewRegion::WholeCollection;
            }
            2 => {
                wrong.structural_types[0].shape =
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
            }
            3 => {
                let StructuralTypeShape::FixedArray { element, .. } =
                    &mut wrong.structural_types[1].shape
                else {
                    panic!()
                };
                *element = structural_type_id(ELEMENT);
            }
            4 => {
                let OperationKind::EstablishMutableElementView {
                    region: terminal_psi::ElementViewRegion::RepresentationBytes { start, .. },
                    ..
                } = &mut wrong.machines[0].blocks[0].operations[0].kind
                else {
                    panic!()
                };
                *start = value_id(999);
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module(&wrong).is_err(),
            "invalid region mutation {mutation}"
        );
    }
}

fn byte_representation_region_module(shared: bool) -> (TerminalModule, ProofBundle) {
    use proof_admission::{
        CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    };
    use semantic_vocabulary::{EvidenceIdentity, ObligationId};
    let mut module = representation_region_module();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
    ));
    module.structural_types[2].shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: None,
        });
    module.machines[0].blocks[1].operations[0].kind = OperationKind::ByteSequenceLength {
        source: place_id(RECEIVED),
    };
    if shared {
        module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
        module.machines[0].blocks[1].structural_parameters[0].access =
            StructuralAccess::SharedBorrow;
        result(&mut module).multiplicity = StructuralMultiplicity::Unrestricted;
        let OperationKind::EstablishMutableElementView {
            destination,
            source,
            element,
            region,
        } = module.machines[0].blocks[0].operations[0].kind.clone()
        else {
            unreachable!()
        };
        module.machines[0].blocks[0].operations[0].kind = OperationKind::EstablishElementView {
            destination,
            source: StructuralArgument {
                access: StructuralAccess::SharedBorrow,
                ..source
            },
            element,
            region,
        };
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        structural_arguments[0].access = StructuralAccess::SharedBorrow;
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut module.machines[0].blocks[1].terminator
        else {
            unreachable!()
        };
        trivial_affine_discards.clear();
    }
    let goal =
        terminal_semantics::representation_region_goal(value_id(30), value_id(31), 8, 1).unwrap();
    module.machines[0].contract.requires = vec![goal.clone()];
    let bundle = ProofBundle {
        evidence: vec![terminal_psi::ObligationEvidence {
            obligation: ObligationId::new(40).unwrap(),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(40).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    (module, bundle)
}

#[test]
fn byte_representation_region_replays_borrowed_result_and_independent_certificate() {
    for shared in [false, true] {
        let (module, proof) = byte_representation_region_module(shared);
        verify_module(&module, &proof, &AdmissionProfile::default())
            .expect("exact u8 region is an ordinary forwarded byte descriptor");
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err()
        );
    }
}

#[test]
fn byte_representation_region_rejects_carrier_access_and_evidence_drift() {
    let (module, proof) = byte_representation_region_module(false);
    for mutation in 0..6 {
        let mut wrong = module.clone();
        match mutation {
            0 => {
                wrong.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
                )
            }
            1 => {
                wrong.structural_types[2].shape = StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                )
            }
            2 => wrong.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow,
            3 => {
                let OperationKind::EstablishMutableElementView {
                    region: terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. },
                    ..
                } = &mut wrong.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                std::mem::swap(start, end);
            }
            4 => {
                let StructuralTypeShape::FixedArray { length, .. } =
                    &mut wrong.structural_types[1].shape
                else {
                    unreachable!()
                };
                *length = 7;
            }
            5 => wrong.machines[0].contract.requires.clear(),
            _ => unreachable!(),
        }
        assert!(
            verify_module(&wrong, &proof, &AdmissionProfile::default()).is_err(),
            "invalid byte-region mutation {mutation}"
        );
    }
}
