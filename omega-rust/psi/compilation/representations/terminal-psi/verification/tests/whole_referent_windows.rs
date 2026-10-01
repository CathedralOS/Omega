//! Whole-referent replacement through an exclusive loan: `verdict = Sound`
//! where `verdict: &mut Verdict` is a root window. `MoveStructuralField` with
//! no field and an empty path vacates the whole referent, a same-typed value
//! is established, and `StoreStructuralField` reseats the root. These
//! fixtures forge the producer's side of that contract and check that the
//! verifier, not the producer, decides: the loan must be exclusive, the types
//! exact, the displaced value's disposal obligation must survive, and nothing
//! may observe the vacated root before the repair.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralCaseId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralArgument, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, VocabularyMarker,
};
use terminal_verifier::{ModuleError, validate_module};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn verdict() -> StructuralTypeId {
    id(1, StructuralTypeId::new)
}

fn cell() -> StructuralTypeId {
    id(2, StructuralTypeId::new)
}

fn sound() -> StructuralCaseId {
    id(2, StructuralCaseId::new)
}

/// The borrowed `&mut Verdict` parameter.
fn loan() -> PlaceId {
    id(1, PlaceId::new)
}

/// The displaced old referent `MoveStructuralField` produces.
fn displaced() -> PlaceId {
    id(2, PlaceId::new)
}

/// The established replacement value.
fn replacement() -> PlaceId {
    id(3, PlaceId::new)
}

fn move_operation() -> OperationId {
    id(1, OperationId::new)
}

fn establish_operation() -> OperationId {
    id(2, OperationId::new)
}

fn store_operation() -> OperationId {
    id(3, OperationId::new)
}

fn structural_types() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: verdict(),
            identity: "Verdict".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: id(1, StructuralCaseId::new),
                        identity: "Invalid".into(),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: sound(),
                        identity: "Sound".into(),
                        fields: Vec::new(),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: cell(),
            identity: "Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1, StructuralFieldId::new),
                    identity: "flag".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Boolean,
                    ),
                }],
            },
        },
    ]
}

fn structural_result(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type,
        multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

/// The root window opening: the whole referent of `loan` moves out.
fn vacate(multiplicity: StructuralMultiplicity) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: move_operation(),
        result: structural_result(displaced(), verdict(), multiplicity),
        kind: OperationKind::MoveStructuralField {
            source: loan(),
            path: Vec::new(),
            field: None,
        },
    }
}

/// `Verdict::Sound`, established whole.
fn establish_sound(multiplicity: StructuralMultiplicity) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: establish_operation(),
        result: structural_result(replacement(), verdict(), multiplicity),
        kind: OperationKind::EstablishStructuralCase {
            result_case: sound(),
            fields: Vec::new(),
        },
    }
}

/// The root repair: `value` reseats the whole referent of `loan`.
fn reseat(value: PlaceId) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: store_operation(),
        result: OperationResult::Unit,
        kind: OperationKind::StoreStructuralField {
            destination: loan(),
            path: Vec::new(),
            field: None,
            value: StructuralArgument {
                place: value,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        },
    }
}

/// `machine settle(verdict: &mut Verdict) { verdict = Verdict::Sound; }`
/// with the displaced and replacing referents at `multiplicity`. The loan
/// parameter's own multiplicity describes its carrier, which stays plain.
fn whole_referent_module(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id(1, MachineId::new),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: loan(),
            position: 0,
            is_self: false,
            structural_type: verdict(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: loan(),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: displaced(),
                kind: StructuralPlaceKind::OperationResult {
                    producer: move_operation(),
                    structural_type: verdict(),
                },
            },
            StructuralPlaceDeclaration {
                id: replacement(),
                kind: StructuralPlaceKind::OperationResult {
                    producer: establish_operation(),
                    structural_type: verdict(),
                },
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: id(1, BlockId::new),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(1, BlockId::new),
            parameters: Vec::new(),
            operations: vec![
                vacate(multiplicity),
                establish_sound(multiplicity),
                reseat(replacement()),
            ],
            terminator: Terminator::ReturnUnit {
                edge: id(1, EdgeId::new),
                trivial_affine_discards: if multiplicity == StructuralMultiplicity::Affine {
                    vec![displaced()]
                } else {
                    Vec::new()
                },
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: id(1, ContractId::new),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id(1, MachineId::new),
        structural_types: structural_types(),
        structural_domains: Vec::new(),
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
        machines: vec![machine],
    }
}

fn operations(module: &mut TerminalModule) -> &mut Vec<Operation> {
    &mut module.machines[0].blocks[0].operations
}

#[test]
fn a_root_window_replaces_the_whole_referent_of_an_exclusive_loan() {
    validate_module(&whole_referent_module(StructuralMultiplicity::Unrestricted))
        .expect("plain contents are displaced without a disposal obligation");
    validate_module(&whole_referent_module(StructuralMultiplicity::Affine))
        .expect("affine displaced contents are disposed on the returning edge");
}

#[test]
fn a_shared_loan_cannot_lend_a_root_window() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
        module.machines[0].structural_parameters[0].access = access;
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::InvalidBorrowedStorageWindow {
                operation: move_operation(),
                place: loan(),
            },
            "a {access:?} loan grants no whole-value replacement"
        );
        // Without the move, the bare repair is still a replacement through
        // the non-exclusive loan, never a restoration.
        operations(&mut module).remove(0);
        module.machines[0]
            .structural_places
            .retain(|place| place.id != displaced());
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::InvalidBorrowedStorageWindow {
                operation: store_operation(),
                place: loan(),
            },
            "a {access:?} loan grants no repair store"
        );
    }
}

#[test]
fn a_root_window_requires_the_referent_type_exactly() {
    // The repair value is a `Cell`, not the loan's `Verdict`.
    let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
    let establish = &mut operations(&mut module)[1];
    establish.result =
        structural_result(replacement(), cell(), StructuralMultiplicity::Unrestricted);
    establish.kind = OperationKind::EstablishRecord {
        fields: vec![terminal_psi::RecordFieldInitializer {
            field: id(1, StructuralFieldId::new),
            value: terminal_psi::RecordFieldValue::Scalar {
                value: id(1, semantic_vocabulary::ValueId::new),
                range_obligation: None,
            },
        }],
    };
    module.machines[0].structural_places[2].kind = StructuralPlaceKind::OperationResult {
        producer: establish_operation(),
        structural_type: cell(),
    };
    operations(&mut module).insert(
        1,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(4, OperationId::new),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
                id: id(1, semantic_vocabulary::ValueId::new),
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        },
    );
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: store_operation(),
            place: loan(),
        },
    );

    // The displaced value cannot be declared at another type either.
    let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
    operations(&mut module)[0].result =
        structural_result(displaced(), cell(), StructuralMultiplicity::Unrestricted);
    module.machines[0].structural_places[1].kind = StructuralPlaceKind::OperationResult {
        producer: move_operation(),
        structural_type: cell(),
    };
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: move_operation(),
            place: loan(),
        },
    );
}

#[test]
fn displaced_affine_contents_keep_their_disposal_obligation() {
    // Returning without disposing the displaced affine referent loses it.
    let mut module = whole_referent_module(StructuralMultiplicity::Affine);
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::UnitReturnAffineDiscardsMismatch {
            machine: id(1, MachineId::new),
            block: id(1, BlockId::new),
        },
        "an undisposed displaced affine referent must reject"
    );

    // Restoring the displaced value itself consumes it at the repair, so the
    // unused replacement is the one owing disposal.
    let mut module = whole_referent_module(StructuralMultiplicity::Affine);
    let store = operations(&mut module).pop().expect("repair");
    let OperationKind::StoreStructuralField { mut value, .. } = store.kind.clone() else {
        unreachable!()
    };
    value.place = displaced();
    operations(&mut module).push(Operation {
        kind: OperationKind::StoreStructuralField {
            destination: loan(),
            path: Vec::new(),
            field: None,
            value,
        },
        ..store
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![replacement()];
    validate_module(&module).expect("the displaced value repairs its own referent");

    // The repair consumed the displaced value: disposing it again rejects.
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![displaced(), replacement()];
    assert!(validate_module(&module).is_err());
}

#[test]
fn the_vacated_root_is_absent_until_its_repair() {
    // Observing the referent's case while the window is open reads absent
    // storage.
    let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
    operations(&mut module).insert(
        2,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(4, OperationId::new),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
                id: id(1, semantic_vocabulary::ValueId::new),
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            kind: OperationKind::StructuralCaseMembership {
                source: loan(),
                path: Vec::new(),
                case: sound(),
            },
        },
    );
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::BorrowedStorageFieldAbsent {
            operation: id(4, OperationId::new),
            place: loan(),
        },
    );

    // A return with the root still vacated owes its restoration.
    let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
    operations(&mut module).pop();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::BorrowedStorageRestorationPending {
            machine: id(1, MachineId::new),
            block: id(1, BlockId::new),
            place: loan(),
        },
        "an open root window cannot reach a return"
    );
}

#[test]
fn a_root_repair_without_a_vacated_root_is_no_restoration() {
    let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
    operations(&mut module).remove(0);
    module.machines[0]
        .structural_places
        .retain(|place| place.id != displaced());
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::BorrowedStorageRepairMismatch {
            operation: store_operation(),
            place: loan(),
        },
    );
}

#[test]
fn a_root_window_names_neither_a_projection_nor_an_owned_home() {
    // A fieldless window spells no path: the whole referent has no projection.
    let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
    let OperationKind::MoveStructuralField { path, .. } = &mut operations(&mut module)[0].kind
    else {
        unreachable!()
    };
    path.push(StructuralPathSegment::FixedIndex(0));
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: move_operation(),
            place: loan(),
        },
    );

    // An owned home's whole value moves by ordinary transfer, not a window.
    let mut module = whole_referent_module(StructuralMultiplicity::Unrestricted);
    module.machines[0].structural_parameters.clear();
    module.machines[0].structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: id(5, OperationId::new),
        structural_type: verdict(),
    };
    let mut home = establish_sound(StructuralMultiplicity::Unrestricted);
    home.id = id(5, OperationId::new);
    home.result = structural_result(loan(), verdict(), StructuralMultiplicity::Unrestricted);
    operations(&mut module).insert(0, home);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidBorrowedStorageWindow {
            operation: move_operation(),
            place: loan(),
        },
    );
}
