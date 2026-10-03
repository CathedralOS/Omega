//! Stored descriptors retain their exact backing through field replacement
//! and ordinary calls. Predicate evidence and lifetime authority are separate:
//! valid storage transport cannot invent a declared byte predicate.
use super::{
    AdmissionProfile, IntegerSign, IntegerType, ModuleError, Operation, OperationKind,
    OperationResult, ProofBundle, ScalarType, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, ValueDeclaration, unit_module, validate_module,
    verify_module,
};
use semantic_vocabulary::PsiSemanticId;
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet,
};

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn utf8() -> ByteSequencePredicateSet {
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    predicates
}

fn shared_view() -> ByteSequenceCarrier {
    ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    }
}

fn store(destination: u64, source: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(2),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralByteViewFieldStore {
            destination: id(destination),
            path: Vec::new(),
            field: id(1),
            source: id(source),
            predicate_obligation: None,
        },
    }
}

/// `place(1)`: an exclusive `Room { label: &[u8] in Utf8 }` parameter.
/// `place(2)`: an established literal `"Gate"`; `place(3)`: a shared view
/// parameter. The store replaces `label` with the literal.
fn fixture() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::Room".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "label".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::PredicateByteSequence {
                        carrier: shared_view(),
                        predicates: utf8(),
                    },
                }],
            },
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::View".into(),
            shape: StructuralTypeShape::ByteSequence(shared_view()),
        },
    ];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![
        StructuralParameterDeclaration {
            place: id(1),
            position: 0,
            is_self: false,
            structural_type: id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            place: id(3),
            position: 1,
            is_self: false,
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    ];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(2),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: id(2),
            },
        },
        StructuralPlaceDeclaration {
            id: id(3),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
    ];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(1),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: id(2),
                bytes: b"Gate".to_vec(),
                qualifications: Vec::new(),
            },
        },
        store(1, 2),
    ];
    module
}

fn rejects(module: &TerminalModule) {
    assert!(
        matches!(
            validate_module(module),
            Err(ModuleError::InvalidStructuralByteViewFieldStore(operation)) if operation == id(2)
        ),
        "{:?}",
        validate_module(module)
    );
}

#[test]
fn literal_source_replaces_a_qualified_shared_view_field() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = fixture();
        module.machines[0].structural_parameters[0].access = access;
        validate_module(&module).unwrap();
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn an_unqualified_parameter_cannot_invent_the_destination_predicate() {
    let mut module = fixture();
    module.machines[0].blocks[0].operations[1] = store(1, 3);
    rejects(&module);
}

#[test]
fn a_parameter_view_is_retained_by_the_exact_plain_destination_field() {
    let mut module = fixture();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::ByteSequence(shared_view());
    module.machines[0].blocks[0].operations[1] = store(1, 3);
    validate_module(&module)
        .expect("a stored view borrows the incoming backing, not the carrier home");
    let mut unreadable = module;
    unreadable.machines[0].structural_parameters[1].access = StructuralAccess::WriteOnlyBorrow;
    rejects(&unreadable);
}

#[test]
fn literal_octets_must_imply_the_field_predicates() {
    let mut module = fixture();
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *bytes = vec![b'G', 0xff];
    rejects(&module);
    // A qualification roster cannot stand in for the octets.
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::ByteSequence(shared_view());
    validate_module(&module).expect("a plain view field imposes no predicate");
}

#[test]
fn a_shared_destination_cannot_replace_its_field() {
    let mut module = fixture();
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    rejects(&module);
}

#[test]
fn only_a_shared_view_field_holds_a_relocatable_descriptor() {
    for carrier in [
        ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        },
        ByteSequenceCarrier::BoundedOwned { capacity: 8 },
    ] {
        let mut module = fixture();
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
            unreachable!()
        };
        fields[0].field_type = StructuralFieldType::ByteSequence(carrier);
        rejects(&module);
    }
}

#[test]
fn the_destination_cannot_be_the_source_or_a_forged_place() {
    let mut module = fixture();
    module.machines[0].blocks[0].operations[1] = store(2, 2);
    assert!(validate_module(&module).is_err());
    let mut module = fixture();
    module.machines[0].blocks[0].operations[1].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(1),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    });
    assert!(validate_module(&module).is_err());
}

#[test]
fn a_store_invalidates_an_earlier_field_length_observation() {
    let mut module = fixture();
    let length = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(3),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(1),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::StructuralByteSequenceFieldLength {
            source: id(1),
            path: Vec::new(),
            field: id(1),
        },
    };
    let index = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(4),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(2),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        },
    };
    let read = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(5),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(3),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        }),
        kind: OperationKind::StructuralByteSequenceFieldRead {
            source: id(1),
            path: Vec::new(),
            field: id(1),
            index: id(2),
            length: id(1),
            obligation: id(1),
        },
    };
    // Observing the stored field after the replacement is current.
    let operations = &mut module.machines[0].blocks[0].operations;
    operations.extend([length.clone(), index.clone(), read.clone()]);
    validate_module(&module).expect("a qualified shared view field reads like a plain one");
    // The same observation taken before the replacement is stale.
    let mut stale = fixture();
    let operations = &mut stale.machines[0].blocks[0].operations;
    operations.insert(1, length);
    operations.extend([index, read]);
    assert!(matches!(
        validate_module(&stale),
        Err(ModuleError::InvalidStructuralByteSequenceFieldAccess(operation)) if operation == id(5)
    ));
}

fn view_result(
    place: u64,
    producer: u64,
    structural_type: u64,
) -> (StructuralPlaceDeclaration, OperationResult) {
    (
        StructuralPlaceDeclaration {
            id: id(place),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(producer),
                structural_type: id(structural_type),
            },
        },
        OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: id(place),
            structural_type: id(structural_type),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
            qualification_establishments: vec![],
            claims: vec![],
        }),
    )
}

fn scalar_operation(identity: u64, bits: u16, kind: OperationKind) -> Operation {
    Operation {
        id: id(identity),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Scalar(ValueDeclaration {
            id: id(identity),
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
            ),
            qualifications: Default::default(),
        }),
        kind,
    }
}

/// The array is an independent mutable ingress. Forming its shared view then
/// storing that view must pin its backing for the field's later observation.
fn borrowed_backing_fixture() -> TerminalModule {
    let mut module = fixture();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::ByteSequence(shared_view());
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: id(3),
            identity: "test::Bytes".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(4),
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: id(4),
            identity: "test::Byte".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
    ]);
    let machine = &mut module.machines[0];
    machine.structural_parameters[1].place = id(4);
    machine.structural_parameters[1].structural_type = id(3);
    machine.structural_parameters[1].access = StructuralAccess::MutableBorrow;
    machine.structural_places[2].id = id(4);
    let (place, result) = view_result(3, 3, 2);
    machine.structural_places.push(place);
    machine.blocks[0].operations = vec![
        machine.blocks[0].operations[0].clone(),
        Operation {
            id: id(3),
            static_reach_binding: None,
            suspension_crossing: None,
            result,
            kind: OperationKind::EstablishElementView {
                region: terminal_psi::ElementViewRegion::WholeCollection,
                source: terminal_psi::StructuralArgument {
                    place: id(4),
                    path: vec![],
                    access: StructuralAccess::SharedBorrow,
                },
                destination: id(3),
                element: id(4),
            },
        },
        store(1, 3),
        scalar_operation(
            4,
            64,
            OperationKind::StructuralByteSequenceFieldLength {
                source: id(1),
                path: vec![],
                field: id(1),
            },
        ),
    ];
    module
}

fn backing_write() -> [Operation; 2] {
    [
        scalar_operation(
            6,
            8,
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
            },
        ),
        Operation {
            id: id(5),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: id(4),
                path: vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
                value: id(6),
            },
        },
    ]
}

#[test]
fn stored_view_pins_backing_until_exact_field_replacement() {
    let valid = borrowed_backing_fixture();
    validate_module(&valid).expect("a caller may observe its stored borrowed view");
    let mut overlapping = valid.clone();
    overlapping.machines[0].blocks[0]
        .operations
        .splice(3..3, backing_write());
    assert!(
        matches!(
            validate_module(&overlapping),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "{:?}",
        validate_module(&overlapping)
    );

    let mut replaced = valid;
    let mut replacement = store(1, 2);
    replacement.id = id(7);
    replaced.machines[0].blocks[0]
        .operations
        .splice(3..3, [replacement].into_iter().chain(backing_write()));
    validate_module(&replaced)
        .expect("replacing the field ends that field's old borrow before the write");
}

#[test]
fn copying_a_stored_view_keeps_the_old_backing_after_field_replacement() {
    let mut module = borrowed_backing_fixture();
    // A leaf copy mints the standalone descriptor type; field access remains
    // independently authenticated at its source.
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(5),
        identity: "test::CopiedView".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: None,
        }),
    });
    let (place, result) = view_result(5, 8, 5);
    module.machines[0].structural_places.push(place);
    let copy = Operation {
        id: id(8),
        static_reach_binding: None,
        suspension_crossing: None,
        result,
        kind: OperationKind::StructuralLeafCopy {
            source: id(1),
            path: vec![terminal_psi::StructuralPathSegment::Field("label".into())],
        },
    };
    let mut replacement = store(1, 2);
    replacement.id = id(7);
    module.machines[0].blocks[0]
        .operations
        .splice(3..3, [copy, replacement]);
    module.machines[0].blocks[0]
        .operations
        .push(scalar_operation(
            9,
            64,
            OperationKind::ByteSequenceLength { source: id(5) },
        ));
    validate_module(&module).expect("copied descriptor survives a field overwrite");
    module.machines[0].blocks[0]
        .operations
        .splice(5..5, backing_write());
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "{:?}",
        validate_module(&module)
    );
}

#[test]
fn stored_view_predicate_obligation_replays_the_exact_current_source() {
    use semantic_vocabulary::{Proposition, ScalarTerm};
    use terminal_psi::{
        CertificateEnvelope, EvidenceRoute, ObligationEvidence, ProofNode, ProofRule,
        ProofSystemMarker,
    };
    let mut module = fixture();
    module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::ByteSequencePredicate {
                predicate: ByteSequencePredicate::ValidUtf8,
                root: id(3),
                path: vec![],
            },
            ScalarTerm::Boolean(true),
        ));
    let mut replacement = store(1, 3);
    let OperationKind::StructuralByteViewFieldStore {
        predicate_obligation,
        ..
    } = &mut replacement.kind
    else {
        unreachable!()
    };
    *predicate_obligation = Some(id(9));
    module.machines[0].blocks[0].operations[1] = replacement;
    validate_module(&module).expect("the proof question is separate from lifetime/type admission");
    let question = terminal_verifier::reconstruct_operation_obligations(&module)
        .unwrap()
        .into_iter()
        .find(|question| question.obligation.id == id(9))
        .unwrap();
    let certificate = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: id(9),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(9),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: question.obligation.proposition,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    verify_module(&module, &certificate, &AdmissionProfile::default())
        .expect("exact source predicate requirement proves the stored field invariant");

    let mut missing = module.clone();
    let OperationKind::StructuralByteViewFieldStore {
        predicate_obligation,
        ..
    } = &mut missing.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *predicate_obligation = None;
    rejects(&missing);

    let mut wrong_source = module.clone();
    let mut parameter = wrong_source.machines[0].structural_parameters[1].clone();
    parameter.place = id(4);
    parameter.position = 2;
    wrong_source.machines[0]
        .structural_parameters
        .push(parameter);
    wrong_source.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: id(4),
            kind: StructuralPlaceKind::Parameter {
                position: 2,
                is_self: false,
            },
        });
    let OperationKind::StructuralByteViewFieldStore { source, .. } =
        &mut wrong_source.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *source = id(4);
    validate_module(&wrong_source)
        .expect("same-typed alternate input still has valid lifetime and custody");
    assert!(
        verify_module(&wrong_source, &certificate, &AdmissionProfile::default()).is_err(),
        "one input's verdict cannot certify another input"
    );

    let mut wrong_predicate = module;
    let Proposition::Equal(ScalarTerm::ByteSequencePredicate { predicate, .. }, _) =
        &mut wrong_predicate.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *predicate = ByteSequencePredicate::NoNul;
    validate_module(&wrong_predicate).unwrap();
    assert!(
        verify_module(&wrong_predicate, &certificate, &AdmissionProfile::default()).is_err(),
        "NoNul is not UTF-8 evidence"
    );
}

fn out_parameter_store_fixture() -> TerminalModule {
    use terminal_psi::{StructuralArgument, Terminator};
    let mut module = borrowed_backing_fixture();
    let mut callee = fixture().machines.remove(0);
    callee.id = id(2);
    callee.contract.id = id(2);
    callee.entry = id(20);
    callee.blocks[0].id = id(20);
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(20),
        trivial_affine_discards: vec![],
    };
    callee.structural_parameters[0].place = id(20);
    callee.structural_parameters[1].place = id(21);
    callee.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(20),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(21),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
    ];
    let mut replacement = store(20, 21);
    replacement.id = id(20);
    callee.blocks[0].operations = vec![replacement];
    module.machines.push(callee);
    module.machines[0].blocks[0].operations[2].kind = OperationKind::CallUnit {
        callee: id(2),
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![
            StructuralArgument {
                place: id(1),
                path: vec![],
                access: StructuralAccess::MutableBorrow,
            },
            StructuralArgument {
                place: id(3),
                path: vec![],
                access: StructuralAccess::SharedBorrow,
            },
        ],
        claim_transfers: vec![],
        requirement_obligations: vec![],
        crash_continuations: vec![],
    };
    module
}

#[test]
fn normal_unit_call_installs_the_caller_field_loan() {
    let mut module = out_parameter_store_fixture();
    validate_module(&module)
        .expect("the caller reads the descriptor installed by an ordinary Unit call");
    module.machines[0].blocks[0]
        .operations
        .splice(3..3, backing_write());
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "{:?}",
        validate_module(&module)
    );
}

#[test]
fn out_parameter_store_rejects_callee_local_backing() {
    let mut module = out_parameter_store_fixture();
    let callee = &mut module.machines[1];
    let (backing, backing_result) = view_result(30, 30, 3);
    let (view, view_result) = view_result(31, 31, 2);
    callee.structural_places.extend([backing, view]);
    let mut replacement = store(20, 31);
    replacement.id = id(20);
    callee.blocks[0].operations = vec![
        Operation {
            id: id(30),
            static_reach_binding: None,
            suspension_crossing: None,
            result: backing_result,
            kind: OperationKind::EstablishZeroedStructural {},
        },
        Operation {
            id: id(31),
            static_reach_binding: None,
            suspension_crossing: None,
            result: view_result,
            kind: OperationKind::EstablishElementView {
                region: terminal_psi::ElementViewRegion::WholeCollection,
                destination: id(31),
                source: terminal_psi::StructuralArgument {
                    place: id(30),
                    path: vec![],
                    access: StructuralAccess::SharedBorrow,
                },
                element: id(4),
            },
        },
        replacement,
    ];
    assert!(
        matches!(validate_module(&module), Err(ModuleError::InvalidStructuralByteViewFieldStore(operation)) if operation == id(20)),
        "{:?}",
        validate_module(&module)
    );
}

#[test]
fn replacing_one_stored_field_preserves_its_siblings_loan() {
    let mut module = borrowed_backing_fixture();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    let mut sibling = fields[0].clone();
    sibling.id = id(2);
    sibling.identity = "sibling".into();
    fields.push(sibling);
    let mut store_sibling = store(1, 3);
    store_sibling.id = id(8);
    let OperationKind::StructuralByteViewFieldStore { field, .. } = &mut store_sibling.kind else {
        unreachable!()
    };
    *field = id(2);
    let mut replacement = store(1, 2);
    replacement.id = id(7);
    module.machines[0].blocks[0]
        .operations
        .splice(3..3, [store_sibling, replacement]);
    let OperationKind::StructuralByteSequenceFieldLength { field, .. } =
        &mut module.machines[0].blocks[0].operations[5].kind
    else {
        unreachable!()
    };
    *field = id(2);
    validate_module(&module).expect("descriptor siblings preserve separate captured slots");
    module.machines[0].blocks[0]
        .operations
        .splice(5..5, backing_write());
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "{:?}",
        validate_module(&module)
    );
}

#[test]
fn repeated_backing_establishment_cannot_overwrite_a_stored_live_view() {
    use terminal_psi::{Block, Terminator};
    let mut module = borrowed_backing_fixture();
    let jump = |edge| Terminator::Jump {
        edge: id(edge),
        target: id(2),
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![],
        residual_affine_discards: vec![],
        trivial_affine_discards: vec![],
    };
    let machine = &mut module.machines[0];
    machine.structural_parameters.pop();
    let (backing, result) = view_result(4, 10, 3);
    *machine
        .structural_places
        .iter_mut()
        .find(|place| place.id == id(4))
        .unwrap() = backing;
    let mut operations = std::mem::take(&mut machine.blocks[0].operations);
    let literal = operations.remove(0);
    let view = operations.remove(0);
    let stored = operations.remove(0);
    let observation = operations.remove(0);
    machine.blocks[0].operations = vec![literal];
    machine.blocks[0].terminator = jump(1);
    machine.blocks.push(Block {
        id: id(2),
        parameters: vec![],
        erased_scalar_formals: vec![],
        erased_proof_formals: vec![],
        structural_parameters: vec![],
        operations: vec![
            Operation {
                id: id(10),
                static_reach_binding: None,
                suspension_crossing: None,
                result,
                kind: OperationKind::EstablishZeroedStructural {},
            },
            // On re-entry this observes the previous iteration's stored view.
            observation,
            view,
            stored,
        ],
        terminator: jump(2),
    });
    let mut replaced = module.clone();
    let mut replacement = store(1, 2);
    replacement.id = id(11);
    replaced.machines[0].blocks[1].operations.push(replacement);
    validate_module(&replaced)
        .expect("replacing the sole stored view retires the old backing loan before re-entry");
    assert!(
        matches!(validate_module(&module), Err(ModuleError::BorrowedDescriptorLoanDisturbed { operation, place }) if operation == id(10) && place == id(4)),
        "{:?}",
        validate_module(&module)
    );
}

/// The generic callee copies an incoming descriptor and then writes a separate
/// byte-array formal. Its symbolic relation must be checked against the exact
/// caller slot contents, not inferred from the two formal names or byte types.
fn incoming_slot_call_fixture() -> TerminalModule {
    use terminal_psi::{StructuralArgument, StructuralPathSegment as Path};
    let mut module = out_parameter_store_fixture();
    // Stored fields bind access; a copied standalone descriptor uses the
    // canonical shell-free carrier identity.
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(5),
        identity: "test::StandaloneView".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: None,
        }),
    });
    let caller = &mut module.machines[0];
    let mut output = caller.structural_parameters[1].clone();
    output.place = id(6);
    output.position = 2;
    caller.structural_parameters.push(output);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: id(6),
        kind: StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    let (holder, result) = view_result(5, 5, 1);
    caller.structural_places.push(holder);
    let mut captured = store(5, 3);
    captured.id = id(6);
    caller.blocks[0].operations.splice(
        2..2,
        [
            Operation {
                id: id(5),
                static_reach_binding: None,
                suspension_crossing: None,
                result,
                kind: OperationKind::EstablishStructuralLocal {},
            },
            captured,
        ],
    );
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[4].kind
    else {
        unreachable!()
    };
    structural_arguments[1].place = id(5);
    structural_arguments.push(StructuralArgument {
        place: id(6),
        path: vec![],
        access: StructuralAccess::MutableBorrow,
    });

    let callee = &mut module.machines[1];
    callee.structural_parameters[1].structural_type = id(1);
    let mut output = callee.structural_parameters[0].clone();
    output.place = id(24);
    output.position = 2;
    output.structural_type = id(3);
    callee.structural_parameters.push(output);
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: id(24),
        kind: StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    let (copied, result) = view_result(22, 22, 5);
    callee.structural_places.push(copied);
    let mut replacement = store(20, 22);
    replacement.id = id(20);
    callee.blocks[0].operations = vec![
        Operation {
            id: id(22),
            static_reach_binding: None,
            suspension_crossing: None,
            result,
            kind: OperationKind::StructuralLeafCopy {
                source: id(21),
                path: vec![Path::Field("label".into())],
            },
        },
        replacement,
        scalar_operation(
            23,
            8,
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(7),
            },
        ),
        Operation {
            id: id(24),
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: id(24),
                path: vec![Path::FixedIndex(0)],
                value: id(23),
            },
        },
    ];
    module
}

#[test]
fn incoming_slot_compatibility_is_discharged_by_exact_actual_backing() {
    let mut module = incoming_slot_call_fixture();
    validate_module(&module).expect("independent actual backing satisfies the generic call");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the complete caller and callee verify together");
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[4].kind
    else {
        unreachable!()
    };
    // Change only the writable actual: the source holder still captures place4.
    // Both arrays have identical types; their exact captured roots decide safety.
    structural_arguments[2].place = id(4);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "{:?}",
        validate_module(&module)
    );
}

#[test]
fn unresolved_incoming_slot_relation_is_not_an_executable_entry_grant() {
    let mut module = incoming_slot_call_fixture();
    validate_module(&module).expect("the original ordinary caller supplies compatible storage");
    module.entry = id(2);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "{:?}",
        validate_module(&module)
    );
}

#[test]
fn incoming_slot_runtime_write_requires_disjoint_enclosing_storage() {
    use terminal_psi::StructuralPathSegment as Path;
    let mut module = incoming_slot_call_fixture();
    let operations = &mut module.machines[1].blocks[0].operations;
    operations.insert(
        3,
        scalar_operation(
            25,
            64,
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(0),
            },
        ),
    );
    let OperationKind::WriteOnlyPrimitiveStore { path, .. } = &mut operations[4].kind else {
        unreachable!()
    };
    *path = vec![Path::RuntimeIndex {
        index: id(25),
        obligation: id(25),
    }];
    validate_module(&module).expect("a dynamic write into independent storage is compatible");
    let mut external = module.clone();
    external.entry = id(2);
    assert!(matches!(
        validate_module(&external),
        Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
    ));
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[4].kind
    else {
        unreachable!()
    };
    structural_arguments[2].place = id(4);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::BorrowedDescriptorLoanDisturbed { .. })
        ),
        "same backing must fail the enlarged demand: {:?}",
        validate_module(&module)
    );
}

#[test]
fn record_capture_of_narrowed_element_view_requires_establishment() {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::I32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            )),
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::Readings".into(),
            shape: StructuralTypeShape::ElementView { element: id(1) },
        },
        StructuralTypeDeclaration {
            id: id(3),
            identity: "test::Sample".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "readings".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(id(2)),
                }],
            },
        },
    ];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: id(1),
        position: 0,
        is_self: false,
        structural_type: id(2),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    }];
    let (view, view_value) = view_result(2, 4, 2);
    let (record, record_value) = view_result(3, 5, 3);
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        view,
        record,
    ];
    machine.blocks[0].operations = vec![
        scalar_operation(1, 64, OperationKind::ElementViewLength { source: id(1) }),
        scalar_operation(
            2,
            64,
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(1),
            },
        ),
        scalar_operation(
            3,
            64,
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(3),
            },
        ),
        Operation {
            id: id(4),
            static_reach_binding: None,
            suspension_crossing: None,
            result: view_value,
            kind: OperationKind::ElementViewSubslice {
                source: id(1),
                start: id(2),
                end: id(3),
                length: id(1),
                obligation: id(1),
            },
        },
        Operation {
            id: id(5),
            static_reach_binding: None,
            suspension_crossing: None,
            result: record_value,
            kind: OperationKind::EstablishRecord {
                fields: vec![terminal_psi::RecordFieldInitializer {
                    field: id(1),
                    value: terminal_psi::RecordFieldValue::Structural(
                        terminal_psi::StructuralArgument {
                            place: id(2),
                            path: vec![],
                            access: StructuralAccess::Owned,
                        },
                    ),
                }],
            },
        },
    ];
    // This gate checks formation and receiving custody; the range's bounds
    // remain a separate proof obligation, just as for direct view formation.
    validate_module(&module).expect("a formed shared subslice can be captured in a record");
    module.machines[0].blocks[0].operations.swap(3, 4);
    assert!(
        matches!(validate_module(&module),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { operation, place })
                if operation == id(5) && place == id(2)),
        "the same descriptor type and producer cannot authorize a pre-establishment capture: {:?}",
        validate_module(&module),
    );
}
