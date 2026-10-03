//! A boundary call lends a mutable block view whole, with the custody an
//! ordinary call gets.
//!
//! A state machine threads `region: &mut Region` (or a mutable byte view)
//! through its states as a borrowed block parameter, and a host call such as
//! `read_bytes(region)` lends that view for the call alone. Argument
//! validation admits the loan only when the view itself is mutable. The
//! mutable-view availability fixed point, the argument-pair aliasing check
//! and the boundary declaration's reference refusal then decide custody: a
//! second overlapping loan in the same call, a loan of a view name that
//! already moved to a successor, and a loan the host could keep past the call
//! all reject.

use super::{
    AdmissionProfile, Block, ModuleError, Operation, OperationKind, OperationResult, ProofBundle,
    ScalarType, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, Terminator, block_id, boundary_id, edge_id, operation_id,
    place_id, structural_type_id, validate_module, verify_module,
    write_only_primitive_store_module,
};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind, ByteSequenceCarrier};

const ROOT: u64 = 1;
const VIEW: u64 = 2;
const FORWARDED: u64 = 3;
const HOST_FORMAL: u64 = 9;

/// The borrowed referent the machine threads through its states.
#[derive(Clone, Copy, Debug)]
enum ViewKind {
    /// `&mut Region`, a plain record, the shape of squalr's memory queryer.
    Record,
    /// `&mut [u8]`, a mutable borrowed byte view.
    Bytes,
}

const VIEW_KINDS: [ViewKind; 2] = [ViewKind::Record, ViewKind::Bytes];

fn view_type(kind: ViewKind) -> StructuralTypeDeclaration {
    let shape = match kind {
        ViewKind::Record => StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "base".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
        },
        ViewKind::Bytes => StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        }),
    };
    StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "test::Region".into(),
        shape,
    }
}

fn mutable_view(place: u64, position: u32) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place_id(place),
        position,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn lend(place: u64, access: StructuralAccess) -> StructuralArgument {
    StructuralArgument {
        place: place_id(place),
        path: Vec::new(),
        access,
    }
}

fn jump(edge: u64, target: u64, structural_arguments: Vec<StructuralArgument>) -> Terminator {
    Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn state(
    id: u64,
    parameters: Vec<StructuralParameterDeclaration>,
    terminator: Terminator,
) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: parameters,
        id: block_id(id),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    }
}

fn boundary_call(structural_arguments: Vec<StructuralArgument>) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(1),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments,
            completion_receipts: Vec::new(),
        },
    }
}

/// `fill(root: &mut Region)` lends its mutable parameter (place 1) along
/// edge 1 into state 2's `region` view (place 2). State 2 calls the host's
/// `fill_region(region: &mut Region)` with the view lent whole, then returns.
fn view_module(kind: ViewKind) -> TerminalModule {
    let mut module = write_only_primitive_store_module();
    module.structural_types = vec![view_type(kind)];
    module.boundary_machines = vec![BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary_id(1),
        identity: "fill_region".into(),
        attachment: None,
        parameter_order: vec![BoundaryParameterKind::Structural],
        scalar_parameters: Vec::new(),
        structural_parameters: vec![mutable_view(HOST_FORMAL, 0)],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let machine = &mut module.machines[0];
    machine.parameters.clear();
    machine.structural_parameters = vec![mutable_view(ROOT, 0)];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(VIEW),
            kind: StructuralPlaceKind::BlockParameter {
                block: block_id(2),
                position: 0,
            },
        },
    ];
    let mut region = state(
        2,
        vec![mutable_view(VIEW, 0)],
        Terminator::ReturnUnit {
            edge: edge_id(2),
            trivial_affine_discards: Vec::new(),
        },
    );
    region.operations = vec![boundary_call(vec![lend(
        VIEW,
        StructuralAccess::MutableBorrow,
    )])];
    machine.blocks = vec![
        state(
            1,
            Vec::new(),
            jump(1, 2, vec![lend(ROOT, StructuralAccess::MutableBorrow)]),
        ),
        region,
    ];
    module
}

fn host_arguments(module: &mut TerminalModule) -> &mut Vec<StructuralArgument> {
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[1].operations[0].kind
    else {
        unreachable!("state 2 opens with the host call")
    };
    structural_arguments
}

/// The refusal for a host loan of an exclusive name no longer available in
/// the calling state. A mutable record name follows the whole-record alias
/// replay; a byte view follows the view-establishment availability check.
fn stale_name(kind: ViewKind, place: u64) -> ModuleError {
    match kind {
        ViewKind::Record => ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation_id(1),
            place: place_id(place),
        },
        ViewKind::Bytes => ModuleError::ByteSequenceViewNotEstablished {
            operation: operation_id(1),
            place: place_id(place),
        },
    }
}

/// Give the host a second `&mut Region` formal and the call a second actual.
fn with_second_host_loan(kind: ViewKind, second: StructuralArgument) -> TerminalModule {
    let mut module = view_module(kind);
    let host = &mut module.boundary_machines[0];
    host.parameter_order.push(BoundaryParameterKind::Structural);
    let mut formal = mutable_view(HOST_FORMAL + 1, 1);
    formal.access = second.access;
    host.structural_parameters.push(formal);
    host_arguments(&mut module).push(second);
    module
}

#[test]
fn boundary_call_lends_a_mutable_block_view_whole() {
    for kind in VIEW_KINDS {
        let module = view_module(kind);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| {
            panic!("a {kind:?} view lent to the host lasts for the call alone: {error:?}")
        });
        // A shared loan of the same view attenuates it for the call.
        let mut shared = view_module(kind);
        host_arguments(&mut shared)[0].access = StructuralAccess::SharedBorrow;
        shared.boundary_machines[0].structural_parameters[0].access =
            StructuralAccess::SharedBorrow;
        validate_module(&shared)
            .unwrap_or_else(|error| panic!("a {kind:?} view attenuates to shared: {error:?}"));
    }
}

#[test]
fn boundary_call_cannot_widen_a_shared_block_view() {
    for kind in VIEW_KINDS {
        let mut module = view_module(kind);
        let machine = &mut module.machines[0];
        machine.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        machine.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!("state 1 binds the view")
        };
        structural_arguments[0].access = StructuralAccess::SharedBorrow;
        if matches!(kind, ViewKind::Bytes) {
            module.structural_types[0].shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                });
        }
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::UnknownStructuralArgument {
                operation: operation_id(1),
                argument_index: 0,
                place: place_id(VIEW),
            }),
            "a shared {kind:?} view cannot supply an exclusive host loan",
        );
    }
}

#[test]
fn boundary_call_rejects_an_overlapping_second_loan_of_the_view() {
    for kind in VIEW_KINDS {
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
        ] {
            let module = with_second_host_loan(kind, lend(VIEW, access));
            assert_eq!(
                validate_module(&module).map(|_| ()),
                Err(ModuleError::OverlappingExclusiveStructuralArguments {
                    operation: operation_id(1),
                    first_argument: 0,
                    second_argument: 1,
                }),
                "an exclusive {kind:?} loan excludes a second {access:?} loan in one call",
            );
        }
    }
}

#[test]
fn boundary_call_rejects_a_loan_through_the_transferred_root() {
    // Edge 1 transferred the root's exclusive name into the view. Lending the
    // root beside the view would give the host two aliases of one referent,
    // and lending it alone would use a name the edge already gave away.
    for kind in VIEW_KINDS {
        let module = with_second_host_loan(kind, lend(ROOT, StructuralAccess::MutableBorrow));
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(stale_name(kind, ROOT)),
            "a {kind:?} view and its transferred root cannot both reach the host",
        );
        let mut module = view_module(kind);
        host_arguments(&mut module)[0].place = place_id(ROOT);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(stale_name(kind, ROOT)),
            "the {kind:?} root's exclusive name moved into the view",
        );
    }
}

#[test]
fn boundary_call_rejects_a_loan_of_a_moved_view() {
    // State 2 forwards its view to state 3 before the host call runs there.
    // The forwarding edge moves the exclusive name: state 3 may lend its own
    // parameter, never the old view name its predecessor gave away.
    for kind in VIEW_KINDS {
        let mut module = view_module(kind);
        let machine = &mut module.machines[0];
        let call = machine.blocks[1].operations.remove(0);
        machine.blocks[1].terminator =
            jump(3, 3, vec![lend(VIEW, StructuralAccess::MutableBorrow)]);
        let mut forwarded = state(
            3,
            vec![mutable_view(FORWARDED, 0)],
            Terminator::ReturnUnit {
                edge: edge_id(4),
                trivial_affine_discards: Vec::new(),
            },
        );
        forwarded.operations = vec![call];
        machine.blocks.push(forwarded);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(FORWARDED),
            kind: StructuralPlaceKind::BlockParameter {
                block: block_id(3),
                position: 0,
            },
        });
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut machine.blocks[2].operations[0].kind
        else {
            unreachable!("state 3 holds the host call")
        };
        structural_arguments[0].place = place_id(FORWARDED);
        validate_module(&module).unwrap_or_else(|error| {
            panic!("state 3 lends its own forwarded {kind:?} view: {error:?}")
        });
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[2].operations[0].kind
        else {
            unreachable!("state 3 holds the host call")
        };
        structural_arguments[0].place = place_id(VIEW);
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(stale_name(kind, VIEW)),
            "the {kind:?} view's name moved along edge 3",
        );
    }
}

#[test]
fn boundary_call_loan_cannot_escape_the_call() {
    for kind in VIEW_KINDS {
        // Lending the view as owned custody would let the host keep it.
        let mut owned = view_module(kind);
        host_arguments(&mut owned)[0].access = StructuralAccess::Owned;
        owned.boundary_machines[0].structural_parameters[0].access = StructuralAccess::Owned;
        assert_eq!(
            validate_module(&owned).map(|_| ()),
            Err(ModuleError::UnknownStructuralArgument {
                operation: operation_id(1),
                argument_index: 0,
                place: place_id(VIEW),
            }),
            "a borrowed {kind:?} view has no owned custody to hand the host",
        );
        // A host result that is itself a loan of the region would outlive
        // the call. Boundary results carry no reference-source contract, so
        // the declaration refuses to return one.
        let mut returned = view_module(kind);
        let reference = StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "test::RegionLoan".into(),
            shape: StructuralTypeShape::Reference {
                referent: structural_type_id(1),
                access: StructuralAccess::MutableBorrow,
            },
        };
        returned.structural_types.push(reference);
        returned.boundary_machines[0].result = terminal_psi::BoundaryMachineResult::Structural(
            terminal_psi::BoundaryStructuralResultDeclaration {
                structural_type: structural_type_id(2),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            },
        );
        assert_eq!(
            validate_module(&returned).map(|_| ()),
            Err(ModuleError::InvalidStructuralTypeIdentity(
                structural_type_id(2)
            )),
            "a host call cannot return a loan of the {kind:?} view",
        );
    }
}
