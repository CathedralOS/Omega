use super::{DeclaredCarrierReference, declared_carrier_references};
use checked_trees::{
    CheckedReferenceOriginProjection as Projection, CheckedStructuralAccess,
    CheckedUnitStructuralPathSegment as Segment,
};
use language_core::ReferenceAccess;
use language_semantics::{DataSupplyMode, Multiplicity};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    data::{DataDefinition, DataField, DataMember, DataProperties, TypeParameter},
    name::Identifier,
    type_identity::TypeIdentityId,
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};

fn array(
    program: &mut TypedTrees,
    element_type: TypeReferenceHandle,
    length: usize,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        })
}

type Row = (
    Vec<Projection>,
    TypeIdentityId,
    CheckedStructuralAccess,
    bool,
);
fn rows(frontier: Option<Vec<DeclaredCarrierReference>>) -> Option<Vec<Row>> {
    frontier.map(|leaves| {
        leaves
            .into_iter()
            .map(|leaf| {
                (
                    leaf.path,
                    leaf.type_identity,
                    leaf.access,
                    leaf.crosses_reference,
                )
            })
            .collect()
    })
}

// The previous array boundary walked the same element declaration anew at
// every index. Retain that N-walk oracle; compare the complete row payload.
fn repeated_elements(
    program: &TypedTrees,
    element: TypeReferenceHandle,
    length: usize,
) -> Option<Vec<DeclaredCarrierReference>> {
    let mut output = Vec::new();
    for index in 0..length {
        for mut leaf in declared_carrier_references(program, element)? {
            leaf.path.insert(
                0,
                Projection::Exact(Segment::FixedIndex(u64::try_from(index).ok()?)),
            );
            output.push(leaf);
        }
    }
    Some(output)
}

#[test]
fn fixed_array_frontier_matches_repeated_element_walks() {
    let mut builder = SymbolTableBuilder::default();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("frontier_test"));
    let declarations = builder
        .insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("EmptyLinear")),
                (SymbolKind::Data, SymbolNameRef::Static("Opaque")),
                (SymbolKind::Data, SymbolNameRef::Static("Envelope")),
            ],
        )
        .handles()
        .collect::<Vec<_>>();
    let parameter = builder
        .insert_children(
            declarations[2],
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice { element_type: unit });
    let mut elements = Vec::new();
    for access in [
        ReferenceAccess::Shared,
        ReferenceAccess::Mutable,
        ReferenceAccess::WriteOnly,
    ] {
        for referee in [unit, slice] {
            elements.push(
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime: Some(Identifier::from("loan")),
                    }),
            );
        }
    }
    for (symbol, name, supply_mode) in [
        (declarations[0], "EmptyLinear", DataSupplyMode::CheckedShape),
        (declarations[1], "Opaque", DataSupplyMode::BoundaryOpaque),
    ] {
        program.push_data_definition(DataDefinition {
            symbol,
            name: Identifier::from(name),
            supply_mode,
            properties: DataProperties {
                multiplicity: Multiplicity::Linear,
                ..DataProperties::default()
            },
            ..DataDefinition::default()
        });
        elements.push(
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::from(name),
                }),
        );
    }
    let parameter_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: parameter,
            name: Identifier::from("T"),
        });
    let nested = array(&mut program, parameter_type, 3);
    let parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: parameter,
        name: Identifier::from("T"),
        ..TypeParameter::default()
    }]);
    let members = program
        .data_members
        .insert_many([DataMember::Field(DataField {
            identity: Some(17),
            name: Identifier::from("items"),
            type_reference: nested,
            ..DataField::default()
        })]);
    program.push_data_definition(DataDefinition {
        symbol: declarations[2],
        name: Identifier::from("Envelope"),
        type_parameters: parameters,
        members,
        ..DataDefinition::default()
    });
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles([elements[0]]);
    let applied = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: declarations[2],
            base_name: Identifier::from("Envelope"),
            lifetime_arguments: Vec::new(),
            arguments,
        });
    elements.push(applied);
    // Nested fields keep the original T environment and nominal namespace.
    let generic = declared_carrier_references(&program, applied).unwrap();
    assert_eq!(generic.len(), 3);
    assert_eq!(
        generic[2].path,
        [
            Projection::Exact(Segment::Field(
                typed_trees::data::DataMemberIdentity::of_member(
                    Some(17),
                    Identifier::from("items"),
                )
            )),
            Projection::Exact(Segment::FixedIndex(2)),
        ]
    );
    let direct = declared_carrier_references(&program, elements[0]).unwrap();
    assert_eq!(generic[2].type_identity, direct[0].type_identity);
    for element in elements {
        for length in 0..=4 {
            let original = rows(repeated_elements(&program, element, length));
            let reference = array(&mut program, element, length);
            assert_eq!(
                rows(declared_carrier_references(&program, reference)),
                original
            );
        }
    }
    // Preflight counts no opaque descriptor, but the real frontier still
    // refuses that element. Only a genuine zero-length array skips its visit.
    let opaque = program
        .type_reference_table
        .find_named_type_reference(declarations[1])
        .unwrap();
    assert!(declared_carrier_references(&program, opaque).is_none());
    let zero = array(&mut program, opaque, 0);
    assert_eq!(
        rows(declared_carrier_references(&program, zero)),
        Some(Vec::new())
    );
    let one = array(&mut program, opaque, 1);
    assert!(declared_carrier_references(&program, one).is_none());
    let excessive = array(&mut program, applied, super::MAX_REFERENCE_ORIGIN_LEAVES);
    assert!(declared_carrier_references(&program, excessive).is_none());
}
