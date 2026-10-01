//! An inline byte field has no standalone structural type identity.
//!
//! A byte field reaches a callee parameter through an inline presentation,
//! which retains the owning root and path. Bounded-owner parameters additionally
//! require the exact field capacity under `structural_access.md`, and a stored
//! shared view field lends its own descriptor only to a shared view formal.
//! No presentation lets a field acquire a same-shaped declaration's identity
//! at a path end and match the parameter directly.
use super::{
    AdmissionProfile, ModuleError, OperationKind, ProofBundle, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule, arguments, buffer_field,
    buffer_module, ordinary_buffer_module, structural_type_id, validate_module, verify_module,
};
use terminal_psi::ByteSequenceCarrier;

/// Every byte carrier a field may declare. `BorrowedView` is the one the
/// module's own `structural_types[0]` declaration initially shares a shape with.
/// `BoundedOwned` can also be declared as a whole owner, but neither carrier
/// gives an inline field a standalone path-end type identity.
const CARRIERS: [ByteSequenceCarrier; 2] = [
    ByteSequenceCarrier::BorrowedView {
        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
    },
    ByteSequenceCarrier::BoundedOwned { capacity: 16 },
];

fn ordinary_call_argument_path(module: &TerminalModule) -> &[StructuralPathSegment] {
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    &structural_arguments[0].path
}

#[test]
fn no_byte_carrier_resolves_to_a_standalone_leaf_shape() {
    for carrier in CARRIERS {
        assert_eq!(
            StructuralFieldType::ByteSequence(carrier).canonical_leaf_shape(),
            None,
            "carrier {carrier:?} must not resolve a path-end type identity"
        );
    }
}

#[test]
fn a_borrowed_view_field_cannot_supply_an_ordinary_view_parameter() {
    let mut module = ordinary_buffer_module();
    buffer_field(&mut module).field_type =
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
    // The presentation route refuses the carrier outright, so the rejection
    // cannot have moved into it.
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    assert_eq!(
        terminal_semantics::boundary_buffer_capacity(
            module.structural_types.iter(),
            module.machines[0].structural_parameters[0].structural_type,
            &structural_arguments[0],
            &module.machines[1].structural_parameters[0],
        ),
        None
    );
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
    assert_eq!(
        ordinary_call_argument_path(&module),
        [StructuralPathSegment::Field("left".into())]
    );
}

#[test]
fn a_borrowed_view_field_lends_only_a_shared_boundary_view() {
    let module_with = |access| {
        let mut module = buffer_module();
        buffer_field(&mut module).field_type =
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            });
        arguments(&mut module)[0].access = access;
        module.boundary_machines[0].structural_parameters[0].access = access;
        module
    };
    // An exclusive loan of a stored shared view would widen its access.
    assert!(
        matches!(
            validate_module(&module_with(StructuralAccess::MutableBorrow)),
            Err(ModuleError::InvalidStructuralArgumentPath { .. })
        ),
        "a mutable boundary loan of a shared view field must reject"
    );
    // A shared loan lends the field's own stored descriptor in place.
    validate_module(&module_with(StructuralAccess::SharedBorrow))
        .expect("a shared boundary loan presents the stored view descriptor");
}

#[test]
fn a_nested_borrowed_view_field_cannot_supply_a_view_parameter() {
    let mut module = buffer_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "BufferContainer".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "owner".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(structural_type_id(2)),
            }],
        },
    });
    module.machines[0].structural_parameters[0].structural_type = structural_type_id(3);
    arguments(&mut module)[0]
        .path
        .insert(0, StructuralPathSegment::Field("owner".into()));
    buffer_field(&mut module).field_type =
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidStructuralArgumentPath { .. })
    ));
}

#[test]
fn a_bounded_owned_field_keeps_every_admitted_presentation() {
    // Positive control: the inline routes the spec does admit still validate.
    // A mutable boundary loan, a shared boundary loan and the ordinary unit
    // subloan each present a bounded-owned field through its owning root.
    let mut module = buffer_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a mutable boundary loan presents the bounded-owned field in place");

    arguments(&mut module)[0].access = StructuralAccess::SharedBorrow;
    module.boundary_machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a shared boundary loan reads the bounded-owned field in place");

    let ordinary = ordinary_buffer_module();
    verify_module(
        &ordinary,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the ordinary unit subloan presents the bounded-owned field in place");
    assert_eq!(
        ordinary_call_argument_path(&ordinary),
        [StructuralPathSegment::Field("left".into())]
    );
}

#[test]
fn a_bounded_owner_parameter_requires_exact_inline_capacity_and_path() {
    let mut module = ordinary_buffer_module();
    module.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 16 });
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a bounded-owner parameter borrows the exact inline field");
    assert_eq!(
        ordinary_call_argument_path(&module),
        [StructuralPathSegment::Field("left".into())]
    );

    for capacity in [15, 17] {
        let mut wrong_capacity = module.clone();
        wrong_capacity.structural_types[0].shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity });
        assert!(
            matches!(
                validate_module(&wrong_capacity),
                Err(ModuleError::InvalidStructuralArgumentPath { .. })
            ),
            "neither a narrower nor a wider owner may substitute for the inline field"
        );
    }

    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path.clear();
    let outcome = validate_module(&module).map(|_| ());
    assert!(
        matches!(
            outcome,
            Err(ModuleError::InvalidStructuralArgumentPath { .. })
        ),
        "the enclosing record cannot replace the exact borrowed field: {outcome:?}"
    );
}
