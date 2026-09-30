//! Completed owned snapshots retain inline byte predicates without admitting
//! byte-field construction or turning borrowed descriptors into owned bytes.

use super::*;
use terminal_psi::{ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet};

const SOURCE: u64 = 20;

fn copied_byte_record(nested: bool, qualified: bool) -> TerminalModule {
    let mut module = local_loan_module(
        vec![loan(&[], StructuralAccess::Owned)],
        &[StructuralAccess::Owned],
    );
    let carrier = ByteSequenceCarrier::BoundedOwned { capacity: 16 };
    let byte_field = if qualified {
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        predicates.insert(ByteSequencePredicate::ValidUtf8);
        StructuralFieldType::PredicateByteSequence {
            carrier,
            predicates,
        }
    } else {
        StructuralFieldType::ByteSequence(carrier)
    };
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![field(1, "label", byte_field)],
    };
    if nested {
        let label = module.structural_types[0].shape.clone();
        module.structural_types.extend([
            StructuralTypeDeclaration {
                id: id(3),
                identity: "test::Label".into(),
                shape: label,
            },
            StructuralTypeDeclaration {
                id: id(4),
                identity: "test::Labels".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: id(3),
                    length: 2,
                },
            },
        ]);
        module.structural_types[0].shape = StructuralTypeShape::Record {
            fields: vec![field(1, "labels", StructuralFieldType::Structural(id(4)))],
        };
    }
    let caller = &mut module.machines[0];
    let mut source = formal(SOURCE, 0, StructuralAccess::MutableBorrow);
    source.structural_type = id(SCREEN_TYPE);
    caller.structural_parameters = vec![source];
    caller.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: id(ESTABLISH),
        structural_type: id(BANNER_TYPE),
    };
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: id(SOURCE),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    let copy = &mut caller.blocks[0].operations[0];
    copy.kind = OperationKind::StructuralLeafCopy {
        source: id(SOURCE),
        path: vec![StructuralPathSegment::Field("top".into())],
    };
    let OperationResult::Structural(result) = &mut copy.result else {
        panic!("the fixture establishes a structural result");
    };
    result.structural_type = id(BANNER_TYPE);
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(900),
        trivial_affine_discards: Vec::new(),
    };
    let callee = &mut module.machines[1];
    callee.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(910),
        trivial_affine_discards: Vec::new(),
    };
    module
}

#[test]
fn copied_byte_records_supply_whole_owned_call_arguments() {
    for nested in [false, true] {
        for qualified in [false, true] {
            verify_module(
                &copied_byte_record(nested, qualified),
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .unwrap_or_else(|error| {
                panic!("owned snapshot nested={nested} qualified={qualified}: {error:?}")
            });
        }
    }
}

#[test]
fn copied_byte_records_retain_exact_path_custody_and_dominance() {
    let valid = copied_byte_record(true, true);
    let mut early = valid.clone();
    early.machines[0].blocks[0].operations.swap(0, 1);
    assert!(matches!(
        validate_module(&early),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { .. })
    ));

    let mut wrong_path = valid.clone();
    let OperationKind::StructuralLeafCopy { path, .. } =
        &mut wrong_path.machines[0].blocks[0].operations[0].kind
    else {
        panic!("the fixture copies a projected record");
    };
    path.clear();
    assert!(validate_module(&wrong_path).is_err());

    let mut unreadable = valid.clone();
    unreadable.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    assert!(validate_module(&unreadable).is_err());

    let mut affine = valid.clone();
    let OperationResult::Structural(result) =
        &mut affine.machines[0].blocks[0].operations[0].result
    else {
        panic!("the copy has a structural result");
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    assert!(validate_module(&affine).is_err());

    let mut substituted = valid;
    let mut other = substituted.structural_types[0].clone();
    other.id = id(5);
    other.identity = "test::OtherBanner".into();
    substituted.structural_types.push(other);
    substituted.machines[1].structural_parameters[0].structural_type = id(5);
    assert!(validate_module(&substituted).is_err());
}

#[test]
fn copied_byte_records_do_not_admit_borrowed_fields_or_constructors() {
    let mut borrowed = copied_byte_record(false, true);
    let StructuralTypeShape::Record { fields } = &mut borrowed.structural_types[0].shape else {
        panic!("the snapshot is a record");
    };
    let StructuralFieldType::PredicateByteSequence { carrier, .. } = &mut fields[0].field_type
    else {
        panic!("the record retains a qualified byte field");
    };
    *carrier = ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    };
    assert!(validate_module(&borrowed).is_err());

    let mut constructed = copied_byte_record(false, true);
    constructed.machines[0].blocks[0].operations[0].kind =
        OperationKind::EstablishRecord { fields: Vec::new() };
    assert!(matches!(
        validate_module(&constructed),
        Err(ModuleError::RecordResultMismatch(_))
    ));
}
