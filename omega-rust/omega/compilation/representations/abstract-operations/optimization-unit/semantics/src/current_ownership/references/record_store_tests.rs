//! Independent loan replay controls. These tests change declarations only to
//! exercise the receiving helpers; they do not claim the mutated whole unit is
//! a verified program. The authored corpus supplies full-pipeline acceptance.

use super::*;
use crate::tests::indexed_byte_fields::indexed_field_unit;
use crate::tests::support::id;
use crate::unit_validation::references::mutable_record_field_destination;
use terminal_psi::{StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape};

/// These helper controls isolate source-contract/live-ledger joins; the authored
/// corpus supplies full-unit acceptance.
#[test]
fn nested_call_borrow_checks_each_descriptor_access_and_lineage() {
    let (mut function, mut declarations, outer) = record_carrier();
    let inner = id(9_010, PlaceId::new);
    let backing = id(9_011, PlaceId::new);
    let mut inner_parameter = function.structural_parameters[0].clone();
    inner_parameter.place = inner;
    inner_parameter.position = function.structural_parameters.len() as u32;
    let inner_type = inner_parameter.structural_type;
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: inner,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: inner_parameter.position,
                is_self: inner_parameter.is_self,
            },
        });
    function.structural_parameters.push(inner_parameter);
    let outer_type = id(9_002, StructuralTypeId::new);
    declarations.push(StructuralTypeDeclaration {
        id: outer_type,
        identity: "test.shared-reference-to-reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: inner_type,
            access: StructuralAccess::SharedBorrow,
        },
    });
    function.structural_parameters[0].structural_type = outer_type;
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let outer_identity = ReferenceIdentity {
        place: outer,
        path: Vec::new(),
    };
    let inner_identity = ReferenceIdentity {
        place: inner,
        path: Vec::new(),
    };
    let live = vec![
        LiveReference {
            identity: outer_identity,
            carrier: outer,
            carrier_path: Vec::new(),
            root: ReferenceOrigin::Primitive(inner),
            parent: ReferenceParent::Root(ReferenceOrigin::Primitive(inner)),
        },
        LiveReference {
            identity: inner_identity,
            carrier: inner,
            carrier_path: Vec::new(),
            root: ReferenceOrigin::Primitive(backing),
            parent: ReferenceParent::Root(ReferenceOrigin::Primitive(backing)),
        },
    ];
    let argument = StructuralArgument {
        place: outer,
        path: vec![
            StructuralPathSegment::Referent,
            StructuralPathSegment::Referent,
        ],
        access: StructuralAccess::SharedBorrow,
    };
    let check = |live: &[LiveReference], argument: &StructuralArgument| {
        call_reference_custody(&function, &types, function.entry, 0, live, argument)
    };
    let custody = check(&live, &argument).unwrap();
    assert_eq!(custody.origins, vec![live[1].root.clone()]);
    assert!(custody.dependencies.contains(&live[0].root));
    let sibling = StructuralArgument {
        place: inner,
        path: Vec::new(),
        access: StructuralAccess::Owned,
    };
    assert!(
        call_dependency_conflict(&custody.dependencies, &sibling),
        "moving descriptor storage conflicts independently of its contained loans"
    );
    assert!(
        !custody
            .origins
            .iter()
            .any(|origin| origins_overlap(origin, &ReferenceOrigin::Primitive(inner))),
        "this control must not be explained by final-referent overlap"
    );
    let mut unrelated = sibling.clone();
    unrelated.place = id(9_099, PlaceId::new);
    assert!(!call_dependency_conflict(&custody.dependencies, &unrelated));
    assert!(
        check(&live[..1], &argument).is_err(),
        "missing inner descriptor"
    );
    assert!(
        check(&live[1..], &argument).is_err(),
        "released outer descriptor"
    );
    let mut direct_inner = StructuralArgument {
        place: inner,
        path: vec![StructuralPathSegment::Referent],
        access: StructuralAccess::MutableBorrow,
    };
    assert!(
        check(&live, &direct_inner).is_err(),
        "direct mutable use must respect enclosing shared descriptor storage"
    );
    assert!(
        check(&live[1..], &direct_inner).is_ok(),
        "ending the enclosing storage loan restores direct mutable use"
    );
    direct_inner.access = StructuralAccess::SharedBorrow;
    assert!(
        check(&live, &direct_inner).is_ok(),
        "shared observation remains compatible with enclosing shared storage"
    );
    let mut changed = argument.clone();
    changed.access = StructuralAccess::MutableBorrow;
    assert!(
        check(&live, &changed).is_err(),
        "shared intermediate cannot grant mutation"
    );
    changed = argument.clone();
    changed.path.push(StructuralPathSegment::Referent);
    assert!(
        check(&live, &changed).is_err(),
        "forged crossing of non-reference storage"
    );
    changed = argument.clone();
    changed
        .path
        .insert(1, StructuralPathSegment::Field("absent".into()));
    assert!(check(&live, &changed).is_err(), "forged descriptor field");
    let mut opaque = live.clone();
    opaque[0].root = ReferenceOrigin::IngressLeaf(opaque[0].identity.clone());
    assert!(
        check(&opaque, &argument).is_err(),
        "opaque referent cannot create a leaf"
    );
    let mut suspended = live.clone();
    let mut child = live[1].clone();
    child.identity.place = id(9_012, PlaceId::new);
    child.carrier = child.identity.place;
    child.parent = ReferenceParent::Reference(live[1].identity.clone());
    suspended.push(child);
    assert!(
        check(&suspended, &argument).is_err(),
        "inner permission is suspended"
    );
    let mut alternatives = live.clone();
    alternatives[0].root = ReferenceOrigin::Alternatives {
        place: inner,
        paths: vec![
            Vec::new(),
            vec![StructuralPathSegment::Field("absent".into())],
        ],
    };
    assert!(
        check(&alternatives, &argument).is_err(),
        "every origin alternative must resolve"
    );
}

fn record_carrier() -> (
    PsiOptimizationFunction,
    Vec<StructuralTypeDeclaration>,
    PlaceId,
) {
    let unit = indexed_field_unit();
    let mut function = unit
        .functions
        .into_iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    let parameter = function.structural_parameters.first_mut().unwrap();
    let mut types = unit.structural_types.to_vec();
    let carrier = id(9_001, StructuralTypeId::new);
    types.push(StructuralTypeDeclaration {
        id: carrier,
        identity: "test.mutable-record-reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: parameter.structural_type,
            access: StructuralAccess::MutableBorrow,
        },
    });
    parameter.structural_type = carrier;
    parameter.access = StructuralAccess::Owned;
    parameter.multiplicity = StructuralMultiplicity::Affine;
    let place = parameter.place;
    (function, types, place)
}

#[test]
fn record_store_requires_live_unsuspended_mutable_carrier() {
    let (function, mut declarations, place) = record_carrier();
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let mut live = entry_references(&function, &types).unwrap();
    let path = [StructuralPathSegment::Referent];
    let check = |types: &BTreeMap<_, _>, live: &[LiveReference]| {
        check_record_field_access(
            &function,
            types,
            function.entry,
            0,
            live,
            place,
            &path,
            None,
        )
    };
    assert!(check(&types, &live).is_ok());
    assert!(
        check(&types, &[]).is_err(),
        "released or not-yet-established carrier"
    );
    assert!(
        check_record_field_access(
            &function,
            &types,
            function.entry,
            0,
            &live,
            place,
            &[],
            None,
        )
        .is_err(),
        "shell is not owned backing"
    );

    let parent = live[0].identity.clone();
    let mut child = live[0].clone();
    child.identity.place = id(9_002, PlaceId::new);
    child.carrier = child.identity.place;
    child.parent = ReferenceParent::Reference(parent);
    live.push(child);
    assert!(
        check(&types, &live).is_err(),
        "child loan suspends its parent"
    );
    live.pop();
    release(&function, function.entry, Some(0), &mut live, place).unwrap();
    assert!(check(&types, &live).is_err());

    let StructuralTypeShape::Reference { access, .. } = &mut declarations.last_mut().unwrap().shape
    else {
        unreachable!()
    };
    *access = StructuralAccess::SharedBorrow;
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let live = entry_references(&function, &types).unwrap();
    assert!(
        check_record_field_access(
            &function,
            &types,
            function.entry,
            0,
            &live,
            place,
            &path,
            None,
        )
        .is_err(),
        "shared reference cannot write"
    );
}

#[test]
fn reference_window_exempts_only_its_active_parent_carrier() {
    let (function, mut declarations, place) = record_carrier();
    let parameter = &function.structural_parameters[0];
    let StructuralTypeShape::Reference { referent, .. } = declarations
        .iter()
        .find(|row| row.id == parameter.structural_type)
        .unwrap()
        .shape
    else {
        unreachable!()
    };
    let child = id(9_032, StructuralTypeId::new);
    let StructuralTypeShape::Record { fields } = &mut declarations
        .iter_mut()
        .find(|row| row.id == referent)
        .unwrap()
        .shape
    else {
        unreachable!()
    };
    // A restoration window moves a structural subtree, never the fixture's
    // original byte field. Keep the loan test on a legal window geometry.
    fields[0].field_type = terminal_psi::StructuralFieldType::Structural(child);
    let field = fields.first().expect("record field").id;
    declarations.push(StructuralTypeDeclaration {
        id: child,
        identity: "test.window-child".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let types: BTreeMap<_, _> = declarations.iter().map(|row| (row.id, row)).collect();
    let root = abstract_operations::StructuralWindowRoot::OwnedResult {
        producer: id(9_030, semantic_vocabulary::OperationId::new),
        result: terminal_psi::StructuralOperationResult {
            place,
            structural_type: parameter.structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
    };
    let path = [StructuralPathSegment::Referent];
    let mut live = entry_references(&function, &types).unwrap();
    let check = |live: &[LiveReference]| {
        check_window_access(
            &function,
            &types,
            function.entry,
            0,
            live,
            &root,
            &path,
            Some(field),
        )
    };
    assert!(
        check(&live).is_ok(),
        "the carrier is not inside its referent field"
    );
    assert!(
        check(&[]).is_err(),
        "released carrier cannot open or restore a window"
    );
    let mut child = live[0].clone();
    child.identity.place = id(9_031, PlaceId::new);
    child.carrier = child.identity.place;
    child.parent = ReferenceParent::Reference(live[0].identity.clone());
    live.push(child);
    assert!(check(&live).is_err(), "a child still suspends the parent");
    live.pop();
    live.push(live[0].clone());
    assert!(
        check(&live).is_err(),
        "ambiguous parent identity grants no exemption"
    );
    live.pop();
    assert!(
        check_window_access(
            &function,
            &types,
            function.entry,
            0,
            &live,
            &root,
            &[],
            Some(field),
        )
        .is_err(),
        "carrier storage is not its referent"
    );
}

#[test]
fn record_store_geometry_rejects_non_affine_and_nested_permission_storage() {
    let (mut function, mut declarations, place) = record_carrier();
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let path = [StructuralPathSegment::Referent];
    assert!(mutable_record_field_destination(&function, &types, place, &path).is_some());
    function.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
    assert!(mutable_record_field_destination(&function, &types, place, &path).is_none());
    function.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    let carrier = function.structural_parameters[0].structural_type;
    let StructuralTypeShape::Reference { referent, .. } = declarations.last().unwrap().shape else {
        unreachable!()
    };
    let StructuralTypeShape::Record { fields } = &mut declarations
        .iter_mut()
        .find(|row| row.id == referent)
        .unwrap()
        .shape
    else {
        unreachable!()
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::Structural(carrier);
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    assert!(
        mutable_record_field_destination(&function, &types, place, &path).is_none(),
        "nested loan is not plain referent storage"
    );
}

#[test]
fn record_store_byte_capacity_and_predicates_follow_referent_declaration() {
    let (function, declarations, place) = record_carrier();
    let mut unit = indexed_field_unit();
    unit.structural_types = declarations.into();
    let types = unit
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect();
    let mut operation = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| {
            matches!(node.operation, O::StructuralByteSequenceFieldStore { .. })
                .then_some(node.operation.clone())
        })
        .unwrap();
    let O::StructuralByteSequenceFieldStore {
        destination,
        path,
        field,
        ..
    } = &mut operation
    else {
        unreachable!()
    };
    *destination = place;
    *path = vec![StructuralPathSegment::Referent];
    let expected_field = *field;
    assert_eq!(
        crate::unit_validation::function_structure::byte_field_store_capacity(
            &function, &operation, &types
        ),
        Some(3)
    );
    let predicates = crate::unit_validation::function_structure::byte_field_predicates(
        &unit,
        &function,
        place,
        &[StructuralPathSegment::Referent],
        expected_field,
    )
    .unwrap();
    assert!(
        !predicates.is_empty(),
        "UTF-8 declaration remains attached to destination"
    );
    let O::StructuralByteSequenceFieldStore { field, .. } = &mut operation else {
        unreachable!()
    };
    *field = id(9_100, semantic_vocabulary::StructuralFieldId::new);
    let unknown_field = *field;
    assert_eq!(
        crate::unit_validation::function_structure::byte_field_store_capacity(
            &function, &operation, &types
        ),
        None
    );
    assert!(
        crate::unit_validation::function_structure::byte_field_predicates(
            &unit,
            &function,
            place,
            &[StructuralPathSegment::Referent],
            unknown_field
        )
        .is_none()
    );
}

#[test]
fn projected_record_origin_preserves_fixed_index_and_rejects_overlap() {
    let unit = indexed_field_unit();
    let mut function = unit
        .functions
        .into_iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    let mut declarations = unit.structural_types.to_vec();
    let parameter = &mut function.structural_parameters[0];
    let record = parameter.structural_type;
    let array = id(9_010, StructuralTypeId::new);
    let bag = id(9_011, StructuralTypeId::new);
    let StructuralTypeShape::Record { fields } = &declarations
        .iter()
        .find(|row| row.id == record)
        .unwrap()
        .shape
    else {
        unreachable!()
    };
    let mut field = fields[0].clone();
    field.identity = "Bag.holders".into();
    field.field_type = terminal_psi::StructuralFieldType::Structural(array);
    declarations.push(StructuralTypeDeclaration {
        id: array,
        identity: "Holder[2]".into(),
        shape: StructuralTypeShape::FixedArray {
            element: record,
            length: 2,
        },
    });
    declarations.push(StructuralTypeDeclaration {
        id: bag,
        identity: "Bag".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![field],
        },
    });
    parameter.structural_type = bag;
    let source = StructuralArgument {
        place: parameter.place,
        path: vec![
            StructuralPathSegment::Field("Bag.holders".into()),
            StructuralPathSegment::FixedIndex(1),
        ],
        access: StructuralAccess::MutableBorrow,
    };
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    assert_eq!(
        reference_source_type(&function, &types, &source),
        Some(record)
    );
    let (origin, parent) =
        normalized_source(&function, &types, function.entry, 0, &[], &source).unwrap();
    assert_eq!(
        origin,
        ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: source.path.clone()
        })
    );
    assert_eq!(formal_origin(&function, &source), Some(origin.clone()));
    let identity = ReferenceIdentity {
        place: id(9_012, PlaceId::new),
        path: vec![],
    };
    let live = vec![LiveReference {
        carrier: identity.place,
        carrier_path: vec![],
        identity,
        root: origin,
        parent,
    }];
    assert!(normalized_source(&function, &types, function.entry, 0, &live, &source).is_err());
    let mut sibling = source.clone();
    sibling.path[1] = StructuralPathSegment::FixedIndex(0);
    let (sibling_origin, _) =
        normalized_source(&function, &types, function.entry, 0, &live, &sibling).unwrap();
    assert_ne!(sibling_origin, live[0].root);
    let mut outside = source;
    outside.path[1] = StructuralPathSegment::FixedIndex(2);
    assert_eq!(reference_source_type(&function, &types, &outside), None);
    assert!(normalized_source(&function, &types, function.entry, 0, &live, &outside).is_err());
}
