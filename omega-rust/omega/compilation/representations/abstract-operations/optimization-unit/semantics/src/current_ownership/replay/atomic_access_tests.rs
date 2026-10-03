//! Isolate current-IR access replay from atomic coherence. These mutations
//! exercise the independent storage authority, availability and loan boundaries.
use super::*;
use crate::tests::support::id;
use crate::unit_validation::references::{
    LiveReference, ReferenceIdentity, ReferenceOrigin, ReferenceParent,
};
use abstract_operations::atomic::AbstractAtomicLocation;
use abstract_operations::{AbstractAtomicEvent, AbstractAtomicReadModifyWrite, AbstractResult};
use language_core::atomic::MemoryOrdering;
use semantic_vocabulary::{
    IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralFieldId,
    StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeShape,
};

fn fixture() -> (PsiOptimizationFunction, StructuralTypeDeclaration, O) {
    let mut function = crate::tests::fixtures::scalar_units::unit()
        .functions
        .remove(0);
    let root = id(90, PlaceId::new);
    let structural_type = id(91, StructuralTypeId::new);
    let field = id(92, StructuralFieldId::new);
    let mut arrival = function.blocks[0].clone();
    arrival.id = id(99, BlockId::new);
    function.structural_places.push(StructuralPlaceDeclaration {
        id: root,
        kind: StructuralPlaceKind::BlockParameter {
            block: arrival.id,
            position: 0,
        },
    });
    arrival
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: root,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    function.blocks.push(arrival);
    let declaration = StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::AtomicStorage".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: "resident".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Atomic(scalar_type()),
            }],
        },
    };
    let store = O::AtomicEvent {
        psi_operation: id(93, OperationId::new),
        event: AbstractAtomicEvent::Store {
            location: AbstractAtomicLocation {
                root,
                path: Vec::new(),
                field,
            },
            ordering: MemoryOrdering::NoOrdering,
            value: id(3, ValueId::new),
        },
        reads_from: None,
        modification_after: Some(abstract_operations::AtomicModificationAfter::InitialResidency),
    };
    (function, declaration, store)
}

fn scalar_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
}

fn empty_frontier() -> CurrentOwnership {
    CurrentOwnership {
        claims: BTreeMap::new(),
        owned_places: BTreeMap::new(),
        partial_custody_paths: BTreeMap::new(),
        live_references: Vec::new(),
    }
}

#[test]
fn atomic_arrivals_preserve_read_and_write_authority() {
    let (mut function, declaration, store) = fixture();
    let types = BTreeMap::from([(declaration.id, &declaration)]);
    let O::AtomicEvent { event, .. } = &store else {
        panic!()
    };
    let location = event.location().unwrap().clone();
    let load = O::AtomicEvent {
        psi_operation: id(94, OperationId::new),
        event: AbstractAtomicEvent::Load {
            location: location.clone(),
            ordering: MemoryOrdering::NoOrdering,
            result: AbstractResult {
                value: id(95, ValueId::new),
                scalar_type: scalar_type(),
            },
        },
        reads_from: Some(abstract_operations::AtomicReadsFrom::InitialResidency),
        modification_after: None,
    };
    let fetch = O::AtomicEvent {
        psi_operation: id(96, OperationId::new),
        event: AbstractAtomicEvent::ReadModifyWrite {
            location,
            ordering: MemoryOrdering::NoOrdering,
            operation: AbstractAtomicReadModifyWrite::FetchAdd,
            operand: id(3, ValueId::new),
            prior: AbstractResult {
                value: id(97, ValueId::new),
                scalar_type: scalar_type(),
            },
        },
        reads_from: Some(abstract_operations::AtomicReadsFrom::InitialResidency),
        modification_after: Some(abstract_operations::AtomicModificationAfter::InitialResidency),
    };
    for (access, store_allowed, load_allowed, fetch_allowed) in [
        (StructuralAccess::MutableBorrow, true, true, true),
        (StructuralAccess::SharedBorrow, true, true, true),
        (StructuralAccess::WriteOnlyBorrow, true, false, false),
    ] {
        function.blocks[1].structural_parameters[0].access = access;
        for (operation, allowed) in [
            (&store, store_allowed),
            (&load, load_allowed),
            (&fetch, fetch_allowed),
        ] {
            assert_eq!(
                validate_atomic_access(
                    &function,
                    &types,
                    function.blocks[1].id,
                    0,
                    operation,
                    &empty_frontier()
                )
                .is_ok(),
                allowed
            );
        }
    }
    function.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let declaration = function.structural_places[0].kind;
    function.structural_places[0].kind = StructuralPlaceKind::Result;
    assert!(
        validate_atomic_access(
            &function,
            &types,
            function.blocks[1].id,
            0,
            &store,
            &empty_frontier(),
        )
        .is_err(),
        "a storage name with the wrong declaration role grants no access"
    );
    function.structural_places[0].kind = declaration;
    function.blocks[1].structural_parameters.clear();
    assert!(
        validate_atomic_access(
            &function,
            &types,
            function.blocks[1].id,
            0,
            &store,
            &empty_frontier()
        )
        .is_err()
    );
}

#[test]
fn ordinary_scalar_identity_cannot_authorize_an_atomic_event() {
    let (mut function, mut declaration, store) = fixture();
    function.blocks[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
        panic!("record fixture")
    };
    fields[0].field_type = StructuralFieldType::Scalar(scalar_type());
    let types = BTreeMap::from([(declaration.id, &declaration)]);
    assert!(
        validate_atomic_access(
            &function,
            &types,
            function.blocks[1].id,
            0,
            &store,
            &empty_frontier(),
        )
        .is_err()
    );
}

#[test]
fn atomic_access_requires_live_whole_owned_storage() {
    let (mut function, declaration, store) = fixture();
    let types = BTreeMap::from([(declaration.id, &declaration)]);
    let parameter = &mut function.blocks[1].structural_parameters[0];
    parameter.access = StructuralAccess::Owned;
    parameter.multiplicity = StructuralMultiplicity::Affine;
    let root = parameter.place;
    let mut frontier = empty_frontier();
    assert!(matches!(
        validate_atomic_access(
            &function,
            &types,
            function.blocks[1].id,
            0,
            &store,
            &frontier
        ),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { .. })
    ));
    frontier
        .owned_places
        .insert(root, StructuralMultiplicity::Affine);
    validate_atomic_access(
        &function,
        &types,
        function.blocks[1].id,
        0,
        &store,
        &frontier,
    )
    .unwrap();
    frontier.partial_custody_paths.insert(
        root,
        BTreeSet::from([vec![StructuralPathSegment::Field("resident".into())]]),
    );
    assert!(matches!(
        validate_atomic_access(
            &function,
            &types,
            function.blocks[1].id,
            0,
            &store,
            &frontier
        ),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { .. })
    ));
}

#[test]
fn atomic_access_cannot_bypass_a_live_storage_loan() {
    let (function, declaration, store) = fixture();
    let types = BTreeMap::from([(declaration.id, &declaration)]);
    let root = function.blocks[1].structural_parameters[0].place;
    let carrier = id(98, PlaceId::new);
    let origin = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
        place: root,
        path: vec![StructuralPathSegment::Field("resident".into())],
    });
    let mut live = vec![LiveReference {
        identity: ReferenceIdentity {
            place: carrier,
            path: Vec::new(),
        },
        carrier,
        carrier_path: Vec::new(),
        root: origin.clone(),
        parent: ReferenceParent::Root(origin),
    }];
    assert!(matches!(
        super::super::references::apply_operation(
            &function,
            &BTreeMap::new(),
            &types,
            function.blocks[1].id,
            0,
            &store,
            &mut live
        ),
        Err(OptimizationUnitValidationError::CurrentReferenceCustodyViolation { .. })
    ));
    live.clear();
    super::super::references::apply_operation(
        &function,
        &BTreeMap::new(),
        &types,
        function.blocks[1].id,
        0,
        &store,
        &mut live,
    )
    .unwrap();
    let writes =
        super::super::storage_regions::written_or_exclusive_regions(&function, &types, &store);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, root);
    assert_eq!(
        writes[0].1.path,
        vec![StructuralPathSegment::Field("resident".into())]
    );
}
