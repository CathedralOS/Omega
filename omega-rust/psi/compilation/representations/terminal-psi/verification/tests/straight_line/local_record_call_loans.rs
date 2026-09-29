//! A static machine's zero-established local record lends an exact field
//! subtree to a Unit callee. The loan moves no custody: the local keeps its
//! whole value and retires on the caller's own return. The loan is still an
//! ordinary argument, so its source, path type, liveness and overlap with a
//! sibling loan are each replayed here rather than trusted.

use super::{
    AdmissionProfile, Block, IntegerSign, IntegerType, MachineContract, ModuleError, Operation,
    OperationKind, OperationResult, ProofBundle, ScalarType, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, unit_module, validate_module, verify_module,
};
use semantic_vocabulary::PsiSemanticId;
use terminal_psi::{BindingRelevance, StructuralPathSegment};

const BANNER_TYPE: u64 = 1;
const SCREEN_TYPE: u64 = 2;
const LOCAL: u64 = 2;
const CALLEE: u64 = 901;
const CALLEE_BLOCK: u64 = 910;
const FIRST_FORMAL: u64 = 10;
const SECOND_FORMAL: u64 = 11;
const ESTABLISH: u64 = 1;
const LOAN_CALL: u64 = 3;

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn field(raw: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: id(raw),
        identity: identity.to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

/// `Banner { width: i32 }` and `Screen { top: Banner, bottom: Banner }`.
fn record_types() -> Vec<StructuralTypeDeclaration> {
    let width = StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
    ));
    vec![
        StructuralTypeDeclaration {
            id: id(BANNER_TYPE),
            identity: "test::Banner".to_owned(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(1, "width", width)],
            },
        },
        StructuralTypeDeclaration {
            id: id(SCREEN_TYPE),
            identity: "test::Screen".to_owned(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "top", StructuralFieldType::Structural(id(BANNER_TYPE))),
                    field(
                        2,
                        "bottom",
                        StructuralFieldType::Structural(id(BANNER_TYPE)),
                    ),
                ],
            },
        },
    ]
}

fn formal(place: u64, position: u32, access: StructuralAccess) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(place),
        position,
        is_self: false,
        structural_type: id(BANNER_TYPE),
        multiplicity: if access == StructuralAccess::Owned {
            StructuralMultiplicity::Affine
        } else {
            StructuralMultiplicity::Unrestricted
        },
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

/// One empty Unit callee whose formals take a `Banner` by the given accesses.
fn callee(accesses: &[StructuralAccess]) -> TerminalMachine {
    let formals = accesses
        .iter()
        .zip([FIRST_FORMAL, SECOND_FORMAL])
        .enumerate()
        .map(|(position, (access, place))| formal(place, position as u32, *access))
        .collect::<Vec<_>>();
    let owned = formals
        .iter()
        .filter(|formal| formal.access == StructuralAccess::Owned)
        .map(|formal| formal.place)
        .collect();
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id(CALLEE),
        attachment: None,
        structural_places: formals
            .iter()
            .map(|formal| StructuralPlaceDeclaration {
                id: formal.place,
                kind: StructuralPlaceKind::Parameter {
                    position: formal.position,
                    is_self: false,
                },
            })
            .collect(),
        structural_parameters: formals,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id(CALLEE_BLOCK),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(CALLEE_BLOCK),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(910),
                trivial_affine_discards: owned,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: id(910),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn loan(path: &[&str], access: StructuralAccess) -> StructuralArgument {
    StructuralArgument {
        place: id(LOCAL),
        path: path
            .iter()
            .map(|identity| StructuralPathSegment::Field((*identity).to_owned()))
            .collect(),
        access,
    }
}

/// `let mut screen: Screen; Banner::widen(&mut screen.top);` with the local
/// retired on the caller's return, exactly as stage 05 emits it.
fn local_loan_module(
    arguments: Vec<StructuralArgument>,
    formals: &[StructuralAccess],
) -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = record_types();
    let caller = &mut module.machines[0];
    caller.structural_places = vec![StructuralPlaceDeclaration {
        id: id(LOCAL),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(ESTABLISH),
            structural_type: id(SCREEN_TYPE),
        },
    }];
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(ESTABLISH),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: id(LOCAL),
                structural_type: id(SCREEN_TYPE),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Default::default(),
                projected_qualifications: Default::default(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishZeroedStructural {},
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(LOAN_CALL),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: id(CALLEE),
                arguments: Vec::new(),
                structural_arguments: arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(900),
        trivial_affine_discards: vec![id(LOCAL)],
    };
    module.machines.push(callee(formals));
    module
}

fn verified(module: &TerminalModule) -> bool {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .is_ok()
}

#[test]
fn a_local_record_lends_an_exact_field_subtree_to_a_unit_call() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let module = local_loan_module(vec![loan(&["top"], access)], &[access]);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{access:?} field loan of a local: {error:?}"));
    }
    // Disjoint sibling subtrees are two independent loans of one root.
    let exclusive = StructuralAccess::MutableBorrow;
    let module = local_loan_module(
        vec![loan(&["top"], exclusive), loan(&["bottom"], exclusive)],
        &[exclusive, exclusive],
    );
    assert!(verified(&module), "disjoint field loans of one local");
}

#[test]
fn a_local_record_loan_replays_source_path_liveness_and_overlap() {
    let exclusive = StructuralAccess::MutableBorrow;

    // Two exclusive loans of the same subtree alias one storage location.
    let overlapping = local_loan_module(
        vec![loan(&["top"], exclusive), loan(&["top"], exclusive)],
        &[exclusive, exclusive],
    );
    assert!(
        validate_module(&overlapping).is_err(),
        "one field lent exclusively twice"
    );

    // A loan of the whole local does not present its `Banner` field.
    let whole = local_loan_module(vec![loan(&[], exclusive)], &[exclusive]);
    assert!(
        validate_module(&whole).is_err(),
        "a local cannot shed its authored field projection"
    );

    // A projection moving a field out of the local is not a loan: it would
    // leave a residual the caller's return does not account for.
    let owned = StructuralAccess::Owned;
    let moved = local_loan_module(vec![loan(&["top"], owned)], &[owned]);
    assert!(
        validate_module(&moved).is_err(),
        "an owned field projection out of a local is refused"
    );

    // The local must be established before its storage is lent.
    let mut early = local_loan_module(vec![loan(&["top"], exclusive)], &[exclusive]);
    early.machines[0].blocks[0].operations.swap(0, 1);
    assert!(
        matches!(
            validate_module(&early),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
        ),
        "a loan before the local's establishment is refused"
    );
}
