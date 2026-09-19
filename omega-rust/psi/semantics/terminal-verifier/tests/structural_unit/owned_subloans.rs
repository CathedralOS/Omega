//! Owned parameter subloans retain exact referents without widening presentations or custody.

use super::write_only_attenuation::{
    call_arguments, give_receiver_call_scalar_result, record_receiver_module,
};
use super::{
    AdmissionProfile, ModuleError, ProofBundle, StructuralAccess, StructuralDomainDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, TerminalModule, domain_id, operation_id, place_id,
    structural_type_id, validate_module, verify_module,
};
use terminal_psi::ByteSequenceCarrier;

fn owned_receiver(path: &[StructuralPathSegment], access: StructuralAccess) -> TerminalModule {
    let mut module = record_receiver_module(path);
    module.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    module.machines[1].structural_parameters[0].access = access;
    call_arguments(&mut module)[0].access = access;
    module
}

fn verifies(module: &TerminalModule) {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the exact owned parameter subloan verifies independently");
}

fn invalid_path() -> ModuleError {
    ModuleError::InvalidStructuralArgumentPath {
        operation: operation_id(1),
        argument_index: 0,
    }
}

#[test]
fn owned_record_subloans_verify_for_unit_and_scalar_results() {
    for path in [
        vec!["record".into()],
        vec!["records".into(), StructuralPathSegment::FixedIndex(1)],
        vec![
            "outer".into(),
            StructuralPathSegment::FixedIndex(1),
            "inner".into(),
            StructuralPathSegment::FixedIndex(0),
            "record".into(),
        ],
    ] {
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut module = owned_receiver(&path, access);
            verifies(&module);
            give_receiver_call_scalar_result(&mut module);
            verifies(&module);
        }
    }
}

#[test]
fn owned_inline_byte_fields_do_not_inherit_borrowed_view_presentation() {
    let mut module = owned_receiver(&["record".into()], StructuralAccess::MutableBorrow);
    // The inline field has no standalone structural identity. Only the
    // borrowed-parent presentation can bridge it to this view parameter.
    module.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        panic!("the receiver's immediate container is a record")
    };
    fields[0].field_type =
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 16 });
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    verifies(&module);

    module.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    assert_eq!(validate_module(&module).unwrap_err(), invalid_path());
    module.machines[1].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    call_arguments(&mut module)[0].access = StructuralAccess::WriteOnlyBorrow;
    assert_eq!(validate_module(&module).unwrap_err(), invalid_path());
}

#[test]
fn owned_subloans_require_the_exact_nominal_referent() {
    let mut module = owned_receiver(&["record".into()], StructuralAccess::MutableBorrow);
    verifies(&module);
    // A second declaration with identical fields still has distinct identity.
    module.structural_types[1].shape = module.structural_types[0].shape.clone();
    module.machines[1].attachment = Some(structural_type_id(2));
    module.machines[1].structural_parameters[0].structural_type = structural_type_id(2);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::StructuralArgumentTypeMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: structural_type_id(2),
            actual: structural_type_id(1),
        }
    );
}

#[test]
fn owned_subloans_resolve_every_field_and_index() {
    let path = vec![
        "outer".into(),
        StructuralPathSegment::FixedIndex(1),
        "inner".into(),
        StructuralPathSegment::FixedIndex(0),
    ];
    let mut module = owned_receiver(&path, StructuralAccess::WriteOnlyBorrow);
    // Scalar-result calls reach argument resolution directly, so a failed
    // predicate cannot mask the path error behind Unit-call slice admission.
    give_receiver_call_scalar_result(&mut module);
    verifies(&module);
    for (position, segment) in path.iter().enumerate() {
        let mut changed = module.clone();
        call_arguments(&mut changed)[0].path[position] = match segment {
            StructuralPathSegment::Field(_) => "absent".into(),
            StructuralPathSegment::FixedIndex(_) => StructuralPathSegment::FixedIndex(2),
            StructuralPathSegment::Referent => panic!("the fixture has only static projections"),
        };
        assert_eq!(validate_module(&changed).unwrap_err(), invalid_path());
    }
}

#[test]
fn owned_indexed_write_only_subloans_keep_the_material_type_check() {
    let mut module = owned_receiver(
        &["records".into(), StructuralPathSegment::FixedIndex(0)],
        StructuralAccess::MutableBorrow,
    );
    give_receiver_call_scalar_result(&mut module);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("the receiver is a record")
    };
    fields[0].field_type = StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView);
    // This is a valid exact record subloan, not an inline-view presentation.
    // Write-only indexed admission additionally requires a material root.
    verifies(&module);
    module.machines[1].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    call_arguments(&mut module)[0].access = StructuralAccess::WriteOnlyBorrow;
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::StructuralArgumentMultiplicityMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: StructuralMultiplicity::Unrestricted,
            actual: StructuralMultiplicity::Linear,
        }
    );
}

#[test]
fn owned_subloans_do_not_reclassify_restricted_multiplicity_or_owned_transfers() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let module = owned_receiver(&["record".into()], access);
        verifies(&module);
        for parameter_owner in [0, 1] {
            let mut changed = module.clone();
            changed.machines[parameter_owner].structural_parameters[0].multiplicity =
                StructuralMultiplicity::Affine;
            assert_eq!(
                validate_module(&changed).unwrap_err(),
                ModuleError::StructuralArgumentMultiplicityMismatch {
                    operation: operation_id(1),
                    argument_index: 0,
                    expected: if parameter_owner == 0 {
                        StructuralMultiplicity::Unrestricted
                    } else {
                        StructuralMultiplicity::Affine
                    },
                    actual: StructuralMultiplicity::Linear,
                }
            );
        }
    }
    let mut module = owned_receiver(&["record".into()], StructuralAccess::Owned);
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::StructuralArgumentMultiplicityMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: StructuralMultiplicity::Unrestricted,
            actual: StructuralMultiplicity::Linear,
        }
    );
    call_arguments(&mut module)[0].access = StructuralAccess::MutableBorrow;
    module.machines[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    verifies(&module);
}

#[test]
fn owned_subloan_admission_does_not_widen_other_access_pairs() {
    let module = owned_receiver(&["record".into()], StructuralAccess::MutableBorrow);
    verifies(&module);
    let mut mismatched = module.clone();
    call_arguments(&mut mismatched)[0].access = StructuralAccess::WriteOnlyBorrow;
    assert_eq!(
        validate_module(&mismatched).unwrap_err(),
        ModuleError::StructuralArgumentAccessMismatch {
            operation: operation_id(1),
            argument_index: 0,
            expected: StructuralAccess::MutableBorrow,
            actual: StructuralAccess::WriteOnlyBorrow,
        }
    );
    for (source, presented) in [
        (
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ),
        (
            StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ),
        (
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::MutableBorrow,
        ),
    ] {
        let mut changed = module.clone();
        changed.machines[0].structural_parameters[0].access = source;
        changed.machines[1].structural_parameters[0].access = presented;
        call_arguments(&mut changed)[0].access = presented;
        assert_eq!(
            validate_module(&changed).unwrap_err(),
            ModuleError::StructuralArgumentAccessExceedsSource {
                operation: operation_id(1),
                argument_index: 0,
                source,
                presented,
            }
        );
    }
}

#[test]
fn owned_subloans_keep_disjoint_siblings_and_reject_overlapping_arguments() {
    let mut module = owned_receiver(
        &["records".into(), StructuralPathSegment::FixedIndex(0)],
        StructuralAccess::MutableBorrow,
    );
    let callee = &mut module.machines[1];
    let mut parameter = callee.structural_parameters[0].clone();
    parameter.position = 1;
    parameter.is_self = false;
    parameter.place = place_id(3);
    callee.structural_parameters.push(parameter);
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let mut sibling = call_arguments(&mut module)[0].clone();
    sibling.path[1] = StructuralPathSegment::FixedIndex(1);
    call_arguments(&mut module).push(sibling);
    verifies(&module);

    let overlap = ModuleError::OverlappingExclusiveStructuralArguments {
        operation: operation_id(1),
        first_argument: 0,
        second_argument: 1,
    };
    for (left, right) in [
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::MutableBorrow,
        ),
        (
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ),
        (
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ),
    ] {
        let mut changed = module.clone();
        for (position, access) in [left, right].into_iter().enumerate() {
            changed.machines[1].structural_parameters[position].access = access;
            call_arguments(&mut changed)[position].access = access;
        }
        verifies(&changed);
        call_arguments(&mut changed)[1].path = call_arguments(&mut changed)[0].path.clone();
        assert_eq!(validate_module(&changed).unwrap_err(), overlap);
    }
    // The whole parent overlaps its child even though the paths differ.
    call_arguments(&mut module)[1].path.clear();
    module.machines[1].structural_parameters[1].structural_type =
        module.machines[0].structural_parameters[0].structural_type;
    assert_eq!(validate_module(&module).unwrap_err(), overlap);
}

#[test]
fn owned_subloans_preserve_qualification_checks() {
    let mut module = owned_receiver(&["record".into()], StructuralAccess::MutableBorrow);
    verifies(&module);
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain_id(1),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "Record::Ready".into(),
        carrier: structural_type_id(1),
        content_projection: None,
    });
    module.machines[1].structural_parameters[0].qualifications = vec![domain_id(1)];
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::StructuralArgumentMissingQualification {
            operation: operation_id(1),
            argument_index: 0,
            domain: domain_id(1),
        }
    );
    module.machines[1].structural_parameters[0]
        .qualifications
        .clear();
    module.structural_domains[0].carrier =
        module.machines[0].structural_parameters[0].structural_type;
    module.machines[0].structural_parameters[0].qualifications = vec![domain_id(1)];
    assert_eq!(validate_module(&module).unwrap_err(), invalid_path());
}
