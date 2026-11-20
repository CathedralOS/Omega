use crate::source_state::TypedTrees;
use crate::source_state::data::{DataDefinition, DataField, DataMember, DataVariant};
use crate::source_state::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use crate::source_symbols::SymbolTableBuilder;
use symbols::{SymbolKind, SymbolNameRef};

#[test]
fn canonical_const_carriers_check_inactive_payloads_and_cycles() {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        crate::source_symbols::builtin_type_symbols()
            .into_iter()
            .chain([
                (SymbolKind::Data, SymbolNameRef::Static("Choice")),
                (SymbolKind::Data, SymbolNameRef::Static("Linear")),
            ]),
    );
    let choice = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
    let linear = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
    let cases = builder.insert_children(
        choice,
        [
            (SymbolKind::Variant, SymbolNameRef::Static("Empty")),
            (SymbolKind::Variant, SymbolNameRef::Static("Value")),
        ],
    );
    let field = builder
        .insert_children(
            cases.handle_at(1),
            [(SymbolKind::Field, SymbolNameRef::Static("value"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let mut named = |symbol, name: &str| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: name.into(),
            })
    };
    let integer = named(
        children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32),
        "u64",
    );
    let choice_type = named(choice, "Choice");
    let linear_type = named(linear, "Linear");
    let payload = program.data_payload_fields.insert_many([DataField {
        symbol: field,
        name: "value".into(),
        type_reference: integer,
        ..Default::default()
    }]);
    let members = program.data_members.insert_many([
        DataMember::Variant(DataVariant {
            symbol: cases.handle_at(0),
            name: "Empty".into(),
            ..Default::default()
        }),
        DataMember::Variant(DataVariant {
            symbol: cases.handle_at(1),
            name: "Value".into(),
            payload,
            ..Default::default()
        }),
    ]);
    program.push_data_definition(DataDefinition {
        symbol: choice,
        name: "Choice".into(),
        members,
        properties: crate::source_state::data::DataProperties {
            multiplicity: language_core::Multiplicity::Unrestricted,
            ..Default::default()
        },
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        symbol: linear,
        name: "Linear".into(),
        ..Default::default()
    });
    assert!(crate::source_state::canonical_const_carrier_is_supported(
        &program,
        choice_type
    ));
    let borrowed = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: integer,
            access: language_core::ReferenceAccess::Shared,
            lifetime: None,
        });
    // Even an Empty value cannot make the other case's reference or
    // linear storage eligible for a canonical constant.
    for ineligible in [borrowed, linear_type] {
        program
            .data_payload_fields
            .get_mut(payload.start())
            .type_reference = ineligible;
        assert!(!crate::source_state::canonical_const_carrier_is_supported(
            &program,
            choice_type
        ));
    }
    program
        .data_payload_fields
        .get_mut(payload.start())
        .type_reference = integer;
    let array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: choice_type,
            length: FixedArrayLength::Literal(3),
        });
    assert!(crate::source_state::canonical_const_carrier_is_supported(
        &program, array
    ));
    program.type_reference_table.substitute_node(
        array,
        TypeReferenceNode::FixedArray {
            element_type: array,
            length: FixedArrayLength::Literal(3),
        },
    );
    assert!(!crate::source_state::canonical_const_carrier_is_supported(
        &program, array
    ));
    assert!(!crate::source_state::canonical_const_carrier_is_supported(
        &program,
        TypeReferenceHandle::invalid()
    ));
}
