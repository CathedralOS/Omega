//! Record-shaped shared-borrow joins at block parameters. The successor lane
//! pins the exact referent root and projected field path the authored borrow
//! named; the joined view stays read-only, and the frontier walk proves the
//! loaned root is still held whole for the binding block's whole duration.

use super::{
    AdmissionProfile, Block, IntegerSign, IntegerType, MachineContract, ModuleError, Operation,
    OperationKind, OperationResult, ProofBundle, ScalarType, StructuralAccess, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, unit_module, validate_module, verify_module,
};
use semantic_vocabulary::PsiSemanticId;
use terminal_psi::{
    BindingRelevance, RecordFieldInitializer, RecordFieldValue, StructuralPathSegment,
};

const PAYLOAD_TYPE: u64 = 1;
const PAIR_TYPE: u64 = 2;
const TWIN_TYPE: u64 = 3;
const ROOT: u64 = 1;
const VIEW: u64 = 2;
const SECOND_VIEW: u64 = 3;
const RESULT_PLACE: u64 = 4;
const PAYLOAD_RESULT: u64 = 5;
const SECOND_PAYLOAD_RESULT: u64 = 6;
const PAIR_RESULT: u64 = 7;
const CALLEE_ROOT: u64 = 10;
const JOIN_BLOCK: u64 = 901;
const SIBLING_BLOCK: u64 = 902;
const CALLEE: u64 = 901;
const CALLEE_BLOCK: u64 = 910;
const INTEGER_VALUE: u64 = 50;
const FLAG_VALUE: u64 = 51;

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

fn field(raw: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: id(raw),
        identity: identity.to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn record_type(
    raw: u64,
    identity: &str,
    fields: Vec<StructuralFieldDeclaration>,
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: id(raw),
        identity: identity.to_owned(),
        shape: StructuralTypeShape::Record { fields },
    }
}

fn payload_type() -> StructuralTypeDeclaration {
    record_type(
        PAYLOAD_TYPE,
        "test::Payload",
        vec![
            field(
                1,
                "value",
                StructuralFieldType::Scalar(ScalarType::Integer(i32_type())),
            ),
            field(2, "flag", StructuralFieldType::Scalar(ScalarType::Boolean)),
        ],
    )
}

fn pair_type() -> StructuralTypeDeclaration {
    record_type(
        PAIR_TYPE,
        "test::Pair",
        vec![
            field(
                1,
                "first",
                StructuralFieldType::Structural(id(PAYLOAD_TYPE)),
            ),
            field(
                2,
                "second",
                StructuralFieldType::Structural(id(PAYLOAD_TYPE)),
            ),
        ],
    )
}

fn loan_argument(root: u64, path: &[&str]) -> StructuralArgument {
    StructuralArgument {
        place: id(root),
        path: path
            .iter()
            .map(|identity| StructuralPathSegment::Field((*identity).to_owned()))
            .collect(),
        access: StructuralAccess::SharedBorrow,
    }
}

fn shared_parameter(
    raw: u64,
    position: u32,
    structural_type: u64,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(raw),
        position,
        is_self: false,
        structural_type: id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn read_flag(operation: u64, source: u64, value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(value),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanStructuralField {
            source: id(source),
            path: Vec::new(),
            field: id(2),
        },
    }
}

fn return_unit(edge: u64, discards: &[u64]) -> Terminator {
    Terminator::ReturnUnit {
        edge: id(edge),
        trivial_affine_discards: discards.iter().map(|raw| id(*raw)).collect(),
    }
}

/// One machine whose owned `Pair` parameter loans `first` into a second block
/// that binds the projected `Payload` as a shared view and reads its flag.
fn pair_module(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![payload_type(), pair_type()];
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(ROOT),
            position: 0,
            is_self: false,
            structural_type: id(PAIR_TYPE),
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(VIEW),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(JOIN_BLOCK),
                position: 0,
            },
        },
    ];
    machine.blocks[0].terminator = Terminator::Jump {
        structural_arguments: vec![loan_argument(ROOT, &["first"])],
        edge: id(901),
        target: id(JOIN_BLOCK),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: vec![shared_parameter(VIEW, 0, PAYLOAD_TYPE)],
        id: id(JOIN_BLOCK),
        parameters: Vec::new(),
        operations: vec![read_flag(901, VIEW, 1)],
        terminator: return_unit(
            902,
            match multiplicity {
                StructuralMultiplicity::Affine => &[ROOT],
                _ => &[],
            },
        ),
    });
    module
}

fn argument(module: &mut TerminalModule) -> &mut StructuralArgument {
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    &mut structural_arguments[0]
}

fn view_parameter(module: &mut TerminalModule) -> &mut StructuralParameterDeclaration {
    &mut module.machines[0].blocks[1].structural_parameters[0]
}

#[test]
fn projected_shared_loan_joins_the_exact_view() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        let module = pair_module(multiplicity);
        validate_module(&module).unwrap_or_else(|error| panic!("{multiplicity:?}: {error}"));
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{multiplicity:?}: {error}"));
    }
}

/// Fixed byte storage and its bounded view have different type identities,
/// but the loan must preserve the exact element type and extent.
fn fixed_byte_loan_module() -> TerminalModule {
    let mut module = pair_module(StructuralMultiplicity::Unrestricted);
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(PAYLOAD_TYPE),
            identity: "test::Byte".to_owned(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
        StructuralTypeDeclaration {
            id: id(PAIR_TYPE),
            identity: "test::ByteArray".to_owned(),
            shape: StructuralTypeShape::FixedArray {
                element: id(PAYLOAD_TYPE),
                length: 48,
            },
        },
        StructuralTypeDeclaration {
            id: id(TWIN_TYPE),
            identity: "test::ByteView".to_owned(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedView {
                    access: Some(StructuralAccess::SharedBorrow),
                    capacity: 48,
                },
            ),
        },
    ];
    argument(&mut module).path.clear();
    view_parameter(&mut module).structural_type = id(TWIN_TYPE);
    module.machines[0].blocks[1].operations.clear();
    module
}

#[test]
fn shared_fixed_byte_loan_preserves_backing_extent() {
    let module = fixed_byte_loan_module();
    assert_eq!(
        terminal_semantics::borrowed_referent_presentation(
            module.structural_types.iter(), id(PAIR_TYPE), id(TWIN_TYPE)),
        Some(terminal_semantics::BorrowedReferentPresentation::AddressOfBacking { byte_count: 48 }),
    );
    assert_eq!(
        terminal_semantics::borrowed_referent_presentation(
            module.structural_types.iter(), id(TWIN_TYPE), id(TWIN_TYPE)),
        Some(terminal_semantics::BorrowedReferentPresentation::Unchanged),
    );
    validate_module(&module).expect("the exact whole fixed array backs its shared view");
    let mut local = module.clone();
    local.machines[0].structural_parameters.clear();
    local.machines[0].structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: id(910),
        structural_type: id(PAIR_TYPE),
    };
    local.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(910),
        result: structural_result(ROOT, PAIR_TYPE),
        kind: OperationKind::EstablishZeroedStructural {},
    });
    validate_module(&local).expect("the local owned array supplies the same exact loan");
    let mut forwarded = local.clone();
    let machine = &mut forwarded.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(SECOND_VIEW),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(SIBLING_BLOCK),
            position: 0,
        },
    });
    let end = machine.blocks[1].terminator.clone();
    machine.blocks[1].terminator = Terminator::Jump {
        edge: id(903),
        target: id(SIBLING_BLOCK),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![loan_argument(VIEW, &[])],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: vec![shared_parameter(SECOND_VIEW, 0, TWIN_TYPE)],
        id: id(SIBLING_BLOCK),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: end,
    });
    validate_module(&forwarded).expect("the bounded loan forwards without changing its backing");
    // A declaration alone cannot authorize reading an unestablished local.
    local.machines[0].blocks[0].operations.clear();
    assert!(validate_module(&local).is_err());
    for mutation in 0..5 {
        let mut invalid = module.clone();
        match mutation {
            0 => {
                invalid.structural_types[1].shape = StructuralTypeShape::FixedArray {
                    element: id(PAYLOAD_TYPE),
                    length: 47,
                }
            }
            1 => {
                invalid.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
                )
            }
            2 => argument(&mut invalid).access = StructuralAccess::MutableBorrow,
            3 => {
                invalid.machines[0].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }
            4 => argument(&mut invalid).place = id(SECOND_VIEW),
            _ => unreachable!(),
        }
        assert!(validate_module(&invalid).is_err(), "mutation {mutation}");
    }
}

#[test]
fn shared_loan_rejects_substituted_access_path_root_or_signature() {
    for mutation in 0..18 {
        let mut module = pair_module(StructuralMultiplicity::Unrestricted);
        match mutation {
            // An owned, exclusive, or write-only presentation cannot satisfy
            // the shared loan even when root and path are exact.
            0 => argument(&mut module).access = StructuralAccess::Owned,
            1 => argument(&mut module).access = StructuralAccess::MutableBorrow,
            2 => argument(&mut module).access = StructuralAccess::WriteOnlyBorrow,
            // A whole root, a missing field, an index segment, or an over-long
            // projection diverges from the borrow's authored path.
            3 => argument(&mut module).path = Vec::new(),
            4 => {
                argument(&mut module).path =
                    vec![StructuralPathSegment::Field("missing".to_owned())]
            }
            5 => {
                argument(&mut module).path = vec![
                    StructuralPathSegment::Field("first".to_owned()),
                    StructuralPathSegment::FixedIndex(0),
                ]
            }
            6 => {
                argument(&mut module).path = vec![
                    StructuralPathSegment::Field("first".to_owned()),
                    StructuralPathSegment::Field("value".to_owned()),
                ]
            }
            // An unknown place or another block's parameter cannot supply the
            // view: neither dominates this edge.
            7 => argument(&mut module).place = id(99),
            8 => argument(&mut module).place = id(VIEW),
            // The joined parameter itself cannot carry affine or linear
            // custody, and no other access mode reads as shared.
            9 => view_parameter(&mut module).multiplicity = StructuralMultiplicity::Affine,
            10 => view_parameter(&mut module).multiplicity = StructuralMultiplicity::Linear,
            11 => view_parameter(&mut module).access = StructuralAccess::Owned,
            12 => view_parameter(&mut module).access = StructuralAccess::MutableBorrow,
            13 => view_parameter(&mut module).access = StructuralAccess::WriteOnlyBorrow,
            // Identity is not layout: an equal-shape record under another type
            // identity is a different referent type.
            14 => {
                module.structural_types.push(record_type(
                    TWIN_TYPE,
                    "test::Twin",
                    vec![
                        field(
                            1,
                            "value",
                            StructuralFieldType::Scalar(ScalarType::Integer(i32_type())),
                        ),
                        field(2, "flag", StructuralFieldType::Scalar(ScalarType::Boolean)),
                    ],
                ));
                view_parameter(&mut module).structural_type = id(TWIN_TYPE);
            }
            // A self-rooted or machine-parameter-shaped view place breaks the
            // parameter's own placement evidence.
            15 => view_parameter(&mut module).is_self = true,
            16 => {
                module.machines[0].structural_places[1].kind = StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                }
            }
            // Arity stays exact: one parameter, one argument.
            17 => {
                let Terminator::Jump {
                    structural_arguments,
                    ..
                } = &mut module.machines[0].blocks[0].terminator
                else {
                    unreachable!()
                };
                structural_arguments.push(loan_argument(ROOT, &["second"]));
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&module).is_err(), "mutation {mutation}");
    }
}

#[test]
fn shared_loan_rejects_non_dominating_sibling_roots() {
    let mut module = pair_module(StructuralMultiplicity::Unrestricted);
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(FLAG_VALUE),
        scalar_type: ScalarType::Boolean,
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(SECOND_VIEW),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(SIBLING_BLOCK),
            position: 0,
        },
    });
    // The false edge asks for a view rooted at the true edge's own parameter:
    // place `VIEW` never dominates block 900's outgoing edges.
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(FLAG_VALUE),
        when_true: SuccessorEdge {
            structural_arguments: vec![loan_argument(ROOT, &["first"])],
            edge: id(903),
            target: id(JOIN_BLOCK),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            structural_arguments: vec![loan_argument(VIEW, &[])],
            edge: id(904),
            target: id(SIBLING_BLOCK),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: vec![shared_parameter(SECOND_VIEW, 0, PAYLOAD_TYPE)],
        id: id(SIBLING_BLOCK),
        parameters: Vec::new(),
        operations: vec![read_flag(902, SECOND_VIEW, 2)],
        terminator: return_unit(905, &[]),
    });
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::InvalidStructuralSuccessorArgument { place, .. })
                if place == id::<semantic_vocabulary::PlaceId>(VIEW)
        ),
        "a view rooted at a sibling block's parameter cannot cross this edge"
    );
    // Repointing the false edge at the machine root's other field keeps both
    // joins legal and exercises the conditional successor lane.
    let Terminator::Conditional { when_false, .. } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_false.structural_arguments[0] = loan_argument(ROOT, &["second"]);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("each arm joins its own exact projected view");
}

#[test]
fn shared_loan_keeps_the_root_whole_for_the_binding_block() {
    // Retiring the loaned root on the very edge that binds the view fails:
    // the loan itself moves no custody but still requires a held root.
    let mut module = pair_module(StructuralMultiplicity::Affine);
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![id(ROOT)];
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::InvalidStructuralSuccessorArgument { place, .. })
                if place == id::<semantic_vocabulary::PlaceId>(ROOT)
        ),
        "a shared loan cannot bind a root the same edge retired"
    );

    // Writing through the loaned root while the view observes it disturbs the
    // pin, even though the store's own access authority is otherwise legal.
    let mut module = pair_module(StructuralMultiplicity::Affine);
    module.machines[0].parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(INTEGER_VALUE),
        scalar_type: ScalarType::Integer(i32_type()),
    });
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(903),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: id(ROOT),
            path: vec![StructuralPathSegment::Field("first".to_owned())],
            field: id(1),
            value: id(INTEGER_VALUE),
            range_obligation: None,
        },
    });
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::SharedStructuralLoanDisturbed { place, .. })
                if place == id::<semantic_vocabulary::PlaceId>(ROOT)
        ),
        "a store through the pinned root must fail"
    );

    // Moving the pinned root into a call, or reborrowing it exclusively,
    // disturbs the same pin. The moving call needs an affine source feeding
    // an affine parameter; an exclusive reborrow only validates against an
    // unrestricted source.
    for (multiplicity, access) in [
        (StructuralMultiplicity::Affine, StructuralAccess::Owned),
        (
            StructuralMultiplicity::Unrestricted,
            StructuralAccess::MutableBorrow,
        ),
    ] {
        let mut module = pair_module(multiplicity);
        module.machines.push(consumer_machine(access));
        module.machines[0].blocks[1].operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(904),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: id(CALLEE),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: id(ROOT),
                    path: Vec::new(),
                    access,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        let outcome = validate_module(&module);
        assert!(
            matches!(
                outcome,
                Err(ModuleError::SharedStructuralLoanDisturbed { place, .. })
                    if place == id::<semantic_vocabulary::PlaceId>(ROOT)
            ),
            "{access:?} call argument through the pinned root must fail, got {outcome:?}"
        );
    }
}

/// One Unit callee taking the pair by the requested access; the affine row
/// moves custody so the caller's pinned root would be consumed.
fn consumer_machine(access: StructuralAccess) -> TerminalMachine {
    let affine = access == StructuralAccess::Owned;
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id(CALLEE),
        attachment: None,
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id(CALLEE_ROOT),
            position: 0,
            is_self: false,
            structural_type: id(PAIR_TYPE),
            multiplicity: if affine {
                StructuralMultiplicity::Affine
            } else {
                StructuralMultiplicity::Unrestricted
            },
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: id(CALLEE_ROOT),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
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
            terminator: return_unit(910, if affine { &[CALLEE_ROOT] } else { &[] }),
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

/// Rebind the original pair, then lend its first field at an ordinary call.
/// The block name retains the incoming authority and the call keeps its path.
fn block_field_call_module(
    source_access: StructuralAccess,
    call_access: StructuralAccess,
) -> TerminalModule {
    let mut module = pair_module(StructuralMultiplicity::Unrestricted);
    module.machines[0].structural_parameters[0].access = source_access;
    *argument(&mut module) = StructuralArgument {
        place: id(ROOT),
        path: Vec::new(),
        access: source_access,
    };
    let parameter = view_parameter(&mut module);
    parameter.structural_type = id(PAIR_TYPE);
    parameter.access = source_access;
    let mut source = loan_argument(VIEW, &["first"]);
    source.access = call_access;
    module.machines[0].blocks[1].operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(904),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: id(CALLEE),
            arguments: Vec::new(),
            structural_arguments: vec![source],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    let mut consumer = consumer_machine(call_access);
    consumer.structural_parameters[0].structural_type = id(PAYLOAD_TYPE);
    consumer.blocks[0].operations = vec![read_flag(905, CALLEE_ROOT, FLAG_VALUE)];
    module.machines.push(consumer);
    module
}

#[test]
fn borrowed_block_parameters_lend_static_fields_at_calls() {
    for (source_access, call_access) in [
        (
            StructuralAccess::SharedBorrow,
            StructuralAccess::SharedBorrow,
        ),
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
        ),
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::MutableBorrow,
        ),
    ] {
        let module = block_field_call_module(source_access, call_access);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{source_access:?} to {call_access:?}: {error}"));
    }
}

#[test]
fn block_field_calls_reject_substituted_projected_type() {
    let mut module = block_field_call_module(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
    );
    module.machines[1].structural_parameters[0].structural_type = id(PAIR_TYPE);
    module.machines[1].blocks[0].operations.clear();
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::StructuralArgumentTypeMismatch { operation, argument_index: 0, .. })
            if operation == id(904)
    ));
}

#[test]
fn block_field_calls_reject_shared_to_mutable_escalation() {
    let module = block_field_call_module(
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    );
    assert!(validate_module(&module).is_err());
}

#[test]
fn block_field_calls_reject_invalid_parameter_position() {
    let mut module = block_field_call_module(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
    );
    module.machines[0].structural_places[1].kind = StructuralPlaceKind::BlockParameter {
        block: id(JOIN_BLOCK),
        position: 1,
    };
    assert!(validate_module(&module).is_err());
}

#[test]
fn block_field_calls_reject_unavailable_parameter() {
    let mut module = block_field_call_module(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
    );
    // A valid declaration is not availability: the entry block cannot use
    // the name its successor will bind later.
    let operations = std::mem::take(&mut module.machines[0].blocks[1].operations);
    module.machines[0].blocks[0].operations = operations;
    assert!(validate_module(&module).is_err());
}

#[test]
fn block_field_calls_reject_overlapping_exclusive_arguments() {
    let mut module = block_field_call_module(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
    );
    let consumer = &mut module.machines[1];
    let mut parameter = consumer.structural_parameters[0].clone();
    parameter.place = id(CALLEE_ROOT + 1);
    parameter.position = 1;
    consumer.structural_places.push(StructuralPlaceDeclaration {
        id: parameter.place,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    consumer.structural_parameters.push(parameter);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[1].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments.push(structural_arguments[0].clone());
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OverlappingExclusiveStructuralArguments {
            operation, first_argument: 0, second_argument: 1,
        }) if operation == id(904)
    ));
}

#[test]
fn joined_shared_view_stays_read_only() {
    // Storing through the joined view fails its access check: a shared loan
    // never carries write authority even when the field path resolves.
    let mut module = pair_module(StructuralMultiplicity::Unrestricted);
    module.machines[0].parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(INTEGER_VALUE),
        scalar_type: ScalarType::Integer(i32_type()),
    });
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(903),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: id(VIEW),
            path: Vec::new(),
            field: id(1),
            value: id(INTEGER_VALUE),
            range_obligation: None,
        },
    });
    assert!(
        validate_module(&module).is_err(),
        "a shared view cannot be a store destination"
    );

    // Returning the shared view as the machine's owned result is rejected:
    // the bound view never carries owned custody to return.
    let mut module = pair_module(StructuralMultiplicity::Unrestricted);
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: id(RESULT_PLACE),
        structural_type: id(PAYLOAD_TYPE),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(RESULT_PLACE),
        kind: StructuralPlaceKind::Result,
    });
    machine.blocks[1].terminator = Terminator::ReturnStructural {
        edge: id(903),
        source: id(VIEW),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    assert!(
        validate_module(&module).is_err(),
        "a shared view cannot satisfy an owned structural result"
    );
}

#[test]
fn shared_loan_joins_from_a_completed_result_root() {
    let mut module = pair_module(StructuralMultiplicity::Unrestricted);
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(INTEGER_VALUE),
            scalar_type: ScalarType::Integer(i32_type()),
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(FLAG_VALUE),
            scalar_type: ScalarType::Boolean,
        },
    ];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(PAYLOAD_RESULT),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(910),
                structural_type: id(PAYLOAD_TYPE),
            },
        },
        StructuralPlaceDeclaration {
            id: id(SECOND_PAYLOAD_RESULT),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(911),
                structural_type: id(PAYLOAD_TYPE),
            },
        },
        StructuralPlaceDeclaration {
            id: id(PAIR_RESULT),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(912),
                structural_type: id(PAIR_TYPE),
            },
        },
        StructuralPlaceDeclaration {
            id: id(VIEW),
            kind: StructuralPlaceKind::BlockParameter {
                block: id(JOIN_BLOCK),
                position: 0,
            },
        },
    ];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(910),
            result: structural_result(PAYLOAD_RESULT, PAYLOAD_TYPE),
            kind: OperationKind::EstablishRecord {
                fields: vec![
                    RecordFieldInitializer {
                        field: id(1),
                        value: RecordFieldValue::Scalar {
                            value: id(INTEGER_VALUE),
                            range_obligation: None,
                        },
                    },
                    RecordFieldInitializer {
                        field: id(2),
                        value: RecordFieldValue::Scalar {
                            value: id(FLAG_VALUE),
                            range_obligation: None,
                        },
                    },
                ],
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(911),
            result: structural_result(SECOND_PAYLOAD_RESULT, PAYLOAD_TYPE),
            kind: OperationKind::EstablishRecord {
                fields: vec![
                    RecordFieldInitializer {
                        field: id(1),
                        value: RecordFieldValue::Scalar {
                            value: id(INTEGER_VALUE),
                            range_obligation: None,
                        },
                    },
                    RecordFieldInitializer {
                        field: id(2),
                        value: RecordFieldValue::Scalar {
                            value: id(FLAG_VALUE),
                            range_obligation: None,
                        },
                    },
                ],
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(912),
            result: structural_result(PAIR_RESULT, PAIR_TYPE),
            kind: OperationKind::EstablishRecord {
                fields: vec![
                    RecordFieldInitializer {
                        field: id(1),
                        value: RecordFieldValue::Structural(StructuralArgument {
                            place: id(PAYLOAD_RESULT),
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        }),
                    },
                    RecordFieldInitializer {
                        field: id(2),
                        value: RecordFieldValue::Structural(StructuralArgument {
                            place: id(SECOND_PAYLOAD_RESULT),
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        }),
                    },
                ],
            },
        },
    ];
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0] = loan_argument(PAIR_RESULT, &["first"]);
    machine.blocks[1].terminator = return_unit(902, &[]);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a completed unrestricted record result supplies a shared loan");

    // The whole result root carries the pair type, not the projected payload:
    // dropping the authored field segment still cannot join.
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0] = loan_argument(PAIR_RESULT, &[]);
    assert!(
        validate_module(&module).is_err(),
        "a result root cannot shed its authored projection"
    );
}

fn structural_result(place: u64, structural_type: u64) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: id(place),
        structural_type: id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Default::default(),
        projected_qualifications: Default::default(),
        claims: Vec::new(),
    })
}

#[test]
fn shared_state_loans_protect_only_their_regions_through_forwarding() {
    for forwarded in [false, true] {
        for written_field in ["first", "second"] {
            let mut module = pair_module(StructuralMultiplicity::Affine);
            let machine = &mut module.machines[0];
            machine.parameters.push(ValueDeclaration {
                qualifications: Default::default(),
                id: id(INTEGER_VALUE),
                scalar_type: ScalarType::Integer(i32_type()),
            });
            if forwarded {
                machine.structural_places.push(StructuralPlaceDeclaration {
                    id: id(SECOND_VIEW),
                    kind: StructuralPlaceKind::BlockParameter {
                        block: id(SIBLING_BLOCK),
                        position: 0,
                    },
                });
                let end = machine.blocks[1].terminator.clone();
                machine.blocks[1].terminator = Terminator::Jump {
                    edge: id(903),
                    target: id(SIBLING_BLOCK),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: vec![loan_argument(VIEW, &[])],
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                };
                machine.blocks.push(Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: vec![shared_parameter(SECOND_VIEW, 0, PAYLOAD_TYPE)],
                    id: id(SIBLING_BLOCK),
                    parameters: Vec::new(),
                    operations: vec![read_flag(902, SECOND_VIEW, 2)],
                    terminator: end,
                });
            }
            machine.blocks.last_mut().unwrap().operations.insert(
                0,
                Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: id(904),
                    result: OperationResult::Unit,
                    kind: OperationKind::StructuralScalarFieldStore {
                        destination: id(ROOT),
                        path: vec![StructuralPathSegment::Field(written_field.to_owned())],
                        field: id(1),
                        value: id(INTEGER_VALUE),
                        range_obligation: None,
                    },
                },
            );
            let result = verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            );
            if written_field == "second" {
                result.unwrap_or_else(|error| {
                    panic!("disjoint field, forwarded={forwarded}: {error:?}")
                });
            } else {
                assert!(
                    matches!(
                        validate_module(&module),
                        Err(ModuleError::SharedStructuralLoanDisturbed { .. })
                    ),
                    "loaned field must stay pinned, forwarded={forwarded}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn a_shared_state_join_protects_every_possible_incoming_field() {
    let mut module = pair_module(StructuralMultiplicity::Affine);
    let machine = &mut module.machines[0];
    machine.parameters.extend([
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(FLAG_VALUE),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(INTEGER_VALUE),
            scalar_type: ScalarType::Integer(i32_type()),
        },
    ]);
    let edge = |identity, field| SuccessorEdge {
        edge: id(identity),
        target: id(JOIN_BLOCK),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![loan_argument(ROOT, &[field])],
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(FLAG_VALUE),
        when_true: edge(901, "first"),
        when_false: edge(903, "second"),
    };
    machine.blocks[1].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(904),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: id(ROOT),
                path: vec![StructuralPathSegment::Field("second".to_owned())],
                field: id(1),
                value: id(INTEGER_VALUE),
                range_obligation: None,
            },
        },
    );
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::SharedStructuralLoanDisturbed { .. })
    ));
}
